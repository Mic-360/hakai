use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{format_age, App, AppMode, FolderStatus};
use crate::util::format_size;
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
        Style::default().fg(theme::AGE_OLD)
    } else if age_days > 180 {
        Style::default().fg(theme::AGE_MID)
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
        Constraint::Length(1), // spacer between progress and results
        Constraint::Min(3),   // results list
        Constraint::Length(2), // status bar
    ])
    .split(area);

    app.list_height = chunks[4].height.saturating_sub(0) as usize;

    draw_header(frame, app, chunks[0]);
    draw_stats(frame, app, chunks[1]);
    draw_progress(frame, app, chunks[2]);
    // chunks[3] = spacer (empty row for breathing room)
    draw_results(frame, app, chunks[4]);
    draw_status_bar(frame, app, chunks[5]);

    if app.show_errors && !app.errors.is_empty() {
        draw_error_popup(frame, app, area);
    }
    if app.mode == AppMode::Help {
        draw_help_popup(frame, area);
    }
    if app.mode == AppMode::Search {
        draw_search_bar(frame, app, area);
    }
    if app.mode == AppMode::Preview {
        draw_preview_popup(frame, app, area);
    }
}

// ── Header ───────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title_line = Line::from(vec![
        Span::styled("  \u{1f980} Hakai", theme::highlight()),
        Span::styled(" v1.0.0", theme::dim()),
        Span::raw("  "),
        Span::styled(
            format!(
                "Sort: {} {}",
                app.sort_mode.label(),
                match app.sort_mode {
                    super::app::SortMode::Path => "\u{25b2}",
                    _ => "\u{25bc}",
                }
            ),
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
    let sep_line = Line::from(Span::styled(sep, theme::border()));

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
        let freed_style = if app.is_freed_flashing() {
            theme::flash_success()
        } else {
            theme::success()
        };
        spans.extend_from_slice(&[
            Span::styled("  \u{00b7}  ", theme::dim()),
            Span::styled("Freed: ", theme::dim()),
            Span::styled(format_size(app.freed_space), freed_style),
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
        let spin = spinner_char();
        let bar_width = area.width.saturating_sub(50) as usize;
        let tick = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 100) as usize;
        let anim_pos = tick % bar_width.max(1);

        let mut bar = String::with_capacity(bar_width);
        for i in 0..bar_width {
            if i < anim_pos.saturating_sub(2) {
                bar.push('\u{2588}');
            } else if i < anim_pos {
                bar.push('\u{2593}');
            } else {
                bar.push('\u{2591}');
            }
        }

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{spin} Scanning... "), theme::highlight()),
            Span::styled(format!("Found: {} ", app.found_count()), theme::success()),
            Span::styled(bar, theme::dim()),
            Span::styled(
                format!("  {} dirs scanned", app.dirs_scanned),
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

    let search_query = &app.search_query;

    let items: Vec<ListItem> = (start..end)
        .map(|vi| {
            let idx = app.filtered_indices[vi];
            let r = &app.results[idx];
            let is_selected = vi == app.selected_index;
            let is_checked = app.selected_paths.contains(&r.path);
            let is_pending = app.pending_delete.as_ref() == Some(&r.path);

            build_result_line(
                r,
                is_selected,
                is_checked,
                app.mode,
                area.width,
                is_pending,
                search_query,
            )
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
    search_query: &str,
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
                Span::styled("  \u{25b6} ", theme::highlight())
            } else {
                Span::raw("    ")
            }
        }
    };

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

    let (risk_span, risk_len) = if r.is_dead {
        (Span::styled(" \u{2620}", theme::warning()), 2)
    } else {
        match r.risk_level {
            crate::risk::RiskLevel::High => (Span::styled(" \u{26a0}", theme::error()), 2),
            crate::risk::RiskLevel::Medium => (Span::styled(" \u{26a1}", theme::warning()), 2),
            crate::risk::RiskLevel::Low => (Span::raw(""), 0),
        }
    };

    // ── Fixed-width 4-column layout ──
    // [prefix][path (fill)][project (max 30)][age (12)][size (10)][risk][status][pad]
    const PROJECT_COL_MAX: usize = 30;
    const RIGHT_PAD: usize = 2; // prevent clipping at terminal right edge

    let size_str = if r.size_bytes > 0 { format_size(r.size_bytes) } else { "...".into() };
    let age_str = format_age(r.newest_ms);

    // Right-side fixed columns
    let age_col = format!("{age_str:>12}");
    let size_col = format!("{size_str:>10}");
    let right_w = 12 + 10 + risk_len + status_len + RIGHT_PAD;

    // Project tag with truncation
    let project_tag = r.project_name.as_deref().unwrap_or("");
    let project_display = if project_tag.is_empty() {
        String::new()
    } else {
        let max_tag = PROJECT_COL_MAX.saturating_sub(3);
        if project_tag.len() > max_tag {
            format!(" ({}...)", &project_tag[..max_tag.saturating_sub(3)])
        } else {
            format!(" ({})", project_tag)
        }
    };

    // Path fills remaining width → age+size always at fixed position from right
    let prefix_len: usize = match mode {
        AppMode::MultiSelect | AppMode::RangeSelect => 6,
        _ => 4,
    };
    let left_w = (width as usize).saturating_sub(prefix_len).saturating_sub(right_w);
    let path_avail = left_w.saturating_sub(project_display.len());
    let path_str = r.path.to_string_lossy().to_string();
    let path_display = truncate_path(&path_str, path_avail);
    let pad_len = path_avail.saturating_sub(path_display.len());

    let path_style = match r.status {
        FolderStatus::Deleted => theme::dim(),
        FolderStatus::Error => theme::error(),
        _ if is_selected => theme::highlight(),
        _ => age_color(r.newest_ms),
    };

    let mut spans = vec![prefix];

    // Path with optional search highlighting
    if !search_query.is_empty() && r.status != FolderStatus::Deleted {
        let path_lower = path_display.to_lowercase();
        let query_lower = search_query.to_lowercase();
        if let Some(pos) = path_lower.find(&query_lower) {
            let end = pos + search_query.len();
            let before = &path_display[..pos];
            let matched = &path_display[pos..end.min(path_display.len())];
            let after = &path_display[end.min(path_display.len())..];
            spans.push(Span::styled(before.to_string(), path_style));
            spans.push(Span::styled(
                matched.to_string(),
                Style::default().fg(theme::YELLOW).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
            spans.push(Span::styled(after.to_string(), path_style));
        } else {
            spans.push(Span::styled(path_display, path_style));
        }
    } else {
        spans.push(Span::styled(path_display, path_style));
    }

    // Padding to align fixed columns
    if pad_len > 0 {
        spans.push(Span::raw(" ".repeat(pad_len)));
    }

    // Project tag column
    if !project_display.is_empty() {
        spans.push(Span::styled(project_display, theme::dim()));
    }

    // Fixed-width age and size columns (right-aligned)
    spans.push(Span::styled(age_col, theme::dim()));
    spans.push(Span::styled(size_col, theme::dim()));

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
    let sep_line = Line::from(Span::styled(sep, theme::border()));

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
        AppMode::Deleting => Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{} ", spinner_char()), theme::warning()),
            Span::styled("Deleting... ", theme::warning()),
            Span::raw("please wait"),
        ]),
        AppMode::Preview => Line::from(vec![
            Span::raw("  "),
            Span::styled("Esc", theme::highlight()),
            Span::raw("/"),
            Span::styled("p", theme::highlight()),
            Span::raw(" Close preview"),
        ]),
        _ => {
            let mut hint_spans = vec![
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
                Span::styled("p", theme::highlight()),
                Span::raw(" Preview  "),
                Span::styled("o", theme::highlight()),
                Span::raw(" Open  "),
            ];
            if app.undo_count() > 0 {
                hint_spans.push(Span::styled("u", theme::highlight()));
                hint_spans.push(Span::raw(" Undo  "));
            }
            hint_spans.push(Span::styled("?", theme::highlight()));
            hint_spans.push(Span::raw(" Help  "));
            hint_spans.push(Span::styled("q", theme::highlight()));
            hint_spans.push(Span::raw(" Quit"));
            Line::from(hint_spans)
        }
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
        .title(Span::styled(title, theme::highlight()))
        .style(theme::popup_block());

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
        .title(Span::styled(title, theme::error()))
        .style(theme::popup_block());

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

fn draw_preview_popup(frame: &mut Frame, app: &App, area: Rect) {
    let width = 74.min(area.width.saturating_sub(4));
    let height = 20.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;

    let popup_area = Rect::new(x, y, width, height);
    frame.render_widget(Clear, popup_area);

    let selected = app.get_selected_result();
    let title = if let Some(r) = selected {
        format!(
            " {} ",
            r.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Preview")
        )
    } else {
        " Preview ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::highlight())
        .title(Span::styled(title, theme::highlight()))
        .style(theme::popup_block());

    let inner_width = width.saturating_sub(4) as usize;

    let mut lines: Vec<Line> = if app.preview_entries.is_empty() {
        vec![Line::from(Span::styled(
            format!("  {} Loading...", spinner_char()),
            theme::dim(),
        ))]
    } else {
        app.preview_entries
            .iter()
            .map(|(name, size)| {
                let name_width = inner_width.saturating_sub(12);
                let display_name = if name.len() > name_width {
                    format!("{}...", &name[..name_width.saturating_sub(3)])
                } else {
                    name.clone()
                };
                Line::from(vec![
                    Span::styled(
                        format!("  {display_name:<width$}", width = name_width),
                        Style::default().fg(theme::WHITE),
                    ),
                    Span::styled(format!("{:>10}", format_size(*size)), theme::dim()),
                ])
            })
            .collect()
    };

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press 'p' or Esc to close",
        theme::dim(),
    )));

    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, popup_area);
}

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
        .title(Span::styled(" Keybindings ", theme::highlight()))
        .style(theme::popup_block());

    let bindings = vec![
        ("Up/Down, j/k", "Navigate"),
        ("Page Up/Down", "Page scroll"),
        ("Home/End", "Jump to start/end"),
        ("Space / Delete", "Delete directory"),
        ("T", "Toggle multi-select mode"),
        ("A", "Select/deselect all (multi)"),
        ("V", "Toggle range select"),
        ("Enter", "Confirm multi-select delete"),
        ("u", "Undo last delete (30s)"),
        ("/", "Open search filter"),
        ("s", "Cycle sort mode"),
        ("p", "Preview directory contents"),
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
