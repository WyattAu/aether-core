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

/// A specialized `Result` type using [`Error`] as the error variant.
pub type Result<T> = std::result::Result<T, Error>;

/// Unique error codes for programmatic error classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ErrorCode {
    /// Failed to parse a configuration file or value.
    ConfigParse = 0x0001,
    /// Configuration value failed validation constraints.
    ConfigValidation = 0x0002,
    /// Required configuration file or key was not found.
    ConfigNotFound = 0x0003,

    /// A required capability was denied to the requesting actor.
    CapabilityDenied = 0x0100,
    /// The requested capability was not declared in the actor's manifest.
    CapabilityNotDeclared = 0x0101,
    /// The capability grant has expired.
    CapabilityExpired = 0x0102,

    /// WASM module compilation failed.
    WasmCompile = 0x0200,
    /// WASM module instantiation failed.
    WasmInstantiate = 0x0201,
    /// WASM function invocation failed.
    WasmInvoke = 0x0202,
    /// WASM execution exhausted its fuel allocation.
    WasmFuelExhausted = 0x0203,
    /// WASM module exceeded its memory limit.
    WasmMemoryLimit = 0x0204,
    /// WASM execution trapped (e.g., division by zero, out-of-bounds access).
    WasmTrap = 0x0205,

    /// The requested actor was not found on this node.
    ActorNotFound = 0x0300,
    /// The actor has been stopped and cannot process messages.
    ActorStopped = 0x0301,
    /// The actor is suspended and not currently processing messages.
    ActorSuspended = 0x0302,
    /// The actor encountered a fatal runtime error.
    ActorFailed = 0x0303,
    /// An actor operation exceeded its timeout.
    ActorTimeout = 0x0304,

    /// A storage read operation failed.
    StorageRead = 0x0400,
    /// A storage write operation failed.
    StorageWrite = 0x0401,
    /// A transactional storage conflict was detected (optimistic concurrency).
    StorageConflict = 0x0402,
    /// Stored data failed integrity verification.
    StorageCorruption = 0x0403,

    /// Failed to establish a mesh network connection.
    MeshConnection = 0x0500,
    /// A mesh network request timed out.
    MeshTimeout = 0x0501,
    /// A mesh node is down or unreachable.
    MeshNodeDown = 0x0502,
    /// Mesh backpressure: the target node cannot accept more messages.
    MeshBackpressure = 0x0503,

    /// A certificate is invalid (e.g., malformed, wrong issuer).
    SecurityCertInvalid = 0x0600,
    /// A certificate has expired.
    SecurityCertExpired = 0x0601,
    /// Authentication failed (e.g., bad credentials, invalid token).
    SecurityAuthFailed = 0x0602,
    /// The caller is not authorized for the requested operation.
    SecurityAccessDenied = 0x0603,

    /// Memory resource limit exceeded.
    ResourceMemory = 0x0700,
    /// CPU resource limit exceeded.
    ResourceCpu = 0x0701,
    /// Fuel (execution metering) resource limit exceeded.
    ResourceFuel = 0x0702,
    /// Maximum number of resource handles (file descriptors, etc.) exceeded.
    ResourceHandles = 0x0703,

    /// Data serialization or deserialization failed.
    SerializationFailed = 0x0800,

    /// An internal bug was detected; this should never happen.
    InternalBug = 0xFF00,
    /// The requested feature or operation is not yet implemented.
    NotImplemented = 0xFF01,
}

impl ErrorCode {
    /// Returns the raw `u16` value of this error code.
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    /// Returns the category name for this error code (e.g., `"config"`, `"wasm"`).
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

/// Severity level for an error, used for logging and alerting decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// A potentially recoverable issue that may warrant attention.
    Warning,
    /// A standard error that prevents the current operation from succeeding.
    Error,
    /// A severe error indicating data loss or significant subsystem failure.
    Critical,
    /// An unrecoverable error requiring immediate operator intervention.
    Fatal,
}

/// The top-level error type for Aether Core.
#[derive(Error, Debug)]
pub enum Error {
    /// A configuration-related error (parsing, validation, or missing config).
    #[error("[{code:?}] {message}")]
    Config {
        /// The specific error code within the config category.
        code: ErrorCode,
        /// Human-readable error description.
        message: Cow<'static, str>,
        /// Optional underlying source error.
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },

    /// A capability check failed: the actor lacks a required capability.
    #[error("[{code:?}] Capability {capability:?} denied for {subject}")]
    Capability {
        /// The specific error code within the capability category.
        code: ErrorCode,
        /// The capability that was denied (e.g., `"fs:read"`).
        capability: String,
        /// The subject (actor or identity) that was denied.
        subject: String,
    },

    /// A WASM runtime error (compilation, instantiation, execution, or trap).
    #[error("[{code:?}] WASM {operation} failed: {message}")]
    Wasm {
        /// The specific error code within the WASM category.
        code: ErrorCode,
        /// The WASM operation that failed (e.g., `"compile"`, `"invoke"`).
        operation: Cow<'static, str>,
        /// Human-readable error description.
        message: Cow<'static, str>,
        /// Remaining fuel when the error occurred, if applicable.
        fuel_remaining: Option<u64>,
    },

    /// An actor lifecycle or messaging error.
    #[error("[{code:?}] Actor {actor_id:?}: {message}")]
    Actor {
        /// The specific error code within the actor category.
        code: ErrorCode,
        /// The actor ID, if available.
        actor_id: Option<String>,
        /// Human-readable error description.
        message: Cow<'static, str>,
    },

    /// A storage backend error (read, write, conflict, or corruption).
    #[error("[{code:?}] Storage {operation}: {message}")]
    Storage {
        /// The specific error code within the storage category.
        code: ErrorCode,
        /// The storage operation that failed (e.g., `"read"`, `"transaction"`).
        operation: Cow<'static, str>,
        /// Human-readable error description.
        message: Cow<'static, str>,
    },

    /// A mesh networking error (connection, timeout, node down, backpressure).
    #[error("[{code:?}] Mesh {operation}: {message}")]
    Mesh {
        /// The specific error code within the mesh category.
        code: ErrorCode,
        /// The mesh operation that failed (e.g., `"connect"`, `"send"`).
        operation: Cow<'static, str>,
        /// Human-readable error description.
        message: Cow<'static, str>,
    },

    /// A security-related error (certificates, auth, authorization).
    #[error("[{code:?}] Security {operation}: {message}")]
    Security {
        /// The specific error code within the security category.
        code: ErrorCode,
        /// The security operation that failed (e.g., `"authenticate"`, `"authorize"`).
        operation: Cow<'static, str>,
        /// Human-readable error description.
        message: Cow<'static, str>,
    },

    /// A resource exhaustion error (memory, CPU, fuel, handles).
    #[error("[{code:?}] Resource {resource_type} exhausted: {message}")]
    Resource {
        /// The specific error code within the resource category.
        code: ErrorCode,
        /// The type of resource that was exhausted (e.g., `"memory"`, `"fuel"`).
        resource_type: Cow<'static, str>,
        /// Human-readable error description.
        message: Cow<'static, str>,
    },

    /// A serialization or deserialization error.
    #[error("[{code:?}] Serialization failed: {message}")]
    Serialization {
        /// The specific error code for serialization failures.
        code: ErrorCode,
        /// Human-readable error description.
        message: Cow<'static, str>,
        /// Optional underlying source error.
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },

    /// An I/O error wrapped with an error code.
    #[error("[{code:?}] I/O error: {message}")]
    Io {
        /// The specific error code (typically `StorageRead`).
        code: ErrorCode,
        /// Human-readable error description.
        message: Cow<'static, str>,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// An internal error (bug or unimplemented feature).
    #[error("[{code:?}] Internal error: {message}")]
    Internal {
        /// The specific error code within the internal category.
        code: ErrorCode,
        /// Human-readable error description.
        message: Cow<'static, str>,
    },
}

impl Error {
    /// Returns the [`ErrorCode`] for this error.
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

    /// Returns the [`ErrorSeverity`] for this error based on its type and code.
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

    /// Returns `true` if this error is transient and the operation may succeed on retry.
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

    /// Creates a config parse error with the given message.
    pub fn config_parse(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Config {
            code: ErrorCode::ConfigParse,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a config validation error with the given message.
    pub fn config_validation(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Config {
            code: ErrorCode::ConfigValidation,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a config not-found error with the given message.
    pub fn config_not_found(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Config {
            code: ErrorCode::ConfigNotFound,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a capability-denied error for the given capability and subject.
    pub fn capability_denied(capability: impl Into<String>, subject: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityDenied,
            capability: capability.into(),
            subject: subject.into(),
        }
    }

    /// Creates a capability-not-declared error for the given capability name.
    pub fn capability_not_declared(capability: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityNotDeclared,
            capability: capability.into(),
            subject: String::new(),
        }
    }

    /// Creates a capability-expired error for the given capability name.
    pub fn capability_expired(capability: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityExpired,
            capability: capability.into(),
            subject: String::new(),
        }
    }

    /// Creates a WASM compile error with the given message.
    pub fn wasm_compile(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmCompile,
            operation: "compile".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    /// Creates a WASM instantiation error with the given message.
    pub fn wasm_instantiate(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmInstantiate,
            operation: "instantiate".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    /// Creates a WASM invocation error with the given message.
    pub fn wasm_invoke(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmInvoke,
            operation: "invoke".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    /// Creates a WASM fuel-exhausted error with the remaining fuel count.
    pub fn wasm_fuel_exhausted(fuel_remaining: u64) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmFuelExhausted,
            operation: "execute".into(),
            message: "fuel exhausted".into(),
            fuel_remaining: Some(fuel_remaining),
        }
    }

    /// Creates a WASM memory limit error with the given message.
    pub fn wasm_memory_limit(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmMemoryLimit,
            operation: "memory".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    /// Creates a WASM trap error with the given message.
    pub fn wasm_trap(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Wasm {
            code: ErrorCode::WasmTrap,
            operation: "execute".into(),
            message: message.into(),
            fuel_remaining: None,
        }
    }

    /// Creates an actor-not-found error for the given actor ID.
    pub fn actor_not_found(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorNotFound,
            actor_id: Some(id.into()),
            message: "Actor not found".into(),
        }
    }

    /// Creates an actor-stopped error for the given actor ID.
    pub fn actor_stopped(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorStopped,
            actor_id: Some(id.into()),
            message: "Actor stopped".into(),
        }
    }

    /// Creates an actor-suspended error for the given actor ID.
    pub fn actor_suspended(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorSuspended,
            actor_id: Some(id.into()),
            message: "Actor suspended".into(),
        }
    }

    /// Creates an actor-failed error for the given actor ID and message.
    pub fn actor_failed(id: impl Into<String>, message: impl Into<Cow<'static, str>>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorFailed,
            actor_id: Some(id.into()),
            message: message.into(),
        }
    }

    /// Creates an actor-timeout error for the given actor ID.
    pub fn actor_timeout(id: impl Into<String>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorTimeout,
            actor_id: Some(id.into()),
            message: "Actor operation timed out".into(),
        }
    }

    /// Creates a storage-read error with the given message.
    pub fn storage_read(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Storage {
            code: ErrorCode::StorageRead,
            operation: "read".into(),
            message: message.into(),
        }
    }

    /// Creates a storage-write error with the given message.
    pub fn storage_write(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Storage {
            code: ErrorCode::StorageWrite,
            operation: "write".into(),
            message: message.into(),
        }
    }

    /// Creates a storage-conflict error (optimistic concurrency failure).
    pub fn storage_conflict() -> Self {
        Self::Storage {
            code: ErrorCode::StorageConflict,
            operation: "transaction".into(),
            message: "Transaction conflict detected".into(),
        }
    }

    /// Creates a storage-corruption error with the given message.
    pub fn storage_corruption(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Storage {
            code: ErrorCode::StorageCorruption,
            operation: "integrity".into(),
            message: message.into(),
        }
    }

    /// Creates a mesh-connection error with the given message.
    pub fn mesh_connection(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshConnection,
            operation: "connect".into(),
            message: message.into(),
        }
    }

    /// Creates a mesh-timeout error with the given message.
    pub fn mesh_timeout(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshTimeout,
            operation: "request".into(),
            message: message.into(),
        }
    }

    /// Creates a mesh-node-down error for the given node identifier.
    pub fn mesh_node_down(node: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshNodeDown,
            operation: "connect".into(),
            message: Cow::Owned(format!("Node {} is unavailable", node.into())),
        }
    }

    /// Creates a mesh-backpressure error with the given message.
    pub fn mesh_backpressure(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshBackpressure,
            operation: "send".into(),
            message: message.into(),
        }
    }

    /// Creates a security cert-invalid error with the given message.
    pub fn security_cert_invalid(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityCertInvalid,
            operation: "certificate".into(),
            message: message.into(),
        }
    }

    /// Creates a security cert-expired error with the given message.
    pub fn security_cert_expired(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityCertExpired,
            operation: "certificate".into(),
            message: message.into(),
        }
    }

    /// Creates a security auth-failed error with the given message.
    pub fn security_auth_failed(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityAuthFailed,
            operation: "authenticate".into(),
            message: message.into(),
        }
    }

    /// Creates a security access-denied error with the given message.
    pub fn security_access_denied(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Security {
            code: ErrorCode::SecurityAccessDenied,
            operation: "authorize".into(),
            message: message.into(),
        }
    }

    /// Creates a resource-memory error with the given message.
    pub fn resource_memory(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceMemory,
            resource_type: "memory".into(),
            message: message.into(),
        }
    }

    /// Creates a resource-CPU error with the given message.
    pub fn resource_cpu(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceCpu,
            resource_type: "cpu".into(),
            message: message.into(),
        }
    }

    /// Creates a resource-fuel error with the given message.
    pub fn resource_fuel(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceFuel,
            resource_type: "fuel".into(),
            message: message.into(),
        }
    }

    /// Creates a resource-handles error with the given message.
    pub fn resource_handles(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Resource {
            code: ErrorCode::ResourceHandles,
            resource_type: "handles".into(),
            message: message.into(),
        }
    }

    /// Creates a serialization error with the given message.
    pub fn serialization(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Serialization {
            code: ErrorCode::SerializationFailed,
            message: message.into(),
            source: None,
        }
    }

    /// Creates an internal error with the given message.
    pub fn internal(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Internal {
            code: ErrorCode::InternalBug,
            message: message.into(),
        }
    }

    /// Creates a not-implemented error with the given message.
    pub fn not_implemented(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Internal {
            code: ErrorCode::NotImplemented,
            message: message.into(),
        }
    }

    /// Creates an I/O error from a `std::io::Error`.
    pub fn io(source: io::Error) -> Self {
        Self::Io {
            code: ErrorCode::StorageRead,
            message: Cow::Borrowed("I/O operation failed"),
            source,
        }
    }

    /// Chains a source error onto this error (only for `Config` and `Serialization` variants).
    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        match &mut self {
            Self::Config { source: s, .. } => *s = Some(Box::new(source)),
            Self::Serialization { source: s, .. } => *s = Some(Box::new(source)),
            _ => {}
        }
        self
    }

    /// Alias for [`Error::config_parse`].
    pub fn config(message: impl Into<Cow<'static, str>>) -> Self {
        Self::config_parse(message)
    }

    /// Creates a capability-denied error without a subject.
    pub fn capability_denied_simple(message: impl Into<String>) -> Self {
        Self::Capability {
            code: ErrorCode::CapabilityDenied,
            capability: message.into(),
            subject: String::new(),
        }
    }

    /// Alias for [`Error::wasm_invoke`].
    pub fn wasm(message: impl Into<Cow<'static, str>>) -> Self {
        Self::wasm_invoke(message)
    }

    /// Creates a generic actor error with the given message (no actor ID).
    pub fn actor(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Actor {
            code: ErrorCode::ActorFailed,
            actor_id: None,
            message: message.into(),
        }
    }

    /// Alias for [`Error::storage_read`].
    pub fn storage(message: impl Into<Cow<'static, str>>) -> Self {
        Self::storage_read(message)
    }

    /// Alias for [`Error::security_auth_failed`].
    pub fn security(message: impl Into<Cow<'static, str>>) -> Self {
        Self::security_auth_failed(message)
    }

    /// Alias for [`Error::resource_memory`].
    pub fn resource_exhausted(message: impl Into<Cow<'static, str>>) -> Self {
        Self::resource_memory(message)
    }

    /// Alias for [`Error::serialization`].
    pub fn serialization_legacy(message: impl Into<Cow<'static, str>>) -> Self {
        Self::serialization(message)
    }

    /// Creates a retryable mesh-timeout error with the given message.
    pub fn retryable(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Mesh {
            code: ErrorCode::MeshTimeout,
            operation: "retry".into(),
            message: message.into(),
        }
    }

    /// Alias for [`Error::storage_conflict`].
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

/// Extension trait for adding context to [`Result`] errors.
pub trait ErrorContext<T> {
    /// Maps the error to a [`Config`](Error::Config) variant with the given message.
    fn context(self, message: impl Into<Cow<'static, str>>) -> Result<T>;
    /// Maps the error to a [`Config`](Error::Config) variant using a closure for lazy evaluation.
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
