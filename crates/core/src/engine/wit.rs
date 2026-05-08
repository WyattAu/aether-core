//! WIT (WebAssembly Interface Types) Bindings
//!
//! Provides interface definition and resolution for WASM Component Model
//! interfaces using the WIT format.

use std::collections::HashMap;

/// A WIT interface definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WitInterface {
    /// Interface name (e.g., "aether:actor/host")
    pub name: String,
    /// Interface version
    pub version: String,
    /// Functions exported by this interface
    pub functions: Vec<WitFunction>,
    /// Types used by this interface
    pub types: Vec<WitType>,
}

/// A function in a WIT interface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WitFunction {
    /// Function name
    pub name: String,
    /// Input parameters
    pub params: Vec<WitParam>,
    /// Return values
    pub results: Vec<WitParam>,
}

/// A parameter or return value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitParam {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub ty: WitTypeKind,
}

/// WIT type definitions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WitType {
    /// Type name
    pub name: String,
    /// Type kind
    pub kind: WitTypeKind,
}

/// WIT type kinds.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WitTypeKind {
    /// Boolean
    Bool,
    /// 8-bit unsigned integer
    U8,
    /// 16-bit unsigned integer
    U16,
    /// 32-bit unsigned integer
    U32,
    /// 64-bit unsigned integer
    U64,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
    /// UTF-8 string
    String,
    /// List of values
    List(Box<WitTypeKind>),
    /// Optional value
    Option(Box<WitTypeKind>),
    /// Tuple of values
    Tuple(Vec<WitTypeKind>),
    /// Named type reference
    Named(String),
    /// Record (struct)
    Record(Vec<WitParam>),
    /// Variant (enum)
    Variant(Vec<WitVariantCase>),
    /// Resource handle
    Resource(String),
}

/// A variant case.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitVariantCase {
    /// Case name
    pub name: String,
    /// Optional payload type
    pub ty: Option<WitTypeKind>,
}

/// Registry of known WIT interfaces.
#[derive(Debug, Default)]
pub struct WitRegistry {
    interfaces: HashMap<String, WitInterface>,
}

impl WitRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a WIT interface.
    pub fn register(&mut self, interface: WitInterface) {
        self.interfaces.insert(interface.name.clone(), interface);
    }

    /// Get a registered interface by name.
    pub fn get(&self, name: &str) -> Option<&WitInterface> {
        self.interfaces.get(name)
    }

    /// List all registered interface names.
    pub fn list(&self) -> Vec<&str> {
        self.interfaces.keys().map(|s| s.as_str()).collect()
    }

    /// Check if an interface is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.interfaces.contains_key(name)
    }

    /// Get the number of registered interfaces.
    pub fn len(&self) -> usize {
        self.interfaces.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.interfaces.is_empty()
    }

    /// Create the built-in Aether host interface.
    pub fn aether_host_interface() -> WitInterface {
        WitInterface {
            name: "aether:actor/host".to_string(),
            version: "0.1.0".to_string(),
            functions: vec![
                WitFunction {
                    name: "send-message".to_string(),
                    params: vec![
                        WitParam {
                            name: "target".to_string(),
                            ty: WitTypeKind::String,
                        },
                        WitParam {
                            name: "payload".to_string(),
                            ty: WitTypeKind::List(Box::new(WitTypeKind::U8)),
                        },
                    ],
                    results: vec![],
                },
                WitFunction {
                    name: "get-state".to_string(),
                    params: vec![WitParam {
                        name: "key".to_string(),
                        ty: WitTypeKind::String,
                    }],
                    results: vec![WitParam {
                        name: "value".to_string(),
                        ty: WitTypeKind::List(Box::new(WitTypeKind::U8)),
                    }],
                },
                WitFunction {
                    name: "set-state".to_string(),
                    params: vec![
                        WitParam {
                            name: "key".to_string(),
                            ty: WitTypeKind::String,
                        },
                        WitParam {
                            name: "value".to_string(),
                            ty: WitTypeKind::List(Box::new(WitTypeKind::U8)),
                        },
                    ],
                    results: vec![],
                },
                WitFunction {
                    name: "log".to_string(),
                    params: vec![
                        WitParam {
                            name: "level".to_string(),
                            ty: WitTypeKind::U8,
                        },
                        WitParam {
                            name: "message".to_string(),
                            ty: WitTypeKind::String,
                        },
                    ],
                    results: vec![],
                },
            ],
            types: vec![],
        }
    }
}

/// Validates that a component's required imports are satisfied by the host.
pub struct WitValidator;

impl WitValidator {
    /// Validate that all required imports are available.
    /// Returns Ok(()) if valid, Err with missing interface names if not.
    pub fn validate_imports(
        required: &[String],
        available: &WitRegistry,
    ) -> Result<(), Vec<String>> {
        let missing: Vec<String> = required
            .iter()
            .filter(|name| !available.contains(name))
            .cloned()
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wit_registry_crud() {
        let mut registry = WitRegistry::new();
        let iface = WitInterface {
            name: "test:iface/v1".to_string(),
            version: "1.0.0".to_string(),
            functions: vec![],
            types: vec![],
        };
        registry.register(iface);
        assert!(registry.contains("test:iface/v1"));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list(), vec!["test:iface/v1"]);
    }

    #[test]
    fn test_wit_registry_get() {
        let mut registry = WitRegistry::new();
        registry.register(WitInterface {
            name: "a:b/c".to_string(),
            version: "0.1.0".to_string(),
            functions: vec![WitFunction {
                name: "do_thing".to_string(),
                params: vec![WitParam {
                    name: "x".to_string(),
                    ty: WitTypeKind::U32,
                }],
                results: vec![],
            }],
            types: vec![],
        });
        let iface = registry.get("a:b/c").unwrap();
        assert_eq!(iface.functions.len(), 1);
        assert_eq!(iface.functions[0].name, "do_thing");
    }

    #[test]
    fn test_wit_validator_pass() {
        let mut registry = WitRegistry::new();
        registry.register(WitInterface {
            name: "aether:actor/host".to_string(),
            version: "0.1.0".to_string(),
            functions: vec![],
            types: vec![],
        });
        let required = vec!["aether:actor/host".to_string()];
        assert!(WitValidator::validate_imports(&required, &registry).is_ok());
    }

    #[test]
    fn test_wit_validator_fail() {
        let registry = WitRegistry::new();
        let required = vec!["missing:interface".to_string()];
        let result = WitValidator::validate_imports(&required, &registry);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), vec!["missing:interface"]);
    }

    #[test]
    fn test_aether_host_interface() {
        let iface = WitRegistry::aether_host_interface();
        assert_eq!(iface.name, "aether:actor/host");
        assert_eq!(iface.functions.len(), 4);
        assert_eq!(iface.functions[0].name, "send-message");
        assert_eq!(iface.functions[3].name, "log");
    }

    #[test]
    fn test_wit_type_kind_serialization() {
        let ty = WitTypeKind::List(Box::new(WitTypeKind::U8));
        let json = serde_json::to_string(&ty).unwrap();
        let deserialized: WitTypeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(ty, deserialized);
    }

    #[test]
    fn test_wit_interface_serialization() {
        let iface = WitRegistry::aether_host_interface();
        let json = serde_json::to_string(&iface).unwrap();
        let deserialized: WitInterface = serde_json::from_str(&json).unwrap();
        assert_eq!(iface.name, deserialized.name);
        assert_eq!(iface.functions.len(), deserialized.functions.len());
    }

    #[test]
    fn test_empty_registry() {
        let registry = WitRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get("anything").is_none());
    }
}
