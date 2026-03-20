pub mod app;
pub mod theme;
mod ui;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::risk::RiskLevel;
use crate::scanner::{ScanEvent, ScanOptions};
use crate::{deleter, risk, scanner, sizer};

use app::{App, AppMode, SortMode};

enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    ScanFound(PathBuf),
    ScanProgress { scanned: u64 },
    ScanError(String),
    ScanComplete { duration_ms: u64 },
    SizeCalculated { path: PathBuf, size: u64, newest_ms: u64, project_name: Option<String> },
    RiskAnalyzed { path: PathBuf, is_dead: bool, risk_level: RiskLevel },
    DeleteResult { path: PathBuf, freed_bytes: u64, error: Option<String>, trash_path: Option<PathBuf> },
    PreviewReady { entries: Vec<(String, u64)> },
    PermissionDenied,
}

// ── Public entry point ───────────────────────────────────────────

pub fn run_tui(
    scan_opts: ScanOptions,
    sort_mode: SortMode,
    dry_run: bool,
    min_size: Option<u64>,
) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::cursor::Hide,
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(sort_mode, dry_run, min_size);

    let result = run_event_loop(&mut terminal, &mut app, scan_opts);

    terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    scan_opts: ScanOptions,
) -> Result<()> {
    let (event_tx, event_rx): (Sender<AppEvent>, Receiver<AppEvent>) =
        crossbeam_channel::unbounded();

    let input_tx = event_tx.clone();
    let input_running = Arc::new(AtomicBool::new(true));
    let input_flag = input_running.clone();
    std::thread::spawn(move || {
        while input_flag.load(Ordering::Relaxed) {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    match evt {
                        Event::Key(key) => {
                            input_tx.send(AppEvent::Key(key)).ok();
                        }
                        Event::Mouse(mouse) => {
                            input_tx.send(AppEvent::Mouse(mouse)).ok();
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    let scanner_tx = event_tx.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    let scan_opts_clone = scan_opts.clone();
    std::thread::spawn(move || {
        let (tx, rx) = crossbeam_channel::unbounded();
        let scan_cancel = cancel_clone.clone();

        std::thread::spawn(move || {
            scanner::scan_parallel(&scan_opts_clone, &tx, scan_cancel);
        });

        for evt in rx {
            match evt {
                ScanEvent::Found { path } => {
                    let _ = scanner_tx.send(AppEvent::ScanFound(path));
                }
                ScanEvent::Progress { dirs_scanned, .. } => {
                    let _ = scanner_tx.send(AppEvent::ScanProgress {
                        scanned: dirs_scanned,
                    });
                }
                ScanEvent::Error { message } => {
                    if message.contains("ermission denied")
                        || message.contains("ccess is denied")
                    {
                        let _ = scanner_tx.send(AppEvent::PermissionDenied);
                    } else {
                        let _ = scanner_tx.send(AppEvent::ScanError(message));
                    }
                }
                ScanEvent::Complete { duration_ms, .. } => {
                    let _ = scanner_tx.send(AppEvent::ScanComplete { duration_ms });
                    break;
                }
            }
        }
    });

    loop {
        app.rebuild_filter_if_dirty();
        terminal.draw(|frame| ui::draw(frame, app))?;

        match event_rx.recv_timeout(Duration::from_millis(33)) {
            Ok(evt) => {
                handle_event(app, &event_tx, evt, &cancel);
                while let Ok(evt) = event_rx.try_recv() {
                    handle_event(app, &event_tx, evt, &cancel);
                    if app.should_quit {
                        break;
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        if app.should_quit {
            break;
        }

        for trash_path in app.cleanup_expired_undo() {
            std::thread::spawn(move || {
                let _ = deleter::fast_remove_dir_all(&trash_path);
            });
        }
    }

    input_running.store(false, Ordering::Relaxed);
    cancel.store(true, Ordering::Relaxed);

    Ok(())
}

fn handle_event(
    app: &mut App,
    event_tx: &Sender<AppEvent>,
    evt: AppEvent,
    cancel: &Arc<AtomicBool>,
) {
    match evt {
        AppEvent::Key(key) => handle_key(app, event_tx, key, cancel),
        AppEvent::Mouse(mouse) => handle_mouse(app, mouse),
        AppEvent::ScanFound(path) => {
            let tx = event_tx.clone();
            let p = path.clone();
            rayon::spawn(move || {
                let (size, newest) = sizer::calculate_size_and_mtime(&p);
                let project_name = detect_project_name(&p);
                tx.send(AppEvent::SizeCalculated {
                    path: p,
                    size,
                    newest_ms: newest,
                    project_name,
                })
                .ok();
            });
            app.add_result(path);
        }
        AppEvent::ScanProgress { scanned } => {
            app.dirs_scanned = scanned;
        }
        AppEvent::ScanError(msg) => {
            app.errors.push(msg);
        }
        AppEvent::ScanComplete { duration_ms } => {
            app.scan_complete = true;
            app.scan_duration_ms = duration_ms;

            for r in &app.results {
                let tx = event_tx.clone();
                let path = r.path.clone();
                let target = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("node_modules")
                    .to_string();
                rayon::spawn(move || {
                    let result = risk::analyze_risk(&path, &target);
                    tx.send(AppEvent::RiskAnalyzed {
                        path,
                        is_dead: result.is_dead,
                        risk_level: result.risk_level,
                    })
                    .ok();
                });
            }
        }
        AppEvent::SizeCalculated {
            path,
            size,
            newest_ms,
            project_name,
        } => {
            app.update_size(&path, size, newest_ms);
            if project_name.is_some() {
                app.update_project_name(&path, project_name);
            }
        }
        AppEvent::RiskAnalyzed {
            path,
            is_dead,
            risk_level,
        } => {
            app.update_risk(&path, is_dead, risk_level);
        }
        AppEvent::DeleteResult {
            path,
            freed_bytes,
            error,
            trash_path,
        } => {
            if let Some(err) = error {
                app.mark_error(&path, err);
            } else {
                app.mark_deleted(&path, freed_bytes);
                app.flash_freed();
                if let Some(tp) = trash_path {
                    app.push_undo(path.clone(), tp, freed_bytes);
                }
            }
            let still_deleting = app
                .results
                .iter()
                .any(|r| r.status == app::FolderStatus::Deleting);
            if !still_deleting && app.mode == AppMode::Deleting {
                app.mode = AppMode::Normal;
            }
        }
        AppEvent::PreviewReady { entries } => {
            app.preview_entries = entries;
        }
        AppEvent::PermissionDenied => {
            app.permission_denied_count += 1;
        }
    }
}

fn handle_key(
    app: &mut App,
    event_tx: &Sender<AppEvent>,
    key: KeyEvent,
    _cancel: &Arc<AtomicBool>,
) {
    if app.mode == AppMode::Search {
        match key.code {
            KeyCode::Esc => app.handle_escape(),
            KeyCode::Enter => app.mode = AppMode::Normal,
            KeyCode::Backspace => app.search_pop_char(),
            KeyCode::Char(c) => app.search_push_char(c),
            _ => {}
        }
        return;
    }

    if app.mode == AppMode::Help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                app.mode = AppMode::Normal;
            }
            _ => {}
        }
        return;
    }

    if app.mode == AppMode::Preview {
        match key.code {
            KeyCode::Esc | KeyCode::Char('p') | KeyCode::Char('q') => {
                app.mode = AppMode::Normal;
            }
            _ => {}
        }
        return;
    }

    if app.show_errors {
        if key.code == KeyCode::Char('e') || key.code == KeyCode::Esc {
            app.show_errors = false;
        }
        return;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    if app.pending_delete.is_some() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char(' ') | KeyCode::Delete => {
                if let Some((path, size)) = app.confirm_pending_delete() {
                    spawn_delete(event_tx.clone(), path, size, app.dry_run);
                }
            }
            _ => {
                app.pending_delete = None;
            }
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
        KeyCode::PageUp => app.move_selection(-(app.list_height as i32)),
        KeyCode::PageDown => app.move_selection(app.list_height as i32),
        KeyCode::Home => app.go_home(),
        KeyCode::End => app.go_end(),
        KeyCode::Char(' ') | KeyCode::Delete => {
            if let Some((path, size)) = app.handle_space_or_delete() {
                spawn_delete(event_tx.clone(), path, size, app.dry_run);
            }
        }
        KeyCode::Enter => {
            let items = app.handle_enter();
            for (path, size) in items {
                spawn_delete(event_tx.clone(), path, size, app.dry_run);
            }
        }
        KeyCode::Char('t') | KeyCode::Char('T') => app.toggle_multi_select(),
        KeyCode::Char('a') | KeyCode::Char('A') => app.select_all(),
        KeyCode::Char('v') | KeyCode::Char('V') => app.toggle_range_select(),
        KeyCode::Char('/') => app.enter_search(),
        KeyCode::Char('s') => app.cycle_sort(),
        KeyCode::Char('e') => app.show_errors = !app.show_errors,
        KeyCode::Char('o') => open_directory(app),
        KeyCode::Char('p') => {
            if let Some(r) = app.get_selected_result() {
                let path = r.path.clone();
                app.mode = AppMode::Preview;
                app.preview_entries.clear();
                let tx = event_tx.clone();
                std::thread::spawn(move || {
                    let entries = collect_preview_entries(&path);
                    tx.send(AppEvent::PreviewReady { entries }).ok();
                });
            }
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            if let Some(entry) = app.undo_stack.pop() {
                let original = entry.original_path.clone();
                app.restore_from_undo(&original, entry.size_bytes);
                std::thread::spawn(move || {
                    let _ = std::fs::rename(&entry.trash_path, &entry.original_path);
                });
            }
        }
        KeyCode::Char('?') => app.mode = AppMode::Help,
        KeyCode::Esc => app.handle_escape(),
        _ => {}
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => app.move_selection(-3),
        MouseEventKind::ScrollDown => app.move_selection(3),
        MouseEventKind::Down(MouseButton::Left) => {
            let header_height = 5u16;
            if mouse.row >= header_height {
                let clicked = (mouse.row - header_height) as usize + app.scroll_offset;
                if clicked < app.filtered_indices.len() {
                    app.selected_index = clicked;
                }
            }
        }
        _ => {}
    }
}

fn spawn_delete(tx: Sender<AppEvent>, path: PathBuf, known_size: u64, dry_run: bool) {
    std::thread::spawn(move || {
        if dry_run {
            tx.send(AppEvent::DeleteResult {
                path,
                freed_bytes: known_size,
                error: None,
                trash_path: None,
            })
            .ok();
            return;
        }

        if let Some(trash) = deleter::trash_path_for(&path) {
            if std::fs::rename(&path, &trash).is_ok() {
                tx.send(AppEvent::DeleteResult {
                    path,
                    freed_bytes: known_size,
                    error: None,
                    trash_path: Some(trash),
                })
                .ok();
                return;
            }
        }

        match deleter::fast_remove_dir_all(&path) {
            Ok(freed) => {
                tx.send(AppEvent::DeleteResult {
                    path,
                    freed_bytes: freed,
                    error: None,
                    trash_path: None,
                })
                .ok();
            }
            Err((freed, e)) => {
                tx.send(AppEvent::DeleteResult {
                    path,
                    freed_bytes: freed,
                    error: Some(e.to_string()),
                    trash_path: None,
                })
                .ok();
            }
        }
    });
}

fn detect_project_name(target_path: &std::path::Path) -> Option<String> {
    let parent = target_path.parent()?;
    let markers = [
        "package.json",
        "Cargo.toml",
        "pubspec.yaml",
        "build.gradle",
        "pom.xml",
        "pyproject.toml",
        "go.mod",
    ];
    for marker in &markers {
        if parent.join(marker).exists() {
            return parent.file_name()?.to_str().map(|s| s.to_string());
        }
    }
    let grandparent = parent.parent()?;
    for marker in &markers {
        if grandparent.join(marker).exists() {
            return Some(format!(
                "{}/{}",
                grandparent.file_name()?.to_str()?,
                parent.file_name()?.to_str()?
            ));
        }
    }
    None
}

fn collect_preview_entries(root: &std::path::Path) -> Vec<(String, u64)> {
    let mut entries = Vec::new();

    fn walk(
        dir: &std::path::Path,
        root: &std::path::Path,
        depth: usize,
        entries: &mut Vec<(String, u64)>,
    ) {
        if depth > 2 {
            return;
        }
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return,
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let name = rel.to_string_lossy().to_string();
            if meta.is_file() {
                entries.push((name, meta.len()));
            } else if meta.is_dir() && depth < 2 {
                walk(&path, root, depth + 1, entries);
            }
        }
    }

    walk(root, root, 0, &mut entries);
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.truncate(15);
    entries
}

fn open_directory(app: &App) {
    let result = match app.get_selected_result() {
        Some(r) => r,
        None => return,
    };

    let parent = match result.path.parent() {
        Some(p) => p.to_owned(),
        None => return,
    };

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(&parent)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&parent).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(&parent)
            .spawn();
    }
}
