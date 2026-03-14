.PHONY: all build test clean run dev deploy lint

all: build

# Build all workspace crates
build:
	cargo build --workspace --all-features

# Build release
build-release:
	cargo build --workspace --release

# Build WASM examples
build-wasm:
	cd examples/hello-actor && cargo build --release --target wasm32-wasip1
	cd examples/stateful-actor && cargo build --release --target wasm32-wasip1

# Run all tests
test:
	cargo test --workspace --all-features

# Run integration tests
test-integration:
	cargo test --test integration --all-features

# Run with coverage
coverage:
	cargo llvm-cov --workspace --all-features

# Run linter
lint:
	cargo clippy --workspace --all-features -- -D warnings
	cargo fmt --all -- --check

# Format code
fmt:
	cargo fmt --all

# Run development server
dev:
	cargo run --bin aether -- dev

# Deploy
deploy:
	cargo run --bin aether -- deploy

# Clean build artifacts
clean:
	cargo clean
	rm -rf examples/*/target

# Install dependencies (Nix)
install:
	nix develop

# Docker build
docker:
	docker build -t aether:latest .

# Run benchmarks
bench:
	cargo bench --workspace

# Security audit
audit:
	cargo audit

# Generate documentation
docs:
	cargo doc --workspace --no-deps --open
