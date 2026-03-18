package io.aether.sdk.capabilities;

import java.util.*;
import java.util.stream.Collectors;

/**
 * A set of capabilities for authorization.
 */
public final class CapabilitySet {
    private final Set<Capability> capabilities;
    
    public CapabilitySet() {
        this.capabilities = new HashSet<>();
    }
    
    public CapabilitySet(Collection<Capability> capabilities) {
        this.capabilities = new HashSet<>(capabilities);
    }
    
    public CapabilitySet(Capability... capabilities) {
        this.capabilities = new HashSet<>(Arrays.asList(capabilities));
    }
    
    /**
     * Add a capability to the set.
     */
    public CapabilitySet add(Capability capability) {
        capabilities.add(capability);
        return this;
    }
    
    /**
     * Add a capability by string value.
     */
    public CapabilitySet add(String capability) {
        capabilities.add(new Capability(capability));
        return this;
    }
    
    /**
     * Remove a capability from the set.
     */
    public CapabilitySet remove(Capability capability) {
        capabilities.remove(capability);
        return this;
    }
    
    /**
     * Check if the set contains a capability.
     */
    public boolean contains(Capability capability) {
        return capabilities.contains(capability);
    }
    
    /**
     * Check if the set contains a capability by string value.
     */
    public boolean contains(String capability) {
        return capabilities.contains(new Capability(capability));
    }
    
    /**
     * Check if any capability in this set implies the required capability.
     * This is the main authorization check.
     */
    public boolean allows(Capability required) {
        for (Capability cap : capabilities) {
            if (cap.implies(required)) {
                return true;
            }
        }
        return false;
    }
    
    /**
     * Check if any capability in this set implies the required capability.
     */
    public boolean allows(String required) {
        return allows(new Capability(required));
    }
    
    /**
     * Check if this set allows all of the required capabilities.
     */
    public boolean allowsAll(Collection<Capability> required) {
        for (Capability cap : required) {
            if (!allows(cap)) {
                return false;
            }
        }
        return true;
    }
    
    /**
     * Check if this set allows any of the required capabilities.
     */
    public boolean allowsAny(Collection<Capability> required) {
        for (Capability cap : required) {
            if (allows(cap)) {
                return true;
            }
        }
        return false;
    }
    
    /**
     * Get all capabilities in this set.
     */
    public Set<Capability> getCapabilities() {
        return Collections.unmodifiableSet(capabilities);
    }
    
    /**
     * Get the number of capabilities.
     */
    public int size() {
        return capabilities.size();
    }
    
    /**
     * Check if the set is empty.
     */
    public boolean isEmpty() {
        return capabilities.isEmpty();
    }
    
    /**
     * Create a new CapabilitySet by merging with another.
     */
    public CapabilitySet merge(CapabilitySet other) {
        Set<Capability> merged = new HashSet<>(this.capabilities);
        merged.addAll(other.capabilities);
        return new CapabilitySet(merged);
    }
    
    /**
     * Create a builder for constructing CapabilitySet.
     */
    public static Builder builder() {
        return new Builder();
    }
    
    /**
     * Create an empty CapabilitySet.
     */
    public static CapabilitySet empty() {
        return new CapabilitySet();
    }
    
    /**
     * Create a CapabilitySet with all capabilities.
     */
    public static CapabilitySet all() {
        return new CapabilitySet(Capability.ALL);
    }
    
    /**
     * Create a CapabilitySet from string values.
     */
    public static CapabilitySet of(String... capabilities) {
        return new CapabilitySet(
            Arrays.stream(capabilities)
                .map(Capability::new)
                .collect(Collectors.toList())
        );
    }
    
    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        CapabilitySet that = (CapabilitySet) o;
        return capabilities.equals(that.capabilities);
    }
    
    @Override
    public int hashCode() {
        return capabilities.hashCode();
    }
    
    @Override
    public String toString() {
        return capabilities.stream()
            .map(Capability::toString)
            .collect(Collectors.joining(", ", "CapabilitySet[", "]"));
    }
    
    /**
     * Builder for CapabilitySet.
     */
    public static class Builder {
        private final Set<Capability> capabilities = new HashSet<>();
        
        public Builder add(Capability capability) {
            capabilities.add(capability);
            return this;
        }
        
        public Builder add(String capability) {
            capabilities.add(new Capability(capability));
            return this;
        }
        
        public Builder addAll(Collection<Capability> capabilities) {
            this.capabilities.addAll(capabilities);
            return this;
        }
        
        public CapabilitySet build() {
            return new CapabilitySet(capabilities);
        }
    }
}
