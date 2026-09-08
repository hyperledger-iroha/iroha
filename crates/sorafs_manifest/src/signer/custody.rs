//! Canonical, independently authenticated hardware signer custody qualification.
//!
//! This verifies an attestation authority's statement about a hardware-generated,
//! non-exportable key. The authority must independently validate the actual device/vendor
//! evidence; a signature from the role key is never custody evidence. Only public bindings and
//! evidence digests cross this boundary. Vendor evidence, credentials and key material stay in
//! the attestation service. Verification uses caller-supplied trust, time and authenticated
//! finalized state, never values discovered from the candidate record or the process clock.
//!
//! The daemon's opaque-operation coordinator and canonical release receipts consume this verifier.
//! Enrollment admission must advance through authoritative CAS exactly once; ordinary use checks
//! the already active record with fresh independent state. Provider I/O and release are fenced by
//! that daemon boundary, not by possession of a public attestation alone.
//!
//! TODO: Finish the remaining SoraFS runtime/receipt consumers and production hardware/state
//! adapters, then remove the software service path while preserving non-SoraFS consensus custody.
//! This module alone neither proves a deployed HSM nor makes a software receipt hardware-qualified.

use super::protocol::{
    SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1, digest_parts, valid_identity,
};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use std::fmt;

/// Maximum complete canonical custody record, checked before decoding.
pub const SIGNER_CUSTODY_MAX_BYTES_V1: usize = 16 * 1024;
/// Domain of exact header-bearing canonical custody statement signing bytes.
pub const SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:sorafs:hardware-signer-custody:v1\0";
/// Exact format marker for the sole first-release custody statement.
pub const SIGNER_CUSTODY_MAGIC_V1: [u8; 8] = *b"IRHSCU01";
/// Exact first-release custody statement version.
pub const SIGNER_CUSTODY_VERSION_V1: u16 = 1;
pub(super) const CUSTODY_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.sorafs.hardware-signer-custody.record.v1";
const MAX_HANDLE_BYTES_V1: usize = 128;
const MAX_VALIDITY_MS_V1: u64 = 24 * 60 * 60 * 1000;

/// Exact public signer identity to which an independent custody statement applies.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerCustodyBindingV1 {
    /// Canonical operator-selected chain label.
    pub chain_id: String,
    /// Exact genesis-derived network identity, independent of the chain label.
    pub network_id: [u8; 32],
    /// Public runtime-provider handle; never a vendor URI containing credentials.
    pub runtime_handle: String,
    /// Public hardware key-generation handle resolved only by the deployment adapter.
    pub key_handle: String,
    /// Public signer service identity.
    pub service_id: String,
    /// Independent signer administrator identity.
    pub administrator_id: String,
    /// Canonical role owner, independent of any runtime provider implementation.
    pub role: SignerRoleV1,
    /// Exact publisher/provider/issuer authority for the role.
    pub purpose: SignerPurposeBindingV1,
    /// Exact permitted signature algorithm.
    pub algorithm: SignerKeyAlgorithmV1,
    /// Exact public key whose hardware generation and non-exportability were attested.
    pub public_key: PublicKey,
    /// Nonzero governed key generation.
    pub key_revision: u64,
    /// Nonzero governed signing-policy generation.
    pub policy_revision: u64,
    /// Digest of the exact public signing policy.
    pub policy_digest: [u8; 32],
}
impl fmt::Debug for SignerCustodyBindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerCustodyBindingV1")
            .field("role", &self.role)
            .field("key_revision", &self.key_revision)
            .finish_non_exhaustive()
    }
}

/// Independently governed identity of the authority validating vendor/device evidence.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerCustodyAuthorityV1 {
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
impl fmt::Debug for SignerCustodyAuthorityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerCustodyAuthorityV1")
            .field("key_revision", &self.key_revision)
            .field("policy_revision", &self.policy_revision)
            .finish_non_exhaustive()
    }
}

/// Exact independently authenticated finalized per-role custody state used for qualification.
///
/// Both original approval and current use anchors must identify genuinely finalized state.
/// Mutable external state cannot be assigned the hash or height of an unrelated finalized block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerCustodyAnchorV1 {
    /// Nonzero finalized block height.
    pub height: u64,
    /// Exact finalized block hash.
    pub block_hash: [u8; 32],
    /// Digest of this role's authoritative custody policy, revocations and enrollment head.
    ///
    /// This excludes operation reservations and the signing/audit journal, which necessarily
    /// advance during a signing operation. Other roles' unrelated custody changes are excluded.
    pub state_digest: [u8; 32],
}

/// Statement signed by the independent attestation authority.
///
/// There is exactly one admitted profile: generation inside hardware with no exportability or
/// prior export. The booleans describe observations authenticated by the independent authority;
/// they cannot replace device-evidence verification performed by that authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerCustodyStatementV1 {
    /// Canonical record format marker [`SIGNER_CUSTODY_MAGIC_V1`].
    pub magic: [u8; 8],
    /// Canonical first-release format version.
    pub version: u16,
    /// Exact key, role, network and signing-policy subject.
    pub binding: SignerCustodyBindingV1,
    /// Independent authority identity, never a candidate-selected trust key.
    pub authority: SignerCustodyAuthorityV1,
    /// Current finalized state against which the authority qualified this key.
    pub anchor: SignerCustodyAnchorV1,
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
impl SignerCustodyStatementV1 {
    /// Return exact domain-separated bytes for the attestation authority to sign.
    ///
    /// # Errors
    /// Rejects malformed, oversized, revoked or non-hardware/non-exportable statements.
    pub fn signing_payload(&self) -> Result<Vec<u8>, SignerCustodyErrorV1> {
        validate_statement(self)?;
        let encoded =
            norito::encode_canonical(self).map_err(|_| SignerCustodyErrorV1::InvalidRecord)?;
        if encoded.len() > SIGNER_CUSTODY_MAX_BYTES_V1 {
            return Err(SignerCustodyErrorV1::InvalidRecord);
        }
        let mut payload =
            Vec::with_capacity(SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1.len() + encoded.len());
        payload.extend_from_slice(SIGNER_CUSTODY_SIGNATURE_DOMAIN_V1);
        payload.extend_from_slice(&encoded);
        Ok(payload)
    }
}
impl fmt::Debug for SignerCustodyStatementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerCustodyStatementV1")
            .field("sequence", &self.sequence)
            .field("revoked", &self.revoked)
            .finish_non_exhaustive()
    }
}

/// Complete bounded canonical Norito custody record with an Ed25519 attestation.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerCustodyRecordV1 {
    /// Exact independent-authority statement.
    pub statement: SignerCustodyStatementV1,
    /// Raw 64-byte Ed25519 signature; the trusted public key is supplied separately.
    pub attestation: [u8; 64],
}
impl fmt::Debug for SignerCustodyRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerCustodyRecordV1")
            .field("sequence", &self.statement.sequence)
            .finish_non_exhaustive()
    }
}

/// Caller-owned attestation trust, loaded independently from governed configuration.
///
/// Never construct this by copying fields out of an untrusted custody record.
#[derive(Clone, Debug)]
pub struct SignerCustodyTrustV1 {
    /// Exact independently governed authority identity and policy.
    pub authority: SignerCustodyAuthorityV1,
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

/// Explicit enrollment inputs obtained from independently authenticated current state.
///
/// The caller must bind the expected signer configuration, sequence, predecessor and revocation
/// flags to `current_anchor.state_digest`. Replays are rejected against `next_sequence`; the
/// verifier is read-only and does not advance that durable authoritative state itself.
/// These inputs admit a new record only. Ordinary use of an already activated record has a
/// separate [`SignerCustodyUseContextV1`] and never repeats next-slot admission.
#[derive(Clone, Copy, Debug)]
pub struct SignerCustodyEnrollmentContextV1 {
    /// Explicit trusted time; this verifier never reads the process clock.
    pub now_unix_ms: u64,
    /// Time the caller independently observed the current authenticated state.
    pub anchor_observed_at_unix_ms: u64,
    /// Exact current finalized state, not a self-asserted candidate anchor.
    pub current_anchor: SignerCustodyAnchorV1,
    /// Next sequence from the current authoritative custody head.
    pub next_sequence: u64,
    /// Exact predecessor digest from that head; zero only for the initial record.
    pub predecessor_digest: [u8; 32],
    /// Current authoritative role-key revocation state.
    pub signer_revoked: bool,
    /// Current authoritative attestation-key revocation state.
    pub attester_revoked: bool,
}

/// One exact already-enrolled active custody head from independently authenticated state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerCustodyActiveHeadV1 {
    /// Exact complete canonical signed record digest committed by enrollment CAS.
    pub record_digest: [u8; 32],
    /// Exact sequence of that already committed record.
    pub sequence: u64,
    /// Finalized anchor against which enrollment originally approved that record.
    pub approved_anchor: SignerCustodyAnchorV1,
    /// Exact currently active key generation.
    pub key_revision: u64,
    /// Exact currently active signing-policy generation.
    pub policy_revision: u64,
    /// Exact currently active signing-policy digest.
    pub policy_digest: [u8; 32],
}

/// Explicit inputs for ordinary use of an already-enrolled active hardware key.
///
/// The caller authenticates `active_head` and both revocation flags under
/// `current_anchor.state_digest` independently of the candidate record. That digest covers this
/// role's custody control state, excluding operation reservations and the signing/audit journal.
/// The original approval anchor remains fixed, while the current finalized anchor may advance
/// without re-attestation. Both anchors must refer to genuinely finalized custody state.
#[derive(Clone, Copy, Debug)]
pub struct SignerCustodyUseContextV1 {
    /// Explicit trusted time, never sourced from the record or process clock by this verifier.
    pub now_unix_ms: u64,
    /// Time the caller independently observed current authenticated custody state.
    pub anchor_observed_at_unix_ms: u64,
    /// Fresh current finalized custody state, which may be later than original approval.
    pub current_anchor: SignerCustodyAnchorV1,
    /// Exact already-enrolled active record and key/policy generation.
    pub active_head: SignerCustodyActiveHeadV1,
    /// Current authoritative role-key revocation state.
    pub signer_revoked: bool,
    /// Current authoritative attestation-key revocation state.
    pub attester_revoked: bool,
}

/// Successfully verified enrollment candidate for one exact predecessor slot.
///
/// No constructor, deserializer, decoder or default is exposed. This result authorizes only
/// attempting authoritative enrollment CAS; it cannot be used as an ordinary key-use observation.
pub struct VerifiedSignerCustodyEnrollmentV1 {
    statement: SignerCustodyStatementV1,
    record_digest: [u8; 32],
    verified_at_unix_ms: u64,
}
impl VerifiedSignerCustodyEnrollmentV1 {
    /// Borrow the exact independently authenticated statement.
    #[must_use]
    pub fn statement(&self) -> &SignerCustodyStatementV1 {
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
impl fmt::Debug for VerifiedSignerCustodyEnrollmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedSignerCustodyEnrollmentV1")
            .field("sequence", &self.statement.sequence)
            .field("verified_at_unix_ms", &self.verified_at_unix_ms)
            .finish_non_exhaustive()
    }
}

/// Verified observation that one exact already-enrolled record is currently eligible for use.
///
/// This cannot be constructed, cloned or deserialized by a caller. It does not itself reserve an
/// operation or authorize signature release: the caller must durably reserve the operation and
/// verify current state again after provider I/O, rejecting any active-state drift.
pub struct VerifiedSignerCustodyV1 {
    statement: SignerCustodyStatementV1,
    record_digest: [u8; 32],
    verified_at_unix_ms: u64,
    current_anchor: SignerCustodyAnchorV1,
}
impl VerifiedSignerCustodyV1 {
    /// Borrow the exact independently authenticated active statement.
    #[must_use]
    pub fn statement(&self) -> &SignerCustodyStatementV1 {
        &self.statement
    }
    /// Exact active enrolled record digest to bind into the operation and its response.
    #[must_use]
    pub const fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }
    /// Explicit trusted verification time.
    #[must_use]
    pub const fn verified_at_unix_ms(&self) -> u64 {
        self.verified_at_unix_ms
    }
    /// Exact current finalized anchor authenticated by this use observation.
    #[must_use]
    pub const fn current_anchor(&self) -> SignerCustodyAnchorV1 {
        self.current_anchor
    }
    /// Whether this post-I/O observation continues the same active state as `previous`.
    ///
    /// Finalized height may advance while per-role custody control state remains unchanged.
    /// Operation reservations and signing/audit appends are outside that state digest. Rotation, policy
    /// drift, revocation-state drift or another active record changes the comparison. Trusted time
    /// and finalized height cannot go backwards, and the same height cannot change block hash.
    /// A revoked key never produces a verified observation in the first place.
    #[must_use]
    pub fn continues_active_state(&self, previous: &Self) -> bool {
        self.record_digest == previous.record_digest
            && self.current_anchor.state_digest == previous.current_anchor.state_digest
            && self.verified_at_unix_ms >= previous.verified_at_unix_ms
            && self.current_anchor.height >= previous.current_anchor.height
            && (self.current_anchor.height != previous.current_anchor.height
                || self.current_anchor.block_hash == previous.current_anchor.block_hash)
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
pub enum SignerCustodyErrorV1 {
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
impl fmt::Display for SignerCustodyErrorV1 {
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
impl std::error::Error for SignerCustodyErrorV1 {}

/// Verify one canonical record against an independently supplied exact signer and authority.
///
/// # Errors
/// Rejects invalid framing/bounds, software or exported keys, self-attestation, binding/authority
/// substitution, invalid signatures, stale or future data, revocation and predecessor replay.
pub fn verify_signer_custody_enrollment_v1(
    bytes: &[u8],
    expected_binding: &SignerCustodyBindingV1,
    trust: &SignerCustodyTrustV1,
    context: &SignerCustodyEnrollmentContextV1,
) -> Result<VerifiedSignerCustodyEnrollmentV1, SignerCustodyErrorV1> {
    let record = decode_bound_record(bytes, expected_binding, trust)?;
    validate_enrollment_context(&record.statement, trust, context)?;
    verify_attestation(&record, trust)?;
    Ok(VerifiedSignerCustodyEnrollmentV1 {
        statement: record.statement,
        record_digest: digest_parts(CUSTODY_RECORD_DIGEST_DOMAIN_V1, &[bytes]),
        verified_at_unix_ms: context.now_unix_ms,
    })
}

/// Verify ordinary use of the exact already-enrolled record named by current authoritative state.
///
/// This never advances or re-admits the enrollment sequence. The original approved anchor remains
/// immutable; the fresh current state independently determines which generation is active now.
///
/// # Errors
/// Rejects invalid/non-hardware attestations, wrong bindings, inactive or substituted record heads,
/// changed key/policy generations, false approval anchors, stale state and either key's revocation.
pub fn verify_signer_custody_use_v1(
    bytes: &[u8],
    expected_binding: &SignerCustodyBindingV1,
    trust: &SignerCustodyTrustV1,
    context: &SignerCustodyUseContextV1,
) -> Result<VerifiedSignerCustodyV1, SignerCustodyErrorV1> {
    let record = decode_bound_record(bytes, expected_binding, trust)?;
    let record_digest = digest_parts(CUSTODY_RECORD_DIGEST_DOMAIN_V1, &[bytes]);
    validate_use_context(&record.statement, record_digest, trust, context)?;
    verify_attestation(&record, trust)?;
    Ok(VerifiedSignerCustodyV1 {
        statement: record.statement,
        record_digest,
        verified_at_unix_ms: context.now_unix_ms,
        current_anchor: context.current_anchor,
    })
}

fn decode_bound_record(
    bytes: &[u8],
    expected_binding: &SignerCustodyBindingV1,
    trust: &SignerCustodyTrustV1,
) -> Result<SignerCustodyRecordV1, SignerCustodyErrorV1> {
    if bytes.is_empty() || bytes.len() > SIGNER_CUSTODY_MAX_BYTES_V1 {
        return Err(SignerCustodyErrorV1::InvalidRecord);
    }
    let record: SignerCustodyRecordV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(4096, SIGNER_CUSTODY_MAX_BYTES_V1, 8192, 256 * 1024, 16),
    )
    .map_err(|_| SignerCustodyErrorV1::InvalidRecord)?;
    validate_statement(&record.statement)?;
    validate_binding(expected_binding)?;
    if record.statement.binding != *expected_binding {
        return Err(SignerCustodyErrorV1::BindingMismatch);
    }
    validate_trust(&record.statement, trust)?;
    Ok(record)
}

fn verify_attestation(
    record: &SignerCustodyRecordV1,
    trust: &SignerCustodyTrustV1,
) -> Result<(), SignerCustodyErrorV1> {
    Signature::try_from_bytes(&record.attestation)
        .map_err(|_| SignerCustodyErrorV1::InvalidAttestation)?
        .verify(&trust.public_key, &record.statement.signing_payload()?)
        .map_err(|_| SignerCustodyErrorV1::InvalidAttestation)
}

fn valid_hardware_handle(value: &str) -> bool {
    if value.len() > MAX_HANDLE_BYTES_V1
        || !value.is_ascii()
        || value
            .to_ascii_lowercase()
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|component| {
                matches!(
                    component,
                    "null" | "mock" | "test" | "dev" | "demo" | "fake" | "dummy" | "placeholder"
                )
            })
    {
        return false;
    }
    let Some((scheme, opaque)) = value.split_once(':') else {
        return false;
    };
    let opaque = opaque.strip_prefix("//").unwrap_or(opaque);
    matches!(scheme, "hsm" | "kms" | "pkcs11")
        && opaque.split('/').all(|component| {
            component.bytes().any(|byte| byte.is_ascii_alphanumeric())
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
}

pub(super) fn validate_binding(
    binding: &SignerCustodyBindingV1,
) -> Result<(), SignerCustodyErrorV1> {
    if iroha_primitives::chain_id::validate_chain_id(&binding.chain_id).is_err()
        || binding.network_id == [0; 32]
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
        return Err(SignerCustodyErrorV1::InvalidRecord);
    }
    Ok(())
}

fn valid_authority(authority: &SignerCustodyAuthorityV1) -> bool {
    valid_identity(&authority.service_id)
        && valid_identity(&authority.administrator_id)
        && authority.service_id != authority.administrator_id
        && authority.key_revision != 0
        && authority.policy_revision != 0
        && authority.policy_digest != [0; 32]
}

fn valid_anchor(anchor: SignerCustodyAnchorV1) -> bool {
    anchor.height != 0 && anchor.block_hash != [0; 32] && anchor.state_digest != [0; 32]
}

fn validate_statement(statement: &SignerCustodyStatementV1) -> Result<(), SignerCustodyErrorV1> {
    validate_binding(&statement.binding)?;
    if statement.magic != SIGNER_CUSTODY_MAGIC_V1
        || statement.version != SIGNER_CUSTODY_VERSION_V1
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
        return Err(SignerCustodyErrorV1::InvalidRecord);
    }
    if !statement.generated_in_hardware || statement.exportable || statement.ever_exported {
        return Err(SignerCustodyErrorV1::HardwareCustodyRequired);
    }
    if statement.revoked {
        return Err(SignerCustodyErrorV1::Revoked);
    }
    Ok(())
}

fn validate_trust(
    statement: &SignerCustodyStatementV1,
    trust: &SignerCustodyTrustV1,
) -> Result<(), SignerCustodyErrorV1> {
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
        return Err(SignerCustodyErrorV1::UntrustedAuthority);
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
        return Err(SignerCustodyErrorV1::SelfAttestation);
    }
    Ok(())
}

fn validate_enrollment_context(
    statement: &SignerCustodyStatementV1,
    trust: &SignerCustodyTrustV1,
    context: &SignerCustodyEnrollmentContextV1,
) -> Result<(), SignerCustodyErrorV1> {
    if context.signer_revoked || context.attester_revoked {
        return Err(SignerCustodyErrorV1::Revoked);
    }
    if !valid_anchor(context.current_anchor) || statement.anchor != context.current_anchor {
        return Err(SignerCustodyErrorV1::AnchorMismatch);
    }
    if context.next_sequence == 0
        || (context.next_sequence == 1) != (context.predecessor_digest == [0; 32])
        || statement.sequence != context.next_sequence
        || statement.predecessor_digest != context.predecessor_digest
    {
        return Err(SignerCustodyErrorV1::ReplayOrRollback);
    }
    validate_freshness(
        statement,
        trust,
        context.now_unix_ms,
        context.anchor_observed_at_unix_ms,
    )
}

fn validate_use_context(
    statement: &SignerCustodyStatementV1,
    record_digest: [u8; 32],
    trust: &SignerCustodyTrustV1,
    context: &SignerCustodyUseContextV1,
) -> Result<(), SignerCustodyErrorV1> {
    if context.signer_revoked || context.attester_revoked {
        return Err(SignerCustodyErrorV1::Revoked);
    }
    let active = context.active_head;
    if !valid_anchor(context.current_anchor)
        || !valid_anchor(active.approved_anchor)
        || statement.anchor != active.approved_anchor
        || context.current_anchor.height < active.approved_anchor.height
        || (context.current_anchor.height == active.approved_anchor.height
            && context.current_anchor != active.approved_anchor)
    {
        return Err(SignerCustodyErrorV1::AnchorMismatch);
    }
    if active.record_digest == [0; 32]
        || active.record_digest != record_digest
        || active.sequence == 0
        || active.sequence != statement.sequence
        || active.key_revision != statement.binding.key_revision
        || active.policy_revision != statement.binding.policy_revision
        || active.policy_digest != statement.binding.policy_digest
    {
        return Err(SignerCustodyErrorV1::ReplayOrRollback);
    }
    validate_freshness(
        statement,
        trust,
        context.now_unix_ms,
        context.anchor_observed_at_unix_ms,
    )
}

fn validate_freshness(
    statement: &SignerCustodyStatementV1,
    trust: &SignerCustodyTrustV1,
    now: u64,
    anchor_observed_at_unix_ms: u64,
) -> Result<(), SignerCustodyErrorV1> {
    if now == 0
        || anchor_observed_at_unix_ms == 0
        || anchor_observed_at_unix_ms > now
        || now - anchor_observed_at_unix_ms > trust.max_anchor_age_ms
        || statement.issued_at_unix_ms > now
        || statement.expires_at_unix_ms <= now
        || statement.issued_at_unix_ms < trust.active_from_unix_ms
        || statement.expires_at_unix_ms > trust.active_until_unix_ms
        || now < trust.active_from_unix_ms
        || now >= trust.active_until_unix_ms
        || statement.expires_at_unix_ms - statement.issued_at_unix_ms > trust.max_validity_ms
    {
        return Err(SignerCustodyErrorV1::Freshness);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
