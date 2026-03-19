from dataclasses import dataclass
from typing import Optional, Any
from enum import Enum
import json


class MessageType(Enum):
    START = "start"
    STOP = "stop"
    SIGNAL = "signal"
    RPC_REQUEST = "rpc_request"
    RPC_RESPONSE = "rpc_response"
    CUSTOM = "custom"
    # Streaming message types
    STREAM_EVENT = "stream_event"
    WATERMARK = "watermark"
    CHECKPOINT = "checkpoint"
    CHECKPOINT_ACK = "checkpoint_ack"


@dataclass
class Message:
    """Actor message."""
    type: MessageType
    payload: Any
    sender: Optional[str] = None
    correlation_id: Optional[str] = None
    
    def to_json(self) -> str:
        return json.dumps({
            "type": self.type.value,
            "payload": self.payload,
            "sender": self.sender,
            "correlation_id": self.correlation_id,
        })
    
    @classmethod
    def from_json(cls, data: str) -> "Message":
        obj = json.loads(data)
        return cls(
            type=MessageType(obj["type"]),
            payload=obj["payload"],
            sender=obj.get("sender"),
            correlation_id=obj.get("correlation_id"),
        )
