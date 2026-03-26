/**
 * Actor Capability System.
 *
 * Defines capability flags and the {@link CapabilitySet} class for managing
 * actor permissions. Capabilities control what resources an actor is allowed
 * to access (network, filesystem, state, etc.).
 *
 * @module aether/capabilities
 */

/**
 * Bit-flag enumeration of actor capabilities.
 *
 * Each capability represents a permission granted to an actor. Capabilities
 * are combined using bitwise operations within a {@link CapabilitySet}.
 */
export enum Capability {
    /** Permit outbound network connections. */
    NETWORK_OUTBOUND = 1 << 0,
    /** Permit inbound network connections. */
    NETWORK_INBOUND = 1 << 1,
    /** Permit reading from the actor's state store. */
    STATE_READ = 1 << 2,
    /** Permit writing to the actor's state store. */
    STATE_WRITE = 1 << 3,
    /** Permit reading from the filesystem. */
    FS_READ = 1 << 4,
    /** Permit writing to the filesystem. */
    FS_WRITE = 1 << 5,
    /** Permit inter-actor messaging. */
    ACTOR_MESSAGING = 1 << 6,
    /** Permit logging. */
    LOG = 1 << 7,
    /** Permit time-related operations. */
    TIME = 1 << 8,
    /** Permit random number generation. */
    RANDOM = 1 << 9,
    /** Permit access to environment variables. */
    ENVIRONMENT = 1 << 10,
    /** Permit outbound HTTP client requests. */
    HTTP_CLIENT = 1 << 11,
    /** Permit running an HTTP server. */
    HTTP_SERVER = 1 << 12,
}

/**
 * A set of capabilities represented as a bitmask.
 *
 * Provides methods to add, query, and serialize capability sets. Internally
 * uses a single numeric bitmask for efficient storage and bitwise operations.
 *
 * @example
 * ```typescript
 * const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND, Capability.STATE_READ);
 *
 * caps.add(Capability.LOG);
 * caps.has(Capability.NETWORK_OUTBOUND); // true
 * caps.hasNetwork();                      // true
 * caps.hasState();                        // true
 *
 * // Serialize and deserialize
 * const num = caps.toNumber();
 * const restored = CapabilitySet.fromNumber(num);
 * ```
 */
export class CapabilitySet {
    private caps: number = 0;

    /**
     * Create a CapabilitySet with initial capabilities.
     *
     * @param capabilities - Zero or more capabilities to include initially.
     */
    constructor(...capabilities: Capability[]) {
        capabilities.forEach(cap => this.add(cap));
    }

    /**
     * Add a capability to the set.
     *
     * @param cap - The capability to add.
     */
    add(cap: Capability): void {
        this.caps |= cap;
    }

    /**
     * Check whether a specific capability is present.
     *
     * @param cap - The capability to check.
     * @returns `true` if the capability is in the set.
     */
    has(cap: Capability): boolean {
        return (this.caps & cap) !== 0;
    }

    /**
     * Check whether any network capability is present.
     *
     * @returns `true` if either {@link Capability.NETWORK_OUTBOUND} or
     *          {@link Capability.NETWORK_INBOUND} is set.
     */
    hasNetwork(): boolean {
        return this.has(Capability.NETWORK_OUTBOUND) || this.has(Capability.NETWORK_INBOUND);
    }

    /**
     * Check whether any state capability is present.
     *
     * @returns `true` if either {@link Capability.STATE_READ} or
     *          {@link Capability.STATE_WRITE} is set.
     */
    hasState(): boolean {
        return this.has(Capability.STATE_READ) || this.has(Capability.STATE_WRITE);
    }

    /**
     * Serialize the capability set to a numeric bitmask.
     *
     * @returns The numeric representation of all capabilities in the set.
     */
    toNumber(): number {
        return this.caps;
    }

    /**
     * Deserialize a numeric bitmask into a CapabilitySet.
     *
     * @param value - The numeric bitmask.
     * @returns A new CapabilitySet containing the represented capabilities.
     */
    static fromNumber(value: number): CapabilitySet {
        const set = new CapabilitySet();
        set.caps = value;
        return set;
    }
}
