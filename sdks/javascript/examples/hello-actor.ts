import { Actor, Message, MessageType, Capability } from '../src';

class HelloActor extends Actor {
    static get name() { return 'hello'; }

    async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.type === MessageType.CUSTOM) {
            const name = message.payload.name || 'World';
            return Message.custom({ greeting: `Hello, ${name}!` });
        }
    }
}

const actor = new HelloActor({ name: 'hello' });
actor.start().catch(console.error);
