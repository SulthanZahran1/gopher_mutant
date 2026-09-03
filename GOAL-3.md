# GOAL 0.3.0 — gopher_mutant: M3 speed

> **Status:** 🔒 **locked** (locked 2026-09-03 by human — criteria frozen; changes require human renegotiation).
> **Prerequisite:** GOAL-2.md (M2 operators + fixtures) signed-off.
> **Scope decided in wayfinder ticket #6:** per-test coverage routing, adaptive timeouts, parallel scheduler, content-addressed cache, incremental mode, `--mutant N`. TCE is NOT in this milestone (folded into M4 — it's opt-in and only touches surviving mutants, which need M3's speed to exist at scale).

## Mission

Make mutation runs fast enough that TCE (M4) is viable on real projects: per-test coverage routing (run only the tests that cover a mutant's line), adaptive timeouts (kill runaway mutants fast, don't false-positive slow ones), parallel mutant execution, a content-addressed cache (warm reruns skip re-running everything), incremental mode (only changed files), and `--mutant N` single-mutant debugging. "Done" means the medium/large fixtures run an order of magnitude faster than M2, and warm reruns are near-instant.

---

## Verifiable Acceptance Criteria

### 1. Per-test coverage routing

Per research #2 (docs/research/coverage-routing.md): build the per-test coverage map once via `go test -c -o <bin>.test -cover`, then run each test binary with `-test.run=^TestName$ -test.coverprofile` per test; index `(file:line) → {(pkg,test)}`; normalize file identity to module-relative forward slashes (the Windows-safe identity). Mutants route to their covering tests only.

**Test:** on the medium fixture, run with routing on vs off (`--no-routing` flag for comparison); inspect JSON for per-mutant `testsRun` counts.
**Pass:** routing on runs ≤ 50% of the total test executions vs routing off (sum of `testsRun` across mutants); per-mutant `testsRun` matches the coverage map for a hand-checked mutant (demo); every test that covers a mutant's line is included (no false negatives — verified by the "routing never kills" rule: a mutant killed by full-suite run is also killed by the routed run).

### 2. Adaptive timeouts

Per-mutant timeout adapts to the baseline test suite duration (e.g. baseline × factor, with a floor), replacing M1's fixed constant. A mutant that exceeds its limit classifies as timeout, not survived.

**Test:** medium fixture has 2-3 designed timeout mutants (loop-bound). Run with default adaptive timeout; then run with `--timeout 1` (1s) forcing everything to timeout.
**Pass:** default run: the 2-3 designed mutants classify as timeout, no others; `--timeout 1`: all long-running mutants classify timeout (count documented), and the run completes (no hang).

### 3. Parallel scheduler

Mutants execute in parallel (Rayon), bounded by `--parallel N` (default: CPU count). Deterministic output regardless of scheduling.

**Test:** `gopher_mutant --path tests/fixtures/large --parallel 1` vs `--parallel 8`; `--json` output diff.
**Pass:** `--parallel 8` wall clock ≤ 30% of `--parallel 1` on the large fixture; JSON outputs byte-identical (modulo timing fields, which are excluded from the comparison).

### 4. Content-addressed cache

Cache keyed on (source hash, test binary hash, Go toolchain version, operator set, flags). Warm rerun with no changes skips re-running killed mutants; changed files invalidate only their own entries.

**Test:** run large fixture, then rerun immediately; then touch one file and rerun.
**Pass:** warm rerun <30s (per M2 contract) with cached results identical to cold run (JSON diff empty modulo timings); after touching one file, only that file's mutants re-run (verify via per-mutant `cached: true/false` in JSON or a `--verbose` note).

### 5. Incremental mode

`--incremental --base-ref <git-ref>`: only files changed since the ref are mutation-tested; unchanged files' results come from the cache.

**Test:** in the large fixture repo, commit, mutate one file, run `gopher_mutant --path tests/fixtures/large --incremental --base-ref HEAD~1`.
**Pass:** only mutants in the changed file appear in output; others reported from cache; exit code and MSI computed over the full (cached + new) result set.

### 6. `--mutant N`

Run a single mutant by index (or by id from the discovery JSON), printing its patch and classification. For debugging fixtures and operators.

**Test:** `gopher_mutant --path tests/fixtures/medium --mutant 42 --json`; cross-check mutant 42's file/line/operator against the full discovery listing.
**Pass:** output contains exactly one mutant, matching the discovery listing for index 42; classification reproducible across runs.

### 7. Benchmark vs gremlins on uuid/cobra

Per research #5 (docs/research/benchmarks.md): google/uuid (~2.3k LOC, 1 pkg, baseline ~1s) and spf13/cobra (~6k LOC, 2 pkgs, baseline ~3s) are the benchmark targets. M3 must beat gremlins' wall-clock on the same targets with like-for-like operators (the 5 gremlins defaults map to ARITHMETIC_BASE, CONDITIONALS_BOUNDARY, CONDITIONALS_NEGATION, INCREMENT_DECREMENT, INVERT_NEGATIVES).

**Test:** benchmark script in `benchmarks/` clones the two repos (pinned commits), runs gremlins and gopher_mutant with the L4L operator set, records wall clocks.
**Pass:** gopher_mutant wall clock < gremlins wall clock on both targets (same machine, same number of mutants within ±10% — document mutant counts in the benchmark output); benchmark results committed as `benchmarks/results/<date>.md`.

---

## Implementation Rules

- Follow AGENTS.md (atomic commits, branch → PR → merge, no direct pushes to main).
- The M1/M2 harness is the regression net — never regress it. Every speed feature ships with a fixture-level integration test proving the speedup claim (timing assertions use behavioral markers over flaky wall-clock thresholds where possible; where wall-clock is the only signal, assert generous bounds).
- Routing correctness > routing speed: the "routing never kills" rule (a mutant killed by the full suite must also be killed by the routed run) is a hard invariant tested in CI.
- Cache and coverage-map formats are internal in M3; they become part of the frozen contract only in M4 (schemaVersion). Still, design them with forward compatibility (version field from day one).
- Parallelism must be deterministic in output: sort/fix ordering of report rows independent of scheduling.
- Commit each piece as soon as it's verified (tests + lint before commit).

## Human check

**Type:** Signed-off live demo (per wayfinder ticket #6 protocol).

The agent runs, live, in front of the human:
1. Routing demo on the medium fixture: `--no-routing` vs default — show per-mutant `testsRun` shrinking and the "routing never kills" check passing on a hand-picked mutant.
2. Parallel + warm-cache demo on the large fixture: `--parallel 1` vs `--parallel 8` timings, warm rerun <30s, JSON diff empty.
3. Incremental + `--mutant N` demo (git commit, touch one file, incremental run shows only that file's mutants).
4. The benchmark run vs gremlins on uuid/cobra — real timings, real mutant counts, committed results file. The human judges fit.

**Sign-off:** the human signs off in-session; the agent records it here and flips status to `signed-off`.

**Failure:** small failures (flaky timing, borderline like 11 min vs 10 min) → rework; criteria do NOT bend. Big failures (e.g. routing loses kills, cache returns wrong results) → renegotiate criteria, then re-demo.
