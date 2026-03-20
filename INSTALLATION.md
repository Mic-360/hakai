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

### Prerequisites

| Tool     | Version | Install                             |
| -------- | ------- | ----------------------------------- |
| **Rust** | 1.70+   | [rustup.rs](https://rustup.rs/)     |
| **Git**  | any     | [git-scm.com](https://git-scm.com/) |

No other runtime or toolchain is required. Hakai is a pure Rust project.

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

**Manual build:**

```bash
cargo build --release
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

## Makefile Targets

| Target           | Description                           |
| ---------------- | ------------------------------------- |
| `make build`     | Build the Rust binary in release mode |
| `make install`   | Build and install to system PATH      |
| `make test`      | Run all tests                         |
| `make clean`     | Remove build artifacts                |

---

## Configuration

After installing, you can optionally create a configuration file:

**Global config:**

```bash
# Linux / macOS
cp .hakairc.example ~/.hakairc

# Windows
copy .hakairc.example %USERPROFILE%\.hakairc
```

**Per-project config:**

```bash
cp .hakairc.example .hakairc
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

The binary is not in your PATH. Either:

- Run `make install` to install it
- Add `target/release/` to your PATH manually
- Use the full path: `./target/release/hakai`

### Permission denied on deletion (Linux/macOS)

Some directories may have restricted permissions. Hakai clears permissions before deletion when possible, but directories created by other users may require elevated privileges:

```bash
sudo hakai --delete-all -d /path/to/dir
```

### Slow scanning on network drives

Network drives have high latency per filesystem call. Use exclusions to skip irrelevant subtrees:

```bash
hakai -d Z:\projects --exclude vendor,dist
```

### "Access is denied" on Windows

Some `node_modules` directories contain read-only files (common with npm). Hakai clears read-only attributes automatically, but if deletion still fails, try running from an elevated terminal (Run as Administrator).

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

This is expected. The `windows` crate is conditionally compiled and only included on Windows builds. Standard `cargo build --release` handles this automatically via `[target.'cfg(windows)'.dependencies]` in `Cargo.toml`.
