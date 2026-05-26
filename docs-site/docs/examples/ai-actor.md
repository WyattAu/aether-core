# AI Actor Example

This example demonstrates AI integration in Aether actors.

## Overview

The AI Actor:

1. Accepts AI processing requests
2. Supports multiple operations (generate, summarize, translate, analyze)
3. Returns AI-generated responses
4. Tracks request statistics

## Go Implementation

```go
package main

import (
    "fmt"
    "strings"
    "time"
    "github.com/WyattAu/aether-core/sdks/go/aether"
)

type AIActor struct {
    aether.Actor
    defaultModel  string
    requestCount  int
}

func (a *AIActor) OnStart() error {
    a.defaultModel = "aether-1.0"
    fmt.Printf("[%s] AI Actor starting with model: %s\n", a.Name, a.defaultModel)
    fmt.Printf("[%s] Capabilities: AI inference, text generation, embeddings\n", a.Name)
    return nil
}

func (a *AIActor) OnStop() error {
    fmt.Printf("[%s] AI Actor stopping. Total requests: %d\n", a.Name, a.requestCount)
    return nil
}

func (a *AIActor) HandleMessage(sender string, msg aether.Message) (aether.Message, error) {
    payload, ok := msg.Payload.(map[string]interface{})
    if !ok {
        return aether.Message{}, fmt.Errorf("invalid payload")
    }
    
    prompt, _ := payload["prompt"].(string)
    if prompt == "" {
        return aether.Message{
            Type:    aether.MessageTypeResponse,
            Payload: map[string]interface{}{"error": "prompt is required"},
        }, nil
    }
    
    model, _ := payload["model"].(string)
    if model == "" {
        model = a.defaultModel
    }
    
    maxTokens := int(payload["max_tokens"].(float64))
    if maxTokens == 0 {
        maxTokens = 256
    }
    
    a.requestCount++
    
    // Process AI request (simulated)
    response := a.processAIRequest(prompt, model, maxTokens)
    
    return aether.Message{
        Type: aether.MessageTypeResponse,
        Payload: map[string]interface{}{
            "request": map[string]interface{}{
                "prompt":     prompt,
                "model":      model,
                "max_tokens": maxTokens,
            },
            "response": response,
            "sender":   sender,
        },
    }, nil
}

func (a *AIActor) processAIRequest(prompt, model string, maxTokens int) map[string]interface{} {
    // Simulate processing time
    time.Sleep(time.Duration(len(prompt)) * time.Millisecond)
    
    // Generate response based on prompt type
    var text string
    promptLower := strings.ToLower(prompt)
    
    if strings.Contains(promptLower, "summarize") {
        text = fmt.Sprintf("[AI Summary] Processed: %s...", prompt[:min(50, len(prompt))])
    } else if strings.Contains(promptLower, "translate") {
        text = fmt.Sprintf("[AI Translation] Would translate: %s...", prompt[:min(50, len(prompt))])
    } else if strings.Contains(promptLower, "analyze") {
        text = fmt.Sprintf("[AI Analysis] Analyzed input with %d characters", len(prompt))
    } else {
        text = fmt.Sprintf("[AI Response] Processed your request: %s...", prompt[:min(100, len(prompt))])
    }
    
    tokensUsed := len(prompt)/4 + len(text)/4
    
    return map[string]interface{}{
        "text":        text,
        "model":       model,
        "tokens_used": tokensUsed,
        "processed_at": time.Now().Format(time.RFC3339),
    }
}

func main() {
    actor := &AIActor{}
    actor.Name = "ai-actor"
    actor.Require("NETWORK_OUTBOUND", "ACTOR_MESSAGING", "LOG", "TIME", "RANDOM")
    
    if err := actor.Start(); err != nil {
        panic(err)
    }
    defer actor.Stop()
    
    fmt.Printf("Starting %s...\n", actor.Name)
    fmt.Println("Supported operations: generate, summarize, translate, analyze")
    fmt.Printf("Default model: %s\n", actor.defaultModel)
    
    actor.Run()
}
```

## Python Implementation

```python
import asyncio
import time
from aether_sdk import Actor, Message, MessageType

class AIActor(Actor):
    def __init__(self):
        super().__init__("ai-actor")
        self.default_model = "aether-1.0"
        self.request_count = 0
        self.require("NETWORK_OUTBOUND", "ACTOR_MESSAGING", "LOG", "TIME", "RANDOM")
    
    async def on_start(self) -> None:
        print(f"[{self.name}] AI Actor starting with model: {self.default_model}")
        print(f"[{self.name}] Capabilities: AI inference, text generation, embeddings")
    
    async def on_stop(self) -> None:
        print(f"[{self.name}] AI Actor stopping. Total requests: {self.request_count}")
    
    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type not in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return None
        
        payload = message.payload
        if not isinstance(payload, dict):
            return Message.response({"error": "invalid payload"})
        
        prompt = payload.get("prompt", "")
        if not prompt:
            return Message.response({"error": "prompt is required"})
        
        model = payload.get("model") or self.default_model
        max_tokens = payload.get("max_tokens", 256)
        
        self.request_count += 1
        
        # Process AI request
        response = await self._process_ai_request(prompt, model, max_tokens)
        
        return Message.response({
            "request": {
                "prompt": prompt,
                "model": model,
                "max_tokens": max_tokens,
            },
            "response": response,
            "sender": sender,
        })
    
    async def _process_ai_request(self, prompt: str, model: str, max_tokens: int) -> dict:
        # Simulate processing time
        await asyncio.sleep(min(len(prompt) * 0.001, 2.0))
        
        # Generate response based on prompt type
        prompt_lower = prompt.lower()
        
        if "summarize" in prompt_lower:
            text = f"[AI Summary] Processed: {prompt[:50]}..."
        elif "translate" in prompt_lower:
            text = f"[AI Translation] Would translate: {prompt[:50]}..."
        elif "analyze" in prompt_lower:
            text = f"[AI Analysis] Analyzed input with {len(prompt)} characters"
        else:
            text = f"[AI Response] Processed your request: {prompt[:100]}..."
        
        tokens_used = len(prompt) // 4 + len(text) // 4
        
        return {
            "text": text,
            "model": model,
            "tokens_used": tokens_used,
            "processed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        }

async def main():
    actor = AIActor()
    await actor.start()
    
    print(f"Starting {actor.name}...")
    print("Supported operations: generate, summarize, translate, analyze")
    print(f"Default model: {actor.default_model}")
    
    await actor.run()

if __name__ == "__main__":
    asyncio.run(main())
```

## JavaScript Implementation

```typescript
import { Actor, Message, MessageType } from '@aether/sdk';

interface AIRequest {
    prompt: string;
    model?: string;
    maxTokens?: number;
    temperature?: number;
}

interface AIResponse {
    text: string;
    model: string;
    tokensUsed: number;
    processedAt: string;
}

class AIActor extends Actor {
    private defaultModel = 'aether-1.0';
    private requestCount = 0;

    constructor() {
        super('ai-actor');
        this.require('NETWORK_OUTBOUND', 'ACTOR_MESSAGING', 'LOG', 'TIME', 'RANDOM');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] AI Actor starting with model: ${this.defaultModel}`);
        console.log(`[${this.name}] Capabilities: AI inference, text generation, embeddings`);
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] AI Actor stopping. Total requests: ${this.requestCount}`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        if (message.type === MessageType.REQUEST || message.type === MessageType.RPC_REQUEST) {
            return this.handleRequest(sender, message);
        }
        return null;
    }

    private async handleRequest(sender: string, message: Message): Promise<Message> {
        const payload = message.payload;
        const request = this.parseRequest(payload);

        if (!request.prompt) {
            return Message.response({ error: 'prompt is required' });
        }

        request.model = request.model || this.defaultModel;
        request.maxTokens = request.maxTokens || 256;

        this.requestCount++;

        const response = await this.processAIRequest(request);

        return Message.response({
            request: {
                prompt: request.prompt,
                model: request.model,
                max_tokens: request.maxTokens,
            },
            response: response,
            sender: sender,
        });
    }

    private parseRequest(payload: any): AIRequest {
        if (typeof payload === 'string') {
            return { prompt: payload };
        }
        if (typeof payload === 'object' && payload !== null) {
            return {
                prompt: payload.prompt || '',
                model: payload.model,
                maxTokens: payload.max_tokens || payload.maxTokens,
            };
        }
        return { prompt: '' };
    }

    private async processAIRequest(request: AIRequest): Promise<AIResponse> {
        const processingTime = Math.min((request.prompt?.length || 0) * 0.001, 2.0);
        await new Promise(resolve => setTimeout(resolve, Math.max(processingTime, 100)));

        const promptLower = (request.prompt || '').toLowerCase();
        let text: string;

        if (promptLower.includes('summarize')) {
            text = `[AI Summary] Processed: ${request.prompt?.substring(0, 50)}...`;
        } else if (promptLower.includes('translate')) {
            text = `[AI Translation] Would translate: ${request.prompt?.substring(0, 50)}...`;
        } else if (promptLower.includes('analyze')) {
            text = `[AI Analysis] Analyzed input with ${request.prompt?.length || 0} characters`;
        } else {
            text = `[AI Response] Processed your request: ${request.prompt?.substring(0, 100)}...`;
        }

        const tokensUsed = Math.floor((request.prompt?.length || 0) / 4) + Math.floor(text.length / 4);

        return {
            text,
            model: request.model || this.defaultModel,
            tokensUsed,
            processedAt: new Date().toISOString(),
        };
    }
}

async function main(): Promise<void> {
    const actor = new AIActor();

    process.on('SIGINT', async () => {
        console.log('Shutting down AI actor...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    console.log('Supported operations: generate, summarize, translate, analyze');
    console.log(`Default model: ${actor['defaultModel']}`);

    await actor.start();
    await actor.run();
}

main();
```

## Running the Example

### Build and Run

```bash
# Go
cd sdks/go/examples/ai_actor
go run main.go

# Python
cd sdks/python/examples
python ai_actor.py

# JavaScript
cd sdks/js/examples
npx ts-node ai_actor.ts
```

### Testing

```bash
# Generate text
aether invoke ai-actor '{"prompt": "Write a hello world program"}'

# Summarize
aether invoke ai-actor '{"prompt": "Summarize this long text about distributed systems..."}'

# Translate
aether invoke ai-actor '{"prompt": "Translate 'hello' to Spanish"}'

# Analyze
aether invoke ai-actor '{"prompt": "Analyze the sentiment of this text"}'
```

## Key Concepts

### AI Capabilities

| Capability | Description |
|------------|-------------|
| `NETWORK_OUTBOUND` | Access external AI APIs |
| `AI_USE` | Use AI services |

### Request Format

```json
{
    "prompt": "Your prompt text",
    "model": "optional-model-name",
    "max_tokens": 256,
    "temperature": 0.7
}
```

### Response Format

```json
{
    "request": { ... },
    "response": {
        "text": "AI generated response",
        "model": "aether-1.0",
        "tokens_used": 42,
        "processed_at": "2026-03-16T12:00:00Z"
    }
}
```
