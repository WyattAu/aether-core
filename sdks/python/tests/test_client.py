"""Tests for AetherClient against the reference server."""

import pytest
from aether_sdk.client import AetherServerError


class TestHealth:
    @pytest.mark.asyncio
    async def test_server_health(self, client):
        info = await client.health()
        assert info.status == "ok"
        assert info.uptime >= 0

    @pytest.mark.asyncio
    async def test_server_info(self, client):
        info = await client.info()
        assert "version" in info
        assert info["uptime"] >= 0
        assert "actor_count" in info
        assert "message_count" in info


class TestActorOperations:
    @pytest.mark.asyncio
    async def test_register_actor(self, client):
        info = await client.register_actor(
            "test-1", "worker", capabilities=["read", "write"]
        )
        assert info.actor_id == "test-1"
        assert info.actor_type == "worker"
        assert info.status == "active"
        assert "read" in info.capabilities

    @pytest.mark.asyncio
    async def test_register_duplicate_fails(self, client):
        await client.register_actor("dup-1", "worker")
        with pytest.raises(AetherServerError) as exc_info:
            await client.register_actor("dup-1", "worker")
        assert exc_info.value.status_code == 409

    @pytest.mark.asyncio
    async def test_get_actor(self, client):
        await client.register_actor("get-1", "worker")
        info = await client.get_actor("get-1")
        assert info.actor_id == "get-1"
        assert info.actor_type == "worker"

    @pytest.mark.asyncio
    async def test_get_nonexistent_actor(self, client):
        with pytest.raises(AetherServerError) as exc_info:
            await client.get_actor("nope")
        assert exc_info.value.status_code == 404

    @pytest.mark.asyncio
    async def test_unregister(self, client):
        await client.register_actor("unreg-1", "worker")
        await client.unregister_actor("unreg-1")
        with pytest.raises(AetherServerError) as exc_info:
            await client.get_actor("unreg-1")
        assert exc_info.value.status_code == 404

    @pytest.mark.asyncio
    async def test_list_actors(self, client):
        await client.register_actor("list-1", "worker")
        await client.register_actor("list-2", "scheduler")
        actors = await client.list_actors()
        assert len(actors) >= 2

    @pytest.mark.asyncio
    async def test_list_actors_by_type(self, client):
        await client.register_actor("type-1", "worker")
        await client.register_actor("type-2", "scheduler")
        workers = await client.list_actors(actor_type="worker")
        assert all(a.actor_type == "worker" for a in workers)
        assert len(workers) >= 1

    @pytest.mark.asyncio
    async def test_heartbeat(self, client):
        await client.register_actor("hb-1", "worker")
        await client.heartbeat("hb-1")
        info = await client.get_actor("hb-1")
        assert info.last_heartbeat is not None

    @pytest.mark.asyncio
    async def test_heartbeat_nonexistent(self, client):
        with pytest.raises(AetherServerError) as exc_info:
            await client.heartbeat("nope")
        assert exc_info.value.status_code == 404


class TestMessaging:
    @pytest.mark.asyncio
    async def test_send_message(self, client):
        await client.register_actor("msg-sender", "worker")
        await client.register_actor("msg-target", "worker")
        receipt = await client.send_message(
            "msg-target", {"data": "hello"}, source="msg-sender"
        )
        assert receipt.status in ("delivered", "buffered")
        assert receipt.message_id

    @pytest.mark.asyncio
    async def test_send_message_with_correlation_id(self, client):
        await client.register_actor("corr-src", "worker")
        await client.register_actor("corr-tgt", "worker")
        receipt = await client.send_message(
            "corr-tgt",
            {"data": "test"},
            source="corr-src",
            correlation_id="my-corr-id",
        )
        assert receipt.correlation_id == "my-corr-id"

    @pytest.mark.asyncio
    async def test_get_pending_messages(self, client):
        await client.register_actor("pend-1", "worker")
        await client.register_actor("pend-2", "worker")
        await client.send_message("pend-2", {"data": "test"}, source="pend-1")
        messages = await client.get_pending_messages("pend-2")
        assert len(messages) >= 1
        assert messages[0].source_actor == "pend-1"
        assert messages[0].target_actor == "pend-2"

    @pytest.mark.asyncio
    async def test_send_to_nonexistent_actor(self, client):
        receipt = await client.send_message("nope", {"data": "test"})
        assert receipt.status == "buffered"


class TestStateManagement:
    @pytest.mark.asyncio
    async def test_set_and_get_state(self, client):
        entry = await client.set_state("state-1", "counter", 42)
        assert entry.version >= 1
        value = await client.get_state("state-1", "counter")
        assert value == 42

    @pytest.mark.asyncio
    async def test_get_nonexistent_state(self, client):
        value = await client.get_state("state-1", "nope")
        assert value is None

    @pytest.mark.asyncio
    async def test_delete_state(self, client):
        await client.set_state("del-1", "temp", "value")
        result = await client.delete_state("del-1", "temp")
        assert result is True
        value = await client.get_state("del-1", "temp")
        assert value is None

    @pytest.mark.asyncio
    async def test_delete_nonexistent_state(self, client):
        result = await client.delete_state("nope", "nope")
        assert result is False

    @pytest.mark.asyncio
    async def test_optimistic_concurrency(self, client):
        entry = await client.set_state("occ-1", "key", "v1")
        v1 = entry.version
        entry2 = await client.set_state("occ-1", "key", "v2", version=v1)
        assert entry2.version == v1 + 1

    @pytest.mark.asyncio
    async def test_optimistic_concurrency_conflict(self, client):
        entry = await client.set_state("occ-2", "key", "v1")
        with pytest.raises(AetherServerError) as exc_info:
            await client.set_state("occ-2", "key", "v2", version=entry.version - 1)
        assert exc_info.value.status_code == 409

    @pytest.mark.asyncio
    async def test_get_all_state(self, client):
        await client.set_state("all-1", "a", 1)
        await client.set_state("all-1", "b", 2)
        state = await client.get_all_state("all-1")
        assert "a" in state
        assert state["a"] == 1
        assert "b" in state
        assert state["b"] == 2


class TestPubSub:
    @pytest.mark.asyncio
    async def test_publish_and_subscribe(self, client):
        sub_id = await client.subscribe("test.topic", "sub-1")
        assert isinstance(sub_id, str)
        count = await client.publish("test.topic", {"msg": "hello"})
        assert isinstance(count, int)
        assert count >= 1

    @pytest.mark.asyncio
    async def test_list_topics(self, client):
        await client.subscribe("topic.list", "sub-1")
        topics = await client.list_topics()
        assert "topic.list" in topics

    @pytest.mark.asyncio
    async def test_unsubscribe(self, client):
        sub_id = await client.subscribe("topic.unsub", "sub-1")
        result = await client.unsubscribe(sub_id)
        assert result is True

    @pytest.mark.asyncio
    async def test_unsubscribe_nonexistent(self, client):
        result = await client.unsubscribe("nope")
        assert result is False

    @pytest.mark.asyncio
    async def test_topic_history(self, client):
        await client.subscribe("hist.topic", "sub-1")
        await client.publish("hist.topic", {"msg": "recorded"})
        history = await client.get_topic_history("hist.topic")
        assert len(history) >= 1


class TestEventSourcing:
    @pytest.mark.asyncio
    async def test_append_and_get_events(self, client):
        event = await client.append_event("agg-1", "ItemCreated", {"name": "Widget"})
        assert event.event_id
        assert event.event_type == "ItemCreated"
        assert event.version == 1
        events = await client.get_events("agg-1")
        assert len(events) >= 1
        assert events[0].event_type == "ItemCreated"

    @pytest.mark.asyncio
    async def test_event_versioning(self, client):
        await client.append_event("agg-2", "Created", {"id": 1})
        event2 = await client.append_event("agg-2", "Updated", {"id": 1})
        assert event2.version == 2
        events = await client.get_events("agg-2")
        assert len(events) == 2

    @pytest.mark.asyncio
    async def test_event_version_conflict(self, client):
        await client.append_event("agg-3", "Created", {})
        with pytest.raises(AetherServerError) as exc_info:
            await client.append_event("agg-3", "Dup", {}, expected_version=0)
        assert exc_info.value.status_code == 409

    @pytest.mark.asyncio
    async def test_get_events_empty(self, client):
        events = await client.get_events("no-aggregate")
        assert events == []
