# BP-WASM-ENGINE-001: WASM Execution Engine Architecture

**Document ID:** BP-WASM-ENGINE-001  
**Domain:** Architecture / Runtime Systems  
**Version:** 1.0.0  
**Status:** Draft  
**Standard:** IEEE 1016-2009  
**Authors:** Construct (Systems Architect)  
**Created:** 2026-03-05  
**Last Modified:** 2026-03-05  
**References:** YP-WASM-RUNTIME-001, YP-SERIAL-RKYV-001

---

## BP-1: Design Overview

### 1.1 System Purpose

The WASM Execution Engine provides the secure, high-performance runtime for executing WebAssembly actors within Project Aether. The architecture enables:

1. **Sub-50µs Cold Starts**: AOT-compiled module instantiation with minimal overhead
2. **Memory Isolation**: Strict linear memory sandboxing with zero cross-instance data leakage
3. **Fuel-Based Execution**: Deterministic instruction counting ensuring bounded computation
4. **Capability Security**: Deny-by-default access control with O(1) verification
5. **State Hydration**: Zero-copy actor state reconstruction from rkyv archives

### 1.2 System Scope

| Scope Element | Description |
|---------------|-------------|
| **In Scope** | Module compilation, instance management, fuel counting, memory management, WASI bridge, capability enforcement, state hydration |
| **Out of Scope** | Actor scheduling, message routing, persistent storage, network I/O |

### 1.3 System Context (C4Context Diagram)

```
┌─────────────────────────────────────────────────────────────┐
│                    Actor Runtime System                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              WASM Execution Engine                     │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐      │  │
│  │  │  Module    │  │  Instance  │  │   Fuel     │      │  │
│  │  │  Loader    │  │  Manager   │  │  Counter   │      │  │
│  │  └────────────┘  └────────────┘  └────────────┘      │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐      │  │
│  │  │  Memory    │  │   WASI     │  │ Capability │      │  │
│  │  │  Manager   │  │   Bridge   │  │ Enforcer   │      │  │
│  │  └────────────┘  └────────────┘  └────────────┘      │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                  │
│                    State Hydration (rkyv)                   │
│                           │                                  │
└─────────────────────────────────────────────────────────────┘
              │                          │
         WASM Modules               Actor Archives
         (AOT compiled)            (rkyv serialized)
```

### 1.4 Design Goals

| Goal ID | Goal | Priority | Rationale |
|---------|------|----------|-----------|
| DG-001 | Sub-50µs cold start latency | Critical | Actor model requires fast instantiation |
| DG-002 | Memory isolation guarantee | Critical | Security invariant for multi-tenant execution |
| DG-003 | Deterministic execution | Critical | Bounded computation via fuel mechanism |
| DG-004 | Capability-based security | High | Zero-trust access control |
| DG-005 | State hydration <50ms | High | Actor migration and checkpointing |

### 1.5 Design Constraints

| Constraint ID | Constraint | Source | Impact |
|---------------|------------|--------|--------|
| DC-001 | Cold start < 50µs | Performance | AOT compilation required |
| DC-002 | Memory per instance ≤ 4GB | WASM spec | 32-bit addressing limit |
| DC-003 | Fuel limit per invocation | Determinism | Traps on exhaustion |
| DC-004 | WASI Preview 2 compliance | Standards | Capability model required |
| DC-005 | Hydration time < 50ms | Migration | Zero-copy deserialization |

---

## BP-2: Design Decomposition

### 2.1 Component Hierarchy

```
BP-WASM-ENGINE-001 (WASM Execution Engine)
│
├── COMP-WASM-001: Module Loader
│   ├── SUBCOMP-001.1: AOT Compiler
│   ├── SUBCOMP-001.2: Validation Engine
│   └── SUBCOMP-001.3: Module Cache
│
├── COMP-WASM-002: Instance Manager
│   ├── SUBCOMP-002.1: Instance Pool
│   ├── SUBCOMP-002.2: Cold Start Optimizer
│   └── SUBCOMP-002.3: Lifecycle Controller
│
├── COMP-WASM-003: Fuel Counter
│   ├── SUBCOMP-003.1: Instruction Cost Table
│   ├── SUBCOMP-003.2: Atomic Counter
│   └── SUBCOMP-003.3: Exhaustion Handler
│
├── COMP-WASM-004: Memory Manager
│   ├── SUBCOMP-004.1: Linear Memory Allocator
│   ├── SUBCOMP-004.2: Bounds Checker
│   └── SUBCOMP-004.3: Grow Handler
│
├── COMP-WASM-005: WASI Bridge
│   ├── SUBCOMP-005.1: System Call Dispatcher
│   ├── SUBCOMP-005.2: Resource Mapper
│   └── SUBCOMP-005.3: Preview 2 Adapter
│
└── COMP-WASM-006: Capability Enforcer
    ├── SUBCOMP-006.1: Capability Bitmap
    ├── SUBCOMP-006.2: Access Validator
    └── SUBCOMP-006.3: Audit Logger
```

### 2.2 Component Specifications

#### COMP-WASM-001: Module Loader

**Purpose**: Compile and cache WASM modules for fast instantiation.

**Responsibilities**:
- Validate WASM binary format compliance
- Perform AOT compilation using Wasmtime
- Cache compiled modules for reuse
- Track module dependencies and versions

**Interfaces**:
- `CompileModule(wasmBytes) → CompiledModule`
- `GetCachedModule(moduleHash) → Option<CompiledModule>`
- `ValidateModule(wasmBytes) → Result<(), ValidationError>`

**Quality Attributes**:
- Performance: O(1) cached module retrieval
- Reliability: Validation before compilation
- Security: Reject malformed modules

#### COMP-WASM-002: Instance Manager

**Purpose**: Create and manage WASM instance lifecycle with sub-50µs cold starts.

**Responsibilities**:
- Instantiate modules with pre-allocated resources
- Execute cold start initialization sequence
- Manage instance lifecycle (create, suspend, resume, destroy)
- Track instance statistics for monitoring

**Interfaces**:
- `CreateInstance(module, config) → Instance`
- `DestroyInstance(instanceId) → Result<(), Error>`
- `GetInstanceStats(instanceId) → InstanceStats`

**Quality Attributes**:
- Performance: Cold start < 50µs (THM-WASM-004)
- Scalability: Support 10,000+ concurrent instances
- Resource boundedness: Memory limits enforced

#### COMP-WASM-003: Fuel Counter

**Purpose**: Track and enforce deterministic execution through fuel consumption.

**Responsibilities**:
- Maintain atomic fuel counter per instance
- Map opcodes to fuel costs per ALG-WASM-003
- Trap on fuel exhaustion
- Support fuel replenishment for long-running computations

**Interfaces**:
- `ConsumeFuel(instance, amount) → Result<(), Trap>`
- `AddFuel(instance, amount) → Result<(), Error>`
- `GetFuelStatus(instance) → FuelStatus`

**Quality Attributes**:
- Performance: O(1) per instruction overhead
- Correctness: Deterministic fuel consumption (AX-WASM-002)
- Safety: Guaranteed termination (THM-WASM-002)

#### COMP-WASM-004: Memory Manager

**Purpose**: Manage linear memory allocation and bounds checking.

**Responsibilities**:
- Allocate initial memory pages per module specification
- Enforce bounds checking on all memory accesses
- Handle memory.grow requests within limits
- Isolate instance memories per AX-WASM-001

**Interfaces**:
- `AllocateMemory(pages) → MemoryHandle`
- `GrowMemory(handle, deltaPages) → Result<u32, Error>`
- `ReadMemory(handle, offset, len) → Result<Vec<u8>, Trap>`
- `WriteMemory(handle, offset, data) → Result<(), Trap>`

**Quality Attributes**:
- Performance: O(1) allocation for bounded sizes
- Security: Memory isolation invariant (THM-WASM-001)
- Safety: Bounds checking prevents OOB access

#### COMP-WASM-005: WASI Bridge

**Purpose**: Provide controlled access to host system via WASI Preview 2.

**Responsibilities**:
- Dispatch WASI system calls to host implementations
- Map WASI resources to host resources
- Enforce capability restrictions on all calls
- Implement WASI Preview 2 interface types

**Interfaces**:
- `InvokeWasi(instance, func, args) → Result<Values, WasiError>`
- `RegisterHostFunction(name, impl) → Result<(), Error>`
- `GetWasiCapabilities(instance) → CapabilitySet`

**Quality Attributes**:
- Compatibility: WASI Preview 2 compliant
- Security: Capability-enforced access (THM-WASM-003)
- Performance: O(1) capability check per call

#### COMP-WASM-006: Capability Enforcer

**Purpose**: Enforce deny-by-default access control for all host operations.

**Responsibilities**:
- Maintain capability bitmap per instance
- Validate capabilities on every host call
- Log capability denials for audit
- Support capability delegation (restriction only)

**Interfaces**:
- `GrantCapability(instance, cap) → Result<(), Error>`
- `RevokeCapability(instance, cap) → Result<(), Error>`
- `CheckCapability(instance, cap) → bool`

**Quality Attributes**:
- Performance: O(1) bitmap lookup
- Security: Unforgeable capabilities (AX-WASM-003)
- Auditability: All denials logged

---

## BP-3: Design Rationale

### 3.1 Why Wasmtime

**Decision**: Use Wasmtime as the WASM runtime.

**Rationale**:

| Criterion | Wasmtime | Wasmer | WasmEdge | Winner |
|-----------|----------|--------|----------|--------|
| AOT Compilation | Excellent | Excellent | Good | Tie |
| Fuel Support | Native | Native | Plugin | Wasmtime |
| WASI Preview 2 | Native | Partial | Partial | Wasmtime |
| Memory Safety | Rust | Rust | C++ | Wasmtime/Wasmer |
| Community | Bytecode Alliance | Individual | Individual | Wasmtime |
| Cold Start | Optimized | Good | Optimized | Tie |
| Security Track Record | Strong | Good | Moderate | Wasmtime |

**Conclusion**: Wasmtime provides the best combination of fuel support, WASI Preview 2 compliance, and security.

### 3.2 Why Fuel-Based Execution

**Decision**: Use fuel/instruction counting for deterministic execution.

**Rationale**:

1. **Bounded Computation**: Fuel ensures all executions terminate (THM-WASM-002)
   - Prevents infinite loops
   - Enables fair scheduling
   - Supports billing/quota enforcement

2. **Determinism**: Same fuel limit produces same execution path
   - Required for consensus protocols
   - Enables reproducible debugging
   - Supports time-travel debugging

3. **Overhead**: Minimal performance impact
   - Per-instruction overhead: ~1-2 CPU cycles
   - Atomic counter: Lock-free on most architectures
   - Amortized cost: < 1% of execution time

4. **Alternative Considered**: Time-based limits
   - Rejected: Non-deterministic (depends on CPU speed, load)
   - Rejected: Difficult to reason about

### 3.3 Why Linear Memory Sandboxing

**Decision**: Use WASM linear memory with hardware-enforced isolation.

**Rationale**:

1. **Security**: Memory isolation by construction (AX-WASM-001)
   - All addresses relative to instance base
   - Hardware bounds checking
   - No pointer arithmetic to escape sandbox

2. **Performance**: Near-native memory access speed
   - Direct memory access within bounds
   - No software bounds check on most paths
   - Virtual memory protection for guard pages

3. **Simplicity**: Single flat address space
   - No complex memory mapping
   - Predictable layout
   - Easy debugging

4. **Alternative Considered**: Software fault isolation
   - Rejected: Higher overhead
   - Rejected: More complex implementation

### 3.4 Why Capability-Based Security

**Decision**: Use capabilities for all host interface access control.

**Rationale**:

1. **Least Privilege**: Each instance gets minimal capabilities (AX-WASM-003)
   - Deny-by-default
   - Explicit grants only
   - No ambient authority

2. **O(1) Verification**: Bitmap lookup is constant time
   - Single bit test per capability
   - No traversal or lookup
   - Cache-friendly

3. **Composability**: Capabilities can be delegated (restricted)
   - Parent can grant subset to child
   - No privilege escalation
   - Supports capability patterns

4. **Alternative Considered**: ACL-based security
   - Rejected: O(n) check where n = number of rules
   - Rejected: More complex administration

---

## BP-4: Traceability

### 4.1 Requirement Traceability Matrix

| Requirement | Component | Interface | Yellow Paper Reference |
|-------------|-----------|-----------|------------------------|
| REQ-WASM-001: Sub-50µs cold start | COMP-WASM-001, COMP-WASM-002 | IF-WASM-001, IF-WASM-002 | ALG-WASM-001, THM-WASM-004 |
| REQ-WASM-002: Memory isolation | COMP-WASM-004 | IF-WASM-004 | AX-WASM-001, THM-WASM-001 |
| REQ-WASM-003: Deterministic execution | COMP-WASM-003 | IF-WASM-003 | AX-WASM-002, THM-WASM-002 |
| REQ-WASM-004: Capability security | COMP-WASM-006 | IF-WASM-003 | AX-WASM-003, THM-WASM-003 |
| REQ-WASM-005: WASI compliance | COMP-WASM-005 | IF-WASM-003 | WASI Preview 2 |
| REQ-WASM-006: State hydration | COMP-WASM-002 | IF-WASM-005 | YP-SERIAL-RKYV-001 |

### 4.2 Theorem Traceability

| Theorem (YP) | Components | Properties Verified | Proof Reference |
|--------------|------------|---------------------|-----------------|
| THM-WASM-001: Memory Isolation | COMP-WASM-004 | PROP-WASM-001 | proof_wasm.lean:MemoryIsolation |
| THM-WASM-002: Fuel Exhaustion | COMP-WASM-003 | PROP-WASM-002 | proof_wasm.lean:FuelExhaustion |
| THM-WASM-003: Capability Confinement | COMP-WASM-006 | PROP-WASM-004 | proof_wasm.lean:CapabilityConfinement |
| THM-WASM-004: Cold Start Complexity | COMP-WASM-001, COMP-WASM-002 | PROP-WASM-003 | proof_wasm.lean:ColdStartTiming |
| THM-SER-002: State Hydration Correctness | COMP-WASM-002 | PROP-WASM-005 | proof_wasm.lean:StateHydration |

### 4.3 Algorithm Traceability

| Algorithm (YP) | Component Implementation | Complexity |
|----------------|--------------------------|------------|
| ALG-WASM-001: Cold Start | COMP-WASM-002: CreateInstance() | O(1) for bounded modules |
| ALG-WASM-002: Capability Check | COMP-WASM-006: CheckCapability() | O(1) |
| ALG-WASM-003: Fuel Management | COMP-WASM-003: ConsumeFuel() | O(1) |
| ALG-WASM-004: Memory Bounds Check | COMP-WASM-004: MemoryBoundsCheck() | O(1) |
| ALG-SER-002: State Hydration | COMP-WASM-002: HydrateState() | O(\|Σ_a\|) |

---

## BP-5: Interface Design

### 5.1 Interface Catalog

| Interface ID | Interface Name | Provider | Consumer | Protocol |
|--------------|----------------|----------|----------|----------|
| IF-WASM-001 | Module Compilation | COMP-WASM-001 | Actor Runtime | Internal API |
| IF-WASM-002 | Instance Creation | COMP-WASM-002 | Actor Runtime | Internal API |
| IF-WASM-003 | Function Invocation | COMP-WASM-002 | Actor Runtime | Internal API |
| IF-WASM-004 | Memory Access | COMP-WASM-004 | Actor Runtime, COMP-WASM-005 | Internal API |
| IF-WASM-005 | State Hydration | COMP-WASM-002 | Actor Runtime | Internal API |

### 5.2 Interface Specifications

#### IF-WASM-001: Module Compilation

**Purpose**: Compile WASM binary to AOT-compiled module.

**Signature**:
```rust
fn compile_module(
    wasm_bytes: &[u8],
    config: CompileConfig
) -> Result<CompiledModule, CompileError>
```

**Parameters**:
- `wasm_bytes`: Raw WASM binary
- `config`: Compilation options (optimization level, target features)

**Returns**:
- `Ok(CompiledModule)`: AOT-compiled module ready for instantiation
- `Err(CompileError::ValidationFailed)`: Invalid WASM binary
- `Err(CompileError::UnsupportedFeature)`: Uses unsupported WASM feature

**Preconditions**:
- `wasm_bytes` is valid WASM binary format
- Sufficient memory for compilation

**Postconditions**:
- Compiled module cached by hash
- Module ready for instantiation

**See Also**: `.specs/02_architecture/interface_contracts/interface_contracts_wasm.toml`

#### IF-WASM-002: Instance Creation

**Purpose**: Create a new WASM instance with cold start optimization.

**Signature**:
```rust
fn create_instance(
    module: &CompiledModule,
    config: InstanceConfig
) -> Result<Instance, InstanceError>
```

**Parameters**:
- `module`: AOT-compiled module
- `config`: Instance configuration (memory limits, fuel, capabilities)

**Returns**:
- `Ok(Instance)`: Ready-to-execute instance
- `Err(InstanceError::MemoryAllocationFailed)`: Cannot allocate memory
- `Err(InstanceError::StartFailed)`: Start function trapped

**Preconditions**:
- Module is valid and compiled
- Sufficient system resources

**Postconditions**:
- Instance memory isolated
- Fuel initialized to config.fuel_limit
- Capabilities bound per config.capabilities
- Cold start time < 50µs

#### IF-WASM-003: Function Invocation

**Purpose**: Invoke exported WASM function with fuel management.

**Signature**:
```rust
fn invoke_function(
    instance: &mut Instance,
    func_name: &str,
    args: &[Value],
    fuel_limit: u64
) -> Result<Vec<Value>, InvokeError>
```

**Parameters**:
- `instance`: Target instance
- `func_name`: Exported function name
- `args`: Function arguments
- `fuel_limit`: Maximum fuel for this invocation

**Returns**:
- `Ok(Vec<Value>)`: Return values
- `Err(InvokeError::OutOfFuel)`: Fuel exhausted
- `Err(InvokeError::Trap)`: Runtime trap
- `Err(InvokeError::CapabilityDenied)`: Missing capability

**Preconditions**:
- Instance is valid and not destroyed
- Function is exported
- Argument types match signature

**Postconditions**:
- Fuel consumed from instance budget
- Instance state modified per function semantics

#### IF-WASM-004: Memory Access

**Purpose**: Read/write instance linear memory.

**Signature**:
```rust
fn read_memory(
    instance: &Instance,
    offset: u32,
    len: u32
) -> Result<Vec<u8>, MemoryError>

fn write_memory(
    instance: &mut Instance,
    offset: u32,
    data: &[u8]
) -> Result<(), MemoryError>
```

**Parameters**:
- `instance`: Target instance
- `offset`: Memory offset
- `len`/`data`: Length to read / data to write

**Returns**:
- `Ok(Vec<u8>)` / `Ok(())`: Success
- `Err(MemoryError::OutOfBounds)`: Access outside memory bounds

**Preconditions**:
- Instance is valid
- Offset + len <= memory.size

**Postconditions**:
- Memory contents read/written
- No side effects on instance state

#### IF-WASM-005: State Hydration

**Purpose**: Reconstruct instance state from rkyv archive.

**Signature**:
```rust
fn hydrate_state(
    archive: &[u8],
    module: &CompiledModule,
    config: HydrationConfig
) -> Result<Instance, HydrationError>
```

**Parameters**:
- `archive`: rkyv-serialized actor state
- `module`: Module to instantiate
- `config`: Hydration options

**Returns**:
- `Ok(Instance)`: Hydrated instance
- `Err(HydrationError::ChecksumMismatch)`: Archive corrupted
- `Err(HydrationError::ValidationFailed)`: Invalid archive structure
- `Err(HydrationError::Timeout)`: Hydration exceeded 50ms budget

**Preconditions**:
- Archive is valid rkyv format
- Module matches archived state

**Postconditions**:
- Instance state equivalent to archived state
- Hydration time < 50ms

---

## BP-6: Data Design

### 6.1 Data Structures

#### CompiledModule Structure

**Purpose**: AOT-compiled WASM module ready for instantiation.

**Layout**:
```rust
struct CompiledModule {
    module_hash: [u8; 32],        // SHA-256 of WASM binary
    compiled_code: Vec<u8>,       // AOT-compiled machine code
    memory_spec: MemorySpec,      // Memory requirements
    table_specs: Vec<TableSpec>,  // Table specifications
    global_specs: Vec<GlobalSpec>,// Global specifications
    data_segments: Vec<DataSegment>, // Initial data
    elem_segments: Vec<ElemSegment>, // Initial elements
    exports: HashMap<String, Export>, // Exported functions
    imports: Vec<Import>,         // Required imports
    start_func: Option<FuncIdx>,  // Optional start function
}

struct MemorySpec {
    min_pages: u32,               // Minimum pages (64KB each)
    max_pages: Option<u32>,       // Maximum pages (None = unlimited)
}

struct DataSegment {
    offset: u32,                  // Memory offset
    data: Vec<u8>,                // Initial data
}
```

**Invariants**:
- `module_hash` uniquely identifies module
- `compiled_code` is valid machine code for target architecture
- `min_pages >= 1` per WASM spec

#### InstanceState Structure

**Purpose**: Runtime state of a WASM instance.

**Layout**:
```rust
struct InstanceState {
    instance_id: InstanceId,      // Unique identifier
    module_hash: [u8; 32],        // Compiled module reference
    memory: MemoryHandle,         // Linear memory
    tables: Vec<TableHandle>,     // Function tables
    globals: Vec<GlobalValue>,    // Global variables
    fuel: AtomicU64,              // Remaining fuel
    capabilities: CapabilityBitmap, // Granted capabilities
    status: InstanceStatus,       // Running, Suspended, Destroyed
    created_at: Instant,          // Creation timestamp
    stats: InstanceStats,         // Execution statistics
}

struct CapabilityBitmap {
    bits: u64,                    // 64 capability slots
}

enum InstanceStatus {
    Running,
    Suspended,
    Destroyed,
}
```

**Invariants**:
- `memory` is isolated from all other instances
- `fuel >= 0`
- `capabilities` subset of allowed capabilities

#### MemoryLayout

**Purpose**: Physical memory layout for WASM linear memory.

**Layout**:
```
┌─────────────────────────────────────────────────────────────┐
│ Linear Memory Layout (per instance)                         │
├─────────────────────────────────────────────────────────────┤
│ Guard Page (4KB) - Read/Write fault                         │
├─────────────────────────────────────────────────────────────┤
│ Stack Region (grows down)                                   │
│   ├─ Stack base (high address)                              │
│   └─ Stack limit (low address)                              │
├─────────────────────────────────────────────────────────────┤
│ Heap Region (grows up)                                      │
│   ├─ Heap start                                             │
│   └─ Heap end (current break)                               │
├─────────────────────────────────────────────────────────────┤
│ Data Segments (initialized from module)                     │
│   ├─ Segment 0                                              │
│   ├─ Segment 1                                              │
│   └─ ...                                                    │
├─────────────────────────────────────────────────────────────┤
│ Guard Page (4KB) - Read/Write fault                         │
└─────────────────────────────────────────────────────────────┘
```

**Memory Protection**:
- Guard pages trap on access
- Stack overflow detected via guard page
- Memory.grow extends heap region

### 6.2 Data Relationships

```
CompiledModule
    │
    ├─instantiates─▶ InstanceState
    │                    │
    │                    ├─has─▶ MemoryHandle
    │                    │          └─backed by─▶ Linear Memory Pages
    │                    │
    │                    ├─has─▶ CapabilityBitmap
    │                    │          └─grants─▶ WASI Access
    │                    │
    │                    └─has─▶ Fuel Counter
    │                               └─consumed by─▶ Instructions
    │
    └─cached by─▶ Module Cache
```

### 6.3 Data Persistence

| Data Type | Persistence | Location | TTL |
|-----------|-------------|----------|-----|
| Compiled modules | Disk cache | /var/lib/aether/modules/ | Until invalidated |
| Instance state | In-memory | RAM | Instance lifetime |
| Actor archives | FoundationDB | Cluster | Per retention policy |
| Capability grants | In-memory | InstanceState | Instance lifetime |

---

## BP-7: Component Design

### 7.1 Module Loading Sequence

```
┌──────────────────────────────────────────────────────────────┐
│                    Module Loading Sequence                    │
└──────────────────────────────────────────────────────────────┘

Actor Runtime                Module Loader               Module Cache
      │                            │                           │
      │ 1. compile_module(bytes)  │                           │
      ├───────────────────────────▶│                           │
      │                            │                           │
      │                            │ 2. compute_hash(bytes)    │
      │                            ├──────────────────────────▶│
      │                            │                           │
      │                            │ 3. check_cache(hash)      │
      │                            ├──────────────────────────▶│
      │                            │                           │
      │                            │ 4. cache miss             │
      │                            │◀──────────────────────────┤
      │                            │                           │
      │                            │ 5. validate_wasm(bytes)   │
      │                            ├──────────────────────────▶│
      │                            │                           │
      │                            │ 6. aot_compile(validated) │
      │                            ├──────────────────────────▶│
      │                            │                           │
      │                            │ 7. cache_module(compiled) │
      │                            ├──────────────────────────▶│
      │                            │                           │
      │ 8. return CompiledModule  │                           │
      │◀───────────────────────────┤                           │
      │                            │                           │
```

### 7.2 Cold Start Flow (<50µs)

```
┌──────────────────────────────────────────────────────────────┐
│                    Cold Start Flow (<50µs)                    │
└──────────────────────────────────────────────────────────────┘

Timeline: t=0 ──────────────────────────────────────────▶ t=50µs

Phase 1: Allocation (<10µs)
├─ allocate InstanceState struct
├─ initialize atomic counters
└─ set up capability bitmap

Phase 2: Memory Setup (<15µs)
├─ mmap(min_pages * 64KB)
├─ set up guard pages
└─ configure memory protection

Phase 3: Data Segments (<10µs)
├─ for each segment:
│   ├─ memcpy(memory.base + offset, segment.data)
│   └─ (parallelized for large segments)
└─ verify copy integrity

Phase 4: Table Init (<5µs)
├─ allocate function table
├─ initialize element segments
└─ set up indirect call targets

Phase 5: Globals (<5µs)
├─ copy global initializers
└─ link imported globals

Phase 6: Capability Bind (<3µs)
├─ set capability bitmap
└─ validate capability subset

Phase 7: Start Function (<2µs if trivial)
├─ if start_func exists:
│   └─ invoke with minimal fuel
└─ skip if no start function

Total: <50µs for well-designed modules
```

### 7.3 State Hydration Protocol

```
┌──────────────────────────────────────────────────────────────┐
│                    State Hydration Protocol                   │
└──────────────────────────────────────────────────────────────┘

Migration Source              Actor Runtime            Migration Target
      │                            │                           │
      │ 1. suspend_actor(id)      │                           │
      ├───────────────────────────▶│                           │
      │                            │                           │
      │                            │ 2. archive_state(actor)   │
      │                            ├──────────────────────────▶│
      │                            │   (rkyv serialization)    │
      │                            │                           │
      │                            │ 3. transfer_archive(A)    │
      │                            ├──────────────────────────▶│
      │                            │                           │
      │                            │                           │ 4. validate_archive(A)
      │                            │                           ├────────────────────▶
      │                            │                           │   - checksum
      │                            │                           │   - structure
      │                            │                           │
      │                            │                           │ 5. hydrate_state(A)
      │                            │                           ├────────────────────▶
      │                            │                           │   - allocate memory
      │                            │                           │   - copy archived state
      │                            │                           │   - remap resources
      │                            │                           │
      │                            │ 6. confirm_hydration     │
      │                            │◀──────────────────────────┤
      │                            │                           │
      │ 7. cleanup_source         │                           │
      │◀───────────────────────────┤                           │
      │                            │                           │
```

---

## BP-8: Deployment Design

### 8.1 Memory Requirements

| Resource | Minimum | Recommended | Maximum |
|----------|---------|-------------|---------|
| Instance memory | 64KB (1 page) | 1MB (16 pages) | 4GB (65536 pages) |
| Instance overhead | 8KB | 16KB | 32KB |
| Module cache | 100MB | 500MB | 2GB |
| Stack per instance | 64KB | 256KB | 1MB |
| Total per node | 1GB | 8GB | 64GB |

### 8.2 CPU Requirements

| Resource | Minimum | Recommended | Notes |
|----------|---------|-------------|-------|
| CPU cores | 2 | 8+ | Parallel instance execution |
| CPU frequency | 2GHz | 3GHz+ | Faster cold starts |
| SIMD support | SSE4.2 | AVX2+ | AOT compilation optimization |
| Virtualization | None | Optional | For additional isolation |

### 8.3 Deployment Configuration

```yaml
wasm_engine:
  module_cache:
    enabled: true
    max_size: 500MB
    eviction_policy: LRU
    
  instance_pool:
    max_instances: 10000
    max_memory_per_instance: 256MB
    total_memory_limit: 8GB
    
  fuel:
    default_limit: 10000000
    max_limit: 1000000000
    replenishment_enabled: true
    
  capabilities:
    default_set: []
    allow_delegation: true
    audit_denials: true
    
  cold_start:
    target_latency_us: 50
    prewarm_pool_size: 100
    
  hydration:
    timeout_ms: 50
    checksum_algorithm: xxhash3_64
```

---

## BP-9: Formal Verification

### 9.1 Verification Properties

#### PROP-WASM-001: Memory Isolation

**Statement**: Instance memories are strictly isolated; no cross-instance access is possible.

**Formal Specification**:
```lean
theorem memory_isolation :
  ∀ (i₁ i₂ : Instance) (addr : Nat) (val : Byte),
    i₁ ≠ i₂ →
    write_memory i₁ addr val →
    read_memory i₂ addr = read_memory i₂ addr (before write)
```

**Assumptions**:
- WASM runtime correctly implements linear memory (AX-WASM-001)
- Hardware memory protection is functional
- No shared memory between instances

**Proof Strategy**: 
1. Show each instance has independent memory allocation
2. Prove addresses are instance-relative, not absolute
3. Demonstrate hardware bounds checking prevents escape

**Reference**: `proof_wasm.lean:MemoryIsolation`

#### PROP-WASM-002: Fuel Exhaustion Handling

**Statement**: Execution always terminates (or traps) when fuel is exhausted.

**Formal Specification**:
```lean
theorem fuel_exhaustion_termination :
  ∀ (inst : Instance) (fuel : Nat),
    fuel < ∞ →
    ∃ n ≤ fuel / min_cost, 
      execute inst n = Normal Termination ∨
      execute inst n = Trap OutOfFuel
```

**Assumptions**:
- Each instruction consumes at least min_cost > 0 fuel (AX-WASM-002)
- Fuel counter is monotonically decreasing
- No fuel replenishment during execution

**Proof Strategy**:
1. Establish well-founded ordering on fuel counter
2. Show each step decreases fuel by at least min_cost
3. Prove termination via well-founded induction

**Reference**: `proof_wasm.lean:FuelExhaustion`

#### PROP-WASM-003: Cold Start Timing

**Statement**: Cold start completes within 50µs for pre-compiled modules.

**Formal Specification**:
```lean
theorem cold_start_timing :
  ∀ (module : CompiledModule) (caps : CapabilitySet),
    module.data_segments_size ≤ MAX_DATA_SIZE →
    module.elem_segments_size ≤ MAX_ELEM_SIZE →
    (module.start_func = None ∨ module.start_func_is_trivial) →
    cold_start_latency module caps < 50µs
```

**Assumptions**:
- Module is AOT-compiled (no parsing)
- System has sufficient resources
- Data/element segments are bounded

**Proof Strategy**:
1. Analyze each cold start phase timing
2. Sum phase budgets
3. Show total < 50µs under assumptions

**Reference**: `proof_wasm.lean:ColdStartTiming`

### 9.2 Verification Methods

| Property | Method | Tool | Status |
|----------|--------|------|--------|
| PROP-WASM-001 | Theorem proving | Lean 4 | Specified |
| PROP-WASM-002 | Theorem proving | Lean 4 | Specified |
| PROP-WASM-003 | Model checking + benchmarks | TLA+ + Criterion | Planned |
| Memory bounds | Runtime assertion | Rust | Implemented |
| Fuel invariants | Runtime assertion | Rust | Implemented |
| Capability checks | Static analysis | Clippy + custom | Implemented |

### 9.3 Invariants

**Instance Invariants**:
```lean
def instance_invariant (inst : Instance) : Prop :=
  inst.memory.isolated ∧
  inst.fuel ≥ 0 ∧
  inst.capabilities ⊆ allowed_capabilities ∧
  inst.status ∈ {Running, Suspended, Destroyed}
```

**Fuel Counter Invariants**:
```lean
def fuel_invariant (inst : Instance) (initial_fuel : Nat) : Prop :=
  inst.fuel ≤ initial_fuel ∧
  (inst.fuel = 0 → execution_trapped inst)
```

---

## BP-10: HAL Specification

### 10.1 Hardware Abstraction Layer

The WASM Engine HAL abstracts low-level operations for portability.

**HAL-WASM-001: Memory Interface**

```rust
trait MemoryHal {
    fn allocate_pages(count: usize) -> Result<MemoryRegion, HalError>;
    fn deallocate_pages(region: MemoryRegion);
    fn protect_pages(region: &MemoryRegion, prot: Protection);
    fn copy_memory(dst: &mut MemoryRegion, src: &MemoryRegion, len: usize);
}

enum Protection {
    None,
    Read,
    ReadWrite,
    ReadExecute,
}
```

**HAL-WASM-002: Atomic Interface**

```rust
trait AtomicHal {
    fn atomic_load_u64(ptr: *const u64) -> u64;
    fn atomic_store_u64(ptr: *mut u64, val: u64);
    fn atomic_compare_exchange_u64(
        ptr: *mut u64,
        expected: u64,
        new: u64
    ) -> Result<u64, u64>;
    fn atomic_fetch_sub_u64(ptr: *mut u64, val: u64) -> u64;
}
```

**HAL-WASM-003: WASI Host Interface**

```rust
trait WasiHal {
    fn fd_read(fd: u32, buf: &mut [u8]) -> Result<usize, WasiError>;
    fn fd_write(fd: u32, buf: &[u8]) -> Result<usize, WasiError>;
    fn random_get(buf: &mut [u8]) -> Result<(), WasiError>;
    fn clock_time_get(clock_id: u32) -> Result<u64, WasiError>;
    fn sched_yield() -> Result<(), WasiError>;
}
```

### 10.2 Platform Implementations

| Platform | Memory HAL | Atomic HAL | WASI HAL |
|----------|------------|------------|----------|
| Linux (x86_64) | `mmap` + `mprotect` | x86 atomic instr | syscalls |
| Linux (ARM64) | `mmap` + `mprotect` | ARM atomic instr | syscalls |
| macOS | `mmap` + `mprotect` | x86/ARM atomic | syscalls |
| Windows | `VirtualAlloc` | Interlocked* APIs | Win32 API |

### 10.3 WASI Preview 2 Host Functions

| WASI Function | Capability Required | Implementation |
|---------------|---------------------|----------------|
| `fd_read` | `fd-read` | WASI HAL |
| `fd_write` | `fd-write` | WASI HAL |
| `fd_seek` | `fd-seek` | WASI HAL |
| `fd_close` | `fd-close` | WASI HAL |
| `path_open` | `path-open` | WASI HAL |
| `random_get` | `random` | WASI HAL |
| `clock_time_get` | `clock-get` | WASI HAL |
| `sched_yield` | `sched-yield` | WASI HAL |
| `sock_open` | `sock-create` | Network Layer |
| `sock_connect` | `sock-connect` | Network Layer |
| `sock_send` | `sock-send` | Network Layer |
| `sock_recv` | `sock-recv` | Network Layer |

---

## BP-11: Compliance Matrix

### 11.1 WASI Preview 2 Compliance

| WASI Module | Function | Status | Notes |
|-------------|----------|--------|-------|
| **wasi:io** | | | |
| `poll` | `poll` | [PASS] Compliant | Event polling |
| `streams` | `read`, `write` | [PASS] Compliant | Stream operations |
| **wasi:cli** | | | |
| `stdin` | `get_stdin` | [PASS] Compliant | Standard input |
| `stdout` | `get_stdout` | [PASS] Compliant | Standard output |
| `stderr` | `get_stderr` | [PASS] Compliant | Standard error |
| `exit` | `exit` | [PASS] Compliant | Process exit |
| **wasi:filesystem** | | | |
| `types` | `Descriptor`, `DirEntry` | [PASS] Compliant | Filesystem types |
| `preopens` | `get_directories` | [PASS] Compliant | Pre-opened directories |
| **wasi:sockets** | | | |
| `tcp` | `create_tcp_socket` | [PASS] Compliant | TCP sockets |
| `udp` | `create_udp_socket` | [PASS] Compliant | UDP sockets |
| `ip-name-lookup` | `resolve_addresses` | [PASS] Compliant | DNS resolution |
| **wasi:random** | | | |
| `random` | `get_random_bytes` | [PASS] Compliant | Random bytes |
| `insecure` | `get_insecure_random` | [PASS] Compliant | Insecure random |
| **wasi:clocks** | | | |
| `wall-clock` | `now`, `resolution` | [PASS] Compliant | Wall clock |
| `monotonic-clock` | `now`, `resolution` | [PASS] Compliant | Monotonic clock |

### 11.2 WebAssembly Core Compliance

| Feature | Status | Notes |
|---------|--------|-------|
| **MVP** | [PASS] Compliant | All MVP features |
| **Mutable Globals** | [PASS] Compliant | `global.set` supported |
| **Sign Extension** | [PASS] Compliant | i32.extend8_s, etc. |
| **Bulk Memory** | [PASS] Compliant | memory.copy, memory.fill |
| **Reference Types** | [PASS] Compliant | externref, funcref |
| **Multi-Value** | [PASS] Compliant | Multi-value returns |
| **SIMD** | [PASS] Compliant | 128-bit SIMD |
| **Tail Call** | [PASS] Compliant | tail-call optimization |
| **Component Model** | Partial | Preview 2 support |
| **Threads** | [WARN] Not Supported | Shared memory disabled |

### 11.3 Security Compliance

| Standard | Requirement | Status | Implementation |
|----------|-------------|--------|----------------|
| CWE-119 | Buffer Overflow | [PASS] Mitigated | Bounds checking |
| CWE-125 | Out-of-bounds Read | [PASS] Mitigated | Memory isolation |
| CWE-787 | Out-of-bounds Write | [PASS] Mitigated | Memory isolation |
| CWE-400 | Uncontrolled Resource | [PASS] Mitigated | Fuel limiting |
| CWE-862 | Missing Authorization | [PASS] Mitigated | Capability enforcement |
| Spectre V1 | Bounds Check Bypass | [WARN] Mitigated | Spectre guards |
| Spectre V4 | Speculative Store Bypass | [WARN] Mitigated | Memory fences |

---

## BP-12: Quality Checklist

### 12.1 Document Completeness

| Section | Status | Notes |
|---------|--------|-------|
| BP-1: Design Overview | [PASS] Complete | Purpose, context, goals |
| BP-2: Design Decomposition | [PASS] Complete | 6 components specified |
| BP-3: Design Rationale | [PASS] Complete | Key decisions justified |
| BP-4: Traceability | [PASS] Complete | Mapped to YP theorems |
| BP-5: Interface Design | [PASS] Complete | 5 interfaces specified |
| BP-6: Data Design | [PASS] Complete | Structures defined |
| BP-7: Component Design | [PASS] Complete | Sequences documented |
| BP-8: Deployment Design | [PASS] Complete | Requirements specified |
| BP-9: Formal Verification | [PASS] Complete | 3 properties specified |
| BP-10: HAL Specification | [PASS] Complete | 3 HAL interfaces |
| BP-11: Compliance Matrix | [PASS] Complete | WASI/WASM compliance |
| BP-12: Quality Checklist | [PASS] Complete | This section |

### 12.2 IEEE 1016-2009 Compliance

| IEEE 1016 Section | BP Section | Status |
|-------------------|------------|--------|
| Design Overview | BP-1 | [PASS] |
| Design Decomposition | BP-2 | [PASS] |
| Design Rationale | BP-3 | [PASS] |
| Traceability | BP-4 | [PASS] |
| Interface Design | BP-5 | [PASS] |
| Data Design | BP-6 | [PASS] |
| Component Design | BP-7 | [PASS] |
| Deployment Design | BP-8 | [PASS] |

### 12.3 Review Status

| Reviewer | Date | Status | Comments |
|----------|------|--------|----------|
| Construct (Author) | 2026-03-05 | Draft | Initial version |
| _Pending_ | - | - | Peer review |
| _Pending_ | - | - | Security review |

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-03-05 | Construct | Initial Blue Paper creation |

---

*End of Blue Paper BP-WASM-ENGINE-001*
