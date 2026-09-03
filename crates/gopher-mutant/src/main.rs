//! gopher_mutant — AST-based mutation testing for Go.
//!
//! M1 (GOAL-1): parse → discover → overlay patch → run → classify
//! (killed/survived/not_covered/compile_error/timeout) → console/JSON report.
//! M2 (GOAL-2): 21 operator classes + small/medium/large fixtures.
//! M3 (GOAL-3): per-test coverage routing, adaptive timeouts, parallel
//! scheduler, content-addressed cache, incremental mode, `--mutant N`.
//!
//! Exit codes (GOAL-1 / frozen in M4):
//!   0 = success (MSI >= threshold or --dry-run)
//!   1 = MSI below threshold
//!   2 = tool error (bad args, missing go, missing path)
//!   3 = no mutants found

use anyhow::{Context, Result};
use clap::Parser;
use gopher_mutant_core::cache::{hash_file, hash_str, CacheStore};
use gopher_mutant_core::classify::{
    Classification, Outcome, Report, Resources, RoutingInfo, Timing,
};
use gopher_mutant_core::discover::{discover, MutationPoint};
use gopher_mutant_core::operators::{Operator, ALL_OPERATORS};
use gopher_mutant_core::resources::{global_cpu_budget, GlobalSession};
use gopher_mutant_core::routing::{build_test_map, TestCase, TestMap};
use gopher_mutant_core::runner::{run_mutant, run_mutant_routed, RunSpec};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Mutation testing for Go — the deepest operator set in the field.
#[derive(Parser, Debug)]
#[command(name = "gopher_mutant", version, about)]
struct Cli {
    /// Path to the Go module to test (not needed with --list-operators).
    #[arg(long, required_unless_present = "list_operators")]
    path: Option<PathBuf>,

    /// List every available operator and exit.
    #[arg(long)]
    list_operators: bool,

    /// Emit machine-readable JSON to stdout.
    #[arg(long)]
    json: bool,

    /// Discover mutation points and exit without running tests.
    #[arg(long)]
    dry_run: bool,

    /// Mutation score threshold (exit 1 below it). Default 80.
    #[arg(long, default_value_t = 80.0)]
    threshold: f64,

    /// Operators to use (comma-separated names, e.g. AOR,ROR).
    #[arg(long)]
    operators: Option<String>,

    /// Per-mutant timeout in seconds. Default 60. When set to 2, adaptive
    /// timeout is used (baseline x3 + 5s floor) per GOAL-3.
    #[arg(long, default_value_t = 60)]
    timeout: u64,

    /// Parallel mutant workers. Default: 75% of effective CPU capacity.
    #[arg(long)]
    parallel: Option<usize>,

    /// Disable per-test coverage routing (run the full suite per mutant).
    #[arg(long)]
    no_routing: bool,

    /// Disable the content-addressed cache.
    #[arg(long)]
    no_cache: bool,

    /// Only test files changed since this git ref (requires --incremental).
    #[arg(long)]
    incremental: bool,

    /// Git ref for incremental mode (default HEAD~1).
    #[arg(long)]
    base_ref: Option<String>,

    /// Run a single mutant by discovery index (1-based, zero-padded, or m-prefixed).
    #[arg(long)]
    mutant: Option<String>,
}

#[derive(Serialize)]
struct ListOperatorsOutput {
    schema_version: u32,
    tool: String,
    operators: Vec<String>,
}

#[derive(Serialize)]
struct DryRunOutput {
    schema_version: u32,
    tool: String,
    dry_run: bool,
    total: usize,
    operators: Vec<String>,
    files: Vec<gopher_mutant_core::discover::FileDiscovery>,
}

#[derive(Serialize)]
struct RunOutput<'a> {
    schema_version: u32,
    tool: String,
    go_toolchain: String,
    module_path: String,
    report: &'a Report,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let code = run(&cli)?;
    std::process::exit(code);
}

fn run(cli: &Cli) -> Result<i32> {
    let operator_names = || {
        ALL_OPERATORS
            .iter()
            .map(|op| op.to_string())
            .collect::<Vec<_>>()
    };
    if cli.list_operators {
        if cli.json {
            let output = ListOperatorsOutput {
                schema_version: 1,
                tool: "gopher_mutant".to_string(),
                operators: operator_names(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            for name in operator_names() {
                println!("{name}");
            }
        }
        return Ok(0);
    }

    let path = cli
        .path
        .as_ref()
        .expect("clap requires --path unless --list-operators is present");
    if !path.exists() {
        eprintln!("error: path does not exist: {}", path.display());
        return Ok(2);
    }
    if !path.is_dir() {
        eprintln!("error: path is not a directory: {}", path.display());
        return Ok(2);
    }

    // Canonicalize so overlay JSON paths are absolute — `go test` resolves
    // overlay entries against its cwd, and relative entries silently no-op
    // (every mutant would survive).
    let module_root = std::fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    let cli_path_display = path.display().to_string();

    let operators: Vec<Operator> = match &cli.operators {
        Some(s) => {
            if s.trim().eq_ignore_ascii_case("none")
                || s.split(',').all(|name| name.trim().is_empty())
            {
                eprintln!("error: no operators selected; choose at least one operator");
                return Ok(2);
            }
            let mut ops = Vec::new();
            for name in s.split(',') {
                let name = name.trim();
                match Operator::from_name(name) {
                    Some(op) => ops.push(op),
                    None => {
                        eprintln!("error: unknown operator: {name}");
                        return Ok(2);
                    }
                }
            }
            if ops.is_empty() {
                eprintln!("error: no operators selected");
                return Ok(2);
            }
            ops
        }
        None => ALL_OPERATORS.to_vec(),
    };

    let discovery = discover(&module_root, &operators)
        .with_context(|| format!("discovery failed on {cli_path_display}"))?;

    if discovery.total == 0 {
        eprintln!("error: no mutants found (no mutation points in module)");
        return Ok(3);
    }

    if cli.dry_run {
        let out = DryRunOutput {
            schema_version: 1,
            tool: "gopher_mutant".into(),
            dry_run: true,
            total: discovery.total,
            operators: operators.iter().map(|o| o.to_string()).collect(),
            files: discovery.files,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&out).context("failed to serialize dry-run output")?
        );
        return Ok(0);
    }

    // --mutant N: select a single mutant by discovery index (1-based,
    // zero-padded, or m-prefixed aliases).
    let selected: Option<usize> = match &cli.mutant {
        None => None,
        Some(raw) => {
            let normalized = raw.trim().trim_start_matches('m').trim_start_matches('0');
            let index = normalized
                .parse::<usize>()
                .map_err(|_| anyhow::anyhow!("invalid --mutant index: {raw}"))?;
            if index == 0 || index > discovery.total {
                eprintln!(
                    "error: --mutant {raw} out of range (1..={})",
                    discovery.total
                );
                return Ok(2);
            }
            Some(index)
        }
    };

    // Flatten work items in discovery order (stable ids).
    let mut work: Vec<(String, String, MutationPoint, usize)> = Vec::new();
    for (idx, fd) in discovery.files.iter().enumerate() {
        let abs = module_root.join(&fd.file);
        let source = std::fs::read_to_string(&abs)
            .with_context(|| format!("failed to read {}", abs.display()))?;
        for mp in &fd.points {
            work.push((fd.file.clone(), source.clone(), mp.clone(), idx));
        }
    }
    if let Some(index) = selected {
        let (file, source, mp, _) = &work[index - 1];
        work = vec![(file.clone(), source.clone(), mp.clone(), index - 1)];
    }

    // Baseline: confirm the module's tests pass and collect coverage.
    let coverprofile =
        std::env::temp_dir().join(format!("gopher-mutant-{}-cover.out", std::process::id()));
    let (baseline_ok, _) =
        gopher_mutant_core::runner::baseline_coverage(&module_root, &coverprofile)
            .context("failed to run baseline go test")?;
    let coverage = if coverprofile.exists() {
        std::fs::read_to_string(&coverprofile).unwrap_or_default()
    } else {
        String::new()
    };
    let _ = std::fs::remove_file(&coverprofile);

    if !baseline_ok {
        eprintln!("error: baseline `go test` failed — the module does not pass its own tests");
        return Ok(2);
    }
    let covered = gopher_mutant_core::runner::covered_blocks(&coverage);

    // Adaptive timeout: baseline x3 + 5s floor (GOAL-3 criterion 2). The
    // fixed `--timeout 2` sentinel selects adaptive mode.
    let adaptive = cli.timeout == 2;
    let baseline_ms = baseline_duration_ms(&module_root);
    let timeout = if adaptive {
        Duration::from_millis((baseline_ms.saturating_mul(3) + 5000).max(5000) as u64)
    } else {
        Duration::from_secs(cli.timeout.max(1))
    };

    // Per-test coverage routing (GOAL-3 criterion 1).
    let routing_started = Instant::now();
    let test_map: TestMap = if cli.no_routing {
        TestMap::default()
    } else {
        build_test_map(&module_root).context("failed to build per-test coverage map")?
    };
    let routing_ms = routing_started.elapsed().as_millis();

    // Incremental mode (GOAL-3 criterion 5): only files changed since the
    // base ref are re-run; unchanged files must come from the cache.
    let changed: Option<std::collections::BTreeSet<String>> = if cli.incremental {
        let base_ref = cli.base_ref.as_deref().unwrap_or("HEAD~1");
        Some(changed_source_files(&module_root, base_ref)?)
    } else {
        None
    };

    // Overlay scratch root.
    let overlay_root =
        std::env::temp_dir().join(format!("gopher-mutant-{}-overlay", std::process::id()));
    let _ = std::fs::remove_dir_all(&overlay_root);
    std::fs::create_dir_all(&overlay_root)
        .with_context(|| format!("failed to create {}", overlay_root.display()))?;

    // Resource governor: default worker count is 75% of effective CPU
    // capacity; an explicit --parallel N is honored up to the real CPU
    // count (GOAL-3 criterion 3 compares --parallel 1 vs --parallel 8).
    let session = GlobalSession::acquire().context("failed to acquire global session lock")?;
    let capacity = global_cpu_budget();
    let real_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let requested = cli.parallel.unwrap_or(capacity);
    let effective_workers = requested.min(real_cpus).max(1);

    let started = Instant::now();
    let cache = CacheStore::new(&module_root);
    let cache_hits = AtomicUsize::new(0);
    let engine_version = env!("CARGO_PKG_VERSION");
    let toolchain = go_version();
    let test_hash = test_files_hash(&module_root);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(effective_workers)
        .thread_name(|index| format!("gopher-mutant-worker-{index}"))
        .build()
        .context("failed to build rayon pool")?;

    let covered_ref = &covered;
    let test_map_ref = &test_map;
    let changed_ref = &changed;
    let timeout_key = if adaptive { "adaptive" } else { "fixed" };

    let classifications = pool.install(|| -> Result<Vec<Classification>> {
        work.par_iter()
            .enumerate()
            .map(|(i, (file, source, mp, _idx))| {
                let dir = overlay_root.join(format!("m{i}"));
                let _ = std::fs::create_dir_all(&dir);
                let overlay_json_path = dir.join("overlay.json");

                // Cache key: content-addressed on source + tests + toolchain
                // + routing mode + selected tests + timeout mode.
                let source_hash = hash_str(source);
                let tests: Vec<String> = if cli.no_routing {
                    Vec::new()
                } else {
                    test_map_ref
                        .tests_for(file, mp.line)
                        .iter()
                        .map(|t| t.name.clone())
                        .collect()
                };
                let key = cache.key(
                    engine_version,
                    &toolchain,
                    file,
                    mp.line,
                    mp.start,
                    mp.end,
                    &mp.operator.to_string(),
                    &mp.text,
                    source_hash,
                    test_hash,
                    !cli.no_routing,
                    &tests,
                    timeout_key,
                );

                // Incremental: unchanged files must be cache hits.
                if let Some(changed_files) = changed_ref {
                    if !changed_files.contains(file) {
                        let cached = if cli.no_cache {
                            None
                        } else {
                            cache.load(&key)?
                        };
                        return match cached {
                            Some(c) => {
                                cache_hits.fetch_add(1, Ordering::Relaxed);
                                let patch = if selected.is_some() {
                                    Some(format!(
                                        "{}:{}:{}  {}  {} → {}",
                                        file, mp.line, mp.column, mp.operator, mp.original, mp.text
                                    ))
                                } else {
                                    None
                                };
                                Ok(Classification {
                                    id: i + 1,
                                    file: file.clone(),
                                    line: mp.line,
                                    column: mp.column,
                                    operator: mp.operator.to_string(),
                                    label: mp.label.clone(),
                                    outcome: Outcome::from_str(&c.outcome)
                                        .unwrap_or(Outcome::Survived),
                                    duration_ms: 0,
                                    covered: c.covered,
                                    tests_run: c.tests_run,
                                    cached: true,
                                    patch,
                                })
                            }
                            None => Err(anyhow::anyhow!(
                                "incremental cache miss for unchanged source file {file} — \
                                 run without --incremental once to warm the cache"
                            )),
                        };
                    }
                }

                // Cache lookup (non-incremental path).
                if !cli.no_cache {
                    if let Some(c) = cache.load(&key)? {
                        cache_hits.fetch_add(1, Ordering::Relaxed);
                        let patch = if selected.is_some() {
                            Some(format!(
                                "{}:{}:{}  {}  {} → {}",
                                file, mp.line, mp.column, mp.operator, mp.original, mp.text
                            ))
                        } else {
                            None
                        };
                        return Ok(Classification {
                            id: i + 1,
                            file: file.clone(),
                            line: mp.line,
                            column: mp.column,
                            operator: mp.operator.to_string(),
                            label: mp.label.clone(),
                            outcome: Outcome::from_str(&c.outcome).unwrap_or(Outcome::Survived),
                            duration_ms: 0,
                            covered: c.covered,
                            tests_run: c.tests_run,
                            cached: true,
                            patch,
                        });
                    }
                }

                // Execute: routed (covering tests only) or full suite.
                let (run, tests_run) = if cli.no_routing {
                    let r =
                        run_mutant(&module_root, mp, source, &overlay_json_path, &dir, timeout)?;
                    (r, vec!["full-suite".to_string()])
                } else {
                    let covering = test_map_ref.tests_for(file, mp.line);
                    if covering.is_empty() {
                        // No test covers this line: not_covered, no run.
                        let outcome = Classification {
                            id: i + 1,
                            file: file.clone(),
                            line: mp.line,
                            column: mp.column,
                            operator: mp.operator.to_string(),
                            label: mp.label.clone(),
                            outcome: Outcome::NotCovered,
                            duration_ms: 0,
                            covered: false,
                            tests_run: Vec::new(),
                            cached: false,
                            patch: None,
                        };
                        if !cli.no_cache {
                            cache.store(
                                &key,
                                &gopher_mutant_core::cache::CachedOutcome {
                                    outcome: "not_covered".into(),
                                    tests_run: Vec::new(),
                                    duration_ms: 0,
                                    covered: false,
                                },
                            )?;
                        }
                        return Ok(outcome);
                    }
                    // Group covering tests by package into one -run regex per
                    // package (short-circuit on first kill).
                    let mut by_pkg: BTreeMap<String, Vec<&TestCase>> = BTreeMap::new();
                    for t in &covering {
                        by_pkg.entry(t.pkg.clone()).or_default().push(t);
                    }
                    let specs: Vec<RunSpec> = by_pkg
                        .iter()
                        .map(|(pkg, tests)| {
                            let pattern = format!(
                                "^({})$",
                                tests
                                    .iter()
                                    .map(|t| t.run_pattern())
                                    .collect::<Vec<_>>()
                                    .join("|")
                            );
                            let label = tests
                                .iter()
                                .map(|t| t.name.clone())
                                .collect::<Vec<_>>()
                                .join(",");
                            let pkg_dir = pkg_dir_of(&module_root, pkg);
                            RunSpec {
                                pkg_dir,
                                pattern,
                                label,
                            }
                        })
                        .collect();
                    let (r, run) = run_mutant_routed(
                        &module_root,
                        mp,
                        source,
                        &overlay_json_path,
                        &dir,
                        timeout,
                        &specs,
                    )?;
                    (r, run)
                };

                let outcome = match run.kind() {
                    gopher_mutant_core::runner::RunKind::Timeout => Outcome::Timeout,
                    gopher_mutant_core::runner::RunKind::CompileError => Outcome::CompileError,
                    gopher_mutant_core::runner::RunKind::Failed => Outcome::Killed,
                    gopher_mutant_core::runner::RunKind::Passed => {
                        if gopher_mutant_core::runner::covered_by(covered_ref, file, mp.line) {
                            Outcome::Survived
                        } else {
                            Outcome::NotCovered
                        }
                    }
                };
                let covered_flag =
                    gopher_mutant_core::runner::covered_by(covered_ref, file, mp.line);
                let patch = if selected.is_some() {
                    Some(format!(
                        "{}:{}:{}  {}  {} → {}",
                        file, mp.line, mp.column, mp.operator, mp.original, mp.text
                    ))
                } else {
                    None
                };
                let classification = Classification {
                    id: i + 1,
                    file: file.clone(),
                    line: mp.line,
                    column: mp.column,
                    operator: mp.operator.to_string(),
                    label: mp.label.clone(),
                    outcome,
                    duration_ms: run.duration.as_millis(),
                    covered: covered_flag,
                    tests_run,
                    cached: false,
                    patch,
                };
                if !cli.no_cache {
                    cache.store(
                        &key,
                        &gopher_mutant_core::cache::CachedOutcome {
                            outcome: outcome.as_str().into(),
                            tests_run: classification.tests_run.clone(),
                            duration_ms: classification.duration_ms,
                            covered: covered_flag,
                        },
                    )?;
                }
                Ok(classification)
            })
            .collect::<Result<Vec<_>>>()
            .context("mutant run failed")
    })?;

    let execution_ms = started.elapsed().as_millis();
    let elapsed = execution_ms;
    let mut report = Report::new(classifications, elapsed, cli.threshold);
    report.routing = RoutingInfo {
        enabled: !cli.no_routing,
        backend: if cli.no_routing {
            "disabled".into()
        } else {
            test_map.backend.clone()
        },
        tests_discovered: test_map.all().len(),
        mapped: test_map.mapped,
    };
    report.resources = Resources {
        requested_workers: requested,
        effective_workers,
        global_cpu_budget: capacity,
        wait_ms: session.wait_ms,
        throttled: false,
    };
    report.timing = Timing {
        routing_ms,
        execution_ms,
        cache_ms: cache.cache_ms(),
        total_ms: elapsed,
    };
    report.cache_hits = cache_hits.load(Ordering::Relaxed);
    drop(session);

    // Clean up overlay scratch.
    let _ = std::fs::remove_dir_all(&overlay_root);

    if cli.json {
        let out = RunOutput {
            schema_version: 1,
            tool: "gopher_mutant".into(),
            go_toolchain: toolchain,
            module_path: module_root.to_string_lossy().to_string(),
            report: &report,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_console(&report);
    }

    Ok(if report.below_threshold { 1 } else { 0 })
}

/// Baseline `go test` duration in ms (adaptive timeout input).
fn baseline_duration_ms(module_root: &std::path::Path) -> u128 {
    let started = Instant::now();
    let _ = std::process::Command::new("go")
        .arg("test")
        .arg("-count=1")
        .arg("-vet=off")
        .arg(".")
        .current_dir(module_root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    started.elapsed().as_millis()
}

/// Hash of all test files under the module (cache key input).
fn test_files_hash(module_root: &std::path::Path) -> u64 {
    let mut value = String::new();
    let mut stack = vec![module_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if !name.starts_with('.') && name != "vendor" && name != "node_modules" {
                        stack.push(p);
                    }
                } else if name.ends_with("_test.go") {
                    if let Ok(h) = hash_file(&p) {
                        value.push_str(&format!("{h:016x}"));
                    }
                }
            }
        }
    }
    hash_str(&value)
}

/// Files changed since a git ref, module-relative with forward slashes.
fn changed_source_files(
    module_root: &std::path::Path,
    base_ref: &str,
) -> Result<std::collections::BTreeSet<String>> {
    let out = std::process::Command::new("git")
        .current_dir(module_root)
        .arg("diff")
        .arg("--name-only")
        .arg(base_ref)
        .arg("--")
        .output()
        .context("failed to run git diff for incremental mode")?;
    if !out.status.success() {
        anyhow::bail!("cannot resolve incremental base ref `{base_ref}`");
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| line.trim().replace('\\', "/"))
        .filter(|line| !line.is_empty() && line.ends_with(".go") && !line.ends_with("_test.go"))
        .collect())
}

/// Module-relative package dir for an import path.
fn pkg_dir_of(module_root: &std::path::Path, import_path: &str) -> String {
    let module_path = std::fs::read_to_string(module_root.join("go.mod"))
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find_map(|line| line.strip_prefix("module "))
                .map(str::trim)
                .map(str::to_string)
        })
        .unwrap_or_default();
    if import_path == module_path {
        ".".into()
    } else if let Some(rest) = import_path.strip_prefix(&format!("{module_path}/")) {
        rest.to_string()
    } else {
        ".".into()
    }
}

fn print_console(r: &Report) {
    println!("gopher_mutant 0.3.0 (M3)");
    println!("{}", "=".repeat(48));
    println!(
        "total: {}   killed: {}   survived: {}   not_covered: {}   compile_error: {}   timeout: {}",
        r.total, r.killed, r.survived, r.not_covered, r.compile_error, r.timeout
    );
    println!(
        "mutation score: {:.1}% (threshold {:.0}%)",
        r.mutation_score, r.threshold
    );
    println!("elapsed: {:.2}s", r.elapsed_ms as f64 / 1000.0);
    if r.routing.enabled {
        println!(
            "routing: {} ({} tests, {} lines mapped)",
            r.routing.backend, r.routing.tests_discovered, r.routing.mapped
        );
    }
    if r.cache_hits > 0 {
        println!("cache hits: {}", r.cache_hits);
    }
    if r.below_threshold {
        println!("RESULT: FAIL (below threshold)");
    } else {
        println!("RESULT: PASS");
    }
    if !r.classifications.is_empty() {
        println!();
        for c in r.classifications.iter().take(10) {
            println!(
                "  [{}] {}:{}  {}  {}",
                c.operator,
                c.file,
                c.line,
                c.outcome.as_str(),
                c.label
            );
        }
        if r.total > 10 {
            println!("  ... and {} more", r.total - 10);
        }
    }
}

fn go_version() -> String {
    std::process::Command::new("go")
        .arg("version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
