//! Actor-to-Actor AI Delegation
//!
//! Enables actors to delegate AI tasks to other actors with AI capabilities.
//! This allows for:
//! - Specialized AI actors for specific tasks
//! - Load distribution across AI-capable actors
//! - Hierarchical AI processing

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::actor::{ActorId, Message as ActorMessage, MessagePayload};
use crate::capability::CapabilitySet;
use crate::error::{Error, Result};

use super::providers::{AiProvider, CompletionRequest, CompletionResponse, Message};

/// Delegation request from one actor to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRequest {
    /// Unique request ID
    pub id: String,
    /// Source actor
    pub source_actor: ActorId,
    /// Target actor (or None for any available)
    pub target_actor: Option<ActorId>,
    /// Task type
    pub task_type: AiTaskType,
    /// Prompt/input
    pub prompt: String,
    /// Context to include
    pub context: HashMap<String, String>,
    /// Priority (0 = highest)
    pub priority: u8,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
    /// Callback actor for response
    pub callback_actor: Option<ActorId>,
}

impl DelegationRequest {
    /// Create a new delegation request
    pub fn new(source: ActorId, prompt: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_actor: source,
            target_actor: None,
            task_type: AiTaskType::General,
            prompt: prompt.into(),
            context: HashMap::new(),
            priority: 128,
            timeout_ms: 30000,
            callback_actor: None,
        }
    }

    /// Target a specific actor
    pub fn to(mut self, actor: ActorId) -> Self {
        self.target_actor = Some(actor);
        self
    }

    /// Set task type
    pub fn with_task_type(mut self, task_type: AiTaskType) -> Self {
        self.task_type = task_type;
        self
    }

    /// Add context
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set callback actor
    pub fn with_callback(mut self, actor: ActorId) -> Self {
        self.callback_actor = Some(actor);
        self
    }
}

/// Types of AI tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTaskType {
    /// General purpose
    General,
    /// Code generation/review
    Code,
    /// Natural language processing
    Nlp,
    /// Data analysis
    Analysis,
    /// Summarization
    Summarization,
    /// Translation
    Translation,
    /// Creative writing
    Creative,
    /// Embeddings
    Embedding,
}

/// Delegation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResponse {
    /// Original request ID
    pub request_id: String,
    /// Actor that processed the request
    pub processor_actor: ActorId,
    /// Response content
    pub content: String,
    /// Whether successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Processing time in ms
    pub processing_time_ms: u64,
}

/// Actor capability for AI tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorAiCapability {
    /// Actor ID
    pub actor_id: ActorId,
    /// Supported task types
    pub task_types: Vec<AiTaskType>,
    /// Current load (0-100)
    pub load: u8,
    /// Maximum concurrent tasks
    pub max_concurrent: u8,
    /// Current tasks
    pub current_tasks: u8,
    /// Average processing time (ms)
    pub avg_processing_time_ms: u64,
}

impl ActorAiCapability {
    /// Check if actor can handle a task type
    pub fn can_handle(&self, task_type: AiTaskType) -> bool {
        self.task_types.contains(&task_type) && self.current_tasks < self.max_concurrent
    }

    /// Get available capacity
    pub fn available_capacity(&self) -> u8 {
        self.max_concurrent.saturating_sub(self.current_tasks)
    }
}

/// AI Delegation manager
pub struct AiDelegationManager {
    /// Registered AI-capable actors
    capabilities: RwLock<HashMap<ActorId, ActorAiCapability>>,
    /// Pending delegations
    pending: RwLock<HashMap<String, DelegationRequest>>,
    /// Completed delegations
    completed: RwLock<HashMap<String, DelegationResponse>>,
}

impl AiDelegationManager {
    /// Create new delegation manager
    pub fn new() -> Self {
        Self {
            capabilities: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
            completed: RwLock::new(HashMap::new()),
        }
    }

    /// Register an AI-capable actor
    pub fn register(&self, capability: ActorAiCapability) {
        self.capabilities
            .write()
            .insert(capability.actor_id.clone(), capability);
    }

    /// Unregister an actor
    pub fn unregister(&self, actor_id: &ActorId) {
        self.capabilities.write().remove(actor_id);
    }

    /// Submit a delegation request
    pub fn delegate(&self, request: DelegationRequest) -> Result<String> {
        let id = request.id.clone();

        // Find a suitable actor if not specified
        let target = if let Some(target) = &request.target_actor {
            target.clone()
        } else {
            self.find_best_actor(&request.task_type)
                .ok_or_else(|| Error::internal("No available AI actor for task"))?
        };

        // Update actor load
        if let Some(cap) = self.capabilities.write().get_mut(&target) {
            cap.current_tasks += 1;
        }

        self.pending.write().insert(id.clone(), request);
        Ok(id)
    }

    /// Find the best actor for a task type
    pub fn find_best_actor(&self, task_type: &AiTaskType) -> Option<ActorId> {
        let caps = self.capabilities.read();

        caps.values()
            .filter(|c| c.can_handle(*task_type))
            .min_by(|a, b| {
                let a_score = a.load as u32 * 100 + a.current_tasks as u32;
                let b_score = b.load as u32 * 100 + b.current_tasks as u32;
                a_score.cmp(&b_score)
            })
            .map(|c| c.actor_id.clone())
    }

    /// Complete a delegation
    pub fn complete(&self, response: DelegationResponse) -> Result<()> {
        let request_id = response.request_id.clone();

        // Remove from pending
        if let Some(request) = self.pending.write().remove(&request_id) {
            // Update actor load
            if let Some(target) = request.target_actor {
                if let Some(cap) = self.capabilities.write().get_mut(&target) {
                    cap.current_tasks = cap.current_tasks.saturating_sub(1);
                }
            }
        }

        // Store completed
        self.completed.write().insert(request_id, response);
        Ok(())
    }

    /// Get delegation result
    pub fn get_result(&self, request_id: &str) -> Option<DelegationResponse> {
        self.completed.write().remove(request_id)
    }

    /// Get pending delegations for an actor
    pub fn pending_for_actor(&self, actor_id: &ActorId) -> Vec<DelegationRequest> {
        self.pending
            .read()
            .values()
            .filter(|r| r.target_actor.as_ref() == Some(actor_id))
            .cloned()
            .collect()
    }

    /// Get all AI-capable actors
    pub fn ai_actors(&self) -> Vec<ActorAiCapability> {
        self.capabilities.read().values().cloned().collect()
    }

    /// Update actor load
    pub fn update_load(&self, actor_id: &ActorId, load: u8) {
        if let Some(cap) = self.capabilities.write().get_mut(actor_id) {
            cap.load = load;
        }
    }
}

impl Default for AiDelegationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// AI Actor Processor - handles delegation requests
pub struct AiActorProcessor {
    /// Actor ID
    actor_id: ActorId,
    /// Delegation manager
    manager: Arc<AiDelegationManager>,
    /// AI Provider
    provider: Arc<dyn AiProvider>,
    /// Default model
    default_model: String,
}

impl AiActorProcessor {
    /// Create new processor
    pub fn new(
        actor_id: ActorId,
        manager: Arc<AiDelegationManager>,
        provider: Arc<dyn AiProvider>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            actor_id,
            manager,
            provider,
            default_model: default_model.into(),
        }
    }

    /// Process pending delegations
    pub async fn process_pending(&self) -> Result<Vec<DelegationResponse>> {
        let pending = self.manager.pending_for_actor(&self.actor_id);
        let mut responses = Vec::new();

        for request in pending {
            let start = std::time::Instant::now();

            // Build completion request
            let messages = vec![
                Message::system("You are a helpful AI assistant processing delegated tasks."),
                Message::user(&request.prompt),
            ];

            let completion = CompletionRequest::new(&self.default_model, messages)
                .with_max_tokens(2048);

            // Process with AI
            let result = self.provider.complete(completion).await;

            let response = match result {
                Ok(completion) => DelegationResponse {
                    request_id: request.id.clone(),
                    processor_actor: self.actor_id.clone(),
                    content: completion.content,
                    success: true,
                    error: None,
                    processing_time_ms: start.elapsed().as_millis() as u64,
                },
                Err(e) => DelegationResponse {
                    request_id: request.id.clone(),
                    processor_actor: self.actor_id.clone(),
                    content: String::new(),
                    success: false,
                    error: Some(e.to_string()),
                    processing_time_ms: start.elapsed().as_millis() as u64,
                },
            };

            self.manager.complete(response.clone())?;
            responses.push(response);
        }

        Ok(responses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegation_request_builder() {
        let actor = ActorId::new();
        let target = ActorId::new();
        let callback = ActorId::new();

        let request = DelegationRequest::new(actor, "Process this data")
            .to(target)
            .with_task_type(AiTaskType::Analysis)
            .with_context("format", "json")
            .with_priority(50)
            .with_timeout(60000)
            .with_callback(callback);

        assert!(request.target_actor.is_some());
        assert_eq!(request.task_type, AiTaskType::Analysis);
        assert_eq!(request.priority, 50);
        assert_eq!(request.timeout_ms, 60000);
    }

    #[test]
    fn test_actor_ai_capability() {
        let actor = ActorId::new();
        let cap = ActorAiCapability {
            actor_id: actor.clone(),
            task_types: vec![AiTaskType::Code, AiTaskType::Analysis],
            load: 30,
            max_concurrent: 5,
            current_tasks: 2,
            avg_processing_time_ms: 500,
        };

        assert!(cap.can_handle(AiTaskType::Code));
        assert!(cap.can_handle(AiTaskType::Analysis));
        assert!(!cap.can_handle(AiTaskType::Translation));
        assert_eq!(cap.available_capacity(), 3);
    }

    #[test]
    fn test_delegation_manager_register() {
        let manager = AiDelegationManager::new();
        let actor = ActorId::new();

        let cap = ActorAiCapability {
            actor_id: actor.clone(),
            task_types: vec![AiTaskType::General],
            load: 0,
            max_concurrent: 5,
            current_tasks: 0,
            avg_processing_time_ms: 100,
        };

        manager.register(cap);

        let actors = manager.ai_actors();
        assert_eq!(actors.len(), 1);
    }

    #[test]
    fn test_delegation_manager_find_best() {
        let manager = AiDelegationManager::new();

        // Register two actors
        let actor1 = ActorId::new();
        let cap1 = ActorAiCapability {
            actor_id: actor1.clone(),
            task_types: vec![AiTaskType::Code],
            load: 50,
            max_concurrent: 5,
            current_tasks: 3,
            avg_processing_time_ms: 200,
        };

        let actor2 = ActorId::new();
        let cap2 = ActorAiCapability {
            actor_id: actor2.clone(),
            task_types: vec![AiTaskType::Code],
            load: 20,
            max_concurrent: 5,
            current_tasks: 1,
            avg_processing_time_ms: 150,
        };

        manager.register(cap1);
        manager.register(cap2);

        // Should pick actor2 (lower load and fewer tasks)
        let best = manager.find_best_actor(&AiTaskType::Code);
        assert_eq!(best, Some(actor2));
    }

    #[test]
    fn test_delegation_flow() {
        let manager = Arc::new(AiDelegationManager::new());
        let actor = ActorId::new();

        // Register actor
        let cap = ActorAiCapability {
            actor_id: actor.clone(),
            task_types: vec![AiTaskType::General],
            load: 0,
            max_concurrent: 5,
            current_tasks: 0,
            avg_processing_time_ms: 100,
        };
        manager.register(cap);

        // Create delegation request
        let source = ActorId::new();
        let request = DelegationRequest::new(source, "Test task").to(actor.clone());

        let request_id = manager.delegate(request).unwrap();

        // Check pending
        let pending = manager.pending_for_actor(&actor);
        assert_eq!(pending.len(), 1);

        // Complete delegation
        let response = DelegationResponse {
            request_id: request_id.clone(),
            processor_actor: actor.clone(),
            content: "Task completed".to_string(),
            success: true,
            error: None,
            processing_time_ms: 50,
        };

        manager.complete(response).unwrap();

        // Get result
        let result = manager.get_result(&request_id).unwrap();
        assert!(result.success);
        assert_eq!(result.content, "Task completed");
    }

    #[test]
    fn test_capability_load_tracking() {
        let manager = AiDelegationManager::new();
        let actor = ActorId::new();

        let cap = ActorAiCapability {
            actor_id: actor.clone(),
            task_types: vec![AiTaskType::General],
            load: 0,
            max_concurrent: 3,
            current_tasks: 0,
            avg_processing_time_ms: 100,
        };
        manager.register(cap);

        // Submit delegation
        let request = DelegationRequest::new(ActorId::new(), "Task 1").to(actor.clone());
        manager.delegate(request).unwrap();

        // Check load increased
        let actors = manager.ai_actors();
        assert_eq!(actors[0].current_tasks, 1);
    }
}
