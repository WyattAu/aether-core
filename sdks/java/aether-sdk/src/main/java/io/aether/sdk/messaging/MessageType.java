package io.aether.sdk.messaging;

/**
 * Types of messages in the Aether system.
 */
public enum MessageType {
    /** Direct message to an actor */
    DIRECT("direct"),
    /** RPC request expecting a response */
    RPC_REQUEST("rpc_request"),
    /** RPC response to a request */
    RPC_RESPONSE("rpc_response"),
    /** Broadcast to all interested actors */
    BROADCAST("broadcast"),
    /** System-level event */
    SYSTEM("system");
    
    private final String value;
    
    MessageType(String value) {
        this.value = value;
    }
    
    public String getValue() {
        return value;
    }
    
    public static MessageType fromValue(String value) {
        for (MessageType type : values()) {
            if (type.value.equals(value)) {
                return type;
            }
        }
        throw new IllegalArgumentException("Unknown message type: " + value);
    }
}
