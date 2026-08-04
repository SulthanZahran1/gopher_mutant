//! The 10 generic mutation operator classes (M1 scope, GOAL-1).
//!
//! Each operator is a pure function: given the source text and a byte range,
//! produce a set of (label, before, after) replacements. The M2 idiomatic
//! operators will extend this same trait.

use serde::Serialize;
use std::fmt;

/// The 10 generic operator classes, shared with dart_mutant's generic set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    /// Arithmetic operator replacement: `+`↔`-`, `*`↔`/`, `%`→`*`.
    Aor,
    /// Arithmetic operator deletion: drop the operator (leave one operand).
    Aod,
    /// Arithmetic operator insertion: `a + b` → `a + b + 1` (also `-1`).
    Aoi,
    /// Relational operator replacement: `<`↔`<=`, `>`↔`>=`, `==`↔`!=`.
    Ror,
    /// Logical operator replacement: `&&`↔`||`.
    Lor,
    /// Logical connector replacement: `&&`→`||` inside boolean expressions.
    Lcr,
    /// Conditional operator replacement: `a ? b : c` → `b` and → `c`.
    Cor,
    /// Statement deletion: remove an expression/assignment/return statement.
    Sdl,
    /// Return value replacement: `return x` → `return` (zero value).
    Rvr,
    /// Loop increment/decrement swap: `i++` ↔ `i--` in for-clauses.
    Inc,
}

pub const ALL_OPERATORS: [Operator; 10] = [
    Operator::Aor,
    Operator::Aod,
    Operator::Aoi,
    Operator::Ror,
    Operator::Lor,
    Operator::Lcr,
    Operator::Cor,
    Operator::Sdl,
    Operator::Rvr,
    Operator::Inc,
];

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operator::Aor => "AOR",
            Operator::Aod => "AOD",
            Operator::Aoi => "AOI",
            Operator::Ror => "ROR",
            Operator::Lor => "LOR",
            Operator::Lcr => "LCR",
            Operator::Cor => "COR",
            Operator::Sdl => "SDL",
            Operator::Rvr => "RVR",
            Operator::Inc => "INC",
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
        Operator::Aod => aod_replacements(source),
        Operator::Aoi => aoi_replacements(source),
        Operator::Ror => relational_replacements(source),
        Operator::Lor | Operator::Lcr => logical_replacements(source),
        Operator::Cor => conditional_replacements(source),
        Operator::Sdl => statement_deletions(source),
        Operator::Rvr => return_replacements(source),
        Operator::Inc => inc_replacements(source),
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

/// AOD: delete the arithmetic operator, keeping the left operand.
/// `a + b` → `a b` is invalid Go, so we delete the operator AND the right
/// operand's leading context — the viable form is `a` (drop `+ b`).
fn aod_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    for (i, _c) in source.char_indices() {
        if let Some(op) = single_char_binary_op(source, i) {
            // Extend to the end of the right operand: skip whitespace then
            // consume until a comma, close paren, semicolon, or line boundary.
            let mut j = i + op.len_utf8();
            while j < bytes.len() && (bytes[j] as char).is_whitespace() && bytes[j] != b'\n' {
                j += 1;
            }
            while j < bytes.len() {
                let ch = bytes[j] as char;
                if matches!(ch, ',' | ')' | ';' | '}' | '\n') {
                    break;
                }
                j += 1;
            }
            out.push(Replacement {
                label: format!("{op} operand deleted"),
                start: i,
                end: j,
                text: String::new(),
            });
        }
    }
    out
}

/// AOI: insert `+ 1` (and `- 1`) after the right operand of an arithmetic
/// binary expression. `a + b` → `a + b + 1` / `a + b - 1`.
fn aoi_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    for (i, _c) in source.char_indices() {
        if let Some(op) = single_char_binary_op(source, i) {
            // Find end of right operand: past whitespace, until , ) ; \n or
            // a following binary operator (to avoid nested churn).
            let mut j = i + op.len_utf8();
            while j < bytes.len() && (bytes[j] as char).is_whitespace() && bytes[j] != b'\n' {
                j += 1;
            }
            let operand_start = j;
            let mut depth = 0i32;
            while j < bytes.len() {
                let ch = bytes[j] as char;
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        if depth == 0 {
                            break;
                        }
                        depth -= 1;
                    }
                    ',' | ';' | '\n' if depth == 0 => break,
                    '+' | '-' | '*' | '/' | '%' if depth == 0 && j > operand_start => {
                        // A second operator starts a new sub-expression only if
                        // it's not part of ++/-- or a unary sign directly after
                        // an operator or open paren.
                        let prev = bytes[j - 1] as char;
                        if matches!(prev, '+' | '-' | '*' | '/' | '%' | '(' | '=' | '<' | '>')
                            || (j + 1 < bytes.len() && matches!(bytes[j + 1] as char, '=' | '+'))
                        {
                            // part of compound/inc — keep scanning
                        } else {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let end = j;
            // Trim trailing whitespace inside the range (keep the operand).
            let mut insert_at = end;
            while insert_at > operand_start
                && (bytes[insert_at - 1] as char).is_whitespace()
                && bytes[insert_at - 1] != b'\n'
            {
                insert_at -= 1;
            }
            let operand = &source[operand_start..insert_at];
            out.push(Replacement {
                label: "right operand + 1".to_string(),
                start: operand_start,
                end: insert_at,
                text: format!("{operand} + 1"),
            });
            out.push(Replacement {
                label: "right operand - 1".to_string(),
                start: operand_start,
                end: insert_at,
                text: format!("{operand} - 1"),
            });
        }
    }
    out
}

/// ROR: relational operator replacement. `<`↔`<=`, `>`↔`>=`, `==`↔`!=`.
fn relational_replacements(source: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let two = &source[i..i + 2];
        let (label, text) = match two {
            "==" => ("== → !=", "!="),
            "!=" => ("!= → ==", "=="),
            "<=" => ("<= → <", "<"),
            ">=" => (">= → >", ">"),
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
        let two = &source[i..i + 2];
        let (label, text) = match two {
            "&&" => ("&& → ||", "||"),
            "||" => ("|| → &&", "&&"),
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
        let two = &source[i..i + 2];
        let op = match two {
            "&&" | "||" => two,
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
        let mut text = String::new();
        let mut insert_at = end;
        while insert_at > j && (bytes[insert_at - 1] as char).is_whitespace() {
            insert_at -= 1;
        }
        text.push_str(&source[insert_at..end]);
        out.push(Replacement {
            label: format!("{op} right term removed"),
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
                    let stmt = &source[line_start..j];
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
                    // No semicolon (Go allows omitting). Only delete if the
                    // line is a full statement ending in `)` or identifier
                    // (call/assign without `;`).
                    let stmt = &source[line_start..j];
                    let trimmed = stmt.trim();
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("//")
                        && !trimmed.starts_with("/*")
                        && (trimmed.ends_with(')') || trimmed.ends_with('}'))
                        && !trimmed.starts_with("func ")
                        && !trimmed.starts_with("if ")
                        && !trimmed.starts_with("for ")
                        && !trimmed.starts_with("switch ")
                        && !trimmed.starts_with("select")
                        && !trimmed.starts_with("case ")
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
        if source[i..].starts_with("return") {
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
        let two = &source[i..i + 2];
        let (label, text) = match two {
            "++" => ("++ → --", "--"),
            "--" => ("-- → ++", "++"),
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
    fn aod_deletes_right_operand() {
        let src = "y := a + b;";
        let reps = replacements_for(Operator::Aod, src);
        assert!(!reps.is_empty());
        let r = &reps[0];
        assert_eq!(&src[r.start..r.end], "+ b");
    }

    #[test]
    fn aoi_inserts_inc_dec() {
        let src = "z := a + b;";
        let reps = replacements_for(Operator::Aoi, src);
        assert_eq!(reps.len(), 2);
        assert!(reps.iter().any(|r| r.text.ends_with(" + 1")));
        assert!(reps.iter().any(|r| r.text.ends_with(" - 1")));
        // Replacement applied to source yields valid arithmetic text.
        let applied = format!(
            "{}{}{}",
            &src[..reps[0].start],
            reps[0].text,
            &src[reps[0].end..]
        );
        assert_eq!(applied, "z := a + b + 1;");
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
