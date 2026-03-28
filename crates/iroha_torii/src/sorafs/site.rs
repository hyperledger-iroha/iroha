//! Static-site binding helpers backed by SoraFS storage.

use std::{fs, path::PathBuf};

use http::uri::Authority;

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

/// Environment variable pointing at the JSON file describing host bindings.
pub const SITE_BINDINGS_FILE_ENV: &str = "IROHA_SORAFS_SITE_BINDINGS_FILE";

#[cfg(test)]
static SITE_BINDINGS_FILE_OVERRIDE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// JSON document loaded from [`SITE_BINDINGS_FILE_ENV`].
#[derive(Debug, Clone, Default, norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
pub struct SiteBindingsDocument {
    /// Schema version for forward compatibility.
    pub version: Option<u8>,
    /// Host bindings served by Torii.
    #[norito(default)]
    pub sites: Vec<SiteBinding>,
}

/// Hostname to manifest mapping used for local static-site serving.
#[derive(
    Debug, Clone, norito::derive::JsonDeserialize, norito::derive::JsonSerialize, PartialEq, Eq,
)]
pub struct SiteBinding {
    /// Public hostname routed to the site.
    pub hostname: String,
    /// Hex-encoded manifest digest stored locally on the node.
    pub manifest_digest_hex: String,
    /// Optional index document. Defaults to `index.html`.
    #[norito(default)]
    pub index_document: Option<String>,
    /// Whether unknown extensionless paths should fall back to the index document.
    #[norito(default)]
    pub spa_fallback: Option<bool>,
}

impl SiteBinding {
    /// Return the configured index document or the default.
    #[must_use]
    pub fn index_document(&self) -> &str {
        self.index_document
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("index.html")
    }

    /// Return the SPA fallback toggle (defaults to `true`).
    #[must_use]
    pub fn spa_fallback_enabled(&self) -> bool {
        self.spa_fallback.unwrap_or(true)
    }

    /// Return the normalized host form used for lookup.
    #[must_use]
    pub fn normalized_hostname(&self) -> Option<String> {
        normalize_host_header(&self.hostname)
    }
}

#[cfg(test)]
fn site_bindings_file_override() -> &'static Mutex<Option<PathBuf>> {
    SITE_BINDINGS_FILE_OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn configured_bindings_path() -> Option<PathBuf> {
    site_bindings_file_override()
        .lock()
        .expect("site bindings override poisoned")
        .clone()
        .or_else(|| std::env::var_os(SITE_BINDINGS_FILE_ENV).map(PathBuf::from))
}

#[cfg(not(test))]
fn configured_bindings_path() -> Option<PathBuf> {
    std::env::var_os(SITE_BINDINGS_FILE_ENV).map(PathBuf::from)
}

/// Override the site-bindings path for tests without touching process environment.
#[cfg(test)]
pub fn set_site_bindings_file_override_for_tests(path: Option<PathBuf>) {
    *site_bindings_file_override()
        .lock()
        .expect("site bindings override poisoned") = path;
}

/// Load the site bindings JSON referenced by [`SITE_BINDINGS_FILE_ENV`].
pub fn load_site_bindings_from_env() -> Result<Option<SiteBindingsDocument>, String> {
    let Some(path_buf) = configured_bindings_path() else {
        return Ok(None);
    };
    let bytes = fs::read(&path_buf).map_err(|err| {
        format!(
            "failed to read site bindings `{}`: {err}",
            path_buf.display()
        )
    })?;
    let document = norito::json::from_slice::<SiteBindingsDocument>(&bytes).map_err(|err| {
        format!(
            "failed to parse site bindings `{}` as JSON: {err}",
            path_buf.display()
        )
    })?;
    Ok(Some(document))
}

/// Normalize an inbound `Host` header or configured hostname.
#[must_use]
pub fn normalize_host_header(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let authority = Authority::try_from(trimmed).ok()?;
    Some(
        authority
            .host()
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase(),
    )
}

/// Resolve the binding matching the provided host.
#[must_use]
pub fn find_site_binding<'a>(
    document: &'a SiteBindingsDocument,
    host: &str,
) -> Option<&'a SiteBinding> {
    let normalized = normalize_host_header(host)?;
    document
        .sites
        .iter()
        .find(|binding| binding.normalized_hostname().as_deref() == Some(normalized.as_str()))
}

/// Convert a request path into dataset path components.
#[must_use]
pub fn path_components_for_request(raw_path: &str, index_document: &str) -> Option<Vec<String>> {
    let trimmed = raw_path.trim_start_matches('/');
    let effective = if trimmed.is_empty() {
        index_document.to_string()
    } else if raw_path.ends_with('/') {
        format!("{trimmed}{index_document}")
    } else {
        trimmed.to_string()
    };

    let mut segments = Vec::new();
    for segment in effective.split('/') {
        let part = segment.trim();
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        segments.push(part.to_string());
    }

    if segments.is_empty() {
        return None;
    }

    Some(segments)
}

/// Decide whether an unknown path should fall back to the SPA entrypoint.
#[must_use]
pub fn should_use_spa_fallback(raw_path: &str, binding: &SiteBinding) -> bool {
    if !binding.spa_fallback_enabled() {
        return false;
    }
    let trimmed = raw_path.trim_end_matches('/');
    let last = trimmed.rsplit('/').next().unwrap_or_default();
    !last.contains('.')
}

/// Best-effort content-type lookup for static site assets.
#[must_use]
pub fn content_type_for_path(path: &[String]) -> &'static str {
    let extension = path
        .last()
        .and_then(|value| value.rsplit('.').next())
        .map(|value| value.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("map") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("xml") => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_hostnames_with_ports() {
        assert_eq!(
            normalize_host_header("taira.sora.org:443"),
            Some("taira.sora.org".to_string())
        );
    }

    #[test]
    fn request_path_defaults_to_index_document() {
        assert_eq!(
            path_components_for_request("/", "index.html"),
            Some(vec!["index.html".to_string()])
        );
        assert_eq!(
            path_components_for_request("/app/", "index.html"),
            Some(vec!["app".to_string(), "index.html".to_string()])
        );
    }

    #[test]
    fn spa_fallback_skips_extensionful_assets() {
        let binding = SiteBinding {
            hostname: "taira.sora.org".to_string(),
            manifest_digest_hex: "ab".repeat(32),
            index_document: None,
            spa_fallback: Some(true),
        };
        assert!(should_use_spa_fallback("/swap", &binding));
        assert!(!should_use_spa_fallback("/assets/app.js", &binding));
    }
}
