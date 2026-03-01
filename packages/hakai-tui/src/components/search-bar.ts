import type { AppState } from "../app";
import { BOLD, FG_CYAN, RESET } from "../constants/colors";

/**
 * Render the search bar overlay (shown when in search mode).
 */
export function renderSearchBar(state: AppState, width: number): string {
	if (state.mode !== "search") return "";

	const barWidth = Math.min(width - 4, 60);
	const top = `  ┌${"─".repeat(barWidth)}┐`;
	const query = state.searchQuery.slice(0, barWidth - 14);
	const content = `  │ ${FG_CYAN}/${RESET} Filter: ${BOLD}${query}${RESET}█${" ".repeat(Math.max(0, barWidth - query.length - 12))}│`;
	const bottom = `  └${"─".repeat(barWidth)}┘`;

	return `${top}\n${content}\n${bottom}`;
}
