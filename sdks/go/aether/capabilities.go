package aether

// Capability represents a permission that an actor can have.
// Capabilities are granted at actor creation time and cannot be
// escalated during runtime.
type Capability int

const (
	// CapabilityNetworkOutbound allows outbound network connections.
	CapabilityNetworkOutbound Capability = iota
	// CapabilityNetworkInbound allows inbound network connections (server).
	CapabilityNetworkInbound
	// CapabilityStateRead allows reading from state storage.
	CapabilityStateRead
	// CapabilityStateWrite allows writing to state storage.
	CapabilityStateWrite
	// CapabilityFSRead allows filesystem read operations.
	CapabilityFSRead
	// CapabilityFSWrite allows filesystem write operations.
	CapabilityFSWrite
	// CapabilityActorMessaging allows sending messages to other actors.
	CapabilityActorMessaging
	// CapabilityLog allows writing to logs.
	CapabilityLog
	// CapabilityTime allows accessing system time.
	CapabilityTime
	// CapabilityRandom allows generating random numbers.
	CapabilityRandom
	// CapabilityEnvironment allows accessing environment variables.
	CapabilityEnvironment
	// CapabilityHTTPClient allows HTTP client operations.
	CapabilityHTTPClient
	// CapabilityHTTPServer allows HTTP server operations.
	CapabilityHTTPServer
	// CapabilityProcessSpawn allows spawning child processes.
	CapabilityProcessSpawn
)

// String returns the string representation of a capability.
func (c Capability) String() string {
	switch c {
	case CapabilityNetworkOutbound:
		return "NETWORK_OUTBOUND"
	case CapabilityNetworkInbound:
		return "NETWORK_INBOUND"
	case CapabilityStateRead:
		return "STATE_READ"
	case CapabilityStateWrite:
		return "STATE_WRITE"
	case CapabilityFSRead:
		return "FS_READ"
	case CapabilityFSWrite:
		return "FS_WRITE"
	case CapabilityActorMessaging:
		return "ACTOR_MESSAGING"
	case CapabilityLog:
		return "LOG"
	case CapabilityTime:
		return "TIME"
	case CapabilityRandom:
		return "RANDOM"
	case CapabilityEnvironment:
		return "ENVIRONMENT"
	case CapabilityHTTPClient:
		return "HTTP_CLIENT"
	case CapabilityHTTPServer:
		return "HTTP_SERVER"
	case CapabilityProcessSpawn:
		return "PROCESS_SPAWN"
	default:
		return "UNKNOWN"
	}
}

// CapabilitySet is a collection of capabilities with helper methods.
type CapabilitySet struct {
	caps map[Capability]bool
}

// NewCapabilitySet creates a new CapabilitySet with the given capabilities.
func NewCapabilitySet(capabilities ...Capability) *CapabilitySet {
	cs := &CapabilitySet{
		caps: make(map[Capability]bool),
	}
	for _, cap := range capabilities {
		cs.caps[cap] = true
	}
	return cs
}

// Add adds a capability to the set.
func (cs *CapabilitySet) Add(cap Capability) {
	cs.caps[cap] = true
}

// Has checks if a specific capability is in the set.
func (cs *CapabilitySet) Has(cap Capability) bool {
	return cs.caps[cap]
}

// HasNetwork returns true if any network capability is granted.
func (cs *CapabilitySet) HasNetwork() bool {
	return cs.caps[CapabilityNetworkOutbound] || cs.caps[CapabilityNetworkInbound]
}

// HasState returns true if any state capability is granted.
func (cs *CapabilitySet) HasState() bool {
	return cs.caps[CapabilityStateRead] || cs.caps[CapabilityStateWrite]
}

// HasFS returns true if any filesystem capability is granted.
func (cs *CapabilitySet) HasFS() bool {
	return cs.caps[CapabilityFSRead] || cs.caps[CapabilityFSWrite]
}

// HasHTTP returns true if any HTTP capability is granted.
func (cs *CapabilitySet) HasHTTP() bool {
	return cs.caps[CapabilityHTTPClient] || cs.caps[CapabilityHTTPServer]
}

// All returns all capabilities in the set.
func (cs *CapabilitySet) All() []Capability {
	result := make([]Capability, 0, len(cs.caps))
	for cap := range cs.caps {
		result = append(result, cap)
	}
	return result
}

// EmptyCapabilitySet returns an empty capability set.
func EmptyCapabilitySet() *CapabilitySet {
	return &CapabilitySet{caps: make(map[Capability]bool)}
}

// AllCapabilities returns a capability set with all capabilities.
func AllCapabilities() *CapabilitySet {
	return NewCapabilitySet(
		CapabilityNetworkOutbound,
		CapabilityNetworkInbound,
		CapabilityStateRead,
		CapabilityStateWrite,
		CapabilityFSRead,
		CapabilityFSWrite,
		CapabilityActorMessaging,
		CapabilityLog,
		CapabilityTime,
		CapabilityRandom,
		CapabilityEnvironment,
		CapabilityHTTPClient,
		CapabilityHTTPServer,
		CapabilityProcessSpawn,
	)
}
