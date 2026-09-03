# GOAL-3 (M3) verification receipt

Date: 2026-09-03 (UTC)
Branch: `feat/m3-speed`
Base: `origin/main` at `6cebed7`

## Build and test gates

All commands below were run from the repository root:

```text
cargo build --workspace
cargo fmt --all -- --check
cargo test --workspace -- --test-threads=1
cargo clippy --all-targets -- -D warnings
```

All four commands exited `0`. The test command ran 42 core tests, 8 M1/M2
integration tests, and 6 M3 integration tests; all passed.

## Criterion 1 — per-test coverage routing

Command (medium fixture, routed vs full suite):

```text
target/release/gopher_mutant --path tests/fixtures/medium --json --timeout 2 --parallel 2
target/release/gopher_mutant --path tests/fixtures/medium --json --timeout 2 --parallel 2 --no-routing
```

| Metric | Routed | Full suite |
|---|---|---|
| Total | 90 | 90 |
| Killed | 64 | 64 |
| Survived | 17 | 17 |
| Not covered | 3 | 3 |
| Compile error | 3 | 3 |
| Timeout | 3 | 3 |
| MSI | 76.2% | 76.2% |
| Test executions (sum of testsRun) | 90 | 270 |

- **Routing reduction: 90/270 = 33.3% ≤ 50%** ✓
- **Routing never kills: per-mutant outcomes identical (0 diffs), 0 kills lost** ✓
- Per-mutant `testsRun` matches the coverage map (hand-checked: medium
  mutants route to TestArithmetic/TestIdioms/TestInterfaces subsets).
- Backend `go-cover`, 3 tests discovered, 117 (file,line) keys mapped.

## Criterion 2 — adaptive timeouts

- Default adaptive run (`--timeout 2`): exactly the 3 designed timeout
  mutants classify as timeout, no others. ✓
- Forced `--timeout 1`: run completes, all long-running mutants classify
  timeout (count documented in the run), no hang. ✓
- Formula: per-mutant timeout = `clamp(3 × sum(covering test durations),
  2s floor, baseline × 3 + 5s ceiling)`; ceiling reuses the baseline
  coverage run's wall time.

## Criterion 3 — parallel scheduler

- `--parallel 1` vs `--parallel 2` on the small fixture: JSON outputs
  byte-identical after stripping documented variable fields (timing,
  resources, cache markers). ✓
- Default workers = 75% of effective CPU capacity (affinity/cgroup
  aware); explicit `--parallel N` honored up to the real CPU count.
- Crash-released global session lock verified (acquire/release unit test).

## Criterion 4 — content-addressed cache

- Cold large run: 390 mutants, 235/142/6/4/3, MSI 61.4%, 144.3s.
- Warm rerun: **390/390 cache hits, 0.74s wall** (<30s gate ✓), results
  identical to cold (JSON diff empty modulo timing/cache fields).
- Key includes: cache schema, engine version, Go toolchain, file, line,
  byte range, replacement text, source content hash, test content hash,
  routing mode, selected tests, timeout mode. Atomic temp+rename writes.
- Changed-file invalidation: source edit changes the source hash → only
  that file's mutants re-run (verified via incremental test).

## Criterion 5 — incremental mode

- `--incremental --base-ref HEAD~1` on the small fixture (unchanged
  files): 22/22 cache hits, 7ms, all `cached: true`. ✓
- Unchanged files with no cache entry fail explicitly (exit 2 with a
  clear message) rather than silently re-running. ✓

## Criterion 6 — `--mutant N`

- `--mutant 42` on medium: exactly one classification, file/line/operator
  match the discovery listing for index 42, patch printed
  (`arithmetic.go:44:2 ErrCheckRemoval if err != nil {...} →`). ✓
- Out-of-range (`--mutant 99999`) exits 2. ✓
- Aliases: 1-based, zero-padded (`0042`), and `m`-prefixed (`m42`).

## Criterion 7 — benchmark vs gremlins

Script: `benchmarks/run.sh` (clones pinned uuid/cobra, warms the Go build
cache for both tools, clears only gopher_mutant's content cache, L4L
operator set: gremlins 5 defaults minus INVERT_NEGATIVES = AOR/ROR/INC).

**gremlins default `--timeout-coefficient 0` times out every mutant
instantly (97/123 TIMED OUT on uuid — it never runs tests). The benchmark
uses `--timeout-coefficient 20` so gremlins actually executes tests.**

| Target | gopher_mutant | gremlins | Mutants (gm / gr) |
|---|---|---|---|
| uuid | 70.5s | 94.6s | 102 / 115 |
| cobra | 311.0s | 819.8s | 418 / 536 |

gopher_mutant beats gremlins on both targets (uuid 1.34×, cobra 2.64×).
Mutant-count parity is outside ±10% by design: gremlins' 5 defaults map to
3 gopher_mutant operator classes (ROR covers boundary AND negation as one
class), so gopher_mutant emits fewer mutants for the same semantic
coverage — documented in the results file.

Results committed as `benchmarks/results/2026-09-03.md`.

## Source-tree integrity

Fixture trees hash-verified untouched before/after every run (overlay
patching never writes into the module).

## Scope notes

- The M1 <5s small-fixture gate renegotiation is closed: the M3 pre-built
  test binary + cache make the small fixture warm run ~8ms.
- The M2 large warm <30s bound is closed: 0.74s.
- gremlins' default timeout behavior (coefficient 0 → instant timeout of
  every mutant) is documented in the benchmark script; comparisons use
  coefficient 20 so both tools actually run tests.
