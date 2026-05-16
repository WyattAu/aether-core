"""Message types for Aether actor communication.

This module defines :class:`MessageType` (an enumeration of supported
message kinds) and :class:`Message` (a dataclass carrying payloads,
sender metadata, and correlation IDs).

Example:
    >>> from aether_sdk.messaging import Message, MessageType
    >>> msg = Message(type=MessageType.CUSTOM, payload={"key": "value"})
    >>> json_str = msg.to_json()
    >>> restored = Message.from_json(json_str)
"""

import json
from dataclasses import dataclass
from enum import Enum
from typing import Any, Optional


class MessageType(Enum):
    """Enumeration of supported message types.

    Attributes:
        START: Signal an actor to start.
        STOP: Signal an actor to stop.
        SIGNAL: General-purpose signal.
        RPC_REQUEST: Request for a remote procedure call.
        RPC_RESPONSE: Response to an RPC request.
        CUSTOM: User-defined message.
        STREAM_EVENT: A streaming data event.
        WATERMARK: Watermark indicating event-time progress.
        CHECKPOINT: Checkpoint barrier for exactly-once semantics.
        CHECKPOINT_ACK: Acknowledgement of a checkpoint barrier.
    """

    START = "start"
    STOP = "stop"
    SIGNAL = "signal"
    RPC_REQUEST = "rpc_request"
    RPC_RESPONSE = "rpc_response"
    CUSTOM = "custom"
    STREAM_EVENT = "stream_event"
    WATERMARK = "watermark"
    CHECKPOINT = "checkpoint"
    CHECKPOINT_ACK = "checkpoint_ack"


@dataclass
class Message:
    """A message exchanged between actors.

    Attributes:
        type: The kind of message.
        payload: Arbitrary data carried by the message.
        sender: Name of the sending actor (set automatically by
            :meth:`Actor.send <aether_sdk.actor.Actor.send>`).
        correlation_id: Optional identifier used to match RPC responses
            to their requests.
    """

    type: MessageType
    payload: Any
    sender: Optional[str] = None
    correlation_id: Optional[str] = None

    def to_json(self) -> str:
        """Serialize the message to a JSON string.

        Returns:
            A JSON-formatted string representation of the message.
        """
        return json.dumps(
            {
                "type": self.type.value,
                "payload": self.payload,
                "sender": self.sender,
                "correlation_id": self.correlation_id,
            }
        )

    @classmethod
    def from_json(cls, data: str) -> "Message":
        """Deserialize a message from a JSON string.

        Args:
            data: JSON string produced by :meth:`to_json`.

        Returns:
            A reconstructed :class:`Message` instance.

        Raises:
            KeyError: If required fields are missing from the JSON.
            ValueError: If the type value is not a valid :class:`MessageType`.
        """
        obj = json.loads(data)
        return cls(
            type=MessageType(obj["type"]),
            payload=obj["payload"],
            sender=obj.get("sender"),
            correlation_id=obj.get("correlation_id"),
        )
