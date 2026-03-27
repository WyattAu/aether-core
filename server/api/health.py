import time

from fastapi import APIRouter

from ..models import HealthResponse

router = APIRouter(tags=["health"])

_start_time = time.time()


def _get_actor_manager():
    from ..app import get_actor_manager
    return get_actor_manager()


def _get_message_router():
    from ..app import get_message_router
    return get_message_router()


@router.get("/health", response_model=HealthResponse)
async def health():
    mgr = _get_actor_manager()
    mr = _get_message_router()
    return HealthResponse(
        status="ok",
        uptime=round(time.time() - _start_time, 2),
        actor_count=mgr.count(),
        message_count=mr.total_message_count(),
    )


@router.get("/health/ready", response_model=HealthResponse)
async def ready():
    return await health()


@router.get("/api/v1/info")
async def info():
    mgr = _get_actor_manager()
    mr = _get_message_router()
    return {
        "version": "0.1.0",
        "uptime": round(time.time() - _start_time, 2),
        "actor_count": mgr.count(),
        "message_count": mr.total_message_count(),
    }
