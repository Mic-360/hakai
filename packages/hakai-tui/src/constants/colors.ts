// ANSI color codes — no external dependency needed
export const RESET = "\x1b[0m";
export const BOLD = "\x1b[1m";
export const DIM = "\x1b[2m";
export const ITALIC = "\x1b[3m";
export const UNDERLINE = "\x1b[4m";

// Foreground colors
export const FG_BLACK = "\x1b[30m";
export const FG_RED = "\x1b[31m";
export const FG_GREEN = "\x1b[32m";
export const FG_YELLOW = "\x1b[33m";
export const FG_BLUE = "\x1b[34m";
export const FG_MAGENTA = "\x1b[35m";
export const FG_CYAN = "\x1b[36m";
export const FG_WHITE = "\x1b[37m";
export const FG_GRAY = "\x1b[90m";

// Background colors
export const BG_BLACK = "\x1b[40m";
export const BG_RED = "\x1b[41m";
export const BG_GREEN = "\x1b[42m";
export const BG_BLUE = "\x1b[44m";
export const BG_CYAN = "\x1b[46m";
export const BG_DARK = "\x1b[48;2;26;26;46m"; // #1a1a2e

// Highlight color mapping (user-configurable)
export const HIGHLIGHT_COLORS: Record<string, string> = {
	blue: FG_BLUE,
	cyan: FG_CYAN,
	magenta: FG_MAGENTA,
	white: FG_WHITE,
	red: FG_RED,
	yellow: FG_YELLOW,
	green: FG_GREEN,
};

export function getHighlightColor(name: string): string {
	return HIGHLIGHT_COLORS[name] ?? FG_CYAN;
}
