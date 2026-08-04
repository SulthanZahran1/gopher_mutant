# GOAL 0.1.0 — gopher_mutant: M1 engine skeleton

> **Status:** draft (proposed 2026-08-04). Locking is a human act — see Human check.
> **Prerequisite:** none — first milestone.
> **Scope decided in wayfinder ticket #6:** M1 is the engine skeleton only — parse, discover, overlay patch, run, classify, console report. No idiomatic operators yet (M2), no speed features (M3), no TCE/reports/distribution (M4).

## Mission

A working end-to-end mutation pipeline: `gopher_mutant --path <go-module>` parses the Go sources with tree-sitter, discovers mutation points, applies each mutant to a copy of the module via `go test -overlay` (source tree untouched), runs `go test`, and classifies every mutant into one of **killed / survived / not_covered / compile_error / timeout**, printing a console report. "Done" means the pipeline round-trips real Go code with correct, verifiable classification — the foundation M2 operators and M3 speed build on.

---

## Verifiable Acceptance Criteria

### 1. Cargo workspace + CLI skeleton

A Cargo workspace with the dart_mutant crate split (CLI binary + core library), and a `gopher_mutant` binary that accepts `--path` and exits with a documented exit code contract.

**Test:** `cargo build` in the workspace root succeeds; `cargo run --release -- --help` prints the full flag set; `cargo run --release -- --path /nonexistent` exits non-zero with a clear error message.
**Pass:** Build clean, help lists `--path`, missing path exits code ≠ 0.

### 2. Parse + discover — mutation points on the 10 generic operators

Discovery runs over the 10 generic operator classes (arithmetic swap, relational boundary, relational negation, logical swap, boolean term removal, increment/decrement, statement removal, return value removal, loop boundary, integer literal inc/dec — the shared dart_mutant set), producing a machine-readable list of mutation points (file, line, column, operator, before/after snippet).

**Test:** `gopher_mutant --path tests/fixtures/small --dry-run --json` on the small fixture.
**Pass:** ≥1 mutation point per operator class that the fixture exercises; JSON has `mutationPoints[].{file,line,column,operator}` and the counts per operator are >0 for the exercised set.

### 3. Mutant application via `go test -overlay` — source tree never touched

Each mutant is applied by writing a patched copy of the source file to a temp dir and pointing `go test -overlay` at it. The original module tree must remain byte-identical before and after a full run.

**Test:** `git status --porcelain` in a fixture module after running the full pipeline; also `sha256sum` of every source file before/after.
**Pass:** Zero modifications to the source tree (clean `git status`, identical hashes).

### 4. Classification — all five buckets correct

Every mutant run resolves to exactly one of: `killed` (tests failed), `survived` (tests passed, mutant covered), `not_covered` (tests passed, mutant never covered), `compile_error` (patch fails to build — NOT_VIABLE), `timeout` (test run exceeds the per-mutant limit).

**Test:** `gopher_mutant --path tests/fixtures/small --json`; the fixture is designed so each bucket appears at least once, and the classification is cross-checked by re-running the fixture's tests manually against a hand-applied mutant (the demo).
**Pass:** All 5 buckets present in the JSON output; classification of the hand-checked mutants matches manual expectation; every mutant classified exactly once; `killed + survived + not_covered + compile_error + timeout == total`.

### 5. Console report

Human-readable summary: total mutants, per-bucket counts, mutation score (MSI = killed / (killed + survived + not_covered)), overall pass/fail on the default threshold, wall-clock time.

**Test:** `gopher_mutant --path tests/fixtures/small` (no flags).
**Pass:** Report prints all buckets with correct sums and a computed MSI; exit code 0 when MSI ≥ threshold, non-zero when below (threshold flag `--threshold`, default 80).

### 6. Small fixture exists and is fast

The small fixture from the fixture sizing contract (decision #7): 1-2 files, 20-30 mutants, 100% kill, <5s wall clock.

**Test:** `time gopher_mutant --path tests/fixtures/small` on a warm machine.
**Pass:** Total wall clock <5s; mutant count within 20-30; MSI 100% on the fixture's default run.

---

## Implementation Rules

- Follow AGENTS.md (atomic commits, branch → PR → merge, no direct pushes to main).
- The CLI is the primary consumer of the core library as a subprocess boundary — keep `core` a pure library crate (no I/O side effects in operator logic).
- Go toolchain discovery: `go` must be on PATH; version checked at startup (error out with a clear message if missing).
- The per-mutant timeout for M1 is a fixed constant (adaptive timeouts are M3) — document the chosen value in the code.
- Overlay patching must be path-stable: the patched file lives at the same module-relative path as the original so `go test` caches and coverage paths stay consistent.
- Commit each piece as soon as it's verified (tests + lint before commit).

## Human check

**Type:** Signed-off live demo (per wayfinder ticket #6 protocol).

The agent runs, live, in front of the human:
1. `cargo build --release` + `--help` (real exit codes, real flags).
2. Full run on the small fixture: `gopher_mutant --path tests/fixtures/small --json` — show all 5 classification buckets with counts, verify `git status` clean in the fixture, show MSI + timing <5s.
3. A hand-applied mutant: the agent patches one line of the fixture manually, runs the fixture's tests, and shows the classification matches (killed/survived as designed) — proving classification is real, not guessed.
4. A real-project smoke run chosen by the human (any small Go module on the machine) — the human judges fit.

**Sign-off:** the human signs off in-session; the agent records it here and flips status to `signed-off`.

**Failure:** small failures → rework (criteria do not bend). Big failures (e.g. overlay patching doesn't work on the human's Go version, classification is wrong on real code) → renegotiate criteria, then re-demo.
