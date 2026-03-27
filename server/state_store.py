import threading
from collections import defaultdict
from datetime import datetime, timezone
from typing import Any, Dict, Optional, Tuple

from .models import StateEntry


class StateStore:
    def __init__(self):
        self._store: Dict[str, Dict[str, StateEntry]] = defaultdict(dict)
        self._lock = threading.Lock()
        self._change_callbacks: list = []

    def get(self, actor_id: str, key: str) -> Optional[Any]:
        entry = self._store.get(actor_id, {}).get(key)
        return entry.value if entry else None

    def set(self, actor_id: str, key: str, value: Any, expected_version: Optional[int] = None) -> StateEntry:
        with self._lock:
            existing = self._store.get(actor_id, {}).get(key)
            if existing is not None:
                if expected_version is not None and existing.version != expected_version:
                    raise ValueError(
                        f"Version conflict: expected {expected_version}, actual {existing.version}"
                    )
                new_version = existing.version + 1
            else:
                if expected_version is not None and expected_version != 0:
                    raise ValueError(f"Version conflict: expected {expected_version}, but entry does not exist")
                new_version = 1

            entry = StateEntry(
                actor_id=actor_id,
                key=key,
                value=value,
                version=new_version,
                updated_at=datetime.now(timezone.utc),
            )
            self._store[actor_id][key] = entry
            for cb in self._change_callbacks:
                try:
                    cb(actor_id, key, value, new_version)
                except Exception:
                    pass
            return entry

    def delete(self, actor_id: str, key: str) -> bool:
        with self._lock:
            bucket = self._store.get(actor_id)
            if bucket and key in bucket:
                del bucket[key]
                return True
            return False

    def get_all(self, actor_id: str) -> Dict[str, Any]:
        bucket = self._store.get(actor_id, {})
        return {k: v.value for k, v in bucket.items()}

    def on_change(self, callback):
        self._change_callbacks.append(callback)
