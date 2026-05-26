//! AI Integration Module
//!
//! Provides multi-provider AI support for Aether actors:
//! - OpenAI (GPT-4, GPT-3.5)
//! - Anthropic (Claude)
//! - Ollama (Local LLMs)
//! - Custom providers
//!
//! # Example
//!
//! ```ignore
//! use aether_core::ai::{ProviderManager, CompletionRequest, Message};
//!
//! #[tokio::main]
//! async fn main() {
//!     let manager = ProviderManager::new();
//!     
//!     // Register providers (configured via environment or config)
//!     // manager.register("openai", openai_provider).await;
//!     
//!     let provider = manager.default().await.unwrap();
//!     
//!     let request = CompletionRequest::new(
//!         "gpt-4",
//!         vec![Message::user("Hello!")]
//!     );
//!     
//!     let response = provider.complete(request).await.unwrap();
//!     println!("{}", response.content);
//! }
//! ```

pub mod autonomous;
pub mod content_policy;
pub mod delegation;
pub mod model_serving;
pub mod providers;
pub mod semantic_routing;
pub mod streaming;
pub mod token_budget;

pub use autonomous::*;
pub use content_policy::*;
pub use delegation::*;
pub use model_serving::*;
pub use providers::*;
pub use semantic_routing::*;
pub use streaming::*;
pub use token_budget::*;
