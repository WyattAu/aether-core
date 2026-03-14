export enum Capability {
    NETWORK_OUTBOUND = 1 << 0,
    NETWORK_INBOUND = 1 << 1,
    STATE_READ = 1 << 2,
    STATE_WRITE = 1 << 3,
    FS_READ = 1 << 4,
    FS_WRITE = 1 << 5,
    ACTOR_MESSAGING = 1 << 6,
    LOG = 1 << 7,
    TIME = 1 << 8,
    RANDOM = 1 << 9,
    ENVIRONMENT = 1 << 10,
    HTTP_CLIENT = 1 << 11,
    HTTP_SERVER = 1 << 12,
}

export class CapabilitySet {
    private caps: number = 0;

    constructor(...capabilities: Capability[]) {
        capabilities.forEach(cap => this.add(cap));
    }

    add(cap: Capability): void {
        this.caps |= cap;
    }

    has(cap: Capability): boolean {
        return (this.caps & cap) !== 0;
    }

    hasNetwork(): boolean {
        return this.has(Capability.NETWORK_OUTBOUND) || this.has(Capability.NETWORK_INBOUND);
    }

    hasState(): boolean {
        return this.has(Capability.STATE_READ) || this.has(Capability.STATE_WRITE);
    }

    toNumber(): number {
        return this.caps;
    }

    static fromNumber(value: number): CapabilitySet {
        const set = new CapabilitySet();
        set.caps = value;
        return set;
    }
}
