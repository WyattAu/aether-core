//! WASM Component Model Support
//!
//! The Component Model extends core WASM with interfaces, composition,
//! and language-agnostic ABI (Canonical ABI). This module provides
//! support for loading and managing WASM components alongside
//! traditional core WASM modules.

use std::sync::Arc;

use crate::capability::CapabilitySet;

#[cfg(feature = "wasm")]
use wasmtime::Engine;

/// A loaded WASM Component, ready for instantiation.
///
/// Components use the Component Model's interface-based composition
/// and Canonical ABI for cross-language interoperability.
#[cfg(feature = "wasm")]
pub struct WasmComponent {
    component: Arc<wasmtime::component::Component>,
    name: String,
    size: usize,
}

#[cfg(feature = "wasm")]
impl std::fmt::Debug for WasmComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmComponent")
            .field("name", &self.name)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "wasm")]
impl WasmComponent {
    /// Load a WASM component from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`WasmComponentError::LoadFailed`] if the bytes are not a valid WASM component.
    pub fn from_bytes(
        engine: &Engine,
        bytes: &[u8],
        name: &str,
    ) -> Result<Self, WasmComponentError> {
        let component = wasmtime::component::Component::new(engine, bytes).map_err(|e| {
            WasmComponentError::LoadFailed {
                name: name.to_string(),
                detail: e.to_string(),
            }
        })?;

        Ok(Self {
            component: Arc::new(component),
            name: name.to_string(),
            size: bytes.len(),
        })
    }

    /// Get the component name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the component size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get a reference to the inner wasmtime component.
    pub fn inner(&self) -> &wasmtime::component::Component {
        &self.component
    }
}

/// Errors that can occur during component operations.
#[derive(Debug, thiserror::Error)]
pub enum WasmComponentError {
    /// Component binary could not be parsed or compiled.
    #[error("failed to load component '{name}': {detail}")]
    LoadFailed {
        /// Name of the component that failed to load.
        name: String,
        /// Underlying error detail.
        detail: String,
    },

    /// Component could not be instantiated.
    #[error("failed to instantiate component '{name}': {detail}")]
    InstantiationFailed {
        /// Name of the component that failed to instantiate.
        name: String,
        /// Underlying error detail.
        detail: String,
    },

    /// A requested export was not found in the component.
    #[error("component '{name}' export '{export}' not found")]
    ExportNotFound {
        /// Name of the component.
        name: String,
        /// Name of the missing export.
        export: String,
    },

    /// An export existed but had an incompatible type.
    #[error("type mismatch for export '{export}' in component '{name}': {detail}")]
    TypeMismatch {
        /// Name of the component.
        name: String,
        /// Name of the export with the type mismatch.
        export: String,
        /// Description of the type mismatch.
        detail: String,
    },

    /// A component function invocation failed at runtime.
    #[error("component invocation failed: {detail}")]
    InvocationFailed {
        /// Description of the invocation failure.
        detail: String,
    },
}

/// Builder for configuring component instance creation.
pub struct ComponentInstanceBuilder {
    name: String,
    fuel: Option<u64>,
    capabilities: Option<CapabilitySet>,
}

impl ComponentInstanceBuilder {
    /// Create a new builder for a named component.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            fuel: None,
            capabilities: None,
        }
    }

    /// Set the fuel limit for the component instance.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    /// Set the capability set for the component instance.
    pub fn with_capabilities(mut self, caps: CapabilitySet) -> Self {
        self.capabilities = Some(caps);
        self
    }

    /// Build the component instance configuration.
    pub fn build(self) -> ComponentInstanceConfig {
        ComponentInstanceConfig {
            name: self.name,
            fuel: self.fuel,
            capabilities: self.capabilities,
        }
    }
}

/// Configuration for a component instance.
#[derive(Debug, Clone)]
pub struct ComponentInstanceConfig {
    name: String,
    fuel: Option<u64>,
    capabilities: Option<CapabilitySet>,
}

impl ComponentInstanceConfig {
    /// Get the instance name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the fuel limit.
    pub fn fuel(&self) -> Option<u64> {
        self.fuel
    }

    /// Get the capability set.
    pub fn capabilities(&self) -> Option<&CapabilitySet> {
        self.capabilities.as_ref()
    }
}

/// A pool of pre-compiled WASM components for fast instantiation.
///
/// Similar to [`crate::engine::InstancePool`] but for Component Model components.
#[derive(Debug)]
pub struct ComponentPool {
    components: std::collections::HashMap<String, Arc<WasmComponent>>,
    max_size: usize,
}

#[cfg(feature = "wasm")]
impl ComponentPool {
    /// Create a new empty component pool with the given maximum size.
    pub fn new(max_size: usize) -> Self {
        Self {
            components: std::collections::HashMap::new(),
            max_size,
        }
    }

    /// Add a pre-compiled component to the pool.
    ///
    /// Returns an error if the pool is full.
    pub fn add(&mut self, component: WasmComponent) -> Result<(), WasmComponentError> {
        if self.components.len() >= self.max_size {
            return Err(WasmComponentError::InstantiationFailed {
                name: component.name().to_string(),
                detail: format!("component pool is full (max_size={})", self.max_size),
            });
        }
        self.components
            .insert(component.name().to_string(), Arc::new(component));
        Ok(())
    }

    /// Get a component from the pool by name.
    pub fn get(&self, name: &str) -> Option<Arc<WasmComponent>> {
        self.components.get(name).cloned()
    }

    /// Get the number of components in the pool.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Get the maximum pool size.
    pub fn max_size(&self) -> usize {
        self.max_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_builder_defaults() {
        let config = ComponentInstanceBuilder::new("test-component").build();
        assert_eq!(config.name(), "test-component");
        assert_eq!(config.fuel(), None);
        assert!(config.capabilities().is_none());
    }

    #[test]
    fn test_component_builder_with_fuel() {
        let config = ComponentInstanceBuilder::new("test-component")
            .with_fuel(1_000_000)
            .build();
        assert_eq!(config.fuel(), Some(1_000_000));
    }

    #[test]
    fn test_component_builder_with_capabilities() {
        let caps = CapabilitySet::all();
        let config = ComponentInstanceBuilder::new("test-component")
            .with_capabilities(caps)
            .build();
        assert!(config.capabilities().is_some());
    }

    #[test]
    fn test_component_error_display() {
        let err = WasmComponentError::ExportNotFound {
            name: "test".to_string(),
            export: "run".to_string(),
        };
        assert!(err.to_string().contains("run"));
        assert!(err.to_string().contains("test"));
    }

    #[cfg(feature = "wasm")]
    mod wasm_tests {
        use super::*;
        use crate::engine::module::create_engine;

        fn empty_component_bytes() -> Vec<u8> {
            vec![
                0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00, 0x01, 0x08, 0x00, 0x61, 0x73, 0x6d,
                0x01, 0x00, 0x00, 0x00, 0x02, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x25, 0x0e, 0x63,
                0x6f, 0x6d, 0x70, 0x6f, 0x6e, 0x65, 0x6e, 0x74, 0x2d, 0x6e, 0x61, 0x6d, 0x65, 0x01,
                0x09, 0x00, 0x11, 0x01, 0x00, 0x04, 0x6d, 0x61, 0x69, 0x6e, 0x01, 0x09, 0x00, 0x12,
                0x01, 0x00, 0x04, 0x6d, 0x61, 0x69, 0x6e, 0x00, 0x2f, 0x09, 0x70, 0x72, 0x6f, 0x64,
                0x75, 0x63, 0x65, 0x72, 0x73, 0x01, 0x0c, 0x70, 0x72, 0x6f, 0x63, 0x65, 0x73, 0x73,
                0x65, 0x64, 0x2d, 0x62, 0x79, 0x01, 0x0d, 0x77, 0x69, 0x74, 0x2d, 0x63, 0x6f, 0x6d,
                0x70, 0x6f, 0x6e, 0x65, 0x6e, 0x74, 0x07, 0x30, 0x2e, 0x32, 0x34, 0x35, 0x2e, 0x31,
            ]
        }

        #[test]
        fn test_component_from_bytes() {
            let engine = create_engine().expect("engine");
            let component_bytes = empty_component_bytes();

            let component = WasmComponent::from_bytes(&engine, &component_bytes, "test-load")
                .expect("component");
            assert_eq!(component.name(), "test-load");
            assert_eq!(component.size(), component_bytes.len());
        }

        #[test]
        fn test_component_pool_basic() {
            let engine = create_engine().expect("engine");
            let component_bytes = empty_component_bytes();

            let component = WasmComponent::from_bytes(&engine, &component_bytes, "test-pool")
                .expect("component");

            let mut pool = ComponentPool::new(10);
            pool.add(component).expect("add");
            assert_eq!(pool.len(), 1);
            assert!(pool.get("test-pool").is_some());
            assert!(pool.get("nonexistent").is_none());
        }

        #[test]
        fn test_component_pool_full() {
            let engine = create_engine().expect("engine");
            let component_bytes = empty_component_bytes();

            let mut pool = ComponentPool::new(1);
            let c1 = WasmComponent::from_bytes(&engine, &component_bytes, "c1").expect("c1");
            pool.add(c1).expect("add c1");

            let c2 = WasmComponent::from_bytes(&engine, &component_bytes, "c2").expect("c2");
            assert!(pool.add(c2).is_err());
        }
    }
}
