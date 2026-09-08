//! Canonical, independently authenticated hardware signer custody qualification.
//!
//! This verifies an attestation authority's statement about a hardware-generated,
//! non-exportable key. The authority must independently validate the actual device/vendor
//! evidence; a signature from the role key is never custody evidence. Only public bindings and
//! evidence digests cross this boundary. Vendor evidence, credentials and key material stay in
//! the attestation service. Verification uses caller-supplied trust, time and authenticated
//! finalized state, never values discovered from the candidate record or the process clock.
//!
//! TODO: Wire this verifier into opaque hardware key operations, authenticated signer receipts,
//! daemon startup and the atomic SoraFS promotion hard cut. Persist advancement of the exact
//! predecessor through an authoritative CAS before using a verified result. Remove the SoraFS
//! software service/naming at that hard cut while retaining non-SoraFS consensus functionality.
//! This module alone neither proves a deployed HSM nor makes a software receipt hardware-qualified.

use super::protocol::{
    SoftwareSignerKeyAlgorithmV1, SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1,
    digest_parts, valid_identity,
};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use iroha_data_model::{ChainId, NetworkId};
use norito::codec::{Decode, Encode};
use std::fmt;

/// Maximum complete canonical custody record, checked before decoding.
pub const HARDWARE_SIGNER_CUSTODY_MAX_BYTES_V1: usize = 16 * 1024;
/// Domain of exact header-bearing canonical custody statement signing bytes.
pub const HARDWARE_SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:sorafs:hardware-signer-custody:v1\0";
/// Exact format marker for the sole first-release custody statement.
pub const HARDWARE_SIGNER_CUSTODY_MAGIC_V1: [u8; 8] = *b"IRHSCU01";
/// Exact first-release custody statement version.
pub const HARDWARE_SIGNER_CUSTODY_VERSION_V1: u16 = 1;
const CUSTODY_RECORD_DIGEST_DOMAIN_V1: &[u8] = b"iroha.sorafs.hardware-signer-custody.record.v1";
const MAX_HANDLE_BYTES_V1: usize = 128;
const MAX_VALIDITY_MS_V1: u64 = 24 * 60 * 60 * 1000;

/// Exact public signer identity to which an independent custody statement applies.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct HardwareSignerCustodyBindingV1 {
    /// Canonical operator-selected chain label.
    pub chain_id: ChainId,
    /// Exact genesis-derived network identity, independent of the chain label.
    pub network_id: NetworkId,
    /// Public runtime-provider handle; never a vendor URI containing credentials.
    pub runtime_handle: String,
    /// Public hardware key-generation handle resolved only by the deployment adapter.
    pub key_handle: String,
    /// Public signer service identity.
    pub service_id: String,
    /// Independent signer administrator identity.
    pub administrator_id: String,
    /// Existing canonical role owner; its software naming is removed at the production hard cut.
    pub role: SoftwareSignerRoleV1,
    /// Exact publisher/provider/issuer authority for the role.
    pub purpose: SoftwareSignerPurposeBindingV1,
    /// Exact permitted signature algorithm.
    pub algorithm: SoftwareSignerKeyAlgorithmV1,
    /// Exact public key whose hardware generation and non-exportability were attested.
    pub public_key: PublicKey,
    /// Nonzero governed key generation.
    pub key_revision: u64,
    /// Nonzero governed signing-policy generation.
    pub policy_revision: u64,
    /// Digest of the exact public signing policy.
    pub policy_digest: [u8; 32],
}
impl fmt::Debug for HardwareSignerCustodyBindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HardwareSignerCustodyBindingV1")
            .field("role", &self.role)
            .field("key_revision", &self.key_revision)
            .finish_non_exhaustive()
    }
}

/// Independently governed identity of the authority validating vendor/device evidence.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct HardwareSignerCustodyAuthorityV1 {
    /// Public attestation service identity, distinct from the signer identities.
    pub service_id: String,
    /// Public independent attestation administrator identity.
    pub administrator_id: String,
    /// Governed generation of the separate attestation key.
    pub key_revision: u64,
    /// Governed attestation-policy generation.
    pub policy_revision: u64,
    /// Digest of the policy for verifying actual device evidence.
    pub policy_digest: [u8; 32],
}
impl fmt::Debug for HardwareSignerCustodyAuthorityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HardwareSignerCustodyAuthorityV1")
            .field("key_revision", &self.key_revision)
            .field("policy_revision", &self.policy_revision)
            .finish_non_exhaustive()
    }
}

/// Exact independently authenticated finalized state used for qualification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct HardwareSignerCustodyAnchorV1 {
    /// Nonzero finalized block height.
    pub height: u64,
    /// Exact finalized block hash.
    pub block_hash: [u8; 32],
    /// Digest of authoritative custody policy, revocation and predecessor state at that block.
    pub state_digest: [u8; 32],
}

/// Statement signed by the independent attestation authority.
///
/// There is exactly one admitted profile: generation inside hardware with no exportability or
/// prior export. The booleans describe observations authenticated by the independent authority;
/// they cannot replace device-evidence verification performed by that authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct HardwareSignerCustodyStatementV1 {
    /// Canonical record format marker [`HARDWARE_SIGNER_CUSTODY_MAGIC_V1`].
    pub magic: [u8; 8],
    /// Canonical first-release format version.
    pub version: u16,
    /// Exact key, role, network and signing-policy subject.
    pub binding: HardwareSignerCustodyBindingV1,
    /// Independent authority identity, never a candidate-selected trust key.
    pub authority: HardwareSignerCustodyAuthorityV1,
    /// Current finalized state against which the authority qualified this key.
    pub anchor: HardwareSignerCustodyAnchorV1,
    /// Exact monotonic qualification sequence expected by the authoritative state.
    pub sequence: u64,
    /// Previous complete signed record digest; zero only for sequence one.
    pub predecessor_digest: [u8; 32],
    /// Beginning of the signed validity interval, in Unix milliseconds.
    pub issued_at_unix_ms: u64,
    /// Exclusive end of the signed validity interval, in Unix milliseconds.
    pub expires_at_unix_ms: u64,
    /// Digest of the hardware module identity verified from device evidence.
    pub hardware_identity_digest: [u8; 32],
    /// Digest of the exact runtime-only vendor/device evidence.
    pub evidence_digest: [u8; 32],
    /// Must be true: importing an exported software key does not qualify.
    pub generated_in_hardware: bool,
    /// Must be false under the sole admitted non-exportable profile.
    pub exportable: bool,
    /// Must be false: no earlier cleartext/exportable copy may have existed.
    pub ever_exported: bool,
    /// Must be false; the authoritative current context also checks revocation.
    pub revoked: bool,
}
impl HardwareSignerCustodyStatementV1 {
    /// Return exact domain-separated bytes for the attestation authority to sign.
    ///
    /// # Errors
    /// Rejects malformed, oversized, revoked or non-hardware/non-exportable statements.
    pub fn signing_payload(&self) -> Result<Vec<u8>, HardwareSignerCustodyErrorV1> {
        validate_statement(self)?;
        let encoded = norito::encode_canonical(self)
            .map_err(|_| HardwareSignerCustodyErrorV1::InvalidRecord)?;
        if encoded.len() > HARDWARE_SIGNER_CUSTODY_MAX_BYTES_V1 {
            return Err(HardwareSignerCustodyErrorV1::InvalidRecord);
        }
        let mut payload =
            Vec::with_capacity(HARDWARE_SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1.len() + encoded.len());
        payload.extend_from_slice(HARDWARE_SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1);
        payload.extend_from_slice(&encoded);
        Ok(payload)
    }
}
impl fmt::Debug for HardwareSignerCustodyStatementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HardwareSignerCustodyStatementV1")
            .field("sequence", &self.sequence)
            .field("revoked", &self.revoked)
            .finish_non_exhaustive()
    }
}

/// Complete bounded canonical Norito custody record with an Ed25519 attestation.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct HardwareSignerCustodyRecordV1 {
    /// Exact independent-authority statement.
    pub statement: HardwareSignerCustodyStatementV1,
    /// Raw 64-byte Ed25519 signature; the trusted public key is supplied separately.
    pub attestation: [u8; 64],
}
impl fmt::Debug for HardwareSignerCustodyRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HardwareSignerCustodyRecordV1")
            .field("sequence", &self.statement.sequence)
            .finish_non_exhaustive()
    }
}

/// Caller-owned attestation trust, loaded independently from governed configuration.
///
/// Never construct this by copying fields out of an untrusted custody record.
#[derive(Clone, Debug)]
pub struct HardwareSignerCustodyTrustV1 {
    /// Exact independently governed authority identity and policy.
    pub authority: HardwareSignerCustodyAuthorityV1,
    /// Independently pinned Ed25519 attestation key, distinct from the role key.
    pub public_key: PublicKey,
    /// Inclusive beginning of attestation-key eligibility.
    pub active_from_unix_ms: u64,
    /// Exclusive end of attestation-key eligibility.
    pub active_until_unix_ms: u64,
    /// Maximum signed validity interval; positive and at most 24 hours.
    pub max_validity_ms: u64,
    /// Maximum age of independently observed current state; positive and at most 24 hours.
    pub max_anchor_age_ms: u64,
}

/// Explicit verification inputs obtained from independently authenticated current state.
///
/// The caller must bind the expected signer configuration, sequence, predecessor and revocation
/// flags to `current_anchor.state_digest`. Replays are rejected against `next_sequence`; the
/// verifier is read-only and does not advance that durable authoritative state itself.
#[derive(Clone, Copy, Debug)]
pub struct HardwareSignerCustodyVerificationContextV1 {
    /// Explicit trusted time; this verifier never reads the process clock.
    pub now_unix_ms: u64,
    /// Time the caller independently observed the current authenticated state.
    pub anchor_observed_at_unix_ms: u64,
    /// Exact current finalized state, not a self-asserted candidate anchor.
    pub current_anchor: HardwareSignerCustodyAnchorV1,
    /// Next sequence from the current authoritative custody head.
    pub next_sequence: u64,
    /// Exact predecessor digest from that head; zero only for the initial record.
    pub predecessor_digest: [u8; 32],
    /// Current authoritative role-key revocation state.
    pub signer_revoked: bool,
    /// Current authoritative attestation-key revocation state.
    pub attester_revoked: bool,
}

/// Successfully verified observation of one exact record and current context.
///
/// No constructor, deserializer, decoder or default is exposed. This is a time-bound observation,
/// not a reusable authorization token: callers must revalidate current state before key use.
pub struct VerifiedSignerCustodyV1 {
    statement: HardwareSignerCustodyStatementV1,
    record_digest: [u8; 32],
    verified_at_unix_ms: u64,
}
impl VerifiedSignerCustodyV1 {
    /// Borrow the exact independently authenticated statement.
    #[must_use]
    pub fn statement(&self) -> &HardwareSignerCustodyStatementV1 {
        &self.statement
    }
    /// Digest to atomically append as the authoritative predecessor after verification.
    #[must_use]
    pub const fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }
    /// Explicit trusted time at which this observation was verified.
    #[must_use]
    pub const fn verified_at_unix_ms(&self) -> u64 {
        self.verified_at_unix_ms
    }
}
impl fmt::Debug for VerifiedSignerCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedSignerCustodyV1")
            .field("sequence", &self.statement.sequence)
            .field("verified_at_unix_ms", &self.verified_at_unix_ms)
            .finish_non_exhaustive()
    }
}

/// Fixed failure classes; no record contents, handles or provider diagnostics are retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareSignerCustodyErrorV1 {
    /// Canonical framing, shape, resource bounds or public identities are invalid.
    InvalidRecord,
    /// The only admitted hardware-generated, never-exported profile was not met.
    HardwareCustodyRequired,
    /// The exact independently supplied signer binding does not match.
    BindingMismatch,
    /// The independent authority identity, policy, key or trust interval is invalid.
    UntrustedAuthority,
    /// Role-key signatures and identities cannot independently attest custody.
    SelfAttestation,
    /// The independent attestation signature is malformed or invalid.
    InvalidAttestation,
    /// The record, current anchor or trust key is stale, expired or from the future.
    Freshness,
    /// The record is not bound to the exact independently authenticated current state.
    AnchorMismatch,
    /// The qualification sequence or predecessor disagrees with authoritative state.
    ReplayOrRollback,
    /// Either the hardware key or its attestation authority is currently revoked.
    Revoked,
}
impl fmt::Display for HardwareSignerCustodyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRecord => "invalid hardware custody record",
            Self::HardwareCustodyRequired => "non-exportable hardware custody is required",
            Self::BindingMismatch => "hardware custody signer binding mismatch",
            Self::UntrustedAuthority => "hardware custody authority is untrusted",
            Self::SelfAttestation => "hardware custody authority is not independent",
            Self::InvalidAttestation => "hardware custody attestation is invalid",
            Self::Freshness => "hardware custody freshness check failed",
            Self::AnchorMismatch => "hardware custody finalized anchor mismatch",
            Self::ReplayOrRollback => "hardware custody sequence or predecessor mismatch",
            Self::Revoked => "hardware custody key or authority is revoked",
        })
    }
}
impl std::error::Error for HardwareSignerCustodyErrorV1 {}

/// Verify one canonical record against an independently supplied exact signer and authority.
///
/// # Errors
/// Rejects invalid framing/bounds, software or exported keys, self-attestation, binding/authority
/// substitution, invalid signatures, stale or future data, revocation and predecessor replay.
pub fn verify_hardware_signer_custody_v1(
    bytes: &[u8],
    expected_binding: &HardwareSignerCustodyBindingV1,
    trust: &HardwareSignerCustodyTrustV1,
    context: &HardwareSignerCustodyVerificationContextV1,
) -> Result<VerifiedSignerCustodyV1, HardwareSignerCustodyErrorV1> {
    if bytes.is_empty() || bytes.len() > HARDWARE_SIGNER_CUSTODY_MAX_BYTES_V1 {
        return Err(HardwareSignerCustodyErrorV1::InvalidRecord);
    }
    let record: HardwareSignerCustodyRecordV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(4096, 4096, 8192, 256 * 1024, 16),
    )
    .map_err(|_| HardwareSignerCustodyErrorV1::InvalidRecord)?;
    validate_statement(&record.statement)?;
    validate_binding(expected_binding)?;
    if record.statement.binding != *expected_binding {
        return Err(HardwareSignerCustodyErrorV1::BindingMismatch);
    }
    validate_trust(&record.statement, trust)?;
    validate_context(&record.statement, trust, context)?;
    Signature::try_from_bytes(&record.attestation)
        .map_err(|_| HardwareSignerCustodyErrorV1::InvalidAttestation)?
        .verify(&trust.public_key, &record.statement.signing_payload()?)
        .map_err(|_| HardwareSignerCustodyErrorV1::InvalidAttestation)?;
    Ok(VerifiedSignerCustodyV1 {
        statement: record.statement,
        record_digest: digest_parts(CUSTODY_RECORD_DIGEST_DOMAIN_V1, &[bytes]),
        verified_at_unix_ms: context.now_unix_ms,
    })
}

fn valid_hardware_handle(value: &str) -> bool {
    value.len() <= MAX_HANDLE_BYTES_V1
        && iroha_config::parameters::validate_production_runtime_handle(value).is_ok()
        && value
            .split_once(':')
            .is_some_and(|(scheme, _)| matches!(scheme, "hsm" | "kms" | "pkcs11"))
}

fn validate_binding(
    binding: &HardwareSignerCustodyBindingV1,
) -> Result<(), HardwareSignerCustodyErrorV1> {
    if binding.network_id.as_bytes() == &[0; 32]
        || !valid_hardware_handle(&binding.runtime_handle)
        || !valid_hardware_handle(&binding.key_handle)
        || !valid_identity(&binding.service_id)
        || !valid_identity(&binding.administrator_id)
        || binding.service_id == binding.administrator_id
        || !binding.purpose.validates_role(binding.role)
        || !binding.role.allows_algorithm(binding.algorithm)
        || binding.public_key.try_algorithm().ok() != Some(binding.algorithm.algorithm())
        || binding.key_revision == 0
        || binding.policy_revision == 0
        || binding.policy_digest == [0; 32]
    {
        return Err(HardwareSignerCustodyErrorV1::InvalidRecord);
    }
    Ok(())
}

fn valid_authority(authority: &HardwareSignerCustodyAuthorityV1) -> bool {
    valid_identity(&authority.service_id)
        && valid_identity(&authority.administrator_id)
        && authority.service_id != authority.administrator_id
        && authority.key_revision != 0
        && authority.policy_revision != 0
        && authority.policy_digest != [0; 32]
}

fn valid_anchor(anchor: HardwareSignerCustodyAnchorV1) -> bool {
    anchor.height != 0 && anchor.block_hash != [0; 32] && anchor.state_digest != [0; 32]
}

fn validate_statement(
    statement: &HardwareSignerCustodyStatementV1,
) -> Result<(), HardwareSignerCustodyErrorV1> {
    validate_binding(&statement.binding)?;
    if statement.magic != HARDWARE_SIGNER_CUSTODY_MAGIC_V1
        || statement.version != HARDWARE_SIGNER_CUSTODY_VERSION_V1
        || !valid_authority(&statement.authority)
        || !valid_anchor(statement.anchor)
        || statement.hardware_identity_digest == [0; 32]
        || statement.evidence_digest == [0; 32]
        || statement.sequence == 0
        || (statement.sequence == 1) != (statement.predecessor_digest == [0; 32])
        || statement.issued_at_unix_ms == 0
        || statement.expires_at_unix_ms <= statement.issued_at_unix_ms
        || statement.expires_at_unix_ms - statement.issued_at_unix_ms > MAX_VALIDITY_MS_V1
    {
        return Err(HardwareSignerCustodyErrorV1::InvalidRecord);
    }
    if !statement.generated_in_hardware || statement.exportable || statement.ever_exported {
        return Err(HardwareSignerCustodyErrorV1::HardwareCustodyRequired);
    }
    if statement.revoked {
        return Err(HardwareSignerCustodyErrorV1::Revoked);
    }
    Ok(())
}

fn validate_trust(
    statement: &HardwareSignerCustodyStatementV1,
    trust: &HardwareSignerCustodyTrustV1,
) -> Result<(), HardwareSignerCustodyErrorV1> {
    if !valid_authority(&trust.authority)
        || statement.authority != trust.authority
        || trust.public_key.try_algorithm().ok() != Some(Algorithm::Ed25519)
        || trust.active_from_unix_ms == 0
        || trust.active_until_unix_ms <= trust.active_from_unix_ms
        || trust.max_validity_ms == 0
        || trust.max_validity_ms > MAX_VALIDITY_MS_V1
        || trust.max_anchor_age_ms == 0
        || trust.max_anchor_age_ms > MAX_VALIDITY_MS_V1
    {
        return Err(HardwareSignerCustodyErrorV1::UntrustedAuthority);
    }
    let signer = &statement.binding;
    if trust.public_key == signer.public_key
        || [
            &trust.authority.service_id,
            &trust.authority.administrator_id,
        ]
        .iter()
        .any(|identity| **identity == signer.service_id || **identity == signer.administrator_id)
    {
        return Err(HardwareSignerCustodyErrorV1::SelfAttestation);
    }
    Ok(())
}

fn validate_context(
    statement: &HardwareSignerCustodyStatementV1,
    trust: &HardwareSignerCustodyTrustV1,
    context: &HardwareSignerCustodyVerificationContextV1,
) -> Result<(), HardwareSignerCustodyErrorV1> {
    if context.signer_revoked || context.attester_revoked {
        return Err(HardwareSignerCustodyErrorV1::Revoked);
    }
    if !valid_anchor(context.current_anchor) || statement.anchor != context.current_anchor {
        return Err(HardwareSignerCustodyErrorV1::AnchorMismatch);
    }
    if context.next_sequence == 0
        || (context.next_sequence == 1) != (context.predecessor_digest == [0; 32])
        || statement.sequence != context.next_sequence
        || statement.predecessor_digest != context.predecessor_digest
    {
        return Err(HardwareSignerCustodyErrorV1::ReplayOrRollback);
    }
    let now = context.now_unix_ms;
    if now == 0
        || context.anchor_observed_at_unix_ms == 0
        || context.anchor_observed_at_unix_ms > now
        || now - context.anchor_observed_at_unix_ms > trust.max_anchor_age_ms
        || statement.issued_at_unix_ms > now
        || statement.expires_at_unix_ms <= now
        || statement.issued_at_unix_ms < trust.active_from_unix_ms
        || statement.expires_at_unix_ms > trust.active_until_unix_ms
        || now < trust.active_from_unix_ms
        || now >= trust.active_until_unix_ms
        || statement.expires_at_unix_ms - statement.issued_at_unix_ms > trust.max_validity_ms
    {
        return Err(HardwareSignerCustodyErrorV1::Freshness);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
