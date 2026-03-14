#!/bin/bash
# Security Scanner Script

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_section() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

SCAN_TYPE=${1:-"all"}
OUTPUT_DIR=${2:-"security-results"}
FAIL_ON_VULN=${3:-"true"}

mkdir -p "$OUTPUT_DIR"

VULNERABILITIES_FOUND=0

run_cargo_audit() {
    log_section "Dependency Vulnerability Scan (cargo-audit)"
    
    if ! command -v cargo-audit >/dev/null 2>&1; then
        log_info "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    log_info "Running cargo audit..."
    
    if cargo audit --json > "$OUTPUT_DIR/audit-results.json" 2>&1; then
        log_info "✅ No vulnerabilities found"
    else
        log_warn "⚠️  Vulnerabilities found!"
        VULNERABILITIES_FOUND=1
    fi
    
    cargo audit 2>&1 | tee "$OUTPUT_DIR/audit-report.txt"
}

run_dependency_review() {
    log_section "Dependency Review"
    
    if ! command -v cargo-license >/dev/null 2>&1; then
        log_info "Installing cargo-license..."
        cargo install cargo-license
    fi
    
    log_info "Checking dependency licenses..."
    
    cargo license --avoid-dev-deps --avoid-build-deps --filter-platform x86_64-unknown-linux-gnu > "$OUTPUT_DIR/licenses.txt"
    
    log_info "License summary:"
    cat "$OUTPUT_DIR/licenses.txt"
    
    if grep -iE "GPL|AGPL|LGPL|SSPL|BSL" "$OUTPUT_DIR/licenses.txt"; then
        log_warn "⚠️  Potentially problematic licenses found"
        VULNERABILITIES_FOUND=1
    else
        log_info "✅ All licenses acceptable"
    fi
}

run_secrets_scan() {
    log_section "Secrets Scan"
    
    if command -v gitleaks >/dev/null 2>&1; then
        log_info "Running gitleaks..."
        
        if gitleaks detect --source . --report-path "$OUTPUT_DIR/gitleaks-report.json" --report-format json --no-git; then
            log_info "✅ No secrets detected"
        else
            log_warn "⚠️  Potential secrets found!"
            VULNERABILITIES_FOUND=1
        fi
    else
        log_warn "Gitleaks not found, skipping secrets scan"
        log_info "Install with: go install github.com/gitleaks/gitleaks/v8@latest"
    fi
}

run_codeql_analysis() {
    log_section "CodeQL Analysis"
    
    if command -v codeql >/dev/null 2>&1; then
        log_info "Running CodeQL queries..."
        
        codeql database create --language=rust --source-root=. --overwrite "$OUTPUT_DIR/codeql-db"
        codeql database analyze "$OUTPUT_DIR/codeql-db" --format=sarif-latest --output="$OUTPUT_DIR/codeql-results.sarif" rust-security-and-quality.qls
        
        log_info "CodeQL results saved to $OUTPUT_DIR/codeql-results.sarif"
    else
        log_warn "CodeQL CLI not found, skipping CodeQL analysis"
        log_info "Download from: https://github.com/github/codeql-cli-binaries/releases"
    fi
}

run_clippy_security_lints() {
    log_section "Clippy Security Lints"
    
    log_info "Running clippy with security lints..."
    
    RUSTFLAGS="-D warnings" cargo clippy --workspace --all-features -- -W clippy::all -W clippy::pedantic -W clippy::nursery 2>&1 | tee "$OUTPUT_DIR/clippy-report.txt"
    
    if [ $? -eq 0 ]; then
        log_info "✅ No security issues found by clippy"
    else
        log_warn "⚠️  Clippy found issues"
        VULNERABILITIES_FOUND=1
    fi
}

run_licenses_check() {
    log_section "License Header Check"
    
    local missing_headers=0
    
    for file in $(find crates -name "*.rs" -type f); do
        if ! head -3 "$file" | grep -q "Copyright\|Licensed\|SPDX"; then
            log_warn "Missing license header: $file"
            missing_headers=$((missing_headers + 1))
        fi
    done
    
    if [ $missing_headers -gt 0 ]; then
        log_warn "⚠️  $missing_headers files missing license headers"
        echo "Files missing headers: $missing_headers" > "$OUTPUT_DIR/license-headers.txt"
    else
        log_info "✅ All files have license headers"
    fi
}

run_sarif_generation() {
    log_section "Generating SARIF Reports"
    
    python3 <<'PYTHON'
import json
import os
import sys

output_dir = os.environ.get('OUTPUT_DIR', 'security-results')

sarif = {
    "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
    "version": "2.1.0",
    "runs": []
}

audit_file = os.path.join(output_dir, 'audit-results.json')
if os.path.exists(audit_file):
    try:
        with open(audit_file, 'r') as f:
            data = json.load(f)
        
        run = {
            "tool": {
                "driver": {
                    "name": "cargo-audit",
                    "informationUri": "https://github.com/RustSec/cargo-audit",
                    "rules": []
                }
            },
            "results": []
        }
        
        vulnerabilities = data.get('vulnerabilities', {}).get('list', [])
        for vuln in vulnerabilities:
            advisory = vuln.get('advisory', {})
            rule_id = advisory.get('id', 'unknown')
            
            run["tool"]["driver"]["rules"].append({
                "id": rule_id,
                "shortDescription": {"text": advisory.get('title', 'Unknown')}
            })
            
            run["results"].append({
                "ruleId": rule_id,
                "message": {"text": advisory.get('description', '')},
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {"uri": "Cargo.lock"}
                    }
                }]
            })
        
        if run["results"]:
            sarif["runs"].append(run)
    except Exception as e:
        print(f"Error processing audit results: {e}", file=sys.stderr)

with open(os.path.join(output_dir, 'security-scan.sarif'), 'w') as f:
    json.dump(sarif, f, indent=2)

print("SARIF report generated")
PYTHON
}

generate_summary() {
    log_section "Security Scan Summary"
    
    summary_file="$OUTPUT_DIR/summary.md"
    
    echo "# Security Scan Summary" > "$summary_file"
    echo "" >> "$summary_file"
    echo "**Date:** $(date)" >> "$summary_file"
    echo "" >> "$summary_file"
    
    echo "## Results" >> "$summary_file"
    echo "" >> "$summary_file"
    echo "| Scan Type | Status |" >> "$summary_file"
    echo "|-----------|--------|" >> "$summary_file"
    
    [ -f "$OUTPUT_DIR/audit-report.txt" ] && echo "| Dependency Audit | $([ -s "$OUTPUT_DIR/audit-report.txt" ] && echo "⚠️ Issues Found" || echo "✅ Passed") |" >> "$summary_file"
    [ -f "$OUTPUT_DIR/licenses.txt" ] && echo "| License Check | ✅ Complete |" >> "$summary_file"
    [ -f "$OUTPUT_DIR/gitleaks-report.json" ] && echo "| Secrets Scan | $([ -s "$OUTPUT_DIR/gitleaks-report.json" ] && echo "⚠️ Issues Found" || echo "✅ Passed") |" >> "$summary_file"
    [ -f "$OUTPUT_DIR/clippy-report.txt" ] && echo "| Clippy Security | $([ -s "$OUTPUT_DIR/clippy-report.txt" ] && echo "⚠️ Issues Found" || echo "✅ Passed") |" >> "$summary_file"
    
    echo "" >> "$summary_file"
    
    if [ $VULNERABILITIES_FOUND -eq 1 ]; then
        echo "## ⚠️ Warnings" >> "$summary_file"
        echo "" >> "$summary_file"
        echo "Security issues were found. Please review the detailed reports." >> "$summary_file"
    else
        echo "## ✅ All Clear" >> "$summary_file"
        echo "" >> "$summary_file"
        echo "No security issues were detected." >> "$summary_file"
    fi
    
    cat "$summary_file"
}

main() {
    log_info "🔒 Starting security scan suite"
    log_info "Type: $SCAN_TYPE"
    log_info "Output directory: $OUTPUT_DIR"
    
    case "$SCAN_TYPE" in
        "audit")
            run_cargo_audit
            ;;
        "licenses")
            run_dependency_review
            run_licenses_check
            ;;
        "secrets")
            run_secrets_scan
            ;;
        "codeql")
            run_codeql_analysis
            ;;
        "clippy")
            run_clippy_security_lints
            ;;
        "all")
            run_cargo_audit
            run_dependency_review
            run_secrets_scan
            run_clippy_security_lints
            run_licenses_check
            ;;
        *)
            log_error "Unknown scan type: $SCAN_TYPE"
            echo "Usage: $0 [audit|licenses|secrets|codeql|clippy|all] [output_dir] [fail_on_vuln]"
            exit 1
            ;;
    esac
    
    run_sarif_generation
    generate_summary
    
    if [ "$FAIL_ON_VULN" == "true" ] && [ $VULNERABILITIES_FOUND -eq 1 ]; then
        log_error "❌ Security scan failed - vulnerabilities found"
        exit 1
    fi
    
    log_info "✅ Security scan completed"
}

main
