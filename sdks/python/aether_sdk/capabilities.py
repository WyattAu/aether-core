"""Capability system for Aether actors.

This module defines the capability flags that control what resources
and operations an actor can access. Capabilities are granted at actor
creation time and cannot be escalated during runtime.

Example:
    >>> from aether_sdk import Capability, CapabilitySet
    >>> caps = CapabilitySet(Capability.NETWORK_OUTBOUND, Capability.STATE_READ)
    >>> caps.has(Capability.NETWORK_OUTBOUND)
    True
    >>> caps.has_network()
    True
"""

from enum import Flag, auto
from typing import Set


class Capability(Flag):
    """Capability flags that define what an actor can do.
    
    Capabilities are granted to actors at creation time and define
    the boundaries of what operations they can perform. This provides
    a fine-grained security model for the actor system.
    
    Attributes:
        NETWORK_OUTBOUND: Allow outbound network connections.
        NETWORK_INBOUND: Allow inbound network connections (server).
        STATE_READ: Allow reading from state storage.
        STATE_WRITE: Allow writing to state storage.
        FS_READ: Allow filesystem read operations.
        FS_WRITE: Allow filesystem write operations.
        ACTOR_MESSAGING: Allow sending messages to other actors.
        LOG: Allow writing to logs.
        TIME: Allow accessing system time.
        RANDOM: Allow generating random numbers.
        ENVIRONMENT: Allow accessing environment variables.
        HTTP_CLIENT: Allow HTTP client operations.
        HTTP_SERVER: Allow HTTP server operations.
    """
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
    """A set of capabilities with helper methods for checking permissions.
    
    CapabilitySet provides a convenient way to manage and check multiple
    capabilities at once. It supports initialization with capabilities
    and provides helper methods for common capability groupings.
    
    Attributes:
        _caps: Internal set of granted capabilities.
    
    Example:
        >>> caps = CapabilitySet(Capability.NETWORK_OUTBOUND)
        >>> caps.has(Capability.NETWORK_OUTBOUND)
        True
        >>> caps.add(Capability.STATE_READ)
        >>> caps.has_state()
        True
    """
    
    def __init__(self, *capabilities: Capability):
        """Initialize a CapabilitySet with optional initial capabilities.
        
        Args:
            *capabilities: Zero or more Capability flags to initialize with.
        """
        self._caps: Set[Capability] = set(capabilities)
    
    def add(self, cap: Capability) -> None:
        """Add a capability to the set.
        
        Args:
            cap: The Capability to add.
        """
        self._caps.add(cap)
    
    def has(self, cap: Capability) -> bool:
        """Check if a specific capability is granted.
        
        Args:
            cap: The Capability to check for.
        
        Returns:
            True if the capability is in the set, False otherwise.
        """
        return cap in self._caps
    
    def has_network(self) -> bool:
        """Check if any network capability is granted.
        
        Returns:
            True if NETWORK_OUTBOUND or NETWORK_INBOUND is granted.
        """
        return Capability.NETWORK_OUTBOUND in self._caps or Capability.NETWORK_INBOUND in self._caps
    
    def has_state(self) -> bool:
        """Check if any state capability is granted.
        
        Returns:
            True if STATE_READ or STATE_WRITE is granted.
        """
        return Capability.STATE_READ in self._caps or Capability.STATE_WRITE in self._caps
