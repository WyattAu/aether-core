package io.aether.workflow;

import java.time.Duration;
import java.time.Instant;
import java.util.*;

/**
 * Core types for the Workflow Engine.
 * 
 * Provides foundational types for the workflow engine including
 * saga patterns, state machines, and human tasks.
 */
public class Types {
    
    // ============================================
    // Enums
    // ============================================
    
    /**
     * Status of a saga execution.
     */
    public enum SagaStatus {
        PENDING,
        RUNNING,
        COMPLETED,
        COMPENSATING,
        COMPENSATED,
        FAILED
    }
    
    /**
     * Status of an individual saga step.
     */
    public enum StepStatus {
        PENDING,
        RUNNING,
        COMPLETED,
        COMPENSATING,
        COMPENSATED,
        FAILED,
        SKIPPED
    }
    
    /**
     * Status of a workflow execution.
     */
    public enum WorkflowStatus {
        CREATED,
        RUNNING,
        SUSPENDED,
        COMPLETED,
        FAILED,
        CANCELLED
    }
    
    /**
     * Status of a state transition.
     */
    public enum TransitionStatus {
        PENDING,
        SUCCESS,
        FAILED,
        ROLLED_BACK
    }
    
    /**
     * Status of a human task.
     */
    public enum HumanTaskStatus {
        PENDING,
        ASSIGNED,
        IN_PROGRESS,
        COMPLETED,
        REJECTED,
        TIMEOUT,
        ESCALATED
    }
    
    /**
     * Retry policy for saga steps.
     */
    public enum RetryPolicy {
        NONE,
        FIXED,
        EXPONENTIAL,
        EXPONENTIAL_JITTER
    }
    
    // ============================================
    // Value Types
    // ============================================
    
    /**
     * Retry configuration for saga steps.
     */
    public static class RetryConfig {
        private final int maxAttempts;
        private final RetryPolicy policy;
        private final Duration initialDelay;
        private final Duration maxDelay;
        private final double multiplier;
        private final double jitter;
        
        public RetryConfig() {
            this(3, RetryPolicy.EXPONENTIAL, Duration.ofSeconds(1), Duration.ofSeconds(60), 2.0, 0.1);
        }
        
        public RetryConfig(int maxAttempts, RetryPolicy policy, Duration initialDelay, 
                          Duration maxDelay, double multiplier, double jitter) {
            this.maxAttempts = maxAttempts;
            this.policy = policy;
            this.initialDelay = initialDelay;
            this.maxDelay = maxDelay;
            this.multiplier = multiplier;
            this.jitter = jitter;
        }
        
        public int getMaxAttempts() { return maxAttempts; }
        public RetryPolicy getPolicy() { return policy; }
        public Duration getInitialDelay() { return initialDelay; }
        public Duration getMaxDelay() { return maxDelay; }
        public double getMultiplier() { return multiplier; }
        public double getJitter() { return jitter; }
    }
    
    // ============================================
    // Context Types
    // ============================================
    
    /**
     * Context passed through saga execution.
     */
    public static class SagaContext<T> {
        private final String sagaId;
        private final T input;
        private final Map<String, Object> state;
        private final List<String> completedSteps;
        private String failedStep;
        private String error;
        private Instant startedAt;
        private Instant completedAt;
        private final Map<String, Object> metadata;
        
        public SagaContext(T input) {
            this(UUID.randomUUID().toString(), input);
        }
        
        public SagaContext(String sagaId, T input) {
            this.sagaId = sagaId;
            this.input = input;
            this.state = new HashMap<>();
            this.completedSteps = new ArrayList<>();
            this.metadata = new HashMap<>();
        }
        
        public String getSagaId() { return sagaId; }
        public T getInput() { return input; }
        public Map<String, Object> getState() { return state; }
        public List<String> getCompletedSteps() { return completedSteps; }
        public String getFailedStep() { return failedStep; }
        public String getError() { return error; }
        public Instant getStartedAt() { return startedAt; }
        public Instant getCompletedAt() { return completedAt; }
        public Map<String, Object> getMetadata() { return metadata; }
        
        public void setState(String key, Object value) { state.put(key, value); }
        public Object getState(String key) { return state.get(key); }
        public Object getState(String key, Object defaultValue) { return state.getOrDefault(key, defaultValue); }
        
        public void markStepCompleted(String stepName) {
            if (!completedSteps.contains(stepName)) {
                completedSteps.add(stepName);
            }
        }
        
        public boolean isStepCompleted(String stepName) {
            return completedSteps.contains(stepName);
        }
        
        public void setFailedStep(String failedStep) { this.failedStep = failedStep; }
        public void setError(String error) { this.error = error; }
        public void setStartedAt(Instant startedAt) { this.startedAt = startedAt; }
        public void setCompletedAt(Instant completedAt) { this.completedAt = completedAt; }
    }
    
    /**
     * Context passed through workflow execution.
     */
    public static class WorkflowContext<T> {
        private final String workflowId;
        private final String workflowType;
        private String currentState;
        private final T input;
        private final Map<String, Object> variables;
        private final List<HistoryEvent> history;
        private Instant startedAt;
        private Instant updatedAt;
        private final Map<String, Object> metadata;
        
        public WorkflowContext(String workflowType, T input) {
            this(UUID.randomUUID().toString(), workflowType, input);
        }
        
        public WorkflowContext(String workflowId, String workflowType, T input) {
            this.workflowId = workflowId;
            this.workflowType = workflowType;
            this.input = input;
            this.variables = new HashMap<>();
            this.history = new ArrayList<>();
            this.metadata = new HashMap<>();
        }
        
        public String getWorkflowId() { return workflowId; }
        public String getWorkflowType() { return workflowType; }
        public String getCurrentState() { return currentState; }
        public T getInput() { return input; }
        public Map<String, Object> getVariables() { return variables; }
        public List<HistoryEvent> getHistory() { return history; }
        public Instant getStartedAt() { return startedAt; }
        public Instant getUpdatedAt() { return updatedAt; }
        public Map<String, Object> getMetadata() { return metadata; }
        
        public void setVariable(String key, Object value) { variables.put(key, value); }
        public Object getVariable(String key) { return variables.get(key); }
        public Object getVariable(String key, Object defaultValue) { return variables.getOrDefault(key, defaultValue); }
        
        public void addHistoryEvent(String type, Map<String, Object> details) {
            history.add(new HistoryEvent(type, Instant.now(), details));
        }
        
        public void setCurrentState(String currentState) { this.currentState = currentState; }
        public void setStartedAt(Instant startedAt) { this.startedAt = startedAt; }
        public void setUpdatedAt(Instant updatedAt) { this.updatedAt = updatedAt; }
    }
    
    /**
     * Context for a human task.
     */
    public static class HumanTaskContext {
        private final String taskId;
        private final String taskType;
        private String workflowId;
        private String stepName;
        private String title;
        private String description;
        private String assignee;
        private List<String> candidateUsers;
        private List<String> candidateGroups;
        private int priority;
        private Instant dueDate;
        private Map<String, Object> formData;
        private Map<String, Object> result;
        private HumanTaskStatus status;
        private final Instant createdAt;
        private Instant updatedAt;
        private Instant completedAt;
        private String completedBy;
        private Map<String, Object> metadata;
        
        public HumanTaskContext(String taskType, String title) {
            this.taskId = UUID.randomUUID().toString();
            this.taskType = taskType;
            this.title = title;
            this.candidateUsers = new ArrayList<>();
            this.candidateGroups = new ArrayList<>();
            this.priority = 5;
            this.formData = new HashMap<>();
            this.status = HumanTaskStatus.PENDING;
            this.createdAt = Instant.now();
            this.metadata = new HashMap<>();
        }
        
        // Getters
        public String getTaskId() { return taskId; }
        public String getTaskType() { return taskType; }
        public String getWorkflowId() { return workflowId; }
        public String getStepName() { return stepName; }
        public String getTitle() { return title; }
        public String getDescription() { return description; }
        public String getAssignee() { return assignee; }
        public List<String> getCandidateUsers() { return candidateUsers; }
        public List<String> getCandidateGroups() { return candidateGroups; }
        public int getPriority() { return priority; }
        public Instant getDueDate() { return dueDate; }
        public Map<String, Object> getFormData() { return formData; }
        public Map<String, Object> getResult() { return result; }
        public HumanTaskStatus getStatus() { return status; }
        public Instant getCreatedAt() { return createdAt; }
        public Instant getUpdatedAt() { return updatedAt; }
        public Instant getCompletedAt() { return completedAt; }
        public String getCompletedBy() { return completedBy; }
        public Map<String, Object> getMetadata() { return metadata; }
        
        // Setters
        public void setWorkflowId(String workflowId) { this.workflowId = workflowId; }
        public void setStepName(String stepName) { this.stepName = stepName; }
        public void setTitle(String title) { this.title = title; }
        public void setDescription(String description) { this.description = description; }
        public void setAssignee(String assignee) { this.assignee = assignee; }
        public void setCandidateUsers(List<String> candidateUsers) { this.candidateUsers = candidateUsers; }
        public void setCandidateGroups(List<String> candidateGroups) { this.candidateGroups = candidateGroups; }
        public void setPriority(int priority) { this.priority = priority; }
        public void setDueDate(Instant dueDate) { this.dueDate = dueDate; }
        public void setFormData(Map<String, Object> formData) { this.formData = formData; }
        public void setResult(Map<String, Object> result) { this.result = result; }
        public void setStatus(HumanTaskStatus status) { this.status = status; }
        public void setUpdatedAt(Instant updatedAt) { this.updatedAt = updatedAt; }
        public void setCompletedAt(Instant completedAt) { this.completedAt = completedAt; }
        public void setCompletedBy(String completedBy) { this.completedBy = completedBy; }
        public void setMetadata(Map<String, Object> metadata) { this.metadata = metadata; }
    }
    
    // ============================================
    // Result Types
    // ============================================
    
    /**
     * Result of a saga execution.
     */
    public static class SagaResult<T> {
        private final String sagaId;
        private final SagaStatus status;
        private final T output;
        private final String error;
        private final List<String> completedSteps;
        private final List<String> compensatedSteps;
        private final Instant startedAt;
        private final Instant completedAt;
        private final Long durationMs;
        
        public SagaResult(String sagaId, SagaStatus status) {
            this.sagaId = sagaId;
            this.status = status;
            this.output = null;
            this.error = null;
            this.completedSteps = new ArrayList<>();
            this.compensatedSteps = new ArrayList<>();
            this.startedAt = null;
            this.completedAt = null;
            this.durationMs = null;
        }
        
        // Getters
        public String getSagaId() { return sagaId; }
        public SagaStatus getStatus() { return status; }
        public T getOutput() { return output; }
        public String getError() { return error; }
        public List<String> getCompletedSteps() { return completedSteps; }
        public List<String> getCompensatedSteps() { return compensatedSteps; }
        public Instant getStartedAt() { return startedAt; }
        public Instant getCompletedAt() { return completedAt; }
        public Long getDurationMs() { return durationMs; }
        
        // Builder pattern for convenience
        public SagaResult<T> withOutput(T output) {
            return new SagaResult<>(sagaId, status, output, error, completedSteps, compensatedSteps, 
                                   startedAt, completedAt, durationMs);
        }
        
        public SagaResult<T> withError(String error) {
            return new SagaResult<>(sagaId, status, output, error, completedSteps, compensatedSteps,
                                   startedAt, completedAt, durationMs);
        }
        
        private SagaResult(String sagaId, SagaStatus status, T output, String error,
                          List<String> completedSteps, List<String> compensatedSteps,
                          Instant startedAt, Instant completedAt, Long durationMs) {
            this.sagaId = sagaId;
            this.status = status;
            this.output = output;
            this.error = error;
            this.completedSteps = completedSteps;
            this.compensatedSteps = compensatedSteps;
            this.startedAt = startedAt;
            this.completedAt = completedAt;
            this.durationMs = durationMs;
        }
    }
    
    /**
     * Result of a workflow execution.
     */
    public static class WorkflowResult<T> {
        private final String workflowId;
        private final WorkflowStatus status;
        private final T output;
        private final String error;
        private final String currentState;
        private final List<HistoryEvent> history;
        private final Instant startedAt;
        private final Instant completedAt;
        private final Long durationMs;
        
        public WorkflowResult(String workflowId, WorkflowStatus status, String currentState) {
            this.workflowId = workflowId;
            this.status = status;
            this.currentState = currentState;
            this.output = null;
            this.error = null;
            this.history = new ArrayList<>();
            this.startedAt = null;
            this.completedAt = null;
            this.durationMs = null;
        }
        
        // Getters
        public String getWorkflowId() { return workflowId; }
        public WorkflowStatus getStatus() { return status; }
        public T getOutput() { return output; }
        public String getError() { return error; }
        public String getCurrentState() { return currentState; }
        public List<HistoryEvent> getHistory() { return history; }
        public Instant getStartedAt() { return startedAt; }
        public Instant getCompletedAt() { return completedAt; }
        public Long getDurationMs() { return durationMs; }
    }
    
    /**
     * Result of a state transition.
     */
    public static class TransitionResult {
        private final boolean success;
        private final String fromState;
        private final String toState;
        private final String error;
        private final Instant timestamp;
        
        public TransitionResult(boolean success, String fromState, String toState) {
            this(success, fromState, toState, null);
        }
        
        public TransitionResult(boolean success, String fromState, String toState, String error) {
            this.success = success;
            this.fromState = fromState;
            this.toState = toState;
            this.error = error;
            this.timestamp = Instant.now();
        }
        
        public boolean isSuccess() { return success; }
        public String getFromState() { return fromState; }
        public String getToState() { return toState; }
        public String getError() { return error; }
        public Instant getTimestamp() { return timestamp; }
    }
    
    /**
     * History event record.
     */
    public static class HistoryEvent {
        private final String type;
        private final Instant timestamp;
        private final Map<String, Object> details;
        
        public HistoryEvent(String type, Instant timestamp, Map<String, Object> details) {
            this.type = type;
            this.timestamp = timestamp;
            this.details = details != null ? details : new HashMap<>();
        }
        
        public String getType() { return type; }
        public Instant getTimestamp() { return timestamp; }
        public Map<String, Object> getDetails() { return details; }
    }
    
    // ============================================
    // Exceptions
    // ============================================
    
    /**
     * Base exception for saga errors.
     */
    public static class SagaException extends RuntimeException {
        private final String stepName;
        private final Throwable cause;
        
        public SagaException(String stepName, Throwable cause) {
            super("Saga step '" + stepName + "' failed: " + (cause != null ? cause.getMessage() : "unknown error"), cause);
            this.stepName = stepName;
            this.cause = cause;
        }
        
        public String getStepName() { return stepName; }
        public Throwable getCause() { return cause; }
    }
    
    /**
     * Exception for compensation failures.
     */
    public static class CompensationException extends RuntimeException {
        private final String stepName;
        private final Throwable cause;
        
        public CompensationException(String stepName, Throwable cause) {
            super("Compensation for '" + stepName + "' failed: " + (cause != null ? cause.getMessage() : "unknown error"), cause);
            this.stepName = stepName;
            this.cause = cause;
        }
        
        public String getStepName() { return stepName; }
        public Throwable getCause() { return cause; }
    }
    
    /**
     * Base exception for workflow errors.
     */
    public static class WorkflowException extends RuntimeException {
        public WorkflowException(String message) {
            super(message);
        }
        
        public WorkflowException(String message, Throwable cause) {
            super(message, cause);
        }
    }
    
    /**
     * Exception for invalid transitions.
     */
    public static class InvalidTransitionException extends WorkflowException {
        private final String fromState;
        private final String toState;
        private final String workflowId;
        
        public InvalidTransitionException(String fromState, String toState, String workflowId) {
            super("Invalid transition from '" + fromState + "' to '" + toState + "' in workflow " + workflowId);
            this.fromState = fromState;
            this.toState = toState;
            this.workflowId = workflowId;
        }
        
        public String getFromState() { return fromState; }
        public String getToState() { return toState; }
        public String getWorkflowId() { return workflowId; }
    }
    
    /**
     * Exception for human task errors.
     */
    public static class HumanTaskException extends RuntimeException {
        public HumanTaskException(String message) {
            super(message);
        }
    }
}
