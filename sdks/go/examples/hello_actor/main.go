// Package main demonstrates a simple Aether actor in Go.
package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

// HelloActor is a simple actor that responds to greeting messages.
type HelloActor struct {
	*aether.BaseActor
}

// NewHelloActor creates a new HelloActor.
func NewHelloActor() *HelloActor {
	return &HelloActor{
		BaseActor: aether.NewBaseActor("hello-actor"),
	}
}

// HandleMessage handles incoming messages.
func (a *HelloActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	switch payload := msg.Payload.(type) {
	case string:
		if payload == "ping" {
			return aether.NewResponse(msg, "pong"), nil
		}
		return aether.NewResponse(msg, fmt.Sprintf("Hello, %s!", payload)), nil
	case map[string]any:
		if name, ok := payload["name"].(string); ok {
			return aether.NewResponse(msg, map[string]any{
				"greeting": fmt.Sprintf("Hello, %s!", name),
				"sender":   sender,
			}), nil
		}
	default:
		return aether.NewResponse(msg, map[string]any{
			"error": "unknown payload type",
		}), nil
	}
	return nil, nil
}

func main() {
	// Create actor with capabilities
	actor := NewHelloActor()
	actor.Require(
		aether.CapabilityActorMessaging,
		aether.CapabilityLog,
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

	log.Printf("Starting %s...", actor.Name())

	// Run the actor
	if err := actor.Run(ctx); err != nil {
		if err != context.Canceled {
			log.Fatalf("Actor error: %v", err)
		}
	}

	log.Println("Actor stopped")
}
