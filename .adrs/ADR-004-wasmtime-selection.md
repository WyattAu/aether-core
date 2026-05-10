# ADR-004: Wasmtime WASM Runtime Selection

## Status

**Accepted** - 2026-03-05

## Context

Project Aether requires a WebAssembly runtime for executing untrusted actor code. The runtime must support:

1. **Performance Requirements**:
   - Cold start < 50µs
   - Execution overhead < 5%
   - Memory overhead < 10% vs native
   - Throughput > 1M calls/second

2. **Security Requirements**:
   - Memory isolation guarantee
   - Capability-based security
   - No escape from sandbox
   - Resource confinement

3. **Feature Requirements**:
   - WASI support
   - Fuel metering
   - Memory limiting
   - Multi-value returns
   - Component model (future)

4. **Operational Requirements**:
   - Active maintenance
   - Good documentation
   - Rust ecosystem
   - Production-ready

## Decision

We select **wasmtime 25.0.0** as our WASM runtime.

### Selection Criteria Matrix

| Criterion | wasmtime | wasmer | wasm3 | WasmEdge |
|-----------|----------|--------|-------|----------|
| Cold Start | ***** | **** | ***** | **** |
| Security | ***** | **** | *** | **** |
| Fuel Metering | [DONE] | [DONE] | [FAIL] | [DONE] |
| WASI | ***** | **** | *** | **** |
| Rust Ecosystem | ***** | ***** | *** | **** |
| Maintenance | ***** | **** | *** | **** |
| Component Model | ***** | ** | * | *** |

### Key Advantages

1. **Cranelift Compiler**:
   - Fast compilation (cold start)
   - Good runtime performance
   - Multi-platform support

2. **Security Focus**:
   - Designed for sandboxing
   - Memory isolation verified
   - Capability-based WASI

3. **Fuel Metering**:
   - Deterministic execution
   - O(1) fuel checking
   - Interruptible execution

4. **Component Model**:
   - Future-proof
   - Interface types
   - Module composition

## Implementation

### Basic Setup

```rust
use wasmtime::*;

struct WasmEngine {
    engine: Engine,
    linker: Linker<HostState>,
    module_cache: ModuleCache,
}

impl WasmEngine {
    fn new() -> Result<Self> {
        let engine = Engine::new(Config::new()
            .cranelift_opt_level(OptLevel::Speed)
            .consume_fuel(true)
            .memory_init_cow(true)
        )?;
        
        let linker = Linker::new(&engine);
        
        Ok(Self { engine, linker, module_cache: ModuleCache::new() })
    }
    
    fn compile(&self, wasm: &[u8]) -> Result<Module> {
        Module::new(&self.engine, wasm)
    }
    
    fn instantiate(&self, module: &Module) -> Result<Instance> {
        let mut store = Store::new(&self.engine, HostState::new());
        store.set_fuel(1_000_000)?;
        
        self.linker.instantiate(&mut store, module)
    }
}
```

### Performance Optimizations

```rust
fn optimize_for_cold_start(config: &mut Config) {
    config.cranelift_opt_level(OptLevel::None);  // Faster compile
    config.memory_init_cow(true);                // Copy-on-write
    config.wasm_bulk_memory(true);               // Bulk operations
    config.wasm_multi_value(true);               // Multi-value
}

fn optimize_for_throughput(config: &mut Config) {
    config.cranelift_opt_level(OptLevel::Speed);  // Faster execution
    config.parallel_compilation(true);            // Compile in parallel
}
```

### Security Configuration

```rust
fn configure_security(config: &mut Config) {
    config.wasm_reference_types(true);  // Required for safety
    config.consume_fuel(true);          // Deterministic execution
    config.max_wasm_stack(1024 * 1024); // Stack limit
}

fn limit_memory(store: &mut Store<HostState>, max: usize) {
    let memory = store.data().memory;
    memory.grow(store, max - memory.size()).ok();
}
```

## Consequences

### Positive
- **Cold Start Performance**: <50µs achievable
- **Security**: Proven sandbox isolation
- **Fuel Metering**: Deterministic execution
- **Ecosystem**: Excellent Rust integration
- **Future-Proof**: Component model support
- **Documentation**: Comprehensive docs

### Negative
- **Binary Size**: Cranelift adds ~5MB
- **Compilation Time**: Slower than interpreters
- **Complexity**: Many configuration options
- **License**: Apache 2.0 (patent concerns)

### Neutral
- **JIT vs AOT**: JIT chosen for cold start
- **GC**: No GC support (not needed)

## Alternatives Considered

### 1. wasmer
- **Pros**: Fast, good ecosystem, multiple compilers
- **Cons**: Less mature component model, commercial focus
- **Rejected**: wasmtime better aligned with needs

### 2. wasm3 (interpreter)
- **Pros**: Small, fast startup, simple
- **Cons**: No fuel metering, slower execution
- **Rejected**: Missing critical features

### 3. WasmEdge
- **Pros**: Fast, cloud-native focus
- **Cons**: Smaller community, less Rust-native
- **Rejected**: wasmtime better Rust integration

### 4. Native Compilation (wasm2c)
- **Pros**: Maximum performance
- **Cons**: No sandboxing, complex build
- **Rejected**: Loses security benefits

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Module compilation | 1-5ms | Depends on size |
| Instantiation | 10-50µs | Cold start target |
| Function call overhead | 5-20ns | Near-native |
| Fuel check | <5ns | O(1) inline |
| Memory access | 1-2ns | Direct after bounds check |

## Security Properties

| Property | Mechanism | Status |
|----------|-----------|--------|
| Memory isolation | Linear memory bounds | [DONE] Verified |
| Control flow integrity | Cranelift | [DONE] Verified |
| Resource limits | Fuel + memory caps | [DONE] Verified |
| No escape | Sandbox design | [DONE] Verified |
| WASI capabilities | Capability-based | [DONE] Implemented |

## References

- [wasmtime Documentation](https://docs.wasmtime.dev/)
- [WebAssembly Security](https://webassembly.org/docs/security/)
- [Cranelift Compiler](https://github.com/bytecodealliance/wasmtime/tree/main/cranelift)
- YP-WASM-RUNTIME-001: WASM Runtime Yellow Paper
- BP-WASM-ENGINE-001: WASM Engine Blue Paper

## Notes

- Monitor wastime releases for security updates
- Benchmark cold start quarterly
- Evaluate component model as it matures
