"""
AI-Powered Actor Example

Demonstrates integration with AI services for text generation.
"""

import asyncio
import logging
import time
from typing import Any

from aether_sdk import Actor, Message, MessageType

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class AIRequest:
    """Request to the AI actor."""

    def __init__(
        self,
        prompt: str,
        model: str = "aether-1.0",
        max_tokens: int = 256,
        temperature: float = 0.7,
    ):
        self.prompt = prompt
        self.model = model
        self.max_tokens = max_tokens
        self.temperature = temperature


class AIResponse:
    """Response from the AI actor."""

    def __init__(self, text: str, model: str, tokens_used: int, processed_at: str):
        self.text = text
        self.model = model
        self.tokens_used = tokens_used
        self.processed_at = processed_at

    def to_dict(self) -> dict:
        return {
            "text": self.text,
            "model": self.model,
            "tokens_used": self.tokens_used,
            "processed_at": self.processed_at,
        }


class AIActor(Actor):
    """An actor that processes AI requests."""

    def __init__(self):
        super().__init__("ai-actor")
        self.default_model = "aether-1.0"
        self.request_count = 0
        self.require("NETWORK_OUTBOUND", "ACTOR_MESSAGING", "LOG", "TIME", "RANDOM")

    async def on_start(self) -> None:
        logger.info(f"[{self.name}] AI Actor starting with model: {self.default_model}")
        logger.info(
            f"[{self.name}] Capabilities: AI inference, text generation, embeddings"
        )

    async def on_stop(self) -> None:
        logger.info(
            f"[{self.name}] AI Actor stopping. Total requests: {self.request_count}"
        )

    async def handle_message(self, sender: str, message: Message) -> Message | None:
        if message.type in (MessageType.REQUEST, MessageType.RPC_REQUEST):
            return await self._handle_request(sender, message)
        elif message.type == MessageType.EVENT:
            return await self._handle_event(sender, message)

        return Message.response({"error": "unsupported message type"})

    async def _handle_request(self, sender: str, message: Message) -> Message:
        payload = message.payload
        request = self._parse_request(payload)

        if not request.prompt:
            return Message.response({"error": "prompt is required"})

        # Set defaults
        if not request.model:
            request.model = self.default_model
        if not request.max_tokens:
            request.max_tokens = 256

        self.request_count += 1

        # Process the AI request
        response = await self._process_ai_request(request)

        if response is None:
            return Message.response({"error": "failed to process request"})

        return Message.response(
            {
                "request": {
                    "prompt": request.prompt,
                    "model": request.model,
                    "max_tokens": request.max_tokens,
                },
                "response": response.to_dict(),
                "sender": sender,
            }
        )

    async def _handle_event(self, sender: str, message: Message) -> Message | None:
        payload = message.payload
        if isinstance(payload, dict):
            event_type = payload.get("type", "")
            logger.info(f"[{self.name}] Received {event_type} event from {sender}")
        return None

    def _parse_request(self, payload: Any) -> AIRequest:
        """Parse request from various payload formats."""
        if isinstance(payload, str):
            return AIRequest(prompt=payload)

        if isinstance(payload, dict):
            return AIRequest(
                prompt=payload.get("prompt", ""),
                model=payload.get("model", self.default_model),
                max_tokens=payload.get("max_tokens", 256),
                temperature=payload.get("temperature", 0.7),
            )

        return AIRequest(prompt="")

    async def _process_ai_request(self, request: AIRequest) -> AIResponse | None:
        """Process an AI request (simulated for demo)."""
        # Simulate AI processing time
        processing_time = min(len(request.prompt) * 0.001, 2.0)
        processing_time = max(processing_time, 0.1)
        await asyncio.sleep(processing_time)

        # Generate simulated response based on prompt content
        prompt_lower = request.prompt.lower()

        if "summarize" in prompt_lower:
            text = f"[AI Summary] Processed: {request.prompt[:50]}..."
        elif "translate" in prompt_lower:
            text = f"[AI Translation] Would translate: {request.prompt[:50]}..."
        elif "analyze" in prompt_lower:
            text = f"[AI Analysis] Analyzed input with {len(request.prompt)} characters"
        elif "generate" in prompt_lower:
            text = f"[AI Generated] Creative output based on: {request.prompt[:50]}..."
        else:
            text = f"[AI Response] Processed your request: {request.prompt[:100]}..."

        tokens_used = len(request.prompt) // 4 + len(text) // 4

        return AIResponse(
            text=text,
            model=request.model,
            tokens_used=tokens_used,
            processed_at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        )


async def main():
    """Run the AI actor."""
    actor = AIActor()

    logger.info(f"Starting {actor.name}...")
    logger.info("Supported operations: generate, summarize, translate, analyze")
    logger.info(f"Default model: {actor.default_model}")

    try:
        await actor.start()
        await actor.run()
    except asyncio.CancelledError:
        logger.info("Actor cancelled")
    finally:
        await actor.stop()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Shutting down...")
