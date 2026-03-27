import asyncio
import json
import logging
from typing import Dict, Set

from fastapi import APIRouter, WebSocket, WebSocketDisconnect

logger = logging.getLogger(__name__)

router = APIRouter()


class ConnectionManager:
    def __init__(self):
        self._connections: Dict[str, Set[WebSocket]] = {}
        self._actor_subscriptions: Dict[str, Set[str]] = {}

    async def connect(self, websocket: WebSocket, actor_id: str):
        await websocket.accept()
        if actor_id not in self._connections:
            self._connections[actor_id] = set()
        self._connections[actor_id].add(websocket)

    def disconnect(self, websocket: WebSocket, actor_id: str):
        conns = self._connections.get(actor_id)
        if conns:
            conns.discard(websocket)
            if not conns:
                del self._connections[actor_id]

    async def send_to_actor(self, actor_id: str, message: dict):
        conns = self._connections.get(actor_id, set())
        dead = []
        for ws in conns:
            try:
                await ws.send_json(message)
            except Exception:
                dead.append(ws)
        for ws in dead:
            conns.discard(ws)
        return len(conns) - len(dead)

    async def broadcast(self, message: dict):
        for actor_id, conns in list(self._connections.items()):
            for ws in list(conns):
                try:
                    await ws.send_json(message)
                except Exception:
                    conns.discard(ws)


_manager = ConnectionManager()


def get_manager() -> ConnectionManager:
    return _manager


@router.websocket("/ws/v1/actors/{actor_id}")
async def websocket_endpoint(websocket: WebSocket, actor_id: str):
    from ..actor_manager import ActorManager
    from ..message_router import MessageRouter
    from ..app import get_actor_manager, get_message_router

    mgr = get_manager()
    await mgr.connect(websocket, actor_id)
    actor_mgr = get_actor_manager()
    router = get_message_router()

    if not actor_mgr.get_actor(actor_id):
        actor_mgr.register(actor_id)

    try:
        while True:
            raw = await websocket.receive_text()
            try:
                data = json.loads(raw)
            except json.JSONDecodeError:
                await websocket.send_json({"type": "error", "message": "Invalid JSON"})
                continue

            msg_type = data.get("type", "")

            if msg_type == "ping":
                await websocket.send_json({"type": "pong"})
                actor_mgr.update_heartbeat(actor_id)

            elif msg_type == "message":
                target = data.get("target")
                if not target:
                    await websocket.send_json({"type": "error", "message": "Missing target"})
                    continue
                from ..models import MessageEnvelope
                envelope = MessageEnvelope(
                    source_actor=actor_id,
                    target_actor=target,
                    message_type=data.get("message_type", "default"),
                    payload=data.get("payload"),
                )
                receipt = await router.route(envelope)
                await mgr.send_to_actor(target, {
                    "type": "message",
                    "source": actor_id,
                    "payload": data.get("payload"),
                    "message_id": receipt.message_id,
                    "status": receipt.status,
                })
                await websocket.send_json({
                    "type": "delivery",
                    "message_id": receipt.message_id,
                    "status": receipt.status,
                })

            elif msg_type == "subscribe":
                topic = data.get("topic")
                if not topic:
                    await websocket.send_json({"type": "error", "message": "Missing topic"})
                    continue

            else:
                await websocket.send_json({"type": "error", "message": f"Unknown type: {msg_type}"})

    except WebSocketDisconnect:
        logger.info("WebSocket disconnected for actor %s", actor_id)
    finally:
        mgr.disconnect(websocket, actor_id)
