"""gRPC authentication interceptor for the Aether server.

Provides server-side interceptor that validates JWT tokens from gRPC call
metadata. Reuses the existing ``TokenService`` from ``auth.py`` so the
same token format works for both REST and gRPC.

Tokens can be passed via gRPC metadata using either:
- ``authorization: Bearer <token>`` header
- ``x-aether-token: <token>`` header

Usage::

    from server.grpc_auth import AuthServerInterceptor
    from server.auth import AuthConfig

    config = AuthConfig(enabled=True, secret="my-secret")
    interceptor = AuthServerInterceptor(config)
    server = grpc.server(executor, interceptors=[interceptor])
"""

import grpc
import logging

from .auth import AuthConfig, AuthError, TokenService

logger = logging.getLogger("aether-server.grpc.auth")

# gRPC method names for Health service that bypass authentication
_PUBLIC_METHODS = frozenset({
    "/aether.server.v1.HealthService/Health",
    "/aether.server.v1.HealthService/Ready",
    "/aether.server.v1.HealthService/Info",
})


class AuthServerInterceptor(grpc.ServerInterceptor):
    """Server-side gRPC interceptor for JWT authentication.

    When auth is disabled (``config.enabled=False``), all calls pass through.
    When enabled, calls must include a valid token in metadata unless the
    method is in the public methods list (Health service).

    The interceptor sets ``context.user`` (via ``context.set_peer_identity``)
    and ``context.auth_claims`` with the verified token claims.
    """

    def __init__(self, config: AuthConfig):
        self._config = config
        self._token_service = TokenService(config)

    def intercept_service(self, continuation, handler_call_details):
        """Intercept incoming gRPC calls to enforce authentication.

        Args:
            continuation: A function that takes a handler call details and
                returns the handler for the call.
            handler_call_details: ``grpc.HandlerCallDetails`` with method
                and metadata.

        Returns:
            A handler function (either the original or an error handler).
        """
        method = handler_call_details.method

        # Skip auth if disabled
        if not self._config.enabled:
            return continuation(handler_call_details)

        # Skip auth for public methods (health checks)
        if method in _PUBLIC_METHODS:
            return continuation(handler_call_details)

        # Extract token from metadata
        token = self._extract_token(handler_call_details.invocation_metadata)
        if not token:
            logger.debug("gRPC auth: no token for %s", method)
            return self._unauthenticated_handler("Authentication required")

        # Verify token
        try:
            claims = self._token_service.verify_token(token)
        except AuthError as e:
            logger.debug("gRPC auth: %s for %s", e.message, method)
            return self._unauthenticated_handler(e.message)

        # Token valid — proceed to handler
        return continuation(handler_call_details)

    @staticmethod
    def _extract_token(metadata) -> str:
        """Extract token from gRPC call metadata.

        Looks for ``authorization`` (Bearer scheme) or ``x-aether-token``.
        """
        if not metadata:
            return None

        for key, value in metadata:
            key_lower = key.lower()
            if key_lower == "authorization":
                if value.lower().startswith("bearer "):
                    return value[7:].strip()
            elif key_lower == "x-aether-token":
                return value.strip()

        return None

    @staticmethod
    def _unauthenticated_handler(message: str):
        """Return a handler that aborts with UNAUTHENTICATED."""
        def _handler(request_or_iterator, context):
            context.abort(grpc.StatusCode.UNAUTHENTICATED, message)

        return grpc.unary_unary_rpc_method_handler(_handler)
