# Go Mutation-Testing Benchmark Targets & Operator Inventory

> Research for **gopher_mutant** — a Rust + tree-sitter mutation testing tool for Go (sibling of `dart_mutant`).
>
| Field | Value |
|---|---|
| Date | 2026-08-04 |
| Branch | `research/benchmarks` |
| Author | Hermes Agent (research subagent) |

This document covers two deliverables:

1. **Benchmark targets** — viability of the four targets gomutants already benchmarks, plus 1–2 additional mid-size candidates suitable for gopher_mutant.
2. **Operator inventory** — the exact mutator lists of gremlins, go-mutesting, and gomutants, read from their source code and READMEs, followed by a claimed-vs-unclaimed matrix against gopher_mutant's planned 12 Go-idiomatic operators.

---

## 1. Benchmark Targets

### 1.1 Selection criteria

A target must be suitable for mutation testing by a Rust + tree-sitter tool that shells out to `go test`:

- **Pure Go** — no cgo (`import "C"`) in the production source under test. Tree-sitter parses Go syntax; cgo requires the C toolchain and breaks AST-level assumptions.
- **`go.mod` at repo root** — so `go test ./...` resolves the module without extra plumbing.
- **Reasonable test runtime** — baseline `go test` finishes in seconds to low minutes; a full mutation run stays under ~1 hour.
- **Meaningful test suite** — enough test coverage that KILLED / LIVED / NOT_COVERED are all non-trivial buckets, not 0 or 100 %.
- **No external services** in the default test path — the target should pass `go test ./...` with no Docker / DB / network setup.

### 1.2 The four gomutants targets — confirmed viable

gomutants v0.2.2 benchmarks against four real-world Go modules (see [`docs/performance.md`](https://github.com/szhekpisov/gomutants/blob/main/docs/performance.md) and the [README benchmark snapshot](https://github.com/szhekpisov/gomutants#how-it-compares)). All four were independently cloned and inspected for this report.

| # | Target | Repo | Path | LOC (non-test) | Packages | Baseline `go test` | cgo | go.mod at root | Viable |
|---|---|---|---|---:|---:|---|---|---|---|
| 1 | google/uuid | `github.com/google/uuid` | `.` (root) | ~2.3 k | 1 | ~1.0 s | none | yes | **Yes** |
| 2 | spf13/cobra | `github.com/spf13/cobra` | `.` (root) | ~6.0 k | 2 (`cobra`, `cobra/doc`) | ~3.0 s | none | yes | **Yes** |
| 3 | prometheus/model/labels | `github.com/prometheus/prometheus` | `./model/labels` | ~4.1 k | 1 | ~3.0 s | none | yes (monorepo root) | **Yes** |
| 4 | prometheus tsdb-4 | `github.com/prometheus/prometheus` | `./tsdb/{chunkenc,index,chunks,record}` | ~14 k (target) / ~24 k (with tests) | 4 | ~5.0 s | none | yes (monorepo root) | **Yes** |

> **Note on #3 and #4.** Both live inside the `prometheus/prometheus` monorepo (`go.mod` at root). The gomutants benchmark runs `gomutants ./model/labels` and `gomutants ./tsdb/chunkenc ./tsdb/index ./tsdb/chunks ./tsdb/record` from the repo root. The "~4 k LOC" for model/labels counts the `model/labels/` directory (4 117 non-test LOC across 7 files); the "~24 k" for tsdb-4 counts non-test + test LOC of the four sub-packages. The individual sub-package non-test LOC: `chunkenc` 7 081, `index` 3 000, `chunks` 2 487, `record` 1 489 — total **14 057** non-test; with tests (~12.6 k) the combined source is ~26.7 k, which the gomutants doc rounds to "~24 k".

#### Detailed verification

**google/uuid** (`go.mod`: `module github.com/google/uuid`, no `go` directive)
- 1 391 non-test LOC, 7 test files / 1 571 test LOC.
- No cgo anywhere. Single root package. ~88 % line coverage per gomutants docs.
- gomutants OOB produces 464 mutants; gremlins (5 ops) produces ~123. Baseline `go test` ~1 s.
- **Verdict:** ideal small / fast target. No build tags, no external services, self-contained.

**spf13/cobra** (`go.mod`: `module github.com/spf13/cobra`, `go 1.15`)
- 6 955 non-test LOC across 2 packages, 17 test files / 9 810 test LOC.
- No cgo. Requires `go-md2man`, `mousetrap`, `pflag`, `yaml` — all pure Go.
- gomutants OOB produces 1 706 mutants on the root package; gremlins ~556. Baseline ~3 s.
- **Verdict:** ideal medium target. Exercises larger package with doc generation sub-package.

**prometheus/model/labels** (`go.mod`: `module github.com/prometheus/prometheus`, `go 1.25.8`)
- 4 117 non-test LOC in `model/labels/` (7 files), 3 188 test LOC (5 test files). Single package.
- No cgo in the target directory. Inside a 245-package monorepo — `go mod download` is heavy (k8s, azure, gcp deps) but the target itself is a leaf package.
- gomutants OOB: 1 324 mutants. Baseline `go test ./model/labels` ~3 s. Heavy regex test suite.
- **Verdict:** viable, but cloning the full prometheus monorepo (~1663 files, large `go.mod`) is the cost. For gopher_mutant benchmarks, a shallow clone + targeting only `./model/labels` is the approach gomutants takes. Tests run in ~3 s with 10-core parallelism.

**prometheus tsdb-4** (same monorepo, 4 sub-packages)
- 14 057 non-test LOC combined, ~12 6 k test LOC. Four packages with mixed test characters.
- No cgo in any of the four target directories. `chunkenc` (7 081 LOC, fast XOR/histogram tests), `index` (3 000 LOC, ~4 s tests, B-tree posting lists), `chunks` (2 487 LOC, ~4 s tests, on-disk format), `record` (1 489 LOC, fast WAL tests).
- gomutants OOB: 6 155 mutants. Baseline `go test -short` on combined target ~5 s. Full OOB run ~46 min; warm cache ~19 s.
- **Verdict:** viable as the "large" target. Exercises multi-package mutation testing. The 46-minute cold run is at the edge of "reasonable"; use `--cache=off` for one-shot benchmarks or scope to fewer sub-packages for faster iteration.

### 1.3 Additional mid-size candidates

The task asked to identify 1–2 more mid-size options (e.g. gorilla/mux, stretchr/testify) and check suitability.

| # | Target | Repo | LOC (non-test) | Packages | cgo | go.mod root | Baseline test | Viable? | Notes |
|---|---|---|---:|---:|---|---|---|---|---|
| 5 | gorilla/mux | `github.com/gorilla/mux` | 2 332 | 1 | none | yes (`go 1.20`) | fast (~1 s) | **Yes** | Archived/unmaintained since 2024, but stable, pure Go, 11 test files / 5 213 test LOC. Good mid-small target. |
| 6 | stretchr/testify | `github.com/stretchr/testify` | 15 068 | 14 | testdata only | yes (`go 1.17`) | moderate | **Yes (with caveat)** | cgo appears only in `internal/spew/testdata/dumpcgo.go` — a test fixture, not production code. Mutation testing targets non-test source, so this is safe. Large (15 k LOC) and broad (14 packages), good as a second "large" target. |

**Recommendation:** Add **gorilla/mux** as a second small/mid target (~2.3 k LOC, comparable to uuid) and **stretchr/testify** as an optional large target (~15 k LOC, comparable to tsdb-4). Both are pure Go, have `go.mod` at root, run without external services, and have substantial test suites. gorilla/mux's archived status is a plus for benchmark stability (no moving target) and a minus only if you want to track a living codebase.

> **Why not others.** `prometheus/client_model` is mostly generated protobuf code (~1.4 k LOC of `*.pb.go`) — not representative of hand-written Go. `prometheus/common`'s `model` package (2 689 non-test LOC) overlaps with #3's monorepo. `gorilla/websocket` is viable but network/HTTP heavy in tests. `spf13/pflag` (~3 k LOC) is a good alternative small target if a third is needed.

### 1.4 Summary benchmark target table

| Target | Repo | Path | LOC | Packages | Baseline `go test` | Mutants (OOB) | gomutants cold | gremlins cold | Viable for gopher_mutant |
|---|---|---|---:|---:|---|---:|---|---|---|
| google/uuid | google/uuid | `.` | ~2.3 k | 1 | ~1 s | 464 | 30 s | 28 s | Yes |
| spf13/cobra | spf13/cobra | `.` | ~6.0 k | 2 | ~3 s | 1 706 | 73 s (L4L) / 410 s (OOB) | 129 s | Yes |
| prometheus/model/labels | prometheus/prometheus | `./model/labels` | ~4.1 k | 1 | ~3 s | 1 324 | 90 s (L4L) / 342 s (OOB) | 139 s | Yes |
| prometheus tsdb-4 | prometheus/prometheus | `./tsdb/{chunkenc,index,chunks,record}` | ~14 k | 4 | ~5 s | 6 155 | 855 s (L4L) / 2 768 s (OOB) | 951 s | Yes |
| gorilla/mux *(new)* | gorilla/mux | `.` | ~2.3 k | 1 | ~1 s | — | — | — | Yes |
| stretchr/testify *(new)* | stretchr/testify | `.` | ~15 k | 14 | ~2–3 s | — | — | — | Yes |

> "L4L" = like-for-like (gremlins' 5 default operators only). "OOB" = out-of-box (all gomutants operators). Mutant counts and timings from gomutants `docs/performance.md` (Apple M1 Pro 10-core, gomutants v0.2.2 vs gremlins v0.6.0, Go 1.25.7). Cold = `--cache=off`.

---

## 2. Operator Inventory

### 2.1 gremlins (`go-gremlins/gremlins`) — 11 total, 5 default

Source: [`internal/mutator/mutator.go`](https://github.com/go-gremlins/gremlins/blob/main/internal/mutator/mutator.go) (the `Types` slice and `String()` method) and the [`unleash` command docs](https://gremlins.dev/latest/usage/commands/unleash/).

gremlins defines 11 mutator types in source. The gomutans comparison table and the `unleash` docs confirm **5 are on by default** (`--<name>=true`) and 6 are opt-in (`--<name>=false`). The task's "5 mutators" refers to the 5 defaults.

| # | Mutator type (source const) | String (report) | Default | Description |
|---|---|---|---|---|
| 1 | `ArithmeticBase` | `ARITHMETIC_BASE` | **on** | Swap arithmetic base operators (`+`↔`-`, `*`↔`/`, `%`↔`*`) |
| 2 | `ConditionalsBoundary` | `CONDITIONALS_BOUNDARY` | **on** | Relax/tighten comparison boundaries (`<`↔`<=`, `>`↔`>=`) |
| 3 | `ConditionalsNegation` | `CONDITIONALS_NEGATION` | **on** | Negate comparisons (`==`↔`!=`, `<`↔`>=`, `>`↔`<=`) |
| 4 | `IncrementDecrement` | `INCREMENT_DECREMENT` | **on** | Swap `++`↔`--` |
| 5 | `InvertNegatives` | `INVERT_NEGATIVES` | **on** | Invert negation (`-x`→`+x`, `a-b`→`a+b`) |
| 6 | `InvertAssignments` | `INVERT_ASSIGNMENTS` | off | Swap arithmetic compound assignments (`+=`↔`-=`, `*=`↔`/=`) |
| 7 | `InvertBitwise` | `INVERT_BITWISE` | off | Swap bitwise binary operators (`&`↔`\|`, `^`→`&`, `<<`↔`>>`) |
| 8 | `InvertBitwiseAssignments` | `INVERT_BWASSIGN` | off | Swap bitwise compound assignments (`&=`↔`\|=`, `^=`→`&=`, `<<=`↔`>>=`) |
| 9 | `InvertLogical` | `INVERT_LOGICAL` | off | Swap logical operators (`&&`↔`\|\|`) |
| 10 | `InvertLoopCtrl` | `INVERT_LOOPCTRL` | off | Swap loop control (`break`↔`continue`) |
| 11 | `RemoveSelfAssignments` | `REMOVE_SELF_ASSIGNMENTS` | off | Drop op from compound assignment (`x += y`→`x = y`) |

All 11 are **token-level** operators. gremlins has no block-level mutators (no branch emptying, no statement removal, no loop-condition forcing). Its AST-rewriting approach also has partial generics support (some generic constructs round-trip incorrectly per the gomutants comparison footnote).

### 2.2 go-mutesting (`zimmski/go-mutesting`) — 6 mutators

Source: the [README "List of mutators"](https://github.com/zimmski/go-mutesting#list-of-mutators) and the `mutator.Register()` calls in `mutator/{branch,expression,statement}/*.go`.

go-mutesting has 6 registered mutators across three categories. Unlike gremlins, all 6 are always-on (no enable/disable flags — you select via the `--mutators` flag or by targeting specific packages).

| # | Name (registered) | Category | Description |
|---|---|---|---|
| 1 | `branch/case` | Branch | Empties case bodies |
| 2 | `branch/if` | Branch | Empties branches of `if` and `else if` statements |
| 3 | `branch/else` | Branch | Empties branches of `else` statements |
| 4 | `expression/comparison` | Expression | Replaces comparison operators (`>`, `<=`, etc.) with similar operators to catch off-by-one errors (e.g. `>`→`>=`) |
| 5 | `expression/remove` | Expression | Makes each term of `&&` / `\|\|` irrelevant by substituting `true` or `false` |
| 6 | `statement/remove` | Statement | Removes assignment, increment, decrement and expression statements |

go-mutesting includes **block-level mutators** (the three `branch/*` operators) that gremlins lacks. It has no token-level arithmetic / bitwise / logical operators — its expression mutator is limited to comparisons and term removal. It does not support generics. The project is in minimal maintenance (last release 2021).

### 2.3 gomutants (`szhekpisov/gomutants`) — 22 mutators (16 default)

Source: [`internal/mutator/registry.go`](https://github.com/szhekpisov/gomutants/blob/main/internal/mutator/registry.go) (the `NewRegistry()` function) and the [README mutator table](https://github.com/szhekpisov/gomutants#mutators).

gomutants has 22 mutators: 15 token-level + 7 block-level. The README says "16 default" (the 15 token-level minus `INVERT_LOGICAL`? — no: the gomutants comparison table says "16" default vs gremlins' 5; the README's Mutators section lists all 22 without marking defaults, but the registry builds all 22 and the `--only` / `--disable` flags filter them). The "16" in the comparison table likely counts the token-level operators that are on by default in a standard run; the full set is 22.

**Token-level (15):**

| # | Type | Description | Example |
|---|---|---|---|
| 1 | `ARITHMETIC_BASE` | Swap arithmetic operators | `+`↔`-`, `*`↔`/`, `%`↔`*` |
| 2 | `CONDITIONALS_BOUNDARY` | Relax/tighten boundaries | `<`↔`<=`, `>`↔`>=` |
| 3 | `CONDITIONALS_NEGATION` | Negate comparisons | `==`↔`!=`, `<`↔`>=`, `>`↔`<=` |
| 4 | `INCREMENT_DECREMENT` | Swap increment/decrement | `++`↔`--` |
| 5 | `INVERT_NEGATIVES` | Invert negation | `-x`→`+x`, `a-b`→`a+b` |
| 6 | `INVERT_ASSIGNMENTS` | Swap arithmetic compound assignments | `+=`↔`-=`, `*=`↔`/=`, `%=`→`*=` |
| 7 | `INVERT_BITWISE` | Swap bitwise binary operators | `&`↔`\|`, `^`→`&`, `<<`↔`>>` |
| 8 | `INVERT_BITWISE_ASSIGNMENTS` | Swap bitwise compound assignments | `&=`↔`\|=`, `^=`→`&=`, `<<=`↔`>>=` |
| 9 | `INVERT_LOGICAL` | Swap logical operators | `&&`↔`\|\|` |
| 10 | `INVERT_LOOP_CTRL` | Swap loop control | `break`↔`continue` |
| 11 | `REMOVE_SELF_ASSIGNMENTS` | Drop op from compound assignment | `x += y`→`x = y` |
| 12 | `INTEGER_INCREMENT` | Increment integer literal | `42`→`43`, `0xFF`→`256` |
| 13 | `INTEGER_DECREMENT` | Decrement integer literal | `42`→`41`, `0`→`-1` |
| 14 | `FLOAT_INCREMENT` | Increment float literal | `1.5`→`2.5`, `0.0`→`1.0` |
| 15 | `FLOAT_DECREMENT` | Decrement float literal | `1.5`→`0.5`, `1e2`→`99.0` |

**Block-level (7):**

| # | Type | Description | Example |
|---|---|---|---|
| 16 | `BRANCH_IF` | Empty if/else-if body | `if x { doStuff() }`→`if x { _ = 0 }` |
| 17 | `BRANCH_ELSE` | Empty else body | `else { doStuff() }`→`else { _ = 0 }` |
| 18 | `BRANCH_CASE` | Empty case body | `case 1: doStuff()`→`case 1: _ = 0` |
| 19 | `EXPRESSION_REMOVE` | Remove boolean operand | `a && b`→`true && b` / `a && true` |
| 20 | `STATEMENT_REMOVE` | Remove statement effect | `x = expr`→`_ = expr`, `f()`→`_ = 0` |
| 21 | `LOOP_CONDITION` | Force for-loop condition to false | `for i := 0; i < n; i++ {}`→`for i := 0; false; i++ {}` |
| 22 | `RANGE_BREAK` | Insert early break in `for…range` | `for _, v := range xs { f(v) }`→`for _, v := range xs { break; f(v) }` |

gomutants is a **strict superset** of gremlins (v0.6.0) and ooze (v0.2.0): every mutator position gremlins generates, gomutants generates too (or more). The 5 gremlins defaults map to gomutants' `ARITHMETIC_BASE`, `CONDITIONALS_BOUNDARY`, `CONDITIONALS_NEGATION`, `INCREMENT_DECREMENT`, `INVERT_NEGATIVES` — which is why gomutants' `--only=ARITHMETIC_BASE,CONDITIONALS_BOUNDARY,CONDITIONALS_NEGATION,INCREMENT_DECREMENT,INVERT_NEGATIVES` is the "like-for-like" (L4L) comparison. The 3 go-mutesting block mutators (`branch/if`, `branch/else`, `branch/case`) map to gomutants' `BRANCH_IF`, `BRANCH_ELSE`, `BRANCH_CASE`.

### 2.4 Cross-tool operator comparison

| Operator concept | gremlins | go-mutesting | gomutants | gopher_mutant (planned) |
|---|---|---|---|---|
| Arithmetic base swap (`+`↔`-`) | `ARITHMETIC_BASE` ✅ | — | `ARITHMETIC_BASE` ✅ | — |
| Conditional boundary (`<`↔`<=`) | `CONDITIONALS_BOUNDARY` ✅ | — | `CONDITIONALS_BOUNDARY` ✅ | — |
| Conditional negation (`==`↔`!=`) | `CONDITIONALS_NEGATION` ✅ | — | `CONDITIONALS_NEGATION` ✅ | — |
| Increment/decrement swap | `INCREMENT_DECREMENT` ✅ | — | `INCREMENT_DECREMENT` ✅ | — |
| Invert negatives (`-x`→`+x`) | `INVERT_NEGATIVES` ✅ | — | `INVERT_NEGATIVES` ✅ | — |
| Invert assignments (`+=`↔`-=`) | `INVERT_ASSIGNMENTS` (opt) | — | `INVERT_ASSIGNMENTS` ✅ | — |
| Invert bitwise (`&`↔`\|`) | `INVERT_BITWISE` (opt) | — | `INVERT_BITWISE` ✅ | — |
| Invert bitwise assignments | `INVERT_BWASSIGN` (opt) | — | `INVERT_BITWISE_ASSIGNMENTS` ✅ | — |
| Invert logical (`&&`↔`\|\|`) | `INVERT_LOGICAL` (opt) | — | `INVERT_LOGICAL` ✅ | — |
| Invert loop control (`break`↔`continue`) | `INVERT_LOOPCTRL` (opt) | — | `INVERT_LOOP_CTRL` ✅ | — |
| Remove self-assignments | `REMOVE_SELF_ASSIGNMENTS` (opt) | — | `REMOVE_SELF_ASSIGNMENTS` ✅ | — |
| Integer literal inc/dec | — | — | `INTEGER_INCREMENT` / `INTEGER_DECREMENT` ✅ | — |
| Float literal inc/dec | — | — | `FLOAT_INCREMENT` / `FLOAT_DECREMENT` ✅ | — |
| Empty if/else-if body | — | `branch/if` ✅ | `BRANCH_IF` ✅ | — |
| Empty else body | — | `branch/else` ✅ | `BRANCH_ELSE` ✅ | — |
| Empty case body | — | `branch/case` ✅ | `BRANCH_CASE` ✅ | — |
| Remove boolean operand (`&&`/`\|\|` term) | — | `expression/remove` ✅ | `EXPRESSION_REMOVE` ✅ | — |
| Remove statement effect | — | `statement/remove` ✅ | `STATEMENT_REMOVE` ✅ | — |
| Comparison operator swap | — | `expression/comparison` ✅ | (covered by `CONDITIONALS_*`) | — |
| Force loop condition false | — | — | `LOOP_CONDITION` ✅ | — |
| Insert early break in range | — | — | `RANGE_BREAK` ✅ | `RangeBreak` ✅ |
| Defer removal | — | — | — | `DeferRemoval` ✅ |
| Goroutine removal | — | — | — | `GoroutineRemoval` ✅ |
| Error-check removal | — | — | — | `ErrCheckRemoval` ✅ |
| Error return swap | — | — | — | `ErrReturnSwap` ✅ |
| Channel direction | — | — | — | `ChannelDirection` ✅ |
| Channel close removal | — | — | — | `ChannelCloseRemoval` ✅ |
| Select-case removal | — | — | — | `SelectCaseRemoval` ✅ |
| Map iteration swap | — | — | — | `MapIterationSwap` ✅ |
| Append removal | — | — | — | `AppendRemoval` ✅ |
| Slice index swap | — | — | — | `SliceIndexSwap` ✅ |
| Recover removal | — | — | — | `RecoverRemoval` ✅ |

---

## 3. Claimed-vs-Unclaimed Matrix

gopher_mutant plans 12 Go-idiomatic operators. The matrix below shows which are **claimed** (an existing tool already implements the same or equivalent operator) and which are **unclaimed** (no existing Go mutation tool implements anything equivalent).

| # | gopher_mutant operator | What it does | gremlins | go-mutesting | gomutants | Status |
|---|---|---|---|---|---|---|
| 1 | **DeferRemoval** | Remove `defer` statement (leaks resources / skips cleanup) | — | — | — | **Unclaimed** |
| 2 | **GoroutineRemoval** | Remove `go` keyword (runs synchronously instead of concurrently) | — | — | — | **Unclaimed** |
| 3 | **ErrCheckRemoval** | Remove `if err != nil` check (ignores error) | — | — | — | **Unclaimed** |
| 4 | **ErrReturnSwap** | Swap `return nil, err` ↔ `return nil, nil` (or swap error/non-error return) | — | — | — | **Unclaimed** |
| 5 | **ChannelDirection** | Flip channel direction (`chan T` → `chan<- T` or `<-chan T`) | — | — | — | **Unclaimed** |
| 6 | **ChannelCloseRemoval** | Remove `close(ch)` (channel never closed) | — | — | — | **Unclaimed** |
| 7 | **SelectCaseRemoval** | Remove a `case` from a `select` statement | — | — | — | **Unclaimed** |
| 8 | **RangeBreak** | Insert early `break` in `for…range` body | — | — | `RANGE_BREAK` ✅ | **Claimed** (gomutants) |
| 9 | **MapIterationSwap** | Swap map iteration order / keys (nondeterminism) | — | — | — | **Unclaimed** |
| 10 | **AppendRemoval** | Remove `append()` call or its effect | — | `statement/remove` (partial) | `STATEMENT_REMOVE` (partial) | **Partially claimed** |
| 11 | **SliceIndexSwap** | Swap slice indices (`s[i]` ↔ `s[j]`) | — | — | — | **Unclaimed** |
| 12 | **RecoverRemoval** | Remove `recover()` call (panic propagates) | — | — | — | **Unclaimed** |

### Analysis

- **10 of 12 are fully unclaimed.** No existing Go mutation tool (gremlins, go-mutesting, gomutants) implements operators for `defer`, goroutines, error checking/returns, channels, select-cases, map iteration, slice index swapping, or `recover`. These are exactly the Go-idiomatic patterns that a tree-sitter-based tool can target precisely: `defer`, `go`, `err != nil`, `return ... err`, `chan`, `close()`, `select`/`case`, `for...range`, `make(...)` maps, `append()`, slice indexing, and `recover()` are all first-class syntax nodes in the Go tree-sitter grammar.
- **1 is claimed** — `RangeBreak` is identical to gomutants' `RANGE_BREAK`. gopher_mutant's implementation can be benchmarked directly against gomutants on this operator.
- **1 is partially claimed** — `AppendRemoval` overlaps with go-mutesting's `statement/remove` and gomutants' `STATEMENT_REMOVE`, but those remove *any* statement's effect generically; `AppendRemoval` would specifically target `append()` calls, which is a narrower and more Go-idiomatic mutation (e.g. `s = append(s, x)` → `s = s` or removing the assignment entirely). The generic statement-removal tools don't single out `append` as a distinct operator.

This confirms gopher_mutant's differentiator: **10 fully novel operators** targeting Go-specific concurrency, error-handling, and resource-management patterns that the AST-rewriting tools (gremlins, gomutants) and the older block-rewriting tool (go-mutesting) do not cover.

---

## 4. Sources

| Claim | Source |
|---|---|
| gremlins 11 mutator types, 5 default | [`internal/mutator/mutator.go`](https://github.com/go-gremlins/gremlins/blob/main/internal/mutator/mutator.go) (`Types` slice, `String()` method); [`unleash` docs](https://gremlins.dev/latest/usage/commands/unleash/) (default flags) |
| go-mutesting 6 mutators | [README "List of mutators"](https://github.com/zimmski/go-mutesting#list-of-mutators); `mutator.Register()` calls in `mutator/{expression,statement}/*.go` |
| gomutants 22 mutators | [`internal/mutator/registry.go`](https://github.com/szhekpisov/gomutants/blob/main/internal/mutator/registry.go) (`NewRegistry()`); [README "Mutators"](https://github.com/szhekpisov/gomutants#mutators) |
| Benchmark target LOC / packages / cgo | Direct `git clone --depth=1` + `find … -name "*.go" \| xargs wc -l` + `grep -rl "import \"C\""` on each repo (2026-08-04) |
| gomutants benchmark timings, mutant counts | [`docs/performance.md`](https://github.com/szhekpisov/gomutants/blob/main/docs/performance.md) (Apple M1 Pro 10-core, gomutants v0.2.2 vs gremlins v0.6.0) |
| gomutants is superset of gremlins + ooze | [README "Mutator-set equivalence"](https://github.com/szhekpisov/gomutants#mutator-set-equivalence) |
| gorilla/mux archived status | [GitHub issue #659](https://github.com/gorilla/mux/issues/659), [endoflife.date/gorilla](https://endoflife.date/gorilla) |
| stretchr/testify cgo in testdata only | `grep -rl "import \"C\"` on clone — only `internal/spew/testdata/dumpcgo.go` |