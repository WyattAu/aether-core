package io.aether.sdk.errors;

/**
 * Base exception for all Aether SDK errors.
 */
public class AetherException extends RuntimeException {
    private final String code;
    
    public AetherException(String code, String message) {
        super(message);
        this.code = code;
    }
    
    public AetherException(String code, String message, Throwable cause) {
        super(message, cause);
        this.code = code;
    }
    
    public String getCode() {
        return code;
    }
    
    public static final class Codes {
        public static final String INTERNAL = "INTERNAL_ERROR";
        public static final String CAPABILITY_DENIED = "CAPABILITY_DENIED";
        public static final String ACTOR_NOT_FOUND = "ACTOR_NOT_FOUND";
        public static final String TIMEOUT = "TIMEOUT";
        public static final String INVALID_ARGUMENT = "INVALID_ARGUMENT";
        public static final String STORAGE_READ = "STORAGE_READ_ERROR";
        public static final String STORAGE_WRITE = "STORAGE_WRITE_ERROR";
        public static final String MESH_CONNECTION = "MESH_CONNECTION_ERROR";
        public static final String VALIDATION = "VALIDATION_ERROR";
    }
    
    /**
     * Create an internal error.
     */
    public static AetherException internal(String message) {
        return new AetherException(Codes.INTERNAL, message);
    }
    
    /**
     * Create an internal error with cause.
     */
    public static AetherException internal(String message, Throwable cause) {
        return new AetherException(Codes.INTERNAL, message, cause);
    }
    
    /**
     * Create a capability denied error.
     */
    public static AetherException capabilityDenied(String capability) {
        return new AetherException(Codes.CAPABILITY_DENIED, 
            "Capability denied: " + capability);
    }
    
    /**
     * Create an actor not found error.
     */
    public static AetherException actorNotFound(String actorId) {
        return new AetherException(Codes.ACTOR_NOT_FOUND, 
            "Actor not found: " + actorId);
    }
    
    /**
     * Create a timeout error.
     */
    public static AetherException timeout(String operation) {
        return new AetherException(Codes.TIMEOUT, 
            "Operation timed out: " + operation);
    }
    
    /**
     * Create an invalid argument error.
     */
    public static AetherException invalidArgument(String message) {
        return new AetherException(Codes.INVALID_ARGUMENT, message);
    }
    
    /**
     * Create a storage read error.
     */
    public static AetherException storageRead(String key, Throwable cause) {
        return new AetherException(Codes.STORAGE_READ, 
            "Failed to read from storage: " + key, cause);
    }
    
    /**
     * Create a storage write error.
     */
    public static AetherException storageWrite(String key, Throwable cause) {
        return new AetherException(Codes.STORAGE_WRITE, 
            "Failed to write to storage: " + key, cause);
    }
    
    /**
     * Create a mesh connection error.
     */
    public static AetherException meshConnection(String message, Throwable cause) {
        return new AetherException(Codes.MESH_CONNECTION, message, cause);
    }
}
