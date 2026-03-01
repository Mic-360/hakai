use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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

/// Delete a single file, handling read-only on Windows.
#[inline]
fn delete_file(path: &Path) {
    if std::fs::remove_file(path).is_err() {
        #[cfg(windows)]
        {
            if let Ok(meta) = std::fs::symlink_metadata(path) {
                let mut perms = meta.permissions();
                if perms.readonly() {
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(path, perms);
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

/// Parallel recursive walk-and-delete using rayon work-stealing.
fn parallel_delete_recursive(dir: &Path, freed: &AtomicU64) {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut files: Vec<PathBuf> = Vec::new();
    let mut subdirs: Vec<PathBuf> = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let path = entry.path();
        if ft.is_dir() {
            subdirs.push(path);
        } else {
            if let Ok(meta) = entry.metadata() {
                freed.fetch_add(meta.len(), Ordering::Relaxed);
            }
            files.push(path);
        }
    }

    rayon::scope(|s| {
        for subdir in &subdirs {
            s.spawn(move |_| {
                parallel_delete_recursive(subdir, freed);
                let _ = std::fs::remove_dir(subdir);
            });
        }

        if files.len() > 256 {
            use rayon::prelude::*;
            files.par_iter().for_each(|f| delete_file(f));
        } else {
            for f in &files {
                delete_file(f);
            }
        }
    });
}

/// Full parallel directory removal. Returns total bytes freed.
fn fast_remove_dir_all(path: &Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} does not exist", path.display()),
        ));
    }

    let root = fast_path(path);
    let freed = AtomicU64::new(0);

    parallel_delete_recursive(&root, &freed);

    let size = freed.load(Ordering::Relaxed);

    if let Err(_) = std::fs::remove_dir(&root) {
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
    }

    Ok(size)
}

/// Generate a unique trash directory name next to the target.
fn trash_path_for(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    // Use a dot-prefix so scanners with exclude_hidden skip it
    Some(parent.join(format!(".hakai_trash_{ts:x}")))
}

/// Deletion via rename trick: renames the directory to a trash name (instant
/// on same filesystem) so the original path disappears immediately, then
/// deletes the renamed directory's contents in parallel before returning.
///
/// `known_size` is the pre-calculated size from the scan phase so we don't
/// need to re-walk the directory for byte counting.
pub async fn delete_dir_instant(path: PathBuf, known_size: u64, dry_run: bool) -> DeleteResult {
    if dry_run {
        return DeleteResult::Success {
            path,
            freed_bytes: known_size,
        };
    }

    // Try the rename trick first — rename is instant on the same filesystem,
    // then delete the renamed directory (the original path disappears immediately).
    if let Some(trash) = trash_path_for(&path) {
        if std::fs::rename(&path, &trash).is_ok() {
            // Delete the renamed directory on a blocking thread and wait for completion
            let result = tokio::task::spawn_blocking(move || {
                let freed = AtomicU64::new(0);
                let root = fast_path(&trash);
                parallel_delete_recursive(&root, &freed);
                let _ = std::fs::remove_dir(&root);
                if root.exists() {
                    let _ = std::fs::remove_dir_all(&root);
                }
            })
            .await;

            return match result {
                Ok(()) => DeleteResult::Success {
                    path,
                    freed_bytes: known_size,
                },
                Err(e) => DeleteResult::Error {
                    path,
                    message: format!("Delete task failed: {e}"),
                },
            };
        }
        // rename failed (cross-device, permission, etc.) — fall through to direct delete
    }

    // Fallback: direct deletion (blocking)
    delete_dir(path, dry_run).await
}

/// Delete a single directory directly (blocking). Used as fallback and for
/// headless CLI mode.
pub async fn delete_dir(path: PathBuf, dry_run: bool) -> DeleteResult {
    if dry_run {
        let size = crate::sizer::calculate_size(&path);
        return DeleteResult::Success {
            path,
            freed_bytes: size,
        };
    }

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
