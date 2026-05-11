/**
 * Core Actor Model Implementation.
 *
 * Provides the base {@link Actor} class for building concurrent, message-driven
 * entities within the Aether framework. Actors communicate exclusively through
 * asynchronous messaging and maintain isolated state.
 *
 * @module aether/actor
 */

import { EventEmitter } from 'events';
import { Capability, CapabilitySet } from './capabilities';
import { Message, MessageType } from './messaging';
import { StateHandle } from './state';
import { ActorConfig, MessageHandler, RpcHandler } from './types';

/**
 * Abstract base class for all actors in the Aether framework.
 *
 * Actors are isolated units of computation that communicate via asynchronous
 * messages. Each actor owns its own state and capabilities, and extends
 * {@link EventEmitter} for lifecycle event observation.
 *
 * @example
 * ```typescript
 * class PingActor extends Actor {
 *   static get name(): string { return 'ping'; }
 *
 *   async handle(sender: string, message: Message): Promise<void> {
 *     if (message.type === MessageType.CUSTOM) {
 *       console.log(`Ping from ${sender}: ${message.payload.msg}`);
 *     }
 *   }
 * }
 *
 * const actor = new PingActor({ name: 'ping' });
 * await actor.start();
 * ```
 *
 * @fires started  - Emitted when the actor has started
 * @fires stopped  - Emitted when the actor has stopped
 */
export abstract class Actor extends EventEmitter {
    /** The set of capabilities granted to this actor. */
    protected capabilities: CapabilitySet;
    /** The state handle for persisting actor state. */
    protected state: StateHandle;
    private running: boolean = false;

    /**
     * Create a new Actor.
     *
     * @param config - Configuration for the actor, including name and optional
     *                 capability flags and initial state.
     */
    constructor(protected config: ActorConfig) {
        super();
        this.capabilities = new CapabilitySet();
        this.state = new StateHandle();
    }

    /**
     * The unique name of this actor type.
     *
     * Subclasses must implement this as a static getter. The decorator
     * {@link actor} provides a default implementation.
     *
     * @throws Error Always — subclasses must override
     */
    static get name(): string {
        throw new Error('Actor.name must be implemented');
    }

    /**
     * Handle an incoming message.
     *
     * Implement this method to define the actor's message-processing logic.
     * The actor framework calls this method for every message dispatched to
     * the actor.
     *
     * @param sender  - The identity of the sending actor.
     * @param message - The message to process.
     * @returns An optional response message, or void.
     *
     * @example
     * ```typescript
     * async handle(sender: string, message: Message): Promise<Message | void> {
     *   if (message.type === MessageType.CUSTOM) {
     *     return Message.custom({ result: 'ok' });
     *   }
     * }
     * ```
     */
    abstract handle(sender: string, message: Message): Promise<Message | void>;

    /**
     * Start the actor.
     *
     * Sets the actor to running state, calls the optional {@link onStart}
     * lifecycle hook, and emits the `'started'` event.
     *
     * @returns A promise that resolves when the actor has started.
     */
    async start(): Promise<void> {
        this.running = true;
        await this.onStart?.();
        this.emit('started');
    }

    /**
     * Stop the actor.
     *
     * Sets the actor to stopped state, calls the optional {@link onStop}
     * lifecycle hook, and emits the `'stopped'` event.
     *
     * @returns A promise that resolves when the actor has stopped.
     */
    async stop(): Promise<void> {
        this.running = false;
        await this.onStop?.();
        this.emit('stopped');
    }

    /**
     * Lifecycle hook called after the actor starts.
     *
     * Override this in subclasses to perform initialization logic such as
     * loading state, opening connections, or registering handlers.
     */
    protected async onStart?(): Promise<void>;

    /**
     * Lifecycle hook called before the actor stops.
     *
     * Override this in subclasses to perform cleanup logic such as flushing
     * state, closing connections, or deregistering handlers.
     */
    protected async onStop?(): Promise<void>;

    /**
     * Declare required capabilities for this actor.
     *
     * Adds each capability in the variadic argument list to the actor's
     * {@link CapabilitySet}. Use this in the constructor to assert the
     * permissions the actor needs.
     *
     * @param capabilities - One or more capabilities to require.
     *
     * @example
     * ```typescript
     * constructor(config: ActorConfig) {
     *   super(config);
     *   this.require(Capability.NETWORK_OUTBOUND, Capability.STATE_READ);
     * }
     * ```
     */
    require(...capabilities: Capability[]): void {
        capabilities.forEach(cap => this.capabilities.add(cap));
    }

    /**
     * Send a fire-and-forget message to another actor.
     *
     * @param target  - The identity of the target actor.
     * @param message - The message to send.
     * @returns A promise that resolves when the message has been dispatched.
     */
    async send(target: string, message: Message): Promise<void> {
        this.emit('message', { target, message });
    }

    /**
     * Perform a remote procedure call on another actor.
     *
     * @typeParam T - The expected return type of the RPC call.
     * @param target  - The identity of the target actor.
     * @param request - The request payload.
     * @param timeout - Optional timeout in milliseconds.
     * @returns A promise resolving to the RPC response.
     * @throws Error If RPC is not implemented or the call times out.
     */
    async call<T>(target: string, request: any, timeout?: number): Promise<T> {
        const correlationId = crypto.randomUUID().toString();
        const rpcMessage = Message.rpc({
            target,
            payload: request,
            correlationId,
        });

        return new Promise((resolve, reject) => {
            const timer = timeout
                ? setTimeout(() => reject(new Error(`RPC timeout: ${target}`)), timeout)
                : null;

            this.once(`rpc-response-${correlationId}`, (response: Message) => {
                if (timer) clearTimeout(timer);
                if (response.type === MessageType.ERROR) {
                    reject(new Error(response.payload as string));
                    return;
                }
                resolve(response.payload as T);
            });

            this.emit('message', rpcMessage);
        });
    }
}

/**
 * Class decorator that sets the static `name` property on an Actor subclass.
 *
 * @param config - The actor configuration containing the desired name.
 * @returns A class decorator that overrides `static get name()`.
 *
 * @example
 * ```typescript
 * @actor({ name: 'my-actor' })
 * class MyActor extends Actor {
 *   // static name is automatically 'my-actor'
 * }
 * ```
 */
export function actor(config: ActorConfig): ClassDecorator {
    // eslint-disable-next-line @typescript-eslint/ban-types
    return function <TFunction extends Function>(constructor: TFunction) {
        return class extends (constructor as unknown as new (...args: any[]) => any) {
            static get name() {
                return config.name;
            }
        } as unknown as TFunction;
    };
}
