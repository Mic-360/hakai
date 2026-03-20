.PHONY: build release clean test

# Build the hakai binary
build:
	cd crates/hakai-core && cargo build --release

# Full release: copy binary to dist/
release: build
	@mkdir -p dist
ifeq ($(OS),Windows_NT)
	copy crates\hakai-core\target\release\hakai.exe dist\hakai.exe
else
	cp crates/hakai-core/target/release/hakai dist/hakai
endif
	@echo "🦀 Release built in dist/"

# Windows cross-compile from Linux/Mac
build-windows:
	cd crates/hakai-core && cargo build --release --target x86_64-pc-windows-gnu

# Run tests
test:
	cd crates/hakai-core && cargo test

# Clean all build artifacts
clean:
	cargo clean
	rm -rf dist/
