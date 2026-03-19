use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::Sender;
use ignore::WalkState;
use serde::Serialize;

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

/// Run a parallel directory scan using work-stealing across all directory levels.
/// Sends events to `tx` as targets are found. The `cancel` flag aborts scanning early.
pub fn scan_parallel(opts: &ScanOptions, tx: &Sender<ScanEvent>, cancel: Arc<AtomicBool>) {
    let start = std::time::Instant::now();
    let dirs_scanned = Arc::new(AtomicU64::new(0));
    let dirs_found = Arc::new(AtomicU64::new(0));

    let targets: Arc<HashSet<String>> = Arc::new(opts.targets.iter().cloned().collect());
    let exclude: Arc<HashSet<String>> = Arc::new(opts.exclude.iter().cloned().collect());

    // Check if root itself is a target
    if let Some(name) = opts.root.file_name().and_then(|n| n.to_str()) {
        if targets.contains(name) {
            dirs_found.fetch_add(1, Ordering::Relaxed);
            let _ = tx.send(ScanEvent::Found {
                path: opts.root.clone(),
            });
            let _ = tx.send(ScanEvent::Complete {
                total_found: 1,
                duration_ms: start.elapsed().as_millis() as u64,
            });
            return;
        }
    }

    let mut builder = ignore::WalkBuilder::new(&opts.root);
    builder
        .hidden(opts.exclude_hidden)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .follow_links(false)
        .threads(0); // auto-detect CPU count

    if let Some(depth) = opts.max_depth {
        builder.max_depth(Some(depth));
    }

    builder.build_parallel().run(|| {
        let tx = tx.clone();
        let cancel = cancel.clone();
        let targets = targets.clone();
        let exclude = exclude.clone();
        let dirs_scanned = dirs_scanned.clone();
        let dirs_found = dirs_found.clone();

        Box::new(move |entry| {
            if cancel.load(Ordering::Relaxed) {
                return WalkState::Quit;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    let _ = tx.send(ScanEvent::Error {
                        message: e.to_string(),
                    });
                    return WalkState::Continue;
                }
            };

            // Only process directories
            if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                return WalkState::Continue;
            }

            // Skip root entry (depth 0)
            if entry.depth() == 0 {
                return WalkState::Continue;
            }

            let name = match entry.file_name().to_str() {
                Some(n) => n,
                None => return WalkState::Continue,
            };

            // Skip excluded directories
            if exclude.contains(name) {
                return WalkState::Skip;
            }

            // Skip known system paths on Windows
            #[cfg(windows)]
            if is_windows_system_path(entry.path()) {
                return WalkState::Skip;
            }

            // Progress tracking
            let scanned = dirs_scanned.fetch_add(1, Ordering::Relaxed) + 1;
            if scanned % 500 == 0 {
                let _ = tx.send(ScanEvent::Progress {
                    dirs_scanned: scanned,
                    dirs_found: dirs_found.load(Ordering::Relaxed),
                });
            }

            // Check if this is a target directory
            if targets.contains(name) {
                dirs_found.fetch_add(1, Ordering::Relaxed);
                let _ = tx.send(ScanEvent::Found {
                    path: entry.into_path(),
                });
                return WalkState::Skip; // Don't recurse into target dirs
            }

            WalkState::Continue
        })
    });

    let total = dirs_found.load(Ordering::Relaxed);
    let _ = tx.send(ScanEvent::Complete {
        total_found: total,
        duration_ms: start.elapsed().as_millis() as u64,
    });
}

#[cfg(windows)]
#[inline]
fn is_windows_system_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};

    let mut comps = path.components();
    let on_c_drive = matches!(
        comps.next(),
        Some(Component::Prefix(prefix_comp))
            if matches!(prefix_comp.kind(), Prefix::Disk(drive) if drive.eq_ignore_ascii_case(&b'c'))
    );

    if !on_c_drive {
        return false;
    }

    if matches!(comps.next(), Some(Component::RootDir)) {
        if let Some(Component::Normal(seg)) = comps.next() {
            let s = seg.to_string_lossy();
            return s.eq_ignore_ascii_case("windows")
                || s.eq_ignore_ascii_case("program files")
                || s.eq_ignore_ascii_case("program files (x86)");
        }
    }

    false
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
        let cancel = Arc::new(AtomicBool::new(false));
        let opts = ScanOptions {
            root: root.to_path_buf(),
            targets: vec!["node_modules".into()],
            exclude: vec![],
            exclude_hidden: false,
            max_depth: None,
        };

        scan_parallel(&opts, &tx, cancel);
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
        let cancel = Arc::new(AtomicBool::new(false));
        let opts = ScanOptions {
            root: root.to_path_buf(),
            targets: vec!["node_modules".into()],
            exclude: vec!["skip_me".into()],
            exclude_hidden: false,
            max_depth: None,
        };

        scan_parallel(&opts, &tx, cancel);
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
        let cancel = Arc::new(AtomicBool::new(false));
        let opts = ScanOptions {
            root: root.to_path_buf(),
            targets: vec!["node_modules".into()],
            exclude: vec![],
            exclude_hidden: false,
            max_depth: None,
        };

        scan_parallel(&opts, &tx, cancel);
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
