#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
pub mod unix;

/// Normalize a path for the current platform.
pub fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        windows::to_long_path(path)
    }
    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}
