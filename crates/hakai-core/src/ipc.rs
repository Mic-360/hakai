use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::config::HakaiConfig;
use crate::deleter;
use crate::risk;
use crate::scanner::{self, ScanEvent, ScanOptions};
use crate::sizer;

// ── Commands Rust receives FROM Bun ──────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd")]
pub enum IpcCommand {
    StartScan {
        root: String,
        targets: Vec<String>,
        exclude: Vec<String>,
        exclude_hidden: bool,
        max_depth: Option<usize>,
    },
    StopScan,
    GetSize {
        path: String,
    },
    Delete {
        paths: Vec<String>,
        dry_run: bool,
    },
    DeleteAll {
        dry_run: bool,
    },
    AnalyzeRisk {
        path: String,
        target: String,
    },
}

// ── Events Rust sends TO Bun ─────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(tag = "event")]
pub enum IpcEvent {
    Ready,
    ScanFound {
        path: String,
    },
    ScanSize {
        path: String,
        size_bytes: u64,
        newest_ms: u64,
    },
    ScanRisk {
        path: String,
        is_dead: bool,
        risk: String,
    },
    ScanProgress {
        scanned: u64,
        found: u64,
    },
    ScanComplete {
        total_found: u64,
        duration_ms: u64,
    },
    DeleteProgress {
        path: String,
        status: String,
        freed_bytes: u64,
    },
    DeleteComplete {
        total_freed: u64,
    },
    Error {
        message: String,
    },
}

/// Write a single IPC event as a JSON line to stdout.
async fn emit(event: &IpcEvent) {
    let mut stdout = tokio::io::stdout();
    if let Ok(json) = serde_json::to_string(event) {
        let line = format!("{json}\n");
        let _ = stdout.write_all(line.as_bytes()).await;
        let _ = stdout.flush().await;
    }
}

/// Emit an event synchronously (for use from non-async contexts like scanner threads).
fn emit_sync(event: &IpcEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        let line = format!("{json}\n");
        let _ = std::io::Write::write_all(&mut std::io::stdout(), line.as_bytes());
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }
}

/// Run the IPC server loop — reads JSON commands from stdin, dispatches actions.
pub async fn run_ipc_server(_config: &HakaiConfig) {
    emit(&IpcEvent::Ready).await;

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut found_paths: Vec<PathBuf> = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<IpcCommand>(&line) {
            Ok(cmd) => {
                handle_command(cmd, &cancel_flag, &mut found_paths).await;
            }
            Err(e) => {
                emit(&IpcEvent::Error {
                    message: format!("Invalid command: {e}"),
                })
                .await;
            }
        }
    }
}

async fn handle_command(
    cmd: IpcCommand,
    cancel_flag: &Arc<AtomicBool>,
    found_paths: &mut Vec<PathBuf>,
) {
    match cmd {
        IpcCommand::StartScan {
            root,
            targets,
            exclude,
            exclude_hidden,
            max_depth,
        } => {
            cancel_flag.store(false, Ordering::Relaxed);
            found_paths.clear();

            let opts = ScanOptions {
                root: PathBuf::from(&root),
                targets,
                exclude,
                exclude_hidden,
                max_depth,
            };

            let (tx, rx) = crossbeam_channel::unbounded();
            let cancel = cancel_flag.clone();

            // Spawn scanner on a blocking thread pool
            let scan_opts = opts.clone();
            tokio::task::spawn_blocking(move || {
                scanner::scan_parallel(&scan_opts, &tx, &cancel);
            });

            // Forward scan events as IPC events
            while let Ok(event) = rx.recv() {
                match event {
                    ScanEvent::Found { path } => {
                        let path_str = path.to_string_lossy().to_string();
                        found_paths.push(path.clone());
                        emit(&IpcEvent::ScanFound { path: path_str }).await;

                        // Concurrently calculate size
                        let p = path.clone();
                        tokio::task::spawn_blocking(move || {
                            let size = sizer::calculate_size(&p);
                            let newest = sizer::get_newest_file_time(&p).unwrap_or(0);
                            emit_sync(&IpcEvent::ScanSize {
                                path: p.to_string_lossy().to_string(),
                                size_bytes: size,
                                newest_ms: newest,
                            });
                        });
                    }
                    ScanEvent::Progress {
                        dirs_scanned,
                        dirs_found,
                    } => {
                        emit(&IpcEvent::ScanProgress {
                            scanned: dirs_scanned,
                            found: dirs_found,
                        })
                        .await;
                    }
                    ScanEvent::Error { message } => {
                        emit(&IpcEvent::Error { message }).await;
                    }
                    ScanEvent::Complete {
                        total_found,
                        duration_ms,
                    } => {
                        emit(&IpcEvent::ScanComplete {
                            total_found,
                            duration_ms,
                        })
                        .await;
                        break;
                    }
                }
            }
        }

        IpcCommand::StopScan => {
            cancel_flag.store(true, Ordering::Relaxed);
        }

        IpcCommand::GetSize { path } => {
            let p = PathBuf::from(&path);
            let size = sizer::calculate_size(&p);
            let newest = sizer::get_newest_file_time(&p).unwrap_or(0);
            emit(&IpcEvent::ScanSize {
                path,
                size_bytes: size,
                newest_ms: newest,
            })
            .await;
        }

        IpcCommand::AnalyzeRisk { path, target } => {
            let p = PathBuf::from(&path);
            let result = risk::analyze_risk(&p, &target);
            emit(&IpcEvent::ScanRisk {
                path,
                is_dead: result.is_dead,
                risk: format!("{:?}", result.risk_level).to_lowercase(),
            })
            .await;
        }

        IpcCommand::Delete { paths, dry_run } => {
            let mut total_freed = 0u64;
            for path_str in &paths {
                let path = PathBuf::from(path_str);
                emit(&IpcEvent::DeleteProgress {
                    path: path_str.clone(),
                    status: "deleting".into(),
                    freed_bytes: 0,
                })
                .await;

                let result = deleter::delete_dir(path, dry_run).await;
                match &result {
                    deleter::DeleteResult::Success { path, freed_bytes } => {
                        total_freed += freed_bytes;
                        emit(&IpcEvent::DeleteProgress {
                            path: path.to_string_lossy().to_string(),
                            status: "deleted".into(),
                            freed_bytes: *freed_bytes,
                        })
                        .await;
                    }
                    deleter::DeleteResult::Error { path, message } => {
                        emit(&IpcEvent::DeleteProgress {
                            path: path.to_string_lossy().to_string(),
                            status: format!("error: {message}"),
                            freed_bytes: 0,
                        })
                        .await;
                    }
                }
            }
            emit(&IpcEvent::DeleteComplete { total_freed }).await;
        }

        IpcCommand::DeleteAll { dry_run } => {
            let paths: Vec<PathBuf> = found_paths.iter().cloned().collect();
            let results = deleter::delete_batch(paths, dry_run, 8).await;

            let mut total_freed = 0u64;
            for result in &results {
                match result {
                    deleter::DeleteResult::Success { path, freed_bytes } => {
                        total_freed += freed_bytes;
                        emit(&IpcEvent::DeleteProgress {
                            path: path.to_string_lossy().to_string(),
                            status: "deleted".into(),
                            freed_bytes: *freed_bytes,
                        })
                        .await;
                    }
                    deleter::DeleteResult::Error { path, message } => {
                        emit(&IpcEvent::DeleteProgress {
                            path: path.to_string_lossy().to_string(),
                            status: format!("error: {message}"),
                            freed_bytes: 0,
                        })
                        .await;
                    }
                }
            }
            emit(&IpcEvent::DeleteComplete { total_freed }).await;
        }
    }
}
