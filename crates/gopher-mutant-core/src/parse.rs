//! Parse Go source with tree-sitter-go and expose the syntax tree
//! for mutation-point discovery.

use anyhow::{Context, Result};
use std::path::Path;

/// A parsed Go source file.
pub struct ParsedFile {
    /// Module-relative path with forward slashes (the canonical file identity,
    /// per research #2 — Windows-safe coverage matching).
    pub rel_path: String,
    /// Full source text.
    pub source: String,
    /// Root node of the syntax tree.
    pub root: tree_sitter::Node<'static>,
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

    // SAFETY: the tree outlives its borrowed root node (self-referential via
    // `tree` dropped last). Node<'static> carries only the cursor/range;
    // traversal stays within this struct's lifetime.
    let root = unsafe {
        std::mem::transmute::<tree_sitter::Node<'_>, tree_sitter::Node<'static>>(tree.root_node())
    };

    Ok(ParsedFile {
        rel_path: rel_path.to_string(),
        source: source.to_string(),
        root,
    })
}

/// Quick check: does the file parse to a tree with no ERROR nodes?
pub fn parses_clean(rel_path: &str, source: &str) -> bool {
    match parse_go_file(rel_path, source) {
        Ok(pf) => !has_error(&pf.root),
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
