//! Shared MCP wire constants for Torii servers and repository clients.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

const BASE64_SENTINEL_PREFIX: &str = "=?base64?";
const BASE64_SENTINEL_SUFFIX: &str = "?=";

/// Native stateless MCP protocol revision implemented by Torii.
pub const MODERN_PROTOCOL_VERSION: &str = "2026-07-28";
/// Initialization-based compatibility revision retained during migration.
pub const LEGACY_PROTOCOL_VERSION: &str = "2025-06-18";

/// HTTP header carrying the request protocol version.
pub const HEADER_PROTOCOL_VERSION: &str = "mcp-protocol-version";
/// HTTP header mirroring the JSON-RPC method.
pub const HEADER_METHOD: &str = "mcp-method";
/// Conditional HTTP header mirroring a tool/prompt name or resource URI.
pub const HEADER_NAME: &str = "mcp-name";

/// Required modern `_meta` key carrying the protocol revision.
pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
/// Recommended modern `_meta` key identifying the client implementation.
pub const META_CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";
/// Required modern `_meta` key carrying request-scoped client capabilities.
pub const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
/// Recommended result `_meta` key identifying the server implementation.
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// Encode a body value for a mirrored MCP HTTP routing header.
///
/// Plain header-safe ASCII is preserved. Empty values, non-ASCII text,
/// leading or trailing whitespace, and values that could be mistaken for the
/// Base64 sentinel are encoded as canonical padded Base64 over UTF-8.
#[must_use]
pub fn encode_mirrored_header_value(value: &str) -> String {
    let bytes = value.as_bytes();
    if plain_header_value_is_safe(bytes)
        && !(value.starts_with(BASE64_SENTINEL_PREFIX) && value.ends_with(BASE64_SENTINEL_SUFFIX))
    {
        return value.to_owned();
    }
    format!(
        "{BASE64_SENTINEL_PREFIX}{}{BASE64_SENTINEL_SUFFIX}",
        BASE64_STANDARD.encode(bytes)
    )
}

/// Decode and validate a mirrored MCP HTTP routing header value.
///
/// Sentinel encodings must be canonical padded Base64. Plain values follow
/// HTTP field-value safety rules and may contain an internal horizontal tab,
/// but not leading or trailing whitespace.
#[must_use]
pub fn decode_mirrored_header_value(value: &str) -> Option<String> {
    if let Some(encoded) = value
        .strip_prefix(BASE64_SENTINEL_PREFIX)
        .and_then(|value| value.strip_suffix(BASE64_SENTINEL_SUFFIX))
    {
        let decoded = BASE64_STANDARD.decode(encoded).ok()?;
        if BASE64_STANDARD.encode(&decoded) != encoded {
            return None;
        }
        return String::from_utf8(decoded).ok();
    }
    plain_header_value_is_safe(value.as_bytes()).then(|| value.to_owned())
}

fn plain_header_value_is_safe(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && !bytes.first().is_some_and(u8::is_ascii_whitespace)
        && !bytes.last().is_some_and(u8::is_ascii_whitespace)
        && bytes
            .iter()
            .all(|byte| *byte == b'\t' || matches!(*byte, 0x20..=0x7e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrored_header_values_roundtrip_plain_and_encoded_text() {
        for value in [
            "iroha.health",
            "iroha.\thealth",
            "iroha.健康",
            " padded ",
            "=?base64?literal?=",
            "",
        ] {
            let encoded = encode_mirrored_header_value(value);
            assert_eq!(
                decode_mirrored_header_value(&encoded).as_deref(),
                Some(value)
            );
        }
        assert_eq!(
            encode_mirrored_header_value("iroha.健康"),
            "=?base64?aXJvaGEu5YGl5bq3?="
        );
    }

    #[test]
    fn mirrored_header_values_reject_ambiguous_or_noncanonical_input() {
        for invalid in [
            "",
            " leading",
            "trailing\t",
            "line\nbreak",
            "=?base64?YQ?=",
            "=?base64?*?=",
        ] {
            assert_eq!(decode_mirrored_header_value(invalid), None, "{invalid:?}");
        }
    }
}
