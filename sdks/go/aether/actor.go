package aether

import (
	"context"
	"sync"
	"time"

	"github.com/google/uuid"
)

// Actor is the interface that all Aether actors must implement.
type Actor interface {
	// Name returns the actor's name for registration.
	Name() string

	// HandleMessage processes an incoming message.
	// Returns an optional response message.
	HandleMessage(ctx context.Context, sender string, message *Message) (*Message, error)

	// OnStart is called when the actor starts.
	// Use this for initialization.
	OnStart(ctx context.Context) error

	// OnStop is called when the actor stops.
	// Use this for cleanup.
	OnStop(ctx context.Context) error
}

// BaseActor provides a base implementation of the Actor interface.
// Embed this in your actor struct to get default implementations.
type BaseActor struct {
	name         string
	capabilities *CapabilitySet
	state        *StateHandle
	mailbox      chan mailboxItem
	pendingRPC   map[string]chan *Message
	pendingMu    sync.RWMutex
	running      bool
	runningMu    sync.RWMutex

	OnStartFunc      func(ctx context.Context) error
	OnStopFunc       func(ctx context.Context) error
	HandleMessageFunc func(ctx context.Context, sender string, message *Message) (*Message, error)
}

type mailboxItem struct {
	action  string
	target  string
	sender  string
	message *Message
}

// NewBaseActor creates a new BaseActor with the given name.
func NewBaseActor(name string) *BaseActor {
	return &BaseActor{
		name:        name,
		capabilities: NewCapabilitySet(),
		state:       NewStateHandle(),
		mailbox:     make(chan mailboxItem, 1000),
		pendingRPC:  make(map[string]chan *Message),
	}
}

// Name returns the actor's name.
func (a *BaseActor) Name() string {
	return a.name
}

// HandleMessage is the default message handler.
// Override this in your actor implementation.
func (a *BaseActor) HandleMessage(ctx context.Context, sender string, message *Message) (*Message, error) {
	if a.HandleMessageFunc != nil {
		return a.HandleMessageFunc(ctx, sender, message)
	}
	return nil, nil
}

func (a *BaseActor) OnStart(ctx context.Context) error {
	if a.OnStartFunc != nil {
		return a.OnStartFunc(ctx)
	}
	return nil
}

func (a *BaseActor) OnStop(ctx context.Context) error {
	if a.OnStopFunc != nil {
		return a.OnStopFunc(ctx)
	}
	return nil
}

// OnStart is called when the actor starts.
func (a *BaseActor) OnStart(ctx context.Context) error {
	return nil
}

// OnStop is called when the actor stops.
func (a *BaseActor) OnStop(ctx context.Context) error {
	return nil
}

// Require declares required capabilities.
func (a *BaseActor) Require(capabilities ...Capability) {
	for _, cap := range capabilities {
		a.capabilities.Add(cap)
	}
}

// Capabilities returns the actor's capability set.
func (a *BaseActor) Capabilities() *CapabilitySet {
	return a.capabilities
}

// State returns the actor's state handle.
func (a *BaseActor) State() *StateHandle {
	return a.state
}

// Send sends a message to another actor.
func (a *BaseActor) Send(ctx context.Context, target string, message *Message) error {
	message.Sender = a.name
	a.mailbox <- mailboxItem{
		action: "send",
		target: target,
		message: message,
	}
	return nil
}

// Call performs an RPC call to another actor and waits for a response.
func (a *BaseActor) Call(ctx context.Context, target string, request any, timeout time.Duration) (any, error) {
	correlationID := uuid.New().String()
	message := NewRPCRequest(a.name, request, correlationID)

	responseChan := make(chan *Message, 1)
	a.pendingMu.Lock()
	a.pendingRPC[correlationID] = responseChan
	a.pendingMu.Unlock()

	defer func() {
		a.pendingMu.Lock()
		delete(a.pendingRPC, correlationID)
		a.pendingMu.Unlock()
		close(responseChan)
	}()

	if err := a.Send(ctx, target, message); err != nil {
		return nil, err
	}

	select {
	case response := <-responseChan:
		if response.Type == MessageTypeError {
			return nil, NewError(ErrCodeRpcError, response.Payload.(string), nil)
		}
		return response.Payload, nil
	case <-time.After(timeout):
		return nil, Timeout("rpc call to " + target)
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

// Deliver delivers a message to this actor's mailbox.
func (a *BaseActor) Deliver(sender string, message *Message) {
	a.mailbox <- mailboxItem{
		action:  "receive",
		sender:  sender,
		message: message,
	}
}

// Run starts the actor's main loop.
func (a *BaseActor) Run(ctx context.Context) error {
	a.runningMu.Lock()
	a.running = true
	a.runningMu.Unlock()

	if err := a.OnStart(ctx); err != nil {
		return err
	}

	defer func() {
		_ = a.OnStop(ctx)
		a.runningMu.Lock()
		a.running = false
		a.runningMu.Unlock()
	}()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case item := <-a.mailbox:
			if err := a.processItem(ctx, item); err != nil {
				// Log error but continue
			}
		}
	}
}

// Stop stops the actor.
func (a *BaseActor) Stop() {
	a.runningMu.Lock()
	a.running = false
	a.runningMu.Unlock()
}

// IsRunning returns whether the actor is running.
func (a *BaseActor) IsRunning() bool {
	a.runningMu.RLock()
	defer a.runningMu.RUnlock()
	return a.running
}

func (a *BaseActor) processItem(ctx context.Context, item mailboxItem) error {
	switch item.action {
	case "send":
		// Route the message to the target actor.
		// In a cluster deployment, this would go through the mesh network.
		// In-process, attempt direct delivery.
		return nil
	case "receive":
		return a.handleIncoming(ctx, item.sender, item.message)
	default:
		return nil
	}
}

func (a *BaseActor) handleIncoming(ctx context.Context, sender string, message *Message) error {
	// Handle RPC responses
	if message.Type == MessageTypeRPCResponse && message.CorrelationID != "" {
		a.pendingMu.RLock()
		if ch, ok := a.pendingRPC[message.CorrelationID]; ok {
			ch <- message
		}
		a.pendingMu.RUnlock()
		return nil
	}

	// Handle regular messages
	response, err := a.HandleMessage(ctx, sender, message)
	if err != nil {
		return err
	}

	// Send response if this was an RPC request
	if response != nil && message.CorrelationID != "" {
		response.Type = MessageTypeRPCResponse
		response.CorrelationID = message.CorrelationID
		response.Sender = a.name
		// In a real implementation, route back to sender
	}

	return nil
}
