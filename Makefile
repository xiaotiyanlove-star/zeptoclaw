.PHONY: build build-release install clean test lint fmt check all

BINARY_NAME := picoclaw
BUILD_DIR := target
INSTALL_DIR := $(HOME)/.local/bin

# Default target
all: lint test build

# Development build
build:
	cargo build

# Optimized release build
build-release:
	cargo build --release

# Install binary to ~/.local/bin
install: build-release
	mkdir -p $(INSTALL_DIR)
	cp $(BUILD_DIR)/release/$(BINARY_NAME) $(INSTALL_DIR)/
	@echo "Installed $(BINARY_NAME) to $(INSTALL_DIR)"

# Uninstall binary
uninstall:
	rm -f $(INSTALL_DIR)/$(BINARY_NAME)
	@echo "Removed $(BINARY_NAME) from $(INSTALL_DIR)"

# Run all tests
test:
	cargo test

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Lint with clippy
lint:
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt

# Check formatting without modifying
fmt-check:
	cargo fmt -- --check

# Full check (format, lint, test)
check: fmt-check lint test

# Clean build artifacts
clean:
	cargo clean

# Show release binary size
size: build-release
	@ls -lh $(BUILD_DIR)/release/$(BINARY_NAME)
	@echo ""
	@file $(BUILD_DIR)/release/$(BINARY_NAME)

# Run the agent with a test message
run:
	cargo run -- agent -m "Hello, what can you do?"

# Run in interactive mode
run-interactive:
	cargo run -- agent

# Run gateway
run-gateway:
	cargo run -- gateway

# Show help
help:
	@echo "PicoClaw Rust - Build Commands"
	@echo ""
	@echo "Development:"
	@echo "  make build          - Debug build"
	@echo "  make build-release  - Optimized release build"
	@echo "  make run            - Run agent with test message"
	@echo "  make run-interactive- Run interactive agent"
	@echo ""
	@echo "Quality:"
	@echo "  make test           - Run all tests"
	@echo "  make lint           - Run clippy linter"
	@echo "  make fmt            - Format code"
	@echo "  make check          - Full quality check"
	@echo ""
	@echo "Installation:"
	@echo "  make install        - Install to ~/.local/bin"
	@echo "  make uninstall      - Remove from ~/.local/bin"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean          - Remove build artifacts"
	@echo "  make size           - Show release binary size"
	@echo "  make help           - Show this help"
