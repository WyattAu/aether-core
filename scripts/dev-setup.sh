#!/bin/bash
# Development Environment Setup

set -e

echo "🛡️  Setting up Aether development environment..."

# Check Rust version
echo "Checking Rust toolchain..."
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi

# Add WASM target
echo "Adding WASM target..."
rustup target add wasm32-wasip1

# Install components
echo "Installing Rust components..."
rustup component add clippy rustfmt rust-src

# Install cargo tools
echo "Installing cargo tools..."
cargo install cargo-nextest cargo-audit cargo-mutants

# Build
echo "Building workspace..."
cargo build --workspace

# Run tests
echo "Running tests..."
cargo test --workspace

echo "✅ Development environment ready!"
echo ""
echo "Next steps:"
echo "  make dev      - Start development server"
echo "  make test     - Run tests"
echo "  make build-wasm - Build WASM examples"
