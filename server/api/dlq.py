import logging
from typing import Optional

from fastapi import APIRouter, HTTPException, Request

logger = logging.getLogger("aether-server.dlq")

router = APIRouter(tags=["dlq"])


def _get_dlq(request: Request):
    return getattr(request.app.state, "dlq", None)


def _get_message_router(request: Request):
    msg_router = getattr(request.app.state, "cluster_router", None)
    if msg_router is None:
        msg_router = getattr(request.app.state, "message_router", None)
    if msg_router is None:
        from ..app import get_message_router
        msg_router = get_message_router()
    return msg_router


@router.get("/stats")
async def dlq_stats(request: Request):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")
    return dlq.get_stats()


@router.get("/messages")
async def list_messages(
    request: Request,
    actor_id: Optional[str] = None,
    source_actor: Optional[str] = None,
    message_type: Optional[str] = None,
    limit: int = 100,
    offset: int = 0,
):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")

    entries = dlq.list_messages(
        actor_id=actor_id,
        source_actor=source_actor,
        message_type=message_type,
        limit=min(limit, 1000),
        offset=offset,
    )
    return {
        "messages": [e.to_dict() for e in entries],
        "total": dlq.size,
    }


@router.get("/messages/{message_id}")
async def get_message(request: Request, message_id: str):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")

    entry = dlq.get(message_id)
    if entry is None:
        raise HTTPException(status_code=404, detail=f"Message {message_id} not found in DLQ")
    return entry.to_dict()


@router.post("/messages/{message_id}/replay")
async def replay_message(request: Request, message_id: str):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")

    envelope = dlq.replay(message_id)
    if envelope is None:
        raise HTTPException(status_code=404, detail=f"Message {message_id} not found in DLQ")

    msg_router = _get_message_router(request)
    receipt = await msg_router.route(envelope)
    return {
        "message_id": receipt.message_id,
        "status": receipt.status,
        "correlation_id": receipt.correlation_id,
    }


@router.post("/messages/replay-all")
async def replay_all(request: Request):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")

    messages = dlq.replay_all()
    msg_router = _get_message_router(request)
    results = []
    for envelope in messages:
        receipt = await msg_router.route(envelope)
        results.append({
            "message_id": receipt.message_id,
            "status": receipt.status,
        })

    return {
        "replayed": len(results),
        "results": results,
    }


@router.delete("/messages/{message_id}")
async def delete_message(request: Request, message_id: str):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")

    removed = dlq.remove(message_id)
    if not removed:
        raise HTTPException(status_code=404, detail=f"Message {message_id} not found in DLQ")

    return {"removed": True, "message_id": message_id}


@router.delete("/messages")
async def purge_messages(request: Request):
    dlq = _get_dlq(request)
    if dlq is None:
        raise HTTPException(status_code=503, detail="DLQ not enabled")

    count = dlq.purge()
    return {"purged": count}
