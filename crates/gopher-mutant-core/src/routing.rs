//! Per-test coverage routing (GOAL-3 criterion 1).
//!
//! Builds a `(file, line) -> {tests}` map once via the research #2 recipe:
//! compile each package's test binary once with `go test -c -o <bin>.test
//! -cover`, then run each test in isolation with `<bin>.test -test.run=^Name$
//! -test.coverprofile=<cov>.out`, and index every covered line to the test.
//! Mutants then run only their covering tests (short-circuit on first kill).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One test case that can cover a mutant's line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestCase {
    /// Package import path (module-relative), e.g. `github.com/x/mod/pkg`.
    pub pkg: String,
    /// Test function name, e.g. `TestArithmetic`.
    pub name: String,
    /// Wall-clock ms of this test in isolation (feeds adaptive timeout).
    pub duration_ms: u128,
}

impl TestCase {
    /// The regex-escaped test name (callers wrap in `^(...)$`).
    pub fn run_pattern(&self) -> String {
        regex_escape(&self.name)
    }
}

/// The per-test coverage map: `(file, line) -> covering tests`.
#[derive(Debug, Clone, Default)]
pub struct TestMap {
    /// `(module-relative file, 1-based line) -> covering tests`.
    by_line: BTreeMap<(String, usize), Vec<TestCase>>,
    /// All tests discovered (fallback when a line has no entry).
    all: Vec<TestCase>,
    /// Per-package summed duration (fallback adaptive timeout).
    pkg_durations: BTreeMap<String, u128>,
    /// Number of (file,line) keys mapped.
    pub mapped: usize,
    /// Backend label for the report.
    pub backend: String,
}

impl TestMap {
    /// Tests covering `file:line`. Falls back to the full suite when the
    /// line has no entry (conservative — never drops a covering test).
    pub fn tests_for(&self, file: &str, line: usize) -> Vec<TestCase> {
        self.by_line
            .get(&(file.to_string(), line))
            .cloned()
            .unwrap_or_else(|| self.all.clone())
    }

    /// All discovered tests.
    pub fn all(&self) -> &[TestCase] {
        &self.all
    }

    /// Summed duration of the given tests (adaptive timeout input).
    pub fn sum_duration(&self, tests: &[TestCase]) -> u128 {
        tests.iter().map(|t| t.duration_ms).sum()
    }

    /// Per-package summed duration (fallback when a line has no entry).
    pub fn pkg_duration(&self, pkg: &str) -> u128 {
        self.pkg_durations.get(pkg).copied().unwrap_or(0)
    }
}

/// Build the per-test coverage map for a Go module.
///
/// `module_root` is the canonicalized module root. The map is cached on disk
/// keyed by a content hash of the module's source + test files, so warm runs
/// skip the (dominant) per-test coverage build cost.
pub fn build_test_map(module_root: &Path) -> Result<TestMap> {
    let cache_path = coverage_cache_path(module_root)?;
    if cache_path.is_file() {
        if let Ok(cached) = std::fs::read_to_string(&cache_path) {
            if let Ok(map) = serde_json::from_str::<TestMapSerde>(&cached) {
                return Ok(map.into());
            }
        }
    }

    let map = build_test_map_fresh(module_root)?;
    // Persist (atomic temp + rename).
    if let Some(dir) = cache_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let serde_map = TestMapSerde::from(&map);
    if let Ok(json) = serde_json::to_string(&serde_map) {
        let tmp = cache_path.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &cache_path);
        }
    }
    Ok(map)
}

/// Build the map from scratch (no cache).
fn build_test_map_fresh(module_root: &Path) -> Result<TestMap> {
    // Enumerate test functions per package via `go test -list .`.
    let packages = discover_packages(module_root)?;
    let mut all = Vec::new();
    let mut pkg_durations = BTreeMap::new();
    let mut by_line: BTreeMap<(String, usize), Vec<TestCase>> = BTreeMap::new();

    for pkg in &packages {
        let pkg_dir = module_root.join(&pkg.rel_dir);
        let test_names = list_tests(&pkg_dir)?;
        if test_names.is_empty() {
            continue;
        }
        // Compile the test binary once with coverage instrumentation.
        let bin_path = compile_test_binary(module_root, pkg, &test_names)?;
        let Some(bin_path) = bin_path else {
            continue;
        };
        for name in &test_names {
            let started = std::time::Instant::now();
            let profile = run_single_test(&bin_path, &pkg_dir, name)?;
            let duration_ms = started.elapsed().as_millis();
            let test = TestCase {
                pkg: pkg.import_path.clone(),
                name: name.clone(),
                duration_ms,
            };
            all.push(test.clone());
            *pkg_durations.entry(pkg.import_path.clone()).or_insert(0) += duration_ms;
            // Index every covered line.
            for block in parse_coverprofile(&profile) {
                if block.count == 0 {
                    continue;
                }
                for line in block.start_line..=block.end_line {
                    let key = (block.file.clone(), line);
                    let entries = by_line.entry(key).or_default();
                    if !entries.iter().any(|t| t.name == test.name) {
                        entries.push(test.clone());
                    }
                }
            }
        }
        // Clean up the compiled binary.
        let _ = std::fs::remove_file(&bin_path);
    }

    let mapped = by_line.len();
    Ok(TestMap {
        by_line,
        all,
        pkg_durations,
        mapped,
        backend: "go-cover".into(),
    })
}

/// A Go package within the module.
struct Package {
    /// Module-relative directory, e.g. `pkg` or `.`.
    rel_dir: String,
    /// Import path, e.g. `github.com/x/mod/pkg`.
    import_path: String,
}

/// Discover packages with test files by walking the module (mirrors the
/// discovery walk: skip hidden, vendor, node_modules).
fn discover_packages(module_root: &Path) -> Result<Vec<Package>> {
    let module_path = read_module_path(module_root)?;
    let mut out = Vec::new();
    let mut stack = vec![module_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir)
            .with_context(|| format!("failed to read dir {}", dir.display()))?;
        let mut has_go = false;
        let mut has_test = false;
        for e in rd {
            let entry = e?;
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_dir() {
                if !name.starts_with('.') && name != "vendor" && name != "node_modules" {
                    stack.push(p);
                }
            } else if name.ends_with(".go") {
                has_go = true;
                if name.ends_with("_test.go") {
                    has_test = true;
                }
            }
        }
        if has_go && has_test {
            let rel = dir
                .strip_prefix(module_root)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            let rel_dir = if rel.is_empty() { ".".into() } else { rel };
            let import_path = if rel_dir == "." {
                module_path.clone()
            } else {
                format!("{module_path}/{rel_dir}")
            };
            out.push(Package {
                rel_dir,
                import_path,
            });
        }
    }
    Ok(out)
}

/// Read the module path from go.mod.
fn read_module_path(module_root: &Path) -> Result<String> {
    let go_mod = module_root.join("go.mod");
    let content = std::fs::read_to_string(&go_mod)
        .with_context(|| format!("failed to read {}", go_mod.display()))?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("module ") {
            return Ok(rest.trim().to_string());
        }
    }
    anyhow::bail!("no `module` line in {}", go_mod.display());
}

/// List top-level test function names in a package via `go test -list .`.
/// Only `Test*` names are kept — `Benchmark*`/`Fuzz*`/`Example*` are not
/// tests and must never be routed to (a `-run` regex matches them too, and
/// benchmarks take ~1s each, inflating every routed run).
fn list_tests(pkg_dir: &Path) -> Result<Vec<String>> {
    let out = Command::new("go")
        .arg("test")
        .arg("-list")
        .arg(".")
        .current_dir(pkg_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to run go test -list")?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("ok")
                && !l.starts_with("?")
                && !l.starts_with("no test")
                && l.starts_with("Test")
        })
        .map(str::to_string)
        .collect())
}

/// Compile the package's test binary once with coverage. Returns the binary
/// path, or None if compilation failed (package has no runnable tests).
fn compile_test_binary(
    module_root: &Path,
    pkg: &Package,
    _test_names: &[String],
) -> Result<Option<PathBuf>> {
    let bin_path = std::env::temp_dir().join(format!(
        "gopher-mutant-{}-{}.test",
        std::process::id(),
        sanitize(&pkg.import_path)
    ));
    let pkg_dir = module_root.join(&pkg.rel_dir);
    let out = Command::new("go")
        .arg("test")
        .arg("-c")
        .arg("-o")
        .arg(&bin_path)
        .arg("-cover")
        .arg(".")
        .current_dir(&pkg_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("failed to compile test binary")?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&bin_path);
        return Ok(None);
    }
    Ok(Some(bin_path))
}

/// Run one test in isolation against the compiled binary, returning its
/// coverprofile text.
fn run_single_test(bin_path: &Path, pkg_dir: &Path, name: &str) -> Result<String> {
    let profile = std::env::temp_dir().join(format!(
        "gopher-mutant-{}-{}.out",
        std::process::id(),
        sanitize(name)
    ));
    let _ = std::fs::remove_file(&profile);
    let out = Command::new(bin_path)
        .arg(format!("-test.run=^{}$", regex_escape(name)))
        .arg(format!("-test.coverprofile={}", profile.display()))
        .current_dir(pkg_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("failed to run single test")?;
    let content = if profile.exists() {
        std::fs::read_to_string(&profile).unwrap_or_default()
    } else {
        String::new()
    };
    let _ = std::fs::remove_file(&profile);
    let _ = out.status;
    Ok(content)
}

/// One coverage block from a Go coverprofile.
#[derive(Debug, Clone)]
struct CoverageBlock {
    file: String,
    start_line: usize,
    end_line: usize,
    count: u64,
}

/// Parse a Go coverprofile (text format) into blocks. File paths are
/// import-path-qualified; we keep them as-is and match by suffix at lookup.
fn parse_coverprofile(profile: &str) -> Vec<CoverageBlock> {
    let mut out = Vec::new();
    for line in profile.lines().skip(1) {
        let Some(colon) = line.find(':') else {
            continue;
        };
        let file = line[..colon].to_string();
        let rest = &line[colon + 1..];
        let Some(comma) = rest.find(',') else {
            continue;
        };
        let Some(dot) = rest[..comma].find('.') else {
            continue;
        };
        let Ok(start_line) = rest[..dot].parse::<usize>() else {
            continue;
        };
        let end_part = &rest[comma + 1..];
        let Some(dot2) = end_part.find('.') else {
            continue;
        };
        let Ok(end_line) = end_part[..dot2].parse::<usize>() else {
            continue;
        };
        let mut fields = end_part[dot2 + 1..].split_whitespace();
        let Some(_end_column) = fields.next() else {
            continue;
        };
        let Some(_statements) = fields.next() else {
            continue;
        };
        let Some(count_text) = fields.next() else {
            continue;
        };
        let Ok(count) = count_text.parse::<u64>() else {
            continue;
        };
        out.push(CoverageBlock {
            file,
            start_line,
            end_line,
            count,
        });
    }
    out
}

/// Cache path for the coverage map, keyed on a content hash of the module's
/// source + test files (so edits invalidate it).
fn coverage_cache_path(module_root: &Path) -> Result<PathBuf> {
    let root = std::env::temp_dir().join("gopher-mutant-coverage-cache");
    let _ = std::fs::create_dir_all(&root);
    Ok(root.join(format!("{:016x}.json", project_content_hash(module_root)?)))
}

/// Content hash of all .go files under the module (source + tests).
fn project_content_hash(module_root: &Path) -> Result<u64> {
    let mut value = String::new();
    let mut stack = vec![module_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir)?;
        for e in rd {
            let entry = e?;
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_dir() {
                if !name.starts_with('.') && name != "vendor" && name != "node_modules" {
                    stack.push(p);
                }
            } else if name.ends_with(".go") {
                let bytes = std::fs::read(&p)?;
                value.push_str(&format!("{:016x}", fnv1a(&bytes)));
            }
        }
    }
    Ok(fnv1a(value.as_bytes()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// Serializable form of the TestMap (persisted to disk).
#[derive(Serialize, Deserialize)]
struct TestMapSerde {
    entries: Vec<TestMapEntry>,
    all: Vec<TestCase>,
    pkg_durations: BTreeMap<String, u128>,
    backend: String,
}

#[derive(Serialize, Deserialize)]
struct TestMapEntry {
    file: String,
    line: usize,
    tests: Vec<TestCase>,
}

impl From<&TestMap> for TestMapSerde {
    fn from(map: &TestMap) -> Self {
        let entries = map
            .by_line
            .iter()
            .map(|((file, line), tests)| TestMapEntry {
                file: file.clone(),
                line: *line,
                tests: tests.clone(),
            })
            .collect();
        TestMapSerde {
            entries,
            all: map.all.clone(),
            pkg_durations: map.pkg_durations.clone(),
            backend: map.backend.clone(),
        }
    }
}

impl From<TestMapSerde> for TestMap {
    fn from(s: TestMapSerde) -> Self {
        let TestMapSerde {
            entries,
            all,
            pkg_durations,
            backend,
        } = s;
        let by_line: BTreeMap<(String, usize), Vec<TestCase>> = entries
            .into_iter()
            .map(|e| ((e.file, e.line), e.tests))
            .collect();
        let mapped = by_line.len();
        TestMap {
            by_line,
            all,
            pkg_durations,
            mapped,
            backend,
        }
    }
}
