//! Static File Serving
//!
//! Serves dashboard UI files with optional embedded assets.

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use std::sync::Arc;

/// Embedded dashboard frontend assets.
#[derive(RustEmbed)]
#[folder = "ui/dist"]
pub struct DashboardAssets;

/// Configuration for static file serving.
#[derive(Clone)]
pub struct StaticFileConfig {
    /// Whether to serve embedded assets
    pub serve_embedded: bool,
    /// Name of the index file
    pub index_file: String,
    /// Prefix for asset URLs
    pub assets_prefix: String,
}

impl Default for StaticFileConfig {
    fn default() -> Self {
        Self {
            serve_embedded: true,
            index_file: "index.html".to_string(),
            assets_prefix: "/assets".to_string(),
        }
    }
}

/// Serves a static file based on the request URI.
pub async fn serve_static(uri: Uri, config: Arc<StaticFileConfig>) -> Response {
    let path = uri.path().trim_start_matches('/');

    let path = if path.is_empty() || path == config.index_file {
        config.index_file.as_str()
    } else {
        path
    };

    if config.serve_embedded {
        serve_embedded_file(path)
    } else {
        (StatusCode::NOT_FOUND, "Static file serving disabled").into_response()
    }
}

fn serve_embedded_file(path: &str) -> Response {
    match DashboardAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();

            // Build response with known valid headers - unwrap is safe here
            // but we use expect for clarity and to satisfy the zero-panic lint
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, &mime)
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(Body::from(content.data.into_owned()))
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to build response for {}: {}", path, e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response()
                })
        }
        None => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

/// Retrieves an embedded asset by path.
pub fn get_asset(path: &str) -> Option<Vec<u8>> {
    DashboardAssets::get(path).map(|f| f.data.into_owned())
}

/// Lists all embedded asset paths.
pub fn list_assets() -> Vec<String> {
    DashboardAssets::iter().map(|f| f.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_config_default() {
        let config = StaticFileConfig::default();
        assert!(config.serve_embedded);
        assert_eq!(config.index_file, "index.html");
    }
}
