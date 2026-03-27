package io.aether.sdk.actor;

import io.aether.sdk.capabilities.*;
import io.aether.sdk.errors.AetherException;
import io.aether.sdk.messaging.*;
import io.aether.sdk.state.StateHandle;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.util.concurrent.CompletableFuture;

class ActorTest {

    static class TestActor extends Actor {
        private boolean activated;
        private boolean deactivated;
        private Message lastUnhandled;

        TestActor(String id) {
            super(id);
        }

        TestActor(String id, CapabilitySet caps) {
            super(id, caps);
        }

        @Override
        protected CompletableFuture<Void> onActivate() {
            activated = true;
            return CompletableFuture.completedFuture(null);
        }

        @Override
        protected CompletableFuture<Void> onDeactivate() {
            deactivated = true;
            return CompletableFuture.completedFuture(null);
        }

        @Override
        protected void onUnhandledMessage(Message message) {
            lastUnhandled = message;
        }

        void registerTestHandler(String type) {
            addMessageHandler(type, msg -> {});
        }

        <T, R> void registerRpcHandler(String type, java.util.function.Function<T, R> handler) {
            addRpcHandler(type, handler);
        }
    }

    private TestActor actor;

    @BeforeEach
    void setUp() {
        actor = new TestActor("test-actor-1");
    }

    @Test
    @DisplayName("actor has correct id")
    void testActorId() {
        assertEquals("test-actor-1", actor.getId());
    }

    @Test
    @DisplayName("actor has all capabilities by default")
    void testDefaultCapabilities() {
        assertNotNull(actor.getCapabilities());
        assertTrue(actor.getCapabilities().allows(Capability.ACTOR_INVOKE));
        assertTrue(actor.getCapabilities().allows(Capability.STATE_READ));
    }

    @Test
    @DisplayName("actor with custom capabilities")
    void testCustomCapabilities() {
        CapabilitySet caps = CapabilitySet.builder()
            .add(Capability.STATE_READ)
            .build();
        TestActor custom = new TestActor("custom", caps);
        assertTrue(custom.hasCapability(Capability.STATE_READ));
        assertFalse(custom.hasCapability(Capability.ACTOR_INVOKE));
    }

    @Test
    @DisplayName("actor has state handle")
    void testStateHandle() {
        assertNotNull(actor.getState());
    }

    @Test
    @DisplayName("hasCapability returns true for allowed capability")
    void testHasCapabilityAllowed() {
        assertTrue(actor.hasCapability(Capability.ACTOR_INVOKE));
    }

    @Test
    @DisplayName("requireCapability throws for missing capability")
    void testRequireCapabilityDenied() {
        CapabilitySet empty = CapabilitySet.empty();
        TestActor restricted = new TestActor("restricted", empty);
        assertThrows(AetherException.class, () -> restricted.requireCapability(Capability.ACTOR_INVOKE));
    }

    @Test
    @DisplayName("requireCapability succeeds for allowed capability")
    void testRequireCapabilityAllowed() {
        assertDoesNotThrow(() -> actor.requireCapability(Capability.ACTOR_INVOKE));
    }

    @Test
    @DisplayName("onActivate returns completed future")
    void testOnActivate() {
        CompletableFuture<Void> result = actor.onActivate();
        assertTrue(result.isDone());
        assertNull(result.join());
    }

    @Test
    @DisplayName("onDeactivate returns completed future")
    void testOnDeactivate() {
        CompletableFuture<Void> result = actor.onDeactivate();
        assertTrue(result.isDone());
        assertNull(result.join());
    }

    @Test
    @DisplayName("handleMessage routes to registered handler")
    void testHandleMessageRouted() {
        final boolean[] called = {false};
        TestActor a = new TestActor("router") {
            {
                addMessageHandler("greet", msg -> called[0] = true);
            }
        };
        Message msg = Message.builder()
            .type(MessageType.DIRECT)
            .payload("hello")
            .build();
        a.handleMessage(msg);
        assertTrue(called[0]);
    }

    @Test
    @DisplayName("handleMessage calls onUnhandled for unknown type")
    void testHandleMessageUnhandled() {
        Message msg = Message.builder()
            .type(MessageType.DIRECT)
            .payload("data")
            .build();
        actor.handleMessage(msg);
        assertNotNull(actor.lastUnhandled);
    }

    @Test
    @DisplayName("reply does nothing without correlation id")
    void testReplyNoCorrelationId() {
        Message msg = Message.builder()
            .type(MessageType.DIRECT)
            .sender("other")
            .build();
        assertDoesNotThrow(() -> actor.reply(msg, "response"));
    }

    @Test
    @DisplayName("send is no-op by default")
    void testSendNoOp() {
        Message msg = Message.builder()
            .type(MessageType.DIRECT)
            .build();
        assertDoesNotThrow(() -> actor.send(msg));
    }
}
