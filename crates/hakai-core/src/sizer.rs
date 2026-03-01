use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

/// Calculate the total size in bytes of all files under `path` (recursively).
/// Symlinks are NOT followed to avoid double-counting.
pub fn calculate_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// Return the unix-ms timestamp of the most recently modified file under `path`.
pub fn get_newest_file_time(path: &Path) -> Option<u64> {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .map(|t| {
            t.duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn calculates_size_of_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap(); // 5 bytes
        fs::write(tmp.path().join("b.txt"), "world!").unwrap(); // 6 bytes
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/c.txt"), "test").unwrap(); // 4 bytes

        let size = calculate_size(tmp.path());
        assert_eq!(size, 15);
    }

    #[test]
    fn newest_file_time_returns_some() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap();

        let t = get_newest_file_time(tmp.path());
        assert!(t.is_some());
        assert!(t.unwrap() > 0);
    }

    #[test]
    fn empty_dir_returns_zero_size() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(calculate_size(tmp.path()), 0);
    }

    #[test]
    fn empty_dir_returns_none_for_newest() {
        let tmp = TempDir::new().unwrap();
        assert!(get_newest_file_time(tmp.path()).is_none());
    }
}
