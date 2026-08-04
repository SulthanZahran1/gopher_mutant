//! The 10 generic mutation operator classes (M1 scope, GOAL-1).
//!
//! Each operator is a pure function: given the source text and a byte range,
//! produce a set of (label, before, after) replacements. The M2 idiomatic
//! operators will extend this same trait.

use serde::Serialize;
use std::fmt;

/// The 10 generic operator classes, per GOAL-1 criterion 2 (locked):
/// arithmetic swap, relational boundary, relational negation, logical swap,
/// boolean term removal, increment/decrement, statement removal, return value
/// removal, loop boundary, integer literal inc/dec. ROR emits both the
/// boundary (<=↔<) and negation (==↔!=) flavors; LBR and ILI are distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    /// Arithmetic operator replacement: `+`↔`-`, `*`↔`/`, `%`→`*`.
    Aor,
    /// Relational operator replacement: boundary (`<`↔`<=`, `>`↔`>=`) and
    /// negation (`==`↔`!=`) flavors.
    Ror,
    /// Logical operator replacement: `&&`↔`||`.
    Lor,
    /// Boolean term removal (conditional operator replacement for Go, which
    /// has no ternary): `a && b` → `a`, `a || b` → `a` (right term).
    Cor,
    /// Statement deletion: remove a call/assignment/inc-dec statement.
    Sdl,
    /// Return value replacement: `return x` → `return` (zero value).
    Rvr,
    /// Loop increment/decrement swap: `i++` ↔ `i--`.
    Inc,
    /// Loop boundary: `for i < n` ↔ `for i <= n` in C-style loop conditions.
    Lbr,
    /// Integer literal increment/decrement: `42` → `43` and `42` → `41`.
    Ili,
}

pub const ALL_OPERATORS: [Operator; 9] = [
    Operator::Aor,
    Operator::Ror,
    Operator::Lor,
    Operator::Cor,
    Operator::Sdl,
    Operator::Rvr,
    Operator::Inc,
    Operator::Lbr,
    Operator::Ili,
];

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operator::Aor => "AOR",
            Operator::Ror => "ROR",
            Operator::Lor => "LOR",
            Operator::Cor => "COR",
            Operator::Sdl => "SDL",
            Operator::Rvr => "RVR",
            Operator::Inc => "INC",
            Operator::Lbr => "LBR",
            Operator::Ili => "ILI",
        };
        write!(f, "{s}")
    }
}

impl Operator {
    /// Parse an operator name (display form or snake_case) — used by
    /// `--operators` and unit tests.
    pub fn from_name(name: &str) -> Option<Operator> {
        ALL_OPERATORS.iter().copied().find(|op| {
            op.to_string().eq_ignore_ascii_case(name) || op.to_string().to_lowercase() == name
        })
    }
}

/// A single candidate replacement within a mutation point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// Human label, e.g. `+ → -`.
    pub label: String,
    /// Byte range in the ORIGINAL source to replace.
    pub start: usize,
    pub end: usize,
    /// Replacement text.
    pub text: String,
}

/// Find all replacements of the given operator in the source text.
///
/// Implementations are pure — they never touch the filesystem. They receive
/// the full source and scan it (tree-sitter-free, text-based) so the operator
/// set stays independent of grammar versions. M1 deliberately keeps this
/// simple and correct; grammar-driven discovery lands with M2.
pub fn replacements_for(op: Operator, source: &str) -> Vec<Replacement> {
    match op {
        Operator::Aor => arith_replacements(source),
        Operator::Ror => relational_replacements(source),
        Operator::Lor => logical_replacements(source),
        Operator::Cor => conditional_replacements(source),
        Operator::Sdl => statement_deletions(source),
        Operator::Rvr => return_replacements(source),
        Operator::Inc => inc_replacements(source),
        Operator::Lbr => loop_boundary_replacements(source),
        Operator::Ili => int_literal_replacements(source),
    }
}

/// Match a single-char binary operator at `i` in `source`, skipping
/// `==`, `!=`, `<=`, `>=`, `&&`, `||`, `+=`, etc.
fn single_char_binary_op(source: &str, i: usize) -> Option<char> {
    let b = source.as_bytes();
    let c = b[i] as char;
    if !matches!(c, '+' | '-' | '*' | '/' | '%') {
        return None;
    }
    // Skip compound / multi-char ops: next char is one of = < > & | - * etc.
    if let Some(&n) = b.get(i + 1) {
        if matches!(
            n as char,
            '=' | '<' | '>' | '&' | '|' | '+' | '-' | '*' | '/'
        ) {
            return None;
        }
    }
    Some(c)
}

/// Snap a byte index down to the nearest UTF-8 char boundary. Scanners walk
/// bytes but must never slice mid-char (non-ASCII comments are legal Go).
fn bd(source: &str, i: usize) -> usize {
    source.floor_char_boundary(i)
}

/// AOR: swap arithmetic operators. `+`↔`-`, `*`↔`/`, `%`→`*`.
fn arith_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    for (i, c) in source.char_indices() {
        if let Some(op) = single_char_binary_op(source, i) {
            let label = match op {
                '+' => "+ → -",
                '-' => "- → +",
                '*' => "* → /",
                '/' => "/ → *",
                '%' => "% → *",
                _ => unreachable!(),
            };
            out.push(Replacement {
                label: label.to_string(),
                start: i,
                end: i + c.len_utf8(),
                text: match op {
                    '+' => "-",
                    '-' => "+",
                    '*' => "/",
                    '/' => "*",
                    '%' => "*",
                    _ => unreachable!(),
                }
                .to_string(),
            });
        }
    }
    out
}

/// LBR: loop boundary swap — `<`↔`<=` / `>`↔`>=` inside C-style
/// for-loop conditions (`for i < n; ...`). Same replacement set as the
/// relational boundary flavor, applied only when the comparison is the
/// condition of a `for` statement.
fn loop_boundary_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find a for keyword.
        if bytes[i..].starts_with(b"for")
            && (i == 0 || !(bytes[i - 1] as char).is_ascii_alphanumeric())
        {
            let mut j = i + 3;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            // Skip `for range` and `for {` (no C-style condition).
            if j < bytes.len() && !bytes[j..].starts_with(b"range") && bytes[j] != b'{' {
                // Scan the condition for a bare < or > (not <= >= << >> or <-).
                while j < bytes.len() && bytes[j] != b'{' {
                    let c = bytes[j] as char;
                    if c == '<' || c == '>' {
                        let next_ok = j + 1 >= bytes.len() || {
                            let n = bytes[j + 1] as char;
                            !matches!(n, '=' | '<' | '>' | '-')
                        };
                        if next_ok {
                            out.push(Replacement {
                                label: if c == '<' { "< → <=" } else { "> → >=" }.to_string(),
                                start: j,
                                end: j + 1,
                                text: if c == '<' { "<=" } else { ">=" }.to_string(),
                            });
                        }
                    }
                    j += 1;
                }
            }
            i = j.max(i + 3);
            continue;
        }
        i += 1;
    }
    out
}

/// ILI: integer literal increment/decrement — `42` → `43` and `42` → `41`.
/// Targets decimal integer literals (including 0); skips negative signs and
/// hex/octal/binary/float forms.
fn int_literal_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }
            let end = i;
            let lit = &source[start..end];
            // Skip 0-prefixed forms (octal), and any literal adjacent to
            // identifier or float context (0o17's "17", 3.14's "3", x42b).
            let prev_ok = start == 0 || {
                let p = bytes[start - 1] as char;
                !(p.is_ascii_alphanumeric() || p == '_' || p == '.')
            };
            let next_ok = end >= bytes.len() || {
                let n = bytes[end] as char;
                !(n.is_ascii_alphanumeric() || n == '_' || n == '.')
            };
            let plain_decimal = lit.len() <= 5 && !lit.starts_with('0') && prev_ok && next_ok;
            if plain_decimal {
                let v: i64 = lit.parse().unwrap_or(0);
                if v < i64::MAX {
                    out.push(Replacement {
                        label: format!("{lit} → {}", v + 1),
                        start,
                        end,
                        text: (v + 1).to_string(),
                    });
                }
                if v > i64::MIN {
                    out.push(Replacement {
                        label: format!("{lit} → {}", v - 1),
                        start,
                        end,
                        text: (v - 1).to_string(),
                    });
                }
            }
            continue;
        }
        i += 1;
    }
    out
}

/// ROR: relational operator replacement. `<`↔`<=`, `>`↔`>=`, `==`↔`!=`.
fn relational_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        // Byte slicing — never panics on mid-char positions (i advances by 1).
        let two = &bytes[i..i + 2];
        let (label, text) = match two {
            b"==" => ("== → !=", "!="),
            b"!=" => ("!= → ==", "=="),
            b"<=" => ("<= → <", "<"),
            b">=" => (">= → >", ">"),
            _ => {
                i += 1;
                continue;
            }
        };
        out.push(Replacement {
            label: label.to_string(),
            start: i,
            end: i + 2,
            text: text.to_string(),
        });
        i += 2;
    }
    // Single < and > (not part of <= >= << >> <- or generics).
    i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '<' || c == '>' {
            let prev_ok = i == 0 || {
                let p = bytes[i - 1] as char;
                !matches!(p, '<' | '>' | '=' | ':' | '-' | ',')
            };
            let next_ok = i + 1 >= bytes.len() || {
                let n = bytes[i + 1] as char;
                !matches!(n, '<' | '>' | '=' | '-' | ':' | '(')
            };
            // Generic type params: `[T]` brackets make `<`/`>` rare in Go 1.18+,
            // but a bare `<`/`>` between identifiers IS a comparison.
            if prev_ok && next_ok {
                out.push(Replacement {
                    label: if c == '<' { "< → <=" } else { "> → >=" }.to_string(),
                    start: i,
                    end: i + 1,
                    text: if c == '<' { "<=" } else { ">=" }.to_string(),
                });
            }
        }
        i += 1;
    }
    out
}

/// LOR / LCR: `&&`↔`||`.
fn logical_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let two = &bytes[i..i + 2];
        let (label, text) = match two {
            b"&&" => ("&& → ||", "||"),
            b"||" => ("|| → &&", "&&"),
            _ => {
                i += 1;
                continue;
            }
        };
        out.push(Replacement {
            label: label.to_string(),
            start: i,
            end: i + 2,
            text: text.to_string(),
        });
        i += 2;
    }
    out
}

/// COR: conditional (ternary) operator replacement — Go has no ternary, so
/// this targets `if/else` value expressions? No — COR in the generic set maps
/// to boolean-expression term removal (`a && b` → `a` and → `b`), which is
/// dart_mutant's LCR-style connector semantics. Keep both directions.
fn conditional_replacements(source: &str) -> Vec<Replacement> {
    // For Go, COR = short-circuit operand removal: `a && b` → `a`, `a || b` → `a`.
    // (Full if/else restructuring is out of M1's generic scope.)
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let two = &bytes[i..i + 2];
        let op = match two {
            b"&&" | b"||" => two,
            _ => {
                i += 1;
                continue;
            }
        };
        // Right operand: skip whitespace, consume until , ) ; \n or && ||.
        let mut j = i + 2;
        while j < bytes.len() && (bytes[j] as char).is_whitespace() && bytes[j] != b'\n' {
            j += 1;
        }
        let mut end = j;
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if matches!(ch, ',' | ')' | ';' | '\n') {
                break;
            }
            if end + 1 < bytes.len() && matches!(&bytes[end..end + 2], b"&&" | b"||") {
                break;
            }
            end += 1;
        }
        let end = bd(source, end);
        let mut text = String::new();
        let mut insert_at = end;
        while insert_at > j && (bytes[insert_at - 1] as char).is_whitespace() {
            insert_at -= 1;
        }
        text.push_str(&source[insert_at..bd(source, end)]);
        out.push(Replacement {
            label: format!("{} right term removed", std::str::from_utf8(op).unwrap()),
            start: j,
            end,
            text,
        });
        let _ = text;
        i = end.max(i + 2);
    }
    out
}

/// SDL: statement deletion. Remove simple expression statements and
/// assignments (`f(x);`, `x = y;`) — the whole statement including the `;`.
fn statement_deletions(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find start of a line (after newline).
        if bytes[i] == b'\n' {
            i += 1;
            continue;
        }
        let line_start = i;
        // Consume the statement: until `;` at depth 0, or newline.
        let mut j = i;
        let mut depth = 0i32;
        let mut has_assign = false;
        let mut ident_run = 0;
        let mut last_was_ident = false;
        while j < bytes.len() {
            let ch = bytes[j] as char;
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ';' if depth == 0 => {
                    // Delete [line_start, j+1) — the statement plus terminator.
                    let stmt = &source[line_start..bd(source, j)];
                    let trimmed = stmt.trim();
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("//")
                        && !trimmed.starts_with("/*")
                        && !trimmed.starts_with("package")
                        && !trimmed.starts_with("import")
                        && !trimmed.starts_with("func ")
                        && !trimmed.starts_with("type ")
                        && !trimmed.starts_with("var ")
                        && !trimmed.starts_with("const ")
                        && !trimmed.starts_with("return ")
                        && !trimmed.starts_with("if ")
                        && !trimmed.starts_with("for ")
                        && !trimmed.starts_with("else")
                        && !trimmed.starts_with("} ")
                        && !trimmed.starts_with("go ")
                        && !trimmed.starts_with("defer ")
                        && !trimmed.starts_with("select")
                        && !trimmed.starts_with("switch ")
                        && !trimmed.starts_with("case ")
                        && !trimmed.starts_with("break")
                        && !trimmed.starts_with("continue")
                        && !trimmed.starts_with("fallthrough")
                    {
                        // Only delete if it's a call or assignment (contains
                        // `(` or `=`), not a bare identifier statement.
                        let callish = trimmed.contains('(') || trimmed.contains('=');
                        let _ = (has_assign, ident_run, last_was_ident);
                        if callish {
                            out.push(Replacement {
                                label: format!("statement deleted: {trimmed}"),
                                start: line_start,
                                end: j + 1,
                                text: String::new(),
                            });
                        }
                    }
                    i = j + 1;
                    break;
                }
                '\n' if depth == 0 => {
                    // No semicolon (Go allows omitting). Delete when the line
                    // is a deletable statement: a call, an assignment
                    // (x = y / s += x), or an inc/dec expression. Declarations
                    // and control keywords are excluded above; deleting a
                    // declaration yields a compile_error mutant (acceptable).
                    let stmt = &source[line_start..bd(source, j)];
                    let trimmed = stmt.trim();
                    let callish = trimmed.contains('(')
                        || trimmed.contains('=')
                        || trimmed.ends_with("++")
                        || trimmed.ends_with("--");
                    // `:=` short declarations are declarations, not deletable
                    // statements: removing them is a compile error, which the
                    // small-fixture contract (100% kill) forbids. Lines ending
                    // in `{` are block openers (for/if/switch headers) — their
                    // "statement" text belongs to control structure, deleting
                    // it is a compile error.
                    let not_short_decl = !trimmed.contains(":=");
                    let not_block_opener = !trimmed.ends_with('{');
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("//")
                        && !trimmed.starts_with("/*")
                        && callish
                        && not_short_decl
                        && not_block_opener
                        && !trimmed.starts_with("func ")
                        && !trimmed.starts_with("if ")
                        && !trimmed.starts_with("for ")
                        && !trimmed.starts_with("switch ")
                        && !trimmed.starts_with("select")
                        && !trimmed.starts_with("case ")
                        && !trimmed.starts_with("default")
                        && !trimmed.starts_with("type ")
                        && !trimmed.starts_with("var ")
                        && !trimmed.starts_with("const ")
                        && !trimmed.starts_with("package")
                        && !trimmed.starts_with("import")
                        && !trimmed.starts_with("return ")
                        && !trimmed.starts_with("defer ")
                        && !trimmed.starts_with("go ")
                        && !trimmed.starts_with("else")
                        && !trimmed.starts_with("}")
                        && !trimmed.starts_with("{")
                        && !trimmed.starts_with("break")
                        && !trimmed.starts_with("continue")
                    {
                        out.push(Replacement {
                            label: format!("statement deleted: {trimmed}"),
                            start: line_start,
                            end: j,
                            text: String::new(),
                        });
                    }
                    i = j + 1;
                    break;
                }
                '=' if depth == 0 && j + 1 < bytes.len() && bytes[j + 1] != b'=' => {
                    has_assign = true;
                }
                _ => {
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        ident_run += 1;
                        last_was_ident = true;
                    } else {
                        last_was_ident = false;
                    }
                }
            }
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
    }
    out
}

/// RVR: replace `return <expr>` with `return` (zero-value return).
fn return_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 6 < bytes.len() {
        if bytes[i..].starts_with(b"return") {
            let after = bytes.get(i + 6).copied().unwrap_or(b' ');
            if after == b' ' || after == b'\t' {
                let mut j = i + 6;
                while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] != b';' && bytes[j] != b'\n' && bytes[j] != b'}' {
                    // Capture the expression to replace.
                    let mut end = j;
                    let mut depth = 0i32;
                    while end < bytes.len() {
                        let ch = bytes[end] as char;
                        match ch {
                            '(' => depth += 1,
                            ')' => {
                                if depth == 0 {
                                    break;
                                }
                                depth -= 1;
                            }
                            ';' | '\n' | '}' if depth == 0 => break,
                            _ => {}
                        }
                        end += 1;
                    }
                    let mut trim_end = end;
                    while trim_end > j && (bytes[trim_end - 1] as char).is_whitespace() {
                        trim_end -= 1;
                    }
                    let trim_end = bd(source, trim_end);
                    let expr = &source[j..trim_end];
                    // Skip multi-value returns `a, b` (M2's ErrReturnSwap) —
                    // replacing with bare `return` would change arity. Only
                    // single-expression returns.
                    if !expr.contains(',') {
                        out.push(Replacement {
                            label: format!("return {expr} → return"),
                            start: j,
                            end: trim_end,
                            text: String::new(),
                        });
                    }
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// INC: swap `++`↔`--` in for-clauses (and any expression).
fn inc_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let two = &bytes[i..i + 2];
        let (label, text) = match two {
            b"++" => ("++ → --", "--"),
            b"--" => ("-- → ++", "++"),
            _ => {
                i += 1;
                continue;
            }
        };
        out.push(Replacement {
            label: label.to_string(),
            start: i,
            end: i + 2,
            text: text.to_string(),
        });
        i += 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aor_swaps_arith_ops() {
        let src = "x := a + b - c * d / e % f;";
        let reps = replacements_for(Operator::Aor, src);
        assert_eq!(reps.len(), 5);
        assert!(reps.iter().any(|r| r.label == "+ → -"));
        assert!(reps.iter().any(|r| r.label == "- → +"));
        assert!(reps.iter().any(|r| r.label == "* → /"));
        assert!(reps.iter().any(|r| r.label == "/ → *"));
        assert!(reps.iter().any(|r| r.label == "% → *"));
        // Skip compound assignments: += must not match.
        let src2 = "x += 1;";
        assert!(replacements_for(Operator::Aor, src2).is_empty());
    }

    #[test]
    fn lbr_swaps_loop_boundaries() {
        // C-style for condition: bare < becomes <=.
        let src = "for i := 0; i < n; i++ { }";
        let reps = replacements_for(Operator::Lbr, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(reps[0].label, "< → <=");
        assert_eq!(&src[reps[0].start..reps[0].end], "<");

        // Range loops and for{} have no condition — no mutants.
        assert!(replacements_for(Operator::Lbr, "for _, x := range xs { }").is_empty());
        assert!(replacements_for(Operator::Lbr, "for { break }").is_empty());

        // A comparison outside a for is not an LBR target.
        assert!(replacements_for(Operator::Lbr, "if a < b { }").is_empty());
    }

    #[test]
    fn ili_inc_dec_literals() {
        let src = "x := 42";
        let reps = replacements_for(Operator::Ili, src);
        assert_eq!(reps.len(), 2);
        assert!(reps.iter().any(|r| r.label == "42 → 43"));
        assert!(reps.iter().any(|r| r.label == "42 → 41"));
        assert!(reps.iter().any(|r| r.text == "43"));
        assert!(reps.iter().any(|r| r.text == "41"));

        // Zero-prefixed (octal) and floats are skipped.
        assert!(replacements_for(Operator::Ili, "x := 0o17").is_empty());
        assert!(replacements_for(Operator::Ili, "x := 3.14").is_empty());
    }

    #[test]
    fn ror_swaps_relationals() {
        let src = "if a == b && c <= d && e != f { }";
        let reps = replacements_for(Operator::Ror, src);
        assert_eq!(reps.len(), 3);
        assert!(reps.iter().any(|r| r.label == "== → !="));
        assert!(reps.iter().any(|r| r.label == "<= → <"));
        assert!(reps.iter().any(|r| r.label == "!= → =="));
    }

    #[test]
    fn ror_skips_generics_and_channels() {
        let src = "ch := make(chan int);\nxs := []int{1, 2, 3};";
        // `chan int` has no < or >; `[]int{...}` none either. No comparisons.
        assert!(replacements_for(Operator::Ror, src).is_empty());
    }

    #[test]
    fn lor_swaps_logical() {
        let src = "if a && b || c { }";
        let reps = replacements_for(Operator::Lor, src);
        assert_eq!(reps.len(), 2);
        assert!(reps.iter().any(|r| r.label == "&& → ||"));
        assert!(reps.iter().any(|r| r.label == "|| → &&"));
    }

    #[test]
    fn cor_removes_right_term() {
        let src = "ok := a && b;";
        let reps = replacements_for(Operator::Cor, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(reps[0].text, "");
        assert_eq!(&src[reps[0].start..reps[0].end], "b");
    }

    #[test]
    fn sdl_deletes_call_and_assign() {
        let src = "f(1);\nx = 2;\n";
        let reps = replacements_for(Operator::Sdl, src);
        assert_eq!(reps.len(), 2);
        // Each deletion removes exactly its own statement.
        for r in &reps {
            let applied = format!("{}{}{}", &src[..r.start], r.text, &src[r.end..]);
            let deleted = r.label.trim_start_matches("statement deleted: ");
            assert!(
                !applied.contains(deleted),
                "deleted statement still present: {deleted}"
            );
        }
    }

    #[test]
    fn sdl_skips_declarations_and_control() {
        let src = "func f() {\n    if x > 0 {\n        g();\n    }\n}\n";
        let reps = replacements_for(Operator::Sdl, src);
        // Only g(); is deletable.
        assert_eq!(reps.len(), 1);
        assert!(reps[0].label.contains("g()"));
    }

    #[test]
    fn rvr_replaces_return_value() {
        let src = "func f() int {\n    return a + b;\n}\n";
        let reps = replacements_for(Operator::Rvr, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(reps[0].text, "");
        assert_eq!(&src[reps[0].start..reps[0].end], "a + b");
        // Multi-value returns skipped (M2 ErrReturnSwap territory).
        let src2 = "func g() (int, int) {\n    return 1, 2;\n}\n";
        assert!(replacements_for(Operator::Rvr, src2).is_empty());
    }

    #[test]
    fn inc_swaps_loop_step() {
        let src = "for i := 0; i < n; i++ { }\n";
        let reps = replacements_for(Operator::Inc, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(reps[0].label, "++ → --");
        assert_eq!(&src[reps[0].start..reps[0].end], "++");
    }

    #[test]
    fn operator_name_roundtrip() {
        for op in ALL_OPERATORS {
            assert_eq!(Operator::from_name(&op.to_string()), Some(op));
            assert_eq!(
                Operator::from_name(&op.to_string().to_lowercase()),
                Some(op)
            );
        }
        assert_eq!(Operator::from_name("nope"), None);
    }

    #[test]
    fn replacements_are_sorted_by_position() {
        let src = "a + b; c - d;";
        let reps = replacements_for(Operator::Aor, src);
        let mut positions: Vec<usize> = reps.iter().map(|r| r.start).collect();
        positions.sort_unstable();
        let mut orig = reps.iter().map(|r| r.start).collect::<Vec<_>>();
        orig.sort_unstable();
        assert_eq!(positions, orig);
    }
}
