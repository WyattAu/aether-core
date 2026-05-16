"""
Circuit Breaker Pattern Implementation

Prevents cascading failures by stopping requests to a failing service.

States:
- CLOSED: Normal operation, requests pass through
- OPEN: Failing, requests are rejected immediately
- HALF_OPEN: Testing if service recovered

Example:
    >>> from aether_sdk.resilience.circuit_breaker import CircuitBreaker, CircuitBreakerConfig
    >>> cb = CircuitBreaker(CircuitBreakerConfig(failure_threshold=3))
    >>> try:
    ...     result = await cb.execute(some_async_func)
    ... except CircuitBreakerError:
    ...     print("Service unavailable")
"""

from __future__ import annotations

import time
from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable, Dict, Optional


class CircuitState(Enum):
    """Possible states of a circuit breaker.

    Attributes:
        CLOSED: Normal operation — requests pass through.
        OPEN: Failing — requests are rejected immediately.
        HALF_OPEN: Probing — a limited number of requests are allowed
            to test whether the service has recovered.
    """

    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half-open"


@dataclass
class CircuitBreakerConfig:
    """Configuration parameters for a :class:`CircuitBreaker`.

    Attributes:
        failure_threshold: Number of failures before opening the circuit.
        success_threshold: Number of consecutive successes in half-open
            state before closing the circuit.
        timeout_ms: Milliseconds to wait before transitioning from
            open to half-open.
        half_open_max_calls: Maximum number of calls allowed while
            in half-open state.
        failure_window_ms: Sliding time window (ms) for counting
            failures. Set to ``0`` to use cumulative failure count.
        on_open: Callback invoked when the circuit opens.
        on_close: Callback invoked when the circuit closes.
        on_half_open: Callback invoked when the circuit enters
            half-open state.
    """

    failure_threshold: int = 5
    success_threshold: int = 3
    timeout_ms: int = 30000
    half_open_max_calls: int = 3
    failure_window_ms: int = 60000
    on_open: Optional[Callable[[], None]] = None
    on_close: Optional[Callable[[], None]] = None
    on_half_open: Optional[Callable[[], None]] = None


@dataclass
class CircuitBreakerStats:
    """Snapshot of a circuit breaker's current statistics.

    Attributes:
        state: Current circuit state.
        failures: Total recorded failures.
        successes: Total recorded successes.
        rejected_calls: Number of calls rejected while the circuit
            was open or half-open at capacity.
        total_calls: Total number of calls attempted.
        last_failure: Unix timestamp of the most recent failure.
        last_success: Unix timestamp of the most recent success.
        last_state_change: Unix timestamp of the most recent state
            transition.
    """

    state: CircuitState = CircuitState.CLOSED
    failures: int = 0
    successes: int = 0
    rejected_calls: int = 0
    total_calls: int = 0
    last_failure: Optional[float] = None
    last_success: Optional[float] = None
    last_state_change: Optional[float] = None


class CircuitBreakerError(Exception):
    """Raised when a call is rejected because the circuit is open.

    Example:
        >>> try:
        ...     await cb.execute(func)
        ... except CircuitBreakerError as e:
        ...     print(f"Blocked: {e}")
    """

    pass


@dataclass
class FailureRecord:
    """Record of a single failure within the sliding window.

    Attributes:
        timestamp: Unix timestamp when the failure occurred.
        error: The exception that caused the failure.
    """

    timestamp: float
    error: Exception


class CircuitBreaker:
    """Circuit breaker for protecting against cascading failures.

    Wraps async function calls and monitors for failures. When the
    failure count exceeds the configured threshold, the circuit opens
    and subsequent calls are rejected with :class:`CircuitBreakerError`.
    After the timeout elapses, the circuit transitions to half-open and
    allows a limited number of probe calls. If those succeed, the
    circuit closes again.

    Example:
        >>> cb = CircuitBreaker()
        >>> try:
        ...     result = await cb.execute(my_async_func)
        ... except CircuitBreakerError:
        ...     print("Circuit is open — service degraded")
    """

    def __init__(self, config: Optional[CircuitBreakerConfig] = None):
        """Initialize the circuit breaker.

        Args:
            config: Optional configuration. Defaults to
                :class:`CircuitBreakerConfig` with sensible defaults.
        """
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
        """Return the current circuit state."""
        return self._state

    @property
    def is_closed(self) -> bool:
        """Check whether the circuit is in the CLOSED state."""
        return self._state == CircuitState.CLOSED

    @property
    def is_open(self) -> bool:
        """Check whether the circuit is in the OPEN state."""
        return self._state == CircuitState.OPEN

    @property
    def is_half_open(self) -> bool:
        """Check whether the circuit is in the HALF_OPEN state."""
        return self._state == CircuitState.HALF_OPEN

    def get_stats(self) -> CircuitBreakerStats:
        """Return a snapshot of the current circuit breaker statistics.

        Returns:
            A :class:`CircuitBreakerStats` dataclass.
        """
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
        """Execute an async function through the circuit breaker.

        Args:
            func: A zero-argument async callable.

        Returns:
            The result of *func* if the call succeeds.

        Raises:
            CircuitBreakerError: If the circuit is open or half-open
                at max calls.
            Exception: Propagates any exception raised by *func*.
        """
        self._total_calls += 1

        if self._state == CircuitState.OPEN:
            if self._should_attempt_reset():
                self._transition_to(CircuitState.HALF_OPEN)
            else:
                self._rejected_calls += 1
                raise CircuitBreakerError("Circuit breaker is open")

        if (
            self._state == CircuitState.HALF_OPEN
            and self._half_open_calls >= self._config.half_open_max_calls
        ):
            self._rejected_calls += 1
            raise CircuitBreakerError("Circuit breaker is half-open and at max calls")

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
        """Force the circuit into the OPEN state.

        Sets the last failure timestamp to the current time to prevent
        an immediate reset to half-open.
        """
        self._last_failure = time.time() * 1000
        self._transition_to(CircuitState.OPEN)

    def force_close(self) -> None:
        """Force the circuit into the CLOSED state, resetting failure counters."""
        self._transition_to(CircuitState.CLOSED)

    def reset(self) -> None:
        """Reset all statistics and return the circuit to CLOSED state."""
        self._failures = 0
        self._successes = 0
        self._rejected_calls = 0
        self._total_calls = 0
        self._half_open_calls = 0
        self._failure_history = []
        self._transition_to(CircuitState.CLOSED)

    def _should_attempt_reset(self) -> bool:
        """Check whether enough time has elapsed to transition OPEN → HALF_OPEN.

        Returns:
            ``True`` if the timeout since the last failure has elapsed.
        """
        if self._last_failure is None:
            return True
        return (time.time() * 1000 - self._last_failure) >= self._config.timeout_ms

    def _on_success(self) -> None:
        """Update internal state after a successful execution."""
        self._last_success = time.time()
        self._failure_history = []

        if self._state == CircuitState.HALF_OPEN:
            self._successes += 1
            if self._successes >= self._config.success_threshold:
                self._transition_to(CircuitState.CLOSED)
        elif self._state == CircuitState.CLOSED:
            self._failures = 0

    def _on_failure(self, error: Exception) -> None:
        """Update internal state after a failed execution.

        Args:
            error: The exception that caused the failure.
        """
        self._last_failure = time.time()
        self._failures += 1

        self._failure_history.append(FailureRecord(timestamp=time.time(), error=error))

        cutoff = time.time() - (self._config.failure_window_ms / 1000)
        self._failure_history = [
            f for f in self._failure_history if f.timestamp >= cutoff
        ]

        if self._state == CircuitState.HALF_OPEN:
            self._transition_to(CircuitState.OPEN)
        elif self._state == CircuitState.CLOSED:
            failure_count = (
                len(self._failure_history)
                if self._config.failure_window_ms > 0
                else self._failures
            )

            if failure_count >= self._config.failure_threshold:
                self._transition_to(CircuitState.OPEN)

    def _transition_to(self, new_state: CircuitState) -> None:
        """Transition to a new circuit state and invoke callbacks.

        Args:
            new_state: The target state.
        """
        if self._state == new_state:
            return

        self._state = new_state
        self._last_state_change = time.time()

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
    """Registry for named :class:`CircuitBreaker` instances.

    Provides ``get-or-create`` semantics so that the same name always
    resolves to the same breaker instance.

    Example:
        >>> mgr = CircuitBreakerManager()
        >>> cb = mgr.get("payment-service")
    """

    def __init__(self, default_config: Optional[CircuitBreakerConfig] = None):
        """Initialize the manager.

        Args:
            default_config: Default configuration applied to breakers
                that do not supply their own config.
        """
        self._breakers: Dict[str, CircuitBreaker] = {}
        self._default_config = default_config or CircuitBreakerConfig()

    def get(
        self, name: str, config: Optional[CircuitBreakerConfig] = None
    ) -> CircuitBreaker:
        """Get or create a circuit breaker by name.

        If a breaker with *name* already exists, the existing instance
        is returned and *config* is ignored. Otherwise a new breaker
        is created with the merged configuration.

        Args:
            name: Unique name for the breaker.
            config: Optional per-breaker configuration. Fields set here
                override the manager's defaults.

        Returns:
            The :class:`CircuitBreaker` instance for *name*.
        """
        if name not in self._breakers:
            merged_config = CircuitBreakerConfig(
                failure_threshold=(
                    config.failure_threshold
                    if config
                    else self._default_config.failure_threshold
                ),
                success_threshold=(
                    config.success_threshold
                    if config
                    else self._default_config.success_threshold
                ),
                timeout_ms=(
                    config.timeout_ms if config else self._default_config.timeout_ms
                ),
                half_open_max_calls=(
                    config.half_open_max_calls
                    if config
                    else self._default_config.half_open_max_calls
                ),
                failure_window_ms=(
                    config.failure_window_ms
                    if config
                    else self._default_config.failure_window_ms
                ),
            )
            self._breakers[name] = CircuitBreaker(merged_config)
        return self._breakers[name]

    def get_all_stats(self) -> Dict[str, CircuitBreakerStats]:
        """Return statistics for every registered circuit breaker.

        Returns:
            A dict mapping breaker names to their
            :class:`CircuitBreakerStats`.
        """
        return {name: breaker.get_stats() for name, breaker in self._breakers.items()}

    def reset_all(self) -> None:
        """Reset all registered circuit breakers to their initial state."""
        for breaker in self._breakers.values():
            breaker.reset()

    def get_open_breakers(self) -> list[str]:
        """Return the names of all circuit breakers currently in OPEN state.

        Returns:
            A list of breaker names.
        """
        return [name for name, breaker in self._breakers.items() if breaker.is_open]
