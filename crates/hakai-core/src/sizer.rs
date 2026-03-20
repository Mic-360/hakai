use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use ignore::WalkState;

pub fn calculate_size_and_mtime(path: &Path) -> (u64, u64) {
    let total_size = Arc::new(AtomicU64::new(0));
    let newest_ms = Arc::new(AtomicU64::new(0));

    ignore::WalkBuilder::new(path)
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .follow_links(false)
        .threads(0)
        .build_parallel()
        .run(|| {
            let total_size = total_size.clone();
            let newest_ms = newest_ms.clone();
            Box::new(move |entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return WalkState::Continue,
                };

                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    return WalkState::Continue;
                }

                if let Ok(meta) = entry.metadata() {
                    total_size.fetch_add(meta.len(), Ordering::Relaxed);

                    if let Ok(modified) = meta.modified() {
                        let ms = modified
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let mut current = newest_ms.load(Ordering::Relaxed);
                        while ms > current {
                            match newest_ms.compare_exchange_weak(
                                current,
                                ms,
                                Ordering::Relaxed,
                                Ordering::Relaxed,
                            ) {
                                Ok(_) => break,
                                Err(c) => current = c,
                            }
                        }
                    }
                }

                WalkState::Continue
            })
        });

    (
        total_size.load(Ordering::Relaxed),
        newest_ms.load(Ordering::Relaxed),
    )
}

pub fn calculate_size(path: &Path) -> u64 {
    calculate_size_and_mtime(path).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn calculates_size_and_mtime_of_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        fs::write(tmp.path().join("b.txt"), "world!").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub/c.txt"), "test").unwrap();

        let (size, mtime) = calculate_size_and_mtime(tmp.path());
        assert_eq!(size, 15);
        assert!(mtime > 0);
    }

    #[test]
    fn size_only_wrapper_works() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        assert_eq!(calculate_size(tmp.path()), 5);
    }

    #[test]
    fn empty_dir_returns_zero() {
        let tmp = TempDir::new().unwrap();
        let (size, mtime) = calculate_size_and_mtime(tmp.path());
        assert_eq!(size, 0);
        assert_eq!(mtime, 0);
    }
}
