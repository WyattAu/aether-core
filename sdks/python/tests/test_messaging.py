from aether_sdk.messaging import Message, MessageType


class TestMessage:
    def test_message_creation(self):
        msg = Message(type=MessageType.CUSTOM, payload={"key": "value"})
        assert msg.type == MessageType.CUSTOM
        assert msg.payload == {"key": "value"}
        assert msg.sender is None
        assert msg.correlation_id is None

    def test_message_with_sender(self):
        msg = Message(type=MessageType.SIGNAL, payload={}, sender="actor1")
        assert msg.sender == "actor1"

    def test_message_to_json(self):
        msg = Message(
            type=MessageType.CUSTOM,
            payload={"test": 123},
            sender="sender",
            correlation_id="corr-123",
        )
        json_str = msg.to_json()
        assert '"type": "custom"' in json_str
        assert '"test": 123' in json_str
        assert '"sender": "sender"' in json_str

    def test_message_from_json(self):
        json_str = '{"type": "custom", "payload": {"test": 123}, "sender": "sender", "correlation_id": "corr-123"}'
        msg = Message.from_json(json_str)
        assert msg.type == MessageType.CUSTOM
        assert msg.payload == {"test": 123}
        assert msg.sender == "sender"
        assert msg.correlation_id == "corr-123"

    def test_message_serialization_roundtrip(self):
        original = Message(
            type=MessageType.RPC_REQUEST,
            payload={"method": "test", "args": [1, 2, 3]},
            sender="caller",
            correlation_id="req-001",
        )
        json_str = original.to_json()
        restored = Message.from_json(json_str)

        assert restored.type == original.type
        assert restored.payload == original.payload
        assert restored.sender == original.sender
        assert restored.correlation_id == original.correlation_id


class TestMessageType:
    def test_message_type_values(self):
        assert MessageType.START.value == "start"
        assert MessageType.STOP.value == "stop"
        assert MessageType.SIGNAL.value == "signal"
        assert MessageType.RPC_REQUEST.value == "rpc_request"
        assert MessageType.RPC_RESPONSE.value == "rpc_response"
        assert MessageType.CUSTOM.value == "custom"
