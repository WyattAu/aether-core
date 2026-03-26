import json
import os
import tempfile

import pytest

from aether_sdk.messaging import Message, MessageType


VECTORS_PATH = os.path.join(os.path.dirname(__file__), "test_vectors.json")

COMMON_MESSAGE_TYPES = {"start", "stop", "signal", "rpc_request", "rpc_response", "custom"}

COMMON_FIELDS = {"type", "payload", "sender", "correlation_id"}

JS_ONLY_FIELDS = {"priority"}


def _load_vectors():
    with open(VECTORS_PATH) as f:
        return json.load(f)


class TestMessageSerializationStructure:
    def test_custom_message_json_structure(self):
        msg = Message(type=MessageType.CUSTOM, payload={"key": "value", "count": 42})
        raw = json.loads(msg.to_json())

        assert set(raw.keys()) == COMMON_FIELDS
        assert raw["type"] == "custom"
        assert raw["payload"] == {"key": "value", "count": 42}
        assert raw["sender"] is None
        assert raw["correlation_id"] is None

    def test_rpc_request_json_structure(self):
        msg = Message(
            type=MessageType.RPC_REQUEST,
            payload={"method": "get_user", "args": [1]},
            sender="client-actor",
            correlation_id="corr-abc-123",
        )
        raw = json.loads(msg.to_json())

        assert raw["type"] == "rpc_request"
        assert raw["sender"] == "client-actor"
        assert raw["correlation_id"] == "corr-abc-123"
        assert raw["payload"]["method"] == "get_user"

    def test_round_trip(self):
        msg = Message(
            type=MessageType.CUSTOM,
            payload={"foo": "bar", "nested": {"a": 1}},
            sender="actor-a",
            correlation_id="c-1",
        )
        restored = Message.from_json(msg.to_json())

        assert restored.type == MessageType.CUSTOM
        assert restored.payload == {"foo": "bar", "nested": {"a": 1}}
        assert restored.sender == "actor-a"
        assert restored.correlation_id == "c-1"

    def test_all_common_types_produce_valid_json(self):
        for mt in MessageType:
            if mt.value not in COMMON_MESSAGE_TYPES:
                continue
            msg = Message(type=mt, payload={})
            raw = json.loads(msg.to_json())
            assert raw["type"] == mt.value
            assert isinstance(raw["payload"], dict)
            assert raw["sender"] is None
            assert raw["correlation_id"] is None

    def test_nested_payload_serialization(self):
        nested = {"l1": {"l2": {"l3": [1, 2, {"k": "v"}]}}}
        msg = Message(type=MessageType.CUSTOM, payload=nested)
        raw = json.loads(msg.to_json())
        assert raw["payload"]["l1"]["l2"]["l3"][2]["k"] == "v"

    def test_python_json_compatible_with_js_contract(self):
        msg = Message(
            type=MessageType.CUSTOM,
            payload={"action": "test"},
            sender="py-actor",
            correlation_id="corr-x",
        )
        raw = json.loads(msg.to_json())

        assert "type" in raw
        assert "payload" in raw
        assert "sender" in raw
        assert "correlation_id" in raw
        for key in raw:
            assert key in COMMON_FIELDS, f"Unexpected field '{key}' not in JS contract"


class TestMessageCrossSDKFileExchange:
    def test_serialize_to_file_for_js_consumer(self):
        msg = Message(
            type=MessageType.RPC_REQUEST,
            payload={"fn": "add", "args": [1, 2]},
            sender="py-sender",
            correlation_id="cross-sdk-001",
        )
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            f.write(msg.to_json())
            path = f.name

        try:
            with open(path) as f:
                data = json.load(f)
            assert data["type"] == "rpc_request"
            assert data["payload"]["fn"] == "add"
            assert data["sender"] == "py-sender"
            assert data["correlation_id"] == "cross-sdk-001"
        finally:
            os.unlink(path)

    def test_deserialize_from_js_format(self):
        js_produced = json.dumps({
            "type": "custom",
            "payload": {"key": "value"},
            "sender": "js-actor",
            "correlationId": "corr-js",
            "priority": 1,
        })
        obj = json.loads(js_produced)

        msg = Message(
            type=MessageType(obj["type"]),
            payload=obj["payload"],
            sender=obj.get("sender") or obj.get("sender"),
            correlation_id=obj.get("correlation_id") or obj.get("correlationId"),
        )

        assert msg.type == MessageType.CUSTOM
        assert msg.payload == {"key": "value"}
        assert msg.sender == "js-actor"


class TestMessageTestVectors:
    def test_vectors_file_exists(self):
        assert os.path.isfile(VECTORS_PATH)

    def test_all_message_vectors_produce_valid_json(self):
        vectors = _load_vectors()
        for vec in vectors["messages"]:
            msg = Message(
                type=MessageType(vec["input"]["type"]),
                payload=vec["input"]["payload"],
                sender=vec["input"].get("sender"),
                correlation_id=vec["input"].get("correlation_id"),
            )
            raw = json.loads(msg.to_json())
            assert raw["type"] == vec["expected_type"]
            assert set(raw["payload"].keys()) == set(vec["expected_payload_keys"])
            if "expected_sender" in vec:
                assert raw["sender"] == vec["expected_sender"]
            if "expected_correlation_id" in vec:
                assert raw["correlation_id"] == vec["expected_correlation_id"]

    def test_message_types_in_vectors_match_sdk(self):
        vectors = _load_vectors()
        sdk_types = {mt.value for mt in MessageType}
        for type_str in vectors["message_types"]["all_types"]:
            assert type_str in sdk_types, f"Vector type '{type_str}' not in Python SDK"
