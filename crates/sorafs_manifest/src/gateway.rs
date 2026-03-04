//! Helpers for SoraFS gateway policy enforcement.

use std::collections::HashMap;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hex::FromHex;
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::json::{Map, Value};
use thiserror::Error;

use crate::gar::{
    GarCdnPolicyV1, GarLicenseSetV1, GarMetricsPolicyV1, GarModerationAction,
    GarModerationDirectiveV1, GarPolicyPayloadV1,
};

const MAX_SAMPLING_BPS: u64 = 10_000;
const GAR_RECORD_VERSION_V1: u16 = 1;

/// Errors returned when decoding or verifying a gateway authorisation record.
#[derive(Debug, Error)]
pub enum GatewayAuthorizationError {
    /// The compact JWS was malformed.
    #[error("invalid JWS format: {0}")]
    InvalidJws(&'static str),
    /// Base64url decoding failed.
    #[error("failed to decode JWS segment: {0}")]
    Base64(#[from] base64::DecodeError),
    /// JSON parsing failed.
    #[error("invalid JSON payload: {0}")]
    Json(String),
    /// A required field was missing.
    #[error("missing field `{0}`")]
    MissingField(&'static str),
    /// A field was present but of an unexpected shape.
    #[error("invalid field `{field}`: {reason}")]
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Description of the violation.
        reason: &'static str,
    },
    /// JWS `alg` header contained an unsupported value.
    #[error("unsupported JWS algorithm `{0}` (expected EdDSA)")]
    UnsupportedAlgorithm(String),
    /// No matching key was registered for the supplied key identifier.
    #[error("unknown GAR key id `{0}`")]
    UnknownKeyId(String),
    /// Record version not supported by this release.
    #[error("unsupported GAR record version {found}")]
    UnsupportedRecordVersion {
        /// Version found in the payload.
        found: u16,
    },
    /// The registered key used an unexpected algorithm.
    #[error("gateway key `{key_id}` must use Ed25519, found {algorithm:?}")]
    InvalidKeyAlgorithm {
        /// Key identifier.
        key_id: String,
        /// Algorithm encountered.
        algorithm: Algorithm,
    },
    /// Signature length didn't match the Ed25519 encoding.
    #[error("invalid signature length: expected 64 bytes, got {found}")]
    InvalidSignatureLength {
        /// Observed signature length.
        found: usize,
    },
    /// Signature verification failed.
    #[error("signature verification failed")]
    Signature(#[source] iroha_crypto::error::Error),
    /// Record isn't yet valid.
    #[error("record not yet valid until {not_before} (current {now})")]
    NotYetValid {
        /// Start of the validity window.
        not_before: u64,
        /// Current timestamp.
        now: u64,
    },
    /// Record expired.
    #[error("record expired at {expires_at} (current {now})")]
    Expired {
        /// End of the validity window.
        expires_at: u64,
        /// Current timestamp.
        now: u64,
    },
    /// Host pattern list was empty.
    #[error("host pattern list may not be empty")]
    EmptyHostPatterns,
    /// Host pattern failed validation.
    #[error("invalid host pattern `{pattern}`: {reason}")]
    InvalidHostPattern {
        /// Provided pattern.
        pattern: String,
        /// Violation description.
        reason: &'static str,
    },
    /// Manifest digest couldn't be decoded.
    #[error("invalid manifest digest hex: {0}")]
    ManifestDigestHex(#[from] hex::FromHexError),
}

/// Parsed SoraFS Gateway Authorization Record (GAR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayAuthorizationRecord {
    record_version: u16,
    name: String,
    manifest_cid: String,
    manifest_digest: Option<[u8; 32]>,
    host_patterns: Vec<HostPattern>,
    csp_template: Option<String>,
    hsts_template: Option<String>,
    valid_from_epoch: u64,
    valid_until_epoch: Option<u64>,
    key_id: String,
    policy: GarPolicyPayloadV1,
    extensions: Map,
}

impl GatewayAuthorizationRecord {
    /// Returns the record schema version.
    #[must_use]
    pub fn record_version(&self) -> u16 {
        self.record_version
    }

    /// Returns the logical gateway name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the manifest CID the record authorises.
    #[must_use]
    pub fn manifest_cid(&self) -> &str {
        &self.manifest_cid
    }

    /// Returns the optional manifest digest (BLAKE3-256).
    #[must_use]
    pub fn manifest_digest(&self) -> Option<&[u8; 32]> {
        self.manifest_digest.as_ref()
    }

    /// Returns the canonical host patterns covered by this record.
    #[must_use]
    pub fn host_patterns(&self) -> &[HostPattern] {
        &self.host_patterns
    }

    /// Returns the optional Content Security Policy template.
    #[must_use]
    pub fn csp_template(&self) -> Option<&str> {
        self.csp_template.as_deref()
    }

    /// Returns the optional HSTS template.
    #[must_use]
    pub fn hsts_template(&self) -> Option<&str> {
        self.hsts_template.as_deref()
    }

    /// Returns the beginning of the validity window (inclusive, in Unix seconds).
    #[must_use]
    pub fn valid_from_epoch(&self) -> u64 {
        self.valid_from_epoch
    }

    /// Returns the end of the validity window (inclusive, in Unix seconds), if present.
    #[must_use]
    pub fn valid_until_epoch(&self) -> Option<u64> {
        self.valid_until_epoch
    }

    /// Returns the key identifier that signed this record.
    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Returns any additional JSON fields preserved from the payload.
    #[must_use]
    pub fn extensions(&self) -> &Map {
        &self.extensions
    }

    /// Returns the structured GAR v2 policy payload (licensing/moderation/telemetry).
    #[must_use]
    pub fn policy_payload(&self) -> &GarPolicyPayloadV1 {
        &self.policy
    }

    /// Checks whether the record is valid at the supplied Unix timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`GatewayAuthorizationError::NotYetValid`] or
    /// [`GatewayAuthorizationError::Expired`] if the timestamp falls outside the
    /// declared validity window.
    pub fn ensure_applicable_at(&self, now: u64) -> Result<(), GatewayAuthorizationError> {
        if now < self.valid_from_epoch {
            return Err(GatewayAuthorizationError::NotYetValid {
                not_before: self.valid_from_epoch,
                now,
            });
        }
        if let Some(until) = self.valid_until_epoch
            && now > until
        {
            return Err(GatewayAuthorizationError::Expired {
                expires_at: until,
                now,
            });
        }
        Ok(())
    }

    /// Determines whether the record authorises the supplied host.
    #[must_use]
    pub fn matches_host(&self, host: &str) -> bool {
        canonicalise_host(host).is_some_and(|candidate| {
            self.host_patterns
                .iter()
                .any(|pattern| pattern.matches(&candidate))
        })
    }
}

/// Canonical host pattern representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    /// Exact host match.
    Exact(String),
    /// Wildcard suffix (matches any subdomain).
    Wildcard { pattern: String, suffix: String },
}

impl HostPattern {
    /// Returns the canonical pattern string (`*.example.com` or `example.com`).
    #[must_use]
    pub fn pattern(&self) -> &str {
        match self {
            Self::Exact(pattern) => pattern,
            Self::Wildcard { pattern, .. } => pattern,
        }
    }

    /// Performs a host match against the canonicalised host value.
    #[must_use]
    pub fn matches(&self, host: &str) -> bool {
        match self {
            Self::Exact(pattern) => host == pattern,
            Self::Wildcard { suffix, .. } => {
                host.ends_with(suffix)
                    && host.len() > suffix.len()
                    && host.as_bytes()[host.len() - suffix.len() - 1] == b'.'
            }
        }
    }

    fn parse(raw: &str) -> Result<Self, GatewayAuthorizationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(GatewayAuthorizationError::InvalidHostPattern {
                pattern: raw.to_string(),
                reason: "pattern may not be empty",
            });
        }
        if trimmed.starts_with('.') || trimmed.ends_with('.') {
            return Err(GatewayAuthorizationError::InvalidHostPattern {
                pattern: raw.to_string(),
                reason: "leading or trailing dots are not permitted",
            });
        }
        let canonical = trimmed.to_ascii_lowercase();
        if canonical
            .bytes()
            .any(|byte| !matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'*'))
        {
            return Err(GatewayAuthorizationError::InvalidHostPattern {
                pattern: raw.to_string(),
                reason: "contains unsupported characters",
            });
        }
        if canonical.contains("..") {
            return Err(GatewayAuthorizationError::InvalidHostPattern {
                pattern: raw.to_string(),
                reason: "contains consecutive dots",
            });
        }
        if canonical.contains('*') {
            if !canonical.starts_with("*.") {
                return Err(GatewayAuthorizationError::InvalidHostPattern {
                    pattern: raw.to_string(),
                    reason: "wildcards must use the `*.example.com` form",
                });
            }
            let suffix = &canonical[2..];
            if suffix.is_empty() {
                return Err(GatewayAuthorizationError::InvalidHostPattern {
                    pattern: raw.to_string(),
                    reason: "wildcard suffix must not be empty",
                });
            }
            if suffix.contains('*') {
                return Err(GatewayAuthorizationError::InvalidHostPattern {
                    pattern: raw.to_string(),
                    reason: "only a single prefix wildcard is supported",
                });
            }
            if !suffix.contains('.') {
                return Err(GatewayAuthorizationError::InvalidHostPattern {
                    pattern: raw.to_string(),
                    reason: "wildcard suffix must contain at least one dot",
                });
            }
            Ok(Self::Wildcard {
                pattern: format!("*.{suffix}"),
                suffix: suffix.to_string(),
            })
        } else {
            Ok(Self::Exact(canonical))
        }
    }
}

/// Verifier that decodes and validates compact GAR JWS payloads.
#[derive(Debug, Clone, Default)]
pub struct GatewayAuthorizationVerifier {
    keys: HashMap<String, PublicKey>,
}

impl GatewayAuthorizationVerifier {
    /// Insert or replace a public key for the supplied identifier.
    pub fn insert(
        &mut self,
        key_id: impl Into<String>,
        public_key: PublicKey,
    ) -> Option<PublicKey> {
        self.keys.insert(key_id.into(), public_key)
    }

    /// Validate the supplied GAR JWS without checking the validity window.
    pub fn verify(
        &self,
        jws: &str,
    ) -> Result<GatewayAuthorizationRecord, GatewayAuthorizationError> {
        self.verify_internal(jws, None)
    }

    /// Validate the supplied GAR JWS and require that it is valid at `now` (Unix seconds).
    pub fn verify_at(
        &self,
        jws: &str,
        now: u64,
    ) -> Result<GatewayAuthorizationRecord, GatewayAuthorizationError> {
        self.verify_internal(jws, Some(now))
    }

    fn verify_internal(
        &self,
        jws: &str,
        now: Option<u64>,
    ) -> Result<GatewayAuthorizationRecord, GatewayAuthorizationError> {
        let trimmed = jws.trim();
        let mut segments = trimmed.split('.');
        let header_segment = segments
            .next()
            .ok_or(GatewayAuthorizationError::InvalidJws(
                "missing header segment",
            ))?;
        let payload_segment = segments
            .next()
            .ok_or(GatewayAuthorizationError::InvalidJws(
                "missing payload segment",
            ))?;
        let signature_segment = segments
            .next()
            .ok_or(GatewayAuthorizationError::InvalidJws(
                "missing signature segment",
            ))?;
        if segments.next().is_some() {
            return Err(GatewayAuthorizationError::InvalidJws(
                "too many segments in compact JWS",
            ));
        }

        let header_bytes = URL_SAFE_NO_PAD.decode(header_segment)?;
        let payload_bytes = URL_SAFE_NO_PAD.decode(payload_segment)?;
        let signature_bytes = URL_SAFE_NO_PAD.decode(signature_segment)?;

        if signature_bytes.len() != 64 {
            return Err(GatewayAuthorizationError::InvalidSignatureLength {
                found: signature_bytes.len(),
            });
        }

        let signing_input = format!("{header_segment}.{payload_segment}");
        let signature = Signature::from_bytes(&signature_bytes);

        let header_value: Value = norito::json::from_slice(&header_bytes)
            .map_err(|err| GatewayAuthorizationError::Json(format!("header: {err}")))?;
        let mut header_map = match header_value {
            Value::Object(map) => map,
            _ => {
                return Err(GatewayAuthorizationError::InvalidJws(
                    "header must be a JSON object",
                ));
            }
        };
        let alg = take_required_string(&mut header_map, "alg")?;
        if alg != "EdDSA" {
            return Err(GatewayAuthorizationError::UnsupportedAlgorithm(alg));
        }
        if let Some(typ) = take_optional_string(&mut header_map, "typ")?
            && typ != "gar+jws"
        {
            return Err(GatewayAuthorizationError::InvalidField {
                field: "typ",
                reason: "expected `gar+jws`",
            });
        }
        let key_id = take_required_string(&mut header_map, "kid")?
            .trim()
            .to_string();

        let public_key = self
            .keys
            .get(&key_id)
            .ok_or_else(|| GatewayAuthorizationError::UnknownKeyId(key_id.clone()))?;
        if public_key.algorithm() != Algorithm::Ed25519 {
            return Err(GatewayAuthorizationError::InvalidKeyAlgorithm {
                key_id: key_id.clone(),
                algorithm: public_key.algorithm(),
            });
        }
        signature
            .verify(public_key, signing_input.as_bytes())
            .map_err(GatewayAuthorizationError::Signature)?;

        let payload_value: Value = norito::json::from_slice(&payload_bytes)
            .map_err(|err| GatewayAuthorizationError::Json(format!("payload: {err}")))?;
        let mut payload_map = match payload_value {
            Value::Object(map) => map,
            _ => {
                return Err(GatewayAuthorizationError::InvalidJws(
                    "payload must be a JSON object",
                ));
            }
        };

        let record_version_raw = take_required_u64(&mut payload_map, "version")?;
        let record_version = u16::try_from(record_version_raw).map_err(|_| {
            GatewayAuthorizationError::InvalidField {
                field: "version",
                reason: "must fit into u16",
            }
        })?;
        if record_version != GAR_RECORD_VERSION_V1 {
            return Err(GatewayAuthorizationError::UnsupportedRecordVersion {
                found: record_version,
            });
        }
        let name = take_required_string(&mut payload_map, "name")?
            .trim()
            .to_string();
        let manifest_cid = take_required_string(&mut payload_map, "manifest_cid")?
            .trim()
            .to_string();
        if manifest_cid.is_empty() {
            return Err(GatewayAuthorizationError::InvalidField {
                field: "manifest_cid",
                reason: "must not be empty",
            });
        }
        let manifest_digest =
            parse_optional_digest(take_optional_string(&mut payload_map, "manifest_digest")?)?;

        let host_values = take_required_array(&mut payload_map, "host_patterns")?;
        let mut host_patterns = Vec::with_capacity(host_values.len());
        for value in host_values {
            let Value::String(pattern) = value else {
                return Err(GatewayAuthorizationError::InvalidField {
                    field: "host_patterns",
                    reason: "expected an array of strings",
                });
            };
            host_patterns.push(HostPattern::parse(&pattern)?);
        }
        if host_patterns.is_empty() {
            return Err(GatewayAuthorizationError::EmptyHostPatterns);
        }

        let csp_template = take_optional_string(&mut payload_map, "csp_template")?;
        let hsts_template = take_optional_string(&mut payload_map, "hsts_template")?;

        let valid_from_epoch =
            take_optional_u64(&mut payload_map, "valid_from_epoch")?.unwrap_or(0);
        let valid_until_epoch = take_optional_u64(&mut payload_map, "valid_until_epoch")?;
        if let Some(until) = valid_until_epoch
            && until < valid_from_epoch
        {
            return Err(GatewayAuthorizationError::InvalidField {
                field: "valid_until_epoch",
                reason: "must be >= valid_from_epoch",
            });
        }
        let policy_payload = parse_policy_payload(&mut payload_map)?;

        let record = GatewayAuthorizationRecord {
            record_version,
            name,
            manifest_cid,
            manifest_digest,
            host_patterns,
            csp_template,
            hsts_template,
            valid_from_epoch,
            valid_until_epoch,
            key_id,
            policy: policy_payload,
            extensions: payload_map,
        };

        if let Some(now) = now {
            record.ensure_applicable_at(now)?;
        }

        Ok(record)
    }
}

impl std::iter::FromIterator<(String, PublicKey)> for GatewayAuthorizationVerifier {
    fn from_iter<I: IntoIterator<Item = (String, PublicKey)>>(iter: I) -> Self {
        Self {
            keys: iter.into_iter().collect(),
        }
    }
}

fn canonicalise_host(host: &str) -> Option<String> {
    let trimmed = host.trim();
    if trimmed.is_empty() || trimmed.starts_with('.') || trimmed.ends_with('.') {
        return None;
    }
    if trimmed.contains("..") {
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

fn take_required_string(
    map: &mut Map,
    key: &'static str,
) -> Result<String, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::String(value)) => {
            if value.trim().is_empty() {
                Err(GatewayAuthorizationError::InvalidField {
                    field: key,
                    reason: "must not be empty",
                })
            } else {
                Ok(value)
            }
        }
        Some(Value::Null) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected string, found null",
        }),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected string",
        }),
        None => Err(GatewayAuthorizationError::MissingField(key)),
    }
}

fn take_optional_string(
    map: &mut Map,
    key: &'static str,
) -> Result<Option<String>, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::String(value)) => {
            if value.trim().is_empty() {
                Err(GatewayAuthorizationError::InvalidField {
                    field: key,
                    reason: "must not be empty when present",
                })
            } else {
                Ok(Some(value))
            }
        }
        Some(Value::Null) => Ok(None),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected string",
        }),
        None => Ok(None),
    }
}

fn take_required_u64(map: &mut Map, key: &'static str) -> Result<u64, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::Number(number)) => {
            number
                .as_u64()
                .ok_or(GatewayAuthorizationError::InvalidField {
                    field: key,
                    reason: "expected unsigned integer",
                })
        }
        Some(Value::Null) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected unsigned integer, found null",
        }),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected unsigned integer",
        }),
        None => Err(GatewayAuthorizationError::MissingField(key)),
    }
}

fn take_optional_u64(
    map: &mut Map,
    key: &'static str,
) -> Result<Option<u64>, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::Number(number)) => number
            .as_u64()
            .ok_or(GatewayAuthorizationError::InvalidField {
                field: key,
                reason: "expected unsigned integer",
            })
            .map(Some),
        Some(Value::Null) => Ok(None),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected unsigned integer",
        }),
        None => Ok(None),
    }
}

fn take_optional_bool(
    map: &mut Map,
    key: &'static str,
) -> Result<Option<bool>, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::Bool(value)) => Ok(Some(value)),
        Some(Value::Null) => Ok(None),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected boolean",
        }),
        None => Ok(None),
    }
}

fn take_required_array(
    map: &mut Map,
    key: &'static str,
) -> Result<Vec<Value>, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::Array(values)) => Ok(values),
        Some(Value::Null) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected array, found null",
        }),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected array",
        }),
        None => Err(GatewayAuthorizationError::MissingField(key)),
    }
}

fn parse_optional_digest(
    candidate: Option<String>,
) -> Result<Option<[u8; 32]>, GatewayAuthorizationError> {
    candidate
        .map(|hex| <[u8; 32]>::from_hex(hex.trim()))
        .transpose()
        .map_err(GatewayAuthorizationError::ManifestDigestHex)
}

fn parse_policy_payload(map: &mut Map) -> Result<GarPolicyPayloadV1, GatewayAuthorizationError> {
    let license_sets = take_optional_array(map, "license_sets")?
        .unwrap_or_default()
        .into_iter()
        .map(parse_license_set)
        .collect::<Result<Vec<_>, _>>()?;
    let moderation_directives = take_optional_array(map, "moderation_directives")?
        .unwrap_or_default()
        .into_iter()
        .map(parse_moderation_directive)
        .collect::<Result<Vec<_>, _>>()?;
    let metrics_policy = match map.remove("metrics_policy") {
        Some(Value::Object(object)) => Some(parse_metrics_policy(object)?),
        Some(Value::Null) => None,
        Some(_) => {
            return Err(GatewayAuthorizationError::InvalidField {
                field: "metrics_policy",
                reason: "expected object",
            });
        }
        None => None,
    };
    let cdn_policy = match map.remove("cdn_policy") {
        Some(Value::Object(object)) => Some(parse_cdn_policy(object)?),
        Some(Value::Null) => None,
        Some(_) => {
            return Err(GatewayAuthorizationError::InvalidField {
                field: "cdn_policy",
                reason: "expected object",
            });
        }
        None => None,
    };
    let telemetry_labels = take_optional_array(map, "telemetry_labels")?
        .map(|values| collect_string_array(values, "telemetry_labels"))
        .transpose()?
        .unwrap_or_default();
    let rpt_digest = parse_optional_digest(take_optional_string(map, "rpt_digest")?)?;

    Ok(GarPolicyPayloadV1 {
        license_sets,
        moderation_directives,
        cdn_policy,
        metrics_policy,
        telemetry_labels,
        rpt_digest,
    })
}

fn parse_license_set(value: Value) -> Result<GarLicenseSetV1, GatewayAuthorizationError> {
    let mut map = value_to_object(value, "license_sets")?;
    let slug = take_required_string(&mut map, "slug")?;
    let jurisdiction = take_required_string(&mut map, "jurisdiction")?;
    let holder = take_required_string(&mut map, "holder")?;
    let valid_from_unix = take_optional_u64(&mut map, "valid_from_unix")?;
    let valid_until_unix = take_optional_u64(&mut map, "valid_until_unix")?;
    let reference_uri = take_optional_string(&mut map, "reference_uri")?;
    Ok(GarLicenseSetV1 {
        slug,
        jurisdiction,
        holder,
        valid_from_unix,
        valid_until_unix,
        reference_uri,
    })
}

fn parse_moderation_directive(
    value: Value,
) -> Result<GarModerationDirectiveV1, GatewayAuthorizationError> {
    let mut map = value_to_object(value, "moderation_directives")?;
    let slug = take_required_string(&mut map, "slug")?;
    let action_label = take_required_string(&mut map, "action")?;
    let action = parse_moderation_action(&action_label)?;
    let sensitivity_classes = take_optional_array(&mut map, "sensitivity_classes")?
        .map(|values| collect_string_array(values, "sensitivity_classes"))
        .transpose()?
        .unwrap_or_default();
    let notes = take_optional_string(&mut map, "notes")?;
    Ok(GarModerationDirectiveV1 {
        slug,
        action,
        sensitivity_classes,
        notes,
    })
}

fn parse_cdn_policy(mut map: Map) -> Result<GarCdnPolicyV1, GatewayAuthorizationError> {
    let ttl_override_secs = take_optional_u64(&mut map, "ttl_override_secs")?;
    let purge_tags = take_optional_array(&mut map, "purge_tags")?
        .map(|values| collect_string_array(values, "purge_tags"))
        .transpose()?
        .unwrap_or_default();
    let moderation_slugs = take_optional_array(&mut map, "moderation_slugs")?
        .map(|values| collect_string_array(values, "moderation_slugs"))
        .transpose()?
        .unwrap_or_default();
    let rate_ceiling_rps = take_optional_u64(&mut map, "rate_ceiling_rps")?;
    let allow_regions = take_optional_array(&mut map, "allow_regions")?
        .map(|values| collect_string_array(values, "allow_regions"))
        .transpose()?
        .unwrap_or_default();
    let deny_regions = take_optional_array(&mut map, "deny_regions")?
        .map(|values| collect_string_array(values, "deny_regions"))
        .transpose()?
        .unwrap_or_default();
    let legal_hold = take_optional_bool(&mut map, "legal_hold")?.unwrap_or(false);

    Ok(GarCdnPolicyV1 {
        ttl_override_secs,
        purge_tags,
        moderation_slugs,
        rate_ceiling_rps,
        allow_regions,
        deny_regions,
        legal_hold,
    })
}

fn parse_metrics_policy(mut map: Map) -> Result<GarMetricsPolicyV1, GatewayAuthorizationError> {
    let policy_id = take_required_string(&mut map, "policy_id")?;
    let sampling_bps_raw = take_required_u64(&mut map, "sampling_bps")?;
    if sampling_bps_raw > MAX_SAMPLING_BPS {
        return Err(GatewayAuthorizationError::InvalidField {
            field: "sampling_bps",
            reason: "must fit into 0-10_000 basis points",
        });
    }
    let sampling_bps = sampling_bps_raw as u16;
    let retention_secs = take_required_u64(&mut map, "retention_secs")?;
    let allowed_metrics = take_optional_array(&mut map, "allowed_metrics")?
        .map(|values| collect_string_array(values, "allowed_metrics"))
        .transpose()?
        .unwrap_or_default();
    Ok(GarMetricsPolicyV1 {
        policy_id,
        sampling_bps,
        retention_secs,
        allowed_metrics,
    })
}

fn parse_moderation_action(value: &str) -> Result<GarModerationAction, GatewayAuthorizationError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "allow" => Ok(GarModerationAction::Allow),
        "warn" => Ok(GarModerationAction::Warn),
        "quarantine" => Ok(GarModerationAction::Quarantine),
        "block" => Ok(GarModerationAction::Block),
        _ => Err(GatewayAuthorizationError::InvalidField {
            field: "action",
            reason: "unsupported moderation action",
        }),
    }
}

fn collect_string_array(
    values: Vec<Value>,
    field: &'static str,
) -> Result<Vec<String>, GatewayAuthorizationError> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::String(entry) => {
                if entry.trim().is_empty() {
                    return Err(GatewayAuthorizationError::InvalidField {
                        field,
                        reason: "elements must not be empty",
                    });
                }
                out.push(entry.trim().to_string());
            }
            Value::Null => {
                return Err(GatewayAuthorizationError::InvalidField {
                    field,
                    reason: "array may not contain null entries",
                });
            }
            _ => {
                return Err(GatewayAuthorizationError::InvalidField {
                    field,
                    reason: "expected array of strings",
                });
            }
        }
    }
    Ok(out)
}

fn value_to_object(value: Value, field: &'static str) -> Result<Map, GatewayAuthorizationError> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(GatewayAuthorizationError::InvalidField {
            field,
            reason: "expected object",
        }),
    }
}

fn take_optional_array(
    map: &mut Map,
    key: &'static str,
) -> Result<Option<Vec<Value>>, GatewayAuthorizationError> {
    match map.remove(key) {
        Some(Value::Array(values)) => Ok(Some(values)),
        Some(Value::Null) => Ok(None),
        Some(_) => Err(GatewayAuthorizationError::InvalidField {
            field: key,
            reason: "expected array",
        }),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, PublicKey};

    use super::*;

    fn build_test_verifier() -> (GatewayAuthorizationVerifier, SigningKey) {
        let signing_key = SigningKey::from_bytes(&[0x11; 32]);
        let verifying_key = signing_key.verifying_key();
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, verifying_key.as_bytes())
            .expect("valid Ed25519 public key");
        let mut verifier = GatewayAuthorizationVerifier::default();
        verifier.insert("council-key-1", public_key);
        (verifier, signing_key)
    }

    fn build_jws(signing_key: &SigningKey, payload: &Value) -> String {
        let header = norito::json!({
            "alg": "EdDSA",
            "typ": "gar+jws",
            "kid": "council-key-1"
        });
        let header_bytes = norito::json::to_vec(&header).expect("header to serialize");
        let payload_bytes = norito::json::to_vec(payload).expect("payload to serialize");
        let header_segment = URL_SAFE_NO_PAD.encode(header_bytes);
        let payload_segment = URL_SAFE_NO_PAD.encode(payload_bytes);
        let signing_input = format!("{header_segment}.{payload_segment}");
        let signature = signing_key.sign(signing_input.as_bytes());
        let signature_segment = URL_SAFE_NO_PAD.encode(signature.to_bytes());
        format!("{header_segment}.{payload_segment}.{signature_segment}")
    }

    fn base_payload() -> Value {
        norito::json!({
            "version": 1,
            "name": "gw-alpha",
            "manifest_cid": "bafyalpha",
            "manifest_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "host_patterns": [
                "alpha.gw.sora.id",
                "*.gw.sora.id"
            ],
            "csp_template": "default-src 'self';",
            "hsts_template": "max-age=31536000;",
            "valid_from_epoch": 1_700_000_000u64,
            "valid_until_epoch": 1_800_000_000u64,
            "license_sets": [{
                "slug": "sg-2026",
                "jurisdiction": "SG",
                "holder": "Example Media LLP",
                "valid_until_unix": 1_850_000_000u64,
                "reference_uri": "https://example.invalid/license"
            }],
            "moderation_directives": [{
                "slug": "global-block",
                "action": "block",
                "sensitivity_classes": ["extreme"],
                "notes": "Auto-block prohibited content"
            }],
            "metrics_policy": {
                "policy_id": "metrics-g1",
                "sampling_bps": 250,
                "retention_secs": 86_400u64,
                "allowed_metrics": ["audience", "rebuffer"]
            },
            "cdn_policy": {
                "ttl_override_secs": 60u64,
                "purge_tags": ["hotfix"],
                "moderation_slugs": ["global-block"],
                "rate_ceiling_rps": 50u64,
                "allow_regions": ["EU"],
                "deny_regions": ["US"],
                "legal_hold": false
            },
            "telemetry_labels": ["pilot", "taikai"],
            "rpt_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        })
    }

    #[test]
    fn verify_success() {
        let (verifier, signing_key) = build_test_verifier();
        let payload = base_payload();
        let jws = build_jws(&signing_key, &payload);

        let record = verifier
            .verify_at(&jws, 1_750_000_000)
            .expect("record should verify");

        assert_eq!(record.record_version(), 1);
        assert_eq!(record.name(), "gw-alpha");
        assert_eq!(record.manifest_cid(), "bafyalpha");
        assert_eq!(record.key_id(), "council-key-1");
        assert!(record.matches_host("alpha.gw.sora.id"));
        assert!(record.matches_host("beta.gw.sora.id"));
        assert!(!record.matches_host("gw-alpha.internal"));
        let policy = record.policy_payload();
        assert_eq!(policy.license_sets.len(), 1);
        assert_eq!(policy.license_sets[0].slug, "sg-2026");
        assert_eq!(
            policy.metrics_policy.as_ref().unwrap().policy_id,
            "metrics-g1"
        );
        assert_eq!(
            policy.telemetry_labels,
            ["pilot".to_string(), "taikai".to_string()]
        );
        assert_eq!(policy.rpt_digest, Some([0xbb; 32]));
        let cdn = policy.cdn_policy.as_ref().expect("cdn policy present");
        assert_eq!(cdn.ttl_override_secs, Some(60));
        assert_eq!(cdn.purge_tags, ["hotfix".to_string()]);
        assert_eq!(cdn.moderation_slugs, ["global-block".to_string()]);
        assert_eq!(cdn.rate_ceiling_rps, Some(50));
        assert_eq!(cdn.allow_regions, ["EU".to_string()]);
        assert_eq!(cdn.deny_regions, ["US".to_string()]);
        assert!(!cdn.legal_hold);
    }

    #[test]
    fn verify_rejects_tampered_signature() {
        let (verifier, signing_key) = build_test_verifier();
        let payload = base_payload();
        let mut jws = build_jws(&signing_key, &payload).into_bytes();
        // Flip the last byte in the signature segment.
        if let Some(last) = jws.last_mut() {
            *last = if *last == b'A' { b'B' } else { b'A' };
        }
        let jws = String::from_utf8(jws).expect("valid UTF-8");
        let err = verifier.verify(&jws).expect_err("signature must fail");
        assert!(matches!(err, GatewayAuthorizationError::Signature(_)));
    }

    #[test]
    fn rejects_unsupported_record_version() {
        let (verifier, signing_key) = build_test_verifier();
        let mut payload = base_payload();
        if let Some(map) = payload.as_object_mut() {
            map.insert("version".to_string(), Value::from(2u64));
        }
        let jws = build_jws(&signing_key, &payload);
        let err = verifier
            .verify(&jws)
            .expect_err("unsupported version should fail");
        assert!(matches!(
            err,
            GatewayAuthorizationError::UnsupportedRecordVersion { found: 2 }
        ));
    }

    #[test]
    fn rejects_record_version_overflow() {
        let (verifier, signing_key) = build_test_verifier();
        let mut payload = base_payload();
        if let Some(map) = payload.as_object_mut() {
            map.insert("version".to_string(), Value::from(70_000u64));
        }
        let jws = build_jws(&signing_key, &payload);
        let err = verifier
            .verify(&jws)
            .expect_err("version overflow should fail");
        assert!(matches!(
            err,
            GatewayAuthorizationError::InvalidField {
                field: "version",
                ..
            }
        ));
    }

    #[test]
    fn verify_rejects_unknown_key() {
        let (_, signing_key) = build_test_verifier();
        let verifier = GatewayAuthorizationVerifier::default();
        let payload = base_payload();
        let jws = build_jws(&signing_key, &payload);
        let err = verifier.verify(&jws).expect_err("unknown key");
        assert!(matches!(err, GatewayAuthorizationError::UnknownKeyId(_)));
    }

    #[test]
    fn applies_time_window() {
        let (verifier, signing_key) = build_test_verifier();
        let payload = base_payload();
        let jws = build_jws(&signing_key, &payload);
        let err = verifier
            .verify_at(&jws, 1_650_000_000)
            .expect_err("too early");
        assert!(matches!(
            err,
            GatewayAuthorizationError::NotYetValid {
                not_before: 1_700_000_000,
                ..
            }
        ));
        let err = verifier
            .verify_at(&jws, 1_900_000_000)
            .expect_err("expired");
        assert!(matches!(
            err,
            GatewayAuthorizationError::Expired {
                expires_at: 1_800_000_000,
                ..
            }
        ));
    }

    #[test]
    fn rejects_invalid_validity_window() {
        let (verifier, signing_key) = build_test_verifier();
        let mut payload = base_payload();
        if let Some(map) = payload.as_object_mut() {
            map.insert(
                "valid_from_epoch".to_string(),
                Value::from(1_800_000_000u64),
            );
            map.insert(
                "valid_until_epoch".to_string(),
                Value::from(1_700_000_000u64),
            );
        }
        let jws = build_jws(&signing_key, &payload);
        let err = verifier
            .verify(&jws)
            .expect_err("invalid validity window should fail");
        assert!(matches!(
            err,
            GatewayAuthorizationError::InvalidField {
                field: "valid_until_epoch",
                ..
            }
        ));
    }

    #[test]
    fn rejects_invalid_pattern() {
        assert!(matches!(
            HostPattern::parse("invalid*suffix"),
            Err(GatewayAuthorizationError::InvalidHostPattern { .. })
        ));
    }

    #[test]
    fn rejects_unknown_moderation_action() {
        let (verifier, signing_key) = build_test_verifier();
        let mut payload = base_payload();
        if let Some(Value::Array(directives)) = payload
            .as_object_mut()
            .and_then(|map| map.get_mut("moderation_directives"))
            && let Some(Value::Object(first)) = directives.first_mut()
        {
            first.insert("action".to_string(), Value::String("unknown".into()));
        }
        let jws = build_jws(&signing_key, &payload);
        let err = verifier.verify(&jws).expect_err("invalid action must fail");
        assert!(matches!(
            err,
            GatewayAuthorizationError::InvalidField {
                field: "action",
                ..
            }
        ));
    }

    #[test]
    fn rejects_metrics_policy_overflow() {
        let (verifier, signing_key) = build_test_verifier();
        let mut payload = base_payload();
        if let Some(policy_value) = payload
            .as_object_mut()
            .and_then(|map| map.get_mut("metrics_policy"))
        {
            *policy_value = norito::json!({
                "policy_id": "metrics-g1",
                "sampling_bps": 65_000u64,
                "retention_secs": 86_400u64,
                "allowed_metrics": ["audience"]
            });
        }
        let jws = build_jws(&signing_key, &payload);
        let err = verifier
            .verify(&jws)
            .expect_err("sampling overflow must fail");
        assert!(matches!(
            err,
            GatewayAuthorizationError::InvalidField {
                field: "sampling_bps",
                ..
            }
        ));
    }

    #[test]
    fn host_matching_rejects_consecutive_dots() {
        let (verifier, signing_key) = build_test_verifier();
        let payload = base_payload();
        let jws = build_jws(&signing_key, &payload);
        let record = verifier.verify(&jws).expect("record should verify");
        assert!(
            !record.matches_host("alpha..gw.sora.id"),
            "consecutive dots should be rejected"
        );
    }
}
