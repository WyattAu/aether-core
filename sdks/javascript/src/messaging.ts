/**
 * Messaging Types and Message Class.
 *
 * Defines the core messaging primitives used for inter-actor communication:
 * message types, priorities, payloads, and the {@link Message} class with
 * factory methods and serialization support.
 *
 * @module aether/messaging
 */

/**
 * Enumeration of standard message types in the Aether framework.
 *
 * These types control how the actor runtime dispatches and processes messages.
 */
export enum MessageType {
    /** Signals the actor to start. */
    START = 'start',
    /** Signals the actor to stop. */
    STOP = 'stop',
    /** A system-level signal (e.g., SIGTERM, SIGINT). */
    SIGNAL = 'signal',
    /** An RPC request expecting a response. */
    RPC_REQUEST = 'rpc_request',
    /** An RPC response correlated to a prior request. */
    RPC_RESPONSE = 'rpc_response',
    /** Application-specific custom message. */
    CUSTOM = 'custom',
}

/**
 * Message priority levels for dispatch ordering.
 *
 * Higher-priority messages are delivered before lower-priority ones.
 */
export enum Priority {
    /** Low priority — processed when idle. */
    LOW = 0,
    /** Normal (default) priority. */
    NORMAL = 1,
    /** High priority — processed ahead of normal messages. */
    HIGH = 2,
    /** Critical priority — processed immediately. */
    CRITICAL = 3,
}

/**
 * A generic, string-keyed message payload.
 *
 * @example
 * ```typescript
 * const payload: MessagePayload = { action: 'greet', target: 'world' };
 * ```
 */
export interface MessagePayload {
    [key: string]: any;
}

/**
 * Immutable message object for inter-actor communication.
 *
 * Messages carry a type, payload, optional sender identity, correlation ID
 * (for RPC request/response matching), and a priority level. Use the static
 * factory methods to construct common message types.
 *
 * @example
 * ```typescript
 * // Create a custom message
 * const msg = Message.custom({ action: 'process' }, Priority.HIGH);
 *
 * // Create an RPC request/response pair
 * const req = Message.rpcRequest({ data: 42 }, 'corr-123');
 * const res = Message.rpcResponse({ result: 'ok' }, 'corr-123');
 * ```
 */
export class Message {
    /**
     * Create a new Message.
     *
     * @param type          - The message type.
     * @param payload       - The message payload.
     * @param sender        - Optional sender identity.
     * @param correlationId - Optional correlation ID for RPC matching.
     * @param priority      - Dispatch priority (defaults to {@link Priority.NORMAL}).
     */
    constructor(
        public readonly type: MessageType,
        public readonly payload: MessagePayload,
        public readonly sender?: string,
        public readonly correlationId?: string,
        public readonly priority: Priority = Priority.NORMAL
    ) {}

    /**
     * Create a START control message.
     *
     * @returns A new Message with type {@link MessageType.START}.
     */
    static start(): Message {
        return new Message(MessageType.START, {});
    }

    /**
     * Create a STOP control message.
     *
     * @returns A new Message with type {@link MessageType.STOP}.
     */
    static stop(): Message {
        return new Message(MessageType.STOP, {});
    }

    /**
     * Create a custom application message.
     *
     * @param payload  - The message payload.
     * @param priority - Optional dispatch priority.
     * @returns A new Message with type {@link MessageType.CUSTOM}.
     */
    static custom(payload: MessagePayload, priority?: Priority): Message {
        return new Message(MessageType.CUSTOM, payload, undefined, undefined, priority);
    }

    /**
     * Create an RPC request message.
     *
     * @param payload       - The request payload.
     * @param correlationId - Unique correlation ID to match with the response.
     * @returns A new Message with type {@link MessageType.RPC_REQUEST}.
     */
    static rpcRequest(payload: MessagePayload, correlationId: string): Message {
        return new Message(MessageType.RPC_REQUEST, payload, undefined, correlationId);
    }

    /**
     * Create an RPC response message.
     *
     * @param payload       - The response payload.
     * @param correlationId - The correlation ID from the original request.
     * @returns A new Message with type {@link MessageType.RPC_RESPONSE}.
     */
    static rpcResponse(payload: MessagePayload, correlationId: string): Message {
        return new Message(MessageType.RPC_RESPONSE, payload, undefined, correlationId);
    }

    /**
     * Serialize the message to a plain JSON object.
     *
     * @returns A plain object representation of the message.
     */
    toJSON(): object {
        return {
            type: this.type,
            payload: this.payload,
            sender: this.sender,
            correlationId: this.correlationId,
            priority: this.priority,
        };
    }

    /**
     * Deserialize a plain object back into a {@link Message}.
     *
     * @param data - The serialized message object.
     * @returns A reconstructed Message instance.
     */
    static fromJSON(data: any): Message {
        return new Message(
            data.type,
            data.payload,
            data.sender,
            data.correlationId,
            data.priority
        );
    }
}
