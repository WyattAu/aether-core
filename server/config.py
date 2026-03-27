from dataclasses import dataclass, field
from typing import Optional, Set


@dataclass
class ServerConfig:
    host: str = "0.0.0.0"
    port: int = 8080
    max_actors: int = 10000
    message_ttl_seconds: int = 300
    state_backend: str = "memory"
    version: str = "0.1.0"
    redis_url: Optional[str] = None
    redis_key_prefix: str = "aether:state:"
    redis_ttl_seconds: Optional[int] = None
    auth_enabled: bool = False
    auth_secret: str = "aether-default-secret-change-me"
    auth_token_ttl: int = 3600
