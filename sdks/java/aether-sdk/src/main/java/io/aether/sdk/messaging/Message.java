package io.aether.sdk.messaging;

import java.time.Instant;
import java.util.*;

/**
 * Represents a message in the Aether actor system.
 */
public final class Message {
    private final String id;
    private final MessageType type;
    private final Object payload;
    private final String sender;
    private final String receiver;
    private final String correlationId;
    private final Priority priority;
    private final Instant timestamp;
    private final Map<String, String> metadata;
    
    private Message(Builder builder) {
        this.id = builder.id;
        this.type = builder.type;
        this.payload = builder.payload;
        this.sender = builder.sender;
        this.receiver = builder.receiver;
        this.correlationId = builder.correlationId;
        this.priority = builder.priority;
        this.timestamp = builder.timestamp != null ? Instant.now() : builder.timestamp;
        this.metadata = builder.metadata != null ? new HashMap<>() : new HashMap<>();
    }
    
    public String getId() {
        return id;
    }
    
    public MessageType getType() {
        return type;
    }
    
    public Object getPayload() {
        return payload;
    }
    
    public String getSender() {
        return sender;
    }
    
    public String getReceiver() {
        return receiver;
    }
    
    public String getCorrelationId() {
        return correlationId;
    }
    
    public Priority getPriority() {
        return priority;
    }
    
    public Instant getTimestamp() {
        return timestamp;
    }
    
    public Map<String, String> getMetadata() {
        return Collections.unmodifiableMap(metadata);
    }
    
    public Optional<String> getMetadata(String key) {
        return Optional.ofNullable(metadata.get(key));
    }
    
    /**
     * Create a builder for constructing messages.
     */
    public static Builder builder() {
        return new Builder();
    }
    
    /**
     * Create a direct message.
     */
    public static Builder direct(String sender, String receiver, Object payload) {
        return builder()
            .type(MessageType.DIRECT)
            .sender(sender)
            .receiver(receiver)
            .payload(payload)
            .priority(Priority.NORMAL);
            .timestamp(Instant.now());
            .build();
    }
    
    /**
     * Create an RPC request message.
     */
    public static Builder rpcRequest(String sender, String receiver, Object payload, String correlationId) {
        return builder()
            .type(MessageType.RPC_REQUEST)
            .sender(sender)
            .receiver(receiver)
            .payload(payload)
            .correlationId(correlationId)
            .priority(Priority.HIGH)
            .timestamp(Instant.now())
            .build();
    }
    
    /**
     * Create an RPC response message.
     */
    public static Builder rpcResponse(String sender, String receiver, Object payload, String correlationId) {
        return builder()
            .type(MessageType.RPC_RESPONSE)
            .sender(sender)
            .receiver(receiver)
            .payload(payload)
            .correlationId(correlationId)
            .priority(Priority.HIGH)
            .timestamp(Instant.now())
            .build();
    }
    
    /**
     * Create a broadcast message.
     */
    public static Builder broadcast(String sender, Object payload) {
        return builder()
            .type(MessageType.BROADCAST)
            .sender(sender)
            .payload(payload)
            .priority(Priority.NORMAL)
            .timestamp(Instant.now())
            .build();
    }
    
    /**
     * Builder for constructing messages.
     */
    public static final class Builder {
        private String id = UUID.randomUUID().toString();
        private MessageType type = MessageType.DIRECT;
        private Object payload;
        private String sender;
        private String receiver;
        private String correlationId;
        private Priority priority = Priority.NORMAL;
        private Instant timestamp;
        private Map<String, String> metadata = new HashMap<>();
        
        public Builder id(String id) {
            this.id = id;
            return this;
        }
        
        public Builder type(MessageType type) {
            this.type = type;
            return this;
        }
        
        public Builder payload(Object payload) {
            this.payload = payload;
            return this;
        }
        
        public Builder sender(String sender) {
            this.sender = sender;
            return this;
        }
        
        public Builder receiver(String receiver) {
            this.receiver = receiver;
            return this;
        }
        
        public Builder correlationId(String correlationId) {
            this.correlationId = correlationId;
            return this;
        }
        
        public Builder priority(Priority priority) {
            this.priority = priority;
            return this;
        }
        
        public Builder timestamp(Instant timestamp) {
            this.timestamp = timestamp;
            return this;
        }
        
        public Builder metadata(String key, String value) {
            this.metadata.put(key, value);
            return this;
        }
        
        public Builder metadata(Map<String, String> metadata) {
            this.metadata.putAll(metadata);
            return this;
        }
        
        public Message build() {
            return new Message(this);
        }
    }
}
