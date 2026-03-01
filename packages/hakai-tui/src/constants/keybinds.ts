// Keybind mappings: raw stdin sequences → action names
export type KeyAction =
	| "UP"
	| "DOWN"
	| "PAGE_UP"
	| "PAGE_DOWN"
	| "HOME"
	| "END"
	| "BACKSPACE"
	| "ENTER"
	| "SPACE"
	| "ESCAPE"
	| "CTRL_C"
	| "CTRL_D"
	| "QUIT"
	| "TOGGLE_MULTI"
	| "SELECT_ALL"
	| "RANGE_SELECT"
	| "OPEN_DIR"
	| "SHOW_ERRORS"
	| "SEARCH"
	| "SORT"
	| "DELETE";

export const KEY_MAP: Record<string, KeyAction> = {
	"\x1b[A": "UP",
	"\x1b[B": "DOWN",
	"\x1b[5~": "PAGE_UP",
	"\x1b[6~": "PAGE_DOWN",
	"\x1b[H": "HOME",
	"\x1b[F": "END",
	"\x7f": "BACKSPACE",
	"\r": "ENTER",
	"\n": "ENTER",
	" ": "SPACE",
	"\x1b": "ESCAPE",
	"\x03": "CTRL_C",
	"\x04": "CTRL_D",
	// Vim-style navigation
	j: "DOWN",
	k: "UP",
	// Actions
	q: "QUIT",
	Q: "QUIT",
	T: "TOGGLE_MULTI",
	A: "SELECT_ALL",
	V: "RANGE_SELECT",
	o: "OPEN_DIR",
	e: "SHOW_ERRORS",
	"/": "SEARCH",
	s: "SORT",
	d: "DELETE",
};
