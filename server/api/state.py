from fastapi import APIRouter, HTTPException
from typing import Dict, Any

from ..models import StateEntry, SetStateRequest

router = APIRouter(prefix="/api/v1/state", tags=["state"])


def _get_state_store():
    from ..app import get_state_store
    return get_state_store()


@router.get("/{actor_id}/{key:path}")
async def get_state(actor_id: str, key: str):
    store = _get_state_store()
    value = store.get(actor_id, key)
    if value is None:
        raise HTTPException(status_code=404, detail=f"State {key} for actor {actor_id} not found")
    return {"actor_id": actor_id, "key": key, "value": value}


@router.put("/{actor_id}/{key:path}", response_model=StateEntry)
async def set_state(actor_id: str, key: str, req: SetStateRequest):
    store = _get_state_store()
    try:
        return store.set(actor_id, key, req.value, expected_version=req.version)
    except ValueError as e:
        raise HTTPException(status_code=409, detail=str(e))


@router.delete("/{actor_id}/{key:path}", status_code=204)
async def delete_state(actor_id: str, key: str):
    store = _get_state_store()
    if not store.delete(actor_id, key):
        raise HTTPException(status_code=404, detail=f"State {key} for actor {actor_id} not found")


@router.get("/{actor_id}")
async def get_all_state(actor_id: str) -> Dict[str, Any]:
    store = _get_state_store()
    return {"actor_id": actor_id, "state": store.get_all(actor_id)}
