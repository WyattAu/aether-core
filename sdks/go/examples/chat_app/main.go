// Package main demonstrates a full application built with Aether actors.
// This is a chat room application with multiple actors working together.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

// ================== Domain Types ==================

// User represents a chat user.
type User struct {
	ID       string `json:"id"`
	Name     string `json:"name"`
	JoinedAt string `json:"joined_at"`
}

// ChatMessage represents a chat message.
type ChatMessage struct {
	ID        string `json:"id"`
	UserID    string `json:"user_id"`
	UserName  string `json:"user_name"`
	Content   string `json:"content"`
	Timestamp string `json:"timestamp"`
	RoomID    string `json:"room_id"`
}

// Room represents a chat room.
type Room struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	CreatedAt   string            `json:"created_at"`
	CreatedBy   string            `json:"created_by"`
	UserCount   int               `json:"user_count"`
	MessageCount int              `json:"message_count"`
}

// ================== Room Actor ==================

// RoomActor manages a single chat room.
type RoomActor struct {
	*aether.BaseActor
	roomID    string
	roomName  string
	users     map[string]*User
	messages  []ChatMessage
	mu        sync.RWMutex
	stateKey  string
}

// NewRoomActor creates a new RoomActor.
func NewRoomActor(roomID, roomName string) *RoomActor {
	return &RoomActor{
		BaseActor: aether.NewBaseActor(fmt.Sprintf("room-%s", roomID)),
		roomID:    roomID,
		roomName:  roomName,
		users:     make(map[string]*User),
		messages:  make([]ChatMessage, 0, 1000),
		stateKey:  fmt.Sprintf("room_%s_state", roomID),
	}
}

// OnStart loads persisted room state.
func (a *RoomActor) OnStart(ctx context.Context) error {
	log.Printf("[%s] Room '%s' starting...", a.Name(), a.roomName)

	// Try to load persisted state
	data, err := a.State().Read(ctx, a.stateKey)
	if err != nil {
		log.Printf("[%s] Warning: could not read state: %v", a.Name(), err)
		return nil
	}

	if data != nil {
		var state struct {
			RoomID   string        `json:"room_id"`
			RoomName string        `json:"room_name"`
			Users    []*User       `json:"users"`
			Messages []ChatMessage `json:"messages"`
		}
		if err := json.Unmarshal(data, &state); err == nil {
			a.roomName = state.RoomName
			for _, u := range state.Users {
				a.users[u.ID] = u
			}
			a.messages = state.Messages
			log.Printf("[%s] Restored %d users and %d messages",
				a.Name(), len(a.users), len(a.messages))
		}
	}

	return nil
}

// OnStop saves room state.
func (a *RoomActor) OnStop(ctx context.Context) error {
	log.Printf("[%s] Room stopping, saving state...", a.Name())
	return a.saveState(ctx)
}

func (a *RoomActor) saveState(ctx context.Context) error {
	a.mu.RLock()
	users := make([]*User, 0, len(a.users))
	for _, u := range a.users {
		users = append(users, u)
	}
	state := struct {
		RoomID   string        `json:"room_id"`
		RoomName string        `json:"room_name"`
		Users    []*User       `json:"users"`
		Messages []ChatMessage `json:"messages"`
	}{
		RoomID:   a.roomID,
		RoomName: a.roomName,
		Users:    users,
		Messages: a.messages,
	}
	a.mu.RUnlock()

	data, err := json.Marshal(state)
	if err != nil {
		return err
	}

	return a.State().Write(ctx, a.stateKey, data)
}

// HandleMessage handles room messages.
func (a *RoomActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	payload, ok := msg.Payload.(map[string]any)
	if !ok {
		return aether.NewResponse(msg, map[string]any{
			"error": "invalid payload format",
		}), nil
	}

	action, _ := payload["action"].(string)
	switch action {
	case "join":
		return a.handleJoin(ctx, payload)
	case "leave":
		return a.handleLeave(ctx, payload)
	case "send":
		return a.handleSend(ctx, payload)
	case "history":
		return a.handleHistory(payload)
	case "users":
		return a.handleUsers()
	case "info":
		return a.handleInfo()
	default:
		return aether.NewResponse(msg, map[string]any{
			"error": fmt.Sprintf("unknown action: %s", action),
		}), nil
	}
}

func (a *RoomActor) handleJoin(ctx context.Context, payload map[string]any) (*aether.Message, error) {
	userID, _ := payload["user_id"].(string)
	userName, _ := payload["user_name"].(string)

	if userID == "" || userName == "" {
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"error": "user_id and user_name required",
		}), nil
	}

	a.mu.Lock()
	user := &User{
		ID:       userID,
		Name:     userName,
		JoinedAt: time.Now().UTC().Format(time.RFC3339),
	}
	a.users[userID] = user
	userCount := len(a.users)
	a.mu.Unlock()

	// Save state
	_ = a.saveState(ctx)

	log.Printf("[%s] User '%s' joined (total: %d)", a.Name(), userName, userCount)

	// Broadcast join event (in real app, this would notify other users)
	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":     "joined",
		"room_id":    a.roomID,
		"room_name":  a.roomName,
		"user_id":    userID,
		"user_count": userCount,
	}), nil
}

func (a *RoomActor) handleLeave(ctx context.Context, payload map[string]any) (*aether.Message, error) {
	userID, _ := payload["user_id"].(string)

	a.mu.Lock()
	var userName string
	if user, ok := a.users[userID]; ok {
		userName = user.Name
		delete(a.users, userID)
	}
	userCount := len(a.users)
	a.mu.Unlock()

	// Save state
	_ = a.saveState(ctx)

	log.Printf("[%s] User '%s' left (remaining: %d)", a.Name(), userName, userCount)

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":     "left",
		"room_id":    a.roomID,
		"user_id":    userID,
		"user_count": userCount,
	}), nil
}

func (a *RoomActor) handleSend(ctx context.Context, payload map[string]any) (*aether.Message, error) {
	userID, _ := payload["user_id"].(string)
	content, _ := payload["content"].(string)

	if userID == "" || content == "" {
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"error": "user_id and content required",
		}), nil
	}

	a.mu.Lock()
	user, userExists := a.users[userID]
	if !userExists {
		a.mu.Unlock()
		return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
			"error": "user not in room, join first",
		}), nil
	}

	msg := ChatMessage{
		ID:        fmt.Sprintf("msg-%d", time.Now().UnixNano()),
		UserID:    userID,
		UserName:  user.Name,
		Content:   content,
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		RoomID:    a.roomID,
	}
	a.messages = append(a.messages, msg)
	msgCount := len(a.messages)
	a.mu.Unlock()

	// Save state
	_ = a.saveState(ctx)

	log.Printf("[%s] [%s] %s: %s", a.Name(), msg.ID[:12], user.Name, truncate(content, 30))

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":        "sent",
		"message_id":    msg.ID,
		"message_count": msgCount,
	}), nil
}

func (a *RoomActor) handleHistory(payload map[string]any) (*aether.Message, error) {
	limit := 50
	if l, ok := payload["limit"].(float64); ok {
		limit = int(l)
	}

	a.mu.RLock()
	defer a.mu.RUnlock()

	// Get last N messages
	start := len(a.messages) - limit
	if start < 0 {
		start = 0
	}
	messages := a.messages[start:]

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":   "history",
		"room_id":  a.roomID,
		"messages": messages,
		"count":    len(messages),
	}), nil
}

func (a *RoomActor) handleUsers() (*aether.Message, error) {
	a.mu.RLock()
	users := make([]User, 0, len(a.users))
	for _, u := range a.users {
		users = append(users, *u)
	}
	a.mu.RUnlock()

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":  "users",
		"room_id": a.roomID,
		"users":   users,
		"count":   len(users),
	}), nil
}

func (a *RoomActor) handleInfo() (*aether.Message, error) {
	a.mu.RLock()
	defer a.mu.RUnlock()

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":        "info",
		"room_id":       a.roomID,
		"room_name":     a.roomName,
		"user_count":    len(a.users),
		"message_count": len(a.messages),
	}), nil
}

// ================== Session Actor ==================

// SessionActor manages a user session across multiple rooms.
type SessionActor struct {
	*aether.BaseActor
	userID   string
	userName string
	rooms    map[string]bool
	mu       sync.RWMutex
}

// NewSessionActor creates a new SessionActor.
func NewSessionActor(userID, userName string) *SessionActor {
	return &SessionActor{
		BaseActor: aether.NewBaseActor(fmt.Sprintf("session-%s", userID)),
		userID:    userID,
		userName:  userName,
		rooms:     make(map[string]bool),
	}
}

// OnStart initializes the session.
func (a *SessionActor) OnStart(ctx context.Context) error {
	log.Printf("[%s] Session started for user '%s'", a.Name(), a.userName)
	return nil
}

// OnStop cleans up the session.
func (a *SessionActor) OnStop(ctx context.Context) error {
	log.Printf("[%s] Session ended for user '%s'", a.Name(), a.userName)
	return nil
}

// HandleMessage handles session messages.
func (a *SessionActor) HandleMessage(ctx context.Context, sender string, msg *aether.Message) (*aether.Message, error) {
	payload, ok := msg.Payload.(map[string]any)
	if !ok {
		return aether.NewResponse(msg, map[string]any{
			"error": "invalid payload format",
		}), nil
	}

	action, _ := payload["action"].(string)
	switch action {
	case "status":
		return a.handleStatus()
	case "join_room":
		return a.handleJoinRoom(payload)
	case "leave_room":
		return a.handleLeaveRoom(payload)
	default:
		return aether.NewResponse(msg, map[string]any{
			"error": fmt.Sprintf("unknown action: %s", action),
		}), nil
	}
}

func (a *SessionActor) handleStatus() (*aether.Message, error) {
	a.mu.RLock()
	rooms := make([]string, 0, len(a.rooms))
	for roomID := range a.rooms {
		rooms = append(rooms, roomID)
	}
	a.mu.RUnlock()

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":    "status",
		"user_id":   a.userID,
		"user_name": a.userName,
		"rooms":     rooms,
	}), nil
}

func (a *SessionActor) handleJoinRoom(payload map[string]any) (*aether.Message, error) {
	roomID, _ := payload["room_id"].(string)

	a.mu.Lock()
	a.rooms[roomID] = true
	roomCount := len(a.rooms)
	a.mu.Unlock()

	log.Printf("[%s] User joined room '%s' (total rooms: %d)",
		a.Name(), roomID, roomCount)

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":     "joined_room",
		"room_id":    roomID,
		"room_count": roomCount,
	}), nil
}

func (a *SessionActor) handleLeaveRoom(payload map[string]any) (*aether.Message, error) {
	roomID, _ := payload["room_id"].(string)

	a.mu.Lock()
	delete(a.rooms, roomID)
	roomCount := len(a.rooms)
	a.mu.Unlock()

	log.Printf("[%s] User left room '%s' (remaining rooms: %d)",
		a.Name(), roomID, roomCount)

	return aether.NewMessage(aether.MessageTypeResponse, map[string]any{
		"action":     "left_room",
		"room_id":    roomID,
		"room_count": roomCount,
	}), nil
}

// ================== Main Application ==================

// ChatApp is the main chat application.
type ChatApp struct {
	roomActor    *RoomActor
	sessionActor *SessionActor
	ctx          context.Context
	cancel       context.CancelFunc
	wg           sync.WaitGroup
}

// NewChatApp creates a new chat application.
func NewChatApp() *ChatApp {
	return &ChatApp{}
}

// Start starts the chat application.
func (app *ChatApp) Start(ctx context.Context) error {
	app.ctx, app.cancel = context.WithCancel(ctx)

	// Create actors
	app.roomActor = NewRoomActor("general", "General Chat")
	app.roomActor.Require(
		aether.CapabilityStateRead,
		aether.CapabilityStateWrite,
		aether.CapabilityActorMessaging,
		aether.CapabilityLog,
		aether.CapabilityTime,
	)

	app.sessionActor = NewSessionActor("demo-user", "Demo User")
	app.sessionActor.Require(
		aether.CapabilityActorMessaging,
		aether.CapabilityLog,
	)

	// Start actors
	app.wg.Add(2)

	go func() {
		defer app.wg.Done()
		if err := app.roomActor.Run(app.ctx); err != nil && err != context.Canceled {
			log.Printf("Room actor error: %v", err)
		}
	}()

	go func() {
		defer app.wg.Done()
		if err := app.sessionActor.Run(app.ctx); err != nil && err != context.Canceled {
			log.Printf("Session actor error: %v", err)
		}
	}()

	// Wait for actors to start
	time.Sleep(100 * time.Millisecond)

	// Demo: Auto-join the room
	app.sessionActor.Deliver("system", aether.NewMessage(aether.MessageTypeRequest, map[string]any{
		"action":   "join_room",
		"room_id":  "general",
	}))
	app.roomActor.Deliver("system", aether.NewMessage(aether.MessageTypeRequest, map[string]any{
		"action":    "join",
		"user_id":   "demo-user",
		"user_name": "Demo User",
	}))

	return nil
}

// Stop stops the chat application.
func (app *ChatApp) Stop() {
	app.cancel()
	app.wg.Wait()
}

// truncate truncates a string.
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

func main() {
	log.Println("=== Aether Chat Application ===")
	log.Println("Demonstrating multi-actor chat system")

	// Create and start the application
	app := NewChatApp()
	ctx := context.Background()

	if err := app.Start(ctx); err != nil {
		log.Fatalf("Failed to start: %v", err)
	}

	log.Println("Application started. Room: 'General Chat', User: 'Demo User'")
	log.Println("Commands: join, leave, send, history, users, info, status, quit")

	// Handle shutdown signals
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	// Interactive command loop
	go func() {
		// Simulate some activity
		time.Sleep(500 * time.Millisecond)

		// Send a welcome message
		app.roomActor.Deliver("system", aether.NewMessage(aether.MessageTypeRequest, map[string]any{
			"action":  "send",
			"user_id": "demo-user",
			"content": "Hello, Aether Chat!",
		}))

		time.Sleep(300 * time.Millisecond)

		// Get room info
		app.roomActor.Deliver("system", aether.NewMessage(aether.MessageTypeRequest, map[string]any{
			"action": "info",
		}))

		time.Sleep(300 * time.Millisecond)

		// Get message history
		app.roomActor.Deliver("system", aether.NewMessage(aether.MessageTypeRequest, map[string]any{
			"action": "history",
			"limit":  10,
		}))
	}()

	// Wait for shutdown signal
	<-sigChan
	log.Println("\nShutting down...")

	app.Stop()
	log.Println("Application stopped")
}
