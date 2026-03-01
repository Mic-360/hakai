import { Subprocess } from "bun";

// ── Types for IPC events from Rust core ──────────────────────────

export interface IpcEventReady {
	event: "Ready";
}
export interface IpcEventScanFound {
	event: "ScanFound";
	path: string;
}
export interface IpcEventScanSize {
	event: "ScanSize";
	path: string;
	size_bytes: number;
	newest_ms: number;
}
export interface IpcEventScanRisk {
	event: "ScanRisk";
	path: string;
	is_dead: boolean;
	risk: string;
}
export interface IpcEventScanProgress {
	event: "ScanProgress";
	scanned: number;
	found: number;
}
export interface IpcEventScanComplete {
	event: "ScanComplete";
	total_found: number;
	duration_ms: number;
}
export interface IpcEventDeleteProgress {
	event: "DeleteProgress";
	path: string;
	status: string;
	freed_bytes: number;
}
export interface IpcEventDeleteComplete {
	event: "DeleteComplete";
	total_freed: number;
}
export interface IpcEventError {
	event: "Error";
	message: string;
}

export type IpcEvent =
	| IpcEventReady
	| IpcEventScanFound
	| IpcEventScanSize
	| IpcEventScanRisk
	| IpcEventScanProgress
	| IpcEventScanComplete
	| IpcEventDeleteProgress
	| IpcEventDeleteComplete
	| IpcEventError;

// ── IPC Commands TO Rust core ────────────────────────────────────

export interface StartScanCmd {
	cmd: "StartScan";
	root: string;
	targets: string[];
	exclude: string[];
	exclude_hidden: boolean;
	max_depth: number | null;
}

export interface StopScanCmd {
	cmd: "StopScan";
}

export interface GetSizeCmd {
	cmd: "GetSize";
	path: string;
}

export interface DeleteCmd {
	cmd: "Delete";
	paths: string[];
	dry_run: boolean;
}

export interface DeleteAllCmd {
	cmd: "DeleteAll";
	dry_run: boolean;
}

export interface AnalyzeRiskCmd {
	cmd: "AnalyzeRisk";
	path: string;
	target: string;
}

export type IpcCommand =
	| StartScanCmd
	| StopScanCmd
	| GetSizeCmd
	| DeleteCmd
	| DeleteAllCmd
	| AnalyzeRiskCmd;

// ── IPC Client ───────────────────────────────────────────────────

export type EventHandler = (event: IpcEvent) => void;

export class IpcClient {
	private proc: Subprocess<"pipe", "pipe", "inherit">;
	private handler: EventHandler;
	private buffer: string = "";
	private running: boolean = true;

	constructor(coreBinPath: string, handler: EventHandler) {
		this.handler = handler;

		this.proc = Bun.spawn([coreBinPath, "--ipc"], {
			stdin: "pipe",
			stdout: "pipe",
			stderr: "inherit",
		});

		// Start reading stdout
		this.readLoop();
	}

	private async readLoop() {
		const reader = this.proc.stdout.getReader();
		const decoder = new TextDecoder();

		try {
			while (this.running) {
				const { done, value } = await reader.read();
				if (done) break;

				this.buffer += decoder.decode(value, { stream: true });

				// Process complete lines
				let newlineIdx: number;
				while ((newlineIdx = this.buffer.indexOf("\n")) !== -1) {
					const line = this.buffer.slice(0, newlineIdx).trim();
					this.buffer = this.buffer.slice(newlineIdx + 1);

					if (line.length > 0) {
						try {
							const event = JSON.parse(line) as IpcEvent;
							this.handler(event);
						} catch {
							// Skip malformed JSON
						}
					}
				}
			}
		} catch {
			// Stream closed
		}
	}

	/** Send a command to the Rust core. */
	async send(cmd: IpcCommand): Promise<void> {
		const json = JSON.stringify(cmd) + "\n";
		this.proc.stdin.write(json);
		await this.proc.stdin.flush();
	}

	/** Gracefully shut down the IPC connection. */
	async close(): Promise<void> {
		this.running = false;
		try {
			this.proc.stdin.end();
		} catch {
			// Already closed
		}
		this.proc.kill();
	}
}
