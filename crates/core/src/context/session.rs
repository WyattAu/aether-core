//! Session Management System
//!
//! Provides conversation session persistence, branching, and replay capabilities.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::PersistentMemoryStore;
use crate::error::{Error, Result};

/// Maximum number of branches per session
const MAX_BRANCHES: usize = 50;

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: String,
    /// Role (user, assistant, system, tool)
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Optional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Parent message ID (for threading)
    pub parent_id: Option<String>,
}

impl Message {
    /// Create a new message
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content: content.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            metadata: HashMap::new(),
            parent_id: None,
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Message role in conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool result message
    Tool,
}

/// A checkpoint in session history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Checkpoint ID
    pub id: String,
    /// Checkpoint name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Message ID at checkpoint time
    pub message_id: String,
    /// Timestamp
    pub timestamp: u64,
    /// Memory snapshot path
    pub memory_snapshot: Option<String>,
}

impl Checkpoint {
    /// Create a new checkpoint
    pub fn new(name: impl Into<String>, message_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            message_id: message_id.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            memory_snapshot: None,
        }
    }
}

/// A branch from a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    /// Branch ID
    pub id: String,
    /// Branch name
    pub name: String,
    /// Source checkpoint ID
    pub source_checkpoint: String,
    /// Branch messages
    pub messages: Vec<Message>,
    /// Created timestamp
    pub created_at: u64,
    /// Is this the active branch?
    pub is_active: bool,
}

impl Branch {
    /// Create a new branch
    pub fn new(name: impl Into<String>, source_checkpoint: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            source_checkpoint: source_checkpoint.into(),
            messages: Vec::new(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            is_active: false,
        }
    }
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Session ID
    pub id: String,
    /// Session name
    pub name: String,
    /// Project path
    pub project_path: Option<String>,
    /// Created timestamp
    pub created_at: u64,
    /// Last updated timestamp
    pub updated_at: u64,
    /// Message count
    pub message_count: usize,
    /// Total tokens
    pub total_tokens: u64,
    /// Active checkpoint
    pub active_checkpoint: Option<String>,
    /// Tags
    pub tags: Vec<String>,
}

impl SessionMetadata {
    /// Create new metadata
    pub fn new(name: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            project_path: None,
            created_at: now,
            updated_at: now,
            message_count: 0,
            total_tokens: 0,
            active_checkpoint: None,
            tags: Vec::new(),
        }
    }
}

/// Serialized session data
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionData {
    metadata: SessionMetadata,
    messages: Vec<Message>,
    checkpoints: Vec<Checkpoint>,
    branches: Vec<Branch>,
    system_prompt: Option<String>,
}

/// A conversation session
pub struct Session {
    metadata: RwLock<SessionMetadata>,
    messages: RwLock<Vec<Message>>,
    checkpoints: RwLock<Vec<Checkpoint>>,
    branches: RwLock<Vec<Branch>>,
    system_prompt: RwLock<Option<String>>,
    memory: Option<Arc<PersistentMemoryStore>>,
    storage_path: PathBuf,
    auto_save: bool,
}

impl Session {
    /// Create a new session
    pub fn new(name: impl Into<String>, storage_path: impl AsRef<Path>) -> Self {
        Self::with_options(name, storage_path, None, true)
    }

    /// Create with options
    pub fn with_options(
        name: impl Into<String>,
        storage_path: impl AsRef<Path>,
        memory: Option<Arc<PersistentMemoryStore>>,
        auto_save: bool,
    ) -> Self {
        let metadata = SessionMetadata::new(name);
        let session_path = storage_path.as_ref().join(&metadata.id);
        Self {
            metadata: RwLock::new(metadata),
            messages: RwLock::new(Vec::new()),
            checkpoints: RwLock::new(Vec::new()),
            branches: RwLock::new(Vec::new()),
            system_prompt: RwLock::new(None),
            memory,
            storage_path: session_path,
            auto_save,
        }
    }

    /// Get session ID
    pub fn id(&self) -> String {
        self.metadata.read().id.clone()
    }

    /// Get session name
    pub fn name(&self) -> String {
        self.metadata.read().name.clone()
    }

    /// Get metadata
    pub fn metadata(&self) -> SessionMetadata {
        self.metadata.read().clone()
    }

    /// Set system prompt
    pub fn set_system_prompt(&self, prompt: impl Into<String>) {
        *self.system_prompt.write() = Some(prompt.into());
    }

    /// Get system prompt
    pub fn system_prompt(&self) -> Option<String> {
        self.system_prompt.read().clone()
    }

    /// Add a message
    pub fn add_message(&self, message: Message) {
        let mut messages = self.messages.write();
        if let Some(last) = messages.last() {
            let mut msg = message;
            msg.parent_id = Some(last.id.clone());
            messages.push(msg);
        } else {
            messages.push(message);
        }
        let count = messages.len();
        drop(messages);
        
        let mut meta = self.metadata.write();
        meta.message_count = count;
        meta.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        drop(meta);

        if self.auto_save {
            let _ = self.save();
        }
    }

    /// Get all messages
    pub fn messages(&self) -> Vec<Message> {
        self.messages.read().clone()
    }

    /// Get last N messages
    pub fn last_messages(&self, n: usize) -> Vec<Message> {
        let messages = self.messages.read();
        let start = messages.len().saturating_sub(n);
        messages[start..].to_vec()
    }

    /// Create a checkpoint
    pub fn create_checkpoint(&self, name: impl Into<String>) -> Result<Checkpoint> {
        let messages = self.messages.read();
        let last = messages
            .last()
            .ok_or_else(|| Error::internal("No messages to checkpoint"))?;
        
        let mut checkpoint = Checkpoint::new(name, &last.id);
        
        if let Some(memory) = &self.memory {
            let snapshot_path = self.storage_path.with_extension("memory.json");
            memory.create_snapshot(&snapshot_path)?;
            checkpoint.memory_snapshot = Some(snapshot_path.to_string_lossy().to_string());
        }
        
        self.checkpoints.write().push(checkpoint.clone());
        
        let mut meta = self.metadata.write();
        meta.active_checkpoint = Some(checkpoint.id.clone());
        meta.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        drop(meta);

        if self.auto_save {
            self.save()?;
        }
        
        Ok(checkpoint)
    }

    /// List checkpoints
    pub fn checkpoints(&self) -> Vec<Checkpoint> {
        self.checkpoints.read().clone()
    }

    /// Get checkpoint by ID
    pub fn get_checkpoint(&self, id: &str) -> Option<Checkpoint> {
        self.checkpoints.read().iter().find(|c| c.id == id).cloned()
    }

    /// Restore to checkpoint
    pub fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let checkpoint = self.get_checkpoint(checkpoint_id)
            .ok_or_else(|| Error::internal("Checkpoint not found"))?;
        
        if let (Some(memory), Some(path)) = (&self.memory, &checkpoint.memory_snapshot) {
            memory.restore_snapshot(Path::new(path))?;
        }
        
        {
            let mut messages = self.messages.write();
            let pos = messages
                .iter()
                .position(|m| m.id == checkpoint.message_id)
                .ok_or_else(|| Error::internal("Checkpoint message not found"))?;
            messages.truncate(pos + 1);
            
            let mut meta = self.metadata.write();
            meta.message_count = messages.len();
            meta.active_checkpoint = Some(checkpoint_id.to_string());
            meta.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    /// Create a branch
    pub fn create_branch(&self, name: impl Into<String>, checkpoint_id: &str) -> Result<Branch> {
        if self.branches.read().len() >= MAX_BRANCHES {
            return Err(Error::internal("Maximum branches reached"));
        }
        
        let _checkpoint = self.get_checkpoint(checkpoint_id)
            .ok_or_else(|| Error::internal("Checkpoint not found"))?;
        
        let mut branches = self.branches.write();
        for b in branches.iter_mut() {
            b.is_active = false;
        }
        
        let branch = Branch::new(name, checkpoint_id);
        branches.push(branch.clone());
        drop(branches);

        if self.auto_save {
            self.save()?;
        }
        Ok(branch)
    }

    /// List branches
    pub fn branches(&self) -> Vec<Branch> {
        self.branches.read().clone()
    }

    /// Delete a branch
    pub fn delete_branch(&self, branch_id: &str) -> Result<()> {
        let mut branches = self.branches.write();
        let pos = branches
            .iter()
            .position(|b| b.id == branch_id)
            .ok_or_else(|| Error::internal("Branch not found"))?;
        
        if branches[pos].is_active {
            return Err(Error::internal("Cannot delete active branch"));
        }
        
        branches.remove(pos);
        drop(branches);

        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    /// Replay session
    pub fn replay(&self) -> Vec<(MessageRole, String)> {
        self.messages
            .read()
            .iter()
            .map(|m| (m.role, m.content.clone()))
            .collect()
    }

    /// Export to JSON
    pub fn export(&self) -> Result<String> {
        let data = SessionData {
            metadata: self.metadata.read().clone(),
            messages: self.messages.read().clone(),
            checkpoints: self.checkpoints.read().clone(),
            branches: self.branches.read().clone(),
            system_prompt: self.system_prompt.read().clone(),
        };
        serde_json::to_string_pretty(&data)
            .map_err(|e| Error::internal(format!("Export failed: {}", e)))
    }

    /// Import from JSON
    pub fn import(&self, json: &str) -> Result<()> {
        let data: SessionData = serde_json::from_str(json)
            .map_err(|e| Error::internal(format!("Import failed: {}", e)))?;
        
        *self.messages.write() = data.messages;
        *self.checkpoints.write() = data.checkpoints;
        *self.branches.write() = data.branches;
        *self.system_prompt.write() = data.system_prompt;
        self.metadata.write().message_count = self.messages.read().len();

        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    /// Save to file
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::storage_write(format!("Failed to create directory: {}", e)))?;
        }
        
        let json = self.export()?;
        std::fs::write(&self.storage_path, json)
            .map_err(|e| Error::storage_write(format!("Failed to save: {}", e)))?;
        Ok(())
    }

    /// Load from file
    pub fn load(&self) -> Result<()> {
        let content = std::fs::read_to_string(&self.storage_path)
            .map_err(|e| Error::storage_read(format!("Failed to read: {}", e)))?;
        self.import(&content)
    }

    /// Clear session
    pub fn clear(&self) {
        *self.messages.write() = Vec::new();
        *self.checkpoints.write() = Vec::new();
        *self.branches.write() = Vec::new();
        *self.system_prompt.write() = None;
        
        let mut meta = self.metadata.write();
        meta.message_count = 0;
        meta.active_checkpoint = None;
        drop(meta);

        if self.auto_save {
            let _ = self.save();
        }
    }

    /// Update token count
    pub fn update_token_count(&self, tokens: u64) {
        self.metadata.write().total_tokens += tokens;
    }

    /// Add tag
    pub fn add_tag(&self, tag: impl Into<String>) {
        let tag = tag.into();
        let mut meta = self.metadata.write();
        if !meta.tags.contains(&tag) {
            meta.tags.push(tag);
        }
    }
}

/// Session manager
pub struct SessionManager {
    storage_dir: PathBuf,
    sessions: RwLock<HashMap<String, Arc<Session>>>,
    memory: Option<Arc<PersistentMemoryStore>>,
    auto_save: bool,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(storage_dir: impl AsRef<Path>) -> Self {
        Self {
            storage_dir: storage_dir.as_ref().to_path_buf(),
            sessions: RwLock::new(HashMap::new()),
            memory: None,
            auto_save: true,
        }
    }

    /// Create with memory store
    pub fn with_memory(storage_dir: impl AsRef<Path>, memory: Arc<PersistentMemoryStore>) -> Self {
        Self {
            storage_dir: storage_dir.as_ref().to_path_buf(),
            sessions: RwLock::new(HashMap::new()),
            memory: Some(memory),
            auto_save: true,
        }
    }

    /// Create a session
    pub fn create_session(&self, name: impl Into<String>) -> Arc<Session> {
        let session = Arc::new(Session::with_options(
            name,
            &self.storage_dir,
            self.memory.clone(),
            self.auto_save,
        ));
        let id = session.id();
        self.sessions.write().insert(id, session.clone());
        session
    }

    /// Get session by ID
    pub fn get_session(&self, id: &str) -> Option<Arc<Session>> {
        self.sessions.read().get(id).cloned()
    }

    /// Load session from storage
    pub fn load_session(&self, id: &str) -> Result<Arc<Session>> {
        let session_path = self.storage_dir.join(id).with_extension("json");
        if !session_path.exists() {
            return Err(Error::internal(format!("Session not found: {}", id)));
        }
        
        let session = Arc::new(Session::with_options(
            "loading",
            &self.storage_dir,
            self.memory.clone(),
            self.auto_save,
        ));
        session.load()?;
        
        self.sessions.write().insert(id.to_string(), session.clone());
        Ok(session)
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Result<Vec<SessionMetadata>> {
        let mut sessions = Vec::new();
        
        if !self.storage_dir.exists() {
            return Ok(sessions);
        }
        
        for entry in std::fs::read_dir(&self.storage_dir)
            .map_err(|e| Error::storage_read(format!("Failed to read directory: {}", e)))?
        {
            let entry = entry.map_err(|e| Error::storage_read(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                        sessions.push(data.metadata);
                    }
                }
            }
        }
        
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Delete a session
    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.sessions.write().remove(id);
        
        let session_path = self.storage_dir.join(id).with_extension("json");
        if session_path.exists() {
            std::fs::remove_file(&session_path)
                .map_err(|e| Error::storage_write(format!("Failed to delete: {}", e)))?;
        }
        
        let memory_path = self.storage_dir.join(id).with_extension("memory.json");
        if memory_path.exists() {
            std::fs::remove_file(&memory_path)
                .map_err(|e| Error::storage_write(format!("Failed to delete memory: {}", e)))?;
        }
        Ok(())
    }

    /// Get active count
    pub fn active_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// Close all sessions
    pub fn close_all(&self) {
        for session in self.sessions.read().values() {
            let _ = session.save();
        }
        self.sessions.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_message_creation() {
        let msg = Message::new(MessageRole::User, "Hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_session_basic() {
        let temp_dir = TempDir::new().unwrap();
        let session = Session::new("Test", temp_dir.path());
        assert!(!session.id().is_empty());
        assert_eq!(session.name(), "Test");
    }

    #[test]
    fn test_session_add_messages() {
        let temp_dir = TempDir::new().unwrap();
        let session = Session::new("Test", temp_dir.path());
        
        session.add_message(Message::new(MessageRole::User, "Hello"));
        session.add_message(Message::new(MessageRole::Assistant, "Hi"));
        
        let messages = session.messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].parent_id, Some(messages[0].id.clone()));
    }

    #[test]
    fn test_session_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let session = Session::new("Test", temp_dir.path());
        
        session.add_message(Message::new(MessageRole::User, "Msg 1"));
        let checkpoint = session.create_checkpoint("Initial").unwrap();
        
        assert_eq!(session.checkpoints().len(), 1);
        assert_eq!(session.metadata().active_checkpoint, Some(checkpoint.id));
    }

    #[test]
    fn test_session_restore_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let session = Session::new("Test", temp_dir.path());
        
        session.add_message(Message::new(MessageRole::User, "Msg 1"));
        let checkpoint = session.create_checkpoint("Initial").unwrap();
        session.add_message(Message::new(MessageRole::User, "Msg 2"));
        
        assert_eq!(session.messages().len(), 2);
        
        session.restore_checkpoint(&checkpoint.id).unwrap();
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn test_session_branch() {
        let temp_dir = TempDir::new().unwrap();
        let session = Session::new("Test", temp_dir.path());
        
        session.add_message(Message::new(MessageRole::User, "Base"));
        let checkpoint = session.create_checkpoint("Branch point").unwrap();
        
        let branch = session.create_branch("Alt", &checkpoint.id).unwrap();
        assert_eq!(branch.name, "Alt");
        assert_eq!(session.branches().len(), 1);
    }

    #[test]
    fn test_session_replay() {
        let temp_dir = TempDir::new().unwrap();
        let session = Session::new("Test", temp_dir.path());
        
        session.add_message(Message::new(MessageRole::User, "Q1"));
        session.add_message(Message::new(MessageRole::Assistant, "A1"));
        
        let replay = session.replay();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].0, MessageRole::User);
    }

    #[test]
    fn test_session_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path());
        
        let session = manager.create_session("Test");
        let id = session.id();
        assert!(manager.get_session(&id).is_some());
        assert_eq!(manager.active_count(), 1);
        
        manager.close_all();
        assert_eq!(manager.active_count(), 0);
    }
}
