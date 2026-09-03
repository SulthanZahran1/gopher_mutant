//! Discovery: walk a Go module's source files and enumerate mutation points.

use crate::operators::{replacements_for, Operator};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// One candidate mutation: an operator, a file, and a byte range.
#[derive(Debug, Clone, Serialize)]
pub struct MutationPoint {
    /// Module-relative path with forward slashes.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    pub operator: Operator,
    /// Human label, e.g. `+ → -`.
    pub label: String,
    /// Byte range in the original source.
    pub start: usize,
    pub end: usize,
    /// Replacement text.
    pub text: String,
    /// The original text being replaced.
    pub original: String,
}

/// Mutation points discovered in one file.
#[derive(Debug, Clone, Serialize)]
pub struct FileDiscovery {
    pub file: String,
    pub points: Vec<MutationPoint>,
}

/// All discoveries for a module.
#[derive(Debug, Clone, Serialize)]
pub struct Discovery {
    pub files: Vec<FileDiscovery>,
    /// Total mutation points across all files.
    pub total: usize,
}

/// Files that end in .go but are never mutation targets.
fn is_go_source(name: &str) -> bool {
    name.ends_with(".go")
        && !name.ends_with("_test.go")
        && !name.ends_with(".pb.go")
        && name != "main.go"
        && !name.starts_with('.')
}

/// Walk `module_root` and discover mutation points for `operators`
/// (default: all 10 generic). Skips test files, vendored/generated code,
/// and `main.go` (no tests exercise main).
pub fn discover(module_root: &Path, operators: &[Operator]) -> Result<Discovery> {
    let mut files = Vec::new();
    let mut total = 0usize;

    for entry in walk(module_root)? {
        let abs = entry?;
        if !abs.is_file() {
            continue;
        }
        let name = abs
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !is_go_source(&name) {
            continue;
        }
        let rel = abs
            .strip_prefix(module_root)
            .unwrap_or(&abs)
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&abs)
            .with_context(|| format!("failed to read {}", abs.display()))?;
        // Scan the masked text (comments/strings blanked) so operators only
        // fire on real code; byte offsets are identical to the original.
        let masked = crate::parse::mask_non_code(&source);

        let mut points = Vec::new();
        for op in operators {
            for rep in replacements_for(*op, &masked) {
                let (line, column) = line_col(&source, rep.start);
                points.push(MutationPoint {
                    file: rel.clone(),
                    line,
                    column,
                    operator: *op,
                    label: rep.label,
                    start: rep.start,
                    end: rep.end,
                    original: source[rep.start..rep.end].to_string(),
                    text: rep.text,
                });
            }
        }
        if !points.is_empty() {
            total += points.len();
            files.push(FileDiscovery { file: rel, points });
        }
    }

    Ok(Discovery { files, total })
}

/// Recursive walk of a directory, skipping hidden dirs, vendor, and .git.
fn walk(root: &Path) -> Result<Vec<std::io::Result<PathBuf>>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir)
            .with_context(|| format!("failed to read dir {}", dir.display()))?;
        for e in rd {
            let entry = e?;
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_dir() {
                if !name.starts_with('.') && name != "vendor" && name != "node_modules" {
                    stack.push(p);
                }
            } else {
                out.push(Ok(p));
            }
        }
    }
    Ok(out)
}

/// Compute 1-based (line, column) for a byte offset.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_counts_correctly() {
        let src = "abc\ndef\nghi";
        assert_eq!(line_col(src, 0), (1, 1));
        assert_eq!(line_col(src, 4), (2, 1));
        assert_eq!(line_col(src, 5), (2, 2));
        assert_eq!(line_col(src, 8), (3, 1));
    }

    #[test]
    fn go_source_filter() {
        assert!(is_go_source("foo.go"));
        assert!(!is_go_source("foo_test.go"));
        assert!(!is_go_source("main.go"));
        assert!(!is_go_source("generated.pb.go"));
        assert!(!is_go_source("foo.txt"));
    }

    #[test]
    fn discover_finds_points_in_tmp_dir() {
        let dir = std::env::temp_dir().join(format!("gm-disc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("calc.go"),
            "package calc\n\nfunc Add(a, b int) int {\n    return a + b\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("calc_test.go"),
            "package calc\n\nimport \"testing\"\n\nfunc TestAdd(t *testing.T) {\n    if Add(1, 2) != 3 { t.Fatal() }\n}\n",
        )
        .unwrap();

        let d = discover(&dir, &crate::operators::ALL_OPERATORS).unwrap();
        assert!(d.total > 0, "expected mutation points");
        assert_eq!(d.files.len(), 1, "test file must be skipped");
        assert_eq!(d.files[0].file, "calc.go");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
