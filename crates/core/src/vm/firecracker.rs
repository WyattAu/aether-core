//! Firecracker API Client
//!
//! Unix socket-based communication with Firecracker MicroVM.

use crate::error::{Error, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::api::*;

const DEFAULT_TIMEOUT_MS: u64 = 5000;
const API_VERSION: &str = "1";

/// Configuration for a Firecracker VM client connection.
#[derive(Debug, Clone)]
pub struct FirecrackerConfig {
    /// Path to the Firecracker Unix domain socket.
    pub socket_path: PathBuf,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Firecracker API version string.
    pub api_version: String,
}

impl Default for FirecrackerConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/firecracker.socket"),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            api_version: API_VERSION.to_string(),
        }
    }
}

/// Async client for communicating with a Firecracker MicroVM over its API socket.
pub struct FirecrackerClient {
    socket_path: PathBuf,
    timeout_ms: u64,
    api_version: String,
}

impl FirecrackerClient {
    /// Creates a new client from the given configuration.
    pub fn new(config: FirecrackerConfig) -> Self {
        Self {
            socket_path: config.socket_path,
            timeout_ms: config.timeout_ms,
            api_version: config.api_version,
        }
    }

    /// Creates a client connected to the given socket path with default timeout and API version.
    pub fn with_socket(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            api_version: API_VERSION.to_string(),
        }
    }

    async fn connect(&self) -> Result<UnixStream> {
        let timeout = Duration::from_millis(self.timeout_ms);

        tokio::time::timeout(timeout, UnixStream::connect(&self.socket_path))
            .await
            .map_err(|_| Error::actor("Connection timeout"))?
            .map_err(Error::io)
    }

    async fn request(&self, method: &str, path: &str, body: Option<&str>) -> Result<String> {
        let mut stream = self.connect().await?;

        let full_path = format!("/{}/{}", self.api_version, path);
        let body_content = body.unwrap_or("");
        let content_length = body_content.len();

        let request = format!(
            "{} {} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nContent-Type: application/json\r\nAccept: application/json\r\n\r\n{}",
            method, full_path, content_length, body_content
        );

        stream
            .write_all(request.as_bytes())
            .await
            .map_err(Error::io)?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();

        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.timeout_ms);

        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(Error::actor("Response timeout"));
            }

            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    response.push_str(&line);
                }
                Err(e) => return Err(Error::io(e)),
            }

            if line == "\r\n" {
                let mut body = String::new();
                let _ = reader.read_to_string(&mut body).await;
                response.push_str(&body);
                break;
            }
        }

        self.parse_response(&response)
    }

    fn parse_response(&self, response: &str) -> Result<String> {
        let header_end = response
            .find("\r\n\r\n")
            .ok_or_else(|| Error::actor("Invalid HTTP response"))?;

        let headers = &response[..header_end];
        let body = &response[header_end + 4..];

        let status_line = headers
            .lines()
            .next()
            .ok_or_else(|| Error::actor("Missing status line"))?;

        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| Error::actor("Missing status code"))?
            .parse::<u16>()
            .map_err(|_| Error::actor("Invalid status code"))?;

        if (200..300).contains(&status_code) {
            Ok(body.to_string())
        } else if status_code >= 400 {
            Err(Error::actor(format!("API error {}: {}", status_code, body)))
        } else {
            Ok(body.to_string())
        }
    }

    fn make_path(&self, endpoint: &str) -> String {
        endpoint.trim_start_matches('/').to_string()
    }

    /// Sets the machine configuration (vCPU count, memory size, etc.).
    pub async fn put_machine_config(&self, config: &MachineConfig) -> Result<()> {
        let body =
            serde_json::to_string(config).map_err(|e| Error::serialization(e.to_string()))?;

        self.request("PUT", &self.make_path("machine-config"), Some(&body))
            .await?;

        Ok(())
    }

    /// Retrieves the current machine configuration.
    pub async fn get_machine_config(&self) -> Result<MachineConfig> {
        let response = self
            .request("GET", &self.make_path("machine-config"), None)
            .await?;

        serde_json::from_str(&response).map_err(|e| Error::serialization(e.to_string()))
    }

    /// Sets the boot source (kernel image and boot args).
    pub async fn put_boot_source(&self, boot_source: &BootSource) -> Result<()> {
        let body =
            serde_json::to_string(boot_source).map_err(|e| Error::serialization(e.to_string()))?;

        self.request("PUT", &self.make_path("boot-source"), Some(&body))
            .await?;

        Ok(())
    }

    /// Adds or updates a block device drive by ID.
    pub async fn put_drive(&self, drive_id: &str, drive: &Drive) -> Result<()> {
        let body = serde_json::to_string(drive).map_err(|e| Error::serialization(e.to_string()))?;

        let path = self.make_path(&format!("drives/{}", drive_id));
        self.request("PUT", &path, Some(&body)).await?;

        Ok(())
    }

    /// Patches an existing block device drive by ID.
    pub async fn patch_drive(&self, drive_id: &str, drive: &Drive) -> Result<()> {
        let body = serde_json::to_string(drive).map_err(|e| Error::serialization(e.to_string()))?;

        let path = self.make_path(&format!("drives/{}", drive_id));
        self.request("PATCH", &path, Some(&body)).await?;

        Ok(())
    }

    /// Adds or updates a network interface by ID.
    pub async fn put_network_interface(
        &self,
        iface_id: &str,
        iface: &NetworkInterface,
    ) -> Result<()> {
        let body = serde_json::to_string(iface).map_err(|e| Error::serialization(e.to_string()))?;

        let path = self.make_path(&format!("network-interfaces/{}", iface_id));
        self.request("PUT", &path, Some(&body)).await?;

        Ok(())
    }

    /// Configures the virtio-vsock device for host-guest communication.
    pub async fn put_vsock(&self, vsock: &Vsock) -> Result<()> {
        let body = serde_json::to_string(vsock).map_err(|e| Error::serialization(e.to_string()))?;

        self.request("PUT", &self.make_path("vsock"), Some(&body))
            .await?;

        Ok(())
    }

    /// Configures the MMDS (MicroVM Metadata Service).
    pub async fn put_mmds_config(&self, config: &MmdsConfig) -> Result<()> {
        let body =
            serde_json::to_string(config).map_err(|e| Error::serialization(e.to_string()))?;

        self.request("PUT", &self.make_path("mmds/config"), Some(&body))
            .await?;

        Ok(())
    }

    /// Sends an instance action (start, halt, etc.).
    pub async fn put_actions(&self, action: InstanceAction) -> Result<()> {
        let payload = ActionPayload {
            action_type: action,
        };
        let body =
            serde_json::to_string(&payload).map_err(|e| Error::serialization(e.to_string()))?;

        self.request("PUT", &self.make_path("actions"), Some(&body))
            .await?;

        Ok(())
    }

    /// Starts the Firecracker VM instance.
    pub async fn start_instance(&self) -> Result<()> {
        self.put_actions(InstanceAction::InstanceStart).await
    }

    /// Halts (stops) the Firecracker VM instance.
    pub async fn halt_instance(&self) -> Result<()> {
        self.put_actions(InstanceAction::InstanceHalt).await
    }

    /// Retrieves basic instance information.
    pub async fn get_info(&self) -> Result<InstanceInfo> {
        let response = self.request("GET", "", None).await?;

        serde_json::from_str(&response).map_err(|e| Error::serialization(e.to_string()))
    }

    /// Retrieves the full machine configuration.
    pub async fn get_full_config(&self) -> Result<FullMachineConfig> {
        let response = self
            .request("GET", &self.make_path("vm/config"), None)
            .await?;

        serde_json::from_str(&response).map_err(|e| Error::serialization(e.to_string()))
    }

    /// Creates a snapshot of the running VM.
    pub async fn create_snapshot(&self, params: &CreateSnapshotParams) -> Result<()> {
        let body =
            serde_json::to_string(params).map_err(|e| Error::serialization(e.to_string()))?;

        let path = self.make_path("snapshot/create");
        self.request("PUT", &path, Some(&body)).await?;

        Ok(())
    }

    /// Loads a VM from a previously created snapshot.
    pub async fn load_snapshot(&self, params: &LoadSnapshotParams) -> Result<()> {
        let body =
            serde_json::to_string(params).map_err(|e| Error::serialization(e.to_string()))?;

        let path = self.make_path("snapshot/load");
        self.request("PUT", &path, Some(&body)).await?;

        Ok(())
    }

    /// Pauses the VM.
    pub async fn pause_vm(&self) -> Result<()> {
        let path = self.make_path("vm");
        self.request("PATCH", &path, Some(r#"{"state":"Paused"}"#))
            .await?;

        Ok(())
    }

    /// Resumes a paused VM.
    pub async fn resume_vm(&self) -> Result<()> {
        let path = self.make_path("vm");
        self.request("PATCH", &path, Some(r#"{"state":"Resumed"}"#))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = FirecrackerConfig {
            socket_path: PathBuf::from("/tmp/test.sock"),
            timeout_ms: 1000,
            api_version: "1".to_string(),
        };

        let client = FirecrackerClient::new(config);
        assert_eq!(client.socket_path, PathBuf::from("/tmp/test.sock"));
        assert_eq!(client.timeout_ms, 1000);
    }

    #[test]
    fn test_default_config() {
        let config = FirecrackerConfig::default();
        assert_eq!(config.socket_path, PathBuf::from("/run/firecracker.socket"));
        assert_eq!(config.timeout_ms, DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn test_make_path() {
        let client = FirecrackerClient::with_socket(PathBuf::from("/tmp/test.sock"));
        assert_eq!(client.make_path("/machine-config"), "machine-config");
        assert_eq!(client.make_path("machine-config"), "machine-config");
    }
}
