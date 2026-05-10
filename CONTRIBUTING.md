# Contributing to Aether

Thank you for your interest in contributing to Project Aether! This document provides guidelines and instructions for contributing.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Development Setup](#development-setup)
3. [Code Standards](#code-standards)
4. [Testing Guidelines](#testing-guidelines)
5. [Pull Request Process](#pull-request-process)
6. [Commit Guidelines](#commit-guidelines)
7. [Architecture Decisions](#architecture-decisions)
8. [Getting Help](#getting-help)

---

## Code of Conduct

Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md). Be respectful, inclusive, and constructive in all interactions.

---

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Nix | 2.18+ | Reproducible build environment |
| Rust | 1.85+ | Primary language (MSRV) |
| FoundationDB | 7.3+ | State backend (optional) |

### Quick Start with Nix (Recommended)

```bash
# 1. Install Nix (if not already installed)
curl -L https://nixos.org/nix/install | sh

# 2. Enable flakes
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf

# 3. Clone the repository
git clone https://github.com/WyattAu/aether-core.git
cd aether-core

# 4. Enter development environment
nix develop

# 5. Build the project
cargo build --workspace

# 6. Run tests
cargo test --workspace
```

### Alternative: Docker Setup

```bash
# Build development container
docker build -t aether-dev -f Dockerfile.dev .

# Run with mounted source
docker run -it -v $(pwd):/workspace aether-dev
```

### IDE Setup

#### VS Code

Recommended extensions:
- `rust-analyzer` - Rust language server
- `CodeLLDB` - Debugging
- `Even Better TOML` - Configuration files
- `Markdown All in One` - Documentation

#### JetBrains IDEs

Install the Rust plugin and enable:
- Rustfmt on save
- Clippy on-the-fly

---

## Code Standards

### Error Handling

**Always use `Result<T>` - never panic!**

```rust
// [PASS] Correct
fn parse_config(input: &str) -> Result<Config> {
    toml::from_str(input)
        .map_err(|e| Error::internal(format!("Invalid config: {}", e)))
}

// [FAIL] Wrong - will fail CI
fn parse_config(input: &str) -> Config {
    toml::from_str(input).expect("Invalid config")  // Fails clippy!
}
```

### Error Type Selection

Use the appropriate error constructor:

| Method | Use Case |
|--------|----------|
| `Error::internal()` | General internal errors |
| `Error::storage_read()` | Failed to read from storage |
| `Error::storage_write()` | Failed to write to storage |
| `Error::mesh_connection()` | Network/mesh failures |

```rust
// [PASS] Correct
std::fs::read(path)
    .map_err(|e| Error::storage_read(format!("Failed to read {}: {}", path, e)))?;

// [FAIL] Wrong
std::fs::read(path)
    .map_err(|e| Error::io(e))?;  // No Error::io() method exists!
```

### Capability Checks

Always check capabilities before operations:

```rust
// [PASS] Correct
if !ctx.has_fs_read() {
    return Err(Error::internal("Missing fs_read capability"));
}

// [FAIL] Wrong - will fail at runtime
std::fs::read_to_string(path)?;  // No capability check!
```

### Metadata Access

Session metadata is wrapped in `RwLock`:

```rust
// [PASS] Correct
let metadata = session.metadata.read();
let actor_id = metadata.actor_id.clone();

// [FAIL] Wrong - compile error
let actor_id = session.metadata.actor_id;  // Can't access directly!
```

### MemoryEntry Tags

Use `Vec::push()` for tags (no `with_tag()` method):

```rust
// [PASS] Correct
let mut entry = MemoryEntry::new(data);
entry.tags.push("cache".to_string());
entry.tags.push("session".to_string());

// [FAIL] Wrong - method doesn't exist
let entry = MemoryEntry::new(data).with_tag("cache");  // No such method!
```

### MessageRole String Conversion

Use match statement (no Display implementation):

```rust
// [PASS] Correct
let role_str = match message.role {
    MessageRole::System => "system",
    MessageRole::User => "user",
    MessageRole::Assistant => "assistant",
    MessageRole::Tool => "tool",
};

// [FAIL] Wrong - compile error
let role_str = message.role.to_string();  // No Display impl!
```

### MCP Tool Results

Use helper methods:

```rust
// [PASS] Correct
impl McpTool for MyTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let data = do_something()?;
        ToolResult::text(serde_json::to_string(&data)?)
    }
}

// [FAIL] Wrong
impl McpTool for MyTool {
    async fn execute(&self, args: Value) -> Result<ToolResult> {
        Ok(ToolResult {  // Manual construction error-prone
            content: vec![Content::Text { text: "..." }],
            is_error: false,
        })
    }
}
```

### Clippy Rules

The project enforces strict clippy rules:

```rust
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(missing_docs)]
```

Run before committing:

```bash
cargo clippy --workspace --all-features -- -D warnings
```

---

## Testing Guidelines

### Unit Tests

All new code requires unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_greeting_actor() {
        let mut actor = GreetingActor::new();
        let ctx = TestContext::new();
        
        let response = actor.handle(&ctx, Greet {
            name: "Test".to_string(),
        }).await.unwrap();
        
        assert_eq!(response.count, 1);
        assert!(response.message.contains("Test"));
    }
}
```

### Integration Tests

Place in `tests/` directory:

```rust
// tests/integration_test.rs
use aether_core::*;

#[tokio::test]
async fn test_actor_mesh_communication() {
    let node = setup_test_node().await;
    
    // Deploy actors
    let actor_a = node.deploy("actor_a.wasm").await.unwrap();
    let actor_b = node.deploy("actor_b.wasm").await.unwrap();
    
    // Test communication
    let response = node.call(actor_a, "Ping", json!({})).await.unwrap();
    assert_eq!(response["status"], "ok");
}
```

### Test Coverage

Run coverage report:

```bash
cargo tarpaulin --workspace --all-features --out Html
```

Target: >80% overall, >95% on critical paths.

---

## Pull Request Process

### Before Submitting

1. **Update from main**
   ```bash
   git fetch origin
   git rebase origin/main
   ```

2. **Run all checks**
   ```bash
   cargo fmt --check
   cargo clippy --workspace --all-features -- -D warnings
   cargo test --workspace --all-features
   ```

3. **Update documentation**
   - Add/update doc comments
   - Update relevant `.docs/` files
   - Update CHANGELOG.md if applicable

### PR Title Format

Use conventional commits:

| Type | Example |
|------|---------|
| `feat` | `feat(actor): Add scheduled task support` |
| `fix` | `fix(mesh): Handle connection timeout correctly` |
| `docs` | `docs: Update getting started guide` |
| `test` | `test(state): Add transaction rollback tests` |
| `refactor` | `refactor(ai): Simplify provider trait` |
| `chore` | `chore: Update dependencies` |

### PR Description Template

```markdown
## Summary
Brief description of changes.

## Changes
- Change 1
- Change 2

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed

## Documentation
- [ ] Doc comments updated
- [ ] `.docs/` files updated (if applicable)

## Breaking Changes
- List any breaking changes or "None"
```

### Review Process

1. At least 1 approval required
2. All CI checks must pass
3. No merge conflicts
4. Squash and merge to main

---

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Example

```
feat(actor): Add scheduled task support

Implements periodic task scheduling for actors with configurable
intervals and retry policies.

- Add ScheduleType enum (Once, Periodic, Cron)
- Add schedule() method to ActorContext
- Add cancel_schedule() for task cancellation
- Add unit and integration tests

Closes #123
```

### Commit Best Practices

- Keep commits atomic (one logical change)
- Write clear, descriptive messages
- Reference issues when applicable
- Keep lines under 72 characters

---

## Architecture Decisions

### ADR Process

All major architectural decisions are documented as Architecture Decision Records (ADRs) in `.adrs/`.

### When to Create an ADR

- Adding new components or modules
- Changing public APIs
- Modifying security model
- Changing performance characteristics
- Introducing new dependencies

### ADR Template

```markdown
# ADR-XXX: Title

## Status
Proposed | Accepted | Deprecated | Superseded

## Context
What is the issue we're addressing?

## Decision
What is the change we're proposing?

## Consequences
What are the positive and negative outcomes?

## Alternatives Considered
What other options did we consider?
```

---

## Getting Help

### Resources

| Resource | Link |
|----------|------|
| Documentation | `.docs/` directory |
| Architecture | `ARCHITECTURE.md` |
| API Reference | `.docs/api_reference.md` |
| Code Examples | `.docs/code_examples.md` |

### Community

| Platform | Purpose |
|----------|---------|
| [Discord](https://discord.gg/aether) | Real-time chat, questions |
| [GitHub Discussions](https://github.com/aether-project/aether/discussions) | Long-form discussions |
| [GitHub Issues](https://github.com/aether-project/aether/issues) | Bug reports, features |

### Good First Issues

Look for issues labeled `good first issue` or `help wanted` on GitHub.

---

## License

By contributing to Aether, you agree that your contributions will be licensed under the project's license (see LICENSE file).
