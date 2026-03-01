#!/usr/bin/env bun
import { App, type SortMode } from "./app";

/**
 * hakai-tui — Interactive TUI for hakai.
 *
 * This process is spawned by the hakai Rust binary.
 * It communicates with hakai-core via stdin/stdout JSON IPC.
 *
 * Environment variables set by hakai:
 *   HAKAI_CORE_BIN   — path to the hakai-core binary
 *   HAKAI_ROOT        — scan root directory
 *   HAKAI_TARGETS     — comma-separated target dir names
 *   HAKAI_EXCLUDE_HIDDEN — "1" to exclude hidden dirs
 *   HAKAI_DRY_RUN     — "1" for dry-run mode
 *   HAKAI_SORT        — sort mode: path, size, last-mod
 *   HAKAI_COLOR       — highlight color name
 */

function main(): void {
	const coreBin = process.env.HAKAI_CORE_BIN;
	if (!coreBin) {
		console.error(
			"hakai-tui: HAKAI_CORE_BIN environment variable is required.\n" +
			"This binary should be launched by the hakai CLI, not directly.",
		);
		process.exit(1);
	}

	const root = process.env.HAKAI_ROOT || process.cwd();
	const targets = (process.env.HAKAI_TARGETS || "node_modules")
		.split(",")
		.filter(Boolean);
	const excludeHidden = process.env.HAKAI_EXCLUDE_HIDDEN === "1";
	const dryRun = process.env.HAKAI_DRY_RUN === "1";
	const sort = (process.env.HAKAI_SORT as SortMode) || "path";
	const color = process.env.HAKAI_COLOR || "cyan";

	const app = new App(coreBin, root, targets, {
		excludeHidden,
		dryRun,
		sort,
		color,
	});

	// Graceful shutdown
	process.on("SIGINT", async () => {
		await app.shutdown();
		process.exit(0);
	});

	process.on("SIGTERM", async () => {
		await app.shutdown();
		process.exit(0);
	});

	app.start();
}

main();
