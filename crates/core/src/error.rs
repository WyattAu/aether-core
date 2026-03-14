//! Error types for Aether Core.
//!
//! This module provides comprehensive error handling following the
//! Zero-Panic Policy (SOP-SAFE-01).
//!
//! # Error Codes
//!
//! Each error has a unique code for programmatic handling:
//! - `0x0000-0x00FF`: Configuration errors
//! - `0x0100-0x01FF`: Capability errors
//! - `0x0200-0x02FF`: WASM errors
//! - `0x0300-0x03FF`: Actor errors
//! - `0x0400-0x04FF`: Storage errors
//! - `0x0500-0x05FF`: Network/Mesh errors
//! - `0x0600-0x06FF`: Security errors
//! - `0x0700-0x07FF`: Resource errors
//! - `0xFF00-0xFFFF`: Internal errors

use std::borrow::Cow;
use std::error::Error as StdError;
use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ErrorCode {
    ConfigParse = 0x0001,
    ConfigValidation = 0x0002,
    ConfigNotFound = 0x0003,

    CapabilityDenied = 0x0100,
    CapabilityNotDeclared = 0x0101,
    CapabilityExpired = 0x0102,

    WasmCompile = 0x0200,
    WasmInstantiate = 0x0201,
    WasmInvoke = 0x0202,
    WasmFuelExhausted = 0x0203,
    WasmMemoryLimit = 0x0204,
    WasmTrap = 0x0205,

    ActorNotFound = 0x0300,
    ActorStopped = 0x0301,
    ActorSuspended = 0x0302,
    ActorFailed = 0x0303,
    ActorTimeout = 0x0304,

    StorageRead = 0x0400,
    StorageWrite = 0x0401,
    StorageConflict = 0x0402,
    StorageCorruption = 0x0403,

    MeshConnection = 0x0500,
    MeshTimeout = 0x0501,
    MeshNodeDown = 0x0502,
    MeshBackpressure = 0x0503,

    SecurityCertInvalid = 0x0600,
    SecurityCertExpired = 0x0601,
    SecurityAuthFailed = 0x0602,
    SecurityAccessDenied = 0x0603,

    ResourceMemory = 0x0700,
    ResourceCpu = 0x0701,
    ResourceFuel = 0x0702,
    ResourceHandles = 0x0703,

    SerializationFailed = 0x0800,

    InternalBug = 0xFF00,
    NotImplemented = 0xFF01,
}

impl ErrorCode {
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn category(self) -> &'static str {
        match self {
            Self::ConfigParse | Self::ConfigValidation | Self::ConfigNotFound => "config",
            Self::CapabilityDenied | Self::CapabilityNotDeclared | Self::CapabilityExpired => {
                "capability"
            }
            Self::WasmCompile
            | Self::WasmInstantiate
            | Self::WasmInvoke
            | Self::WasmFuelExhausted
            | Self::WasmMemoryLimit
            | Self::WasmTrap => "wasm",
            Self::ActorNotFound
            | Self::ActorStopped
            | Self::ActorSuspended
            | Self::ActorFailed
            | Self::ActorTimeout => "actor",
            Self::StorageRead
            | Self::StorageWrite
            | Self::StorageConflict
            | Self::StorageCorruption => "storage",
            Self::MeshConnection
            | Self::MeshTimeout
            | Self::MeshNodeDown
            | Self::MeshBackpressure => "mesh",
            Self::SecurityCertInvalid
            | Self::SecurityCertExpired
            | Self::SecurityAuthFailed
            | Self::SecurityAccessDenied => "security",
            Self::ResourceMemory
            | Self::ResourceCpu
            | Self::ResourceFuel
            | Self::ResourceHandles => "resource",
            Self::SerializationFailed => "serialization",
            Self::InternalBug | Self::NotImplemented => "internal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
    Fatal,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("[{code:?}] {message}")]
    Config {
        code: ErrorCode,
        message: Cow<'static, str>,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },

    #[error("[{code:?}] Capability {capability:?} denied for {subject}")]
    Capability {
        code: ErrorCode,
        capability: String,
        subject: String,
    },

    #[error("[{code:?}] WASM {operation} failed: {message}")]
    Wasm {
        code: ErrorCode,
        operation: Cow<'static, str>,
        message: Cow<'static, str>,
        fuel_remaining: Option<u64>,
    },

    #[error("[{code:?}] Actor {actor_id:?}: {message}")]
    Actor {
        code: ErrorCode,
        actor_id: Option<String>,
        message: Cow<'static, str>,
    },

    #[error("[{code:?}] Storage {operation}: {message}")]
    Storage {
        code: ErrorCode,
        operation: Cow<'static, str>,
        message: Cow<'static, str>,
    },

    #[error("[{code:?}] Mesh {operation}: {message}")]
    Mesh {
        code: ErrorCode,
        operation: Cow<'static, str>,
        message: Cow<'static, str>,
    },

    #[error("[{code:?}] Security {operation}: {message}")]
    Security {
        code: ErrorCode,
        operation: Cow<'static, str>,
        message: Cow<'static, str>,
    },

    #[error("[{code:?}] Resource {resource_type} exhausted: {message}")]
    Resource {
        code: ErrorCode,
        resource_type: Cow<'static, str>,
        message: Cow<'static, str>,
    },

    #[error("[{code:?}] Serialization failed: {message}")]
    Serialization {
        code: ErrorCode,
        message: Cow<'static, str>,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },

    #[error("[{code:?}] I/O error: {message}")]
    Io {
        code: ErrorCode,
        message: Cow<'static, str>,
        #[source]
        source: io::Error,
    },

    #[error("[{code:?}] Internal error: {message}")]
    Internal {
        code: ErrorCode,
        message: Cow<'static, str>,
    },
}

impl Error {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Config { code, .. } => *code,
            Self::Capability { code, .. } => *code,
            Self::Wasm { code, .. } => *code,
            Self::Actor { code, .. } => *code,
            Self::Storage { code, .. } => *code,
            Self::Mesh { code, .. } => *code,
            Self::Security { code, .. } => *code,
            Self::Resource { code, .. } => *code,
            Self::Serialization { code, .. } => *code,
            Self::Io { code, .. } => *code,
            Self::Internal { code, .. } => *code,
        }
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Capability { .. } => ErrorSeverity::Error,
            Self::Wasm {
                code: ErrorCode::WasmFuelExhausted,
                ..
            } => ErrorSeverity::Warning,
            Self::Wasm { .. } => ErrorSeverity::Error,
            Self::Storage {
                code: ErrorCode::StorageConflict,
                ..
            } => ErrorSeverity::Warning,
            Self::Storage {
                code: ErrorCode::StorageCorruption,
                ..
            } => ErrorSeverity::Critical,
            Self::Storage { .. } => ErrorSeverity::Error,
            Self::Mesh {
                code: ErrorCode::MeshTimeout,
                ..
            } => ErrorSeverity::Warning,
            Self::Mesh {
                code: ErrorCode::MeshBackpressure,
                ..
            } => ErrorSeverity::Warning,
            Self::Mesh {
                code: ErrorCode::MeshNodeDown,
                ..
            } => ErrorSeverity::Critical,
            Self::Mesh { .. } => ErrorSeverity::Error,
            Self::Security {
                code: ErrorCode::SecurityCertExpired,
                ..
            } => ErrorSeverity::Warning,
            Self::Security { .. } => ErrorSeverity::Error,
            Self::Resource { .. } => ErrorSeverity::Error,
            Self::Internal { .. } => ErrorSeverity::Critical,
            Self::Config { .. } => ErrorSeverity::Error,
            Self::Actor { .. } => ErrorSeverity::Error,
            Self::Serialization { .. } => ErrorSeverity::Error,
            Self::Io { .. } => ErrorSeverity::Error,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Mesh {
                code: ErrorCode::MeshTimeout,
                ..
            } | Self::Mesh {
                code: ErrorCode::MeshBackpressure,
                ..
            } | Self::Storage {
                code: ErrorCode::StorageConflict,
                ..
            } | Self::Wasm {
                code: ErrorCode::WasmFuelExhausted,
                ..
            }
        )
    }

    pub fn config_parse(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Config {
            code: ErrorCode::ConfigParse,
            message: message.into(),
            source: None,
        }
    }

    pub fn config_validation(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Config {
            code: ErrorCode::ConfigValidation,
            message: message.into(),
            source: None,
        }
    }

    pub fn config_not_found(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Config {
            code: ErrorCode::ConfigNotFound,
            message: message.into(),
            source: None,
        }
    }

    pub fn capability_denied(capability: impl Into<String>, subject: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityDenied,
            capability: capability.into(),
            subject: subject.into(),
        }
    }

    pub fn capability_not_declared(capability: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityNotDeclared,
            capability: capability.into(),
            subject: String::new(),
        }
    }

    pub fn capability_expired(capability: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityExpired,
            capability: capability.into(),
            subject: String::new(),
        }
    }

    pub fn wasm_compile(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmCompile,
            operation: "compile".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    pub fn wasm_instantiate(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmInstantiate,
            operation: "instantiate".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    pub fn wasm_invoke(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmInvoke,
            operation: "invoke".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    pub fn wasm_fuel_exhausted(fuel_remaining: u64) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmFuelExhausted,
            operation: "execute".into(),
            message: "fuel exhausted".into(),
            fuel_remaining: Some(fuel_remaining),
        }
    }

    pub fn wasm_memory_limit(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmMemoryLimit,
            operation: "memory".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    pub fn wasm_trap(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmTrap,
            operation: "execute".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    pub fn actor_not_found(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorNotFound,
            actor_id: Some(id.into()),
            message: "Actor not found".into(),
        }
    }

    pub fn actor_stopped(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorStopped,
            actor_id: Some(id.into()),
            message: "Actor stopped".into(),
        }
    }

    pub fn actor_suspended(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorSuspended,
            actor_id: Some(id.into()),
            message: "Actor suspended".into(),
        }
    }

    pub fn actor_failed(id: impl Into<String>, message: impl Into<Cow<'static, str>>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorFailed,
            actor_id: Some(id.into()),
            message: message.into(),
        }
    }

    pub fn actor_timeout(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorTimeout,
            actor_id: Some(id.into()),
            message: "Actor operation timed out".into(),
        }
    }

    pub fn storage_read(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Storage {
            code: ErrorCode::StorageRead,
            operation: "read".into(),
            message: message.into(),
        }
    }

    pub fn storage_write(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Storage {
            code: ErrorCode::StorageWrite,
            operation: "write".into(),
            message: message.into(),
        }
    }

    pub fn storage_conflict() -> Self {
        Self::Storage {
            code: ErrorCode::StorageConflict,
            operation: "transaction".into(),
            message: "Transaction conflict detected".into(),
        }
    }

    pub fn storage_corruption(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Storage {
            code: ErrorCode::StorageCorruption,
            operation: "integrity".into(),
            message: message.into(),
        }
    }

    pub fn mesh_connection(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshConnection,
            operation: "connect".into(),
            message: message.into(),
        }
    }

    pub fn mesh_timeout(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshTimeout,
            operation: "request".into(),
            message: message.into(),
        }
    }

    pub fn mesh_node_down(node: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshNodeDown,
            operation: "connect".into(),
            message: Cow::Owned(format!("Node {} is unavailable", node.into())),
        }
    }

    pub fn mesh_backpressure(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshBackpressure,
            operation: "send".into(),
            message: message.into(),
        }
    }

    pub fn security_cert_invalid(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityCertInvalid,
            operation: "certificate".into(),
            message: message.into(),
        }
    }

    pub fn security_cert_expired(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityCertExpired,
            operation: "certificate".into(),
            message: message.into(),
        }
    }

    pub fn security_auth_failed(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityAuthFailed,
            operation: "authenticate".into(),
            message: message.into(),
        }
    }

    pub fn security_access_denied(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityAccessDenied,
            operation: "authorize".into(),
            message: message.into(),
        }
    }

    pub fn resource_memory(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceMemory,
            resource_type: "memory".into(),
            message: message.into(),
        }
    }

    pub fn resource_cpu(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceCpu,
            resource_type: "cpu".into(),
            message: message.into(),
        }
    }

    pub fn resource_fuel(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceFuel,
            resource_type: "fuel".into(),
            message: message.into(),
        }
    }

    pub fn resource_handles(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceHandles,
            resource_type: "handles".into(),
            message: message.into(),
        }
    }

    pub fn serialization(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Serialization {
            code: ErrorCode::SerializationFailed,
            message: message.into(),
            source: None,
        }
    }

    pub fn internal(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Internal {
            code: ErrorCode::InternalBug,
            message: message.into(),
        }
    }

    pub fn not_implemented(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Internal {
            code: ErrorCode::NotImplemented,
            message: message.into(),
        }
    }

    pub fn io(source: io::Error) -> Self {
        Self::Io {
            code: ErrorCode::StorageRead,
            message: Cow::Borrowed("I/O operation failed"),
            source,
        }
    }

    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        match &mut self {
            Self::Config { source: s, .. } => *s = Some(Box::new(source)),
            Self::Serialization { source: s, .. } => *s = Some(Box::new(source)),
            _ => {}
        }
        self
    }

    pub fn config(message: impl Into<Cow<'static, str>>) -> Self {
        Self::config_parse(message)
    }

    pub fn capability_denied_simple(message: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityDenied,
            capability: message.into(),
            subject: String::new(),
        }
    }

    pub fn wasm(message: impl Into<Cow<'static, str>>) -> Self {
        Self::wasm_invoke(message)
    }

    pub fn actor(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorFailed,
            actor_id: None,
            message: message.into(),
        }
    }

    pub fn storage(message: impl Into<Cow<'static, str>>) -> Self {
        Self::storage_read(message)
    }

    pub fn security(message: impl Into<Cow<'static, str>>) -> Self {
        Self::security_auth_failed(message)
    }

    pub fn resource_exhausted(message: impl Into<Cow<'static, str>>) -> Self {
        Self::resource_memory(message)
    }

    pub fn serialization_legacy(message: impl Into<Cow<'static, str>>) -> Self {
        Self::serialization(message)
    }

    pub fn retryable(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshTimeout,
            operation: "retry".into(),
            message: message.into(),
        }
    }

    pub fn conflict() -> Self {
        Self::storage_conflict()
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io {
            code: ErrorCode::StorageRead,
            message: Cow::Borrowed("I/O operation failed"),
            source: e,
        }
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Self::Config {
            code: ErrorCode::ConfigParse,
            message: Cow::Owned(e.to_string()),
            source: None,
        }
    }
}

impl From<rkyv::rancor::Error> for Error {
    fn from(e: rkyv::rancor::Error) -> Self {
        Self::Serialization {
            code: ErrorCode::SerializationFailed,
            message: Cow::Owned(e.to_string()),
            source: None,
        }
    }
}

pub trait ErrorContext<T> {
    fn context(self, message: impl Into<Cow<'static, str>>) -> Result<T>;
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> Cow<'static, str>;
}

impl<T> ErrorContext<T> for Result<T> {
    fn context(self, message: impl Into<Cow<'static, str>>) -> Result<T> {
        self.map_err(|e| Error::Config {
            code: e.code(),
            message: message.into(),
            source: Some(Box::new(e)),
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> Cow<'static, str>,
    {
        self.map_err(|e| Error::Config {
            code: e.code(),
            message: f(),
            source: Some(Box::new(e)),
        })
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::internal(Cow::Owned(s))
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::internal(Cow::Owned(s.to_string()))
    }
}

impl From<crate::context::ContextLoadError> for Error {
    fn from(err: crate::context::ContextLoadError) -> Self {
        Self::internal(Cow::Owned(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_values() {
        assert_eq!(ErrorCode::ConfigParse.as_u16(), 0x0001);
        assert_eq!(ErrorCode::ConfigValidation.as_u16(), 0x0002);
        assert_eq!(ErrorCode::ConfigNotFound.as_u16(), 0x0003);

        assert_eq!(ErrorCode::CapabilityDenied.as_u16(), 0x0100);
        assert_eq!(ErrorCode::CapabilityNotDeclared.as_u16(), 0x0101);
        assert_eq!(ErrorCode::CapabilityExpired.as_u16(), 0x0102);

        assert_eq!(ErrorCode::WasmCompile.as_u16(), 0x0200);
        assert_eq!(ErrorCode::WasmInstantiate.as_u16(), 0x0201);
        assert_eq!(ErrorCode::WasmInvoke.as_u16(), 0x0202);
        assert_eq!(ErrorCode::WasmFuelExhausted.as_u16(), 0x0203);
        assert_eq!(ErrorCode::WasmMemoryLimit.as_u16(), 0x0204);
        assert_eq!(ErrorCode::WasmTrap.as_u16(), 0x0205);

        assert_eq!(ErrorCode::ActorNotFound.as_u16(), 0x0300);
        assert_eq!(ErrorCode::ActorStopped.as_u16(), 0x0301);
        assert_eq!(ErrorCode::ActorSuspended.as_u16(), 0x0302);
        assert_eq!(ErrorCode::ActorFailed.as_u16(), 0x0303);
        assert_eq!(ErrorCode::ActorTimeout.as_u16(), 0x0304);

        assert_eq!(ErrorCode::StorageRead.as_u16(), 0x0400);
        assert_eq!(ErrorCode::StorageWrite.as_u16(), 0x0401);
        assert_eq!(ErrorCode::StorageConflict.as_u16(), 0x0402);
        assert_eq!(ErrorCode::StorageCorruption.as_u16(), 0x0403);

        assert_eq!(ErrorCode::MeshConnection.as_u16(), 0x0500);
        assert_eq!(ErrorCode::MeshTimeout.as_u16(), 0x0501);
        assert_eq!(ErrorCode::MeshNodeDown.as_u16(), 0x0502);
        assert_eq!(ErrorCode::MeshBackpressure.as_u16(), 0x0503);

        assert_eq!(ErrorCode::SecurityCertInvalid.as_u16(), 0x0600);
        assert_eq!(ErrorCode::SecurityCertExpired.as_u16(), 0x0601);
        assert_eq!(ErrorCode::SecurityAuthFailed.as_u16(), 0x0602);
        assert_eq!(ErrorCode::SecurityAccessDenied.as_u16(), 0x0603);

        assert_eq!(ErrorCode::ResourceMemory.as_u16(), 0x0700);
        assert_eq!(ErrorCode::ResourceCpu.as_u16(), 0x0701);
        assert_eq!(ErrorCode::ResourceFuel.as_u16(), 0x0702);
        assert_eq!(ErrorCode::ResourceHandles.as_u16(), 0x0703);

        assert_eq!(ErrorCode::InternalBug.as_u16(), 0xFF00);
        assert_eq!(ErrorCode::NotImplemented.as_u16(), 0xFF01);
    }

    #[test]
    fn test_error_code_categories() {
        assert_eq!(ErrorCode::ConfigParse.category(), "config");
        assert_eq!(ErrorCode::ConfigValidation.category(), "config");
        assert_eq!(ErrorCode::ConfigNotFound.category(), "config");

        assert_eq!(ErrorCode::CapabilityDenied.category(), "capability");
        assert_eq!(ErrorCode::CapabilityNotDeclared.category(), "capability");

        assert_eq!(ErrorCode::WasmCompile.category(), "wasm");
        assert_eq!(ErrorCode::WasmTrap.category(), "wasm");

        assert_eq!(ErrorCode::ActorNotFound.category(), "actor");
        assert_eq!(ErrorCode::ActorTimeout.category(), "actor");

        assert_eq!(ErrorCode::StorageRead.category(), "storage");
        assert_eq!(ErrorCode::StorageConflict.category(), "storage");

        assert_eq!(ErrorCode::MeshConnection.category(), "mesh");
        assert_eq!(ErrorCode::MeshTimeout.category(), "mesh");

        assert_eq!(ErrorCode::SecurityCertInvalid.category(), "security");
        assert_eq!(ErrorCode::SecurityAuthFailed.category(), "security");

        assert_eq!(ErrorCode::ResourceMemory.category(), "resource");
        assert_eq!(ErrorCode::ResourceFuel.category(), "resource");

        assert_eq!(ErrorCode::InternalBug.category(), "internal");
        assert_eq!(ErrorCode::NotImplemented.category(), "internal");
    }

    #[test]
    fn test_error_severity() {
        let err = Error::config_parse("test");
        assert_eq!(err.severity(), ErrorSeverity::Error);

        let err = Error::wasm_fuel_exhausted(0);
        assert_eq!(err.severity(), ErrorSeverity::Warning);

        let err = Error::wasm_trap("test");
        assert_eq!(err.severity(), ErrorSeverity::Error);

        let err = Error::storage_conflict();
        assert_eq!(err.severity(), ErrorSeverity::Warning);

        let err = Error::storage_corruption("test");
        assert_eq!(err.severity(), ErrorSeverity::Critical);

        let err = Error::mesh_timeout("test");
        assert_eq!(err.severity(), ErrorSeverity::Warning);

        let err = Error::mesh_node_down("node-1");
        assert_eq!(err.severity(), ErrorSeverity::Critical);

        let err = Error::internal("test");
        assert_eq!(err.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_is_retryable() {
        let err = Error::mesh_timeout("test");
        assert!(err.is_retryable());

        let err = Error::mesh_backpressure("test");
        assert!(err.is_retryable());

        let err = Error::storage_conflict();
        assert!(err.is_retryable());

        let err = Error::wasm_fuel_exhausted(0);
        assert!(err.is_retryable());

        let err = Error::mesh_connection("test");
        assert!(!err.is_retryable());

        let err = Error::actor_not_found("test-id");
        assert!(!err.is_retryable());

        let err = Error::config_parse("test");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_error_constructors() {
        let err = Error::config_parse("failed to parse");
        assert_eq!(err.code(), ErrorCode::ConfigParse);
        assert!(err.to_string().contains("ConfigParse"));
        assert!(err.to_string().contains("failed to parse"));

        let err = Error::capability_denied("fs:read", "actor-123");
        assert_eq!(err.code(), ErrorCode::CapabilityDenied);
        assert!(err.to_string().contains("fs:read"));
        assert!(err.to_string().contains("actor-123"));

        let err = Error::wasm_trap("division by zero");
        assert_eq!(err.code(), ErrorCode::WasmTrap);
        assert!(err.to_string().contains("WasmTrap"));
        assert!(err.to_string().contains("division by zero"));

        let err = Error::actor_not_found("actor-456");
        assert_eq!(err.code(), ErrorCode::ActorNotFound);
        assert!(err.to_string().contains("actor-456"));

        let err = Error::storage_conflict();
        assert_eq!(err.code(), ErrorCode::StorageConflict);

        let err = Error::mesh_node_down("node-789");
        assert_eq!(err.code(), ErrorCode::MeshNodeDown);
        assert!(err.to_string().contains("node-789"));

        let err = Error::security_cert_invalid("expired cert");
        assert_eq!(err.code(), ErrorCode::SecurityCertInvalid);

        let err = Error::resource_memory("OOM");
        assert_eq!(err.code(), ErrorCode::ResourceMemory);

        let err = Error::internal("unexpected state");
        assert_eq!(err.code(), ErrorCode::InternalBug);

        let err = Error::not_implemented("feature X");
        assert_eq!(err.code(), ErrorCode::NotImplemented);
    }

    #[test]
    fn test_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert_eq!(err.code(), ErrorCode::StorageRead);
    }

    #[test]
    fn test_error_context() {
        fn inner_fn() -> Result<()> {
            Err(Error::config_parse("bad syntax"))
        }

        let result = inner_fn().context("while loading config");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), ErrorCode::ConfigParse);
    }

    #[test]
    fn test_error_display() {
        let err = Error::actor_not_found("my-actor");
        let display = err.to_string();
        assert!(display.contains("[ActorNotFound]"));
        assert!(display.contains("my-actor"));
    }

    #[test]
    fn test_cow_static_and_owned() {
        let err_static = Error::config_parse("static message");
        assert!(matches!(
            err_static,
            Error::Config {
                message: Cow::Borrowed(_),
                ..
            }
        ));

        let owned_msg = format!("dynamic: {}", 42);
        let err_owned = Error::config_parse(owned_msg);
        assert!(matches!(
            err_owned,
            Error::Config {
                message: Cow::Owned(_),
                ..
            }
        ));
    }
}
