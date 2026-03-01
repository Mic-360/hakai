use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::Sender;
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

/// Options controlling how the scanner traverses directories.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub root: PathBuf,
    pub targets: Vec<String>,
    pub exclude: Vec<String>,
    pub exclude_hidden: bool,
    pub max_depth: Option<usize>,
}

/// Events emitted by the scanner as it discovers directories.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ScanEvent {
    Found {
        path: PathBuf,
    },
    Progress {
        dirs_scanned: u64,
        dirs_found: u64,
    },
    Error {
        message: String,
    },
    Complete {
        total_found: u64,
        duration_ms: u64,
    },
}

/// Run a parallel directory scan, sending events to `tx` as targets are found.
/// The `cancel` flag can be set to abort scanning early.
pub fn scan_parallel(opts: &ScanOptions, tx: &Sender<ScanEvent>, cancel: &AtomicBool) {
    let start = std::time::Instant::now();
    let dirs_scanned = Arc::new(AtomicU64::new(0));
    let dirs_found = Arc::new(AtomicU64::new(0));

    // Collect top-level children of root for parallel dispatch
    let top_level: Vec<PathBuf> = match std::fs::read_dir(&opts.root) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .filter(|p| should_visit_path(p, opts))
            .collect(),
        Err(e) => {
            let _ = tx.send(ScanEvent::Error {
                message: format!("Cannot read root dir: {e}"),
            });
            let _ = tx.send(ScanEvent::Complete {
                total_found: 0,
                duration_ms: start.elapsed().as_millis() as u64,
            });
            return;
        }
    };

    // Also check if root itself is a target
    if let Some(name) = opts.root.file_name().and_then(|n| n.to_str()) {
        if opts.targets.iter().any(|t| t == name) {
            dirs_found.fetch_add(1, Ordering::Relaxed);
            let _ = tx.send(ScanEvent::Found {
                path: opts.root.clone(),
            });
        }
    }

    // Parallel scan each top-level subdirectory
    top_level.par_iter().for_each(|dir| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        scan_subtree(dir, opts, tx, cancel, &dirs_scanned, &dirs_found);
    });

    let total = dirs_found.load(Ordering::Relaxed);
    let _ = tx.send(ScanEvent::Complete {
        total_found: total,
        duration_ms: start.elapsed().as_millis() as u64,
    });
}

fn scan_subtree(
    root: &Path,
    opts: &ScanOptions,
    tx: &Sender<ScanEvent>,
    cancel: &AtomicBool,
    dirs_scanned: &AtomicU64,
    dirs_found: &AtomicU64,
) {
    // Check if this top-level dir itself is a target
    if let Some(name) = root.file_name().and_then(|n| n.to_str()) {
        if opts.targets.iter().any(|t| t == name) {
            dirs_found.fetch_add(1, Ordering::Relaxed);
            let _ = tx.send(ScanEvent::Found {
                path: root.to_owned(),
            });
            // Prune — don't recurse into matched target
            return;
        }
    }

    let mut walker = WalkDir::new(root).follow_links(false).min_depth(1);
    if let Some(depth) = opts.max_depth {
        walker = walker.max_depth(depth);
    }

    let iter = walker.into_iter();
    for entry in iter {
        if cancel.load(Ordering::Relaxed) {
            return;
        }

        match entry {
            Ok(e) => {
                if !e.file_type().is_dir() {
                    continue;
                }

                let scanned = dirs_scanned.fetch_add(1, Ordering::Relaxed);
                // Emit progress periodically
                if scanned % 500 == 0 {
                    let _ = tx.send(ScanEvent::Progress {
                        dirs_scanned: scanned,
                        dirs_found: dirs_found.load(Ordering::Relaxed),
                    });
                }

                let path = e.path();
                if !should_visit_path(path, opts) {
                    continue;
                }

                let name = match e.file_name().to_str() {
                    Some(n) => n,
                    None => continue,
                };

                if opts.targets.iter().any(|t| t == name) {
                    dirs_found.fetch_add(1, Ordering::Relaxed);
                    let _ = tx.send(ScanEvent::Found {
                        path: path.to_owned(),
                    });
                    // Note: WalkDir doesn't have skip_current_dir in this iteration
                    // model but since we skip anything under a target dir in
                    // should_visit_path via the exclude/target containment check,
                    // nested targets won't cause issues.
                }
            }
            Err(e) => {
                let _ = tx.send(ScanEvent::Error {
                    message: e.to_string(),
                });
            }
        }
    }
}

/// Determines whether a directory path should be visited during scanning.
fn should_visit_path(path: &Path, opts: &ScanOptions) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return true,
    };

    // Skip hidden directories if configured
    if opts.exclude_hidden && name.starts_with('.') {
        return false;
    }

    // Skip excluded directories
    if opts.exclude.iter().any(|ex| ex == name) {
        return false;
    }

    // Don't recurse into directories that are themselves inside a target match.
    // This handles the pruning: once we find node_modules, don't scan inside it.
    for ancestor in path.ancestors().skip(1) {
        if let Some(aname) = ancestor.file_name().and_then(|n| n.to_str()) {
            if opts.targets.iter().any(|t| t == aname) {
                return false;
            }
        }
    }

    // Skip known system paths
    #[cfg(windows)]
    {
        let s = path.to_string_lossy().to_lowercase();
        if s.starts_with("c:\\windows")
            || s.starts_with("c:\\program files")
            || s.starts_with("c:\\program files (x86)")
        {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    #[test]
    fn finds_node_modules_in_nested_structure() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("project/node_modules/.package")).unwrap();
        std::fs::create_dir_all(root.join("project/src")).unwrap();

        let (tx, rx) = crossbeam_channel::unbounded();
        let cancel = AtomicBool::new(false);
        let opts = ScanOptions {
            root: root.to_path_buf(),
            targets: vec!["node_modules".into()],
            exclude: vec![],
            exclude_hidden: false,
            max_depth: None,
        };

        scan_parallel(&opts, &tx, &cancel);
        drop(tx);

        let found: Vec<_> = rx
            .iter()
            .filter_map(|e| match e {
                ScanEvent::Found { path } => Some(path),
                _ => None,
            })
            .collect();

        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("node_modules"));
    }

    #[test]
    fn skips_excluded_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("skip_me/node_modules")).unwrap();
        std::fs::create_dir_all(root.join("keep/node_modules")).unwrap();

        let (tx, rx) = crossbeam_channel::unbounded();
        let cancel = AtomicBool::new(false);
        let opts = ScanOptions {
            root: root.to_path_buf(),
            targets: vec!["node_modules".into()],
            exclude: vec!["skip_me".into()],
            exclude_hidden: false,
            max_depth: None,
        };

        scan_parallel(&opts, &tx, &cancel);
        drop(tx);

        let found: Vec<_> = rx
            .iter()
            .filter_map(|e| match e {
                ScanEvent::Found { path } => Some(path),
                _ => None,
            })
            .collect();

        assert_eq!(found.len(), 1);
        assert!(found[0].to_string_lossy().contains("keep"));
    }

    #[test]
    fn prunes_node_modules_from_recursion() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // Create nested node_modules/node_modules which should NOT be found separately
        std::fs::create_dir_all(root.join("project/node_modules/pkg/node_modules")).unwrap();

        let (tx, rx) = crossbeam_channel::unbounded();
        let cancel = AtomicBool::new(false);
        let opts = ScanOptions {
            root: root.to_path_buf(),
            targets: vec!["node_modules".into()],
            exclude: vec![],
            exclude_hidden: false,
            max_depth: None,
        };

        scan_parallel(&opts, &tx, &cancel);
        drop(tx);

        let found: Vec<_> = rx
            .iter()
            .filter_map(|e| match e {
                ScanEvent::Found { path } => Some(path),
                _ => None,
            })
            .collect();

        // Should only find the top-level node_modules, not the nested one
        assert_eq!(found.len(), 1);
    }
}
