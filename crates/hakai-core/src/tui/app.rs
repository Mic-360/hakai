use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::risk::RiskLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    MultiSelect,
    RangeSelect,
    Search,
    Deleting,
    Help,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Path,
    Size,
    LastMod,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Path => "path",
            SortMode::Size => "size",
            SortMode::LastMod => "last-mod",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SortMode::Path => SortMode::Size,
            SortMode::Size => SortMode::LastMod,
            SortMode::LastMod => SortMode::Path,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderStatus {
    Found,
    Deleting,
    Deleted,
    Error,
}

#[derive(Debug, Clone)]
pub struct FolderResult {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub newest_ms: u64,
    pub is_dead: bool,
    pub risk_level: RiskLevel,
    pub status: FolderStatus,
    pub error_message: Option<String>,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub original_path: PathBuf,
    pub trash_path: PathBuf,
    pub size_bytes: u64,
    pub expires_ms: u64,
}

pub struct App {
    pub mode: AppMode,
    pub results: Vec<FolderResult>,
    pub result_map: HashMap<PathBuf, usize>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub selected_paths: HashSet<PathBuf>,
    pub range_start: Option<usize>,
    pub search_query: String,
    pub scan_complete: bool,
    pub total_size: u64,
    pub freed_space: u64,
    pub errors: Vec<String>,
    pub permission_denied_count: u64,
    pub show_errors: bool,
    pub scroll_offset: usize,
    pub sort_mode: SortMode,
    pub scan_duration_ms: u64,
    pub dirs_scanned: u64,
    pub current_quote: &'static str,
    pub should_quit: bool,
    pub dry_run: bool,
    pub min_size: Option<u64>,
    pub list_height: usize,
    pub pending_delete: Option<PathBuf>,
    pub preview_entries: Vec<(String, u64)>,
    pub undo_stack: Vec<UndoEntry>,
    pub freed_flash_until: u64,
    filter_dirty: bool,
    sort_dirty: bool,
}

impl App {
    pub fn new(sort_mode: SortMode, dry_run: bool, min_size: Option<u64>) -> Self {
        use super::theme::QUOTES;
        let quote_idx = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize
            % QUOTES.len();

        Self {
            mode: AppMode::Normal,
            results: Vec::new(),
            result_map: HashMap::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            selected_paths: HashSet::new(),
            range_start: None,
            search_query: String::new(),
            scan_complete: false,
            total_size: 0,
            freed_space: 0,
            errors: Vec::new(),
            permission_denied_count: 0,
            show_errors: false,
            scroll_offset: 0,
            sort_mode,
            scan_duration_ms: 0,
            dirs_scanned: 0,
            current_quote: QUOTES[quote_idx],
            should_quit: false,
            dry_run,
            min_size,
            list_height: 10,
            pending_delete: None,
            preview_entries: Vec::new(),
            undo_stack: Vec::new(),
            freed_flash_until: 0,
            filter_dirty: true,
            sort_dirty: false,
        }
    }


    pub fn add_result(&mut self, path: PathBuf) {
        let idx = self.results.len();
        self.result_map.insert(path.clone(), idx);
        self.results.push(FolderResult {
            path,
            size_bytes: 0,
            newest_ms: 0,
            is_dead: false,
            risk_level: RiskLevel::Low,
            status: FolderStatus::Found,
            error_message: None,
            project_name: None,
        });
        self.filter_dirty = true;
    }

    pub fn update_size(&mut self, path: &PathBuf, size: u64, newest_ms: u64) {
        if let Some(&idx) = self.result_map.get(path) {
            let old_size = self.results[idx].size_bytes;
            self.results[idx].size_bytes = size;
            self.results[idx].newest_ms = newest_ms;
            self.total_size = self.total_size - old_size + size;
            self.sort_dirty = true;
            self.filter_dirty = true;
        }
    }

    pub fn update_risk(&mut self, path: &PathBuf, is_dead: bool, risk_level: RiskLevel) {
        if let Some(&idx) = self.result_map.get(path) {
            self.results[idx].is_dead = is_dead;
            self.results[idx].risk_level = risk_level;
        }
    }

    pub fn mark_deleted(&mut self, path: &PathBuf, freed_bytes: u64) {
        if let Some(&idx) = self.result_map.get(path) {
            self.results[idx].status = FolderStatus::Deleted;
            self.freed_space += freed_bytes;
            self.filter_dirty = true;
        }
    }

    pub fn mark_error(&mut self, path: &PathBuf, message: String) {
        if let Some(&idx) = self.result_map.get(path) {
            self.results[idx].status = FolderStatus::Error;
            self.results[idx].error_message = Some(message);
            self.filter_dirty = true;
        }
    }


    pub fn rebuild_filter_if_dirty(&mut self) {
        if !self.filter_dirty {
            return;
        }
        self.filter_dirty = false;

        if self.sort_dirty {
            self.sort_dirty = false;
            match self.sort_mode {
                SortMode::Size => self.results.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
                SortMode::LastMod => self.results.sort_by(|a, b| b.newest_ms.cmp(&a.newest_ms)),
                SortMode::Path => self.results.sort_by(|a, b| a.path.cmp(&b.path)),
            }
            self.result_map.clear();
            for (i, r) in self.results.iter().enumerate() {
                self.result_map.insert(r.path.clone(), i);
            }
        }

        self.filtered_indices.clear();
        let query = &self.search_query;
        let min_size = self.min_size.unwrap_or(0);

        for (i, r) in self.results.iter().enumerate() {
            if min_size > 0 && r.size_bytes < min_size && r.size_bytes > 0 {
                continue;
            }
            if !query.is_empty() {
                let path_str = r.path.to_string_lossy();
                let path_lower = path_str.to_lowercase();
                let query_lower = query.to_lowercase();
                if !path_lower.contains(&query_lower) {
                    continue;
                }
            }
            self.filtered_indices.push(i);
        }

        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
        self.ensure_visible();
    }

    pub fn cycle_sort(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.sort_dirty = true;
        self.filter_dirty = true;
    }


    pub fn move_selection(&mut self, delta: i32) {
        self.pending_delete = None;
        let len = self.filtered_indices.len();
        if len == 0 {
            return;
        }
        let new_idx = self.selected_index as i32 + delta;
        self.selected_index = new_idx.clamp(0, len as i32 - 1) as usize;

        if self.mode == AppMode::RangeSelect {
            self.update_range_selection();
        }
        self.ensure_visible();
    }

    pub fn go_home(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        if self.mode == AppMode::RangeSelect {
            self.update_range_selection();
        }
    }

    pub fn go_end(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = self.filtered_indices.len() - 1;
            self.ensure_visible();
        }
        if self.mode == AppMode::RangeSelect {
            self.update_range_selection();
        }
    }

    fn ensure_visible(&mut self) {
        let h = self.list_height;
        if h == 0 {
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + h {
            self.scroll_offset = self.selected_index - h + 1;
        }
    }


    pub fn handle_space_or_delete(&mut self) -> Option<(PathBuf, u64)> {
        if self.filtered_indices.is_empty() {
            return None;
        }

        if self.mode == AppMode::MultiSelect || self.mode == AppMode::RangeSelect {
            let idx = self.filtered_indices[self.selected_index];
            let path = self.results[idx].path.clone();
            if self.selected_paths.contains(&path) {
                self.selected_paths.remove(&path);
            } else {
                self.selected_paths.insert(path);
            }
            None
        } else {
            let idx = self.filtered_indices[self.selected_index];
            let r = &self.results[idx];
            if r.status != FolderStatus::Found {
                return None;
            }
            let path = r.path.clone();
            self.pending_delete = Some(path);
            None
        }
    }

    pub fn confirm_pending_delete(&mut self) -> Option<(PathBuf, u64)> {
        let pending = self.pending_delete.take()?;
        let &idx = self.result_map.get(&pending)?;
        if self.results[idx].status == FolderStatus::Found {
            let size = self.results[idx].size_bytes;
            self.results[idx].status = FolderStatus::Deleting;
            self.filter_dirty = true;
            Some((pending, size))
        } else {
            None
        }
    }

    pub fn handle_enter(&mut self) -> Vec<(PathBuf, u64)> {
        if self.mode == AppMode::Search {
            self.mode = AppMode::Normal;
            return Vec::new();
        }

        if (self.mode == AppMode::MultiSelect || self.mode == AppMode::RangeSelect)
            && !self.selected_paths.is_empty()
        {
            let mut items = Vec::new();
            for path in self.selected_paths.drain() {
                if let Some(&idx) = self.result_map.get(&path) {
                    if self.results[idx].status == FolderStatus::Found {
                        self.results[idx].status = FolderStatus::Deleting;
                        items.push((path, self.results[idx].size_bytes));
                    }
                }
            }
            if !items.is_empty() {
                self.mode = AppMode::Deleting;
                self.filter_dirty = true;
            }
            items
        } else {
            Vec::new()
        }
    }

    pub fn toggle_multi_select(&mut self) {
        if self.mode == AppMode::MultiSelect {
            self.mode = AppMode::Normal;
            self.selected_paths.clear();
            self.range_start = None;
        } else {
            self.mode = AppMode::MultiSelect;
        }
    }

    pub fn select_all(&mut self) {
        if self.mode != AppMode::MultiSelect && self.mode != AppMode::RangeSelect {
            return;
        }
        if self.selected_paths.len() == self.filtered_indices.len() {
            self.selected_paths.clear();
        } else {
            self.selected_paths.clear();
            for &idx in &self.filtered_indices {
                if self.results[idx].status == FolderStatus::Found {
                    self.selected_paths.insert(self.results[idx].path.clone());
                }
            }
        }
    }

    pub fn toggle_range_select(&mut self) {
        if self.mode == AppMode::RangeSelect {
            self.mode = AppMode::MultiSelect;
            self.range_start = None;
        } else {
            self.mode = AppMode::RangeSelect;
            self.range_start = Some(self.selected_index);
        }
    }

    fn update_range_selection(&mut self) {
        let start = match self.range_start {
            Some(s) => s,
            None => return,
        };
        let end = self.selected_index;
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };

        self.selected_paths.clear();
        for i in lo..=hi {
            if i < self.filtered_indices.len() {
                let idx = self.filtered_indices[i];
                if self.results[idx].status == FolderStatus::Found {
                    self.selected_paths
                        .insert(self.results[idx].path.clone());
                }
            }
        }
    }


    pub fn enter_search(&mut self) {
        self.mode = AppMode::Search;
        self.search_query.clear();
        self.filter_dirty = true;
    }

    pub fn search_push_char(&mut self, ch: char) {
        self.search_query.push(ch);
        self.filter_dirty = true;
    }

    pub fn search_pop_char(&mut self) {
        self.search_query.pop();
        self.filter_dirty = true;
    }

    pub fn exit_search(&mut self) {
        self.mode = AppMode::Normal;
        self.search_query.clear();
        self.filter_dirty = true;
    }


    pub fn handle_escape(&mut self) {
        match self.mode {
            AppMode::Search => self.exit_search(),
            AppMode::Help => self.mode = AppMode::Normal,
            AppMode::MultiSelect | AppMode::RangeSelect => {
                self.mode = AppMode::Normal;
                self.selected_paths.clear();
                self.range_start = None;
            }
            _ => {}
        }
    }


    pub fn get_selected_result(&self) -> Option<&FolderResult> {
        self.filtered_indices
            .get(self.selected_index)
            .map(|&idx| &self.results[idx])
    }

    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn found_count(&self) -> usize {
        self.results.len()
    }

    pub fn max_result_size(&self) -> u64 {
        self.results.iter().map(|r| r.size_bytes).max().unwrap_or(1)
    }

    pub fn update_project_name(&mut self, path: &PathBuf, name: Option<String>) {
        if let Some(&idx) = self.result_map.get(path) {
            self.results[idx].project_name = name;
        }
    }

    pub fn push_undo(&mut self, original_path: PathBuf, trash_path: PathBuf, size_bytes: u64) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.undo_stack.push(UndoEntry {
            original_path,
            trash_path,
            size_bytes,
            expires_ms: now_ms + 30_000,
        });
    }

    pub fn flash_freed(&mut self) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.freed_flash_until = now_ms + 2000;
    }

    pub fn is_freed_flashing(&self) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now_ms < self.freed_flash_until
    }

    pub fn cleanup_expired_undo(&mut self) -> Vec<PathBuf> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let mut expired = Vec::new();
        self.undo_stack.retain(|entry| {
            if now_ms >= entry.expires_ms {
                expired.push(entry.trash_path.clone());
                false
            } else {
                true
            }
        });
        expired
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn restore_from_undo(&mut self, original_path: &PathBuf, size_bytes: u64) {
        if let Some(&idx) = self.result_map.get(original_path) {
            self.results[idx].status = FolderStatus::Found;
            self.freed_space = self.freed_space.saturating_sub(size_bytes);
            self.filter_dirty = true;
        }
    }
}


pub fn format_age(newest_ms: u64) -> String {
    if newest_ms == 0 {
        return "...".into();
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    if newest_ms > now_ms {
        return "just now".into();
    }
    let diff_s = (now_ms - newest_ms) / 1000;
    if diff_s < 60 {
        "just now".into()
    } else if diff_s < 3600 {
        format!("{}m ago", diff_s / 60)
    } else if diff_s < 86400 {
        format!("{}h ago", diff_s / 3600)
    } else if diff_s < 2_592_000 {
        let days = diff_s / 86400;
        if days == 1 {
            "1 day".into()
        } else {
            format!("{days} days")
        }
    } else if diff_s < 31_536_000 {
        let months = diff_s / 2_592_000;
        if months == 1 {
            "1 month".into()
        } else {
            format!("{months} months")
        }
    } else {
        let years = diff_s / 31_536_000;
        if years == 1 {
            "1 year".into()
        } else {
            format!("{years} years")
        }
    }
}
