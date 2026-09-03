# GOAL-2 (M2) verification receipt

Date: 2026-09-03 (UTC)
Branch: `feat/m2-idiomatic-operators`
Base: `origin/main` at `8c73961`

## Build and test gates

All commands below were run from the repository root:

```text
cargo build --workspace
cargo fmt --all -- --check
cargo test --workspace -- --test-threads=1
cargo clippy --all-targets -- -D warnings
```

All four commands exited `0`. The test command ran 40 core tests and 8
integration tests; all passed. The integration suite includes exact operator
listing, operator gating, fixture discovery shape, source-tree integrity, and
the 22-mutant small-fixture contract.

## Operator listing and gating

Command:

```text
/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant --list-operators
```

Exit `0`; exact output:

```text
AOR
ROR
LOR
COR
SDL
RVR
INC
LBR
ILI
DeferRemoval
GoroutineRemoval
ErrCheckRemoval
ErrReturnSwap
ChannelDirection
ChannelCloseRemoval
SelectCaseRemoval
RangeBreak
MapIterationSwap
AppendRemoval
SliceIndexSwap
RecoverRemoval
```

`--list-operators --json` also exited `0` and returned schema version 1 with
the same 21 display names:

```json
{
  "schema_version": 1,
  "tool": "gopher_mutant",
  "operators": [
    "AOR", "ROR", "LOR", "COR", "SDL", "RVR", "INC", "LBR", "ILI",
    "DeferRemoval", "GoroutineRemoval", "ErrCheckRemoval", "ErrReturnSwap",
    "ChannelDirection", "ChannelCloseRemoval", "SelectCaseRemoval",
    "RangeBreak", "MapIterationSwap", "AppendRemoval", "SliceIndexSwap",
    "RecoverRemoval"
  ]
}
```

Commands and observed results:

```text
/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant \
  --path tests/fixtures/small --dry-run --json --operators ROR,SDL
# exit 0; total 9; every point operator is ror or sdl

/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant \
  --path tests/fixtures/small --dry-run --operators NOPE
# exit 2; error: unknown operator: NOPE

/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant \
  --path tests/fixtures/small --dry-run --operators none
# exit 2; error: no operators selected; choose at least one operator
```

## Fixture runs

Each fixture was baseline-checked with `go test ./...` before its mutation
run. The mutation commands were run against the real built binary. Medium and
large use `--timeout 10` as required; `--parallel 8` is included to keep the
real-binary receipt practical. Medium and large exit `1` because their MSI is
below the default 80% threshold; their JSON reports are valid and contain the
contract bucket mix.

| Fixture | Go files (production + test) | Total | Killed | Survived | Not covered | Compile error | Timeout | MSI | Binary exit | Wall time | `elapsed_ms` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small | 1 + 1 | 22 | 22 | 0 | 0 | 0 | 0 | 100.0000% | 0 | 7.162 s | 6,775 |
| medium | 4 + 1 | 90 | 64 | 17 | 3 | 3 | 3 | 76.1905% | 1 | 21.987 s | 21,380 |
| large, first run | 9 + 2 | 390 | 235 | 142 | 6 | 4 | 3 | 61.3577% | 1 | 100.942 s | 100,414 |
| large, second run | 9 + 2 | 390 | 235 | 142 | 6 | 4 | 3 | 61.3577% | 1 | 90.407 s | 89,924 |

Exact full-run commands:

```text
/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant \
  --path tests/fixtures/small --json

/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant \
  --path tests/fixtures/medium --json --timeout 10 --parallel 8

/home/dev/hosted_projects/gopher_mutant-m2/target/debug/gopher_mutant \
  --path tests/fixtures/large --json --timeout 10 --parallel 8

# The large command above was run twice; the first row is the first run and
# the second row is the immediately following run.
```

The small dry-run had exactly these 22 points: `AOR` 2, `ROR` 6, `LOR` 2,
`COR` 2, `SDL` 3, `RVR` 2, `INC` 1, `LBR` 1, `ILI` 2, and `DeferRemoval` 1.
The `DeferRemoval` point was killed. No small fixture source or test file was
mutated.

## Operator coverage

Counts are from the real-binary JSON classifications (display names are
preserved by the report). A listed count is at least one discovered mutant.

```text
small:
  AOR=2 ROR=6 LOR=2 COR=2 SDL=3 RVR=2 INC=1 LBR=1 ILI=2
  DeferRemoval=1
  (10 classes exercised: all generic classes plus DeferRemoval)

medium:
  AOR=3 ROR=13 LOR=2 COR=2 SDL=15 RVR=4 INC=1 LBR=1 ILI=20
  DeferRemoval=3 GoroutineRemoval=1 ErrCheckRemoval=1 ErrReturnSwap=5
  ChannelDirection=3 ChannelCloseRemoval=2 SelectCaseRemoval=2
  RangeBreak=4 MapIterationSwap=4 AppendRemoval=1 SliceIndexSwap=1
  RecoverRemoval=2
  (all 21 classes exercised)

large:
  AOR=21 ROR=62 LOR=17 COR=17 SDL=41 RVR=22 INC=1 LBR=7 ILI=172
  DeferRemoval=3 GoroutineRemoval=2 ErrCheckRemoval=1 ErrReturnSwap=4
  ChannelDirection=4 ChannelCloseRemoval=2 SelectCaseRemoval=4
  RangeBreak=3 MapIterationSwap=3 AppendRemoval=1 SliceIndexSwap=1
  RecoverRemoval=2
  (all 21 classes exercised)
```

## Source-tree integrity and compile-error rate

The fixture tree hashes below include every file and its relative path. Each
hash was identical before and after its full run:

```text
small:  2707fe5be1c1a78cdac160e9681b2a739a7c88e2363a01f3d13323c2092f412d
medium: efe125aa9ffcb4ea306deea9c210794f6414ea85e15304891841f65977a4464b
large:  774da754b01dd355ed7cb2dc5807d5697d783e1ffb856127188958b0a2bad00c
```

Compile errors across all three fixtures:

```text
(0 + 3 + 4) / (22 + 90 + 390) = 7 / 502 = 1.394422310756972%
```

This is below the locked 2% limit.

## Scope notes

- The locked prose says 22 operators, but M1 contains 9 generic enum variants,
  not 10. M2 therefore implements 9 generic + 12 idiomatic = 21 operator
  classes; `ROR` contains both boundary and negation flavors. This is called
  out in the root README rather than silently inventing a tenth class.
- The large second run took 90.407 s, so the optional sub-30-second warm bound
  is not met. That bound requires M3's content-addressed cache and is deferred
  explicitly; M2's required cold/first run was under 10 minutes.
- Medium and large deliberately include timeout, not-covered, survivor, and
  compile-error cases. The three compile-error cases in medium and four in
  large remain under the aggregate rate above.
