use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status")]
pub enum DeleteResult {
    Success { path: PathBuf, freed_bytes: u64 },
    Error { path: PathBuf, message: String, freed_bytes: u64 },
}

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

pub fn fast_remove_dir_all(path: &Path) -> Result<u64, (u64, std::io::Error)> {
    if !path.exists() {
        return Err((
            0,
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            ),
        ));
    }

    let root = fast_path(path);
    let freed = AtomicU64::new(0);

    parallel_delete_recursive(&root, &freed);

    let size = freed.load(Ordering::Relaxed);

    if std::fs::remove_dir(&root).is_err() && root.exists() {
        if let Err(e) = std::fs::remove_dir_all(&root) {
            return Err((size, e));
        }
    }

    Ok(size)
}

pub fn trash_path_for(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Some(parent.join(format!(".hakai_trash_{ts:x}")))
}

pub fn delete_dir_instant(path: PathBuf, known_size: u64, dry_run: bool) -> DeleteResult {
    if dry_run {
        return DeleteResult::Success {
            path,
            freed_bytes: known_size,
        };
    }

    if let Some(trash) = trash_path_for(&path) {
        if std::fs::rename(&path, &trash).is_ok() {
            return match fast_remove_dir_all(&trash) {
                Ok(freed) => DeleteResult::Success {
                    path,
                    freed_bytes: freed,
                },
                Err((freed, e)) => DeleteResult::Error {
                    path,
                    message: e.to_string(),
                    freed_bytes: freed,
                },
            };
        }
    }

    delete_dir(path, dry_run)
}

pub fn delete_dir(path: PathBuf, dry_run: bool) -> DeleteResult {
    if dry_run {
        let size = crate::sizer::calculate_size(&path);
        return DeleteResult::Success {
            path,
            freed_bytes: size,
        };
    }

    match fast_remove_dir_all(&path) {
        Ok(freed_bytes) => DeleteResult::Success { path, freed_bytes },
        Err((freed_bytes, e)) => DeleteResult::Error {
            path,
            message: e.to_string(),
            freed_bytes,
        },
    }
}

pub fn delete_batch(items: Vec<(PathBuf, u64)>, dry_run: bool) -> Vec<DeleteResult> {
    use rayon::prelude::*;
    items
        .into_par_iter()
        .map(|(path, known_size)| {
            if known_size > 0 {
                delete_dir_instant(path, known_size, dry_run)
            } else {
                delete_dir(path, dry_run)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("node_modules");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "data").unwrap();

        let result = delete_dir(dir.clone(), true);
        assert!(matches!(result, DeleteResult::Success { .. }));
        assert!(dir.exists());
    }

    #[test]
    fn actual_delete_removes_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("node_modules");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "data").unwrap();

        let result = delete_dir(dir.clone(), false);
        assert!(matches!(result, DeleteResult::Success { .. }));
        assert!(!dir.exists());
    }

    #[test]
    fn batch_delete_handles_partial_failures() {
        let tmp = TempDir::new().unwrap();
        let dir1 = tmp.path().join("a");
        let dir2 = tmp.path().join("nonexistent_dir_xyz_12345");
        fs::create_dir(&dir1).unwrap();

        let items = vec![(dir1, 0u64), (dir2, 0u64)];
        let results = delete_batch(items, false);
        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], DeleteResult::Success { .. }));
        assert!(matches!(&results[1], DeleteResult::Error { .. }));
    }
}
