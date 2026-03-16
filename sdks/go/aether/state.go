package aether

import (
	"context"
	"sync"
)

// StateHandle provides persistent state storage for actors.
// State persists across actor restarts and is scoped to the actor's ID.
type StateHandle struct {
	mu    sync.RWMutex
	store map[string][]byte
}

// NewStateHandle creates a new state handle.
func NewStateHandle() *StateHandle {
	return &StateHandle{
		store: make(map[string][]byte),
	}
}

// Read retrieves a value from state by key.
// Returns nil if the key doesn't exist.
func (s *StateHandle) Read(ctx context.Context, key string) ([]byte, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	if val, ok := s.store[key]; ok {
		// Return a copy to prevent mutation
		result := make([]byte, len(val))
		copy(result, val)
		return result, nil
	}
	return nil, nil
}

// Write stores a value in state by key.
func (s *StateHandle) Write(ctx context.Context, key string, value []byte) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Store a copy to prevent mutation
	stored := make([]byte, len(value))
	copy(stored, value)
	s.store[key] = stored
	return nil
}

// Delete removes a value from state by key.
func (s *StateHandle) Delete(ctx context.Context, key string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	delete(s.store, key)
	return nil
}

// ListKeys returns all keys that match the given prefix.
func (s *StateHandle) ListKeys(ctx context.Context, prefix string) ([]string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var keys []string
	for key := range s.store {
		if len(key) >= len(prefix) && key[:len(prefix)] == prefix {
			keys = append(keys, key)
		}
	}
	return keys, nil
}

// Clear removes all keys from state.
func (s *StateHandle) Clear(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	s.store = make(map[string][]byte)
	return nil
}

// Exists checks if a key exists in state.
func (s *StateHandle) Exists(ctx context.Context, key string) (bool, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	_, ok := s.store[key]
	return ok, nil
}

// Delete removes a key from state.
func (s *StateHandle) Delete(ctx context.Context, key string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	delete(s.store, key)
	return nil
}
