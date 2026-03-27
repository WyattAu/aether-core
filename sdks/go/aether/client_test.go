package aether

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func newTestClient(handler http.HandlerFunc) (*Client, *httptest.Server) {
	server := httptest.NewServer(handler)
	return NewClient(server.URL), server
}

func TestHealth(t *testing.T) {
	t.Run("ok", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path != "/api/v1/info" || r.Method != "GET" {
				t.Errorf("unexpected request: %s %s", r.Method, r.URL.Path)
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]any{
				"version": "0.1.0", "uptime": 123.456, "actor_count": 5, "message_count": 100,
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		info, err := client.Health()
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if info.Version != "0.1.0" {
			t.Errorf("expected version 0.1.0, got %s", info.Version)
		}
		if info.ActorCount != 5 {
			t.Errorf("expected actor_count 5, got %d", info.ActorCount)
		}
		if info.MessageCount != 100 {
			t.Errorf("expected message_count 100, got %d", info.MessageCount)
		}
	})

	t.Run("server error", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusInternalServerError)
			w.Write([]byte("internal error"))
		})
		client, server := newTestClient(handler)
		defer server.Close()

		_, err := client.Health()
		if err == nil {
			t.Fatal("expected error, got nil")
		}
		srvErr, ok := err.(*AetherServerError)
		if !ok {
			t.Fatalf("expected AetherServerError, got %T", err)
		}
		if srvErr.StatusCode != 500 {
			t.Errorf("expected status 500, got %d", srvErr.StatusCode)
		}
	})
}

func TestRegisterActor(t *testing.T) {
	actorResp := map[string]any{
		"actor_id": "test-actor", "actor_type": "worker",
		"capabilities": []string{"compute"}, "metadata": map[string]any{"region": "us-east"},
		"status": "active", "created_at": "2026-01-01T00:00:00Z",
	}

	tests := []struct {
		name       string
		statusCode int
		respBody   any
		wantErr    bool
		errCode    int
	}{
		{"created", 201, actorResp, false, 0},
		{"duplicate 409", 409, map[string]string{"detail": "already exists"}, true, 409},
		{"server error 507", 507, map[string]string{"detail": "insufficient storage"}, true, 507},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.Method != "POST" || r.URL.Path != "/api/v1/actors" {
					t.Errorf("unexpected request: %s %s", r.Method, r.URL.Path)
					return
				}
				var body map[string]any
				json.NewDecoder(r.Body).Decode(&body)
				if body["actor_id"] != "test-actor" {
					t.Errorf("expected actor_id test-actor, got %v", body["actor_id"])
				}
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(tt.statusCode)
				json.NewEncoder(w).Encode(tt.respBody)
			})
			client, server := newTestClient(handler)
			defer server.Close()

			info, err := client.RegisterActor("test-actor", "worker", []string{"compute"}, map[string]any{"region": "us-east"})
			if tt.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				srvErr, ok := err.(*AetherServerError)
				if !ok {
					t.Fatalf("expected AetherServerError, got %T", err)
				}
				if srvErr.StatusCode != tt.errCode {
					t.Errorf("expected status %d, got %d", tt.errCode, srvErr.StatusCode)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.ActorID != "test-actor" {
				t.Errorf("expected actor_id test-actor, got %s", info.ActorID)
			}
			if info.ActorType != "worker" {
				t.Errorf("expected actor_type worker, got %s", info.ActorType)
			}
		})
	}
}

func TestUnregisterActor(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		wantErr    bool
	}{
		{"ok 204", 204, false},
		{"not found 404", 404, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.Method != "DELETE" {
					t.Errorf("unexpected method: %s", r.Method)
				}
				w.WriteHeader(tt.statusCode)
			})
			client, server := newTestClient(handler)
			defer server.Close()

			err := client.UnregisterActor("test-actor")
			if tt.wantErr && err == nil {
				t.Fatal("expected error, got nil")
			}
			if !tt.wantErr && err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestGetActor(t *testing.T) {
	t.Run("found", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.Method != "GET" {
				t.Errorf("unexpected method: %s", r.Method)
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]any{
				"actor_id": "actor-1", "actor_type": "worker", "status": "active",
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		info, err := client.GetActor("actor-1")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if info.ActorID != "actor-1" {
			t.Errorf("expected actor-1, got %s", info.ActorID)
		}
	})

	t.Run("not found", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"detail": "not found"})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		_, err := client.GetActor("missing")
		if err == nil {
			t.Fatal("expected error, got nil")
		}
		srvErr := err.(*AetherServerError)
		if srvErr.StatusCode != 404 {
			t.Errorf("expected 404, got %d", srvErr.StatusCode)
		}
	})
}

func TestListActors(t *testing.T) {
	t.Run("all actors", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Query().Get("type") != "" {
				t.Error("expected no type filter")
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode([]map[string]any{
				{"actor_id": "a1", "actor_type": "worker"},
				{"actor_id": "a2", "actor_type": "scheduler"},
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		actors, err := client.ListActors("", "")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if len(actors) != 2 {
			t.Errorf("expected 2 actors, got %d", len(actors))
		}
	})

	t.Run("filter by type", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Query().Get("type") != "worker" {
				t.Errorf("expected type=worker, got %s", r.URL.Query().Get("type"))
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode([]map[string]any{
				{"actor_id": "a1", "actor_type": "worker"},
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		actors, err := client.ListActors("worker", "")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if len(actors) != 1 {
			t.Errorf("expected 1 actor, got %d", len(actors))
		}
	})
}

func TestHeartbeat(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		wantErr    bool
	}{
		{"ok", 204, false},
		{"not found", 404, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.Method != "POST" {
					t.Errorf("expected POST, got %s", r.Method)
				}
				if r.URL.Path != "/api/v1/actors/my-actor/heartbeat" {
					t.Errorf("unexpected path: %s", r.URL.Path)
				}
				w.WriteHeader(tt.statusCode)
			})
			client, server := newTestClient(handler)
			defer server.Close()

			err := client.Heartbeat("my-actor")
			if tt.wantErr && err == nil {
				t.Fatal("expected error, got nil")
			}
			if !tt.wantErr && err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestSendMessage(t *testing.T) {
	t.Run("accepted", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.Method != "POST" {
				t.Errorf("expected POST, got %s", r.Method)
			}
			var env MessageEnvelope
			json.NewDecoder(r.Body).Decode(&env)
			if env.SourceActor != "sender-1" {
				t.Errorf("expected source sender-1, got %s", env.SourceActor)
			}
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusAccepted)
			json.NewEncoder(w).Encode(map[string]any{
				"message_id": "msg-1", "status": "delivered", "delivered_at": "2026-01-01T00:00:00Z",
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		env := &MessageEnvelope{
			SourceActor: "sender-1", MessageType: "request", Payload: "hello",
		}
		receipt, err := client.SendMessage("target-1", env)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if receipt.Status != "delivered" {
			t.Errorf("expected delivered, got %s", receipt.Status)
		}
	})

	t.Run("target not found", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusNotFound)
			json.NewEncoder(w).Encode(map[string]string{"detail": "actor not found"})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		_, err := client.SendMessage("missing", &MessageEnvelope{})
		if err == nil {
			t.Fatal("expected error, got nil")
		}
	})
}

func TestGetPendingMessages(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			t.Errorf("expected GET, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode([]map[string]any{
			{"message_id": "m1", "source_actor": "a1", "payload": "data1"},
			{"message_id": "m2", "source_actor": "a2", "payload": "data2"},
		})
	})
	client, server := newTestClient(handler)
	defer server.Close()

	msgs, err := client.GetPendingMessages("actor-1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(msgs) != 2 {
		t.Errorf("expected 2 messages, got %d", len(msgs))
	}
	if msgs[0].MessageID != "m1" {
		t.Errorf("expected m1, got %s", msgs[0].MessageID)
	}
}

func TestGetState(t *testing.T) {
	t.Run("found", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]any{
				"actor_id": "a1", "key": "counter", "value": 42,
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		entry, err := client.GetState("a1", "counter")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if entry.Key != "counter" {
			t.Errorf("expected counter, got %s", entry.Key)
		}
	})

	t.Run("not found returns nil", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusNotFound)
		})
		client, server := newTestClient(handler)
		defer server.Close()

		entry, err := client.GetState("a1", "missing")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if entry != nil {
			t.Errorf("expected nil, got %+v", entry)
		}
	})
}

func TestSetState(t *testing.T) {
	t.Run("ok", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.Method != "PUT" {
				t.Errorf("expected PUT, got %s", r.Method)
			}
			var body SetStateRequest
			json.NewDecoder(r.Body).Decode(&body)
			if body.Value != "new-value" {
				t.Errorf("expected new-value, got %v", body.Value)
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(map[string]any{
				"actor_id": "a1", "key": "k", "value": "new-value", "version": 2,
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		entry, err := client.SetState("a1", "k", "new-value", nil)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if entry.Version != 2 {
			t.Errorf("expected version 2, got %d", entry.Version)
		}
	})

	t.Run("conflict 409", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusConflict)
			json.NewEncoder(w).Encode(map[string]string{"detail": "version mismatch"})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		v := 1
		_, err := client.SetState("a1", "k", "val", &v)
		if err == nil {
			t.Fatal("expected error, got nil")
		}
	})
}

func TestDeleteState(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		wantErr    bool
	}{
		{"ok", 204, false},
		{"not found is nil error", 404, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(tt.statusCode)
			})
			client, server := newTestClient(handler)
			defer server.Close()

			err := client.DeleteState("a1", "k")
			if tt.wantErr && err == nil {
				t.Fatal("expected error, got nil")
			}
			if !tt.wantErr && err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestGetAllState(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"actor_id": "a1",
			"state": map[string]any{"k1": "v1", "k2": 42},
		})
	})
	client, server := newTestClient(handler)
	defer server.Close()

	resp, err := client.GetAllState("a1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.ActorID != "a1" {
		t.Errorf("expected a1, got %s", resp.ActorID)
	}
	if len(resp.State) != 2 {
		t.Errorf("expected 2 keys, got %d", len(resp.State))
	}
}

func TestPublish(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			t.Errorf("expected POST, got %s", r.Method)
		}
		var body PublishRequest
		json.NewDecoder(r.Body).Decode(&body)
		if body.Topic != "events.test" {
			t.Errorf("expected topic events.test, got %s", body.Topic)
		}
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusAccepted)
		json.NewEncoder(w).Encode(map[string]any{"topic": "events.test", "subscriber_count": 3})
	})
	client, server := newTestClient(handler)
	defer server.Close()

	result, err := client.Publish(&PublishRequest{Topic: "events.test", Payload: "data"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Topic != "events.test" {
		t.Errorf("expected events.test, got %s", result.Topic)
	}
	if result.SubscriberCount != 3 {
		t.Errorf("expected 3 subscribers, got %d", result.SubscriberCount)
	}
}

func TestSubscribe(t *testing.T) {
	t.Run("ok", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusCreated)
			json.NewEncoder(w).Encode(map[string]any{
				"subscription_id": "sub-1", "topic": "events.test",
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		info, err := client.Subscribe(&SubscribeRequest{Topic: "events.test", SubscriberID: "s1"})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if info.SubscriptionID != "sub-1" {
			t.Errorf("expected sub-1, got %s", info.SubscriptionID)
		}
	})
}

func TestUnsubscribe(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		wantErr    bool
	}{
		{"ok", 204, false},
		{"not found", 404, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.WriteHeader(tt.statusCode)
			})
			client, server := newTestClient(handler)
			defer server.Close()

			err := client.Unsubscribe("sub-1")
			if tt.wantErr && err == nil {
				t.Fatal("expected error, got nil")
			}
			if !tt.wantErr && err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
		})
	}
}

func TestListTopics(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode([]string{"topic.a", "topic.b"})
	})
	client, server := newTestClient(handler)
	defer server.Close()

	topics, err := client.ListTopics()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(topics) != 2 {
		t.Errorf("expected 2 topics, got %d", len(topics))
	}
	if topics[0] != "topic.a" {
		t.Errorf("expected topic.a, got %s", topics[0])
	}
}

func TestGetTopicHistory(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode([]map[string]any{
			{"topic": "t1", "payload": "p1", "message_id": "m1"},
		})
	})
	client, server := newTestClient(handler)
	defer server.Close()

	msgs, err := client.GetTopicHistory("t1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(msgs) != 1 {
		t.Errorf("expected 1 message, got %d", len(msgs))
	}
	if msgs[0].Topic != "t1" {
		t.Errorf("expected t1, got %s", msgs[0].Topic)
	}
}

func TestAppendEvent(t *testing.T) {
	t.Run("ok", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			var body AppendEventRequest
			json.NewDecoder(r.Body).Decode(&body)
			if body.EventType != "created" {
				t.Errorf("expected created, got %s", body.EventType)
			}
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusCreated)
			json.NewEncoder(w).Encode(map[string]any{
				"event_id": "e1", "aggregate_id": "agg-1", "event_type": "created",
				"data": map[string]string{"name": "test"}, "version": 1,
			})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		event, err := client.AppendEvent(&AppendEventRequest{
			AggregateID: "agg-1", EventType: "created", Data: map[string]string{"name": "test"},
		})
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}
		if event.EventID != "e1" {
			t.Errorf("expected e1, got %s", event.EventID)
		}
		if event.Version != 1 {
			t.Errorf("expected version 1, got %d", event.Version)
		}
	})

	t.Run("conflict 409", func(t *testing.T) {
		handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusConflict)
			json.NewEncoder(w).Encode(map[string]string{"detail": "version mismatch"})
		})
		client, server := newTestClient(handler)
		defer server.Close()

		_, err := client.AppendEvent(&AppendEventRequest{
			AggregateID: "agg-1", EventType: "created",
		})
		if err == nil {
			t.Fatal("expected error, got nil")
		}
	})
}

func TestGetEvents(t *testing.T) {
	handler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			t.Errorf("expected GET, got %s", r.Method)
		}
		if r.URL.Path != "/api/v1/events/agg-1" {
			t.Errorf("expected /api/v1/events/agg-1, got %s", r.URL.Path)
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode([]map[string]any{
			{"event_id": "e1", "aggregate_id": "agg-1", "event_type": "created", "version": 1},
			{"event_id": "e2", "aggregate_id": "agg-1", "event_type": "updated", "version": 2},
		})
	})
	client, server := newTestClient(handler)
	defer server.Close()

	events, err := client.GetEvents("agg-1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(events) != 2 {
		t.Errorf("expected 2 events, got %d", len(events))
	}
	if events[0].EventType != "created" {
		t.Errorf("expected created, got %s", events[0].EventType)
	}
	if events[1].Version != 2 {
		t.Errorf("expected version 2, got %d", events[1].Version)
	}
}

func TestClientOptions(t *testing.T) {
	t.Run("with actor id", func(t *testing.T) {
		client := NewClient("http://localhost:8080", WithActorID("my-actor"))
		if client.actorID != "my-actor" {
			t.Errorf("expected my-actor, got %s", client.actorID)
		}
	})

	t.Run("base url trimming", func(t *testing.T) {
		client := NewClient("http://localhost:8080/")
		if client.baseURL != "http://localhost:8080" {
			t.Errorf("expected trimmed url, got %s", client.baseURL)
		}
	})
}
