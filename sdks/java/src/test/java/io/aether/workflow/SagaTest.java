package io.aether.workflow;

import io.aether.workflow.Types.*;
import org.junit.jupiter.api.*;
import static org.junit.jupiter.api.Assertions.*;

import java.time.Duration;
import java.util.List;
import java.util.Map;

class SagaTest {

    @Test
    @DisplayName("SagaStatus enum values")
    void testSagaStatusEnum() {
        assertEquals(6, Types.SagaStatus.values().length);
        assertNotNull(Types.SagaStatus.PENDING);
        assertNotNull(Types.SagaStatus.RUNNING);
        assertNotNull(Types.SagaStatus.COMPLETED);
        assertNotNull(Types.SagaStatus.COMPENSATING);
        assertNotNull(Types.SagaStatus.COMPENSATED);
        assertNotNull(Types.SagaStatus.FAILED);
    }

    @Test
    @DisplayName("StepStatus enum values")
    void testStepStatusEnum() {
        assertEquals(7, Types.StepStatus.values().length);
        assertNotNull(Types.StepStatus.SKIPPED);
    }

    @Test
    @DisplayName("RetryPolicy enum values")
    void testRetryPolicyEnum() {
        assertEquals(4, Types.RetryPolicy.values().length);
        assertNotNull(Types.RetryPolicy.NONE);
        assertNotNull(Types.RetryPolicy.FIXED);
        assertNotNull(Types.RetryPolicy.EXPONENTIAL);
        assertNotNull(Types.RetryPolicy.EXPONENTIAL_JITTER);
    }

    @Test
    @DisplayName("RetryConfig defaults")
    void testRetryConfigDefaults() {
        Types.RetryConfig config = new Types.RetryConfig();
        assertEquals(3, config.getMaxAttempts());
        assertEquals(Types.RetryPolicy.EXPONENTIAL, config.getPolicy());
        assertEquals(Duration.ofSeconds(1), config.getInitialDelay());
        assertEquals(Duration.ofSeconds(60), config.getMaxDelay());
        assertEquals(2.0, config.getMultiplier());
        assertEquals(0.1, config.getJitter());
    }

    @Test
    @DisplayName("RetryConfig custom values")
    void testRetryConfigCustom() {
        Types.RetryConfig config = new Types.RetryConfig(
            5, Types.RetryPolicy.FIXED, Duration.ofMillis(500),
            Duration.ofSeconds(10), 3.0, 0.2);
        assertEquals(5, config.getMaxAttempts());
        assertEquals(Types.RetryPolicy.FIXED, config.getPolicy());
        assertEquals(500, config.getInitialDelay().toMillis());
    }

    @Test
    @DisplayName("SagaResult minimal creation")
    void testSagaResultMinimal() {
        Types.SagaResult<String> result =
            new Types.SagaResult<>("saga-1", Types.SagaStatus.COMPLETED);
        assertEquals("saga-1", result.getSagaId());
        assertEquals(Types.SagaStatus.COMPLETED, result.getStatus());
        assertNull(result.getOutput());
        assertNull(result.getError());
        assertTrue(result.getCompletedSteps().isEmpty());
    }

    @Test
    @DisplayName("SagaException contains step name")
    void testSagaException() {
        Types.SagaException ex = new Types.SagaException("step1", new RuntimeException("fail"));
        assertEquals("step1", ex.getStepName());
        assertNotNull(ex.getCause());
        assertTrue(ex.getMessage().contains("step1"));
    }

    @Test
    @DisplayName("SagaException with null cause")
    void testSagaExceptionNullCause() {
        Types.SagaException ex = new Types.SagaException("step1", null);
        assertEquals("step1", ex.getStepName());
        assertTrue(ex.getMessage().contains("unknown error"));
    }

    @Test
    @DisplayName("CompensationException contains step name")
    void testCompensationException() {
        Types.CompensationException ex =
            new Types.CompensationException("step2", new RuntimeException("comp fail"));
        assertEquals("step2", ex.getStepName());
        assertTrue(ex.getMessage().contains("Compensation"));
    }

    @Test
    @DisplayName("WorkflowException hierarchy")
    void testWorkflowException() {
        Types.WorkflowException ex = new Types.WorkflowException("workflow error");
        assertTrue(ex instanceof RuntimeException);
        assertEquals("workflow error", ex.getMessage());
    }

    @Test
    @DisplayName("WorkflowException with cause")
    void testWorkflowExceptionWithCause() {
        Throwable cause = new RuntimeException("root");
        Types.WorkflowException ex = new Types.WorkflowException("error", cause);
        assertEquals(cause, ex.getCause());
    }

    @Test
    @DisplayName("InvalidTransitionException contains state info")
    void testInvalidTransitionException() {
        Types.InvalidTransitionException ex =
            new Types.InvalidTransitionException("draft", "review", "wf-1");
        assertEquals("draft", ex.getFromState());
        assertEquals("review", ex.getToState());
        assertEquals("wf-1", ex.getWorkflowId());
        assertTrue(ex.getMessage().contains("draft"));
    }

    @Test
    @DisplayName("HumanTaskException")
    void testHumanTaskException() {
        Types.HumanTaskException ex =
            new Types.HumanTaskException("task failed");
        assertEquals("task failed", ex.getMessage());
        assertTrue(ex instanceof RuntimeException);
    }

    @Test
    @DisplayName("HumanTaskContext default constructor")
    void testHumanTaskContextDefault() {
        Types.HumanTaskContext ctx = new Types.HumanTaskContext("approval", "Review");
        assertNotNull(ctx.getTaskId());
        assertEquals("approval", ctx.getTaskType());
        assertEquals("Review", ctx.getTitle());
        assertEquals(Types.HumanTaskStatus.PENDING, ctx.getStatus());
        assertEquals(5, ctx.getPriority());
        assertNotNull(ctx.getCreatedAt());
        assertTrue(ctx.getCandidateUsers().isEmpty());
    }

    @Test
    @DisplayName("HumanTaskContext setters work")
    void testHumanTaskContextSetters() {
        Types.HumanTaskContext ctx = new Types.HumanTaskContext("type", "title");
        ctx.setAssignee("alice");
        ctx.setStatus(Types.HumanTaskStatus.ASSIGNED);
        ctx.setPriority(10);
        ctx.setDescription("desc");
        assertEquals("alice", ctx.getAssignee());
        assertEquals(Types.HumanTaskStatus.ASSIGNED, ctx.getStatus());
        assertEquals(10, ctx.getPriority());
    }

    @Test
    @DisplayName("SagaContext state set and get")
    void testSagaContextStateSetGet() {
        Types.SagaContext<Map<String, Object>> ctx =
            new Types.SagaContext<>(Map.of());
        ctx.setState("step1_result", "done");
        assertEquals("done", ctx.getState("step1_result"));
        assertNull(ctx.getState("nonexistent"));
    }

    @Test
    @DisplayName("SagaContext failure tracking")
    void testSagaContextFailureTracking() {
        Types.SagaContext<String> ctx = new Types.SagaContext<>("input");
        ctx.setFailedStep("step3");
        ctx.setError("connection refused");
        assertEquals("step3", ctx.getFailedStep());
        assertEquals("connection refused", ctx.getError());
    }

    @Test
    @DisplayName("SagaContext timing")
    void testSagaContextTiming() {
        Types.SagaContext<String> ctx = new Types.SagaContext<>("input");
        java.time.Instant now = java.time.Instant.now();
        ctx.setStartedAt(now);
        ctx.setCompletedAt(now.plusSeconds(10));
        assertEquals(now, ctx.getStartedAt());
        assertEquals(now.plusSeconds(10), ctx.getCompletedAt());
    }
}
