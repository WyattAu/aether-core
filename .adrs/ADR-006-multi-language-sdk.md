# ADR-006: Multi-Language SDK Strategy

## Status

Accepted

## Context

We needed to provide SDKs for multiple programming languages to maximize developer adoption and support different use cases.

## Decision

We implemented **multi-language SDKs** with a consistent API pattern across languages.

### Supported Languages

| Language | Package | Version | Status |
|----------|---------|---------|--------|
| Go | `github.com/WyattAu/aether-core/sdks/go/aether` | 0.1.0 | Stable |
| Python | `aether-sdk` | 0.1.0 | Stable |
| JavaScript | `@aether/sdk` | 0.1.0 | Stable |
| Rust | `aether` | 0.1.0 | Beta |

### API Consistency

All SDKs share:
- Same Actor interface pattern
- Same Message types
- Same Capability model
- Same State API
- Same Error types

### Example Pattern

```go
// Go
type MyActor struct {
    *aether.BaseActor
}

func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
    return response, nil
}
```

```python
# Python
class MyActor(Actor):
    async def handle_message(self, sender: str, message: Message) -> Message:
        return Message.response(result)
```

```javascript
// JavaScript
class MyActor extends Actor {
    async handleMessage(sender, message) {
        return Message.response(result);
    }
}
```

## Consequences

### Positive
- Developers can use their preferred language
- Consistent mental model across languages
- Easy to port code between languages
- Broader ecosystem reach

### Negative
- More codebases to maintain
- API consistency requires coordination
- Different testing tools per language

## Alternatives Considered

1. **Single Language (Rust only)**
   - Rejected: Limits adoption, steep learning curve

2. **Language Bindings (FFI)**
   - Rejected: Complex to maintain, platform-specific issues

3. **Protocol-Only (gRPC)**
   - Rejected: Less idiomatic, more boilerplate

---

# ADR-007: MkDocs for Documentation Site

## Status

Accepted

## Context

We needed a documentation site that is easy to maintain, supports code examples, and can be deployed to GitHub Pages.

## Decision

We chose **MkDocs with Material theme** for the documentation site.

### Rationale

1. **Markdown-based**: Easy to write and maintain
2. **Material theme**: Modern, responsive design
3. **Code highlighting**: Excellent support for multiple languages
4. **GitHub Pages**: Native deployment support
5. **Search**: Built-in search functionality
6. **Versioning**: Mike plugin for version management

### Site Structure

```
docs-site/
├── mkdocs.yml          # Configuration
├── docs/
│   ├── index.md        # Home page
│   ├── getting-started/
│   ├── sdks/
│   ├── examples/
│   └── architecture/
```

## Consequences

### Positive
- Fast to write and update
- Beautiful default theme
- Great code example support
- Easy GitHub Pages deployment
- Version management built-in

### Negative
- Python dependency for local builds
- Less customization than custom site
- Some Material theme limitations

## Alternatives Considered

1. **Docusaurus**
   - Rejected: More complex, React-based, overkill for our needs

2. **GitBook**
   - Rejected: Proprietary, less control

3. **Hugo**
   - Rejected: Steeper learning curve, less suited for docs

4. **Custom Site**
   - Rejected: Too much maintenance overhead

## Related
- [MkDocs](https://www.mkdocs.org/)
- [Material for MkDocs](https://squidfunk.github.io/mkdocs-material/)
