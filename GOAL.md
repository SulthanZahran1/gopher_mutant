# Goals — gopher_mutant

The path to **1.0.0** runs through milestone documents, each following the dart_mutant GOAL pattern (validated on dart-mutant 2026-08-02): a one-line mission, verifiable acceptance criteria (each with a Test and a Pass), implementation rules, and a **human check** gate at the end.

Each milestone file is independent and self-contained. A milestone moves through three states:

| State | Meaning |
|---|---|
| **draft** | Being written / not yet started |
| **locked** | Criteria agreed; work may begin |
| **signed-off** | Human check passed; milestone complete |

---

## Milestones

| File | Version | Focus | State | Human check |
|---|---|---|---|---|
| [GOAL-1.md](GOAL-1.md) | 0.1.0 | **M1 — Engine skeleton**: parse, discover, overlay patch, run, classify (killed/survived/not_covered/compile_error/timeout), console report | ✅ signed-off (2026-09-03) | Signed-off live demo (classification counts on small fixture) |
| [GOAL-2.md](GOAL-2.md) | 0.2.0 | **M2 — Operators + fixtures**: all 22 operators (12 Go-idiomatic + 10 generic), small/medium/large fixture suite, compile-error <2% | ✅ signed-off (2026-09-03) | Signed-off live demo (operator counts + MSI + compile-error rate) |
| [GOAL-3.md](GOAL-3.md) | 0.3.0 | **M3 — Speed**: per-test coverage routing, adaptive timeouts, parallel scheduler, content-addressed cache, incremental mode, `--mutant N` | 🔒 locked (2026-09-03) | Signed-off live demo (benchmark vs gremlins on uuid/cobra + warm-rerun speedup) |
| [GOAL-4.md](GOAL-4.md) | 1.0.0 | **M4 — TCE + reports + distribution**: TCE, Stryker/JUnit/HTML, threshold gate, agent JSON, install.sh, Homebrew tap, releases, CI + Windows | 📝 draft | Signed-off live demo (full release demo) |

---

## How a milestone is completed

1. Milestone doc is **locked** (criteria agreed — no changes without a human renegotiation).
2. Work proceeds; every acceptance criterion is machine-verifiable.
3. **Human check** (per the protocol decided in wayfinder ticket #6): the agent runs the milestone's acceptance criteria live — harness tests, real-project run, generated reports, install paths. The human spot-checks the receipts and judges real-project fit, then signs off in-session. The agent records the sign-off on the milestone doc.
4. Small failures → rework (criteria do not bend). Big failures → renegotiate criteria, then re-demo.

## Milestone lifecycle

- **Draft** → **Locked**: the human reviews the milestone doc and agrees the criteria are correct and complete. Locking is itself a human act (the agent proposes, the human disposes).
- **Locked** → **Signed-off**: the human check demo passes and the human signs off.
- A milestone is never self-signed-off by the agent.

## Scope foundations (wayfinder map #1)

- **Sibling of dart_mutant**: same Rust + tree-sitter architecture, same feature bar, same fixture methodology (see `mutation-testing-fixtures` skill).
- **Differentiation**: 12 Go-idiomatic operators (defer, goroutines, error handling, channels, select, range, slices) on top of 10 generic ones — 22 total, the deepest operator set in the Go mutation-testing field. 10 of the 12 idiomatic operators are unclaimed by gremlins/go-mutesting/gomutants (research #5).
- **Mutant application**: `go test -overlay` byte-patching — the source tree is never touched.
- **1.0.0 scope floor** (decision #6): Windows binaries IN (CI matrix, MSVC zip — parity with dart_mutant; module-relative forward-slash file identity is the known fix for Windows coverage paths), crates.io publish on tag, CLI/JSON contract frozen + `schemaVersion`.

## Research index

- [docs/research/coverage-routing.md](docs/research/coverage-routing.md) — per-test coverage attribution via `go test -c -cover` + per-test `.coverprofile` (decision #2)
- [docs/research/tce.md](docs/research/tce.md) — TCE via normalized `-gcflags=-S` assembly comparison, survivors only (decision #3)
- [docs/research/grammar.md](docs/research/grammar.md) — tree-sitter-go coverage of the 12 idiomatic operators (decision #4)
- [docs/research/benchmarks.md](docs/research/benchmarks.md) — benchmark targets + gremlins/go-mutesting/gomutants operator inventory (decision #5)
