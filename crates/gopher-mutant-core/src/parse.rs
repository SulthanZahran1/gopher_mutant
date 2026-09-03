//! Parse Go source with tree-sitter-go and expose the syntax tree
//! for mutation-point discovery.

use anyhow::{Context, Result};
use std::path::Path;

/// A parsed Go source file. The owned `Tree` keeps node cursors valid —
/// callers must re-root via [`ParsedFile::root`] rather than holding nodes
/// across function boundaries (nodes borrow the tree).
pub struct ParsedFile {
    /// Module-relative path with forward slashes (the canonical file identity,
    /// per research #2 — Windows-safe coverage matching).
    pub rel_path: String,
    /// Full source text.
    pub source: String,
    tree: tree_sitter::Tree,
}

/// Parse a Go source file. `rel_path` is the module-relative identity used in
/// all downstream output (JSON, coverage matching).
pub fn parse_go_file(rel_path: &str, source: &str) -> Result<ParsedFile> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .context("failed to load tree-sitter-go grammar")?;

    let tree = parser
        .parse(source, None)
        .context("failed to parse Go source")?;

    Ok(ParsedFile {
        rel_path: rel_path.to_string(),
        source: source.to_string(),
        tree,
    })
}

impl ParsedFile {
    /// The syntax tree root, borrowed from the live tree.
    pub fn root(&self) -> tree_sitter::Node<'_> {
        self.tree.root_node()
    }

    /// All comments and string/rune literal byte ranges in the file.
    pub fn non_code_ranges(&self) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        collect_mask_ranges(&self.tree.root_node(), &mut ranges);
        ranges
    }
}

/// Quick check: does the file parse to a tree with no ERROR nodes?
pub fn parses_clean(rel_path: &str, source: &str) -> bool {
    match parse_go_file(rel_path, source) {
        Ok(pf) => !has_error(&pf.root()),
        Err(_) => false,
    }
}

/// True if the subtree rooted at `node` contains any ERROR or MISSING node.
pub fn has_error(node: &tree_sitter::Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_error(&child) {
            return true;
        }
    }
    false
}

/// Read a file from disk and parse it.
pub fn parse_file_on_disk(abs_path: &Path, rel_path: &str) -> Result<ParsedFile> {
    let source = std::fs::read_to_string(abs_path)
        .with_context(|| format!("failed to read {}", abs_path.display()))?;
    parse_go_file(rel_path, &source)
}

/// Return a copy of `source` with comments and string/rune literals blanked
/// to spaces (newlines preserved — positions stay identical to the original).
///
/// Operator scanners run against the masked text so comments and string
/// content never produce mutation points; byte offsets still refer to the
/// real source. Falls back to the unmasked text if parsing fails.
pub fn mask_non_code(source: &str) -> String {
    let Ok(pf) = parse_go_file("_mask", source) else {
        return source.to_string();
    };
    if has_error(&pf.root()) {
        return source.to_string();
    }
    let ranges = pf.non_code_ranges();
    let mut bytes: Vec<u8> = source.as_bytes().to_vec();
    for (s, e) in ranges {
        for b in &mut bytes[s..e] {
            if *b != b'\n' && *b != b'\r' {
                *b = b' ';
            }
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_string())
}

fn collect_mask_ranges(node: &tree_sitter::Node, ranges: &mut Vec<(usize, usize)>) {
    let kind = node.kind();
    if matches!(
        kind,
        "comment" | "interpreted_string_literal" | "raw_string_literal" | "rune_literal"
    ) {
        ranges.push((node.start_byte(), node.end_byte()));
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_mask_ranges(&child, ranges);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_blanks_comments_and_strings_keeps_positions() {
        let src = "// a + b < c\nx := \"a && b\"\n";
        let masked = mask_non_code(src);
        assert_eq!(masked.len(), src.len());
        assert!(!masked.contains('+'));
        assert!(!masked.contains('<'));
        assert!(!masked.contains('&'));
        // Newlines survive.
        assert_eq!(masked.matches('\n').count(), 2);
        // Code outside comments/strings survives; literal content (incl.
        // quotes) is blanked.
        assert!(masked.contains("x :="));
        assert!(!masked.contains('"'));
    }

    #[test]
    fn mask_keeps_multiline_strings_line_count() {
        let src = "s := `line1\nline2`\n";
        let masked = mask_non_code(src);
        assert_eq!(masked.matches('\n').count(), 2);
        assert!(!masked.contains("line1"));
    }

    #[test]
    fn parse_roundtrip_roots_are_stable() {
        let src = "package p\nfunc f() int { return 1 }\n";
        let pf = parse_go_file("p.go", src).unwrap();
        let r1 = pf.root();
        let r2 = pf.root();
        assert_eq!(r1.start_byte(), r2.start_byte());
        assert_eq!(r1.end_byte(), r2.end_byte());
    }
}
