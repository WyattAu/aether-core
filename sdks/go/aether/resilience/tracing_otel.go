//go:build otel

package resilience

import (
	"context"
	"fmt"
	"time"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/exporters/stdout/stdouttrace"
	"go.opentelemetry.io/otel/sdk/resource"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	semconv "go.opentelemetry.io/otel/semconv/v1.24.0"
	"go.opentelemetry.io/otel/trace"
)

var globalTracer trace.Tracer

func init() {
	TracingEnabled = false
}

type TracingContext struct {
	span       trace.Span
	ctx        context.Context
	spanName   string
	attributes map[string]interface{}
	startTime  time.Time
	enabled    bool
}

func InitTracer(serviceName string) error {
	exporter, err := stdouttrace.New(stdouttrace.WithPrettyPrint())
	if err != nil {
		return err
	}

	tp := sdktrace.NewTracerProvider(
		sdktrace.WithBatcher(exporter),
		sdktrace.WithResource(resource.NewWithAttributes(
			semconv.SchemaURL,
			semconv.ServiceNameKey.String(serviceName),
		)),
	)

	otel.SetTracerProvider(tp)
	globalTracer = tp.Tracer(serviceName)
	TracingEnabled = true
	return nil
}

func StartSpan(ctx context.Context, spanName string, attrs ...map[string]interface{}) (*TracingContext, context.Context) {
	tc := &TracingContext{
		spanName:  spanName,
		startTime: time.Now(),
		enabled:   TracingEnabled && globalTracer != nil,
	}

	if !tc.enabled {
		if len(attrs) > 0 {
			tc.attributes = attrs[0]
		}
		return tc, ctx
	}

	var span trace.Span
	tc.ctx, span = globalTracer.Start(ctx, spanName)
	tc.span = span

	if len(attrs) > 0 {
		tc.attributes = attrs[0]
		for k, v := range attrs[0] {
			span.SetAttributes(convertAttr(k, v))
		}
	}

	return tc, tc.ctx
}

func (tc *TracingContext) End(err error) {
	if !tc.enabled || tc.span == nil {
		return
	}
	if err != nil {
		tc.span.RecordError(err)
		tc.span.SetAttributes(attribute.String("error", "true"))
	}
	tc.span.End()
}

func (tc *TracingContext) SetAttribute(key string, value interface{}) {
	if tc.attributes == nil {
		tc.attributes = make(map[string]interface{})
	}
	tc.attributes[key] = value
	if tc.enabled && tc.span != nil {
		tc.span.SetAttributes(convertAttr(key, value))
	}
}

func (tc *TracingContext) AddEvent(name string, attrs ...map[string]interface{}) {
	if !tc.enabled || tc.span == nil {
		return
	}
	var otelAttrs []attribute.KeyValue
	if len(attrs) > 0 {
		for k, v := range attrs[0] {
			otelAttrs = append(otelAttrs, convertAttr(k, v))
		}
	}
	tc.span.AddEvent(name, trace.WithAttributes(otelAttrs...))
}

func convertAttr(key string, value interface{}) attribute.KeyValue {
	switch v := value.(type) {
	case bool:
		return attribute.Bool(key, v)
	case int:
		return attribute.Int64(key, int64(v))
	case int64:
		return attribute.Int64(key, v)
	case float64:
		return attribute.Float64(key, v)
	case string:
		return attribute.String(key, v)
	default:
		return attribute.String(key, fmt.Sprintf("%v", v))
	}
}
