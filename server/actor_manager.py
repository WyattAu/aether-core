import time
from collections import defaultdict
from datetime import datetime, timezone
from typing import Dict, List, Optional

from .config import ServerConfig
from .models import ActorInfo, ActorRegistration


class ActorManager:
    def __init__(self, config: ServerConfig):
        self._config = config
        self._actors: Dict[str, ActorInfo] = {}
        self._actor_handlers: Dict[str, callable] = {}

    def register(
        self,
        actor_id: str,
        actor_type: str = "default",
        capabilities: Optional[List[str]] = None,
        metadata: Optional[dict] = None,
    ) -> ActorInfo:
        if len(self._actors) >= self._config.max_actors:
            raise RuntimeError("Maximum actor limit reached")
        if actor_id in self._actors:
            raise ValueError(f"Actor {actor_id} already registered")

        info = ActorInfo(
            actor_id=actor_id,
            actor_type=actor_type,
            capabilities=capabilities or [],
            metadata=metadata or {},
            status="active",
            created_at=datetime.now(timezone.utc),
            last_heartbeat=datetime.now(timezone.utc),
        )
        self._actors[actor_id] = info
        return info

    def unregister(self, actor_id: str) -> bool:
        if actor_id not in self._actors:
            return False
        del self._actors[actor_id]
        self._actor_handlers.pop(actor_id, None)
        return True

    def get_actor(self, actor_id: str) -> Optional[ActorInfo]:
        return self._actors.get(actor_id)

    def list_actors(
        self,
        actor_type: Optional[str] = None,
        status: Optional[str] = None,
    ) -> List[ActorInfo]:
        actors = list(self._actors.values())
        if actor_type is not None:
            actors = [a for a in actors if a.actor_type == actor_type]
        if status is not None:
            actors = [a for a in actors if a.status == status]
        return actors

    def update_status(self, actor_id: str, status: str) -> bool:
        actor = self._actors.get(actor_id)
        if actor is None:
            return False
        actor.status = status
        return True

    def update_heartbeat(self, actor_id: str) -> bool:
        actor = self._actors.get(actor_id)
        if actor is None:
            return False
        actor.last_heartbeat = datetime.now(timezone.utc)
        return True

    def count(self) -> int:
        return len(self._actors)
