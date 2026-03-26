Quick Start Guide
=================

Installation
------------

Install the SDK::

    pip install aether-sdk

Creating Your First Actor
--------------------------

.. code-block:: python

    from aether_sdk import Actor, Context

    class GreetingActor(Actor):
        async def on_message(self, message: str, ctx: Context):
            await ctx.reply(f"Hello, {message}!")

Next Steps
----------

* :doc:`api/core` - Core actor, messaging, and state APIs
* :doc:`api/event` - Event system (pub/sub, event sourcing)
* :doc:`api/resilience` - Resilience patterns (circuit breaker, retry)
* :doc:`api/streaming` - Streaming (backpressure, windowing, batching)
* :doc:`api/workflow` - Workflow (saga, state machine, human tasks)
* :doc:`api/validation` - Validation utilities
