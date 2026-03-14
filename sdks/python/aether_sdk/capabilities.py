from enum import Flag, auto
from typing import Set


class Capability(Flag):
    """Actor capabilities."""
    NETWORK_OUTBOUND = auto()
    NETWORK_INBOUND = auto()
    STATE_READ = auto()
    STATE_WRITE = auto()
    FS_READ = auto()
    FS_WRITE = auto()
    ACTOR_MESSAGING = auto()
    LOG = auto()
    TIME = auto()
    RANDOM = auto()
    ENVIRONMENT = auto()
    HTTP_CLIENT = auto()
    HTTP_SERVER = auto()


class CapabilitySet:
    """Set of capabilities with helper methods."""
    
    def __init__(self, *capabilities: Capability):
        self._caps: Set[Capability] = set(capabilities)
    
    def add(self, cap: Capability) -> None:
        self._caps.add(cap)
    
    def has(self, cap: Capability) -> bool:
        return cap in self._caps
    
    def has_network(self) -> bool:
        return Capability.NETWORK_OUTBOUND in self._caps or Capability.NETWORK_INBOUND in self._caps
    
    def has_state(self) -> bool:
        return Capability.STATE_READ in self._caps or Capability.STATE_WRITE in self._caps
