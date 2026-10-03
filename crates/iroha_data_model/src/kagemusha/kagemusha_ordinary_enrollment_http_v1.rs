//! Sole five-stage ordinary enrollment HTTP DATA contract shared by Core, FI and SDKs.
//!
//! Parsing and correlation supply no current owner, issuer, credential or monetary authority.
//! Native retains the original reservation and independently admits C515/raw314/E424/FI originals.
//! Core authenticates the actual FI workload and obtains its genuine current account observation.

use super::*;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use norito::json::{self, JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};

/// Complete request or response bound, measured before parsing any JSON.
pub const KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_MAX_BYTES_V1: usize = 256 * 1024;
/// Complete original raw platform attestation bound.
pub const KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_RAW_MAX_BYTES_V1: usize = 128 * 1024;
const RAW_SCHEMA: &str = "iroha.kagemusha.ordinary-app-raw-admission-request.v1";
const CREDENTIAL_SCHEMA: &str = "iroha.kagemusha.ordinary-app-credential-request.v1";
type Result<T> = std::result::Result<T, String>;

/// Closed HTTP stage census. These stages contain DATA and cannot create Native holders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryEnrollmentHttpStageV1 {
    /// Native-selected C reservation request.
    Prepare,
    /// Original platform attestation and requested raw314 admission.
    RawAttestation,
    /// Original platform possession and requested credential.
    Certificate,
    /// Complete retained enrollment originals followed by the FI account challenge.
    Start,
    /// Exact wallet Ed64 followed by the retained FI certificate.
    Finish,
}
impl KagemushaOrdinaryEnrollmentHttpStageV1 {
    /// Sole closed HTTP DATA path intake; a match creates no authenticated request or owner.
    /// Unknown and retired prefixes, query strings and trailing path components are rejected.
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        match path {
            "/v1/kagemusha/enrollment/ordinary/prepare" => Some(Self::Prepare),
            "/v1/kagemusha/enrollment/ordinary/raw-attestation" => Some(Self::RawAttestation),
            "/v1/kagemusha/enrollment/ordinary/certificate" => Some(Self::Certificate),
            "/v1/kagemusha/enrollment/ordinary/start" => Some(Self::Start),
            "/v1/kagemusha/enrollment/ordinary/finish" => Some(Self::Finish),
            _ => None,
        }
    }
    /// Sole first-release route; Core and FI authenticate their distinct workload/user roles.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Prepare => "/v1/kagemusha/enrollment/ordinary/prepare",
            Self::RawAttestation => "/v1/kagemusha/enrollment/ordinary/raw-attestation",
            Self::Certificate => "/v1/kagemusha/enrollment/ordinary/certificate",
            Self::Start => "/v1/kagemusha/enrollment/ordinary/start",
            Self::Finish => "/v1/kagemusha/enrollment/ordinary/finish",
        }
    }
}

macro_rules! record {
    ($(#[$meta:meta])* $name:ident { $($(#[$field_meta:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize)]
        #[norito(deny_unknown_fields)]
        pub struct $name { $($(#[$field_meta])* pub $field: $ty),* }
    };
}
record! {
    /// Entire Native reservation carrier. All selectors are public correlation DATA.
    KagemushaOrdinaryPreparationHttpRequestV1 {
        /// Actual selected canonical I105 account; FI must compare its live authenticated owner.
        account_id: String,
        /// Original Native-generated nonce, fixed before HTTP.
        client_nonce_hex: String,
        /// Original independently selected release ID.
        release_id_hex: String,
        /// Original independently selected profile ID.
        profile_id_hex: String,
        /// Original selected lane; issuer independently derives and compares it.
        lane_id_hex: String,
        /// Original commitment to the distinct Native financial secret.
        financial_authority_commitment_hex: String,
    }
}
record! {
    /// Original platform inputs. No caller verdict or policy selector is accepted.
    KagemushaOrdinaryRawAttestationHttpRequestV1 {
        /// Exact raw-admission DATA schema.
        schema: String,
        /// Exact issue purpose.
        operation: String,
        /// SHA256 of the Model-owned C signing message.
        operation_id: String,
        /// Complete original signed C515.
        signed_preparation_base64: String,
        /// Complete original uncompressed P256 SEC1 point65.
        attested_public_key_sec1_base64: String,
        /// Complete original platform container.
        raw_attestation_base64: String,
    }
}
record! {
    /// Public possession bytes. The issuer and Native separately verify the actual platform.
    KagemushaOrdinaryPossessionHttpV1 {
        /// Sole Android or Apple platform discriminator.
        platform: String,
        /// Original Android low-S DER signature; absent for Apple.
        #[norito(default, skip_serializing_if = "Option::is_none")]
        signature_der_base64: Option<String>,
        /// Original Apple assertion; absent for Android.
        #[norito(default, skip_serializing_if = "Option::is_none")]
        raw_assertion_base64: Option<String>,
    }
}
record! {
    /// Complete credential request. An opaque Google token is DATA, never a verdict.
    KagemushaOrdinaryCredentialHttpRequestV1 {
        /// Exact credential DATA schema.
        schema: String,
        /// Exact issue purpose.
        operation: String,
        /// SHA256 of the Model-owned C signing message.
        operation_id: String,
        /// Complete original signed C515.
        signed_preparation_base64: String,
        /// Complete original uncompressed P256 SEC1 point65.
        attested_public_key_sec1_base64: String,
        /// Complete original platform container.
        raw_attestation_base64: String,
        /// Actual platform possession original.
        app_possession: KagemushaOrdinaryPossessionHttpV1,
        /// Original opaque provider token, or explicit null under the independently selected policy.
        play_integrity_token: Option<String>,
    }
}
record! {
    /// Complete original selected Integrity pair. Initial Start requires an explicit null:
    /// no refresh owner exists before the original FI enrollment has completed.
    #[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryStartIntegrityHttpV1")]
    KagemushaOrdinaryStartIntegrityHttpV1 {
        /// Original selected Integrity challenge, never an approved projection.
        challenge: String,
        /// Original selected Integrity lease, never a provider verdict.
        lease: String,
    }
}
// Keep the required Option literal in this struct: a macro-forwarded type fragment
// becomes an opaque type group to the derives and cannot select required-Option semantics.
/// Sole complete-original initial FI Start DATA. All seven fields are required.
/// Decoding this carrier supplies no platform, issuer, current or monetary authority.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::Encode,
    norito::Decode,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryRetailStartHttpRequestV1")]
pub struct KagemushaOrdinaryRetailStartHttpRequestV1 {
    /// Canonical actual Native-selected wallet account, distinct from the FI signatory.
    pub wallet: String,
    /// Complete original signed C515.
    pub signed_preparation_base64: String,
    /// Complete original signed raw314.
    pub raw_admission_original_base64: String,
    /// Complete canonical ordered original platform container.
    pub platform_original_base64: String,
    /// Complete canonical E archive, including the actual platform possession original.
    pub core_possession_original_base64: String,
    /// Complete original canonical app credential.
    pub app_certificate_base64: String,
    /// Explicit null for initial Start; absence or caller-selected refresh originals refuse.
    #[norito(required)]
    pub selected_integrity: Option<KagemushaOrdinaryStartIntegrityHttpV1>,
}
record! {
    /// Finish supplies only the retained challenge identity and exact wallet Ed64.
    KagemushaOrdinaryRetailFinishHttpRequestV1 {
        /// Original C signing-message digest, distinct from the stable enrollment ID.
        challenge_id: String,
        /// Original wallet Ed64, without prehashing the original Model raw32 message again.
        account_signature_base64: String,
    }
}

/// Closed parsed request DATA. No variant contains or creates an authenticated Current.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryEnrollmentHttpRequestV1 {
    /// Six-field Native preparation carrier.
    Prepare(KagemushaOrdinaryPreparationHttpRequestV1),
    /// Original platform attestation carrier.
    RawAttestation(KagemushaOrdinaryRawAttestationHttpRequestV1),
    /// Original platform possession carrier.
    Certificate(KagemushaOrdinaryCredentialHttpRequestV1),
    /// Complete seven-field retained-original carrier.
    Start(KagemushaOrdinaryRetailStartHttpRequestV1),
    /// Original wallet signature carrier.
    Finish(KagemushaOrdinaryRetailFinishHttpRequestV1),
}

fn reject<T>() -> Result<T> {
    Err("ordinary enrollment HTTP DATA differs".into())
}
fn hash(original: &[u8]) -> [u8; 32] {
    Sha256::digest(original).into()
}
#[cfg(test)]
fn hex(original: &[u8]) -> String {
    hex::encode(original)
}
fn hex32(value: &str) -> Result<[u8; 32]> {
    let raw = value.as_bytes();
    if raw.len() != 64
        || !raw
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return reject();
    }
    let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
    let mut out = [0; 32];
    for (i, pair) in raw.chunks_exact(2).enumerate() {
        out[i] = digit(pair[0]) * 16 + digit(pair[1]);
    }
    if out == [0; 32] {
        return reject();
    }
    Ok(out)
}
fn base64(value: &str, maximum: usize, exact: Option<usize>) -> Result<Vec<u8>> {
    if value.is_empty() || value.len() > maximum.div_ceil(3) * 4 {
        return reject();
    }
    let raw = STANDARD
        .decode(value)
        .map_err(|_| "ordinary original base64 differs")?;
    if raw.is_empty()
        || raw.len() > maximum
        || exact.is_some_and(|n| raw.len() != n)
        || STANDARD.encode(&raw) != value
    {
        return reject();
    }
    Ok(raw)
}
fn preparation(value: &str) -> Result<KagemushaSignedOrdinaryAppEnrollmentChallengeV1> {
    KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&base64(
        value,
        515,
        Some(515),
    )?)
}
fn point(value: &str) -> Result<()> {
    let raw = base64(value, 65, Some(65))?;
    KagemushaDevicePublicKeyV1::from_sec1_bytes(&raw).map_err(|e| e.to_string())?;
    Ok(())
}
fn platform(value: &str, c: &KagemushaSignedOrdinaryAppEnrollmentChallengeV1) -> Result<()> {
    let raw = base64(
        value,
        KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_RAW_MAX_BYTES_V1,
        None,
    )?;
    let original = KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&raw)?;
    if original.platform_class() != c.challenge.platform_class {
        return reject();
    }
    Ok(())
}
fn bounded(original: &[u8]) -> Result<()> {
    if original.is_empty() || original.len() > KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_MAX_BYTES_V1 {
        return reject();
    }
    Ok(())
}
fn decode<T: JsonDeserialize>(original: &[u8]) -> Result<T> {
    bounded(original)?;
    json::from_slice(original).map_err(|e| e.to_string())
}
fn encode<T: JsonSerialize>(value: &T) -> Result<Vec<u8>> {
    // Native Value uses sorted object keys; output is the sole minimal JSON representation.
    let raw = json::to_vec(&json::to_value(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    bounded(&raw)?;
    Ok(raw)
}
fn operation(value: &str, c: &KagemushaSignedOrdinaryAppEnrollmentChallengeV1) -> Result<()> {
    if hex32(value)? != c.challenge.attestation_challenge()? {
        return reject();
    }
    Ok(())
}
impl KagemushaOrdinaryEnrollmentHttpRequestV1 {
    /// Parse the exact closed HTTP path and full DATA body through the sole bounded codec.
    /// Core/FI must separately obtain genuine workload/user/current and signing custody.
    /// # Errors
    /// Refuses retired/unknown route intake before parsing, or an invalid complete stage carrier.
    pub fn parse_http_data(path: &str, original: &[u8]) -> Result<Self> {
        let stage = KagemushaOrdinaryEnrollmentHttpStageV1::from_path(path)
            .ok_or("ordinary enrollment HTTP path is outside the closed first-release census")?;
        Self::parse(stage, original)
    }
    /// Decode the fixed stage with strict field/platform/bounds and complete-original joins.
    /// These are DATA checks; no signature, policy, provider or live owner admission occurs.
    /// # Errors
    /// Rejects malformed/unknown/duplicate fields, wrong stage or substituted original correlations.
    pub fn parse(stage: KagemushaOrdinaryEnrollmentHttpStageV1, original: &[u8]) -> Result<Self> {
        let value = match stage {
            KagemushaOrdinaryEnrollmentHttpStageV1::Prepare => Self::Prepare(decode(original)?),
            KagemushaOrdinaryEnrollmentHttpStageV1::RawAttestation => {
                Self::RawAttestation(decode(original)?)
            }
            KagemushaOrdinaryEnrollmentHttpStageV1::Certificate => {
                Self::Certificate(decode(original)?)
            }
            KagemushaOrdinaryEnrollmentHttpStageV1::Start => Self::Start(decode(original)?),
            KagemushaOrdinaryEnrollmentHttpStageV1::Finish => Self::Finish(decode(original)?),
        };
        value.validate()?;
        if let Self::Certificate(v) = &value {
            let observed: norito::json::Value = decode(original)?;
            let actual = observed
                .get("app_possession")
                .and_then(norito::json::Value::as_object)
                .ok_or("original possession object absent")?;
            let expected = match v.app_possession.platform.as_str() {
                "android_keystore" => ["platform", "signature_der_base64"],
                "apple_app_attest" => ["platform", "raw_assertion_base64"],
                _ => return reject(),
            };
            if actual.len() != expected.len()
                || expected.iter().any(|key| !actual.contains_key(*key))
                || observed.get("play_integrity_token").is_none()
            {
                return reject();
            }
        }
        Ok(value)
    }
    /// Sole stage selected by this closed DATA variant.
    #[must_use]
    pub const fn stage(&self) -> KagemushaOrdinaryEnrollmentHttpStageV1 {
        use KagemushaOrdinaryEnrollmentHttpStageV1 as Stage;
        match self {
            Self::Prepare(_) => Stage::Prepare,
            Self::RawAttestation(_) => Stage::RawAttestation,
            Self::Certificate(_) => Stage::Certificate,
            Self::Start(_) => Stage::Start,
            Self::Finish(_) => Stage::Finish,
        }
    }
    /// Sole sorted canonical JSON projection. Original HTTP bytes remain retained separately.
    /// # Errors
    /// Refuses an invalid or oversized complete DATA carrier.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        match self {
            Self::Prepare(v) => encode(v),
            Self::RawAttestation(v) => encode(v),
            Self::Certificate(v) => encode(v),
            Self::Start(v) => encode(v),
            Self::Finish(v) => encode(v),
        }
    }
    fn validate(&self) -> Result<()> {
        match self {
            Self::Prepare(v) => {
                if v.account_id.is_empty() || v.account_id.len() > 4096 {
                    return reject();
                }
                let account = AccountId::parse_encoded(&v.account_id).map_err(|e| e.to_string())?;
                if account.canonical_i105().map_err(|e| e.to_string())? != v.account_id {
                    return reject();
                }
                for value in [
                    &v.client_nonce_hex,
                    &v.release_id_hex,
                    &v.profile_id_hex,
                    &v.lane_id_hex,
                    &v.financial_authority_commitment_hex,
                ] {
                    hex32(value)?;
                }
            }
            Self::RawAttestation(v) => {
                if v.schema != RAW_SCHEMA || v.operation != "issue" {
                    return reject();
                }
                let c = preparation(&v.signed_preparation_base64)?;
                operation(&v.operation_id, &c)?;
                point(&v.attested_public_key_sec1_base64)?;
                platform(&v.raw_attestation_base64, &c)?;
            }
            Self::Certificate(v) => {
                if v.schema != CREDENTIAL_SCHEMA || v.operation != "issue" {
                    return reject();
                }
                let c = preparation(&v.signed_preparation_base64)?;
                operation(&v.operation_id, &c)?;
                point(&v.attested_public_key_sec1_base64)?;
                platform(&v.raw_attestation_base64, &c)?;
                match v.app_possession.platform.as_str() {
                    "android_keystore"
                        if c.challenge.platform_class
                            == KagemushaHardwarePlatformClassV1::AndroidKeyMint
                            && v.app_possession.raw_assertion_base64.is_none() =>
                    {
                        let raw = base64(
                            v.app_possession
                                .signature_der_base64
                                .as_deref()
                                .ok_or("original DER absent")?,
                            72,
                            None,
                        )?;
                        if raw.len() < 8 {
                            return reject();
                        }
                    }
                    "apple_app_attest"
                        if c.challenge.platform_class
                            == KagemushaHardwarePlatformClassV1::AppleAppAttest
                            && v.app_possession.signature_der_base64.is_none()
                            && v.play_integrity_token.is_none() =>
                    {
                        base64(
                            v.app_possession
                                .raw_assertion_base64
                                .as_deref()
                                .ok_or("original assertion absent")?,
                            KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1,
                            None,
                        )?;
                    }
                    _ => return reject(),
                }
                if let Some(token) = &v.play_integrity_token
                    && (token.is_empty()
                        || token.len() > 64 * 1024
                        || !token.bytes().all(|b| (0x21..=0x7e).contains(&b)))
                {
                    return reject();
                }
            }
            Self::Start(v) => v.validate_original_data()?,
            Self::Finish(v) => {
                hex32(&v.challenge_id)?;
                base64(&v.account_signature_base64, 64, Some(64))?;
            }
        }
        Ok(())
    }
}

impl KagemushaOrdinaryRetailStartHttpRequestV1 {
    fn validate_original_data(&self) -> Result<()> {
        if self.wallet.is_empty() || self.wallet.len() > 4096 || self.selected_integrity.is_some() {
            return reject();
        }
        let account = AccountId::parse_encoded(&self.wallet).map_err(|e| e.to_string())?;
        let c = preparation(&self.signed_preparation_base64)?;
        if account.canonical_i105().map_err(|e| e.to_string())? != self.wallet
            || kagemusha_ordinary_app_account_binding_v1(&account) != c.challenge.account_binding
        {
            return reject();
        }
        let raw_bytes = base64(&self.raw_admission_original_base64, 314, Some(314))?;
        let raw = KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&raw_bytes)?;
        let platform_bytes = base64(
            &self.platform_original_base64,
            KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_RAW_MAX_BYTES_V1,
            None,
        )?;
        let platform =
            KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&platform_bytes)?;
        let e_bytes = base64(
            &self.core_possession_original_base64,
            KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1024,
            None,
        )?;
        let e = KagemushaAppEnrollmentPossessionV1::decode_canonical_exact(&e_bytes)?;
        let credential_bytes = base64(&self.app_certificate_base64, 16 * 1024, None)?;
        let credential =
            KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&credential_bytes)?;
        let c = &c.challenge;
        let r = &raw.subject;
        let s = &credential.subject;
        let original_e = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            c,
            &r.app_public_key,
            hash(&platform_bytes),
        )?;
        let possession = match &e.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der }
                if c.platform_class == KagemushaHardwarePlatformClassV1::AndroidKeyMint
                    && (8..=72).contains(&signature_der.len()) =>
            {
                signature_der.as_slice()
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion }
                if c.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest
                    && (37..=KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1)
                        .contains(&raw_assertion.len()) =>
            {
                raw_assertion.as_slice()
            }
            _ => return reject(),
        };
        if platform.platform_class() != c.platform_class
            || r.platform_class != c.platform_class
            || r.enrollment_challenge_digest != c.attestation_challenge()?
            || r.authority_policy_digest != c.app_authority_policy_digest
            || r.raw_platform_evidence_digest != hash(&platform_bytes)
            || r.issued_at_ms != c.issued_at_ms
            || r.expires_at_ms != c.expires_at_ms
            || e.challenge != original_e
            || s.platform_class != c.platform_class
            || s.enrollment_id != c.enrollment_id
            || s.client_nonce != c.client_nonce
            || s.server_nonce != c.server_nonce
            || s.account_binding != c.account_binding
            || s.network_id != c.network_id
            || s.lane_id != c.lane_id
            || s.release_id != c.release_id
            || s.hardware_profile_id != c.hardware_profile_id
            || s.suite_id != c.suite_id
            || s.trust_policy_digest != c.trust_policy_digest
            || s.app_authority_policy_digest != c.app_authority_policy_digest
            || s.policy_epoch != c.policy_epoch
            || s.hardware_epoch != c.hardware_epoch
            || s.enrollment_challenge_digest != c.attestation_challenge()?
            || s.financial_authority_commitment != c.financial_authority_commitment
            || s.app_public_key != r.app_public_key
            || s.attested_key_id != r.attested_key_id
            || s.security_level != r.security_level
            || s.app_signing_identity_digest != r.app_signing_identity_digest
            || s.platform_evidence_digest
                != kagemusha_ordinary_app_enrollment_evidence_digest_v1(
                    &platform_bytes,
                    possession,
                )?
        {
            return reject();
        }
        Ok(())
    }
    /// Bounded canonical Norito DATA roundtrip of the same seven complete originals.
    /// # Errors
    /// Refuses malformed joined originals or a full canonical frame above the HTTP bound.
    pub fn canonical_norito_bytes(&self) -> Result<Vec<u8>> {
        self.validate_original_data()?;
        encode(self)?;
        let size = norito::canonical_frame_len(self).map_err(|e| e.to_string())?;
        if size > KAGEMUSHA_ORDINARY_ENROLLMENT_HTTP_MAX_BYTES_V1 {
            return reject();
        }
        norito::encode_canonical(self).map_err(|e| e.to_string())
    }
    /// Exact bounded canonical Norito intake; no authority is granted by decoding.
    /// # Errors
    /// Refuses overflow, unknown fields, a tail or a noncanonical complete-original join.
    pub fn decode_canonical_norito_exact(original: &[u8]) -> Result<Self> {
        bounded(original)?;
        let value: Self = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_norito_bytes()? != original {
            return reject();
        }
        Ok(value)
    }
}

record! {
    /// Entire C515 reply and its public correlation; signature admission remains Native.
    KagemushaOrdinaryPreparationHttpReplyV1 {
        /// SHA256 of the sole C signing message.
        operation_id: String,
        /// Complete original signed C515.
        signed_preparation_base64: String,
        /// Exact same C signing-message SHA256 as raw32.
        attestation_challenge_base64: String,
        /// Exact original C exclusive expiry; never a renewed interval.
        expires_at_ms: u64,
    }
}
record! {
    /// Entire raw314 reply; parsing supplies no platform verification or raw authority.
    KagemushaOrdinaryRawAdmissionHttpReplyV1 {
        /// Complete original signed raw314.
        raw_admission_base64: String,
        /// SHA256 of the complete original signed raw314.
        raw_admission_sha256_hex: String,
    }
}
record! {
    /// Entire canonical credential reply; Native authenticates its complete issuer signatures.
    KagemushaOrdinaryCredentialHttpReplyV1 {
        /// Complete original canonical credential.
        certificate_base64: String,
        /// SHA256 of the complete original canonical credential.
        certificate_sha256_hex: String,
    }
}
record! {
    /// Entire ordinary retail challenge and actual wallet raw32 projection.
    KagemushaOrdinaryRetailStartHttpReplyV1 {
        /// Original C signing-message SHA256.
        challenge_id: String,
        /// Complete original canonical ordinary retail challenge.
        canonical_challenge_base64: String,
        /// Exact Model-owned raw32 account-signing message.
        account_signing_message_base64: String,
        /// Exact original challenge exclusive expiry.
        expires_at_ms: u64,
    }
}
record! {
    /// Entire ordinary FI certificate; public selectors supply no enrollment admission.
    KagemushaOrdinaryRetailFinishHttpReplyV1 {
        /// Original C signing-message SHA256.
        challenge_id: String,
        /// Stable exact Model-derived original ownership selector.
        enrollment_id_hex: String,
        /// Complete original canonical ordinary FI certificate.
        canonical_certificate_base64: String,
    }
}
/// Closed reply DATA census. Detached values cannot create a Native or issuer capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryEnrollmentHttpReplyV1 {
    /// C515 original and correlation.
    Prepare(KagemushaOrdinaryPreparationHttpReplyV1),
    /// Raw314 original and complete-original digest.
    RawAttestation(KagemushaOrdinaryRawAdmissionHttpReplyV1),
    /// Canonical credential original and complete-original digest.
    Certificate(KagemushaOrdinaryCredentialHttpReplyV1),
    /// Full ordinary challenge and original wallet message.
    Start(KagemushaOrdinaryRetailStartHttpReplyV1),
    /// Full ordinary FI certificate and exact ownership correlation.
    Finish(KagemushaOrdinaryRetailFinishHttpReplyV1),
}
impl KagemushaOrdinaryEnrollmentHttpReplyV1 {
    /// Parse the exact reply stage and join complete originals to the actual request DATA.
    /// This does not verify issuer signatures, FI approval, device possession or currentness.
    /// # Errors
    /// Refuses wrong/extra/duplicate fields, incomplete originals or substituted correlation.
    pub fn parse(
        request: &KagemushaOrdinaryEnrollmentHttpRequestV1,
        original: &[u8],
    ) -> Result<Self> {
        use KagemushaOrdinaryEnrollmentHttpStageV1 as Stage;
        let value = match request.stage() {
            Stage::Prepare => Self::Prepare(decode(original)?),
            Stage::RawAttestation => Self::RawAttestation(decode(original)?),
            Stage::Certificate => Self::Certificate(decode(original)?),
            Stage::Start => Self::Start(decode(original)?),
            Stage::Finish => Self::Finish(decode(original)?),
        };
        value.require_request_data(request)?;
        Ok(value)
    }
    /// Sole sorted complete reply JSON; called only after actual issuer durable publication.
    /// # Errors
    /// Refuses complete DATA mismatches or a response beyond the HTTP bound.
    pub fn canonical_bytes(
        &self,
        request: &KagemushaOrdinaryEnrollmentHttpRequestV1,
    ) -> Result<Vec<u8>> {
        self.require_request_data(request)?;
        match self {
            Self::Prepare(v) => encode(v),
            Self::RawAttestation(v) => encode(v),
            Self::Certificate(v) => encode(v),
            Self::Start(v) => encode(v),
            Self::Finish(v) => encode(v),
        }
    }
    /// Correlate complete public originals only; no authority is granted by a successful join.
    /// # Errors
    /// Refuses substituted request phase, C, nonce, release/profile/lane, key or signed original.
    pub fn require_request_data(
        &self,
        request: &KagemushaOrdinaryEnrollmentHttpRequestV1,
    ) -> Result<()> {
        request.validate()?;
        match (self, request) {
            (Self::Prepare(v), KagemushaOrdinaryEnrollmentHttpRequestV1::Prepare(r)) => {
                let c = preparation(&v.signed_preparation_base64)?;
                let account = AccountId::parse_encoded(&r.account_id).map_err(|e| e.to_string())?;
                operation(&v.operation_id, &c)?;
                if base64(&v.attestation_challenge_base64, 32, Some(32))?
                    != c.challenge.attestation_challenge()?
                    || v.expires_at_ms != c.challenge.expires_at_ms
                    || c.challenge.client_nonce != hex32(&r.client_nonce_hex)?
                    || c.challenge.release_id != hex32(&r.release_id_hex)?
                    || c.challenge.hardware_profile_id != hex32(&r.profile_id_hex)?
                    || c.challenge.lane_id != hex32(&r.lane_id_hex)?
                    || c.challenge.financial_authority_commitment
                        != hex32(&r.financial_authority_commitment_hex)?
                    || c.challenge.account_binding
                        != kagemusha_ordinary_app_account_binding_v1(&account)
                {
                    return reject();
                }
            }
            (
                Self::RawAttestation(v),
                KagemushaOrdinaryEnrollmentHttpRequestV1::RawAttestation(r),
            ) => {
                let original = base64(&v.raw_admission_base64, 314, Some(314))?;
                if hex32(&v.raw_admission_sha256_hex)? != hash(&original) {
                    return reject();
                }
                let raw = KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(&original)?;
                let c = preparation(&r.signed_preparation_base64)?;
                if raw.subject.enrollment_challenge_digest != c.challenge.attestation_challenge()?
                    || raw.subject.authority_policy_digest
                        != c.challenge.app_authority_policy_digest
                    || raw.subject.platform_class != c.challenge.platform_class
                    || raw.subject.issued_at_ms != c.challenge.issued_at_ms
                    || raw.subject.expires_at_ms != c.challenge.expires_at_ms
                    || raw.subject.app_public_key.as_sec1_bytes().as_slice()
                        != base64(&r.attested_public_key_sec1_base64, 65, Some(65))?
                    || raw.subject.raw_platform_evidence_digest
                        != hash(&base64(&r.raw_attestation_base64, 128 * 1024, None)?)
                {
                    return reject();
                }
            }
            (Self::Certificate(v), KagemushaOrdinaryEnrollmentHttpRequestV1::Certificate(r)) => {
                let original = base64(&v.certificate_base64, 16 * 1024, None)?;
                if hex32(&v.certificate_sha256_hex)? != hash(&original) {
                    return reject();
                }
                let credential =
                    KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&original)?;
                let c = preparation(&r.signed_preparation_base64)?;
                if credential.subject.enrollment_id != c.challenge.enrollment_id
                    || credential.subject.enrollment_challenge_digest
                        != c.challenge.attestation_challenge()?
                    || credential.subject.app_public_key.as_sec1_bytes().as_slice()
                        != base64(&r.attested_public_key_sec1_base64, 65, Some(65))?
                    || credential.subject.financial_authority_commitment
                        != c.challenge.financial_authority_commitment
                {
                    return reject();
                }
            }
            (Self::Start(v), KagemushaOrdinaryEnrollmentHttpRequestV1::Start(r)) => {
                let c = preparation(&r.signed_preparation_base64)?;
                operation(&v.challenge_id, &c)?;
                let original = base64(&v.canonical_challenge_base64, 32 * 1024, None)?;
                let challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1 =
                    norito::decode_canonical_with_limits(
                        &original,
                        norito::canonical_decode_limits(original.len()),
                    )
                    .map_err(|e| e.to_string())?;
                if challenge.canonical_bytes()? != original
                    || challenge.preparation != c
                    || challenge.issuance.credential.canonical_bytes()?
                        != base64(&r.app_certificate_base64, 16 * 1024, None)?
                    || challenge.owner.enrollment_id().map_err(|e| e.to_string())?
                        != c.challenge.enrollment_id
                    || challenge.issued_at_ms < c.challenge.issued_at_ms
                    || challenge.expires_at_ms > c.challenge.expires_at_ms
                    || v.expires_at_ms != challenge.expires_at_ms
                    || base64(&v.account_signing_message_base64, 32, Some(32))?
                        != challenge.account_signing_message()?
                {
                    return reject();
                }
            }
            (Self::Finish(v), KagemushaOrdinaryEnrollmentHttpRequestV1::Finish(r)) => {
                if hex32(&v.challenge_id)? != hex32(&r.challenge_id)? {
                    return reject();
                }
                let original = base64(&v.canonical_certificate_base64, 16 * 1024, None)?;
                let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 =
                    norito::decode_canonical_with_limits(
                        &original,
                        norito::canonical_decode_limits(original.len()),
                    )
                    .map_err(|e| e.to_string())?;
                if certificate.canonical_bytes()? != original
                    || certificate.subject.enrollment_id != hex32(&v.enrollment_id_hex)?
                    || certificate
                        .subject
                        .owner
                        .enrollment_id()
                        .map_err(|e| e.to_string())?
                        != hex32(&v.enrollment_id_hex)?
                    || certificate
                        .subject
                        .issuance
                        .credential
                        .subject
                        .enrollment_id
                        != certificate.subject.enrollment_id
                    || certificate
                        .subject
                        .issuance
                        .credential
                        .subject
                        .enrollment_challenge_digest
                        != hex32(&r.challenge_id)?
                {
                    return reject();
                }
            }
            _ => return reject(),
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "kagemusha_ordinary_enrollment_http_v1_tests.rs"]
mod tests;
