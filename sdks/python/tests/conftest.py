import importlib.util
import os
import sys

_SERVER_ROOT = os.path.realpath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "server")
)
if _SERVER_ROOT not in sys.path:
    sys.path.insert(0, _SERVER_ROOT)

_spec = importlib.util.spec_from_file_location(
    "server",
    os.path.join(_SERVER_ROOT, "__init__.py"),
    submodule_search_locations=[_SERVER_ROOT],
)
_server_mod = importlib.util.module_from_spec(_spec)
sys.modules["server"] = _server_mod
_spec.loader.exec_module(_server_mod)

import pytest  # noqa: E402
import server.app as _app_mod  # noqa: E402
from server.actor_manager import ActorManager  # noqa: E402
from server.app import create_app  # noqa: E402
from server.config import ServerConfig  # noqa: E402
from server.event_store import EventStore  # noqa: E402
from server.message_router import MessageRouter  # noqa: E402
from server.pubsub_service import PubSubService  # noqa: E402
from server.state_store import MemoryStateStore  # noqa: E402


@pytest.fixture
def app():
    """Create a fresh FastAPI app with initialized state for each test."""
    _app_mod._actor_manager = ActorManager(ServerConfig())
    _app_mod._message_router = MessageRouter()
    _app_mod._state_store = MemoryStateStore()
    _app_mod._pubsub_service = PubSubService()
    _app_mod._event_store = EventStore()
    return create_app()


@pytest.fixture
async def client(app):
    """Create an AetherClient connected to the server via ASGI transport."""
    import httpx

    from aether_sdk.client import AetherClient

    transport = httpx.ASGITransport(app=app)
    http_client = httpx.AsyncClient(
        transport=transport,
        base_url="http://test",
    )
    aether = AetherClient(http_client=http_client, actor_id="test-sender")
    await aether.connect()
    yield aether
    await aether.close()
