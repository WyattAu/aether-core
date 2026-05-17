package io.aether.sdk.resilience;

/**
 * Circuit breaker states.
 */
public enum CircuitState {
    CLOSED,
    OPEN,
    HALF_OPEN
}
