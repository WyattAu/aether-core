# Aether Dependency Specification

**Purpose:** Instructions for materializing, verifying, and managing dependencies.

---

## Quick Start

```bash
# 1. Clone repository
git clone https://forgejo.wyatt.au/Aether/aether-core.git
cd aether-core

# 2. Materialize dependencies
cargo fetch

# 3. Verify dependencies
cargo verify-project

# 4. Run security audit
cargo audit
```

---

## Dependency Materialization

### Standard Build

```bash
# Download and compile all dependencies
cargo build --release

# Development build with all features
cargo build --all-features
```

### Offline Build

For air-gapped environments:

```bash
# Pre-download dependencies (online machine)
cargo fetch

# Create vendor directory
cargo vendor vendor/

# Copy vendor/ and Cargo.lock to offline machine

# Configure cargo to use vendored deps
mkdir -p .cargo
cat > .cargo/config.toml << 'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF

# Build offline
cargo build --offline --release
```

### Docker Build

```dockerfile
FROM rust:1.82.0 AS builder

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo fetch

# Build application
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/aether /usr/local/bin/
CMD ["aether"]
```

---

## External Binaries

### Firecracker

```bash
# Download and verify
FIRECRACKER_VERSION="1.9.0"
ARCH=$(uname -m)

curl -Lo /tmp/firecracker-${FIRECRACKER_VERSION}-${ARCH}.tgz \
  https://github.com/firecracker-microvm/firecracker/releases/download/v${FIRECRACKER_VERSION}/firecracker-${FIRECRACKER_VERSION}-${ARCH}.tgz

# Verify SHA256
sha256sum -c << EOF
<sha256-from-supply_chain.lock>  /tmp/firecracker-${FIRECRACKER_VERSION}-${ARCH}.tgz
EOF

# Extract and install
tar -xzf /tmp/firecracker-${FIRECRACKER_VERSION}-${ARCH}.tgz -C /tmp
sudo mv /tmp/release-${FIRECRACKER_VERSION}-${ARCH}/firecracker-${FIRECRACKER_VERSION}-${ARCH} /usr/local/bin/firecracker
sudo mv /tmp/release-${FIRECRACKER_VERSION}-${ARCH}/jailer-${FIRECRACKER_VERSION}-${ARCH} /usr/local/bin/jailer
sudo chmod +x /usr/local/bin/firecracker /usr/local/bin/jailer
```

### FoundationDB

```bash
# Download and install
FDB_VERSION="7.3.51"

# Debian/Ubuntu
curl -Lo /tmp/foundationdb-clients_${FDB_VERSION}-1_amd64.deb \
  https://github.com/apple/foundationdb/releases/download/${FDB_VERSION}/foundationdb-clients_${FDB_VERSION}-1_amd64.deb

curl -Lo /tmp/foundationdb-server_${FDB_VERSION}-1_amd64.deb \
  https://github.com/apple/foundationdb/releases/download/${FDB_VERSION}/foundationdb-server_${FDB_VERSION}-1_amd64.deb

sudo dpkg -i /tmp/foundationdb-clients_${FDB_VERSION}-1_amd64.deb
sudo dpkg -i /tmp/foundationdb-server_${FDB_VERSION}-1_amd64.deb
```

---

## Verification

### Dependency Integrity

```bash
# Verify Cargo.lock matches Cargo.toml
cargo verify-project

# Check for duplicate dependencies
cargo tree --duplicates

# Verify all dependencies are accounted for
cargo deny check bans
```

### Security Audit

```bash
# Run security audit
cargo audit

# Check for known vulnerabilities
cargo audit --version

# Update advisory database
cargo audit -D
```

### License Compliance

```bash
# Check license compliance
cargo deny check licenses

# Generate license report
cargo about generate about.hbs > .reports/licenses.html

# Check for unlicensed dependencies
cargo deny check licenses --show-license-information
```

---

## Dependency Updates

### Check for Updates

```bash
# List outdated dependencies
cargo outdated

# Check for updates without applying
cargo update --dry-run
```

### Update Specific Dependency

```bash
# Update single dependency
cargo update -p wasmtime

# Update to specific version
cargo update -p wasmtime --precise 25.0.1
```

### Update All Dependencies

```bash
# Update all dependencies
cargo update

# Verify after update
cargo test
cargo audit
```

---

## Lockfile Management

### Regenerate Lockfile

```bash
# Remove and regenerate
rm Cargo.lock
cargo generate-lockfile

# Verify against supply_chain.lock
./scripts/verify-lockfile.sh
```

### Lockfile Verification Script

```bash
#!/bin/bash
# scripts/verify-lockfile.sh

echo "Verifying dependency lockfile..."

# Parse supply_chain.lock and verify Cargo.lock matches
cargo tree --prefix none --no-dedupe | while read dep; do
    name=$(echo "$dep" | cut -d' ' -f1)
    version=$(echo "$dep" | cut -d' ' -f2 | tr -d 'v')
    
    # Check against supply_chain.lock
    if ! grep -q "name = \"$name\"" .specs/01_5_supply_chain/supply_chain.lock; then
        echo "WARNING: $name not in supply_chain.lock"
    fi
done

echo "Lockfile verification complete."
```

---

## SBOM Generation

### Generate SBOM

```bash
# Using cargo-sbom
cargo install cargo-sbom
cargo sbom > .specs/01_5_supply_chain/sbom.spdx

# Using cyclonedx-bom
cargo install cargo-cyclonedx
cargo cyclonedx --format json --output .specs/01_5_supply_chain/sbom.cdx.json

# Using trivy
trivy fs --format spdx-json --output .specs/01_5_supply_chain/sbom-trivy.json .
```

### Merge SBOMs

```bash
# Combine Rust and binary SBOMs
./scripts/merge-sboms.sh
```

---

## CI/CD Integration

### GitHub Actions

```yaml
name: Dependency Verification

on: [push, pull_request]

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.82.0
      
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Fetch dependencies
        run: cargo fetch
      
      - name: Security audit
        run: |
          cargo install cargo-audit
          cargo audit
      
      - name: License check
        run: |
          cargo install cargo-deny
          cargo deny check licenses
      
      - name: Verify lockfile
        run: ./scripts/verify-lockfile.sh
      
      - name: Generate SBOM
        run: |
          cargo install cargo-sbom
          cargo sbom > sbom.spdx
      
      - name: Upload SBOM
        uses: actions/upload-artifact@v4
        with:
          name: sbom
          path: sbom.spdx
```

---

## Troubleshooting

### Dependency Resolution Issues

```bash
# Clear cargo cache
cargo clean
rm -rf ~/.cargo/registry/cache
rm -rf ~/.cargo/registry/index
rm -rf ~/.cargo/git

# Re-fetch
cargo fetch
```

### Version Conflicts

```bash
# Show dependency tree
cargo tree

# Find duplicates
cargo tree --duplicates

# Force specific version
cargo update -p <conflicting-dep> --precise <version>
```

### Feature Unification

```bash
# Check feature unification
cargo tree --features <feature>

# Build with specific features
cargo build --no-default-features --features <feature>
```

---

## Security Contacts

- **Security Team:** security@aether.dev
- **Vulnerability Reports:** security@aether.dev (PGP key available)

---

## Related Documents

- `.specs/01_5_supply_chain/supply_chain.lock` - Pinned dependency versions
- `.specs/01_5_supply_chain/sbom.spdx` - Software Bill of Materials
- `.specs/01_5_supply_chain/vulnerability_policy.md` - CVE handling
- `.specs/01_5_supply_chain/license_compliance.md` - License policy
