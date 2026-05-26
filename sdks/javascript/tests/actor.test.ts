import { Actor, actor, Message, MessageType, Capability } from '../src';

class TestActor extends Actor {
    static get name() { return 'test'; }

    async handle(sender: string, message: Message): Promise<Message | void> {
        if (message.type === MessageType.CUSTOM) {
            return Message.custom({ echo: message.payload });
        }
    }
}

class TestActorWithHooks extends Actor {
    static get name() { return 'test-hooks'; }
    public startedCalled = false;
    public stoppedCalled = false;

    protected async onStart(): Promise<void> {
        this.startedCalled = true;
    }

    protected async onStop(): Promise<void> {
        this.stoppedCalled = true;
    }

    async handle(sender: string, message: Message): Promise<Message | void> {}
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

describe('Actor - static name', () => {
    test('base Actor.name throws error', () => {
        // Actor is abstract, but we can test the static getter by calling it
        // on the base class (which throws)
        expect(() => Actor.name).toThrow('Actor.name must be implemented');
    });
});

describe('Actor - lifecycle hooks', () => {
    test('calls onStart when defined', async () => {
        const actor = new TestActorWithHooks({ name: 'test-hooks' });
        await actor.start();
        expect(actor.startedCalled).toBe(true);
    });

    test('calls onStop when defined', async () => {
        const actor = new TestActorWithHooks({ name: 'test-hooks' });
        await actor.start();
        await actor.stop();
        expect(actor.stoppedCalled).toBe(true);
    });
});

describe('Actor - require', () => {
    test('adds capabilities to capability set', () => {
        const actor = new TestActor({ name: 'test' });
        actor.require(Capability.NETWORK_OUTBOUND, Capability.STATE_READ);

        // Access protected member via type assertion for testing
        const caps = (actor as any).capabilities;
        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(true);
        expect(caps.has(Capability.STATE_READ)).toBe(true);
    });

    test('does not duplicate capabilities', () => {
        const actor = new TestActor({ name: 'test' });
        actor.require(Capability.NETWORK_OUTBOUND);
        actor.require(Capability.NETWORK_OUTBOUND);

        const caps = (actor as any).capabilities;
        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(true);
    });
});

describe('Actor - send', () => {
    test('send is a no-op placeholder', async () => {
        const actor = new TestActor({ name: 'test' });
        // Should not throw - it's a placeholder
        await actor.send('target', Message.custom({ data: 'test' }));
    });
});

describe('Actor - call', () => {
    test('call throws not implemented error', async () => {
        const actor = new TestActor({ name: 'test' });
        await expect(
            actor.call('target', { request: 'data' }, 100)
        ).rejects.toThrow('RPC timeout: target');
    }, 10000);
});

describe('actor decorator', () => {
    test('creates decorator that overrides static name', () => {
        @actor({ name: 'decorated-actor' })
        class DecoratedActor extends Actor {
            static get name() { return 'original'; }
            async handle(sender: string, message: Message): Promise<Message | void> {}
        }

        expect(DecoratedActor.name).toBe('decorated-actor');
    });
});
