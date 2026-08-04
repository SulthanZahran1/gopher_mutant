# GOAL 1.0.0 — gopher_mutant: M4 TCE + reports + distribution

> **Status:** draft (proposed 2026-08-04). Locking is a human act — see Human check.
> **Prerequisite:** GOAL-3.md (M3 speed) signed-off.
> **Scope decided in wayfinder ticket #6:** TCE equivalent detection, Stryker JSON/JUnit/HTML reports, threshold gate, agent-friendly JSON, install.sh, Homebrew tap, releases, CI matrix with Windows. **1.0.0 scope floor:** Windows binaries IN, crates.io publish on tag, CLI/JSON contract frozen + `schemaVersion`.

## Mission

Ship gopher_mutant 1.0.0: TCE equivalence detection over surviving mutants (assembly-level, per research #3), the full dart_mutant report family (Stryker JSON, JUnit XML, self-contained HTML, console), a threshold gate for CI, an agent-friendly JSON mode, and the complete distribution story — install.sh, Homebrew tap, GitHub releases with Linux + Windows binaries, crates.io publish, CI matrix. "Done" means 1.0.0 is installable by anyone, verifiable by machines, and provably the deepest Go mutation-testing tool on the market.

---

## Verifiable Acceptance Criteria

### 1. TCE — Trivial Compiler Equivalence on surviving mutants

Per research #3 (docs/research/tce.md): for each surviving mutant, compile original and patched source with `go build -gcflags=<pkg>=-S` via `-overlay` (same file path for both), strip the `# <importPath>` header line + trailing whitespace, compare normalized assembly. Identical normalized assembly ⇒ equivalent. Check survivors only (killed mutants skip). One-sided soundness: a killable mutant is never marked equivalent.

**Test:** fixture with deliberately equivalent mutants (e.g. `x + 0` → `x`, dead-code reorder); `gopher_mutant --path tests/fixtures/medium --detect-equivalent --json`.
**Pass:** designed-equivalent mutants classify as `equivalent` (new bucket) and are excluded from MSI; a designed-killable mutant at the same location never classifies equivalent (100 runs or full suite run); `equivalent` count matches the fixture's designed count (±1 per fixture); per-mutant TCE cost documented (research says ~80-160ms, survivors only).

### 2. Reports — Stryker JSON, JUnit XML, self-contained HTML

The dart_mutant report family. Stryker JSON follows the mutation-testing-elements schema; JUnit XML is CI-consumable; HTML is a single self-contained file (no external assets).

**Test:** `gopher_mutant --path tests/fixtures/medium --format json,junit,html --out reports/`; validate each output.
**Pass:** Stryker JSON validates against the mutation-testing-elements schema (validator script in repo); JUnit XML parses with a standard parser (e.g. `python -c "import xml.etree.ElementTree..."`) and test counts sum to mutant counts; HTML file is self-contained (no `http`/`https` external references) and renders (screenshot in demo).

### 3. Threshold gate for CI

`--threshold <pct>` exits non-zero when MSI (after TCE removal) is below the threshold. Exit code contract documented and stable (0 = pass, 1 = below threshold, 2 = tool error, 3 = no mutants found).

**Test:** run medium fixture with `--threshold 10` and `--threshold 99`.
**Pass:** `--threshold 10` exits 0; `--threshold 99` exits 1; `--path /nonexistent` exits 2; empty module exits 3. Exit codes asserted in integration tests.

### 4. Agent-friendly JSON

`--json` output is stable, complete, and self-describing: `schemaVersion`, tool version, Go toolchain version, timings, per-mutant records (id, file, line, column, operator, status, testsRun, duration, equivalent), per-file and per-operator summaries, MSI. No timestamps that vary per-run in a way that breaks diffing (fixed `generatedAt` ISO field is fine, excluded from equality comparisons).

**Test:** two identical runs diff cleanly (modulo `generatedAt`); a script in `scripts/` consumes the JSON to produce a summary table.
**Pass:** `diff <(run1 --json) <(run2 --json)` empty after stripping `generatedAt`; consumer script runs and prints correct totals matching the console report.

### 5. CLI/JSON contract frozen + schemaVersion

The 1.0.0 CLI surface (flags, exit codes, JSON schema) is the frozen contract. `schemaVersion` field in JSON output; breaking changes require a major version bump.

**Test:** `docs/cli-contract.md` committed describing flags, exit codes, JSON schema; integration test asserts `schemaVersion` matches `docs/cli-contract.md`.
**Pass:** contract doc exists and matches implementation (tested); schemaVersion present and correct.

### 6. Distribution — install.sh, Homebrew tap, releases, CI + Windows

- `install.sh`: downloads the platform binary from GitHub releases, verifies checksum, installs to `/usr/local/bin` (or `$HOME/.local/bin` fallback).
- Homebrew tap: formula installs the release binary.
- GitHub Actions: build + test on Linux (cargo test, clippy, fmt), build + test on Windows (MSVC, `cargo test`), release workflow on tag: builds Linux + Windows binaries (MSVC zip — dart_mutant parity), attaches to GitHub release, publishes to crates.io, bumps Homebrew formula.
- Windows support verified per decision #6: module-relative forward-slash file identity is the coverage-path fix (from M3's routing).

**Test:** run the release workflow on a draft tag; then on a clean machine (or container): `curl -fsSL https://raw.githubusercontent.com/SulthanZahran1/gopher_mutant/main/install.sh | sh`, then `gopher_mutant --version`.
**Pass:** release assets: Linux x86_64 binary + Windows x86_64 zip (MSVC), each with checksums; install.sh installs and `--version` matches the tag; Homebrew `brew install SulthanZahran1/tap/gopher_mutant` works (verified in CI or demo); crates.io has the published crate; CI green on Linux + Windows for the same commit; `cargo install gopher_mutant` works from the published crate.

### 7. README — positioning + benchmarks

README states the differentiation (22 operators, 10/12 idiomatic unclaimed), includes the benchmark table from M3, install instructions, usage, and the operator matrix from M2.

**Test:** README check script (or manual review in demo): presence of operator matrix, benchmark results, install instructions.
**Pass:** all three sections present and consistent with the frozen contract.

---

## Implementation Rules

- Follow AGENTS.md (atomic commits, branch → PR → merge, no direct pushes to main).
- **Contract freeze is a hard gate**: no CLI flag, exit code, or JSON field changes after this milestone locks — any change requires a human renegotiation (per the failure mechanics).
- The M1-M3 harness is the regression net — never regress it; the full fixture suite runs green on Linux AND Windows in CI before the release demo.
- TCE is one-sided: never mark a mutant equivalent unless assembly matches byte-for-byte after normalization. When in doubt, classify as survived (report it, don't hide it).
- Reports share one data source: the same in-memory result model feeds JSON/JUnit/HTML/console — no divergent copies.
- Release artifacts are reproducible: `--release` builds with pinned rust-toolchain file; checksums committed or attached with the release.
- Commit each piece as soon as it's verified (tests + lint before commit).

## Human check

**Type:** Signed-off live demo (per wayfinder ticket #6 protocol) — the full release demo.

The agent runs, live, in front of the human:
1. TCE demo on the medium fixture: `--detect-equivalent` shows designed-equivalents classified and excluded from MSI; survivors re-checked in real time.
2. Reports: `--format json,junit,html` on the medium fixture, validate Stryker schema, parse JUnit, open the HTML (screenshot), show the agent-JSON consumer script.
3. Threshold gate: exit codes 0/1/2/3 live.
4. Distribution: run install.sh on a clean container (or the machine), `gopher_mutant --version` == tag; show the GitHub release page with Linux + Windows assets + checksums; `brew install` or `cargo install` path; CI green on Linux + Windows.
5. A real-project run chosen by the human (benchmark target from research #5) — the human judges fit. This is the 1.0.0 sign-off.

**Sign-off:** the human signs off in-session; the agent records it here, flips status to `signed-off`, closes the wayfinder map (#1) as complete, and tags 1.0.0.

**Failure:** small failures → rework (criteria do not bend). Big failures (e.g. TCE marks killable mutants equivalent, install.sh broken on a clean machine, Windows CI red) → renegotiate criteria, then re-demo. The contract freeze means even "small" CLI/JSON changes during rework are renegotiations.
