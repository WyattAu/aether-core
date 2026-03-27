import logging
import time
import uuid
from contextlib import asynccontextmanager
from typing import Optional

from fastapi import FastAPI, Request, Response
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import PlainTextResponse

from .config import ServerConfig
from .actor_manager import ActorManager
from .message_router import MessageRouter
from .state_store import StateStore, create_state_store
from .pubsub_service import PubSubService
from .event_store import EventStore
from .tracing import TRACING_AVAILABLE, setup_tracing, trace_span, get_trace_id_hex

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
    config = getattr(app.state, "server_config", ServerConfig())

    # Setup structured logging
    from .logging_config import setup_logging
    setup_logging(level=config.log_level, json_enabled=config.json_logging_enabled)

    setup_tracing()

    # Setup graceful shutdown
    from .shutdown import ShutdownManager
    shutdown_mgr = ShutdownManager(drain_timeout_seconds=config.drain_timeout_seconds)
    app.state.shutdown_manager = shutdown_mgr

    # Setup metrics
    from .metrics import MetricsCollector
    metrics = MetricsCollector()
    app.state.metrics = metrics

    _actor_manager = ActorManager(config)
    _message_router = MessageRouter(message_ttl=config.message_ttl_seconds)
    _state_store = create_state_store(
        config.state_backend,
        redis_url=config.redis_url,
        key_prefix=config.redis_key_prefix,
        ttl_seconds=config.redis_ttl_seconds,
    )
    _pubsub_service = PubSubService()
    _event_store = EventStore()
    logger.info("Aether server started (auth=%s, state=%s)", config.auth_enabled, config.state_backend)

    yield

    # Shutdown phase
    logger.info("Aether server shutting down")


def create_app(config: Optional[ServerConfig] = None) -> FastAPI:
    if config is None:
        config = ServerConfig()

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

    # Add authentication middleware if enabled
    if config.auth_enabled:
        from .auth import AuthMiddleware, AuthConfig
        auth_config = AuthConfig(
            enabled=True,
            secret=config.auth_secret,
            token_ttl=config.auth_token_ttl,
        )
        app.add_middleware(AuthMiddleware, config=auth_config)
        logger.info("Authentication middleware enabled")

    # Add rate limiting middleware if enabled
    if config.rate_limit_enabled:
        from .rate_limit import RateLimitMiddleware, RateLimitConfig
        rl_config = RateLimitConfig(
            enabled=True,
            requests_per_second=config.rate_limit_rps,
            burst=config.rate_limit_burst,
            default_limit=config.rate_limit_rps,
            default_burst=config.rate_limit_burst,
            per_endpoint=config.rate_limit_per_endpoint,
            endpoint_limits=config.rate_limit_endpoint_overrides,
        )
        app.add_middleware(RateLimitMiddleware, config=rl_config)
        logger.info("Rate limiting enabled (%.0f rps, burst=%d)", config.rate_limit_rps, config.rate_limit_burst)

    app.state.server_config = config

    @app.middleware("http")
    async def request_id_middleware(request: Request, call_next):
        import time as _time
        request_id = request.headers.get("X-Request-ID", str(uuid.uuid4()))
        trace_id = None
        start = _time.perf_counter()
        with trace_span(
            f"HTTP {request.method} {request.url.path}",
            attributes={"http.method": request.method, "http.url": str(request.url.path)},
        ):
            response: Response = await call_next(request)
            trace_id = get_trace_id_hex()
        response.headers["X-Request-ID"] = request_id
        if trace_id:
            response.headers["X-Trace-Id"] = trace_id

        # Record metrics if enabled
        metrics = getattr(app.state, "metrics", None)
        if metrics is not None:
            duration = _time.perf_counter() - start
            metrics.observe_request(
                method=request.method,
                path=str(request.url.path),
                status=response.status_code,
                duration=duration,
            )

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

    # Metrics endpoint (Prometheus text format)
    if config.metrics_enabled:
        @app.get("/metrics", include_in_schema=False)
        async def metrics_endpoint():
            metrics = getattr(app.state, "metrics", None)
            if metrics is None:
                return PlainTextResponse("# Metrics not enabled", status_code=503)
            return PlainTextResponse(
                content=metrics.collect(),
                media_type="text/plain; version=0.0.4; charset=utf-8",
            )

    try:
        from .api.graphql import graphql_app
        if graphql_app is not None:
            app.include_router(graphql_app, prefix="/graphql")
            logger.info("GraphQL API mounted at /graphql")
    except Exception as e:
        logger.warning("GraphQL not available: %s", e)

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
