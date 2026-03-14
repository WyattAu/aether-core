//! Trace Context Propagation
//!
//! Implements W3C TraceContext and Baggage propagation for distributed tracing.

use opentelemetry::baggage::BaggageExt;
use opentelemetry::propagation::{Extractor, Injector, TextMapPropagator};
use opentelemetry::trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState};
use opentelemetry::{Context, KeyValue};
use opentelemetry_sdk::propagation::{BaggagePropagator, TraceContextPropagator};
use std::collections::HashMap;
use std::str::FromStr;

const TRACE_PARENT_HEADER: &str = "traceparent";
const TRACE_STATE_HEADER: &str = "tracestate";
const BAGGAGE_HEADER: &str = "baggage";

#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub trace_flags: Option<String>,
    pub trace_state: Option<String>,
    pub baggage: HashMap<String, String>,
}

impl TraceContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_span_id(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = Some(span_id.into());
        self
    }

    pub fn with_trace_flags(mut self, flags: impl Into<String>) -> Self {
        self.trace_flags = Some(flags.into());
        self
    }

    pub fn with_baggage(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.baggage.insert(key.into(), value.into());
        self
    }

    pub fn to_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        if let (Some(trace_id), Some(span_id)) = (&self.trace_id, &self.span_id) {
            let flags = self.trace_flags.as_deref().unwrap_or("01");
            let traceparent = format!("00-{}-{}-{}", trace_id, span_id, flags);
            headers.insert(TRACE_PARENT_HEADER.to_string(), traceparent);
        }

        if let Some(state) = &self.trace_state {
            headers.insert(TRACE_STATE_HEADER.to_string(), state.clone());
        }

        if !self.baggage.is_empty() {
            let baggage_str = self
                .baggage
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(",");
            headers.insert(BAGGAGE_HEADER.to_string(), baggage_str);
        }

        headers
    }

    pub fn from_headers(headers: &HashMap<String, String>) -> Self {
        let mut ctx = Self::new();

        if let Some(traceparent) = headers.get(TRACE_PARENT_HEADER) {
            if let Some((trace_id, span_id, flags)) = parse_traceparent(traceparent) {
                ctx.trace_id = Some(trace_id);
                ctx.span_id = Some(span_id);
                ctx.trace_flags = Some(flags);
            }
        }

        if let Some(state) = headers.get(TRACE_STATE_HEADER) {
            ctx.trace_state = Some(state.clone());
        }

        if let Some(baggage) = headers.get(BAGGAGE_HEADER) {
            ctx.baggage = parse_baggage(baggage);
        }

        ctx
    }

    pub fn to_context(&self) -> Option<Context> {
        let trace_id = self.trace_id.as_ref()?;
        let span_id = self.span_id.as_ref()?;

        let trace_id = TraceId::from_hex(trace_id).ok()?;
        let span_id = SpanId::from_hex(span_id).ok()?;

        let flags = self
            .trace_flags
            .as_ref()
            .and_then(|f| u8::from_str_radix(f, 16).ok())
            .unwrap_or(1);

        let trace_flags = TraceFlags::new(flags);

        let trace_state = if let Some(state) = &self.trace_state {
            TraceState::from_str(state).unwrap_or_default()
        } else {
            TraceState::default()
        };

        let span_context = SpanContext::new(trace_id, span_id, trace_flags, false, trace_state);

        Some(Context::new().with_remote_span_context(span_context))
    }
}

fn parse_traceparent(traceparent: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = traceparent.split('-').collect();

    if parts.len() != 4 {
        return None;
    }

    let version = parts[0];
    if version != "00" {
        return None;
    }

    let trace_id = parts[1];
    let span_id = parts[2];
    let flags = parts[3];

    if trace_id.len() != 32 || span_id.len() != 16 || flags.len() != 2 {
        return None;
    }

    Some((trace_id.to_string(), span_id.to_string(), flags.to_string()))
}

fn parse_baggage(baggage: &str) -> HashMap<String, String> {
    baggage
        .split(',')
        .filter_map(|item| {
            let parts: Vec<&str> = item.splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
            } else {
                None
            }
        })
        .collect()
}

pub struct HeaderInjector<'a>(pub &'a mut HashMap<String, String>);

impl<'a> Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

pub struct HeaderExtractor<'a>(pub &'a HashMap<String, String>);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|v| v.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

pub fn inject_context(context: &Context, headers: &mut HashMap<String, String>) {
    let trace_propagator = TraceContextPropagator::new();
    let baggage_propagator = BaggagePropagator::new();

    trace_propagator.inject_context(context, &mut HeaderInjector(headers));
    baggage_propagator.inject_context(context, &mut HeaderInjector(headers));
}

pub fn extract_context(headers: &HashMap<String, String>) -> Context {
    let trace_propagator = TraceContextPropagator::new();
    let baggage_propagator = BaggagePropagator::new();

    let parent_context = Context::new();

    let context = trace_propagator.extract_with_context(&parent_context, &HeaderExtractor(headers));
    

    baggage_propagator.extract_with_context(&context, &HeaderExtractor(headers))
}

pub fn inject_span_context(span_context: &SpanContext, headers: &mut HashMap<String, String>) {
    let context = Context::new().with_remote_span_context(span_context.clone());
    inject_context(&context, headers);
}

pub fn extract_span_context(headers: &HashMap<String, String>) -> Option<SpanContext> {
    let context = extract_context(headers);
    if context.has_active_span() {
        Some(context.span().span_context().clone())
    } else {
        None
    }
}

pub fn baggage_from_context(context: &Context) -> HashMap<String, String> {
    let mut baggage = HashMap::new();

    for (key, (value, _metadata)) in context.baggage() {
        baggage.insert(key.to_string(), value.to_string());
    }

    baggage
}

pub fn add_baggage_to_context(context: &Context, key: &str, value: &str) -> Context {
    context.with_baggage(vec![KeyValue::new(key.to_string(), value.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context_creation() {
        let ctx = TraceContext::new()
            .with_trace_id("0af7651916cd43dd8448eb211c80319c")
            .with_span_id("b7ad6b7169203331")
            .with_trace_flags("01");

        assert_eq!(
            ctx.trace_id,
            Some("0af7651916cd43dd8448eb211c80319c".to_string())
        );
        assert_eq!(ctx.span_id, Some("b7ad6b7169203331".to_string()));
        assert_eq!(ctx.trace_flags, Some("01".to_string()));
    }

    #[test]
    fn test_trace_context_to_headers() {
        let ctx = TraceContext::new()
            .with_trace_id("0af7651916cd43dd8448eb211c80319c")
            .with_span_id("b7ad6b7169203331")
            .with_trace_flags("01")
            .with_baggage("user_id", "12345");

        let headers = ctx.to_headers();

        assert_eq!(
            headers.get("traceparent"),
            Some(&"00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string())
        );
        assert!(headers.contains_key("baggage"));
    }

    #[test]
    fn test_trace_context_from_headers() {
        let mut headers = HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string(),
        );
        headers.insert(
            "baggage".to_string(),
            "user_id=12345,request_id=abc".to_string(),
        );

        let ctx = TraceContext::from_headers(&headers);

        assert_eq!(
            ctx.trace_id,
            Some("0af7651916cd43dd8448eb211c80319c".to_string())
        );
        assert_eq!(ctx.span_id, Some("b7ad6b7169203331".to_string()));
        assert_eq!(ctx.trace_flags, Some("01".to_string()));
        assert_eq!(ctx.baggage.get("user_id"), Some(&"12345".to_string()));
        assert_eq!(ctx.baggage.get("request_id"), Some(&"abc".to_string()));
    }

    #[test]
    fn test_parse_traceparent() {
        let valid = "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        let result = parse_traceparent(valid);
        assert!(result.is_some());

        let (trace_id, span_id, flags) = result.unwrap();
        assert_eq!(trace_id, "0af7651916cd43dd8448eb211c80319c");
        assert_eq!(span_id, "b7ad6b7169203331");
        assert_eq!(flags, "01");

        let invalid_version = "01-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01";
        assert!(parse_traceparent(invalid_version).is_none());

        let invalid_format = "00-invalid-invalid-01";
        assert!(parse_traceparent(invalid_format).is_none());
    }

    #[test]
    fn test_parse_baggage() {
        let baggage = "user_id=12345,request_id=abc,session=xyz";
        let result = parse_baggage(baggage);

        assert_eq!(result.get("user_id"), Some(&"12345".to_string()));
        assert_eq!(result.get("request_id"), Some(&"abc".to_string()));
        assert_eq!(result.get("session"), Some(&"xyz".to_string()));
    }

    #[test]
    fn test_inject_extract_roundtrip() {
        let ctx = TraceContext::new()
            .with_trace_id("0af7651916cd43dd8448eb211c80319c")
            .with_span_id("b7ad6b7169203331")
            .with_trace_flags("01");

        let headers = ctx.to_headers();
        let extracted = TraceContext::from_headers(&headers);

        assert_eq!(extracted.trace_id, ctx.trace_id);
        assert_eq!(extracted.span_id, ctx.span_id);
        assert_eq!(extracted.trace_flags, ctx.trace_flags);
    }

    #[test]
    fn test_header_injector_extractor() {
        let mut headers = HashMap::new();
        let mut injector = HeaderInjector(&mut headers);

        injector.set("test-key", "test-value".to_string());
        assert_eq!(headers.get("test-key"), Some(&"test-value".to_string()));

        let extractor = HeaderExtractor(&headers);
        assert_eq!(extractor.get("test-key"), Some("test-value"));
        assert!(extractor.keys().contains(&"test-key"));
    }
}
