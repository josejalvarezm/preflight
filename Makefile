.PHONY: build test compile-instructions clean check fmt

# Build the entire workspace
build:
	cargo build

# Run all tests
test:
	cargo test

# Run clippy lints
check:
	cargo clippy --workspace -- -D warnings

# Format all code
fmt:
	cargo fmt --all

# Compile instruction files into a contract manifest
compile-instructions:
	cargo run -p ai-os-compiler -- compile .instructions/ .instructions/contracts/contract.json

# Clean build artefacts
clean:
	cargo clean
