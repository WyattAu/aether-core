# Aether Community Guide

**Version:** 1.0.0-alpha  
**Last Updated:** 2026-03-14

---

## Table of Contents

1. [Welcome](#welcome)
2. [Communication Channels](#communication-channels)
3. [Getting Help](#getting-help)
4. [Contributing](#contributing)
5. [Good First Issues](#good-first-issues)
6. [Recognition](#recognition)

---

## Welcome

Welcome to the Aether community! We're building a high-performance distributed computing platform, and we're excited to have you here.

### Our Values

- **Openness**: We welcome contributions from everyone
- **Quality**: We maintain high standards for code and documentation
- **Collaboration**: We work together to solve problems
- **Respect**: We treat everyone with dignity and respect

---

## Communication Channels

### Discord Server

Join us on Discord: [discord.gg/aether](https://discord.gg/aether)

#### Channel Structure

| Channel | Purpose |
|---------|---------|
| `#welcome` | Introductions and community guidelines |
| `#announcements` | Project updates and releases |
| `#general` | General discussion about Aether |
| `#help` | Get help with Aether usage |
| `#actor-development` | Questions about building actors |
| `#mesh-networking` | Distributed systems discussion |
| `#ai-integration` | AI provider and delegation topics |
| `#contributing` | Contribution discussions |
| `#showcase` | Show off what you've built |
| `#off-topic` | Non-Aether discussion |

#### Code of Conduct

All Discord interactions follow our [Code of Conduct](../CODE_OF_CONDUCT.md). Please be respectful and inclusive.

### GitHub

| Platform | Purpose |
|----------|---------|
| [Issues](https://github.com/aether-project/aether/issues) | Bug reports, feature requests |
| [Discussions](https://github.com/aether-project/aether/discussions) | Long-form discussions, Q&A |
| [Pull Requests](https://github.com/aether-project/aether/pulls) | Code contributions |

---

## Getting Help

### Before Asking

1. **Search existing resources**
   - [Documentation](/.docs/)
   - [FAQ](#faq)
   - [GitHub Issues](https://github.com/aether-project/aether/issues)
   - [Discord history](https://discord.gg/aether)

2. **Prepare your question**
   - What are you trying to do?
   - What have you tried?
   - What errors are you seeing?
   - What's your environment?

### Where to Ask

| Question Type | Best Channel |
|---------------|--------------|
| Usage question | Discord `#help` |
| Bug report | GitHub Issues |
| Feature request | GitHub Issues |
| Actor development | Discord `#actor-development` |
| Architecture discussion | GitHub Discussions |
| Security issue | security@aether.dev (private) |

### FAQ

#### General

**Q: What's the difference between WASM and OCI actors?**

A: WASM actors run in a WebAssembly runtime with sub-50µs cold starts and strict sandboxing. OCI actors run as containers in KVM-isolated microVMs for legacy workloads.

**Q: Does Aether require FoundationDB?**

A: For distributed state, yes. For single-node development, you can use the in-memory state backend.

**Q: What languages can I write actors in?**

A: Any language that compiles to WebAssembly (Rust, AssemblyScript, TinyGo, etc.) or runs in containers (any OCI image).

#### Development

**Q: Why does my actor fail to start?**

A: Check the actor logs with `aether logs <actor-id>`. Common issues:
- Missing capabilities
- Memory limit exceeded
- Initialization panic

**Q: How do I access external services from an actor?**

A: Actors need the appropriate capabilities:
- `net_outbound` for network access
- `fs_read`/`fs_write` for file system
- Configure in `aether.toml`

**Q: How do I share state between actors?**

A: Use the distributed key-value store or send messages between actors via the mesh network.

---

## Contributing

See our [Contributing Guide](../CONTRIBUTING.md) for detailed information on:

- Development setup
- Code standards
- Testing guidelines
- Pull request process

### Contribution Types

| Type | Description |
|------|-------------|
| 🐛 Bug fixes | Fix issues in existing code |
| ✨ Features | Add new functionality |
| 📝 Documentation | Improve docs, examples |
| 🧪 Tests | Add or improve tests |
| 🔧 Refactoring | Improve code quality |
| 🌐 Translations | Translate documentation |

---

## Good First Issues

### Finding Good First Issues

1. **GitHub Labels**
   - Look for [`good first issue`](https://github.com/aether-project/aether/issues?q=is%3Aopen+label%3A%22good+first+issue%22)
   - Look for [`help wanted`](https://github.com/aether-project/aether/issues?q=is%3Aopen+label%3A%22help+wanted%22)

2. **Areas for New Contributors**

| Area | Difficulty | Description |
|------|------------|-------------|
| Documentation | Easy | Improve docs, add examples |
| Unit tests | Easy | Add test coverage |
| Error messages | Easy | Improve error clarity |
| CLI improvements | Medium | Add commands, flags |
| Actor examples | Medium | Create example actors |
| Performance tests | Medium | Add benchmarks |

### Example Good First Issues

```markdown
### Documentation
- Add example for scheduled actors
- Document error handling patterns
- Translate getting started guide

### Testing
- Add tests for actor registry
- Add integration test for mesh reconnection
- Add property tests for state store

### Code
- Improve error messages in actor loader
- Add health check for mesh connections
- Implement actor metrics collection
```

### Claiming an Issue

1. Comment on the issue: "I'd like to work on this"
2. Wait for maintainer assignment
3. Create a branch and start working
4. Reference the issue in your PR

---

## Recognition

### Contributors

We recognize contributors in several ways:

- **CHANGELOG.md**: All contributors listed
- **README.md**: Top contributors highlighted
- **Discord**: Special role for contributors
- **Release Notes**: Contribution highlights

### Becoming a Maintainer

Regular contributors may be invited to become maintainers. Qualities we look for:

- Consistent quality contributions
- Helpful community participation
- Understanding of project architecture
- Commitment to project values

---

## Security

### Reporting Security Issues

**Do not report security vulnerabilities publicly.**

Email security@aether.dev with:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We respond within 48 hours and aim to fix critical issues within 7 days.

---

## Stay Connected

| Platform | Handle |
|----------|--------|
| Discord | [discord.gg/aether](https://discord.gg/aether) |
| Twitter | [@aether_project](https://twitter.com/aether_project) |
| GitHub | [aether-project/aether](https://github.com/aether-project/aether) |
| Blog | [blog.aether.dev](https://blog.aether.dev) |

---

Thank you for being part of the Aether community! 🚀
