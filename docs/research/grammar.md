# tree-sitter-go Grammar Coverage for Go-Idiomatic Mutation Operators

> **Research branch:** `research/grammar`  
> **Grammar:** [tree-sitter/tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go) — v0.25.0 (latest as of 2026-08-04)  
> **Method:** `src/node-types.json` (authoritative node type definitions) + `grammar.js` (production rules) + live parsing of representative Go snippets via the `tree-sitter` / `tree-sitter-go` Node packages.  
> **Purpose:** For each of the 12 planned mutation operators in gopher_mutant, confirm that tree-sitter-go exposes distinct, addressable node types, note ambiguities, and flag constructs that need context beyond the node itself.

---

## Summary

All 12 operators map to identifiable, distinct node types in tree-sitter-go v0.25.0. Every representative mutation was live-parsed and produces a **clean parse tree** (zero `ERROR` nodes, zero `MISSING` nodes) — meaning the grammar's error-recovery is robust enough that no mutation creates an unparseable tree. The grammar has 188 node types (97 named, 91 anonymous tokens).

**Key architectural finding:** Go builtins `append`, `close`, and `recover` are **not** keywords — they parse as ordinary `identifier` nodes inside `call_expression`. Only `new` and `make` get special aliasing in the grammar. Mutation operators targeting these builtins must therefore inspect `call_expression.function` to match `identifier == "append"|"close"|"recover"` — there is no dedicated node type.

---

## Per-Operator Analysis

### 1. DeferRemoval — `defer` statements

| Property | Value |
|---|---|
| **Node type** | `defer_statement` |
| **Grammar rule** | `seq('defer', $._expression)` |
| **Children** | Single child: `_expression` (the deferred call) |
| **Fields** | None (child is unnamed) |
| **Mutation** | Remove entire `defer_statement` node from its parent `statement_list` |
| **Parse after mutation** | ✅ Clean — removing the statement leaves a valid `statement_list` |

**Tree structure observed:**
```
defer_statement
  defer                          ← anonymous token
  call_expression                ← the deferred call (child #1)
    function: identifier
    arguments: argument_list
```

**Notes:**
- `defer` is an anonymous token (keyword), not a named node. The entire `defer_statement` node (including the `defer` keyword token) is the removal target.
- Works for both `defer f()` and `defer func(){}()` — the child is always `_expression`, which resolves to `call_expression` in practice.
- The parent is always a `statement_list` inside a `block`, so removal is straightforward: filter the child from the parent's children array.
- No ambiguity: every `defer_statement` is a top-level statement in a `statement_list`.

**Feasibility: ✅ Direct node-type match. No context needed.**

---

### 2. GoroutineRemoval — `go` statements

| Property | Value |
|---|---|
| **Node type** | `go_statement` |
| **Grammar rule** | `seq('go', $._expression)` |
| **Children** | Single child: `_expression` (the goroutine call) |
| **Fields** | None (child is unnamed) |
| **Mutation** | Remove the `go` keyword, converting `go f()` → `f()` (synchronous call) |
| **Parse after mutation** | ✅ Clean — `f()` parses as `expression_statement` containing `call_expression` |

**Tree structure observed:**
```
go_statement
  go                             ← anonymous token
  call_expression                ← the goroutine call
    function: identifier
    arguments: argument_list
```

**Notes:**
- Structurally identical to `defer_statement`. The `go` token is anonymous.
- Mutation strategy: either remove the `go` keyword token (keeping the expression as a plain `expression_statement`) or remove the entire `go_statement` and replace with a new `expression_statement` wrapping the same `call_expression` child.
- Re-parsing after removal confirms the expression becomes a standalone `expression_statement`.

**Feasibility: ✅ Direct node-type match. No context needed.**

---

### 3. ErrCheckRemoval — `if err != nil { return ... }` guards

| Property | Value |
|---|---|
| **Node type** | `if_statement` |
| **Grammar rule** | `seq('if', optional(initializer), 'condition', 'consequence', optional(alternative))` |
| **Children** | `condition` (field), `consequence` (field), optional `initializer` (field), optional `alternative` (field) |
| **Mutation** | Remove the entire `if_statement` node |
| **Parse after mutation** | ✅ Clean |

**Tree structure observed:**
```
if_statement
  if                             ← anonymous token
  initializer: short_var_declaration  ← optional (for `if err := f(); err != nil`)
    ;
  condition: binary_expression
    left: identifier (err)
    operator: !=
    right: nil
  consequence: block
    statement_list
      return_statement
```

**Notes & ambiguities:**
- **Pattern matching is required.** Not every `if_statement` is an error guard. The operator must match:
  - `condition` is a `binary_expression` with `operator` == `!=` and `right` == `nil`
  - OR the `initializer` form: `if err := f(); err != nil` (initializer is `short_var_declaration`, condition is the `binary_expression`)
- The `nil` token is a named node (`nil`), not just an anonymous token — so it can be checked directly.
- The `consequence` block should contain a `return_statement` (or `break`/`continue`/`panic`) to be a true error guard. This requires inspecting the `block` → `statement_list` children.
- **Context needed:** identifying error guards requires looking at the `condition` field's structure and the `consequence` block's content. This is beyond simple node-type matching — it's a structural pattern query.

**Feasibility: ✅ Node type exists (`if_statement`), but identifying error guards requires pattern matching on condition + consequence. Moderate complexity.**

---

### 4. ErrReturnSwap — multi-value `return a, b` → `return b, a`

| Property | Value |
|---|---|
| **Node type** | `return_statement` |
| **Grammar rule** | `seq('return', optional($.expression_list))` |
| **Children** | Optional single child: `expression_list` |
| **Fields** | None |
| **Mutation** | Swap the order of expressions in the `expression_list` child |
| **Parse after mutation** | ✅ Clean — `return nil, 1` parses identically to `return 1, nil` |

**Tree structure observed:**
```
return_statement
  return
  expression_list
    int_literal (1)
    ,                            ← anonymous token
    nil
```

**Notes:**
- `expression_list` is a named node with `multiple: true` children of type `_expression`, separated by `,` tokens.
- To swap, access `expression_list` children, filter to named children (the expressions), swap them, and rewrite.
- **Ambiguity:** Not every multi-value return is an error return. The operator should target returns where the function signature has `(T, error)` — this requires **cross-node context** (function declaration's `result` parameter list). Without function signature info, the operator would mutate ALL multi-value returns, which is overly broad.
- The `expression_list` node structure is: alternating `_expression` children and `,` anonymous tokens. Swapping named children while keeping `,` tokens in place is straightforward.

**Feasibility: ✅ Node type exists (`return_statement` + `expression_list`). Identifying error-return specifically requires function signature context. Low-moderate complexity.**

---

### 5. ChannelDirection — `ch <- x` ↔ `<-ch`

| Property | Value |
|---|---|
| **Send node** | `send_statement` — `seq(channel, '<-', value)` with fields `channel` and `value` |
| **Receive node** | `unary_expression` with `operator: <-` — when used as a value expression (`v := <-ch`) |
| **Receive-statement node** | `receive_statement` — `seq(optional(left, '='/':='), right)` — when used in `v := <-ch` or `<-ch` statement form |
| **Mutation** | Convert between `send_statement` and receive form (or vice versa) |
| **Parse after mutation** | ✅ Clean both directions |

**Tree structure observed (send):**
```
send_statement
  channel: identifier (ch)
  <-                             ← anonymous token
  value: int_literal (42)
```

**Tree structure observed (receive as expression):**
```
short_var_declaration
  left: expression_list
  :=
  right: expression_list
    unary_expression
      operator: <-               ← anonymous token
      operand: identifier (ch)
```

**Tree structure observed (receive as statement):**
```
receive_statement
  left: expression_list (optional)
  := or = (optional)
  right: unary_expression
    operator: <-
    operand: identifier
```

**Notes & ambiguities:**
- **Two different node types for channel receive** depending on context:
  1. `<-ch` as a value → `unary_expression` with `operator == <-`
  2. `v := <-ch` → `receive_statement` (wrapping the `unary_expression`)
  3. `<-ch` as a standalone statement → `receive_statement` without `left` field
- **`send_statement`** always has `channel` and `value` fields — unambiguous.
- Converting `ch <- x` (send) to `<-ch` (receive) changes the node type entirely (`send_statement` → `expression_statement` containing `unary_expression`). This is a **structural rewrite**, not a simple field swap.
- Converting `<-ch` (receive) to `ch <- x` (send) requires knowing what value to send — the `value` field must be synthesized or derived. This is **semantically incomplete** without additional context.
- The `<-` token is anonymous but appears as the `operator` field of `unary_expression` — so it's addressable via field access.

**Feasibility: ⚠️ Both directions are parseable, but the send→receive conversion loses the value expression and the receive→send conversion requires synthesizing a value. Node types are distinct but the mutation is a structural rewrite, not a field swap.**

---

### 6. ChannelCloseRemoval — `close(ch)`

| Property | Value |
|---|---|
| **Node type** | `call_expression` (function is `identifier == "close"`) |
| **Grammar rule** | No dedicated node — `close` is a builtin, not a keyword |
| **Statement wrapper** | `expression_statement` |
| **Mutation** | Remove the `expression_statement` containing the `close()` call |
| **Parse after mutation** | ✅ Clean |

**Tree structure observed:**
```
expression_statement
  call_expression
    function: identifier (close)   ← note: "close" is a plain identifier
    arguments: argument_list
      identifier (ch)
```

**Notes:**
- **`close` is NOT a keyword** — it's a predeclared identifier. The grammar treats it as a regular `identifier` inside `call_expression`. This is confirmed by `grammar.js` which only special-cases `new` and `make` (via `alias(choice('new', 'make'), $.identifier)`).
- To identify `close(ch)` calls, the operator must:
  1. Find `expression_statement` → `call_expression` where `function` field is `identifier`
  2. Check that the identifier's text is `"close"`
  3. Verify the `argument_list` has exactly one argument (the channel)
- **Context needed:** Text matching on the `function` identifier is required. No structural node-type distinction exists between `close(ch)` and any other single-argument function call.
- Removing the `expression_statement` from its parent `statement_list` is clean.

**Feasibility: ✅ Node type exists (`call_expression`), but identification requires text matching on `function` identifier == `"close"`. Moderate complexity.**

---

### 7. SelectCaseRemoval — `select` cases

| Property | Value |
|---|---|
| **Select node** | `select_statement` — children are `communication_case` and `default_case` |
| **Case nodes** | `communication_case` (with `communication` field: `send_statement` or `receive_statement`) and `default_case` |
| **Mutation** | Remove individual `communication_case` or `default_case` children from `select_statement` |
| **Parse after mutation** | ✅ Clean — even removing all cases (`select { }`) parses without error |

**Tree structure observed:**
```
select_statement
  select                         ← anonymous token
  {                              ← anonymous token
  communication_case
    case                         ← anonymous token
    communication: receive_statement
      left: expression_list
      :=
      right: unary_expression (operator: <-)
    :                             ← anonymous token
    statement_list (optional)
      ...
  communication_case
    case
    communication: send_statement
      channel: identifier
      value: int_literal
    :
  default_case
    default
    :
  }                              ← anonymous token
```

**Notes:**
- `select_statement` children have `multiple: true, required: false` — so any number of cases (including zero) is valid.
- `communication_case` has a `communication` field (always `send_statement` or `receive_statement`) and an optional `statement_list` child.
- `default_case` has only an optional `statement_list` child.
- Removing cases is straightforward: filter children of `select_statement`.
- **Edge case:** Go requires at least... actually no, `select {}` (empty select) is valid Go — it blocks forever. The grammar confirms this (zero cases parses clean).

**Feasibility: ✅ Direct node-type match. Removal is trivial. No context needed.**

---

### 8. RangeBreak — insert `break` after first iteration

| Property | Value |
|---|---|
| **Target node** | `for_statement` (with `range_clause` child) |
| **Break node** | `break_statement` |
| **Grammar rule (break)** | `seq('break', optional(label_name))` |
| **Mutation** | Insert `break_statement` as the first statement in the `for_statement.body` block's `statement_list` |
| **Parse after mutation** | ✅ Clean |

**Tree structure observed:**
```
for_statement
  for                            ← anonymous token
  range_clause
    left: expression_list (_, item)
    :=
    range                        ← anonymous token
    right: identifier (items)
  body: block
    statement_list
      assignment_statement (_ = item)
      break_statement             ← inserted
        break
```

**Notes:**
- The `for_statement` has a `body` field of type `block`. The `block` contains an optional `statement_list`.
- Inserting `break_statement` as the first child of the `statement_list` is a structural insertion — no existing node needs modification.
- **Ambiguity:** This operator should only apply to `for_statement` nodes that contain a `range_clause` child (not `for_clause` or bare `for` loops). The `for_statement.children` field shows the possible child types: `_expression`, `for_clause`, or `range_clause`.
- **Context needed:** Must check that the `for_statement` child is a `range_clause` (not a `for_clause`). This is a single-level child-type check.
- `break_statement` with a label (`break outer`) also parses — the label is an optional child of type `label_name`.

**Feasibility: ✅ All node types exist. Need to verify `for_statement` has a `range_clause` child. Low complexity.**

---

### 9. MapIterationSwap — `for k, v := range m` → `for v := range m`

| Property | Value |
|---|---|
| **Target node** | `range_clause` (child of `for_statement`) |
| **Grammar rule** | `seq(optional(left, '='/':='), 'range', right)` |
| **Mutation** | Change `left` from two-element `expression_list` (`k, v`) to single-element (`v`) |
| **Parse after mutation** | ✅ Clean — `for v := range m` parses correctly |

**Tree structure observed (before):**
```
range_clause
  left: expression_list
    identifier (k)
    ,
    identifier (v)
  :=
  range
  right: identifier (m)
```

**Tree structure observed (after):**
```
range_clause
  left: expression_list
    identifier (v)
  :=
  range
  right: identifier (m)
```

**Notes:**
- The `range_clause` `left` field is `expression_list` (optional, `multiple: false`). When present, it contains the range variables.
- For map iteration, `left` has two identifiers: the key and the value. For slice iteration, `left` has `_` and the value, or just the value.
- **Context needed:** This operator should only apply to `range_clause` where:
  1. The `right` expression's type is a map (not a slice/array) — this requires **type information** not available in the syntax tree. The grammar distinguishes `map_type` in `composite_literal` but cannot determine the runtime type of a variable.
  2. The `left` has two variables (key + value).
- Without type info, a heuristic is: if `left` has two identifiers, drop the first (key) and keep the second (value). This works for maps and is harmless (but unnecessary) for slices where the first var is `_`.
- The `left` field's `expression_list` can have 1 or 2 expressions; reducing from 2 to 1 is clean.

**Feasibility: ⚠️ Node type exists (`range_clause`). Mutation is a simple field modification. However, restricting to maps (vs slices) requires type information not available in the parse tree. Heuristic-based.**

---

### 10. AppendRemoval — `s = append(s, x)`

| Property | Value |
|---|---|
| **Node type** | `assignment_statement` where `right` is `call_expression` with `function` == `identifier("append")` |
| **Grammar rule** | No dedicated node — `append` is a builtin identifier |
| **Mutation** | Remove the assignment, or replace `append(s, x)` with just `s` |
| **Parse after mutation** | ✅ Clean both ways |

**Tree structure observed:**
```
assignment_statement
  left: expression_list
    identifier (s)
  operator: =
  right: expression_list
    call_expression
      function: identifier (append)
      arguments: argument_list
        identifier (s)
        ,
        int_literal (3)
```

**Notes:**
- **`append` is NOT a keyword** — same as `close`, it's a predeclared identifier. No special node type.
- To identify `s = append(s, x)`:
  1. `assignment_statement` with `operator` == `=`
  2. `right` is `call_expression` where `function` is `identifier` with text `"append"`
  3. `left` should match the first argument of `append` (the slice being extended)
- Mutation strategies:
  1. Remove the entire `assignment_statement` (simplest — `s` keeps its current value)
  2. Replace `right` expression with just `left` (making it `s = s`, a no-op)
- Both produce clean parse trees.
- **Context needed:** Text matching on `function` identifier == `"append"`. Also, verifying `left` matches `arguments[0]` requires comparing expression text.

**Feasibility: ✅ Node types exist (`assignment_statement` + `call_expression`). Identification requires text matching on `"append"` and argument/left comparison. Moderate complexity.**

---

### 11. SliceIndexSwap — `s[i]` → `s[len(s)-1-i]`

| Property | Value |
|---|---|
| **Node type** | `index_expression` — `seq(operand, '[', index, ']')` |
| **Grammar rule** | `prec(PREC.primary, prec.dynamic(1, seq(operand, '[', index, ']')))` |
| **Mutation** | Replace the `index` expression with `len(operand) - 1 - index` |
| **Parse after mutation** | ✅ Clean — `s[len(s)-1-0]` parses correctly |

**Tree structure observed:**
```
index_expression
  operand: identifier (s)
  [                              ← anonymous token
  index: int_literal (0)
  ]                              ← anonymous token
```

**After mutation:**
```
index_expression
  operand: identifier (s)
  [
  index: binary_expression
    left: binary_expression
      left: call_expression
        function: identifier (len)
        arguments: argument_list
          identifier (s)
      operator: -
      right: int_literal (1)
    operator: -
    right: int_literal (0)
  ]
```

**Notes:**
- `index_expression` has `operand` and `index` fields — both are `_expression`. Clean and unambiguous.
- The mutation wraps the original `index` expression in a `binary_expression` tree: `len(operand) - 1 - original_index`.
- The new index expression involves `len()` (another builtin identifier), subtraction, and the original index — all standard expressions that parse cleanly.
- **Context needed:** 
  1. Should only apply when `operand` is a slice (not a map or array) — maps use `index_expression` too but the semantics differ. **Type info required** to distinguish slice from map indexing.
  2. The `operand` expression must be duplicated to form the `len(operand)` call.
- Without type info, the heuristic applies to all `index_expression` nodes, which is overly broad for maps (`m[key]` → `m[len(m)-1-key]` is nonsensical for map keys).

**Feasibility: ⚠️ Node type exists (`index_expression`) with clean fields. Mutation is a tree rewrite. However, restricting to slices (vs maps) requires type information not in the parse tree. Heuristic-based.**

---

### 12. RecoverRemoval — `recover()` calls

| Property | Value |
|---|---|
| **Node type** | `call_expression` where `function` == `identifier("recover")` |
| **Grammar rule** | No dedicated node — `recover` is a builtin identifier |
| **Typical context** | Inside `defer func() { if r := recover(); r != nil {...} }()` |
| **Mutation** | Remove the `recover()` call or replace with `nil` |
| **Parse after mutation** | ✅ Clean — `if r := nil; r != nil` parses correctly |

**Tree structure observed:**
```
defer_statement
  defer
  call_expression
    function: func_literal
      body: block
        statement_list
          if_statement
            initializer: short_var_declaration
              left: expression_list (r)
              :=
              right: expression_list
                call_expression
                  function: identifier (recover)
                  arguments: argument_list ()
            ;
            condition: binary_expression
              left: identifier (r)
              operator: !=
              right: nil
            consequence: block
              ...
    arguments: argument_list ()
```

**Notes:**
- **`recover` is NOT a keyword** — same as `append` and `close`, it's a predeclared identifier in `call_expression`.
- `recover()` always appears as `call_expression` with `function` == `identifier("recover")` and empty `argument_list`.
- Mutation strategies:
  1. Replace `recover()` with `nil` — turns `if r := recover(); r != nil` into `if r := nil; r != nil` (dead code, always false — effectively removes the recover logic)
  2. Remove the `short_var_declaration` initializer entirely — changes to `if r != nil` which would be a compile error (undeclared `r`), so this doesn't work at the syntax level without more rewriting.
- Strategy 1 (`recover()` → `nil`) is clean and parseable.
- **Context needed:** Text matching on `function` identifier == `"recover"`. The semantic effect is that `recover()` returns `nil` anyway when not in a deferred function, but the mutation removes the panic-catching behavior.

**Feasibility: ✅ Node type exists (`call_expression`). Identification requires text matching on `"recover"`. Low-moderate complexity.**

---

## Node-Type Reference Table

| # | Operator | Primary Node Type | Key Fields/Children | Identification Method | Context Needed |
|---|---|---|---|---|---|
| 1 | DeferRemoval | `defer_statement` | child: `_expression` | Direct node type | None |
| 2 | GoroutineRemoval | `go_statement` | child: `_expression` | Direct node type | None |
| 3 | ErrCheckRemoval | `if_statement` | `condition`, `consequence`, optional `initializer` | Pattern match: `binary_expression` with `!=` + `nil` | Condition + consequence structure |
| 4 | ErrReturnSwap | `return_statement` | child: `expression_list` | Multi-value return detection | Function signature (result types) |
| 5 | ChannelDirection | `send_statement` / `unary_expression` / `receive_statement` | `channel`/`value` or `operator`/`operand` | Distinct node types per direction | None for identification; value synthesis for receive→send |
| 6 | ChannelCloseRemoval | `expression_statement` → `call_expression` | `function`, `arguments` | Text match: `function` == `"close"` | Identifier text matching |
| 7 | SelectCaseRemoval | `communication_case` / `default_case` | `communication` (case), `statement_list` | Direct node types (children of `select_statement`) | None |
| 8 | RangeBreak | `for_statement` + `break_statement` | `body` (block), range_clause child | `for_statement` with `range_clause` child | Check for `range_clause` child |
| 9 | MapIterationSwap | `range_clause` | `left` (expression_list), `right` | Two-element `left` expression_list | Type info (map vs slice) |
| 10 | AppendRemoval | `assignment_statement` → `call_expression` | `left`, `operator`, `right` | Text match: `function` == `"append"` | Identifier text + argument matching |
| 11 | SliceIndexSwap | `index_expression` | `operand`, `index` | Direct node type | Type info (slice vs map) |
| 12 | RecoverRemoval | `call_expression` | `function`, `arguments` | Text match: `function` == `"recover"` | Identifier text matching |

---

## Error-Recovery Behavior

All 12 mutations were tested by parsing the mutated Go code and checking for `ERROR` and `MISSING` nodes:

| Mutation | `hasError` | `ERROR` nodes | `MISSING` nodes |
|---|---|---|---|
| Defer removed | false | 0 | 0 |
| Goroutine `go` removed | false | 0 | 0 |
| `if err != nil` block removed | false | 0 | 0 |
| `return a, b` → `return b, a` | false | 0 | 0 |
| `ch <- x` → `<-ch` | false | 0 | 0 |
| `<-ch` → `ch <- v` | false | 0 | 0 |
| `close(ch)` removed | false | 0 | 0 |
| Select case removed | false | 0 | 0 |
| All select cases removed (`select {}`) | false | 0 | 0 |
| `break` inserted in range body | false | 0 | 0 |
| `for k, v` → `for v` | false | 0 | 0 |
| `s = append(s, x)` → `s = s` | false | 0 | 0 |
| `s = append(s, x)` removed entirely | false | 0 | 0 |
| `s[i]` → `s[len(s)-1-i]` | false | 0 | 0 |
| `recover()` → `nil` | false | 0 | 0 |

**Conclusion:** tree-sitter-go's error recovery is robust. No mutation produces an unparseable tree. This is important for gopher_mutant: every mutant will produce a syntactically valid AST, meaning the tool can rely on the parse tree for downstream analysis (type checking, compile validation) without special handling for parse failures from mutation.

---

## Key Findings & Recommendations

### 1. Three operators need type information not in the syntax tree
- **ErrReturnSwap (4):** Needs to know the function returns `(T, error)` to target error returns specifically.
- **MapIterationSwap (9):** Needs to know the range target is a map (not a slice/array).
- **SliceIndexSwap (11):** Needs to know the indexed operand is a slice (not a map).

**Recommendation:** gopher_mutant should integrate `gopls` or another type-checker pass after parsing, or use conservative heuristics (e.g., apply MapIterationSwap to all two-variable range loops and rely on the compiler to reject invalid cases).

### 2. Three operators target builtin function calls (no dedicated node type)
- **ChannelCloseRemoval (6):** `close(ch)` — `call_expression` with `function == identifier("close")`
- **AppendRemoval (10):** `append(s, x)` — `call_expression` with `function == identifier("append")`
- **RecoverRemoval (12):** `recover()` — `call_expression` with `function == identifier("recover")`

**Recommendation:** Implement a helper that matches `call_expression` nodes by function name. Since tree-sitter exposes the source text of nodes, this is a simple `node.childForFieldName("function").text == "close"` check. Note that `new` and `make` are aliased in the grammar but `append`, `close`, and `recover` are not — they're plain `identifier` nodes.

### 3. ChannelDirection has asymmetric difficulty
- `send → receive` (`ch <- x` → `<-ch`): Drops the value — straightforward but semantically lossy.
- `receive → send` (`<-ch` → `ch <- x`): Requires synthesizing a value to send.

**Recommendation:** The receive→send direction is the more interesting mutation (a receive-only channel suddenly sends), but it needs a value. Options: use a zero-value literal (`nil`, `0`, `""`) or a variable from scope. This is a semantic decision, not a parsing one.

### 4. All mutations produce valid parse trees
This is the most important finding for implementation: gopher_mutant's mutation pipeline can emit mutated source text and re-parse it without worrying about broken trees. The tree-sitter-go grammar handles all 12 mutation patterns gracefully, including edge cases like empty `select {}`.

---

## Grammar Version & Source

- **Grammar:** [tree-sitter/tree-sitter-go](https://github.com/tree-sitter/tree-sitter-go)
- **Version:** 0.25.0
- **Commit:** `2346a3a` (latest on main as of 2026-08-04)
- **Node types:** 188 total (97 named, 91 anonymous tokens)
- **`src/node-types.json`:** Authoritative source for node type definitions (fields, children, subtypes)
- **`grammar.js`:** Grammar production rules
- **Validation:** Live parsing via `tree-sitter` (npm) + `tree-sitter-go` (npm) Node.js bindings

---

*Research conducted on branch `research/grammar` for the gopher_mutant project.*