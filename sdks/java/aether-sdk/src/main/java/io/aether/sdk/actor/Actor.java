package io.aether.sdk.actor;

import io.aether.sdk.capabilities.*;
import io.aether.sdk.errors.AetherException;
import io.aether.sdk.messaging.*;
import io.aether.sdk.state.StateHandle;

import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * Base class for Aether actors.
 * 
 * Example:
 * <pre>
 * public class MyActor extends Actor {
 *     public MyActor(String id) {
 *         super(id);
 *         addMessageHandler("greet", this::handleGreet);
 *     }
 *     
 *     private void handleGreet(Message msg) {
 *         String name = (String) msg.getPayload();
 *         reply(msg, "Hello, " + name + "!");
 *     }
 * }
 * </pre>
 */
public abstract class Actor {
    protected final String id;
    protected final CapabilitySet capabilities;
    protected final StateHandle state;
    private final Map<String, Consumer<Message>> handlers;
    
    protected Actor(String id) {
        this(id, CapabilitySet.all());
    }
    
    protected Actor(String id, CapabilitySet capabilities) {
        this.id = id;
        this.capabilities = capabilities;
        this.state = new StateHandle(id, capabilities);
        this.handlers = new java.util.HashMap<>();
    }
    
    /**
     * Get the actor's unique identifier.
     */
    public String getId() {
        return id;
    }
    
    /**
     * Get the actor's capabilities.
     */
    public CapabilitySet getCapabilities() {
        return capabilities;
    }
    
    /**
     * Get the actor's state handle.
     */
    public StateHandle getState() {
        return state;
    }
    
    /**
     * Register a handler for a message type.
     */
    protected void addMessageHandler(String type, Consumer<Message> handler) {
        handlers.put(type, handler);
    }
    
    /**
     * Register a handler for a message type with response.
     */
    protected <T, R> void addRpcHandler(String type, Function<T, R> handler) {
        handlers.put(type, msg -> {
            @SuppressWarnings("unchecked")
            T payload = (T) msg.getPayload();
            R result = handler.apply(payload);
            reply(msg, result);
        });
    }
    
    /**
     * Handle an incoming message.
     */
    public void handleMessage(Message message) {
        Consumer<Message> handler = handlers.get(message.getType().getValue());
        if (handler != null) {
            handler.accept(message);
        } else {
            onUnhandledMessage(message);
        }
    }
    
    /**
     * Called when no handler is registered for a message type.
     */
    protected void onUnhandledMessage(Message message) {
        // Default: log warning
    }
    
    /**
     * Send a reply to a message.
     */
    protected void reply(Message original, Object response) {
        if (original.getCorrelationId() == null) {
            return; // No correlation ID, can't reply
        }
        
        Message replyMsg = Message.builder()
            .type(MessageType.RPC_RESPONSE)
            .sender(id)
            .receiver(original.getSender())
            .payload(response)
            .correlationId(original.getCorrelationId())
            .build();
        
        send(replyMsg);
    }
    
    /**
     * Send a message to another actor.
     */
    protected void send(Message message) {
        // Override in subclasses or use messaging client
    }
    
    /**
     * Called when the actor is activated.
     */
    protected CompletableFuture<Void> onActivate() {
        return CompletableFuture.completedFuture(null);
    }
    
    /**
     * Called when the actor is deactivated.
     */
    protected CompletableFuture<Void> onDeactivate() {
        return CompletableFuture.completedFuture(null);
    }
    
    /**
     * Check if actor has a capability.
     */
    protected boolean hasCapability(Capability capability) {
        return capabilities.allows(capability);
    }
    
    /**
     * Require a capability, throwing if not present.
     */
    protected void requireCapability(Capability capability) {
        if (!hasCapability(capability)) {
            throw AetherException.capabilityDenied(capability.getValue());
        }
    }
}
