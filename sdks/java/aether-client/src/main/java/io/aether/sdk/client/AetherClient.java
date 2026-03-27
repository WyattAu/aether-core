package io.aether.sdk.client;

import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.time.Instant;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.StringJoiner;

/**
 * HTTP client for the Aether reference server.
 *
 * <p>Provides methods for actor management, messaging, state management,
 * pub/sub, and event sourcing through the server's REST API.</p>
 *
 * <p>Usage example:</p>
 * <pre>{@code
 * AetherClient client = AetherClient.builder("http://localhost:8080").build();
 * client.registerActor("my-actor", "worker");
 * client.setState("my-actor", "counter", 42);
 * StateEntry entry = client.getState("my-actor", "counter");
 * client.close();
 * }</pre>
 */
public final class AetherClient implements AutoCloseable {

    private static final ObjectMapper DEFAULT_MAPPER = createDefaultMapper();

    private final HttpClient httpClient;
    private final ObjectMapper mapper;
    private final String baseUrl;
    private final String defaultActorId;
    private final Duration requestTimeout;
    private volatile boolean closed;

    private AetherClient(Builder builder) {
        this.baseUrl = builder.baseUrl.endsWith("/") ? builder.baseUrl.substring(0, builder.baseUrl.length() - 1) : builder.baseUrl;
        this.defaultActorId = builder.actorId;
        this.requestTimeout = builder.requestTimeout;
        this.mapper = builder.mapper != null ? builder.mapper : DEFAULT_MAPPER;
        this.httpClient = builder.httpClient != null
                ? builder.httpClient
                : HttpClient.newBuilder()
                        .connectTimeout(builder.connectTimeout)
                        .build();
    }

    private static ObjectMapper createDefaultMapper() {
        ObjectMapper mapper = new ObjectMapper();
        mapper.registerModule(new JavaTimeModule());
        mapper.disable(SerializationFeature.WRITE_DATES_AS_TIMESTAMPS);
        mapper.disable(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES);
        mapper.setSerializationInclusion(JsonInclude.Include.NON_NULL);
        return mapper;
    }

    /**
     * Create a new builder for configuring the client.
     *
     * @param baseUrl the server base URL (e.g. {@code "http://localhost:8080"})
     * @return a new builder instance
     */
    public static Builder builder(String baseUrl) {
        return new Builder(baseUrl);
    }

    @Override
    public void close() {
        closed = true;
    }

    private void ensureOpen() {
        if (closed) {
            throw new IllegalStateException("Client is closed");
        }
    }

    // === HTTP Layer ===

    private <T> T get(String path, Class<T> responseType) {
        return request("GET", path, null, responseType);
    }

    private <T> T get(String path, TypeReference<T> typeRef) {
        return request("GET", path, null, typeRef);
    }

    private <T> T post(String path, Object body, Class<T> responseType) {
        return request("POST", path, body, responseType);
    }

    private <T> T post(String path, Object body, TypeReference<T> typeRef) {
        return request("POST", path, body, typeRef);
    }

    private void delete(String path) {
        request("DELETE", path, null, Void.class);
    }

    private HttpResponse<String> rawRequest(String method, String path, Object body) {
        ensureOpen();
        try {
            HttpRequest.Builder reqBuilder = HttpRequest.newBuilder()
                    .uri(URI.create(baseUrl + path))
                    .timeout(requestTimeout)
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json");

            switch (method) {
                case "GET" -> reqBuilder.GET();
                case "DELETE" -> reqBuilder.DELETE();
                case "POST" -> {
                    byte[] jsonBytes = mapper.writeValueAsBytes(body);
                    reqBuilder.POST(HttpRequest.BodyPublishers.ofByteArray(jsonBytes));
                }
                case "PUT" -> {
                    byte[] jsonBytes = mapper.writeValueAsBytes(body);
                    reqBuilder.PUT(HttpRequest.BodyPublishers.ofByteArray(jsonBytes));
                }
                default -> throw new IllegalArgumentException("Unsupported HTTP method: " + method);
            }

            return httpClient.send(reqBuilder.build(), HttpResponse.BodyHandlers.ofString());
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            throw new AetherServerError(-1, "Request failed: " + e.getMessage());
        }
    }

    private <T> T request(String method, String path, Object body, Class<T> responseType) {
        HttpResponse<String> resp = rawRequest(method, path, body);
        checkError(resp);
        if (responseType == Void.class) {
            return null;
        }
        try {
            return mapper.readValue(resp.body(), responseType);
        } catch (IOException e) {
            throw new AetherServerError(-1, "Failed to parse response: " + e.getMessage());
        }
    }

    private <T> T request(String method, String path, Object body, TypeReference<T> typeRef) {
        HttpResponse<String> resp = rawRequest(method, path, body);
        checkError(resp);
        try {
            return mapper.readValue(resp.body(), typeRef);
        } catch (IOException e) {
            throw new AetherServerError(-1, "Failed to parse response: " + e.getMessage());
        }
    }

    private void checkError(HttpResponse<String> resp) {
        if (resp.statusCode() >= 400) {
            String detail;
            try {
                Map<String, String> body = mapper.readValue(resp.body(), new TypeReference<>() {});
                detail = body.getOrDefault("detail", resp.body());
            } catch (IOException e) {
                detail = resp.body();
            }
            throw new AetherServerError(resp.statusCode(), detail);
        }
    }

    // === Health ===

    /**
     * Check server health.
     *
     * @return server health info
     */
    public ServerInfo health() {
        return get("/health", ServerInfo.class);
    }

    /**
     * Get server info including version and counts.
     *
     * @return server info map
     */
    public Map<String, Object> info() {
        return get("/api/v1/info", new TypeReference<>() {});
    }

    // === Actors ===

    /**
     * Register an actor with the server.
     *
     * @param actorId       unique actor identifier
     * @param actorType     type of actor (default: {@code "default"})
     * @param capabilities  list of capabilities
     * @param metadata      actor metadata
     * @return registered actor info
     */
    public ActorInfo registerActor(String actorId, String actorType,
                                    List<String> capabilities, Map<String, Object> metadata) {
        Map<String, Object> body = Map.of(
                "actor_id", actorId,
                "actor_type", actorType != null ? actorType : "default",
                "capabilities", capabilities != null ? capabilities : List.of(),
                "metadata", metadata != null ? metadata : Map.of()
        );
        return post("/api/v1/actors", body, ActorInfo.class);
    }

    /**
     * Register an actor with default type.
     *
     * @param actorId unique actor identifier
     * @return registered actor info
     */
    public ActorInfo registerActor(String actorId) {
        return registerActor(actorId, "default", List.of(), Map.of());
    }

    /**
     * Unregister an actor from the server.
     *
     * @param actorId the actor to remove
     */
    public void unregisterActor(String actorId) {
        delete("/api/v1/actors/" + urlEncode(actorId));
    }

    /**
     * Get info for a specific actor.
     *
     * @param actorId the actor identifier
     * @return actor info
     */
    public ActorInfo getActor(String actorId) {
        return get("/api/v1/actors/" + urlEncode(actorId), ActorInfo.class);
    }

    /**
     * List actors with optional filters.
     *
     * @param actorType filter by actor type (nullable)
     * @param status    filter by status (nullable)
     * @return list of matching actors
     */
    public List<ActorInfo> listActors(String actorType, String status) {
        StringBuilder path = new StringBuilder("/api/v1/actors");
        StringJoiner params = new StringJoiner("&");
        if (actorType != null && !actorType.isEmpty()) {
            params.add("type=" + urlEncode(actorType));
        }
        if (status != null && !status.isEmpty()) {
            params.add("status=" + urlEncode(status));
        }
        if (params.length() > 0) {
            path.append("?").append(params);
        }
        return get(path.toString(), new TypeReference<List<ActorInfo>>() {});
    }

    /**
     * List all actors.
     *
     * @return list of all registered actors
     */
    public List<ActorInfo> listActors() {
        return listActors(null, null);
    }

    /**
     * Send a heartbeat for an actor.
     *
     * @param actorId the actor to heartbeat
     */
    public void heartbeat(String actorId) {
        post("/api/v1/actors/" + urlEncode(actorId) + "/heartbeat", null, Void.class);
    }

    // === Messaging ===

    /**
     * Send a message to an actor.
     *
     * @param target          target actor ID
     * @param payload         message payload
     * @param source          source actor ID (uses default if null)
     * @param messageType     type of message (default: {@code "default"})
     * @param correlationId   correlation ID for request-response patterns
     * @param priority        message priority (default: 0)
     * @return delivery receipt
     */
    public DeliveryReceipt sendMessage(String target, Object payload, String source,
                                        String messageType, String correlationId, int priority) {
        Map<String, Object> body = new java.util.LinkedHashMap<>();
        body.put("source_actor", source != null ? source : (defaultActorId != null ? defaultActorId : "unknown"));
        body.put("target_actor", target);
        body.put("message_type", messageType != null ? messageType : "default");
        body.put("payload", payload);
        body.put("priority", priority);
        if (correlationId != null) {
            body.put("correlation_id", correlationId);
        }
        return post("/api/v1/actors/" + urlEncode(target) + "/messages", body, DeliveryReceipt.class);
    }

    /**
     * Send a message with default source and type.
     *
     * @param target  target actor ID
     * @param payload message payload
     * @return delivery receipt
     */
    public DeliveryReceipt sendMessage(String target, Object payload) {
        return sendMessage(target, payload, null, null, null, 0);
    }

    /**
     * Get pending messages for an actor.
     *
     * @param actorId the actor to poll
     * @return list of pending messages
     */
    public List<MessageEnvelope> getPendingMessages(String actorId) {
        return get("/api/v1/actors/" + urlEncode(actorId) + "/messages",
                new TypeReference<List<MessageEnvelope>>() {});
    }

    // === State ===

    /**
     * Get a state value for an actor.
     *
     * @param actorId the actor
     * @param key     state key
     * @return state entry, or empty if not found
     */
    public Optional<StateEntry> getState(String actorId, String key) {
        try {
            return Optional.of(get("/api/v1/state/" + urlEncode(actorId) + "/" + urlEncode(key), StateEntry.class));
        } catch (AetherServerError e) {
            if (e.getStatusCode() == 404) {
                return Optional.empty();
            }
            throw e;
        }
    }

    /**
     * Set a state value for an actor.
     *
     * @param actorId actor identifier
     * @param key     state key
     * @param value   state value
     * @param version expected version for optimistic concurrency (nullable)
     * @return the updated state entry
     */
    public StateEntry setState(String actorId, String key, Object value, Integer version) {
        Map<String, Object> body = new java.util.LinkedHashMap<>();
        body.put("value", value);
        if (version != null) {
            body.put("version", version);
        }
        return request("PUT", "/api/v1/state/" + urlEncode(actorId) + "/" + urlEncode(key), body, StateEntry.class);
    }

    /**
     * Set a state value for an actor.
     *
     * @param actorId actor identifier
     * @param key     state key
     * @param value   state value
     * @return the updated state entry
     */
    public StateEntry setState(String actorId, String key, Object value) {
        return setState(actorId, key, value, null);
    }

    /**
     * Delete a state value for an actor.
     *
     * @param actorId actor identifier
     * @param key     state key
     * @return {@code true} if deleted, {@code false} if not found
     */
    public boolean deleteState(String actorId, String key) {
        try {
            delete("/api/v1/state/" + urlEncode(actorId) + "/" + urlEncode(key));
            return true;
        } catch (AetherServerError e) {
            if (e.getStatusCode() == 404) {
                return false;
            }
            throw e;
        }
    }

    /**
     * Get all state for an actor.
     *
     * @param actorId the actor
     * @return all state entries as a map
     */
    public Map<String, Object> getAllState(String actorId) {
        AllStateResponse resp = get("/api/v1/state/" + urlEncode(actorId), AllStateResponse.class);
        return resp.state != null ? resp.state : Map.of();
    }

    // === Pub/Sub ===

    /**
     * Publish a message to a topic.
     *
     * @param topic   the topic to publish to
     * @param payload message payload
     * @param headers optional headers
     * @return number of subscribers that received the message
     */
    public int publish(String topic, Object payload, Map<String, String> headers) {
        Map<String, Object> body = Map.of(
                "topic", topic,
                "payload", payload,
                "headers", headers != null ? headers : Map.of()
        );
        PublishResult result = post("/api/v1/events/publish", body, PublishResult.class);
        return result.subscriberCount;
    }

    /**
     * Publish a message to a topic without headers.
     *
     * @param topic   the topic
     * @param payload message payload
     * @return number of subscribers
     */
    public int publish(String topic, Object payload) {
        return publish(topic, payload, null);
    }

    /**
     * Subscribe to a topic.
     *
     * @param topic        the topic
     * @param subscriberId unique subscriber identifier
     * @param filter       optional filter expression
     * @return subscription ID
     */
    public String subscribe(String topic, String subscriberId, String filter) {
        Map<String, Object> body = new java.util.LinkedHashMap<>();
        body.put("topic", topic);
        body.put("subscriber_id", subscriberId);
        if (filter != null) {
            body.put("filter", filter);
        }
        SubscriptionInfo result = post("/api/v1/events/subscribe", body, SubscriptionInfo.class);
        return result.subscriptionId;
    }

    /**
     * Subscribe to a topic without a filter.
     *
     * @param topic        the topic
     * @param subscriberId unique subscriber identifier
     * @return subscription ID
     */
    public String subscribe(String topic, String subscriberId) {
        return subscribe(topic, subscriberId, null);
    }

    /**
     * Unsubscribe from a topic.
     *
     * @param subscriptionId the subscription to cancel
     * @return {@code true} if unsubscribed, {@code false} if not found
     */
    public boolean unsubscribe(String subscriptionId) {
        try {
            delete("/api/v1/events/subscribe/" + urlEncode(subscriptionId));
            return true;
        } catch (AetherServerError e) {
            if (e.getStatusCode() == 404) {
                return false;
            }
            throw e;
        }
    }

    /**
     * List all active topics.
     *
     * @return list of topic names
     */
    public List<String> listTopics() {
        return get("/api/v1/events/topics", new TypeReference<List<String>>() {});
    }

    /**
     * Get recent messages for a topic.
     *
     * @param topic topic name
     * @param limit maximum messages to return
     * @return list of pub/sub messages
     */
    public List<PubSubMessage> getTopicHistory(String topic, int limit) {
        String path = "/api/v1/events/topics/" + urlEncode(topic) + "/history?limit=" + limit;
        return get(path, new TypeReference<List<PubSubMessage>>() {});
    }

    /**
     * Get recent messages for a topic with default limit.
     *
     * @param topic topic name
     * @return list of pub/sub messages
     */
    public List<PubSubMessage> getTopicHistory(String topic) {
        return getTopicHistory(topic, 50);
    }

    // === Event Sourcing ===

    /**
     * Append an event to an aggregate's event stream.
     *
     * @param aggregateId     the aggregate identifier
     * @param eventType       type of event
     * @param data            event data
     * @param expectedVersion expected version for optimistic concurrency (nullable)
     * @return the appended event record
     */
    public EventRecord appendEvent(String aggregateId, String eventType,
                                    Object data, Integer expectedVersion) {
        Map<String, Object> body = new java.util.LinkedHashMap<>();
        body.put("aggregate_id", aggregateId);
        body.put("event_type", eventType);
        body.put("data", data);
        if (expectedVersion != null) {
            body.put("expected_version", expectedVersion);
        }
        return post("/api/v1/events/append", body, EventRecord.class);
    }

    /**
     * Append an event without version check.
     *
     * @param aggregateId the aggregate identifier
     * @param eventType   type of event
     * @param data        event data
     * @return the appended event record
     */
    public EventRecord appendEvent(String aggregateId, String eventType, Object data) {
        return appendEvent(aggregateId, eventType, data, null);
    }

    /**
     * Get all events for an aggregate.
     *
     * @param aggregateId the aggregate identifier
     * @return list of event records
     */
    public List<EventRecord> getEvents(String aggregateId) {
        return get("/api/v1/events/" + urlEncode(aggregateId),
                new TypeReference<List<EventRecord>>() {});
    }

    // === Utility ===

    private static String urlEncode(String value) {
        return java.net.URLEncoder.encode(value, java.nio.charset.StandardCharsets.UTF_8);
    }

    // === Model Types ===

    /**
     * Server health information.
     */
    public static final class ServerInfo {
        @JsonProperty("status") private String status;
        @JsonProperty("uptime") private double uptime;
        @JsonProperty("actor_count") private int actorCount;
        @JsonProperty("message_count") private int messageCount;

        public ServerInfo() {}

        public String getStatus() { return status; }
        public double getUptime() { return uptime; }
        public int getActorCount() { return actorCount; }
        public int getMessageCount() { return messageCount; }

        @Override
        public String toString() {
            return "ServerInfo{status='" + status + "', uptime=" + uptime
                    + ", actors=" + actorCount + ", messages=" + messageCount + "}";
        }
    }

    /**
     * Information about a registered actor.
     */
    public static final class ActorInfo {
        @JsonProperty("actor_id") private String actorId;
        @JsonProperty("actor_type") private String actorType;
        @JsonProperty("capabilities") private List<String> capabilities;
        @JsonProperty("metadata") private Map<String, Object> metadata;
        @JsonProperty("status") private String status;
        @JsonProperty("created_at") private String createdAt;
        @JsonProperty("last_heartbeat") private String lastHeartbeat;

        public ActorInfo() {}

        public String getActorId() { return actorId; }
        public String getActorType() { return actorType; }
        public List<String> getCapabilities() {
            return capabilities != null ? Collections.unmodifiableList(capabilities) : List.of();
        }
        public Map<String, Object> getMetadata() {
            return metadata != null ? Collections.unmodifiableMap(metadata) : Map.of();
        }
        public String getStatus() { return status; }
        public String getCreatedAt() { return createdAt; }
        public String getLastHeartbeat() { return lastHeartbeat; }

        @Override
        public String toString() {
            return "ActorInfo{id='" + actorId + "', type='" + actorType
                    + "', status='" + status + "'}";
        }
    }

    /**
     * A message envelope containing routing and payload information.
     */
    public static final class MessageEnvelope {
        @JsonProperty("message_id") private String messageId;
        @JsonProperty("source_actor") private String sourceActor;
        @JsonProperty("target_actor") private String targetActor;
        @JsonProperty("message_type") private String messageType;
        @JsonProperty("payload") private Object payload;
        @JsonProperty("correlation_id") private String correlationId;
        @JsonProperty("timestamp") private String timestamp;
        @JsonProperty("priority") private int priority;

        public MessageEnvelope() {}

        public String getMessageId() { return messageId; }
        public String getSourceActor() { return sourceActor; }
        public String getTargetActor() { return targetActor; }
        public String getMessageType() { return messageType; }
        public Object getPayload() { return payload; }
        public String getCorrelationId() { return correlationId; }
        public String getTimestamp() { return timestamp; }
        public int getPriority() { return priority; }

        @Override
        public String toString() {
            return "MessageEnvelope{from='" + sourceActor + "', to='" + targetActor
                    + "', type='" + messageType + "', priority=" + priority + "}";
        }
    }

    /**
     * Receipt confirming message delivery.
     */
    public static final class DeliveryReceipt {
        @JsonProperty("message_id") private String messageId;
        @JsonProperty("status") private String status;
        @JsonProperty("delivered_at") private String deliveredAt;
        @JsonProperty("correlation_id") private String correlationId;

        public DeliveryReceipt() {}

        public String getMessageId() { return messageId; }
        public String getStatus() { return status; }
        public String getDeliveredAt() { return deliveredAt; }
        public String getCorrelationId() { return correlationId; }

        @Override
        public String toString() {
            return "DeliveryReceipt{id='" + messageId + "', status='" + status + "'}";
        }
    }

    /**
     * A state entry for an actor.
     */
    public static final class StateEntry {
        @JsonProperty("actor_id") private String actorId;
        @JsonProperty("key") private String key;
        @JsonProperty("value") private Object value;
        @JsonProperty("version") private int version;
        @JsonProperty("updated_at") private String updatedAt;

        public StateEntry() {}

        public String getActorId() { return actorId; }
        public String getKey() { return key; }
        public Object getValue() { return value; }
        public int getVersion() { return version; }
        public String getUpdatedAt() { return updatedAt; }

        @Override
        public String toString() {
            return "StateEntry{actor='" + actorId + "', key='" + key
                    + "', version=" + version + "}";
        }
    }

    /**
     * An event record in an aggregate's event stream.
     */
    public static final class EventRecord {
        @JsonProperty("event_id") private String eventId;
        @JsonProperty("aggregate_id") private String aggregateId;
        @JsonProperty("event_type") private String eventType;
        @JsonProperty("data") private Object data;
        @JsonProperty("version") private int version;
        @JsonProperty("timestamp") private String timestamp;

        public EventRecord() {}

        public String getEventId() { return eventId; }
        public String getAggregateId() { return aggregateId; }
        public String getEventType() { return eventType; }
        public Object getData() { return data; }
        public int getVersion() { return version; }
        public String getTimestamp() { return timestamp; }

        @Override
        public String toString() {
            return "EventRecord{id='" + eventId + "', type='" + eventType
                    + "', version=" + version + "}";
        }
    }

    /**
     * A pub/sub message received from a topic.
     */
    public static final class PubSubMessage {
        @JsonProperty("topic") private String topic;
        @JsonProperty("payload") private Object payload;
        @JsonProperty("headers") private Map<String, String> headers;
        @JsonProperty("timestamp") private String timestamp;
        @JsonProperty("message_id") private String messageId;

        public PubSubMessage() {}

        public String getTopic() { return topic; }
        public Object getPayload() { return payload; }
        public Map<String, String> getHeaders() {
            return headers != null ? Collections.unmodifiableMap(headers) : Map.of();
        }
        public String getTimestamp() { return timestamp; }
        public String getMessageId() { return messageId; }

        @Override
        public String toString() {
            return "PubSubMessage{topic='" + topic + "', id='" + messageId + "'}";
        }
    }

    // Internal response types

    private static final class AllStateResponse {
        @JsonProperty("actor_id") String actorId;
        @JsonProperty("state") Map<String, Object> state;
    }

    private static final class PublishResult {
        @JsonProperty("topic") String topic;
        @JsonProperty("subscriber_count") int subscriberCount;
    }

    private static final class SubscriptionInfo {
        @JsonProperty("subscription_id") String subscriptionId;
        @JsonProperty("topic") String topic;
    }

    // === Builder ===

    /**
     * Builder for constructing {@link AetherClient} instances.
     */
    public static final class Builder {
        private final String baseUrl;
        private String actorId;
        private Duration requestTimeout = Duration.ofSeconds(30);
        private Duration connectTimeout = Duration.ofSeconds(10);
        private HttpClient httpClient;
        private ObjectMapper mapper;

        private Builder(String baseUrl) {
            if (baseUrl == null || baseUrl.isBlank()) {
                throw new IllegalArgumentException("Base URL must not be null or blank");
            }
            this.baseUrl = baseUrl;
        }

        /**
         * Set the default actor ID for messages.
         *
         * @param actorId the default actor ID
         * @return this builder
         */
        public Builder actorId(String actorId) {
            this.actorId = actorId;
            return this;
        }

        /**
         * Set the request timeout.
         *
         * @param timeout the timeout duration
         * @return this builder
         */
        public Builder requestTimeout(Duration timeout) {
            this.requestTimeout = timeout;
            return this;
        }

        /**
         * Set the connect timeout.
         *
         * @param timeout the connect timeout duration
         * @return this builder
         */
        public Builder connectTimeout(Duration timeout) {
            this.connectTimeout = timeout;
            return this;
        }

        /**
         * Use a pre-configured {@link HttpClient}.
         *
         * @param httpClient the HTTP client to use
         * @return this builder
         */
        public Builder httpClient(HttpClient httpClient) {
            this.httpClient = httpClient;
            return this;
        }

        /**
         * Use a pre-configured {@link ObjectMapper}.
         *
         * @param mapper the object mapper to use
         * @return this builder
         */
        public Builder objectMapper(ObjectMapper mapper) {
            this.mapper = mapper;
            return this;
        }

        /**
         * Build the client instance.
         *
         * @return a new {@link AetherClient}
         */
        public AetherClient build() {
            return new AetherClient(this);
        }
    }
}
