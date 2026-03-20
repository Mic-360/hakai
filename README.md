<div align="center">
  <img src="hakai-logo.png" align="center" alt="hakai logo" width="30%" />

# hakai (破壊)

**"Throughout the filesystem and the disk, I alone am the honored one."**

**The strongest directory destroyer — find and obliterate `node_modules`, `target`, `__pycache__`, and more.**

[![License: MIT](https://img.shields.io/badge/License-MIT-red.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue.svg)](#compatibility)

</div>

---

## Why hakai?

Build artifact directories like `node_modules`, `target`, and `__pycache__` silently consume tens of gigabytes across your projects. Hakai finds them all in under a second and deletes them with parallel precision — reclaiming your disk space instantly.

Built entirely in Rust, hakai leverages work-stealing parallelism for scanning, sizing, and deletion. It runs on every major platform with zero external runtime dependencies.

## hakai vs npkill

[npkill](https://github.com/voidcosma/npkill) is a popular Node.js tool for removing `node_modules`. Hakai was built to solve its fundamental limitations: single-threaded architecture, slow scans on large directory trees, sequential deletion, and broken behavior on Windows Git Bash.

| Metric                   | npkill (Node.js)  | hakai (Rust)            | Improvement    |
| ------------------------ | ----------------- | ----------------------- | -------------- |
| Scan 50,000 directories  | 8–12 s            | < 1 s                   | **10–15x**     |
| Size calculation          | Sequential         | Parallel (rayon)        | **8–12x**      |
| Delete a 5 GB directory  | ~45 s, blocks UI  | Parallel, non-blocking  | **4–6x**       |
| Batch delete 100 folders | Sequential         | Parallel (rayon)        | **20–40x**     |
| Cold startup             | ~400 ms            | < 50 ms                 | **8x**         |
| Windows Git Bash         | Broken             | Full support            | Fixed          |
| Language support          | node_modules only  | 6 built-in profiles     | Multi-language |
| Runtime dependency        | Node.js            | None (static binary)    | Zero-dep       |

**Why is hakai faster?**

- **Parallel scanning**: The `ignore` crate's parallel walker distributes directory traversal across all CPU cores via rayon's work-stealing thread pool — npkill walks the tree sequentially on a single thread.
- **Concurrent sizing**: Each discovered directory is sized on a rayon worker thread immediately, overlapping I/O with the ongoing scan — npkill calculates sizes one at a time after discovery.
- **Parallel deletion**: Directories are deleted concurrently using rayon, with an internal recursive parallel walk that shreds files across threads — npkill deletes one directory at a time, blocking the UI.
- **Instant rename-to-trash**: On supported platforms, hakai renames the directory to a hidden trash path first (an atomic O(1) operation), then cleans up in the background — the directory disappears from the UI instantly.
- **Native binary**: Hakai compiles to a single static binary with no runtime, no garbage collector, and no JIT warmup.

## Demo

```
💀 hakai v1.0.0                                              Sort: size ↓
"Nah, I'd clean."
Found: 47 dirs  ·  Total: 23.4 GB  ·  Freed: 0 B  ·  Scan: 0.8s
[████████████████████████████████████████]  Done  ·  8,423 dirs scanned
──────────────────────────────────────────────────────────────────────────
    C:\projects\myapp\node_modules                2d ago     2.3 GB
  ▶ C:\projects\backend\node_modules             8mo ago    14.1 GB  ⚠
    C:\projects\legacy\node_modules               1y ago     4.7 GB
    C:\work\dashboard\node_modules                3d ago     1.2 GB
──────────────────────────────────────────────────────────────────────────
  ↑↓/jk: navigate  ·  Space/Del: delete  ·  T: multi  ·  /: search
  o: open dir  ·  e: errors  ·  s: sort  ·  q: quit
```

## Features

- **Parallel scanning** — rayon work-stealing thread pool scans directory trees across all CPU cores
- **Parallel deletion** — recursive rayon-based deletion with concurrent file removal
- **Real-time size calculation** — sizes appear as the scan progresses, not after
- **6 built-in profiles** — `node`, `rust`, `python`, `flutter`, `java`, `all`
- **Interactive TUI** — ratatui-powered interface with vim keybindings, search, and multi-select
- **Risk analysis** — flags orphaned `node_modules` (missing package.json) and system-level directories
- **Cross-platform** — Windows (including Git Bash), macOS, and Linux with native path handling
- **JSON output** — `--json` and `--json-stream` for scripting and CI pipelines
- **Dry-run mode** — preview what would be deleted without removing anything
- **Configurable** — `.hakairc` TOML config with custom profiles and exclusions
- **Zero dependencies** — ships as a single static binary

## Quick Start

```bash
# Install (see INSTALLATION.md for all methods)
git clone https://github.com/mic-360/hakai.git
cd hakai && cargo build --release

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
hakai -p all                       # Scan for all known build artifacts
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

### Interactive TUI

| Key                     | Action                                              |
| ----------------------- | --------------------------------------------------- |
| `↑`/`↓` or `j`/`k`     | Navigate results                                    |
| `PgUp`/`PgDn`           | Page navigation                                     |
| `Home`/`End`            | Jump to first / last result                         |
| `Space` or `Del`        | Delete selected directory (with confirmation)       |
| `T`                     | Toggle multi-select mode                            |
| `V`                     | Toggle range select mode                            |
| `A`                     | Select all / deselect all                           |
| `Enter`                 | Delete all selected (in multi/range-select mode)    |
| `/`                     | Search and filter results                           |
| `s`                     | Cycle sort mode: path → size → last-modified        |
| `o`                     | Open parent directory in file manager               |
| `e`                     | Toggle error display                                |
| `?`                     | Show help                                           |
| `Esc`                   | Cancel search / exit multi-select / dismiss dialog  |
| `q` or `Ctrl+C`         | Quit                                                |

### Headless / Scripting Mode

```bash
# JSON output (single object after scan completes)
hakai --json -d ~/projects > results.json

# Streaming JSON (one object per line as found)
hakai --json-stream -d ~/projects | jq '.path'

# Auto-delete everything found, skip confirmation
hakai --delete-all -y -d ~/old-projects

# Dry run — see what would be deleted without removing anything
hakai --delete-all --dry-run -d ~/projects

# Filter large directories from JSON output
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
  -d, --directory <PATH>     Start scan from PATH (default: current directory)
  -f, --full                 Start scan from $HOME
  -t, --target <NAME,...>    Target directory names (default: node_modules)
  -p, --profile <NAME>       Use a built-in or custom profile
  -E, --exclude <DIRS>       Exclude directories (comma-separated)
  -x, --exclude-hidden       Exclude hidden/dot directories
      --max-depth <N>        Maximum scan depth
      --min-size <SIZE>      Minimum directory size to display (e.g., 10mb, 1gb)

DELETE OPTIONS:
  -D, --delete-all           Auto-delete all found directories
  -y                         Skip confirmation on --delete-all
      --dry-run              Simulate deletion (no actual deletes)

DISPLAY OPTIONS:
  -s, --sort <MODE>          Sort by: path, size, last-mod
  -c, --color <COLOR>        Highlight color (blue, cyan, magenta, white, red, yellow)
      --size-unit <UNIT>     Size unit: auto, mb, gb

OUTPUT OPTIONS:
      --json                 Output all results as JSON at end of scan
      --json-stream          Stream results as newline-delimited JSON
  -e, --hide-errors          Suppress error messages

PERFORMANCE OPTIONS:
      --threads <N>          Rayon thread pool size (0 = auto-detect)
      --no-parallel          Disable parallel scan

OTHER:
      --no-check-update      Skip update check
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
threads         = 0          # 0 = auto (uses all CPU cores)
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

See [.hakairc.example](.hakairc.example) for a complete reference.

## Architecture

Hakai is a single Rust binary. In interactive mode, it renders a TUI using ratatui. In headless mode (`--json`, `--json-stream`, `--delete-all`), it writes directly to stdout/stderr.

```
┌──────────────────────────────────────────────────────────────────────┐
│                           hakai CLI                                  │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────────────┐  │
│  │   Scanner     │  │   Sizer      │  │   Interactive TUI         │  │
│  │   (rayon +    │  │   (ignore    │  │   (ratatui + crossterm)   │  │
│  │    ignore)    │  │    parallel  │  │                           │  │
│  └──────┬───────┘  │    walker)   │  │  • Keyboard handling      │  │
│         │          └──────┬───────┘  │  • Multi-select / search  │  │
│         │                 │          │  • Real-time rendering     │  │
│  ┌──────▼─────────────────▼───────┐  └───────────────────────────┘  │
│  │   Crossbeam Event Channel      │                                  │
│  └──────┬─────────────────────────┘                                  │
│         │                                                            │
│  ┌──────▼───────┐  ┌──────────────┐  ┌───────────────────────────┐  │
│  │   Deleter     │  │   Risk       │  │   Config                  │  │
│  │   (rayon      │  │   Analysis   │  │   (.hakairc TOML parser)  │  │
│  │    parallel   │  │              │  │                           │  │
│  │    + rename   │  │  Dead proj   │  │  Built-in profiles        │  │
│  │    to trash)  │  │  detection   │  │  Custom profiles          │  │
│  └──────────────┘  └──────────────┘  └───────────────────────────┘  │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │   Platform Layer (Windows: \\?\ paths, junctions, readonly)  │    │
│  │                   (Unix: symlinks, permissions)               │    │
│  └──────────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

- **Scanner uses `ignore::WalkBuilder` with rayon** — the parallel walker distributes directory traversal across all CPU cores using work-stealing scheduling
- **Pruning on match** — when a target directory is found, hakai does not recurse into it, avoiding unnecessary I/O
- **Size calculation overlaps scanning** — discovered directories are immediately dispatched to rayon worker threads for sizing while the scan continues
- **Rename-to-trash deletion** — directories are atomically renamed to a hidden `.hakai_trash_*` path, then cleaned up in the background — the directory disappears from the user's perspective instantly
- **Parallel recursive deletion** — the deleter walks directories with rayon, spawning parallel tasks for subdirectories and using `par_iter` for batches of 256+ files
- **Windows long path support** — all paths are prefixed with `\\?\` to bypass the 260-character MAX_PATH limit

## Project Structure

```
hakai/
├── Cargo.toml                    # Workspace root
├── Makefile                      # Build, test, and release targets
├── .hakairc.example              # Configuration reference
├── install.sh                    # Unix installer
├── install.ps1                   # Windows installer
├── crates/
│   └── hakai-core/               # Main crate (binary: hakai)
│       └── src/
│           ├── main.rs           # CLI entry point, argument parsing, headless mode
│           ├── scanner.rs        # Parallel directory traversal
│           ├── sizer.rs          # Parallel size and mtime calculation
│           ├── deleter.rs        # Parallel directory deletion
│           ├── risk.rs           # Risk analysis and dead project detection
│           ├── config.rs         # .hakairc parser and built-in profiles
│           ├── util.rs           # Shared formatting utilities
│           ├── tui/
│           │   ├── mod.rs        # TUI event loop and input handling
│           │   ├── app.rs        # Application state, modes, and logic
│           │   ├── ui.rs         # ratatui widget rendering
│           │   └── theme.rs      # Colors, branding, and Gojo quotes
│           └── platform/
│               ├── mod.rs        # Platform detection
│               ├── windows.rs    # Long paths, junctions, readonly clearing
│               └── unix.rs       # Symlinks, permission handling
```

## Risk Analysis

Hakai evaluates each discovered directory and flags potential risks:

| Indicator   | Meaning                                                                    |
| ----------- | -------------------------------------------------------------------------- |
| ⚠ (red)     | **High risk** — system-level location (`Program Files`, `/usr/lib`)        |
| ⚡ (yellow) | **Medium risk** — global package manager location (`.nvm`, `.volta`)       |
| ☠ (yellow)  | **Orphaned** — `node_modules` whose parent has no `package.json`           |
| _(none)_    | **Low risk** — standard project directory, safe to delete                  |

## Compatibility

| Platform        | Status                                                                 |
| --------------- | ---------------------------------------------------------------------- |
| Windows 10/11   | Full support — CMD, PowerShell, Windows Terminal, Git Bash             |
| macOS 12+       | Full support                                                           |
| Linux           | Full support on any modern distribution                                |

Windows long paths (> 260 characters) are handled automatically via the `\\?\` UNC prefix. Read-only files inside `node_modules` (common with npm) are detected and permissions are cleared before deletion.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, architecture details, and guidelines.

## License

[MIT](LICENSE) © hakai contributors
