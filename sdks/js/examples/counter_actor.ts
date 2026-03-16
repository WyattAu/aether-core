/**
 * Stateful Counter Actor Example
 * 
 * Demonstrates persistent state that survives actor restarts.
 */
import { Actor, Message } from '@aether/sdk';

interface CounterState {
    value: number;
    lastUpdated: string;
}

class CounterActor extends Actor {
    private stateKey = 'counter_state';
    private stateData: CounterState = {
        value: 0,
        lastUpdated: ''
    };

    constructor() {
        super('counter-actor');
        this.require('STATE_READ', 'STATE_WRITE', 'ACTOR_MESSAGING', 'LOG');
    }

    async onStart(): Promise<void> {
        console.log(`[${this.name}] Starting counter actor...`);
        
        // Load persisted state
        const data = await this.state.read(this.stateKey);
        if (data) {
            try {
                this.stateData = JSON.parse(data.toString());
                console.log(`[${this.name}] Restored state: value=${this.stateData.value}`);
            } catch (e) {
                console.error(`[${this.name}] Failed to parse state:`, e);
            }
        } else {
            await this.saveState();
            console.log(`[${this.name}] Initialized new counter state`);
        }
    }

    async onStop(): Promise<void> {
        console.log(`[${this.name}] Counter actor stopping`);
    }

    async handleMessage(sender: string, message: Message): Promise<Message | null> {
        const payload = message.payload;

        if (typeof payload === 'string') {
            return this.handleStringCommand(payload);
        }

        if (typeof payload === 'object' && payload !== null) {
            return this.handleDictCommand(payload as Record<string, any>);
        }

        return Message.response({
            error: 'unknown payload type',
            value: this.stateData.value
        });
    }

    private async handleStringCommand(command: string): Promise<Message> {
        switch (command) {
            case 'increment':
                this.stateData.value++;
                await this.saveState();
                return Message.response({ action: 'increment', value: this.stateData.value });

            case 'decrement':
                this.stateData.value--;
                await this.saveState();
                return Message.response({ action: 'decrement', value: this.stateData.value });

            case 'reset':
                this.stateData.value = 0;
                await this.saveState();
                return Message.response({ action: 'reset', value: this.stateData.value });

            case 'get':
                return Message.response({ action: 'get', value: this.stateData.value });

            default:
                return Message.response({
                    error: `unknown command: ${command}`,
                    value: this.stateData.value,
                    usage: 'commands: increment, decrement, reset, get'
                });
        }
    }

    private async handleDictCommand(payload: Record<string, any>): Promise<Message> {
        const command = payload.command;

        switch (command) {
            case 'add':
                const addAmount = payload.amount || 0;
                this.stateData.value += addAmount;
                await this.saveState();
                return Message.response({
                    action: 'add',
                    amount: addAmount,
                    value: this.stateData.value
                });

            case 'subtract':
                const subAmount = payload.amount || 0;
                this.stateData.value -= subAmount;
                await this.saveState();
                return Message.response({
                    action: 'subtract',
                    amount: subAmount,
                    value: this.stateData.value
                });

            case 'set':
                this.stateData.value = payload.value || 0;
                await this.saveState();
                return Message.response({
                    action: 'set',
                    value: this.stateData.value
                });

            default:
                return this.handleStringCommand(command);
        }
    }

    private async saveState(): Promise<void> {
        this.stateData.lastUpdated = new Date().toISOString();
        const data = JSON.stringify(this.stateData);
        await this.state.write(this.stateKey, Buffer.from(data));
    }
}

async function main() {
    const actor = new CounterActor();
    
    // Handle shutdown
    process.on('SIGINT', async () => {
        console.log('Shutting down...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    console.log('Commands: increment, decrement, reset, get, add, subtract, set');
    
    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
