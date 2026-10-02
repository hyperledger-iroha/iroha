//! Complete governed platform root/evaluation originals for ordinary enrollment.
//!
//! Threshold signatures authenticate the actual ordered DER anchors and evaluation originals
//! under the independently checked ordinary governance configuration. They do not assert PKIX,
//! KeyMint/AppAttest validity, Apple receipt metrics, Play verdicts or native installation.
//! Actual server verifiers must consume this opaque owner and retain their complete originals.
use super::{
    KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1, KagemushaHardwarePlatformClassV1,
    KagemushaOrdinaryAppIdentityAuthorityPolicyV1,
};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Complete bounded original root/evaluation archive, before any decoding/allocation.
pub const KAGEMUSHA_GOVERNED_PLATFORM_ORIGINALS_MAX_BYTES_V1: usize = 2 * 1024 * 1024;
const ROOT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-platform-root-originals\0";
const APPROVAL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-platform-originals-approval\0";
const APPLE_RISK_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-apple-receipt-risk-policy\0";
const MAX_DER: usize = 16 * 1024;
const MAX_STATUS: usize = 512 * 1024;
/// Ordered complete root DER originals. Receipt roots are a distinct Apple signing role.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlatformRootOriginalsV1")]
pub struct KagemushaPlatformRootOriginalsV1 {
    /// Fixed original codec version.
    pub version: u16,
    /// Original independently selected platform.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Exact ordered platform anchor DER bytes; certificates are never supplied by the response.
    pub platform_roots_der: Vec<Vec<u8>>,
    /// Exact ordered Apple receipt-signing anchors; never relabeled attestation roots.
    pub apple_receipt_roots_der: Vec<Vec<u8>>,
}
impl KagemushaPlatformRootOriginalsV1 {
    /// Exact domain-separated digest of the complete ordered original root archive.
    /// # Errors
    /// Rejects malformed/duplicate/missing/bounded role selectors; no certificate math is asserted.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        Ok(digest(ROOT_DOMAIN, &encode(self)?))
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || !(1..=8).contains(&self.platform_roots_der.len())
            || self
                .platform_roots_der
                .iter()
                .any(|d| d.is_empty() || d.len() > MAX_DER)
            || self
                .platform_roots_der
                .iter()
                .enumerate()
                .any(|(i, d)| self.platform_roots_der[..i].contains(d))
            || self
                .apple_receipt_roots_der
                .iter()
                .any(|d| d.is_empty() || d.len() > MAX_DER)
            || self
                .apple_receipt_roots_der
                .iter()
                .enumerate()
                .any(|(i, d)| self.apple_receipt_roots_der[..i].contains(d))
        {
            return Err("governed complete platform roots malformed".into());
        }
        match self.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint
                if self.apple_receipt_roots_der.is_empty() =>
            {
                Ok(())
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest
                if self.platform_roots_der.len() == 1
                    && (1..=8).contains(&self.apple_receipt_roots_der.len())
                    && !self
                        .apple_receipt_roots_der
                        .iter()
                        .any(|r| self.platform_roots_der.contains(r)) =>
            {
                Ok(())
            }
            _ => Err("governed platform/receipt root roles differ".into()),
        }
    }
}
/// Signed Apple receipt risk policy. Real CMS signature/path/fields and metric remain mandatory.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaAppleReceiptRiskPolicyV1")]
pub struct KagemushaAppleReceiptRiskPolicyV1 {
    /// Codec version.
    pub version: u16,
    /// Exact TeamID.bundleID bytes whose SHA256 is the selected policy's App ID digest.
    pub app_id_utf8: Vec<u8>,
    /// Governed receipt creation freshness, never greater than Apple's five-minute bound.
    pub maximum_creation_age_ms: u64,
    /// Governed maximum signed RECEIPT risk metric; ATTEST has no metric and cannot satisfy it.
    pub maximum_risk_metric: u64,
}
impl KagemushaAppleReceiptRiskPolicyV1 {
    fn validate(&self) -> Result<(), String> {
        let s = std::str::from_utf8(&self.app_id_utf8).map_err(|_| "Apple governed App ID UTF8")?;
        let (team, bundle) = s.split_once('.').ok_or("Apple governed App ID shape")?;
        if self.version != 1
            || self.app_id_utf8.len() > 512
            || team.len() != 10
            || !team
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || bundle.is_empty()
            || !bundle
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            || !(1..=300_000).contains(&self.maximum_creation_age_ms)
        {
            return Err("Apple governed receipt risk policy malformed".into());
        }
        Ok(())
    }
    /// Exact independently governed Apple risk-policy preimage digest.
    /// # Errors
    /// Rejects invalid AppID/freshness policy; no risk metric success is granted.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        Ok(digest(APPLE_RISK_DOMAIN, &encode(self)?))
    }
}
/// Complete original evaluation policy. Android's existing ASCII preimage is reused byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlatformEvaluationOriginalsV1")]
pub enum KagemushaPlatformEvaluationOriginalsV1 {
    /// Complete committed Android status original and canonical SDK snapshot.
    AndroidRevocation {
        /// Exact iroha.android.attestation.revocation.snapshot.v1 ASCII bytes.
        canonical_snapshot_original: Vec<u8>,
        /// Full original status response payload whose SHA256 appears in the canonical snapshot.
        original_status_payload: Vec<u8>,
    },
    /// Actual Apple receipt signature/path/metric policy, under a separate receipt-root role.
    AppleReceiptRisk(KagemushaAppleReceiptRiskPolicyV1),
}
impl KagemushaPlatformEvaluationOriginalsV1 {
    /// Exact evaluation digest selected by the ordinary trust policy.
    /// # Errors
    /// Rejects noncanonical/incomplete originals, payload mismatch or invalid Apple policy.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        match self {
            Self::AndroidRevocation {
                canonical_snapshot_original,
                original_status_payload,
            } => {
                let s = AndroidSnapshot::decode(canonical_snapshot_original)?;
                if original_status_payload.is_empty()
                    || original_status_payload.len() > MAX_STATUS
                    || s.payload_sha256 != <[u8; 32]>::from(Sha256::digest(original_status_payload))
                {
                    return Err("Android complete status payload differs".into());
                }
                Ok(Sha256::digest(canonical_snapshot_original).into())
            }
            Self::AppleReceiptRisk(p) => p.canonical_digest(),
        }
    }
}
/// Threshold-signed root/evaluation snapshot complete original body.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaGovernedPlatformOriginalsSubjectV1")]
pub struct KagemushaGovernedPlatformOriginalsSubjectV1 {
    /// Codec version.
    pub version: u16,
    /// Independently installed ordinary governance signer set coordinate.
    pub authority_set_id: [u8; 32],
    /// Exact network coordinate of the independently checked ordinary policy.
    pub network_id: [u8; 32],
    /// Complete ordered platform and distinct receipt anchor originals.
    pub roots: KagemushaPlatformRootOriginalsV1,
    /// Complete original status/risk policy.
    pub evaluation: KagemushaPlatformEvaluationOriginalsV1,
    /// Original inclusive activation.
    pub valid_from_ms: u64,
    /// Original exclusive expiry; signatures/retries cannot renew it.
    pub expires_at_ms: u64,
}
impl KagemushaGovernedPlatformOriginalsSubjectV1 {
    /// Complete exact approval message, including every DER/status/risk original byte.
    /// # Errors
    /// Rejects wrong version/platform/interval or bounded/noncanonical evaluation originals.
    pub fn approval_signing_bytes(&self) -> Result<Vec<u8>, String> {
        self.roots.validate()?;
        self.evaluation.canonical_digest()?;
        if self.version != 1
            || self.authority_set_id == [0; 32]
            || self.network_id == [0; 32]
            || self.valid_from_ms == 0
            || self.valid_from_ms >= self.expires_at_ms
            || !matches!(
                (&self.evaluation, self.roots.platform_class),
                (
                    KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation { .. },
                    KagemushaHardwarePlatformClassV1::AndroidKeyMint
                ) | (
                    KagemushaPlatformEvaluationOriginalsV1::AppleReceiptRisk(_),
                    KagemushaHardwarePlatformClassV1::AppleAppAttest
                )
            )
        {
            return Err("governed complete original snapshot scope malformed".into());
        }
        let b = encode(self)?;
        let mut m = APPROVAL_DOMAIN.to_vec();
        m.extend_from_slice(&(b.len() as u64).to_le_bytes());
        m.extend_from_slice(&b);
        Ok(m)
    }
}
/// Actual original signer and Ed25519 signature over the complete root/evaluation subject.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaGovernedPlatformOriginalsApprovalV1")]
pub struct KagemushaGovernedPlatformOriginalsApprovalV1 {
    /// Full independent approved signer key; response cannot select another signer set.
    pub public_key: PublicKey,
    /// Full original Ed25519 signature.
    pub signature: Signature,
}
/// Complete original threshold-signed snapshot; decoded values confer no native authority.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaGovernedPlatformOriginalsV1")]
pub struct KagemushaGovernedPlatformOriginalsV1 {
    /// Entire original root/evaluation subject.
    pub subject: KagemushaGovernedPlatformOriginalsSubjectV1,
    /// Exact sorted original authorized signer approvals.
    pub approvals: Vec<KagemushaGovernedPlatformOriginalsApprovalV1>,
}
/// Opaque authenticated complete governance originals; no Clone, decoder or public constructor.
/// Actual PKIX/KeyMint/AppAttest/receipt/Play evaluation and durable issuer custody remain required.
pub struct KagemushaAuthenticatedGovernedPlatformOriginalsV1 {
    snapshot: KagemushaGovernedPlatformOriginalsV1,
    original: Vec<u8>,
    identity_policy_original: Vec<u8>,
    identity_authority_original: Vec<u8>,
    authenticated_at_ms: u64,
}
impl KagemushaGovernedPlatformOriginalsV1 {
    /// Bound exact canonical decode before signature evaluation.
    /// # Errors
    /// Rejects tails/re-encoding differences/resources/malformed original subject.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_GOVERNED_PLATFORM_ORIGINALS_MAX_BYTES_V1 {
            return Err("governed original bound".into());
        }
        let s: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|e| e.to_string())?;
        s.subject.approval_signing_bytes()?;
        if encode(&s)? != bytes {
            return Err("governed original noncanonical".into());
        }
        Ok(s)
    }
    /// Authenticate complete originals under the exact independently checked ordinary root set.
    /// # Errors
    /// Rejects foreign/missing/duplicate/insufficient signers, selectors, full originals or expiry.
    pub fn authenticate(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        now: u64,
    ) -> Result<KagemushaAuthenticatedGovernedPlatformOriginalsV1, String> {
        policy.recheck_at_trusted_time(now)?;
        let selected = policy.policy();
        let s = &self.subject;
        let message = s.approval_signing_bytes()?;
        let anchors: KagemushaOrdinaryAppIdentityAuthorityPolicyV1 =
            norito::decode_canonical_with_limits(
                policy.authority_original(),
                norito::canonical_decode_limits(policy.authority_original().len()),
            )
            .map_err(|e| e.to_string())?;
        anchors.validate()?;
        if s.authority_set_id != selected.authority_set_id
            || s.authority_set_id != anchors.authority_set_id
            || s.network_id != *selected.network_id.as_bytes()
            || s.roots.platform_class != selected.profile.platform_class
            || s.roots.canonical_digest() != Ok(selected.trust.platform_trust_roots_digest)
            || s.evaluation.canonical_digest()
                != Ok(selected.trust.platform_revocation_policy_digest)
            || now < s.valid_from_ms
            || now >= s.expires_at_ms
            || self.approvals.len() < usize::from(anchors.threshold)
            || self.approvals.len() > anchors.authorized_signers.len()
            || !self
                .approvals
                .windows(2)
                .all(|p| p[0].public_key < p[1].public_key)
        {
            return Err("governed original selector/threshold/interval differs".into());
        }
        if let KagemushaPlatformEvaluationOriginalsV1::AppleReceiptRisk(p) = &s.evaluation
            && <[u8; 32]>::from(Sha256::digest(&p.app_id_utf8))
                != selected.app_signing_identity_digest
        {
            return Err("Apple governed App ID differs".into());
        }
        for a in &self.approvals {
            if a.public_key.algorithm() != Algorithm::Ed25519
                || a.signature.payload().len() != 64
                || !anchors.authorized_signers.contains(&a.public_key)
            {
                return Err("governed original signer differs".into());
            }
            a.signature
                .verify(&a.public_key, &message)
                .map_err(|_| "governed original signature rejected")?;
        }
        let checked = KagemushaAuthenticatedGovernedPlatformOriginalsV1 {
            snapshot: self.clone(),
            original: encode(self)?,
            identity_policy_original: policy.original().to_vec(),
            identity_authority_original: policy.authority_original().to_vec(),
            authenticated_at_ms: now,
        };
        checked.recheck_current(policy, now)?;
        Ok(checked)
    }
}
impl KagemushaAuthenticatedGovernedPlatformOriginalsV1 {
    /// Complete original signed snapshot for durable issuer retention, not a root ID receipt.
    #[must_use]
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Complete ordered approved platform DER anchors.
    #[must_use]
    pub fn platform_roots_der(&self) -> &[Vec<u8>] {
        &self.snapshot.subject.roots.platform_roots_der
    }
    /// Complete separately governed Apple receipt-signing DER anchors.
    #[must_use]
    pub fn apple_receipt_roots_der(&self) -> &[Vec<u8>] {
        &self.snapshot.subject.roots.apple_receipt_roots_der
    }
    /// Borrow all original status/risk-policy bytes for actual evaluation.
    #[must_use]
    pub fn evaluation_originals(&self) -> &KagemushaPlatformEvaluationOriginalsV1 {
        &self.snapshot.subject.evaluation
    }
    /// Recheck current full policy/root configuration and original snapshot/evaluation expiry.
    /// # Errors
    /// Rejects policy/root drift, trusted-time regression or any expired original interval.
    pub fn recheck_current(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        now: u64,
    ) -> Result<(), String> {
        policy.recheck_at_trusted_time(now)?;
        let s = &self.snapshot.subject;
        if self.identity_policy_original != policy.original()
            || self.identity_authority_original != policy.authority_original()
            || now < self.authenticated_at_ms
            || now < s.valid_from_ms
            || now >= s.expires_at_ms
        {
            return Err("governed current original authority/time differs".into());
        }
        if let KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation {
            canonical_snapshot_original,
            ..
        } = &s.evaluation
        {
            AndroidSnapshot::decode(canonical_snapshot_original)?.validate_at(now)?;
        }
        Ok(())
    }
    /// Check actual certificate serial/TBS against the approved complete Android status snapshot.
    /// # Errors
    /// Rejects foreign platform/scope/time, noncanonical serial or an actual governed deny entry.
    pub fn require_android_certificate_status(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
        serial_lowercase_hex: &str,
        tbs_sha256: [u8; 32],
        now: u64,
    ) -> Result<(), String> {
        self.recheck_current(policy, now)?;
        let KagemushaPlatformEvaluationOriginalsV1::AndroidRevocation {
            canonical_snapshot_original,
            ..
        } = &self.snapshot.subject.evaluation
        else {
            return Err("Android governed evaluation absent".into());
        };
        if !canonical_serial(serial_lowercase_hex) {
            return Err("Android certificate serial noncanonical".into());
        }
        let s = AndroidSnapshot::decode(canonical_snapshot_original)?;
        if s.serials.iter().any(|v| v == serial_lowercase_hex)
            || s.tbs_digests.contains(&tbs_sha256)
        {
            return Err("Android original certificate revoked".into());
        }
        Ok(())
    }
}
fn encode<T: Encode + norito::NoritoSchema>(v: &T) -> Result<Vec<u8>, String> {
    if norito::canonical_frame_len(v).map_err(|e| e.to_string())?
        > KAGEMUSHA_GOVERNED_PLATFORM_ORIGINALS_MAX_BYTES_V1
    {
        return Err("governed original archive bound".into());
    }
    norito::encode_canonical(v).map_err(|e| e.to_string())
}
fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    h.finalize().into()
}
#[path = "kagemusha_android_revocation_snapshot_original_v1.rs"]
mod android_snapshot;
use android_snapshot::{AndroidSnapshot, canonical_serial};
#[cfg(test)]
#[path = "kagemusha_governed_platform_originals_v1_tests.rs"]
mod tests;
