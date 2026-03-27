package aether

import "testing"

func TestCapability_String(t *testing.T) {
	tests := []struct {
		cap      Capability
		expected string
	}{
		{CapabilityNetworkOutbound, "NETWORK_OUTBOUND"},
		{CapabilityNetworkInbound, "NETWORK_INBOUND"},
		{CapabilityStateRead, "STATE_READ"},
		{CapabilityStateWrite, "STATE_WRITE"},
		{CapabilityFSRead, "FS_READ"},
		{CapabilityFSWrite, "FS_WRITE"},
		{CapabilityActorMessaging, "ACTOR_MESSAGING"},
		{CapabilityLog, "LOG"},
		{CapabilityTime, "TIME"},
		{CapabilityRandom, "RANDOM"},
		{CapabilityEnvironment, "ENVIRONMENT"},
		{CapabilityHTTPClient, "HTTP_CLIENT"},
		{CapabilityHTTPServer, "HTTP_SERVER"},
		{CapabilityProcessSpawn, "PROCESS_SPAWN"},
		{Capability(999), "UNKNOWN"},
		{Capability(-1), "UNKNOWN"},
	}
	for _, tt := range tests {
		if got := tt.cap.String(); got != tt.expected {
			t.Errorf("Capability(%d).String() = %q, want %q", tt.cap, got, tt.expected)
		}
	}
}

func TestNewCapabilitySet(t *testing.T) {
	tests := []struct {
		name         string
		caps         []Capability
		checkHas     Capability
		checkHasBool bool
	}{
		{"empty", nil, CapabilityLog, false},
		{"single", []Capability{CapabilityLog}, CapabilityLog, true},
		{"multiple", []Capability{CapabilityNetworkOutbound, CapabilityStateRead}, CapabilityNetworkOutbound, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cs := NewCapabilitySet(tt.caps...)
			if cs.Has(tt.checkHas) != tt.checkHasBool {
				t.Errorf("Has(%v) = %v, want %v", tt.checkHas, cs.Has(tt.checkHas), tt.checkHasBool)
			}
		})
	}
}

func TestCapabilitySet_Add(t *testing.T) {
	cs := NewCapabilitySet()
	cs.Add(CapabilityLog)
	if !cs.Has(CapabilityLog) {
		t.Error("expected LOG capability after Add")
	}
	cs.Add(CapabilityLog)
	if !cs.Has(CapabilityLog) {
		t.Error("re-adding should keep capability")
	}
}

func TestCapabilitySet_Has(t *testing.T) {
	cs := NewCapabilitySet(CapabilityNetworkOutbound, CapabilityLog)

	if !cs.Has(CapabilityNetworkOutbound) {
		t.Error("expected NETWORK_OUTBOUND")
	}
	if !cs.Has(CapabilityLog) {
		t.Error("expected LOG")
	}
	if cs.Has(CapabilityFSRead) {
		t.Error("did not expect FS_READ")
	}
}

func TestCapabilitySet_HasNetwork(t *testing.T) {
	tests := []struct {
		name string
		caps []Capability
		want bool
	}{
		{"outbound only", []Capability{CapabilityNetworkOutbound}, true},
		{"inbound only", []Capability{CapabilityNetworkInbound}, true},
		{"both", []Capability{CapabilityNetworkOutbound, CapabilityNetworkInbound}, true},
		{"neither", []Capability{CapabilityLog}, false},
		{"empty", nil, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cs := NewCapabilitySet(tt.caps...)
			if got := cs.HasNetwork(); got != tt.want {
				t.Errorf("HasNetwork() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestCapabilitySet_HasState(t *testing.T) {
	tests := []struct {
		name string
		caps []Capability
		want bool
	}{
		{"read only", []Capability{CapabilityStateRead}, true},
		{"write only", []Capability{CapabilityStateWrite}, true},
		{"both", []Capability{CapabilityStateRead, CapabilityStateWrite}, true},
		{"neither", []Capability{CapabilityLog}, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cs := NewCapabilitySet(tt.caps...)
			if got := cs.HasState(); got != tt.want {
				t.Errorf("HasState() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestCapabilitySet_HasFS(t *testing.T) {
	tests := []struct {
		name string
		caps []Capability
		want bool
	}{
		{"read only", []Capability{CapabilityFSRead}, true},
		{"write only", []Capability{CapabilityFSWrite}, true},
		{"neither", []Capability{CapabilityLog}, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cs := NewCapabilitySet(tt.caps...)
			if got := cs.HasFS(); got != tt.want {
				t.Errorf("HasFS() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestCapabilitySet_HasHTTP(t *testing.T) {
	tests := []struct {
		name string
		caps []Capability
		want bool
	}{
		{"client only", []Capability{CapabilityHTTPClient}, true},
		{"server only", []Capability{CapabilityHTTPServer}, true},
		{"neither", []Capability{CapabilityLog}, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cs := NewCapabilitySet(tt.caps...)
			if got := cs.HasHTTP(); got != tt.want {
				t.Errorf("HasHTTP() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestCapabilitySet_All(t *testing.T) {
	caps := []Capability{CapabilityLog, CapabilityTime, CapabilityRandom}
	cs := NewCapabilitySet(caps...)

	all := cs.All()
	if len(all) != len(caps) {
		t.Errorf("expected %d capabilities, got %d", len(caps), len(all))
	}

	found := make(map[Capability]bool)
	for _, c := range all {
		found[c] = true
	}
	for _, c := range caps {
		if !found[c] {
			t.Errorf("expected capability %v in All()", c)
		}
	}
}

func TestEmptyCapabilitySet(t *testing.T) {
	cs := EmptyCapabilitySet()
	if cs.Has(CapabilityLog) {
		t.Error("empty set should not have any capabilities")
	}
	if cs.HasNetwork() {
		t.Error("empty set should not have network")
	}
	all := cs.All()
	if len(all) != 0 {
		t.Errorf("expected empty All(), got %d", len(all))
	}
}

func TestAllCapabilities(t *testing.T) {
	cs := AllCapabilities()
	expectedCount := 14
	all := cs.All()
	if len(all) != expectedCount {
		t.Errorf("expected %d capabilities, got %d", expectedCount, len(all))
	}
	if !cs.Has(CapabilityNetworkOutbound) {
		t.Error("expected NETWORK_OUTBOUND in AllCapabilities")
	}
	if !cs.Has(CapabilityProcessSpawn) {
		t.Error("expected PROCESS_SPAWN in AllCapabilities")
	}
}

func TestCapabilitySet_All_HasDuplicates(t *testing.T) {
	cs := NewCapabilitySet(CapabilityLog, CapabilityLog)
	all := cs.All()
	if len(all) != 1 {
		t.Errorf("expected 1 unique capability, got %d", len(all))
	}
}

func TestCapabilitySet_NilMap(t *testing.T) {
	cs := &CapabilitySet{caps: make(map[Capability]bool)}
	if cs.Has(CapabilityLog) {
		t.Error("empty map should not have capabilities")
	}
}
