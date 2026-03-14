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

#[derive(RustEmbed)]
#[folder = "ui/dist"]
pub struct DashboardAssets;

#[derive(Clone)]
pub struct StaticFileConfig {
    pub serve_embedded: bool,
    pub index_file: String,
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

pub async fn serve_static(uri: Uri, config: Arc<StaticFileConfig>) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    let path = if path.is_empty() || path == config.index_file {
        config.index_file.as_str()
    } else {
        path
    };

    if config.serve_embedded {
        serve_embedded_file(path)
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Static file serving disabled"))
            .unwrap()
    }
}

fn serve_embedded_file(path: &str) -> Response {
    match DashboardAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("File not found"))
            .unwrap(),
    }
}

pub fn get_asset(path: &str) -> Option<Vec<u8>> {
    DashboardAssets::get(path).map(|f| f.data.into_owned())
}

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
