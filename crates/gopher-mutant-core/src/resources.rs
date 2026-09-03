//! Resource governor (GOAL-3 criterion 3 + skill reference §6-7).
//!
//! Effective CPU capacity from affinity/cgroup, a 75% host-wide worker
//! budget, and a crash-released global session lock so concurrent sessions
//! wait instead of multiplying workers.

use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Effective CPU capacity: affinity list, else cgroup quota, else
/// available_parallelism.
pub fn effective_cpu_capacity() -> usize {
    let affinity = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("Cpus_allowed_list:\t"))
                .map(|value| {
                    value
                        .split(',')
                        .map(|part| {
                            let mut range = part.split('-');
                            let start = range
                                .next()
                                .and_then(|x| x.parse::<usize>().ok())
                                .unwrap_or(0);
                            let end = range
                                .next()
                                .and_then(|x| x.parse::<usize>().ok())
                                .unwrap_or(start);
                            end.saturating_sub(start) + 1
                        })
                        .sum::<usize>()
                })
                .filter(|count| *count > 0)
        });
    let quota = std::fs::read_to_string("/sys/fs/cgroup/cpu.max")
        .ok()
        .and_then(|value| {
            let mut fields = value.split_whitespace();
            let quota = fields.next()?;
            let period = fields.next()?.parse::<u64>().ok()?;
            if quota == "max" || period == 0 {
                return None;
            }
            let quota = quota.parse::<u64>().ok()?;
            Some((quota / period).max(1) as usize)
        });
    affinity
        .or(quota)
        .or_else(|| std::thread::available_parallelism().map(|n| n.get()).ok())
        .unwrap_or(1)
}

/// Host-wide worker budget: 75% of effective capacity, at least 1.
pub fn global_cpu_budget() -> usize {
    (effective_cpu_capacity().saturating_mul(3) / 4).max(1)
}

/// A crash-released global session lock. Only one mutation session runs at a
/// time on the host; a stale lock (dead owner PID) is reclaimed.
pub struct GlobalSession {
    path: PathBuf,
    pub wait_ms: u128,
}

impl GlobalSession {
    pub fn acquire() -> Result<Self> {
        let path = std::env::temp_dir().join("gopher-mutant-global-session.lock");
        let started = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    use std::io::Write;
                    writeln!(file, "{}", std::process::id())?;
                    return Ok(Self {
                        path,
                        wait_ms: started.elapsed().as_millis(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|value| value.trim().parse::<u32>().ok())
                        .is_some_and(|pid| !PathBuf::from(format!("/proc/{pid}")).exists());
                    if stale {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for GlobalSession {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_at_least_one() {
        assert!(global_cpu_budget() >= 1);
    }

    #[test]
    fn session_lock_acquires_and_releases() {
        let session = GlobalSession::acquire().unwrap();
        let path = session.path.clone();
        drop(session);
        assert!(!path.exists(), "lock must be released on drop");
    }
}
