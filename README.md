# gopher_mutant

`gopher_mutant` is a Rust mutation-testing driver for Go modules. It discovers
syntax-level mutants, runs the module's tests against an overlay, and reports
killed, survived, not-covered, compile-error, and timeout outcomes.

## Quick start

```sh
cargo build --workspace
./target/debug/gopher_mutant --path ./path/to/module
./target/debug/gopher_mutant --path ./path/to/module --json
```

The source module is never rewritten. Mutants are applied through Go's
`-overlay` mechanism. Exit status is `0` when the threshold passes, `1` when
it does not, `2` for an invocation/input error, and `3` when no mutants are
discovered.

## Operator set

M2 exposes **21 operator classes: 9 generic + 12 Go-idiomatic**. The locked
GOAL-2 wording says 22 operators (10 generic + 12 idiomatic), but M1 shipped
nine generic enum variants. `ROR` emits both its boundary and negation
flavors, so it remains one operator class and no artificial tenth generic
variant is added.

The generic classes are `AOR`, `ROR`, `LOR`, `COR`, `SDL`, `RVR`, `INC`, `LBR`,
and `ILI`. The idiomatic matrix below records the claimed-vs-unclaimed
comparison from `docs/research/benchmarks.md`:

| # | Idiomatic operator | What it does | gremlins | go-mutesting | gomutants | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `DeferRemoval` | Remove a `defer` statement | — | — | — | **Unclaimed** |
| 2 | `GoroutineRemoval` | Remove the `go` keyword | — | — | — | **Unclaimed** |
| 3 | `ErrCheckRemoval` | Remove an `if err != nil` check | — | — | — | **Unclaimed** |
| 4 | `ErrReturnSwap` | Swap the first two return expressions | — | — | — | **Unclaimed** |
| 5 | `ChannelDirection` | Flip channel send and receive direction | — | — | — | **Unclaimed** |
| 6 | `ChannelCloseRemoval` | Remove `close(ch)` | — | — | — | **Unclaimed** |
| 7 | `SelectCaseRemoval` | Remove one `select` case | — | — | — | **Unclaimed** |
| 8 | `RangeBreak` | Insert an early `break` in a ranged loop | — | — | `RANGE_BREAK` | **Claimed by gomutants** |
| 9 | `MapIterationSwap` | Drop the first variable from a two-variable range | — | — | — | **Unclaimed** |
| 10 | `AppendRemoval` | Remove an `append()` call or its effect | — | `statement/remove` | `STATEMENT_REMOVE` | **Partially claimed** |
| 11 | `SliceIndexSwap` | Replace a slice index with its reverse index | — | — | — | **Unclaimed** |
| 12 | `RecoverRemoval` | Replace `recover()` with `nil` | — | — | — | **Unclaimed** |

Thus ten of the twelve idiomatic classes are unclaimed in the benchmark
survey, `RangeBreak` is claimed by gomutants, and `AppendRemoval` is only
partially claimed: existing tools remove arbitrary statements, rather than
selecting `append` as a distinct operator.

### Listing and gating operators

List display names, one per line:

```sh
./target/debug/gopher_mutant --list-operators
./target/debug/gopher_mutant --list-operators --json
```

Restrict discovery and classification with a comma-separated list. Names are
case-insensitive; `none` is rejected rather than silently producing an empty
run:

```sh
./target/debug/gopher_mutant --path ./path/to/module --operators ROR,SDL
```

## Fixtures

The M2 fixtures live under `tests/fixtures/{small,medium,large}`. They are
baseline-green Go modules and are designed to exercise every idiomatic class at
medium and large scale. The real-binary verification receipt is in
[`docs/receipts/m2-verification.md`](docs/receipts/m2-verification.md).

## M3 speed features (GOAL-3, 0.3.0)

- **Per-test coverage routing** (default on): the per-test coverage map is
  built once via `go test -c -o <bin>.test -cover` + per-test
  `-test.run=^Name$ -test.coverprofile`, then each mutant runs only its
  covering tests, short-circuiting on the first kill. `--no-routing` runs the
  full suite per mutant for comparison. The "routing never kills" invariant
  (a mutant killed by the full suite is also killed when routed) is asserted
  in CI.
- **Adaptive timeouts**: `--timeout 2` selects adaptive mode —
  `max(3 × baseline + 5s, 5s)` per mutant, replacing the fixed constant.
- **Content-addressed cache**: keyed on engine + Go toolchain + mutant
  identity + source/test content + routing mode + selected tests + timeout
  mode. Warm reruns skip re-running everything (large fixture: 390/390 hits
  in <1s). `--no-cache` disables it.
- **Incremental mode**: `--incremental --base-ref <ref>` only re-runs mutants
  in files changed since the ref; unchanged files must come from the cache.
- **`--mutant N`**: run a single mutant by discovery index (1-based,
  zero-padded, or `m`-prefixed), printing its patch and classification.
- **Parallel scheduler**: Rayon workers, default 75% of effective CPU
  capacity (affinity/cgroup aware), explicit `--parallel N` honored up to the
  real CPU count. Output is deterministic regardless of scheduling.

```sh
./target/debug/gopher_mutant --path ./path/to/module --timeout 2          # routed + adaptive
./target/debug/gopher_mutant --path ./path/to/module --no-routing         # full suite per mutant
./target/debug/gopher_mutant --path ./path/to/module --incremental --base-ref HEAD~1
./target/debug/gopher_mutant --path ./path/to/module --mutant 42 --json   # single mutant + patch
```

The large fixture is run twice in the receipt to show cold and second-run
behavior. The M3 warm bound (<30s) is met: the large fixture warm rerun is
~1s with 390/390 cache hits. M2 passes `--timeout 10` to keep deliberate
infinite-loop mutants bounded; M3 uses `--timeout 2` (adaptive).
