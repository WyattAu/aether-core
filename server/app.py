import logging
import time
import uuid
from contextlib import asynccontextmanager
from typing import Optional

from fastapi import FastAPI, Request, Response
from fastapi.middleware.cors import CORSMiddleware

from .config import ServerConfig
from .actor_manager import ActorManager
from .message_router import MessageRouter
from .state_store import StateStore
from .pubsub_service import PubSubService
from .event_store import EventStore

logger = logging.getLogger("aether-server")

_actor_manager: Optional[ActorManager] = None
_message_router: Optional[MessageRouter] = None
_state_store: Optional[StateStore] = None
_pubsub_service: Optional[PubSubService] = None
_event_store: Optional[EventStore] = None


def get_actor_manager() -> ActorManager:
    assert _actor_manager is not None
    return _actor_manager


def get_message_router() -> MessageRouter:
    assert _message_router is not None
    return _message_router


def get_state_store() -> StateStore:
    assert _state_store is not None
    return _state_store


def get_pubsub_service() -> PubSubService:
    assert _pubsub_service is not None
    return _pubsub_service


def get_event_store() -> EventStore:
    assert _event_store is not None
    return _event_store


@asynccontextmanager
async def lifespan(app: FastAPI):
    global _actor_manager, _message_router, _state_store, _pubsub_service, _event_store
    config = ServerConfig()
    _actor_manager = ActorManager(config)
    _message_router = MessageRouter(message_ttl=config.message_ttl_seconds)
    _state_store = StateStore()
    _pubsub_service = PubSubService()
    _event_store = EventStore()
    logger.info("Aether server started")
    yield
    logger.info("Aether server shutting down")


def create_app() -> FastAPI:
    app = FastAPI(
        title="Aether Server",
        description="Aether protocol reference server",
        version="0.1.0",
        lifespan=lifespan,
    )

    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    @app.middleware("http")
    async def request_id_middleware(request: Request, call_next):
        request_id = request.headers.get("X-Request-ID", str(uuid.uuid4()))
        response: Response = await call_next(request)
        response.headers["X-Request-ID"] = request_id
        return response

    from .api.actors import router as actors_router
    from .api.state import router as state_router
    from .api.events import router as events_router
    from .api.health import router as health_router
    from .websocket_handler import router as ws_router

    app.include_router(actors_router)
    app.include_router(state_router)
    app.include_router(events_router)
    app.include_router(health_router)
    app.include_router(ws_router)

    @app.exception_handler(ValueError)
    async def value_error_handler(request, exc):
        from fastapi.responses import JSONResponse
        return JSONResponse(status_code=400, detail=str(exc))

    return app


app = create_app()


def main():
    import uvicorn
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=8080)
    args = parser.parse_args()
    uvicorn.run("server.app:app", host=args.host, port=args.port, reload=True)


if __name__ == "__main__":
    main()
