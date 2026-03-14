import { Actor, Message, MessageType } from '../src';

class TestActor extends Actor {
    static get name() { return 'test'; }

    async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.type === MessageType.CUSTOM) {
            return Message.custom({ echo: message.payload });
        }
    }
}

describe('Actor', () => {
    let actor: TestActor;

    beforeEach(() => {
        actor = new TestActor({ name: 'test' });
    });

    test('should start and emit started event', async () => {
        const listener = jest.fn();
        actor.on('started', listener);
        await actor.start();
        expect(listener).toHaveBeenCalled();
    });

    test('should stop and emit stopped event', async () => {
        const listener = jest.fn();
        actor.on('stopped', listener);
        await actor.start();
        await actor.stop();
        expect(listener).toHaveBeenCalled();
    });

    test('should handle custom messages', async () => {
        await actor.start();
        const response = await actor.handle('sender', Message.custom({ test: 'data' }));
        expect(response).toBeDefined();
        expect(response?.payload).toEqual({ echo: { test: 'data' } });
    });
});
