import { renderErrorsPopup } from "./components/errors-popup";
import { renderHeader } from "./components/header";
import { renderProgressBar } from "./components/progress-bar";
import { renderResultsList } from "./components/results-list";
import { renderStatsBar } from "./components/stats-bar";
import { renderStatusBar } from "./components/status-bar";
import type { KeyAction } from "./constants/keybinds";
import { InputHandler } from "./input";
import { IpcClient, type IpcEvent } from "./ipc";
import { APP_SUBTITLE, APP_TITLE, HAKAI_QUOTES, VERSION } from "./constants/cli";
import { DiffRenderer } from "./renderer";

// ── Types ────────────────────────────────────────────────────────

export type AppMode =
	| "normal"
	| "multi-select"
	| "range-select"
	| "search"
	| "deleting"
	| "done";

export type SortMode = "path" | "size" | "last-mod";

export interface FolderResult {
	path: string;
	sizeBytes: number;
	newestMs: number;
	isDead: boolean;
	riskLevel: "low" | "medium" | "high";
	status: "found" | "deleting" | "deleted" | "error";
	errorMessage?: string;
}

export interface AppState {
	mode: AppMode;
	results: FolderResult[];
	filteredResults: FolderResult[];
	selectedIndex: number;
	selectedPaths: Set<string>;
	rangeStart: number | null;
	searchQuery: string;
	scanComplete: boolean;
	totalSize: number;
	freedSpace: number;
	errors: string[];
	showErrors: boolean;
	scrollOffset: number;
	sortMode: SortMode;
	scanDurationMs: number;
	dirsScanned: number;
	dirsPending: number;
	currentQuote: string;
	title: string;
	subtitle: string;
	version: string;
}

// ── App ──────────────────────────────────────────────────────────

export class App {
	private state: AppState;
	private renderer: DiffRenderer;
	private input: InputHandler;
	private ipc: IpcClient;
	private renderTimer: ReturnType<typeof setInterval> | null = null;
	private resizeHandler: (() => void) | null = null;
	private dryRun: boolean;

	constructor(
		coreBinPath: string,
		root: string,
		targets: string[],
		options: {
			excludeHidden?: boolean;
			dryRun?: boolean;
			sort?: SortMode;
			color?: string;
		} = {},
	) {
		this.dryRun = options.dryRun ?? false;

		this.state = {
			mode: "normal",
			results: [],
			filteredResults: [],
			selectedIndex: 0,
			selectedPaths: new Set(),
			rangeStart: null,
			searchQuery: "",
			scanComplete: false,
			totalSize: 0,
			freedSpace: 0,
			errors: [],
			showErrors: false,
			scrollOffset: 0,
			sortMode: options.sort ?? "path",
			scanDurationMs: 0,
			dirsScanned: 0,
			dirsPending: 0,
			currentQuote: HAKAI_QUOTES[Math.floor(Math.random() * HAKAI_QUOTES.length)],
			title: APP_TITLE,
			subtitle: APP_SUBTITLE,
			version: VERSION,
		};

		this.renderer = new DiffRenderer();
		this.input = new InputHandler((action, raw) => this.handleKey(action, raw));
		this.ipc = new IpcClient(coreBinPath, (event) => this.handleIpcEvent(event));

		// Start scan immediately
		this.ipc.send({
			cmd: "StartScan",
			root,
			targets,
			exclude: [],
			exclude_hidden: options.excludeHidden ?? false,
			max_depth: null,
		});
	}

	/** Boot up the TUI and start rendering. */
	start(): void {
		this.renderer.enterAltScreen();
		this.renderer.hideCursor();
		this.renderer.clear();
		this.input.start();

		// Render loop at ~30fps
		this.renderTimer = setInterval(() => this.render(), 33);

		// Handle terminal resize
		this.resizeHandler = () => {
			this.renderer.updateSize();
			this.renderer.invalidate();
			this.render();
		};
		process.stdout.on("resize", this.resizeHandler);
	}

	/** Clean shutdown. */
	async shutdown(): Promise<void> {
		if (this.renderTimer) clearInterval(this.renderTimer);
		this.input.stop();
		this.renderer.showCursor();
		this.renderer.leaveAltScreen();
		await this.ipc.close();
	}

	// ── IPC Event Handling ──────────────────────────────────────

	private handleIpcEvent(event: IpcEvent): void {
		switch (event.event) {
			case "Ready":
				break;

			case "ScanFound":
				this.state.results.push({
					path: event.path,
					sizeBytes: 0,
					newestMs: 0,
					isDead: false,
					riskLevel: "low",
					status: "found",
				});
				this.applyFilter();
				break;

			case "ScanSize": {
				const r = this.state.results.find((r) => r.path === event.path);
				if (r) {
					r.sizeBytes = event.size_bytes;
					r.newestMs = event.newest_ms;
					this.state.totalSize = this.state.results.reduce(
						(sum, r) => sum + r.sizeBytes,
						0,
					);
					this.applySort();
					this.applyFilter();
				}
				break;
			}

			case "ScanRisk": {
				const r = this.state.results.find((r) => r.path === event.path);
				if (r) {
					r.isDead = event.is_dead;
					r.riskLevel = event.risk as "low" | "medium" | "high";
				}
				break;
			}

			case "ScanProgress":
				this.state.dirsScanned = event.scanned;
				break;

			case "ScanComplete":
				this.state.scanComplete = true;
				this.state.scanDurationMs = event.duration_ms;
				this.applySort();
				this.applyFilter();

				// Request risk analysis for all found dirs
				for (const r of this.state.results) {
					const target = r.path.split(/[\\/]/).pop() ?? "node_modules";
					this.ipc.send({
						cmd: "AnalyzeRisk",
						path: r.path,
						target,
					});
				}
				break;

			case "DeleteProgress": {
				const r = this.state.results.find((r) => r.path === event.path);
				if (r) {
					if (event.status === "deleted") {
						r.status = "deleted";
						this.state.freedSpace += event.freed_bytes;
					} else if (event.status.startsWith("error")) {
						r.status = "error";
						r.errorMessage = event.status.replace("error: ", "");
					} else {
						r.status = "deleting";
					}
				}
				this.applyFilter();
				break;
			}

			case "DeleteComplete":
				if (this.state.mode === "deleting") {
					this.state.mode = "normal";
				}
				break;

			case "Error":
				this.state.errors.push(event.message);
				break;
		}
	}

	// ── Keyboard Handling ───────────────────────────────────────

	private handleKey(action: KeyAction, _raw: string): void {
		// Error popup takes priority
		if (this.state.showErrors && action === "SHOW_ERRORS") {
			this.state.showErrors = false;
			return;
		}

		switch (action) {
			case "CTRL_C":
			case "QUIT":
				this.shutdown().then(() => process.exit(0));
				return;

			case "UP":
				this.moveSelection(-1);
				break;

			case "DOWN":
				this.moveSelection(1);
				break;

			case "PAGE_UP":
				this.moveSelection(-(this.renderer.getHeight() - 8));
				break;

			case "PAGE_DOWN":
				this.moveSelection(this.renderer.getHeight() - 8);
				break;

			case "HOME":
				this.state.selectedIndex = 0;
				this.state.scrollOffset = 0;
				break;

			case "END":
				this.state.selectedIndex = Math.max(0, this.state.filteredResults.length - 1);
				this.ensureVisible();
				break;

			case "SPACE":
			case "DELETE":
				this.handleDeleteOrToggle();
				break;

			case "ENTER":
				this.handleEnter();
				break;

			case "TOGGLE_MULTI":
				this.toggleMultiSelect();
				break;

			case "SELECT_ALL":
				this.selectAll();
				break;

			case "RANGE_SELECT":
				this.toggleRangeSelect();
				break;

			case "SEARCH":
				this.enterSearchMode();
				break;

			case "ESCAPE":
				this.handleEscape();
				break;

			case "BACKSPACE":
				if (this.state.mode === "search") {
					this.state.searchQuery = this.state.searchQuery.slice(0, -1);
					this.applyFilter();
				}
				break;

			case "SORT":
				this.cycleSort();
				break;

			case "OPEN_DIR":
				this.openDirectory();
				break;

			case "SHOW_ERRORS":
				this.state.showErrors = !this.state.showErrors;
				break;
		}
	}

	private moveSelection(delta: number): void {
		const len = this.state.filteredResults.length;
		if (len === 0) return;
		this.state.selectedIndex = Math.max(
			0,
			Math.min(len - 1, this.state.selectedIndex + delta),
		);

		// Update range selection
		if (this.state.mode === "range-select" && this.state.rangeStart !== null) {
			this.updateRangeSelection();
		}

		this.ensureVisible();
	}

	private ensureVisible(): void {
		const listHeight = this.renderer.getHeight() - 8; // header + stats + progress + separator + status bars
		if (this.state.selectedIndex < this.state.scrollOffset) {
			this.state.scrollOffset = this.state.selectedIndex;
		} else if (this.state.selectedIndex >= this.state.scrollOffset + listHeight) {
			this.state.scrollOffset = this.state.selectedIndex - listHeight + 1;
		}
	}

	private handleDeleteOrToggle(): void {
		if (this.state.filteredResults.length === 0) return;

		if (this.state.mode === "multi-select" || this.state.mode === "range-select") {
			// Toggle selection
			const path = this.state.filteredResults[this.state.selectedIndex]?.path;
			if (path) {
				if (this.state.selectedPaths.has(path)) {
					this.state.selectedPaths.delete(path);
				} else {
					this.state.selectedPaths.add(path);
				}
			}
		} else {
			// Delete single item
			const result = this.state.filteredResults[this.state.selectedIndex];
			if (result && result.status === "found") {
				result.status = "deleting";
				this.ipc.send({
					cmd: "Delete",
					paths: [result.path],
					sizes: { [result.path]: result.sizeBytes },
					dry_run: this.dryRun,
				});
			}
		}
	}

	private handleEnter(): void {
		if (this.state.mode === "search") {
			this.input.exitSearchMode();
			this.state.mode = "normal";
			return;
		}

		if (
			(this.state.mode === "multi-select" || this.state.mode === "range-select") &&
			this.state.selectedPaths.size > 0
		) {
			// Delete all selected
			const paths = Array.from(this.state.selectedPaths);
			for (const path of paths) {
				const r = this.state.results.find((r) => r.path === path);
				if (r) r.status = "deleting";
			}
			this.state.mode = "deleting";
			const sizes: Record<string, number> = {};
			for (const p of paths) {
				const r = this.state.results.find((r) => r.path === p);
				if (r) sizes[p] = r.sizeBytes;
			}
			this.ipc.send({
				cmd: "Delete",
				paths,
				sizes,
				dry_run: this.dryRun,
			});
			this.state.selectedPaths.clear();
		}
	}

	private toggleMultiSelect(): void {
		if (this.state.mode === "multi-select") {
			this.state.mode = "normal";
			this.state.selectedPaths.clear();
			this.state.rangeStart = null;
		} else {
			this.state.mode = "multi-select";
		}
	}

	private selectAll(): void {
		if (this.state.mode !== "multi-select" && this.state.mode !== "range-select") return;

		if (this.state.selectedPaths.size === this.state.filteredResults.length) {
			// Deselect all
			this.state.selectedPaths.clear();
		} else {
			// Select all
			for (const r of this.state.filteredResults) {
				if (r.status === "found") {
					this.state.selectedPaths.add(r.path);
				}
			}
		}
	}

	private toggleRangeSelect(): void {
		if (this.state.mode === "range-select") {
			this.state.mode = "multi-select";
			this.state.rangeStart = null;
		} else {
			this.state.mode = "range-select";
			this.state.rangeStart = this.state.selectedIndex;
		}
	}

	private updateRangeSelection(): void {
		if (this.state.rangeStart === null) return;

		const start = Math.min(this.state.rangeStart, this.state.selectedIndex);
		const end = Math.max(this.state.rangeStart, this.state.selectedIndex);

		this.state.selectedPaths.clear();
		for (let i = start; i <= end; i++) {
			const r = this.state.filteredResults[i];
			if (r && r.status === "found") {
				this.state.selectedPaths.add(r.path);
			}
		}
	}

	private enterSearchMode(): void {
		this.state.mode = "search";
		this.state.searchQuery = "";
		this.input.enterSearchMode((char) => {
			this.state.searchQuery += char;
			this.applyFilter();
		});
	}

	private handleEscape(): void {
		if (this.state.mode === "search") {
			this.input.exitSearchMode();
			this.state.mode = "normal";
			this.state.searchQuery = "";
			this.applyFilter();
		} else if (
			this.state.mode === "multi-select" ||
			this.state.mode === "range-select"
		) {
			this.state.mode = "normal";
			this.state.selectedPaths.clear();
			this.state.rangeStart = null;
		}
	}

	private cycleSort(): void {
		const modes: SortMode[] = ["path", "size", "last-mod"];
		const idx = modes.indexOf(this.state.sortMode);
		this.state.sortMode = modes[(idx + 1) % modes.length];
		this.applySort();
		this.applyFilter();
	}

	private applySort(): void {
		switch (this.state.sortMode) {
			case "size":
				this.state.results.sort((a, b) => b.sizeBytes - a.sizeBytes);
				break;
			case "last-mod":
				this.state.results.sort((a, b) => b.newestMs - a.newestMs);
				break;
			default:
				this.state.results.sort((a, b) => a.path.localeCompare(b.path));
		}
	}

	private applyFilter(): void {
		const query = this.state.searchQuery;
		if (!query) {
			this.state.filteredResults = [...this.state.results];
		} else {
			try {
				const regex = new RegExp(query, "i");
				this.state.filteredResults = this.state.results.filter((r) =>
					regex.test(r.path),
				);
			} catch {
				// Invalid regex: fall back to literal string match
				const lower = query.toLowerCase();
				this.state.filteredResults = this.state.results.filter((r) =>
					r.path.toLowerCase().includes(lower),
				);
			}
		}

		// Clamp selection
		if (this.state.selectedIndex >= this.state.filteredResults.length) {
			this.state.selectedIndex = Math.max(0, this.state.filteredResults.length - 1);
		}
		this.ensureVisible();
	}

	private openDirectory(): void {
		const result = this.state.filteredResults[this.state.selectedIndex];
		if (!result) return;

		// Open the parent directory in the system file manager
		const dir = result.path.replace(/[\\/][^\\/]+$/, "");
		const isWindows = process.platform === "win32";

		if (isWindows) {
			Bun.spawn(["explorer", dir], { stdout: "ignore", stderr: "ignore" });
		} else if (process.platform === "darwin") {
			Bun.spawn(["open", dir], { stdout: "ignore", stderr: "ignore" });
		} else {
			Bun.spawn(["xdg-open", dir], { stdout: "ignore", stderr: "ignore" });
		}
	}

	// ── Rendering ───────────────────────────────────────────────

	private render(): void {
		const width = this.renderer.getWidth();
		const height = this.renderer.getHeight();
		const frame: string[] = [];

		// Header (2 lines: title + separator)
		frame.push(...renderHeader(this.state, width));

		// Stats bar (2 lines: stats + blank)
		frame.push(...renderStatsBar(this.state, width));

		// Progress bar (2 lines: bar + blank)
		frame.push(...renderProgressBar(this.state, width));

		// Status bar lines (reserve space at bottom)
		const statusLines = renderStatusBar(this.state, width);
		const statusHeight = statusLines.length;

		// Results list (fills remaining space)
		const usedLines = frame.length + statusHeight;
		const listHeight = Math.max(1, height - usedLines);
		const resultLines = renderResultsList(this.state, width, listHeight);
		frame.push(...resultLines);

		// Status bar
		frame.push(...statusLines);

		// Error popup overlay (if visible)
		if (this.state.showErrors && this.state.errors.length > 0) {
			const errLines = renderErrorsPopup(this.state, width, height);
			// Overlay errors starting at line 5
			const overlayStart = 4;
			for (let i = 0; i < errLines.length && overlayStart + i < frame.length; i++) {
				frame[overlayStart + i] = errLines[i];
			}
		}

		this.renderer.render(frame);
	}
}
