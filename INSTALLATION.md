# Installation Guide

## Quick Install

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/mic-360/hakai/main/install.ps1 | iex
```

Or download and run manually:

```powershell
.\install.ps1
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/mic-360/hakai/main/install.sh | bash
```

Or download and run manually:

```bash
chmod +x install.sh
./install.sh
```

---

## Build from Source

Building from source gives you the latest version and works on any platform that Rust and Bun support.

### Prerequisites

| Tool     | Version | Install                             |
| -------- | ------- | ----------------------------------- |
| **Rust** | 1.70+   | [rustup.rs](https://rustup.rs/)     |
| **Bun**  | 1.0+    | [bun.sh](https://bun.sh/)           |
| **Git**  | any     | [git-scm.com](https://git-scm.com/) |

### Step 1: Clone

```bash
git clone https://github.com/mic-360/hakai.git
cd hakai
```

### Step 2: Build

**Using Make (recommended):**

```bash
make build
```

This builds the Rust binary in release mode and installs TUI dependencies.

**Manual build:**

```bash
# Build Rust core (release mode)
cargo build --release

# Install Bun TUI dependencies
cd packages/hakai-tui
bun install
cd ../..
```

The binary will be at `target/release/hakai` (or `target/release/hakai.exe` on Windows).

### Step 3: Install

**Using Make:**

```bash
make install
```

**Manual install (add to PATH):**

Windows (PowerShell):

```powershell
# Create directory and copy binary
New-Item -ItemType Directory -Force -Path "$env:LOCALAPPDATA\hakai\bin"
Copy-Item "target\release\hakai.exe" "$env:LOCALAPPDATA\hakai\bin\hakai.exe"

# Add to PATH (current user)
$path = [Environment]::GetEnvironmentVariable("Path", "User")
if ($path -notlike "*hakai*") {
    [Environment]::SetEnvironmentVariable("Path", "$path;$env:LOCALAPPDATA\hakai\bin", "User")
}
```

macOS / Linux:

```bash
sudo cp target/release/hakai /usr/local/bin/hakai
```

### Step 4: Verify

```bash
hakai --version
# hakai 1.0.0
```

---

## Headless Mode (No Bun Required)

If you only need the CLI without the interactive TUI, you can skip installing Bun entirely. The headless flags work with just the Rust binary:

```bash
# Build only the Rust binary
cargo build --release

# Use headless mode
hakai --json -d ~/projects
hakai --json-stream -d ~/projects
hakai --delete-all -d ~/projects
```

These modes output JSON to stdout and don't require the TUI at all.

---

## Makefile Targets

| Target         | Description                      |
| -------------- | -------------------------------- |
| `make build`   | Build both Rust core and Bun TUI |
| `make install` | Build and install to system PATH |
| `make clean`   | Remove build artifacts           |
| `make test`    | Run all Rust tests               |

---

## Configuration

After installing, you can optionally create a config file:

**Per-project config:**

```bash
cp .hakairc.example .hakairc
```

**Global config:**

```bash
# Linux / macOS
cp .hakairc.example ~/.hakairc

# Windows
copy .hakairc.example %USERPROFILE%\.hakairc
```

See [.hakairc.example](.hakairc.example) for all available options.

---

## Updating

```bash
cd hakai
git pull
make build
make install
```

---

## Uninstalling

**Windows:**

```powershell
Remove-Item "$env:LOCALAPPDATA\hakai" -Recurse -Force
# Remove from PATH manually via System Settings > Environment Variables
```

**macOS / Linux:**

```bash
sudo rm /usr/local/bin/hakai
rm -rf ~/.hakai
```

---

## Troubleshooting

### "command not found: hakai"

The binary isn't in your PATH. Either:

- Run `make install` to install it
- Add `target/release/` to your PATH manually
- Use the full path: `./target/release/hakai`

### "TUI not found" / hakai runs in headless mode unexpectedly

The Bun TUI binary can't be found. Make sure:

1. Bun is installed (`bun --version`)
2. TUI dependencies are installed (`cd packages/hakai-tui && bun install`)
3. The `hakai-tui` directory is in the expected location relative to the binary

### Permission denied on deletion (Linux/macOS)

Some directories may have restricted permissions. hakai attempts to fix permissions before deletion, but if you're cleaning directories created by other users:

```bash
sudo hakai --delete-all -d /path/to/dir
```

### Slow scanning on Windows network drives

Network drives have high latency per filesystem call. Use exclusions to skip known irrelevant subtrees:

```bash
hakai -d Z:\projects --exclude vendor,dist
```

### "Access is denied" on Windows

Some `node_modules` directories contain read-only files (e.g., from npm). hakai clears read-only attributes automatically, but if it still fails, try running from an elevated terminal (Administrator).

### Build fails with "linker not found"

On Linux, you may need build essentials:

```bash
# Ubuntu/Debian
sudo apt install build-essential

# Fedora
sudo dnf install gcc

# Arch
sudo pacman -S base-devel
```

### Build fails with Windows crate errors on Linux/macOS

This is expected — the `windows` crate dependency is conditionally compiled. Make sure you're building with:

```bash
cargo build --release
```

The `Cargo.toml` uses `[target.'cfg(windows)'.dependencies]` to only include the Windows crate on Windows builds.
