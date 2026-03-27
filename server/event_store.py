import time
from collections import defaultdict, deque
from typing import Any, Dict, List, Optional

from .models import EventRecord


class EventStore:
    def __init__(self, history_size: int = 10000):
        self._events: Dict[str, List[EventRecord]] = defaultdict(list)
        self._snapshots: Dict[str, Dict] = {}
        self._versions: Dict[str, int] = {}
        self._all_events: List[EventRecord] = deque(maxlen=history_size)
        self._counter = 0

    def append(
        self,
        aggregate_id: str,
        event_type: str,
        data: Any = None,
        expected_version: Optional[int] = None,
    ) -> EventRecord:
        current = self._versions.get(aggregate_id, 0)
        if expected_version is not None and expected_version != current:
            raise ValueError(
                f"Version conflict: expected {expected_version}, actual {current}"
            )

        new_version = current + 1
        self._counter += 1
        record = EventRecord(
            event_id=f"evt_{self._counter}",
            aggregate_id=aggregate_id,
            event_type=event_type,
            data=data,
            version=new_version,
        )
        self._events[aggregate_id].append(record)
        self._versions[aggregate_id] = new_version
        self._all_events.append(record)
        return record

    def get_events(self, aggregate_id: str) -> List[EventRecord]:
        return list(self._events.get(aggregate_id, []))

    def get_events_by_type(self, event_type: str) -> List[EventRecord]:
        return [e for e in self._all_events if e.event_type == event_type]

    def get_snapshot(self, aggregate_id: str) -> Optional[Dict]:
        return self._snapshots.get(aggregate_id)

    def create_snapshot(self, aggregate_id: str, state: Dict) -> None:
        version = self._versions.get(aggregate_id, 0)
        self._snapshots[aggregate_id] = {"state": state, "version": version}

    def get_version(self, aggregate_id: str) -> int:
        return self._versions.get(aggregate_id, 0)
