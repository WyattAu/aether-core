# Capability Security Model - Project Aether

**Document Version:** 1.0.0  
**Classification:** Confidential  
**Last Updated:** 2026-03-05  
**Author:** Security Engineering Team  

---

## Executive Summary

Project Aether implements a capability-based security model with deny-by-default semantics. Every operation requires an explicit capability grant, and capabilities are unforgeable, transferable only with explicit delegation, and immediately revocable. This document details the capability types, grant/revocation protocols, enforcement points, and audit mechanisms.

### Capability Model Principles

| Principle | Description |
|-----------|-------------|
| Deny-by-default | No operation allowed without explicit capability |
| Unforgeability | Capabilities are cryptographically signed |
| Delegation | Capabilities can be delegated with attenuation |
| Immediate revocation | Revocation takes effect within seconds |
| Auditability | All capability operations are logged |

---

## 1. Capability Types

### 1.1 Resource Capabilities

#### Filesystem Capabilities

| Capability | Description | Example |
|------------|-------------|---------|
| `fs:read` | Read files | `fs:read:/data/config.json` |
| `fs:write` | Write files | `fs:write:/data/output/*` |
| `fs:delete` | Delete files | `fs:delete:/tmp/*` |
| `fs:list` | List directories | `fs:list:/data/` |
| `fs:metadata` | Read file metadata | `fs:metadata:/data/*` |

**Capability Specification:**
```rust
pub struct FilesystemCapability {
    pub path_pattern: GlobPattern,
    pub permissions: HashSet<FilePermission>,
    pub constraints: FilesystemConstraints,
}

pub enum FilePermission {
    Read,
    Write,
    Delete,
    List,
    Metadata,
}

pub struct FilesystemConstraints {
    pub max_file_size: Option<u64>,
    pub max_total_size: Option<u64>,
    pub allowed_extensions: Option<Vec<String>>,
}
```

---

#### Network Capabilities

| Capability | Description | Example |
|------------|-------------|---------|
| `net:tcp:connect` | Outbound TCP | `net:tcp:connect:10.0.0.0/8:443` |
| `net:tcp:listen` | Listen on TCP | `net:tcp:listen:0.0.0.0:8080` |
| `net:udp:send` | Send UDP | `net:udp:send:*:53` |
| `net:udp:recv` | Receive UDP | `net:udp:recv:0.0.0.0:53` |
| `net:resolve` | DNS resolution | `net:resolve:*` |

**Capability Specification:**
```rust
pub struct NetworkCapability {
    pub protocol: Protocol,
    pub direction: Direction,
    pub address: AddressPattern,
    pub port: PortRange,
    pub constraints: NetworkConstraints,
}

pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
}

pub enum Direction {
    Inbound,
    Outbound,
    Both,
}

pub struct NetworkConstraints {
    pub max_bandwidth: Option<Bandwidth>,
    pub max_connections: Option<u32>,
    pub allowed_protocols: Option<Vec<String>>,
}
```

---

#### Compute Capabilities

| Capability | Description | Example |
|------------|-------------|---------|
| `compute:cpu` | CPU allocation | `compute:cpu:50%` |
| `compute:memory` | Memory allocation | `compute:memory:256MiB` |
| `compute:time` | Execution time | `compute:time:30s` |
| `compute:threads` | Thread creation | `compute:threads:4` |

**Capability Specification:**
```rust
pub struct ComputeCapability {
    pub cpu: CpuQuota,
    pub memory: MemoryQuota,
    pub time: TimeQuota,
    pub threads: ThreadQuota,
}

pub struct CpuQuota {
    pub percent: Option<u8>,
    pub cores: Option<u32>,
    pub shares: Option<u32>,
}

pub struct MemoryQuota {
    pub max_bytes: u64,
    pub swap_max_bytes: Option<u64>,
}
```

---

### 1.2 Host Function Capabilities

#### System Capabilities

| Capability | Description | Risk Level |
|------------|-------------|------------|
| `sys:clock` | Access wall clock | Low |
| `sys:random` | Secure random | Low |
| `sys:env` | Environment variables | Medium |
| `sys:args` | Command-line arguments | Low |
| `sys:exit` | Exit process | High |

---

#### Crypto Capabilities

| Capability | Description | Risk Level |
|------------|-------------|------------|
| `crypto:hash` | Hash functions | Low |
| `crypto:hmac` | HMAC | Low |
| `crypto:sign` | Signatures | Medium |
| `crypto:encrypt` | Symmetric encryption | Medium |
| `crypto:decrypt` | Symmetric decryption | Medium |
| `crypto:kdf` | Key derivation | Medium |

---

#### Secrets Capabilities

| Capability | Description | Risk Level |
|------------|-------------|------------|
| `secrets:read` | Read secrets | High |
| `secrets:write` | Write secrets | Critical |
| `secrets:list` | List secrets | Medium |
| `secrets:delete` | Delete secrets | Critical |

---

### 1.3 Module Capabilities

#### WASM Module Capabilities

| Capability | Description | Example |
|------------|-------------|---------|
| `wasm:instantiate` | Instantiate module | `wasm:instantiate:module-hash` |
| `wasm:call` | Call function | `wasm:call:module-hash:func-name` |
| `wasm:memory` | Access module memory | `wasm:memory:module-hash:read` |
| `wasm:table` | Access function table | `wasm:table:module-hash` |

---

#### Container Capabilities

| Capability | Description | Example |
|------------|-------------|---------|
| `container:create` | Create container | `container:create:image-digest` |
| `container:exec` | Execute in container | `container:exec:container-id` |
| `container:stop` | Stop container | `container:stop:container-id` |
| `container:logs` | Read logs | `container:logs:container-id` |

---

### 1.4 Capability Composition

Capabilities can be composed into capability sets.

```rust
pub struct CapabilitySet {
    pub id: CapabilitySetId,
    pub capabilities: Vec<Capability>,
    pub constraints: GlobalConstraints,
    pub inheritance: InheritancePolicy,
}

pub enum InheritancePolicy {
    None,
    Attenuated,
    Full,
}
```

**Predefined Capability Sets:**

| Set | Capabilities | Use Case |
|-----|--------------|----------|
| `minimal` | `sys:clock`, `sys:random`, `compute:cpu:10%`, `compute:memory:64MiB` | Basic WASM module |
| `network-client` | `minimal` + `net:tcp:connect:*:443`, `net:resolve:*` | HTTP client |
| `network-server` | `minimal` + `net:tcp:listen:0.0.0.0:8080` | HTTP server |
| `file-processor` | `minimal` + `fs:read:/input/*`, `fs:write:/output/*` | File processing |
| `compute-intensive` | `minimal` + `compute:cpu:100%`, `compute:memory:1GiB`, `compute:time:300s` | Computation |

---

## 2. Grant/Revocation Protocol

### 2.1 Capability Grant

#### Grant Request

```rust
pub struct CapabilityGrantRequest {
    pub requester: ActorId,
    pub capability: Capability,
    pub justification: String,
    pub duration: Option<Duration>,
    pub delegation_depth: Option<u8>,
}
```

#### Grant Flow

```
┌──────────────┐                    ┌──────────────┐
│   Requester  │                    │   Capability │
│   (Module)   │                    │   Manager    │
└──────────────┘                    └──────────────┘
       │                                   │
       │ 1. Request capability             │
       │ ─────────────────────────────────▶│
       │                                   │
       │                                   │ 2. Validate request
       │                                   │    - Actor authorized?
       │                                   │    - Capability exists?
       │                                   │    - Justification valid?
       │                                   │
       │                                   │ 3. Check policy
       │                                   │    - Auto-approve?
       │                                   │    - Require approval?
       │                                   │
       │                    ┌──────────────┤
       │                    │              │
       │                    │   ┌──────────┴──────────┐
       │                    │   │  Approval Workflow  │
       │                    │   │  (if required)      │
       │                    │   └──────────┬──────────┘
       │                    │              │
       │                    └──────────────┤
       │                                   │
       │                                   │ 4. Generate token
       │                                   │
       │ 5. Grant response                 │
       │ ◀─────────────────────────────────│
       │   - Capability token              │
       │   - Expiration                    │
       │   - Constraints                   │
       │                                   │
       │                                   │ 6. Audit log
       │                                   │
```

#### Grant Implementation

```rust
impl CapabilityManager {
    pub async fn grant_capability(
        &self,
        request: CapabilityGrantRequest,
    ) -> Result<CapabilityToken> {
        // Validate request
        self.validate_request(&request)?;
        
        // Check policy
        let policy = self.policy_for(&request.capability)?;
        if policy.requires_approval {
            self.request_approval(&request).await?;
        }
        
        // Generate capability token
        let token = CapabilityToken {
            id: TokenId::new(),
            actor: request.requester,
            capability: request.capability.clone(),
            granted_at: Utc::now(),
            expires_at: Utc::now() + request.duration.unwrap_or(Duration::hours(1)),
            granted_by: self.actor_id(),
            constraints: request.capability.constraints.clone(),
        };
        
        // Sign token
        let signed_token = self.signer.sign(&token)?;
        
        // Store token
        self.tokens.insert(token.id.clone(), signed_token.clone());
        
        // Audit log
        self.audit_log(AuditEvent::CapabilityGranted {
            token_id: token.id,
            actor: request.requester,
            capability: request.capability,
            duration: request.duration,
        });
        
        Ok(signed_token)
    }
}
```

---

### 2.2 Capability Delegation

Capabilities can be delegated with attenuation (reduced permissions).

```rust
pub struct DelegationRequest {
    pub delegator: ActorId,
    pub delegate: ActorId,
    pub capability: Capability,
    pub attenuation: Attenuation,
}

pub struct Attenuation {
    pub reduce_permissions: Option<HashSet<Permission>>,
    pub reduce_duration: Option<Duration>,
    pub reduce_scope: Option<Scope>,
    pub max_delegation_depth: u8,
}
```

**Delegation Rules:**
1. Can only delegate capabilities you possess
2. Delegated capability must be attenuated (no privilege escalation)
3. Delegation depth is limited (prevents infinite chains)
4. Delegation is logged

**Implementation:**
```rust
impl CapabilityManager {
    pub async fn delegate_capability(
        &self,
        request: DelegationRequest,
    ) -> Result<CapabilityToken> {
        // Verify delegator has capability
        let delegator_token = self.get_token(&request.delegator, &request.capability)?;
        
        // Apply attenuation
        let attenuated = self.apply_attenuation(
            &request.capability,
            &request.attenuation,
        )?;
        
        // Verify attenuation (no privilege escalation)
        if !attenuated.is_subset_of(&delegator_token.capability) {
            return Err(Error::AttenuationViolation);
        }
        
        // Check delegation depth
        if delegator_token.delegation_depth >= MAX_DELEGATION_DEPTH {
            return Err(Error::MaxDelegationDepthExceeded);
        }
        
        // Generate delegated token
        let token = CapabilityToken {
            id: TokenId::new(),
            actor: request.delegate,
            capability: attenuated,
            granted_at: Utc::now(),
            expires_at: delegator_token.expires_at.min(
                Utc::now() + request.attenuation.reduce_duration.unwrap_or(Duration::MAX)
            ),
            granted_by: request.delegator,
            delegation_depth: delegator_token.delegation_depth + 1,
            ..Default::default()
        };
        
        // Sign and store
        let signed_token = self.signer.sign(&token)?;
        self.tokens.insert(token.id.clone(), signed_token.clone());
        
        // Audit log
        self.audit_log(AuditEvent::CapabilityDelegated {
            token_id: token.id,
            delegator: request.delegator,
            delegate: request.delegate,
            capability: token.capability,
        });
        
        Ok(signed_token)
    }
}
```

---

### 2.3 Capability Revocation

#### Revocation Request

```rust
pub struct RevocationRequest {
    pub revoker: ActorId,
    pub token_id: TokenId,
    pub reason: RevocationReason,
}

pub enum RevocationReason {
    Compromised,
    NoLongerNeeded,
    PolicyViolation,
    Expiration,
    Manual,
}
```

#### Revocation Flow

```
┌──────────────┐                    ┌──────────────┐
│   Revoker    │                    │   Capability │
│              │                    │   Manager    │
└──────────────┘                    └──────────────┘
       │                                   │
       │ 1. Request revocation             │
       │ ─────────────────────────────────▶│
       │                                   │
       │                                   │ 2. Validate revoker
       │                                   │    - Authorized?
       │                                   │    - Own capability?
       │                                   │
       │                                   │ 3. Mark revoked
       │                                   │    - Update revocation list
       │                                   │    - Propagate to caches
       │                                   │
       │                                   │ 4. Notify affected actors
       │                                   │
       │ 5. Revocation confirmed           │
       │ ◀─────────────────────────────────│
       │                                   │
       │                                   │ 6. Audit log
       │                                   │
```

#### Revocation Implementation

```rust
impl CapabilityManager {
    pub async fn revoke_capability(
        &self,
        request: RevocationRequest,
    ) -> Result<()> {
        // Get token
        let token = self.tokens.get(&request.token_id)
            .ok_or(Error::TokenNotFound)?;
        
        // Validate revoker
        if !self.can_revoke(&request.revoker, &token) {
            return Err(Error::Unauthorized);
        }
        
        // Mark as revoked
        self.revocation_list.insert(
            request.token_id.clone(),
            RevocationEntry {
                revoked_at: Utc::now(),
                reason: request.reason.clone(),
                revoked_by: request.revoker.clone(),
            },
        );
        
        // Propagate revocation to caches
        self.propagate_revocation(&request.token_id).await?;
        
        // Notify affected actors
        self.notify_revocation(&token.actor, &request.token_id).await?;
        
        // Audit log
        self.audit_log(AuditEvent::CapabilityRevoked {
            token_id: request.token_id,
            revoker: request.revoker,
            reason: request.reason,
        });
        
        Ok(())
    }
    
    async fn propagate_revocation(&self, token_id: &TokenId) -> Result<()> {
        // Broadcast revocation to all nodes
        let revocation = RevocationMessage {
            token_id: token_id.clone(),
            timestamp: Utc::now(),
            signature: self.signer.sign(token_id)?,
        };
        
        self.mesh.broadcast(revocation).await?;
        
        Ok(())
    }
}
```

---

### 2.4 Revocation Propagation

Revocation must propagate quickly to all enforcement points.

**Propagation Mechanism:**
```
┌─────────────────────────────────────────────────────────────┐
│                  Revocation Propagation                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐      Broadcast      ┌─────────────┐        │
│  │   Node A    │────────────────────▶│   Node B    │        │
│  │ (revoker)   │                     │             │        │
│  └─────────────┘                     └─────────────┘        │
│        │                                    │               │
│        │ Broadcast                          │ Broadcast     │
│        ▼                                    ▼               │
│  ┌─────────────┐                     ┌─────────────┐        │
│  │   Node C    │                     │   Node D    │        │
│  │             │                     │             │        │
│  └─────────────┘                     └─────────────┘        │
│                                                              │
│  Propagation latency: < 5 seconds (P99)                     │
│  Consistency: eventual (with gossip)                         │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. Enforcement Points

### 3.1 WASM Engine Enforcement

The WASM engine enforces capabilities at host function boundaries.

```rust
impl WasmEngine {
    pub fn call_host_function(
        &mut self,
        function: &str,
        args: &[Value],
    ) -> Result<Vec<Value>> {
        // Get capability required for function
        let required = self.get_required_capability(function)?;
        
        // Check capability token
        let token = self.current_capability_token()?;
        if !token.has_capability(&required) {
            // Audit denial
            self.audit_log(AuditEvent::CapabilityDenied {
                actor: token.actor.clone(),
                required: required.clone(),
                granted: token.capabilities.clone(),
            });
            
            return Err(Error::CapabilityDenied(required));
        }
        
        // Check if revoked
        if self.is_revoked(&token.id)? {
            return Err(Error::CapabilityRevoked(token.id));
        }
        
        // Execute function
        let result = self.execute_host_function(function, args)?;
        
        // Audit success
        self.audit_log(AuditEvent::CapabilityUsed {
            actor: token.actor.clone(),
            capability: required,
        });
        
        Ok(result)
    }
}
```

**Host Function Capability Mapping:**
```rust
fn get_required_capability(function: &str) -> Result<Capability> {
    match function {
        // Filesystem
        "fs.read" => Ok(Capability::Filesystem(FilesystemCapability {
            permission: FilePermission::Read,
            ..Default::default()
        })),
        "fs.write" => Ok(Capability::Filesystem(FilesystemCapability {
            permission: FilePermission::Write,
            ..Default::default()
        })),
        
        // Network
        "net.tcp_connect" => Ok(Capability::Network(NetworkCapability {
            protocol: Protocol::Tcp,
            direction: Direction::Outbound,
            ..Default::default()
        })),
        "net.tcp_listen" => Ok(Capability::Network(NetworkCapability {
            protocol: Protocol::Tcp,
            direction: Direction::Inbound,
            ..Default::default()
        })),
        
        // System
        "sys.clock" => Ok(Capability::System(SystemCapability::Clock)),
        "sys.random" => Ok(Capability::System(SystemCapability::Random)),
        
        // Crypto
        "crypto.hash" => Ok(Capability::Crypto(CryptoCapability::Hash)),
        "crypto.encrypt" => Ok(Capability::Crypto(CryptoCapability::Encrypt)),
        
        // Secrets
        "secrets.get" => Ok(Capability::Secrets(SecretsCapability::Read)),
        
        _ => Err(Error::UnknownFunction(function.to_string())),
    }
}
```

---

### 3.2 Firecracker Manager Enforcement

The Firecracker manager enforces capabilities for container operations.

```rust
impl FirecrackerManager {
    pub async fn create_vm(
        &self,
        request: VmCreateRequest,
        capability_token: &CapabilityToken,
    ) -> Result<VmId> {
        // Check container:create capability
        let container_cap = Capability::Container(ContainerCapability::Create {
            image: request.image.clone(),
        });
        
        if !capability_token.has_capability(&container_cap) {
            return Err(Error::CapabilityDenied(container_cap));
        }
        
        // Check compute capabilities
        if let Some(cpu) = request.cpu_quota {
            if !capability_token.has_capability(&Capability::Compute(ComputeCapability::Cpu(cpu))) {
                return Err(Error::CapabilityDenied(Capability::Compute(ComputeCapability::Cpu(cpu))));
            }
        }
        
        if let Some(memory) = request.memory_quota {
            if !capability_token.has_capability(&Capability::Compute(ComputeCapability::Memory(memory))) {
                return Err(Error::CapabilityDenied(Capability::Compute(ComputeCapability::Memory(memory))));
            }
        }
        
        // Check network capabilities
        if let Some(port) = request.exposed_port {
            if !capability_token.has_capability(&Capability::Network(NetworkCapability {
                protocol: Protocol::Tcp,
                direction: Direction::Inbound,
                port: PortRange::single(port),
                ..Default::default()
            })) {
                return Err(Error::CapabilityDenied(...));
            }
        }
        
        // Create VM
        let vm_id = self.do_create_vm(request).await?;
        
        // Audit
        self.audit_log(AuditEvent::VmCreated {
            vm_id: vm_id.clone(),
            actor: capability_token.actor.clone(),
            capabilities_used: vec![container_cap],
        });
        
        Ok(vm_id)
    }
}
```

---

### 3.3 State Manager Enforcement

The state manager enforces capabilities for state access.

```rust
impl StateManager {
    pub async fn read_state(
        &self,
        key: &StateKey,
        capability_token: &CapabilityToken,
    ) -> Result<StateValue> {
        // Check state:read capability for key
        let state_cap = Capability::State(StateCapability::Read {
            key_pattern: GlobPattern::exact(key),
        });
        
        if !capability_token.has_capability(&state_cap) {
            return Err(Error::CapabilityDenied(state_cap));
        }
        
        // Read state
        let value = self.store.read(key).await?;
        
        // Audit
        self.audit_log(AuditEvent::StateRead {
            key: key.clone(),
            actor: capability_token.actor.clone(),
        });
        
        Ok(value)
    }
}
```

---

### 3.4 Mesh Network Enforcement

The mesh network enforces capabilities for inter-node communication.

```rust
impl MeshNetwork {
    pub async fn handle_message(
        &self,
        message: MeshMessage,
        peer_cert: &Certificate,
    ) -> Result<()> {
        // Extract capability token from message
        let capability_token = message.capability_token()?;
        
        // Verify token signature
        self.verify_token(&capability_token)?;
        
        // Check if revoked
        if self.revocation_cache.is_revoked(&capability_token.id)? {
            return Err(Error::CapabilityRevoked);
        }
        
        // Check capability for message type
        let required = self.get_required_capability(&message.message_type)?;
        if !capability_token.has_capability(&required) {
            return Err(Error::CapabilityDenied);
        }
        
        // Process message
        self.process_message(message).await
    }
}
```

---

## 4. Audit Logging

### 4.1 Audit Events

All capability operations are logged.

**Event Types:**
```rust
pub enum AuditEvent {
    // Grant events
    CapabilityGranted {
        token_id: TokenId,
        actor: ActorId,
        capability: Capability,
        duration: Option<Duration>,
    },
    
    // Delegation events
    CapabilityDelegated {
        token_id: TokenId,
        delegator: ActorId,
        delegate: ActorId,
        capability: Capability,
    },
    
    // Revocation events
    CapabilityRevoked {
        token_id: TokenId,
        revoker: ActorId,
        reason: RevocationReason,
    },
    
    // Usage events
    CapabilityUsed {
        actor: ActorId,
        capability: Capability,
        resource: Option<String>,
    },
    
    // Denial events
    CapabilityDenied {
        actor: ActorId,
        required: Capability,
        granted: CapabilitySet,
    },
}
```

---

### 4.2 Audit Log Format

```json
{
  "timestamp": "2026-03-05T12:34:56.789Z",
  "event_id": "evt-123456",
  "event_type": "capability_used",
  "actor": {
    "type": "wasm_module",
    "id": "module-hash-abc123",
    "tenant": "tenant-a"
  },
  "capability": {
    "type": "filesystem",
    "permission": "read",
    "path": "/data/config.json"
  },
  "resource": "/data/config.json",
  "result": "success",
  "source_ip": "10.0.0.1",
  "session_id": "sess-xyz",
  "correlation_id": "req-456"
}
```

---

### 4.3 Audit Log Storage

Audit logs are:
- Signed with Ed25519 key
- Chained with hash of previous entry
- Replicated across nodes
- Retained per compliance requirements

**Storage:**
```rust
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    signer: Ed25519Signer,
    last_hash: Hash,
}

pub struct AuditEntry {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub event: AuditEvent,
    pub prev_hash: Hash,
    pub hash: Hash,
    pub signature: Signature,
}

impl AuditLog {
    pub fn append(&mut self, event: AuditEvent) -> Result<()> {
        let entry = AuditEntry {
            sequence: self.entries.len() as u64,
            timestamp: Utc::now(),
            event,
            prev_hash: self.last_hash.clone(),
            hash: Hash::zeroed(),
            signature: Signature::zeroed(),
        };
        
        // Calculate hash
        let mut entry = entry;
        entry.hash = self.hash_entry(&entry);
        
        // Sign
        entry.signature = self.signer.sign(&entry.hash)?;
        
        // Append
        self.entries.push(entry.clone());
        self.last_hash = entry.hash;
        
        Ok(())
    }
}
```

---

## 5. Capability Token Format

### 5.1 Token Structure

```rust
pub struct CapabilityToken {
    // Header
    pub version: u8,
    pub algorithm: SignatureAlgorithm,
    
    // Claims
    pub id: TokenId,
    pub actor: ActorId,
    pub capabilities: Vec<Capability>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub granted_by: ActorId,
    pub delegation_depth: u8,
    
    // Constraints
    pub constraints: GlobalConstraints,
    
    // Proofs
    pub attestation: Option<Attestation>,
    
    // Signature
    pub signature: Signature,
}
```

### 5.2 Token Serialization

Tokens are serialized as CBOR with Ed25519 signature.

```rust
impl CapabilityToken {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        
        // Header
        bytes.push(1); // version
        bytes.push(SignatureAlgorithm::Ed25519 as u8);
        
        // Payload (CBOR)
        let payload = serde_cbor::to_vec(&TokenPayload {
            id: self.id.clone(),
            actor: self.actor.clone(),
            capabilities: self.capabilities.clone(),
            granted_at: self.granted_at,
            expires_at: self.expires_at,
            granted_by: self.granted_by.clone(),
            delegation_depth: self.delegation_depth,
            constraints: self.constraints.clone(),
        })?;
        bytes.extend_from_slice(&payload);
        
        // Signature
        bytes.extend_from_slice(&self.signature);
        
        Ok(bytes)
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        // Parse and verify
        let (header, payload, signature) = Self::parse(bytes)?;
        
        // Verify signature
        let public_key = Self::get_signer_public_key(&payload.granted_by)?;
        Ed25519Verifier::verify(&public_key, &payload.to_bytes()?, &signature)?;
        
        // Verify expiration
        if Utc::now() > payload.expires_at {
            return Err(Error::TokenExpired);
        }
        
        Ok(Self {
            version: header.version,
            algorithm: header.algorithm,
            id: payload.id,
            actor: payload.actor,
            capabilities: payload.capabilities,
            granted_at: payload.granted_at,
            expires_at: payload.expires_at,
            granted_by: payload.granted_by,
            delegation_depth: payload.delegation_depth,
            constraints: payload.constraints,
            signature,
        })
    }
}
```

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-03-05 | Security Engineering | Initial model |
