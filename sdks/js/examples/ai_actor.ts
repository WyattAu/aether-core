/**
 * AI-Powered Actor Example
 * 
 * Demonstrates integration with AI services for text generation.
 */
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
        } else if (message.type === MessageType.EVENT) {
            this.handleEvent(sender, message);
            return null;
        }
        return Message.response({ error: 'unsupported message type' });
    }

    private async handleRequest(sender: string, message: Message): Promise<Message> {
        const payload = message.payload;
        const request = this.parseRequest(payload);

        if (!request.prompt) {
            return Message.response({ error: 'prompt is required' });
        }

        // Set defaults
        request.model = request.model || this.defaultModel;
        request.maxTokens = request.maxTokens || 256;

        this.requestCount++;

        // Process the AI request
        const response = await this.processAIRequest(request);

        if (!response) {
            return Message.response({ error: 'failed to process request' });
        }

        return Message.response({
            request: {
                prompt: request.prompt,
                model: request.model,
                max_tokens: request.maxTokens
            },
            response: response,
            sender: sender
        });
    }

    private handleEvent(sender: string, message: Message): void {
        const payload = message.payload as Record<string, any> | null;
        if (payload?.type) {
            console.log(`[${this.name}] Received ${payload.type} event from ${sender}`);
        }
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
                temperature: payload.temperature
            };
        }

        return { prompt: '' };
    }

    private async processAIRequest(request: AIRequest): Promise<AIResponse | null> {
        // Simulate AI processing time
        const processingTime = Math.min((request.prompt?.length || 0) * 0.001, 2.0);
        await new Promise(resolve => setTimeout(resolve, Math.max(processingTime, 100)));

        // Generate simulated response based on prompt content
        const promptLower = (request.prompt || '').toLowerCase();
        let text: string;

        if (promptLower.includes('summarize')) {
            text = `[AI Summary] Processed: ${request.prompt?.substring(0, 50)}...`;
        } else if (promptLower.includes('translate')) {
            text = `[AI Translation] Would translate: ${request.prompt?.substring(0, 50)}...`;
        } else if (promptLower.includes('analyze')) {
            text = `[AI Analysis] Analyzed input with ${request.prompt?.length || 0} characters`;
        } else if (promptLower.includes('generate')) {
            text = `[AI Generated] Creative output based on: ${request.prompt?.substring(0, 50)}...`;
        } else {
            text = `[AI Response] Processed your request: ${request.prompt?.substring(0, 100)}...`;
        }

        const tokensUsed = Math.floor((request.prompt?.length || 0) / 4) + Math.floor(text.length / 4);

        return {
            text,
            model: request.model || this.defaultModel,
            tokensUsed,
            processedAt: new Date().toISOString()
        };
    }
}

async function main(): Promise<void> {
    const actor = new AIActor();

    // Handle shutdown
    process.on('SIGINT', async () => {
        console.log('Shutting down AI actor...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    console.log('Supported operations: generate, summarize, translate, analyze');
    console.log(`Default model: ${actor['defaultModel']}`);

    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
