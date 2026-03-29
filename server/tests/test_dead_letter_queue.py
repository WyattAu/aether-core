import threading
import time

import pytest

from server.dead_letter_queue import DeadLetterEntry, DeadLetterQueue
from server.models import MessageEnvelope


def _make_envelope(
    message_id: str = "msg_1",
    target: str = "actor-1",
    source: str = "sender",
    message_type: str = "test",
    payload=None,
) -> MessageEnvelope:
    return MessageEnvelope(
        source_actor=source,
        target_actor=target,
        message_type=message_type,
        payload=payload,
        correlation_id="corr-1",
        message_id=message_id,
    )


class TestDeadLetterEntry:

    def test_create_entry(self):
        entry = DeadLetterEntry(
            message=_make_envelope(),
            reason="Handler failed",
        )
        assert entry.message.message_id == "msg_1"
        assert entry.reason == "Handler failed"
        assert entry.retry_count == 1
        assert entry.source_node == ""

    def test_to_dict(self):
        entry = DeadLetterEntry(
            message=_make_envelope(),
            reason="timeout",
            source_node="node-1",
            metadata={"key": "value"},
        )
        d = entry.to_dict()
        assert d["message_id"] == "msg_1"
        assert d["reason"] == "timeout"
        assert d["source_node"] == "node-1"
        assert d["metadata"] == {"key": "value"}
        assert "first_failed_at" in d
        assert "last_failed_at" in d


class TestEnqueue:

    def test_enqueue_new_message(self):
        dlq = DeadLetterQueue()
        entry = dlq.enqueue(_make_envelope(), "failed")
        assert dlq.size == 1
        assert entry.message_id == "msg_1"
        assert dlq.total_enqueued == 1

    def test_enqueue_duplicate_increments_retry(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail1")
        dlq.enqueue(_make_envelope(message_id="m1"), "fail2")
        assert dlq.size == 1
        entry = dlq.get("m1")
        assert entry.retry_count == 2
        assert entry.reason == "fail2"

    def test_enqueue_callback(self):
        calls = []
        dlq = DeadLetterQueue(on_enqueue=lambda e: calls.append(e.message_id))
        dlq.enqueue(_make_envelope(), "fail")
        assert calls == ["msg_1"]

    def test_enqueue_callback_error_is_caught(self):
        dlq = DeadLetterQueue(on_enqueue=lambda e: 1 / 0)
        dlq.enqueue(_make_envelope(), "fail")


class TestGet:

    def test_get_existing(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        entry = dlq.get("m1")
        assert entry is not None
        assert entry.message_id == "m1"

    def test_get_missing(self):
        dlq = DeadLetterQueue()
        assert dlq.get("nonexistent") is None


class TestList:

    def test_list_all(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail1")
        dlq.enqueue(_make_envelope(message_id="m2"), "fail2")
        dlq.enqueue(_make_envelope(message_id="m3"), "fail3")
        entries = dlq.list_messages()
        assert len(entries) == 3

    def test_filter_by_actor(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1", target="actor-a"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2", target="actor-b"), "fail")
        dlq.enqueue(_make_envelope(message_id="m3", target="actor-a"), "fail")
        entries = dlq.list_messages(actor_id="actor-a")
        assert len(entries) == 2
        assert all(e.message.target_actor == "actor-a" for e in entries)

    def test_filter_by_source(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1", source="s1"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2", source="s2"), "fail")
        entries = dlq.list_messages(source_actor="s1")
        assert len(entries) == 1

    def test_filter_by_type(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1", message_type="alert"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2", message_type="task"), "fail")
        entries = dlq.list_messages(message_type="alert")
        assert len(entries) == 1

    def test_pagination_limit(self):
        dlq = DeadLetterQueue()
        for i in range(10):
            dlq.enqueue(_make_envelope(message_id=f"m{i}"), "fail")
        entries = dlq.list_messages(limit=3)
        assert len(entries) == 3

    def test_pagination_offset(self):
        dlq = DeadLetterQueue()
        for i in range(10):
            dlq.enqueue(_make_envelope(message_id=f"m{i}"), "fail")
        entries = dlq.list_messages(offset=5)
        assert len(entries) == 5

    def test_pagination_combined(self):
        dlq = DeadLetterQueue()
        for i in range(10):
            dlq.enqueue(_make_envelope(message_id=f"m{i}"), "fail")
        entries = dlq.list_messages(offset=3, limit=3)
        assert len(entries) == 3

    def test_empty_queue(self):
        dlq = DeadLetterQueue()
        assert dlq.list_messages() == []

    def test_sorted_by_most_recent(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail1")
        time.sleep(0.01)
        dlq.enqueue(_make_envelope(message_id="m2"), "fail2")
        entries = dlq.list_messages()
        assert entries[0].message_id == "m2"


class TestRemove:

    def test_remove_existing(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        assert dlq.remove("m1") is True
        assert dlq.size == 0

    def test_remove_missing(self):
        dlq = DeadLetterQueue()
        assert dlq.remove("ghost") is False


class TestReplay:

    def test_replay_existing(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        envelope = dlq.replay("m1")
        assert envelope is not None
        assert envelope.message_id == "m1"
        assert dlq.size == 0
        assert dlq.total_replayed == 1

    def test_replay_missing(self):
        dlq = DeadLetterQueue()
        assert dlq.replay("ghost") is None

    def test_replay_callback(self):
        calls = []
        dlq = DeadLetterQueue(on_replay=lambda e: calls.append(e.message_id))
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        dlq.replay("m1")
        assert calls == ["m1"]

    def test_replay_all(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2"), "fail")
        dlq.enqueue(_make_envelope(message_id="m3"), "fail")
        messages = dlq.replay_all()
        assert len(messages) == 3
        assert dlq.size == 0
        assert dlq.total_replayed == 3


class TestPurge:

    def test_purge_all(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2"), "fail")
        count = dlq.purge()
        assert count == 2
        assert dlq.size == 0
        assert dlq.total_purged == 2

    def test_purge_empty(self):
        dlq = DeadLetterQueue()
        assert dlq.purge() == 0


class TestMaxSizeEviction:

    def test_eviction_fifo(self):
        dlq = DeadLetterQueue(max_size=3)
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2"), "fail")
        dlq.enqueue(_make_envelope(message_id="m3"), "fail")
        assert dlq.size == 3
        dlq.enqueue(_make_envelope(message_id="m4"), "fail")
        assert dlq.size == 3
        assert dlq.get("m1") is None
        assert dlq.get("m4") is not None

    def test_eviction_increments_expired(self):
        dlq = DeadLetterQueue(max_size=2)
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2"), "fail")
        dlq.enqueue(_make_envelope(message_id="m3"), "fail")
        assert dlq.total_expired == 1


class TestTTLExpiry:

    def test_ttl_expires_old_entries(self):
        dlq = DeadLetterQueue(max_size=100, ttl_seconds=0.05)
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        time.sleep(0.1)
        entries = dlq.list_messages()
        assert len(entries) == 0
        assert dlq.size == 0

    def test_no_ttl_keeps_all(self):
        dlq = DeadLetterQueue(max_size=100, ttl_seconds=0)
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        time.sleep(0.01)
        entries = dlq.list_messages()
        assert len(entries) == 1

    def test_ttl_does_not_affect_new_entries(self):
        dlq = DeadLetterQueue(max_size=100, ttl_seconds=0.05)
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2"), "fail")
        time.sleep(0.1)
        dlq.enqueue(_make_envelope(message_id="m3"), "fail")
        entries = dlq.list_messages()
        assert len(entries) == 1
        assert entries[0].message_id == "m3"


class TestStats:

    def test_stats_empty(self):
        dlq = DeadLetterQueue()
        stats = dlq.get_stats()
        assert stats["size"] == 0
        assert stats["total_enqueued"] == 0
        assert stats["total_replayed"] == 0
        assert stats["total_purged"] == 0

    def test_stats_with_data(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1", target="a"), "fail")
        dlq.enqueue(_make_envelope(message_id="m2", target="a"), "fail")
        dlq.enqueue(_make_envelope(message_id="m3", target="b"), "fail")

        dlq.replay("m1")
        dlq.purge()

        stats = dlq.get_stats()
        assert stats["size"] == 0
        assert stats["total_enqueued"] == 3
        assert stats["total_replayed"] == 1
        assert stats["total_purged"] == 2

    def test_stats_oldest_age(self):
        dlq = DeadLetterQueue()
        dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        time.sleep(0.05)
        stats = dlq.get_stats()
        assert stats["oldest_message_age_seconds"] >= 0.04


class TestThreadSafety:

    def test_concurrent_enqueue(self):
        dlq = DeadLetterQueue(max_size=1000)
        errors = []

        def enqueuer(i):
            try:
                for j in range(50):
                    dlq.enqueue(_make_envelope(message_id=f"m{i}-{j}"), "fail")
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=enqueuer, args=(i,)) for i in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert not errors
        assert dlq.total_enqueued == 500
        assert dlq.size == 500


class TestDLQAPI:

    def test_list_messages_endpoint(self):
        from fastapi.testclient import TestClient
        from server.app import create_app
        from server.dead_letter_queue import DeadLetterQueue

        app = create_app()
        app.state.dlq = DeadLetterQueue()

        from server.api.dlq import router as dlq_router
        app.include_router(dlq_router, prefix="/dlq")

        app.state.dlq.enqueue(_make_envelope(message_id="m1"), "fail1")
        app.state.dlq.enqueue(_make_envelope(message_id="m2"), "fail2")

        client = TestClient(app, raise_server_exceptions=False)
        resp = client.get("/dlq/messages")
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 2
        assert len(data["messages"]) == 2

    def test_replay_endpoint(self):
        from fastapi.testclient import TestClient
        from server.app import create_app
        from server.dead_letter_queue import DeadLetterQueue
        from server.message_router import MessageRouter

        app = create_app()

        dlq = DeadLetterQueue()
        msg_router = MessageRouter()
        app.state.dlq = dlq
        app.state.message_router = msg_router

        from server.api.dlq import router as dlq_router
        app.include_router(dlq_router, prefix="/dlq")

        dlq.enqueue(_make_envelope(message_id="m1"), "fail")

        client = TestClient(app, raise_server_exceptions=False)
        resp = client.post("/dlq/messages/m1/replay")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] in ("delivered", "buffered")
        assert dlq.size == 0

    def test_stats_endpoint(self):
        from fastapi.testclient import TestClient
        from server.app import create_app
        from server.dead_letter_queue import DeadLetterQueue

        app = create_app()
        app.state.dlq = DeadLetterQueue()
        from server.api.dlq import router as dlq_router
        app.include_router(dlq_router, prefix="/dlq")

        app.state.dlq.enqueue(_make_envelope(message_id="m1"), "fail")

        client = TestClient(app, raise_server_exceptions=False)
        resp = client.get("/dlq/stats")
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_enqueued"] == 1
        assert data["size"] == 1

    def test_delete_single(self):
        from fastapi.testclient import TestClient
        from server.app import create_app
        from server.dead_letter_queue import DeadLetterQueue

        app = create_app()
        app.state.dlq = DeadLetterQueue()
        from server.api.dlq import router as dlq_router
        app.include_router(dlq_router, prefix="/dlq")

        app.state.dlq.enqueue(_make_envelope(message_id="m1"), "fail")

        client = TestClient(app, raise_server_exceptions=False)
        resp = client.delete("/dlq/messages/m1")
        assert resp.status_code == 200
        assert resp.json()["removed"] is True
        assert app.state.dlq.size == 0

    def test_purge_endpoint(self):
        from fastapi.testclient import TestClient
        from server.app import create_app
        from server.dead_letter_queue import DeadLetterQueue

        app = create_app()
        app.state.dlq = DeadLetterQueue()
        from server.api.dlq import router as dlq_router
        app.include_router(dlq_router, prefix="/dlq")

        app.state.dlq.enqueue(_make_envelope(message_id="m1"), "fail")
        app.state.dlq.enqueue(_make_envelope(message_id="m2"), "fail")

        client = TestClient(app, raise_server_exceptions=False)
        resp = client.delete("/dlq/messages")
        assert resp.status_code == 200
        assert resp.json()["purged"] == 2

    def test_not_found(self):
        from fastapi.testclient import TestClient
        from server.app import create_app
        from server.dead_letter_queue import DeadLetterQueue

        app = create_app()
        app.state.dlq = DeadLetterQueue()
        from server.api.dlq import router as dlq_router
        app.include_router(dlq_router, prefix="/dlq")

        client = TestClient(app, raise_server_exceptions=False)
        resp = client.get("/dlq/messages/nonexistent")
        assert resp.status_code == 404
