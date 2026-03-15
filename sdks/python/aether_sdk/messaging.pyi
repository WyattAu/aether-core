"""Type stubs for messaging module."""

from dataclasses import dataclass
from typing import Optional, Any
from enum import Enum
import json

class MessageType(Enum):
    START: str = "start"
    STOP: str = "stop"
    SIGNAL: str = "signal"
    RPC_REQUEST: str = "rpc_request"
    RPC_RESPONSE: str = "rpc_response"
    CUSTOM: str = "custom"

@dataclass
class Message:
    """Actor message."""
    type: MessageType
    payload: Any
    sender: Optional[str] = None
    correlation_id: Optional[str] = None
    
    def to_json(self) -> str: ...
    
    @classmethod
    def from_json(cls, data: str) -> "Message": ...
