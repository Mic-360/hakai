import { KEY_MAP, type KeyAction } from "./constants/keybinds";

export type InputCallback = (action: KeyAction, raw: string) => void;
export type SearchInputCallback = (char: string) => void;

/**
 * Raw keyboard input handler.
 * Reads stdin in raw mode and maps sequences to KeyAction values.
 */
export class InputHandler {
	private callback: InputCallback;
	private searchCallback: SearchInputCallback | null = null;
	private isSearchMode: boolean = false;

	constructor(callback: InputCallback) {
		this.callback = callback;
	}

	/** Start reading raw input from stdin. */
	start(): void {
		if (process.stdin.isTTY) {
			process.stdin.setRawMode(true);
		}
		process.stdin.resume();
		process.stdin.on("data", (data: Buffer) => this.handleData(data));
	}

	/** Stop reading input. */
	stop(): void {
		if (process.stdin.isTTY) {
			process.stdin.setRawMode(false);
		}
		process.stdin.pause();
	}

	/** Enable search mode — typed characters go to searchCallback. */
	enterSearchMode(cb: SearchInputCallback): void {
		this.isSearchMode = true;
		this.searchCallback = cb;
	}

	/** Exit search mode. */
	exitSearchMode(): void {
		this.isSearchMode = false;
		this.searchCallback = null;
	}

	private handleData(data: Buffer): void {
		const raw = data.toString("utf-8");

		// Always handle Ctrl+C
		if (raw === "\x03") {
			this.callback("CTRL_C", raw);
			return;
		}

		if (this.isSearchMode) {
			// In search mode, special keys still get routed as actions
			const action = KEY_MAP[raw];
			if (action === "ESCAPE" || action === "ENTER" || action === "BACKSPACE") {
				this.callback(action, raw);
				return;
			}
			// Everything else is a search character
			if (this.searchCallback && raw.length === 1 && raw.charCodeAt(0) >= 32) {
				this.searchCallback(raw);
				return;
			}
			return;
		}

		// Normal mode: look up key mapping
		const action = KEY_MAP[raw];
		if (action) {
			this.callback(action, raw);
		}
		// Unknown keys are ignored
	}
}
