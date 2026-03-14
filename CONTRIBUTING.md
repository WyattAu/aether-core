# Contributing to Aether

## Development Setup

1. Install Nix (recommended) or use the provided Dockerfile
2. Run `direnv allow` to load the development environment
3. Build: `cargo build --workspace`
4. Test: `cargo test --workspace`

## Code Standards

- **No unwrap/expect**: Use Result types everywhere
- **No panics**: Compile with `panic = "abort"`
- **Tests required**: All new code needs tests
- **Clippy clean**: No clippy warnings

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run `cargo test && cargo clippy`
5. Submit PR

## Architecture Decisions

All major decisions are documented in `.adrs/`.
