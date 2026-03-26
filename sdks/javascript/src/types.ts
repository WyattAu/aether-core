/**
 * Core Type Definitions.
 *
 * Shared interfaces and type aliases used throughout the Aether SDK,
 * including actor configuration and handler function signatures.
 *
 * @module aether/types
 */

/**
 * Configuration for creating an Actor instance.
 */
export interface ActorConfig {
    /** The unique name of the actor. */
    name: string;
    /** Bitmask of capabilities (see {@link Capability}). */
    capabilities?: number;
    /** Optional initial state entries. */
    state?: Map<string, Buffer>;
}

/**
 * Function signature for handling incoming actor messages.
 *
 * @param sender  - The identity of the sending actor.
 * @param message - The message payload.
 * @returns An optional response value, or void.
 */
export interface MessageHandler {
    (sender: string, message: any): Promise<any | void>;
}

/**
 * Function signature for handling RPC requests.
 *
 * @param request - The RPC request payload.
 * @returns The RPC response value.
 */
export interface RpcHandler {
    (request: any): Promise<any>;
}
