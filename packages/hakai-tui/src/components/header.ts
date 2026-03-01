import type { AppState } from "../app";
import { APP_SUBTITLE, APP_TITLE, VERSION } from "../constants/cli";
import { BOLD, DIM, FG_GRAY, FG_RED, RESET } from "../constants/colors";

/**
 * Render the header lines.
 * Shows: app name, version, mode indicator, sort mode.
 */
export function renderHeader(state: AppState, width: number): string[] {
	const title = `${BOLD}${FG_RED}${APP_TITLE}${RESET} ${DIM}v${VERSION}${RESET}    ${FG_GRAY}${APP_SUBTITLE}${RESET}`;

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
		`${DIM}${'─'.repeat(width)}${RESET}`,
	];
}
