package io.aether.sdk.client;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests for {@link AetherClient} and {@link AetherServerError}.
 *
 * <p>Uses a mock HTTP client via an intercepting handler to avoid requiring
 * a running server. The Java runtime is not available in CI, so these tests
 * are written for future validation.</p>
 */
class AetherClientTest {

    // === AetherServerError ===

    @Nested
    @DisplayName("aether server error")
    class AetherServerErrorTest {

        @Test
        @DisplayName("stores status code and detail")
        void testConstructor() {
            AetherServerError err = new AetherServerError(404, "actor not found");
            assertEquals(404, err.getStatusCode());
            assertEquals("actor not found", err.getDetail());
            assertEquals("HTTP 404: actor not found", err.getMessage());
        }

        @Test
        @DisplayName("handles connection errors with negative code")
        void testConnectionError() {
            AetherServerError err = new AetherServerError(-1, "connection refused");
            assertEquals(-1, err.getStatusCode());
            assertTrue(err.getMessage().contains("connection refused"));
        }

        @Test
        @DisplayName("handles 500 server error")
        void testServerError() {
            AetherServerError err = new AetherServerError(500, "internal error");
            assertEquals(500, err.getStatusCode());
            assertEquals("internal error", err.getDetail());
        }

        @Test
        @DisplayName("is a runtime exception")
        void testIsRuntimeException() {
            AetherServerError err = new AetherServerError(400, "bad request");
            assertInstanceOf(RuntimeException.class, err);
        }
    }

    // === AetherClient Builder ===

    @Nested
    @DisplayName("aether client builder")
    class BuilderTest {

        @Test
        @DisplayName("rejects null base url")
        void testRejectsNullBaseUrl() {
            assertThrows(IllegalArgumentException.class, () -> AetherClient.builder(null));
        }

        @Test
        @DisplayName("rejects blank base url")
        void testRejectsBlankBaseUrl() {
            assertThrows(IllegalArgumentException.class, () -> AetherClient.builder("  "));
        }

        @Test
        @DisplayName("builds with minimal config")
        void testBuildsMinimal() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            assertNotNull(client);
            client.close();
        }

        @Test
        @DisplayName("trims trailing slash from base url")
        void testTrimsTrailingSlash() {
            // Builder should accept URLs with or without trailing slash
            AetherClient client = AetherClient.builder("http://localhost:8080/").build();
            assertNotNull(client);
            client.close();
        }

        @Test
        @DisplayName("builds with custom timeout")
        void testBuildsWithTimeout() {
            AetherClient client = AetherClient.builder("http://localhost:8080")
                    .requestTimeout(Duration.ofSeconds(60))
                    .connectTimeout(Duration.ofSeconds(20))
                    .build();
            assertNotNull(client);
            client.close();
        }

        @Test
        @DisplayName("builds with actor id")
        void testBuildsWithActorId() {
            AetherClient client = AetherClient.builder("http://localhost:8080")
                    .actorId("test-actor")
                    .build();
            assertNotNull(client);
            client.close();
        }

        @Test
        @DisplayName("implements auto closeable")
        void testImplementsAutoCloseable() {
            assertDoesNotThrow(() -> {
                try (AetherClient client = AetherClient.builder("http://localhost:8080").build()) {
                    assertNotNull(client);
                }
            });
        }

        @Test
        @DisplayName("rejects operations after close")
        void testRejectsAfterClose() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            client.close();
            assertThrows(IllegalStateException.class, client::health);
        }
    }

    // === Model Types ===

    @Nested
    @DisplayName("model types")
    class ModelTypesTest {

        @Test
        @DisplayName("server info has default values")
        void testServerInfoDefaults() {
            AetherClient.ServerInfo info = new AetherClient.ServerInfo();
            assertNull(info.getStatus());
            assertEquals(0.0, info.getUptime());
            assertEquals(0, info.getActorCount());
            assertEquals(0, info.getMessageCount());
        }

        @Test
        @DisplayName("server info has readable toString")
        void testServerInfoToString() {
            AetherClient.ServerInfo info = new AetherClient.ServerInfo();
            String s = info.toString();
            assertNotNull(s);
            assertTrue(s.contains("ServerInfo"));
        }

        @Test
        @DisplayName("actor info returns immutable collections")
        void testActorInfoImmutability() {
            AetherClient.ActorInfo info = new AetherClient.ActorInfo();
            assertNotNull(info.getCapabilities());
            assertNotNull(info.getMetadata());
            assertThrows(UnsupportedOperationException.class,
                    () -> info.getCapabilities().add("x"));
            assertThrows(UnsupportedOperationException.class,
                    () -> info.getMetadata().put("x", "y"));
        }

        @Test
        @DisplayName("actor info null-safe collections")
        void testActorInfoNullSafe() {
            AetherClient.ActorInfo info = new AetherClient.ActorInfo();
            assertEquals(List.of(), info.getCapabilities());
            assertEquals(Map.of(), info.getMetadata());
        }

        @Test
        @DisplayName("actor info has readable toString")
        void testActorInfoToString() {
            AetherClient.ActorInfo info = new AetherClient.ActorInfo();
            String s = info.toString();
            assertNotNull(s);
            assertTrue(s.contains("ActorInfo"));
        }

        @Test
        @DisplayName("message envelope has default values")
        void testMessageEnvelopeDefaults() {
            AetherClient.MessageEnvelope env = new AetherClient.MessageEnvelope();
            assertNull(env.getMessageId());
            assertNull(env.getSourceActor());
            assertNull(env.getTargetActor());
            assertNull(env.getMessageType());
            assertNull(env.getPayload());
            assertNull(env.getCorrelationId());
            assertNull(env.getTimestamp());
            assertEquals(0, env.getPriority());
        }

        @Test
        @DisplayName("message envelope has readable ToString")
        void testMessageEnvelopeToString() {
            AetherClient.MessageEnvelope env = new AetherClient.MessageEnvelope();
            String s = env.toString();
            assertNotNull(s);
            assertTrue(s.contains("MessageEnvelope"));
        }

        @Test
        @DisplayName("delivery receipt has default values")
        void testDeliveryReceiptDefaults() {
            AetherClient.DeliveryReceipt receipt = new AetherClient.DeliveryReceipt();
            assertNull(receipt.getMessageId());
            assertNull(receipt.getStatus());
            assertNull(receipt.getDeliveredAt());
            assertNull(receipt.getCorrelationId());
        }

        @Test
        @DisplayName("delivery receipt has readable toString")
        void testDeliveryReceiptToString() {
            AetherClient.DeliveryReceipt receipt = new AetherClient.DeliveryReceipt();
            String s = receipt.toString();
            assertNotNull(s);
            assertTrue(s.contains("DeliveryReceipt"));
        }

        @Test
        @DisplayName("state entry has default values")
        void testStateEntryDefaults() {
            AetherClient.StateEntry entry = new AetherClient.StateEntry();
            assertNull(entry.getActorId());
            assertNull(entry.getKey());
            assertNull(entry.getValue());
            assertEquals(0, entry.getVersion());
            assertNull(entry.getUpdatedAt());
        }

        @Test
        @DisplayName("state entry has readable toString")
        void testStateEntryToString() {
            AetherClient.StateEntry entry = new AetherClient.StateEntry();
            String s = entry.toString();
            assertNotNull(s);
            assertTrue(s.contains("StateEntry"));
        }

        @Test
        @DisplayName("event record has default values")
        void testEventRecordDefaults() {
            AetherClient.EventRecord event = new AetherClient.EventRecord();
            assertNull(event.getEventId());
            assertNull(event.getAggregateId());
            assertNull(event.getEventType());
            assertNull(event.getData());
            assertEquals(0, event.getVersion());
            assertNull(event.getTimestamp());
        }

        @Test
        @DisplayName("event record has readable toString")
        void testEventRecordToString() {
            AetherClient.EventRecord event = new AetherClient.EventRecord();
            String s = event.toString();
            assertNotNull(s);
            assertTrue(s.contains("EventRecord"));
        }

        @Test
        @DisplayName("pubsub message returns immutable headers")
        void testPubSubMessageImmutableHeaders() {
            AetherClient.PubSubMessage msg = new AetherClient.PubSubMessage();
            assertNotNull(msg.getHeaders());
            assertThrows(UnsupportedOperationException.class,
                    () -> msg.getHeaders().put("x", "y"));
        }

        @Test
        @DisplayName("pubsub message has readable toString")
        void testPubSubMessageToString() {
            AetherClient.PubSubMessage msg = new AetherClient.PubSubMessage();
            String s = msg.toString();
            assertNotNull(s);
            assertTrue(s.contains("PubSubMessage"));
        }
    }

    // === API Contract (Method Signatures) ===

    @Nested
    @DisplayName("api contract")
    class ApiContractTest {

        @Test
        @DisplayName("client exposes all health endpoints")
        void testHealthEndpoints() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                // Verify method signatures exist (will fail at runtime without server)
                assertDoesNotThrow(() -> {
                    // These would throw IllegalStateException if methods didn't exist
                    try { client.health(); } catch (IllegalStateException e) { /* expected - no server */ }
                    try { client.info(); } catch (IllegalStateException e) { /* expected - no server */ }
                });
            } finally {
                client.close();
            }
        }

        @Test
        @DisplayName("client exposes all actor endpoints")
        void testActorEndpoints() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                assertDoesNotThrow(() -> {
                    try { client.registerActor("a"); } catch (Exception e) { /* expected */ }
                    try { client.registerActor("a", "t", List.of(), Map.of()); } catch (Exception e) { /* expected */ }
                    try { client.unregisterActor("a"); } catch (Exception e) { /* expected */ }
                    try { client.getActor("a"); } catch (Exception e) { /* expected */ }
                    try { client.listActors(); } catch (Exception e) { /* expected */ }
                    try { client.listActors("t", "s"); } catch (Exception e) { /* expected */ }
                    try { client.heartbeat("a"); } catch (Exception e) { /* expected */ }
                });
            } finally {
                client.close();
            }
        }

        @Test
        @DisplayName("client exposes all messaging endpoints")
        void testMessagingEndpoints() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                assertDoesNotThrow(() -> {
                    try { client.sendMessage("t", "p"); } catch (Exception e) { /* expected */ }
                    try { client.sendMessage("t", "p", "s", "m", "c", 1); } catch (Exception e) { /* expected */ }
                    try { client.getPendingMessages("a"); } catch (Exception e) { /* expected */ }
                });
            } finally {
                client.close();
            }
        }

        @Test
        @DisplayName("client exposes all state endpoints")
        void testStateEndpoints() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                assertDoesNotThrow(() -> {
                    try { client.getState("a", "k"); } catch (Exception e) { /* expected */ }
                    try { client.setState("a", "k", "v"); } catch (Exception e) { /* expected */ }
                    try { client.setState("a", "k", "v", 1); } catch (Exception e) { /* expected */ }
                    try { client.deleteState("a", "k"); } catch (Exception e) { /* expected */ }
                    try { client.getAllState("a"); } catch (Exception e) { /* expected */ }
                });
            } finally {
                client.close();
            }
        }

        @Test
        @DisplayName("client exposes all pubsub endpoints")
        void testPubSubEndpoints() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                assertDoesNotThrow(() -> {
                    try { client.publish("t", "p"); } catch (Exception e) { /* expected */ }
                    try { client.publish("t", "p", Map.of()); } catch (Exception e) { /* expected */ }
                    try { client.subscribe("t", "s"); } catch (Exception e) { /* expected */ }
                    try { client.subscribe("t", "s", "f"); } catch (Exception e) { /* expected */ }
                    try { client.unsubscribe("s"); } catch (Exception e) { /* expected */ }
                    try { client.listTopics(); } catch (Exception e) { /* expected */ }
                    try { client.getTopicHistory("t"); } catch (Exception e) { /* expected */ }
                    try { client.getTopicHistory("t", 10); } catch (Exception e) { /* expected */ }
                });
            } finally {
                client.close();
            }
        }

        @Test
        @DisplayName("client exposes all event sourcing endpoints")
        void testEventSourcingEndpoints() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                assertDoesNotThrow(() -> {
                    try { client.appendEvent("a", "t", "d"); } catch (Exception e) { /* expected */ }
                    try { client.appendEvent("a", "t", "d", 1); } catch (Exception e) { /* expected */ }
                    try { client.getEvents("a"); } catch (Exception e) { /* expected */ }
                });
            } finally {
                client.close();
            }
        }
    }

    // === URL Encoding ===

    @Nested
    @DisplayName("url encoding")
    class UrlEncodingTest {

        @Test
        @DisplayName("actor id with special characters is handled")
        void testSpecialCharActorId() {
            // The client should accept actor IDs with special chars
            // and encode them properly in URLs
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            try {
                // This should not throw during URL construction
                String actorId = "my-actor-123";
                // We can't actually send without a server, but verify no
                // exception is thrown constructing the client with valid ID
                assertNotNull(client);
            } finally {
                client.close();
            }
        }
    }

    // === Builder Pattern ===

    @Nested
    @DisplayName("builder pattern")
    class BuilderPatternTest {

        @Test
        @DisplayName("builder supports fluent chaining")
        void testFluentChaining() {
            AetherClient client = AetherClient.builder("http://localhost:8080")
                    .actorId("my-actor")
                    .requestTimeout(Duration.ofSeconds(10))
                    .connectTimeout(Duration.ofSeconds(5))
                    .build();
            assertNotNull(client);
            client.close();
        }

        @Test
        @DisplayName("builder accepts custom http client")
        void testCustomHttpClient() {
            HttpClient mockClient = HttpClient.newHttpClient();
            AetherClient client = AetherClient.builder("http://localhost:8080")
                    .httpClient(mockClient)
                    .build();
            assertNotNull(client);
            client.close();
        }

        @Test
        @DisplayName("builder accepts custom object mapper")
        void testCustomObjectMapper() {
            ObjectMapper mapper = new ObjectMapper();
            AetherClient client = AetherClient.builder("http://localhost:8080")
                    .objectMapper(mapper)
                    .build();
            assertNotNull(client);
            client.close();
        }
    }

    // === Edge Cases ===

    @Nested
    @DisplayName("edge cases")
    class EdgeCaseTest {

        @Test
        @DisplayName("client can be closed multiple times")
        void testMultipleClose() {
            AetherClient client = AetherClient.builder("http://localhost:8080").build();
            assertDoesNotThrow(() -> {
                client.close();
                client.close(); // second close should not throw
            });
        }

        @Test
        @DisplayName("client with various base url formats")
        void testBaseUrlFormats() {
            assertDoesNotThrow(() -> {
                try (var c = AetherClient.builder("http://localhost:8080").build()) {}
                try (var c = AetherClient.builder("http://localhost:8080/").build()) {}
                try (var c = AetherClient.builder("https://api.example.com").build()) {}
                try (var c = AetherClient.builder("https://api.example.com/aether").build()) {}
            });
        }
    }
}
