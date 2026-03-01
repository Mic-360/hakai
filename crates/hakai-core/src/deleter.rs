use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rayon::prelude::*;
use serde::Serialize;
use tokio::sync::Semaphore;

/// Result of a deletion attempt.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum DeleteResult {
    Success { path: PathBuf, freed_bytes: u64 },
    Error { path: PathBuf, message: String },
}

/// Normalize path for faster I/O on Windows (\\?\ prefix skips MAX_PATH checks).
#[cfg(windows)]
#[inline]
fn fast_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("\\\\?\\") {
        path.to_owned()
    } else {
        PathBuf::from(format!("\\\\?\\{}", s))
    }
}

#[cfg(not(windows))]
#[inline]
fn fast_path(path: &Path) -> PathBuf {
    path.to_owned()
}

/// Fast parallel directory removal. Walks once (collecting files + computing
/// size), deletes files in parallel, then removes directories bottom-up.
/// Returns the total bytes freed.
fn fast_remove_dir_all(path: &Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} does not exist", path.display()),
        ));
    }

    // Use \\?\ prefix for the root to enable long-path support and faster I/O
    let root = fast_path(path);

    let mut files: Vec<PathBuf> = Vec::with_capacity(16384);
    let mut dirs: Vec<(PathBuf, usize)> = Vec::with_capacity(2048); // (path, depth)
    let total_bytes = AtomicU64::new(0);

    // Single walk: collect files + dirs, sum file sizes
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .contents_first(false)
    {
        match entry {
            Ok(e) => {
                if e.file_type().is_dir() {
                    dirs.push((e.path().to_owned(), e.depth()));
                } else {
                    // Accumulate size from the DirEntry metadata (already cached by walkdir)
                    if let Ok(meta) = e.metadata() {
                        total_bytes.fetch_add(meta.len(), Ordering::Relaxed);
                    }
                    files.push(e.path().to_owned());
                }
            }
            Err(_) => {
                // Continue past errors (permission denied, etc.)
            }
        }
    }

    let size = total_bytes.load(Ordering::Relaxed);

    // Delete all files in parallel using rayon
    files.par_iter().for_each(|f| {
        if std::fs::remove_file(f).is_err() {
            // On Windows, try clearing read-only and retry
            #[cfg(windows)]
            {
                if let Ok(meta) = std::fs::metadata(f) {
                    let mut perms = meta.permissions();
                    if perms.readonly() {
                        perms.set_readonly(false);
                        let _ = std::fs::set_permissions(f, perms);
                        let _ = std::fs::remove_file(f);
                    }
                }
            }
        }
    });

    // Remove directories bottom-up (deepest first) — sort by depth descending
    dirs.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    // Parallel dir removal in depth layers
    let mut i = 0;
    while i < dirs.len() {
        let current_depth = dirs[i].1;
        let start = i;
        while i < dirs.len() && dirs[i].1 == current_depth {
            i += 1;
        }
        // Remove all dirs at this depth level in parallel
        dirs[start..i].par_iter().for_each(|(dir, _)| {
            let _ = std::fs::remove_dir(dir);
        });
    }

    // Final cleanup: if root still exists (e.g. files were locked), try once more
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }

    Ok(size)
}

/// Delete a single directory. If `dry_run`, simulate without actually removing.
pub async fn delete_dir(path: PathBuf, dry_run: bool) -> DeleteResult {
    if dry_run {
        let size = crate::sizer::calculate_size(&path);
        return DeleteResult::Success {
            path,
            freed_bytes: size,
        };
    }

    // Perform fast parallel deletion on a blocking thread — size is computed
    // during the walk so we don't need a separate `calculate_size` call.
    let p = path.clone();
    let result = tokio::task::spawn_blocking(move || fast_remove_dir_all(&p)).await;

    match result {
        Ok(Ok(freed_bytes)) => DeleteResult::Success { path, freed_bytes },
        Ok(Err(e)) => DeleteResult::Error {
            path,
            message: e.to_string(),
        },
        Err(e) => DeleteResult::Error {
            path,
            message: format!("Task join error: {e}"),
        },
    }
}

/// Delete multiple directories concurrently (up to `concurrency` at once).
pub async fn delete_batch(
    paths: Vec<PathBuf>,
    dry_run: bool,
    concurrency: usize,
) -> Vec<DeleteResult> {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut handles = Vec::with_capacity(paths.len());

    for path in paths {
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            delete_dir(path, dry_run).await
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => results.push(DeleteResult::Error {
                path: PathBuf::from("<unknown>"),
                message: format!("Task join error: {e}"),
            }),
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("node_modules");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "data").unwrap();

        let result = delete_dir(dir.clone(), true).await;
        assert!(matches!(result, DeleteResult::Success { .. }));
        // Directory should still exist
        assert!(dir.exists());
    }

    #[tokio::test]
    async fn actual_delete_removes_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("node_modules");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "data").unwrap();

        let result = delete_dir(dir.clone(), false).await;
        assert!(matches!(result, DeleteResult::Success { .. }));
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn batch_delete_handles_partial_failures() {
        let tmp = TempDir::new().unwrap();
        let dir1 = tmp.path().join("a");
        let dir2 = tmp.path().join("nonexistent_dir_xyz_12345");
        fs::create_dir(&dir1).unwrap();

        let results = delete_batch(vec![dir1, dir2], false, 4).await;
        assert_eq!(results.len(), 2);
        // First should succeed, second should error
        assert!(matches!(&results[0], DeleteResult::Success { .. }));
        assert!(matches!(&results[1], DeleteResult::Error { .. }));
    }
}
