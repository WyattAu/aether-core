import asyncio
from collections import defaultdict, deque
from datetime import datetime, timezone
from typing import Callable, Dict, List, Optional

from .models import DeliveryReceipt, MessageEnvelope


class MessageRouter:
    def __init__(self, max_buffer_size: int = 1000, message_ttl: int = 300):
        self._handlers: Dict[str, Callable] = {}
        self._pending: Dict[str, deque] = defaultdict(lambda: deque(maxlen=max_buffer_size))
        self._receipts: Dict[str, DeliveryReceipt] = {}
        self._message_ttl = message_ttl
        self._total_messages = 0

    def register_handler(self, actor_id: str, handler_fn: Callable):
        self._handlers[actor_id] = handler_fn

    def unregister_handler(self, actor_id: str):
        self._handlers.pop(actor_id, None)

    async def route(self, envelope: MessageEnvelope) -> DeliveryReceipt:
        self._total_messages += 1
        handler = self._handlers.get(envelope.target_actor)

        if handler is not None:
            try:
                if asyncio.iscoroutinefunction(handler):
                    await handler(envelope)
                else:
                    handler(envelope)
                status = "delivered"
            except Exception:
                status = "failed"
                self._pending[envelope.target_actor].append(envelope)
        else:
            status = "buffered"
            self._pending[envelope.target_actor].append(envelope)

        receipt = DeliveryReceipt(
            message_id=envelope.message_id,
            status=status,
            delivered_at=datetime.now(timezone.utc),
            correlation_id=envelope.correlation_id,
        )
        self._receipts[envelope.message_id] = receipt
        return receipt

    def get_pending_messages(self, actor_id: str) -> List[MessageEnvelope]:
        return list(self._pending.get(actor_id, []))

    def clear_pending(self, actor_id: str) -> int:
        count = len(self._pending.get(actor_id, []))
        self._pending[actor_id].clear()
        return count

    def get_receipt(self, message_id: str) -> Optional[DeliveryReceipt]:
        return self._receipts.get(message_id)

    def total_message_count(self) -> int:
        return self._total_messages
