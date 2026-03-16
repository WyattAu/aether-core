/**
 * Hello World Actor Example
 * 
 * Demonstrates a simple actor that responds to greeting messages.
 */
import { Actor, Message } from '@aether/sdk';

class HelloActor extends Actor {
    constructor() {
        super('hello-actor');
        this.require('ACTOR_MESSAGING', 'LOG');
    }

    async handleMessage(sender, message) {
        const payload = message.payload;

        if (typeof payload === 'string') {
            if (payload === 'ping') {
                console.log(`Received ping from ${sender}`);
                return Message.response('pong');
            }
            const greeting = `Hello, ${payload}!`;
            console.log(`Greeting ${sender}: ${greeting}`);
            return Message.response(greeting);
        }

        if (typeof payload === 'object' && payload !== null) {
            if (payload.name) {
                return Message.response({
                    greeting: `Hello, ${payload.name}!`,
                    sender: sender
                });
            }
        }

        return Message.response({
            error: 'unknown payload type'
        });
    }
}

async function main() {
    const actor = new HelloActor();
    
    // Handle shutdown
    process.on('SIGINT', async () => {
        console.log('Shutting down...');
        await actor.stop();
        process.exit(0);
    });

    console.log(`Starting ${actor.name}...`);
    
    try {
        await actor.start();
        await actor.run();
    } catch (error) {
        console.error('Actor error:', error);
        process.exit(1);
    }
}

main();
