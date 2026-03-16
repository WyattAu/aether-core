# Examples Overview

Explore example applications built with Aether.

## Beginner Examples

### Hello World
The simplest possible actor that responds to greeting messages.

- **Topics:** Actor basics, message handling
- **Languages:** Go, Python, JavaScript
- **Time:** 5 minutes

[View Example →](hello-world.md)

### Stateful Counter
An actor that maintains persistent state across restarts.

- **Topics:** State persistence, lifecycle hooks
- **Languages:** Go, Python, JavaScript
- **Time:** 10 minutes

[View Example →](counter.md)

## Intermediate Examples

### AI-Powered Actor
An actor that integrates with AI services for text generation.

- **Topics:** External APIs, async operations
- **Languages:** Go, Python
- **Time:** 15 minutes

[View Example →](ai-actor.md)

### Mesh Communication
Actors communicating across a distributed mesh network.

- **Topics:** Distributed actors, mesh networking
- **Languages:** Go
- **Time:** 20 minutes

[View Example →](mesh.md)

## Advanced Examples

### Chat Application
A complete multi-room chat application with multiple actors.

- **Topics:** Multi-actor coordination, state management
- **Languages:** Go
- **Time:** 30 minutes

[View Example →](chat-app.md)

## Running Examples

### From Source

```bash
# Clone the repository
git clone https://github.com/WyattAu/aether-core.git
cd aether-core

# Run a Go example
cd sdks/go/examples/hello_actor
go run main.go

# Run a Python example
cd sdks/python/examples
python hello_actor.py

# Run a JavaScript example
cd sdks/js/examples
npm install
npm run hello
```

### Using Docker

```bash
# Run hello world example
docker run --rm ghcr.io/wyattau/aether-examples:hello-world

# Run counter example
docker run --rm ghcr.io/wyattau/aether-examples:counter
```

## Example Structure

Each example follows this structure:

```
example-name/
├── main.go           # Main implementation
├── README.md         # Example-specific documentation
├── go.mod            # Dependencies
└── test/
    └── main_test.go  # Tests
```

## Learning Path

```
Hello World → Counter → AI Actor → Mesh → Chat App
    │            │          │         │        │
    │            │          │         │        └── Multi-actor apps
    │            │          │         └── Distributed systems
    │            │          └── External integrations
    │            └── State management
    └── Actor basics
```

## Contributing Examples

Want to add an example? See our [Contributing Guide](https://github.com/WyattAu/aether-core/blob/main/CONTRIBUTING.md).

### Example Requirements

1. **Self-contained** - Should work independently
2. **Well-documented** - Clear README and code comments
3. **Tested** - Include unit tests
4. **Idiomatic** - Follow language best practices

## Next Steps

- [Hello World](hello-world.md) - Start here if you're new
- [Stateful Counter](counter.md) - Learn about state persistence
- [Chat Application](chat-app.md) - See a complete application
