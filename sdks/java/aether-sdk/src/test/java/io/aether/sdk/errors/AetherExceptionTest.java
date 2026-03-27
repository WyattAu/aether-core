package io.aether.sdk.errors;

import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

class AetherExceptionTest {

    @Test
    @DisplayName("constructor with code and message")
    void testConstructor() {
        AetherException ex = new AetherException("TEST_CODE", "test message");
        assertEquals("TEST_CODE", ex.getCode());
        assertEquals("test message", ex.getMessage());
    }

    @Test
    @DisplayName("constructor with cause")
    void testConstructorWithCause() {
        Throwable cause = new RuntimeException("root cause");
        AetherException ex = new AetherException("CODE", "msg", cause);
        assertEquals("CODE", ex.getCode());
        assertEquals(cause, ex.getCause());
    }

    @Test
    @DisplayName("is RuntimeException")
    void testIsRuntimeException() {
        AetherException ex = AetherException.internal("test");
        assertTrue(ex instanceof RuntimeException);
    }

    @Test
    @DisplayName("internal factory")
    void testInternal() {
        AetherException ex = AetherException.internal("something failed");
        assertEquals(AetherException.Codes.INTERNAL, ex.getCode());
        assertTrue(ex.getMessage().contains("something failed"));
    }

    @Test
    @DisplayName("internal factory with cause")
    void testInternalWithCause() {
        Throwable cause = new NullPointerException("npe");
        AetherException ex = AetherException.internal("oops", cause);
        assertEquals(AetherException.Codes.INTERNAL, ex.getCode());
        assertEquals(cause, ex.getCause());
    }

    @Test
    @DisplayName("capabilityDenied factory")
    void testCapabilityDenied() {
        AetherException ex = AetherException.capabilityDenied("state:write");
        assertEquals(AetherException.Codes.CAPABILITY_DENIED, ex.getCode());
        assertTrue(ex.getMessage().contains("state:write"));
    }

    @Test
    @DisplayName("actorNotFound factory")
    void testActorNotFound() {
        AetherException ex = AetherException.actorNotFound("actor-123");
        assertEquals(AetherException.Codes.ACTOR_NOT_FOUND, ex.getCode());
        assertTrue(ex.getMessage().contains("actor-123"));
    }

    @Test
    @DisplayName("timeout factory")
    void testTimeout() {
        AetherException ex = AetherException.timeout("db-query");
        assertEquals(AetherException.Codes.TIMEOUT, ex.getCode());
        assertTrue(ex.getMessage().contains("db-query"));
    }

    @Test
    @DisplayName("invalidArgument factory")
    void testInvalidArgument() {
        AetherException ex = AetherException.invalidArgument("bad input");
        assertEquals(AetherException.Codes.INVALID_ARGUMENT, ex.getCode());
        assertTrue(ex.getMessage().contains("bad input"));
    }

    @Test
    @DisplayName("storageRead factory")
    void testStorageRead() {
        Throwable cause = new RuntimeException("io error");
        AetherException ex = AetherException.storageRead("my-key", cause);
        assertEquals(AetherException.Codes.STORAGE_READ, ex.getCode());
        assertTrue(ex.getMessage().contains("my-key"));
        assertEquals(cause, ex.getCause());
    }

    @Test
    @DisplayName("storageWrite factory")
    void testStorageWrite() {
        Throwable cause = new RuntimeException("io error");
        AetherException ex = AetherException.storageWrite("my-key", cause);
        assertEquals(AetherException.Codes.STORAGE_WRITE, ex.getCode());
        assertTrue(ex.getMessage().contains("my-key"));
        assertEquals(cause, ex.getCause());
    }

    @Test
    @DisplayName("meshConnection factory")
    void testMeshConnection() {
        Throwable cause = new RuntimeException("conn refused");
        AetherException ex = AetherException.meshConnection("node-1 down", cause);
        assertEquals(AetherException.Codes.MESH_CONNECTION, ex.getCode());
        assertEquals(cause, ex.getCause());
    }

    @Test
    @DisplayName("codes constants are defined")
    void testCodesConstants() {
        assertNotNull(AetherException.Codes.INTERNAL);
        assertNotNull(AetherException.Codes.CAPABILITY_DENIED);
        assertNotNull(AetherException.Codes.ACTOR_NOT_FOUND);
        assertNotNull(AetherException.Codes.TIMEOUT);
        assertNotNull(AetherException.Codes.INVALID_ARGUMENT);
        assertNotNull(AetherException.Codes.STORAGE_READ);
        assertNotNull(AetherException.Codes.STORAGE_WRITE);
        assertNotNull(AetherException.Codes.MESH_CONNECTION);
        assertNotNull(AetherException.Codes.VALIDATION);
    }
}
