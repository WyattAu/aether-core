# Release Process

This document describes the release process for Aether.

## Version Numbering

Aether follows [Semantic Versioning](https://semver.org/) with the following format:

```
MAJOR.MINOR.PATCH[-PRERELEASE]
```

- **MAJOR**: Incompatible API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible
- **PRERELEASE**: Optional pre-release identifier
  - `alpha.N`: Early testing, unstable API
  - `beta.N`: Feature complete, testing phase
  - `rc.N`: Release candidate, final testing

### Examples

- `0.1.0` - Initial alpha release
- `0.2.0-alpha.1` - First alpha of 0.2.0
- `0.2.0-beta.1` - First beta of 0.2.0
- `0.2.0-rc.1` - First release candidate of 0.2.0
- `0.2.0` - Stable release
- `1.0.0` - First stable major release

## Release Types

### Patch Release (0.1.1, 0.1.2, etc.)

- Bug fixes
- Documentation updates
- Performance improvements
- No new features
- No breaking changes

**Process:**
1. Fix bugs in `develop` branch
2. Merge to `main`
3. Tag with patch version
4. Automated release builds

### Minor Release (0.2.0, 0.3.0, etc.)

- New features
- Backward compatible
- May include bug fixes

**Process:**
1. Develop features in feature branches
2. Merge to `develop`
3. Test thoroughly
4. Merge to `main`
5. Tag with minor version
6. Automated release builds

### Major Release (1.0.0, 2.0.0, etc.)

- Breaking changes
- Major new features
- Requires migration guide

**Process:**
1. Plan breaking changes
2. Develop in feature branches
3. Create migration guide
4. Extensive testing
5. Beta/RC releases
6. Merge to `main`
7. Tag with major version
8. Publish release notes

## Release Workflow

### Automated Release (Recommended)

The release process is automated via GitHub Actions. To create a release:

1. **Update version in Cargo.toml:**
   ```bash
   # Update workspace version
   # Also update individual crate versions if needed
   ```

2. **Commit and push:**
   ```bash
   git add Cargo.toml crates/*/Cargo.toml
   git commit -m "chore: bump version to x.y.z"
   git push origin main
   ```

3. **Create and push tag:**
   ```bash
   git tag -a vx.y.z -m "Release vx.y.z"
   git push origin vx.y.z
   ```

4. **Monitor the release:**
   - GitHub Actions will build binaries for all platforms
   - A GitHub release will be created automatically
   - Artifacts will be uploaded to the release

### Manual Release (Using Script)

Use the release script for more control:

```bash
# Patch release (0.1.0 -> 0.1.1)
./scripts/release.sh patch

# Minor release (0.1.0 -> 0.2.0)
./scripts/release.sh minor

# Major release (0.1.0 -> 1.0.0)
./scripts/release.sh major

# Dry run (no actual changes)
./scripts/release.sh patch --dry-run
```

The script will:
1. Check dependencies
2. Verify git status
3. Run tests
4. Run linting
5. Run security audit
6. Bump version
7. Build release binaries
8. Build WASM examples
9. Generate changelog
10. Create git tag
11. Push changes

## Release Checklist

Before creating a release:

- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Security audit clean
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in Cargo.toml
- [ ] Merged to main branch
- [ ] Tag created and pushed

## Changelog Format

Maintain a `CHANGELOG.md` file with the following format:

```markdown
# Changelog

## [Unreleased]

### Added
- New features

### Changed
- Changes to existing features

### Deprecated
- Features to be removed

### Removed
- Removed features

### Fixed
- Bug fixes

### Security
- Security fixes

## [0.1.0] - 2024-01-15

### Added
- Initial release
- Basic actor system
- WASM runtime support
```

## Pre-release Testing

Before a stable release:

1. **Alpha Testing:**
   - Internal testing
   - API experimentation
   - Early adopter feedback

2. **Beta Testing:**
   - Feature complete
   - Broader testing
   - Performance testing
   - Integration testing

3. **Release Candidate:**
   - Final testing
   - Documentation review
   - No new features
   - Only critical bug fixes

## Post-Release Tasks

After a release:

1. **Verify Release:**
   - Check GitHub release page
   - Verify download links
   - Test binaries on all platforms

2. **Update Documentation:**
   - Update installation docs
   - Update API documentation
   - Publish to GitHub Pages

3. **Announcements:**
   - GitHub Discussions
   - Social media
   - Discord/Slack

4. **Monitor:**
   - Watch for issue reports
   - Monitor crash reports
   - Check performance metrics

## Rollback Process

If a critical issue is found:

1. **Assess Severity:**
   - Security vulnerability
   - Data corruption
   - Breaking change

2. **Create Hotfix:**
   ```bash
   git checkout -b hotfix/vx.y.z+1 main
   # Fix the issue
   git commit -m "fix: critical issue"
   ./scripts/release.sh patch
   ```

3. **Yank Release (if necessary):**
   ```bash
   # Yank from crates.io
   cargo yank --version x.y.z
   
   # Mark as pre-release on GitHub
   # Update release notes with warning
   ```

## Release Automation

### GitHub Actions Workflows

The following workflows automate releases:

1. **`.github/workflows/release.yml`**
   - Triggered on version tags
   - Builds for multiple platforms
   - Creates GitHub release
   - Uploads artifacts

2. **`.github/workflows/security.yml`**
   - Security audit
   - Dependency scan
   - SARIF output

3. **`.github/workflows/benchmarks.yml`**
   - Performance benchmarks
   - Regression detection
   - Baseline comparison

4. **`.github/workflows/docs.yml`**
   - Documentation build
   - Deploy to GitHub Pages

5. **`.github/workflows/integration.yml`**
   - Integration tests
   - E2E tests
   - Matrix testing

### Required Secrets

Configure these secrets in GitHub:

- `CARGO_REGISTRY_TOKEN`: crates.io API token
- `GITHUB_TOKEN`: Automatically provided

## Platform Support

Releases are built for:

- **Linux:**
  - x86_64-unknown-linux-gnu
  - aarch64-unknown-linux-gnu (ARM64)

- **macOS:**
  - x86_64-apple-darwin (Intel)
  - aarch64-apple-darwin (Apple Silicon)

- **WASM:**
  - wasm32-wasip1

## Installation Methods

Users can install Aether via:

1. **Binary Download:**
   ```bash
   curl -LO https://github.com/aether-project/aether/releases/latest/download/aether-linux-x86_64.tar.gz
   tar xzf aether-linux-x86_64.tar.gz
   sudo mv aether /usr/local/bin/
   ```

2. **Cargo:**
   ```bash
   cargo install aether-cli
   ```

3. **From Source:**
   ```bash
   git clone https://github.com/aether-project/aether
   cd aether
   cargo install --path crates/cli
   ```

## Support

For release-related issues:

- GitHub Issues: https://github.com/aether-project/aether/issues
- Discussions: https://github.com/aether-project/aether/discussions
- Security: security@aether-project.io
