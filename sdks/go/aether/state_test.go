package aether

import (
	"context"
	"testing"
)

func TestNewStateHandle(t *testing.T) {
	sh := NewStateHandle()
	if sh == nil {
		t.Fatal("expected non-nil StateHandle")
	}
	if sh.store == nil {
		t.Error("expected initialized store")
	}
}

func TestStateHandle_WriteAndRead(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	err := sh.Write(ctx, "key1", []byte("value1"))
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	val, err := sh.Read(ctx, "key1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if string(val) != "value1" {
		t.Errorf("expected 'value1', got %q", string(val))
	}
}

func TestStateHandle_Read_NonexistentKey(t *testing.T) {
	sh := NewStateHandle()
	val, err := sh.Read(context.Background(), "missing")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if val != nil {
		t.Errorf("expected nil for missing key, got %v", val)
	}
}

func TestStateHandle_Write_Overwrite(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	sh.Write(ctx, "key", []byte("v1"))
	sh.Write(ctx, "key", []byte("v2"))

	val, _ := sh.Read(ctx, "key")
	if string(val) != "v2" {
		t.Errorf("expected 'v2', got %q", string(val))
	}
}

func TestStateHandle_Write_NilValue(t *testing.T) {
	sh := NewStateHandle()
	err := sh.Write(context.Background(), "key", nil)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	val, _ := sh.Read(context.Background(), "key")
	if val != nil {
		t.Errorf("expected nil, got %v", val)
	}
}

func TestStateHandle_Delete(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	sh.Write(ctx, "key", []byte("value"))
	sh.Delete(ctx, "key")

	val, _ := sh.Read(ctx, "key")
	if val != nil {
		t.Errorf("expected nil after delete, got %v", val)
	}
}

func TestStateHandle_Delete_Nonexistent(t *testing.T) {
	sh := NewStateHandle()
	err := sh.Delete(context.Background(), "missing")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestStateHandle_Exists(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	exists, _ := sh.Exists(ctx, "key")
	if exists {
		t.Error("should not exist before write")
	}

	sh.Write(ctx, "key", []byte("val"))
	exists, _ = sh.Exists(ctx, "key")
	if !exists {
		t.Error("should exist after write")
	}
}

func TestStateHandle_ListKeys(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	sh.Write(ctx, "user:1", []byte("a"))
	sh.Write(ctx, "user:2", []byte("b"))
	sh.Write(ctx, "order:1", []byte("c"))

	keys, err := sh.ListKeys(ctx, "user:")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
	if len(keys) != 2 {
		t.Errorf("expected 2 keys with prefix 'user:', got %d", len(keys))
	}

	keys, _ = sh.ListKeys(ctx, "order:")
	if len(keys) != 1 {
		t.Errorf("expected 1 key with prefix 'order:', got %d", len(keys))
	}

	keys, _ = sh.ListKeys(ctx, "nonexistent:")
	if len(keys) != 0 {
		t.Errorf("expected 0 keys, got %d", len(keys))
	}
}

func TestStateHandle_ListKeys_EmptyPrefix(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	sh.Write(ctx, "a", []byte("1"))
	sh.Write(ctx, "b", []byte("2"))

	keys, _ := sh.ListKeys(ctx, "")
	if len(keys) != 2 {
		t.Errorf("expected 2 keys with empty prefix, got %d", len(keys))
	}
}

func TestStateHandle_Clear(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	sh.Write(ctx, "k1", []byte("v1"))
	sh.Write(ctx, "k2", []byte("v2"))

	sh.Clear(ctx)

	keys, _ := sh.ListKeys(ctx, "")
	if len(keys) != 0 {
		t.Errorf("expected 0 keys after clear, got %d", len(keys))
	}
}

func TestStateHandle_Clear_Empty(t *testing.T) {
	sh := NewStateHandle()
	err := sh.Clear(context.Background())
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestStateHandle_ReadIsolation(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	original := []byte("original")
	sh.Write(ctx, "key", original)

	val, _ := sh.Read(ctx, "key")
	val[0] = 'X'

	val2, _ := sh.Read(ctx, "key")
	if string(val2) == "Xriginal" {
		t.Error("mutation of returned slice should not affect stored value")
	}
}

func TestStateHandle_WriteIsolation(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	data := []byte("mutable")
	sh.Write(ctx, "key", data)
	data[0] = 'X'

	val, _ := sh.Read(ctx, "key")
	if string(val) == "Xutable" {
		t.Error("mutation of input slice should not affect stored value")
	}
}

func TestStateHandle_EmptyByteArray(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()

	sh.Write(ctx, "key", []byte{})

	val, _ := sh.Read(ctx, "key")
	if val == nil {
		t.Error("expected empty byte array, not nil")
	}
	if len(val) != 0 {
		t.Errorf("expected empty byte array, got %d bytes", len(val))
	}
}

func TestStateHandle_ConcurrentAccess(t *testing.T) {
	sh := NewStateHandle()
	ctx := context.Background()
	done := make(chan struct{})

	go func() {
		defer close(done)
		for i := 0; i < 100; i++ {
			sh.Write(ctx, "key", []byte("val"))
			sh.Read(ctx, "key")
			sh.Exists(ctx, "key")
			sh.Delete(ctx, "key")
		}
	}()

	for i := 0; i < 100; i++ {
		sh.Write(ctx, "key", []byte("val"))
		sh.Read(ctx, "key")
	}
	<-done
}
