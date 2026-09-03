# gopher_mutant — Mutation Testing for Go

AST-based mutation testing for Go — the deepest operator set in the field. Built as dart_mutant's sibling: same Rust + tree-sitter architecture, `go test -overlay` byte-patching (source tree never touched).

**The goal is in [GOAL.md](GOAL.md).** Read it before touching anything. Every acceptance criterion is measurable.

## Status

- **M1 (GOAL-1, 0.1.0)**: engine skeleton — parse, discover, overlay patch, run, classify (killed/survived/not_covered/compile_error/timeout), console + JSON report. **Locked, in progress.**
- M2–M4 (GOAL-2..4): drafted, not started.

## Agent skills

### Issue tracker

Work is tracked as GitHub issues. Skills that read/write the tracker use the `gh` CLI conventions. See `docs/agents/issue-tracker.md`.

### Milestones

GOAL.md is the index; each milestone is a self-contained GOAL-N.md with verifiable acceptance criteria (Test/Pass), implementation rules, and a signed-off live demo human check. Locking and sign-off are human acts.

## Architecture

```
                    ┌─────────────────────────────────┐
                    │         gopher_mutant CLI        │
                    │                                  │
                    │  1. Discover mutation points     │
                    │  2. Baseline coverage profile    │
                    │  3. Per-mutant go test -overlay  │
                    │  4. Classify (5 buckets)         │
                    │  5. Report (console/JSON)        │
                    └─────────────────────────────────┘
```

**Written in Rust.** Cargo workspace: `gopher-mutant` (CLI binary) + `gopher-mutant-core` (library). Single static binary.

### Why Rust

- tree-sitter has first-class Rust bindings (`tree-sitter-go` grammar)
- Single static binary — no runtime dependency
- Parallel mutant execution is trivial (Rayon)
- Same stack as dart_mutant — crate patterns reuse directly

### Why `go test -overlay` byte-patching

- The source tree is never touched (mutants live in a temp overlay)
- Parallel-safe by construction (each mutant gets its own overlay)
- Interrupted runs cannot corrupt the project (no in-place file rewrites)
- Go ≥1.22 requires the `{"Replace": {...}}` overlay JSON shape — the bare flat map is silently ignored (M1 regression test covers this)

## Monorepo layout

```
gopher_mutant/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── gopher-mutant/            # CLI binary
│   │   └── src/main.rs           # clap args, orchestration, reports
│   └── gopher-mutant-core/       # library
│       └── src/
│           ├── lib.rs
│           ├── parse.rs          # tree-sitter-go wrapper + comment/string masking
│           ├── discover.rs       # module walk → mutation points
│           ├── operators.rs      # the 9 M1 operators (pure text-range scanners)
│           ├── mutate.rs         # byte-patch application + overlay JSON
│           ├── runner.rs         # go test execution, timeout, coverage blocks
│           └── classify.rs       # 5-bucket outcome model + Report
├── tests/
│   └── fixtures/
│       └── small/                # calc.go + calc_test.go (21 mutants, 100% kill)
├── docs/
│   └── research/                 # wayfinder research findings (M1 references)
└── GOAL.md, GOAL-1..4.md         # milestone docs
```

## Development commands

```bash
# Build
cargo build                          # debug build
cargo build --release                # release binary

# Test
cargo test                           # unit tests (core library)
cargo test --test integration        # E2E suite against fixtures (when added)

# Lint
cargo clippy -- -D warnings          # zero warnings
cargo fmt -- --check                 # formatting check

# Run against a Go module
cargo run --release -- --path tests/fixtures/small

# Options
cargo run --release -- --path <mod> --json          # machine-readable
cargo run --release -- --path <mod> --dry-run       # discovery only
cargo run --release -- --path <mod> --threshold 80  # gate (exit 1 below)
cargo run --release -- --path <mod> --operators AOR,ROR
cargo run --release -- --path <mod> --parallel 4    # workers (default CPUs)
cargo run --release -- --path <mod> --timeout 60    # per-mutant seconds
```

## Exit codes (M1 contract, frozen in M4)

| Code | Meaning |
|---|---|
| 0 | MSI ≥ threshold (or `--dry-run`) |
| 1 | MSI below threshold |
| 2 | Tool error (bad args, missing path, baseline failing, missing go) |
| 3 | No mutants found |

## Environment

- `go` on PATH (Go ≥1.22 for the overlay `Replace` shape; tested on 1.26)
- `cargo` / `rustc` (stable)

## M1 operator set (9 generic classes per locked GOAL-1)

| Operator | What it does |
|---|---|
| AOR | arithmetic swap `+`↔`-`, `*`↔`/`, `%`→`*` |
| ROR | relational: boundary `<`↔`<=`, `>`↔`>=` and negation `==`↔`!=` |
| LOR | logical swap `&&`↔`||` |
| COR | boolean term removal (Go has no ternary) `a && b` → `a` |
| SDL | statement deletion (calls/assignments/inc-dec; skips `:=` and block openers) |
| RVR | return-value replacement — M1: bool expressions → `return false`; other types need M2's type-info pass |
| INC | loop increment/decrement swap `i++`↔`i--` |
| LBR | loop boundary `<`↔`<=` in C-style for conditions |
| ILI | integer literal inc/dec `42`→`43`/`41` (skips octal/float/identifier-adjacent) |

The 12 Go-idiomatic operators (defer, goroutines, channels, select, range, slices, error handling) land in **M2 (GOAL-2)**.

## Research index

- [docs/research/coverage-routing.md](docs/research/coverage-routing.md) — per-test coverage attribution (M3)
- [docs/research/tce.md](docs/research/tce.md) — TCE via normalized assembly (M4)
- [docs/research/grammar.md](docs/research/grammar.md) — tree-sitter-go operator coverage (M2)
- [docs/research/benchmarks.md](docs/research/benchmarks.md) — benchmark targets + field operator inventory
