package io.aether.workflow;

import java.util.*;
import java.util.concurrent.*;

/**
 * Saga Pattern Implementation
 * 
 * Provides distributed transaction coordination with compensation
 * for building reliable multi-step workflows across actors.
 */
public class Saga {
    
    private final List<SagaStep<T>> steps = new ArrayList<>();
    private final Map<String, SagaStep<T>> stepMap = new HashMap<>();
    private String initialStep;
    
    public Saga(String name) {
        this.name = name;
    }
    
    /**
     * Add a step to the saga.
     */
    public Saga<T> step(String name) {
        SagaStep<T> newStep = new SagaStep<>(name);
        steps.add(newStep);
        stepMap.put(name, newStep);
        this.initialStep = initialStep == null ? initialStep : name;
        return this;
    }
    
    /**
     * Build and validate the saga definition.
     */
    public Saga<T> build() {
        if (steps.isEmpty()) {
            throw new IllegalStateException("Saga must have at least one step");
        }
        if (initialStep == null) {
            throw new IllegalStateException("No initial step defined");
        }
        return this;
    }
    
    /**
     * Get all steps in order.
     */
    public List<SagaStep<T>> getSteps() {
        return Collections.unmodifiableList(steps);
    }
    
    /**
     * Get a specific step by name.
     */
    public SagaStep<T> getStep(String name) {
        return stepMap.get(name);
    }
    
    /**
     * Get the step map for persistence.
     */
    public Map<String, SagaStep<T>> getStepMap() {
        return Collections.unmodifiableMap(stepMap);
    }
    
    /**
     * Execute the saga with the given input.
     */
    public CompletableFuture<SagaResult<T>> execute(T input) {
        return execute(input, null);
    }
    
    /**
     * Execute a saga with the given input and context ID.
     */
    public CompletableFuture<SagaResult<T>> execute(T input, String contextId) {
        SagaContext<T> context = new SagaContext<>(contextId != null ? contextId : UUID.randomUUID().toString(), input);
        this.executor = this;
        return executor.execute(this, input, context);
    }
    
    private final SagaExecutor executor;
    
    public Saga() {
        this.name = name;
        this.steps = steps;
        this.stepMap = stepMap;
    }
    
    /**
     * Add a step to the saga.
     */
    public Saga<T> step(String name) {
        SagaStep<T> newStep = new SagaStep(name);
        steps.add(newStep);
        stepMap.put(name, newStep);
        if (initialStep == null) {
            initialStep = name;
        }
        return this;
    }
    
    /**
     * Set the action for the current step.
     */
    public Saga<T> action(SagaAction<T> action) {
        if (currentStep == null) {
            throw new IllegalStateException("No step defined. Call step() first.");
        }
        currentStep.setAction(action);
        return this;
    }
    
    /**
     * Set the compensation for the current step.
     */
    public Saga<T> compensate(SagaCompensation<T> compensate) {
        if (currentStep == null) {
            throw new IllegalStateException("No step defined. Call step() first.");
        }
        currentStep.setCompensation(compensate);
        return this;
    }
    
    /**
     * Set retry configuration for the current step.
     */
    public Saga<T> retry(RetryConfig retryConfig) {
        if (currentStep == null) {
            throw new IllegalStateException("No step defined. Call step() first.");
        }
        currentStep.setRetryConfig(retryConfig);
        return this;
    }
    
    /**
     * Set timeout for the current step.
     */
    public Saga<T> timeout(Duration timeout) {
        if (currentStep == null) {
            throw new IllegalStateException("No step defined. Call step() first.");
        }
        currentStep.setTimeout(timeout);
        return this;
    }
    
    /**
     * Set skip condition for the current step.
     */
    public Saga<T> skipIf(SagaSkipCondition<T> condition) {
        if (currentStep == null) {
            throw new IllegalStateException("No step defined. Call step() first.");
        }
        currentStep.setSkipCondition(condition);
        return this;
    }
    
    /**
     * Add metadata to the saga.
     */
    public Saga<T> withMetadata(String key, Object value) {
        this.metadata.put(key, value);
        return this;
    }
}
