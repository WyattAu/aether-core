package aether

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

type ActorInfo struct {
	ActorID       string         `json:"actor_id"`
	ActorType     string         `json:"actor_type"`
	Capabilities  []string       `json:"capabilities"`
	Metadata      map[string]any `json:"metadata"`
	Status        string         `json:"status"`
	CreatedAt     string         `json:"created_at"`
	LastHeartbeat *string        `json:"last_heartbeat"`
}

type MessageEnvelope struct {
	MessageID     string `json:"message_id"`
	SourceActor   string `json:"source_actor"`
	TargetActor   string `json:"target_actor"`
	MessageType   string `json:"message_type"`
	Payload       any    `json:"payload"`
	CorrelationID string `json:"correlation_id,omitempty"`
	Timestamp     string `json:"timestamp"`
	Priority      int    `json:"priority"`
}

type DeliveryReceipt struct {
	MessageID     string `json:"message_id"`
	Status        string `json:"status"`
	DeliveredAt   string `json:"delivered_at"`
	CorrelationID string `json:"correlation_id,omitempty"`
}

type StateEntry struct {
	ActorID   string `json:"actor_id"`
	Key       string `json:"key"`
	Value     any    `json:"value"`
	Version   int    `json:"version"`
	UpdatedAt string `json:"updated_at"`
}

type EventRecord struct {
	EventID     string `json:"event_id"`
	AggregateID string `json:"aggregate_id"`
	EventType   string `json:"event_type"`
	Data        any    `json:"data"`
	Version     int    `json:"version"`
	Timestamp   string `json:"timestamp"`
}

type ServerInfo struct {
	Version      string  `json:"version"`
	Uptime       float64 `json:"uptime"`
	ActorCount   int     `json:"actor_count"`
	MessageCount int     `json:"message_count"`
}

type PubSubMessage struct {
	Topic     string            `json:"topic"`
	Payload   any               `json:"payload"`
	Headers   map[string]string `json:"headers"`
	Timestamp string            `json:"timestamp"`
	MessageID string            `json:"message_id"`
}

type SubscriptionInfo struct {
	SubscriptionID string `json:"subscription_id"`
	Topic          string `json:"topic"`
}

type PublishResult struct {
	Topic           string `json:"topic"`
	SubscriberCount int    `json:"subscriber_count"`
}

type AllStateResponse struct {
	ActorID string         `json:"actor_id"`
	State   map[string]any `json:"state"`
}

type SetStateRequest struct {
	Value   any  `json:"value"`
	Version *int `json:"version,omitempty"`
}

type AppendEventRequest struct {
	AggregateID     string `json:"aggregate_id"`
	EventType       string `json:"event_type"`
	Data            any    `json:"data"`
	ExpectedVersion *int   `json:"expected_version,omitempty"`
}

type SubscribeRequest struct {
	Topic        string `json:"topic"`
	SubscriberID string `json:"subscriber_id"`
	Filter       string `json:"filter,omitempty"`
}

type PublishRequest struct {
	Topic   string            `json:"topic"`
	Payload any               `json:"payload"`
	Headers map[string]string `json:"headers,omitempty"`
}

type AetherServerError struct {
	StatusCode int
	Detail     string
}

func (e *AetherServerError) Error() string {
	return fmt.Sprintf("HTTP %d: %s", e.StatusCode, e.Detail)
}

type Client struct {
	baseURL    string
	httpClient *http.Client
	actorID    string
}

func NewClient(baseURL string, opts ...ClientOption) *Client {
	c := &Client{
		baseURL:    strings.TrimRight(baseURL, "/"),
		httpClient: &http.Client{Timeout: 30 * time.Second},
		actorID:    "",
	}
	for _, opt := range opts {
		opt(c)
	}
	return c
}

type ClientOption func(*Client)

func WithActorID(id string) ClientOption {
	return func(c *Client) { c.actorID = id }
}

func WithTimeout(timeout time.Duration) ClientOption {
	return func(c *Client) { c.httpClient.Timeout = timeout }
}

func (c *Client) doRequest(method, path string, body any) ([]byte, int, error) {
	var reqBody io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, 0, err
		}
		reqBody = bytes.NewReader(data)
	}

	req, err := http.NewRequest(method, c.baseURL+path, reqBody)
	if err != nil {
		return nil, 0, err
	}
	if reqBody != nil {
		req.Header.Set("Content-Type", "application/json")
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, 0, err
	}
	defer resp.Body.Close()

	data, err := io.ReadAll(resp.Body)
	return data, resp.StatusCode, err
}

func (c *Client) Health() (*ServerInfo, error) {
	data, status, err := c.doRequest("GET", "/api/v1/info", nil)
	if err != nil {
		return nil, err
	}
	if status >= 400 {
		return nil, &AetherServerError{StatusCode: status, Detail: string(data)}
	}
	var info ServerInfo
	if err := json.Unmarshal(data, &info); err != nil {
		return nil, err
	}
	return &info, nil
}

func (c *Client) RegisterActor(actorID, actorType string, capabilities []string, metadata map[string]any) (*ActorInfo, error) {
	body := map[string]any{
		"actor_id":     actorID,
		"actor_type":   actorType,
		"capabilities": capabilities,
		"metadata":     metadata,
	}
	data, status, err := c.doRequest("POST", "/api/v1/actors", body)
	if err != nil {
		return nil, err
	}
	if status >= 400 {
		return nil, &AetherServerError{StatusCode: status, Detail: string(data)}
	}
	var info ActorInfo
	if err := json.Unmarshal(data, &info); err != nil {
		return nil, err
	}
	return &info, nil
}

func (c *Client) UnregisterActor(actorID string) error {
	_, status, err := c.doRequest("DELETE", "/api/v1/actors/"+url.PathEscape(actorID), nil)
	if err != nil {
		return err
	}
	if status >= 400 {
		return &AetherServerError{StatusCode: status, Detail: fmt.Sprintf("actor %s", actorID)}
	}
	return nil
}

func (c *Client) GetActor(actorID string) (*ActorInfo, error) {
	data, status, err := c.doRequest("GET", "/api/v1/actors/"+url.PathEscape(actorID), nil)
	if err != nil {
		return nil, err
	}
	if status >= 400 {
		return nil, &AetherServerError{StatusCode: status, Detail: string(data)}
	}
	var info ActorInfo
	if err := json.Unmarshal(data, &info); err != nil {
		return nil, err
	}
	return &info, nil
}

func (c *Client) ListActors(actorType, status string) ([]ActorInfo, error) {
	path := "/api/v1/actors"
	qs := url.Values{}
	if actorType != "" {
		qs.Set("type", actorType)
	}
	if status != "" {
		qs.Set("status", status)
	}
	if len(qs) > 0 {
		path += "?" + qs.Encode()
	}
	data, code, err := c.doRequest("GET", path, nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var actors []ActorInfo
	if err := json.Unmarshal(data, &actors); err != nil {
		return nil, err
	}
	return actors, nil
}

func (c *Client) Heartbeat(actorID string) error {
	_, code, err := c.doRequest("POST", "/api/v1/actors/"+url.PathEscape(actorID)+"/heartbeat", nil)
	if err != nil {
		return err
	}
	if code >= 400 {
		return &AetherServerError{StatusCode: code, Detail: string(fmt.Sprintf("actor %s", actorID))}
	}
	return nil
}

func (c *Client) SendMessage(targetActor string, envelope *MessageEnvelope) (*DeliveryReceipt, error) {
	data, code, err := c.doRequest("POST", "/api/v1/actors/"+url.PathEscape(targetActor)+"/messages", envelope)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var receipt DeliveryReceipt
	if err := json.Unmarshal(data, &receipt); err != nil {
		return nil, err
	}
	return &receipt, nil
}

func (c *Client) GetPendingMessages(actorID string) ([]MessageEnvelope, error) {
	data, code, err := c.doRequest("GET", "/api/v1/actors/"+url.PathEscape(actorID)+"/messages", nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var msgs []MessageEnvelope
	if err := json.Unmarshal(data, &msgs); err != nil {
		return nil, err
	}
	return msgs, nil
}

func (c *Client) GetState(actorID, key string) (*StateEntry, error) {
	data, code, err := c.doRequest("GET", "/api/v1/state/"+url.PathEscape(actorID)+"/"+url.PathEscape(key), nil)
	if err != nil {
		return nil, err
	}
	if code == 404 {
		return nil, nil
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var entry StateEntry
	if err := json.Unmarshal(data, &entry); err != nil {
		return nil, err
	}
	return &entry, nil
}

func (c *Client) SetState(actorID, key string, value any, version *int) (*StateEntry, error) {
	body := SetStateRequest{Value: value, Version: version}
	data, code, err := c.doRequest("PUT", "/api/v1/state/"+url.PathEscape(actorID)+"/"+url.PathEscape(key), body)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var entry StateEntry
	if err := json.Unmarshal(data, &entry); err != nil {
		return nil, err
	}
	return &entry, nil
}

func (c *Client) DeleteState(actorID, key string) error {
	_, code, err := c.doRequest("DELETE", "/api/v1/state/"+url.PathEscape(actorID)+"/"+url.PathEscape(key), nil)
	if err != nil {
		return err
	}
	if code == 404 {
		return nil
	}
	if code >= 400 {
		return &AetherServerError{StatusCode: code, Detail: fmt.Sprintf("state %s for actor %s", key, actorID)}
	}
	return nil
}

func (c *Client) GetAllState(actorID string) (*AllStateResponse, error) {
	data, code, err := c.doRequest("GET", "/api/v1/state/"+url.PathEscape(actorID), nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var resp AllStateResponse
	if err := json.Unmarshal(data, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

func (c *Client) Publish(req *PublishRequest) (*PublishResult, error) {
	data, code, err := c.doRequest("POST", "/api/v1/events/publish", req)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var result PublishResult
	if err := json.Unmarshal(data, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

func (c *Client) Subscribe(req *SubscribeRequest) (*SubscriptionInfo, error) {
	data, code, err := c.doRequest("POST", "/api/v1/events/subscribe", req)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var info SubscriptionInfo
	if err := json.Unmarshal(data, &info); err != nil {
		return nil, err
	}
	return &info, nil
}

func (c *Client) Unsubscribe(subscriptionID string) error {
	_, code, err := c.doRequest("DELETE", "/api/v1/events/subscribe/"+url.PathEscape(subscriptionID), nil)
	if err != nil {
		return err
	}
	if code >= 400 {
		return &AetherServerError{StatusCode: code, Detail: fmt.Sprintf("subscription %s", subscriptionID)}
	}
	return nil
}

func (c *Client) ListTopics() ([]string, error) {
	data, code, err := c.doRequest("GET", "/api/v1/events/topics", nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var topics []string
	if err := json.Unmarshal(data, &topics); err != nil {
		return nil, err
	}
	return topics, nil
}

func (c *Client) ListSubscribers(topic string) ([]string, error) {
	data, code, err := c.doRequest("GET", "/api/v1/events/topics/"+url.PathEscape(topic)+"/subscribers", nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var subs []string
	if err := json.Unmarshal(data, &subs); err != nil {
		return nil, err
	}
	return subs, nil
}

func (c *Client) GetTopicHistory(topic string) ([]PubSubMessage, error) {
	data, code, err := c.doRequest("GET", "/api/v1/events/topics/"+url.PathEscape(topic)+"/history", nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var msgs []PubSubMessage
	if err := json.Unmarshal(data, &msgs); err != nil {
		return nil, err
	}
	return msgs, nil
}

func (c *Client) AppendEvent(req *AppendEventRequest) (*EventRecord, error) {
	data, code, err := c.doRequest("POST", "/api/v1/events/append", req)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var event EventRecord
	if err := json.Unmarshal(data, &event); err != nil {
		return nil, err
	}
	return &event, nil
}

func (c *Client) GetEvents(aggregateID string) ([]EventRecord, error) {
	data, code, err := c.doRequest("GET", "/api/v1/events/"+url.PathEscape(aggregateID), nil)
	if err != nil {
		return nil, err
	}
	if code >= 400 {
		return nil, &AetherServerError{StatusCode: code, Detail: string(data)}
	}
	var events []EventRecord
	if err := json.Unmarshal(data, &events); err != nil {
		return nil, err
	}
	return events, nil
}
