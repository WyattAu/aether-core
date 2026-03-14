//! Context Loading System
//!
//! Provides functionality for loading and managing project context files
//! such as AETHER.md and MEMORY.md that provide AI assistants
//! with project-specific knowledge and conventions.

mod loader;
mod memory;
mod persistent_memory;
mod session;

pub use loader::{ContextConfig, ContextFile, ContextLoadError, ContextLoader, ContextSection};
pub use memory::{
    MemoryEntry, MemoryStats, MemoryStore, MAX_ENTRIES, MAX_ENTRY_AGE, MAX_TOTAL_SIZE,
};
pub use persistent_memory::{PersistentMemoryEntry, PersistentMemoryStore};
pub use session::{
    Branch, Checkpoint, Message, MessageRole, Session, SessionManager, SessionMetadata,
};
