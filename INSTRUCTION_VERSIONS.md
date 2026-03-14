# Instruction Versions: Project Aether

## Agent Instruction Versioning

This document tracks versions of agent instructions used throughout the R&D lifecycle.

## Core Instructions

| Instruction Set | Version | Phase | Last Updated | Status |
|-----------------|---------|-------|--------------|--------|
| Omni-Protocol SOP | 1.0.0 | All | 2026-03-05 | Active |
| Clean Hands Protocol | 1.0.0 | -1 | 2026-03-05 | Active |
| Domain Analyst | 1.0.0 | -1 | 2026-03-05 | Active |

## Phase-Specific Instructions

| Phase | Role | Version | Status |
|-------|------|---------|--------|
| -1 | Domain Analyst | 1.0.0 | Active |
| 0 | Architect | - | Planned |
| 1 | Core Developer | - | Planned |
| 2 | Network Engineer | - | Planned |
| 3 | State Engineer | - | Planned |
| 4 | Security Engineer | - | Planned |
| 5 | Integration Engineer | - | Planned |

## Engineering Principles (Immutable)

| Principle | Version | Description |
|-----------|---------|-------------|
| Zero-Panic Policy | 1.0.0 | No use of unwrap/expect; all errors must be handled |
| No-OS Hot Path | 1.0.0 | Zero heap allocations on request path |
| Deterministic Invariants | 1.0.0 | Time/entropy injected by host |
| Hardware Sympathy | 1.0.0 | Cache-aligned to 64 bytes |
| Capability-Based Security | 1.0.0 | Deny-by-default access control |

## Version History

### 2026-03-05
- Initial version tracking established
- Phase -1 instructions activated
- Engineering principles defined

## Change Log

| Date | Change | Version | Author |
|------|--------|---------|--------|
| 2026-03-05 | Initial instruction versioning | 1.0.0 | Domain Analyst |

---
Last Updated: 2026-03-05
