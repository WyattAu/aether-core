import { Actor, Message, MessageType, Capability } from '../src';

interface CounterState {
    count: number;
}

class StatefulActor extends Actor {
    static get name() { return 'counter'; }

    async onStart(): Promise<void> {
        this.require(Capability.STATE_READ, Capability.STATE_WRITE);
        const existing = await this.state.getJSON<CounterState>('counter');
        if (!existing) {
            await this.state.setJSON<CounterState>('counter', { count: 0 });
        }
    }

    async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.type !== MessageType.CUSTOM) return;

        const state = await this.state.getJSON<CounterState>('counter') || { count: 0 };

        switch (message.payload.action) {
            case 'increment':
                state.count++;
                await this.state.setJSON('counter', state);
                return Message.custom({ count: state.count });

            case 'decrement':
                state.count--;
                await this.state.setJSON('counter', state);
                return Message.custom({ count: state.count });

            case 'get':
                return Message.custom({ count: state.count });

            default:
                return Message.custom({ error: 'Unknown action' });
        }
    }
}

const actor = new StatefulActor({ name: 'counter' });
actor.start().catch(console.error);
