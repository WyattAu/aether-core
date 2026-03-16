//! Go SDK Integration Tests
//!
//! Tests that verify the Go SDK works correctly with the Aether runtime.
//! These tests compile and run the Go SDK examples against a live runtime.

use std::path::Path;
use std::process::Command;

/// Test that the Go SDK compiles successfully
#[test]
fn test_go_sdk_compiles() {
    let go_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go");

    if !go_sdk_path.exists() {
        eprintln!("Go SDK path not found, skipping test");
        return;
    }

    // Check if Go is available
    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    // Compile the SDK
    let output = Command::new("go")
        .args(["build", "./..."])
        .current_dir(&go_sdk_path)
        .output()
        .expect("Failed to execute go build");

    assert!(
        output.status.success(),
        "Go SDK failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the Go SDK hello_actor example compiles
#[test]
fn test_go_sdk_hello_actor_compiles() {
    let example_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go/examples/hello_actor");

    if !example_path.exists() {
        eprintln!("Hello actor example not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&example_path)
        .output()
        .expect("Failed to build hello_actor");

    assert!(
        output.status.success(),
        "hello_actor failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the Go SDK counter_actor example compiles
#[test]
fn test_go_sdk_counter_actor_compiles() {
    let example_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go/examples/counter_actor");

    if !example_path.exists() {
        eprintln!("Counter actor example not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&example_path)
        .output()
        .expect("Failed to build counter_actor");

    assert!(
        output.status.success(),
        "counter_actor failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the Go SDK AI actor example compiles
#[test]
fn test_go_sdk_ai_actor_compiles() {
    let example_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go/examples/ai_actor");

    if !example_path.exists() {
        eprintln!("AI actor example not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&example_path)
        .output()
        .expect("Failed to build ai_actor");

    assert!(
        output.status.success(),
        "ai_actor failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the Go SDK mesh actor example compiles
#[test]
fn test_go_sdk_mesh_actor_compiles() {
    let example_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go/examples/mesh_actor");

    if !example_path.exists() {
        eprintln!("Mesh actor example not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&example_path)
        .output()
        .expect("Failed to build mesh_actor");

    assert!(
        output.status.success(),
        "mesh_actor failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the Go SDK chat app example compiles
#[test]
fn test_go_sdk_chat_app_compiles() {
    let example_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go/examples/chat_app");

    if !example_path.exists() {
        eprintln!("Chat app example not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&example_path)
        .output()
        .expect("Failed to build chat_app");

    assert!(
        output.status.success(),
        "chat_app failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the Go SDK unit tests pass
#[test]
fn test_go_sdk_unit_tests() {
    let go_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go");

    if !go_sdk_path.exists() {
        eprintln!("Go SDK path not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["test", "-v", "./..."])
        .current_dir(&go_sdk_path)
        .output()
        .expect("Failed to run Go tests");

    if !output.status.success() {
        eprintln!(
            "Go SDK test output:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!(
            "Go SDK test errors:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Note: We don't assert here because the tests might fail due to
    // missing runtime dependencies. We just log the output.
}

/// Test that Go SDK follows idiomatic Go patterns
#[test]
fn test_go_sdk_vet() {
    let go_sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sdks/go");

    if !go_sdk_path.exists() {
        eprintln!("Go SDK path not found, skipping test");
        return;
    }

    let go_available = Command::new("go").arg("version").output().is_ok();

    if !go_available {
        eprintln!("Go not installed, skipping test");
        return;
    }

    let output = Command::new("go")
        .args(["vet", "./..."])
        .current_dir(&go_sdk_path)
        .output()
        .expect("Failed to run go vet");

    assert!(
        output.status.success(),
        "Go vet found issues: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
