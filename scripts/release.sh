#!/bin/bash
# Enhanced Release Automation Script

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

VERSION=${1:-"patch"}
DRY_RUN=${2:-"false"}

if [[ "$VERSION" == "--dry-run" ]]; then
    DRY_RUN="true"
    VERSION="patch"
fi

log_info "🚀 Starting release process (version: $VERSION, dry-run: $DRY_RUN)"

check_dependencies() {
    log_info "Checking dependencies..."
    
    local missing_deps=()
    
    command -v cargo >/dev/null 2>&1 || missing_deps+=("cargo")
    command -v git >/dev/null 2>&1 || missing_deps+=("git")
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        exit 1
    fi
    
    if ! command -v cargo-release >/dev/null 2>&1; then
        log_warn "cargo-release not found, installing..."
        cargo install cargo-release
    fi
    
    log_info "✅ All dependencies satisfied"
}

check_git_status() {
    log_info "Checking git status..."
    
    if ! git diff-index --quiet HEAD --; then
        log_error "Working directory has uncommitted changes"
        git status
        exit 1
    fi
    
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "main" && "$current_branch" != "master" ]]; then
        log_warn "Not on main branch (currently on $current_branch)"
        read -p "Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    log_info "✅ Git status clean"
}

run_tests() {
    log_info "Running tests..."
    
    cargo test --workspace --all-features
    
    if [ $? -ne 0 ]; then
        log_error "Tests failed"
        exit 1
    fi
    
    log_info "✅ Tests passed"
}

run_linting() {
    log_info "Running linter..."
    
    cargo clippy --workspace --all-features -- -D warnings
    
    if [ $? -ne 0 ]; then
        log_error "Linting failed"
        exit 1
    fi
    
    log_info "✅ Linting passed"
}

run_security_audit() {
    log_info "Running security audit..."
    
    if command -v cargo-audit >/dev/null 2>&1; then
        cargo audit
    else
        log_warn "cargo-audit not installed, skipping security audit"
    fi
    
    log_info "✅ Security audit complete"
}

bump_version() {
    log_info "Bumping version ($VERSION)..."
    
    if [ "$DRY_RUN" == "true" ]; then
        log_info "[DRY RUN] Would bump version with: cargo release $VERSION"
    else
        cargo release $VERSION --workspace --no-confirm --execute
    fi
    
    log_info "✅ Version bumped"
}

build_release() {
    log_info "Building release binaries..."
    
    cargo build --workspace --release
    
    if [ $? -ne 0 ]; then
        log_error "Build failed"
        exit 1
    fi
    
    log_info "✅ Release build complete"
}

build_wasm() {
    log_info "Building WASM examples..."
    
    if [ -d "examples" ]; then
        for example in examples/*/; do
            if [ -f "$example/Cargo.toml" ]; then
                name=$(basename "$example")
                log_info "Building WASM example: $name"
                cargo build --release --target wasm32-wasip1 --manifest-path "$example/Cargo.toml"
            fi
        done
    fi
    
    log_info "✅ WASM examples built"
}

generate_changelog() {
    log_info "Generating changelog..."
    
    local current_tag=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    local previous_tag=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
    
    if [ -n "$previous_tag" ]; then
        log_info "Changes since $previous_tag:"
        git log --oneline $previous_tag..HEAD
    else
        log_info "All changes:"
        git log --oneline
    fi
    
    log_info "✅ Changelog generated"
}

create_git_tag() {
    log_info "Creating git tag..."
    
    local new_version=$(grep -m1 'version = "' Cargo.toml | cut -d'"' -f2)
    local tag_name="v$new_version"
    
    if [ "$DRY_RUN" == "true" ]; then
        log_info "[DRY RUN] Would create tag: $tag_name"
    else
        git tag -a "$tag_name" -m "Release $tag_name"
        log_info "✅ Tag created: $tag_name"
    fi
}

push_changes() {
    log_info "Pushing changes..."
    
    if [ "$DRY_RUN" == "true" ]; then
        log_info "[DRY RUN] Would push changes and tags to remote"
    else
        git push
        git push --tags
        log_info "✅ Changes pushed"
    fi
}

main() {
    check_dependencies
    check_git_status
    run_tests
    run_linting
    run_security_audit
    bump_version
    build_release
    build_wasm
    generate_changelog
    create_git_tag
    push_changes
    
    log_info "🎉 Release process completed successfully!"
    
    if [ "$DRY_RUN" == "true" ]; then
        log_info "This was a dry run. No changes were made."
    else
        log_info "Monitor the CI/CD pipeline for the release build."
    fi
}

main
