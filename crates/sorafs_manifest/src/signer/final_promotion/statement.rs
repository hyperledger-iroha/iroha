//! Admission of the sole canonical final-promotion statement before key-provider I/O.
//!
//! This authenticates no evidence. The statement records administrator assertions about the
//! separately reviewed replay, negative archive, toolchain and cosign/OIDC inputs. Their complete
//! verification, public-URL policy, freshness and independent trust remain with the promotion
//! checker. Signer authorization and completed-operation evidence remain separate receipt inputs.

use super::{
    SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1, SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1,
};
use crate::signer::{
    custody::{SignerCustodyBindingV1, validate_binding},
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1, digest_canonical},
};
use iroha_crypto::sha256;
use norito::json::{Map, Value};
use std::fmt;

const ROOT_FIELDS: &[&str] = &[
    "schema",
    "status",
    "attestation_scope",
    "generated_at_unix",
    "chain_id",
    "network_id_hex",
    "deployment_id",
    "signing_provider",
    "baseline_input_count",
    "baseline_input_set_sha256",
    "negative_archive_manifest_sha256",
    "negative_receipts",
    "aggregate_runner_sha256",
    "aggregate_checker_sha256",
    "aggregate_toolchain_sha256",
    "python_runtime",
    "positive_output_sha256",
    "cosign_bundle_sha256",
    "provenance_certificate_identity",
    "provenance_oidc_issuer",
    "oidc_identity_status",
    "cosign_provenance_status",
    "authentication",
    "errors",
];
const AUTH_FIELDS: &[&str] = &[
    "kind",
    "algorithm",
    "service_id",
    "administrator_id",
    "key_revision",
    "policy_revision",
    "policy_digest_sha256",
    "public_key_fingerprint_sha256",
];
const POSITIVE_FIELDS: &[&str] = &[
    "first_aggregate_sha256",
    "second_aggregate_sha256",
    "aggregate_semantic_sha256",
    "replay_manifest_sha256",
];
const NEGATIVE_CASES: &[(&str, &str)] = &[
    (
        "tampered-lane-summary-bytes",
        "01-tampered-lane-summary-bytes.json",
    ),
    ("stale-explicit-clock", "02-stale-explicit-clock.json"),
    ("missing-lane-summary", "03-missing-lane-summary.json"),
    ("duplicate-lane-summary", "04-duplicate-lane-summary.json"),
    (
        "predecessor-expectation-mismatch",
        "05-predecessor-expectation-mismatch.json",
    ),
    (
        "foundational-signature-forgery",
        "06-foundational-signature-forgery.json",
    ),
];
const MAX_TEXT_BYTES: usize = 4_096;
// No caller count is trusted: this conservative
// finite ceiling also admits malformed controls far enough to reject their exact shape.
const MAX_ELEMENTS: usize = 128;

/// Fixed, payload-free failures at the final-promotion statement boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalPromotionStatementErrorV1 {
    /// Invalid or different independently supplied role, purpose or public custody binding.
    InvalidBinding,
    /// The statement's chain, network, deployment or signer identity differs from custody.
    BindingMismatch,
    /// Malformed JSON, a different domain, or a noncanonical spelling.
    InvalidEncoding,
    /// The exact first-release statement schema or an admitted field value differs.
    InvalidSchema,
    /// A raw, structural, string or allocation limit was exceeded.
    ResourceLimit,
}
impl fmt::Display for FinalPromotionStatementErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::InvalidBinding => "invalid final promotion custody binding",
            Self::BindingMismatch => "final promotion statement differs from custody binding",
            Self::InvalidEncoding => "invalid canonical final promotion statement encoding",
            Self::InvalidSchema => "invalid final promotion statement schema",
            Self::ResourceLimit => "final promotion statement exceeds resource limits",
        })
    }
}
impl std::error::Error for FinalPromotionStatementErrorV1 {}

/// Immutable borrowed statement admitted for one exact independently supplied custody binding.
///
/// This is a syntactic and binding witness, never signer custody, finality or production qualification.
/// It cannot be decoded, defaulted or constructed from caller-supplied digests.
pub struct PreparedFinalPromotionStatementV1<'a> {
    message: &'a [u8],
    sha256: [u8; 32],
    binding_digest: [u8; 32],
}
impl fmt::Debug for PreparedFinalPromotionStatementV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("PreparedFinalPromotionStatementV1")
            .field("size", &self.len())
            .finish_non_exhaustive()
    }
}
impl<'a> PreparedFinalPromotionStatementV1<'a> {
    /// Exact domain-prefixed bytes, borrowed without reserialization.
    #[must_use]
    pub fn message(&self) -> &'a [u8] {
        self.message
    }
    /// SHA-256 of the entire exact domain-prefixed statement.
    #[must_use]
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    /// Exact domain-prefixed byte count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.message.len()
    }
    /// Whether the admitted message is empty; successful preparation always returns false.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.message.is_empty()
    }
    /// Canonical commitment to every leaf of the independently supplied custody binding.
    #[must_use]
    pub fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }
}

/// Prepare exactly one canonical final-promotion signing statement.
///
/// The JSON is closed, sorted, compact and ASCII-escaped exactly like Python `sort_keys=True`,
/// `separators=(",", ":")`, `ensure_ascii=True`, `allow_nan=False`. Detached signatures and
/// custody/operation evidence cannot appear in this preimage. This checks no process clock and
/// fetches no URLs or artifacts; the independently pinned checker supplies those authorities.
///
/// # Errors
/// Rejects malformed/oversized/noncanonical input, other roles/profiles, different independently
/// supplied bindings, unknown/duplicate fields, floats, booleans in integer fields, or malformed
/// exact replay/archive/toolchain digests and inventories.
pub fn prepare_final_promotion_statement_v1<'a>(
    message: &'a [u8],
    binding: &SignerCustodyBindingV1,
) -> Result<PreparedFinalPromotionStatementV1<'a>, FinalPromotionStatementErrorV1> {
    use FinalPromotionStatementErrorV1 as Error;
    if message.len() > SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 {
        return Err(Error::ResourceLimit);
    }
    validate_binding(binding).map_err(|_| Error::InvalidBinding)?;
    if binding.role != SignerRoleV1::FinalPromotionProvenance
        || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
    {
        return Err(Error::InvalidBinding);
    }
    let SignerPurposeBindingV1::FinalPromotionProvenance { deployment_id } = &binding.purpose
    else {
        return Err(Error::InvalidBinding);
    };
    let json = message
        .strip_prefix(SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1)
        .filter(|json| !json.is_empty() && json.is_ascii())
        .ok_or(Error::InvalidEncoding)?;
    let limits = norito::DecodeLimits::new(32, MAX_TEXT_BYTES, MAX_ELEMENTS, 512 * 1024, 4);
    norito::json::preflight_slice(
        json,
        norito::json::JsonPreflightLimits::from_decode_limits(
            SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1,
            limits,
        ),
    )
    .map_err(|error| {
        if error.resource_kind().is_some() {
            Error::ResourceLimit
        } else {
            Error::InvalidEncoding
        }
    })?;
    let value: Value = norito::with_decode_limits_scope(limits, || norito::json::from_slice(json))
        .map_err(|error| {
            if error.is_decode_resource_limit() {
                Error::ResourceLimit
            } else {
                Error::InvalidEncoding
            }
        })?;
    validate_statement(&value, binding, deployment_id)?;
    if canonical_ascii(&value, json.len())?.as_bytes() != json {
        return Err(Error::InvalidEncoding);
    }
    Ok(PreparedFinalPromotionStatementV1 {
        message,
        sha256: sha256(message),
        binding_digest: digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", binding)
            .map_err(|_| Error::InvalidBinding)?,
    })
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map, FinalPromotionStatementErrorV1> {
    let object = value
        .as_object()
        .ok_or(FinalPromotionStatementErrorV1::InvalidSchema)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(FinalPromotionStatementErrorV1::InvalidSchema);
    }
    Ok(object)
}
fn text(value: &Value) -> Result<&str, FinalPromotionStatementErrorV1> {
    value
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= MAX_TEXT_BYTES
                && s.trim() == *s
                && !s.chars().any(char::is_control)
        })
        .ok_or(FinalPromotionStatementErrorV1::InvalidSchema)
}
fn unsigned(value: &Value) -> Result<u64, FinalPromotionStatementErrorV1> {
    value
        .as_u64()
        .ok_or(FinalPromotionStatementErrorV1::InvalidSchema)
}
fn digest(value: &Value) -> Result<[u8; 32], FinalPromotionStatementErrorV1> {
    let hex = text(value)?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(FinalPromotionStatementErrorV1::InvalidSchema);
    }
    let mut out = [0; 32];
    hex::decode_to_slice(hex, &mut out)
        .map_err(|_| FinalPromotionStatementErrorV1::InvalidSchema)?;
    if out == [0; 32] {
        return Err(FinalPromotionStatementErrorV1::InvalidSchema);
    }
    Ok(out)
}
fn validate_statement(
    value: &Value,
    binding: &SignerCustodyBindingV1,
    deployment_id: &str,
) -> Result<(), FinalPromotionStatementErrorV1> {
    use FinalPromotionStatementErrorV1 as Error;
    let root = exact_object(value, ROOT_FIELDS)?;
    for (field, expected) in [
        (
            "schema",
            "sorafs.production_readiness.production_promotion_provenance.v1",
        ),
        ("status", "verified"),
        ("attestation_scope", "production-promotion-bundle"),
        ("signing_provider", "authenticated_external_signer"),
        ("oidc_identity_status", "verified"),
        ("cosign_provenance_status", "verified"),
    ] {
        if text(&root[field])? != expected {
            return Err(Error::InvalidSchema);
        }
    }
    if !matches!(&root["errors"], Value::Array(errors) if errors.is_empty())
        || unsigned(&root["baseline_input_count"])? != 22
        || !(1..=i64::MAX as u64).contains(&unsigned(&root["generated_at_unix"])?)
    {
        return Err(Error::InvalidSchema);
    }
    if text(&root["chain_id"])? != binding.chain_id
        || digest(&root["network_id_hex"])? != binding.network_id
        || text(&root["deployment_id"])? != deployment_id
    {
        return Err(Error::BindingMismatch);
    }
    for field in [
        "baseline_input_set_sha256",
        "negative_archive_manifest_sha256",
        "aggregate_runner_sha256",
        "aggregate_checker_sha256",
        "aggregate_toolchain_sha256",
        "cosign_bundle_sha256",
    ] {
        digest(&root[field])?;
    }
    let positive = exact_object(&root["positive_output_sha256"], POSITIVE_FIELDS)?;
    for hash in positive.values() {
        digest(hash)?;
    }
    let runtime = exact_object(
        &root["python_runtime"],
        &["implementation", "version", "executable_sha256"],
    )?;
    text(&runtime["implementation"])?;
    text(&runtime["version"])?;
    digest(&runtime["executable_sha256"])?;
    let negatives = root["negative_receipts"]
        .as_array()
        .ok_or(Error::InvalidSchema)?;
    if negatives.len() != NEGATIVE_CASES.len() {
        return Err(Error::InvalidSchema);
    }
    for (row, (id, filename)) in negatives.iter().zip(NEGATIVE_CASES) {
        let row = exact_object(row, &["mutation_id", "receipt_file", "sha256"])?;
        if text(&row["mutation_id"])? != *id || text(&row["receipt_file"])? != *filename {
            return Err(Error::InvalidSchema);
        }
        digest(&row["sha256"])?;
    }
    for field in ["provenance_certificate_identity", "provenance_oidc_issuer"] {
        if !bounded_https_syntax(text(&root[field])?) {
            return Err(Error::InvalidSchema);
        }
    }
    let auth = exact_object(&root["authentication"], AUTH_FIELDS)?;
    for (field, expected) in [("kind", "external-ed25519"), ("algorithm", "ed25519")] {
        if text(&auth[field])? != expected {
            return Err(Error::InvalidSchema);
        }
    }
    if text(&auth["service_id"])? != binding.service_id
        || text(&auth["administrator_id"])? != binding.administrator_id
        || unsigned(&auth["key_revision"])? != binding.key_revision
        || unsigned(&auth["policy_revision"])? != binding.policy_revision
        || digest(&auth["policy_digest_sha256"])? != binding.policy_digest
        || digest(&auth["public_key_fingerprint_sha256"])?
            != sha256(binding.public_key.to_bytes().1)
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

// Only bounded HTTPS syntax is owned here. Public address classification, sensitive path policy,
// exact independent identity trust and cryptographic cosign verification are not inferred here.
fn bounded_https_syntax(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if !url.is_ascii()
        || url.len() > MAX_TEXT_BYTES
        || url
            .bytes()
            .any(|b| b <= b' ' || b >= 0x7f || matches!(b, b'\\' | b'?' | b'#' | b'@'))
    {
        return false;
    }
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut path_bytes = path.bytes();
    while let Some(byte) = path_bytes.next() {
        if byte == b'%' {
            if !path_bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                || !path_bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
            {
                return false;
            }
        } else if !byte.is_ascii_alphanumeric()
            && !matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'/'
            )
        {
            return false;
        }
    }
    if authority.is_empty() || authority.bytes().any(|b| b.is_ascii_uppercase()) {
        return false;
    }
    if let Some(address) = authority
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
    {
        return address.parse::<std::net::Ipv6Addr>().is_ok();
    }
    authority.len() <= 253
        && authority.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

// Norito owns bounded parsing and serialization. Convert only actual non-ASCII scalar values in
// its compact sorted JSON to Python's lowercase UTF-16 escapes; existing JSON escapes stay exact.
fn canonical_ascii(
    value: &Value,
    maximum: usize,
) -> Result<String, FinalPromotionStatementErrorV1> {
    use FinalPromotionStatementErrorV1 as Error;
    let compact =
        norito::json::to_json_bounded(value, maximum).map_err(|_| Error::ResourceLimit)?;
    let mut ascii = String::new();
    ascii
        .try_reserve_exact(maximum)
        .map_err(|_| Error::ResourceLimit)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for ch in compact.chars() {
        if ch < '\u{7f}' {
            if ascii.len() == maximum {
                return Err(Error::ResourceLimit);
            }
            ascii.push(ch);
        } else {
            let mut units = [0; 2];
            for &mut unit in ch.encode_utf16(&mut units) {
                if maximum.saturating_sub(ascii.len()) < 6 {
                    return Err(Error::ResourceLimit);
                }
                ascii.push_str("\\u");
                for shift in [12, 8, 4, 0] {
                    ascii.push(char::from(HEX[usize::from((unit >> shift) & 15)]));
                }
            }
        }
    }
    Ok(ascii)
}

#[cfg(test)]
#[path = "statement_tests.rs"]
mod tests;
