"""
Circuit Breaker Pattern Implementation

Prevents cascading failures by stopping requests to a failing service.

States:
- CLOSED: Normal operation, requests pass through
- OPEN: Failing, requests are rejected immediately
- HALF_OPEN: Testing if service recovered
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Callable, Optional, Dict, Any
from enum import Enum
import asyncio
import time


class CircuitState(Enum):
    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half-open"


@dataclass
class CircuitBreakerConfig:
    """Configuration for circuit breaker."""
    failure_threshold: int = 5
    success_threshold: int = 3
    timeout_ms: int = 30000
    half_open_max_calls: int = 3
    failure_window_ms: int = 60000
    # Callbacks
    on_open: Optional[Callable[[], None]] = None
    on_close: Optional[Callable[[], None]] = None
    on_half_open: Optional[Callable[[], None]] = None


@dataclass
class CircuitBreakerStats:
    """Statistics for circuit breaker."""
    state: CircuitState = CircuitState.CLOSED
    failures: int = 0
    successes: int = 0
    rejected_calls: int = 0
    total_calls: int = 0
    last_failure: Optional[float] = None
    last_success: Optional[float] = None
    last_state_change: Optional[float] = None


class CircuitBreakerError(Exception):
    """Raised when circuit is open."""
    pass


@dataclass
class FailureRecord:
    """Record of failure for window-based counting."""
    timestamp: float
    error: Exception


class CircuitBreaker:
    """Circuit breaker for protecting against cascading failures."""
    
    def __init__(self, config: Optional[CircuitBreakerConfig] = None):
        self._config = config or CircuitBreakerConfig()
        self._state = CircuitState.CLOSED
        self._failures = 0
        self._successes = 0
        self._rejected_calls = 0
        self._total_calls = 0
        self._half_open_calls = 0
        self._last_failure: Optional[float] = None
        self._last_success: Optional[float] = None
        self._last_state_change: Optional[float] = None
        self._failure_history: list[FailureRecord] = []
    
    @property
    def state(self) -> CircuitState:
        return self._state
    
    @property
    def is_closed(self) -> bool:
        return self._state == CircuitState.CLOSED
    
    @property
    def is_open(self) -> bool:
        return self._state == CircuitState.OPEN
    
    @property
    def is_half_open(self) -> bool:
        return self._state == CircuitState.HALF_OPEN
    
    def get_stats(self) -> CircuitBreakerStats:
        """Get current statistics."""
        return CircuitBreakerStats(
            state=self._state,
            failures=self._failures,
            successes=self._successes,
            rejected_calls=self._rejected_calls,
            total_calls=self._total_calls,
            last_failure=self._last_failure,
            last_success=self._last_success,
            last_state_change=self._last_state_change,
        )
    
    async def execute(self, func: Callable[[], Any]) -> Any:
        """Execute a function through the circuit breaker.
        
        Args:
            func: Async function to execute
            
        Returns:
            Result of            function
            
        Raises:
            CircuitBreakerError: If circuit is open
            Exception: If function raises an exception
        """
        self._total_calls += 1
        
        # Check if we should transition from open to half-open
        if self._state == CircuitState.OPEN:
            if self._should_attempt_reset():
                self._transition_to(CircuitState.HALF_OPEN)
            else:
                self._rejected_calls += 1
                raise CircuitBreakerError("Circuit breaker is open")
        
        # Check half-open call limit
        if (self._state == CircuitState.HALF_OPEN and 
            self._half_open_calls >= self._config.half_open_max_calls):
            self._rejected_calls += 1
            raise CircuitBreakerError(
                "Circuit breaker is half-open and at max calls"
            )
        
        try:
            if self._state == CircuitState.HALF_OPEN:
                self._half_open_calls += 1
            
            result = await func()
            self._on_success()
            return result
        except Exception as e:
            self._on_failure(e)
            raise
    
    def force_open(self) -> None:
        """Force the circuit to open state."""
        self._transition_to(CircuitState.OPEN)
    
    def force_close(self) -> None:
        """Force the circuit to closed state."""
        self._transition_to(CircuitState.CLOSED)
    
    def reset(self) -> None:
        """Reset all statistics and state."""
        self._failures = 0
        self._successes = 0
        self._rejected_calls = 0
        self._total_calls = 0
        self._half_open_calls = 0
        self._failure_history = []
        self._transition_to(CircuitState.CLOSED)
    
    def _should_attempt_reset(self) -> bool:
        """Check if we should attempt reset from open to half-open."""
        if self._last_failure is None:
            return True
        return (time.time() * 1000 - self._last_failure) >= self._config.timeout_ms
    
    def _on_success(self) -> None:
        """Handle successful execution."""
        self._last_success = time.time()
        self._failure_history = []
        
        if self._state == CircuitState.HALF_OPEN:
            self._successes += 1
            if self._successes >= self._config.success_threshold:
                self._transition_to(CircuitState.CLOSED)
        elif self._state == CircuitState.CLOSED:
            self._failures = 0
    
    def _on_failure(self, error: Exception) -> None:
        """Handle failed execution."""
        self._last_failure = time.time()
        self._failures += 1
        
        # Record failure for window
        self._failure_history.append(FailureRecord(
            timestamp=time.time(),
            error=error
        ))
        
        # Clean old failures outside window
        cutoff = time.time() - (self._config.failure_window_ms / 1000)
        self._failure_history = [
            f for f in self._failure_history if f.timestamp >= cutoff
        ]
        
        if self._state == CircuitState.HALF_OPEN:
            # Any failure in half-open immediately opens
            self._transition_to(CircuitState.OPEN)
        elif self._state == CircuitState.CLOSED:
            # Check if we should open based on failure count
            failure_count = (
                len(self._failure_history) 
                if self._config.failure_window_ms > 0 
                else self._failures
            )
            
            if failure_count >= self._config.failure_threshold:
                self._transition_to(CircuitState.OPEN)
    
    def _transition_to(self, new_state: CircuitState) -> None:
        """Transition to a new state."""
        if self._state == new_state:
            return
        
        old_state = self._state
        self._state = new_state
        self._last_state_change = time.time()
        
        # Reset counters on state change
        if new_state == CircuitState.CLOSED:
            self._failures = 0
            self._successes = 0
            self._half_open_calls = 0
            self._failure_history = []
            if self._config.on_close:
                self._config.on_close()
        elif new_state == CircuitState.OPEN:
            self._successes = 0
            self._half_open_calls = 0
            if self._config.on_open:
                self._config.on_open()
        elif new_state == CircuitState.HALF_OPEN:
            self._successes = 0
            self._half_open_calls = 0
            if self._config.on_half_open:
                self._config.on_half_open()


class CircuitBreakerManager:
    """Manages multiple circuit breakers by name."""
    
    def __init__(self, default_config: Optional[CircuitBreakerConfig] = None):
        self._breakers: Dict[str, CircuitBreaker] = {}
        self._default_config = default_config or CircuitBreakerConfig()
    
    def get(self, name: str, config: Optional[CircuitBreakerConfig] = None) -> CircuitBreaker:
        """Get or create a circuit breaker by name."""
        if name not in self._breakers:
            merged_config = CircuitBreakerConfig(
                failure_threshold=config.failure_threshold if config else self._default_config.failure_threshold,
                success_threshold=config.success_threshold if config else self._default_config.success_threshold,
                timeout_ms=config.timeout_ms if config else self._default_config.timeout_ms,
                half_open_max_calls=config.half_open_max_calls if config else self._default_config.half_open_max_calls,
                failure_window_ms=config.failure_window_ms if config else self._default_config.failure_window_ms,
            )
            self._breakers[name] = CircuitBreaker(merged_config)
        return self._breakers[name]
    
    def get_all_stats(self) -> Dict[str, CircuitBreakerStats]:
        """Get statistics for all circuit breakers."""
        return {name: breaker.get_stats() for name, breaker in self._breakers.items()}
    
    def reset_all(self) -> None:
        """Reset all circuit breakers."""
        for breaker in self._breakers.values():
            breaker.reset()
    
    def get_open_breakers(self) -> list[str]:
        """Get names of all open circuit breakers."""
        return [
            name for name, breaker in self._breakers.items()
            if breaker.is_open
        ]
