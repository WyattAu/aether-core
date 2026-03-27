"""JWT authentication middleware for the Aether server.

Provides optional JWT-based authentication that can be enabled via
``ServerConfig.auth_enabled``. When disabled (default), all requests
pass through unauthenticated.

The middleware supports:
- Bearer token authentication via ``Authorization`` header
- Token-based authentication via ``X-Aether-Token`` header
- Configurable public routes (health, info)
- HMAC-SHA256 token signing with configurable secret
- Token expiration with configurable TTL

Usage::

    # In ServerConfig:
    #   auth_enabled = True
    #   auth_secret = "your-secret-key"
    #   auth_token_ttl = 3600  # seconds

    # Generating a token (e.g. in a CLI tool):
    #   python -c "
    #   import hmac, hashlib, json, time, base64
    #   payload = json.dumps({'sub': 'user1', 'exp': int(time.time()) + 3600})
    #   b64 = base64.urlsafe_b64encode(payload.encode()).rstrip(b'=').decode()
    #   sig = hmac.new(b'secret', b64.encode(), hashlib.sha256).hexdigest()
    #   print(f'{b64}.{sig}')
    #   "
"""

import base64
import hashlib
import hmac
import json
import logging
import time
from dataclasses import dataclass
from typing import Any, Dict, Optional, Set

from fastapi import Request, Response
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import JSONResponse

logger = logging.getLogger("aether-server.auth")


@dataclass
class AuthConfig:
    """Configuration for JWT authentication.

    Attributes:
        enabled: Whether authentication is required.
        secret: Secret key for signing tokens (min 16 chars).
        token_ttl: Token time-to-live in seconds (default: 3600).
        public_paths: Set of URL paths that bypass authentication.
        algorithm: HMAC algorithm (default: sha256).
    """
    enabled: bool = False
    secret: str = "aether-default-secret-change-me"
    token_ttl: int = 3600
    public_paths: Set[str] = None
    algorithm: str = "sha256"

    def __post_init__(self):
        if self.public_paths is None:
            self.public_paths = {
                "/health",
                "/health/ready",
                "/api/v1/info",
                "/docs",
                "/openapi.json",
                "/redoc",
            }
        if self.enabled and len(self.secret) < 16:
            logger.warning(
                "Auth secret is less than 16 characters. "
                "Set a stronger secret in production."
            )


class AuthError(Exception):
    """Raised when authentication fails."""

    def __init__(self, message: str, status_code: int = 401):
        self.message = message
        self.status_code = status_code
        super().__init__(message)


class TokenService:
    """HMAC-based token creation and verification.

    Uses a simple ``payload.signature`` format (not full JWT)
    for lightweight token management without external dependencies.

    Tokens are base64url-encoded JSON payloads signed with HMAC.
    """

    def __init__(self, config: AuthConfig):
        self._config = config

    def create_token(self, subject: str, extra_claims: Optional[Dict[str, Any]] = None,
                     ttl: Optional[int] = None) -> str:
        """Create a signed token.

        Args:
            subject: The subject (e.g. user ID or actor ID).
            extra_claims: Additional claims to include in the payload.
            ttl: Token TTL in seconds. Uses config default if ``None``.

        Returns:
            A signed token string.
        """
        now = int(time.time())
        payload: Dict[str, Any] = {
            "sub": subject,
            "iat": now,
            "exp": now + (ttl if ttl is not None else self._config.token_ttl),
        }
        if extra_claims:
            payload.update(extra_claims)

        payload_json = json.dumps(payload, separators=(",", ":"))
        payload_b64 = base64.urlsafe_b64encode(
            payload_json.encode()
        ).rstrip(b"=").decode()

        signature = hmac.new(
            self._config.secret.encode(),
            payload_b64.encode(),
            getattr(hashlib, self._config.algorithm),
        ).hexdigest()

        return f"{payload_b64}.{signature}"

    def verify_token(self, token: str) -> Dict[str, Any]:
        """Verify a token and return its claims.

        Args:
            token: The token string to verify.

        Returns:
            The token payload claims.

        Raises:
            AuthError: If the token is invalid, expired, or tampered.
        """
        if not token:
            raise AuthError("Missing authentication token")

        parts = token.split(".")
        if len(parts) != 2:
            raise AuthError("Invalid token format")

        payload_b64, signature = parts

        # Verify signature
        expected_sig = hmac.new(
            self._config.secret.encode(),
            payload_b64.encode(),
            getattr(hashlib, self._config.algorithm),
        ).hexdigest()

        if not hmac.compare_digest(signature, expected_sig):
            raise AuthError("Invalid token signature")

        # Decode payload
        # Restore base64 padding
        padding = 4 - len(payload_b64) % 4
        if padding != 4:
            payload_b64 += "=" * padding

        try:
            payload_json = base64.urlsafe_b64decode(payload_b64)
            claims = json.loads(payload_json)
        except Exception:
            raise AuthError("Invalid token payload")

        # Check expiration
        exp = claims.get("exp")
        if exp is not None and int(time.time()) > exp:
            raise AuthError("Token expired", status_code=401)

        return claims


class AuthMiddleware(BaseHTTPMiddleware):
    """FastAPI middleware for JWT authentication.

    When auth is disabled, all requests pass through.
    When enabled, requests must include a valid Bearer token or X-Aether-Token header,
    unless the path is in the public paths list.

    The authenticated subject is stored in ``request.state.auth_subject``.
    """

    def __init__(self, app, config: AuthConfig):
        super().__init__(app)
        self._config = config
        self._token_service = TokenService(config)

    def _is_public_path(self, path: str) -> bool:
        """Check if a path should bypass authentication."""
        # Exact match
        if path in self._config.public_paths:
            return True
        # Prefix match for WebSocket and static paths
        for public in self._config.public_paths:
            if public.endswith("/") and path.startswith(public):
                return True
        return False

    def _extract_token(self, request: Request) -> Optional[str]:
        """Extract token from Authorization or X-Aether-Token header."""
        auth_header = request.headers.get("Authorization", "")
        if auth_header.startswith("Bearer "):
            return auth_header[7:].strip()

        # Alternative header for non-browser clients
        token_header = request.headers.get("X-Aether-Token")
        if token_header:
            return token_header.strip()

        return None

    async def dispatch(self, request: Request, call_next):
        """Process request through authentication."""
        path = request.url.path

        # Skip auth if disabled
        if not self._config.enabled:
            return await call_next(request)

        # Skip auth for public paths
        if self._is_public_path(path):
            return await call_next(request)

        # Skip auth for OPTIONS (CORS preflight)
        if request.method == "OPTIONS":
            return await call_next(request)

        # Extract and verify token
        token = self._extract_token(request)
        if not token:
            return JSONResponse(
                status_code=401,
                content={"detail": "Authentication required"},
            )

        try:
            claims = self._token_service.verify_token(token)
            request.state.auth_subject = claims.get("sub")
            request.state.auth_claims = claims
        except AuthError as e:
            return JSONResponse(
                status_code=e.status_code,
                content={"detail": e.message},
            )

        return await call_next(request)
