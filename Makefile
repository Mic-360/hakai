.PHONY: build build-rust build-bun release clean test

# Build everything
build: build-rust build-bun

# Build Rust core
build-rust:
	cd crates/hakai-core && cargo build --release

# Build Bun TUI as self-contained binary
build-bun:
	cd packages/hakai-tui && bun build --compile --target=bun src/index.ts --outfile=../../dist/hakai-tui

# Full release: copy both binaries to dist/
release: build
	@mkdir -p dist
ifeq ($(OS),Windows_NT)
	copy crates\hakai-core\target\release\hakai.exe dist\hakai.exe
else
	cp crates/hakai-core/target/release/hakai dist/hakai
endif
	@echo "💀 Release built in dist/"

# Windows cross-compile from Linux/Mac
build-windows:
	cd crates/hakai-core && cargo build --release --target x86_64-pc-windows-gnu
	cd packages/hakai-tui && bun build --compile --target=bun-windows-x64 src/index.ts --outfile=../../dist/hakai-tui.exe

# Run tests
test:
	cd crates/hakai-core && cargo test

# Clean all build artifacts
clean:
	cargo clean
	rm -rf dist/
