//! Exact native boundary for the private E1 verifier process.
//!
//! This codec owns no issuer key and grants no platform or policy authority. The issuer must
//! authenticate its worker/runtime/configuration, retain the original request before dispatch,
//! enforce challenge single use and recover uncertain attempts without verifying them again.
//! The node journal retains these original requests and checked replies before signing.
//! TODO: connect the node service's approved policy, private process custody and signer.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model::kagemusha::*;
use norito::json::{self, Value};
use sha2::{Digest as _, Sha256};

use super::{IssuerEvidenceV1, PlatformEvidenceV1, RequestV1};

#[path = "issuer_worker_configuration.rs"]
mod configuration;
pub use configuration::{
    GoogleDecoderOriginalV1, VerifierConfigurationV1, VerifierRuntimeSelectionV1,
};

const SCHEMA: &str = "iroha.kagemusha.wallet-e1-verifier.v1";
const MAX_REQUEST: usize = 320 * 1024;
const MAX_RESULT: usize = 360 * 1024;
/// Maximum private JSON packet, excluding its four-byte little-endian length.
pub const MAX_PACKET: usize = 768 * 1024;

/// Refused private frame or binding. Diagnostics never contain original platform evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("enrollment verifier boundary: {0}")]
pub struct Error(&'static str);

/// One retained private verifier request. Its timestamps are supplied by the issuer journal,
/// never by a mobile request. Construction checks consistency, not operator approval or KYC.
pub struct VerifierRequestV1 {
    request: RequestV1,
    original: Vec<u8>,
    configuration: [u8; 32],
    verification_time_ms: u64,
}

/// Action selected by the durable issuer owner. An uncertain verification must only recover.
#[derive(Debug, Clone, Copy)]
pub enum ActionV1 {
    /// Exactly one dispatch after durable attempt selection.
    Verify,
    /// Read the retained result for exactly the same original request.
    Recover,
}

/// Separate outcome classes; none is a credential, approval, or permission to repeat verification.
#[derive(Debug)]
pub enum OutcomeV1 {
    /// The private worker retained this projection and its original evidence.
    Evidence(EvidenceProjectionV1),
    /// An attempted verification has no recoverable result; keep the attempt consumed.
    OutcomeUnknown,
    /// The worker could not finish; recovery remains distinct from a new verification.
    Unavailable,
    /// The worker rejected the request or evidence; this is no narrower attestation verdict.
    Rejected,
}

/// Checked consistency of the worker's exact result. Only an authenticated worker channel
/// establishes its provenance; this value alone grants no authority to sign a credential.
#[derive(Debug)]
pub struct EvidenceProjectionV1 {
    /// Evidence kind returned by the configured platform verifier.
    pub kind: KagemushaWalletEvidenceKindV1,
    /// Model evidence with a digest independently derived from exact originals.
    pub evidence: KagemushaWalletEvidenceV1,
    /// Original platform objects, including issuer-acquired Google response bytes on Android.
    pub originals: IssuerEvidenceV1,
    /// Apple's retained key/counter pair; absent on Android.
    pub apple_counter: Option<([u8; 32], u32)>,
    /// Exact private result for the issuer's durable original archive.
    pub original_result: Vec<u8>,
}

fn encode(value: &Value, maximum: usize) -> Result<Vec<u8>, Error> {
    let bytes = json::to_vec(value).map_err(|_| Error("JSON encoding"))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error("frame bound"));
    }
    Ok(bytes)
}
fn decode(bytes: &[u8], maximum: usize) -> Result<Value, Error> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error("frame bound"));
    }
    norito::with_decode_limits_scope(
        norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 8),
        || json::from_slice(bytes),
    )
    .map_err(|_| Error("JSON decoding"))
}
fn fields(value: &Value, expected: &[&str]) -> Result<(), Error> {
    let object = value.as_object().ok_or(Error("JSON object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(Error("JSON fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error("text field"))
}
fn integer(value: &Value, key: &str) -> Result<u64, Error> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(Error("integer field"))
}
fn digest(value: &Value, key: &str) -> Result<[u8; 32], Error> {
    let value = text(value, key)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error("digest spelling"));
    }
    let mut output = [0; 32];
    hex::decode_to_slice(value, &mut output).map_err(|_| Error("digest encoding"))?;
    Ok(output)
}
fn binary(value: &str, maximum: usize) -> Result<Vec<u8>, Error> {
    if value.is_empty() || value.len() > maximum.div_ceil(3) * 4 {
        return Err(Error("binary bound"));
    }
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| Error("binary encoding"))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error("binary bound"));
    }
    if STANDARD.encode(&bytes) != value {
        return Err(Error("binary spelling"));
    }
    Ok(bytes)
}

impl VerifierRequestV1 {
    /// Build from exact issuer-retained E5, approved policy originals and genuine journal time.
    /// The caller must establish authentication, current account eligibility and exclusive
    /// challenge ownership before dispatch. The resulting bytes must be retained unchanged.
    pub fn from_retained(
        request: RequestV1,
        approved_app: &KagemushaWalletAppPolicyV1,
        approved_policy: &KagemushaWalletEnrollmentPolicyV1,
        challenge_created_at_ms: u64,
        verification_time_ms: u64,
        configuration: [u8; 32],
    ) -> Result<Self, Error> {
        request.validate().map_err(Error)?;
        if request.body.app != *approved_app
            || request.body.policy != *approved_policy
            || configuration == [0; 32]
        {
            return Err(Error("approved configuration binding"));
        }
        approved_policy
            .require_live_challenge(challenge_created_at_ms, verification_time_ms)
            .map_err(|_| Error("retained challenge time"))?;
        let expires = challenge_created_at_ms
            .checked_add(approved_policy.challenge_lifetime_ms)
            .ok_or(Error("challenge expiry overflow"))?;
        let mobile =
            PlatformEvidenceV1::decode(&request.body.evidence, approved_policy).map_err(Error)?;
        let (platform, evidence) = match mobile {
            PlatformEvidenceV1::Android {
                certificates,
                play_integrity_token,
            } => {
                let token = std::str::from_utf8(&play_integrity_token)
                    .map_err(|_| Error("opaque token UTF-8"))?;
                (
                    "android",
                    norito::json!({
                        "chain_base64": (certificates.iter().map(|v| STANDARD.encode(v)).collect::<Vec<_>>()),
                        "play_integrity_token": (token),
                    }),
                )
            }
            PlatformEvidenceV1::Apple {
                key_id,
                attestation,
                key_binding_assertion,
            } => (
                "apple",
                norito::json!({
                    "key_id_hex": (hex::encode(key_id)),
                    "attestation_base64": (STANDARD.encode(attestation)),
                    "assertion_base64": (STANDARD.encode(key_binding_assertion)),
                }),
            ),
        };
        let original = encode(
            &norito::json!({
                "challenge_transcript_base64": (STANDARD.encode(request.body.challenge.transcript())),
                "payment_key_base64": (STANDARD.encode(request.body.marker.payment_key.as_sec1_bytes())),
                "issued_at_ms": (challenge_created_at_ms),
                "expires_at_ms": (expires),
                "trusted_time_ms": (verification_time_ms),
                "platform": (platform),
                "evidence": (evidence),
            }),
            MAX_REQUEST,
        )?;
        Ok(Self {
            request,
            original,
            configuration,
            verification_time_ms,
        })
    }

    /// The exact request to archive before dispatch and reuse for every recovery.
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// One framed private exchange. The process owner supplies a fresh nonzero random exchange
    /// identity; it must not reuse it within a worker session, including during recovery.
    pub fn packet(&self, action: ActionV1, exchange: [u8; 32]) -> Result<Vec<u8>, Error> {
        if exchange == [0; 32] {
            return Err(Error("empty exchange identity"));
        }
        let action = match action {
            ActionV1::Verify => "verify",
            ActionV1::Recover => "recover",
        };
        let bytes = encode(
            &norito::json!({
                "schema": (SCHEMA), "version": (1_u16), "exchange_id": (hex::encode(exchange)),
                "action": (action), "original_base64": (STANDARD.encode(&self.original)),
            }),
            MAX_PACKET,
        )?;
        let mut framed = Vec::with_capacity(bytes.len() + 4);
        framed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        framed.extend_from_slice(&bytes);
        Ok(framed)
    }

    /// Decode one complete length-framed response from the authenticated private worker.
    /// Every identity, original, kind, fact and recorded time is checked before projection.
    pub fn response(&self, exchange: [u8; 32], frame: &[u8]) -> Result<OutcomeV1, Error> {
        if exchange == [0; 32] || frame.len() < 4 || frame.len() > MAX_PACKET + 4 {
            return Err(Error("response frame bound"));
        }
        let length =
            u32::from_le_bytes(frame[..4].try_into().map_err(|_| Error("frame prefix"))?) as usize;
        if length != frame.len() - 4 {
            return Err(Error("response frame length"));
        }
        let value = decode(&frame[4..], MAX_PACKET)?;
        fields(
            &value,
            &[
                "schema",
                "version",
                "exchange_id",
                "original_sha256",
                "outcome",
                "evidence_base64",
            ],
        )?;
        if text(&value, "schema")? != SCHEMA
            || integer(&value, "version")? != 1
            || digest(&value, "exchange_id")? != exchange
            || digest(&value, "original_sha256")?
                != <[u8; 32]>::from(Sha256::digest(&self.original))
        {
            return Err(Error("response identity"));
        }
        if text(&value, "outcome")? == "evidence" {
            let original = binary(text(&value, "evidence_base64")?, MAX_RESULT)?;
            return self.projection(original).map(OutcomeV1::Evidence);
        }
        if value.get("evidence_base64") != Some(&Value::Null) {
            return Err(Error("failure evidence"));
        }
        match text(&value, "outcome")? {
            "outcome_unknown" => Ok(OutcomeV1::OutcomeUnknown),
            "unavailable" => Ok(OutcomeV1::Unavailable),
            "rejected" => Ok(OutcomeV1::Rejected),
            _ => Err(Error("unknown outcome")),
        }
    }

    /// Recheck an exact evidence original retained by the issuer after an authenticated reply.
    /// This proves the same request/configuration/time bindings as `response`; it does not
    /// authenticate arbitrary caller-supplied evidence or grant credential-signing authority.
    pub fn retained_evidence(&self, original: &[u8]) -> Result<EvidenceProjectionV1, Error> {
        if original.is_empty() || original.len() > MAX_RESULT {
            return Err(Error("frame bound"));
        }
        self.projection(original.to_vec())
    }

    fn projection(&self, original_result: Vec<u8>) -> Result<EvidenceProjectionV1, Error> {
        let value = decode(&original_result, MAX_RESULT)?;
        fields(
            &value,
            &[
                "config_sha256",
                "challenge_digest",
                "key_binding",
                "payment_key_base64",
                "kind_tag",
                "time_ms",
                "facts",
                "os_patch_level",
                "vendor_patch_level",
                "boot_patch_level",
                "evidence_digest",
                "original_items_base64",
                "app_attest_key_id",
                "app_attest_counter",
            ],
        )?;
        let body = &self.request.body;
        if digest(&value, "config_sha256")? != self.configuration
            || digest(&value, "challenge_digest")? != body.challenge.challenge_digest()
            || digest(&value, "key_binding")?
                != kagemusha_wallet_enrollment_key_binding_v1(
                    &body.challenge.challenge_digest(),
                    &body.marker.payment_key,
                )
            || binary(text(&value, "payment_key_base64")?, 65)?
                != body.marker.payment_key.as_sec1_bytes()
            || integer(&value, "time_ms")? != self.verification_time_ms
        {
            return Err(Error("evidence identity"));
        }
        let kind = match integer(&value, "kind_tag")? {
            1 => KagemushaWalletEvidenceKindV1::AndroidKeyMintTee,
            2 => KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox,
            3 => KagemushaWalletEvidenceKindV1::AppleAppAttest,
            _ => return Err(Error("evidence kind")),
        };
        let encoded = value
            .get("original_items_base64")
            .and_then(Value::as_array)
            .ok_or(Error("evidence items"))?;
        if !(2..=9).contains(&encoded.len()) {
            return Err(Error("evidence item count"));
        }
        let mut items = encoded
            .iter()
            .enumerate()
            .map(|(index, v)| {
                let maximum = if kind.is_android() {
                    if index + 1 == encoded.len() {
                        128 * 1024
                    } else {
                        16 * 1024
                    }
                } else if index == 0 {
                    65_536
                } else {
                    4_096
                };
                binary(v.as_str().ok_or(Error("evidence item"))?, maximum)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (originals, apple_counter) = match (body.policy.platform, kind) {
            (
                KagemushaWalletEnrollmentPlatformV1::Android { hardware, .. },
                KagemushaWalletEvidenceKindV1::AndroidKeyMintTee
                | KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox,
            ) => {
                if matches!(
                    (hardware, kind),
                    (
                        KagemushaWalletAndroidHardwareV1::Tee,
                        KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox
                    ) | (
                        KagemushaWalletAndroidHardwareV1::StrongBox,
                        KagemushaWalletEvidenceKindV1::AndroidKeyMintTee
                    )
                ) || value.get("app_attest_key_id") != Some(&Value::Null)
                    || value.get("app_attest_counter") != Some(&Value::Null)
                {
                    return Err(Error("Android evidence policy"));
                }
                let google_response = items.pop().ok_or(Error("Google original"))?;
                (
                    IssuerEvidenceV1::Android {
                        certificates: items,
                        google_response,
                    },
                    None,
                )
            }
            (
                KagemushaWalletEnrollmentPlatformV1::Apple { .. },
                KagemushaWalletEvidenceKindV1::AppleAppAttest,
            ) => {
                if items.len() != 2 {
                    return Err(Error("Apple original count"));
                }
                let key_id = digest(&value, "app_attest_key_id")?;
                let counter = u32::try_from(integer(&value, "app_attest_counter")?)
                    .map_err(|_| Error("Apple counter"))?;
                let PlatformEvidenceV1::Apple {
                    key_id: expected, ..
                } = PlatformEvidenceV1::decode(&body.evidence, &body.policy).map_err(Error)?
                else {
                    return Err(Error("Apple original kind"));
                };
                if key_id != expected || counter == 0 {
                    return Err(Error("Apple key/counter"));
                }
                let key_binding_assertion = items.pop().ok_or(Error("Apple assertion"))?;
                let attestation = items.pop().ok_or(Error("Apple attestation"))?;
                (
                    IssuerEvidenceV1::Apple {
                        attestation,
                        key_binding_assertion,
                    },
                    Some((key_id, counter)),
                )
            }
            _ => return Err(Error("configured platform differs")),
        };
        let evidence = KagemushaWalletEvidenceV1 {
            digest: originals.digest(body, kind).map_err(Error)?,
            time_ms: self.verification_time_ms,
            facts: u32::try_from(integer(&value, "facts")?).map_err(|_| Error("fact range"))?,
            os_patch_level: u32::try_from(integer(&value, "os_patch_level")?)
                .map_err(|_| Error("patch range"))?,
            vendor_patch_level: u32::try_from(integer(&value, "vendor_patch_level")?)
                .map_err(|_| Error("patch range"))?,
            boot_patch_level: u32::try_from(integer(&value, "boot_patch_level")?)
                .map_err(|_| Error("patch range"))?,
        };
        evidence
            .validate_enrollment_for_kind(kind)
            .map_err(|_| Error("evidence facts"))?;
        // These are facts of the actual retained adapters, beyond the generic Model minimum.
        let required = if kind.is_android() {
            KAGEMUSHA_WALLET_FACT_PLAY_INTEGRITY_SIGNAL_V1
                | KAGEMUSHA_WALLET_FACT_REVOCATION_LIST_CLEAR_V1
        } else {
            KAGEMUSHA_WALLET_FACT_APP_ATTEST_PRODUCTION_V1
        };
        let allowed = kind.required_enrollment_facts()
            | required
            | if kind.is_android() {
                KAGEMUSHA_WALLET_FACT_PATCH_POLICY_MET_V1
            } else {
                0
            };
        if evidence.facts & required != required
            || evidence.facts & !allowed != 0
            || digest(&value, "evidence_digest")? != evidence.digest
        {
            return Err(Error("evidence projection differs"));
        }
        Ok(EvidenceProjectionV1 {
            kind,
            evidence,
            originals,
            apple_counter,
            original_result,
        })
    }
}

#[cfg(test)]
#[path = "issuer_worker_tests.rs"]
mod tests;
