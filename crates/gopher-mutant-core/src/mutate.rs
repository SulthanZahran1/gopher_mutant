//! Mutant application: byte-patch a copy of the source file so `go test
//! -overlay` can run the mutant WITHOUT touching the original tree.

use crate::discover::MutationPoint;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Apply a mutation point to `source`, returning the patched text.
pub fn apply_mutant(source: &str, mp: &MutationPoint) -> String {
    let mut out = String::with_capacity(source.len() + 16);
    out.push_str(&source[..mp.start]);
    out.push_str(&mp.text);
    out.push_str(&source[mp.end..]);
    out
}

/// Write a patched copy of one file to `overlay_dir` at the same
/// module-relative path, and return the absolute path of the patched file.
///
/// `go test -overlay` maps `original -> replacement` (absolute paths, JSON).
/// Keeping the module-relative structure in the overlay dir is not required
/// by the overlay mechanism (paths are explicit in the JSON map), but it
/// keeps the layout debuggable.
pub fn write_patched_file(
    overlay_dir: &Path,
    module_root: &Path,
    rel_path: &str,
    patched: &str,
) -> Result<PathBuf> {
    let out = overlay_dir.join(rel_path);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&out, patched).with_context(|| format!("failed to write {}", out.display()))?;
    let _ = module_root;
    Ok(out)
}

/// Build the overlay JSON map for one mutant:
/// `{ "<abs original>": "<abs patched>" }`.
pub fn overlay_json(original_abs: &Path, patched_abs: &Path) -> Result<String> {
    let mut map = serde_json::Map::new();
    map.insert(
        original_abs.to_string_lossy().to_string(),
        serde_json::Value::String(patched_abs.to_string_lossy().to_string()),
    );
    Ok(serde_json::to_string(&serde_json::Value::Object(map))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::MutationPoint;
    use crate::operators::Operator;

    fn mp(src: &str, start: usize, end: usize, text: &str) -> MutationPoint {
        MutationPoint {
            file: "calc.go".into(),
            line: 1,
            column: 1,
            operator: Operator::Aor,
            label: "test".into(),
            start,
            end,
            original: src[start..end].to_string(),
            text: text.into(),
        }
    }

    #[test]
    fn apply_replaces_range() {
        let src = "return a + b\n";
        let m = mp(src, 9, 10, "-");
        assert_eq!(apply_mutant(src, &m), "return a - b\n");
    }

    #[test]
    fn apply_deletion() {
        let src = "f(1);\n";
        let m = mp(src, 0, 5, "");
        assert_eq!(apply_mutant(src, &m), "\n");
    }

    #[test]
    fn overlay_json_maps_paths() {
        let j = overlay_json(Path::new("/orig/calc.go"), Path::new("/ovl/calc.go")).unwrap();
        assert!(j.contains("/orig/calc.go"));
        assert!(j.contains("/ovl/calc.go"));
    }
}
