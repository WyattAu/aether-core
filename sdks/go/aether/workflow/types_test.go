package workflow

import (
	"testing"
	"time"
)

func TestDuration_FromMillis(t *testing.T) {
	d := FromMillis(1500)
	if d.Milliseconds != 1500 {
		t.Errorf("expected 1500, got %d", d.Milliseconds)
	}
}

func TestDuration_FromSeconds(t *testing.T) {
	d := FromSeconds(2.5)
	if d.Milliseconds != 2500 {
		t.Errorf("expected 2500, got %d", d.Milliseconds)
	}
}

func TestDuration_FromMinutes(t *testing.T) {
	d := FromMinutes(1.5)
	if d.Milliseconds != 90000 {
		t.Errorf("expected 90000, got %d", d.Milliseconds)
	}
}

func TestDuration_FromHours(t *testing.T) {
	d := FromHours(2)
	if d.Milliseconds != 2*3600*1000 {
		t.Errorf("expected %d, got %d", 2*3600*1000, d.Milliseconds)
	}
}

func TestDuration_FromDays(t *testing.T) {
	d := FromDays(1)
	if d.Milliseconds != 24*3600*1000 {
		t.Errorf("expected %d, got %d", 24*3600*1000, d.Milliseconds)
	}
}

func TestDuration_FromTimeDuration(t *testing.T) {
	d := FromTimeDuration(5 * time.Second)
	if d.Milliseconds != 5000 {
		t.Errorf("expected 5000, got %d", d.Milliseconds)
	}
}

func TestDuration_ToTimeDuration(t *testing.T) {
	d := Duration{Milliseconds: 5000}
	td := d.ToTimeDuration()
	if td != 5*time.Second {
		t.Errorf("expected 5s, got %v", td)
	}
}

func TestDuration_TotalSeconds(t *testing.T) {
	d := Duration{Milliseconds: 2500}
	if d.TotalSeconds() != 2.5 {
		t.Errorf("expected 2.5, got %f", d.TotalSeconds())
	}
}

func TestDuration_Add(t *testing.T) {
	d1 := Duration{Milliseconds: 1000}
	d2 := Duration{Milliseconds: 500}
	result := d1.Add(d2)
	if result.Milliseconds != 1500 {
		t.Errorf("expected 1500, got %d", result.Milliseconds)
	}
}

func TestDuration_Sub(t *testing.T) {
	d1 := Duration{Milliseconds: 2000}
	d2 := Duration{Milliseconds: 500}
	result := d1.Sub(d2)
	if result.Milliseconds != 1500 {
		t.Errorf("expected 1500, got %d", result.Milliseconds)
	}
}

func TestSagaStatus_String(t *testing.T) {
	tests := []struct {
		s    SagaStatus
		want string
	}{
		{SagaStatusPending, "pending"},
		{SagaStatusRunning, "running"},
		{SagaStatusCompleted, "completed"},
		{SagaStatusCompensating, "compensating"},
		{SagaStatusCompensated, "compensated"},
		{SagaStatusFailed, "failed"},
		{SagaStatus(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.String(); got != tt.want {
			t.Errorf("SagaStatus(%d).String() = %q, want %q", tt.s, got, tt.want)
		}
	}
}

func TestStepStatus_String(t *testing.T) {
	tests := []struct {
		s    StepStatus
		want string
	}{
		{StepStatusPending, "pending"},
		{StepStatusRunning, "running"},
		{StepStatusCompleted, "completed"},
		{StepStatusCompensating, "compensating"},
		{StepStatusCompensated, "compensated"},
		{StepStatusFailed, "failed"},
		{StepStatusSkipped, "skipped"},
		{StepStatus(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.String(); got != tt.want {
			t.Errorf("StepStatus(%d).String() = %q, want %q", tt.s, got, tt.want)
		}
	}
}

func TestWorkflowStatus_String(t *testing.T) {
	tests := []struct {
		s    WorkflowStatus
		want string
	}{
		{WorkflowStatusCreated, "created"},
		{WorkflowStatusRunning, "running"},
		{WorkflowStatusSuspended, "suspended"},
		{WorkflowStatusCompleted, "completed"},
		{WorkflowStatusFailed, "failed"},
		{WorkflowStatusCancelled, "cancelled"},
		{WorkflowStatus(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.String(); got != tt.want {
			t.Errorf("WorkflowStatus(%d).String() = %q, want %q", tt.s, got, tt.want)
		}
	}
}

func TestTransitionStatus_String(t *testing.T) {
	tests := []struct {
		s    TransitionStatus
		want string
	}{
		{TransitionStatusPending, "pending"},
		{TransitionStatusSuccess, "success"},
		{TransitionStatusFailed, "failed"},
		{TransitionStatusRolledBack, "rolled_back"},
		{TransitionStatus(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.String(); got != tt.want {
			t.Errorf("TransitionStatus(%d).String() = %q, want %q", tt.s, got, tt.want)
		}
	}
}

func TestHumanTaskStatus_String(t *testing.T) {
	tests := []struct {
		s    HumanTaskStatus
		want string
	}{
		{HumanTaskStatusPending, "pending"},
		{HumanTaskStatusAssigned, "assigned"},
		{HumanTaskStatusInProgress, "in_progress"},
		{HumanTaskStatusCompleted, "completed"},
		{HumanTaskStatusRejected, "rejected"},
		{HumanTaskStatusTimeout, "timeout"},
		{HumanTaskStatusEscalated, "escalated"},
		{HumanTaskStatus(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.s.String(); got != tt.want {
			t.Errorf("HumanTaskStatus(%d).String() = %q, want %q", tt.s, got, tt.want)
		}
	}
}

func TestRetryPolicy_String(t *testing.T) {
	tests := []struct {
		p    RetryPolicy
		want string
	}{
		{RetryPolicyNone, "none"},
		{RetryPolicyFixed, "fixed"},
		{RetryPolicyExponential, "exponential"},
		{RetryPolicyExponentialJitter, "exponential_jitter"},
		{RetryPolicy(99), "unknown"},
	}
	for _, tt := range tests {
		if got := tt.p.String(); got != tt.want {
			t.Errorf("RetryPolicy(%d).String() = %q, want %q", tt.p, got, tt.want)
		}
	}
}

func TestDefaultRetryConfig(t *testing.T) {
	cfg := DefaultRetryConfig()
	if cfg.MaxAttempts != 3 {
		t.Errorf("expected 3, got %d", cfg.MaxAttempts)
	}
	if cfg.Policy != RetryPolicyExponential {
		t.Errorf("expected exponential, got %v", cfg.Policy)
	}
	if cfg.InitialDelay.TotalSeconds() != 1.0 {
		t.Errorf("expected 1s, got %f", cfg.InitialDelay.TotalSeconds())
	}
}

func TestSagaContext_New(t *testing.T) {
	ctx := NewSagaContext("input-data")
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
	if ctx.SagaID == "" {
		t.Error("expected non-empty saga ID")
	}
	if ctx.Input != "input-data" {
		t.Errorf("expected 'input-data', got %v", ctx.Input)
	}
	if ctx.StartedAt == nil {
		t.Error("expected started at to be set")
	}
}

func TestSagaContext_State(t *testing.T) {
	ctx := NewSagaContext(nil)
	ctx.SetState("key1", "value1")
	ctx.SetState("key2", 42)

	if ctx.GetState("key1") != "value1" {
		t.Errorf("expected 'value1', got %v", ctx.GetState("key1"))
	}
	if ctx.GetState("key2") != 42 {
		t.Errorf("expected 42, got %v", ctx.GetState("key2"))
	}
	if ctx.GetStateDefault("missing", "default") != "default" {
		t.Errorf("expected 'default', got %v", ctx.GetStateDefault("missing", "default"))
	}
	if ctx.GetStateDefault("key1", "fallback") != "value1" {
		t.Errorf("expected 'value1', got %v", ctx.GetStateDefault("key1", "fallback"))
	}
}

func TestSagaContext_StepCompletion(t *testing.T) {
	ctx := NewSagaContext(nil)
	ctx.MarkStepCompleted("step1")
	ctx.MarkStepCompleted("step2")

	if !ctx.IsStepCompleted("step1") {
		t.Error("step1 should be completed")
	}
	if !ctx.IsStepCompleted("step2") {
		t.Error("step2 should be completed")
	}
	if ctx.IsStepCompleted("step3") {
		t.Error("step3 should not be completed")
	}

	ctx.MarkStepCompleted("step1")
	if len(ctx.CompletedSteps) != 2 {
		t.Errorf("duplicate completion should not add, got %d", len(ctx.CompletedSteps))
	}
}

func TestWorkflowContext_New(t *testing.T) {
	ctx := NewWorkflowContext("test-workflow", "input")
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
	if ctx.WorkflowType != "test-workflow" {
		t.Errorf("expected 'test-workflow', got %q", ctx.WorkflowType)
	}
	if ctx.Status != WorkflowStatusCreated {
		t.Errorf("expected created status, got %v", ctx.Status)
	}
}

func TestWorkflowContext_Variables(t *testing.T) {
	ctx := NewWorkflowContext("test", nil)
	ctx.SetVariable("key1", "value1")
	ctx.SetVariable("key2", 42)

	if ctx.GetVariable("key1") != "value1" {
		t.Errorf("expected 'value1', got %v", ctx.GetVariable("key1"))
	}
	if ctx.GetVariableDefault("missing", "default") != "default" {
		t.Errorf("expected 'default', got %v", ctx.GetVariableDefault("missing", "default"))
	}
}

func TestWorkflowContext_History(t *testing.T) {
	ctx := NewWorkflowContext("test", nil)
	ctx.AddHistoryEvent("start", map[string]interface{}{"key": "val"})
	ctx.AddHistoryEvent("complete", nil)

	if len(ctx.History) != 2 {
		t.Errorf("expected 2 events, got %d", len(ctx.History))
	}
	if ctx.History[0].Type != "start" {
		t.Errorf("expected 'start', got %q", ctx.History[0].Type)
	}
	if ctx.UpdatedAt == nil {
		t.Error("updated at should be set")
	}
}

func TestHumanTaskContext_New(t *testing.T) {
	ctx := NewHumanTaskContext("approval", "Approve Request")
	if ctx == nil {
		t.Fatal("expected non-nil context")
	}
	if ctx.Title != "Approve Request" {
		t.Errorf("expected 'Approve Request', got %q", ctx.Title)
	}
	if ctx.Priority != 5 {
		t.Errorf("expected priority 5, got %d", ctx.Priority)
	}
	if ctx.Status != HumanTaskStatusPending {
		t.Errorf("expected pending status, got %v", ctx.Status)
	}
}

func TestSagaError(t *testing.T) {
	err := NewSagaError("step1", nil)
	if err.Error() == "" {
		t.Error("error message should not be empty")
	}
	if !contains(err.Error(), "step1") {
		t.Errorf("error should contain step name, got %q", err.Error())
	}

	err2 := NewSagaError("step2", &SagaError{StepName: "inner"})
	if !contains(err2.Error(), "step2") {
		t.Errorf("error should contain step name, got %q", err2.Error())
	}
}

func TestCompensationError(t *testing.T) {
	err := NewCompensationError("step1", nil)
	if !contains(err.Error(), "step1") {
		t.Errorf("error should contain step name, got %q", err.Error())
	}
}

func TestWorkflowError(t *testing.T) {
	err := NewWorkflowError("something failed")
	if err.Error() != "something failed" {
		t.Errorf("expected 'something failed', got %q", err.Error())
	}
}

func TestInvalidTransitionError(t *testing.T) {
	err := NewInvalidTransitionError("start", "end", "wf-1")
	msg := err.Error()
	if !contains(msg, "start") || !contains(msg, "end") || !contains(msg, "wf-1") {
		t.Errorf("error should contain all parts, got %q", msg)
	}
}

func TestHumanTaskError(t *testing.T) {
	err := NewHumanTaskError("task-1", "assignment failed")
	if !contains(err.Error(), "task-1") || !contains(err.Error(), "assignment failed") {
		t.Errorf("error should contain parts, got %q", err.Error())
	}
}

func TestSagaResult_Fields(t *testing.T) {
	now := time.Now()
	sr := SagaResult{
		SagaID:     "saga-1",
		Status:     SagaStatusCompleted,
		Output:     "result",
		StartedAt:  &now,
		DurationMs: 5000,
	}
	if sr.SagaID != "saga-1" {
		t.Errorf("expected 'saga-1', got %q", sr.SagaID)
	}
}

func TestWorkflowResult_Fields(t *testing.T) {
	now := time.Now()
	wr := WorkflowResult{
		WorkflowID:   "wf-1",
		Status:       WorkflowStatusCompleted,
		Output:       "result",
		StartedAt:    &now,
		DurationMs:   3000,
		CurrentState: "done",
	}
	if wr.WorkflowID != "wf-1" {
		t.Errorf("expected 'wf-1', got %q", wr.WorkflowID)
	}
}

func TestTransitionResult_Fields(t *testing.T) {
	tr := TransitionResult{
		Success:   true,
		FromState: "start",
		ToState:   "end",
		Timestamp: time.Now(),
	}
	if !tr.Success {
		t.Error("expected success")
	}
}

func TestState_Fields(t *testing.T) {
	s := State{
		Name:      "initial",
		IsInitial: true,
		IsFinal:   false,
		Metadata:  map[string]interface{}{"key": "val"},
	}
	if !s.IsInitial {
		t.Error("should be initial")
	}
}

func TestTransition_Fields(t *testing.T) {
	tr := Transition{
		Name:      "start-to-process",
		FromState: "start",
		ToState:   "process",
		Metadata:  map[string]interface{}{"auto": true},
	}
	if tr.Name != "start-to-process" {
		t.Errorf("expected 'start-to-process', got %q", tr.Name)
	}
}

func TestSagaStep_Fields(t *testing.T) {
	now := time.Now()
	step := SagaStep{
		Name:        "create-order",
		Status:      StepStatusCompleted,
		Attempts:    1,
		StartedAt:   &now,
		CompletedAt: &now,
	}
	if step.Name != "create-order" {
		t.Errorf("expected 'create-order', got %q", step.Name)
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && containsStr(s, substr)
}

func containsStr(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
