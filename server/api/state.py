from fastapi import APIRouter, HTTPException
from typing import Dict, Any

from ..models import StateEntry, SetStateRequest, GetStateResponse, GetAllStateResponse
from ..tracing import trace_span

router = APIRouter(prefix="/api/v1/state", tags=["state"])


def _get_state_store():
    from ..app import get_state_store
    return get_state_store()


@router.get("/{actor_id}/{key:path}", response_model=GetStateResponse)
async def get_state(actor_id: str, key: str):
    with trace_span("state.get", {"actor.id": actor_id, "state.key": key}):
        store = _get_state_store()
        value = store.get(actor_id, key)
        if value is None:
            raise HTTPException(status_code=404, detail=f"State {key} for actor {actor_id} not found")
        return GetStateResponse(actor_id=actor_id, key=key, value=value)


@router.put("/{actor_id}/{key:path}", response_model=StateEntry)
async def set_state(actor_id: str, key: str, req: SetStateRequest):
    with trace_span("state.set", {"actor.id": actor_id, "state.key": key}):
        store = _get_state_store()
        try:
            return store.set(actor_id, key, req.value, expected_version=req.version)
        except ValueError as e:
            raise HTTPException(status_code=409, detail=str(e))


@router.delete("/{actor_id}/{key:path}", status_code=204)
async def delete_state(actor_id: str, key: str):
    with trace_span("state.delete", {"actor.id": actor_id, "state.key": key}):
        store = _get_state_store()
        if not store.delete(actor_id, key):
            raise HTTPException(status_code=404, detail=f"State {key} for actor {actor_id} not found")


@router.get("/{actor_id}", response_model=GetAllStateResponse)
async def get_all_state(actor_id: str):
    store = _get_state_store()
    return GetAllStateResponse(actor_id=actor_id, state=store.get_all(actor_id))
