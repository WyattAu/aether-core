# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for Project Aether.

## What is an ADR?

An Architecture Decision Record (ADR) captures a significant architectural decision along with its context and consequences. ADRs help teams understand the "why" behind architectural choices.

## ADR Format

Each ADR follows this structure:
- **Title**: A short noun phrase describing the decision
- **Status**: Proposed, Accepted, Deprecated, Superseded
- **Context**: The issue motivating this decision
- **Decision**: The change being proposed or made
- **Consequences**: What becomes easier or harder as a result
- **References**: Related documents, papers, or discussions

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [ADR-001](ADR-001-dual-runtime.md) | Dual Runtime Architecture | Accepted | 2026-03-05 |
| [ADR-002](ADR-002-deny-by-default.md) | Deny-by-Default Capability Model | Accepted | 2026-03-05 |
| [ADR-003](ADR-003-panic-abort.md) | Panic Abort Policy | Accepted | 2026-03-05 |
| [ADR-004](ADR-004-wasmtime-selection.md) | Wasmtime WASM Runtime Selection | Accepted | 2026-03-05 |
| [ADR-005](ADR-005-firecracker-selection.md) | Firecracker VMM Selection | Accepted | 2026-03-05 |

## Decision Categories

### Runtime & Execution
- **ADR-001**: Dual runtime (Monoio + Tokio)
- **ADR-004**: WASM runtime choice (wasmtime)

### Security
- **ADR-002**: Capability security model

### Reliability
- **ADR-003**: Panic handling policy

### Infrastructure
- **ADR-005**: Virtualization choice (Firecracker)

## Creating a New ADR

1. Copy the template: `ADR-NNN-template.md`
2. Fill in the sections
3. Submit for review
4. Update this index

## ADR Lifecycle

```
Proposed → Accepted → (Optional) Deprecated → (Optional) Superseded
```

- **Proposed**: Under discussion
- **Accepted**: Approved and in effect
- **Deprecated**: No longer recommended for new use
- **Superseded**: Replaced by a newer ADR

## References

- [Documenting Architecture Decisions - Michael Nygard](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [Architecture Decision Records - GitHub](https://adr.github.io/)
- [When to use ADRs](https://github.com/npryce/adr-tools)

## Related Documents

- [Yellow Papers](../.specs/01_research/) - Theoretical foundations
- [Blue Papers](../.specs/02_architecture/) - Architectural specifications
- [Requirements](../.specs/00_requirements/) - System requirements
