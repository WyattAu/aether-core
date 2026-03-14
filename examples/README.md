# Aether Example Actors

This directory contains example actors demonstrating Aether features.

## Examples

### hello-actor
Basic hello world actor.

```bash
cd examples/hello-actor
cargo build --release --target wasm32-wasip1
aether deploy
```

### stateful-actor
Actor with persistent state.

```bash
cd examples/stateful-actor
cargo build --release --target wasm32-wasip1
aether deploy
```

## Building All Examples

```bash
./build-examples.sh
```
