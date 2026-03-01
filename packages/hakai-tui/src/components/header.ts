import type { AppState } from "../app";
import { BOLD, DIM, FG_GRAY, FG_RED, RESET } from "../constants/colors";

/**
 * Render the header lines.
 * Shows: app name, version, mode indicator, sort mode.
 */
export function renderHeader(state: AppState, width: number): string[] {
	const title = `${BOLD}${FG_RED}${state.title}${RESET} ${DIM}v${state.version}${RESET}    ${FG_GRAY}${state.subtitle}${RESET}`;

	let modeStr = "";
	if (state.mode === "multi-select") {
		modeStr = `  ${BOLD}[MULTI-SELECT: ${state.selectedPaths.size} selected]${RESET}`;
	} else if (state.mode === "search") {
		modeStr = `  ${BOLD}[SEARCH]${RESET}`;
	} else if (state.mode === "range-select") {
		modeStr = `  ${BOLD}[RANGE-SELECT]${RESET}`;
	}

	const sortStr = `  ${DIM}Sort: ${state.sortMode} ↓${RESET}`;

	const line = (title + modeStr + sortStr).slice(0, width + 100); // +100 for ANSI codes

	return [
		`  ${line}`,
		`  ${DIM}${italic(state.currentQuote)}${RESET}`,
		`${DIM}${'─'.repeat(width)}${RESET}`,
	];
}

function italic(text: string): string {
	return `\x1b[3m${text}\x1b[23m`;
}
