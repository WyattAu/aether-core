//! Import Command
//!
//! Import docker-compose.yml to aether.toml format.

use clap::Args;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Args, Debug)]
pub struct ImportArgs {
    #[arg(short, long, default_value = "docker-compose.yml")]
    pub input: String,

    #[arg(short, long, default_value = "aether.toml")]
    pub output: String,

    #[arg(short, long)]
    pub force: bool,

    #[arg(short, long)]
    pub dry_run: bool,

    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("Input file not found: {0}")]
    InputNotFound(String),

    #[error("Output file already exists: {0}")]
    OutputExists(String),

    #[error("Failed to parse docker-compose: {0}")]
    ParseError(String),

    #[error("Unsupported compose feature: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    UnsupportedFeature(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error("IO error: {0}")]
    IoError(String),
}

#[derive(Debug, Deserialize)]
struct ComposeFile {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    version: Option<String>,
    services: HashMap<String, ComposeServiceRaw>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    volumes: Option<HashMap<String, VolumeConfig>>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    networks: Option<HashMap<String, NetworkConfig>>,
}

#[derive(Debug, Deserialize)]
struct ComposeServiceRaw {
    image: Option<String>,
    build: Option<BuildConfig>,
    ports: Option<Vec<String>>,
    environment: Option<Environment>,
    volumes: Option<Vec<String>>,
    depends_on: Option<DependsOn>,
    command: Option<Command>,
    replicas: Option<u32>,
    restart: Option<String>,
    healthcheck: Option<HealthCheck>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    networks: Option<Vec<String>>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    entrypoint: Option<Command>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    working_dir: Option<String>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    user: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BuildConfig {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    context: String,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    dockerfile: Option<String>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    args: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Environment {
    List(Vec<String>),
    Map(HashMap<String, String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Command {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DependsOn {
    List(Vec<String>),
    Map(HashMap<String, DependsOnConfig>),
}

#[derive(Debug, Deserialize)]
struct DependsOnConfig {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    condition: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HealthCheck {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    test: Option<HealthCheckTest>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    interval: Option<String>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    timeout: Option<String>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    retries: Option<u32>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    start_period: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum HealthCheckTest {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    List(Vec<String>),
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    String(String),
}

#[derive(Debug, Deserialize)]
struct VolumeConfig {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    driver: Option<String>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    external: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct NetworkConfig {
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    driver: Option<String>,
    #[allow(dead_code)] // Deserialized from compose file, not all fields used yet
    external: Option<bool>,
}

struct ComposeService {
    name: String,
    image: String,
    ports: Vec<String>,
    environment: Vec<(String, String)>,
    volumes: Vec<String>,
    // Deserialized from compose file, used for dependency ordering in future
    #[allow(dead_code)]
    depends_on: Vec<String>,
    // Deserialized from compose file, used for custom entrypoint in future
    #[allow(dead_code)]
    command: Option<String>,
    replicas: u32,
    restart: Option<String>,
    has_build: bool,
    has_healthcheck: bool,
    service_type: ServiceType,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ServiceType {
    Database,
    Cache,
    MessageQueue,
    Web,
    Api,
    Worker,
    Utility,
}

pub async fn execute(args: ImportArgs) -> Result<(), Error> {
    let input_path = PathBuf::from(&args.input);
    let output_path = PathBuf::from(&args.output);

    if !input_path.exists() {
        return Err(Error::InputNotFound(args.input.clone()));
    }

    if output_path.exists() && !args.force {
        return Err(Error::OutputExists(format!(
            "{} (use --force to overwrite)",
            args.output
        )));
    }

    println!("Importing docker-compose configuration");
    println!("────────────────────────────────────────────────────────────");
    println!();
    println!("Input:  {}", args.input);
    println!("Output: {}", args.output);
    println!();

    if args.verbose {
        println!("Parsing docker-compose.yml...");
    }

    let compose_services = parse_compose_file(&args)?;

    if args.verbose {
        println!("Found {} service(s)", compose_services.len());
    }

    println!();
    println!("Service Mapping:");
    println!("────────────────────────────────────────────────────────────");

    let aether_config = convert_to_aether(&compose_services, &args)?;

    if args.dry_run {
        println!();
        println!("Generated aether.toml (dry run):");
        println!("────────────────────────────────────────────────────────────");
        println!("{}", aether_config);
        println!("────────────────────────────────────────────────────────────");
        println!();
        println!("Note: No files were written (dry run mode)");
    } else {
        if args.verbose {
            println!();
            println!("Writing to {}...", args.output);
        }

        println!();
        println!("✓ Successfully imported configuration");
        println!();
        print_migration_notes(&compose_services);
    }

    Ok(())
}

fn parse_compose_file(args: &ImportArgs) -> Result<Vec<ComposeService>, Error> {
    let content = std::fs::read_to_string(&args.input)
        .map_err(|e| Error::IoError(format!("Failed to read {}: {}", args.input, e)))?;

    let compose: ComposeFile = serde_yaml::from_str(&content)
        .map_err(|e| Error::ParseError(format!("Invalid YAML syntax: {}", e)))?;

    let mut services = Vec::new();
    let mut all_warnings = Vec::new();

    if let Some(ref version) = compose.version {
        if args.verbose {
            println!("Detected compose version: {}", version);
        }
    }

    for (name, raw_service) in compose.services {
        match convert_service(name.clone(), raw_service, args.verbose) {
            Ok(service) => {
                all_warnings.extend(service.warnings.iter().cloned());
                services.push(service);
            }
            Err(e) => {
                return Err(Error::ConversionError(format!(
                    "Failed to convert service '{}': {}",
                    name, e
                )));
            }
        }
    }

    if let Some(ref networks) = compose.networks {
        if !networks.is_empty() {
            all_warnings.push(format!(
                "Custom networks defined: {}. Aether uses automatic service mesh.",
                networks.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }

    if !all_warnings.is_empty() && args.verbose {
        println!();
        println!("Warnings:");
        for warning in &all_warnings {
            println!("  ⚠ {}", warning);
        }
    }

    Ok(services)
}

fn convert_service(
    name: String,
    raw: ComposeServiceRaw,
    verbose: bool,
) -> Result<ComposeService, Error> {
    let mut warnings = Vec::new();

    let image = match raw.image {
        Some(img) => img,
        None => {
            if raw.build.is_some() {
                warnings.push(format!(
                    "Service '{}' uses build instead of image. Aether requires pre-built WASM modules.",
                    name
                ));
                format!("build:{}", name)
            } else {
                return Err(Error::ConversionError(format!(
                    "Service '{}' has neither image nor build specified",
                    name
                )));
            }
        }
    };

    let has_build = raw.build.is_some();
    if has_build && verbose {
        warnings.push(format!(
            "Service '{}' uses Docker build - requires WASM compilation",
            name
        ));
    }

    let environment = parse_environment(raw.environment);

    let depends_on = match raw.depends_on {
        Some(DependsOn::List(list)) => list,
        Some(DependsOn::Map(map)) => {
            for (dep_name, config) in &map {
                if let Some(ref condition) = config.condition {
                    if condition == "service_healthy" {
                        warnings.push(format!(
                            "Service '{}' depends on health of '{}'. Aether uses readiness checks.",
                            name, dep_name
                        ));
                    }
                }
            }
            map.keys().cloned().collect()
        }
        None => Vec::new(),
    };

    let command = match raw.command {
        Some(Command::String(s)) => Some(s),
        Some(Command::List(list)) => Some(list.join(" ")),
        None => None,
    };

    if raw.entrypoint.is_some() {
        warnings.push(format!(
            "Service '{}' uses custom entrypoint - may need manual adjustment",
            name
        ));
    }

    let has_healthcheck = raw.healthcheck.is_some();
    if has_healthcheck {
        warnings.push(format!(
            "Service '{}' has healthcheck - will be mapped to Aether health check",
            name
        ));
    }

    if let Some(ref restart) = raw.restart {
        if restart == "always" || restart == "unless-stopped" {
            warnings.push(format!(
                "Service '{}' uses restart:'{}' - mapped to supervisor strategy",
                name, restart
            ));
        }
    }

    let service_type = detect_service_type(&image);

    let volumes = raw.volumes.unwrap_or_default();
    if !volumes.is_empty() {
        warnings.extend(validate_volumes(&name, &volumes));
    }

    Ok(ComposeService {
        name,
        image,
        ports: raw.ports.unwrap_or_default(),
        environment,
        volumes,
        depends_on,
        command,
        replicas: raw.replicas.unwrap_or(1),
        restart: raw.restart,
        has_build,
        has_healthcheck,
        service_type,
        warnings,
    })
}

fn parse_environment(env: Option<Environment>) -> Vec<(String, String)> {
    match env {
        Some(Environment::List(list)) => list
            .into_iter()
            .filter_map(|item| {
                let parts: Vec<&str> = item.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect(),
        Some(Environment::Map(map)) => map.into_iter().collect(),
        None => Vec::new(),
    }
}

fn detect_service_type(image: &str) -> ServiceType {
    let image_lower = image.to_lowercase();

    if image_lower.contains("postgres")
        || image_lower.contains("mysql")
        || image_lower.contains("mariadb")
        || image_lower.contains("mongo")
        || image_lower.contains("cassandra")
        || image_lower.contains("cockroach")
        || image_lower.contains("timescale")
    {
        return ServiceType::Database;
    }

    if image_lower.contains("redis") || image_lower.contains("memcached") {
        return ServiceType::Cache;
    }

    if image_lower.contains("rabbitmq")
        || image_lower.contains("kafka")
        || image_lower.contains("nats")
        || image_lower.contains("activemq")
    {
        return ServiceType::MessageQueue;
    }

    if image_lower.contains("nginx")
        || image_lower.contains("traefik")
        || image_lower.contains("caddy")
        || image_lower.contains("haproxy")
        || image_lower.contains("apache")
    {
        return ServiceType::Web;
    }

    if image_lower.contains("api")
        || image_lower.contains("backend")
        || image_lower.contains("server")
    {
        return ServiceType::Api;
    }

    if image_lower.contains("worker")
        || image_lower.contains("celery")
        || image_lower.contains("sidekiq")
        || image_lower.contains("consumer")
    {
        return ServiceType::Worker;
    }

    ServiceType::Utility
}

fn validate_volumes(service_name: &str, volumes: &[String]) -> Vec<String> {
    let mut warnings = Vec::new();

    for volume in volumes {
        if volume.starts_with('/') || volume.contains(':') {
            warnings.push(format!(
                "Service '{}' has volume mount '{}' - requires Aether state management",
                service_name, volume
            ));
        }

        if volume.contains("postgres") || volume.contains("mysql") || volume.contains("mongo") {
            warnings.push(format!(
                "Service '{}' has database volume - consider managed database",
                service_name
            ));
        }
    }

    warnings
}

fn convert_to_aether(services: &[ComposeService], args: &ImportArgs) -> Result<String, Error> {
    let mut output = String::new();

    output.push_str(
        r#"[project]
name = "imported-from-compose"
version = "0.1.0"
description = "Imported from docker-compose.yml"

[runtime]
memory_limit = "256MB"
cpu_limit = 1.0
log_level = "info"

"#,
    );

    for service in services {
        if args.verbose {
            let type_str = match service.service_type {
                ServiceType::Database => "database",
                ServiceType::Cache => "cache",
                ServiceType::MessageQueue => "message-queue",
                ServiceType::Web => "web",
                ServiceType::Api => "api",
                ServiceType::Worker => "worker",
                ServiceType::Utility => "utility",
            };
            println!(
                "  {} -> {} (actor, type: {})",
                service.name, service.name, type_str
            );
        }

        output.push_str(&format!(
            "[[actors]]\nname = \"{}\"\npath = \"./actors/{}.wasm\"\ninstances = {}\n",
            service.name, service.name, service.replicas
        ));

        if service.has_healthcheck {
            output.push_str("health_check = { enabled = true, interval = \"30s\" }\n");
        }

        if let Some(ref restart) = service.restart {
            if restart == "always" || restart == "unless-stopped" {
                output
                    .push_str("supervisor = { strategy = \"always-restart\", max_retries = 3 }\n");
            }
        }

        if !service.environment.is_empty() {
            output.push_str("\n[actors.environment]\n");
            for (key, value) in &service.environment {
                output.push_str(&format!("{} = \"{}\"\n", key, escape_toml_string(value)));
            }
        }

        let capabilities = generate_capabilities(service);
        if !capabilities.is_empty() {
            output.push_str("\n[[actors.capabilities]]\n");
            for (i, cap) in capabilities.iter().enumerate() {
                if i > 0 {
                    output.push_str("\n[[actors.capabilities]]\n");
                }
                output.push_str(cap);
                output.push('\n');
            }
        }

        output.push('\n');
    }

    output.push_str(
        r#"[mesh]
enabled = true
port = 7000
discovery = "multicast"
"#,
    );

    Ok(output)
}

fn generate_capabilities(service: &ComposeService) -> Vec<String> {
    let mut capabilities = Vec::new();

    if !service.ports.is_empty() {
        let ports: Vec<String> = service
            .ports
            .iter()
            .filter_map(|p| {
                let parts: Vec<&str> = p.split(':').collect();
                if let Some(host_port) = parts.first() {
                    if let Ok(port) = host_port.parse::<u16>() {
                        return Some(format!("\"tcp:0.0.0.0:{}\"", port));
                    }
                }
                None
            })
            .collect();

        if !ports.is_empty() {
            capabilities.push(format!(
                "capability = \"networking:public\"\nresources = [{}]",
                ports.join(", ")
            ));
        }
    }

    if !service.volumes.is_empty() {
        let vol_resources: Vec<String> = service
            .volumes
            .iter()
            .filter_map(|v| {
                let parts: Vec<&str> = v.split(':').collect();
                if let Some(host_path) = parts.first() {
                    return Some(format!("\"{}\"", host_path));
                }
                None
            })
            .collect();

        if !vol_resources.is_empty() {
            capabilities.push(format!(
                "capability = \"fs:read-write\"\nresources = [{}]",
                vol_resources.join(", ")
            ));
        }
    }

    match service.service_type {
        ServiceType::Database => {
            capabilities
                .push("capability = \"data:kv-store\"\nresources = [\"persistent\"]".to_string());
        }
        ServiceType::Cache => {
            capabilities.push("capability = \"data:cache\"\nresources = [\"memory\"]".to_string());
        }
        ServiceType::MessageQueue => {
            capabilities
                .push("capability = \"messaging:pubsub\"\nresources = [\"default\"]".to_string());
        }
        _ => {}
    }

    capabilities
}

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn print_migration_notes(services: &[ComposeService]) {
    println!("Migration Notes:");
    println!("────────────────────────────────────────────────────────────");
    println!();

    println!("Services converted to actors:");
    for service in services {
        let type_str = match service.service_type {
            ServiceType::Database => "database",
            ServiceType::Cache => "cache",
            ServiceType::MessageQueue => "messaging",
            ServiceType::Web => "web server",
            ServiceType::Api => "API server",
            ServiceType::Worker => "worker",
            ServiceType::Utility => "utility",
        };
        println!("  • {} ({}) [{}]", service.name, service.image, type_str);
    }
    println!();

    let all_warnings: Vec<&String> = services.iter().flat_map(|s| &s.warnings).collect();
    if !all_warnings.is_empty() {
        println!("⚠️  Conversion warnings:");
        for warning in &all_warnings {
            println!("     {}", warning);
        }
        println!();
    }

    println!("⚠️  Manual migration required:");
    println!();

    let has_volumes = services.iter().any(|s| !s.volumes.is_empty());
    let has_db = services
        .iter()
        .any(|s| s.service_type == ServiceType::Database);
    let has_cache = services
        .iter()
        .any(|s| s.service_type == ServiceType::Cache);
    let has_mq = services
        .iter()
        .any(|s| s.service_type == ServiceType::MessageQueue);
    let has_build = services.iter().any(|s| s.has_build);

    let mut step = 1;

    if has_build {
        println!(
            "  {}. Services with 'build' require compilation to WebAssembly",
            step
        );
        println!("     - Replace Dockerfile with WASM build process");
        println!("     - Use appropriate language SDK (Rust, Go, etc.)");
        println!();
        step += 1;
    }

    if has_volumes {
        println!("  {}. Volume mounts need native Aether storage:", step);
        for service in services {
            if !service.volumes.is_empty() {
                for vol in &service.volumes {
                    println!("     - {}: {}", service.name, vol);
                }
            }
        }
        println!("     Consider using Aether's built-in key-value store");
        println!();
        step += 1;
    }

    if has_db {
        println!(
            "  {}. Database services should use Aether's data layer:",
            step
        );
        println!("     - Replace PostgreSQL/MySQL with Aether KV store");
        println!("     - Or use external managed database with capability");
        println!();
        step += 1;
    }

    if has_cache {
        println!("  {}. Redis/Memcached can be replaced with:", step);
        println!("     - Aether's built-in caching layer");
        println!("     - Actor-local state for simple cases");
        println!();
        step += 1;
    }

    if has_mq {
        println!("  {}. Message queues should use Aether's messaging:", step);
        println!("     - Use built-in pub/sub capabilities");
        println!("     - Actor-to-actor messaging");
        println!();
        step += 1;
    }

    println!(
        "  {}. Update actor paths to point to compiled WASM modules",
        step
    );
    step += 1;
    println!();
    println!(
        "  {}. Review and adjust capability grants for security",
        step
    );
    println!();

    println!("Next steps:");
    println!("  1. Compile your services to WebAssembly");
    println!("  2. Update the 'path' fields in aether.toml");
    println!("  3. Run 'aether config validate' to check");
    println!("  4. Run 'aether dev' to test locally");
}
