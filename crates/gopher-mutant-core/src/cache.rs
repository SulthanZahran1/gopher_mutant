//! Content-addressed cache (GOAL-3 criterion 4).
//!
//! Keyed on (schema version, engine version, Go toolchain identity, mutant
//! identity, mutated source content, test source content, routing mode,
//! selected tests, timeout mode). Warm reruns with no changes skip re-running
//! killed mutants; changed files invalidate only their own entries.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Bump when the cache entry format or key semantics change.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// One cached mutant outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOutcome {
    pub outcome: String,
    pub tests_run: Vec<String>,
    pub duration_ms: u128,
    pub covered: bool,
}

/// The on-disk cache store. Shared immutably across Rayon workers; hit
/// counting is atomic.
pub struct CacheStore {
    dir: PathBuf,
    cache_ms: AtomicU64,
}

impl CacheStore {
    /// Cache root for a project: `$TMP/gopher-mutant-cache/<project-hash>`.
    pub fn new(project: &Path) -> Self {
        Self {
            dir: std::env::temp_dir()
                .join("gopher-mutant-cache")
                .join(format!("{:016x}", stable_path_hash(project))),
            cache_ms: AtomicU64::new(0),
        }
    }

    /// Build the content-addressed key for a mutant run.
    ///
    /// `source_hash` is the hash of the mutated file's current content;
    /// `test_hash` covers all test files; `toolchain` is the Go version
    /// string; `timeout_key` is `adaptive` or the fixed seconds. `start`/`end`
    /// are the mutant's byte range and `replacement` its text — REQUIRED to
    /// disambiguate two mutants at the same position (e.g. ILI emits both
    /// `42→43` and `42→41` at the same range; `a + b + c` AOR twice).
    #[allow(clippy::too_many_arguments)] // every input is a distinct cache-key dimension
    pub fn key(
        &self,
        engine_version: &str,
        toolchain: &str,
        file: &str,
        line: usize,
        start: usize,
        end: usize,
        operator: &str,
        replacement: &str,
        source_hash: u64,
        test_hash: u64,
        routing: bool,
        tests: &[String],
        timeout_key: &str,
    ) -> String {
        let mut value = format!(
            "cacheSchema={CACHE_SCHEMA_VERSION};engine={engine_version};toolchain={toolchain};file={file};line={line};range={start}-{end};op={operator};repl={replacement};src={source_hash:016x};tests={test_hash:016x};routing={routing};timeout={timeout_key};"
        );
        for t in tests {
            value.push_str(t);
            value.push(';');
        }
        format!("{:016x}", fnv1a(value.as_bytes()))
    }

    /// Load a cached outcome, if present.
    pub fn load(&self, key: &str) -> Result<Option<CachedOutcome>> {
        let started = std::time::Instant::now();
        let path = self.dir.join(format!("{key}.json"));
        let result = if !path.is_file() {
            None
        } else {
            Some(serde_json::from_slice(&std::fs::read(&path)?)?)
        };
        self.cache_ms
            .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        Ok(result)
    }

    /// Store a cached outcome (atomic temp + rename).
    pub fn store(&self, key: &str, outcome: &CachedOutcome) -> Result<()> {
        let started = std::time::Instant::now();
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("failed to create cache dir {}", self.dir.display()))?;
        let path = self.dir.join(format!("{key}.json"));
        let temp = self.dir.join(format!(".{key}.tmp-{}", std::process::id()));
        std::fs::write(&temp, serde_json::to_vec(outcome)?)?;
        std::fs::rename(&temp, &path)?;
        self.cache_ms
            .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
        Ok(())
    }

    /// Total time spent in cache load/store (report field).
    pub fn cache_ms(&self) -> u128 {
        u128::from(self.cache_ms.load(Ordering::Relaxed))
    }
}

/// Stable hash of a path string (for cache dir naming).
pub fn stable_path_hash(path: &Path) -> u64 {
    fnv1a(path.to_string_lossy().as_bytes())
}

/// FNV-1a 64-bit.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Hash a file's bytes (content-addressing input).
pub fn hash_file(path: &Path) -> Result<u64> {
    Ok(fnv1a(&std::fs::read(path)?))
}

/// Hash a string's bytes.
pub fn hash_str(value: &str) -> u64 {
    fnv1a(value.as_bytes())
}
