package io.aether.sdk.state;

import io.aether.sdk.capabilities.Capability;
import io.aether.sdk.capabilities.CapabilitySet;
import io.aether.sdk.errors.AetherException;

import java.util.*;
import java.util.concurrent.CompletableFuture;
import java.util.function.Function;

/**
 * Handle for accessing actor state.
 */
public class StateHandle {
    private final String actorId;
    private final CapabilitySet capabilities;
    private final Map<String, Object> storage;
    private final StateBackend backend;
    
    public StateHandle(String actorId, CapabilitySet capabilities) {
        this.actorId = actorId;
        this.capabilities = capabilities;
        this.storage = new HashMap<>();
        this.backend = new InMemoryStateBackend();
    }
    
    public StateHandle(String actorId, CapabilitySet capabilities, StateBackend backend) {
        this.actorId = actorId;
        this.capabilities = capabilities;
        this.storage = new HashMap<>();
        this.backend = backend;
    }
    
    /**
     * Get a value from state.
     */
    @SuppressWarnings("unchecked")
    public <T> T get(String key) {
        checkCapability(Capability.readState(key));
        try {
            return (T) storage.get(key);
        } catch (ClassCastException e) {
            throw AetherException.internal("Type mismatch for state key: " + key, e);
        }
    }
    
    /**
     * Get a value from state with a default.
     */
    @SuppressWarnings("unchecked")
    public <T> T get(String key, T defaultValue) {
        checkCapability(Capability.readState(key));
        Object value = storage.get(key);
        return value != null ? (T) value : defaultValue;
    }
    
    /**
     * Get a value from state, computing if absent.
     */
    @SuppressWarnings("unchecked")
    public <T> T computeIfAbsent(String key, Function<String, T> supplier) {
        checkCapability(Capability.readState(key));
        checkCapability(Capability.writeState(key));
        return (T) storage.computeIfAbsent(key, k -> supplier.apply(k));
    }
    
    /**
     * Set a value in state.
     */
    public void set(String key, Object value) {
        checkCapability(Capability.writeState(key));
        storage.put(key, value);
    }
    
    /**
     * Check if a key exists in state.
     */
    public boolean exists(String key) {
        checkCapability(Capability.readState(key));
        return storage.containsKey(key);
    }
    
    /**
     * Delete a key from state.
     */
    public void delete(String key) {
        checkCapability(Capability.writeState(key));
        storage.remove(key);
    }
    
    /**
     * Get all keys in state.
     */
    public Set<String> keys() {
        checkCapability(Capability.STATE_READ);
        return Collections.unmodifiableSet(storage.keySet());
    }
    
    /**
     * Clear all state.
     */
    public void clear() {
        checkCapability(Capability.STATE_WRITE);
        storage.clear();
    }
    
    /**
     * Get the number of keys in state.
     */
    public int size() {
        return storage.size();
    }
    
    /**
     * Persist state to backend (async).
     */
    public CompletableFuture<Void> persist() {
        checkCapability(Capability.STATE_WRITE);
        return backend.save(actorId, new HashMap<>(storage));
    }
    
    /**
     * Load state from backend (async).
     */
    public CompletableFuture<Void> load() {
        checkCapability(Capability.STATE_READ);
        return backend.load(actorId).thenAccept(loaded -> {
            storage.clear();
            storage.putAll(loaded);
        });
    }
    
    private void checkCapability(Capability required) {
        if (!capabilities.allows(required)) {
            throw AetherException.capabilityDenied(required.getValue());
        }
    }
    
    /**
     * State backend interface for persistence.
     */
    public interface StateBackend {
        CompletableFuture<Map<String, Object>> load(String actorId);
        CompletableFuture<Void> save(String actorId, Map<String, Object> state);
    }
    
    /**
     * In-memory state backend (for testing).
     */
    public static class InMemoryStateBackend implements StateBackend {
        private final Map<String, Map<String, Object>> storage = new HashMap<>();
        
        @Override
        public CompletableFuture<Map<String, Object>> load(String actorId) {
            return CompletableFuture.completedFuture(
                storage.getOrDefault(actorId, new HashMap<>())
            );
        }
        
        @Override
        public CompletableFuture<Void> save(String actorId, Map<String, Object> state) {
            storage.put(actorId, new HashMap<>(state));
            return CompletableFuture.completedFuture(null);
        }
    }
}
