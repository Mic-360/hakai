#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

echo "💀 Installing hakai..."

# Check prerequisites
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo is required. Install from https://rustup.rs"
    exit 1
fi

if ! command -v bun &> /dev/null; then
    echo "Error: Bun is required. Install from https://bun.sh"
    exit 1
fi

# Build Rust core
echo "Building Rust core..."
cd crates/hakai-core
cargo build --release
cd ../..

# Build Bun TUI
echo "Building Bun TUI..."
cd packages/hakai-tui
bun build --compile --target=bun src/index.ts --outfile="../../dist/hakai-tui"
cd ../..

# Install
mkdir -p "$INSTALL_DIR"
cp crates/hakai-core/target/release/hakai "$INSTALL_DIR/hakai"
cp dist/hakai-tui "$INSTALL_DIR/hakai-tui"
chmod +x "$INSTALL_DIR/hakai" "$INSTALL_DIR/hakai-tui"

# Check if in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "Add to your shell profile:"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi

echo ""
echo "💀 hakai installed to $INSTALL_DIR"
echo "   Run: hakai"
echo "   Run: hakai --help for options"
