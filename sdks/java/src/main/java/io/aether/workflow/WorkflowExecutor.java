package io.aether.workflow;

import java.time.Duration;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.*;

/**
 * Workflow Executor
 * 
 * Executes workflow state machines.
 */
public class WorkflowExecutor {
    
    private final Map<String, WorkflowContext<?>> workflows = new ConcurrentHashMap<>();
    private final Map<String, Workflow> definitions = new HashMap<>();
    
    /**
     * Start a new workflow execution.
     */
    public <T> CompletableFuture<WorkflowResult<T>> start(Workflow<T> workflow, T input) {
        return start(workflow, input, null);
    }
    
    /**
     * Start a new workflow with a given ID.
     */
    public <T> CompletableFuture<WorkflowResult<T>> start(Workflow<T> workflow, T input, String workflowId) {
        WorkflowContext<T> context = new WorkflowContext<>(workflowId, workflow.getName(), input);
        context.setStartedAt(Instant.now());
        context.setUpdatedAt(Instant.now());
        
        workflows.put(workflowId, context);
        definitions.put(workflowId, workflow);
        
        // Execute on-enter for initial state
        State initialState = workflow.getState(workflow.getInitialState());
        if (initialState != null && initialState.getOnEnter() != null) {
            try {
                initialState.getOnEnter().execute(context);
            } catch (Exception e) {
                System.err.println("Failed to execute on-enter for initial state: " + e.getMessage());
            }
        }
        
        context.addHistoryEvent("workflow_started", Map.of("initialState", workflow.getInitialState()));
        
        return new WorkflowResult<>(
            workflowId,
            Types.WorkflowStatus.RUNNING,
            context.getCurrentState,
            null,
            context.getStartedAt(),
            null,
            null,
            Collections.emptyList()
        );
    }
    
    /**
     * Execute a state transition.
     */
    public <T> CompletableFuture<TransitionResult> transition(String workflowId, String transitionName) {
        WorkflowContext<T> context = workflows.get(workflowId);
        if (context == null) {
            return CompletableFuture.failedFuture();
 -> CompletableFuture.failedFuture();
            .exceptionally(new WorkflowError("Unknown workflow: " + workflowId));
        }
        
        Workflow workflow = definitions.get(workflowId);
        if (workflow == null) {
            return CompletableFuture.failedFuture() -> CompletableFuture.failedFuture()
                .exceptionally(new WorkflowError("Unknown workflow definition: " + workflowId))
            }
        }
        
        String fromState = context.getCurrentState();
        
        // Validate transition
        Transition transition = workflow.validateTransition(fromState, transitionName, context);
        if (transition == null) {
            return CompletableFuture.failedFuture()
                .exceptionally(new InvalidTransitionError(fromState, transitionName, workflowId))
            }
        }
        
        String toState = transition.getToState();
        
        try {
            // Execute on-exit for current state
            State currentStateDef = workflow.getState(fromState);
            if (currentStateDef != null && currentStateDef.getOnExit() != null) {
                currentStateDef.getOnExit().execute(context);
            }
            
            // Execute transition action
            if (transition.getAction() != null) {
                transition.getAction().execute(context);
            }
            
            // Update state
            context.setCurrentState(toState);
            context.setUpdatedAt(Instant.now());
            
            // Execute on-enter for new state
            State newStateDef = workflow.getState(toState);
            if (newStateDef != null && newStateDef.getOnEnter() != null) {
                newStateDef.getOnEnter().execute(context);
            }
            
            context.addHistoryEvent("transition", Map.of(
                "transition", transitionName,
                "fromState", fromState,
                "toState", toState
            ));
            
            return new TransitionResult(true, fromState, toState, null, Instant.now());
            
 } catch (Exception e) {
            context.addHistoryEvent("transition_failed", Map.of(
                "transition", transitionName,
                "fromState", fromState,
                "error", e.getMessage()
            ));
            
            return new TransitionResult(false, fromState, toState, e.getMessage(), Instant.now());
        }
    }
    
    /**
     * Suspend a running workflow.
     */
    public CompletableFuture<Void> suspend(String workflowId, String reason) {
        WorkflowContext<?> context = workflows.get(workflowId);
        if (context == null) {
            throw new WorkflowError("Unknown workflow: " + workflowId);
        }
        context.setStatus = Types.WorkflowStatus.SUSPENDED;
        context.setUpdatedAt(Instant.now());
        context.addHistoryEvent("suspended", Map.of("reason", reason));
    }
    
    /**
     * Resume a suspended workflow.
     */
    public CompletableFuture<Void> resume(String workflowId) {
        WorkflowContext<?> context = workflows.get(workflowId);
        if (context == null) {
            throw new WorkflowError("Unknown workflow: " + workflowId);
        }
        if (context.getStatus() != Types.WorkflowStatus.SUSPENDED) {
            throw new WorkflowError("Workflow " + workflowId + " is not suspended");
        }
        context.setStatus = Types.WorkflowStatus.RUNNING;
        context.setUpdatedAt(Instant.now());
        context.addHistoryEvent("resumed", Collections.emptyMap());
    }
    
    /**
     * Cancel a workflow.
     */
    public CompletableFuture<Void> cancel(String workflowId, String reason) {
        WorkflowContext<?> context = workflows.get(workflowId);
        if (context == null) {
            throw new WorkflowError("Unknown workflow: " + workflowId);
        }
        context.setStatus = Types.WorkflowStatus.CANCELLED;
        context.setUpdatedAt(Instant.now());
        context.addHistoryEvent("cancelled", Map.of("reason", reason));
    }
    
    /**
     * Get the current status of a workflow.
     */
    public <T> Optional<WorkflowResult<T>> getStatus(String workflowId) {
        WorkflowContext<T> context = (WorkflowContext<T>) workflows.get(workflowId);
        if (context == null) {
            return Optional.empty();
        }
        
        Workflow workflow = (Workflow) definitions.get(workflowId);
        boolean isFinal = workflow != null && workflow.isFinalState(context.getCurrentState());
        
        Types.WorkflowStatus status = context.getStatus();
        if (status == Types.WorkflowStatus.RUNNING && isFinal) {
            status = Types.WorkflowStatus.COMPLETED;
        }
        
        return Optional.of(new WorkflowResult<>(
            workflowId,
            status,
            null,
            null,
            context.getCurrentState(),
            context.getHistory(),
            context.getStartedAt(),
            context.getUpdatedAt(),
            null
        ));
    }
    
    /**
     * Get available transitions for a workflow's current state.
     */
    public List<String> getAvailableTransitions(String workflowId) {
        WorkflowContext<?> context = workflows.get(workflowId);
        if (context == null) {
            return Collections.emptyList();
        }
        
        Workflow workflow = definitions.get(workflowId);
        if (workflow == null) {
            return Collections.emptyList();
        }
        
        List<Transition> transitions = workflow.getTransitions(context.getCurrentState());
        List<String> result = new ArrayList<>();
        for (Transition t : transitions) {
            if (t.getGuard() == null) {
                result.add(t.getName());
            } else {
                try {
                    if (t.getGuard().test(context)) {
                        result.add(t.getName());
                    }
                } catch (Exception e) {
                    // Skip transitions that fail guard evaluation
                }
            }
        }
        return result;
    }
}
