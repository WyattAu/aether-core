package io.aether.sdk.client;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.DisabledIfEnvironmentVariable;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Integration tests for the Java SDK against a running Aether server.
 *
 * <p>These tests require a running Aether server at the AETHER_BASE_URL
 * (default: http://localhost:8080). They are skipped automatically if
 * the server is not reachable.</p>
 *
 * <p>Run with:
 * <pre>{@code
 * mvn verify -pl aether-client
 * AETHER_BASE_URL=http://localhost:9090 mvn verify -pl aether-client
 * }</pre></p>
 */
@DisabledIfEnvironmentVariable(named = "AETHER_SKIP_INTEGRATION", matches = "true")
class IntegrationTest {

    private static final String BASE_URL = System.getenv().getOrDefault("AETHER_BASE_URL", "http://localhost:8080");
    private static boolean serverReachable;

    @BeforeAll
    static void checkServer() {
        try {
            HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(3)).build();
            HttpResponse<String> resp = client.send(
                    HttpRequest.newBuilder().uri(URI.create(BASE_URL + "/health")).GET().build(),
                    HttpResponse.BodyHandlers.ofString()
            );
            serverReachable = resp.statusCode() == 200;
        } catch (Exception e) {
            serverReachable = false;
        }
        org.junit.jupiter.api.Assumptions.assumeTrue(serverReachable,
                "Aether server not reachable at " + BASE_URL);
    }

    private static String uniqueId() {
        return "inttest-" + UUID.randomUUID().toString().replace("-", "").substring(0, 12);
    }

    @Nested
    @DisplayName("Health Check")
    class HealthCheck {

        @Test
        @DisplayName("health returns ok status")
        void healthReturnsOk() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                AetherClient.ServerInfo info = client.health();
                assertEquals("ok", info.getStatus());
                assertTrue(info.getUptime() >= 0);
            }
        }

        @Test
        @DisplayName("info returns version or counts")
        void infoReturnsData() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                Map<String, Object> info = client.info();
                assertTrue(info.containsKey("version") || info.containsKey("actor_count"),
                        "expected 'version' or 'actor_count' in info response");
            }
        }
    }

    @Nested
    @DisplayName("Actor Spawn and Message Send")
    class ActorSpawnAndMessage {

        @Test
        @DisplayName("register actor returns correct info")
        void registerActor() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                AetherClient.ActorInfo info = client.registerActor(actorId, "worker", List.of(), Map.of());
                assertEquals(actorId, info.getActorId());
                assertEquals("worker", info.getActorType());
                assertEquals("active", info.getStatus());
            }
        }

        @Test
        @DisplayName("get actor returns correct info")
        void getActor() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                AetherClient.ActorInfo info = client.getActor(actorId);
                assertEquals(actorId, info.getActorId());
            }
        }

        @Test
        @DisplayName("list actors includes registered actor")
        void listActors() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "integration-test", List.of(), Map.of());
                List<AetherClient.ActorInfo> actors = client.listActors();
                boolean found = actors.stream().anyMatch(a -> a.getActorId().equals(actorId));
                assertTrue(found, "registered actor not found in list");
            }
        }

        @Test
        @DisplayName("unregister actor then get returns error")
        void unregisterActor() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                client.unregisterActor(actorId);
                assertThrows(AetherServerError.class, () -> client.getActor(actorId));
            }
        }

        @Test
        @DisplayName("send message returns delivered receipt")
        void sendMessage() {
            try (AetherClient client = AetherClient.builder(BASE_URL)
                    .actorId("integration-test-sender").build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                AetherClient.DeliveryReceipt receipt = client.sendMessage(
                        actorId, Map.of("hello", "world"));
                assertEquals("delivered", receipt.getStatus());
                assertNotNull(receipt.getMessageId());
            }
        }

        @Test
        @DisplayName("heartbeat succeeds for registered actor")
        void heartbeat() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                assertDoesNotThrow(() -> client.heartbeat(actorId));
            }
        }
    }

    @Nested
    @DisplayName("State Read/Write")
    class StateReadWrite {

        @Test
        @DisplayName("set and get state round-trips correctly")
        void setAndGetState() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                client.setState(actorId, "counter", 42);
                Optional<AetherClient.StateEntry> entry = client.getState(actorId, "counter");
                assertTrue(entry.isPresent());
                assertEquals(42, entry.get().getValue());
            }
        }

        @Test
        @DisplayName("get missing state returns empty")
        void getMissingState() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                Optional<AetherClient.StateEntry> entry = client.getState(actorId, "nonexistent");
                assertTrue(entry.isEmpty());
            }
        }

        @Test
        @DisplayName("delete state returns true for existing key")
        void deleteState() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                client.setState(actorId, "temp", "data");
                assertTrue(client.deleteState(actorId, "temp"));
            }
        }

        @Test
        @DisplayName("get all state returns written keys")
        void getAllState() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                client.setState(actorId, "a", 1);
                client.setState(actorId, "b", 2);
                Map<String, Object> state = client.getAllState(actorId);
                assertTrue(state.containsKey("a") || state.containsKey("b"));
            }
        }

        @Test
        @DisplayName("state version increments on update")
        void stateVersionIncrements() {
            try (AetherClient client = AetherClient.builder(BASE_URL).build()) {
                String actorId = uniqueId();
                client.registerActor(actorId, "worker", List.of(), Map.of());
                AetherClient.StateEntry e1 = client.setState(actorId, "counter", 1);
                AetherClient.StateEntry e2 = client.setState(actorId, "counter", 2);
                assertTrue(e2.getVersion() > e1.getVersion(),
                        "expected version to increment: " + e1.getVersion() + " -> " + e2.getVersion());
            }
        }
    }

    @Nested
    @DisplayName("Mesh Peer Discovery")
    class MeshPeerDiscovery {

        @Test
        @DisplayName("cluster info endpoint returns data")
        void clusterInfo() {
            try {
                HttpClient http = HttpClient.newBuilder()
                        .connectTimeout(Duration.ofSeconds(5)).build();
                HttpResponse<String> resp = http.send(
                        HttpRequest.newBuilder().uri(URI.create(BASE_URL + "/cluster/info")).GET().build(),
                        HttpResponse.BodyHandlers.ofString());
                if (resp.statusCode() == 404) {
                    org.junit.jupiter.api.Assumptions.assumeTrue(false,
                            "Cluster endpoints not available on this server");
                }
                assertEquals(200, resp.statusCode());
                assertTrue(resp.body().contains("node_id")
                                || resp.body().contains("cluster_enabled")
                                || resp.body().contains("status"),
                        "expected cluster info fields in response");
            } catch (IOException | InterruptedException e) {
                org.junit.jupiter.api.Assumptions.assumeTrue(false,
                        "Cluster endpoint not available: " + e.getMessage());
            }
        }

        @Test
        @DisplayName("cluster nodes endpoint returns data")
        void clusterNodes() {
            try {
                HttpClient http = HttpClient.newBuilder()
                        .connectTimeout(Duration.ofSeconds(5)).build();
                HttpResponse<String> resp = http.send(
                        HttpRequest.newBuilder().uri(URI.create(BASE_URL + "/cluster/nodes")).GET().build(),
                        HttpResponse.BodyHandlers.ofString());
                if (resp.statusCode() == 404) {
                    org.junit.jupiter.api.Assumptions.assumeTrue(false,
                            "Cluster endpoints not available on this server");
                }
                assertEquals(200, resp.statusCode());
            } catch (IOException | InterruptedException e) {
                org.junit.jupiter.api.Assumptions.assumeTrue(false,
                        "Cluster endpoint not available: " + e.getMessage());
            }
        }
    }
}
