//! Deterministic `SoraDNS` gateway host derivation.

use blake3::hash as blake3_hash;
use thiserror::Error;

/// Maximum length of a fully-qualified domain name accepted by the helper.
const MAX_FQDN_LENGTH: usize = 253;
/// Maximum length of an individual DNS label.
const MAX_LABEL_LENGTH: usize = 63;
const CANONICAL_SUFFIX: &str = "gw.sora.id";
const PRETTY_SUFFIX: &str = "gw.sora.name";
const CANONICAL_WILDCARD: &str = "*.gw.sora.id";

/// Errors returned when deriving gateway hosts from an FQDN.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum GatewayHostError {
    /// The supplied FQDN was empty or consisted solely of whitespace.
    #[error("fqdn must not be empty")]
    EmptyName,
    /// The FQDN may not begin or end with a dot.
    #[error("fqdn may not start or end with a dot")]
    LeadingOrTrailingDot,
    /// Labels must be separated by single dots.
    #[error("fqdn contains consecutive dots")]
    ConsecutiveDots,
    /// Individual labels must not exceed 63 characters.
    #[error("fqdn label `{label}` exceeds {MAX_LABEL_LENGTH} characters (found {length})")]
    LabelTooLong {
        /// Offending label.
        label: String,
        /// Observed length.
        length: usize,
    },
    /// The entire FQDN exceeded the DNS limit of 253 characters.
    #[error("fqdn exceeds {MAX_FQDN_LENGTH} characters (found {length})")]
    NameTooLong {
        /// Observed length.
        length: usize,
    },
    /// Unsupported ASCII character encountered within the FQDN.
    #[error("fqdn contains unsupported character `{character}`")]
    UnsupportedCharacter {
        /// Unsupported character.
        character: char,
    },
}

/// Deterministic gateway hosts associated with a `SoraDNS` name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayHostBindings {
    normalized_name: String,
    canonical_label: String,
    canonical_host: String,
    pretty_host: String,
}

impl GatewayHostBindings {
    /// Returns the canonicalised FQDN used for hashing.
    #[must_use]
    pub fn normalized_name(&self) -> &str {
        &self.normalized_name
    }

    /// Returns the base32-encoded Blake3 label used for canonical resolution.
    #[must_use]
    pub fn canonical_label(&self) -> &str {
        &self.canonical_label
    }

    /// Returns the canonical gateway host (`<hash>.gw.sora.id`).
    #[must_use]
    pub fn canonical_host(&self) -> &str {
        &self.canonical_host
    }

    /// Returns the pretty gateway host (`<fqdn>.gw.sora.name`).
    #[must_use]
    pub fn pretty_host(&self) -> &str {
        &self.pretty_host
    }

    /// Returns the wildcard pattern authorised for canonical hosts.
    #[must_use]
    pub const fn canonical_wildcard() -> &'static str {
        CANONICAL_WILDCARD
    }

    /// Returns the host patterns that a GAR entry must authorise.
    #[must_use]
    pub fn host_patterns(&self) -> [&str; 3] {
        [
            self.canonical_host(),
            Self::canonical_wildcard(),
            self.pretty_host(),
        ]
    }

    /// Checks whether the supplied hostname matches one of the derived hosts.
    #[must_use]
    pub fn matches_host(&self, candidate: &str) -> bool {
        canonicalise_host(candidate)
            .is_some_and(|host| host == self.canonical_host || host == self.pretty_host)
    }
}

/// Return the canonical gateway suffix (`gw.sora.id`).
#[must_use]
pub const fn canonical_gateway_suffix() -> &'static str {
    CANONICAL_SUFFIX
}

/// Return the pretty gateway suffix (`gw.sora.name`).
#[must_use]
pub const fn pretty_gateway_suffix() -> &'static str {
    PRETTY_SUFFIX
}

/// Return the wildcard pattern authorised for canonical hosts (`*.gw.sora.id`).
#[must_use]
pub const fn canonical_gateway_wildcard_pattern() -> &'static str {
    CANONICAL_WILDCARD
}

/// Derive deterministic gateway hosts for the supplied `SoraDNS` FQDN.
///
/// The FQDN must already be ASCII-compatible (Punycode) and follow the
/// normalisation rules specified in `soradns_registry_rfc.md`.
///
/// # Errors
///
/// Returns [`GatewayHostError`] when the FQDN violates DNS label rules.
pub fn derive_gateway_hosts(fqdn: &str) -> Result<GatewayHostBindings, GatewayHostError> {
    let normalised = normalise_fqdn(fqdn)?;
    let digest = blake3_hash(normalised.as_bytes());
    let canonical_label = encode_base32_lower(digest.as_bytes());
    let canonical_host = format!("{canonical_label}.{CANONICAL_SUFFIX}");
    let pretty_host = format!("{normalised}.{PRETTY_SUFFIX}");
    Ok(GatewayHostBindings {
        normalized_name: normalised,
        canonical_label,
        canonical_host,
        pretty_host,
    })
}

fn normalise_fqdn(input: &str) -> Result<String, GatewayHostError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(GatewayHostError::EmptyName);
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return Err(GatewayHostError::LeadingOrTrailingDot);
    }
    let mut normalised = String::with_capacity(trimmed.len());
    let mut label_len = 0usize;
    for ch in trimmed.chars() {
        match ch {
            'A'..='Z' => {
                normalised.push(ch.to_ascii_lowercase());
                label_len += 1;
            }
            'a'..='z' | '0'..='9' | '-' => {
                normalised.push(ch);
                label_len += 1;
            }
            '.' => {
                if label_len == 0 {
                    return Err(GatewayHostError::ConsecutiveDots);
                }
                if label_len > MAX_LABEL_LENGTH {
                    let start = normalised.len() - label_len;
                    let label = normalised[start..].to_string();
                    return Err(GatewayHostError::LabelTooLong {
                        label,
                        length: label_len,
                    });
                }
                normalised.push('.');
                label_len = 0;
            }
            other if other.is_ascii() => {
                return Err(GatewayHostError::UnsupportedCharacter { character: other });
            }
            other => {
                return Err(GatewayHostError::UnsupportedCharacter { character: other });
            }
        }
    }
    if label_len == 0 {
        // Ending with a dot is caught earlier; reaching zero here means two dots in a row.
        return Err(GatewayHostError::ConsecutiveDots);
    }
    if label_len > MAX_LABEL_LENGTH {
        let start = normalised.len() - label_len;
        let label = normalised[start..].to_string();
        return Err(GatewayHostError::LabelTooLong {
            label,
            length: label_len,
        });
    }
    if normalised.len() > MAX_FQDN_LENGTH {
        return Err(GatewayHostError::NameTooLong {
            length: normalised.len(),
        });
    }
    Ok(normalised)
}

fn canonicalise_host(host: &str) -> Option<String> {
    let trimmed = host.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return None;
    }
    if trimmed
        .bytes()
        .any(|byte| !matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.'))
    {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

fn encode_base32_lower(data: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if data.is_empty() {
        return String::new();
    }
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity((data.len() * 8).div_ceil(5));
    for &byte in data {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((acc >> bits) & 0x1f) as usize;
            out.push(ALPHABET[index]);
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[index]);
    }
    String::from_utf8(out).expect("alphabet contains valid ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_canonical_and_pretty_hosts() {
        let bindings = derive_gateway_hosts("App.Dao.SORA").expect("hosts to derive");
        assert_eq!(bindings.normalized_name(), "app.dao.sora");
        assert_eq!(bindings.pretty_host(), "app.dao.sora.gw.sora.name");
        assert!(bindings.canonical_host().ends_with(".gw.sora.id"));
        assert_eq!(
            bindings.host_patterns(),
            [
                bindings.canonical_host(),
                GatewayHostBindings::canonical_wildcard(),
                bindings.pretty_host()
            ]
        );
        assert!(bindings.matches_host(bindings.canonical_host()));
        assert!(bindings.matches_host("APP.DAO.SORA.GW.SORA.NAME"));
        assert!(!bindings.matches_host("example.com"));
    }

    #[test]
    fn canonical_label_matches_blake3_base32() {
        let bindings = derive_gateway_hosts("docs.sora").expect("hosts to derive");
        let digest = blake3_hash(bindings.normalized_name().as_bytes());
        let expected_label = encode_base32_lower(digest.as_bytes());
        assert_eq!(bindings.canonical_label(), expected_label);
        assert_eq!(
            bindings.canonical_host(),
            format!("{expected_label}.{}", canonical_gateway_suffix())
        );
    }

    #[test]
    fn rejects_invalid_characters() {
        let err = derive_gateway_hosts("app dao sora").expect_err("invalid character");
        assert!(matches!(
            err,
            GatewayHostError::UnsupportedCharacter { character: ' ' }
        ));
    }

    #[test]
    fn rejects_consecutive_dots() {
        let err = derive_gateway_hosts("app..sora").expect_err("consecutive dots");
        assert!(matches!(err, GatewayHostError::ConsecutiveDots));
    }

    #[test]
    fn rejects_long_labels() {
        let label = "a".repeat(MAX_LABEL_LENGTH + 1);
        let fqdn = format!("{label}.sora");
        let err = derive_gateway_hosts(&fqdn).expect_err("label too long");
        assert!(matches!(
            err,
            GatewayHostError::LabelTooLong { length, .. } if length == MAX_LABEL_LENGTH + 1
        ));
    }
}
