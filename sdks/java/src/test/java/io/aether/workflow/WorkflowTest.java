package io.aether.workflow;

import io.aether.workflow.Types.WorkflowContext;
import io.aether.workflow.Types.WorkflowStatus;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Duration;
import java.util.Map;

class WorkflowTest {

    @Test
    @DisplayName("workflow creation with name")
    void testWorkflowCreation() {
        Workflow wf = new Workflow("test-workflow");
        assertNotNull(wf);
    }

    @Test
    @DisplayName("add state")
    void testAddState() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null));
        assertNotNull(wf.getState("draft"));
    }

    @Test
    @DisplayName("add transition")
    void testAddTransition() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null))
            .state("review", new Workflow.StateOptions(false, false, null, null))
            .transition("submit", "draft", "review");
        assertNotNull(wf.getTransition("draft", "submit"));
    }

    @Test
    @DisplayName("transition throws for unknown state")
    void testTransitionUnknownState() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null));
        assertThrows(IllegalArgumentException.class, () ->
            wf.transition("submit", "draft", "unknown"));
    }

    @Test
    @DisplayName("multiple initial states throws")
    void testMultipleInitialStates() {
        Workflow wf = new Workflow("test")
            .state("a", new Workflow.StateOptions(true, false, null, null));
        assertThrows(IllegalStateException.class, () ->
            wf.state("b", new Workflow.StateOptions(true, false, null, null)));
    }

    @Test
    @DisplayName("build without initial state throws")
    void testBuildNoInitialState() {
        Workflow wf = new Workflow("test")
            .state("a", new Workflow.StateOptions(false, false, null, null));
        assertThrows(IllegalStateException.class, wf::build);
    }

    @Test
    @DisplayName("isFinalState")
    void testIsFinalState() {
        Workflow wf = new Workflow("test")
            .state("done", new Workflow.StateOptions(true, true, null, null));
        assertTrue(wf.isFinalState("done"));
        assertFalse(wf.isFinalState("nonexistent"));
    }

    @Test
    @DisplayName("getTransitions returns empty for unknown state")
    void testGetTransitionsEmpty() {
        Workflow wf = new Workflow("test");
        assertTrue(wf.getTransitions("unknown").isEmpty());
    }

    @Test
    @DisplayName("validateTransition returns null for unknown transition")
    void testValidateTransitionUnknown() {
        Workflow wf = new Workflow("test");
        assertNull(wf.validateTransition("a", "unknown", null));
    }

    @Test
    @DisplayName("onEnter handler")
    void testOnEnter() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null));
        assertDoesNotThrow(() ->
            wf.onEnter("draft", ctx -> {}));
    }

    @Test
    @DisplayName("onEnter unknown state throws")
    void testOnEnterUnknownState() {
        Workflow wf = new Workflow("test");
        assertThrows(IllegalArgumentException.class, () ->
            wf.onEnter("unknown", ctx -> {}));
    }

    @Test
    @DisplayName("onExit handler")
    void testOnExit() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null));
        assertDoesNotThrow(() ->
            wf.onExit("draft", ctx -> {}));
    }

    @Test
    @DisplayName("guard on transition")
    void testGuard() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null))
            .state("review", new Workflow.StateOptions(false, false, null, null))
            .transition("submit", "draft", "review")
            .guard("submit", ctx -> true);
        var t = wf.getTransition("draft", "submit");
        assertNotNull(t);
        assertNotNull(t.getGuard());
    }

    @Test
    @DisplayName("guard unknown transition throws")
    void testGuardUnknown() {
        Workflow wf = new Workflow("test");
        assertThrows(IllegalArgumentException.class, () ->
            wf.guard("nonexistent", ctx -> true));
    }

    @Test
    @DisplayName("action on transition")
    void testAction() {
        Workflow wf = new Workflow("test")
            .state("draft", new Workflow.StateOptions(true, false, null, null))
            .state("review", new Workflow.StateOptions(false, false, null, null))
            .transition("submit", "draft", "review")
            .action("submit", ctx -> {});
        var t = wf.getTransition("draft", "submit");
        assertNotNull(t.getAction());
    }

    @Test
    @DisplayName("action unknown transition throws")
    void testActionUnknown() {
        Workflow wf = new Workflow("test");
        assertThrows(IllegalArgumentException.class, () ->
            wf.action("nonexistent", ctx -> {}));
    }

    @Test
    @DisplayName("withMetadata")
    void testWithMetadata() {
        Workflow wf = new Workflow("test")
            .withMetadata("version", 1);
        assertNotNull(wf);
    }

    @Test
    @DisplayName("WorkflowContext creation")
    void testWorkflowContext() {
        WorkflowContext<String> ctx = new WorkflowContext<>("test-type", "input");
        assertNotNull(ctx.getWorkflowId());
        assertEquals("test-type", ctx.getWorkflowType());
        assertEquals("input", ctx.getInput());
    }

    @Test
    @DisplayName("WorkflowContext variables")
    void testWorkflowContextVariables() {
        WorkflowContext<String> ctx = new WorkflowContext<>("type", "input");
        ctx.setVariable("key", "value");
        assertEquals("value", ctx.getVariable("key"));
        assertEquals("default", ctx.getVariable("missing", "default"));
    }

    @Test
    @DisplayName("WorkflowContext history")
    void testWorkflowContextHistory() {
        WorkflowContext<String> ctx = new WorkflowContext<>("type", "input");
        ctx.addHistoryEvent("start", Map.of("init", true));
        assertEquals(1, ctx.getHistory().size());
        assertEquals("start", ctx.getHistory().get(0).getType());
    }

    @Test
    @DisplayName("Types enums have expected values")
    void testTypesEnums() {
        assertEquals(6, Types.WorkflowStatus.values().length);
        assertEquals(4, Types.TransitionStatus.values().length);
        assertNotNull(Types.WorkflowStatus.RUNNING);
        assertNotNull(Types.WorkflowStatus.COMPLETED);
        assertNotNull(Types.WorkflowStatus.CANCELLED);
    }

    @Test
    @DisplayName("WorkflowResult creation")
    void testWorkflowResult() {
        Types.WorkflowResult<String> result =
            new Types.WorkflowResult<>("wf-1", Types.WorkflowStatus.RUNNING, "draft");
        assertEquals("wf-1", result.getWorkflowId());
        assertEquals(Types.WorkflowStatus.RUNNING, result.getStatus());
        assertEquals("draft", result.getCurrentState());
    }

    @Test
    @DisplayName("TransitionResult creation")
    void testTransitionResult() {
        Types.TransitionResult success = new Types.TransitionResult(true, "a", "b");
        assertTrue(success.isSuccess());
        assertEquals("a", success.getFromState());
        assertEquals("b", success.getToState());

        Types.TransitionResult failure = new Types.TransitionResult(false, "a", "b", "error");
        assertFalse(failure.isSuccess());
        assertEquals("error", failure.getError());
    }

    @Test
    @DisplayName("HistoryEvent creation")
    void testHistoryEvent() {
        Types.HistoryEvent event = new Types.HistoryEvent("type", java.time.Instant.now(), Map.of());
        assertEquals("type", event.getType());
        assertNotNull(event.getTimestamp());
    }

    @Test
    @DisplayName("HistoryEvent with null details defaults to empty map")
    void testHistoryEventNullDetails() {
        Types.HistoryEvent event = new Types.HistoryEvent("type", java.time.Instant.now(), null);
        assertNotNull(event.getDetails());
        assertTrue(event.getDetails().isEmpty());
    }

    @Test
    @DisplayName("SagaContext creation")
    void testSagaContext() {
        Types.SagaContext<String> ctx = new Types.SagaContext<>("input");
        assertNotNull(ctx.getSagaId());
        assertEquals("input", ctx.getInput());
        assertTrue(ctx.getCompletedSteps().isEmpty());
    }

    @Test
    @DisplayName("SagaContext step tracking")
    void testSagaContextStepTracking() {
        Types.SagaContext<String> ctx = new Types.SagaContext<>("input");
        ctx.markStepCompleted("step1");
        assertTrue(ctx.isStepCompleted("step1"));
        assertFalse(ctx.isStepCompleted("step2"));
        ctx.markStepCompleted("step1");
        assertEquals(1, ctx.getCompletedSteps().size());
    }

    @Test
    @DisplayName("SagaContext state management")
    void testSagaContextState() {
        Types.SagaContext<String> ctx = new Types.SagaContext<>("input");
        ctx.setState("key", "value");
        assertEquals("value", ctx.getState("key"));
        assertEquals("default", ctx.getState("missing", "default"));
    }
}
