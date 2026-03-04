//! Torii API version negotiation helpers.
//!
//! Torii exposes a single major API surface today. This module provides
//! parsing/formatting helpers and a minimal policy container so that version
//! negotiation can run inside middleware without pulling additional state
//! into handlers.

use std::{fmt, str::FromStr};

use axum::http::{HeaderMap, StatusCode};
use iroha_torii_shared::{
    API_MIN_PROOF_VERSION, API_VERSION_DEFAULT, API_VERSION_SUNSET_UNIX, API_VERSION_SUPPORTED,
    HEADER_API_VERSION,
};

use crate::Error;

/// Semantic API version (major/minor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApiVersion {
    /// Major component of the version (breaking changes).
    pub major: u16,
    /// Minor component of the version (additive changes).
    pub minor: u16,
}

impl ApiVersion {
    /// Construct a new semantic API version.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Render the version as `major.minor` (e.g., `1.0`).
    pub fn to_label(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for ApiVersion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let mut parts = trimmed.split('.');
        let major = parts
            .next()
            .ok_or("missing major")?
            .parse::<u16>()
            .map_err(|_| "invalid major")?;
        let minor = parts
            .next()
            .unwrap_or("0")
            .parse::<u16>()
            .map_err(|_| "invalid minor")?;
        if parts.next().is_some() {
            return Err("invalid version");
        }
        Ok(Self::new(major, minor))
    }
}

/// Static policy describing supported Torii API versions.
#[derive(Debug, Clone)]
pub struct ApiVersionPolicy {
    /// Ordered list of supported versions (oldest → newest).
    pub supported: Vec<ApiVersion>,
    /// Default version used when the client omits the header.
    pub default: ApiVersion,
    /// Minimum version required for proof/staking/fee endpoints.
    pub min_proof: ApiVersion,
    /// Optional unix timestamp when the lowest supported version sunsets.
    pub sunset_unix: Option<u64>,
}

impl Default for ApiVersionPolicy {
    fn default() -> Self {
        Self::from_labels(
            API_VERSION_SUPPORTED.iter().map(|v| (*v).to_string()),
            API_VERSION_DEFAULT.to_string(),
            API_MIN_PROOF_VERSION.to_string(),
            API_VERSION_SUNSET_UNIX,
        )
        .expect("default API version set is valid")
    }
}

impl ApiVersionPolicy {
    /// Build a policy from human-readable labels (e.g., `["1.0", "1.1"]`).
    pub fn from_labels(
        supported: impl IntoIterator<Item = String>,
        default_label: String,
        min_proof_label: String,
        sunset_unix: Option<u64>,
    ) -> Result<Self, String> {
        let mut parsed: Vec<ApiVersion> = supported
            .into_iter()
            .map(|label| parse_label(&label, "torii.api_versions"))
            .collect::<Result<_, _>>()?;
        if parsed.is_empty() {
            return Err("torii.api_versions must include at least one entry".to_string());
        }
        parsed.sort();
        parsed.dedup();

        let default = parse_label(&default_label, "torii.api_version_default")?;
        let min_proof = parse_label(&min_proof_label, "torii.api_min_proof_version")?;
        if !parsed.contains(&default) {
            return Err(format!(
                "torii.api_version_default `{default_label}` must be present in torii.api_versions ({})",
                supported_labels_from(&parsed)
            ));
        }
        if !parsed.contains(&min_proof) {
            return Err(format!(
                "torii.api_min_proof_version `{min_proof_label}` must be present in torii.api_versions ({})",
                supported_labels_from(&parsed)
            ));
        }
        if let Some(newest) = parsed.last() {
            if min_proof > *newest {
                return Err(format!(
                    "torii.api_min_proof_version `{min_proof_label}` exceeds newest supported version `{}`",
                    newest
                ));
            }
        }

        Ok(Self {
            supported: parsed,
            default,
            min_proof,
            sunset_unix,
        })
    }

    /// Supported versions rendered as comma-separated labels.
    #[must_use]
    pub fn supported_labels(&self) -> String {
        supported_labels_from(&self.supported)
    }
}

/// API version negotiation/gating error.
#[derive(Debug, Clone)]
pub enum ApiVersionError {
    /// Header could not be parsed as `major.minor`.
    InvalidHeader {
        /// Raw header value.
        raw: String,
        /// Supported versions for diagnostics.
        supported: Vec<ApiVersion>,
    },
    /// Header parsed but requested a version the node does not support.
    Unsupported {
        /// Parsed request version.
        requested: ApiVersion,
        /// Supported versions for diagnostics.
        supported: Vec<ApiVersion>,
    },
    /// Request targets a surface that requires a newer version.
    BelowMinimum {
        /// Parsed request version.
        requested: ApiVersion,
        /// Minimum required version for the surface.
        minimum: ApiVersion,
        /// Supported versions for diagnostics.
        supported: Vec<ApiVersion>,
    },
}

impl ApiVersionError {
    /// Stable error code used in responses.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidHeader { .. } => "torii_api_version_invalid",
            Self::Unsupported { .. } => "torii_api_version_unsupported",
            Self::BelowMinimum { .. } => "torii_api_version_too_old",
        }
    }

    /// Human-readable error message.
    pub fn message(&self) -> String {
        match self {
            Self::InvalidHeader { raw, supported } => format!(
                "invalid Torii API version `{}`; expected one of {}",
                raw,
                supported_labels_from(supported)
            ),
            Self::Unsupported {
                requested,
                supported,
            } => format!(
                "unsupported Torii API version `{}`; expected one of {}",
                requested,
                supported_labels_from(supported)
            ),
            Self::BelowMinimum {
                requested,
                minimum,
                supported,
            } => format!(
                "Torii API version `{}` is below the minimum `{}` required for this endpoint; supported versions: {}",
                requested,
                minimum,
                supported_labels_from(supported)
            ),
        }
    }

    /// HTTP status code to surface for the error.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BelowMinimum { .. } => StatusCode::UPGRADE_REQUIRED,
            Self::InvalidHeader { .. } | Self::Unsupported { .. } => StatusCode::BAD_REQUEST,
        }
    }

    /// Metric label for the outcome.
    pub fn result_label(&self) -> &'static str {
        match self {
            Self::InvalidHeader { .. } => "invalid",
            Self::Unsupported { .. } => "unsupported",
            Self::BelowMinimum { .. } => "too_old",
        }
    }

    /// Version label to attribute to the metric.
    pub fn version_label(&self) -> String {
        match self {
            Self::InvalidHeader { raw, .. } => raw.clone(),
            Self::Unsupported { requested, .. } | Self::BelowMinimum { requested, .. } => {
                requested.to_label()
            }
        }
    }

    /// Convert the API version error into Torii's HTTP error type.
    pub fn into_error(self) -> Error {
        Error::ApiVersion(self)
    }
}

/// Negotiation result with optional fallback indicator.
#[derive(Debug, Clone, Copy)]
pub struct NegotiatedVersion {
    /// Version ultimately selected.
    pub version: ApiVersion,
    /// Whether the selection was inferred (no header) rather than explicitly requested.
    pub inferred: bool,
}

/// Negotiate the Torii API version from request headers.
///
/// If no header is present, the default version is returned. When the header
/// value is unparsable or unsupported, an error response suitable for Axum
/// handlers is produced.
pub fn negotiate(
    headers: &HeaderMap,
    policy: &ApiVersionPolicy,
) -> Result<NegotiatedVersion, ApiVersionError> {
    let Some(value) = headers.get(HEADER_API_VERSION) else {
        return Ok(NegotiatedVersion {
            version: policy.default,
            inferred: true,
        });
    };
    let raw = value.to_str().map_err(|_| ApiVersionError::InvalidHeader {
        raw: "<non-utf8>".to_string(),
        supported: policy.supported.clone(),
    })?;

    let parsed = ApiVersion::from_str(raw).map_err(|_| ApiVersionError::InvalidHeader {
        raw: raw.to_string(),
        supported: policy.supported.clone(),
    })?;

    if policy.supported.contains(&parsed) {
        return Ok(NegotiatedVersion {
            version: parsed,
            inferred: false,
        });
    }

    Err(ApiVersionError::Unsupported {
        requested: parsed,
        supported: policy.supported.clone(),
    })
}

/// Enforce a minimum API version for a protected surface.
pub fn enforce_minimum(
    version: ApiVersion,
    minimum: ApiVersion,
    policy: &ApiVersionPolicy,
) -> Result<(), ApiVersionError> {
    if version < minimum {
        return Err(ApiVersionError::BelowMinimum {
            requested: version,
            minimum,
            supported: policy.supported.clone(),
        });
    }
    Ok(())
}

fn parse_label(label: &str, field: &str) -> Result<ApiVersion, String> {
    ApiVersion::from_str(label).map_err(|_| {
        format!(
            "invalid {} `{}` (expected a semantic version like `1.0`)",
            field, label
        )
    })
}

fn supported_labels_from(policy: &[ApiVersion]) -> String {
    policy
        .iter()
        .copied()
        .map(ApiVersion::to_label)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::*;

    #[test]
    fn to_label_formats_semver() {
        let version = ApiVersion::new(2, 5);
        assert_eq!(version.to_label(), "2.5");
    }

    #[test]
    fn supported_labels_join_in_order() {
        let versions = [ApiVersion::new(1, 1), ApiVersion::new(1, 3)];
        assert_eq!(supported_labels_from(&versions), "1.1, 1.3");
    }

    #[test]
    fn from_str_accepts_major_minor_and_rejects_extra_segments() {
        assert_eq!(ApiVersion::from_str("1").unwrap(), ApiVersion::new(1, 0));
        assert_eq!(ApiVersion::from_str("v1.2").unwrap(), ApiVersion::new(1, 2));
        assert!(ApiVersion::from_str("1.2.3").is_err());
        assert!(ApiVersion::from_str("v1.2.3").is_err());
    }

    #[test]
    fn negotiate_rejects_non_utf8_header_values() {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_bytes(&[0x80]).expect("header value");
        headers.insert(HEADER_API_VERSION, value);
        let policy = ApiVersionPolicy::default();

        let err = negotiate(&headers, &policy).expect_err("invalid header rejected");
        assert!(matches!(err, ApiVersionError::InvalidHeader { .. }));
    }
}
