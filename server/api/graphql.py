from __future__ import annotations

import asyncio
import logging
from typing import AsyncGenerator, Optional

GRAPHQL_AVAILABLE = False

try:
    import strawberry
    from strawberry.fastapi import GraphQLRouter
    from strawberry.tools import merge_types
    from strawberry.subscriptions import GRAPHQL_TRANSPORT_WS_PROTOCOL
    GRAPHQL_AVAILABLE = True
except ImportError:
    strawberry = None
    GraphQLRouter = None


def _get_actor_manager():
    from ..app import get_actor_manager
    return get_actor_manager()


def _get_state_store():
    from ..app import get_state_store
    return get_state_store()


def _get_event_store():
    from ..app import get_event_store
    return get_event_store()


def _get_pubsub_service():
    from ..app import get_pubsub_service
    return get_pubsub_service()


def _get_message_router():
    from ..app import get_message_router
    return get_message_router()


def _get_context():
    """Build the GraphQL context with auth info.

    The Strawberry FastAPI router automatically provides 'request',
    'background_tasks', and 'response' in the context dict. We
    augment it with 'auth' claims when available.

    Note: this function must take NO parameters — Strawberry wraps
    it as a FastAPI Depends, and any parameter would become a query
    param in the request validation schema.
    """
    return {"auth": None}


if GRAPHQL_AVAILABLE and strawberry is not None:

    @strawberry.type
    class ActorType:
        actor_id: str
        actor_type: str
        status: str
        capabilities: list[str]
        created_at: str

    @strawberry.type
    class MessageType:
        message_id: str
        source_actor: str
        target_actor: str
        message_type: str
        payload: str
        timestamp: str

    @strawberry.type
    class StateEntryType:
        key: str
        value: str
        version: int
        updated_at: str

    @strawberry.type
    class EventType:
        event_id: str
        aggregate_id: str
        event_type: str
        data: str
        version: int
        timestamp: str

    @strawberry.type
    class PubSubTopicType:
        name: str
        subscriber_count: int
        message_count: int

    @strawberry.type
    class Query:
        @strawberry.field
        def actors(self, actor_type: Optional[str] = None) -> list[ActorType]:
            mgr = _get_actor_manager()
            results = mgr.list_actors(actor_type=actor_type)
            return [
                ActorType(
                    actor_id=a.actor_id,
                    actor_type=a.actor_type,
                    status=a.status,
                    capabilities=a.capabilities,
                    created_at=a.created_at.isoformat(),
                )
                for a in results
            ]

        @strawberry.field
        def actor(self, actor_id: str) -> Optional[ActorType]:
            mgr = _get_actor_manager()
            a = mgr.get_actor(actor_id)
            if a is None:
                return None
            return ActorType(
                actor_id=a.actor_id,
                actor_type=a.actor_type,
                status=a.status,
                capabilities=a.capabilities,
                created_at=a.created_at.isoformat(),
            )

        @strawberry.field
        def actor_state(self, actor_id: str) -> list[StateEntryType]:
            store = _get_state_store()
            from ..app import get_actor_manager
            mgr = get_actor_manager()
            if mgr.get_actor(actor_id) is None:
                raise ValueError(f"Actor {actor_id} not found")
            bucket = store._store.get(actor_id, {})
            return [
                StateEntryType(
                    key=k,
                    value=str(v.value),
                    version=v.version,
                    updated_at=v.updated_at.isoformat(),
                )
                for k, v in bucket.items()
            ]

        @strawberry.field
        def events(
            self,
            aggregate_id: Optional[str] = None,
            event_type: Optional[str] = None,
        ) -> list[EventType]:
            store = _get_event_store()
            if aggregate_id:
                records = store.get_events(aggregate_id)
            elif event_type:
                records = store.get_events_by_type(event_type)
            else:
                records = list(store._all_events)
            return [
                EventType(
                    event_id=e.event_id,
                    aggregate_id=e.aggregate_id,
                    event_type=e.event_type,
                    data=str(e.data) if e.data is not None else "",
                    version=e.version,
                    timestamp=e.timestamp.isoformat(),
                )
                for e in records
            ]

        @strawberry.field
        def topics(self) -> list[PubSubTopicType]:
            pubsub = _get_pubsub_service()
            result = []
            for topic_name in pubsub.list_topics():
                subs = pubsub._subscriptions.get(topic_name, {})
                history = pubsub._history.get(topic_name, [])
                result.append(PubSubTopicType(
                    name=topic_name,
                    subscriber_count=len(subs),
                    message_count=len(history),
                ))
            return result

        @strawberry.field
        def topic_history(self, topic: str, limit: int = 50) -> list[MessageType]:
            pubsub = _get_pubsub_service()
            msgs = pubsub.get_history(topic)
            msgs = msgs[-limit:] if limit else msgs
            return [
                MessageType(
                    message_id=m.message_id,
                    source_actor=m.headers.get("source_actor", ""),
                    target_actor=m.headers.get("target_actor", ""),
                    message_type=m.headers.get("message_type", ""),
                    payload=str(m.payload) if m.payload is not None else "",
                    timestamp=m.timestamp.isoformat(),
                )
                for m in msgs
            ]

    @strawberry.type
    class Mutation:
        @strawberry.mutation
        def register_actor(self, actor_id: str, actor_type: str) -> ActorType:
            mgr = _get_actor_manager()
            info = mgr.register(actor_id=actor_id, actor_type=actor_type)
            return ActorType(
                actor_id=info.actor_id,
                actor_type=info.actor_type,
                status=info.status,
                capabilities=info.capabilities,
                created_at=info.created_at.isoformat(),
            )

        @strawberry.mutation
        async def send_message(
            self,
            target: str,
            payload: str,
            message_type: str = "command",
        ) -> MessageType:
            router = _get_message_router()
            from ..models import MessageEnvelope
            envelope = MessageEnvelope(
                source_actor="graphql",
                target_actor=target,
                message_type=message_type,
                payload=payload,
            )
            receipt = await router.route(envelope)
            return MessageType(
                message_id=receipt.message_id,
                source_actor="graphql",
                target_actor=target,
                message_type=message_type,
                payload=payload,
                timestamp=receipt.delivered_at.isoformat(),
            )

        @strawberry.mutation
        def set_state(self, actor_id: str, key: str, value: str) -> StateEntryType:
            store = _get_state_store()
            entry = store.set(actor_id, key, value)
            return StateEntryType(
                key=entry.key,
                value=str(entry.value),
                version=entry.version,
                updated_at=entry.updated_at.isoformat(),
            )

    @strawberry.type
    class Subscription:
        """GraphQL subscription for receiving pub/sub events in real-time.

        Requires WebSocket transport (graphql-transport-ws protocol).

        Example with GraphQL client::

            const ws = new GraphQLWebSocket('ws://localhost:8080/graphql', {
                connectionParams: { token: 'your-token-here' },
            });
            ws.subscribe({
                query: 'subscription { pubsub_events(topic: "my-topic") { '
                       + 'messageId sourceActor payload timestamp } }',
            });
        """

        @strawberry.subscription
        async def pubsub_events(
            self,
            topic: str,
        ) -> AsyncGenerator[MessageType, None]:
            """Subscribe to pub/sub events for a topic.

            Args:
                topic: The pub/sub topic to subscribe to.
                    Supports wildcards (e.g., "events.*").
            """
            import fnmatch

            pubsub = _get_pubsub_service()
            queue: asyncio.Queue = asyncio.Queue()

            def _on_publish(pub_topic: str, msg) -> None:
                if fnmatch.fnmatch(pub_topic, topic):
                    try:
                        queue.put_nowait(msg)
                    except asyncio.QueueFull:
                        pass  # Drop if consumer is slow

            pubsub.add_publish_listener(_on_publish)
            try:
                while True:
                    msg = await queue.get()
                    yield MessageType(
                        message_id=msg.message_id,
                        source_actor=msg.headers.get("source_actor", ""),
                        target_actor=msg.headers.get("target_actor", ""),
                        message_type=msg.headers.get("message_type", ""),
                        payload=str(msg.payload) if msg.payload is not None else "",
                        timestamp=msg.timestamp.isoformat(),
                    )
            except asyncio.CancelledError:
                pass
            finally:
                pubsub.remove_publish_listener(_on_publish)

    schema = strawberry.Schema(
        query=Query,
        mutation=Mutation,
        subscription=Subscription,
    )
    graphql_app = GraphQLRouter(
        schema,
        context_getter=_get_context,
    )
else:
    schema = None
    graphql_app = None
