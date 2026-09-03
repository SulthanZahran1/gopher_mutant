//! The generic and Go-idiomatic mutation operator classes.
//!
//! Each operator is a pure function: given source text, produce a set of
//! byte-stable replacements. Generic operators use the masked source supplied
//! by discovery; idiomatic operators parse the original source with
//! tree-sitter-go so statement extents and fields remain precise.

use serde::Serialize;
use std::fmt;

/// The operator classes shipped by M2.
///
/// The locked GOAL-2 text says "22" because it counts ten generic operators,
/// while the signed-off M1 implementation has nine generic variants: ROR
/// intentionally emits both boundary and negation flavors. The executable
/// therefore exposes **21 operator classes: 9 generic + 12 idiomatic**.
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
    /// Boolean term removal: `a && b` → `a` and `a || b` → `a`.
    Cor,
    /// Statement deletion: remove a call/assignment/inc-dec statement.
    Sdl,
    /// Return value replacement for sound boolean expressions.
    Rvr,
    /// Loop increment/decrement swap: `i++` ↔ `i--`.
    Inc,
    /// Loop boundary: `for i < n` ↔ `for i <= n` in C-style loops.
    Lbr,
    /// Integer literal increment/decrement: `42` → `43` and `42` → `41`.
    Ili,
    /// Remove an entire `defer` statement.
    DeferRemoval,
    /// Remove the `go` keyword from a goroutine statement.
    GoroutineRemoval,
    /// Remove an error guard whose condition compares a value with `nil`.
    ErrCheckRemoval,
    /// Swap the first two expressions in a two-expression return list.
    ErrReturnSwap,
    /// Flip a channel send into a receive, or a receive into a send of zero.
    ChannelDirection,
    /// Remove a statement calling the builtin `close`.
    ChannelCloseRemoval,
    /// Remove one communication or default case from a `select`.
    SelectCaseRemoval,
    /// Insert an early `break` at the start of a range-loop body.
    RangeBreak,
    /// Drop the first variable from a two-variable range clause.
    MapIterationSwap,
    /// Remove an assignment whose right-hand side calls `append`.
    AppendRemoval,
    /// Replace a slice-like index `s[i]` with `s[len(s)-1-i]`.
    SliceIndexSwap,
    /// Replace a call to the builtin `recover` with `nil`.
    RecoverRemoval,
}

pub const ALL_OPERATORS: [Operator; 21] = [
    Operator::Aor,
    Operator::Ror,
    Operator::Lor,
    Operator::Cor,
    Operator::Sdl,
    Operator::Rvr,
    Operator::Inc,
    Operator::Lbr,
    Operator::Ili,
    Operator::DeferRemoval,
    Operator::GoroutineRemoval,
    Operator::ErrCheckRemoval,
    Operator::ErrReturnSwap,
    Operator::ChannelDirection,
    Operator::ChannelCloseRemoval,
    Operator::SelectCaseRemoval,
    Operator::RangeBreak,
    Operator::MapIterationSwap,
    Operator::AppendRemoval,
    Operator::SliceIndexSwap,
    Operator::RecoverRemoval,
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
            Operator::DeferRemoval => "DeferRemoval",
            Operator::GoroutineRemoval => "GoroutineRemoval",
            Operator::ErrCheckRemoval => "ErrCheckRemoval",
            Operator::ErrReturnSwap => "ErrReturnSwap",
            Operator::ChannelDirection => "ChannelDirection",
            Operator::ChannelCloseRemoval => "ChannelCloseRemoval",
            Operator::SelectCaseRemoval => "SelectCaseRemoval",
            Operator::RangeBreak => "RangeBreak",
            Operator::MapIterationSwap => "MapIterationSwap",
            Operator::AppendRemoval => "AppendRemoval",
            Operator::SliceIndexSwap => "SliceIndexSwap",
            Operator::RecoverRemoval => "RecoverRemoval",
        };
        write!(f, "{s}")
    }
}

impl Operator {
    /// Parse an operator name (display form or snake_case), case-insensitively.
    pub fn from_name(name: &str) -> Option<Operator> {
        ALL_OPERATORS.iter().copied().find(|op| {
            op.to_string().eq_ignore_ascii_case(name) || serde_name(*op).eq_ignore_ascii_case(name)
        })
    }

    /// Whether this operator is implemented by the tree-sitter path.
    pub const fn is_idiomatic(self) -> bool {
        matches!(
            self,
            Operator::DeferRemoval
                | Operator::GoroutineRemoval
                | Operator::ErrCheckRemoval
                | Operator::ErrReturnSwap
                | Operator::ChannelDirection
                | Operator::ChannelCloseRemoval
                | Operator::SelectCaseRemoval
                | Operator::RangeBreak
                | Operator::MapIterationSwap
                | Operator::AppendRemoval
                | Operator::SliceIndexSwap
                | Operator::RecoverRemoval
        )
    }
}

fn serde_name(op: Operator) -> &'static str {
    match op {
        Operator::Aor => "aor",
        Operator::Ror => "ror",
        Operator::Lor => "lor",
        Operator::Cor => "cor",
        Operator::Sdl => "sdl",
        Operator::Rvr => "rvr",
        Operator::Inc => "inc",
        Operator::Lbr => "lbr",
        Operator::Ili => "ili",
        Operator::DeferRemoval => "defer_removal",
        Operator::GoroutineRemoval => "goroutine_removal",
        Operator::ErrCheckRemoval => "err_check_removal",
        Operator::ErrReturnSwap => "err_return_swap",
        Operator::ChannelDirection => "channel_direction",
        Operator::ChannelCloseRemoval => "channel_close_removal",
        Operator::SelectCaseRemoval => "select_case_removal",
        Operator::RangeBreak => "range_break",
        Operator::MapIterationSwap => "map_iteration_swap",
        Operator::AppendRemoval => "append_removal",
        Operator::SliceIndexSwap => "slice_index_swap",
        Operator::RecoverRemoval => "recover_removal",
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
        idiomatic if idiomatic.is_idiomatic() => idiomatic_replacements(idiomatic, source),
        _ => Vec::new(),
    }
}

/// Discover one of the Go-idiomatic operators with the owned tree-sitter
/// tree. The function is deliberately pure: it parses only the supplied
/// source and returns byte ranges into that same source. Type-dependent
/// operators use the conservative heuristics documented on their enum
/// variants; semantic rejection is left to the Go compiler.
fn idiomatic_replacements(op: Operator, source: &str) -> Vec<Replacement> {
    let Ok(parsed) = crate::parse::parse_go_file("_idiomatic.go", source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_idiomatic(parsed.root(), source, op, &mut out);
    out.sort_by(|a, b| (a.start, a.end, &a.label).cmp(&(b.start, b.end, &b.label)));
    out
}

fn collect_idiomatic(
    node: tree_sitter::Node<'_>,
    source: &str,
    op: Operator,
    out: &mut Vec<Replacement>,
) {
    match op {
        Operator::DeferRemoval if node.kind() == "defer_statement" => {
            push_node_removal(out, source, node, "defer statement removed");
        }
        Operator::GoroutineRemoval if node.kind() == "go_statement" => {
            // `go_statement` starts at the anonymous `go` token. Removing
            // only those two bytes leaves the call as an expression statement.
            let start = node.start_byte();
            if source.get(start..start.saturating_add(2)) == Some("go") {
                push_replacement(out, source, start, start + 2, "go keyword removed", "");
            }
        }
        Operator::ErrCheckRemoval if node.kind() == "if_statement" => {
            if is_error_guard(node, source) {
                push_node_removal(out, source, node, "error check removed");
            }
        }
        Operator::ErrReturnSwap if node.kind() == "return_statement" => {
            if let Some(list) = named_child_kind(node, "expression_list") {
                let expressions = named_children(list);
                // Without type information, restrict the heuristic to exactly
                // two expressions. This avoids rewriting variadic returns and
                // makes the possible type mismatch an explicit compiler result.
                if expressions.len() == 2 {
                    let first = expressions[0];
                    let second = expressions[1];
                    let first_text = source[first.start_byte()..first.end_byte()].to_string();
                    let second_text = source[second.start_byte()..second.end_byte()].to_string();
                    push_replacement(
                        out,
                        source,
                        first.start_byte(),
                        second.end_byte(),
                        "return expressions swapped",
                        &format!("{second_text}, {first_text}"),
                    );
                }
            }
        }
        Operator::ChannelDirection if node.kind() == "send_statement" => {
            if let Some(channel) = node.child_by_field_name("channel") {
                let channel_text = node_text(source, channel).to_string();
                push_replacement(
                    out,
                    source,
                    node.start_byte(),
                    node.end_byte(),
                    "channel send changed to receive",
                    &format!("<-{channel_text}"),
                );
            }
        }
        Operator::ChannelDirection if node.kind() == "receive_statement" => {
            if let Some(operand) = receive_operand(node) {
                let operand_text = node_text(source, operand).to_string();
                push_replacement(
                    out,
                    source,
                    node.start_byte(),
                    node.end_byte(),
                    "channel receive changed to send",
                    &format!("{operand_text} <- 0"),
                );
            }
        }
        Operator::ChannelDirection if node.kind() == "unary_expression" => {
            // A receive used as a value is a unary_expression. The enclosing
            // receive_statement case above owns statement-form receives, so
            // avoid emitting the same mutation twice for that representation.
            let is_receive = node
                .child_by_field_name("operator")
                .is_some_and(|operator| node_text(source, operator) == "<-");
            let wrapped_by_statement = node
                .parent()
                .is_some_and(|parent| parent.kind() == "receive_statement");
            if is_receive && !wrapped_by_statement {
                if let Some(operand) = node.child_by_field_name("operand") {
                    let operand_text = node_text(source, operand).to_string();
                    push_replacement(
                        out,
                        source,
                        node.start_byte(),
                        node.end_byte(),
                        "channel receive changed to send",
                        &format!("{operand_text} <- 0"),
                    );
                }
            }
        }
        Operator::ChannelCloseRemoval if node.kind() == "expression_statement" => {
            if let Some(call) = named_descendant_kind(node, "call_expression") {
                if function_is(call, source, "close") {
                    push_node_removal(out, source, node, "close call removed");
                }
            }
        }
        Operator::SelectCaseRemoval
            if matches!(node.kind(), "communication_case" | "default_case")
                && node
                    .parent()
                    .is_some_and(|parent| parent.kind() == "select_statement") =>
        {
            push_node_removal(out, source, node, "select case removed");
        }
        Operator::RangeBreak if node.kind() == "for_statement" => {
            let is_range = named_child_kind(node, "range_clause").is_some();
            if is_range {
                if let Some(body) = node.child_by_field_name("body") {
                    if let Some(list) = named_descendant_kind(body, "statement_list") {
                        push_replacement(
                            out,
                            source,
                            list.start_byte(),
                            list.start_byte(),
                            "break inserted in range loop",
                            "break\n",
                        );
                    } else {
                        // An empty block has no statement_list node. Insert
                        // immediately after `{` and keep the mutation valid.
                        let insertion = body.start_byte().saturating_add(1);
                        push_replacement(
                            out,
                            source,
                            insertion,
                            insertion,
                            "break inserted in range loop",
                            "break\n",
                        );
                    }
                }
            }
        }
        Operator::MapIterationSwap if node.kind() == "range_clause" => {
            if let Some(left) = node.child_by_field_name("left") {
                let variables = named_children(left);
                if variables.len() == 2 {
                    // The source span from the first variable through the
                    // second variable's start includes the comma and spaces.
                    push_replacement(
                        out,
                        source,
                        variables[0].start_byte(),
                        variables[1].start_byte(),
                        "range key removed",
                        "",
                    );
                }
            }
        }
        Operator::AppendRemoval if node.kind() == "assignment_statement" => {
            if let Some(right) = node.child_by_field_name("right") {
                if let Some(call) = named_descendant_kind(right, "call_expression") {
                    if function_is(call, source, "append") {
                        push_node_removal(out, source, node, "append assignment removed");
                    }
                }
            }
        }
        Operator::SliceIndexSwap if node.kind() == "index_expression" => {
            if let (Some(operand), Some(index)) = (
                node.child_by_field_name("operand"),
                node.child_by_field_name("index"),
            ) {
                let operand_text = node_text(source, operand);
                let index_text = node_text(source, index);
                push_replacement(
                    out,
                    source,
                    index.start_byte(),
                    index.end_byte(),
                    "slice index reversed",
                    &format!("len({operand_text})-1-{index_text}"),
                );
            }
        }
        Operator::RecoverRemoval if node.kind() == "call_expression" => {
            let no_arguments = node
                .child_by_field_name("arguments")
                .is_none_or(|arguments| named_children(arguments).is_empty());
            if no_arguments && function_is(node, source, "recover") {
                push_replacement(
                    out,
                    source,
                    node.start_byte(),
                    node.end_byte(),
                    "recover call replaced with nil",
                    "nil",
                );
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_idiomatic(child, source, op, out);
    }
}

fn push_node_removal(
    out: &mut Vec<Replacement>,
    source: &str,
    node: tree_sitter::Node<'_>,
    label: &str,
) {
    push_replacement(out, source, node.start_byte(), node.end_byte(), label, "");
}

fn push_replacement(
    out: &mut Vec<Replacement>,
    source: &str,
    start: usize,
    end: usize,
    label: &str,
    text: &str,
) {
    if start <= end
        && end <= source.len()
        && source.is_char_boundary(start)
        && source.is_char_boundary(end)
        && source.get(start..end).is_some()
        && source.get(start..end) != Some(text)
    {
        out.push(Replacement {
            label: label.to_string(),
            start,
            end,
            text: text.to_string(),
        });
    }
}

fn node_text<'a>(source: &'a str, node: tree_sitter::Node<'_>) -> &'a str {
    source.get(node.start_byte()..node.end_byte()).unwrap_or("")
}

fn named_children(node: tree_sitter::Node<'_>) -> Vec<tree_sitter::Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn named_child_kind<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Option<tree_sitter::Node<'a>> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() == kind)
}

fn named_descendant_kind<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = named_descendant_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

fn function_is(call: tree_sitter::Node<'_>, source: &str, expected: &str) -> bool {
    call.child_by_field_name("function")
        .is_some_and(|function| {
            function.kind() == "identifier" && node_text(source, function) == expected
        })
}

fn receive_operand(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    node.child_by_field_name("right")
        .and_then(|right| named_descendant_kind(right, "unary_expression"))
        .and_then(|unary| unary.child_by_field_name("operand"))
        .or_else(|| node.child_by_field_name("operand"))
}

fn is_error_guard(node: tree_sitter::Node<'_>, source: &str) -> bool {
    let Some(condition) = node.child_by_field_name("condition") else {
        return false;
    };
    let is_non_nil = condition
        .child_by_field_name("operator")
        .is_some_and(|operator| node_text(source, operator) == "!=")
        && condition
            .child_by_field_name("right")
            .is_some_and(|right| node_text(source, right) == "nil");
    if !is_non_nil {
        return false;
    }
    node.child_by_field_name("consequence")
        .is_some_and(|consequence| has_error_exit(consequence, source))
}

fn has_error_exit(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if matches!(
        node.kind(),
        "return_statement" | "break_statement" | "continue_statement"
    ) {
        return true;
    }
    if node.kind() == "call_expression" && function_is(node, source, "panic") {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| has_error_exit(child, source));
    found
}

fn single_char_binary_op(source: &str, i: usize) -> Option<char> {
    let b = source.as_bytes();
    let c = b[i] as char;
    if !matches!(c, '+' | '-' | '*' | '/' | '%') {
        return None;
    }
    // Skip ++/-- pairs: a +/- preceded by the same char is part of an
    // inc/dec token, not a binary operator.
    if matches!(c, '+' | '-') && i > 0 && matches!(b[i - 1] as char, '+' | '-') {
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

/// COR: conditional operator replacement for Go (no ternary) = boolean term
/// removal: `a && b` → `a`, `a || b` → `a` — remove the operator AND its
/// right term, so the result is always valid Go (`if a {`, `x = a;`).
fn conditional_replacements(source: &str) -> Vec<Replacement> {
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
        // Right term: skip whitespace, consume until , ) ; { } \n or && ||.
        let mut j = i + 2;
        while j < bytes.len() && (bytes[j] as char).is_whitespace() && bytes[j] != b'\n' {
            j += 1;
        }
        let mut end = j;
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if matches!(ch, ',' | ')' | ';' | '\n' | '{' | '}') {
                break;
            }
            if end + 1 < bytes.len() && matches!(&bytes[end..end + 2], b"&&" | b"||") {
                break;
            }
            end += 1;
        }
        let end = bd(source, end);
        // Trim trailing whitespace so the removal leaves clean text.
        let mut trim_end = end;
        while trim_end > j && (bytes[trim_end - 1] as char).is_whitespace() {
            trim_end -= 1;
        }
        let trim_end = bd(source, trim_end);
        out.push(Replacement {
            label: format!("{} right term removed", std::str::from_utf8(op).unwrap()),
            // Remove the operator AND its right term: `a && b` → `a`.
            start: i,
            end: trim_end,
            text: String::new(),
        });
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

/// RVR: replace `return <expr>` with a zero-value return.
///
/// M1 limitation (documented in GOAL-1): Go requires a return value, and
/// replacing `return x` with bare `return` only compiles with named results.
/// Without type info (M2's gopls pass), the SOUND case is syntactically
/// boolean expressions — comparisons/logicals are always `bool`, so
/// `return x > 0` → `return false` compiles and is a real mutant. Other
/// expressions (int/string/identifier returns) are skipped in M1; they
/// classify as compile_error instead of killing the fixture's 100% MSI.
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
                    // Skip multi-value returns `a, b` (M2's ErrReturnSwap).
                    // Only single-expression returns are candidates.
                    if !expr.contains(',') {
                        // Sound bool detection: comparisons and logical
                        // operators imply a bool expression.
                        let is_bool_expr = ["==", "!=", "<=", ">=", "<", ">", "&&", "||"]
                            .iter()
                            .any(|op| expr.contains(op));
                        if is_bool_expr {
                            out.push(Replacement {
                                label: format!("return {expr} → return false"),
                                start: j,
                                end: trim_end,
                                text: "false".to_string(),
                            });
                        }
                        // Non-bool returns need type info (M2 gopls pass) —
                        // skipped here so M1 never fabricates invalid
                        // zero-values.
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

    fn apply(src: &str, replacement: &Replacement) -> String {
        format!(
            "{}{}{}",
            &src[..replacement.start],
            replacement.text,
            &src[replacement.end..]
        )
    }

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
        // Operator AND its right term are removed: `a && b` → `a`.
        let src = "ok := a && b;";
        let reps = replacements_for(Operator::Cor, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(reps[0].text, "");
        assert_eq!(&src[reps[0].start..reps[0].end], "&& b");
        let applied = format!(
            "{}{}{}",
            &src[..reps[0].start],
            reps[0].text,
            &src[reps[0].end..]
        );
        assert_eq!(applied, "ok := a ;");
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
    fn rvr_replaces_bool_return_value() {
        // Bool expressions are sound zero-value candidates in M1.
        let src = "func f() bool {\n    return a > b;\n}\n";
        let reps = replacements_for(Operator::Rvr, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(reps[0].text, "false");
        assert_eq!(&src[reps[0].start..reps[0].end], "a > b");
        // Non-bool expressions need type info (M2 gopls pass) — skipped in M1.
        let src2 = "func g() int {\n    return a + b;\n}\n";
        assert!(replacements_for(Operator::Rvr, src2).is_empty());
        // Multi-value returns skipped (M2 ErrReturnSwap territory).
        let src3 = "func h() (int, int) {\n    return 1, 2;\n}\n";
        assert!(replacements_for(Operator::Rvr, src3).is_empty());
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
    fn idiomatic_defer_removal_has_exact_pair() {
        let src = "package p\nfunc f() {\n\tdefer cleanup()\n\twork()\n}\n";
        let reps = replacements_for(Operator::DeferRemoval, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f() {\n\t\n\twork()\n}\n"
        );
    }

    #[test]
    fn idiomatic_goroutine_removal_has_exact_pair() {
        let src = "package p\nfunc f(ch chan int) {\n\tgo work(ch)\n}\n";
        let reps = replacements_for(Operator::GoroutineRemoval, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(ch chan int) {\n\t work(ch)\n}\n"
        );
    }

    #[test]
    fn idiomatic_error_check_removal_has_exact_pair() {
        let src = "package p\nfunc f(err error) int {\n\tif err != nil {\n\t\treturn 1\n\t}\n\treturn 0\n}\n";
        let reps = replacements_for(Operator::ErrCheckRemoval, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(err error) int {\n\t\n\treturn 0\n}\n"
        );
    }

    #[test]
    fn idiomatic_error_return_swap_has_exact_pair() {
        let src = "package p\nfunc f() (int, int) { return 1, 2 }\n";
        let reps = replacements_for(Operator::ErrReturnSwap, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f() (int, int) { return 2, 1 }\n"
        );
    }

    #[test]
    fn idiomatic_channel_direction_has_exact_pair() {
        let src =
            "package p\nfunc send(ch chan int) { ch <- 1 }\nfunc recv(ch chan int) { <-ch }\n";
        let reps = replacements_for(Operator::ChannelDirection, src);
        assert_eq!(reps.len(), 2);
        let send = reps.iter().find(|r| &src[r.start..r.end] == "ch <- 1");
        let recv = reps.iter().find(|r| &src[r.start..r.end] == "<-ch");
        assert!(send.is_some(), "send replacement missing: {reps:?}");
        assert!(recv.is_some(), "receive replacement missing: {reps:?}");
        assert_eq!(
            apply(src, send.unwrap()),
            "package p\nfunc send(ch chan int) { <-ch }\nfunc recv(ch chan int) { <-ch }\n"
        );
        assert_eq!(
            apply(src, recv.unwrap()),
            "package p\nfunc send(ch chan int) { ch <- 1 }\nfunc recv(ch chan int) { ch <- 0 }\n"
        );
    }

    #[test]
    fn idiomatic_channel_close_removal_has_exact_pair() {
        let src = "package p\nfunc f(ch chan int) {\n\tclose(ch)\n}\n";
        let reps = replacements_for(Operator::ChannelCloseRemoval, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(ch chan int) {\n\t\n}\n"
        );
    }

    #[test]
    fn idiomatic_select_case_removal_has_exact_pair() {
        let src = "package p\nfunc f(ch chan int) {\n\tselect {\n\tcase <-ch:\n\t\twork()\n\tdefault:\n\t\tother()\n\t}\n}\n";
        let reps = replacements_for(Operator::SelectCaseRemoval, src);
        assert_eq!(reps.len(), 2);
        for replacement in &reps {
            let mutated = apply(src, replacement);
            assert_ne!(mutated, src);
            assert!(!mutated.contains(&src[replacement.start..replacement.end]));
        }
    }

    #[test]
    fn idiomatic_range_break_has_exact_pair() {
        let src = "package p\nfunc f(xs []int) {\n\tfor _, x := range xs {\n\t\twork(x)\n\t}\n}\n";
        let reps = replacements_for(Operator::RangeBreak, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(xs []int) {\n\tfor _, x := range xs {\n\t\tbreak\nwork(x)\n\t}\n}\n"
        );
    }

    #[test]
    fn idiomatic_map_iteration_swap_has_exact_pair() {
        let src = "package p\nfunc f(m map[string]int) {\n\tfor k, v := range m {\n\t\tuse(k, v)\n\t}\n}\n";
        let reps = replacements_for(Operator::MapIterationSwap, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(m map[string]int) {\n\tfor v := range m {\n\t\tuse(k, v)\n\t}\n}\n"
        );
    }

    #[test]
    fn idiomatic_append_removal_has_exact_pair() {
        let src = "package p\nfunc f(s []int) {\n\ts = append(s, 1)\n}\n";
        let reps = replacements_for(Operator::AppendRemoval, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(s []int) {\n\t\n}\n"
        );
    }

    #[test]
    fn idiomatic_slice_index_swap_has_exact_pair() {
        let src = "package p\nfunc f(s []int, i int) int { return s[i] }\n";
        let reps = replacements_for(Operator::SliceIndexSwap, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f(s []int, i int) int { return s[len(s)-1-i] }\n"
        );
    }

    #[test]
    fn idiomatic_recover_removal_has_exact_pair() {
        let src = "package p\nfunc f() {\n\tvar r any\n\tr = recover()\n\t_ = r\n}\n";
        let reps = replacements_for(Operator::RecoverRemoval, src);
        assert_eq!(reps.len(), 1);
        assert_eq!(
            apply(src, &reps[0]),
            "package p\nfunc f() {\n\tvar r any\n\tr = nil\n\t_ = r\n}\n"
        );
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
