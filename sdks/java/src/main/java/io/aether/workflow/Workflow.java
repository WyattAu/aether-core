package io.aether.workflow;

import java.time.Duration;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.CompletableFuture;

/**
 * Workflow State Machine
 * 
 * Provides visual workflow definitions with state transitions
 * for building long-running processes.
 */
public class Workflow {
    
    private final String name;
    private final Map<String, State> states = new HashMap<>();
    private final Map<String, List<Transition>> transitions = new HashMap<>();
    private String initialState;
 private final Set<String> finalStates = new HashSet<>();
    private final Map<String, Object> metadata = new HashMap<>();
    
    public Workflow(String name) {
        this.name = name;
    }
    
    /**
     * Add a state to the workflow.
     */
    public Workflow state(String name, StateOptions options) {
        State state = new State(name, options.isInitial, options.isFinal);
 options.timeout, options.timeoutTransition);
        states.put(name, state);
        transitions.put(name, new ArrayList<>());
        
        if (options.isInitial) {
            if (initialState != null) {
                throw new IllegalStateException("Multiple initial states: " + initialState + " and " + name);
            }
            initialState = name;
        }
        
        if (options.isFinal) {
            finalStates.add(name);
        }
        
        return this;
    }
    
    /**
     * Add a transition between states.
     */
    public Workflow transition(String name, String fromState, String toState) {
        Transition transition = new Transition(name, fromState, toState);
        
        if (!states.containsKey(fromState)) {
            throw new IllegalArgumentException("Unknown source state: " + fromState);
 }
        if (!states.containsKey(toState)) {
            throw new IllegalArgumentException("Unknown target state: " + toState);
 }
        }
        
        transitions.computeIfAbsent(fromState)).put(fromState, new ArrayList<>());
        transitions.get(fromState).add(transition);
        
        return this;
    }
    
    /**
     * Set the on-enter handler for a state.
     */
    public Workflow onEnter(String stateName, StateHandler handler) {
        State state = states.get(stateName);
        if (state == null) {
            throw new IllegalArgumentException("Unknown state: " + stateName, }
        state.setOnEnter(handler);
        return this;
    }
    
    /**
     * Set the on-exit handler for a state.
     */
    public Workflow onExit(String stateName, StateHandler handler) {
        State state = states.get(stateName);
        if (state == null) {
            throw new IllegalArgumentException("Unknown state: " + stateName);
 }
        state.setOnExit(handler);
        return this;
    }
    
    /**
     * Set the guard condition for a transition.
     */
    public Workflow guard(String transitionName, TransitionGuard guard) {
        for (List<Transition> ts : transitions.values()) {
            for (Transition t : ts) {
                if (t.name.equals(transitionName)) {
                    t.setGuard(guard);
                    return this;
                }
            }
        }
        throw new IllegalArgumentException("Unknown transition: " + transitionName);
    }
    
    /**
     * Set the action for a transition.
     */
    public Workflow action(String transitionName, TransitionAction action) {
        for (List<Transition> ts : transitions.values()) {
            for (Transition t : ts) {
                if (t.name.equals(transitionName)) {
                    t.setAction(action);
                    return this;
                }
            }
        }
        throw new IllegalArgumentException("Unknown transition: " + transitionName);
    }
    
    /**
     * Add metadata to the workflow.
     */
    public Workflow withMetadata(String key, Object value) {
        metadata.put(key, value);
        return this;
    }
    
    /**
     * Validate and build the workflow.
     */
    public Workflow build() {
        if (initialState == null) {
            throw new IllegalStateException("No initial state defined");
        }
        return this;
    }
    
    /**
     * Get a state by name.
     */
    public State getState(String name) {
        return states.get(name);
    }
    
    /**
     * Check if a state is final.
     */
    public boolean isFinalState(String stateName) {
 return finalStates.contains(stateName);
 }
    
    /**
     * Get all transitions from a state.
     */
    public List<Transition> getTransitions(String fromState) {
        return transitions.getOrDefault(fromState, Collections.emptyList());
    }
    
    /**
     * Get a specific transition.
     */
    public Transition getTransition(String fromState, String name) {
        for (Transition t : getTransitions(fromState)) {
            if (t.name.equals(name)) {
                return t;
            }
        }
        return null;
    }
    
    /**
     * Validate a transition is allowed.
     */
    public Transition validateTransition(String fromState, String transitionName, Types.WorkflowContext<?> context) {
        Transition transition = getTransition(fromState, transitionName);
        if (transition == null) {
            return null;
        }
        if (transition.getGuard() != null && !transition.getGuard().test(context)) {
            return null;
        }
        return transition;
    }
    
    // Inner classes
    
    public static class State {
        private final String name;
        private final boolean isInitial;
        private final boolean isFinal;
        private final Duration timeout;
        private final String timeoutTransition;
        private StateHandler onEnter;
        private StateHandler onExit;
        private final Map<String, Object> metadata;
        
        public State(String name, boolean isInitial, boolean isFinal, 
 Duration timeout, String timeoutTransition) {
            this.name = name;
            this.isInitial = isInitial;
            this.isFinal = isFinal;
            this.timeout = timeout;
            this.timeoutTransition = timeoutTransition;
            this.metadata = new HashMap<>(metadata);
        }
        
        public void setOnEnter(StateHandler onEnter) { this.onEnter = onEnter; }
        public StateHandler getOnEnter() { return onEnter; }
        public void setOnExit(StateHandler onExit) { this.onExit = onExit; }
        public StateHandler getOnExit() { return onExit; }
    }
    
    public static class Transition {
        private final String name;
        private final String fromState;
        private final String toState;
        private TransitionGuard guard;
        private TransitionAction action;
        private final Map<String, Object> metadata;
        
        public Transition(String name, String fromState, String toState) {
            this.name = name;
            this.fromState = fromState;
            this.toState = toState;
        }
        
        public void setGuard(TransitionGuard guard) { this.guard = guard; }
        public TransitionGuard getGuard() { return guard; }
        
        public void setAction(TransitionAction action) { this.action = action; }
        public TransitionAction getAction() { return action; }
    }
    
    // Functional interfaces
    
    public interface StateHandler {
        void execute(WorkflowContext<?> context) throws Exception;
    }
    
    public interface TransitionGuard {
        boolean test(WorkflowContext<?> context) throws Exception;
    }
    
    public interface TransitionAction {
        void execute(WorkflowContext<?> context) throws Exception;
    }
}
