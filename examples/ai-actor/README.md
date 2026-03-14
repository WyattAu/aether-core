# AI Actor Example

This example demonstrates how to create an actor that uses AI capabilities to process requests.

## Building

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Deploying

```bash
aether deploy ./target/wasm32-unknown-unknown/release/ai_actor.wasm
```

## Testing

### Send a Summarize request:

```bash
aether call <actor-id> Summarize '{"text": "Your long text here..."}'
```

### Expected response:
```json
{
  "status": "success",
  "summary": "Your summarized text..."
}
```

### Send a Translate request:

```bash
aether call <actor-id> Translate '{"text": "Hello world", "target_language": "es"}'
```

### Expected response:
```json
{
  "status": "success",
  "translation": "¡Hola mundo!",
}
```

### Send a Health check:

```bash
aether call <actor-id> HealthCheck '{}'
```

### Expected response:
```json
{
  "status": "healthy",
  "provider_available": true
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      AI Actor                                  │
├─────────────────────────────────────────────────────────────┤
│  Message Handler    │    AI Provider    │   External API    │
│  (Summarize)        │    (OpenAI/Anthropic)  │                   │
│  (Translate)        │                              │                   │
│  (HealthCheck)       │                              │                   │
└─────────────────────────────────────────────────────────────┘
```
