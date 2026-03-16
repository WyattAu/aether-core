// Package aether provides the Go SDK for Aether actors.
//
// Aether is a distributed actor framework for building resilient,
// scalable applications. This SDK provides the core types and
// interfaces needed to implement Aether actors in Go.
//
// # Overview
//
// The SDK provides:
//   - Actor interface and base implementation
//   - Capability-based security model
//   - Message passing between actors
//   - State management
//   - HTTP client with capability checks
//
// # Quick Start
//
// Define an actor by implementing the Actor interface:
//
//	type MyActor struct {
//	    *aether.BaseActor
//	}
//
//	func (a *MyActor) Name() string {
//	    return "my-actor"
//	}
//
//	func (a *MyActor) HandleMessage(ctx context.Context, sender string, msg aether.Message) (*aether.Message, error) {
//	    // Handle the message
//	    return aether.NewResponse(msg, "pong"), nil
//	}
//
// # Capabilities
//
// Actors must declare required capabilities:
//
//	caps := aether.NewCapabilitySet(
//	    aether.CapabilityNetworkOutbound,
//	    aether.CapabilityStateRead,
//	)
//
// # Messaging
//
// Send messages to other actors:
//
//	err := actor.Send(ctx, "target-actor", aether.NewMessage(
//	    aether.MessageTypeRequest,
//	    payload,
//	))
//
// Or use RPC for request-response:
//
//	response, err := actor.Call(ctx, "target-actor", request, 30*time.Second)
//
// # State Management
//
// Persist state across actor restarts:
//
//	state := actor.State()
//	err := state.Write(ctx, "key", []byte("value"))
//	value, err := state.Read(ctx, "key")
package aether
