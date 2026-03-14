#!/bin/bash
set -e

echo "Building example actors..."

for dir in examples/*/; do
    if [ -f "$dir/Cargo.toml" ]; then
        echo "Building $(basename $dir)..."
        cd "$dir"
        cargo build --release --target wasm32-wasip1
        cd - > /dev/null
    fi
done

echo "All examples built!"
