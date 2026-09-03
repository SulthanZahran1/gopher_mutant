# Per-Test Coverage Attribution for Mutant Routing

> Research for **gopher_mutant** — a Rust + tree-sitter mutation testing tool for Go.
>
> **Goal:** For each mutant (a mutated source line), run only the tests whose coverage touches that line, instead of the whole test suite. This document investigates the Go toolchain primitives, prior-art implementations, cost model, and pitfalls, then recommends a concrete design.

---

## 1. `go test -coverprofile` and the Coverprofile Format

### 1.1 The `-coverprofile` flag

```bash
go test -coverprofile=cover.out ./...
```

This instruments the package(s) under test, runs the test binary, and writes a **coverage profile** in the legacy text format. The profile is a flat file: one `mode:` header line, then one line per coverage block.

Sources: [cmd/go documentation (pkg.go.dev)](https://pkg.go.dev/cmd/go), [cmd/cover documentation](https://pkg.go.dev/cmd/cover)

### 1.2 The `-coverpkg` flag

```bash
go test -coverpkg=./pkg/foo,./pkg/bar -coverprofile=cover.out ./test/...
```

`-coverpkg` controls **which packages get instrumented**. By default, `go test -cover` instruments only the package being tested — not its dependencies. With `-coverpkg`, you specify a comma-separated list of import-path patterns to instrument.

Key semantics:
- `-coverpkg` accepts **import paths or patterns**, not package names. `main` as a name won't work; you need the module path (e.g. `mydomain.com`). ([source](https://go.dev/doc/build-cover))
- When tests live in a separate package (external test package `foo_test`), the package under test is a dependency that must be explicitly covered. Without `-coverpkg`, the coverage profile won't include the production code at all.
- The `-coverpkg` flag **sets `-cover`** implicitly — you don't need both.

### 1.3 The `-covermode` flag

```
-covermode set,count,atomic
```

| Mode    | Counter type | Use case |
|---------|-------------|----------|
| `set`   | bool        | Default. "Did this block run?" |
| `count` | int         | How many times. Not safe for concurrent tests. |
| `atomic`| int (atomic)| Thread-safe count. Default when `-race` is on. ~2× overhead. |

For mutation testing, **`set` mode suffices** — we only need "did this line execute", not execution counts. `atomic` is only needed if tests run in parallel within a single binary and you need accurate counts.

### 1.4 The coverprofile text format

```
mode: set
github.com/example/foo/foo.go:10.34,12.2 2 1
github.com/example/foo/foo.go:14.27,16.2 1 0
github.com/example/bar/bar.go:3.24,5.2 1 1
```

Each line after the `mode:` header:

```
<file>:<startLine>.<startCol>,<endLine>.<endCol> <numStatements> <count>
```

| Field        | Meaning |
|-------------|---------|
| `file`      | Import path + base filename (e.g. `github.com/example/foo/foo.go`). On some configurations this is an absolute path. **Platform-dependent.** |
| `startLine.startCol` | Beginning of the coverage block (1-indexed line, column) |
| `endLine.endCol`     | End of the coverage block |
| `numStmt`   | Number of statements in this block |
| `count`     | 0 = not executed, >0 = executed (in `set` mode, always 1) |

The regex equivalent (from Go's own `cmd/cover/profile.go`):
```
^(.+):([0-9]+)\.([0-9]+),([0-9]+)\.([0-9]+) ([0-9]+) ([0-9]+)$
```

Sources: [Go cover/profile.go source](https://go.googlesource.com/tools/+/refs/heads/master/cover/profile.go), [golang/go#40251](https://github.com/golang/go/issues/40251)

### 1.5 File path identity — the cross-platform pitfall

The `file` field in the coverprofile is **not a bare filename**. Its format depends on how the Go command resolves it:

- **Module mode (GOPATH/module-aware):** `<importPath>/<baseName>` — e.g. `github.com/example/foo/foo.go`
- **GOPATH mode:** absolute filesystem path — e.g. `/home/user/gopath/src/github.com/example/foo/foo.go`

On **Windows**, absolute paths come out as `\\?\C:\repo\...\foo.go` and coverage URIs as `file:///C:/repo/.../foo.go`. Backslash vs. forward-slash path comparison **silently fails**, producing an empty coverage map → every mutant classified `NOT_COVERED` → `killed: 0`.

**Durable fix:** Normalize file identity to **module-root-relative paths with forward slashes** (e.g. `foo/foo.go`). This is the common suffix of both the on-disk path and the coverage profile's file field on every platform. gopher_mutant should store mutant file paths in this normalized form at scan time. (This is the same lesson documented in the `mutation-testing-fixtures` skill's Windows coverage-path pitfall.)

---

## 2. Per-Test Attribution Approaches

Go's standard `go test -coverprofile` produces **package-level** coverage — the merged result of all tests in a package. There is no built-in flag for per-test coverage attribution. Three approaches exist:

### 2.1 Approach A: Run each test individually with `-run` (gomutants' approach)

```bash
# 1. Compile the test binary once
go test -c -o /tmp/testbin-foo.test -cover ./pkg/foo

# 2. For each test function, run it in isolation with coverage
/tmp/testbin-foo.test -test.run=^TestAdd$ -test.coverprofile=/tmp/cov-TestAdd.out
/tmp/testbin-foo.test -test.run=^TestSub$ -test.coverprofile=/tmp/cov-TestSub.out
# ... one invocation per test
```

Each per-test coverprofile contains only the blocks that **that specific test** executed. Parse each profile, index `(file, line) → {test names}`.

**Pros:**
- Uses only standard Go toolchain features — no custom instrumentation.
- The compiled binary is reused across all per-test runs, amortizing compilation.
- Per-test wall-time is captured alongside coverage, enabling adaptive per-mutant timeouts.
- Works on any Go version (the `-test.run` and `-test.coverprofile` flags on the test binary are stable).

**Cons:**
- N test invocations per package (one `go test -c` + N binary executions). For a package with 50 tests, that's 50 process spawns.
- Parallel tests (`t.Parallel()`) run differently in isolation — a test that passes alone may fail when run alongside others (or vice versa). The per-test coverage map won't capture inter-test dependencies.
- `-run` is a regex; subtests using `t.Run()` need careful pattern construction.

### 2.2 Approach B: `go test -json` streaming with per-test coverage snapshots

```bash
go test -json -coverprofile=cover.out ./pkg/foo
```

Parse the JSON event stream for `Test` events with `Package` and `Test` fields. The `-coverprofile` still produces merged coverage, but you can correlate test start/end timestamps with coverage. The `testing.Coverage()` function returns a fractional coverage snapshot at any point during the test run.

**Problem:** `testing.Coverage()` returns a single float (fraction of statements covered), not per-block data. You can't reconstruct which specific lines a test covered from this. This approach gives you per-test timing and pass/fail, but not per-test line coverage.

**Verdict:** Not sufficient for mutant routing. Only useful as a supplement for timing data.

### 2.3 Approach C: Third-party per-test coverage tools (tobari)

[tobari](https://github.com/goccy/tobari) is a Go coverage tool that hooks into Go's internal coverage runtime via `//go:linkname` and overlay builds to produce true per-test coverage data. It intercepts the `coverTearDown` function and writes a `tobari.json` with per-test coverage blocks.

**Pros:**
- True per-test coverage in a single test run (no N invocations).
- Rich coverage data per test, not just package-level.

**Cons:**
- Heavy dependency on Go internals (`//go:linkname`, overlay of `runtime` and `testing/internal/testdeps`).
- Fragile across Go versions — any change to the internal coverage API breaks it.
- Significant integration complexity for a Rust tool that shells out to `go test`.

**Verdict:** Overkill for gopher_mutant. The complexity and fragility don't justify the benefit over Approach A.

### 2.4 Recommended approach: A (per-test `-run` with pre-compiled binary)

This is what gomutants uses, and it's the right tradeoff: standard toolchain, amortized compilation, per-test coverage + timing, and no internal-API dependencies.

---

## 3. How gomutants Implements Per-Test Coverage Routing

[gomutants](https://github.com/szhekpisov/gomutants) (szhekpisov, ~5 stars, created 2026-04) is a Go mutation testing tool that implements per-test coverage routing as a first-class feature. This section analyzes its actual source code.

### 3.1 Architecture overview

The per-test coverage system lives in [`internal/coverage/`](https://github.com/szhekpisov/gomutants/tree/main/internal/coverage) and consists of two key files:

- **`parse.go`** — Parses the Go coverprofile text format into `Block` structs and provides `IsCovered(file, line, col)` lookup.
- **`testmap.go`** — Builds the per-test coverage map by compiling each package's test binary once and running each test in isolation.

### 3.2 The `TestMap` data structure

```go
type TestMap struct {
    // index maps "file:line" to the set of covering tests, keyed by (pkg, name)
    index map[string]map[testKey]bool

    // per-test wall-time, keyed by (pkg, test) — for adaptive timeouts
    durations map[testKey]time.Duration

    // per-package sum of test durations — fallback timeout
    pkgDurations map[string]time.Duration
}

type testKey struct {
    pkg, name string
}
```

The index maps a `"file:line"` string key to a **set** of `testKey{pkg, name}` pairs. Package context is retained so cross-package integration routing can dispatch a covering test to its own `go test` invocation.

### 3.3 The `BuildTestMap` pipeline

```
BuildTestMap(ctx, projectDir, packages, coverPkg, tags, tmpDir, workers)
    │
    ├── 1. listTests: `go test -list .` per package → enumerate all test function names
    ├── 2. resolvePackages: `go list -f "{{.ImportPath}}\t{{.Dir}}"` → resolve patterns to packages
    ├── 3. buildPkgBins: `go test -c -o <tmp>/testbin-<pkg>.test -cover [-coverpkg=<pattern>]` per package
    ├── 4. For each test: run the pre-compiled binary with -test.run=^<TestName>$ -test.coverprofile=<tmp>/testmap-<workerID>.cov
    ├── 5. Parse each per-test coverprofile → Block[]
    └── 6. addBlocks: for each block with Count>0, index every line in [startLine, endLine] → testKey
```

Key implementation details from the source:

**Step 1 — Enumerate tests:**
```go
func listTests(ctx context.Context, projectDir string, packages []string, tags string) ([]testEntry, error) {
    for _, pkg := range packages {
        args := []string{"test", "-list", "."}
        // ...
        cmd := exec.CommandContext(ctx, "go", args...)
        cmd.Dir = projectDir
        // Parse stdout: each non-empty, non-"ok"-prefixed line is one test name
    }
}
```

**Step 3 — Compile once per package:**
```go
func compileTestBinary(ctx context.Context, projectDir, tmpDir, coverPkg, tags string, pkg resolvedPkg) (*compiledPkg, error) {
    binPath := filepath.Join(tmpDir, "testbin-"+sanitize(pkg.importPath)+"..test")
    args := []string{"test", "-c", "-o", binPath, "-cover"}
    if coverPkg != "" {
        args = append(args, "-coverpkg="+coverPkg)
    }
    // ...
    cmd := exec.CommandContext(ctx, "go", args...)
    cmd.Dir = projectDir
}
```

**Step 4 — Run each test in isolation:**
```go
func runCompiledTest(ctx context.Context, cp *compiledPkg, testName, profilePath string) ([]Block, time.Duration) {
    args := []string{
        fmt.Sprintf("-test.run=^%s$", regexp.QuoteMeta(testName)),
        "-test.coverprofile=" + profilePath,
    }
    cmd := exec.CommandContext(ctx, cp.binPath, args...)
    cmd.Dir = cp.dir
    start := time.Now()
    runErr := cmd.Run()
    dur := time.Since(start)
    // ...
    profile, _ := parseFileFunc(profilePath)
    return profile.blocks, dur
}
```

**Step 6 — Index lines to tests:**
```go
func (tm *TestMap) addBlocks(pkg, testName string, blocks []Block) {
    for _, b := range blocks {
        if b.Count == 0 {
            continue  // only index covered blocks
        }
        for line := b.StartLine; line <= b.EndLine; line++ {
            key := b.File + ":" + fmt.Sprint(line)
            tm.index[key][testKey{pkg: pkg, name: testName}] = true
        }
    }
}
```

### 3.4 Mutant routing at test time

When a worker processes a mutant, it looks up covering tests:

```go
func (w *Worker) testInvocations(m mutator.Mutant, short bool, timeout time.Duration) [][]string {
    groups := map[string][]string{}
    if w.testMap != nil {
        for _, ref := range w.testMap.TestRefsFor(m.CoverageFile, m.Line) {
            groups[ref.Pkg] = append(groups[ref.Pkg], ref.Name)
        }
    }
    // No routing info → run the whole mutant's own package
    if len(groups) == 0 {
        return [][]string{w.buildTestArgs(m, short, timeout)}
    }
    // Build one go test invocation per covering package, with -run regex
    for _, pkg := range orderRoutePackages(groups, m.Pkg) {
        args := append(base,
            fmt.Sprintf("-run=%s", coverage.RunPattern(groups[pkg])), pkg)
        // ...
    }
}
```

The `-run` regex pattern is built to match exactly the covering tests:
```go
func RunPattern(tests []string) string {
    escaped := make([]string, len(tests))
    for i, t := range tests {
        escaped[i] = regexp.QuoteMeta(t)
    }
    return "^(" + strings.Join(escaped, "|") + ")$"
}
```

The worker **short-circuits on first kill**: it runs each covering package's tests in turn, and returns immediately if any produces a non-Lived outcome. The mutant's own package is ordered first (cheapest, most likely to kill).

### 3.5 Mutation application: `go test -overlay`

gomutants does **not** modify source files on disk. It uses `go test -overlay=<json>` to substitute the patched source at build time:

```go
type overlay struct {
    Replace map[string]string `json:"Replace"`
}
// overlay.Replace = { "/abs/path/to/foo.go": "/tmp/worker-1.go" }
```

This preserves generics, never touches the source tree, and is fully reversible (just delete the temp files).

### 3.6 Cross-package routing (integration mode)

gomutants supports an optional `--integration` mode that routes mutants to covering tests in **any** package that imports the mutated package:

1. Compute the reverse-dependency closure (every package whose imports or test imports reach a target).
2. Pin `-coverpkg` to the target packages so importing tests record coverage on the mutated code.
3. Run each covering package's tests in its own `go test` invocation, short-circuiting on first kill.

This is more expensive (wider build + `-coverpkg` overhead) and makes scores non-comparable to per-package runs, but catches mutants only killed by downstream/E2E tests.

### 3.7 Adaptive per-mutant timeouts

Per-test durations captured during the coverage map build feed into per-mutant timeout sizing:

```
per-mutant timeout = clamp(
    sum(selected test durations) × timeout-margin,
    timeout-min,          // floor (e.g. 2s)
    global ceiling       // baseline × timeout-coefficient
)
```

A 50ms unit test gets a ~2s floor instead of waiting out a multi-minute whole-suite ceiling. Falls back to per-package sum, then to the global ceiling when no per-test data exists.

---

## 4. Cost Model

### 4.1 Setup phase (one-time per run)

| Step | Cost | Invocations |
|------|------|-------------|
| `go list -json` | ~100ms | 1 |
| `go test -count=1 -coverprofile` (package-level coverage) | test-suite floor (~1-5s) | 1 per package set |
| `go test -list .` (enumerate tests) | ~0.5s | 1 per package |
| `go test -c -o <bin> -cover` (compile test binary) | ~1-3s | 1 per package |
| Per-test binary runs (coverage map) | ~50ms-3s each | **N per package** (N = # tests) |

**The per-test coverage map is the dominant setup cost.** For a package with 50 tests averaging 100ms each, the map build takes ~5s. For 500 tests at 1s each, it's ~500s — but parallelized across workers.

### 4.2 Per-mutant phase (amortized)

Each mutant runs only its covering tests:
- If 3 tests cover a mutant's line, the `go test` invocation runs only those 3 (via `-run` regex), not all 50.
- The pre-compiled binary is NOT reused for mutants (each mutant needs the patched source compiled fresh via `go test -overlay`), but the `-run` filter means the test binary only executes the relevant subset.
- Short-circuit on first kill means many mutants finish after 1-2 test runs.

**Per-mutant cost ≈ go-test-overhead + sum(covering test durations)**

The `go test -overlay` overhead (~1-3s for compile + link) is the dominant per-mutant cost for fast tests. This is why gomutants uses `--workers` to parallelize mutants.

### 4.3 When does per-test routing pay off?

| Scenario | Per-test routing benefit |
|-----------|------------------------|
| High coverage (>70%), many mutants | **High** — setup amortizes over many mutants, each running few tests |
| Low coverage (<70%) | **Low** — many mutants are NOT_COVERED anyway; package-level filter suffices |
| Single run, few mutants | **Negative** — setup cost exceeds savings |
| Warm-cache rerun (incremental dev) | **High** — cached mutants skip the test loop; only changed mutants run |
| Many small, fast tests | **High** — per-mutant timeout is tight, and routing skips irrelevant tests |
| Few large, slow tests | **Moderate** — routing helps less when each test is already expensive |

gomutants' performance data shows the crossover happens around ~100-200 mutants: below that, the one-time setup dominates; above it, per-test routing pulls ahead of whole-suite-per-mutant approaches.

### 4.4 Pre-built binary amortization

The `go test -c` compilation happens **once per package** during the coverage map build. The compiled binary is then reused for all N per-test coverage runs — you pay the compile cost once, not N times. Without this, per-test coverage would require N × compile time, which is prohibitive.

For the mutation testing phase itself, the compiled binary **cannot** be reused — each mutant modifies the source, requiring a fresh compile via `go test -overlay`. However, the per-test coverage map means each mutant's `go test -overlay` invocation runs only a few tests, not the whole suite.

---

## 5. Pitfalls

### 5.1 Coverage of dependencies vs. package under test

- By default, `go test -cover` instruments **only the package under test**. Dependencies (including the package being tested if tests are in an external `_test` package) are NOT instrumented.
- Use `-coverpkg=<importPath>` to instrument the package(s) where mutants live. If tests are in `foo_test` (external test package), `-coverpkg=github.com/example/foo` is mandatory.
- `-coverpkg=./...` instruments every package in the module — correct but slower.

### 5.2 `-coverpkg` semantics

- `-coverpkg` takes **import path patterns**, not file paths or package names.
- `main` as a package name won't match; use the module's import path.
- The pattern applies to the **build**, not just the test target — all matching packages are instrumented.

### 5.3 Parallel tests (`t.Parallel()`)

- A test using `t.Parallel()` behaves differently in isolation vs. in the full suite. Running it alone with `-run=^TestFoo$` may pass or fail differently than when run alongside other parallel tests.
- The per-test coverage map captures **what the test covers when run alone**, which may differ from what it covers during the full suite (shared state, global variables, init order).
- **Mitigation:** This is a known limitation. In practice, most mutation testing results are still correct because a mutant that changes behavior will be caught regardless of test isolation quirks. The cases where it matters are rare and surface as LIVED mutants that should have been KILLED.

### 5.4 Cached test results

- `go test` caches successful test results in package list mode. A cached result prints `(cached)` and runs in ~0 time.
- **Problem:** A cached `go test` won't pick up source modifications (mutations). The cache key includes the test binary identity; `go test -overlay` creates a different binary, so the cache is naturally bypassed.
- For the coverage map build, use `-count=1` to defeat caching: `go test -count=1 -coverprofile=...`.
- `-count=1` is the idiomatic way to disable test caching. It's not a cacheable flag, so it forces a fresh run.

### 5.5 File path identity (cross-platform)

- The coverprofile `file` field is `<importPath>/<baseName>` in module mode, or an absolute path in GOPATH mode.
- On Windows, absolute paths have `\\?\` prefixes and backslashes; coverage URIs use `file:///C:/...` with forward slashes.
- **Store mutant file paths as module-root-relative with forward slashes** (e.g. `pkg/foo/foo.go`). This is the common suffix of both the on-disk path and the coverprofile file field on every platform.
- Never use `ends_with` matching against the absolute path — it fails on Windows because `\\?\C:\repo\...` and `file:///C:/repo/...` don't share a suffix. (Documented in the `mutation-testing-fixtures` skill.)

### 5.6 Subtests and `-run` regex

- `go test -run=^TestFoo$` runs `TestFoo` and all its subtests (those created via `t.Run("SubTest", ...)` within `TestFoo`).
- To run a specific subtest: `go test -run=^TestFoo/SubTest$`.
- gomutants enumerates only top-level test names via `go test -list .` and runs each as `^TestName$`, which includes all subtests. This is correct for coverage attribution — the subtests are part of the parent test's execution.

### 5.7 `-test.run` vs `-run` on the test binary

- When running a pre-compiled test binary directly (`./pkg.test -test.run=...`), the flag is `-test.run`, not `-run`.
- When running via `go test`, the flag is `-run` (which `go test` translates to `-test.run` for the binary).
- gomutants runs the compiled binary directly and correctly uses `-test.run=^TestName$`.

### 5.8 Test binary `cmd.Dir` must be the package directory

- The pre-compiled test binary expects to run from the package's directory (it looks for testdata, relative imports, etc.).
- gomutants sets `cmd.Dir = cp.dir` (the package's source directory) when running the binary.

---

## 6. Recommended Design for gopher_mutant

### 6.1 Data structures (Rust)

```rust
/// One coverage block from a Go coverprofile.
struct CoverBlock {
    file: String,       // module-root-relative, forward slashes
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
    num_stmt: u32,
    count: u32,
}

/// Parsed coverprofile.
struct CoverProfile {
    blocks: Vec<CoverBlock>,
}

/// Per-test coverage map: (file, line) → set of (pkg, test_name).
struct TestMap {
    index: HashMap<String, HashSet<TestRef>>,
    durations: HashMap<TestRef, Duration>,
    pkg_durations: HashMap<String, Duration>,
}

struct TestRef {
    pkg: String,
    name: String,
}
```

### 6.2 Coverage map build pipeline

```bash
# Step 1: Enumerate tests
go test -list . -tags=<tags> <pkg>

# Step 2: Resolve packages
go list -f "{{.ImportPath}}\t{{.Dir}}" -tags=<tags> <patterns...>

# Step 3: Compile test binary per package (ONCE)
go test -c -o /tmp/testbin-<sanitized_pkg>.test -cover [-coverpkg=<pattern>] -tags=<tags> <pkg>

# Step 4: Run each test in isolation with coverage
/tmp/testbin-<pkg>.test -test.run=^<TestName>$ -test.coverprofile=/tmp/cov-<worker>.out
# (repeat per test, parallelized across workers)

# Step 5: Parse each per-test coverprofile, index (file:line) → test
```

### 6.3 Mutant routing at test time

```bash
# For a mutant at (file, line):
# 1. Look up covering tests: testmap.index["file:line"] → {TestRef(pkg, name), ...}
# 2. If empty → NOT_COVERED (skip)
# 3. If non-empty → build -run regex and invoke:
go test -overlay=<overlay.json> -run='^(TestA|TestB|TestC)$' -timeout=<adaptive> <pkg>
# 4. Short-circuit on first non-pass outcome → KILLED
# 5. If all pass → LIVED
```

### 6.4 File identity normalization

At scan time, normalize every Go source file path to module-root-relative with forward slashes:
```
/home/user/repo/pkg/foo/foo.go  →  pkg/foo/foo.go
```
Store this as the mutant's `file` field. Use the same normalization when parsing the coverprofile's `file` field for comparison. This is the **durable cross-platform fix** — it's the common suffix on Linux, macOS, and Windows.

### 6.5 Mutation application: `go test -overlay`

```json
{
  "Replace": {
    "/abs/path/to/foo.go": "/tmp/gopher_mutant/worker-1.go"
  }
}
```

```bash
go test -overlay=/tmp/overlay-1.json -run='^(TestA)$' -timeout=5s ./pkg/foo
```

No source tree modification. Fully reversible. Preserves generics.

### 6.6 Adaptive timeout

```rust
fn compute_timeout(testmap: &TestMap, mutant: &Mutant, policy: &TimeoutPolicy) -> Duration {
    let refs = testmap.test_refs_for(&mutant.file, mutant.line);
    if let (sum, true) = testmap.sum_durations_for_refs(&refs) {
        return clamp(sum * policy.margin, policy.min, policy.global_ceiling);
    }
    // Fallback: per-package sum
    if let Some(pkg_dur) = testmap.package_duration(&mutant.pkg) {
        return clamp(pkg_dur * policy.margin, policy.min, policy.global_ceiling);
    }
    // Final fallback: global ceiling
    policy.global_ceiling
}
```

### 6.7 Parallelism model

- **Coverage map build:** parallelize per-test binary runs across workers (channel + goroutine pattern in Go; in Rust, rayon or tokio tasks).
- **Mutation testing phase:** each worker owns a stable temp source file + overlay JSON. Workers run mutants in parallel, each spawning `go test -overlay` in its own process group with RSS monitoring.

---

## 7. Verdict on Feasibility

**Feasible and recommended.** Per-test coverage routing for gopher_mutant is:

1. **Technically sound** — uses only standard Go toolchain features (`go test -c`, `-test.run`, `-test.coverprofile`, `go test -overlay`). No internal API dependencies.

2. **Proven in practice** — gomutants implements exactly this approach and reports 100% efficacy on its own codebase, with performance data showing it pulls ahead of package-level approaches (gremlins) once mutant count exceeds ~100-200.

3. **Well-bounded cost** — the one-time setup (coverage gather + per-test map build) is O(N_tests) per package but amortizes over the mutation testing phase. Pre-compiled test binaries amortize compilation. Warm-cache reruns skip the test loop entirely.

4. **Cross-platform safe** — the file-path normalization pitfall (module-root-relative, forward slashes) is a known, solved problem. Apply it at scan time.

5. **Synergistic with adaptive timeouts** — per-test durations captured during the coverage map build directly feed per-mutant timeout sizing, which is critical for fast test suites (fixed-coefficient timeouts lose 26-70% of mutants to false timeouts).

The main risk is the setup cost on small projects with few mutants — gomutants is 1.89× slower than gremlins on a 19-mutant fixture. The mitigation is an incremental cache (content-hash key on source + covering tests) that makes warm reruns near-instant.

**Recommended implementation order for gopher_mutant:**
1. Coverprofile parser (parse the text format into `CoverBlock` structs)
2. Package-level coverage filter (NOT_COVERED classification)
3. Per-test coverage map build (compile + per-test `-run` runs)
4. Mutant routing (`-run` regex from covering tests + `go test -overlay`)
5. Adaptive per-mutant timeouts from per-test durations
6. Incremental cache (content-hash key, warm-rerun skip)

---

## References

- [cmd/go documentation — `-cover`, `-covermode`, `-coverpkg`](https://pkg.go.dev/cmd/go)
- [cmd/cover documentation](https://pkg.go.dev/cmd/cover)
- [Go coverage profiling for integration tests (go.dev)](https://go.dev/doc/build-cover)
- [Go cover/profile.go source — format parser](https://go.googlesource.com/tools/+/refs/heads/master/cover/profile.go)
- [golang/go#40251 — coverprofile format clarification](https://github.com/golang/go/issues/40251)
- [testing package — `-run` regex semantics, `testing.Coverage()`](https://pkg.go.dev/testing)
- [gomutants — source code and README](https://github.com/szhekpisov/gomutants)
- [gomutants internal/coverage/testmap.go — per-test coverage map implementation](https://github.com/szhekpisov/gomutants/blob/main/internal/coverage/testmap.go)
- [gomutants internal/coverage/parse.go — coverprofile parser](https://github.com/szhekpisov/gomutants/blob/main/internal/coverage/parse.go)
- [gomutants internal/runner/worker.go — mutant routing with `go test -overlay`](https://github.com/szhekpisov/gomutants/blob/main/internal/runner/worker.go)
- [gomutants docs/performance.md — cost model and performance data](https://github.com/szhekpisov/gomutants/blob/main/docs/performance.md)
- [gomutants commit: --integration cross-package routing](https://github.com/szhekpisov/gomutants/commit/a7b918685b02a0c9dd19988186a870432187fa01)
- [tobari — third-party per-test coverage tool](https://github.com/goccy/tobari)
- [mutation-testing-fixtures skill — Windows coverage-path pitfall](https://github.com/SulthanZahran1/gopher_mutant)