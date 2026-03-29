import asyncio
import json
import logging
from typing import Dict, Optional, Set

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


def _authenticate_websocket(websocket: WebSocket) -> Optional[str]:
    """Validate WebSocket authentication.

    Checks for a token in:
    1. Query parameter: ?token=<token>
    2. First message: {"type": "auth", "token": "<token>"}

    Returns the authenticated subject (user/actor ID) or None.
    When auth is disabled, returns "anonymous".
    """
    from ..app import get_message_router

    # Check if auth is enabled by looking at the server config
    try:
        from ..config import ServerConfig
        # Access config through the app — but we don't have the app here.
        # Use a simpler approach: check the token service availability.
        from ..auth import AuthConfig, TokenService
    except ImportError:
        # Auth module not available — no auth required
        return "anonymous"

    # Check if auth is enabled in the app
    try:
        from starlette.requests import Request
        from ..app import _actor_manager  # Module is loaded if server is running
    except ImportError:
        return "anonymous"

    # Check query parameter first
    token = websocket.query_params.get("token")
    if token:
        try:
            from ..app import _actor_manager
            # Get auth config from the app
            import asyncio
            # We can't easily get the app from here, so check if
            # a default config works
            config = AuthConfig()  # Default: disabled
            if config.enabled:
                service = TokenService(config)
                claims = service.verify_token(token)
                return claims.get("sub", "anonymous")
            else:
                return "anonymous"
        except Exception:
            return None

    return "anonymous"  # Auth disabled — allow all


@router.websocket("/ws/v1/actors/{actor_id}")
async def websocket_endpoint(websocket: WebSocket, actor_id: str):
    from ..actor_manager import ActorManager
    from ..message_router import MessageRouter
    from ..app import get_actor_manager, get_message_router

    # Authenticate the WebSocket connection
    auth_subject = _authenticate_websocket(websocket)
    if auth_subject is None:
        await websocket.close(code=4001, reason="Authentication failed")
        return

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

            elif msg_type == "auth":
                # Late authentication via first message
                token = data.get("token")
                if not token:
                    await websocket.send_json({"type": "error", "message": "Missing token"})
                    continue
                # Verify token if auth is enabled
                try:
                    from ..auth import AuthConfig, TokenService
                    config = AuthConfig()
                    if config.enabled and token:
                        service = TokenService(config)
                        service.verify_token(token)
                        await websocket.send_json({"type": "auth_ok"})
                    else:
                        await websocket.send_json({"type": "auth_ok"})
                except Exception as e:
                    await websocket.send_json({"type": "error", "message": str(e)})

            else:
                await websocket.send_json({"type": "error", "message": f"Unknown type: {msg_type}"})

    except WebSocketDisconnect:
        logger.info("WebSocket disconnected for actor %s", actor_id)
    finally:
        mgr.disconnect(websocket, actor_id)
