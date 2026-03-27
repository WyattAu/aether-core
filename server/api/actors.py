from fastapi import APIRouter, HTTPException
from typing import Optional, List

from ..models import (
    ActorRegistration,
    ActorInfo,
    MessageEnvelope,
    DeliveryReceipt,
)

router = APIRouter(prefix="/api/v1/actors", tags=["actors"])


def _get_actor_manager():
    from ..app import get_actor_manager
    return get_actor_manager()


def _get_message_router():
    from ..app import get_message_router
    return get_message_router()


@router.post("", response_model=ActorInfo, status_code=201)
async def register_actor(reg: ActorRegistration):
    mgr = _get_actor_manager()
    try:
        return mgr.register(
            actor_id=reg.actor_id,
            actor_type=reg.actor_type,
            capabilities=reg.capabilities,
            metadata=reg.metadata,
        )
    except ValueError as e:
        raise HTTPException(status_code=409, detail=str(e))
    except RuntimeError as e:
        raise HTTPException(status_code=507, detail=str(e))


@router.delete("/{actor_id}", status_code=204)
async def unregister_actor(actor_id: str):
    mgr = _get_actor_manager()
    if not mgr.unregister(actor_id):
        raise HTTPException(status_code=404, detail=f"Actor {actor_id} not found")


@router.get("/{actor_id}", response_model=ActorInfo)
async def get_actor(actor_id: str):
    mgr = _get_actor_manager()
    actor = mgr.get_actor(actor_id)
    if actor is None:
        raise HTTPException(status_code=404, detail=f"Actor {actor_id} not found")
    return actor


@router.get("", response_model=List[ActorInfo])
async def list_actors(
    type: Optional[str] = None,
    status: Optional[str] = None,
):
    mgr = _get_actor_manager()
    return mgr.list_actors(actor_type=type, status=status)


@router.post("/{actor_id}/messages", response_model=DeliveryReceipt, status_code=202)
async def send_message(actor_id: str, envelope: MessageEnvelope):
    envelope.target_actor = actor_id
    router = _get_message_router()
    receipt = await router.route(envelope)
    return receipt


@router.get("/{actor_id}/messages", response_model=List[MessageEnvelope])
async def get_pending_messages(actor_id: str):
    router = _get_message_router()
    return router.get_pending_messages(actor_id)


@router.post("/{actor_id}/heartbeat", status_code=204)
async def heartbeat(actor_id: str):
    mgr = _get_actor_manager()
    if not mgr.update_heartbeat(actor_id):
        raise HTTPException(status_code=404, detail=f"Actor {actor_id} not found")
