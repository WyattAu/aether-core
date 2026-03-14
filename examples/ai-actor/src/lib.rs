//! AI-Powered Actor Example
//!
//! This example demonstrates how to create an actor that uses AI capabilities
//! to process requests. It shows:
//! - Using the AI provider system from an actor
//! - Handling streaming responses
//! - Actor-to-actor AI delegation
//!
//! # Usage
//!
//! ```bash
//! # Build the actor
//! cargo build --target wasm32-unknown-unknown --release
//!
//! # Deploy to Aether
//! aether deploy ./target/wasm32-unknown-unknown/release/ai_actor.wasm
//!
//! # Call the actor
//! aether call <actor-id> Summarize '{"text": "Long text to summarize..."}'
//! ```

use aether_core::actor::{Actor, ActorContext, ActorId, Handler, Message};
use aether_core::ai::{CompletionRequest, Message as AiMessage, ProviderManager};
use aether_core::capability::CapabilitySet;
use aether_core::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// AI-powered text processing actor
pub struct AiActor {
    /// AI provider manager
    provider: Option<ProviderManager>,
    /// Actor ID
    id: ActorId,
}

impl AiActor {
    /// Create a new AI actor
    pub fn new() -> Self {
        Self {
            provider: None,
            id: ActorId::new_local("ai-actor"),
        }
    }

    /// Initialize AI providers
    async fn init_providers(&mut self) -> Result<()> {
        let manager = ProviderManager::new();
        
        // Note: In a real deployment, API keys would be injected via
        // environment variables or secret management
        // For this example, we just initialize the structure
        
        self.provider = Some(manager);
        Ok(())
    }

    /// Generate a completion using the configured AI provider
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let manager = self.provider.as_ref()
            .ok_or_else(|| Error::internal("AI provider not initialized"))?;

        let provider = manager.default().await
            .ok_or_else(|| Error::internal("No default AI provider configured"))?;

        let mut messages = Vec::new();
        
        if let Some(system_prompt) = system {
            messages.push(AiMessage::system(system_prompt));
        }
        
        messages.push(AiMessage::user(prompt));

        let request = CompletionRequest::new("gpt-4", messages)
            .with_max_tokens(1000)
            .with_temperature(0.7);

        let response = provider.complete(request).await?;
        Ok(response.content)
    }
}

impl Default for AiActor {
    fn default() -> Self {
        Self::new()
    }
}

/// Summarization request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summarize {
    /// Text to summarize
    pub text: String,
    /// Maximum summary length (words)
    pub max_words: Option<u32>,
}

/// Summarization response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Generated summary
    pub summary: String,
    /// Word count
    pub word_count: usize,
    /// Original length
    pub original_length: usize,
}

impl Message for Summarize {
    type Response = Summary;
}

#[async_trait]
impl Handler<Summarize> for AiActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: Summarize) -> Result<Summary> {
        let max_words = msg.max_words.unwrap_or(100);
        
        let system = format!(
            "You are a concise summarizer. Summarize the following text in at most {} words. \
             Focus on the key points and main ideas. Be objective and clear.",
            max_words
        );

        let summary = self.complete(&msg.text, Some(&system)).await?;
        let word_count = summary.split_whitespace().count();

        Ok(Summary {
            summary,
            word_count,
            original_length: msg.text.len(),
        })
    }
}

/// Sentiment analysis request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeSentiment {
    /// Text to analyze
    pub text: String,
}

/// Sentiment analysis response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResult {
    /// Sentiment label
    pub sentiment: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Brief explanation
    pub explanation: String,
}

impl Message for AnalyzeSentiment {
    type Response = SentimentResult;
}

#[async_trait]
impl Handler<AnalyzeSentiment> for AiActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: AnalyzeSentiment) -> Result<SentimentResult> {
        let system = r#"Analyze the sentiment of the text and respond in JSON format:
{
    "sentiment": "positive" | "negative" | "neutral",
    "confidence": 0.0-1.0,
    "explanation": "brief explanation"
}

Respond only with valid JSON."#;

        let response = self.complete(&msg.text, Some(system)).await?;
        
        // Parse the JSON response
        let result: SentimentResult = serde_json::from_str(&response)
            .map_err(|e| Error::internal(format!("Failed to parse sentiment: {}", e)))?;

        Ok(result)
    }
}

/// Code review request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCode {
    /// Code to review
    pub code: String,
    /// Programming language
    pub language: String,
}

/// Code review response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReview {
    /// Overall assessment
    pub assessment: String,
    /// Suggested improvements
    pub suggestions: Vec<String>,
    /// Potential issues
    pub issues: Vec<String>,
}

impl Message for ReviewCode {
    type Response = CodeReview;
}

#[async_trait]
impl Handler<ReviewCode> for AiActor {
    async fn handle(&mut self, _ctx: &ActorContext, msg: ReviewCode) -> Result<CodeReview> {
        let system = format!(
            r#"You are a code reviewer. Analyze the {} code and respond in JSON format:
{{
    "assessment": "overall assessment (1-2 sentences)",
    "suggestions": ["suggestion 1", "suggestion 2", ...],
    "issues": ["issue 1", "issue 2", ...]
}}

Focus on:
- Code quality and readability
- Potential bugs or security issues
- Performance considerations
- Best practices

Respond only with valid JSON."#,
            msg.language
        );

        let response = self.complete(&msg.code, Some(&system)).await?;
        
        let review: CodeReview = serde_json::from_str(&response)
            .map_err(|e| Error::internal(format!("Failed to parse review: {}", e)))?;

        Ok(review)
    }
}

#[async_trait]
impl Actor for AiActor {
    type Config = ();

    async fn on_start(&mut self, ctx: &ActorContext) -> Result<()> {
        self.id = ctx.actor_id().clone();
        
        // Initialize AI providers
        self.init_providers().await?;
        
        tracing::info!("AI Actor started with id {}", self.id);
        Ok(())
    }

    fn capabilities() -> CapabilitySet {
        // AI actors need network access to call AI APIs
        CapabilitySet::new(&["net_outbound"])
    }
}

/// Health check message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck;

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub provider_available: bool,
}

impl Message for HealthCheck {
    type Response = HealthStatus;
}

#[async_trait]
impl Handler<HealthCheck> for AiActor {
    async fn handle(&mut self, _ctx: &ActorContext, _msg: HealthCheck) -> Result<HealthStatus> {
        let provider_available = self.provider.is_some();
        
        Ok(HealthStatus {
            status: if provider_available { "healthy" } else { "degraded" }.to_string(),
            provider_available,
        })
    }
}

// WASM entry point
#[no_mangle]
pub extern "C" fn __aether_main() {
    // Actor initialization handled by Aether runtime
}
