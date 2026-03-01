<div align="center">

# 💀 hakai (破壊)

**Blazing-fast directory destroyer — find and obliterate `node_modules`, `target`, `__pycache__`, and more.**

A high-performance rewrite of [npkill](https://github.com/voidcosmos/npkill), built with **Rust** for the filesystem engine and **Bun** for the interactive TUI.

[![License: MIT](https://img.shields.io/badge/License-MIT-red.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Bun](https://img.shields.io/badge/Bun-1.0%2B-white.svg)](https://bun.sh/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue.svg)](#installation)

</div>

---

## Why hakai?

**npkill** is great, but it has known limits: slow scans on large trees, single-threaded deletion, broken TTY on Windows Git Bash, and performance issues from high-level directories. **hakai eliminates all of these.**

| Operation                | npkill (Node.js) | hakai (Rust+Bun)    | Gain       |
| ------------------------ | ---------------- | ------------------- | ---------- |
| Scan 50k dirs            | ~8–12s           | <1s                 | **10–15×** |
| Size calculation         | Sequential       | Parallel (rayon)    | **8–12×**  |
| Delete 5GB folder        | ~45s, blocks UI  | Async, non-blocking | **4–6×**   |
| Delete-all (100 folders) | Sequential       | 8 concurrent        | **20–40×** |
| Startup                  | ~400ms           | <50ms               | **8×**     |
| Windows Git Bash         | ❌ Broken        | ✅ Works            | Fixed      |

## Demo

```
💀 hakai v1.0.0    ⚡ Rust+Bun                        Sort: size ↓
Found: 47 dirs  ·  Total: 23.4 GB  ·  Freed: 0 B  ·  Scan: 0.8s
[████████████████████████████████████████]  Done  ·  8,423 dirs scanned
──────────────────────────────────────────────────────────────────
    C:\projects\myapp\node_modules              2d ago     2.3 GB
  ▶ C:\projects\backend\node_modules           8mo ago    14.1 GB ⚠
    C:\projects\legacy\node_modules             1y ago     4.7 GB
    C:\work\dashboard\node_modules              3d ago     1.2 GB
──────────────────────────────────────────────────────────────────
  ↑↓/jk: navigate  ·  Space/Del: delete  ·  T: multi  ·  /: search
  o: open dir  ·  e: errors  ·  s: sort  ·  q: quit
```

## Features

- **Parallel scanning** — rayon thread pool scans directory trees across all CPU cores
- **Concurrent deletion** — tokio async runtime deletes up to 8 directories simultaneously
- **Real-time size calculation** — sizes appear as the scan progresses, not after
- **15+ built-in profiles** — `node`, `rust`, `python`, `flutter`, `java`, `all`, and more
- **Interactive TUI** — diff-based flicker-free rendering, vim keybinds, search/filter with regex
- **Multi-select & range select** — bulk operations with `T`, `V`, `A` keys
- **Risk analysis** — flags orphaned `node_modules` (⚠) and system-level directories
- **Cross-platform** — Windows (incl. Git Bash), macOS, Linux with native path handling
- **JSON output** — `--json` and `--json-stream` for scripting and CI pipelines
- **Dry-run mode** — preview what would be deleted without removing anything
- **Configurable** — `.hakairc` TOML config with custom profiles and exclusions

## Quick Start

```bash
# Install (see INSTALLATION.md for all methods)
git clone https://github.com/mic-360/hakai.git
cd hakai
cargo build --release

# Scan current directory for node_modules
./target/release/hakai

# Scan home directory for everything
./target/release/hakai --full --profile all

# Scan and auto-delete with confirmation
./target/release/hakai --delete-all

# JSON output for scripting
./target/release/hakai --json --sort size
```

See [INSTALLATION.md](INSTALLATION.md) for detailed setup instructions.

## Usage

### Basic Commands

```bash
hakai                              # Scan cwd for node_modules (interactive TUI)
hakai -d ~/projects                # Scan specific directory
hakai -f                           # Scan from $HOME
hakai -t target                    # Scan for Rust target dirs
hakai -t node_modules,target       # Multiple targets
hakai -p rust                      # Use built-in "rust" profile
hakai -p all                       # Use "all" profile (node_modules, target, __pycache__, etc.)
```

### Profiles

Built-in profiles target common build artifact directories:

| Profile   | Targets                                                                       |
| --------- | ----------------------------------------------------------------------------- |
| `node`    | `node_modules`                                                                |
| `rust`    | `target`                                                                      |
| `python`  | `__pycache__`, `.venv`, `venv`, `.mypy_cache`, `.ruff_cache`, `.pytest_cache` |
| `flutter` | `build`, `.dart_tool`, `ios/Pods`, `android/build`, `android/.gradle`         |
| `java`    | `build`, `.gradle`, `out`, `target`                                           |
| `all`     | All of the above + `dist`, `.next`, `.nuxt`, `.turbo`, `.svelte-kit`          |

Custom profiles can be defined in `~/.hakairc` — see [Configuration](#configuration).

### Interactive TUI Keys

| Key                | Action                                              |
| ------------------ | --------------------------------------------------- |
| `↑`/`↓` or `j`/`k` | Navigate results                                    |
| `Space` or `Del`   | Delete selected directory                           |
| `T`                | Toggle multi-select mode                            |
| `V`                | Range select (select all between anchor and cursor) |
| `A`                | Select all / deselect all                           |
| `Enter`            | Delete all selected (in multi-select mode)          |
| `/`                | Search / filter results (supports regex)            |
| `s`                | Cycle sort mode: path → size → last-modified        |
| `o`                | Open parent directory in file manager               |
| `e`                | Toggle error popup                                  |
| `Esc`              | Cancel search / exit multi-select                   |
| `q` or `Ctrl+C`    | Quit                                                |

### Headless / Scripting Mode

```bash
# JSON output (single object after scan completes)
hakai --json -d ~/projects > results.json

# Streaming JSON (one object per line as found — great for piping)
hakai --json-stream -d ~/projects | jq '.path'

# Auto-delete everything found, skip confirmation
hakai --delete-all -y -d ~/old-projects

# Dry run — see what would be deleted without removing anything
hakai --delete-all --dry-run -d ~/projects

# Sort by size, show only large dirs
hakai --json --sort size -d ~/projects | jq '.results[] | select(.size > 1073741824)'
```

### JSON Output Format

```json
{
  "meta": {
    "version": "1.0.0",
    "scan_root": "/home/user/projects",
    "targets": ["node_modules"],
    "duration_ms": 823,
    "dirs_scanned": 47
  },
  "results": [
    {
      "path": "/home/user/projects/myapp/node_modules",
      "size": 2415919104,
      "modificationTime": 1704067200000,
      "isDead": false,
      "riskLevel": "low"
    }
  ],
  "summary": {
    "total_found": 47,
    "total_size_bytes": 24897593344,
    "total_size_human": "23.2 GB"
  }
}
```

### All CLI Options

```
hakai [OPTIONS]

SCAN OPTIONS:
  -d, --directory <PATH>     Start scan from PATH (default: current dir)
  -f, --full                 Start from $HOME
  -t, --target <NAME,...>    Target dir names (default: node_modules)
  -p, --profile <NAME>      Use a named profile from .hakairc
  -E, --exclude <DIRS>       Exclude directories (comma-separated)
  -x, --exclude-hidden       Exclude hidden/dot directories
      --max-depth <N>        Maximum scan depth

DELETE OPTIONS:
  -D, --delete-all           Auto-delete all found dirs
  -y                         Skip confirmation on --delete-all
      --dry-run              Simulate deletion (no actual deletes)

DISPLAY OPTIONS:
  -s, --sort <MODE>          Sort by: path, size, last-mod
  -c, --color <COLOR>        Highlight color: blue, cyan, magenta, white, red, yellow

OUTPUT OPTIONS:
      --json                 Output all results as JSON at end of scan
      --json-stream          Stream results as newline-delimited JSON
  -e, --hide-errors          Suppress error messages

PERFORMANCE OPTIONS:
      --threads <N>          Rayon thread pool size (0 = auto)
      --no-parallel          Disable parallel scan

OTHER:
      --no-check-update      Skip update check
      --size-unit <UNIT>     Size unit: auto, mb, gb
  -v, --version              Show version
  -h, --help                 Show help
```

## Configuration

Create `~/.hakairc` (TOML format) to customize defaults and add profiles:

```toml
[settings]
default_sort    = "size"     # path | size | last-mod
size_unit       = "auto"     # auto | mb | gb
color           = "cyan"     # highlight color
threads         = 0          # 0 = auto (num_cpus)
exclude_hidden  = true

[profiles.frontend]
targets = ["node_modules", "dist", ".next", ".nuxt"]

[profiles.monorepo]
targets = ["node_modules", "target", "build", "dist", ".turbo"]

[exclude]
directories = [
    "C:\\Program Files",
    "C:\\Windows",
]
```

See [.hakairc.example](.hakairc.example) for a full example.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        hakai CLI                            │
│  ┌─────────────────────────┐  ┌───────────────────────────┐ │
│  │   Rust Core             │  │   Bun TUI                 │ │
│  │  • Parallel scan (rayon)│  │  • Interactive UI         │ │
│  │  • Async delete (tokio) │  │  • Keyboard handling      │ │
│  │  • Size calculation     │  │  • Diff-based rendering   │ │
│  │  • Risk analysis        │  │  • Filtering / sorting    │ │
│  │  • Windows path support │  │  • JSON output            │ │
│  └───────────┬─────────────┘  └──────────┬────────────────┘ │
│              │  IPC (stdin/stdout JSON)  │                  │
│              └───────────────────────────┘  ****                │
└─────────────────────────────────────────────────────────────┘
```

- **Rust** handles all I/O-bound and CPU-bound work: parallel traversal, concurrent deletion, size calculation
- **Bun** handles the TUI, user input, and rendering (starts 5–10× faster than Node.js)
- Communication via newline-delimited JSON over stdin/stdout pipes

## Project Structure

```
hakai/
├── Cargo.toml                    # Rust workspace root
├── package.json                  # Bun workspace root
├── Makefile                      # Build all targets
├── .hakairc.example              # Example configuration
├── crates/
│   └── hakai-core/               # Rust crate: filesystem engine
│       └── src/
│           ├── main.rs           # CLI + IPC server + headless mode
│           ├── scanner.rs        # Parallel directory traversal
│           ├── sizer.rs          # Concurrent size calculation
│           ├── deleter.rs        # Async batch deletion
│           ├── risk.rs           # Risk analysis engine
│           ├── ipc.rs            # JSON IPC protocol
│           ├── config.rs         # .hakairc parser + built-in profiles
│           └── platform/         # OS-specific code (Windows/Unix)
└── packages/
    └── hakai-tui/                # Bun TUI package
        └── src/
            ├── index.ts          # Entry point
            ├── app.ts            # State machine + event handling
            ├── ipc.ts            # IPC client
            ├── renderer.ts       # Diff-based terminal renderer
            ├── input.ts          # Raw keyboard input
            └── components/       # UI components
```

## Risk Analysis

hakai flags directories that may be risky to delete:

| Indicator   | Meaning                                                               |
| ----------- | --------------------------------------------------------------------- |
| ⚠ (red)     | **High risk** — system-level location (Program Files, /usr/lib)       |
| ⚡ (yellow) | **Medium risk** — global package manager location (.nvm, .volta)      |
| ☠ (yellow)  | **Orphaned** — parent has no package.json (project was moved/deleted) |
| _(none)_    | **Low risk** — normal project directory                               |

## Compatibility

- **Windows 10+** — full support including Git Bash, CMD, PowerShell, Windows Terminal. Long paths (>260 chars) handled via `\\?\` prefix.
- **macOS 12+** — full support
- **Linux** — full support on any modern distro

### npkill Compatibility

hakai's `--json` output is format-compatible with npkill, so existing scripts work without modification.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

[MIT](LICENSE) © hakai contributors
