use std::path::{Path, PathBuf};

/// Enable long path support (>260 chars) via `\\?\` prefix.
pub fn to_long_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("\\\\?\\") {
        path.to_owned()
    } else if let Ok(abs) = std::fs::canonicalize(path) {
        let abs_str = abs.to_string_lossy();
        if abs_str.starts_with("\\\\?\\") {
            abs
        } else {
            PathBuf::from(format!("\\\\?\\{}", abs_str))
        }
    } else {
        path.to_owned()
    }
}

/// Check if a path is a Windows junction/reparse point.
pub fn is_junction(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    path.symlink_metadata()
        .map(|m| m.file_attributes() & 0x400 != 0) // FILE_ATTRIBUTE_REPARSE_POINT
        .unwrap_or(false)
}

/// Clear read-only flags recursively (needed before deletion on Windows).
pub fn clear_readonly_recursive(path: &Path) -> std::io::Result<()> {
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        if let Ok(entry) = entry {
            if let Ok(meta) = entry.metadata() {
                let mut perms = meta.permissions();
                if perms.readonly() {
                    perms.set_readonly(false);
                    std::fs::set_permissions(entry.path(), perms)?;
                }
            }
        }
    }
    Ok(())
}
