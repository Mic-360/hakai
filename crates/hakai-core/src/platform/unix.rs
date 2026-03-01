use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Check if a path is a symbolic link.
pub fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Make a file/dir writable (clear immutable/read-only bits).
pub fn make_writable(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    let mut perms = meta.permissions();
    let mode = perms.mode();
    // Add owner write permission
    perms.set_mode(mode | 0o200);
    std::fs::set_permissions(path, perms)
}
