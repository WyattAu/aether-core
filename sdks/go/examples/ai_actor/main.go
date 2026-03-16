// Package main demonstrates an AI-powered actor in Go.
// This example shows how to integrate AI capabilities with the Aether actor model.
package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

// AIRequest represents a request to the AI actor.
type AIRequest struct {
	Prompt      string `json:"prompt"`
	Model       string `json:"model,omitempty"`
	MaxTokens   int    `json:"max_tokens,omitempty"`
	Temperature int    `json:"temperature,omitempty"`
}

// AIResponse represents a response from the AI actor.
type AIResponse struct {
	Text        string `json:"text"`
	Model       string `json:"model"`
	TokensUsed  int    `json:"tokens_used"`
	ProcessedAt string `json:"processed_at"`
}

// AIActor is an actor that processes AI requests.
// In a real implementation, this would connect to an AI provider.
type AIActor struct {
	*aether.BaseActor
	defaultModel string
	requestCount int64
}

// NewAIActor creates a new AIActor.
func NewAIActor() *AIActor {
	return &AIActor{
		BaseActor:    aether.NewBaseActor("ai-actor"),
		defaultModel: "aether-1.0",
	}
}

// OnStart is called when the actor starts.
func (a *AIActor) OnStart(ctx context.Context) error {
	log.Printf("[%s] AI Actor starting with model: %s", a.Name(), a.defaultModel)
	log.Printf("[%s] Capabilities: AI inference, text generation, embeddings", a.Name())
	return nil
}

// OnStop is called when the actor stops.
func (a *AIActor) OnStop(ctx context.Context) error {
	log.Printf("[%s] AI Actor stopping. Total requests processed: %d", a.Name(), a.requestCount)
	return nil
}

// HandleMessage handles incoming messages.
func (a *AIActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	switch msg.Type {
	case aether.MessageTypeRPCRequest, aether.MessageTypeRequest:
		return a.handleRequest(ctx, sender, msg)
	case aether.MessageTypeEvent:
		return a.handleEvent(ctx, sender, msg)
	default:
		return aether.NewResponse(msg, map[string]any{
			"error": "unsupported message type",
			"type":  string(msg.Type),
		}), nil
	}
}

func (a *AIActor) handleRequest(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	// Parse the request
	var req AIRequest
	switch payload := msg.Payload.(type) {
	case string:
		req.Prompt = payload
	case map[string]any:
		if prompt, ok := payload["prompt"].(string); ok {
			req.Prompt = prompt
		}
		if model, ok := payload["model"].(string); ok {
			req.Model = model
		}
		if maxTokens, ok := payload["max_tokens"].(float64); ok {
			req.MaxTokens = int(maxTokens)
		}
		if temp, ok := payload["temperature"].(float64); ok {
			req.Temperature = int(temp)
		}
	default:
		return aether.NewResponse(msg, map[string]any{
			"error": "invalid payload type, expected string or object",
		}), nil
	}

	// Validate request
	if req.Prompt == "" {
		return aether.NewResponse(msg, map[string]any{
			"error": "prompt is required",
		}), nil
	}

	// Set defaults
	if req.Model == "" {
		req.Model = a.defaultModel
	}
	if req.MaxTokens == 0 {
		req.MaxTokens = 256
	}

	// Increment request counter
	a.requestCount++

	// Process the AI request
	response, err := a.processAIRequest(ctx, req)
	if err != nil {
		return aether.NewResponse(msg, map[string]any{
			"error":   err.Error(),
			"request": req,
		}), nil
	}

	// Return the response
	return aether.NewResponse(msg, map[string]any{
		"request":  req,
		"response": response,
		"sender":   sender,
	}), nil
}

func (a *AIActor) handleEvent(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	// AI actors can also react to events
	switch payload := msg.Payload.(type) {
	case string:
		log.Printf("[%s] Received event from %s: %s", a.Name(), sender, payload)
	case map[string]any:
		if eventType, ok := payload["type"].(string); ok {
			log.Printf("[%s] Received %s event from %s", a.Name(), eventType, sender)
			// In a real implementation, you might trigger AI processing based on events
		}
	}
	// Events don't require responses
	return nil, nil
}

// processAIRequest simulates AI processing.
// In a real implementation, this would call an AI provider API.
func (a *AIActor) processAIRequest(ctx context.Context, req AIRequest) (*AIResponse, error) {
	// Check context cancellation
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	default:
	}

	// Simulate AI processing time
	processingTime := time.Duration(len(req.Prompt)) * time.Millisecond
	if processingTime > 2*time.Second {
		processingTime = 2 * time.Second
	}
	if processingTime < 100*time.Millisecond {
		processingTime = 100 * time.Millisecond
	}

	// In production, this would be an actual AI API call
	// For demo purposes, we simulate intelligent responses
	time.Sleep(processingTime)

	// Generate a simulated response
	response := &AIResponse{
		Model:       req.Model,
		ProcessedAt: time.Now().UTC().Format(time.RFC3339),
		TokensUsed:  len(req.Prompt) / 4, // Rough estimate
	}

	// Simulate different AI capabilities based on prompt content
	promptLower := strings.ToLower(req.Prompt)

	switch {
	case strings.Contains(promptLower, "summarize"):
		response.Text = fmt.Sprintf("[AI Summary] Processed: %s", truncate(req.Prompt, 50))
	case strings.Contains(promptLower, "translate"):
		response.Text = fmt.Sprintf("[AI Translation] Would translate: %s", truncate(req.Prompt, 50))
	case strings.Contains(promptLower, "analyze"):
		response.Text = fmt.Sprintf("[AI Analysis] Analyzed input with %d characters", len(req.Prompt))
	case strings.Contains(promptLower, "generate"):
		response.Text = fmt.Sprintf("[AI Generated] Creative output based on: %s", truncate(req.Prompt, 50))
	default:
		response.Text = fmt.Sprintf("[AI Response] Processed your request: %s", truncate(req.Prompt, 100))
	}

	response.TokensUsed += len(response.Text) / 4

	return response, nil
}

// truncate truncates a string to a maximum length.
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

func main() {
	// Create actor with AI capabilities
	actor := NewAIActor()
	actor.Require(
		aether.CapabilityNetworkOutbound, // For API calls to AI providers
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
		log.Println("Shutting down AI actor...")
		actor.Stop()
		cancel()
	}()

	log.Printf("Starting %s...", actor.Name())
	log.Printf("Supported operations: generate, summarize, translate, analyze")
	log.Printf("Default model: %s", actor.defaultModel)

	// Run the actor
	if err := actor.Run(ctx); err != nil {
		if err != context.Canceled {
			log.Fatalf("Actor error: %v", err)
		}
	}

	log.Println("AI Actor stopped")
}
