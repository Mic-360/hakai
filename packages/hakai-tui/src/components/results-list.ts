import type { AppState, FolderResult } from "../app";
import {
	BG_DARK,
	BOLD,
	DIM,
	FG_CYAN,
	FG_GRAY,
	FG_GREEN,
	FG_RED,
	FG_YELLOW,
	RESET
} from "../constants/colors";
import { formatAge, formatSize } from "./stats-bar";

/**
 * Render the scrollable results list.
 * Returns an array of lines to render.
 */
export function renderResultsList(state: AppState, width: number, height: number): string[] {
	const lines: string[] = [];
	const results = state.filteredResults;

	if (results.length === 0) {
		if (state.scanComplete) {
			lines.push("");
			lines.push(`  ${DIM}No directories found. My Six Eyes see everything, and there are no curses here.${RESET}`);
		} else {
			lines.push("");
			lines.push(`  ${DIM}Scanning... Domain Expansion: Infinite Scan!${RESET}`);
		}
		while (lines.length < height) {
			lines.push("");
		}
		return lines;
	}

	// Column header
	const multiMode = state.mode === "multi-select" || state.mode === "range-select";
	const prefix = multiMode ? "      " : "    ";
	const headerPath = "PATH";
	const headerAge = "AGE".padStart(8);
	const headerSize = "SIZE".padStart(9);
	lines.push(`${DIM}${prefix}${headerPath}${'  '.padEnd(Math.max(1, width - prefix.length - headerPath.length - 24))}${headerAge}  ${headerSize}${RESET}`);
	lines.push(`${DIM}${'─'.repeat(width)}${RESET}`);

	const listHeight = height - 2; // Minus header row and separator

	// Determine visible range based on scroll offset
	const visibleCount = listHeight;
	const start = state.scrollOffset;
	const end = Math.min(start + visibleCount, results.length);

	for (let i = start; i < end; i++) {
		const result = results[i];
		const isSelected = i === state.selectedIndex;
		const isChecked = state.selectedPaths.has(result.path);
		lines.push(renderRow(result, isSelected, isChecked, width, multiMode));
	}

	// Scrollbar indicator
	if (results.length > visibleCount && end < results.length) {
		lines.push(`${DIM}  ↓ ${results.length - end} more${RESET}`);
	} else {
		lines.push("");
	}

	// Pad remaining height
	while (lines.length < height) {
		lines.push("");
	}

	return lines;
}

function renderRow(
	result: FolderResult,
	isSelected: boolean,
	isChecked: boolean,
	width: number,
	multiSelectMode: boolean,
): string {
	// Risk indicators
	const risk =
		result.riskLevel === "high"
			? ` ${FG_RED}⚠${RESET}`
			: result.riskLevel === "medium"
				? ` ${FG_YELLOW}⚡${RESET}`
				: result.isDead
					? ` ${FG_YELLOW}☠${RESET}`
					: "";

	const age = result.newestMs > 0 ? formatAge(result.newestMs) : "—";
	const size = result.sizeBytes > 0 ? formatSize(result.sizeBytes) : "   ...  ";

	// Status column
	let status = "";
	if (result.status === "deleting") {
		status = ` ${FG_YELLOW}⟳ deleting...${RESET}`;
	} else if (result.status === "deleted") {
		status = ` ${FG_GREEN}✓ deleted${RESET}`;
	} else if (result.status === "error") {
		status = ` ${FG_RED}✗ ${result.errorMessage ?? "error"}${RESET}`;
	}

	// Checkbox prefix (only in multi-select mode)
	const checkbox = multiSelectMode ? (isChecked ? `${FG_GREEN}☑ ${RESET}` : `${FG_GRAY}☐ ${RESET}`) : "  ";
	const arrow = isSelected ? `${FG_CYAN}▶ ${RESET}` : "  ";

	// Fixed-width columns for alignment: age (10), size (11), risk (3), status (variable)
	const metaWidth = 28 + (status.length > 0 ? 20 : 0);
	const pathWidth = Math.max(20, width - metaWidth);
	const displayPath = truncatePath(result.path, pathWidth);

	const sizeStr = `${BOLD}${size.padStart(9)}${RESET}`;
	const ageStr = `${DIM}${age.padStart(8)}${RESET}`;

	const line = `${checkbox}${arrow}${displayPath}  ${ageStr}  ${sizeStr}${risk}${status}`;

	if (isSelected) {
		return `${BG_DARK}${line}${RESET}`;
	}

	if (result.status === "deleted") {
		return `${DIM}${line}${RESET}`;
	}

	return line;
}

function truncatePath(path: string, maxWidth: number): string {
	if (path.length <= maxWidth) {
		return path.padEnd(maxWidth);
	}
	// Show start...end of path
	const ellipsis = "...";
	const keepEnd = Math.floor(maxWidth * 0.6);
	const keepStart = maxWidth - keepEnd - ellipsis.length;
	if (keepStart <= 0) {
		return path.slice(-maxWidth).padEnd(maxWidth);
	}
	return (path.slice(0, keepStart) + ellipsis + path.slice(-keepEnd)).padEnd(maxWidth);
}
