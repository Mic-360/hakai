use ratatui::style::{Color, Modifier, Style};

// ── Gojo Satoru color palette ────────────────────────────────────

pub const CYAN: Color = Color::Cyan;
pub const RED: Color = Color::Red;
pub const GREEN: Color = Color::Green;
pub const YELLOW: Color = Color::Yellow;
pub const DIM: Color = Color::DarkGray;
pub const WHITE: Color = Color::White;
pub const BG_HIGHLIGHT: Color = Color::Rgb(30, 30, 60);

// ── Quotes ───────────────────────────────────────────────────────

pub const QUOTES: &[&str] = &[
    "\"Throughout the filesystem, I alone am the honored one.\"",
    "\"Are you the strongest because you delete node_modules, or do you delete node_modules because you are the strongest?\"",
    "\"Don't worry. I'm the strongest disk cleaner.\"",
    "\"Nah, I'd clean.\"",
    "\"Standing at the top means deleting everything beneath you.\"",
    "\"This is Domain Expansion: Infinite Free Space.\"",
];

// ── Style helpers ────────────────────────────────────────────────

pub fn highlight() -> Style {
    Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
}

pub fn selected_bg() -> Style {
    Style::default().bg(BG_HIGHLIGHT)
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

pub fn success() -> Style {
    Style::default().fg(GREEN)
}

pub fn error() -> Style {
    Style::default().fg(RED)
}

pub fn warning() -> Style {
    Style::default().fg(YELLOW)
}

pub fn flash_success() -> Style {
    Style::default()
        .fg(GREEN)
        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
}
