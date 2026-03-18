package io.aether.sdk.capabilities;

import java.util.*;
import java.util.regex.Pattern;

/**
 * Represents a single capability in the Aether system.
 * 
 * Capabilities are strings that follow a hierarchical format:
 * - "actor:invoke" - Invoke any actor
 * - "actor:invoke:my-actor" - Invoke specific actor
 * - "state:read" - Read state
 * - "state:write" - Write state
 */
public final class Capability {
    private final String value;
    private final String[] parts;
    
    public Capability(String value) {
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException("Capability cannot be null or blank");
        }
        this.value = value;
        this.parts = value.split(":");
    }
    
    public String getValue() {
        return value;
    }
    
    public String[] getParts() {
        return parts.clone();
    }
    
    public String getDomain() {
        return parts[0];
    }
    
    public String getAction() {
        return parts.length > 1 ? parts[1] : null;
    }
    
    public String getResource() {
        return parts.length > 2 ? parts[2] : null;
    }
    
    /**
     * Check if this capability implies another capability.
     * A capability implies another if all its parts match or are wildcards.
     */
    public boolean implies(Capability other) {
        if (this.parts.length > other.parts.length) {
            return false;
        }
        
        for (int i = 0; i < this.parts.length; i++) {
            String thisPart = this.parts[i];
            String otherPart = other.parts[i];
            
            if (!thisPart.equals("*") && !thisPart.equals(otherPart)) {
                return false;
            }
        }
        
        return true;
    }
    
    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        Capability that = (Capability) o;
        return value.equals(that.value);
    }
    
    @Override
    public int hashCode() {
        return value.hashCode();
    }
    
    @Override
    public String toString() {
        return value;
    }
    
    // Predefined capabilities
    public static final Capability ACTOR_INVOKE = new Capability("actor:invoke");
    public static final Capability ACTOR_INVOKE_ALL = new Capability("actor:invoke:*");
    public static final Capability STATE_READ = new Capability("state:read");
    public static final Capability STATE_WRITE = new Capability("state:write");
    public static final Capability STATE_ALL = new Capability("state:*");
    public static final Capability HTTP_REQUEST = new Capability("http:request");
    public static final Capability MESSAGE_SEND = new Capability("message:send");
    public static final Capability MESSAGE_RECEIVE = new Capability("message:receive");
    public static final Capability MESH_CONNECT = new Capability("mesh:connect");
    public static final Capability ALL = new Capability("*");
    
    /**
     * Create a capability for invoking a specific actor.
     */
    public static Capability invokeActor(String actorId) {
        return new Capability("actor:invoke:" + actorId);
    }
    
    /**
     * Create a capability for reading a specific state key.
     */
    public static Capability readState(String key) {
        return new Capability("state:read:" + key);
    }
    
    /**
     * Create a capability for writing a specific state key.
     */
    public static Capability writeState(String key) {
        return new Capability("state:write:" + key);
    }
}
