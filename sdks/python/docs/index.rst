Aether Python SDK
=================

Aether is a distributed actor framework SDK for Python, providing
messaging, state management, resilience patterns, streaming, workflows,
and event-driven architecture.

.. toctree::
   :maxdepth: 2
   :caption: Contents

   quickstart
   api/core
   api/event
   api/resilience
   api/streaming
   api/workflow
   api/validation

Quick Start
-----------

Install the SDK::

    pip install aether-sdk

Create an actor::

    from aether_sdk import Actor, Context

    class MyActor(Actor):
        async def on_message(self, message, ctx: Context):
            await ctx.reply(f"Got: {message}")

Indices and tables
==================

* :ref:`genindex`
* :ref:`modindex`
* :ref:`search`
