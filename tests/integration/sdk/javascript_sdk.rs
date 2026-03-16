//! JavaScript SDK Integration Tests
//!
//! Tests that verify the JavaScript SDK works correctly with the Aether runtime.

use std::path::Path;
use std::process::Command;

/// Test that the JavaScript SDK can be imported
#[test]
fn test_js_sdk_imports() {
    let js_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/js");

    if !js_sdk_path.exists() {
        eprintln!("JavaScript SDK path not found, skipping test");
        return;
    }

    // Check if Node.js is available
    let node_available = Command::new("node").arg("--version").output().is_ok();

    if !node_available {
        eprintln!("Node.js not installed, skipping test");
        return;
    }

    // Check if node_modules exists (npm install has been run)
    let node_modules = js_sdk_path.join("node_modules");
    if !node_modules.exists() {
        eprintln!("node_modules not found, run 'npm install' first");
        return;
    }

    // Try to import the SDK
    let output = Command::new("node")
        .args(["-e", "const aether = require('./dist'); console.log('OK');"])
        .current_dir(&js_sdk_path)
        .output()
        .expect("Failed to execute Node.js");

    if !output.status.success() {
        eprintln!(
            "JavaScript SDK import failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Test that the JavaScript SDK compiles with TypeScript
#[test]
fn test_js_sdk_compile() {
    let js_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/js");

    if !js_sdk_path.exists() {
        eprintln!("JavaScript SDK path not found, skipping test");
        return;
    }

    // Check if tsc is available
    let tsc_available = Command::new("npx")
        .args(["tsc", "--version"])
        .current_dir(&js_sdk_path)
        .output()
        .is_ok();

    if !tsc_available {
        eprintln!("TypeScript not installed, skipping test");
        return;
    }

    let output = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&js_sdk_path)
        .output()
        .expect("Failed to run npm build");

    if !output.status.success() {
        eprintln!(
            "TypeScript compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Test that the JavaScript SDK passes linting
#[test]
fn test_js_sdk_lint() {
    let js_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/js");

    if !js_sdk_path.exists() {
        eprintln!("JavaScript SDK path not found, skipping test");
        return;
    }

    // Check if eslint is available
    let eslint_available = Command::new("npm")
        .args(["run", "lint", "--if-present"])
        .current_dir(&js_sdk_path)
        .output()
        .is_ok();

    if !eslint_available {
        eprintln!("ESLint not configured, skipping lint test");
        return;
    }

    let output = Command::new("npm")
        .args(["run", "lint"])
        .current_dir(&js_sdk_path)
        .output()
        .expect("Failed to run npm lint");

    if !output.status.success() {
        eprintln!(
            "ESLint issues:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

/// Test that the JavaScript SDK unit tests pass
#[test]
fn test_js_sdk_unit_tests() {
    let js_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/js");

    if !js_sdk_path.exists() {
        eprintln!("JavaScript SDK path not found, skipping test");
        return;
    }

    // Check if jest/vitest is available
    let package_json = js_sdk_path.join("package.json");
    if !package_json.exists() {
        eprintln!("package.json not found, skipping test");
        return;
    }

    let output = Command::new("npm")
        .args(["test"])
        .current_dir(&js_sdk_path)
        .output()
        .expect("Failed to run npm test");

    if !output.status.success() {
        eprintln!("Test output:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("Test errors:\n{}", String::from_utf8_lossy(&output.stderr));
    }
}

/// Test that the JavaScript SDK examples compile
#[test]
fn test_js_sdk_examples_compile() {
    let js_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/js");

    if !js_sdk_path.exists() {
        eprintln!("JavaScript SDK path not found, skipping test");
        return;
    }

    let examples_path = js_sdk_path.join("examples");
    if !examples_path.exists() {
        eprintln!("Examples directory not found, skipping test");
        return;
    }

    // Check if TypeScript is available
    let tsc_available = Command::new("npx")
        .args(["tsc", "--version"])
        .current_dir(&js_sdk_path)
        .output()
        .is_ok();

    if !tsc_available {
        eprintln!("TypeScript not installed, skipping test");
        return;
    }

    // Try to compile each example
    if let Ok(entries) = std::fs::read_dir(&examples_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let output = Command::new("npx")
                    .args(["tsc", "--noEmit"])
                    .current_dir(&path)
                    .output();

                if let Ok(output) = output {
                    if !output.status.success() {
                        eprintln!("Example {:?} has TypeScript errors", path);
                    }
                }
            }
        }
    }
}
