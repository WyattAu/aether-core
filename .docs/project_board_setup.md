# Project Board Setup Guide

> **HISTORICAL**: This document describes the v1.3.0 project board setup and is preserved for reference only. The current version is v2.0.0 (Rust-native). See `docs/ROADMAP_TO_PRODUCTION.md` for the active roadmap.

## GitHub Project Board: Aether v1.3.0 Roadmap

### Board Structure

#### Columns

1. **Backlog** - Items not yet prioritized
2. **Ready** - Items ready to start
3. **In Progress** - Currently being worked on
4. **In Review** - Pull requests open
5. **Done** - Completed items

#### Views

- **Roadmap View** - Timeline with milestones
- **Board View** - Kanban-style workflow
- **Table View** - Detailed list with metadata

### Initial Issues to Add

| Issue | Priority | Column |
|-------|----------|--------|
| #7 - WASM Execution Fix | Critical | Ready |
| #8 - Local Mesh Routing | Critical | Ready |
| #9 - Vault Integration | Critical | Ready |
| #10 - Panic Code Fix | Critical | Ready |
| #11 - Deadlock Fix | High | Backlog |
| #12 - TUI Real Metrics | High | Backlog |
| #13 - MCP Tests | High | Backlog |
| #14 - Python SDK | High | Backlog |

### Labels to Create

```bash
# Create labels
gh label create "critical" --color "FF0000" --description "Must fix before release"
gh label create "v1.3.0" --color "00FF00" --description "Target v1.3.0 release"
gh label create "sdk" --color "0000FF" --description "SDK development"
gh label create "technical-debt" --color "FFA500" --description "Technical debt items"
```

### Milestones

```bash
# Create milestones
gh milestone create "v1.3.0-alpha" --due-date "2026-03-28"
gh milestone create "v1.3.0-beta" --due-date "2026-04-11"
gh milestone create "v1.3.0" --due-date "2026-04-25"
```

### Manual Setup

1. Go to https://github.com/WyattAu/aether-core/projects
2. Click "New Project"
3. Select "Board" template
4. Name: "Aether v1.3.0 Roadmap"
5. Add columns: Backlog, Ready, In Progress, In Review, Done
6. Add issues #7-#14 to Backlog
7. Create views: Roadmap, Board, Table

### Automation Rules

- When issue assigned → Move to "In Progress"
- When PR opened → Move to "In Review"
- When PR merged → Move to "Done"
- When issue closed → Move to "Done"
