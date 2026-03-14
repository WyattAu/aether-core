import { Message, MessageType, Priority } from '../src';

describe('Message', () => {
    test('should create a start message', () => {
        const msg = Message.start();
        expect(msg.type).toBe(MessageType.START);
        expect(msg.payload).toEqual({});
    });

    test('should create a stop message', () => {
        const msg = Message.stop();
        expect(msg.type).toBe(MessageType.STOP);
        expect(msg.payload).toEqual({});
    });

    test('should create a custom message', () => {
        const msg = Message.custom({ key: 'value' });
        expect(msg.type).toBe(MessageType.CUSTOM);
        expect(msg.payload).toEqual({ key: 'value' });
    });

    test('should create an RPC request', () => {
        const msg = Message.rpcRequest({ method: 'test' }, 'corr-123');
        expect(msg.type).toBe(MessageType.RPC_REQUEST);
        expect(msg.correlationId).toBe('corr-123');
    });

    test('should create an RPC response', () => {
        const msg = Message.rpcResponse({ result: 'ok' }, 'corr-123');
        expect(msg.type).toBe(MessageType.RPC_RESPONSE);
        expect(msg.correlationId).toBe('corr-123');
    });

    test('should serialize to JSON', () => {
        const msg = Message.custom({ test: 'data' }, Priority.HIGH);
        const json = msg.toJSON();
        expect(json).toMatchObject({
            type: MessageType.CUSTOM,
            payload: { test: 'data' },
            priority: Priority.HIGH,
        });
    });

    test('should deserialize from JSON', () => {
        const data = {
            type: MessageType.CUSTOM,
            payload: { key: 'value' },
            sender: 'actor1',
            correlationId: 'corr-1',
            priority: Priority.NORMAL,
        };
        const msg = Message.fromJSON(data);
        expect(msg.type).toBe(MessageType.CUSTOM);
        expect(msg.payload).toEqual({ key: 'value' });
        expect(msg.sender).toBe('actor1');
    });
});
