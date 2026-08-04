//! gopher_mutant — AST-based mutation testing for Go.
//!
//! M1 (GOAL-1): parse → discover → overlay patch → run → classify
//! (killed/survived/not_covered/compile_error/timeout) → console/JSON report.
//!
//! Exit codes (GOAL-1 / frozen in M4):
//!   0 = success (MSI >= threshold or --dry-run)
//!   1 = MSI below threshold
//!   2 = tool error (bad args, missing go, missing path)
//!   3 = no mutants found

use anyhow::{Context, Result};
use clap::Parser;
use gopher_mutant_core::classify::{Classification, Outcome, Report};
use gopher_mutant_core::discover::{discover, MutationPoint};
use gopher_mutant_core::operators::{Operator, ALL_OPERATORS};
use gopher_mutant_core::runner::{baseline_coverage, run_mutant};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Mutation testing for Go — the deepest operator set in the field.
#[derive(Parser, Debug)]
#[command(name = "gopher_mutant", version, about)]
struct Cli {
    /// Path to the Go module to test.
    #[arg(long)]
    path: PathBuf,

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

    /// Per-mutant timeout in seconds. Default 60.
    #[arg(long, default_value_t = 60)]
    timeout: u64,
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
    if !cli.path.exists() {
        eprintln!("error: path does not exist: {}", cli.path.display());
        return Ok(2);
    }
    if !cli.path.is_dir() {
        eprintln!("error: path is not a directory: {}", cli.path.display());
        return Ok(2);
    }

    let operators: Vec<Operator> = match &cli.operators {
        Some(s) => {
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

    let discovery = discover(&cli.path, &operators)
        .with_context(|| format!("discovery failed on {}", cli.path.display()))?;

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

    // Baseline: confirm the module's tests pass and collect coverage.
    let coverprofile =
        std::env::temp_dir().join(format!("gopher-mutant-{}-cover.out", std::process::id()));
    let (baseline_ok, _) =
        baseline_coverage(&cli.path, &coverprofile).context("failed to run baseline go test")?;
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
    let covered = gopher_mutant_core::runner::covered_lines(&coverage);

    // Overlay scratch dir.
    let overlay_dir =
        std::env::temp_dir().join(format!("gopher-mutant-{}-overlay", std::process::id()));
    let _ = std::fs::remove_dir_all(&overlay_dir);
    std::fs::create_dir_all(&overlay_dir)
        .with_context(|| format!("failed to create {}", overlay_dir.display()))?;

    let started = Instant::now();
    let timeout = std::time::Duration::from_secs(cli.timeout.max(1));
    let mut classifications = Vec::with_capacity(discovery.total);

    for (idx, fd) in discovery.files.iter().enumerate() {
        let abs = cli.path.join(&fd.file);
        let source = std::fs::read_to_string(&abs)
            .with_context(|| format!("failed to read {}", abs.display()))?;
        for mp in &fd.points {
            classifications.push(run_one(
                &cli.path,
                &source,
                mp,
                &overlay_dir,
                timeout,
                &covered,
                idx,
            )?);
        }
    }

    let elapsed = started.elapsed().as_millis();
    let report = Report::new(classifications, elapsed, cli.threshold);

    // Clean up overlay scratch.
    let _ = std::fs::remove_dir_all(&overlay_dir);

    if cli.json {
        let out = RunOutput {
            schema_version: 1,
            tool: "gopher_mutant".into(),
            go_toolchain: go_version(),
            module_path: cli.path.to_string_lossy().to_string(),
            report: &report,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print_console(&report);
    }

    Ok(if report.below_threshold { 1 } else { 0 })
}

/// Run one mutant and classify it.
fn run_one(
    module_root: &Path,
    source: &str,
    mp: &MutationPoint,
    overlay_dir: &Path,
    timeout: std::time::Duration,
    covered: &std::collections::HashSet<(String, usize)>,
    idx: usize,
) -> Result<Classification> {
    let run = run_mutant(module_root, mp, source, overlay_dir, timeout)?;
    let outcome = match run.kind() {
        gopher_mutant_core::runner::RunKind::Timeout => Outcome::Timeout,
        gopher_mutant_core::runner::RunKind::CompileError => Outcome::CompileError,
        gopher_mutant_core::runner::RunKind::Failed => Outcome::Killed,
        gopher_mutant_core::runner::RunKind::Passed => {
            if covered.contains(&(mp.file.clone(), mp.line)) {
                Outcome::Survived
            } else {
                Outcome::NotCovered
            }
        }
    };
    Ok(Classification {
        id: idx,
        file: mp.file.clone(),
        line: mp.line,
        column: mp.column,
        operator: mp.operator.to_string(),
        label: mp.label.clone(),
        outcome,
        duration_ms: run.duration.as_millis(),
        covered: covered.contains(&(mp.file.clone(), mp.line)),
    })
}

fn print_console(r: &Report) {
    println!("gopher_mutant 0.1.0 (M1)");
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
