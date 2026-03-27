package io.aether.sdk.state;

import io.aether.sdk.capabilities.*;
import io.aether.sdk.errors.AetherException;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;

class StateHandleTest {

    private StateHandle state;

    @BeforeEach
    void setUp() {
        CapabilitySet caps = CapabilitySet.all();
        state = new StateHandle("actor-1", caps);
    }

    @Test
    @DisplayName("get returns null for missing key")
    void testGetMissing() {
        assertNull(state.get("nonexistent"));
    }

    @Test
    @DisplayName("get with default returns default for missing key")
    void testGetWithDefault() {
        assertEquals("fallback", state.get("missing", "fallback"));
    }

    @Test
    @DisplayName("set and get round-trip")
    void testSetGet() {
        state.set("key1", "value1");
        assertEquals("value1", state.get("key1"));
    }

    @Test
    @DisplayName("set overwrites existing value")
    void testSetOverwrite() {
        state.set("key1", "v1");
        state.set("key1", "v2");
        assertEquals("v2", state.get("key1"));
    }

    @Test
    @DisplayName("exists returns true for existing key")
    void testExistsTrue() {
        state.set("key1", "val");
        assertTrue(state.exists("key1"));
    }

    @Test
    @DisplayName("exists returns false for missing key")
    void testExistsFalse() {
        assertFalse(state.exists("missing"));
    }

    @Test
    @DisplayName("delete removes key")
    void testDelete() {
        state.set("key1", "val");
        state.delete("key1");
        assertFalse(state.exists("key1"));
        assertNull(state.get("key1"));
    }

    @Test
    @DisplayName("delete non-existent key is no-op")
    void testDeleteNonExistent() {
        assertDoesNotThrow(() -> state.delete("nope"));
    }

    @Test
    @DisplayName("keys returns all keys")
    void testKeys() {
        state.set("a", 1);
        state.set("b", 2);
        Set<String> keys = state.keys();
        assertTrue(keys.contains("a"));
        assertTrue(keys.contains("b"));
        assertEquals(2, keys.size());
    }

    @Test
    @DisplayName("keys is unmodifiable")
    void testKeysUnmodifiable() {
        assertThrows(UnsupportedOperationException.class, () ->
            state.keys().add("new"));
    }

    @Test
    @DisplayName("size returns count of keys")
    void testSize() {
        assertEquals(0, state.size());
        state.set("a", 1);
        assertEquals(1, state.size());
        state.set("b", 2);
        assertEquals(2, state.size());
    }

    @Test
    @DisplayName("clear removes all keys")
    void testClear() {
        state.set("a", 1);
        state.set("b", 2);
        state.clear();
        assertEquals(0, state.size());
        assertTrue(state.keys().isEmpty());
    }

    @Test
    @DisplayName("computeIfAbsent creates if missing")
    void testComputeIfAbsentCreates() {
        String result = state.computeIfAbsent("key", k -> "computed-" + k);
        assertEquals("computed-key", result);
        assertEquals("computed-key", state.get("key"));
    }

    @Test
    @DisplayName("computeIfAbsent returns existing value")
    void testComputeIfAbsentExisting() {
        state.set("key", "existing");
        String result = state.computeIfAbsent("key", k -> "computed-" + k);
        assertEquals("existing", result);
    }

    @Test
    @DisplayName("capability check throws on denied read")
    void testCapabilityDeniedRead() {
        CapabilitySet empty = CapabilitySet.empty();
        StateHandle restricted = new StateHandle("actor-2", empty);
        assertThrows(AetherException.class, () -> restricted.get("key"));
    }

    @Test
    @DisplayName("capability check throws on denied write")
    void testCapabilityDeniedWrite() {
        CapabilitySet empty = CapabilitySet.empty();
        StateHandle restricted = new StateHandle("actor-2", empty);
        assertThrows(AetherException.class, () -> restricted.set("key", "val"));
    }

    @Test
    @DisplayName("persist completes successfully")
    void testPersist() {
        state.set("key", "val");
        CompletableFuture<Void> result = state.persist();
        assertTrue(result.isDone());
        assertNull(result.join());
    }

    @Test
    @DisplayName("load completes successfully")
    void testLoad() {
        CompletableFuture<Void> result = state.load();
        assertTrue(result.isDone());
        assertNull(result.join());
    }

    @Test
    @DisplayName("custom backend persist and load")
    void testCustomBackend() {
        StateHandle.StateBackend backend = new StateHandle.InMemoryStateBackend();
        StateHandle s1 = new StateHandle("a", CapabilitySet.all(), backend);
        s1.set("x", 42);
        s1.persist().join();

        StateHandle s2 = new StateHandle("a", CapabilitySet.all(), backend);
        s2.load().join();
        assertEquals(42, s2.get("x"));
    }
}
