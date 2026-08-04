# TCE Feasibility for Go (`gopher_mutant`)

**Research date:** 2026-08-04
**Verdict:** ✅ **GO** — Trivial Compiler Equivalence (TCE) is feasible for Go via assembly comparison. `go build -gcflags=<pkg>=-S` + `-overlay` is the proven recipe, already validated in production by `szhekpisov/gomutants`.

---

## TL;DR

| Question | Answer |
|----------|--------|
| Can `go tool compile` / `go build` output be normalized for byte-identical comparison? | **Partially.** The Go toolchain is *already deterministic* (same input → identical `.o`/binary). But the object/binary files embed **file paths and DWARF line tables** that differ when the mutant source lives at a different path. Normalization of `.o` files is fragile (1300+ differing bytes even after `-trimpath` and `-dwarf=false`, from pclntab line data). |
| Can assembly (`-S`) be normalized? | **Yes.** `go build -gcflags=<pkg>=-S` + `-overlay` produces assembly where the mutant compiles from the *same file path* as the original. Only the `# <importPath>` header line + trailing whitespace need stripping. Identical normalized assembly ⇒ identical machine code ⇒ provably equivalent. |
| What does `gomutants` TCE do? | Exactly this: package-scoped `-S` via `-overlay`, hash the normalized assembly, compare to a per-package reference. One-sided soundness: a killable mutant is never marked equivalent. |
| Cost per mutant? | ~80–160ms per `go build -S` compile (small–medium packages). The per-package reference is compiled once and memoized. Viable inside a routing loop that only checks *surviving* mutants. |
| Fallback if normalization is infeasible? | Assembly comparison *is* the viable path; raw `.o`/binary comparison is not. If assembly comparison were unavailable, the next-best signal is `go vet` (but it doesn't detect equivalent mutants — it's a linter, not an equivalence prover). |

---

## 1. Can `go tool compile` output be normalized?

### 1.1 Determinism of the Go toolchain

**Experiment:** Compiled the same Go file twice with `go tool compile -o out1.o src.go` and `go tool compile -o out2.o src.go`.

**Result:** `out1.o` and `out2.o` are **byte-identical** (`md5sum` matches, `cmp -l` shows 0 differences).

**Conclusion:** The Go compiler (`go tool compile`) is already deterministic. There are no build IDs, timestamps, or non-deterministic randomness injected at the `compile` level. The same input always produces the same output.

`go build` (the wrapper) also produces identical binaries on repeated runs (the build ID is a *content hash*, not a random UUID, so it's stable for identical input).

### 1.2 The path/DWARF problem

**Experiment:** Compiled two *behaviorally equivalent* but *syntactically different* sources (`a + b` vs `a + b + 0`) at *different file paths* with `go tool compile`.

**Result:** The `.o` files differ by **~1300 bytes** (out of ~2550), even though the generated assembly is identical (the compiler constant-folds `+ 0`).

**Root cause:** The object file embeds:
1. **Absolute file paths** in DWARF debug info (`/tmp/.../pkg_orig/add.go` vs `/tmp/.../pkg_equiv/add.go`)
2. **PC-line tables (pclntab)** that map PC values to source line numbers — these differ because the AST nodes are at different positions (different line/column for the `+ 0` expression)
3. **Line-number data** in the function metadata

**Normalization attempts on `.o` files:**

| Flag | Effect on `.o` diff (equiv mutant) |
|------|-----------------------------------|
| (none) | ~1393 differing bytes |
| `-trimpath /prefix` | ~1366 differing bytes (paths stripped, but pclntab/line data still differ) |
| `-trimpath` + `-p <importpath>` | ~1360 differing bytes |
| `-dwarf=false` | ~1230 differing bytes (DWARF gone, but pclntab still carries line info) |

**Conclusion:** Normalizing `.o` files to byte-identity for equivalent-but-different-source is **infeasible** without patching the pclntab itself, which is fragile and compiler-version-dependent.

### 1.3 The assembly (`-S`) path — this is the answer

**Experiment:** Compared assembly output (`-S`) for equivalent and non-equivalent mutants.

```bash
go tool compile -S pkg_orig/add.go    # a + b
go tool compile -S pkg_equiv/add.go   # a + b + 0
go tool compile -S pkg_neq/add.go     # a - b
```

After normalizing source file paths in the assembly output:

| Comparison | Assembly | Verdict |
|-----------|----------|---------|
| `a + b` vs `a + b + 0` | **Identical** (both → `ADDQ BX, AX; RET`) | ✅ Equivalent detected |
| `a + b` vs `a - b` | **Different** (`ADDQ` vs `SUBQ`) | ✅ Non-equivalent detected |
| `a + b` vs `a + (b)` | **Identical** | ✅ Equivalent detected |
| `a + b` vs `1*a + b` | **Identical** (constant fold `*1`) | ✅ Equivalent detected |
| `a + b` vs `a + b*1` | **Identical** (constant fold `*1`) | ✅ Equivalent detected |
| `a * b` vs `b * a` | **Identical** (commutative, same operands) | ✅ Equivalent detected |
| `!false` dead branch vs removed | **Identical** (dead code eliminated) | ✅ Equivalent detected |
| `a + b` vs `a | b` | **Different** (`ADDQ` vs `ORQ`) | ✅ Non-equivalent detected |
| `a + b` vs `a + b + 1` | **Different** (extra `ADDQ $1`) | ✅ Non-equivalent detected |
| `if true { ... }` vs removed | **Different** (line tables differ) | ⚠️ False negative (acceptable) |

**Why `-S` works but `.o` doesn't:** The `-S` flag prints the *assembly listing* — the actual instructions, symbol references, and data constants. The compiler's optimization passes (constant folding, dead-code elimination, etc.) run *before* code generation, so equivalent mutants that are folded away produce identical instructions. The remaining differences in `-S` output are only the `file:line` annotations (which are path-dependent and easy to normalize) and the `# <package>` header.

### 1.4 The `-overlay` trick (gomutants' key insight)

The remaining path-normalization problem is solved elegantly by using `go build -overlay`:

1. Write the mutant source to a **temp file**.
2. Create an overlay JSON mapping the **original source path → temp file path**.
3. Compile with `go build -gcflags=<importPath>=-S -overlay=<overlay.json> -o /dev/null <importPath>`.

The compiler sees the mutant source as if it lived at the **original file path**, so the `file:line` annotations in the `-S` output are identical between original and mutant. The only differing line is the `# <importPath>` header, which is stripped.

**Normalization recipe:**

```bash
# Original (reference) — compiled once per package, memoized
go build -gcflags=<importPath>=-S -o /dev/null <importPath> 2>ref_asm.txt

# Mutant — compiled per surviving mutant
go build -gcflags=<importPath>=-S -overlay=<overlay.json> -o /dev/null <importPath> 2>mut_asm.txt

# Normalize: strip the "# <importPath>" header + trailing whitespace, SHA-256
grep -v "^# <importPath>" ref_asm.txt | sed 's/[ \t]*$//' | sha256sum
grep -v "^# <importPath>" mut_asm.txt | sed 's/[ \t]*$//' | sha256sum

# If hashes match → EQUIVALENT
```

This is **exactly** what `gomutants` implements (see §2).

---

## 2. What `szhekpisov/gomutants` TCE does

### 2.1 Mechanism

Source: [`internal/tce/tce.go`](https://github.com/szhekpisov/gomutants/blob/main/internal/tce/tce.go) (PR [#56](https://github.com/szhekpisov/gomutants/pull/56), merged 2026-07-18).

**Opt-in flag:** `--detect-equivalent`

**Flow:**
1. After the test run, collect all `LIVED` (surviving) mutants.
2. Sort survivors by `(Pkg, File, StartOffset)` so consecutive survivors in the same package share the per-package reference compile and the dependency build cache.
3. For each package with survivors, compile the **original** (unmutated) package once with `go build -gcflags=<importPath>=-S -o /dev/null <importPath>` → capture stderr (where `-S` emits assembly) → normalize → hash. This is the **reference**, memoized per package.
4. For each surviving mutant:
   - Apply the byte patch to a per-worker temp file.
   - Write an overlay JSON mapping `originalSourcePath → tempFilePath`.
   - Compile with `go build -gcflags=<importPath>=-S -overlay=<overlay.json> -o /dev/null <importPath>`.
   - Normalize the assembly, hash it.
   - If `mutantHash == referenceHash` → reclassify as `EQUIVALENT` (drops out of the efficacy denominator).
5. Run survivors in parallel with bounded workers. Each worker owns stable temp files (no collision).

**Normalization (`normalizeAsm`):**
```go
func normalizeAsm(b []byte, importPath string) string {
    header := append([]byte("# "), importPath...)
    h := sha256.New()
    for line := range bytes.SplitSeq(b, []byte("\n")) {
        line = bytes.TrimRight(line, " \t\r")
        if bytes.Equal(line, header) {
            continue  // skip the "# <importPath>" header
        }
        h.Write(line)
        h.Write([]byte{'\n'})
    }
    return hex.EncodeToString(h.Sum(nil))
}
```

Only the `# <importPath>` header line is stripped, plus trailing whitespace. **File paths and line numbers are kept** — they're identical between original and mutant because the `-overlay` mechanism makes the compiler see the mutant at the original path.

### 2.2 Soundness guarantee

The comparison is **one-sided (sound)**:
- If assembly matches → mutant is **provably equivalent** (the compiler produced identical machine code; no test can distinguish them).
- If assembly differs → mutant is **left as LIVED** (might be equivalent with a different assembly representation, but we don't claim it — under-detection is the safe direction).

A killable mutant is **never** marked equivalent, because any behavioral difference that a test could catch would manifest as different generated code (different instruction, different constant, different data symbol — all of which `-S` dumps).

### 2.3 Known limitations

1. **Runtime-equivalent mutants are not caught.** TCE only detects mutants the *compiler* folds away (constant folding, dead-code elimination). Behaviorally-equivalent-at-runtime mutants that produce different assembly but have no observable behavioral difference (e.g., reordering independent statements, swapping equivalent register uses) are out of TCE's reach. gomutants handles these with manual `// gomutants:disable-next-line` directives (6 such suppressions in `tce.go` itself).

2. **Platform-specific.** The verdict is meaningful only for the host build configuration (Go toolchain version, `GOOS`/`GOARCH`/`GOEXPERIMENT`). The cache gates `EQUIVALENT` reuse on the `go_toolchain` string so a toolchain upgrade doesn't carry stale verdicts.

3. **False negatives on some compiler-folded equivalents.** Our experiments showed `if true { return a+b }` vs `return a+b` produces *different* assembly (the line tables differ even though the branch is dead-code-eliminated at the instruction level). This is a false negative (equivalent mutant left as LIVED) — safe but incomplete.

4. **Only runs on survivors.** TCE is a post-test pass, not a pre-test filter. It only fires on `LIVED` mutants, adding one compile per survivor. This is by design — testing is more expensive than compiling, and most mutants are killed.

5. **No OOM safety on TCE subprocesses.** The `go build -S` compiles are run as plain `exec.CommandContext` children without the 2 GiB RSS cap / process-group controls that the `go test` children get. This is a deliberate, low-risk gap: `go build -S` is far lighter than a linked test binary.

---

## 3. Cost per mutant (compile time)

**Measured on this machine** (Linux 5.15, Go 1.26.2, amd64):

| Package size | `go build -gcflags=-S` time |
|-------------|---------------------------|
| 1 function (~4 lines) | 80–150ms |
| 50 functions (~51 lines) | 100–160ms (warm cache: ~40ms) |

The per-package **reference** compile happens once and is memoized. Each surviving mutant adds one `go build -S` compile. With warm Go build cache, subsequent compiles of the same package (with only a patched file) are ~40–100ms.

**Viability inside a routing loop:**
- TCE only runs on **surviving** mutants, not all mutants. In a well-tested codebase, survivors are a small fraction.
- With parallel workers (gomutants uses bounded concurrency), N survivors across P packages = P reference compiles + N mutant compiles, parallelizable.
- At ~100ms/compile with 4 workers, 100 survivors = ~2.5s total — well within CI gate budgets.
- The dependency build cache is shared between consecutive survivors in the same package, amortizing import compilation.

**For `gopher_mutant`:** Since gopher_mutant uses tree-sitter (not `go test -overlay`) to generate mutants, the TCE pass would need to invoke `go build -gcflags=-S` with an overlay or temp-file approach. The cost is the same: ~100ms per surviving mutant compile. This is **viable** as a post-test classification pass.

---

## 4. Fallback if normalization were infeasible

Assembly comparison **is** the viable path, so this section covers the hypothetical case where `-S` output couldn't be normalized.

### 4.1 `go vet` / static analysis

`go vet` is a **linter**, not an equivalence prover. It detects suspicious constructs (printf format mismatches, shadowed variables, unreachable code) but does **not** compare two source files for behavioral equivalence.

**Experiment:** `go vet` passes identically on `a + b`, `a + b + 0`, and `a - b` — it cannot distinguish equivalent from non-equivalent mutants.

**Verdict:** Not a viable TCE fallback.

### 4.2 Object file (`go tool objdump`) comparison

`go tool objdump` disassembles `.o` files. After normalizing paths and stripping absolute addresses, the instruction sequences can be compared. However, the output includes relocated addresses that differ between compiles even for identical code, making byte comparison unreliable.

**Experiment:** `go tool objdump` on `a + b` vs `a + b + 0` (equivalent) shows different addresses (`0x68b` vs `0x68c`) even though instructions match. Stripping to instruction-only (`awk '{print $NF}'`) produces a **false positive** (a + b vs a - b also shows identical when only the last field is compared, because the last field is `RET` for both).

**Verdict:** Possible but requires careful normalization of addresses + full instruction columns, not just the mnemonic. Less reliable than `-S` assembly comparison.

### 4.3 Semantic comparison (AST-level)

A tree-sitter-based AST comparison (what `gopher_mutant` already does for mutation generation) could be extended to detect equivalent mutants by pattern-matching known-equivalent transformations (e.g., `x + 0 → x`, `x * 1 → x`, `!false → true`). This is **incomplete** (can't cover all compiler foldings) but **fast** (no compilation needed).

**Verdict:** Useful as a cheap pre-filter before TCE, but not a replacement. Complementary, not fallback.

---

## 5. Concrete normalization recipe for `gopher_mutant`

```
VERDICT: GO

Recipe:
1. After testing, collect surviving (LIVED) mutants.
2. Group survivors by package (import path).
3. For each package:
   a. Compile reference:
      go build -gcflags=<importPath>=-S -o /dev/null <importPath> 2>ref.txt
      ref_hash = sha256(normalize(ref.txt, importPath))
   b. For each surviving mutant in this package:
      - Apply the mutation to a temp copy of the source file.
      - Write overlay JSON: { "Replace": { "<originalSourcePath>": "<tempFilePath>" } }
      - Compile:
        go build -gcflags=<importPath>=-S -overlay=<overlay.json> -o /dev/null <importPath> 2>mut.txt
        mut_hash = sha256(normalize(mut.txt, importPath))
      - If mut_hash == ref_hash → classify EQUIVALENT

normalize(asm, importPath):
  - Split into lines
  - For each line: trim trailing whitespace
  - Skip the line "# <importPath>"
  - Hash remaining lines with SHA-256

Key flags:
  -gcflags=<importPath>=-S   # assembly output, scoped to the target package
  -overlay=<overlay.json>     # mutant compiled from same path as original
  -o /dev/null                # discard the binary
  -tags=<tags>                # match test run's build tags

Soundness: one-sided. Identical assembly ⇒ equivalent (proven).
           Different assembly ⇒ left as LIVED (might be equivalent, safe).
```

---

## 6. Comparison: dart_mutant TCE vs Go TCE

| Aspect | dart_mutant | gopher_mutant (proposed) |
|--------|------------|------------------------|
| Compilation target | Dart kernel (bytecode) | Go assembly (`-S`) |
| Comparison unit | Bytecode bytes | Normalized assembly text hash |
| Equivalence proof | Identical bytecode ⇒ identical behavior | Identical assembly ⇒ identical machine code ⇒ identical behavior |
| Determinism | Dart kernel is deterministic | Go `-S` is deterministic; `-overlay` ensures path-identical compiles |
| Normalization needed | Minimal (strip build metadata) | Strip `# <importPath>` header + trailing whitespace only |
| Soundness | One-sided | One-sided (same guarantee) |
| Cost per mutant | Kernel compile | `go build -S` (~100ms) |

The approaches are structurally identical: compile both original and mutant to an intermediate representation, normalize, compare. Go uses assembly instead of bytecode, but the principle is the same.

---

## References

- `gomutants` TCE source: [`internal/tce/tce.go`](https://github.com/szhekpisov/gomutants/blob/main/internal/tce/tce.go)
- gomutants TCE PR: [#56](https://github.com/szhekpisov/gomutants/pull/56) (merged 2026-07-18)
- Go compiler flags: `go tool compile -h` (for `-S`, `-trimpath`, `-dwarf`, `-p`)
- `go build -overlay` documentation: `go help build`
- `dart_mutant` TCE: compile original + mutant to Dart kernel, compare bytecode (sibling project)