import { EventEmitter } from 'events';
import { Capability, CapabilitySet } from './capabilities';
import { Message, MessageType } from './messaging';
import { StateHandle } from './state';
import { ActorConfig, MessageHandler, RpcHandler } from './types';

export abstract class Actor extends EventEmitter {
    protected capabilities: CapabilitySet;
    protected state: StateHandle;
    private running: boolean = false;

    constructor(protected config: ActorConfig) {
        super();
        this.capabilities = new CapabilitySet();
        this.state = new StateHandle();
    }

    static get name(): string {
        throw new Error('Actor.name must be implemented');
    }

    abstract handle(sender: string, message: Message): Promise<Message | void>;

    async start(): Promise<void> {
        this.running = true;
        await this.onStart?.();
        this.emit('started');
    }

    async stop(): Promise<void> {
        this.running = false;
        await this.onStop?.();
        this.emit('stopped');
    }

    protected async onStart?(): Promise<void>;
    protected async onStop?(): Promise<void>;

    require(...capabilities: Capability[]): void {
        capabilities.forEach(cap => this.capabilities.add(cap));
    }

    async send(target: string, message: Message): Promise<void> {
        // Implementation placeholder for inter-actor messaging
    }

    async call<T>(target: string, request: any, timeout?: number): Promise<T> {
        // RPC implementation placeholder
        throw new Error('RPC not implemented');
    }
}

export function actor(config: ActorConfig): ClassDecorator {
    return function <T extends new (...args: any[]) => any>(constructor: T) {
        return class extends constructor {
            static get name() {
                return config.name;
            }
        };
    };
}
