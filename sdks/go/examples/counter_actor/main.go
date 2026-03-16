// Package main demonstrates a stateful counter actor in Go.
// This example shows how to use persistent state that survives actor restarts.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

// CounterState represents the persistent state of the counter actor.
type CounterState struct {
	Value    int64  `json:"value"`
	LastPush string `json:"last_push,omitempty"`
}

// CounterActor is a stateful actor that maintains a persistent counter.
type CounterActor struct {
	*aether.BaseActor
	stateKey string
}

// NewCounterActor creates a new CounterActor.
func NewCounterActor() *CounterActor {
	return &CounterActor{
		BaseActor: aether.NewBaseActor("counter-actor"),
		stateKey:  "counter_state",
	}
}

// OnStart is called when the actor starts.
// It loads the persisted state if it exists.
func (a *CounterActor) OnStart(ctx context.Context) error {
	log.Printf("[%s] Starting counter actor...", a.Name())

	// Check if we have persisted state
	exists, err := a.State().Exists(ctx, a.stateKey)
	if err != nil {
		return fmt.Errorf("failed to check state: %w", err)
	}

	if exists {
		// Load existing state
		data, err := a.State().Read(ctx, a.stateKey)
		if err != nil {
			return fmt.Errorf("failed to read state: %w", err)
		}

		var state CounterState
		if err := json.Unmarshal(data, &state); err != nil {
			return fmt.Errorf("failed to unmarshal state: %w", err)
		}

		log.Printf("[%s] Restored state: value=%d, last_push=%s",
			a.Name(), state.Value, state.LastPush)
	} else {
		// Initialize new state
		state := CounterState{
			Value:    0,
			LastPush: "",
		}
		if err := a.saveState(ctx, state); err != nil {
			return fmt.Errorf("failed to initialize state: %w", err)
		}
		log.Printf("[%s] Initialized new counter state", a.Name())
	}

	return nil
}

// OnStop is called when the actor stops.
// State is already persisted on each operation, so nothing special needed here.
func (a *CounterActor) OnStop(ctx context.Context) error {
	log.Printf("[%s] Counter actor stopping", a.Name())
	return nil
}

// HandleMessage handles incoming messages.
func (a *CounterActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	// Load current state
	state, err := a.loadState(ctx)
	if err != nil {
		return nil, err
	}

	// Process the command
	switch payload := msg.Payload.(type) {
	case string:
		return a.handleStringCommand(ctx, state, payload)

	case map[string]any:
		return a.handleMapCommand(ctx, state, payload)

	default:
		return aether.NewResponse(msg, map[string]any{
			"error":  "unknown payload type",
			"value":  state.Value,
			"sender": sender,
		}), nil
	}
}

func (a *CounterActor) handleStringCommand(ctx context.Context, state *CounterState, cmd string) (*aether.Message, error) {
	switch cmd {
	case "increment":
		state.Value++
		if err := a.saveState(ctx, *state); err != nil {
			return nil, err
		}
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action": "increment",
			"value":  state.Value,
		}), nil

	case "decrement":
		state.Value--
		if err := a.saveState(ctx, *state); err != nil {
			return nil, err
		}
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action": "decrement",
			"value":  state.Value,
		}), nil

	case "reset":
		state.Value = 0
		if err := a.saveState(ctx, *state); err != nil {
			return nil, err
		}
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action": "reset",
			"value":  state.Value,
		}), nil

	case "get":
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action": "get",
			"value":  state.Value,
		}), nil

	default:
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"error":   fmt.Sprintf("unknown command: %s", cmd),
			"value":   state.Value,
			"usage":   "commands: increment, decrement, reset, get",
		}), nil
	}
}

func (a *CounterActor) handleMapCommand(ctx context.Context, state *CounterState, payload map[string]any) (*aether.Message, error) {
	cmd, ok := payload["command"].(string)
	if !ok {
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"error": "missing 'command' field",
			"value": state.Value,
		}), nil
	}

	// Handle commands with additional parameters
	switch cmd {
	case "add":
		amount, _ := payload["amount"].(float64)
		state.Value += int64(amount)
		if err := a.saveState(ctx, *state); err != nil {
			return nil, err
		}
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action": "add",
			"amount": amount,
			"value":  state.Value,
		}), nil

	case "subtract":
		amount, _ := payload["amount"].(float64)
		state.Value -= int64(amount)
		if err := a.saveState(ctx, *state); err != nil {
			return nil, err
		}
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action":   "subtract",
			"amount":   amount,
			"value":    state.Value,
		}), nil

	case "set":
		value, _ := payload["value"].(float64)
		state.Value = int64(value)
		if err := a.saveState(ctx, *state); err != nil {
			return nil, err
		}
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"action": "set",
			"value":  state.Value,
		}), nil

	default:
		// Delegate to string command handler
		return a.handleStringCommand(ctx, state, cmd)
	}
}

// loadState loads the counter state from persistent storage.
func (a *CounterActor) loadState(ctx context.Context) (*CounterState, error) {
	data, err := a.State().Read(ctx, a.stateKey)
	if err != nil {
		return nil, fmt.Errorf("failed to read state: %w", err)
	}

	if data == nil {
		return &CounterState{Value: 0}, nil
	}

	var state CounterState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, fmt.Errorf("failed to unmarshal state: %w", err)
	}

	return &state, nil
}

// saveState saves the counter state to persistent storage.
func (a *CounterActor) saveState(ctx context.Context, state CounterState) error {
	state.LastPush = time.Now().UTC().Format(time.RFC3339)

	data, err := json.Marshal(state)
	if err != nil {
		return fmt.Errorf("failed to marshal state: %w", err)
	}

	if err := a.State().Write(ctx, a.stateKey, data); err != nil {
		return fmt.Errorf("failed to write state: %w", err)
	}

	return nil
}

func main() {
	// Create actor with capabilities
	actor := NewCounterActor()
	actor.Require(
		aether.CapabilityStateRead,
		aether.CapabilityStateWrite,
		aether.CapabilityActorMessaging,
		aether.CapabilityLog,
		aether.CapabilityTime,
	)

	// Setup context with cancellation
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Handle shutdown signals
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		log.Println("Shutting down...")
		actor.Stop()
		cancel()
	}()

	log.Printf("Starting %s with state persistence...", actor.Name())
	log.Printf("Commands: increment, decrement, reset, get, add, subtract, set")

	// Run the actor
	if err := actor.Run(ctx); err != nil {
		if err != context.Canceled {
			log.Fatalf("Actor error: %v", err)
		}
	}

	log.Println("Actor stopped")
}
