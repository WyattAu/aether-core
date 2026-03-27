import asyncio
import fnmatch
import time
from collections import defaultdict, deque
from typing import Any, Callable, Dict, List, Optional

from .models import PubSubMessage, Subscription


class PubSubService:
    def __init__(self, history_size: int = 100):
        self._subscriptions: Dict[str, Dict[str, Subscription]] = defaultdict(dict)
        self._history: Dict[str, deque] = defaultdict(lambda: deque(maxlen=history_size))
        self._sub_counter = 0

    def publish(self, topic: str, payload: Any = None, headers: Optional[Dict[str, str]] = None) -> int:
        msg = PubSubMessage(topic=topic, payload=payload, headers=headers or {})
        self._history[topic].append(msg)

        count = 0
        for sub_id, sub in self._subscriptions.get(topic, {}).items():
            count += 1
        return count

    def publish_with_handler(
        self,
        topic: str,
        handler: Callable,
        payload: Any = None,
        headers: Optional[Dict[str, str]] = None,
    ) -> int:
        msg = PubSubMessage(topic=topic, payload=payload, headers=headers or {})
        self._history[topic].append(msg)

        count = 0
        for sub_id, sub in self._subscriptions.get(topic, {}).items():
            try:
                handler(sub.subscriber_id, msg)
            except Exception:
                pass
            count += 1
        return count

    def subscribe(
        self,
        topic: str,
        subscriber_id: str,
        filter: Optional[str] = None,
    ) -> str:
        self._sub_counter += 1
        sub_id = f"sub_{self._sub_counter}"
        sub = Subscription(
            subscription_id=sub_id,
            topic=topic,
            subscriber_id=subscriber_id,
            filter=filter,
        )
        self._subscriptions[topic][sub_id] = sub
        return sub_id

    def unsubscribe(self, subscription_id: str) -> bool:
        for topic, subs in self._subscriptions.items():
            if subscription_id in subs:
                del subs[subscription_id]
                if not subs:
                    del self._subscriptions[topic]
                return True
        return False

    def list_topics(self) -> List[str]:
        return list(self._subscriptions.keys())

    def list_subscribers(self, topic: str) -> List[str]:
        subs = self._subscriptions.get(topic, {})
        return [s.subscriber_id for s in subs.values()]

    def get_history(self, topic: str) -> List[PubSubMessage]:
        return list(self._history.get(topic, []))

    def get_matching_subscribers(self, topic: str) -> List[Subscription]:
        results = []
        for pattern, subs in self._subscriptions.items():
            if fnmatch.fnmatch(topic, pattern):
                results.extend(subs.values())
        return results
