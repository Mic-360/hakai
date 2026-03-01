/**
 * Diff-based terminal renderer — only updates lines that have changed.
 * Uses ANSI cursor positioning for flicker-free updates.
 */
export class DiffRenderer {
	private lastFrame: string[] = [];
	private width: number = 80;
	private height: number = 24;

	constructor() {
		this.updateSize();
	}

	/** Re-read terminal dimensions. */
	updateSize(): void {
		this.width = process.stdout.columns || 80;
		this.height = process.stdout.rows || 24;
	}

	getWidth(): number {
		return this.width;
	}

	getHeight(): number {
		return this.height;
	}

	/** Render a new frame, only writing changed lines. */
	render(newFrame: string[]): void {
		const height = Math.min(newFrame.length, this.height);

		for (let i = 0; i < height; i++) {
			const line = newFrame[i] ?? "";
			if (line !== this.lastFrame[i]) {
				// Move cursor to row i+1 col 1, clear line, write content
				process.stdout.write(`\x1b[${i + 1};1H\x1b[2K${line}`);
			}
		}

		// Clear any extra lines from previous frame
		for (let i = height; i < this.lastFrame.length; i++) {
			process.stdout.write(`\x1b[${i + 1};1H\x1b[2K`);
		}

		this.lastFrame = [...newFrame];
	}

	/** Full screen clear. */
	clear(): void {
		process.stdout.write("\x1b[2J\x1b[H");
		this.lastFrame = [];
	}

	hideCursor(): void {
		process.stdout.write("\x1b[?25l");
	}

	showCursor(): void {
		process.stdout.write("\x1b[?25h");
	}

	/** Enable alternative screen buffer (saves/restores terminal content). */
	enterAltScreen(): void {
		process.stdout.write("\x1b[?1049h");
	}

	/** Leave alternative screen buffer (restores original terminal content). */
	leaveAltScreen(): void {
		process.stdout.write("\x1b[?1049l");
	}

	/** Force full re-render on next call. */
	invalidate(): void {
		this.lastFrame = [];
	}
}
