# KMS Makefile - Integration with OP-TEE build system

# This Makefile will be expanded to integrate with the existing OP-TEE
# build system once eth_wallet code is integrated

.PHONY: all build test clean help

all: build

build:
	@echo "Building KMS workspace..."
	cargo build --workspace

test:
	@echo "Running tests..."
	cargo test --workspace

clean:
	@echo "Cleaning build artifacts..."
	cargo clean

check:
	@echo "Running cargo check..."
	cargo check --workspace

fmt:
	@echo "Formatting code..."
	cargo fmt --all

clippy:
	@echo "Running clippy..."
	cargo clippy --workspace -- -D warnings

help:
	@echo "Available targets:"
	@echo "  build   - Build all KMS components"
	@echo "  test    - Run tests"
	@echo "  clean   - Clean build artifacts"
	@echo "  check   - Run cargo check"
	@echo "  fmt     - Format code"
	@echo "  clippy  - Run clippy linter"
	@echo "  help    - Show this help"