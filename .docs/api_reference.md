# Project Aether API Reference

**Version:** 1.0.0-alpha  
**Last Updated:** 2026-03-12

---

## Table of Contents

1. [Host Interface (WIT)](#1-host-interface-wit)
2. [CLI Commands](#2-cli-commands)
3. [Configuration Schema](#3-configuration-schema)
4. [Error Codes](#4-error-codes)

---

## 1. Host Interface (WIT)

### 1.1 Actor Interface

```wit
package aether:actor;

interface types {
    resource actor-id {
        to-string: func() -> string;
    }
    
    record message {
        sender: actor-id;
        payload: list<u8>;
        timestamp: u64;
        correlation-id: option<string>;
    }
    
    record response {
        payload: list<u8>;
        status: u32;
    }
    
    variant actor-error {
        not-found,
        timeout,
        capability-denied,
        out-of-memory,
        out-of-fuel,
        trap,
    }
}

interface handler {
    use types.{message, response, actor-error};
    
    handle: func(msg: message) -> result<response, actor-error>;
    on-start: func() -> result<(), actor-error>;
    on-stop: func() -> ();
}

world actor {
    export handler;
    
    import log: func(level: log-level, message: string);
    import send-message: func(to: actor-id, payload: list<u8>) -> result<response, actor-error>;
    import get-state: func(key: string) -> result<option<list<u8>>, actor-error>;
    import set-state: func(key: string, value: list<u8>) -> result<(), actor-error>;
    import get-config: func(key: string) -> option<string>;
    import get-env: func(key: string) -> option<string>;
}
```

### 1.2 Clocks Interface

```wit
package aether:clocks;

interface wall-clock {
    resource instant {
        now: func() -> instant;
        add: func(self, duration: duration) -> instant;
        sub: func(self, other: instant) -> duration;
    }
    
    record datetime {
        year: u16;
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    }
    
    to-datetime: func(instant: instant) -> datetime;
}

interface monotonic-clock {
    resource instant {
        now: func() -> instant;
        elapsed: func(self, earlier: instant) -> duration;
    }
    
    resolution: func() -> duration;
}
```

### 1.3 Filesystem Interface

```wit
package aether:filesystem;

interface types {
    resource descriptor {
        read-via-stream: func(offset: u64, len: u64) -> result<input-stream, error>;
        write-via-stream: func(offset: u64) -> result<output-stream, error>;
        read-directory: func() -> result<list<directory-entry>, error>;
        sync: func() -> result<(), error>;
        get-type: func() -> result<descriptor-type, error>;
        get-size: func() -> result<u64, error>;
        stat: func() -> result<metadata, error>;
        metadata-at: func(path: string) -> result<metadata, error>;
        remove-directory-at: func(path: string) -> result<(), error>;
        unlink-file-at: func(path: string) -> result<(), error>;
        rename-at: func(old-path: string, new-descriptor: descriptor, new-path: string) -> result<(), error>;
        readlink-at: func(path: string) -> result<string, error>;
        symlink-at: func(old-path: string, new-path: string) -> result<(), error>;
        open-at: func(path: string, flags: open-flags) -> result<descriptor, error>;
        create-directory-at: func(path: string) -> result<(), error>;
    }
    
    record metadata {
        type: descriptor-type;
        size: u64;
        accessed-timestamp: u64;
        modified-timestamp: u64;
        created-timestamp: u64;
    }
    
    enum descriptor-type {
        unknown,
        block-device,
        character-device,
        directory,
        fifo,
        symbolic-link,
        regular-file,
        socket,
    }
    
    flags open-flags: u16 {
        const create = 0x01;
        const directory = 0x02;
        const exclusive = 0x04;
        const truncate = 0x08;
        const read = 0x10;
        const write = 0x20;
    }
    
    variant error {
        access-denied,
        would-block,
        already-exists,
        bad-descriptor,
        busy,
        deadlock,
        quota-exceeded,
        invalid-seek,
        io,
        lookup,
        too-large,
        invalid-input,
        name-too-long,
        no-device,
        no-entry,
        not-directory,
        not-empty,
        not-tty,
        overflow,
        not-supported,
        pre-open-disabled,
        read-only,
        illegal-byte-sequence,
        no-such-device,
    }
}

interface preopens {
    use types.{descriptor};
    
    get-directories: func() -> list<tuple<string, descriptor>>;
}
```

### 1.4 Network Interface

```wit
package aether:sockets;

interface tcp {
    resource tcp-socket {
        bind: func(network: network, local-address: ip-address) -> result<(), error>;
        connect: func(network: network, remote-address: ip-socket-address) -> result<tuple<input-stream, output-stream>, error>;
        listen: func() -> result<(), error>;
        accept: func() -> result<tuple<tcp-socket, ip-socket-address>, error>;
        local-address: func() -> result<ip-socket-address, error>;
        remote-address: func() -> result<ip-socket-address, error>;
        address-family: func() -> ip-address-family;
        set-listen-backlog-size: func(value: u64) -> result<(), error>;
        keep-alive-enabled: func() -> result<bool, error>;
        set-keep-alive-enabled: func(value: bool) -> result<(), error>;
        receive-buffer-size: func() -> result<u64, error>;
        set-receive-buffer-size: func(value: u64) -> result<(), error>;
        send-buffer-size: func() -> result<u64, error>;
        set-send-buffer-size: func(value: u64) -> result<(), error>;
        hop-limit: func() -> result<u8, error>;
        set-hop-limit: func(value: u8) -> result<(), error>;
    }
    
    resource network {
        drop: func(self);
    }
    
    enum ip-address-family {
        ipv4,
        ipv6,
    }
    
    record ip-address {
        family: ip-address-family,
        value: list<u8>,
    }
    
    record ip-socket-address {
        address: ip-address,
        port: u16,
    }
    
    variant error {
        access-denied,
        already-bound,
        already-connected,
        already-listening,
        connection-refused,
        connection-reset,
        concurrent,
        in-progress,
        invalid-argument,
        invalid-state,
        name-unresolvable,
        not-bound,
        not-connected,
        not-listening,
        would-block,
        timeout,
        unreachable,
    }
}

interface udp {
    resource udp-socket {
        bind: func(network: network, local-address: ip-address) -> result<(), error>;
        connect: func(network: network, remote-address: ip-socket-address) -> result<(), error>;
        receive: func(max-results: u64) -> result<list<incoming-datagram>, error>;
        send: func(datagrams: list<outgoing-datagram>) -> result<u64, error>;
        local-address: func() -> result<ip-socket-address, error>;
        remote-address: func() -> result<ip-socket-address, error>;
        address-family: func() -> ip-address-family;
        unicast-hop-limit: func() -> result<u8, error>;
        set-unicast-hop-limit: func(value: u8) -> result<(), error>;
        receive-buffer-size: func() -> result<u64, error>;
        set-receive-buffer-size: func(value: u64) -> result<(), error>;
    }
    
    record incoming-datagram {
        data: list<u8>,
        source: ip-socket-address,
    }
    
    record outgoing-datagram {
        data: list<u8>,
        destination: option<ip-socket-address>,
    }
}

interface ip-name-lookup {
    use tcp.{network, ip-address, error};
    
    resolve-addresses: func(network: network, name: string) -> result<list<ip-address>, error>;
}
```

### 1.5 Random Interface

```wit
package aether:random;

interface random {
    get-random-bytes: func(len: u64) -> result<list<u8>, error>;
    get-random-u64: func() -> result<u64, error>;
    insecure: func() -> insecure-random;
    insecure-seed: func() -> tuple<u64, u64>;
    
    variant error {
        request-too-large,
        unreachable,
    }
}

interface insecure-random {
    get-insecure-random-bytes: func(len: u64) -> list<u8>;
    get-insecure-random-u64: func() -> u64;
}
```

### 1.6 Crypto Interface

```wit
package aether:crypto;

interface hash {
    resource digester {
        new: func(algorithm: hash-algorithm) -> result<digester, error>;
        update: func(self, data: list<u8>) -> result<(), error>;
        finish: func(self) -> result<list<u8>, error>;
    }
    
    enum hash-algorithm {
        sha256,
        sha384,
        sha512,
        blake2b-256,
        blake2b-512,
    }
    
    variant error {
        unsupported-algorithm,
        invalid-state,
    }
}

interface signature {
    resource key-pair {
        generate: func(algorithm: signature-algorithm) -> result<key-pair, error>;
        from-pkcs8: func(algorithm: signature-algorithm, encoded: list<u8>) -> result<key-pair, error>;
        public-key: func(self) -> public-key;
        sign: func(self, data: list<u8>) -> result<signature-output, error>;
    }
    
    resource public-key {
        from-raw: func(algorithm: signature-algorithm, encoded: list<u8>) -> result<public-key, error>;
        verify: func(self, data: list<u8>, signature: signature-output) -> result<(), error>;
    }
    
    resource signature-output {
        from-raw: func(algorithm: signature-algorithm, encoded: list<u8>) -> result<signature-output, error>;
        to-raw: func(self) -> list<u8>;
    }
    
    enum signature-algorithm {
        ed25519,
        ecdsa-p256-sha256,
        ecdsa-p384-sha384,
    }
}
```

---

## 2. CLI Commands

### 2.1 Command Overview

| Command | Description |
|---------|-------------|
| `aether init` | Initialize new project |
| `aether apply` | Apply configuration |
| `aether destroy` | Destroy actors |
| `aether status` | Show status |
| `aether logs` | View logs |
| `aether metrics` | Show metrics |
| `aether invoke` | Invoke actor |
| `aether exec` | Execute in container |
| `aether port-forward` | Forward port |
| `aether daemon` | Run daemon |

### 2.2 Global Flags

```
Flags:
  -c, --config <FILE>     Configuration file (default: aether.toml)
      --context <NAME>    Context name
      --no-color          Disable colored output
  -o, --output <FORMAT>   Output format: table, json, yaml
  -q, --quiet             Suppress output
  -v, --verbose           Increase verbosity
  -h, --help              Show help
  -V, --version           Show version
```

### 2.3 Command Details

#### init

```bash
aether init [OPTIONS] <NAME>

Arguments:
  <NAME>                   Project name

Options:
  -t, --template <TYPE>    Template: wasm, oci, mixed (default: wasm)
  -d, --directory <DIR>    Target directory (default: NAME)
      --rust               Create Rust WASM project
      --go                 Create Go WASM project
      --python             Create Python WASM project
```

#### apply

```bash
aether apply [OPTIONS]

Options:
  -f, --file <FILE>        Configuration file (default: aether.toml)
      --dry-run            Validate without applying
      --wait               Wait for readiness
      --timeout <DUR>      Timeout (default: 60s)
      --force              Force apply changes
      --prune              Remove orphaned resources
```

#### destroy

```bash
aether destroy [OPTIONS] [ACTOR]

Arguments:
  [ACTOR]                  Actor name (all if not specified)

Options:
      --force              Force destruction
      --keep-volumes       Preserve volumes
      --grace-period <DUR> Grace period (default: 30s)
```

#### status

```bash
aether status [OPTIONS] [ACTOR]

Arguments:
  [ACTOR]                  Actor name

Options:
  -w, --watch              Watch mode
      --all-namespaces     All namespaces
  -l, --selector <EXPR>    Label selector
```

#### logs

```bash
aether logs [OPTIONS] <ACTOR>

Arguments:
  <ACTOR>                  Actor name

Options:
  -f, --follow             Follow output
      --tail <N>           Lines to show (default: 100)
      --since <TIME>       Since time
      --until <TIME>       Until time
      --level <LEVEL>      Log level filter
      --timestamps         Show timestamps
```

#### invoke

```bash
aether invoke [OPTIONS] <ACTOR>

Arguments:
  <ACTOR>                  Actor name

Options:
  -m, --message <DATA>     Message payload
  -f, --file <FILE>        Payload from file
      --timeout <DUR>      Timeout (default: 30s)
      --async              Async invocation
      --correlation <ID>   Correlation ID
```

---

## 3. Configuration Schema

### 3.1 Root Schema

```toml
version = "1.0"              # Required: Schema version

[settings]                   # Optional: Global settings
log-level = "info"           # Log level: trace, debug, info, warn, error
shutdown-timeout = "30s"     # Graceful shutdown timeout
max-actors = 10000           # Maximum actors per node
max-memory = "32GiB"         # Maximum memory usage

[node]                       # Optional: Node configuration
id = "node-1"                # Node identifier (auto-generated if not set)
labels = { zone = "a" }      # Node labels
annotations = { }            # Node annotations
```

### 3.2 Actor Schema

```toml
[actors.<name>]              # Actor definition
runtime = "wasm"             # Required: "wasm" or "oci"

# WASM-specific
module = "./actor.wasm"      # Path to WASM module
entrypoint = "_start"        # Entrypoint function

# OCI-specific
image = "postgres:15"        # Container image
command = ["/bin/sh"]        # Override command
args = ["-c", "echo hello"]  # Command arguments
working-dir = "/app"         # Working directory
user = "app"                 # Run as user

# Common
capabilities = []            # Capability grants
instances = 1                # Number of instances
memory = "64MiB"             # Memory limit
cpu = "10%"                  # CPU limit
timeout = "30s"              # Invocation timeout
env = { }                    # Environment variables
```

### 3.3 Capability Schema

```toml
# Filesystem
capabilities = ["fs:read:/data/*", "fs:write:/output/*"]

# Network
capabilities = ["net:tcp:connect:10.0.0.0/8:443"]
capabilities = ["net:tcp:listen:0.0.0.0:8080"]

# Compute
capabilities = ["compute:cpu:50%", "compute:memory:512MiB"]

# Predefined sets
capabilities = ["network-client", "file-processor"]
```

### 3.4 Volume Schema

```toml
[actors.<name>.volumes.<vol-name>]
path = "/data"               # Mount path
size = "10GiB"               # Volume size
storage-class = "fast-ssd"   # Storage class
access-mode = "ReadWriteOnce" # Access mode

# Existing volume
volume-id = "vol-123"        # Use existing volume

# Secret volume
secret = { name = "tls-cert" }

# ConfigMap volume
config-map = { name = "config" }
```

### 3.5 Placement Schema

```toml
[actors.<name>.placement]
node-name = "node-1"         # Pin to specific node
node-selector = { gpu = "true" }  # Node labels
anti-affinity = ["api"]      # Anti-affinity rules

[[actors.<name>.placement.tolerations]]
key = "dedicated"
operator = "Equal"
value = "gpu"
effect = "NoSchedule"
```

### 3.6 Secrets Schema

```toml
[secrets.<name>]
source = "env"               # Source: env, file, vault
value = "ENV_VAR_NAME"       # Environment variable name

# File source
[secrets.tls-cert]
source = "file"
path = "/etc/certs/tls.pem"

# Inline (not recommended for production)
[secrets.api-key]
source = "inline"
value = "secret-key-value"
```

### 3.7 Network Schema

```toml
[networks.<name>]
cidr = "10.0.0.0/16"         # Network CIDR
gateway = "10.0.0.1"         # Gateway IP
dns-servers = ["8.8.8.8"]    # DNS servers

# Network policies
[networks.<name>.policies]
default-deny-ingress = true
default-deny-egress = false
```

### 3.8 Complete Example

```toml
version = "1.0"

[settings]
log-level = "info"
shutdown-timeout = "30s"

[node]
labels = { region = "us-west", zone = "a" }

[actors.api]
runtime = "wasm"
module = "./api.wasm"
capabilities = [
    "net:tcp:listen:0.0.0.0:8080",
    "net:tcp:connect:10.0.0.0/8:5432",
    "compute:cpu:25%",
    "compute:memory:256MiB"
]
instances = 3
timeout = "10s"

[actors.api.env]
LOG_LEVEL = "debug"
DB_HOST = "db"

[actors.api.placement]
node-selector = { zone = "a" }
anti-affinity = ["api"]

[actors.db]
runtime = "oci"
image = "postgres:15-alpine"
capabilities = [
    "compute:cpu:100%",
    "compute:memory:1GiB",
    "net:tcp:listen:0.0.0.0:5432",
    "fs:read:/data",
    "fs:write:/data"
]

[actors.db.volumes.data]
path = "/var/lib/postgresql/data"
size = "50GiB"
storage-class = "fast-ssd"

[actors.db.env]
POSTGRES_DB = "app"
POSTGRES_USER = "app"
POSTGRES_PASSWORD = { secret = "db-password" }

[secrets.db-password]
source = "env"
value = "AETHER_DB_PASSWORD"

[networks.default]
cidr = "10.0.0.0/16"
```

---

## 4. Error Codes

### 4.1 System Error Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | `SUCCESS` | Operation successful |
| 1 | `UNKNOWN` | Unknown error |
| 2 | `INVALID_ARGUMENT` | Invalid argument |
| 3 | `NOT_FOUND` | Resource not found |
| 4 | `ALREADY_EXISTS` | Resource already exists |
| 5 | `PERMISSION_DENIED` | Permission denied |
| 6 | `UNAUTHENTICATED` | Authentication required |
| 7 | `RESOURCE_EXHAUSTED` | Resource exhausted |
| 8 | `FAILED_PRECONDITION` | Precondition failed |
| 9 | `ABORTED` | Operation aborted |
| 10 | `OUT_OF_RANGE` | Out of range |
| 11 | `UNIMPLEMENTED` | Not implemented |
| 12 | `INTERNAL` | Internal error |
| 13 | `UNAVAILABLE` | Service unavailable |
| 14 | `DATA_LOSS` | Data loss |
| 15 | `TIMEOUT` | Operation timeout |

### 4.2 Actor Error Codes

| Code | Name | Description |
|------|------|-------------|
| 100 | `ACTOR_NOT_FOUND` | Actor not found |
| 101 | `ACTOR_CRASHED` | Actor crashed |
| 102 | `ACTOR_TIMEOUT` | Actor invocation timeout |
| 103 | `ACTOR_OUT_OF_MEMORY` | Actor out of memory |
| 104 | `ACTOR_OUT_OF_FUEL` | Actor out of fuel |
| 105 | `ACTOR_TRAP` | WASM trap |
| 106 | `ACTOR_INVALID_STATE` | Invalid actor state |
| 107 | `ACTOR_MIGRATION_FAILED` | Migration failed |

### 4.3 Capability Error Codes

| Code | Name | Description |
|------|------|-------------|
| 200 | `CAPABILITY_DENIED` | Capability not granted |
| 201 | `CAPABILITY_REVOKED` | Capability revoked |
| 202 | `CAPABILITY_EXPIRED` | Capability token expired |
| 203 | `CAPABILITY_INVALID` | Invalid capability token |
| 204 | `CAPABILITY_INSUFFICIENT` | Insufficient capabilities |

### 4.4 Network Error Codes

| Code | Name | Description |
|------|------|-------------|
| 300 | `CONNECTION_REFUSED` | Connection refused |
| 301 | `CONNECTION_RESET` | Connection reset |
| 302 | `CONNECTION_TIMEOUT` | Connection timeout |
| 303 | `HOST_UNREACHABLE` | Host unreachable |
| 304 | `NETWORK_UNREACHABLE` | Network unreachable |
| 305 | `DNS_RESOLUTION_FAILED` | DNS resolution failed |
| 306 | `TLS_HANDSHAKE_FAILED` | TLS handshake failed |

### 4.5 Storage Error Codes

| Code | Name | Description |
|------|------|-------------|
| 400 | `VOLUME_NOT_FOUND` | Volume not found |
| 401 | `VOLUME_ALREADY_MOUNTED` | Volume already mounted |
| 402 | `VOLUME_FULL` | Volume full |
| 403 | `IO_ERROR` | I/O error |
| 404 | `CHECKPOINT_FAILED` | Checkpoint failed |
| 405 | `STATE_CORRUPTED` | State corrupted |

### 4.6 Error Response Format

```json
{
  "error": {
    "code": 200,
    "name": "CAPABILITY_DENIED",
    "message": "Capability 'net:tcp:connect:*:443' not granted to actor 'worker'",
    "details": {
      "actor": "worker",
      "capability": "net:tcp:connect:*:443",
      "granted_capabilities": ["compute:cpu:10%", "compute:memory:64MiB"]
    },
    "request_id": "req-abc123",
    "timestamp": "2026-03-06T12:00:00Z"
  }
}
```

### 4.7 Error Handling Best Practices

1. **Check error codes, not messages**: Error messages may change, codes are stable
2. **Handle specific errors**: Don't catch all errors generically
3. **Retry transient errors**: Codes 13, 14, 302 are typically transient
4. **Log error details**: Include request_id for debugging
5. **Respect rate limits**: Code 7 indicates backoff needed

```rust
match result {
    Ok(response) => handle_response(response),
    Err(Error::CapabilityDenied { capability }) => {
        log::warn!("Missing capability: {}", capability);
        request_capability(capability)?;
    }
    Err(Error::Timeout { duration }) => {
        log::info!("Request timed out after {:?}", duration);
        retry_with_backoff(|| invoke_actor())?;
    }
    Err(e) => return Err(e.into()),
}
```

---

## Appendix: Type Definitions

### Duration Format

Duration strings: `10s`, `5m`, `2h`, `100ms`, `500us`

### Size Format

Size strings: `64MiB`, `1GiB`, `500KiB`, `2TB`

### Capability Format

```
<domain>:<action>:<resource>[:<constraint>]

Examples:
  fs:read:/data/*:max-size=1GiB
  net:tcp:connect:10.0.0.0/8:443
  compute:cpu:50%
```

---

## 5. MCP Tools (Model Context Protocol)

### 5.1 Overview

Aether provides MCP-compatible tools for AI integration, enabling AI assistants to interact with actors, files, memory, and execution contexts.

### 5.2 File Tools

#### read_file
Read file contents from the context directory.

```json
{
  "name": "read_file",
  "description": "Read file contents",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": { "type": "string", "description": "File path relative to context" }
    },
    "required": ["path"]
  }
}
```

**Required Capabilities:** `fs:read`

**Returns:** File contents as text

#### write_file
Write content to a file in the context directory.

```json
{
  "name": "write_file",
  "description": "Write content to file",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": { "type": "string" },
      "content": { "type": "string" }
    },
    "required": ["path", "content"]
  }
}
```

**Required Capabilities:** `fs:write`

#### list_directory
List directory contents.

```json
{
  "name": "list_directory",
  "description": "List directory contents",
  "input_schema": {
    "type": "object",
    "properties": {
      "path": { "type": "string" }
    },
    "required": ["path"]
  }
}
```

**Required Capabilities:** `fs:read`

#### search_files
Search for files matching a pattern.

```json
{
  "name": "search_files",
  "description": "Search for files",
  "input_schema": {
    "type": "object",
    "properties": {
      "pattern": { "type": "string" },
      "path": { "type": "string" }
    },
    "required": ["pattern"]
  }
}
```

**Required Capabilities:** `fs:read`

### 5.3 Execution Tools

#### execute_command
Execute a shell command.

```json
{
  "name": "execute_command",
  "description": "Execute a shell command",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": { "type": "string" },
      "args": { "type": "array", "items": { "type": "string" } },
      "timeout_ms": { "type": "integer" },
      "cwd": { "type": "string" }
    },
    "required": ["command"]
  }
}
```

**Required Capabilities:** `exec:run`

**Returns:** `{ "stdout": "...", "stderr": "...", "exit_code": 0, "success": true }`

#### execute_wasm
Execute a WASM module.

```json
{
  "name": "execute_wasm",
  "description": "Execute WASM module",
  "input_schema": {
    "type": "object",
    "properties": {
      "module_path": { "type": "string" },
      "function": { "type": "string" },
      "args": { "type": "array" },
      "timeout_ms": { "type": "integer" }
    },
    "required": ["module_path", "function"]
  }
}
```

**Required Capabilities:** `wasm:execute`

### 5.4 Actor Tools

#### invoke_actor
Invoke an actor with a message.

```json
{
  "name": "invoke_actor",
  "description": "Invoke actor with message",
  "input_schema": {
    "type": "object",
    "properties": {
      "actor_id": { "type": "string" },
      "message": { "type": "string" },
      "timeout_ms": { "type": "integer" }
    },
    "required": ["actor_id", "message"]
  }
}
```

**Required Capabilities:** `actor:invoke`

#### spawn_actor
Spawn a new actor.

```json
{
  "name": "spawn_actor",
  "description": "Spawn new actor",
  "input_schema": {
    "type": "object",
    "properties": {
      "module_path": { "type": "string" },
      "name": { "type": "string" },
      "capabilities": { "type": "array", "items": { "type": "string" } }
    },
    "required": ["module_path"]
  }
}
```

**Required Capabilities:** `actor:spawn`

#### list_actors
List active actors.

```json
{
  "name": "list_actors",
  "description": "List active actors",
  "input_schema": { "type": "object", "properties": {} }
}
```

**Required Capabilities:** None (read-only)

#### get_actor_status
Get actor status and metrics.

```json
{
  "name": "get_actor_status",
  "description": "Get actor status",
  "input_schema": {
    "type": "object",
    "properties": {
      "actor_id": { "type": "string" }
    },
    "required": ["actor_id"]
  }
}
```

### 5.5 Memory Tools

#### store_memory
Store a memory entry.

```json
{
  "name": "store_memory",
  "description": "Store memory entry",
  "input_schema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" },
      "role": { "type": "string", "enum": ["user", "assistant", "system", "tool"] },
      "content": { "type": "string" },
      "tags": { "type": "array", "items": { "type": "string" } },
      "ttl_seconds": { "type": "integer" }
    },
    "required": ["id", "role", "content"]
  }
}
```

#### recall_memory
Recall memory by ID.

```json
{
  "name": "recall_memory",
  "description": "Recall memory by ID",
  "input_schema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" }
    },
    "required": ["id"]
  }
}
```

#### search_memory
Search memory by query.

```json
{
  "name": "search_memory",
  "description": "Search memory",
  "input_schema": {
    "type": "object",
    "properties": {
      "query": { "type": "string" },
      "limit": { "type": "integer" }
    },
    "required": ["query"]
  }
}
```

#### memory_stats
Get memory statistics.

```json
{
  "name": "memory_stats",
  "description": "Get memory statistics",
  "input_schema": { "type": "object", "properties": {} }
}
```

#### clear_memory
Clear all memory (requires confirmation).

```json
{
  "name": "clear_memory",
  "description": "Clear all memory",
  "input_schema": {
    "type": "object",
    "properties": {
      "confirm": { "type": "boolean" }
    },
    "required": ["confirm"]
  }
}
```

### 5.6 AI-Actor Integration

#### actor_ai_interact
Interact with actors from AI context.

```json
{
  "name": "actor_ai_interact",
  "description": "Interact with actors from AI context",
  "input_schema": {
    "type": "object",
    "properties": {
      "action": {
        "type": "string",
        "enum": ["get_context", "store", "respond", "pending", "history"]
      },
      "request_id": { "type": "string" },
      "content": { "type": "string" },
      "query": { "type": "string" },
      "memory_entry": {
        "type": "object",
        "properties": {
          "id": { "type": "string" },
          "role": { "type": "string" },
          "content": { "type": "string" }
        }
      }
    },
    "required": ["action"]
  }
}
```

**Actions:**
| Action | Description |
|--------|-------------|
| `get_context` | Search memory for context |
| `store` | Store memory entry |
| `respond` | Respond to pending request |
| `pending` | List pending requests |
| `history` | Get conversation history |

---

## 6. Session Management

### 6.1 Session API

Sessions provide conversation management with checkpointing and branching.

```rust
use aether::context::{Session, SessionManager, MessageRole};

// Create session
let session = Session::new("session-1", "AI Assistant");

// Add messages
session.add_message(MessageRole::User, "Hello!");
session.add_message(MessageRole::Assistant, "Hi there!");

// Create checkpoint
let checkpoint_id = session.checkpoint()?;

// Branch from checkpoint
let branch = session.branch("experiment-1")?;

// Restore checkpoint
session.restore_checkpoint(&checkpoint_id)?;
```

### 6.2 Session Manager

```rust
let manager = SessionManager::new();

// Create session
let session = manager.create_session("main", "Main Session")?;

// List sessions
let sessions = manager.list_sessions();

// Get session
let session = manager.get_session("main")?;

// Delete session
manager.delete_session("main")?;
```

---

## 7. Persistent Memory

### 7.1 Memory Store

```rust
use aether::context::{PersistentMemoryStore, MemoryEntry};

let store = PersistentMemoryStore::new("./memory.json");

// Add entry
let entry = MemoryEntry::new("id-1", "user", "Hello world")
    .with_tag("greeting")
    .with_ttl(Duration::from_secs(3600));
store.add(entry);

// Search
let results = store.search("hello");

// Get by tag
let tagged = store.by_tag("greeting");

// Stats
let stats = store.stats();
```

### 7.2 Memory Entry Schema

```json
{
  "id": "entry-123",
  "role": "user",
  "content": "Message content",
  "tags": ["tag1", "tag2"],
  "metadata": {
    "source": "chat",
    "timestamp": "2026-03-14T12:00:00Z"
  },
  "created_at": "2026-03-14T12:00:00Z",
  "ttl": 3600
}
```

---

## 8. Capability System

### 8.1 AI Capabilities

| Capability | Description |
|------------|-------------|
| `AI_USE` | Use AI services from actors |
| `SESSION_ACCESS` | Access session management |

### 8.2 Checking Capabilities

```rust
use aether::capability::CapabilitySet;

let caps = CapabilitySet::new();
caps.add(CapabilitySet::AI_USE);
caps.add(CapabilitySet::SESSION_ACCESS);

if caps.contains(CapabilitySet::AI_USE) {
    // Allow AI operations
}
```

---

*For more information, visit https://aether.dev/docs/api*
