# Contributing to hakai

Thank you for your interest in contributing to hakai! This guide will help you get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Architecture](#project-architecture)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Style Guide](#style-guide)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

This project follows the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/) code of conduct. By participating, you are expected to uphold this standard. Be respectful, constructive, and inclusive.

## Getting Started

### Prerequisites

- **Rust** 1.70+ — [Install via rustup](https://rustup.rs/)
- **Bun** 1.0+ — [Install from bun.sh](https://bun.sh/)
- **Git**

### Development Setup

```bash
# Clone the repo
git clone https://github.com/mic-360/hakai.git
cd hakai

# Build the Rust core (debug mode for faster compilation)
cargo build

# Install Bun TUI dependencies
cd packages/hakai-tui
bun install
cd ../..

# Run tests
cargo test

# Run hakai in headless mode (no TUI needed)
cargo run -- --json -d .
```

### Quick Iteration

```bash
# Fast debug build + run
cargo run -- --json-stream -d ~/projects

# Run with TUI (requires bun and built hakai-tui)
cargo run

# Run just the Rust tests
cargo test

# Run a specific test
cargo test scanner::tests::finds_node_modules
```

## Project Architecture

hakai is a hybrid Rust + Bun application:

```
hakai/
├── crates/hakai-core/        # Rust: filesystem engine (this is where most logic lives)
│   └── src/
│       ├── main.rs           # CLI parsing, headless mode, TUI spawning
│       ├── scanner.rs        # Parallel directory traversal (rayon + crossbeam)
│       ├── sizer.rs          # File size calculation
│       ├── deleter.rs        # Async directory deletion (tokio)
│       ├── risk.rs           # Risk analysis (orphaned dirs, system paths)
│       ├── ipc.rs            # JSON IPC protocol (stdin/stdout)
│       ├── config.rs         # .hakairc config parsing (TOML)
│       └── platform/         # OS-specific code
│           ├── windows.rs    # Long paths, junction points, readonly clearing
│           └── unix.rs       # Symlink handling, permissions
│
└── packages/hakai-tui/       # Bun: interactive terminal UI
    └── src/
        ├── index.ts          # Entry point
        ├── app.ts            # App state machine, key handling, IPC event routing
        ├── ipc.ts            # IPC client (spawns hakai-core --ipc)
        ├── renderer.ts       # Diff-based ANSI renderer
        ├── input.ts          # Raw keyboard input handler
        ├── components/       # UI components (header, results list, etc.)
        └── constants/        # Colors, keybinds, CLI constants
```

### How the two halves communicate

1. The user runs `hakai` (the Rust binary)
2. Rust parses CLI args and spawns `hakai-tui` (the Bun binary)
3. The TUI spawns a second `hakai --ipc` process
4. They communicate via newline-delimited JSON over stdin/stdout
5. Rust does the heavy lifting (scan, size, delete), TUI does rendering

For headless mode (`--json`, `--json-stream`, `--delete-all`), the TUI is not involved at all — Rust handles everything directly.

### Key Design Decisions

- **Scanner uses `WalkDir` + rayon** — top-level subdirectories are dispatched to rayon's thread pool for parallel traversal
- **Pruning on match** — when a target dir (e.g. `node_modules`) is found, hakai does NOT recurse into it
- **Size calculation is concurrent with scanning** — as dirs are found, they're immediately sent for size calculation on separate threads
- **Diff-based rendering** — only changed terminal lines are redrawn, preventing flicker
- **Alternative screen buffer** — TUI uses `\x1b[?1049h` so your terminal content is preserved when hakai exits

## Making Changes

### Where to make changes

| I want to...                   | File(s) to edit                    |
| ------------------------------ | ---------------------------------- |
| Add a new CLI flag             | `main.rs` (Args struct + handling) |
| Change scanning behavior       | `scanner.rs`                       |
| Fix size calculation           | `sizer.rs`                         |
| Fix deletion issues            | `deleter.rs`                       |
| Add/change risk analysis rules | `risk.rs`                          |
| Add a built-in profile         | `config.rs` (builtin_profiles fn)  |
| Fix Windows-specific bugs      | `platform/windows.rs`              |
| Change IPC protocol            | `ipc.rs` (Rust) + `ipc.ts` (Bun)   |
| Change TUI rendering           | `renderer.ts` or `components/`     |
| Change keybinds                | `constants/keybinds.ts`            |
| Change TUI behavior/state      | `app.ts`                           |

### Branch Naming

```
feat/description     # New feature
fix/description      # Bug fix
docs/description     # Documentation
refactor/description # Code refactoring
perf/description     # Performance improvement
```

## Testing

### Rust Tests

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

- **Scanner**: Directory traversal, pruning, exclusions, hidden dir handling
- **Sizer**: Accurate byte counts, symlink handling, empty dirs
- **Deleter**: Actual deletion, dry-run behavior, partial failure handling, batch concurrency
- **Risk**: Orphan detection, system path detection, risk level assignment
- **Config**: TOML parsing, profile merging, default values

### Writing Tests

Tests use `tempfile::TempDir` to create isolated directory structures:

```rust
#[test]
fn my_scanner_test() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Create test directory structure
    std::fs::create_dir_all(root.join("project/node_modules")).unwrap();
    std::fs::create_dir_all(root.join("project/src")).unwrap();

    // Run scanner
    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel = AtomicBool::new(false);
    let opts = ScanOptions { /* ... */ };

    scan_parallel(&opts, &tx, &cancel);
    drop(tx);

    // Assert results
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
# Test scanning works
cargo run -- --json -d /tmp/test-dir

# Test dry-run (verify nothing is deleted)
cargo run -- --delete-all --dry-run -d /tmp/test-dir

# Test different profiles
cargo run -- --json --profile rust -d ~/projects
cargo run -- --json --profile python -d ~/projects

# Test Windows-specific scenarios (on Windows)
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

### Rust

- Follow standard Rust conventions (`cargo fmt`, `cargo clippy`)
- Use `anyhow::Result` for error handling in application code
- Prefer `PathBuf` and `&Path` over string manipulation for file paths
- Use `#[cfg(windows)]` / `#[cfg(unix)]` for platform-specific code
- Write doc comments for public functions and types
- Keep functions focused — if a function is >50 lines, consider splitting it

### TypeScript (Bun TUI)

- Use TypeScript strict mode
- No external dependencies for rendering — raw ANSI codes only
- Keep ANSI color codes in `constants/colors.ts`
- Components are pure functions: `(state, width) => string[]`
- State mutations happen only in `app.ts`

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
3. **How** it might work (optional — just your thoughts)

### Security Issues

If you discover a security vulnerability, please **do not** open a public issue. Instead, email the maintainers directly. See [LICENSE](LICENSE) for contact info.

## Areas Looking for Help

- **macOS testing** — especially Apple Silicon edge cases
- **Linux distributions** — packaging for apt, dnf, pacman, nix
- **New profiles** — language/framework-specific cleanup profiles
- **Performance** — faster Windows directory enumeration via Win32 APIs
- **Accessibility** — screen reader support, high-contrast themes
