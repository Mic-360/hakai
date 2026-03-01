import type { AppState } from "../app";
import { BOLD, DIM, FG_GRAY, FG_GREEN, FG_YELLOW, RESET } from "../constants/colors";

/**
 * Render a tri-color progress bar for the scan.
 */
export function renderProgressBar(state: AppState, width: number): string[] {
	if (state.scanComplete) {
		const barWidth = Math.min(width - 30, 40);
		const bar = `${FG_GREEN}${"█".repeat(barWidth)}${RESET}`;
		return [
			`  [${bar}]  ${FG_GREEN}${BOLD}Done${RESET}  ${DIM}${state.dirsScanned.toLocaleString()} dirs scanned${RESET}`,
			"",
		];
	}

	const barWidth = Math.min(width - 40, 40);
	const dirsScanned = state.dirsScanned;

	// Animate the progress bar
	const animFrame = Math.floor(Date.now() / 100) % barWidth;
	let bar = "";
	for (let i = 0; i < barWidth; i++) {
		if (i <= animFrame) {
			bar += `${FG_GREEN}█${RESET}`;
		} else if (i <= animFrame + 3) {
			bar += `${FG_YELLOW}▓${RESET}`;
		} else {
			bar += `${FG_GRAY}░${RESET}`;
		}
	}

	return [
		`  [${bar}]  ${DIM}${dirsScanned.toLocaleString()} dirs scanned${RESET}`,
		"",
	];
}
