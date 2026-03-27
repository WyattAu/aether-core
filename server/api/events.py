from fastapi import APIRouter, HTTPException
from typing import List, Optional, Dict, Any

from ..models import (
    PublishRequest,
    SubscribeRequest,
    PubSubMessage,
    EventRecord,
    AppendEventRequest,
)

router = APIRouter(prefix="/api/v1/events", tags=["events"])


def _get_pubsub():
    from ..app import get_pubsub_service
    return get_pubsub_service()


def _get_event_store():
    from ..app import get_event_store
    return get_event_store()


@router.post("/publish", status_code=202)
async def publish(req: PublishRequest):
    pubsub = _get_pubsub()
    count = pubsub.publish(topic=req.topic, payload=req.payload, headers=req.headers)
    return {"topic": req.topic, "subscriber_count": count}


@router.post("/subscribe", status_code=201)
async def subscribe(req: SubscribeRequest):
    pubsub = _get_pubsub()
    sub_id = pubsub.subscribe(
        topic=req.topic,
        subscriber_id=req.subscriber_id,
        filter=req.filter,
    )
    return {"subscription_id": sub_id, "topic": req.topic}


@router.delete("/subscribe/{sub_id}", status_code=204)
async def unsubscribe(sub_id: str):
    pubsub = _get_pubsub()
    if not pubsub.unsubscribe(sub_id):
        raise HTTPException(status_code=404, detail=f"Subscription {sub_id} not found")


@router.get("/topics")
async def list_topics() -> List[str]:
    pubsub = _get_pubsub()
    return pubsub.list_topics()


@router.get("/topics/{topic}/subscribers")
async def list_subscribers(topic: str) -> List[str]:
    pubsub = _get_pubsub()
    return pubsub.list_subscribers(topic)


@router.get("/topics/{topic}/history", response_model=List[PubSubMessage])
async def topic_history(topic: str):
    pubsub = _get_pubsub()
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
