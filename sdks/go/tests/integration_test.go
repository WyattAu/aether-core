package aether_test

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/WyattAu/aether-core/sdks/go/aether"
)

var baseURL = "http://localhost:8080"

func init() {
	if u := os.Getenv("AETHER_BASE_URL"); u != "" {
		baseURL = strings.TrimRight(u, "/")
	}
}

func serverReachable() bool {
	client := &http.Client{Timeout: 3 * time.Second}
	resp, err := client.Get(baseURL + "/health")
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	return resp.StatusCode == 200
}

func needsServer(t *testing.T) {
	t.Helper()
	if !serverReachable() {
		t.Skipf("Aether server not reachable at %s", baseURL)
	}
}

func uniqueID(prefix string) string {
	return fmt.Sprintf("%s-%d", prefix, time.Now().UnixNano())
}

func TestIntegrationHealthCheck(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	info, err := client.Health()
	if err != nil {
		t.Fatalf("health check failed: %v", err)
	}

	if info.Uptime < 0 {
		t.Errorf("expected uptime >= 0, got %f", info.Uptime)
	}
}

func TestIntegrationInfoEndpoint(t *testing.T) {
	needsServer(t)

	httpClient := &http.Client{Timeout: 5 * time.Second}
	resp, err := httpClient.Get(baseURL + "/api/v1/info")
	if err != nil {
		t.Fatalf("info request failed: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var data map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&data); err != nil {
		t.Fatalf("failed to decode info: %v", err)
	}

	if _, ok := data["version"]; !ok {
		if _, ok := data["actor_count"]; !ok {
			t.Error("expected 'version' or 'actor_count' in info response")
		}
	}
}

func TestIntegrationRegisterActor(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	info, err := client.RegisterActor(actorID, "worker", []string{"compute"}, map[string]any{"region": "us-east"})
	if err != nil {
		t.Fatalf("register actor failed: %v", err)
	}

	if info.ActorID != actorID {
		t.Errorf("expected actor_id %s, got %s", actorID, info.ActorID)
	}
	if info.ActorType != "worker" {
		t.Errorf("expected actor_type worker, got %s", info.ActorType)
	}
}

func TestIntegrationGetActor(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	_, err := client.RegisterActor(actorID, "worker", nil, nil)
	if err != nil {
		t.Fatalf("register actor failed: %v", err)
	}

	got, err := client.GetActor(actorID)
	if err != nil {
		t.Fatalf("get actor failed: %v", err)
	}

	if got.ActorID != actorID {
		t.Errorf("expected %s, got %s", actorID, got.ActorID)
	}
}

func TestIntegrationListActors(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	_, err := client.RegisterActor(actorID, "integration-test", nil, nil)
	if err != nil {
		t.Fatalf("register actor failed: %v", err)
	}

	actors, err := client.ListActors("", "")
	if err != nil {
		t.Fatalf("list actors failed: %v", err)
	}

	found := false
	for _, a := range actors {
		if a.ActorID == actorID {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("actor %s not found in list", actorID)
	}
}

func TestIntegrationUnregisterActor(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	client.RegisterActor(actorID, "worker", nil, nil)

	err := client.UnregisterActor(actorID)
	if err != nil {
		t.Fatalf("unregister actor failed: %v", err)
	}

	_, err = client.GetActor(actorID)
	if err == nil {
		t.Error("expected error when getting unregistered actor")
	}
}

func TestIntegrationSendMessage(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL, aether.WithActorID("integration-test-sender"))
	actorID := uniqueID("inttest")

	_, err := client.RegisterActor(actorID, "worker", nil, nil)
	if err != nil {
		t.Fatalf("register actor failed: %v", err)
	}

	envelope := &aether.MessageEnvelope{
		SourceActor: "integration-test-sender",
		TargetActor: actorID,
		MessageType: "greeting",
		Payload:     map[string]string{"hello": "world"},
	}

	receipt, err := client.SendMessage(actorID, envelope)
	if err != nil {
		t.Fatalf("send message failed: %v", err)
	}

	if receipt.Status != "delivered" {
		t.Errorf("expected status 'delivered', got '%s'", receipt.Status)
	}
	if receipt.MessageID == "" {
		t.Error("expected non-empty message_id")
	}
}

func TestIntegrationHeartbeat(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	_, err := client.RegisterActor(actorID, "worker", nil, nil)
	if err != nil {
		t.Fatalf("register actor failed: %v", err)
	}

	err = client.Heartbeat(actorID)
	if err != nil {
		t.Fatalf("heartbeat failed: %v", err)
	}
}

func TestIntegrationSetGetState(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	client.RegisterActor(actorID, "worker", nil, nil)

	entry, err := client.SetState(actorID, "counter", 42, nil)
	if err != nil {
		t.Fatalf("set state failed: %v", err)
	}
	if entry.Version < 1 {
		t.Errorf("expected version >= 1, got %d", entry.Version)
	}

	got, err := client.GetState(actorID, "counter")
	if err != nil {
		t.Fatalf("get state failed: %v", err)
	}
	if got.Value != float64(42) {
		t.Errorf("expected value 42, got %v", got.Value)
	}
}

func TestIntegrationGetMissingState(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	client.RegisterActor(actorID, "worker", nil, nil)

	got, err := client.GetState(actorID, "nonexistent")
	if err != nil {
		t.Fatalf("get missing state should not error: %v", err)
	}
	if got != nil {
		t.Error("expected nil for missing state key")
	}
}

func TestIntegrationDeleteState(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	client.RegisterActor(actorID, "worker", nil, nil)
	client.SetState(actorID, "temp", "data", nil)

	err := client.DeleteState(actorID, "temp")
	if err != nil {
		t.Fatalf("delete state failed: %v", err)
	}
}

func TestIntegrationGetAllState(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	client.RegisterActor(actorID, "worker", nil, nil)
	client.SetState(actorID, "a", 1, nil)
	client.SetState(actorID, "b", 2, nil)

	resp, err := client.GetAllState(actorID)
	if err != nil {
		t.Fatalf("get all state failed: %v", err)
	}
	if len(resp.State) < 2 {
		t.Errorf("expected >= 2 state keys, got %d", len(resp.State))
	}
}

func TestIntegrationStateVersionIncrements(t *testing.T) {
	needsServer(t)

	client := aether.NewClient(baseURL)
	actorID := uniqueID("inttest")

	client.RegisterActor(actorID, "worker", nil, nil)

	e1, _ := client.SetState(actorID, "counter", 1, nil)
	e2, _ := client.SetState(actorID, "counter", 2, nil)

	if e2.Version <= e1.Version {
		t.Errorf("expected version to increment: %d -> %d", e1.Version, e2.Version)
	}
}

func TestIntegrationClusterInfo(t *testing.T) {
	needsServer(t)

	httpClient := &http.Client{Timeout: 5 * time.Second}
	resp, err := httpClient.Get(baseURL + "/cluster/info")
	if err != nil {
		t.Skipf("Cluster endpoint not available: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == 404 {
		t.Skip("Cluster endpoints not available on this server")
	}
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var data map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&data); err != nil {
		t.Fatalf("failed to decode cluster info: %v", err)
	}

	if _, ok := data["node_id"]; !ok {
		if _, ok := data["cluster_enabled"]; !ok {
			if _, ok := data["status"]; !ok {
				t.Error("expected node_id, cluster_enabled, or status in cluster info")
			}
		}
	}
}

func TestIntegrationClusterNodes(t *testing.T) {
	needsServer(t)

	httpClient := &http.Client{Timeout: 5 * time.Second}
	resp, err := httpClient.Get(baseURL + "/cluster/nodes")
	if err != nil {
		t.Skipf("Cluster endpoint not available: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == 404 {
		t.Skip("Cluster endpoints not available on this server")
	}
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}
}
