from fastapi import APIRouter, HTTPException, Request
from typing import List, Optional, Dict, Any

from ..models import (
    PublishRequest,
    SubscribeRequest,
    PubSubMessage,
    EventRecord,
    AppendEventRequest,
    PublishResponse,
    SubscribeResponse,
)

router = APIRouter(prefix="/api/v1/events", tags=["events"])


def _get_pubsub(request: Request = None):
    """Get pubsub service from app state (preferred) or module global (fallback).

    Using app.state ensures each server instance gets its own pubsub,
    which is critical for multi-node integration tests where multiple
    FastAPI apps share the same Python process.
    """
    if request is not None:
        ps = getattr(request.app.state, "pubsub_service", None)
        if ps is not None:
            return ps
    from ..app import get_pubsub_service
    return get_pubsub_service()


def _get_event_store():
    from ..app import get_event_store
    return get_event_store()


@router.post("/publish", status_code=202, response_model=PublishResponse)
async def publish(req: PublishRequest, request: Request):
    pubsub = _get_pubsub(request)
    count = pubsub.publish(topic=req.topic, payload=req.payload, headers=req.headers)
    return PublishResponse(topic=req.topic, subscriber_count=count)


@router.post("/subscribe", status_code=201, response_model=SubscribeResponse)
async def subscribe(req: SubscribeRequest, request: Request):
    pubsub = _get_pubsub(request)
    sub_id = pubsub.subscribe(
        topic=req.topic,
        subscriber_id=req.subscriber_id,
        filter=req.filter,
    )
    return SubscribeResponse(subscription_id=sub_id, topic=req.topic)


@router.delete("/subscribe/{sub_id}", status_code=204)
async def unsubscribe(sub_id: str, request: Request):
    pubsub = _get_pubsub(request)
    if not pubsub.unsubscribe(sub_id):
        raise HTTPException(status_code=404, detail=f"Subscription {sub_id} not found")


@router.get("/topics")
async def list_topics(request: Request) -> List[str]:
    pubsub = _get_pubsub(request)
    return pubsub.list_topics()


@router.get("/topics/{topic}/subscribers")
async def list_subscribers(topic: str, request: Request) -> List[str]:
    pubsub = _get_pubsub(request)
    return pubsub.list_subscribers(topic)


@router.get("/topics/{topic}/history", response_model=List[PubSubMessage])
async def topic_history(topic: str, request: Request):
    pubsub = _get_pubsub(request)
    return pubsub.get_history(topic)


@router.post("/append", response_model=EventRecord, status_code=201)
async def append_event(req: AppendEventRequest):
    store = _get_event_store()
    try:
        return store.append(
            aggregate_id=req.aggregate_id,
            event_type=req.event_type,
            data=req.data,
            expected_version=req.expected_version,
        )
    except ValueError as e:
        raise HTTPException(status_code=409, detail=str(e))


@router.get("/{aggregate_id}", response_model=List[EventRecord])
async def get_events(aggregate_id: str):
    store = _get_event_store()
    return store.get_events(aggregate_id)
