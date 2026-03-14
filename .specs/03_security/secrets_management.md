# Secrets Management - Project Aether

**Document Version:** 1.0.0  
**Classification:** Confidential  
**Last Updated:** 2026-03-05  
**Author:** Security Engineering Team  

---

## Executive Summary

This document defines the secrets management strategy for Project Aether, encompassing the secure handling of certificates, cryptographic keys, API tokens, and other sensitive credentials. Aether implements a memory-only, never-to-disk secrets architecture with hardware-backed protection where available.

### Secrets Management Principles

| Principle | Description |
|-----------|-------------|
| Memory-only | Secrets never written to persistent storage in plaintext |
| Hardware-backed | TPM/SGX for key storage when available |
| Short-lived | Maximum 24-hour lifetime for most secrets |
| Automatic rotation | Secrets rotated automatically before expiration |
| Minimal exposure | Secrets exposed only to authorized components |
| Audit all access | All secret access logged |

---

## 1. Secret Types

### 1.1 Cryptographic Secrets

#### mTLS Certificates

| Property | Value |
|----------|-------|
| Type | X.509 certificates with ECDSA P-256 keys |
| Lifetime | 24 hours (maximum) |
| Storage | Memory only, TPM-backed private key |
| Rotation | Automatic, 4 hours before expiration |
| Access | Certificate manager component only |

**Certificate Types:**
| Certificate | Purpose | Lifetime |
|-------------|---------|----------|
| Node certificate | Node identity in mesh | 24 hours |
| Client certificate | API client authentication | 1 hour |
| Service certificate | Service-to-service auth | 24 hours |
| CA certificate | Certificate signing | 1 year |

---

#### Ed25519 Signing Keys

| Property | Value |
|----------|-------|
| Type | Ed25519 key pair |
| Lifetime | Module lifetime (ephemeral for modules) |
| Storage | Memory only, TPM-backed if available |
| Rotation | Per-module instantiation |
| Access | Signing service only |

**Key Types:**
| Key | Purpose | Lifetime |
|-----|---------|----------|
| Module signing key | Sign WASM modules | Permanent |
| State signing key | Sign state transitions | 24 hours |
| Configuration signing key | Sign configuration | 1 year |
| Audit signing key | Sign audit entries | 1 year |

---

#### Symmetric Encryption Keys

| Property | Value |
|----------|-------|
| Type | AES-256 or ChaCha20 |
| Lifetime | Per-session or 24 hours |
| Storage | Memory only |
| Rotation | Every 24 hours or on-demand |
| Access | Encryption service only |

**Key Types:**
| Key | Purpose | Lifetime |
|-----|---------|----------|
| Session key | Encrypt session data | Per-session |
| State encryption key | Encrypt state at rest | 24 hours |
| Log encryption key | Encrypt log entries | 24 hours |

---

### 1.2 Authentication Tokens

#### JWT Tokens

| Property | Value |
|----------|-------|
| Type | JWT with Ed25519 signature |
| Lifetime | 1 hour (maximum) |
| Storage | Memory only, never cached |
| Rotation | Refresh token flow |
| Access | API gateway only |

**Token Types:**
| Token | Purpose | Lifetime |
|-------|---------|----------|
| Access token | API access | 1 hour |
| Refresh token | Token refresh | 24 hours |
| Capability token | Capability grant | 1 hour |

---

#### Capability Tokens

| Property | Value |
|----------|-------|
| Type | Signed token with capability list |
| Lifetime | 1 hour (maximum) |
| Storage | Memory only |
| Rotation | Automatic refresh |
| Access | Capability manager only |

**Token Contents:**
```json
{
  "jti": "unique-token-id",
  "sub": "module-or-actor-id",
  "iat": 1709616000,
  "exp": 1709619600,
  "capabilities": [
    {
      "resource": "fs:/data/*",
      "actions": ["read", "write"]
    },
    {
      "resource": "net:tcp://*:8080",
      "actions": ["listen"]
    }
  ],
  "constraints": {
    "max_memory_mb": 256,
    "max_cpu_percent": 50
  }
}
```

---

### 1.3 Infrastructure Secrets

#### Database Credentials

| Property | Value |
|----------|-------|
| Type | Username/password |
| Lifetime | 24 hours |
| Storage | Memory only |
| Rotation | Automatic, daily |
| Access | State manager only |

---

#### Registry Credentials

| Property | Value |
|----------|-------|
| Type | Token-based authentication |
| Lifetime | 1 hour |
| Storage | Memory only |
| Rotation | On-demand |
| Access | Image puller only |

---

#### Cloud Provider Credentials

| Property | Value |
|----------|-------|
| Type | IAM role / Service account |
| Lifetime | 1 hour (STS) |
| Storage | Memory only |
| Rotation | Automatic (cloud provider) |
| Access | Cloud integration only |

---

## 2. Storage Strategy

### 2.1 Memory-Only Storage

All secrets are stored exclusively in memory. The secrets are never written to disk in plaintext.

**Implementation:**
```rust
pub struct SecretsStore {
    secrets: HashMap<SecretId, Zeroizing<SecretValue>>,
    #[cfg(feature = "tpm")]
    tpm: TpmContext,
}

pub struct Zeroizing<T> {
    value: T,
}

impl<T> Drop for Zeroizing<T> {
    fn drop(&mut self) {
        // Zeroize memory on drop
        unsafe {
            ptr::write_volatile(&mut self.value, std::mem::zeroed());
        }
        atomic::fence(atomic::Ordering::SeqCst);
    }
}
```

**Memory Protection:**
| Protection | Implementation |
|------------|----------------|
| Zeroization | Memory zeroed on deallocation |
| Locked memory | mlock() to prevent swapping |
| Secure allocator | Custom allocator with guard pages |
| Constant-time operations | Timing-safe comparisons |

---

### 2.2 Hardware Security Module (HSM) Integration

When TPM 2.0 or Intel SGX is available, keys are stored in hardware.

**TPM Integration:**
```rust
pub struct TpmSecrets {
    context: TpmContext,
    primary_key: KeyHandle,
}

impl TpmSecrets {
    pub fn generate_key(&mut self) -> Result<KeyHandle> {
        // Generate key in TPM, never leaves hardware
        self.context.create_key(
            self.primary_key,
            KeyParams::Ecc(EccCurve::NistP256),
        )
    }
    
    pub fn sign(&mut self, key: KeyHandle, data: &[u8]) -> Result<Signature> {
        // Sign in TPM, private key never exposed
        self.context.sign(key, data)
    }
}
```

**SGX Integration:**
```rust
pub struct SgxSecrets {
    enclave: SgxEnclave,
}

impl SgxSecrets {
    pub fn seal_secret(&self, secret: &[u8]) -> Result<SealedSecret> {
        // Seal secret to SGX enclave, can only be unsealed by same enclave
        self.enclave.call("seal_secret", secret)
    }
}
```

---

### 2.3 Secrets Isolation

Secrets are isolated by component and tenant.

**Isolation Model:**
```
┌─────────────────────────────────────────────────────────────┐
│                      Secrets Manager                         │
├─────────────────┬─────────────────┬─────────────────────────┤
│   Tenant A      │   Tenant B      │   System Secrets        │
├─────────────────┼─────────────────┼─────────────────────────┤
│ ┌─────────────┐ │ ┌─────────────┐ │ ┌─────────────────────┐ │
│ │ Cert Store  │ │ │ Cert Store  │ │ │ Node Certificates   │ │
│ └─────────────┘ │ └─────────────┘ │ └─────────────────────┘ │
│ ┌─────────────┐ │ ┌─────────────┐ │ ┌─────────────────────┐ │
│ │ Cap Tokens  │ │ │ Cap Tokens  │ │ │ CA Private Key      │ │
│ └─────────────┘ │ └─────────────┘ │ └─────────────────────┘ │
│ ┌─────────────┐ │ ┌─────────────┐ │ ┌─────────────────────┐ │
│ │ Enc Keys    │ │ │ Enc Keys    │ │ │ Audit Signing Key   │ │
│ └─────────────┘ │ └─────────────┘ │ └─────────────────────┘ │
└─────────────────┴─────────────────┴─────────────────────────┘
```

---

## 3. Injection Mechanisms

### 3.1 Runtime Injection

Secrets are injected at runtime via secure channels, not from configuration files.

**Injection Flow:**
```
┌──────────────┐     mTLS      ┌──────────────┐
│   Secrets    │──────────────▶│   Aether     │
│   Service    │               │   Runtime    │
└──────────────┘               └──────────────┘
       │                              │
       │ 1. Request secrets           │
       │ ◀────────────────────────────│
       │                              │
       │ 2. Authenticate              │
       │ ────────────────────────────▶│
       │                              │
       │ 3. Attestation               │
       │ ◀────────────────────────────│
       │                              │
       │ 4. Deliver secrets           │
       │ ────────────────────────────▶│
       │         (memory only)        │
```

**Implementation:**
```rust
pub async fn inject_secrets(
    secrets_service: &SecretsClient,
    component: ComponentId,
    attestation: Attestation,
) -> Result<SecretsBundle> {
    // Authenticate with attestation
    let session = secrets_service.authenticate(attestation).await?;
    
    // Request secrets for component
    let secrets = session.get_secrets(&component).await?;
    
    // Store in memory-only secrets store
    SECRETS_STORE.insert(component, secrets)?;
    
    Ok(secrets)
}
```

---

### 3.2 Environment Variable Injection (Avoided)

Aether **does not** use environment variables for secrets. Environment variables are:
- Visible in process listings
- Logged in crash dumps
- Propagated to child processes
- Stored in shell history

**Instead:** Secrets are injected via the secure injection API.

---

### 3.3 File-Based Injection (Avoided)

Aether **does not** use files for secrets. Files are:
- Visible on disk
- Backed up
- Cached by filesystem
- May persist after deletion

**Instead:** Secrets are stored in memory only.

---

### 3.4 WASM Module Secrets

WASM modules receive secrets via host functions with capability checks.

**Implementation:**
```rust
#[host_function("secrets.get")]
pub fn secrets_get(
    mut caller: Caller<'_, Context>,
    key_ptr: i32,
    key_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> Result<i32> {
    let context = caller.data_mut();
    
    // Check capability
    context.capabilities.require("secrets:read")?;
    
    // Get key from WASM memory
    let key = context.memory.get(key_ptr, key_len)?;
    
    // Get secret from store
    let secret = context.secrets.get(key)?;
    
    // Copy to WASM memory
    context.memory.put(out_ptr, &secret)?;
    
    Ok(secret.len() as i32)
}
```

---

## 4. Rotation Policy

### 4.1 Rotation Schedule

| Secret Type | Lifetime | Rotation Window | Automation |
|-------------|----------|-----------------|------------|
| Node certificates | 24 hours | 4 hours before expiry | Automatic |
| Client certificates | 1 hour | 10 minutes before expiry | Automatic |
| JWT access tokens | 1 hour | Refresh before expiry | Automatic |
| Capability tokens | 1 hour | Refresh before expiry | Automatic |
| Symmetric keys | 24 hours | 4 hours before expiry | Automatic |
| Database credentials | 24 hours | 4 hours before expiry | Automatic |
| CA certificates | 1 year | 30 days before expiry | Manual |

---

### 4.2 Rotation Process

**Certificate Rotation:**
```
┌─────────────────────────────────────────────────────────────┐
│                   Certificate Rotation                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  T-4h: New certificate issued                                │
│    │                                                         │
│    ▼                                                         │
│  T-4h: Old certificate marked for revocation                 │
│    │                                                         │
│    ▼                                                         │
│  T-4h to T: Grace period (both certs valid)                  │
│    │                                                         │
│    ▼                                                         │
│  T: Old certificate revoked                                  │
│    │                                                         │
│    ▼                                                         │
│  T+4h: Old certificate removed from cache                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Implementation:**
```rust
pub async fn rotate_certificates(
    cert_manager: &CertManager,
    node_id: &NodeId,
) -> Result<()> {
    // Generate new certificate
    let new_cert = cert_manager.issue_certificate(node_id).await?;
    
    // Store new certificate (grace period)
    cert_manager.add_certificate(node_id, new_cert.clone()).await?;
    
    // Schedule old certificate revocation
    let old_cert = cert_manager.get_current_certificate(node_id).await?;
    cert_manager.schedule_revocation(&old_cert, Duration::hours(4)).await?;
    
    // Activate new certificate
    cert_manager.set_active_certificate(node_id, new_cert.id).await?;
    
    // Notify peers of new certificate
    broadcast_new_certificate(node_id, new_cert).await?;
    
    Ok(())
}
```

---

### 4.3 Key Rotation

**Symmetric Key Rotation:**
```rust
pub async fn rotate_encryption_key(
    key_manager: &KeyManager,
    key_id: &KeyId,
) -> Result<()> {
    // Generate new key
    let new_key = key_manager.generate_key(KeyType::Aes256Gcm)?;
    
    // Re-encrypt data with new key
    let encrypted_data = key_manager.get_encrypted_data(key_id).await?;
    let plaintext = key_manager.decrypt(&encrypted_data)?;
    let new_encrypted = key_manager.encrypt_with(&plaintext, &new_key)?;
    
    // Atomic swap
    key_manager.swap_key(key_id, new_key, new_encrypted).await?;
    
    // Zeroize old key
    key_manager.zeroize_key(key_id)?;
    
    Ok(())
}
```

---

### 4.4 Emergency Rotation

In case of suspected compromise, emergency rotation can be triggered.

**Emergency Rotation Procedure:**
1. Revoke all certificates immediately
2. Issue new certificates
3. Invalidate all sessions
4. Force re-authentication
5. Audit all access logs

**Implementation:**
```rust
pub async fn emergency_rotation(
    secrets_manager: &SecretsManager,
    reason: &str,
) -> Result<()> {
    // Log emergency rotation
    audit_log(AuditEvent::EmergencyRotation { reason });
    
    // Revoke all certificates
    secrets_manager.revoke_all_certificates().await?;
    
    // Issue new certificates
    secrets_manager.issue_all_certificates().await?;
    
    // Invalidate all sessions
    session_manager.invalidate_all().await?;
    
    // Alert security team
    alert_security_team("Emergency rotation triggered").await?;
    
    Ok(())
}
```

---

## 5. Access Control

### 5.1 Secret Access Authorization

Access to secrets requires:
1. Valid authentication
2. Specific capability grant
3. Audit logging

**Access Control Matrix:**
| Component | Node Certs | CA Key | Module Keys | Tenant Secrets |
|-----------|------------|--------|-------------|----------------|
| Certificate Manager | Read/Write | Read | - | - |
| WASM Engine | - | - | Read | Per-capability |
| State Manager | - | - | - | Read |
| API Gateway | Read | - | - | - |

---

### 5.2 Capability-Based Secret Access

WASM modules can only access secrets they have explicit capabilities for.

**Capability Definition:**
```rust
pub struct SecretCapability {
    pub namespace: String,
    pub key_pattern: GlobPattern,
    pub actions: Vec<SecretAction>,
}

pub enum SecretAction {
    Read,
    Write,
    Delete,
}
```

**Example:**
```rust
// Grant read access to database credentials
let capability = SecretCapability {
    namespace: "tenant-a".to_string(),
    key_pattern: GlobPattern::new("db/*").unwrap(),
    actions: vec![SecretAction::Read],
};

capability_manager.grant(module_id, capability)?;
```

---

### 5.3 Audit Logging

All secret access is logged.

**Audit Event:**
```rust
pub struct SecretAccessAuditEvent {
    pub timestamp: DateTime<Utc>,
    pub component: ComponentId,
    pub secret_type: SecretType,
    pub secret_id: SecretId,
    pub action: SecretAction,
    pub result: Result<(), Error>,
    pub source_ip: Option<IpAddr>,
    pub session_id: Option<SessionId>,
}
```

**Log Format:**
```json
{
  "timestamp": "2026-03-05T12:34:56.789Z",
  "event_type": "secret_access",
  "component": "wasm-engine",
  "secret_type": "symmetric_key",
  "secret_id": "enc-key-001",
  "action": "read",
  "result": "success",
  "source_ip": "10.0.0.1",
  "session_id": "sess-123"
}
```

---

## 6. Secrets Lifecycle

### 6.1 Creation

```
┌─────────────────────────────────────────────────────────────┐
│                    Secret Creation                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Request ───▶ 2. Authorization ───▶ 3. Generation        │
│                          │                    │              │
│                          ▼                    ▼              │
│                    4. Audit Log ◀──── 5. Storage (memory)   │
│                                              │               │
│                                              ▼               │
│                                        6. Distribution       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

### 6.2 Usage

```
┌─────────────────────────────────────────────────────────────┐
│                    Secret Usage                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Request ───▶ 2. Capability Check ───▶ 3. Retrieval      │
│       │               │                        │             │
│       │               ▼                        ▼             │
│       │         4. Audit Log ◀──── 5. In-memory Use         │
│       │                                        │             │
│       │                                        ▼             │
│       └──────────────────────────────── 6. Zeroize Copy      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

### 6.3 Rotation

```
┌─────────────────────────────────────────────────────────────┐
│                    Secret Rotation                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Rotation Timer ───▶ 2. New Secret Generation            │
│         │                        │                           │
│         │                        ▼                           │
│         │                   3. Grace Period                  │
│         │                   (both valid)                     │
│         │                        │                           │
│         ▼                        ▼                           │
│    4. Audit Log ◀──── 5. Old Secret Revocation              │
│                              │                               │
│                              ▼                               │
│                         6. Old Secret Zeroization           │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

### 6.4 Destruction

```
┌─────────────────────────────────────────────────────────────┐
│                    Secret Destruction                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. Revocation Request ───▶ 2. Immediate Invalidation       │
│         │                          │                         │
│         │                          ▼                         │
│         │                     3. Memory Zeroization          │
│         │                          │                         │
│         ▼                          ▼                         │
│    4. Audit Log ◀──── 5. Cache Invalidation                 │
│                                 │                            │
│                                 ▼                            │
│                            6. Notification                   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. Backup and Recovery

### 7.1 Backup Strategy

Secrets are **not** backed up directly. Instead:
- Keys are derived from master key
- Master key is split using Shamir's Secret Sharing
- Shares are distributed to trusted custodians

**Implementation:**
```rust
pub fn backup_master_key(master_key: &[u8], threshold: u8, shares: u8) -> Result<Vec<KeyShare>> {
    // Split master key using Shamir's Secret Sharing
    let shares = shamir::split(master_key, threshold, shares)?;
    
    // Encrypt each share with custodian's public key
    let encrypted_shares: Vec<_> = shares
        .into_iter()
        .zip(CUSTODIANS.iter())
        .map(|(share, custodian)| {
            custodian.public_key().encrypt(&share)
        })
        .collect()?;
    
    Ok(encrypted_shares)
}
```

---

### 7.2 Recovery Procedure

1. Gather threshold number of key shares
2. Combine shares to reconstruct master key
3. Derive all keys from master key
4. Re-issue certificates
5. Audit entire process

**Implementation:**
```rust
pub async fn recover_master_key(shares: &[KeyShare]) -> Result<MasterKey> {
    // Verify threshold
    if shares.len() < THRESHOLD {
        return Err(Error::InsufficientShares);
    }
    
    // Decrypt shares
    let decrypted: Vec<_> = shares
        .iter()
        .map(|share| share.custodian.private_key().decrypt(share))
        .collect::<Result<_>>()?;
    
    // Combine shares
    let master_key = shamir::combine(&decrypted)?;
    
    // Audit recovery
    audit_log(AuditEvent::MasterKeyRecovery {
        shares_used: shares.len(),
        custodians: shares.iter().map(|s| s.custodian_id).collect(),
    });
    
    Ok(MasterKey::new(master_key))
}
```

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-03-05 | Security Engineering | Initial strategy |
