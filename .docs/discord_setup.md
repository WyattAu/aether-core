# Discord Server Setup Guide

This guide explains how to set up the Aether Discord server as described in the community guide.

## Server Creation

### Step 1: Create Server

1. Open Discord and click the "+" button
2. Select "Create My Own"
3. Name: **Aether**
4. Upload server icon (use project logo)

### Step 2: Create Channels

#### Category: Information
| Channel | Type | Description |
|---------|------|-------------|
| `#welcome` | Text | Server rules, getting started |
| `#announcements` | Text | Project updates (locked) |
| `#roadmap` | Text | Feature roadmap and planning |

#### Category: General
| Channel | Type | Description |
|---------|------|-------------|
| `#general` | Text | General Aether discussion |
| `#introductions` | Text | Introduce yourself |
| `#showcase` | Text | Show off projects built with Aether |
| `#off-topic` | Text | Non-Aether chat |

#### Category: Support
| Channel | Type | Description |
|---------|------|-------------|
| `#help` | Text | Get help using Aether |
| `#actor-development` | Text | Building actors |
| `#mesh-networking` | Text | Distributed systems |
| `#ai-integration` | Text | AI provider integration |
| `#troubleshooting` | Text | Debugging help |

#### Category: Development
| Channel | Type | Description |
|---------|------|-------------|
| `#contributing` | Text | Contribution discussion |
| `#code-review` | Text | PR discussion |
| `#architecture` | Text | Architecture decisions |

#### Category: Voice
| Channel | Type | Description |
|---------|------|-------------|
| `#general-voice` | Voice | General discussion |
| `#office-hours` | Voice | Weekly maintainer hours |
| `#pair-programming` | Voice | Collaborate on code |

### Step 3: Create Roles

| Role | Color | Permissions |
|------|-------|-------------|
| **Admin** | Red | All permissions |
| **Moderator** | Orange | Manage messages, kick members |
| **Maintainer** | Purple | Manage channels, pin messages |
| **Contributor** | Green | Mention @everyone |
| **Member** | Blue | Default permissions |

### Step 4: Set Up Bots

#### Required Bots

1. **GitHub Bot** (github.com/integrations/discord)
   - Post release announcements
   - Show issue/PR links with previews
   - Post commit updates

2. **MEE6** (mee6.xyz)
   - Auto-role on join
   - Level system for contributors
   - Custom commands

3. **Carl-bot** (carl.gg)
   - Reaction roles
   - Logging
   - Moderation

### Step 5: Welcome Message

Create a welcome message in `#welcome`:

```markdown
# Welcome to Aether! 

Aether is a high-performance distributed computing platform for WebAssembly actors and containers.

## Getting Started

1. Read the [Code of Conduct](../CODE_OF_CONDUCT.md)
2. Pick a role in #roles
3. Introduce yourself in #introductions
4. Check out [documentation](../README.md)

## Quick Links

-  [Documentation](https://github.com/WyattAu/aether-core/tree/main/.docs)
-  [Report a Bug](https://github.com/WyattAu/aether-core/issues/new?template=bug_report.md)
-  [Request a Feature](https://github.com/WyattAu/aether-core/issues/new?template=feature_request.md)
-  [GitHub Discussions](https://github.com/WyattAu/aether-core/discussions)

## Need Help?

- Ask in #help for usage questions
- Check #troubleshooting for common issues
- Open a GitHub issue for bugs
```

### Step 6: Configure Auto-Moderation

#### Rules

1. **No spam** - Auto-delete duplicate messages
2. **No links in first 5 messages** - Prevent bot spam
3. **No @everyone abuse** - Warn then mute
4. **Keep it civil** - Word filter for profanity

### Step 7: Integration Settings

#### GitHub Integration

1. Go to Server Settings → Integrations
2. Add GitHub integration
3. Subscribe to repository events:
   - Releases
   - Issues
   - Pull Requests

#### Channel for Notifications

- `#announcements` - Releases
- `#code-review` - PR activity
- `#contributing` - New issues

## Moderation Guidelines

### Escalation Path

1. **Warning** - DM from moderator
2. **Mute** - 1 hour to 24 hours
3. **Kick** - Temporary removal
4. **Ban** - Permanent removal

### Reporting Issues

Users can report problems by:
1. DMing any Moderator or Admin
2. Using the `!report @user reason` command
3. Emailing conduct@aether.dev

## Scheduled Events

### Weekly Office Hours

- **Time**: Every Thursday, 3-4 PM UTC
- **Channel**: `#office-hours`
- **Format**: Open Q&A with maintainers

### Monthly Community Call

- **Time**: First Monday of month, 5 PM UTC
- **Channel**: `#general-voice`
- **Format**: Project updates, roadmap discussion

## Metrics to Track

- Member count
- Active users (daily/weekly)
- Message volume by channel
- Response time in help channels

## Launch Checklist

- [ ] Server created
- [ ] All channels created
- [ ] Roles configured
- [ ] Bots added and configured
- [ ] Welcome message posted
- [ ] GitHub integration connected
- [ ] First announcement posted
- [ ] Invite link generated
- [ ] Add invite link to README.md
- [ ] Add invite link to community.md

## Invite Link

Generate a permanent invite link:
1. Server Settings → Invites
2. Create Invite
3. Set to never expire
4. Set max uses to "No limit"
5. Copy link

Default: `https://discord.gg/aether` (requires vanity URL)
Alternative: `https://discord.gg/XXXXXXXX` (generated code)
