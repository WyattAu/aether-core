import { Capability, CapabilitySet } from '../src/capabilities';

describe('Capability', () => {
    test('should have unique values', () => {
        const values = Object.values(Capability).filter(v => typeof v === 'number') as number[];
        const uniqueValues = new Set(values);
        expect(values.length).toBe(uniqueValues.size);
    });

    test('should have NETWORK_OUTBOUND capability', () => {
        expect(Capability.NETWORK_OUTBOUND).toBeDefined();
        expect(typeof Capability.NETWORK_OUTBOUND).toBe('number');
    });

    test('should have NETWORK_INBOUND capability', () => {
        expect(Capability.NETWORK_INBOUND).toBeDefined();
        expect(typeof Capability.NETWORK_INBOUND).toBe('number');
    });

    test('should have STATE_READ capability', () => {
        expect(Capability.STATE_READ).toBeDefined();
        expect(typeof Capability.STATE_READ).toBe('number');
    });

    test('should have STATE_WRITE capability', () => {
        expect(Capability.STATE_WRITE).toBeDefined();
        expect(typeof Capability.STATE_WRITE).toBe('number');
    });

    test('should have FS_READ capability', () => {
        expect(Capability.FS_READ).toBeDefined();
        expect(typeof Capability.FS_READ).toBe('number');
    });

    test('should have FS_WRITE capability', () => {
        expect(Capability.FS_WRITE).toBeDefined();
        expect(typeof Capability.FS_WRITE).toBe('number');
    });

    test('should have ACTOR_MESSAGING capability', () => {
        expect(Capability.ACTOR_MESSAGING).toBeDefined();
        expect(typeof Capability.ACTOR_MESSAGING).toBe('number');
    });

    test('should have expected capability count', () => {
        const capabilities = Object.keys(Capability).filter(k => isNaN(Number(k)));
        expect(capabilities.length).toBe(13);
    });
});

describe('CapabilitySet', () => {
    test('should initialize empty', () => {
        const caps = new CapabilitySet();
        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(false);
        expect(caps.has(Capability.STATE_READ)).toBe(false);
    });

    test('should initialize with capabilities', () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND, Capability.STATE_READ);
        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(true);
        expect(caps.has(Capability.STATE_READ)).toBe(true);
        expect(caps.has(Capability.STATE_WRITE)).toBe(false);
    });

    test('should add capability', () => {
        const caps = new CapabilitySet();
        caps.add(Capability.NETWORK_OUTBOUND);
        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(true);
    });

    test('should add multiple capabilities', () => {
        const caps = new CapabilitySet();
        caps.add(Capability.NETWORK_OUTBOUND);
        caps.add(Capability.STATE_READ);
        caps.add(Capability.STATE_WRITE);

        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(true);
        expect(caps.has(Capability.STATE_READ)).toBe(true);
        expect(caps.has(Capability.STATE_WRITE)).toBe(true);
        expect(caps.has(Capability.FS_READ)).toBe(false);
    });

    test('should detect network capability', () => {
        const capsWithNetwork = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        expect(capsWithNetwork.hasNetwork()).toBe(true);

        const capsWithInbound = new CapabilitySet(Capability.NETWORK_INBOUND);
        expect(capsWithInbound.hasNetwork()).toBe(true);

        const capsWithoutNetwork = new CapabilitySet(Capability.STATE_READ);
        expect(capsWithoutNetwork.hasNetwork()).toBe(false);
    });

    test('should detect state capability', () => {
        const capsWithState = new CapabilitySet(Capability.STATE_READ);
        expect(capsWithState.hasState()).toBe(true);

        const capsWithWrite = new CapabilitySet(Capability.STATE_WRITE);
        expect(capsWithWrite.hasState()).toBe(true);

        const capsWithoutState = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        expect(capsWithoutState.hasState()).toBe(false);
    });

    test('should return false for missing capability', () => {
        const caps = new CapabilitySet(Capability.NETWORK_OUTBOUND);
        expect(caps.has(Capability.STATE_READ)).toBe(false);
    });

    test('should not duplicate when adding same capability', () => {
        const caps = new CapabilitySet();
        caps.add(Capability.NETWORK_OUTBOUND);
        caps.add(Capability.NETWORK_OUTBOUND);
        expect(caps.has(Capability.NETWORK_OUTBOUND)).toBe(true);
    });

    test('should add all capabilities', () => {
        const caps = new CapabilitySet();
        Object.values(Capability).forEach(cap => {
            if (typeof cap === 'number') {
                caps.add(cap);
            }
        });

        Object.keys(Capability).forEach(key => {
            if (isNaN(Number(key))) {
                expect(caps.has((Capability as any)[key as any])).toBe(true);
            }
        });
    });
});
