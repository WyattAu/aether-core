from dataclasses import dataclass, field
from typing import Dict, Optional, Set, Tuple


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
    rate_limit_enabled: bool = False
    rate_limit_rps: float = 100.0
    rate_limit_burst: int = 200
    rate_limit_per_endpoint: bool = True
    rate_limit_endpoint_overrides: Dict[str, Tuple[float, int]] = field(default_factory=dict)
    drain_timeout_seconds: float = 30.0
    json_logging_enabled: bool = False
    log_level: str = "INFO"
    metrics_enabled: bool = False
