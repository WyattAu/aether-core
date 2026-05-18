//! Structured Error Taxonomy
//!
//! Defines a ten-level severity classification for Aether errors,
//! aligned with the R&D mega prompt error taxonomy.
//!
//! # Levels
//!
//! | Level | Name        | Description                              |
//! |-------|-------------|------------------------------------------|
//! | 1     | Syntactical | Parsing / formatting errors              |
//! | 2     | Semantic    | Type / validation errors                 |
//! | 3     | Interface   | Contract / boundary mismatches           |
//! | 4     | Structural  | Architecture / topology errors           |
//! | 5     | Fundamental | Runtime invariant violations             |
//! | 6     | Compliance  | Policy / regulatory violations          |
//! | 7     | Knowledge   | Missing data / configuration            |
//! | 8     | Translation | Protocol / format conversion failures    |
//! | 9     | Integration | External service dependency failures     |
//! | 10    | SupplyChain | Third-party / dependency chain failures  |

use super::{Error, ErrorCode};

/// Ten-level error severity taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Level 1 – Parsing, formatting, lexical errors.
    Syntactical = 1,
    /// Level 2 – Type checking, schema validation, constraint violations.
    Semantic = 2,
    /// Level 3 – API contract mismatches, boundary errors.
    Interface = 3,
    /// Level 4 – Architecture, topology, or state-machine errors.
    Structural = 4,
    /// Level 5 – Runtime invariant violations, logic bugs.
    Fundamental = 5,
    /// Level 6 – Policy, regulatory, or governance violations.
    Compliance = 6,
    /// Level 7 – Missing data, knowledge, or configuration.
    Knowledge = 7,
    /// Level 8 – Protocol translation or format conversion failures.
    Translation = 8,
    /// Level 9 – External service / dependency failures.
    Integration = 9,
    /// Level 10 – Third-party or supply-chain failures.
    SupplyChain = 10,
}

impl Severity {
    /// Numeric level value (1–10).
    pub const fn level(self) -> u8 {
        self as u8
    }

    /// Human-readable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Syntactical => "syntactical",
            Self::Semantic => "semantic",
            Self::Interface => "interface",
            Self::Structural => "structural",
            Self::Fundamental => "fundamental",
            Self::Compliance => "compliance",
            Self::Knowledge => "knowledge",
            Self::Translation => "translation",
            Self::Integration => "integration",
            Self::SupplyChain => "supply_chain",
        }
    }

    /// Returns `true` if the severity indicates a system-level issue (>= 5).
    pub const fn is_system_level(self) -> bool {
        self.level() >= 5
    }

    /// Returns `true` if the severity is operator-actionable (>= 6).
    pub const fn is_operator_actionable(self) -> bool {
        self.level() >= 6
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.name(), self.level())
    }
}

impl Error {
    /// Classify this error using the ten-level taxonomy severity.
    pub fn taxonomy_severity(&self) -> Severity {
        match self {
            Self::Config { code, .. } => match code {
                ErrorCode::ConfigParse => Severity::Syntactical,
                ErrorCode::ConfigValidation => Severity::Semantic,
                ErrorCode::ConfigNotFound => Severity::Knowledge,
                _ => Severity::Knowledge,
            },

            Self::Capability { code, .. } => match code {
                ErrorCode::CapabilityDenied => Severity::Compliance,
                ErrorCode::CapabilityNotDeclared => Severity::Interface,
                ErrorCode::CapabilityExpired => Severity::Compliance,
                _ => Severity::Compliance,
            },

            Self::Wasm { code, .. } => match code {
                ErrorCode::WasmCompile => Severity::Syntactical,
                ErrorCode::WasmInstantiate => Severity::Structural,
                ErrorCode::WasmInvoke => Severity::Fundamental,
                ErrorCode::WasmFuelExhausted => Severity::Semantic,
                ErrorCode::WasmMemoryLimit => Severity::Structural,
                ErrorCode::WasmTrap => Severity::Fundamental,
                _ => Severity::Fundamental,
            },

            Self::Actor { code, .. } => match code {
                ErrorCode::ActorNotFound => Severity::Knowledge,
                ErrorCode::ActorStopped => Severity::Structural,
                ErrorCode::ActorSuspended => Severity::Structural,
                ErrorCode::ActorFailed => Severity::Fundamental,
                ErrorCode::ActorTimeout => Severity::Integration,
                _ => Severity::Structural,
            },

            Self::Storage { code, .. } => match code {
                ErrorCode::StorageRead => Severity::Integration,
                ErrorCode::StorageWrite => Severity::Integration,
                ErrorCode::StorageConflict => Severity::Structural,
                ErrorCode::StorageCorruption => Severity::Fundamental,
                _ => Severity::Integration,
            },

            Self::Mesh { code, .. } => match code {
                ErrorCode::MeshConnection => Severity::Integration,
                ErrorCode::MeshTimeout => Severity::Integration,
                ErrorCode::MeshNodeDown => Severity::Integration,
                ErrorCode::MeshBackpressure => Severity::Structural,
                _ => Severity::Integration,
            },

            Self::Security { code, .. } => match code {
                ErrorCode::SecurityCertInvalid => Severity::Compliance,
                ErrorCode::SecurityCertExpired => Severity::Compliance,
                ErrorCode::SecurityAuthFailed => Severity::Compliance,
                ErrorCode::SecurityAccessDenied => Severity::Compliance,
                _ => Severity::Compliance,
            },

            Self::Resource { code, .. } => match code {
                ErrorCode::ResourceMemory => Severity::Structural,
                ErrorCode::ResourceCpu => Severity::Structural,
                ErrorCode::ResourceFuel => Severity::Semantic,
                ErrorCode::ResourceHandles => Severity::Structural,
                _ => Severity::Structural,
            },

            Self::Serialization { .. } => Severity::Translation,

            Self::Io { .. } => Severity::Integration,

            Self::Internal { code, .. } => match code {
                ErrorCode::InternalBug => Severity::Fundamental,
                ErrorCode::NotImplemented => Severity::Knowledge,
                _ => Severity::Fundamental,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_levels() {
        assert_eq!(Severity::Syntactical.level(), 1);
        assert_eq!(Severity::Semantic.level(), 2);
        assert_eq!(Severity::Interface.level(), 3);
        assert_eq!(Severity::Structural.level(), 4);
        assert_eq!(Severity::Fundamental.level(), 5);
        assert_eq!(Severity::Compliance.level(), 6);
        assert_eq!(Severity::Knowledge.level(), 7);
        assert_eq!(Severity::Translation.level(), 8);
        assert_eq!(Severity::Integration.level(), 9);
        assert_eq!(Severity::SupplyChain.level(), 10);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Syntactical < Severity::Semantic);
        assert!(Severity::Integration < Severity::SupplyChain);
    }

    #[test]
    fn test_severity_system_level() {
        assert!(!Severity::Syntactical.is_system_level());
        assert!(!Severity::Structural.is_system_level());
        assert!(Severity::Fundamental.is_system_level());
        assert!(Severity::SupplyChain.is_system_level());
    }

    #[test]
    fn test_severity_operator_actionable() {
        assert!(!Severity::Semantic.is_operator_actionable());
        assert!(Severity::Compliance.is_operator_actionable());
        assert!(Severity::SupplyChain.is_operator_actionable());
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(format!("{}", Severity::Syntactical), "syntactical(1)");
        assert_eq!(format!("{}", Severity::Integration), "integration(9)");
    }

    #[test]
    fn test_config_error_taxonomy() {
        assert_eq!(
            Error::config_parse("bad").taxonomy_severity(),
            Severity::Syntactical
        );
        assert_eq!(
            Error::config_validation("invalid").taxonomy_severity(),
            Severity::Semantic
        );
        assert_eq!(
            Error::config_not_found("missing").taxonomy_severity(),
            Severity::Knowledge
        );
    }

    #[test]
    fn test_wasm_error_taxonomy() {
        assert_eq!(
            Error::wasm_compile("fail").taxonomy_severity(),
            Severity::Syntactical
        );
        assert_eq!(
            Error::wasm_trap("div0").taxonomy_severity(),
            Severity::Fundamental
        );
        assert_eq!(
            Error::wasm_fuel_exhausted(0).taxonomy_severity(),
            Severity::Semantic
        );
    }

    #[test]
    fn test_actor_error_taxonomy() {
        assert_eq!(
            Error::actor_not_found("x").taxonomy_severity(),
            Severity::Knowledge
        );
        assert_eq!(
            Error::actor_failed("y", "err").taxonomy_severity(),
            Severity::Fundamental
        );
        assert_eq!(
            Error::actor_timeout("z").taxonomy_severity(),
            Severity::Integration
        );
    }

    #[test]
    fn test_mesh_error_taxonomy() {
        assert_eq!(
            Error::mesh_timeout("t").taxonomy_severity(),
            Severity::Integration
        );
        assert_eq!(
            Error::mesh_node_down("n").taxonomy_severity(),
            Severity::Integration
        );
        assert_eq!(
            Error::mesh_backpressure("bp").taxonomy_severity(),
            Severity::Structural
        );
    }

    #[test]
    fn test_security_error_taxonomy() {
        assert_eq!(
            Error::security_cert_invalid("bad").taxonomy_severity(),
            Severity::Compliance
        );
        assert_eq!(
            Error::security_auth_failed("fail").taxonomy_severity(),
            Severity::Compliance
        );
    }

    #[test]
    fn test_resource_error_taxonomy() {
        assert_eq!(
            Error::resource_memory("oom").taxonomy_severity(),
            Severity::Structural
        );
        assert_eq!(
            Error::resource_fuel("empty").taxonomy_severity(),
            Severity::Semantic
        );
    }

    #[test]
    fn test_serialization_taxonomy() {
        assert_eq!(
            Error::serialization("fail").taxonomy_severity(),
            Severity::Translation
        );
    }

    #[test]
    fn test_internal_error_taxonomy() {
        assert_eq!(
            Error::internal("bug").taxonomy_severity(),
            Severity::Fundamental
        );
        assert_eq!(
            Error::not_implemented("todo").taxonomy_severity(),
            Severity::Knowledge
        );
    }
}
