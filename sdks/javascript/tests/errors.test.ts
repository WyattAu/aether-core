import { AetherError, CapabilityDenied, ActorNotFound, StateError, RpcError } from '../src/errors';

describe('AetherError', () => {
    test('should be an Error', () => {
        expect(AetherError.prototype).toBeInstanceOf(Error);
    });

    test('should be throwable', () => {
        expect(() => {
            throw new AetherError('test error');
        }).toThrow(AetherError);
    });

    test('should preserve message', () => {
        const error = new AetherError('test message');
        expect(error.message).toBe('test message');
    });
});

describe('CapabilityDenied', () => {
    test('should be an AetherError', () => {
        expect(CapabilityDenied.prototype).toBeInstanceOf(AetherError);
    });

    test('should be throwable', () => {
        expect(() => {
            throw new CapabilityDenied('NETWORK_OUTBOUND');
        }).toThrow(CapabilityDenied);
    });

    test('should format message with prefix', () => {
        const error = new CapabilityDenied('NETWORK_OUTBOUND required');
        expect(error.message).toContain('Capability denied');
        expect(error.message).toContain('NETWORK_OUTBOUND required');
    });

    test('should be catchable as AetherError', () => {
        expect(() => {
            throw new CapabilityDenied('test');
        }).toThrow(AetherError);
    });
});

describe('ActorNotFound', () => {
    test('should be an AetherError', () => {
        expect(ActorNotFound.prototype).toBeInstanceOf(AetherError);
    });

    test('should be throwable', () => {
        expect(() => {
            throw new ActorNotFound('my_actor');
        }).toThrow(ActorNotFound);
    });

    test('should format message with actor name', () => {
        const error = new ActorNotFound('my_actor');
        expect(error.message).toContain('Actor not found');
        expect(error.message).toContain('my_actor');
    });

    test('should be catchable as AetherError', () => {
        expect(() => {
            throw new ActorNotFound('test');
        }).toThrow(AetherError);
    });
});

describe('StateError', () => {
    test('should be an AetherError', () => {
        expect(StateError.prototype).toBeInstanceOf(AetherError);
    });

    test('should be throwable', () => {
        expect(() => {
            throw new StateError('Failed to read state');
        }).toThrow(StateError);
    });

    test('should preserve message', () => {
        const error = new StateError('Key not found');
        expect(error.message).toBe('Key not found');
    });

    test('should be catchable as AetherError', () => {
        expect(() => {
            throw new StateError('test');
        }).toThrow(AetherError);
    });
});

describe('RpcError', () => {
    test('should be an AetherError', () => {
        expect(RpcError.prototype).toBeInstanceOf(AetherError);
    });

    test('should be throwable', () => {
        expect(() => {
            throw new RpcError('RPC call failed');
        }).toThrow(RpcError);
    });

    test('should preserve message', () => {
        const error = new RpcError('Connection timeout');
        expect(error.message).toBe('Connection timeout');
    });

    test('should have code attribute', () => {
        const error = new RpcError('Timeout', 'TIMEOUT');
        expect(error.code).toBe('TIMEOUT');
    });

    test('should have undefined code by default', () => {
        const error = new RpcError('Generic error');
        expect(error.code).toBeUndefined();
    });

    test('should be catchable as AetherError', () => {
        expect(() => {
            throw new RpcError('test');
        }).toThrow(AetherError);
    });

    test('should distinguish error codes', () => {
        const timeoutError = new RpcError('Timeout', 'TIMEOUT');
        const notFoundError = new RpcError('Not found', 'NOT_FOUND');

        expect(timeoutError.code).toBe('TIMEOUT');
        expect(notFoundError.code).toBe('NOT_FOUND');
        expect(timeoutError.code).not.toBe(notFoundError.code);
    });
});

describe('Exception Hierarchy', () => {
    test('all exceptions should inherit from AetherError', () => {
        const exceptions = [CapabilityDenied, ActorNotFound, StateError, RpcError];
        exceptions.forEach(Exception => {
            expect(Exception.prototype).toBeInstanceOf(AetherError);
        });
    });

    test('all exceptions should inherit from Error', () => {
        const exceptions = [AetherError, CapabilityDenied, ActorNotFound, StateError, RpcError];
        exceptions.forEach(Exception => {
            expect(Exception.prototype).toBeInstanceOf(Error);
        });
    });

    test('all should be catchable with AetherError', () => {
        const exceptions = [new CapabilityDenied('test'), new ActorNotFound('test'), new StateError('test'), new RpcError('test')];
        exceptions.forEach(error => {
            expect(() => { throw error; }).toThrow(AetherError);
        });
    });
});
