//! Runner: execute `go test` against a mutant via `-overlay`, with
//! process-group timeouts and compile-error detection.

use crate::discover::MutationPoint;
use crate::mutate::{overlay_json, write_patched_file};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Result of one mutant's test run.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// None => compile error (go test never produced test results).
    pub tests_failed: Option<bool>,
    pub exit_code: i32,
    pub timed_out: bool,
    pub duration: Duration,
    pub output_tail: String,
}

/// How the mutant's test run behaved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Failed,
    Passed,
    CompileError,
    Timeout,
}

impl RunResult {
    pub fn kind(&self) -> RunKind {
        if self.timed_out {
            RunKind::Timeout
        } else if let Some(failed) = self.tests_failed {
            if failed {
                RunKind::Failed
            } else {
                RunKind::Passed
            }
        } else {
            RunKind::CompileError
        }
    }
}

/// M1 fixed per-mutant timeout (GOAL-1: adaptive timeouts land in M3).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// One routed test invocation: a package (module-relative dir, `.` for the
/// root package) and a `-run` regex selecting its covering tests.
#[derive(Debug, Clone)]
pub struct RunSpec {
    pub pkg_dir: String,
    pub pattern: String,
    /// Human label for the report (the covering test names).
    pub label: String,
}

/// Prepare the patched file + overlay JSON for a mutant. Shared by the
/// full-suite and routed paths.
fn prepare_overlay(
    module_root: &Path,
    mp: &MutationPoint,
    source: &str,
    overlay_json_path: &Path,
    patch_dir: &Path,
) -> Result<()> {
    let patched = crate::mutate::apply_mutant(source, mp);
    let patched_path = write_patched_file(patch_dir, module_root, &mp.file, &patched)?;
    let original_abs = module_root.join(&mp.file);
    std::fs::write(
        overlay_json_path,
        overlay_json(&original_abs, &patched_path)?,
    )
    .with_context(|| {
        format!(
            "failed to write overlay map {}",
            overlay_json_path.display()
        )
    })?;
    Ok(())
}

/// Run `go test` once against the overlay, in a fresh process group so a
/// runaway test (infinite loop) can be killed without leaving orphans.
/// `pkg_dir` is the module-relative package dir (`.` for root); `run_pattern`
/// is an optional `-run` regex (routed runs).
fn run_go_test(
    module_root: &Path,
    pkg_dir: &str,
    overlay_json_path: &Path,
    run_pattern: Option<&str>,
    timeout: Duration,
) -> Result<RunResult> {
    let started = Instant::now();

    let mut cmd = Command::new("go");
    cmd.arg("test")
        .arg("-count=1")
        .arg("-vet=off")
        .arg("-overlay")
        .arg(overlay_json_path);
    if let Some(pattern) = run_pattern {
        cmd.arg("-run").arg(pattern);
    }
    cmd.arg(".")
        .current_dir(module_root.join(pkg_dir))
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    // Put the child in its own process group so kill() hits the whole tree.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd.spawn().context("failed to spawn go test")?;

    // Poll for completion up to the timeout (stable std has no wait_timeout).
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    // Timed out: kill the process group (negative pid = group).
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                    #[cfg(not(unix))]
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(RunResult {
                        tests_failed: None,
                        exit_code: -9,
                        timed_out: true,
                        duration: started.elapsed(),
                        output_tail: "TIMEOUT".to_string(),
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e).context("go test wait failed");
            }
        }
    };

    let mut output = String::new();
    use std::io::Read;
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut output);
    }
    let duration = started.elapsed();
    let exit_code = status.code().unwrap_or(-1);

    // Compile-error detection: `go test` exits 1 for both failed tests and
    // build failures. A build failure is present when stderr contains the
    // canonical markers. GOAL-1 requires the compile_error bucket be real.
    let compile_error = status.code() == Some(1)
        && (output.contains("[build failed]")
            || output.contains("cannot find package")
            || output.contains("undefined:")
            || output.contains("syntax error")
            || output.contains("compile")
            || output.contains(".go:")
                && (output.contains("expected")
                    || output.contains("too many")
                    || output.contains("not enough")));

    Ok(RunResult {
        tests_failed: if compile_error {
            None
        } else if status.success() {
            Some(false)
        } else {
            Some(true)
        },
        exit_code,
        timed_out: false,
        duration,
        output_tail: output
            .chars()
            .rev()
            .take(600)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
    })
}

/// Run `go test` for one mutant using an overlay file, in a fresh process
/// group so a runaway test (infinite loop) can be killed without leaving
/// orphans. Returns the raw run result.
///
/// `overlay_json_path` and `patch_dir` are per-mutant (the caller owns their
/// uniqueness) — parallel workers must never share an overlay.json path.
pub fn run_mutant(
    module_root: &Path,
    mp: &MutationPoint,
    source: &str,
    overlay_json_path: &Path,
    patch_dir: &Path,
    timeout: Duration,
) -> Result<RunResult> {
    prepare_overlay(module_root, mp, source, overlay_json_path, patch_dir)?;
    run_go_test(module_root, ".", overlay_json_path, None, timeout)
}

/// Run a mutant against a set of routed test invocations (per-package
/// `-run` regexes), short-circuiting on the first non-passed result.
/// Returns the final run result and the labels of the tests executed.
pub fn run_mutant_routed(
    module_root: &Path,
    mp: &MutationPoint,
    source: &str,
    overlay_json_path: &Path,
    patch_dir: &Path,
    timeout: Duration,
    specs: &[RunSpec],
) -> Result<(RunResult, Vec<String>)> {
    prepare_overlay(module_root, mp, source, overlay_json_path, patch_dir)?;
    let mut tests_run = Vec::new();
    let mut final_result = None;
    for spec in specs {
        tests_run.push(spec.label.clone());
        let result = run_go_test(
            module_root,
            &spec.pkg_dir,
            overlay_json_path,
            Some(&spec.pattern),
            timeout,
        )?;
        let kind = result.kind();
        let stop = !matches!(kind, RunKind::Passed);
        final_result = Some(result);
        if stop {
            break;
        }
    }
    Ok((final_result.unwrap(), tests_run))
}

/// One coverage block from a Go coverprofile: file, start line, end line.
#[derive(Debug, Clone)]
pub struct CoverageBlock {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Execution count from the coverprofile. Zero means the block is not
    /// covered by the baseline test suite.
    pub count: u64,
}

/// Parse a Go coverage profile (text format) into blocks. The profile
/// stores import-path-qualified paths (`module/pkg/file.go`); matching
/// against module-relative identities is suffix-based (see [`covered_by`]).
pub fn covered_blocks(profile: &str) -> Vec<CoverageBlock> {
    let mut out = Vec::new();
    for line in profile.lines().skip(1) {
        // format: <file>:<startLine>.<startCol>,<endLine>.<endCol> <stmts> <count>
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

/// Is `rel_path:line` covered by any block in the profile? The profile
/// qualifies paths with the module/import path, so match on the trailing
/// slash-separated suffix of the profile path (platform-safe, per research
/// #2 file-identity). A line is covered when it falls inside a block's
/// [start_line, end_line] range.
pub fn covered_by(blocks: &[CoverageBlock], rel_path: &str, line: usize) -> bool {
    let suffix = format!("/{rel_path}");
    blocks.iter().any(|b| {
        b.count > 0
            && line >= b.start_line
            && line <= b.end_line
            && (b.file == rel_path || b.file.ends_with(&suffix))
    })
}
/// (b) collect the coverage profile used for covered/not_covered.
pub fn baseline_coverage(module_root: &Path, coverprofile_path: &Path) -> Result<(bool, Vec<u8>)> {
    let mut cmd = Command::new("go");
    cmd.arg("test")
        .arg("-count=1")
        .arg("-vet=off")
        .arg("-coverprofile")
        .arg(coverprofile_path)
        .arg(".")
        .current_dir(module_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().context("failed to run baseline go test")?;
    Ok((out.status.success(), out.stdout))
}

/// Build the absolute path of a file within the module.
pub fn abs_of(module_root: &Path, rel: &str) -> PathBuf {
    module_root.join(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covered_lines_parses_profile() {
        let profile = "mode: set\npkg/calc.go:3.5,5.2 2 1\npkg/calc.go:7.1,9.2 1 0\n";
        let blocks = covered_blocks(profile);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].start_line, 3);
        assert_eq!(blocks[0].end_line, 5);
        assert_eq!(blocks[0].count, 1);
        assert_eq!(blocks[1].start_line, 7);
        assert_eq!(blocks[1].end_line, 9);
        assert_eq!(blocks[1].count, 0);
    }

    #[test]
    fn covered_by_matches_ranges_and_suffix() {
        let profile = "mode: set\ngithub.com/acme/mod/calc.go:10.1,12.2 2 1\ngithub.com/acme/mod/calc.go:20.1,22.2 2 0\n";
        let blocks = covered_blocks(profile);
        assert!(covered_by(&blocks, "calc.go", 10));
        assert!(covered_by(&blocks, "calc.go", 12));
        assert!(!covered_by(&blocks, "calc.go", 13));
        assert!(!covered_by(&blocks, "calc.go", 20));
        // Wrong dir: suffix /sub/calc.go does not match .../calc.go.
        assert!(!covered_by(&blocks, "sub/calc.go", 10));
        assert!(!covered_by(&blocks, "other.go", 10));
    }

    #[test]
    fn timeout_default_is_60s() {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(60));
    }
}
