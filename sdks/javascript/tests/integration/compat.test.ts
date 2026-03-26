import * as fs from 'fs';
import * as path from 'path';
import { Message, MessageType, Priority } from '../../src/messaging';
import { Timestamp, Duration, Watermark } from '../../src/streaming/types';

const VECTORS_PATH = path.resolve(
    __dirname,
    '../../../../tests/integration/test_vectors.json'
);

const COMMON_MESSAGE_TYPES = ['start', 'stop', 'signal', 'rpc_request', 'rpc_response', 'custom'];

function loadVectors(): any {
    return JSON.parse(fs.readFileSync(VECTORS_PATH, 'utf-8'));
}

describe('Message serialization compatibility', () => {
    test('custom message JSON structure', () => {
        const msg = Message.custom({ key: 'value', count: 42 });
        const json = msg.toJSON() as any;

        expect(json.type).toBe('custom');
        expect(json.payload).toEqual({ key: 'value', count: 42 });
        expect(json.sender).toBeUndefined();
        expect(json.correlationId).toBeUndefined();
        expect(json.priority).toBe(Priority.NORMAL);
    });

    test('RPC request JSON structure', () => {
        const msg = Message.rpcRequest({ method: 'get_user', args: [1] }, 'corr-abc-123');
        const json = msg.toJSON() as any;

        expect(json.type).toBe('rpc_request');
        expect(json.correlationId).toBe('corr-abc-123');
        expect(json.payload.method).toBe('get_user');
    });

    test('round-trip through toJSON/fromJSON', () => {
        const original = Message.rpcRequest(
            { fn: 'add', args: [1, 2] },
            'corr-rt-001'
        );
        const json = original.toJSON();
        const restored = Message.fromJSON(json);

        expect(restored.type).toBe(original.type);
        expect(restored.payload).toEqual(original.payload);
        expect(restored.correlationId).toBe(original.correlationId);
    });

    test('all common message types produce valid JSON', () => {
        const start = Message.start();
        const stop = Message.stop();
        const custom = Message.custom({ x: 1 });
        const req = Message.rpcRequest({}, 'c');
        const res = Message.rpcResponse({}, 'c');

        for (const msg of [start, stop, custom, req, res]) {
            const json = msg.toJSON() as any;
            expect(typeof json.type).toBe('string');
            expect(COMMON_MESSAGE_TYPES).toContain(json.type);
            expect(typeof json.payload).toBe('object');
        }
    });

    test('JS message can be deserialized from Python JSON format', () => {
        const pythonJson = JSON.stringify({
            type: 'custom',
            payload: { action: 'test' },
            sender: 'py-actor',
            correlation_id: 'corr-py',
        });
        const data = JSON.parse(pythonJson);

        const msg = Message.fromJSON({
            type: data.type,
            payload: data.payload,
            sender: data.sender,
            correlationId: data.correlation_id,
        });

        expect(msg.type).toBe(MessageType.CUSTOM);
        expect(msg.payload).toEqual({ action: 'test' });
        expect(msg.sender).toBe('py-actor');
    });
});

describe('Message test vectors', () => {
    test('vectors file exists', () => {
        expect(fs.existsSync(VECTORS_PATH)).toBe(true);
    });

    test('all message vectors produce valid JSON', () => {
        const vectors = loadVectors();
        for (const vec of vectors.messages) {
            const typeValue = vec.input.type as string;
            const msg = new Message(
                typeValue as MessageType,
                vec.input.payload,
                vec.input.sender,
                vec.input.correlation_id,
                Priority.NORMAL
            );
            const json = msg.toJSON() as any;

            expect(json.type).toBe(vec.expected_type);
            expect(Object.keys(json.payload).sort()).toEqual(vec.expected_payload_keys.sort());
            if (vec.expected_sender) {
                expect(json.sender).toBe(vec.expected_sender);
            }
            if (vec.expected_correlation_id) {
                expect(json.correlationId).toBe(vec.expected_correlation_id);
            }
        }
    });

    test('message types in vectors match SDK', () => {
        const vectors = loadVectors();
        const sdkTypes = new Set(
            Object.values(MessageType).filter((v) => typeof v === 'string') as string[]
        );
        for (const typeStr of vectors.message_types.all_types) {
            if (COMMON_MESSAGE_TYPES.includes(typeStr)) {
                expect(sdkTypes.has(typeStr)).toBe(true);
            }
        }
    });
});

describe('Timestamp/Duration compatibility', () => {
    test('timestamp from milliseconds', () => {
        const ts = new Timestamp(0);
        expect(ts.milliseconds).toBe(0);
        expect(ts.toSeconds()).toBe(0);
    });

    test('timestamp from seconds', () => {
        const ts = Timestamp.fromSeconds(1.5);
        expect(ts.milliseconds).toBe(1500);
        expect(ts.toSeconds()).toBe(1.5);
    });

    test('timestamp toJSON returns integer milliseconds', () => {
        const ts = new Timestamp(1700000000000);
        const json = ts.toJSON();
        expect(json).toBe(1700000000000);
        expect(Number.isInteger(json)).toBe(true);
    });

    test('timestamp fromJSON round-trip', () => {
        const original = new Timestamp(1700000000000);
        const restored = Timestamp.fromJSON(original.toJSON());
        expect(restored.milliseconds).toBe(original.milliseconds);
    });

    test('timestamp arithmetic', () => {
        const ts = new Timestamp(1000);
        const d = Duration.fromSeconds(5);
        const result = ts.add(d);
        expect(result.milliseconds).toBe(6000);
    });

    test('timestamp subtraction', () => {
        const a = new Timestamp(10000);
        const b = new Timestamp(3000);
        const diff = a.subtract(b);
        expect(diff.milliseconds).toBe(7000);
    });

    test('duration from minutes', () => {
        const d = Duration.fromMinutes(5);
        expect(d.toSeconds()).toBe(300);
        expect(d.toMillis()).toBe(300000);
    });

    test('duration from hours', () => {
        const d = Duration.fromHours(1);
        expect(d.toMillis()).toBe(3600000);
    });

    test('all timestamp vectors', () => {
        const vectors = loadVectors();
        for (const vec of vectors.timestamps) {
            const ts = new Timestamp(vec.input_ms);
            expect(ts.milliseconds).toBe(vec.input_ms);
        }
    });

    test('all duration vectors', () => {
        const vectors = loadVectors();
        for (const vec of vectors.durations) {
            const d = Duration.fromMillis(vec.input_ms);
            expect(d.toSeconds()).toBe(vec.expected_seconds);
        }
    });
});

describe('Watermark compatibility', () => {
    test('watermark creation', () => {
        const ts = new Timestamp(5000);
        const wm = new Watermark(ts, 'input-stream');
        expect(wm.timestamp.milliseconds).toBe(5000);
        expect((wm as any).streamId).toBe('input-stream');
    });

    test('watermark late detection', () => {
        const wm = new Watermark(new Timestamp(10000), 's');
        expect(wm.isLate(new Timestamp(5000))).toBe(true);
        expect(wm.isLate(new Timestamp(10000))).toBe(false);
        expect(wm.isLate(new Timestamp(15000))).toBe(false);
    });

    test('watermark serialization', () => {
        const wm = new Watermark(new Timestamp(7000), 'stream-1', 3);
        const json = wm.toJSON() as any;
        expect(json.timestamp).toBe(7000);
        expect(json.streamId).toBe('stream-1');
        expect(json.partition).toBe(3);
    });
});

describe('Resilience pattern consistency', () => {
    test('resilience vectors defined', () => {
        const vectors = loadVectors();
        expect(vectors.resilience.length).toBeGreaterThan(0);
    });

    test('circuit breaker config is valid', () => {
        const vectors = loadVectors();
        const vec = vectors.resilience.find((v: any) => v.name === 'circuit_breaker_closed_allows_5');
        expect(vec).toBeDefined();
        expect(vec.config.failureThreshold).toBe(5);
        expect(vec.config.successThreshold).toBe(3);
        expect(vec.config.timeout).toBe(1000);
        expect(vec.expected).toBe('allows_5_then_opens');
    });
});
