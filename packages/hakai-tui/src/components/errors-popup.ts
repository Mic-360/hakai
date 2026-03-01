import type { AppState } from "../app";
import { BG_RED, BOLD, DIM, FG_RED, FG_WHITE, FG_YELLOW, RESET } from "../constants/colors";

/**
 * Render the errors popup overlay.
 * Shows the last N errors in a bordered box.
 */
export function renderErrorsPopup(state: AppState, width: number, height: number): string[] {
	if (!state.showErrors || state.errors.length === 0) return [];

	const boxWidth = Math.min(width - 4, 80);
	const maxLines = Math.min(height - 6, 20);
	const lines: string[] = [];

	lines.push(`  ${FG_RED}┌${"─".repeat(boxWidth - 2)}┐${RESET}`);
	lines.push(`  ${FG_RED}│${RESET} ${BG_RED}${FG_WHITE}${BOLD} ERRORS (${state.errors.length}) ${RESET}${" ".repeat(Math.max(0, boxWidth - 18 - String(state.errors.length).length))}${FG_RED}│${RESET}`);
	lines.push(`  ${FG_RED}│${RESET}${" ".repeat(boxWidth - 2)}${FG_RED}│${RESET}`);

	const errorsToShow = state.errors.slice(-maxLines);
	for (const err of errorsToShow) {
		const truncated = err.slice(0, boxWidth - 6);
		const padding = " ".repeat(Math.max(0, boxWidth - truncated.length - 4));
		lines.push(`  ${FG_RED}│${RESET} ${FG_YELLOW}${truncated}${RESET}${padding}${FG_RED}│${RESET}`);
	}

	lines.push(`  ${FG_RED}│${RESET}${" ".repeat(boxWidth - 2)}${FG_RED}│${RESET}`);
	lines.push(`  ${FG_RED}│${RESET} ${DIM}Press 'e' to close${RESET}${" ".repeat(Math.max(0, boxWidth - 21))}${FG_RED}│${RESET}`);
	lines.push(`  ${FG_RED}└${"─".repeat(boxWidth - 2)}┘${RESET}`);

	return lines;
}
