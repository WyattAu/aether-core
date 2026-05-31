#![allow(missing_docs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn aether_cli() -> Command {
    let mut cmd = Command::cargo_bin("aether-cli");
    if cmd.is_err() {
        cmd = Ok({
            let mut c = Command::new("cargo");
            c.args(["run", "--bin", "aether-cli", "--"]);
            c
        });
    }
    let mut c = match cmd {
        Ok(c) => c,
        Err(e) => {
            let _ = e;
            Command::new("aether-cli")
        }
    };
    c.timeout(std::time::Duration::from_secs(5));
    c
}

// ── dev command ────────────────────────────────────────────────────────

#[test]
fn dev_parses_with_defaults() {
    let r = aether_cli().arg("dev").assert();
    r.failure().stderr(predicates::str::contains("not found"));
}

#[test]
fn dev_parses_short_args() {
    let r = aether_cli()
        .args(["dev", "-c", "my.toml", "--port", "9090", "-l", "debug"])
        .assert();
    r.failure().stderr(predicates::str::contains("not found"));
}

#[test]
fn dev_parses_long_args() {
    let r = aether_cli()
        .args([
            "dev",
            "--config",
            "custom.toml",
            "--port",
            "3000",
            "--log-level",
            "trace",
        ])
        .assert();
    r.failure().stderr(predicates::str::contains("not found"));
}

#[test]
fn dev_empty_config_value_accepted() {
    let r = aether_cli().args(["dev", "-c", ""]).assert();
    r.failure().stderr(predicates::str::contains("not found"));
}

// ── deploy command ──────────────────────────────────────────────────────

#[test]
fn deploy_parses_with_defaults() {
    let r = aether_cli().arg("deploy").assert();
    r.failure();
}

#[test]
fn deploy_parses_short_args() {
    let r = aether_cli()
        .args([
            "deploy",
            "-c",
            "prod.toml",
            "-e",
            "production",
            "-n",
            "5",
            "-a",
            "web,worker",
        ])
        .assert();
    r.failure();
}

#[test]
fn deploy_parses_long_args() {
    let r = aether_cli()
        .args([
            "deploy",
            "--config",
            "staging.toml",
            "--env",
            "staging",
            "--replicas",
            "3",
            "--no-build",
            "--push",
            "--dry-run",
            "--api-addr",
            "http://localhost:9999",
        ])
        .assert();
    r.failure();
}

#[test]
fn deploy_accepts_actors_comma_separated() {
    let r = aether_cli()
        .args(["deploy", "--actors", "svc-a,svc-b,svc-c"])
        .assert();
    r.failure();
}

// ── status command ─────────────────────────────────────────────────────

#[test]
fn status_parses_with_defaults() {
    let r = aether_cli().arg("status").assert();
    r.failure().stderr(
        predicates::str::contains("error")
            .or(predicates::str::contains("Error").or(predicates::str::contains("refused"))),
    );
}

#[test]
fn status_parses_with_actor_filter() {
    let r = aether_cli()
        .args(["status", "--actor", "my-actor"])
        .assert();
    r.failure();
}

#[test]
fn status_parses_format_json() {
    let r = aether_cli().args(["status", "--format", "json"]).assert();
    r.failure();
}

#[test]
fn status_parses_watch_flag() {
    let r = aether_cli().args(["status", "-w"]).assert();
    r.failure();
}

#[test]
fn status_parses_custom_api_addr() {
    let r = aether_cli()
        .args(["status", "--api-addr", "http://example.com:1234"])
        .assert();
    r.failure();
}

// ── logs command ────────────────────────────────────────────────────────

#[test]
fn logs_parses_with_defaults() {
    let r = aether_cli().arg("logs").assert();
    r.failure();
}

#[test]
fn logs_parses_with_actor() {
    let r = aether_cli().args(["logs", "--actor", "web"]).assert();
    r.failure();
}

#[test]
fn logs_parses_follow() {
    let r = aether_cli().args(["logs", "--follow"]).assert();
    r.failure();
}

#[test]
fn logs_parses_lines_arg() {
    let r = aether_cli().args(["logs", "--lines", "50"]).assert();
    r.failure();
}

#[test]
fn logs_parses_level_filter() {
    let r = aether_cli().args(["logs", "--level", "ERROR"]).assert();
    r.failure();
}

#[test]
fn logs_parses_format_json() {
    let r = aether_cli().args(["logs", "--format", "json"]).assert();
    r.failure();
}

#[test]
fn logs_parses_file_arg() {
    let r = aether_cli()
        .args(["logs", "--file", "/var/log/aether.log"])
        .assert();
    r.failure();
}

#[test]
fn logs_parses_websocket_arg() {
    let r = aether_cli()
        .args(["logs", "--websocket", "ws://localhost:8080/ws"])
        .assert();
    r.failure();
}

#[test]
fn logs_parses_tracing_filter() {
    let r = aether_cli()
        .args(["logs", "--tracing-filter", "warn"])
        .assert();
    r.failure();
}

// ── scale command ───────────────────────────────────────────────────────

#[test]
fn scale_parses_required_actor() {
    let r = aether_cli().args(["scale", "--actor", "web"]).assert();
    r.failure();
}

#[test]
fn scale_parses_replicas() {
    let r = aether_cli()
        .args(["scale", "--actor", "api", "--replicas", "5"])
        .assert();
    r.failure();
}

#[test]
fn scale_parses_min_max() {
    let r = aether_cli()
        .args([
            "scale",
            "--actor",
            "worker",
            "--min",
            "2",
            "--max",
            "10",
            "--replicas",
            "4",
        ])
        .assert();
    r.failure();
}

#[test]
fn scale_missing_actor_fails() {
    let r = aether_cli().arg("scale").assert();
    r.failure().stderr(
        predicates::str::contains("required")
            .or(predicates::str::contains("unique").or(predicates::str::contains("'-m'"))),
    );
}

// ── capability command ──────────────────────────────────────────────────

#[test]
fn capability_list_parses() {
    let r = aether_cli()
        .args(["capability", "list", "--actor", "web"])
        .assert();
    r.success();
}

#[test]
fn capability_grant_parses() {
    let r = aether_cli()
        .args([
            "capability",
            "grant",
            "--actor",
            "web",
            "--capability",
            "networking:public",
        ])
        .assert();
    r.success();
}

#[test]
fn capability_revoke_parses() {
    let r = aether_cli()
        .args([
            "capability",
            "revoke",
            "--actor",
            "web",
            "--capability",
            "fs:read:/data/*",
        ])
        .assert();
    r.success();
}

#[test]
fn capability_missing_subcommand_fails() {
    let r = aether_cli().arg("capability").assert();
    r.failure()
        .stderr(predicates::str::contains("required").or(predicates::str::contains("command")));
}

#[test]
fn capability_missing_actor_fails() {
    let r = aether_cli().args(["capability", "list"]).assert();
    r.failure().stderr(predicates::str::contains("required"));
}

// ── exec command ───────────────────────────────────────────────────────

#[test]
fn exec_parses_required_actor() {
    let r = aether_cli().args(["exec", "--actor", "web"]).assert();
    r.failure();
}

#[test]
fn exec_parses_with_command() {
    let r = aether_cli()
        .args(["exec", "--actor", "web", "--", "ls", "-la"])
        .assert();
    r.failure();
}

#[test]
fn exec_parses_interactive() {
    let r = aether_cli()
        .args(["exec", "--actor", "web", "--interactive", "--tty"])
        .assert();
    r.failure();
}

#[test]
fn exec_parses_custom_shell() {
    let r = aether_cli()
        .args(["exec", "--actor", "web", "--shell", "/bin/bash"])
        .assert();
    r.failure();
}

#[test]
fn exec_missing_actor_fails() {
    let r = aether_cli().arg("exec").assert();
    r.failure().stderr(
        predicates::str::contains("required")
            .or(predicates::str::contains("unique").or(predicates::str::contains("'-a'"))),
    );
}

// ── inspect command ───────────────────────────────────────────────────

#[test]
fn inspect_memory_parses() {
    let r = aether_cli()
        .args(["inspect", "memory", "--actor", "web"])
        .assert();
    r.failure();
}

#[test]
fn inspect_memory_with_format_and_bytes() {
    let r = aether_cli()
        .args([
            "inspect", "memory", "--actor", "web", "--format", "json", "--bytes", "128",
        ])
        .assert();
    r.failure();
}

#[test]
fn inspect_memory_with_offset() {
    let r = aether_cli()
        .args(["inspect", "memory", "--actor", "web", "--offset", "256"])
        .assert();
    r.failure();
}

#[test]
fn inspect_state_parses() {
    let r = aether_cli()
        .args(["inspect", "state", "--actor", "web"])
        .assert();
    r.failure();
}

#[test]
fn inspect_state_with_key() {
    let r = aether_cli()
        .args(["inspect", "state", "--actor", "web", "--key", "counter"])
        .assert();
    r.failure();
}

#[test]
fn inspect_stack_parses() {
    let r = aether_cli()
        .args(["inspect", "stack", "--actor", "web"])
        .assert();
    r.success();
}

#[test]
fn inspect_stack_with_depth() {
    let r = aether_cli()
        .args(["inspect", "stack", "--actor", "web", "--depth", "20"])
        .assert();
    r.success();
}

#[test]
fn inspect_metadata_parses() {
    let r = aether_cli()
        .args(["inspect", "metadata", "--actor", "web"])
        .assert();
    r.failure();
}

#[test]
fn inspect_all_parses() {
    let r = aether_cli()
        .args(["inspect", "all", "--actor", "web"])
        .assert();
    r.success();
}

#[test]
fn inspect_all_json_format() {
    let r = aether_cli()
        .args(["inspect", "all", "--actor", "web", "--format", "json"])
        .assert();
    r.success();
}

#[test]
fn inspect_missing_subcommand_fails() {
    let r = aether_cli().arg("inspect").assert();
    r.failure()
        .stderr(predicates::str::contains("required").or(predicates::str::contains("command")));
}

// ── mesh command ──────────────────────────────────────────────────────

#[test]
fn mesh_status_parses() {
    let r = aether_cli().args(["mesh", "status"]).assert();
    r.failure();
}

#[test]
fn mesh_status_with_format() {
    let r = aether_cli()
        .args(["mesh", "status", "--format", "json"])
        .assert();
    r.failure();
}

#[test]
fn mesh_status_watch() {
    let r = aether_cli().args(["mesh", "status", "--watch"]).assert();
    r.failure();
}

#[test]
fn mesh_peers_parses() {
    let r = aether_cli().args(["mesh", "peers"]).assert();
    r.failure();
}

#[test]
fn mesh_peers_detailed() {
    let r = aether_cli().args(["mesh", "peers", "--detailed"]).assert();
    r.failure();
}

#[test]
fn mesh_connect_parses() {
    let r = aether_cli()
        .args(["mesh", "connect", "--peer", "10.0.0.2:7000"])
        .assert();
    r.failure();
}

#[test]
fn mesh_connect_with_port_and_timeout() {
    let r = aether_cli()
        .args([
            "mesh",
            "connect",
            "--peer",
            "10.0.0.2",
            "--port",
            "7000",
            "--timeout",
            "15",
        ])
        .assert();
    r.failure();
}

#[test]
fn mesh_disconnect_parses() {
    let r = aether_cli()
        .args(["mesh", "disconnect", "--peer", "node-2"])
        .assert();
    r.failure();
}

#[test]
fn mesh_disconnect_force() {
    let r = aether_cli()
        .args(["mesh", "disconnect", "--peer", "node-2", "--force"])
        .assert();
    r.failure();
}

#[test]
fn mesh_topology_parses() {
    let r = aether_cli().args(["mesh", "topology"]).assert();
    r.failure();
}

#[test]
fn mesh_topology_format_dot() {
    let r = aether_cli()
        .args(["mesh", "topology", "--format", "dot"])
        .assert();
    r.failure();
}

#[test]
fn mesh_topology_with_output() {
    let r = aether_cli()
        .args(["mesh", "topology", "--output", "/tmp/topology.txt"])
        .assert();
    r.failure();
}

#[test]
fn mesh_missing_subcommand_fails() {
    let r = aether_cli().arg("mesh").assert();
    r.failure()
        .stderr(predicates::str::contains("required").or(predicates::str::contains("command")));
}

#[test]
fn mesh_connect_missing_peer_fails() {
    let r = aether_cli().args(["mesh", "connect"]).assert();
    r.failure().stderr(
        predicates::str::contains("required")
            .or(predicates::str::contains("unique").or(predicates::str::contains("'-p'"))),
    );
}

// ── config command ────────────────────────────────────────────────────

#[test]
fn config_validate_parses() {
    let r = aether_cli().args(["config", "validate"]).assert();
    r.failure();
}

#[test]
fn config_validate_strict() {
    let r = aether_cli()
        .args(["config", "validate", "--strict"])
        .assert();
    r.failure();
}

#[test]
fn config_validate_custom_file() {
    let r = aether_cli()
        .args(["config", "validate", "--config", "/path/to/aether.toml"])
        .assert();
    r.failure();
}

#[test]
fn config_generate_parses() {
    let r = aether_cli()
        .args([
            "config",
            "generate",
            "--output",
            "/tmp/aether-test-gen.toml",
        ])
        .assert();
    r.success();
}

#[test]
fn config_generate_with_template() {
    let r = aether_cli()
        .args([
            "config",
            "generate",
            "--template",
            "web",
            "--force",
            "--output",
            "/tmp/aether-test-web.toml",
        ])
        .assert();
    r.success();
}

#[test]
fn config_generate_custom_output() {
    let r = aether_cli()
        .args([
            "config",
            "generate",
            "--output",
            "/tmp/aether-test-custom.toml",
        ])
        .assert();
    r.success();
}

#[test]
fn config_view_parses() {
    let r = aether_cli().args(["config", "view"]).assert();
    r.failure();
}

#[test]
fn config_view_with_section() {
    let r = aether_cli()
        .args(["config", "view", "--section", "actors"])
        .assert();
    r.failure();
}

#[test]
fn config_view_json_format() {
    let r = aether_cli()
        .args(["config", "view", "--format", "json"])
        .assert();
    r.failure();
}

#[test]
fn config_schema_parses() {
    let r = aether_cli()
        .args([
            "config",
            "schema",
            "--output",
            "/tmp/aether-test-schema.json",
        ])
        .assert();
    r.success();
}

#[test]
fn config_schema_markdown() {
    let r = aether_cli()
        .args([
            "config",
            "schema",
            "--format",
            "markdown",
            "--output",
            "/tmp/aether-test-schema.md",
        ])
        .assert();
    r.success();
}

#[test]
fn config_missing_subcommand_fails() {
    let r = aether_cli().arg("config").assert();
    r.failure()
        .stderr(predicates::str::contains("required").or(predicates::str::contains("command")));
}

// ── import command ────────────────────────────────────────────────────

#[test]
fn import_parses_with_defaults() {
    let r = aether_cli().arg("import").assert();
    r.failure();
}

#[test]
fn import_custom_input_output() {
    let r = aether_cli()
        .args([
            "import",
            "--input",
            "docker-compose.yml",
            "--output",
            "aether.toml",
        ])
        .assert();
    r.failure();
}

#[test]
fn import_force_and_dry_run() {
    let r = aether_cli()
        .args(["import", "--force", "--dry-run", "--verbose"])
        .assert();
    r.failure();
}

#[test]
fn import_verbose() {
    let r = aether_cli().args(["import", "--verbose"]).assert();
    r.failure();
}

// ── dashboard command ───────────────────────────────────────────────────

#[test]
fn dashboard_parses_with_defaults() {
    let r = aether_cli().arg("dashboard").assert();
    r.failure();
}

#[test]
fn dashboard_custom_port() {
    let r = aether_cli().args(["dashboard", "--port", "3000"]).assert();
    r.failure();
}

#[test]
fn dashboard_custom_host() {
    let r = aether_cli()
        .args(["dashboard", "--host", "0.0.0.0", "--port", "80"])
        .assert();
    r.failure();
}

#[test]
fn dashboard_open_flag() {
    let r = aether_cli().args(["dashboard", "--open"]).assert();
    r.failure();
}

// ── top command ────────────────────────────────────────────────────────

#[test]
fn top_parses_with_defaults() {
    let r = aether_cli().arg("top").assert();
    r.failure();
}

#[test]
fn top_custom_refresh() {
    let r = aether_cli().args(["top", "--refresh", "500"]).assert();
    r.failure();
}

#[test]
fn top_with_filter() {
    let r = aether_cli().args(["top", "--filter", "web"]).assert();
    r.failure();
}

#[test]
fn top_sort_by_memory() {
    let r = aether_cli().args(["top", "--sort", "memory"]).assert();
    r.failure();
}

#[test]
fn top_sort_by_name() {
    let r = aether_cli().args(["top", "--sort", "name"]).assert();
    r.failure();
}

#[test]
fn top_custom_api_addr() {
    let r = aether_cli()
        .args(["top", "--api-addr", "http://192.168.1.1:8080"])
        .assert();
    r.failure();
}

// ── rollback command ───────────────────────────────────────────────────

#[test]
fn rollback_parses_with_actor() {
    let r = aether_cli().args(["rollback", "--actor", "web"]).assert();
    r.failure();
}

#[test]
fn rollback_with_revision() {
    let r = aether_cli()
        .args(["rollback", "--actor", "api", "--revision", "3"])
        .assert();
    r.failure();
}

#[test]
fn rollback_dry_run() {
    let r = aether_cli()
        .args(["rollback", "--actor", "web", "--dry-run"])
        .assert();
    r.failure();
}

#[test]
fn rollback_force() {
    let r = aether_cli()
        .args(["rollback", "--actor", "web", "--force", "--timeout", "30"])
        .assert();
    r.failure();
}

#[test]
fn rollback_custom_history() {
    let r = aether_cli()
        .args([
            "rollback",
            "--actor",
            "web",
            "--history",
            "/tmp/my-history.json",
        ])
        .assert();
    r.failure();
}

#[test]
fn rollback_missing_actor_fails() {
    let r = aether_cli().arg("rollback").assert();
    r.failure().stderr(predicates::str::contains("required"));
}

// ── completion command ────────────────────────────────────────────────

#[test]
fn completion_bash_parses() {
    let r = aether_cli()
        .args(["completion", "bash", "--output", "/tmp/aether-test-bash.sh"])
        .assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("written")));
}

#[test]
fn completion_zsh_parses() {
    let r = aether_cli()
        .args(["completion", "zsh", "--output", "/tmp/aether-test-zsh.sh"])
        .assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("written")));
}

#[test]
fn completion_fish_parses() {
    let r = aether_cli()
        .args(["completion", "fish", "--output", "/tmp/aether-test-fish.sh"])
        .assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("written")));
}

#[test]
fn completion_elvish_parses() {
    let r = aether_cli()
        .args([
            "completion",
            "elvish",
            "--output",
            "/tmp/aether-test-elvish.sh",
        ])
        .assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("written")));
}

#[test]
fn completion_powershell_parses() {
    let r = aether_cli()
        .args([
            "completion",
            "powershell",
            "--output",
            "/tmp/aether-test-ps1.sh",
        ])
        .assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("written")));
}

#[test]
fn completion_with_output_file() {
    let r = aether_cli()
        .args([
            "completion",
            "bash",
            "--output",
            "/tmp/aether-test-comp.bash",
        ])
        .assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("written")));
}

#[test]
fn completion_invalid_shell_fails() {
    let r = aether_cli().args(["completion", "nonexistent"]).assert();
    r.failure().stderr(
        predicates::str::contains("invalid")
            .or(predicates::str::contains("valid").or(predicates::str::contains("unknown"))),
    );
}

#[test]
fn completion_missing_shell_fails() {
    let r = aether_cli().arg("completion").assert();
    r.failure().stderr(predicates::str::contains("required"));
}

// ── observability command ────────────────────────────────────────────

#[test]
fn observability_push_metrics_parses() {
    let r = aether_cli()
        .args(["observability", "push-metrics"])
        .assert();
    r.failure();
}

#[test]
fn observability_push_logs_parses() {
    let r = aether_cli().args(["observability", "push-logs"]).assert();
    r.failure();
}

#[test]
fn observability_status_parses() {
    let r = aether_cli().args(["observability", "status"]).assert();
    r.success();
}

#[test]
fn observability_missing_subcommand_fails() {
    let r = aether_cli().arg("observability").assert();
    r.failure()
        .stderr(predicates::str::contains("required").or(predicates::str::contains("command")));
}

// ── run command ───────────────────────────────────────────────────────

#[test]
fn run_parses_with_defaults() {
    let r = aether_cli().arg("run").assert();
    r.failure();
}

#[test]
fn run_custom_port() {
    let r = aether_cli().args(["run", "--port", "9090"]).assert();
    r.failure();
}

#[test]
fn run_custom_host_and_port() {
    let r = aether_cli()
        .args(["run", "--host", "0.0.0.0", "--port", "3000"])
        .assert();
    r.failure();
}

// ── top-level / edge cases ───────────────────────────────────────────

#[test]
fn no_args_shows_help() {
    let r = aether_cli().assert();
    r.failure().stderr(predicates::str::contains("Usage"));
}

#[test]
fn help_flag_works() {
    let r = aether_cli().arg("--help").assert();
    r.success().stdout(predicates::str::contains("Aether"));
}

#[test]
fn version_flag_works() {
    let r = aether_cli().arg("--version").assert();
    r.success();
}

#[test]
fn unknown_command_fails() {
    let r = aether_cli().arg("nonexistent").assert();
    r.failure().stderr(
        predicates::str::contains("unrecognized")
            .or(predicates::str::contains("unknown").or(predicates::str::contains("invalid"))),
    );
}

#[test]
fn dev_help_shows_dev_usage() {
    let r = aether_cli().args(["dev", "--help"]).assert();
    r.success().stdout(predicates::str::contains("development"));
}

#[test]
fn deploy_help_shows_deploy_usage() {
    let r = aether_cli().args(["deploy", "--help"]).assert();
    r.success().stdout(predicates::str::contains("Deploy"));
}

#[test]
fn status_help_shows_status_usage() {
    let r = aether_cli().args(["status", "--help"]).assert();
    r.success().stdout(predicates::str::contains("status"));
}

#[test]
fn exec_help_shows_exec_usage() {
    let r = aether_cli().args(["exec", "--help"]).assert();
    r.failure()
        .stderr(predicates::str::contains("'-a'").or(predicates::str::contains("Execute")));
}

#[test]
fn logs_help_shows_logs_usage() {
    let r = aether_cli().args(["logs", "--help"]).assert();
    r.failure()
        .stderr(predicates::str::contains("'-f'").or(predicates::str::contains("logs")));
}

#[test]
fn scale_help_shows_scale_usage() {
    let r = aether_cli().args(["scale", "--help"]).assert();
    r.failure()
        .stderr(predicates::str::contains("'-m'").or(predicates::str::contains("Scale")));
}

#[test]
fn inspect_help_shows_inspect_usage() {
    let r = aether_cli().args(["inspect", "--help"]).assert();
    r.success().stdout(predicates::str::contains("Inspect"));
}

#[test]
fn mesh_help_shows_mesh_usage() {
    let r = aether_cli().args(["mesh", "--help"]).assert();
    r.success().stdout(predicates::str::contains("mesh"));
}

#[test]
fn config_help_shows_config_usage() {
    let r = aether_cli().args(["config", "--help"]).assert();
    r.success()
        .stdout(predicates::str::contains("configuration"));
}

#[test]
fn import_help_shows_import_usage() {
    let r = aether_cli().args(["import", "--help"]).assert();
    r.success().stdout(predicates::str::contains("Import"));
}

#[test]
fn dashboard_help_shows_dashboard_usage() {
    let r = aether_cli().args(["dashboard", "--help"]).assert();
    r.success().stdout(predicates::str::contains("dashboard"));
}

#[test]
fn top_help_shows_top_usage() {
    let r = aether_cli().args(["top", "--help"]).assert();
    r.success().stdout(predicates::str::contains("dashboard"));
}

#[test]
fn rollback_help_shows_rollback_usage() {
    let r = aether_cli().args(["rollback", "--help"]).assert();
    r.success().stdout(predicates::str::contains("Rollback"));
}

#[test]
fn completion_help_shows_completion_usage() {
    let r = aether_cli().args(["completion", "--help"]).assert();
    r.success().stdout(predicates::str::contains("completion"));
}

#[test]
fn observability_help_shows_observability_usage() {
    let r = aether_cli().args(["observability", "--help"]).assert();
    r.success()
        .stdout(predicates::str::contains("observability"));
}

#[test]
fn run_help_shows_run_usage() {
    let r = aether_cli().args(["run", "--help"]).assert();
    r.success()
        .stdout(predicates::str::contains("server").or(predicates::str::contains("Start")));
}

// ── error type / error message tests ───────────────────────────────────

#[test]
fn dev_nonexistent_config_shows_error() {
    let r = aether_cli()
        .args(["dev", "--config", "/nonexistent/path.toml"])
        .assert();
    r.failure().stderr(
        predicates::str::contains("Configuration not found")
            .or(predicates::str::contains("not found")),
    );
}

#[test]
fn deploy_nonexistent_config_shows_error() {
    let r = aether_cli()
        .args(["deploy", "--config", "/nonexistent/path.toml"])
        .assert();
    r.failure();
}

#[test]
fn import_nonexistent_input_shows_error() {
    let r = aether_cli()
        .args(["import", "--input", "/nonexistent/compose.yml"])
        .assert();
    r.failure();
}

// ── edge cases ─────────────────────────────────────────────────────────

#[test]
fn logs_invalid_level_accepted_by_parser() {
    let r = aether_cli()
        .args(["logs", "--level", "invalid_level"])
        .assert();
    r.failure();
}

#[test]
fn top_zero_refresh_accepted_by_parser() {
    let r = aether_cli().args(["top", "--refresh", "0"]).assert();
    r.failure();
}

#[test]
fn scale_zero_replicas_fails_execution() {
    let r = aether_cli()
        .args(["scale", "--actor", "test", "--replicas", "0"])
        .assert();
    r.failure();
}

#[test]
fn rollback_invalid_revision_fails() {
    let r = aether_cli()
        .args(["rollback", "--actor", "web", "--revision", "999"])
        .assert();
    r.failure();
}

#[test]
fn config_view_invalid_section_fails() {
    let r = aether_cli()
        .args(["config", "view", "--section", "nonexistent"])
        .assert();
    r.failure();
}

#[test]
fn capability_grant_invalid_format_fails_execution() {
    let r = aether_cli()
        .args([
            "capability",
            "grant",
            "--actor",
            "web",
            "--capability",
            "invalid_no_colon",
        ])
        .assert();
    r.failure();
}
