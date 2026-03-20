# Contributing to hakai

Thank you for your interest in contributing to hakai. This guide covers everything you need to get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Project Architecture](#project-architecture)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Style Guide](#style-guide)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project follows the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) code of conduct. By participating, you are expected to uphold this standard.

## Getting Started

### Prerequisites

| Tool     | Version | Install                             |
| -------- | ------- | ----------------------------------- |
| **Rust** | 1.70+   | [rustup.rs](https://rustup.rs/)     |
| **Git**  | any     | [git-scm.com](https://git-scm.com/) |

No other runtime or language toolchain is required. Hakai is a pure Rust project.

### Development Setup

```bash
git clone https://github.com/mic-360/hakai.git
cd hakai

# Debug build (faster compilation)
cargo build

# Run tests
cargo test

# Run in headless mode
cargo run -- --json -d .

# Run the interactive TUI
cargo run
```

### Quick Iteration

```bash
# Fast debug build + headless run
cargo run -- --json-stream -d ~/projects

# Run with interactive TUI
cargo run

# Run a specific test module
cargo test scanner::tests
cargo test deleter::tests

# Run a single test
cargo test scanner::tests::finds_node_modules
```

## Project Architecture

Hakai is a single-binary Rust application. The interactive TUI is built with ratatui and crossterm. All scanning, sizing, and deletion use rayon for parallelism.

```
crates/hakai-core/src/
├── main.rs           # CLI entry point, argument parsing, headless mode
├── scanner.rs        # Parallel directory traversal (ignore + rayon + crossbeam)
├── sizer.rs          # Parallel size and mtime calculation (ignore::WalkBuilder)
├── deleter.rs        # Parallel directory deletion (rayon work-stealing)
├── risk.rs           # Risk analysis (dead project detection, system path flagging)
├── config.rs         # .hakairc TOML parser and built-in profiles
├── util.rs           # Shared formatting utilities
├── tui/
│   ├── mod.rs        # TUI event loop, input polling, scanner thread coordination
│   ├── app.rs        # Application state machine, modes, selection logic
│   ├── ui.rs         # ratatui widget rendering (header, results, status bar)
│   └── theme.rs      # Color palette, branding, Gojo Satoru quotes
└── platform/
    ├── mod.rs        # Platform detection and dispatch
    ├── windows.rs    # \\?\ long paths, junction detection, readonly clearing
    └── unix.rs       # Symlink detection, permission handling
```

### How it works

1. `main.rs` parses CLI arguments and loads configuration from `.hakairc`
2. If headless flags are present (`--json`, `--json-stream`, `--delete-all`), the scan runs directly with results written to stdout
3. Otherwise, the ratatui TUI starts with three concurrent threads:
   - **Input thread** — polls keyboard and mouse events via crossterm
   - **Scanner thread** — runs `scanner::scan_parallel()` and sends `ScanEvent`s over a crossbeam channel
   - **Main thread** — processes events, updates application state, and renders the UI
4. As directories are discovered, rayon worker threads calculate sizes and risk levels concurrently
5. Deletion uses rayon's work-stealing scheduler to recursively remove files across all cores

### Key Design Decisions

- **`ignore::WalkBuilder` for scanning** — provides parallel directory traversal with automatic `.gitignore` respect and work-stealing across all CPU cores
- **Pruning on match** — when a target directory is found, hakai does not recurse into it
- **Crossbeam channels for event streaming** — lock-free, bounded channels connect the scanner, sizer, and UI threads
- **Rename-to-trash deletion** — directories are atomically renamed to `.hakai_trash_*`, then cleaned up in the background — the directory disappears from the UI instantly
- **Rayon parallel recursive deletion** — subdirectories are deleted in parallel via `rayon::scope`, and file batches over 256 use `par_iter`
- **Windows `\\?\` prefix** — bypasses the 260-character MAX_PATH limit for all filesystem operations

## Making Changes

### Where to make changes

| I want to...                       | File(s) to edit                           |
| ---------------------------------- | ----------------------------------------- |
| Add a new CLI flag                 | `main.rs` (Args struct + handling)        |
| Change scanning behavior           | `scanner.rs`                              |
| Fix size calculation               | `sizer.rs`                                |
| Fix deletion issues                | `deleter.rs`                              |
| Add or change risk analysis rules  | `risk.rs`                                 |
| Add a built-in profile             | `config.rs` (`builtin_profiles` function) |
| Fix Windows-specific bugs          | `platform/windows.rs`                     |
| Fix Unix-specific bugs             | `platform/unix.rs`                        |
| Change TUI rendering               | `tui/ui.rs`                               |
| Change TUI keybindings or behavior | `tui/mod.rs` (input) + `tui/app.rs`      |
| Change TUI colors or branding      | `tui/theme.rs`                            |

### Branch Naming

```
feat/description     # New feature
fix/description      # Bug fix
docs/description     # Documentation
refactor/description # Code refactoring
perf/description     # Performance improvement
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific module's tests
cargo test scanner::tests
cargo test deleter::tests
cargo test risk::tests
cargo test config::tests
cargo test sizer::tests
```

### What to Test

- **Scanner** — directory traversal, target matching, pruning, exclusions, hidden directory handling
- **Sizer** — byte count accuracy, symlink handling, empty directories
- **Deleter** — actual deletion, dry-run behavior, partial failure handling, batch operations
- **Risk** — orphan detection, system path detection, risk level assignment
- **Config** — TOML parsing, profile resolution, default values

### Writing Tests

Tests use `tempfile::TempDir` to create isolated directory structures:

```rust
#[test]
fn my_scanner_test() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    std::fs::create_dir_all(root.join("project/node_modules")).unwrap();
    std::fs::create_dir_all(root.join("project/src")).unwrap();

    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel = Arc::new(AtomicBool::new(false));
    let opts = ScanOptions {
        root: root.to_path_buf(),
        targets: vec!["node_modules".into()],
        exclude: vec![],
        exclude_hidden: false,
        max_depth: None,
    };

    scanner::scan_parallel(&opts, &tx, cancel);
    drop(tx);

    let found: Vec<_> = rx.iter()
        .filter_map(|e| match e {
            ScanEvent::Found { path } => Some(path),
            _ => None,
        })
        .collect();

    assert_eq!(found.len(), 1);
}
```

### Manual Testing

```bash
# Verify scanning
cargo run -- --json -d /tmp/test-dir

# Verify dry-run (nothing deleted)
cargo run -- --delete-all --dry-run -d /tmp/test-dir

# Verify profiles
cargo run -- --json --profile rust -d ~/projects
cargo run -- --json --profile python -d ~/projects

# Verify Windows paths (on Windows)
cargo run -- --json -d "C:\Users\me\a path with spaces"
```

## Pull Request Process

1. **Fork the repository** and create your branch from `main`
2. **Make your changes** with clear, focused commits
3. **Add tests** for any new functionality
4. **Run `cargo test`** and ensure all tests pass
5. **Run `cargo clippy`** and fix any warnings
6. **Update documentation** if your change affects user-facing behavior
7. **Open a PR** with a clear description of what and why

### PR Description Template

```markdown
## What

Brief description of the change.

## Why

Why is this change needed?

## How

How does the implementation work? Any design decisions worth noting?

## Testing

How was this tested? Any new tests added?
```

### Review Criteria

- Does it compile on all platforms (Windows, macOS, Linux)?
- Are there tests for new functionality?
- Does it maintain backward compatibility with `--json` output?
- Is the code clear and well-structured?
- Does it handle errors gracefully?

## Style Guide

- Follow standard Rust conventions (`cargo fmt`, `cargo clippy`)
- Use `anyhow::Result` for error handling in application code
- Prefer `PathBuf` and `&Path` over string manipulation for file paths
- Use `#[cfg(windows)]` / `#[cfg(unix)]` for platform-specific code
- Keep functions focused — if a function exceeds 50 lines, consider splitting it
- Use meaningful variable names over abbreviations

### Commit Messages

Use conventional commits:

```
feat: add --min-size flag to filter by directory size
fix: handle permission errors on Windows junction points
docs: add troubleshooting section to INSTALLATION.md
perf: use rayon par_iter for size calculation of large dirs
refactor: extract path normalization into platform module
test: add tests for orphaned node_modules detection
```

## Reporting Issues

### Bug Reports

Please include:

1. **OS and version** (e.g., Windows 11, macOS 14.2, Ubuntu 24.04)
2. **Terminal** (e.g., Windows Terminal, Git Bash, iTerm2, Ghostty)
3. **hakai version** (`hakai --version`)
4. **Command you ran** (the full command line)
5. **Expected behavior** vs **actual behavior**
6. **Error output** (if any — run with `2>error.log` to capture stderr)

### Feature Requests

Open an issue describing:

1. **What** you want hakai to do
2. **Why** — the use case or problem it solves
3. **How** it might work (optional)

### Security Issues

If you discover a security vulnerability, please **do not** open a public issue. Instead, email the maintainers directly.

## Areas Looking for Help

- **macOS testing** — especially Apple Silicon edge cases
- **Linux packaging** — apt, dnf, pacman, nix packages
- **New profiles** — language and framework-specific cleanup profiles
- **Performance** — faster Windows directory enumeration via Win32 APIs
- **Accessibility** — screen reader support, high-contrast themes
