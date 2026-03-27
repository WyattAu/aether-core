import time

from fastapi import APIRouter, Request

from ..models import HealthResponse

router = APIRouter(tags=["health"])

_start_time = time.time()


def _get_actor_manager():
    from ..app import get_actor_manager
    return get_actor_manager()


def _get_message_router():
    from ..app import get_message_router
    return get_message_router()


def _get_shutdown_state(request: Request) -> str:
    """Get the shutdown state from the app, if available."""
    shutdown_mgr = getattr(request.app.state, "shutdown_manager", None)
    if shutdown_mgr is not None and not shutdown_mgr.is_running:
        return "draining"
    return "ok"


@router.get("/health", response_model=HealthResponse)
async def health(request: Request):
    mgr = _get_actor_manager()
    mr = _get_message_router()
    return HealthResponse(
        status=_get_shutdown_state(request),
        uptime=round(time.time() - _start_time, 2),
        actor_count=mgr.count(),
        message_count=mr.total_message_count(),
    )


@router.get("/health/ready", response_model=HealthResponse)
async def ready(request: Request):
    mgr = _get_actor_manager()
    mr = _get_message_router()
    return HealthResponse(
        status=_get_shutdown_state(request),
        uptime=round(time.time() - _start_time, 2),
        actor_count=mgr.count(),
        message_count=mr.total_message_count(),
    )


@router.get("/api/v1/info")
async def info(request: Request):
    mgr = _get_actor_manager()
    mr = _get_message_router()
    return {
        "version": "0.1.0",
        "status": _get_shutdown_state(request),
        "uptime": round(time.time() - _start_time, 2),
        "actor_count": mgr.count(),
        "message_count": mr.total_message_count(),
    }
