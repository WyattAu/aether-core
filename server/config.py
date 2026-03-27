from dataclasses import dataclass, field


@dataclass
class ServerConfig:
    host: str = "0.0.0.0"
    port: int = 8080
    max_actors: int = 10000
    message_ttl_seconds: int = 300
    state_backend: str = "memory"
    version: str = "0.1.0"
