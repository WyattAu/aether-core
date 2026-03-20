// Package workflow provides workflow engine types for the Aether SDK.
// This includes saga patterns, state machines, and human task integration.
package workflow

import (
	"fmt"
	"time"
)

// ============================================
// Duration Type
// ============================================

// Duration represents a duration of time in milliseconds.
type Duration struct {
	Milliseconds int64
}

// FromMillis creates a Duration from milliseconds.
func FromMillis(ms int64) Duration {
	return Duration{Milliseconds: ms}
}

// FromSeconds creates a Duration from seconds.
func FromSeconds(s float64) Duration {
	return Duration{Milliseconds: int64(s * 1000)}
}

// FromMinutes creates a Duration from minutes.
func FromMinutes(m float64) Duration {
	return Duration{Milliseconds: int64(m * 60 * 1000)}
}

// FromHours creates a Duration from hours.
func FromHours(h float64) Duration {
	return Duration{Milliseconds: int64(h * 3600 * 1000)}
}

// FromDays creates a Duration from days.
func FromDays(d float64) Duration {
	return Duration{Milliseconds: int64(d * 24 * 3600 * 1000)}
}

// FromTimeDuration creates a Duration from time.Duration.
func FromTimeDuration(d time.Duration) Duration {
	return Duration{Milliseconds: d.Milliseconds()}
}

// ToTimeDuration converts Duration to time.Duration.
func (d Duration) ToTimeDuration() time.Duration {
	return time.Duration(d.Milliseconds) * time.Millisecond
}

// TotalSeconds returns the total duration in seconds.
func (d Duration) TotalSeconds() float64 {
	return float64(d.Milliseconds) / 1000
}

// Add adds another duration.
func (d Duration) Add(other Duration) Duration {
	return Duration{Milliseconds: d.Milliseconds + other.Milliseconds}
}

// Sub subtracts another duration.
func (d Duration) Sub(other Duration) Duration {
	return Duration{Milliseconds: d.Milliseconds - other.Milliseconds}
}

// ============================================
// Enums
// ============================================

// SagaStatus represents the status of a saga execution.
type SagaStatus int

const (
	SagaStatusPending SagaStatus = iota
	SagaStatusRunning
	SagaStatusCompleted
	SagaStatusCompensating
	SagaStatusCompensated
	SagaStatusFailed
)

func (s SagaStatus) String() string {
	switch s {
	case SagaStatusPending:
		return "pending"
	case SagaStatusRunning:
		return "running"
	case SagaStatusCompleted:
		return "completed"
	case SagaStatusCompensating:
		return "compensating"
	case SagaStatusCompensated:
		return "compensated"
	case SagaStatusFailed:
		return "failed"
	default:
		return "unknown"
	}
}

// StepStatus represents the status of a saga step.
type StepStatus int

const (
	StepStatusPending StepStatus = iota
	StepStatusRunning
	StepStatusCompleted
	StepStatusCompensating
	StepStatusCompensated
	StepStatusFailed
	StepStatusSkipped
)

func (s StepStatus) String() string {
	switch s {
	case StepStatusPending:
		return "pending"
	case StepStatusRunning:
		return "running"
	case StepStatusCompleted:
		return "completed"
	case StepStatusCompensating:
		return "compensating"
	case StepStatusCompensated:
		return "compensated"
	case StepStatusFailed:
		return "failed"
	case StepStatusSkipped:
		return "skipped"
	default:
		return "unknown"
	}
}

// WorkflowStatus represents the status of a workflow execution.
type WorkflowStatus int

const (
	WorkflowStatusCreated WorkflowStatus = iota
	WorkflowStatusRunning
	WorkflowStatusSuspended
	WorkflowStatusCompleted
	WorkflowStatusFailed
	WorkflowStatusCancelled
)

func (s WorkflowStatus) String() string {
	switch s {
	case WorkflowStatusCreated:
		return "created"
	case WorkflowStatusRunning:
		return "running"
	case WorkflowStatusSuspended:
		return "suspended"
	case WorkflowStatusCompleted:
		return "completed"
	case WorkflowStatusFailed:
		return "failed"
	case WorkflowStatusCancelled:
		return "cancelled"
	default:
		return "unknown"
	}
}

// TransitionStatus represents the status of a state transition.
type TransitionStatus int

const (
	TransitionStatusPending TransitionStatus = iota
	TransitionStatusSuccess
	TransitionStatusFailed
	TransitionStatusRolledBack
)

func (s TransitionStatus) String() string {
	switch s {
	case TransitionStatusPending:
		return "pending"
	case TransitionStatusSuccess:
		return "success"
	case TransitionStatusFailed:
		return "failed"
	case TransitionStatusRolledBack:
		return "rolled_back"
	default:
		return "unknown"
	}
}

// HumanTaskStatus represents the status of a human task.
type HumanTaskStatus int

const (
	HumanTaskStatusPending HumanTaskStatus = iota
	HumanTaskStatusAssigned
	HumanTaskStatusInProgress
	HumanTaskStatusCompleted
	HumanTaskStatusRejected
	HumanTaskStatusTimeout
	HumanTaskStatusEscalated
)

func (s HumanTaskStatus) String() string {
	switch s {
	case HumanTaskStatusPending:
		return "pending"
	case HumanTaskStatusAssigned:
		return "assigned"
	case HumanTaskStatusInProgress:
		return "in_progress"
	case HumanTaskStatusCompleted:
		return "completed"
	case HumanTaskStatusRejected:
		return "rejected"
	case HumanTaskStatusTimeout:
		return "timeout"
	case HumanTaskStatusEscalated:
		return "escalated"
	default:
		return "unknown"
	}
}

// RetryPolicy represents the retry policy for saga steps.
type RetryPolicy int

const (
	RetryPolicyNone RetryPolicy = iota
	RetryPolicyFixed
	RetryPolicyExponential
	RetryPolicyExponentialJitter
)

func (p RetryPolicy) String() string {
	switch p {
	case RetryPolicyNone:
		return "none"
	case RetryPolicyFixed:
		return "fixed"
	case RetryPolicyExponential:
		return "exponential"
	case RetryPolicyExponentialJitter:
		return "exponential_jitter"
	default:
		return "unknown"
	}
}

// ============================================
// Configuration Types
// ============================================

// RetryConfig represents configuration for retry behavior.
type RetryConfig struct {
	MaxAttempts   int
	Policy        RetryPolicy
	InitialDelay  Duration
	MaxDelay      Duration
	Multiplier    float64
	Jitter        float64
}

// DefaultRetryConfig returns the default retry configuration.
func DefaultRetryConfig() RetryConfig {
	return RetryConfig{
		MaxAttempts:   3,
		Policy:        RetryPolicyExponential,
		InitialDelay:  FromSeconds(1),
		MaxDelay:      FromSeconds(60),
		Multiplier:    2.0,
		Jitter:        0.1,
	}
}

// ============================================
// Context Types
// ============================================

// SagaContext represents the context passed through saga execution.
type SagaContext struct {
	SagaID         string
	Input          interface{}
	State          map[string]interface{}
	CompletedSteps []string
	FailedStep     string
	Error          string
	StartedAt      *time.Time
	CompletedAt    *time.Time
	Metadata       map[string]interface{}
}

// NewSagaContext creates a new SagaContext.
func NewSagaContext(input interface{}) *SagaContext {
	now := time.Now()
	return &SagaContext{
		SagaID:         generateID(),
		Input:          input,
		State:          make(map[string]interface{}),
		CompletedSteps: []string{},
		Metadata:       make(map[string]interface{}),
		StartedAt:      &now,
	}
}

// SetState sets a state value.
func (c *SagaContext) SetState(key string, value interface{}) {
	c.State[key] = value
}

// GetState gets a state value.
func (c *SagaContext) GetState(key string) interface{} {
	return c.State[key]
}

// GetStateDefault gets a state value with a default.
func (c *SagaContext) GetStateDefault(key string, defaultValue interface{}) interface{} {
	if v, ok := c.State[key]; ok {
		return v
	}
	return defaultValue
}

// MarkStepCompleted marks a step as completed.
func (c *SagaContext) MarkStepCompleted(stepName string) {
	for _, s := range c.CompletedSteps {
		if s == stepName {
			return
		}
	}
	c.CompletedSteps = append(c.CompletedSteps, stepName)
}

// IsStepCompleted checks if a step has been completed.
func (c *SagaContext) IsStepCompleted(stepName string) bool {
	for _, s := range c.CompletedSteps {
		if s == stepName {
			return true
		}
	}
	return false
}

// WorkflowContext represents the context passed through workflow execution.
type WorkflowContext struct {
	WorkflowID   string
	WorkflowType string
	CurrentState string
	Input        interface{}
	Variables    map[string]interface{}
	History      []HistoryEvent
	StartedAt    *time.Time
	UpdatedAt    *time.Time
	Metadata     map[string]interface{}
	Status       WorkflowStatus
}

// HistoryEvent represents an event in workflow history.
type HistoryEvent struct {
	Type      string
	Timestamp time.Time
	Details   map[string]interface{}
}

// NewWorkflowContext creates a new WorkflowContext.
func NewWorkflowContext(workflowType string, input interface{}) *WorkflowContext {
	now := time.Now()
	return &WorkflowContext{
		WorkflowID:   generateID(),
		WorkflowType: workflowType,
		Input:        input,
		Variables:    make(map[string]interface{}),
		History:      []HistoryEvent{},
		Metadata:     make(map[string]interface{}),
		StartedAt:    &now,
		UpdatedAt:    &now,
		Status:       WorkflowStatusCreated,
	}
}

// SetVariable sets a workflow variable.
func (c *WorkflowContext) SetVariable(key string, value interface{}) {
	c.Variables[key] = value
}

// GetVariable gets a workflow variable.
func (c *WorkflowContext) GetVariable(key string) interface{} {
	return c.Variables[key]
}

// GetVariableDefault gets a workflow variable with a default.
func (c *WorkflowContext) GetVariableDefault(key string, defaultValue interface{}) interface{} {
	if v, ok := c.Variables[key]; ok {
		return v
	}
	return defaultValue
}

// AddHistoryEvent adds an event to the history.
func (c *WorkflowContext) AddHistoryEvent(eventType string, details map[string]interface{}) {
	c.History = append(c.History, HistoryEvent{
		Type:      eventType,
		Timestamp: time.Now(),
		Details:   details,
	})
	c.UpdatedAt = ptrTime(time.Now())
}

// HumanTaskContext represents the context for a human task.
type HumanTaskContext struct {
	TaskID          string
	TaskType        string
	WorkflowID      string
	StepName        string
	Title           string
	Description     string
	Assignee        string
	CandidateUsers  []string
	CandidateGroups []string
	Priority        int
	DueDate         *time.Time
	FormData        map[string]interface{}
	Result          map[string]interface{}
	Status          HumanTaskStatus
	CreatedAt       time.Time
	UpdatedAt       *time.Time
	CompletedAt     *time.Time
	CompletedBy     string
	Metadata        map[string]interface{}
}

// NewHumanTaskContext creates a new HumanTaskContext.
func NewHumanTaskContext(taskType, title string) *HumanTaskContext {
	return &HumanTaskContext{
		TaskID:     generateID(),
		TaskType:   taskType,
		Title:      title,
		Priority:   5,
		FormData:   make(map[string]interface{}),
		Status:     HumanTaskStatusPending,
		CreatedAt:  time.Now(),
		Metadata:   make(map[string]interface{}),
	}
}

// ============================================
// Result Types
// ============================================

// SagaResult represents the result of a saga execution.
type SagaResult struct {
	SagaID          string
	Status          SagaStatus
	Output          interface{}
	Error           string
	CompletedSteps  []string
	CompensatedSteps []string
	StartedAt       *time.Time
	CompletedAt     *time.Time
	DurationMs      int64
}

// WorkflowResult represents the result of a workflow execution.
type WorkflowResult struct {
	WorkflowID   string
	Status       WorkflowStatus
	Output       interface{}
	Error        string
	CurrentState string
	History      []HistoryEvent
	StartedAt    *time.Time
	CompletedAt  *time.Time
	DurationMs   int64
}

// TransitionResult represents the result of a state transition.
type TransitionResult struct {
	Success   bool
	FromState string
	ToState   string
	Error     string
	Timestamp time.Time
}

// ============================================
// State and Transition Definitions
// ============================================

// State represents a state in the workflow state machine.
type State struct {
	Name             string
	IsInitial        bool
	IsFinal          bool
	Timeout          *Duration
	TimeoutTransition string
	Metadata         map[string]interface{}
}

// Transition represents a transition between states.
type Transition struct {
	Name      string
	FromState string
	ToState   string
	Metadata  map[string]interface{}
}

// SagaStep represents a single step in a saga.
type SagaStep struct {
	Name        string
	Status      StepStatus
	Attempts    int
	Error       string
	StartedAt   *time.Time
	CompletedAt *time.Time
	Timeout     *Duration
	RetryConfig *RetryConfig
}

// ============================================
// Errors
// ============================================

// SagaError represents a saga-related error.
type SagaError struct {
	StepName string
	Cause    error
}

func (e *SagaError) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("saga step '%s' failed: %v", e.StepName, e.Cause)
	}
	return fmt.Sprintf("saga step '%s' failed", e.StepName)
}

// NewSagaError creates a new SagaError.
func NewSagaError(stepName string, cause error) *SagaError {
	return &SagaError{StepName: stepName, Cause: cause}
}

// CompensationError represents a compensation failure.
type CompensationError struct {
	StepName string
	Cause    error
}

func (e *CompensationError) Error() string {
	if e.Cause != nil {
		return fmt.Sprintf("compensation for '%s' failed: %v", e.StepName, e.Cause)
	}
	return fmt.Sprintf("compensation for '%s' failed", e.StepName)
}

// NewCompensationError creates a new CompensationError.
func NewCompensationError(stepName string, cause error) *CompensationError {
	return &CompensationError{StepName: stepName, Cause: cause}
}

// WorkflowError represents a workflow-related error.
type WorkflowError struct {
	Message string
}

func (e *WorkflowError) Error() string {
	return e.Message
}

// NewWorkflowError creates a new WorkflowError.
func NewWorkflowError(message string) *WorkflowError {
	return &WorkflowError{Message: message}
}

// InvalidTransitionError represents an invalid state transition.
type InvalidTransitionError struct {
	FromState  string
	ToState    string
	WorkflowID string
}

func (e *InvalidTransitionError) Error() string {
	return fmt.Sprintf("invalid transition from '%s' to '%s' in workflow %s", 
		e.FromState, e.ToState, e.WorkflowID)
}

// NewInvalidTransitionError creates a new InvalidTransitionError.
func NewInvalidTransitionError(fromState, toState, workflowID string) *InvalidTransitionError {
	return &InvalidTransitionError{
		FromState:  fromState,
		ToState:    toState,
		WorkflowID: workflowID,
	}
}

// HumanTaskError represents a human task-related error.
type HumanTaskError struct {
	TaskID  string
	Message string
}

func (e *HumanTaskError) Error() string {
	return fmt.Sprintf("human task %s: %s", e.TaskID, e.Message)
}

// NewHumanTaskError creates a new HumanTaskError.
func NewHumanTaskError(taskID, message string) *HumanTaskError {
	return &HumanTaskError{TaskID: taskID, Message: message}
}

// ============================================
// Helper Functions
// ============================================

func generateID() string {
	return fmt.Sprintf("%d", time.Now().UnixNano())
}

func ptrTime(t time.Time) *time.Time {
	return &t
}
