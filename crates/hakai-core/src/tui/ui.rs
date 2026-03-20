use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{format_age, format_size, App, AppMode, FolderStatus};
use super::theme;

const SPINNER: &[char] = &['\u{280b}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283c}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280f}'];

fn spinner_char() -> char {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    SPINNER[(ms / 80) as usize % SPINNER.len()]
}

fn age_color(newest_ms: u64) -> Style {
    if newest_ms == 0 {
        return theme::dim();
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let age_days = now_ms.saturating_sub(newest_ms) / 86_400_000;
    if age_days > 365 {
        Style::default().fg(Color::DarkGray)
    } else if age_days > 180 {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(theme::WHITE)
    }
}

// ── Main draw entry point ────────────────────────────────────────

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(3), // header
        Constraint::Length(1), // stats
        Constraint::Length(1), // progress
        Constraint::Min(3),   // results list
        Constraint::Length(2), // status bar
    ])
    .split(area);

    app.list_height = chunks[3].height.saturating_sub(0) as usize;

    draw_header(frame, app, chunks[0]);
    draw_stats(frame, app, chunks[1]);
    draw_progress(frame, app, chunks[2]);
    draw_results(frame, app, chunks[3]);
    draw_status_bar(frame, app, chunks[4]);

    // Overlays
    if app.show_errors && !app.errors.is_empty() {
        draw_error_popup(frame, app, area);
    }
    if app.mode == AppMode::Help {
        draw_help_popup(frame, area);
    }
    if app.mode == AppMode::Search {
        draw_search_bar(frame, app, area);
    }
}

// ── Header ───────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title_line = Line::from(vec![
        Span::styled("  \u{1f980} Hakai", theme::highlight()),
        Span::styled(" v1.0.0", theme::dim()),
        Span::raw("  "),
        Span::styled(
            format!("Sort: {} \u{25bc}", app.sort_mode.label()),
            theme::dim(),
        ),
    ]);

    let mode_span = match app.mode {
        AppMode::MultiSelect => Span::styled(" [MULTI-SELECT] ", theme::warning()),
        AppMode::RangeSelect => Span::styled(" [RANGE-SELECT] ", theme::warning()),
        AppMode::Deleting => Span::styled(" [DELETING...] ", theme::error()),
        _ => Span::raw(""),
    };

    let quote_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(app.current_quote, Style::default().fg(theme::DIM).add_modifier(Modifier::ITALIC)),
        mode_span,
    ]);

    let sep = "\u{2500}".repeat(area.width as usize);
    let sep_line = Line::from(Span::styled(sep, theme::dim()));

    let para = Paragraph::new(vec![title_line, quote_line, sep_line]);
    frame.render_widget(para, area);
}

// ── Stats bar ────────────────────────────────────────────────────

fn draw_stats(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::raw("  "),
        Span::styled("Found: ", theme::dim()),
        Span::styled(format!("{}", app.found_count()), theme::highlight()),
    ];

    if !app.search_query.is_empty() {
        spans.push(Span::styled(
            format!(" (showing {})", app.visible_count()),
            theme::dim(),
        ));
    }

    spans.extend_from_slice(&[
        Span::styled("  \u{00b7}  ", theme::dim()),
        Span::styled("Total: ", theme::dim()),
        Span::styled(format_size(app.total_size), theme::highlight()),
    ]);

    if app.freed_space > 0 {
        spans.extend_from_slice(&[
            Span::styled("  \u{00b7}  ", theme::dim()),
            Span::styled("Freed: ", theme::dim()),
            Span::styled(format_size(app.freed_space), theme::success()),
        ]);
    }

    if app.permission_denied_count > 0 {
        spans.extend_from_slice(&[
            Span::styled("  \u{00b7}  ", theme::dim()),
            Span::styled(
                format!("{} access denied", app.permission_denied_count),
                theme::warning(),
            ),
        ]);
    }

    if app.scan_complete && app.scan_duration_ms > 0 {
        spans.extend_from_slice(&[
            Span::styled("  \u{00b7}  ", theme::dim()),
            Span::styled(
                format!("{:.1}s", app.scan_duration_ms as f64 / 1000.0),
                theme::dim(),
            ),
        ]);
    }

    // Pagination indicator
    if !app.filtered_indices.is_empty() {
        spans.extend_from_slice(&[
            Span::styled("  \u{00b7}  ", theme::dim()),
            Span::styled(
                format!("[{}/{}]", app.selected_index + 1, app.visible_count()),
                theme::dim(),
            ),
        ]);
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

// ── Progress bar ─────────────────────────────────────────────────

fn draw_progress(frame: &mut Frame, app: &App, area: Rect) {
    if app.scan_complete {
        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled("\u{2713} ", theme::success()),
            Span::styled("Exorcised ", theme::success()),
            Span::styled(
                format!("\u{2014} {} directories found", app.found_count()),
                theme::dim(),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    } else {
        let bar_width = area.width.saturating_sub(30) as usize;
        let tick = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 100) as usize;
        let anim_pos = tick % bar_width.max(1);

        let mut bar = String::with_capacity(bar_width);
        for i in 0..bar_width {
            if i < anim_pos.saturating_sub(2) {
                bar.push('\u{2588}'); // █
            } else if i < anim_pos {
                bar.push('\u{2593}'); // ▓
            } else {
                bar.push('\u{2591}'); // ░
            }
        }

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled("Scanning... ", theme::highlight()),
            Span::styled(bar, theme::dim()),
            Span::raw(" "),
            Span::styled(
                format!("{} dirs", app.dirs_scanned),
                theme::dim(),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
}

// ── Results list ─────────────────────────────────────────────────

fn draw_results(frame: &mut Frame, app: &App, area: Rect) {
    if app.filtered_indices.is_empty() {
        let msg = if app.scan_complete {
            "No directories found."
        } else {
            "Scanning..."
        };
        let para = Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(msg, theme::dim()),
        ]));
        frame.render_widget(para, area);
        return;
    }

    let visible_h = area.height as usize;
    let start = app.scroll_offset;
    let end = (start + visible_h).min(app.filtered_indices.len());

    let items: Vec<ListItem> = (start..end)
        .map(|vi| {
            let idx = app.filtered_indices[vi];
            let r = &app.results[idx];
            let is_selected = vi == app.selected_index;
            let is_checked = app.selected_paths.contains(&r.path);
            let is_pending = app.pending_delete.as_ref() == Some(&r.path);

            build_result_line(r, is_selected, is_checked, app.mode, area.width, is_pending)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, area);
}

fn build_result_line(
    r: &super::app::FolderResult,
    is_selected: bool,
    is_checked: bool,
    mode: AppMode,
    width: u16,
    is_pending_delete: bool,
) -> ListItem<'static> {
    let prefix = match mode {
        AppMode::MultiSelect | AppMode::RangeSelect => {
            if is_checked {
                Span::styled("  [\u{2713}] ", theme::highlight())
            } else {
                Span::styled("  [ ] ", theme::dim())
            }
        }
        _ => {
            if is_selected {
                Span::styled("  \u{2192} ", theme::highlight())
            } else {
                Span::raw("    ")
            }
        }
    };

    // Status indicators
    let (status_span, status_len) = if is_pending_delete {
        (Span::styled(" Delete? [y/Space]", Style::default().fg(theme::RED).add_modifier(Modifier::BOLD)), 18)
    } else {
        match r.status {
            FolderStatus::Deleted => (Span::styled(" \u{2713} deleted", theme::success()), 10),
            FolderStatus::Deleting => (Span::styled(format!(" {} deleting...", spinner_char()), theme::warning()), 14),
            FolderStatus::Error => (
                Span::styled(
                    format!(" \u{2717} {}", r.error_message.as_deref().unwrap_or("error")),
                    theme::error(),
                ),
                15,
            ),
            FolderStatus::Found => (Span::raw(""), 0),
        }
    };

    // Risk indicator
    let (risk_span, risk_len) = if r.is_dead {
        (Span::styled(" \u{2620}", theme::warning()), 2)
    } else {
        match r.risk_level {
            crate::risk::RiskLevel::High => (Span::styled(" \u{26a0}", theme::error()), 2),
            crate::risk::RiskLevel::Medium => (Span::styled(" \u{26a1}", theme::warning()), 2),
            crate::risk::RiskLevel::Low => (Span::raw(""), 0),
        }
    };

    // Size and age
    let size_str = if r.size_bytes > 0 {
        format_size(r.size_bytes)
    } else {
        "...".into()
    };
    let age_str = format_age(r.newest_ms);
    let meta = format!("  {age_str:>10}  {size_str:>10}");
    let meta_len = meta.len();

    // Path — truncate to available width
    let prefix_len = match mode {
        AppMode::MultiSelect | AppMode::RangeSelect => 6,
        _ => 4,
    };
    let avail = (width as usize)
        .saturating_sub(prefix_len)
        .saturating_sub(meta_len)
        .saturating_sub(status_len)
        .saturating_sub(risk_len);
    let path_str = r.path.to_string_lossy().to_string();
    let path_display = truncate_path(&path_str, avail);

    let path_style = match r.status {
        FolderStatus::Deleted => theme::dim(),
        FolderStatus::Error => theme::error(),
        _ if is_selected => theme::highlight(),
        _ => age_color(r.newest_ms),
    };

    let mut spans = vec![
        prefix,
        Span::styled(path_display, path_style),
        Span::styled(meta, theme::dim()),
    ];

    if risk_len > 0 {
        spans.push(risk_span);
    }
    if status_len > 0 {
        spans.push(status_span);
    }

    let line = Line::from(spans);
    let style = if is_selected {
        theme::selected_bg()
    } else {
        Style::default()
    };

    ListItem::new(line).style(style)
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len || max_len < 10 {
        return path.to_string();
    }
    let keep = max_len.saturating_sub(3); // space for "..."
    let head = keep / 2;
    let tail = keep - head;
    format!(
        "{}...{}",
        &path[..head],
        &path[path.len().saturating_sub(tail)..]
    )
}

// ── Status bar ───────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let sep = "\u{2500}".repeat(area.width as usize);
    let sep_line = Line::from(Span::styled(sep, theme::dim()));

    let hints = match app.mode {
        AppMode::MultiSelect | AppMode::RangeSelect => {
            let count = app.selected_paths.len();
            Line::from(vec![
                Span::raw("  "),
                Span::styled("\u{2191}\u{2193}", theme::highlight()),
                Span::raw(" Navigate  "),
                Span::styled("Space", theme::highlight()),
                Span::raw(" Toggle  "),
                Span::styled("Enter", theme::highlight()),
                Span::raw(format!(" Delete ({count})  ")),
                Span::styled("A", theme::highlight()),
                Span::raw(" All  "),
                Span::styled("V", theme::highlight()),
                Span::raw(" Range  "),
                Span::styled("Esc", theme::highlight()),
                Span::raw(" Cancel"),
            ])
        }
        AppMode::Search => Line::from(vec![
            Span::raw("  "),
            Span::styled("Type", theme::highlight()),
            Span::raw(" to filter  "),
            Span::styled("Enter", theme::highlight()),
            Span::raw(" confirm  "),
            Span::styled("Esc", theme::highlight()),
            Span::raw(" cancel"),
        ]),
        _ => Line::from(vec![
            Span::raw("  "),
            Span::styled("\u{2191}\u{2193}", theme::highlight()),
            Span::raw(" Navigate  "),
            Span::styled("Space", theme::highlight()),
            Span::raw(" Delete  "),
            Span::styled("T", theme::highlight()),
            Span::raw(" Multi  "),
            Span::styled("/", theme::highlight()),
            Span::raw(" Search  "),
            Span::styled("s", theme::highlight()),
            Span::raw(" Sort  "),
            Span::styled("o", theme::highlight()),
            Span::raw(" Open  "),
            Span::styled("?", theme::highlight()),
            Span::raw(" Help  "),
            Span::styled("q", theme::highlight()),
            Span::raw(" Quit"),
        ]),
    };

    let para = Paragraph::new(vec![sep_line, hints]);
    frame.render_widget(para, area);
}

// ── Search bar overlay ───────────────────────────────────────────

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let width = 60.min(area.width.saturating_sub(4)) as u16;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(4);

    let popup_area = Rect::new(x, y, width, 3);
    frame.render_widget(Clear, popup_area);

    let content = format!("/ {}\u{2588}", app.search_query);
    let matches = app.visible_count();
    let title = format!(" Search [{matches} matches] ");

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::highlight())
        .title(Span::styled(title, theme::highlight()));

    let para = Paragraph::new(content).block(block);
    frame.render_widget(para, popup_area);
}

// ── Error popup ──────────────────────────────────────────────────

fn draw_error_popup(frame: &mut Frame, app: &App, area: Rect) {
    let width = 70.min(area.width.saturating_sub(4));
    let height = 15.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;

    let popup_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup_area);

    let title = format!(" Errors ({}) ", app.errors.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::error())
        .title(Span::styled(title, theme::error()));

    let max_lines = height.saturating_sub(3) as usize;
    let start = app.errors.len().saturating_sub(max_lines);
    let inner_width = width.saturating_sub(4) as usize;

    let lines: Vec<Line> = app.errors[start..]
        .iter()
        .map(|e| {
            let display = if e.len() > inner_width {
                format!("{}...", &e[..inner_width.saturating_sub(3)])
            } else {
                e.clone()
            };
            Line::from(Span::styled(display, theme::error()))
        })
        .collect();

    let mut content = lines;
    content.push(Line::from(""));
    content.push(Line::from(Span::styled(
        "Press 'e' to close",
        theme::dim(),
    )));

    let para = Paragraph::new(content).block(block).wrap(Wrap { trim: false });
    frame.render_widget(para, popup_area);
}

// ── Help popup ───────────────────────────────────────────────────

fn draw_help_popup(frame: &mut Frame, area: Rect) {
    let width = 56.min(area.width.saturating_sub(4));
    let height = 20.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;

    let popup_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::highlight())
        .title(Span::styled(" Keybindings ", theme::highlight()));

    let bindings = vec![
        ("Up/Down, j/k", "Navigate"),
        ("Page Up/Down", "Page scroll"),
        ("Home/End", "Jump to start/end"),
        ("Space / Delete", "Delete directory"),
        ("T", "Toggle multi-select mode"),
        ("A", "Select/deselect all (multi)"),
        ("V", "Toggle range select"),
        ("Enter", "Confirm multi-select delete"),
        ("/", "Open search filter"),
        ("s", "Cycle sort mode"),
        ("o", "Open parent directory"),
        ("e", "Show/hide errors"),
        ("?", "Toggle this help"),
        ("Esc", "Cancel current mode"),
        ("q / Ctrl+C", "Quit"),
    ];

    let lines: Vec<Line> = bindings
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {key:<20}"), theme::highlight()),
                Span::raw(*desc),
            ])
        })
        .collect();

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, popup_area);
}
