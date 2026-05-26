//! WASM Component Model Preparation
//!
//! Provides types and a registry for managing WASM Component Model entities,
//! including dependency resolution via topological sort and interface
//! compatibility checking between component exports and imports.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A named interface point that a component makes available to consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentExport {
    /// Export name (e.g. `"wasi:clocks/monotonic-clock@0.2.0"`).
    pub name: String,
    /// Fully-qualified interface identifier.
    pub interface: String,
    /// Human-readable description of this export.
    #[serde(default)]
    pub description: String,
}

/// A dependency that a component requires from another component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentImport {
    /// Import name (local to this component).
    pub name: String,
    /// Component that is expected to provide this import.
    pub from_component: String,
    /// Fully-qualified interface identifier that must match an export.
    pub interface: String,
}

/// A single WASM component with metadata, exports, and imports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmComponent {
    /// Logical component name (unique within a registry).
    pub name: String,
    /// Semantic version of the component.
    pub version: semver::Version,
    /// Interfaces this component exports.
    #[serde(default)]
    pub exports: Vec<ComponentExport>,
    /// Interfaces this component imports from other components.
    #[serde(default)]
    pub imports: Vec<ComponentImport>,
}

impl PartialEq for WasmComponent {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.version == other.version
    }
}

impl Eq for WasmComponent {}

impl std::hash::Hash for WasmComponent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.version.hash(state);
    }
}

/// Runtime state of an instantiated component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    /// The instance has been created but not yet started.
    Inactive,
    /// The instance is currently running.
    Running,
    /// The instance has been stopped and can be restarted.
    Stopped,
    /// The instance encountered a fatal error.
    Failed,
}

/// A running (or recently stopped) instance of a [`WasmComponent`].
#[derive(Debug)]
pub struct ComponentInstance {
    /// Reference to the component definition.
    pub component: Arc<WasmComponent>,
    /// Current lifecycle state of this instance.
    pub state: InstanceState,
}

impl ComponentInstance {
    /// Creates a new instance for the given component, starting in the
    /// [`InstanceState::Inactive`] state.
    pub fn new(component: Arc<WasmComponent>) -> Self {
        Self {
            component,
            state: InstanceState::Inactive,
        }
    }

    /// Transitions the instance to [`InstanceState::Running`].
    pub fn start(&mut self) -> Result<()> {
        match self.state {
            InstanceState::Inactive | InstanceState::Stopped => {
                self.state = InstanceState::Running;
                Ok(())
            }
            InstanceState::Running => Err(Error::actor(format!(
                "component '{}' is already running",
                self.component.name
            ))),
            InstanceState::Failed => Err(Error::actor(format!(
                "component '{}' is in Failed state, cannot start",
                self.component.name
            ))),
        }
    }

    /// Transitions the instance to [`InstanceState::Stopped`].
    pub fn stop(&mut self) -> Result<()> {
        match self.state {
            InstanceState::Running => {
                self.state = InstanceState::Stopped;
                Ok(())
            }
            InstanceState::Inactive | InstanceState::Stopped => Err(Error::actor(
                "cannot stop a component that is not running".to_string(),
            )),
            InstanceState::Failed => Err(Error::actor(
                "cannot stop a component that has failed".to_string(),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Composition errors
// ---------------------------------------------------------------------------

/// Errors that can occur during component composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionError {
    /// A component with this name is already registered.
    DuplicateComponent(String),
    /// An import references a component that does not exist in the registry.
    UnknownComponent(String),
    /// The import interface does not match any export of the source component.
    InterfaceMismatch {
        /// Name of the import that failed to match.
        import_name: String,
        /// Interface signature required by the import.
        required_interface: String,
        /// Interface signatures available from the source component.
        available_interfaces: Vec<String>,
    },
    /// A cycle was detected in the component dependency graph.
    CycleDetected(Vec<String>),
    /// A component imports from itself.
    SelfImport(String),
}

impl std::fmt::Display for CompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateComponent(name) => {
                write!(f, "duplicate component: '{name}'")
            }
            Self::UnknownComponent(name) => {
                write!(f, "unknown component: '{name}'")
            }
            Self::InterfaceMismatch {
                import_name,
                required_interface,
                available_interfaces,
            } => {
                write!(
                    f,
                    "interface mismatch for import '{import_name}': requires '{required_interface}', available: [{}]",
                    available_interfaces.join(", ")
                )
            }
            Self::CycleDetected(cycle) => {
                write!(f, "dependency cycle detected: {}", cycle.join(" -> "))
            }
            Self::SelfImport(name) => {
                write!(f, "component '{name}' imports from itself")
            }
        }
    }
}

impl std::error::Error for CompositionError {}

// ---------------------------------------------------------------------------
// Component registry
// ---------------------------------------------------------------------------

/// Thread-safe registry for managing WASM components and their dependencies.
///
/// Supports registration, dependency resolution via topological sort, and
/// interface compatibility checking.
pub struct ComponentRegistry {
    components: HashMap<String, Arc<WasmComponent>>,
}

impl ComponentRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Registers a component. Returns an error if a component with the same
    /// name is already registered.
    pub fn register(
        &mut self,
        component: WasmComponent,
    ) -> std::result::Result<(), CompositionError> {
        if self.components.contains_key(&component.name) {
            return Err(CompositionError::DuplicateComponent(component.name));
        }
        self.components
            .insert(component.name.clone(), Arc::new(component));
        Ok(())
    }

    /// Removes a component from the registry by name.
    ///
    /// Returns the removed component, or `None` if not found.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<WasmComponent>> {
        self.components.remove(name)
    }

    /// Returns a reference to the component with the given name, if registered.
    pub fn get(&self, name: &str) -> Option<&Arc<WasmComponent>> {
        self.components.get(name)
    }

    /// Returns the number of registered components.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Returns `true` when the registry contains no components.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Returns the names of all registered components.
    pub fn component_names(&self) -> Vec<&str> {
        self.components.keys().map(String::as_str).collect()
    }

    /// Checks interface compatibility for all imports of all registered
    /// components.
    ///
    /// For each import, verifies that the `from_component` exists in the
    /// registry and exports an interface matching `import.interface`.
    pub fn check_interfaces(&self) -> std::result::Result<(), Vec<CompositionError>> {
        let mut errors = Vec::new();

        for component in self.components.values() {
            for imp in &component.imports {
                if imp.from_component == component.name {
                    errors.push(CompositionError::SelfImport(component.name.clone()));
                    continue;
                }

                if let Some(source) = self.components.get(&imp.from_component) {
                    let matching: Vec<String> = source
                        .exports
                        .iter()
                        .filter(|e| e.interface == imp.interface)
                        .map(|e| e.interface.clone())
                        .collect();

                    if matching.is_empty() {
                        let available: Vec<String> =
                            source.exports.iter().map(|e| e.interface.clone()).collect();
                        errors.push(CompositionError::InterfaceMismatch {
                            import_name: imp.name.clone(),
                            required_interface: imp.interface.clone(),
                            available_interfaces: available,
                        });
                    }
                } else {
                    errors.push(CompositionError::UnknownComponent(
                        imp.from_component.clone(),
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Resolves component dependencies and returns a topological ordering
    /// suitable for instantiation.
    ///
    /// An edge `A -> B` is added when component `A` imports from component `B`
    /// (i.e. `A` depends on `B`).  Components with no dependencies appear
    /// first.
    pub fn resolve_dependencies(&self) -> std::result::Result<Vec<String>, CompositionError> {
        let names: HashSet<&str> = self.components.keys().map(String::as_str).collect();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();

        for name in &names {
            in_degree.insert(name, 0);
            forward.insert(name, Vec::new());
        }

        for component in self.components.values() {
            for imp in &component.imports {
                if imp.from_component == component.name {
                    return Err(CompositionError::SelfImport(component.name.clone()));
                }
                if names.contains(imp.from_component.as_str()) {
                    if let Some(targets) = forward.get_mut(imp.from_component.as_str()) {
                        targets.push(component.name.as_str());
                    }
                    if let Some(d) = in_degree.get_mut(component.name.as_str()) {
                        *d += 1;
                    }
                }
            }
        }

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(&n, _)| n)
            .collect();

        let mut order = Vec::with_capacity(names.len());

        while let Some(node) = queue.pop_front() {
            order.push(node.to_owned());
            if let Some(targets) = forward.get(node) {
                for &t in targets {
                    if let Some(d) = in_degree.get_mut(t) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(t);
                        }
                    }
                }
            }
        }

        if order.len() != names.len() {
            let remaining: Vec<String> = names
                .iter()
                .filter(|n| !order.iter().any(|o| o == *n))
                .map(|n| n.to_string())
                .collect();
            return Err(CompositionError::CycleDetected(remaining));
        }

        Ok(order)
    }

    /// Returns all components that the named component transitively depends on.
    pub fn dependencies_of(
        &self,
        name: &str,
    ) -> std::result::Result<Vec<String>, CompositionError> {
        let order = self.resolve_dependencies()?;
        let pos = order
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| CompositionError::UnknownComponent(name.to_string()))?;

        let mut deps = Vec::new();
        for earlier in &order[..pos] {
            if let Some(comp) = self.components.get(earlier) {
                for imp in &comp.imports {
                    if imp.from_component == name || imp.from_component == *earlier {
                        continue;
                    }
                }
            }
            deps.push(earlier.clone());
        }

        deps.retain(|dep| {
            if *dep == name {
                return false;
            }
            let mut queue = VecDeque::new();
            queue.push_back(name);
            let mut visited = HashSet::new();
            visited.insert(name.to_string());
            while let Some(current) = queue.pop_front() {
                if current == *dep {
                    return true;
                }
                if let Some(comp) = self.components.get(current) {
                    for imp in &comp.imports {
                        if imp.from_component == *dep {
                            return true;
                        }
                        if names_contains(&visited, &imp.from_component) {
                            visited.insert(imp.from_component.clone());
                            queue.push_back(&imp.from_component);
                        }
                    }
                }
            }
            false
        });

        Ok(deps)
    }

    /// Creates instances for all registered components in dependency order.
    pub fn instantiate_all(&self) -> std::result::Result<Vec<ComponentInstance>, CompositionError> {
        let order = self.resolve_dependencies()?;
        let mut instances = Vec::with_capacity(order.len());
        for name in &order {
            let component = self
                .components
                .get(name)
                .ok_or_else(|| CompositionError::UnknownComponent(name.clone()))?
                .clone();
            instances.push(ComponentInstance::new(component));
        }
        Ok(instances)
    }
}

fn names_contains(set: &HashSet<String>, name: &str) -> bool {
    set.contains(name)
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_component(
        name: &str,
        version: &str,
        exports: Vec<ComponentExport>,
        imports: Vec<ComponentImport>,
    ) -> WasmComponent {
        WasmComponent {
            name: name.to_string(),
            version: semver::Version::parse(version).expect("valid semver"),
            exports,
            imports,
        }
    }

    fn export(name: &str, interface: &str) -> ComponentExport {
        ComponentExport {
            name: name.to_string(),
            interface: interface.to_string(),
            description: String::new(),
        }
    }

    fn imp(name: &str, from: &str, interface: &str) -> ComponentImport {
        ComponentImport {
            name: name.to_string(),
            from_component: from.to_string(),
            interface: interface.to_string(),
        }
    }

    // -- basic registration --

    #[test]
    fn register_and_get() {
        let mut reg = ComponentRegistry::new();
        let c = make_component("a", "1.0.0", vec![], vec![]);
        reg.register(c).expect("register");
        assert!(reg.get("a").is_some());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn duplicate_registration_fails() {
        let mut reg = ComponentRegistry::new();
        let c1 = make_component("a", "1.0.0", vec![], vec![]);
        let c2 = make_component("a", "2.0.0", vec![], vec![]);
        reg.register(c1).expect("first");
        let err = reg.register(c2).expect_err("duplicate should fail");
        assert_eq!(err, CompositionError::DuplicateComponent("a".to_string()));
    }

    #[test]
    fn unregister_removes_component() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component("a", "1.0.0", vec![], vec![]))
            .expect("register");
        assert!(reg.get("a").is_some());
        reg.unregister("a");
        assert!(reg.get("a").is_none());
        assert!(reg.is_empty());
    }

    #[test]
    fn component_names_list() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component("x", "1.0.0", vec![], vec![]))
            .expect("ok");
        reg.register(make_component("y", "2.0.0", vec![], vec![]))
            .expect("ok");
        let mut names = reg.component_names();
        names.sort();
        assert_eq!(names, vec!["x", "y"]);
    }

    // -- interface compatibility --

    #[test]
    fn interface_check_passes() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "logger",
            "1.0.0",
            vec![export("log", "wasi:logging/log@0.2.0")],
            vec![],
        ))
        .expect("ok");
        reg.register(make_component(
            "app",
            "1.0.0",
            vec![],
            vec![imp("log", "logger", "wasi:logging/log@0.2.0")],
        ))
        .expect("ok");
        assert!(reg.check_interfaces().is_ok());
    }

    #[test]
    fn interface_check_unknown_source() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "app",
            "1.0.0",
            vec![],
            vec![imp("log", "nonexistent", "wasi:logging/log@0.2.0")],
        ))
        .expect("ok");
        let errs = reg.check_interfaces().expect_err("should fail");
        assert!(
            errs.iter()
                .any(|e| matches!(e, CompositionError::UnknownComponent(_)))
        );
    }

    #[test]
    fn interface_check_mismatch() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "db",
            "1.0.0",
            vec![export("get", "custom:db/get@0.1.0")],
            vec![],
        ))
        .expect("ok");
        reg.register(make_component(
            "app",
            "1.0.0",
            vec![],
            vec![imp("store", "db", "custom:db/put@0.1.0")],
        ))
        .expect("ok");
        let errs = reg.check_interfaces().expect_err("should fail");
        assert!(
            errs.iter()
                .any(|e| matches!(e, CompositionError::InterfaceMismatch { .. }))
        );
    }

    #[test]
    fn interface_check_self_import() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "loop",
            "1.0.0",
            vec![export("foo", "custom:foo@0.1.0")],
            vec![imp("foo", "loop", "custom:foo@0.1.0")],
        ))
        .expect("ok");
        let errs = reg.check_interfaces().expect_err("should fail");
        assert!(
            errs.iter()
                .any(|e| matches!(e, CompositionError::SelfImport(_)))
        );
    }

    // -- dependency resolution --

    #[test]
    fn resolve_no_deps() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component("a", "1.0.0", vec![], vec![]))
            .expect("ok");
        let order = reg.resolve_dependencies().expect("ok");
        assert_eq!(order, vec!["a"]);
    }

    #[test]
    fn resolve_linear_deps() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component("a", "1.0.0", vec![], vec![]))
            .expect("ok");
        reg.register(make_component(
            "b",
            "1.0.0",
            vec![export("x", "if:x@1")],
            vec![imp("x", "a", "if:x@1")],
        ))
        .expect("ok");
        let order = reg.resolve_dependencies().expect("ok");
        let pos_a = order.iter().position(|n| n == "a").expect("a in order");
        let pos_b = order.iter().position(|n| n == "b").expect("b in order");
        assert!(pos_a < pos_b);
    }

    #[test]
    fn resolve_diamond_deps() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "base",
            "1.0.0",
            vec![export("x", "if:x@1")],
            vec![],
        ))
        .expect("ok");
        reg.register(make_component(
            "left",
            "1.0.0",
            vec![export("y", "if:y@1")],
            vec![imp("x", "base", "if:x@1")],
        ))
        .expect("ok");
        reg.register(make_component(
            "right",
            "1.0.0",
            vec![export("z", "if:z@1")],
            vec![imp("x", "base", "if:x@1")],
        ))
        .expect("ok");
        reg.register(make_component(
            "top",
            "1.0.0",
            vec![],
            vec![imp("y", "left", "if:y@1"), imp("z", "right", "if:z@1")],
        ))
        .expect("ok");

        let order = reg.resolve_dependencies().expect("ok");
        let pos = |n: &str| order.iter().position(|x| x == n).expect(n);
        assert!(pos("base") < pos("left"));
        assert!(pos("base") < pos("right"));
        assert!(pos("left") < pos("top"));
        assert!(pos("right") < pos("top"));
    }

    #[test]
    fn resolve_cycle_detected() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "a",
            "1.0.0",
            vec![export("x", "if:x@1")],
            vec![imp("y", "b", "if:y@1")],
        ))
        .expect("ok");
        reg.register(make_component(
            "b",
            "1.0.0",
            vec![export("y", "if:y@1")],
            vec![imp("x", "a", "if:x@1")],
        ))
        .expect("ok");
        let err = reg.resolve_dependencies().expect_err("cycle");
        assert!(matches!(err, CompositionError::CycleDetected(_)));
    }

    #[test]
    fn resolve_self_import_error() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "self-loop",
            "1.0.0",
            vec![export("x", "if:x@1")],
            vec![imp("x", "self-loop", "if:x@1")],
        ))
        .expect("ok");
        let err = reg.resolve_dependencies().expect_err("self import");
        assert!(matches!(err, CompositionError::SelfImport(_)));
    }

    // -- instantiation --

    #[test]
    fn instantiate_all_in_order() {
        let mut reg = ComponentRegistry::new();
        reg.register(make_component(
            "a",
            "1.0.0",
            vec![export("x", "if:x@1")],
            vec![],
        ))
        .expect("ok");
        reg.register(make_component(
            "b",
            "1.0.0",
            vec![],
            vec![imp("x", "a", "if:x@1")],
        ))
        .expect("ok");
        let instances = reg.instantiate_all().expect("ok");
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].component.name, "a");
        assert_eq!(instances[1].component.name, "b");
        assert_eq!(instances[0].state, InstanceState::Inactive);
    }

    // -- instance lifecycle --

    #[test]
    fn instance_start_stop_lifecycle() {
        let c = Arc::new(make_component("x", "1.0.0", vec![], vec![]));
        let mut inst = ComponentInstance::new(Arc::clone(&c));
        assert_eq!(inst.state, InstanceState::Inactive);

        inst.start().expect("start");
        assert_eq!(inst.state, InstanceState::Running);

        inst.stop().expect("stop");
        assert_eq!(inst.state, InstanceState::Stopped);

        inst.start().expect("restart");
        assert_eq!(inst.state, InstanceState::Running);
    }

    #[test]
    fn instance_start_when_running_fails() {
        let c = Arc::new(make_component("x", "1.0.0", vec![], vec![]));
        let mut inst = ComponentInstance::new(c);
        inst.start().expect("start");
        assert!(inst.start().is_err());
    }

    #[test]
    fn instance_stop_when_inactive_fails() {
        let c = Arc::new(make_component("x", "1.0.0", vec![], vec![]));
        let mut inst = ComponentInstance::new(c);
        assert!(inst.stop().is_err());
    }

    // -- wasm component equality --

    #[test]
    fn wasm_component_equality() {
        let a = make_component("x", "1.0.0", vec![], vec![]);
        let b = make_component("x", "1.0.0", vec![], vec![]);
        assert_eq!(a, b);
    }

    #[test]
    fn wasm_component_inequality_version() {
        let a = make_component("x", "1.0.0", vec![], vec![]);
        let b = make_component("x", "2.0.0", vec![], vec![]);
        assert_ne!(a, b);
    }

    // -- composition error display --

    #[test]
    fn composition_error_display() {
        let err = CompositionError::DuplicateComponent("dup".to_string());
        assert_eq!(format!("{err}"), "duplicate component: 'dup'");

        let err = CompositionError::CycleDetected(vec!["a".to_string(), "b".to_string()]);
        let msg = format!("{err}");
        assert!(msg.contains("cycle"));
        assert!(msg.contains("a -> b"));
    }
}
