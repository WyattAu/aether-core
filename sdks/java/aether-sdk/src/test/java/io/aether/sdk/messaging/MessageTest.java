package io.aether.sdk.messaging;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Instant;
import java.util.Map;
import java.util.Optional;

class MessageTest {

    @Test
    @DisplayName("build message with all fields")
    void testBuildMessage() {
        Message msg = Message.builder()
            .id("msg-1")
            .type(MessageType.DIRECT)
            .sender("actor-a")
            .receiver("actor-b")
            .payload("hello")
            .correlationId("corr-1")
            .priority(Priority.HIGH)
            .metadata("key1", "val1")
            .build();

        assertEquals("msg-1", msg.getId());
        assertEquals(MessageType.DIRECT, msg.getType());
        assertEquals("actor-a", msg.getSender());
        assertEquals("actor-b", msg.getReceiver());
        assertEquals("hello", msg.getPayload());
        assertEquals("corr-1", msg.getCorrelationId());
        assertEquals(Priority.HIGH, msg.getPriority());
        assertEquals("val1", msg.getMetadata("key1").orElse(null));
    }

    @Test
    @DisplayName("default values in builder")
    void testBuilderDefaults() {
        Message msg = Message.builder().build();
        assertNotNull(msg.getId());
        assertEquals(MessageType.DIRECT, msg.getType());
        assertNull(msg.getPayload());
        assertNull(msg.getSender());
        assertNull(msg.getReceiver());
        assertNull(msg.getCorrelationId());
        assertEquals(Priority.NORMAL, msg.getPriority());
        assertNotNull(msg.getTimestamp());
    }

    @Test
    @DisplayName("metadata map is unmodifiable")
    void testMetadataUnmodifiable() {
        Message msg = Message.builder().metadata("k", "v").build();
        assertThrows(UnsupportedOperationException.class, () ->
            msg.getMetadata().put("k2", "v2"));
    }

    @Test
    @DisplayName("getMetadata returns empty optional for missing key")
    void testGetMetadataMissing() {
        Message msg = Message.builder().build();
        assertTrue(msg.getMetadata("nonexistent").isEmpty());
    }

    @Test
    @DisplayName("metadata map with multiple entries")
    void testMetadataMultiple() {
        Message msg = Message.builder()
            .metadata(Map.of("a", "1", "b", "2"))
            .build();
        assertEquals("1", msg.getMetadata("a").orElse(null));
        assertEquals("2", msg.getMetadata("b").orElse(null));
        assertEquals(2, msg.getMetadata().size());
    }

    @Test
    @DisplayName("MessageType fromValue")
    void testMessageTypeFromValue() {
        assertEquals(MessageType.DIRECT, MessageType.fromValue("direct"));
        assertEquals(MessageType.RPC_REQUEST, MessageType.fromValue("rpc_request"));
        assertEquals(MessageType.RPC_RESPONSE, MessageType.fromValue("rpc_response"));
        assertEquals(MessageType.BROADCAST, MessageType.fromValue("broadcast"));
        assertEquals(MessageType.SYSTEM, MessageType.fromValue("system"));
    }

    @Test
    @DisplayName("MessageType fromValue throws for unknown")
    void testMessageTypeFromValueUnknown() {
        assertThrows(IllegalArgumentException.class, () -> MessageType.fromValue("unknown_type"));
    }

    @Test
    @DisplayName("MessageType getValue returns string")
    void testMessageTypeGetValue() {
        assertEquals("direct", MessageType.DIRECT.getValue());
    }

    @Test
    @DisplayName("Priority fromValue")
    void testPriorityFromValue() {
        assertEquals(Priority.LOW, Priority.fromValue(0));
        assertEquals(Priority.NORMAL, Priority.fromValue(1));
        assertEquals(Priority.HIGH, Priority.fromValue(2));
        assertEquals(Priority.CRITICAL, Priority.fromValue(3));
    }

    @Test
    @DisplayName("Priority fromValue throws for unknown")
    void testPriorityFromValueUnknown() {
        assertThrows(IllegalArgumentException.class, () -> Priority.fromValue(99));
    }

    @Test
    @DisplayName("Priority ordering by value")
    void testPriorityOrdering() {
        assertTrue(Priority.LOW.getValue() < Priority.NORMAL.getValue());
        assertTrue(Priority.NORMAL.getValue() < Priority.HIGH.getValue());
        assertTrue(Priority.HIGH.getValue() < Priority.CRITICAL.getValue());
    }

    @Test
    @DisplayName("builder id overrides default uuid")
    void testBuilderCustomId() {
        Message msg = Message.builder().id("custom-id").build();
        assertEquals("custom-id", msg.getId());
    }
}
