import type { AppState } from "../app";
import { BOLD, DIM, FG_CYAN, FG_YELLOW, RESET } from "../constants/colors";

/**
 * Render the bottom status bar showing key hints.
 */
export function renderStatusBar(state: AppState, width: number): string[] {
	const lines: string[] = [];

	// Search bar (if in search mode)
	if (state.mode === "search") {
		const prompt = `  ${FG_YELLOW}/${RESET} Filter: ${BOLD}${state.searchQuery}${RESET}█`;
		lines.push(prompt);
	}

	// Separator
	lines.push(`${DIM}${"─".repeat(width)}${RESET}`);

	// Key hints with consistent spacing
	if (state.mode === "multi-select" || state.mode === "range-select") {
		lines.push(
			`  ${FG_CYAN}↑↓${RESET}/jk navigate    ${FG_CYAN}Space${RESET} toggle    ${FG_CYAN}Enter${RESET} delete selected    ${FG_CYAN}A${RESET} select all    ${FG_CYAN}T${RESET} exit multi`,
		);
	} else {
		lines.push(
			`  ${FG_CYAN}↑↓${RESET}/jk navigate    ${FG_CYAN}Space${RESET}/Del delete    ${FG_CYAN}T${RESET} multi-select    ${FG_CYAN}/${RESET} search    ${FG_CYAN}s${RESET} sort    ${FG_CYAN}o${RESET} open    ${FG_CYAN}e${RESET} errors    ${FG_CYAN}q${RESET} quit`,
		);
	}

	return lines;
}
