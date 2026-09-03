# GOAL 0.2.0 — gopher_mutant: M2 operators + fixtures

> **Status:** draft (proposed 2026-08-04). Locking is a human act — see Human check.
> **Prerequisite:** GOAL-1.md (M1 engine skeleton) signed-off.
> **Scope decided in wayfinder ticket #6 + #7:** all 22 operators (12 Go-idiomatic + 10 generic), small/medium/large fixture suite per the fixture sizing contract, compile-error rate <2%.

## Mission

The complete operator set — **22 operators total: 10 generic + 12 Go-idiomatic** (DeferRemoval, GoroutineRemoval, ErrCheckRemoval, ErrReturnSwap, ChannelDirection, ChannelCloseRemoval, SelectCaseRemoval, RangeBreak, MapIterationSwap, AppendRemoval, SliceIndexSwap, RecoverRemoval) — each verified against a three-tier fixture suite (small/medium/large) with deliberate kill/survive design and a compile-error rate under 2%. "Done" means the deepest operator set in the Go mutation-testing field demonstrably works on the fixture contract from decision #7.

---

## Verifiable Acceptance Criteria

### 1. All 22 operators implemented

The 10 generic operators (from M1, now at full strength) plus the 12 Go-idiomatic operators, per the grammar research (docs/research/grammar.md): direct node-type matches for defer/go/select/range; pattern matching for error guards (ErrCheckRemoval: `if err != nil { return … }`); identifier text-match for builtins (append/close/recover); type-information needed for ErrReturnSwap, MapIterationSwap, SliceIndexSwap — via conservative heuristics where a gopls pass is not yet wired (gopls integration is acceptable as a stretch, not a requirement).

**Test:** `gopher_mutant --list-operators` prints 22 distinct operator names; per-operator unit tests exist in the core crate.
**Pass:** 22/22 operators listed; each operator has ≥1 unit test asserting a mutation it produces (before/after source pair).

### 2. Operator uniqueness in the field — 10/12 unclaimed verified

The claimed-vs-unclaimed matrix from research #5 must hold in the shipped README: 10 of the 12 idiomatic operators unclaimed by gremlins/go-mutesting/gomutants (RangeBreak claimed by gomutants, AppendRemoval partially).

**Test:** README section "Operator set" with the matrix; grep the README for the 12 names.
**Pass:** README documents all 12 idiomatic operators and correctly states which existing tools cover which (10 unclaimed, RangeBreak claimed by gomutants, AppendRemoval partial).

### 3. Small fixture — 100% kill, fast

Per contract (decision #7): 1-2 files, 20-30 mutants, 100% kill, <5s. Operators exercised: the 10 generic + DeferRemoval (idiomatic parser-path smoke test).

**Test:** `gopher_mutant --path tests/fixtures/small --json`.
**Pass:** 20-30 mutants; MSI 100% (all mutants killed or compile_error-classified, zero survived); <5s wall clock; DeferRemoval mutants present.

### 4. Medium fixture — all five buckets, all 22 operators

Per contract: 3-5 files, 80-120 mutants, ~30s, all five classification buckets with deliberate design: ~70% killed, ~15% survived (by design — e.g. covered-but-unasserted functions), 2-3 timeout (loop-bound mutations), 2-3 not_covered (bare-call covered-but-unasserted), small compile_error presence (NOT_VIABLE classification). All 22 operators exercised with deliberate kill/survive design per operator.

**Test:** `gopher_mutant --path tests/fixtures/medium --json`; per-operator breakdown via `--json --verbose`.
**Pass:** Mutant count 80-120; every bucket present (killed ≥ 1, survived ≥ 1, timeout 2-3, not_covered 2-3, compile_error ≥ 1); all 22 operators produce ≥1 mutant; wall clock ~30s (±50%).

### 5. Large fixture — realistic scale

Per contract: 8-12 files, 300+ mutants, realistic MSI 55-75%, all buckets, Go idioms at scale (structs, methods, interfaces, goroutines + channels + select across files). Cold <10min, warm <30s.

**Test:** `gopher_mutant --path tests/fixtures/large --json` twice (cold, then warm with cache).
**Pass:** 300+ mutants; MSI within 55-75%; all buckets present; cold wall clock <10min; warm rerun <30s.

### 6. Compile-error rate <2%

Across all three fixtures, mutants that fail to compile (compile_error / NOT_VIABLE) must stay under 2% of total mutants. Per-fixture operator config is the control: error-prone operator/construct combos may be excluded per fixture where the contract demands.

**Test:** sum compile_error counts across the three fixture JSON outputs, divide by total mutants.
**Pass:** compile_error / total < 2% (strictly).

### 7. Operator gating — per-fixture operator config works

A config mechanism (per-fixture or CLI `--operators`) that includes/excludes operators, so fixtures can exclude error-prone ops and users can scope runs.

**Test:** `gopher_mutant --path tests/fixtures/medium --operators ROR,SDL` produces only those operators' mutants; `--operators none` errors cleanly.
**Pass:** Output JSON shows only the requested operator classes; clear error on invalid operator name; exit codes documented.

---

## Implementation Rules

- Follow AGENTS.md (atomic commits, branch → PR → merge, no direct pushes to main).
- The M1 harness (integration tests against fixtures) is the regression net — never regress it; every operator ships with unit tests before it's wired into discovery.
- Fixtures are generated from the fixture sizing contract (decision #7) — sizes, bucket mixes, and operator coverage are checked by integration tests, not eyeballed.
- Type-dependent operators (ErrReturnSwap, MapIterationSwap, SliceIndexSwap): conservative heuristics first; document each heuristic's false-positive/false-negative tradeoff in the operator's doc comment. Compiler-rejected mutants classify as compile_error — a real bucket, so heuristics may over-apply.
- Serial execution for fixture-mutating test suites (per the mutation-testing-fixtures skill): fixtures must not be mutated in place; overlay patching from M1 is the only mutation path.
- Commit each piece as soon as it's verified (tests + lint before commit).

## Human check

**Type:** Signed-off live demo (per wayfinder ticket #6 protocol).

The agent runs, live, in front of the human:
1. `gopher_mutant --list-operators` — 22 operators, plus the README uniqueness matrix.
2. All three fixture runs: small (fast, 100% kill), medium (all 5 buckets, per-operator breakdown), large (realistic MSI, cold/warm timings).
3. Compile-error rate calculation across fixtures (<2%).
4. A real-project run chosen by the human — e.g. one of the benchmark targets from research #5 (uuid/cobra) — the human judges fit.

**Sign-off:** the human signs off in-session; the agent records it here and flips status to `signed-off`.

**Failure:** small failures → rework (criteria do not bend). Big failures (e.g. an idiomatic operator's premise doesn't hold — mutations are all compile errors or all survive vacuously) → renegotiate criteria, then re-demo.
