/**
 * Aether Exception Hierarchy.
 *
 * Defines the base {@link AetherError} class and all domain-specific error
 * subclasses used throughout the Aether SDK.
 *
 * @module aether/errors
 */

/**
 * Base error class for all Aether SDK errors.
 *
 * All domain-specific exceptions extend this class, enabling consumers to
 * catch any Aether-related error with a single `catch` clause.
 *
 * @example
 * ```typescript
 * try {
 *   await actor.send(target, message);
 * } catch (e) {
 *   if (e instanceof AetherError) {
 *     console.error('Aether error:', e.message);
 *   }
 * }
 * ```
 */
export class AetherError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'AetherError';
    }
}

/**
 * Thrown when an actor attempts to use a capability it does not possess.
 *
 * Typically raised by the runtime or by resource guards (e.g., {@link HttpClient})
 * that verify capability permissions before performing an operation.
 *
 * @example
 * ```typescript
 * if (!capabilities.has(Capability.HTTP_CLIENT)) {
 *   throw new CapabilityDenied('HTTP client requires HTTP_CLIENT capability');
 * }
 * ```
 */
export class CapabilityDenied extends AetherError {
    constructor(message: string) {
        super(`Capability denied: ${message}`);
        this.name = 'CapabilityDenied';
    }
}

/**
 * Thrown when a referenced actor cannot be found.
 *
 * Raised when sending a message or performing an RPC call to a non-existent
 * actor identity.
 *
 * @param actor - The identity of the actor that was not found.
 */
export class ActorNotFound extends AetherError {
    constructor(actor: string) {
        super(`Actor not found: ${actor}`);
        this.name = 'ActorNotFound';
    }
}

/**
 * Thrown when an RPC call fails.
 *
 * Carries an optional error code for programmatic error classification.
 *
 * @example
 * ```typescript
 * try {
 *   const result = await actor.call('service', request);
 * } catch (e) {
 *   if (e instanceof RpcError) {
 *     console.error(`RPC failed (${e.code}): ${e.message}`);
 *   }
 * }
 * ```
 */
export class RpcError extends AetherError {
    /**
     * Create a new RpcError.
     *
     * @param message - Human-readable error description.
     * @param code    - Optional machine-readable error code.
     */
    constructor(
        message: string,
        public readonly code?: string
    ) {
        super(message);
        this.name = 'RpcError';
    }
}

/**
 * Thrown when a state operation fails.
 *
 * Raised on invalid keys, serialization errors, or when the state store
 * is unavailable.
 */
export class StateError extends AetherError {
    constructor(message: string) {
        super(message);
        this.name = 'StateError';
    }
}
