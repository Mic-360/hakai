import type { AppState } from "../app";
import { BOLD, DIM, FG_CYAN, FG_GREEN, FG_YELLOW, RESET } from "../constants/colors";

/**
 * Render the stats bar showing scan summary.
 */
export function renderStatsBar(state: AppState, width: number): string[] {
	const foundCount = state.filteredResults.length;
	const totalCount = state.results.length;

	const foundStr = `${BOLD}Found:${RESET} ${FG_CYAN}${totalCount} dirs${RESET}`;
	const totalSizeStr = `${BOLD}Total:${RESET} ${FG_YELLOW}${formatSize(state.totalSize)}${RESET}`;
	const freedStr = `${BOLD}Freed:${RESET} ${FG_GREEN}${formatSize(state.freedSpace)}${RESET}`;

	let parts = `  ${foundStr}    ${totalSizeStr}    ${freedStr}`;

	if (state.scanComplete && state.scanDurationMs > 0) {
		const scanTime = (state.scanDurationMs / 1000).toFixed(1);
		parts += `    ${DIM}Scan: ${scanTime}s${RESET}`;
	}

	if (foundCount !== totalCount) {
		parts += `  ${DIM}(showing ${foundCount} of ${totalCount})${RESET}`;
	}

	return [parts, ""];
}

export function formatSize(bytes: number): string {
	if (bytes >= 1_073_741_824) {
		return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
	} else if (bytes >= 1_048_576) {
		return `${(bytes / 1_048_576).toFixed(1)} MB`;
	} else if (bytes >= 1024) {
		return `${(bytes / 1024).toFixed(1)} KB`;
	} else if (bytes > 0) {
		return `${bytes} B`;
	}
	return "—";
}

export function formatAge(timestampMs: number): string {
	if (timestampMs === 0) return "—";
	const now = Date.now();
	const diffMs = now - timestampMs;
	const seconds = Math.floor(diffMs / 1000);
	const minutes = Math.floor(seconds / 60);
	const hours = Math.floor(minutes / 60);
	const days = Math.floor(hours / 24);
	const months = Math.floor(days / 30);
	const years = Math.floor(days / 365);

	if (years > 0) return `${years}y ago`;
	if (months > 0) return `${months}mo ago`;
	if (days > 0) return `${days}d ago`;
	if (hours > 0) return `${hours}h ago`;
	if (minutes > 0) return `${minutes}m ago`;
	return "just now";
}
