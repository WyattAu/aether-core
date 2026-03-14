# Standard Conflicts Summary

## Overview
This document provides a high-level summary of identified standard conflicts. For detailed analysis and resolution strategies, see `.specs/00_requirements/standard_conflicts.md`.

## Active Conflicts

| ID | Conflict | Status | Resolution |
|----|----------|--------|------------|
| C-001 | Determinism vs Entropy | ✅ Resolved | Host-injected entropy model |
| C-002 | Memory Safety vs Zero-Copy | ✅ Resolved | Validated zero-copy with capabilities |
| C-003 | Isolation vs Performance | ✅ Resolved | Hybrid isolation (WASM + MicroVM) |
| C-004 | Audit Logging vs Performance | ✅ Resolved | Async offload to dedicated cores |
| C-005 | FIPS vs Performance | ⏳ Partial | Mode switching, validation pending |
| C-006 | WASI Stability vs Production | ✅ Mitigated | Abstraction layer, version lock |
| C-007 | Data Sovereignty vs Distribution | ✅ Resolved | Topology-aware placement |

## Resolution Statistics

- **Total Conflicts:** 7
- **Resolved:** 5
- **Mitigated:** 1
- **Partial:** 1
- **Open:** 0

## Related Documents

- Detailed Analysis: `.specs/00_requirements/standard_conflicts.md`
- Architecture Decisions: `.adrs/` (to be created)
- Traceability: `TRACEABILITY_MATRIX.md`

---
Last Updated: 2026-03-05
