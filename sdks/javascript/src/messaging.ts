export enum MessageType {
    START = 'start',
    STOP = 'stop',
    SIGNAL = 'signal',
    RPC_REQUEST = 'rpc_request',
    RPC_RESPONSE = 'rpc_response',
    CUSTOM = 'custom',
}

export enum Priority {
    LOW = 0,
    NORMAL = 1,
    HIGH = 2,
    CRITICAL = 3,
}

export interface MessagePayload {
    [key: string]: any;
}

export class Message {
    constructor(
        public readonly type: MessageType,
        public readonly payload: MessagePayload,
        public readonly sender?: string,
        public readonly correlationId?: string,
        public readonly priority: Priority = Priority.NORMAL
    ) {}

    static start(): Message {
        return new Message(MessageType.START, {});
    }

    static stop(): Message {
        return new Message(MessageType.STOP, {});
    }

    static custom(payload: MessagePayload, priority?: Priority): Message {
        return new Message(MessageType.CUSTOM, payload, undefined, undefined, priority);
    }

    static rpcRequest(payload: MessagePayload, correlationId: string): Message {
        return new Message(MessageType.RPC_REQUEST, payload, undefined, correlationId);
    }

    static rpcResponse(payload: MessagePayload, correlationId: string): Message {
        return new Message(MessageType.RPC_RESPONSE, payload, undefined, correlationId);
    }

    toJSON(): object {
        return {
            type: this.type,
            payload: this.payload,
            sender: this.sender,
            correlationId: this.correlationId,
            priority: this.priority,
        };
    }

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
