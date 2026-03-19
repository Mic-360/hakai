mod config;
mod deleter;
mod ipc;
pub mod platform;
mod risk;
mod scanner;
mod sizer;
mod tui;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::Parser;
use serde::Serialize;

use scanner::ScanOptions;

/// 🦀 hakai — The strongest directory destroyer
#[derive(Parser, Debug)]
#[command(name = "hakai", version = "1.0.0", about = "🦀 Hakai — \"Throughout the filesystem, I alone am the honored one.\"")]
struct Args {
    /// Start scan from this directory (default: current dir)
    #[arg(short = 'd', long = "directory")]
    directory: Option<String>,

    /// Start scan from $HOME
    #[arg(short = 'f', long = "full")]
    full: bool,

    /// Target directory names (comma-separated)
    #[arg(short = 't', long = "target", value_delimiter = ',')]
    target: Option<Vec<String>>,

    /// Use a named profile from .hakairc
    #[arg(short = 'p', long = "profile")]
    profile: Option<String>,

    /// Exclude directories (comma-separated)
    #[arg(short = 'E', long = "exclude", value_delimiter = ',')]
    exclude: Option<Vec<String>>,

    /// Exclude hidden/dot directories
    #[arg(short = 'x', long = "exclude-hidden")]
    exclude_hidden: bool,

    /// Maximum scan depth
    #[arg(long = "max-depth")]
    max_depth: Option<usize>,

    /// Auto-delete all found directories
    #[arg(short = 'D', long = "delete-all")]
    delete_all: bool,

    /// Skip confirmation on --delete-all
    #[arg(short = 'y')]
    yes: bool,

    /// Simulate deletion (no actual deletes)
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Sort by: path, size, last-mod
    #[arg(short = 's', long = "sort")]
    sort: Option<String>,

    /// Output all results as JSON at end of scan
    #[arg(long = "json")]
    json: bool,

    /// Stream results as newline-delimited JSON
    #[arg(long = "json-stream")]
    json_stream: bool,

    /// Suppress error messages
    #[arg(short = 'e', long = "hide-errors")]
    hide_errors: bool,

    /// Rayon thread pool size (0 = auto)
    #[arg(long = "threads")]
    threads: Option<usize>,

    /// Disable parallel scan
    #[arg(long = "no-parallel")]
    no_parallel: bool,

    /// Run in IPC server mode (used by hakai-tui)
    #[arg(long = "ipc", hide = true)]
    ipc: bool,

    /// Skip update check
    #[arg(long = "no-check-update")]
    no_check_update: bool,

    /// Highlight color
    #[arg(short = 'c', long = "color")]
    color: Option<String>,

    /// Size unit: auto, mb, gb
    #[arg(long = "size-unit")]
    size_unit: Option<String>,

    /// Minimum directory size to display (e.g. 10mb, 1gb)
    #[arg(long = "min-size")]
    min_size: Option<String>,
}

#[derive(Serialize)]
struct JsonOutput {
    meta: JsonMeta,
    results: Vec<JsonResult>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonMeta {
    version: String,
    scan_root: String,
    targets: Vec<String>,
    duration_ms: u64,
    dirs_scanned: u64,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct JsonResult {
    path: String,
    size: u64,
    #[serde(rename = "modificationTime")]
    modification_time: u64,
    #[serde(rename = "isDead")]
    is_dead: bool,
    #[serde(rename = "riskLevel")]
    risk_level: String,
}

#[derive(Serialize)]
struct JsonSummary {
    total_found: u64,
    total_size_bytes: u64,
    total_size_human: String,
}

fn main() {
    let args = Args::parse();
    let cfg = config::load_config();

    // Configure thread pool
    if let Some(threads) = args.threads {
        if threads > 0 {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()
                .ok();
        }
    } else if cfg.settings.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(cfg.settings.threads)
            .build_global()
            .ok();
    }

    if args.ipc {
        // IPC server mode — communicate with Bun TUI via stdin/stdout JSON
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(ipc::run_ipc_server(&cfg));
        return;
    }

    // Determine targets
    let targets = if let Some(profile_name) = &args.profile {
        let builtins = config::builtin_profiles();
        let all_profiles = cfg.profiles.iter().chain(builtins.iter());
        let mut found = None;
        for (name, profile) in all_profiles {
            if name == profile_name {
                found = Some(profile.targets.clone());
                break;
            }
        }
        found.unwrap_or_else(|| {
            eprintln!("Unknown profile: {profile_name}. Available: node, rust, python, flutter, java, all");
            std::process::exit(1);
        })
    } else if let Some(ref t) = args.target {
        t.clone()
    } else {
        vec!["node_modules".into()]
    };

    // Determine root directory
    let root = if args.full {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else if let Some(ref d) = args.directory {
        PathBuf::from(d)
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let exclude = args.exclude.clone().unwrap_or_default();
    let exclude_hidden = args.exclude_hidden || cfg.settings.exclude_hidden;

    let scan_opts = ScanOptions {
        root: root.clone(),
        targets: targets.clone(),
        exclude,
        exclude_hidden,
        max_depth: args.max_depth,
    };

    if args.json || args.json_stream || args.delete_all {
        // Headless mode
        run_headless(args, scan_opts, &targets);
    } else {
        // Interactive TUI mode (built-in ratatui)
        let sort_mode = match args.sort.as_deref().unwrap_or(&cfg.settings.default_sort) {
            "size" => tui::app::SortMode::Size,
            "last-mod" => tui::app::SortMode::LastMod,
            _ => tui::app::SortMode::Path,
        };
        let min_size_bytes = args.min_size.as_deref().and_then(parse_size);

        if let Err(e) = tui::run_tui(scan_opts, sort_mode, args.dry_run, min_size_bytes) {
            eprintln!("TUI error: {e}");
            std::process::exit(1);
        }
    }
}

fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_lowercase();
    if let Some(num) = s.strip_suffix("gb") {
        num.trim().parse::<f64>().ok().map(|n| (n * 1_073_741_824.0) as u64)
    } else if let Some(num) = s.strip_suffix("mb") {
        num.trim().parse::<f64>().ok().map(|n| (n * 1_048_576.0) as u64)
    } else if let Some(num) = s.strip_suffix("kb") {
        num.trim().parse::<f64>().ok().map(|n| (n * 1024.0) as u64)
    } else {
        s.parse::<u64>().ok()
    }
}

fn run_headless(args: Args, scan_opts: ScanOptions, targets: &[String]) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel = Arc::new(AtomicBool::new(false));

    let opts_clone = scan_opts.clone();
    std::thread::spawn(move || {
        scanner::scan_parallel(&opts_clone, &tx, cancel);
    });

    let mut results: Vec<JsonResult> = Vec::new();
    let mut total_size = 0u64;
    let mut duration_ms = 0u64;

    for event in rx {
        match event {
            scanner::ScanEvent::Found { path } => {
                let (size, newest) = sizer::calculate_size_and_mtime(&path);

                let target_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let risk_result = risk::analyze_risk(&path, target_name);

                total_size += size;

                let jr = JsonResult {
                    path: path.to_string_lossy().to_string(),
                    size,
                    modification_time: newest,
                    is_dead: risk_result.is_dead,
                    risk_level: format!("{:?}", risk_result.risk_level).to_lowercase(),
                };

                if args.json_stream {
                    if let Ok(json) = serde_json::to_string(&jr) {
                        println!("{json}");
                    }
                }

                results.push(jr);
            }
            scanner::ScanEvent::Complete {
                total_found: _,
                duration_ms: d,
            } => {
                duration_ms = d;
                break;
            }
            scanner::ScanEvent::Error { message } => {
                if !args.hide_errors {
                    eprintln!("Error: {message}");
                }
            }
            scanner::ScanEvent::Progress { .. } => {}
        }
    }

    // Sort results
    let sort_mode = args
        .sort
        .as_deref()
        .unwrap_or(&"path");
    match sort_mode {
        "size" => results.sort_by(|a, b| b.size.cmp(&a.size)),
        "last-mod" => results.sort_by(|a, b| b.modification_time.cmp(&a.modification_time)),
        _ => results.sort_by(|a, b| a.path.cmp(&b.path)),
    }

    if args.json {
        let total_found = results.len() as u64;
        let output = JsonOutput {
            meta: JsonMeta {
                version: "1.0.0".into(),
                scan_root: scan_opts.root.to_string_lossy().to_string(),
                targets: targets.to_vec(),
                duration_ms,
                dirs_scanned: total_found,
            },
            results,
            summary: JsonSummary {
                total_found,
                total_size_bytes: total_size,
                total_size_human: format_human_size(total_size),
            },
        };
        if let Ok(json) = serde_json::to_string_pretty(&output) {
            println!("{json}");
        }
    } else if args.delete_all {
        if results.is_empty() {
            eprintln!("No directories found to delete.");
            return;
        }

        let total_display = format_human_size(total_size);
        if !args.yes {
            eprintln!(
                "🔵 Red, 🔴 Blue... 🟣 Hollow Purple! Prepare to delete {} directories ({}). Continue? [y/N] ",
                results.len(),
                total_display
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if !input.trim().eq_ignore_ascii_case("y") {
                eprintln!("Tch. Changed your mind? Aborted.");
                return;
            }
        }

        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        let items: Vec<(PathBuf, u64)> = results
            .iter()
            .map(|r| (PathBuf::from(&r.path), r.size))
            .collect();
        let delete_results = rt.block_on(deleter::delete_batch_with_sizes(items, args.dry_run, 8));

        let mut total_freed = 0u64;
        for result in &delete_results {
            match result {
                deleter::DeleteResult::Success { path, freed_bytes } => {
                    total_freed += freed_bytes;
                    eprintln!("✓ Deleted: {} ({})", path.display(), format_human_size(*freed_bytes));
                }
                deleter::DeleteResult::Error { path, message } => {
                    eprintln!("✗ Error: {} — {}", path.display(), message);
                }
            }
        }
        eprintln!("\n🦀 Freed: {}. Domain Expansion: Infinite Free Space!", format_human_size(total_freed));
    }
}

fn format_human_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

