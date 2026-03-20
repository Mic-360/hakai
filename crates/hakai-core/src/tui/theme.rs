use ratatui::style::{Color, Modifier, Style};

// ── Material Sage Green palette ────────────────────────────────
//
// Dark theme built on sage green as the primary accent.
// Colors follow Material Design 3 tonal principles with natural,
// warm-shifted tones for a cohesive organic aesthetic.

// Primary
pub const CYAN: Color = Color::Rgb(134, 166, 141);            // #86A68D sage green
pub const RED: Color = Color::Rgb(198, 105, 105);              // #C66969 muted coral
pub const GREEN: Color = Color::Rgb(129, 189, 139);            // #81BD8B soft sage
pub const YELLOW: Color = Color::Rgb(214, 179, 107);           // #D6B36B warm amber
pub const DIM: Color = Color::Rgb(90, 110, 98);                // #5A6E62 sage gray
pub const WHITE: Color = Color::Rgb(200, 214, 205);            // #C8D6CD sage white
pub const BG_HIGHLIGHT: Color = Color::Rgb(35, 50, 40);        // #233228 selection bg

// Extended palette
pub const BORDER: Color = Color::Rgb(65, 85, 72);              // #415548 borders/separators
pub const AGE_OLD: Color = Color::Rgb(70, 85, 75);             // #46554B dim (>365 days)
pub const AGE_MID: Color = Color::Rgb(115, 135, 122);          // #73877A mid (180-365 days)
pub const SURFACE: Color = Color::Rgb(25, 32, 28);             // #19201C popup surface

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

pub fn border() -> Style {
    Style::default().fg(BORDER)
}

pub fn popup_block() -> Style {
    Style::default().bg(SURFACE)
}
