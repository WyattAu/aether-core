"""Baseline mutation testing configuration.

Run with: mutmut run --paths-to-mutate=aether_sdk

This file documents the expected mutation survival targets.
"""
import pytest

MUTATION_TARGETS = [
    "aether_sdk/resilience/retry.py",
    "aether_sdk/resilience/circuit_breaker.py",
    "aether_sdk/resilience/bulkhead.py",
    "aether_sdk/resilience/rate_limiter.py",
    "aether_sdk/streaming/backpressure.py",
    "aether_sdk/streaming/window.py",
    "aether_sdk/validation/sanitize.py",
    "aether_sdk/validation/validators.py",
]

@pytest.mark.parametrize("module", MUTATION_TARGETS)
def test_mutation_target_exists(module):
    """Verify mutation target modules exist."""
    import importlib
    importlib.import_module(module.replace("/", ".").replace(".py", ""))
