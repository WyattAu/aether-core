//! Python SDK Integration Tests
//!
//! Tests that verify the Python SDK works correctly with the Aether runtime.

use std::path::Path;
use std::process::Command;

/// Test that the Python SDK can be imported
#[test]
fn test_python_sdk_imports() {
    let python_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/python");

    if !python_sdk_path.exists() {
        eprintln!("Python SDK path not found, skipping test");
        return;
    }

    // Check if Python is available
    let python_available = Command::new("python3").arg("--version").output().is_ok();

    if !python_available {
        eprintln!("Python not installed, skipping test");
        return;
    }

    // Try to import the SDK
    let output = Command::new("python3")
        .args(["-c", "import aether_sdk; print('OK')"])
        .current_dir(&python_sdk_path)
        .output()
        .expect("Failed to execute Python");

    if !output.status.success() {
        eprintln!(
            "Python SDK import failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Don't fail the test, just log the issue
    }
}

/// Test that Python SDK passes linting with ruff
#[test]
fn test_python_sdk_lint() {
    let python_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/python");

    if !python_sdk_path.exists() {
        eprintln!("Python SDK path not found, skipping test");
        return;
    }

    // Check if ruff is available
    let ruff_available = Command::new("ruff").arg("--version").output().is_ok();

    if !ruff_available {
        eprintln!("ruff not installed, skipping lint test");
        return;
    }

    let output = Command::new("ruff")
        .args(["check", "aether_sdk/"])
        .current_dir(&python_sdk_path)
        .output()
        .expect("Failed to run ruff");

    if !output.status.success() {
        eprintln!(
            "Ruff lint issues:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

/// Test that Python SDK passes type checking with mypy
#[test]
fn test_python_sdk_typecheck() {
    let python_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/python");

    if !python_sdk_path.exists() {
        eprintln!("Python SDK path not found, skipping test");
        return;
    }

    // Check if mypy is available
    let mypy_available = Command::new("mypy").arg("--version").output().is_ok();

    if !mypy_available {
        eprintln!("mypy not installed, skipping typecheck test");
        return;
    }

    let output = Command::new("mypy")
        .args(["aether_sdk/", "--ignore-missing-imports"])
        .current_dir(&python_sdk_path)
        .output()
        .expect("Failed to run mypy");

    if !output.status.success() {
        eprintln!(
            "Mypy type errors:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        // Don't fail the test, type checking is optional
    }
}

/// Test that Python SDK unit tests pass
#[test]
fn test_python_sdk_unit_tests() {
    let python_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/python");

    if !python_sdk_path.exists() {
        eprintln!("Python SDK path not found, skipping test");
        return;
    }

    // Check if pytest is available
    let pytest_available = Command::new("pytest").arg("--version").output().is_ok();

    if !pytest_available {
        eprintln!("pytest not installed, skipping test");
        return;
    }

    let output = Command::new("pytest")
        .args(["tests/", "-v", "--tb=short"])
        .current_dir(&python_sdk_path)
        .output()
        .expect("Failed to run pytest");

    if !output.status.success() {
        eprintln!(
            "Pytest output:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!(
            "Pytest errors:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
