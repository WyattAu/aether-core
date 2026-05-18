import pytest
from aether_sdk.state import StateHandle


class TestStateHandle:
    @pytest.mark.asyncio
    async def test_get_set(self):
        state = StateHandle()
        await state.set("key1", b"value1")
        result = await state.get("key1")
        assert result == b"value1"

    @pytest.mark.asyncio
    async def test_get_nonexistent(self):
        state = StateHandle()
        result = await state.get("nonexistent")
        assert result is None

    @pytest.mark.asyncio
    async def test_delete(self):
        state = StateHandle()
        await state.set("key1", b"value1")
        await state.delete("key1")
        result = await state.get("key1")
        assert result is None

    @pytest.mark.asyncio
    async def test_delete_nonexistent(self):
        state = StateHandle()
        await state.delete("nonexistent")

    @pytest.mark.asyncio
    async def test_get_json(self):
        state = StateHandle()
        await state.set_json("json_key", {"name": "test", "value": 42})
        result = await state.get_json("json_key")
        assert result == {"name": "test", "value": 42}

    @pytest.mark.asyncio
    async def test_get_json_nonexistent(self):
        state = StateHandle()
        result = await state.get_json("nonexistent")
        assert result is None

    @pytest.mark.asyncio
    async def test_set_json(self):
        state = StateHandle()
        await state.set_json("config", {"enabled": True, "count": 10})

        raw = await state.get("config")
        assert raw is not None
        assert b'"enabled": true' in raw or b'"enabled":True' in raw

    @pytest.mark.asyncio
    async def test_overwrite(self):
        state = StateHandle()
        await state.set("key", b"first")
        await state.set("key", b"second")
        result = await state.get("key")
        assert result == b"second"
