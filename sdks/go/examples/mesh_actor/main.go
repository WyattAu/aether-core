// Package main demonstrates mesh communication between actors in Go.
// This example shows how actors can communicate across the Aether mesh network.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"math/rand"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

// MeshNode represents a node in the mesh network.
type MeshNode struct {
	ID       string            `json:"id"`
	Region   string            `json:"region"`
	Endpoint string            `json:"endpoint"`
	Status   string            `json:"status"`
	Metadata map[string]string `json:"metadata,omitempty"`
}

// MeshMessage represents a message sent across the mesh.
type MeshMessage struct {
	SourceNode string `json:"source_node"`
	TargetNode string `json:"target_node,omitempty"` // Empty = broadcast
	Content    string `json:"content"`
	Timestamp  string `json:"timestamp"`
	HopCount   int    `json:"hop_count"`
}

// MeshActor is an actor that participates in mesh communication.
type MeshActor struct {
	*aether.BaseActor
	nodeID      string
	region      string
	knownNodes  map[string]*MeshNode
	nodesMu     sync.RWMutex
	messageLog  []MeshMessage
	logMu       sync.Mutex
	isLeader    bool
	leaderID    string
}

// NewMeshActor creates a new MeshActor.
func NewMeshActor(region string) *MeshActor {
	nodeID := fmt.Sprintf("node-%s-%d", region, rand.Intn(10000))
	return &MeshActor{
		BaseActor:  aether.NewBaseActor("mesh-actor"),
		nodeID:     nodeID,
		region:     region,
		knownNodes: make(map[string]*MeshNode),
		messageLog: make([]MeshMessage, 0, 1000),
	}
}

// OnStart is called when the actor starts.
func (a *MeshActor) OnStart(ctx context.Context) error {
	log.Printf("[%s] Starting mesh actor in region: %s", a.nodeID, a.region)
	log.Printf("[%s] Node ID: %s", a.nodeID, a.nodeID)

	// Register self in known nodes
	a.knownNodes[a.nodeID] = &MeshNode{
		ID:       a.nodeID,
		Region:   a.region,
		Endpoint: fmt.Sprintf("localhost:%d", 4000+rand.Intn(1000)),
		Status:   "active",
		Metadata: map[string]string{
			"started_at": time.Now().UTC().Format(time.RFC3339),
		},
	}

	return nil
}

// OnStop is called when the actor stops.
func (a *MeshActor) OnStop(ctx context.Context) error {
	log.Printf("[%s] Mesh actor stopping", a.nodeID)
	a.nodesMu.RLock()
	nodeCount := len(a.knownNodes)
	a.nodesMu.RUnlock()
	log.Printf("[%s] Known nodes: %d, Messages processed: %d",
		a.nodeID, nodeCount, len(a.messageLog))
	return nil
}

// HandleMessage handles incoming messages.
func (a *MeshActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	switch msg.Type {
	case aether.MessageTypeRequest, aether.MessageTypeRPCRequest:
		return a.handleRequest(ctx, sender, msg)
	case aether.MessageTypeEvent:
		a.handleEvent(ctx, sender, msg)
		return nil, nil
	case aether.MessageTypeResponse, aether.MessageTypeRPCResponse:
		return a.handleResponse(ctx, sender, msg)
	default:
		return aether.NewResponse(msg, map[string]any{
			"error": "unsupported message type",
			"type":  string(msg.Type),
		}), nil
	}
}

func (a *MeshActor) handleRequest(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	payload, ok := msg.Payload.(map[string]any)
	if !ok {
		return aether.NewResponse(msg, map[string]any{
			"error": "invalid payload format",
		}), nil
	}

	action, _ := payload["action"].(string)
	switch action {
	case "ping":
		return a.handlePing(ctx, sender, payload)
	case "discover":
		return a.handleDiscover(ctx, sender, payload)
	case "broadcast":
		return a.handleBroadcast(ctx, sender, payload)
	case "direct_message":
		return a.handleDirectMessage(ctx, sender, payload)
	case "get_status":
		return a.handleGetStatus(ctx, sender)
	case "elect_leader":
		return a.handleLeaderElection(ctx, sender, payload)
	default:
		return aether.NewResponse(msg, map[string]any{
			"error":  fmt.Sprintf("unknown action: %s", action),
			"node":   a.nodeID,
			"region": a.region,
		}), nil
	}
}

func (a *MeshActor) handleEvent(ctx context.Context, sender string, msg *aether.Message) {
	payload, ok := msg.Payload.(map[string]any)
	if !ok {
		return
	}

	eventType, _ := payload["type"].(string)
	switch eventType {
	case "node_join":
		a.handleNodeJoin(ctx, sender, payload)
	case "node_leave":
		a.handleNodeLeave(ctx, sender, payload)
	case "mesh_update":
		a.handleMeshUpdate(ctx, sender, payload)
	}
}

func (a *MeshActor) handleResponse(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	// Handle responses from other nodes
	log.Printf("[%s] Received response from %s", a.nodeID, sender)
	return nil, nil
}

// handlePing responds to ping requests.
func (a *MeshActor) handlePing(ctx context.Context, sender string, payload map[string]any) (*aether.Message, error) {
	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":    "pong",
		"node_id":   a.nodeID,
		"region":    a.region,
		"timestamp": time.Now().UTC().Format(time.RFC3339),
		"status":    "healthy",
	}), nil
}

// handleDiscover returns known nodes in the mesh.
func (a *MeshActor) handleDiscover(ctx context.Context, sender string, payload map[string]any) (*aether.Message, error) {
	a.nodesMu.RLock()
	defer a.nodesMu.RUnlock()

	nodes := make([]MeshNode, 0, len(a.knownNodes))
	for _, node := range a.knownNodes {
		nodes = append(nodes, *node)
	}

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":      "discover_response",
		"node_id":     a.nodeID,
		"known_nodes": nodes,
		"count":       len(nodes),
	}), nil
}

// handleBroadcast processes broadcast messages.
func (a *MeshActor) handleBroadcast(ctx context.Context, sender string, payload map[string]any) (*aether.Message, error) {
	content, _ := payload["content"].(string)
	sourceNode, _ := payload["source_node"].(string)
	hopCount, _ := payload["hop_count"].(float64)

	// Log the message
	meshMsg := MeshMessage{
		SourceNode: sourceNode,
		Content:    content,
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		HopCount:   int(hopCount),
	}

	a.logMu.Lock()
	a.messageLog = append(a.messageLog, meshMsg)
	a.logMu.Unlock()

	log.Printf("[%s] Broadcast received from %s: %s (hops: %d)",
		a.nodeID, sourceNode, truncate(content, 30), int(hopCount))

	// In a real implementation, we would forward to other nodes
	// Here we just acknowledge receipt
	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":   "broadcast_ack",
		"node_id":  a.nodeID,
		"received": true,
	}), nil
}

// handleDirectMessage processes direct messages.
func (a *MeshActor) handleDirectMessage(ctx context.Context, sender string, payload map[string]any) (*aether.Message, error) {
	content, _ := payload["content"].(string)
	sourceNode, _ := payload["source_node"].(string)

	// Log the message
	meshMsg := MeshMessage{
		SourceNode: sourceNode,
		TargetNode: a.nodeID,
		Content:    content,
		Timestamp:  time.Now().UTC().Format(time.RFC3339),
		HopCount:   0,
	}

	a.logMu.Lock()
	a.messageLog = append(a.messageLog, meshMsg)
	a.logMu.Unlock()

	log.Printf("[%s] Direct message from %s: %s", a.nodeID, sourceNode, content)

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":    "direct_message_ack",
		"node_id":   a.nodeID,
		"received":  true,
		"timestamp": time.Now().UTC().Format(time.RFC3339),
	}), nil
}

// handleGetStatus returns the current status of this node.
func (a *MeshActor) handleGetStatus(ctx context.Context, sender string) (*aether.Message, error) {
	a.nodesMu.RLock()
	nodeCount := len(a.knownNodes)
	a.nodesMu.RUnlock()

	a.logMu.Lock()
	msgCount := len(a.messageLog)
	a.logMu.Unlock()

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":           "status",
		"node_id":          a.nodeID,
		"region":           a.region,
		"status":           "active",
		"known_nodes":      nodeCount,
		"messages_handled": msgCount,
		"is_leader":        a.isLeader,
		"leader_id":        a.leaderID,
		"uptime":           time.Since(a.getStartTime()).String(),
	}), nil
}

// handleLeaderElection handles leader election requests.
func (a *MeshActor) handleLeaderElection(ctx context.Context, sender string, payload map[string]any) (*aether.Message, error) {
	candidateID, _ := payload["candidate_id"].(string)

	// Simple leader election: highest node ID wins
	if candidateID > a.nodeID {
		a.leaderID = candidateID
		a.isLeader = false
		log.Printf("[%s] Acknowledging %s as leader", a.nodeID, candidateID)
	} else {
		a.leaderID = a.nodeID
		a.isLeader = true
		log.Printf("[%s] Claiming leadership (over %s)", a.nodeID, candidateID)
	}

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":     "election_vote",
		"voter_id":   a.nodeID,
		"leader_id":  a.leaderID,
		"is_leader":  a.isLeader,
		"timestamp":  time.Now().UTC().Format(time.RFC3339),
	}), nil
}

// handleNodeJoin handles node join events.
func (a *MeshActor) handleNodeJoin(ctx context.Context, sender string, payload map[string]any) {
	nodeData, _ := json.Marshal(payload["node"])
	var node MeshNode
	if err := json.Unmarshal(nodeData, &node); err != nil {
		return
	}

	a.nodesMu.Lock()
	a.knownNodes[node.ID] = &node
	a.nodesMu.Unlock()

	log.Printf("[%s] Node joined mesh: %s (region: %s)", a.nodeID, node.ID, node.Region)
}

// handleNodeLeave handles node leave events.
func (a *MeshActor) handleNodeLeave(ctx context.Context, sender string, payload map[string]any) {
	nodeID, _ := payload["node_id"].(string)

	a.nodesMu.Lock()
	delete(a.knownNodes, nodeID)
	a.nodesMu.Unlock()

	// If leader left, trigger re-election
	if nodeID == a.leaderID {
		a.leaderID = ""
		a.isLeader = false
		log.Printf("[%s] Leader %s left, re-election needed", a.nodeID, nodeID)
	}

	log.Printf("[%s] Node left mesh: %s", a.nodeID, nodeID)
}

// handleMeshUpdate handles mesh topology updates.
func (a *MeshActor) handleMeshUpdate(ctx context.Context, sender string, payload map[string]any) {
	nodesData, _ := payload["nodes"].([]any)

	a.nodesMu.Lock()
	for _, n := range nodesData {
		nodeData, _ := json.Marshal(n)
		var node MeshNode
		if err := json.Unmarshal(nodeData, &node); err != nil {
			continue
		}
		a.knownNodes[node.ID] = &node
	}
	a.nodesMu.Unlock()

	log.Printf("[%s] Mesh topology updated: %d nodes", a.nodeID, len(nodesData))
}

// getStartTime returns the start time from metadata.
func (a *MeshActor) getStartTime() time.Time {
	a.nodesMu.RLock()
	defer a.nodesMu.RUnlock()
	if node, ok := a.knownNodes[a.nodeID]; ok {
		if t, err := time.Parse(time.RFC3339, node.Metadata["started_at"]); err == nil {
			return t
		}
	}
	return time.Now()
}

// truncate truncates a string.
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

func main() {
	// Seed random for node ID generation
	rand.Seed(time.Now().UnixNano())

	// Get region from environment or use default
	region := os.Getenv("AETHER_REGION")
	if region == "" {
		region = "us-east-1"
	}

	// Create actor with mesh capabilities
	actor := NewMeshActor(region)
	actor.Require(
		aether.CapabilityNetworkOutbound,
		aether.CapabilityNetworkInbound,
		aether.CapabilityActorMessaging,
		aether.CapabilityLog,
		aether.CapabilityTime,
		aether.CapabilityRandom,
	)

	// Setup context with cancellation
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Handle shutdown signals
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		log.Println("Shutting down mesh actor...")
		actor.Stop()
		cancel()
	}()

	log.Printf("Starting mesh actor...")
	log.Printf("Node ID: %s", actor.nodeID)
	log.Printf("Region: %s", actor.region)
	log.Printf("Supported actions: ping, discover, broadcast, direct_message, get_status, elect_leader")

	// Run the actor
	if err := actor.Run(ctx); err != nil {
		if err != context.Canceled {
			log.Fatalf("Actor error: %v", err)
		}
	}

	log.Println("Mesh actor stopped")
}
