package io.aether.sdk.capabilities;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.Arrays;
import java.util.List;

class CapabilitySetTest {

    @Test
    @DisplayName("empty set allows nothing")
    void testEmptySetAllowsNothing() {
        CapabilitySet set = CapabilitySet.empty();
        assertFalse(set.allows(Capability.ACTOR_INVOKE));
        assertFalse(set.allows(Capability.STATE_READ));
        assertTrue(set.isEmpty());
        assertEquals(0, set.size());
    }

    @Test
    @DisplayName("all set allows everything")
    void testAllSetAllowsEverything() {
        CapabilitySet set = CapabilitySet.all();
        assertTrue(set.allows(Capability.ACTOR_INVOKE));
        assertTrue(set.allows(Capability.STATE_READ));
        assertTrue(set.allows(new Capability("anything")));
    }

    @Test
    @DisplayName("add capability by object")
    void testAddCapabilityObject() {
        CapabilitySet set = CapabilitySet.empty();
        set.add(Capability.STATE_READ);
        assertTrue(set.contains(Capability.STATE_READ));
        assertEquals(1, set.size());
    }

    @Test
    @DisplayName("add capability by string")
    void testAddCapabilityString() {
        CapabilitySet set = CapabilitySet.empty();
        set.add("state:read");
        assertTrue(set.contains("state:read"));
        assertTrue(set.allows("state:read"));
    }

    @Test
    @DisplayName("remove capability")
    void testRemoveCapability() {
        CapabilitySet set = new CapabilitySet(Capability.STATE_READ, Capability.STATE_WRITE);
        set.remove(Capability.STATE_WRITE);
        assertTrue(set.contains(Capability.STATE_READ));
        assertFalse(set.contains(Capability.STATE_WRITE));
    }

    @Test
    @DisplayName("allowsAll requires all capabilities")
    void testAllowsAll() {
        CapabilitySet set = new CapabilitySet(Capability.STATE_READ, Capability.STATE_WRITE);
        assertTrue(set.allowsAll(Arrays.asList(Capability.STATE_READ, Capability.STATE_WRITE)));
        assertFalse(set.allowsAll(Arrays.asList(Capability.STATE_READ, Capability.ACTOR_INVOKE)));
    }

    @Test
    @DisplayName("allowsAny requires at least one capability")
    void testAllowsAny() {
        CapabilitySet set = new CapabilitySet(Capability.STATE_READ);
        assertTrue(set.allowsAny(Arrays.asList(Capability.STATE_READ, Capability.ACTOR_INVOKE)));
        assertFalse(set.allowsAny(Arrays.asList(Capability.ACTOR_INVOKE, Capability.MESSAGE_SEND)));
    }

    @Test
    @DisplayName("wildcard implies specific capabilities")
    void testWildcardCapability() {
        CapabilitySet set = new CapabilitySet(Capability.ALL);
        assertTrue(set.allows("actor:invoke:my-actor"));
        assertTrue(set.allows("state:read:my-key"));
    }

    @Test
    @DisplayName("domain wildcard implies domain actions")
    void testDomainWildcard() {
        CapabilitySet set = new CapabilitySet(Capability.STATE_ALL);
        assertTrue(set.allows("state:read"));
        assertTrue(set.allows("state:write"));
        assertTrue(set.allows("state:read:my-key"));
        assertFalse(set.allows("actor:invoke"));
    }

    @Test
    @DisplayName("actor invoke wildcard implies specific actor")
    void testActorInvokeWildcard() {
        CapabilitySet set = new CapabilitySet(Capability.ACTOR_INVOKE_ALL);
        assertTrue(set.allows("actor:invoke:my-actor"));
        assertTrue(set.allows("actor:invoke"));
        assertFalse(set.allows("state:read"));
    }

    @Test
    @DisplayName("merge combines two sets")
    void testMerge() {
        CapabilitySet a = new CapabilitySet(Capability.STATE_READ);
        CapabilitySet b = new CapabilitySet(Capability.ACTOR_INVOKE);
        CapabilitySet merged = a.merge(b);
        assertTrue(merged.allows(Capability.STATE_READ));
        assertTrue(merged.allows(Capability.ACTOR_INVOKE));
    }

    @Test
    @DisplayName("builder pattern")
    void testBuilder() {
        CapabilitySet set = CapabilitySet.builder()
            .add(Capability.STATE_READ)
            .add("actor:invoke")
            .build();
        assertTrue(set.allows(Capability.STATE_READ));
        assertTrue(set.allows(Capability.ACTOR_INVOKE));
        assertEquals(2, set.size());
    }

    @Test
    @DisplayName("of creates from strings")
    void testOf() {
        CapabilitySet set = CapabilitySet.of("state:read", "actor:invoke");
        assertEquals(2, set.size());
        assertTrue(set.contains("state:read"));
        assertTrue(set.contains("actor:invoke"));
    }

    @Test
    @DisplayName("getCapabilities returns unmodifiable set")
    void testGetCapabilitiesUnmodifiable() {
        CapabilitySet set = CapabilitySet.empty();
        assertThrows(UnsupportedOperationException.class, () ->
            set.getCapabilities().add(Capability.ACTOR_INVOKE));
    }

    @Test
    @DisplayName("equals and hashCode")
    void testEqualsHashCode() {
        CapabilitySet a = new CapabilitySet(Capability.STATE_READ, Capability.STATE_WRITE);
        CapabilitySet b = new CapabilitySet(Capability.STATE_READ, Capability.STATE_WRITE);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
    }

    @Test
    @DisplayName("toString contains capability names")
    void testToString() {
        CapabilitySet set = new CapabilitySet(Capability.STATE_READ);
        String str = set.toString();
        assertTrue(str.contains("CapabilitySet"));
        assertTrue(str.contains("state:read"));
    }
}
