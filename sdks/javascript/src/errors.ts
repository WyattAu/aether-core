export class AetherError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'AetherError';
    }
}

export class CapabilityDenied extends AetherError {
    constructor(message: string) {
        super(`Capability denied: ${message}`);
        this.name = 'CapabilityDenied';
    }
}

export class ActorNotFound extends AetherError {
    constructor(actor: string) {
        super(`Actor not found: ${actor}`);
        this.name = 'ActorNotFound';
    }
}

export class RpcError extends AetherError {
    constructor(
        message: string,
        public readonly code?: string
    ) {
        super(message);
        this.name = 'RpcError';
    }
}

export class StateError extends AetherError {
    constructor(message: string) {
        super(message);
        this.name = 'StateError';
    }
}
