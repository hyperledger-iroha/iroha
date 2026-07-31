//! Production SFM-4b3 moderation evidence-viewer service.
//!
//! This module deliberately keeps authentication secrets, WebAuthn assertions,
//! bearer grants, and evidence bytes outside durable state. The authoritative
//! checkpoint contains only finalized authorization anchors, one-way
//! token/assertion digests, and Ed25519-authenticated payload-free receipts.
//! Runtime providers are injected by the embedding daemon; there is no file,
//! environment, or in-process key fallback. The local checkpoint file is a
//! revalidated cache of the qualified external CAS authority and can never seed
//! or replace it. Expired challenge/session records are pruned only after an
//! exact signer-authenticated artifact is durably installed and read back from
//! a qualified immutable archive. Production audit/readback comes from the
//! exact signed receipt checkpoint and
//! [`EvidenceViewerTransparencyProjectionV1`]; the older `NodeHandle`
//! session/access registry is intentionally not fed by this service.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use iroha_config::parameters::{
    ProductionRuntimeHandleError, is_production_runtime_handle, validate_production_runtime_handle,
};
use iroha_crypto::{Algorithm, PublicKey, Signature as IrohaSignature};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use url::Url;

use crate::{
    ModerationEvidenceViewerAccessKind, ModerationEvidenceViewerSessionInput,
    ModerationEvidenceViewerSessionRecord, ModerationQuarantineObjectError,
    ModerationQuarantineObjectRangePayload, ModerationQuarantineObjectRecord, NodeHandle,
    decode_local_checkpoint_canonical,
    moderation::{
        evidence_viewer_session_record_from_input, validate_evidence_viewer_session_record,
    },
    read_local_checkpoint_bounded, write_local_checkpoint_atomic_bounded,
};

/// Deployment-owned monotonic transparency producer for signed viewer state.
pub mod transparency_producer;

/// Canonical evidence-viewer checkpoint schema version.
pub const EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1: u16 = 1;
/// Canonical external checkpoint-authority record schema version.
pub const EVIDENCE_VIEWER_CHECKPOINT_STORE_RECORD_VERSION_V1: u16 = 1;
/// Canonical signed compaction-archive schema version.
pub const EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1: u16 = 1;
/// Canonical evidence-viewer manifest schema version.
pub const EVIDENCE_VIEWER_MANIFEST_VERSION_V1: u16 = 1;
/// Canonical signed receipt schema version.
pub const EVIDENCE_VIEWER_RECEIPT_VERSION_V1: u16 = 1;
/// Canonical receipt-derived transparency projection schema version.
pub const EVIDENCE_VIEWER_TRANSPARENCY_PROJECTION_VERSION_V1: u16 = 1;
/// Maximum accepted session lifetime.
pub const EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1: u64 = 15 * 60 * 1_000;
/// Maximum opaque challenge or grant size accepted at the trust boundary.
pub const EVIDENCE_VIEWER_MAX_OPAQUE_TOKEN_BYTES_V1: usize = 4 * 1024;
/// Maximum canonical WebAuthn assertion size.
pub const EVIDENCE_VIEWER_MAX_WEBAUTHN_ASSERTION_BYTES_V1: usize = 64 * 1024;
/// Maximum configured checkpoint size.
pub const EVIDENCE_VIEWER_MAX_CHECKPOINT_BYTES_V1: u64 = 64 * 1024 * 1024;
/// Maximum accepted opaque runtime-provider handle size.
pub const EVIDENCE_VIEWER_RUNTIME_PROVIDER_HANDLE_MAX_BYTES_V1: usize = 256;
/// Maximum records removed by one authenticated compaction transition.
pub const EVIDENCE_VIEWER_MAX_COMPACTION_RECORDS_V1: u32 = 1_024;
/// Minimum supervised archive-compaction cadence.
pub const EVIDENCE_VIEWER_MIN_COMPACTION_INTERVAL_MS_V1: u64 = 1_000;
/// Maximum supervised archive-compaction cadence.
pub const EVIDENCE_VIEWER_MAX_COMPACTION_INTERVAL_MS_V1: u64 = 24 * 60 * 60 * 1_000;

const CHALLENGE_BINDING_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.challenge-binding.v1";
const SESSION_REQUEST_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.session-request.v1";
const GRANT_CLAIMS_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.grant-claims.v1";
const RECEIPT_BODY_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.receipt-body.v1";
const RECEIPT_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.receipt-signature.v1";
const CHECKPOINT_SIGNATURE_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.checkpoint-signature.v1";
const WATERMARK_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.watermark.v1";
const REQUEST_BINDING_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.request-binding.v1";
const ERASURE_OPERATION_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.erasure-operation.v1";
const CHECKPOINT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.evidence-viewer.checkpoint.v1";
const CHECKPOINT_STORE_RECORD_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.checkpoint-store-record-signature.v1";
const CHECKPOINT_STORE_RECORD_REVISION_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.checkpoint-store-record-revision.v1";
const CHECKPOINT_STORE_RECORD_MAX_OVERHEAD_BYTES_V1: u64 = 16 * 1024;
const COMPACTION_ARCHIVE_PAYLOAD_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.compaction-archive-payload.v1";
const COMPACTION_ARCHIVE_OPERATION_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.compaction-archive-operation.v1";
const COMPACTION_ARCHIVE_SIGNATURE_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.compaction-archive-signature.v1";
const COMPACTION_ARCHIVE_HEAD_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.compaction-archive-head.v1";
const COMPACTION_ARCHIVE_RECEIPT_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.compaction-archive-receipt.v1";
const COMPACTION_ARCHIVE_MAX_OVERHEAD_BYTES_V1: u64 = 16 * 1024;
const TRANSPARENCY_PROJECTION_DOMAIN_V1: &[u8] =
    b"sorafs.evidence-viewer.transparency-projection.v1";
const PAYLOAD_FREE_PURPOSE_LABEL_V1: &str = "case_bound_review";

/// Governed evidence-viewer service policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceViewerConfigV1 {
    /// Canonical local cache path for the externally authoritative checkpoint.
    pub checkpoint_path: PathBuf,
    /// Maximum canonical checkpoint size.
    pub checkpoint_max_bytes: u64,
    /// Session lifetime in milliseconds.
    pub session_ttl_ms: u64,
    /// Rotating grant lifetime in milliseconds.
    pub grant_ttl_ms: u64,
    /// WebAuthn challenge lifetime in milliseconds.
    pub challenge_ttl_ms: u64,
    /// Maximum authenticated range returned by one request.
    pub max_range_bytes: u64,
    /// Maximum retained challenges.
    pub max_challenges: usize,
    /// Maximum retained sessions.
    pub max_sessions: usize,
    /// Maximum retained signed receipts.
    pub max_receipts: usize,
    /// Maximum retained idempotency records.
    pub max_idempotency_records: usize,
    /// Retention interval after a session expires.
    pub retention_after_expiry_ms: u64,
    /// WebAuthn relying-party identifier.
    pub webauthn_rp_id: String,
    /// Exact HTTPS origins accepted by the injected WebAuthn verifier.
    pub webauthn_allowed_origins: Vec<String>,
    /// Governed identity of the injected WebAuthn runtime.
    pub webauthn_handle: String,
    /// Independently governed WebAuthn adapter and policy qualification.
    pub expected_webauthn_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Governed identity of the injected rotating-grant runtime.
    pub grant_handle: String,
    /// Independently governed grant adapter and policy qualification.
    pub expected_grant_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Governed identity of the injected irreversible-erasure runtime.
    pub erasure_handle: String,
    /// Independently governed erasure adapter and policy qualification.
    pub expected_erasure_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Governed identity of the immutable compaction archive.
    pub compaction_archive_handle: String,
    /// Independently governed archive adapter and policy qualification.
    pub expected_compaction_archive_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Stable non-secret archive namespace identity.
    pub compaction_archive_id: [u8; 32],
    /// Exact Ed25519 key authenticating durable archive install/readback.
    pub compaction_archive_public_key: [u8; 32],
    /// Supervised archive-compaction cadence.
    pub compaction_interval_ms: u64,
    /// Maximum expired records archived by one supervised tick.
    pub compaction_max_records: u32,
    /// Opaque runtime signer handle.
    pub receipt_signer_handle: String,
    /// Independently governed receipt-signer adapter and policy qualification.
    pub expected_receipt_signer_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    /// Governed Ed25519 receipt-verification key.
    pub receipt_signer_public_key: [u8; 32],
}

impl EvidenceViewerConfigV1 {
    /// Validate bounded production invariants.
    ///
    /// # Errors
    ///
    /// Returns a payload-free configuration error when a bound, origin,
    /// runtime handle, key, or time interval is invalid.
    pub fn validate(&self) -> Result<(), EvidenceViewerErrorV1> {
        if self.checkpoint_path.file_name().is_none() {
            return Err(EvidenceViewerErrorV1::InvalidConfig);
        }
        if self.checkpoint_max_bytes == 0
            || self.checkpoint_max_bytes > EVIDENCE_VIEWER_MAX_CHECKPOINT_BYTES_V1
            || self.session_ttl_ms == 0
            || self.session_ttl_ms > EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1
            || self.grant_ttl_ms == 0
            || self.grant_ttl_ms > self.session_ttl_ms
            || self.challenge_ttl_ms == 0
            || self.challenge_ttl_ms > self.session_ttl_ms
            || self.max_range_bytes == 0
            || self.max_challenges == 0
            || self.max_sessions == 0
            || self.max_receipts == 0
            || self.max_idempotency_records == 0
            || self.retention_after_expiry_ms == 0
        {
            return Err(EvidenceViewerErrorV1::InvalidConfig);
        }
        if !is_canonical_rp_id(&self.webauthn_rp_id) {
            return Err(EvidenceViewerErrorV1::InvalidConfig);
        }
        if !is_production_runtime_handle(&self.webauthn_handle)
            || !is_production_runtime_handle(&self.grant_handle)
            || !is_production_runtime_handle(&self.erasure_handle)
            || !is_production_runtime_handle(&self.compaction_archive_handle)
            || !is_production_runtime_handle(&self.receipt_signer_handle)
            || !self.expected_webauthn_qualification.is_valid()
            || !self.expected_grant_qualification.is_valid()
            || !self.expected_erasure_qualification.is_valid()
            || !self.expected_compaction_archive_qualification.is_valid()
            || !self.expected_receipt_signer_qualification.is_valid()
            || is_zero_digest(self.compaction_archive_id)
            || self.compaction_interval_ms < EVIDENCE_VIEWER_MIN_COMPACTION_INTERVAL_MS_V1
            || self.compaction_interval_ms > EVIDENCE_VIEWER_MAX_COMPACTION_INTERVAL_MS_V1
            || self.compaction_max_records == 0
            || self.compaction_max_records > EVIDENCE_VIEWER_MAX_COMPACTION_RECORDS_V1
        {
            return Err(EvidenceViewerErrorV1::InvalidConfig);
        }
        if self.webauthn_allowed_origins.is_empty()
            || self.webauthn_allowed_origins.len() > 16
            || self
                .webauthn_allowed_origins
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.webauthn_allowed_origins.len()
            || self
                .webauthn_allowed_origins
                .iter()
                .any(|origin| !is_canonical_https_origin(origin, &self.webauthn_rp_id))
        {
            return Err(EvidenceViewerErrorV1::InvalidConfig);
        }
        if is_zero_digest(self.receipt_signer_public_key)
            || is_zero_digest(self.compaction_archive_public_key)
        {
            return Err(EvidenceViewerErrorV1::InvalidConfig);
        }
        PublicKey::from_bytes(Algorithm::Ed25519, &self.receipt_signer_public_key)
            .map_err(|_| EvidenceViewerErrorV1::InvalidConfig)?;
        PublicKey::from_bytes(Algorithm::Ed25519, &self.compaction_archive_public_key)
            .map_err(|_| EvidenceViewerErrorV1::InvalidConfig)?;
        Ok(())
    }
}

/// Exact role authorized for one finalized evidence assignment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum EvidenceViewerRoleV1 {
    /// Juror explicitly present in the finalized case roster.
    Juror,
    /// Account holding the explicit evidence-auditor role.
    Auditor,
    /// Account holding the explicit legal-reviewer role.
    Legal,
}

impl EvidenceViewerRoleV1 {
    /// Stable lower-case role label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Juror => "juror",
            Self::Auditor => "auditor",
            Self::Legal => "legal",
        }
    }
}

/// Finalized-chain authorization returned by the injected reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceViewerFinalizedAuthorizationV1 {
    /// Exact case identifier.
    pub case_id: String,
    /// Exact round identifier.
    pub round_id: String,
    /// Exact viewer account.
    pub viewer_account: String,
    /// Exact granted role.
    pub role: EvidenceViewerRoleV1,
    /// Evidence-bundle digest committed by the finalized case.
    pub evidence_bundle_digest: [u8; 32],
    /// Active policy digest committed by the finalized case.
    pub policy_digest: [u8; 32],
    /// Finalized block height.
    pub finalized_height: u64,
    /// Finalized block hash.
    pub finalized_block_hash: [u8; 32],
    /// Finalized block timestamp.
    pub finalized_at_unix_ms: u64,
}

/// Fixed finalized-reader failure classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerAuthorizationErrorV1 {
    /// Finalized state is temporarily unavailable.
    #[error("finalized moderation state unavailable")]
    Unavailable,
    /// Case, evidence, account, or explicit role did not authorize access.
    #[error("finalized moderation assignment denied")]
    Denied,
    /// A configured resource bound was reached.
    #[error("finalized moderation query bound exhausted")]
    ResourceExhausted,
}

/// Runtime-only reader for exact finalized moderation assignments and roles.
pub trait EvidenceViewerFinalizedAuthorizationReaderV1: Send + Sync + fmt::Debug {
    /// Authorize one exact case/evidence/account/role tuple.
    ///
    /// Operator role alone must never produce an authorization.
    fn authorize(
        &self,
        case_id: &str,
        round_id: &str,
        viewer_account: &str,
        role: EvidenceViewerRoleV1,
        evidence_bundle_digest: [u8; 32],
    ) -> Result<EvidenceViewerFinalizedAuthorizationV1, EvidenceViewerAuthorizationErrorV1>;
}

/// Successful WebAuthn assertion result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceViewerWebAuthnResultV1 {
    /// Digest of the complete verified attestation transcript.
    pub attestation_digest: [u8; 32],
    /// Digest of the credential identifier.
    pub credential_id_digest: [u8; 32],
    /// Monotonic authenticator counter observed by the verifier.
    pub authenticator_counter: u64,
}

/// Fixed external security-provider failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerExternalErrorV1 {
    /// Provider is temporarily unavailable.
    #[error("external evidence-viewer provider unavailable")]
    Unavailable,
    /// Provider rejected the operation.
    #[error("external evidence-viewer provider rejected request")]
    Rejected,
    /// Provider is saturated.
    #[error("external evidence-viewer provider backpressure")]
    Backpressure,
}

/// Public, non-secret qualification for an evidence-viewer runtime provider.
///
/// `revision` identifies the deployment-owned adapter and public policy
/// revision. `policy_digest` binds that exact public policy. The evidence
/// viewer pins both values before opening durable state and requires the same
/// values before and after every external security-provider operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceViewerRuntimeProviderQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
}

impl EvidenceViewerRuntimeProviderQualificationV1 {
    /// Construct one provider qualification observation.
    ///
    /// The evidence-viewer service rejects zero revisions and all-zero policy
    /// digests.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }

    /// Return the non-zero deployment adapter/policy revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Return the non-zero digest of the public provider policy.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }

    fn is_valid(self) -> bool {
        self.revision != 0 && !is_zero_digest(self.policy_digest)
    }
}

/// Stable, payload-free evidence-viewer provider qualification failures.
///
/// Provider implementations retain credentials, key identifiers, and vendor
/// diagnostics behind the typed readiness boundary. Startup and per-operation
/// checks expose only these fixed classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerRuntimeProviderQualificationErrorV1 {
    /// The configured opaque provider handle is malformed.
    #[error("configured evidence-viewer runtime provider handle is invalid")]
    InvalidConfiguredHandle,
    /// The configured handle is explicitly marked for test or development use.
    #[error("configured evidence-viewer runtime provider handle is test-marked")]
    TestMarkedConfiguredHandle,
    /// The injected provider's opaque handle is malformed.
    #[error("injected evidence-viewer runtime provider handle is invalid")]
    InvalidProviderHandle,
    /// The injected provider advertises a test- or development-marked handle.
    #[error("injected evidence-viewer runtime provider handle is test-marked")]
    TestMarkedProviderHandle,
    /// The configured provider revision or public policy digest is zero.
    #[error("configured evidence-viewer runtime provider qualification is invalid")]
    InvalidConfiguredQualification,
    /// The injected provider does not match the configured stable handle.
    #[error("evidence-viewer runtime provider handle does not match configured handle")]
    SubstitutedProvider,
    /// Qualification could not prove that the provider is current and usable.
    #[error("evidence-viewer runtime provider is unavailable, stale, or unqualified")]
    UnavailableOrStale,
    /// The provider returned a zero revision or all-zero public policy digest.
    #[error("evidence-viewer runtime provider returned an invalid qualification")]
    InvalidQualification,
    /// The provider does not match the independently governed qualification.
    #[error("evidence-viewer runtime provider qualification does not match configuration")]
    QualificationMismatch,
    /// The provider identity or public policy changed after it was pinned.
    #[error("evidence-viewer runtime provider identity or policy changed after qualification")]
    IdentityOrPolicyChanged,
    /// The receipt signer does not expose the exact governed verification key.
    #[error("evidence-viewer receipt signer public key does not match configuration")]
    SignerPublicKeyChanged,
    /// The immutable archive does not expose the exact configured namespace.
    #[error("evidence-viewer compaction archive identity does not match configuration")]
    ArchiveIdentityChanged,
    /// The immutable archive does not expose the exact governed verification key.
    #[error("evidence-viewer compaction archive public key does not match configuration")]
    ArchivePublicKeyChanged,
}

/// Fixed readiness failures returned by an evidence-viewer runtime provider.
///
/// Implementations retain vendor diagnostics inside protected provider
/// telemetry and return only these payload-free classes to the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerRuntimeProviderReadinessErrorV1 {
    /// The provider or a required credential is temporarily unavailable.
    #[error("evidence-viewer runtime provider unavailable")]
    Unavailable,
    /// The provider is revoked, stale, unauthorized, or otherwise ineligible.
    #[error("evidence-viewer runtime provider rejected qualification")]
    Rejected,
}

/// Stable identity and readiness exposed by an external evidence-viewer provider.
///
/// Implementations own all credentials, signing keys, authentication material,
/// and provider-specific diagnostics. The handle, revision, and policy digest
/// are bounded non-secret deployment metadata. `qualification` must fail when
/// the provider is unavailable, revoked, stale, test-marked, or otherwise not
/// production-ready. Vendor diagnostics must not cross this typed boundary.
pub trait EvidenceViewerRuntimeProviderV1: Send + Sync + fmt::Debug {
    /// Return the stable opaque deployment handle for this provider.
    fn handle(&self) -> &str;

    /// Qualify the active adapter and its public policy revision.
    fn qualification(
        &self,
    ) -> Result<
        EvidenceViewerRuntimeProviderQualificationV1,
        EvidenceViewerRuntimeProviderReadinessErrorV1,
    >;
}

/// Runtime-only WebAuthn boundary.
pub trait EvidenceViewerWebAuthnBoundaryV1: EvidenceViewerRuntimeProviderV1 {
    /// Issue an unpredictable challenge bound to exact non-secret claims.
    fn issue_challenge(
        &self,
        binding_digest: [u8; 32],
        expires_at_unix_ms: u64,
    ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1>;

    /// Verify and consume one challenge/assertion pair.
    fn verify_and_consume(
        &self,
        challenge: &str,
        assertion: &[u8],
        binding_digest: [u8; 32],
        rp_id: &str,
        allowed_origins: &[String],
        now_unix_ms: u64,
    ) -> Result<EvidenceViewerWebAuthnResultV1, EvidenceViewerExternalErrorV1>;
}

/// Claims bound into every rotating grant.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerGrantClaimsV1 {
    /// Session identifier.
    pub session_id: [u8; 16],
    /// Case identifier.
    pub case_id: String,
    /// Round identifier.
    pub round_id: String,
    /// Quarantine identifier.
    pub quarantine_id: [u8; 16],
    /// Viewer account.
    pub viewer_account: String,
    /// Exact authorized role.
    pub role: EvidenceViewerRoleV1,
    /// Review-purpose digest.
    pub purpose_digest: [u8; 32],
    /// Grant generation, beginning at one.
    pub generation: u64,
    /// Grant issue timestamp.
    pub issued_at_unix_ms: u64,
    /// Grant expiry timestamp.
    pub expires_at_unix_ms: u64,
}

/// Runtime-only rotating-grant boundary.
pub trait EvidenceViewerGrantBoundaryV1: EvidenceViewerRuntimeProviderV1 {
    /// Issue one unforgeable grant for exact claims.
    fn issue(
        &self,
        claims: &EvidenceViewerGrantClaimsV1,
    ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1>;

    /// Verify an unexpired grant against exact claims.
    fn verify(
        &self,
        token: &str,
        claims: &EvidenceViewerGrantClaimsV1,
        now_unix_ms: u64,
    ) -> Result<(), EvidenceViewerExternalErrorV1>;

    /// Revoke a previously issued token digest. Implementations must be
    /// idempotent.
    fn revoke(&self, token_digest: [u8; 32]) -> Result<(), EvidenceViewerExternalErrorV1>;
}

/// Runtime-only Ed25519 receipt signer.
pub trait EvidenceViewerReceiptSignerV1: EvidenceViewerRuntimeProviderV1 {
    /// Exact Ed25519 public key.
    fn public_key(&self) -> [u8; 32];

    /// Sign one exact canonical receipt message.
    fn sign(&self, message: &[u8]) -> Result<[u8; 64], EvidenceViewerExternalErrorV1>;
}

/// Runtime-only erasure/KMS boundary.
pub trait EvidenceViewerErasureBoundaryV1: EvidenceViewerRuntimeProviderV1 {
    /// Irreversibly erase or cryptographically destroy one exact object.
    ///
    /// The service records success only after this boundary reports a definite
    /// committed result. `operation_id` is stable across crash recovery and
    /// implementations must make exact replays idempotent, returning the same
    /// commit digest without repeating the irreversible operation. Ambiguous
    /// results must be returned as unavailable.
    fn erase(
        &self,
        operation_id: [u8; 32],
        quarantine_id: [u8; 16],
        object_id: [u8; 16],
        evidence_digest: [u8; 32],
    ) -> Result<[u8; 32], EvidenceViewerExternalErrorV1>;
}

/// Fixed payload-free failures returned by the authoritative checkpoint store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerCheckpointStoreExternalErrorV1 {
    /// The checkpoint store is temporarily unavailable.
    #[error("evidence-viewer checkpoint store unavailable")]
    Unavailable,
    /// The checkpoint store rejected the exact request.
    #[error("evidence-viewer checkpoint store rejected request")]
    Rejected,
    /// The CAS outcome is unknown and requires authoritative readback.
    #[error("evidence-viewer checkpoint store CAS outcome is ambiguous")]
    Ambiguous,
}

/// Signed canonical record retained by the authoritative checkpoint store.
///
/// The record carries no credentials or private material. Its generation and
/// predecessor fields form the monotonic lineage, `checkpoint_digest` binds the
/// current payload-free checkpoint, `revision` is the deterministic CAS
/// identity, and the existing governed receipt signer authenticates the whole
/// public record.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerCheckpointStoreRecordV1 {
    /// Record schema version.
    pub version: u16,
    /// Monotonic checkpoint generation, beginning at one.
    pub generation: u64,
    /// Exact predecessor CAS revision, absent only at genesis.
    pub predecessor_revision: Option<[u8; 32]>,
    /// Exact predecessor checkpoint digest, absent only at genesis.
    pub predecessor_checkpoint_digest: Option<[u8; 32]>,
    /// Digest of the current payload-free checkpoint body.
    pub checkpoint_digest: [u8; 32],
    /// Canonical signed checkpoint envelope.
    pub checkpoint_bytes: Vec<u8>,
    /// Stable opaque identity of the authoritative checkpoint store.
    pub checkpoint_store_handle: String,
    /// Exact checkpoint-store adapter/public-policy revision.
    pub checkpoint_store_revision: u64,
    /// Exact digest of the checkpoint-store public policy.
    pub checkpoint_store_policy_digest: [u8; 32],
    /// Stable opaque identity of the existing governed receipt signer.
    pub signer_handle: String,
    /// Exact governed Ed25519 receipt-signer public key.
    pub signer_public_key: [u8; 32],
    /// Ed25519 authentication over the complete public record.
    pub signature: [u8; 64],
    /// Deterministic content-addressed CAS revision.
    pub revision: [u8; 32],
}

impl fmt::Debug for EvidenceViewerCheckpointStoreRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerCheckpointStoreRecordV1")
            .field("version", &self.version)
            .field("generation", &self.generation)
            .field("predecessor_revision", &self.predecessor_revision)
            .field(
                "predecessor_checkpoint_digest",
                &self.predecessor_checkpoint_digest,
            )
            .field("checkpoint_digest", &self.checkpoint_digest)
            .field("checkpoint_bytes", &"<payload-free-checkpoint>")
            .field("checkpoint_store_handle", &self.checkpoint_store_handle)
            .field("checkpoint_store_revision", &self.checkpoint_store_revision)
            .field(
                "checkpoint_store_policy_digest",
                &self.checkpoint_store_policy_digest,
            )
            .field("signer_handle", &self.signer_handle)
            .field("signer_public_key", &self.signer_public_key)
            .field("signature", &"<ed25519-signature>")
            .field("revision", &self.revision)
            .finish()
    }
}

/// Runtime-only linearizable authority for evidence-viewer checkpoints.
///
/// Implementations must retain exact records across restarts and enforce CAS
/// over the deterministic `revision`. Durable implementations must encode
/// records with canonical Norito. Credentials, signing keys, vendor
/// diagnostics, and private provider state must never cross this interface.
pub trait EvidenceViewerCheckpointStoreV1: EvidenceViewerRuntimeProviderV1 {
    /// Load the exact current authoritative record.
    ///
    /// # Errors
    ///
    /// Returns only a fixed payload-free external failure.
    fn load_latest(
        &self,
    ) -> Result<
        Option<EvidenceViewerCheckpointStoreRecordV1>,
        EvidenceViewerCheckpointStoreExternalErrorV1,
    >;

    /// Install `next` only when the current deterministic revision matches.
    ///
    /// Implementations must return
    /// [`EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous`] whenever the
    /// commit result is not definite.
    ///
    /// # Errors
    ///
    /// Returns only a fixed payload-free external failure.
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &EvidenceViewerCheckpointStoreRecordV1,
    ) -> Result<(), EvidenceViewerCheckpointStoreExternalErrorV1>;
}

/// Authenticated exact readback from the immutable compaction archive.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceViewerCompactionArchiveReadbackV1 {
    /// Exact canonical service-signed artifact bytes.
    pub canonical_artifact: Vec<u8>,
    /// Archive Ed25519 signature emitted only after durable installation.
    pub signature: [u8; 64],
}

impl fmt::Debug for EvidenceViewerCompactionArchiveReadbackV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerCompactionArchiveReadbackV1")
            .field("canonical_artifact", &"<payload-free-archive-artifact>")
            .field("canonical_artifact_len", &self.canonical_artifact.len())
            .field("signature", &"<ed25519-signature>")
            .finish()
    }
}

/// Deployment-owned immutable archive for signed compaction artifacts.
///
/// `install` must durably bind one exact canonical artifact to `operation_id`
/// before returning success. Repeating the same identifier and bytes must be
/// idempotent and return the same signature; the same identifier with
/// substituted bytes or receipt message must be rejected. The signature
/// authenticates the exact operation/head commitment carried by the artifact
/// and must only be emitted after durable installation. `read` must return the
/// exact installed bytes and signature. Credentials, private keys, vendor
/// diagnostics, and evidence payloads must never cross this boundary.
pub trait EvidenceViewerCompactionArchiveV1: EvidenceViewerRuntimeProviderV1 {
    /// Return the stable non-secret archive namespace identity.
    fn archive_id(&self) -> [u8; 32];

    /// Return the exact Ed25519 key authenticating install/readback.
    fn signing_public_key(&self) -> [u8; 32];

    /// Durably install one exact signed archive artifact.
    ///
    /// `receipt_message` is the service-derived fixed digest bound to the
    /// archive namespace, verification key, operation identifier, and signed
    /// archive head.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free external failure.
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], EvidenceViewerExternalErrorV1>;

    /// Read back the exact artifact bound to `operation_id`.
    ///
    /// `Ok(None)` is valid only when the identifier has never been installed.
    ///
    /// # Errors
    ///
    /// Returns a fixed payload-free external failure.
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Option<EvidenceViewerCompactionArchiveReadbackV1>, EvidenceViewerExternalErrorV1>;
}

/// Runtime-only dependencies for the evidence-viewer service.
#[derive(Clone)]
pub struct EvidenceViewerRuntimeDepsV1 {
    /// Exact finalized moderation reader.
    pub authorization_reader: Arc<dyn EvidenceViewerFinalizedAuthorizationReaderV1>,
    /// WebAuthn challenge and verification boundary.
    pub webauthn: Arc<dyn EvidenceViewerWebAuthnBoundaryV1>,
    /// Rotating-grant issuer/verifier.
    pub grants: Arc<dyn EvidenceViewerGrantBoundaryV1>,
    /// Ed25519 receipt signer.
    pub receipt_signer: Arc<dyn EvidenceViewerReceiptSignerV1>,
    /// Irreversible object-erasure boundary.
    pub erasure: Arc<dyn EvidenceViewerErasureBoundaryV1>,
    /// Immutable authenticated compaction archive.
    pub compaction_archive: Arc<dyn EvidenceViewerCompactionArchiveV1>,
}

impl fmt::Debug for EvidenceViewerRuntimeDepsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerRuntimeDepsV1")
            .field("authorization_reader", &"<runtime-only>")
            .field("webauthn", &"<runtime-only>")
            .field("grants", &"<runtime-only>")
            .field("receipt_signer", &"<runtime-only>")
            .field("erasure", &"<runtime-only>")
            .field("compaction_archive", &"<runtime-only>")
            .finish()
    }
}

struct QualifiedEvidenceViewerProviderV1<P: EvidenceViewerRuntimeProviderV1 + ?Sized> {
    handle: String,
    qualification: EvidenceViewerRuntimeProviderQualificationV1,
    provider: Arc<P>,
}

impl<P: EvidenceViewerRuntimeProviderV1 + ?Sized> QualifiedEvidenceViewerProviderV1<P> {
    fn try_new(
        expected_handle: &str,
        expected_qualification: EvidenceViewerRuntimeProviderQualificationV1,
        provider: Arc<P>,
    ) -> Result<Self, EvidenceViewerRuntimeProviderQualificationErrorV1> {
        validate_evidence_viewer_runtime_provider_handle(expected_handle, true)?;
        if !expected_qualification.is_valid() {
            return Err(
                EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidConfiguredQualification,
            );
        }
        qualify_evidence_viewer_runtime_provider(
            expected_handle,
            expected_qualification,
            provider.as_ref(),
        )?;
        Ok(Self {
            handle: expected_handle.to_owned(),
            qualification: expected_qualification,
            provider,
        })
    }

    fn revalidate(&self) -> Result<(), EvidenceViewerRuntimeProviderQualificationErrorV1> {
        assert_evidence_viewer_runtime_provider_qualification(
            &self.handle,
            self.qualification,
            self.provider.as_ref(),
        )
    }

    fn invoke<T>(
        &self,
        operation: impl FnOnce(&P) -> Result<T, EvidenceViewerExternalErrorV1>,
    ) -> Result<T, EvidenceViewerExternalErrorV1> {
        self.revalidate()
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        let result = operation(self.provider.as_ref());
        self.revalidate()
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        result
    }
}

impl<P: EvidenceViewerRuntimeProviderV1 + ?Sized> fmt::Debug
    for QualifiedEvidenceViewerProviderV1<P>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedEvidenceViewerProviderV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("provider", &"<runtime-only>")
            .finish()
    }
}

impl QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerCheckpointStoreV1> {
    fn load_latest(
        &self,
    ) -> Result<
        Option<EvidenceViewerCheckpointStoreRecordV1>,
        EvidenceViewerCheckpointStoreExternalErrorV1,
    > {
        self.revalidate()
            .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?;
        let result = self.provider.load_latest();
        self.revalidate()
            .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?;
        result
    }

    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &EvidenceViewerCheckpointStoreRecordV1,
    ) -> Result<(), EvidenceViewerCheckpointStoreExternalErrorV1> {
        self.revalidate()
            .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?;
        let result = self
            .provider
            .compare_and_swap_latest(expected_revision, next);
        self.revalidate()
            .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?;
        result
    }
}

impl QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerWebAuthnBoundaryV1> {
    fn issue_challenge(
        &self,
        binding_digest: [u8; 32],
        expires_at_unix_ms: u64,
    ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1> {
        self.invoke(|provider| provider.issue_challenge(binding_digest, expires_at_unix_ms))
    }

    fn verify_and_consume(
        &self,
        challenge: &str,
        assertion: &[u8],
        binding_digest: [u8; 32],
        rp_id: &str,
        allowed_origins: &[String],
        now_unix_ms: u64,
    ) -> Result<EvidenceViewerWebAuthnResultV1, EvidenceViewerExternalErrorV1> {
        self.invoke(|provider| {
            provider.verify_and_consume(
                challenge,
                assertion,
                binding_digest,
                rp_id,
                allowed_origins,
                now_unix_ms,
            )
        })
    }
}

impl QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerGrantBoundaryV1> {
    fn issue(
        &self,
        claims: &EvidenceViewerGrantClaimsV1,
    ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1> {
        self.invoke(|provider| provider.issue(claims))
    }

    fn verify(
        &self,
        token: &str,
        claims: &EvidenceViewerGrantClaimsV1,
        now_unix_ms: u64,
    ) -> Result<(), EvidenceViewerExternalErrorV1> {
        self.invoke(|provider| provider.verify(token, claims, now_unix_ms))
    }

    fn revoke(&self, token_digest: [u8; 32]) -> Result<(), EvidenceViewerExternalErrorV1> {
        self.invoke(|provider| provider.revoke(token_digest))
    }
}

impl QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerErasureBoundaryV1> {
    fn erase(
        &self,
        operation_id: [u8; 32],
        quarantine_id: [u8; 16],
        object_id: [u8; 16],
        evidence_digest: [u8; 32],
    ) -> Result<[u8; 32], EvidenceViewerExternalErrorV1> {
        self.invoke(|provider| {
            provider.erase(operation_id, quarantine_id, object_id, evidence_digest)
        })
    }
}

struct QualifiedEvidenceViewerReceiptSignerV1 {
    inner: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerReceiptSignerV1>,
    public_key: [u8; 32],
}

impl QualifiedEvidenceViewerReceiptSignerV1 {
    fn try_new(
        expected_handle: &str,
        expected_qualification: EvidenceViewerRuntimeProviderQualificationV1,
        expected_public_key: [u8; 32],
        provider: Arc<dyn EvidenceViewerReceiptSignerV1>,
    ) -> Result<Self, EvidenceViewerRuntimeProviderQualificationErrorV1> {
        let inner = QualifiedEvidenceViewerProviderV1::try_new(
            expected_handle,
            expected_qualification,
            provider,
        )?;
        let public_key = Self::read_qualified_public_key(&inner)?;
        if public_key != expected_public_key {
            return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::SignerPublicKeyChanged);
        }
        Ok(Self {
            inner,
            public_key: expected_public_key,
        })
    }

    fn read_qualified_public_key(
        inner: &QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerReceiptSignerV1>,
    ) -> Result<[u8; 32], EvidenceViewerRuntimeProviderQualificationErrorV1> {
        inner.revalidate()?;
        let public_key = inner.provider.public_key();
        inner.revalidate()?;
        Ok(public_key)
    }

    fn sign(&self, message: &[u8]) -> Result<[u8; 64], EvidenceViewerExternalErrorV1> {
        let public_key_before = Self::read_qualified_public_key(&self.inner)
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        if public_key_before != self.public_key {
            return Err(EvidenceViewerExternalErrorV1::Unavailable);
        }
        let result = self.inner.provider.sign(message);
        let public_key_after = Self::read_qualified_public_key(&self.inner)
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        if public_key_after != self.public_key {
            return Err(EvidenceViewerExternalErrorV1::Unavailable);
        }
        result
    }
}

impl fmt::Debug for QualifiedEvidenceViewerReceiptSignerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedEvidenceViewerReceiptSignerV1")
            .field("inner", &self.inner)
            .field("public_key", &self.public_key)
            .finish()
    }
}

struct QualifiedEvidenceViewerCompactionArchiveV1 {
    inner: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerCompactionArchiveV1>,
    archive_id: [u8; 32],
    public_key: [u8; 32],
}

impl QualifiedEvidenceViewerCompactionArchiveV1 {
    fn try_new(
        expected_handle: &str,
        expected_qualification: EvidenceViewerRuntimeProviderQualificationV1,
        expected_archive_id: [u8; 32],
        expected_public_key: [u8; 32],
        provider: Arc<dyn EvidenceViewerCompactionArchiveV1>,
    ) -> Result<Self, EvidenceViewerRuntimeProviderQualificationErrorV1> {
        let inner = QualifiedEvidenceViewerProviderV1::try_new(
            expected_handle,
            expected_qualification,
            provider,
        )?;
        let (archive_id, public_key) = Self::read_qualified_identity(&inner)?;
        if archive_id != expected_archive_id {
            return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::ArchiveIdentityChanged);
        }
        if public_key != expected_public_key {
            return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::ArchivePublicKeyChanged);
        }
        Ok(Self {
            inner,
            archive_id: expected_archive_id,
            public_key: expected_public_key,
        })
    }

    fn read_qualified_identity(
        inner: &QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerCompactionArchiveV1>,
    ) -> Result<([u8; 32], [u8; 32]), EvidenceViewerRuntimeProviderQualificationErrorV1> {
        inner.revalidate()?;
        let identity = (
            inner.provider.archive_id(),
            inner.provider.signing_public_key(),
        );
        inner.revalidate()?;
        Ok(identity)
    }

    fn revalidate_identity(&self) -> Result<(), EvidenceViewerExternalErrorV1> {
        let (archive_id, public_key) = Self::read_qualified_identity(&self.inner)
            .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
        if archive_id != self.archive_id || public_key != self.public_key {
            return Err(EvidenceViewerExternalErrorV1::Unavailable);
        }
        Ok(())
    }

    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], EvidenceViewerExternalErrorV1> {
        self.revalidate_identity()?;
        let result = self
            .inner
            .provider
            .install(operation_id, receipt_message, canonical_artifact);
        self.revalidate_identity()?;
        result
    }

    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Option<EvidenceViewerCompactionArchiveReadbackV1>, EvidenceViewerExternalErrorV1>
    {
        self.revalidate_identity()?;
        let result = self.inner.provider.read(operation_id);
        self.revalidate_identity()?;
        result
    }

    fn handle(&self) -> &str {
        &self.inner.handle
    }

    fn qualification(&self) -> EvidenceViewerRuntimeProviderQualificationV1 {
        self.inner.qualification
    }
}

impl fmt::Debug for QualifiedEvidenceViewerCompactionArchiveV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedEvidenceViewerCompactionArchiveV1")
            .field("inner", &self.inner)
            .field("archive_id", &self.archive_id)
            .field("public_key", &self.public_key)
            .finish()
    }
}

struct QualifiedEvidenceViewerRuntimeDepsV1 {
    authorization_reader: Arc<dyn EvidenceViewerFinalizedAuthorizationReaderV1>,
    webauthn: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerWebAuthnBoundaryV1>,
    grants: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerGrantBoundaryV1>,
    receipt_signer: QualifiedEvidenceViewerReceiptSignerV1,
    erasure: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerErasureBoundaryV1>,
    compaction_archive: QualifiedEvidenceViewerCompactionArchiveV1,
}

impl QualifiedEvidenceViewerRuntimeDepsV1 {
    fn try_new(
        config: &EvidenceViewerConfigV1,
        deps: EvidenceViewerRuntimeDepsV1,
    ) -> Result<Self, EvidenceViewerRuntimeProviderQualificationErrorV1> {
        let EvidenceViewerRuntimeDepsV1 {
            authorization_reader,
            webauthn,
            grants,
            receipt_signer,
            erasure,
            compaction_archive,
        } = deps;
        let webauthn = QualifiedEvidenceViewerProviderV1::try_new(
            &config.webauthn_handle,
            config.expected_webauthn_qualification,
            webauthn,
        )?;
        let grants = QualifiedEvidenceViewerProviderV1::try_new(
            &config.grant_handle,
            config.expected_grant_qualification,
            grants,
        )?;
        let receipt_signer = QualifiedEvidenceViewerReceiptSignerV1::try_new(
            &config.receipt_signer_handle,
            config.expected_receipt_signer_qualification,
            config.receipt_signer_public_key,
            receipt_signer,
        )?;
        let erasure = QualifiedEvidenceViewerProviderV1::try_new(
            &config.erasure_handle,
            config.expected_erasure_qualification,
            erasure,
        )?;
        let compaction_archive = QualifiedEvidenceViewerCompactionArchiveV1::try_new(
            &config.compaction_archive_handle,
            config.expected_compaction_archive_qualification,
            config.compaction_archive_id,
            config.compaction_archive_public_key,
            compaction_archive,
        )?;
        Ok(Self {
            authorization_reader,
            webauthn,
            grants,
            receipt_signer,
            erasure,
            compaction_archive,
        })
    }
}

impl fmt::Debug for QualifiedEvidenceViewerRuntimeDepsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedEvidenceViewerRuntimeDepsV1")
            .field("authorization_reader", &"<runtime-only>")
            .field("webauthn", &self.webauthn)
            .field("grants", &self.grants)
            .field("receipt_signer", &self.receipt_signer)
            .field("erasure", &self.erasure)
            .field("compaction_archive", &self.compaction_archive)
            .finish()
    }
}

/// Secret token returned exactly once to an authenticated caller.
pub struct OpaqueEvidenceViewerSecretV1(String);

impl OpaqueEvidenceViewerSecretV1 {
    /// Construct a bounded opaque token at a runtime trust boundary.
    ///
    /// # Errors
    ///
    /// Rejects empty, over-sized, whitespace-containing, or control-bearing
    /// values.
    pub fn new(value: String) -> Result<Self, EvidenceViewerExternalErrorV1> {
        if value.is_empty()
            || value.len() > EVIDENCE_VIEWER_MAX_OPAQUE_TOKEN_BYTES_V1
            || !value.is_ascii()
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        {
            return Err(EvidenceViewerExternalErrorV1::Rejected);
        }
        Ok(Self(value))
    }

    /// Borrow the token for the immediate response or provider call.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    fn digest(&self) -> [u8; 32] {
        *blake3::hash(self.0.as_bytes()).as_bytes()
    }
}

impl fmt::Debug for OpaqueEvidenceViewerSecretV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueEvidenceViewerSecretV1(<redacted>)")
    }
}

impl Drop for OpaqueEvidenceViewerSecretV1 {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
        let _ = std::hint::black_box(&bytes);
    }
}

/// Challenge issuance request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceViewerChallengeRequestV1 {
    /// Exact case identifier.
    pub case_id: String,
    /// Exact round identifier.
    pub round_id: String,
    /// Local quarantine object identifier.
    pub quarantine_id: [u8; 16],
    /// Canonical authenticated viewer account.
    pub viewer_account: String,
    /// Requested exact role.
    pub role: EvidenceViewerRoleV1,
    /// Human-readable review purpose.
    pub purpose: String,
    /// Caller-supplied idempotency key.
    pub idempotency_key: [u8; 32],
    /// Runtime clock timestamp.
    pub now_unix_ms: u64,
}

/// Challenge issued to an authenticated browser.
#[derive(Debug)]
pub struct EvidenceViewerChallengeIssuedV1 {
    /// One-way challenge identifier.
    pub challenge_id: [u8; 16],
    /// Runtime-only challenge returned exactly once.
    pub challenge: OpaqueEvidenceViewerSecretV1,
    /// Challenge expiry.
    pub expires_at_unix_ms: u64,
}

/// Session creation request.
pub struct EvidenceViewerSessionRequestV1 {
    /// Exact case identifier.
    pub case_id: String,
    /// Exact round identifier.
    pub round_id: String,
    /// Local quarantine object identifier.
    pub quarantine_id: [u8; 16],
    /// Canonical authenticated viewer account.
    pub viewer_account: String,
    /// Requested exact role.
    pub role: EvidenceViewerRoleV1,
    /// Human-readable review purpose.
    pub purpose: String,
    /// Opaque challenge returned by the challenge endpoint.
    pub challenge: OpaqueEvidenceViewerSecretV1,
    /// Canonical WebAuthn assertion bytes.
    pub webauthn_assertion: Vec<u8>,
    /// Caller-supplied idempotency key.
    pub idempotency_key: [u8; 32],
    /// Runtime clock timestamp.
    pub now_unix_ms: u64,
}

impl fmt::Debug for EvidenceViewerSessionRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerSessionRequestV1")
            .field("case_id", &self.case_id)
            .field("round_id", &self.round_id)
            .field("quarantine_id", &self.quarantine_id)
            .field("viewer_account", &self.viewer_account)
            .field("role", &self.role)
            .field("purpose", &self.purpose)
            .field("challenge", &"<redacted>")
            .field("webauthn_assertion", &"<redacted>")
            .field("webauthn_assertion_len", &self.webauthn_assertion.len())
            .field("idempotency_key", &self.idempotency_key)
            .field("now_unix_ms", &self.now_unix_ms)
            .finish()
    }
}

impl Drop for EvidenceViewerSessionRequestV1 {
    fn drop(&mut self) {
        self.webauthn_assertion.fill(0);
        let _ = std::hint::black_box(&self.webauthn_assertion);
    }
}

/// One case-bound session returned with a rotating grant.
#[derive(Debug)]
pub struct EvidenceViewerSessionIssuedV1 {
    /// Durable payload-free session manifest.
    pub session: EvidenceViewerSessionSecurityRecordV1,
    /// Initial runtime-only grant.
    pub grant: OpaqueEvidenceViewerSecretV1,
    /// Signed payload-free issuance receipt.
    pub receipt: EvidenceViewerSignedReceiptV1,
}

/// Payload-free case-bound security metadata for one session.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerSessionSecurityRecordV1 {
    /// Underlying local payload-free session record.
    pub local_session: ModerationEvidenceViewerSessionRecord,
    /// Finalized case identifier.
    pub case_id: String,
    /// Finalized round identifier.
    pub round_id: String,
    /// Exact authorized role.
    pub role: EvidenceViewerRoleV1,
    /// Digest of the human-readable purpose.
    pub purpose_digest: [u8; 32],
    /// WebAuthn credential-id digest.
    pub credential_id_digest: [u8; 32],
    /// Digest of the exact consumed WebAuthn assertion bytes.
    pub webauthn_assertion_digest: [u8; 32],
    /// Monotonic authenticator counter.
    pub authenticator_counter: u64,
    /// Finalized policy digest.
    pub policy_digest: [u8; 32],
    /// Finalized block height.
    pub finalized_height: u64,
    /// Finalized block hash.
    pub finalized_block_hash: [u8; 32],
    /// Finalized block timestamp.
    pub finalized_at_unix_ms: u64,
    /// Active grant generation.
    pub grant_generation: u64,
    /// Active grant issue timestamp.
    pub active_grant_issued_at_unix_ms: u64,
    /// One-way digest of the active grant.
    pub active_grant_digest: [u8; 32],
    /// Active grant expiry.
    pub active_grant_expires_at_unix_ms: u64,
    /// Whether the session was revoked or erased.
    pub revoked: bool,
}

/// Payload-free browser manifest.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerManifestV1 {
    /// Schema version.
    pub version: u16,
    /// Session identifier.
    pub session_id: [u8; 16],
    /// Case identifier.
    pub case_id: String,
    /// Round identifier.
    pub round_id: String,
    /// Quarantine identifier.
    pub quarantine_id: [u8; 16],
    /// Object identifier.
    pub object_id: [u8; 16],
    /// Evidence digest.
    pub evidence_digest: [u8; 32],
    /// Plaintext byte length.
    pub payload_len: u64,
    /// Optional coarse allowlisted media type.
    pub content_type: Option<String>,
    /// Maximum authenticated range bytes per request.
    pub max_range_bytes: u64,
    /// Exact authorized viewer role.
    pub role: EvidenceViewerRoleV1,
    /// Purpose digest.
    pub purpose_digest: [u8; 32],
    /// Visible watermark label.
    pub visible_watermark: String,
    /// Watermark metadata digest.
    pub watermark_metadata_digest: [u8; 32],
    /// Session expiry.
    pub expires_at_unix_ms: u64,
    /// Finalized authorization height.
    pub finalized_height: u64,
    /// Finalized authorization block hash.
    pub finalized_block_hash: [u8; 32],
}

/// One authenticated manifest response with a rotated grant.
#[derive(Debug)]
pub struct EvidenceViewerManifestOutcomeV1 {
    /// Payload-free manifest.
    pub manifest: EvidenceViewerManifestV1,
    /// Replacement runtime-only grant.
    pub rotated_grant: OpaqueEvidenceViewerSecretV1,
    /// Signed access receipt.
    pub receipt: EvidenceViewerSignedReceiptV1,
}

/// One authenticated range response with a rotated grant.
pub struct EvidenceViewerRangeOutcomeV1 {
    /// Authenticated decrypted range. This value must never be formatted into a
    /// log or readiness artifact.
    pub range: ModerationQuarantineObjectRangePayload,
    /// Per-session watermark metadata digest for the embedded renderer.
    pub watermark_metadata_digest: [u8; 32],
    /// Replacement runtime-only grant.
    pub rotated_grant: OpaqueEvidenceViewerSecretV1,
    /// Signed access receipt committed before bytes are returned.
    pub receipt: EvidenceViewerSignedReceiptV1,
}

impl fmt::Debug for EvidenceViewerRangeOutcomeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerRangeOutcomeV1")
            .field("quarantine_id", &self.range.record.quarantine_id)
            .field("object_id", &self.range.record.object_id)
            .field("start", &self.range.start)
            .field("end", &self.range.end)
            .field("payload", &"<redacted>")
            .field("payload_len", &self.range.payload.len())
            .field("watermark_metadata_digest", &self.watermark_metadata_digest)
            .field("rotated_grant", &"<redacted>")
            .field("receipt", &self.receipt)
            .finish()
    }
}

/// Payload-free signed receipt kind.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum EvidenceViewerReceiptKindV1 {
    /// WebAuthn challenge issued.
    ChallengeIssued,
    /// Session and initial grant issued.
    SessionIssued,
    /// Manifest accessed.
    ManifestAccessed,
    /// Evidence range accessed.
    RangeAccessed,
    /// Browser interaction recorded.
    InteractionRecorded,
    /// Legal hold placed.
    LegalHoldPlaced,
    /// Legal hold released.
    LegalHoldReleased,
    /// Retention decision recorded.
    RetentionEvaluated,
    /// Erasure completed.
    ErasureCompleted,
    /// Erasure denied because a legal hold had precedence.
    ErasureDeniedLegalHold,
}

/// Canonical payload-free receipt body.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerReceiptBodyV1 {
    /// Schema version.
    pub version: u16,
    /// Monotonic global sequence.
    pub sequence: u64,
    /// Receipt kind.
    pub kind: EvidenceViewerReceiptKindV1,
    /// Optional session identifier.
    pub session_id: Option<[u8; 16]>,
    /// Optional case identifier.
    pub case_id: Option<String>,
    /// Optional round identifier.
    pub round_id: Option<String>,
    /// Quarantine identifier.
    pub quarantine_id: [u8; 16],
    /// Object identifier.
    pub object_id: [u8; 16],
    /// Evidence digest.
    pub evidence_digest: [u8; 32],
    /// Canonical actor-account digest.
    pub actor_account_digest: [u8; 32],
    /// Idempotency-key digest.
    pub idempotency_key_digest: [u8; 32],
    /// Event/request metadata digest.
    pub request_digest: [u8; 32],
    /// Optional byte-range start.
    pub range_start: Option<u64>,
    /// Optional byte-range end.
    pub range_end: Option<u64>,
    /// Receipt issue timestamp.
    pub issued_at_unix_ms: u64,
    /// Previous receipt digest, or zeroes for the first receipt.
    pub previous_receipt_digest: [u8; 32],
}

/// Ed25519-authenticated payload-free receipt.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerSignedReceiptV1 {
    /// Canonical body.
    pub body: EvidenceViewerReceiptBodyV1,
    /// Domain-separated digest of the exact canonical body.
    pub receipt_digest: [u8; 32],
    /// Opaque runtime signer handle.
    pub signer_handle: String,
    /// Ed25519 public key.
    pub signer_public_key: [u8; 32],
    /// Ed25519 signature.
    pub signature: [u8; 64],
}

impl EvidenceViewerSignedReceiptV1 {
    /// Verify the body digest, chain-neutral signature, and governed identity.
    ///
    /// # Errors
    ///
    /// Returns an invalid-checkpoint error for any mismatch.
    pub fn verify(
        &self,
        expected_handle: &str,
        expected_public_key: [u8; 32],
    ) -> Result<(), EvidenceViewerErrorV1> {
        if self.body.version != EVIDENCE_VIEWER_RECEIPT_VERSION_V1
            || self.body.sequence == 0
            || self.body.issued_at_unix_ms == 0
            || self.signer_handle != expected_handle
            || self.signer_public_key != expected_public_key
            || self.receipt_digest != receipt_body_digest(&self.body)?
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        let key = PublicKey::from_bytes(Algorithm::Ed25519, &self.signer_public_key)
            .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        let signature = IrohaSignature::try_from_bytes(&self.signature)
            .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        signature
            .verify(&key, &receipt_signature_message(self.receipt_digest))
            .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)
    }
}

/// Exact cursor into the durable signed evidence-viewer receipt chain.
///
/// Consumers must persist both fields. A sequence without its exact digest is
/// not a valid continuation cursor because it cannot detect checkpoint
/// substitution or rollback.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct EvidenceViewerReceiptCursorV1 {
    /// Monotonic receipt sequence.
    pub sequence: u64,
    /// Exact signed receipt digest at `sequence`.
    pub receipt_digest: [u8; 32],
}

/// Public Ed25519-authenticated anchor for one exact durable checkpoint.
///
/// The signature covers the checkpoint generation and predecessor, checkpoint
/// digest, retained receipt count, exact receipt-chain and compaction-archive
/// heads, plus the qualified checkpoint-store handle, revision, and policy
/// digest. Audit GETs return this retained anchor without invoking the signer.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerSignedCheckpointAnchorV1 {
    /// Checkpoint/anchor schema version.
    pub version: u16,
    /// Monotonic authoritative checkpoint-store generation.
    pub checkpoint_generation: u64,
    /// Exact predecessor checkpoint-store revision, absent only at genesis.
    pub predecessor_checkpoint_revision: Option<[u8; 32]>,
    /// Exact predecessor checkpoint digest, absent only at genesis.
    pub predecessor_checkpoint_digest: Option<[u8; 32]>,
    /// Digest of the complete canonical durable checkpoint.
    pub checkpoint_digest: [u8; 32],
    /// Number of signed receipts committed by that checkpoint.
    pub receipt_count: u64,
    /// Exact receipt-chain head, or `None` for an empty checkpoint.
    pub chain_head: Option<EvidenceViewerReceiptCursorV1>,
    /// Exact signed immutable compaction-archive head committed by this
    /// checkpoint, or `None` before the first compaction.
    pub compaction_archive_head_digest: Option<[u8; 32]>,
    /// Stable public identity of the authoritative checkpoint store.
    pub checkpoint_store_handle: String,
    /// Exact checkpoint-store adapter/public-policy revision.
    pub checkpoint_store_revision: u64,
    /// Exact digest of the checkpoint-store public policy.
    pub checkpoint_store_policy_digest: [u8; 32],
    /// Opaque governed runtime signer handle.
    pub signer_handle: String,
    /// Governed Ed25519 public key.
    pub signer_public_key: [u8; 32],
    /// Ed25519 signature over the exact anchor fields.
    pub signature: [u8; 64],
}

impl EvidenceViewerSignedCheckpointAnchorV1 {
    /// Verify the exact signer identity, structural head/count binding, and
    /// Ed25519 signature.
    ///
    /// # Errors
    ///
    /// Returns an invalid-checkpoint error for any malformed, substituted, or
    /// forged anchor.
    pub fn verify(
        &self,
        expected_signer_handle: &str,
        expected_signer_public_key: [u8; 32],
    ) -> Result<(), EvidenceViewerErrorV1> {
        let valid_lineage = match self.checkpoint_generation {
            1 => {
                self.predecessor_checkpoint_revision.is_none()
                    && self.predecessor_checkpoint_digest.is_none()
            }
            2.. => {
                self.predecessor_checkpoint_revision
                    .is_some_and(|digest| !is_zero_digest(digest))
                    && self
                        .predecessor_checkpoint_digest
                        .is_some_and(|digest| !is_zero_digest(digest))
            }
            0 => false,
        };
        if self.version != EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1
            || !valid_lineage
            || is_zero_digest(self.checkpoint_digest)
            || !is_production_runtime_handle(&self.signer_handle)
            || is_zero_digest(self.signer_public_key)
            || self.signer_handle != expected_signer_handle
            || self.signer_public_key != expected_signer_public_key
            || !checkpoint_anchor_head_is_valid(self.receipt_count, self.chain_head)
            || self
                .compaction_archive_head_digest
                .is_some_and(is_zero_digest)
            || !is_production_runtime_handle(&self.checkpoint_store_handle)
            || self.checkpoint_store_revision == 0
            || is_zero_digest(self.checkpoint_store_policy_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        let key = PublicKey::from_bytes(Algorithm::Ed25519, &self.signer_public_key)
            .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        let signature = IrohaSignature::try_from_bytes(&self.signature)
            .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        signature
            .verify(&key, &checkpoint_anchor_signature_message(self))
            .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)
    }
}

/// Exact caller fence for one signed compaction/archive transition.
///
/// The signed checkpoint anchor and archive predecessor must both match the
/// service's authoritative state. This prevents a delayed worker from pruning
/// against a substituted checkpoint or a forked archive head.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerCompactionArchiveRequestV1 {
    /// Exact signed checkpoint whose expired records may be archived.
    pub expected_checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
    /// Exact current archive head, or `None` before the first transition.
    pub expected_archive_head_digest: Option<[u8; 32]>,
    /// Inclusive expiry cutoff applied to challenges and sessions.
    pub compacted_through_unix_ms: u64,
    /// Maximum combined challenge/session records removed by this transition.
    pub maximum_records: u32,
}

/// Signed monotonic head for one immutable compaction archive artifact.
///
/// The head contains only payload-free metadata. The archive artifact contains
/// the exact expired challenge/session records and is durably installed under
/// `operation_id` before the authoritative checkpoint may prune them.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerSignedCompactionArchiveHeadV1 {
    /// Archive schema version.
    pub version: u16,
    /// Monotonic archive generation, beginning at one.
    pub generation: u64,
    /// Exact predecessor archive-head digest, absent only at generation one.
    pub predecessor_head_digest: Option<[u8; 32]>,
    /// Exact predecessor archive operation, absent only at generation one.
    ///
    /// Retaining the content-addressed operation identity makes every
    /// historical generation recoverable and permits authenticated lineage
    /// verification after restart.
    pub predecessor_operation_id: Option<[u8; 32]>,
    /// Stable deterministic identifier used for exact external replay.
    pub operation_id: [u8; 32],
    /// Exact source checkpoint-store generation.
    pub source_checkpoint_generation: u64,
    /// Exact source checkpoint-store revision.
    pub source_checkpoint_revision: [u8; 32],
    /// Complete signed source checkpoint anchor.
    pub source_checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
    /// Inclusive expiry cutoff applied by the transition.
    pub compacted_through_unix_ms: u64,
    /// Caller-supplied deterministic work bound.
    pub maximum_records: u32,
    /// Number of expired challenge records installed in the archive.
    pub challenge_count: u32,
    /// Number of expired session records installed in the archive.
    pub session_count: u32,
    /// Digest of the exact canonical archived records.
    pub compacted_payload_digest: [u8; 32],
    /// Stable authenticated archive-provider identity.
    pub archive_handle: String,
    /// Exact archive adapter/public-policy revision.
    pub archive_revision: u64,
    /// Exact archive public-policy digest.
    pub archive_policy_digest: [u8; 32],
    /// Stable non-secret archive namespace identity.
    pub archive_id: [u8; 32],
    /// Exact Ed25519 key authenticating durable archive readback.
    pub archive_public_key: [u8; 32],
    /// Governed evidence-viewer signer identity.
    pub signer_handle: String,
    /// Governed Ed25519 signer public key.
    pub signer_public_key: [u8; 32],
    /// Ed25519 signature over the exact transition and operation identifier.
    pub signature: [u8; 64],
    /// Signed content-addressed archive-chain head.
    pub head_digest: [u8; 32],
    /// Archive signature emitted only after durable exact installation.
    pub archive_signature: [u8; 64],
}

impl EvidenceViewerSignedCompactionArchiveHeadV1 {
    /// Verify structure, deterministic identities, and the governed signature.
    ///
    /// # Errors
    ///
    /// Rejects malformed generations, predecessor substitution, forged source
    /// anchors, invalid operation/head digests, and signer substitution.
    pub fn verify(
        &self,
        expected_signer_handle: &str,
        expected_signer_public_key: [u8; 32],
    ) -> Result<(), EvidenceViewerErrorV1> {
        verify_compaction_archive_head(self, expected_signer_handle, expected_signer_public_key)
    }
}

/// Bounded payload-free projection for transparency and durable readback.
///
/// `receipts` is a contiguous suffix of the authoritative signed checkpoint
/// chain. It contains only receipt metadata and one-way actor/idempotency
/// digests; evidence bytes, assertions, bearer grants, holder secrets, and raw
/// viewer identities are never projected.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerTransparencyProjectionV1 {
    /// Transparency projection schema version.
    pub version: u16,
    /// Exact signed durable checkpoint that this page projects.
    pub checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
    /// Exact signed immutable compaction-archive head committed by the same
    /// checkpoint, or `None` before the first compaction.
    ///
    /// Carrying the authenticated head in the projection lets a
    /// deployment-owned transparency producer verify archive identity and
    /// monotonic lineage without reading private checkpoint state.
    pub compaction_archive_head: Option<EvidenceViewerSignedCompactionArchiveHeadV1>,
    /// Exact exclusive predecessor supplied by the consumer, or genesis.
    pub predecessor: Option<EvidenceViewerReceiptCursorV1>,
    /// Exact bounded request page size committed into `projection_digest`.
    pub page_limit: u16,
    /// Contiguous signed receipts after `predecessor`.
    pub receipts: Vec<EvidenceViewerSignedReceiptV1>,
    /// Exact cursor after the final returned receipt, or the unchanged
    /// predecessor when this projection is empty.
    pub next_cursor: Option<EvidenceViewerReceiptCursorV1>,
    /// Whether another bounded page is available.
    pub has_more: bool,
    /// Domain-separated digest of the exact cursor, page, and continuation
    /// marker.
    pub projection_digest: [u8; 32],
}

impl EvidenceViewerTransparencyProjectionV1 {
    /// Verify projection structure, exact receipt-chain continuation, every
    /// receipt signature, and the projection digest.
    ///
    /// # Errors
    ///
    /// Returns an invalid-checkpoint error for any version, cursor, chain,
    /// signer, signature, or digest mismatch.
    pub fn verify(
        &self,
        expected_signer_handle: &str,
        expected_signer_public_key: [u8; 32],
    ) -> Result<(), EvidenceViewerErrorV1> {
        if self.version != EVIDENCE_VIEWER_TRANSPARENCY_PROJECTION_VERSION_V1
            || self.page_limit == 0
            || usize::from(self.page_limit) > 1_024
            || self.receipts.len() > 1_024
            || self.receipts.len() > usize::from(self.page_limit)
            || self
                .predecessor
                .is_some_and(|cursor| cursor.sequence == 0 || is_zero_digest(cursor.receipt_digest))
            || (self.receipts.is_empty() && self.has_more)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        self.checkpoint_anchor
            .verify(expected_signer_handle, expected_signer_public_key)?;
        if let Some(head) = self.compaction_archive_head.as_ref() {
            head.verify(expected_signer_handle, expected_signer_public_key)?;
        }
        if self.checkpoint_anchor.compaction_archive_head_digest
            != self
                .compaction_archive_head
                .as_ref()
                .map(|head| head.head_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        let mut expected_sequence = match self.predecessor {
            Some(cursor) => cursor
                .sequence
                .checked_add(1)
                .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?,
            None => 1,
        };
        let mut previous_digest = self
            .predecessor
            .map_or([0; 32], |cursor| cursor.receipt_digest);
        for receipt in &self.receipts {
            receipt.verify(expected_signer_handle, expected_signer_public_key)?;
            if receipt.body.sequence != expected_sequence
                || receipt.body.previous_receipt_digest != previous_digest
            {
                return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
            }
            expected_sequence = expected_sequence
                .checked_add(1)
                .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
            previous_digest = receipt.receipt_digest;
        }
        let expected_next = self
            .receipts
            .last()
            .map(receipt_cursor)
            .or(self.predecessor);
        let head = self.checkpoint_anchor.chain_head;
        let continuation_is_consistent = match (self.has_more, self.next_cursor, head) {
            (false, next, head) => next == head,
            (true, Some(next), Some(head)) => next.sequence < head.sequence,
            _ => false,
        };
        if self.next_cursor != expected_next
            || !continuation_is_consistent
            || self.projection_digest
                != transparency_projection_digest(
                    &self.checkpoint_anchor,
                    self.compaction_archive_head.as_ref(),
                    self.predecessor,
                    self.page_limit,
                    &self.receipts,
                    self.next_cursor,
                    self.has_more,
                )?
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        Ok(())
    }
}

/// Legal-hold state for one evidence object.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerLegalHoldV1 {
    /// Stable legal-hold identifier.
    pub hold_id: [u8; 16],
    /// Quarantine identifier.
    pub quarantine_id: [u8; 16],
    /// Object identifier.
    pub object_id: [u8; 16],
    /// Evidence digest.
    pub evidence_digest: [u8; 32],
    /// Digest of the legal authority/case reference.
    pub authority_digest: [u8; 32],
    /// Placement timestamp.
    pub placed_at_unix_ms: u64,
    /// Optional release timestamp.
    pub released_at_unix_ms: Option<u64>,
}

/// Payload-free erasure state.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerErasureRecordV1 {
    /// Quarantine identifier.
    pub quarantine_id: [u8; 16],
    /// Object identifier.
    pub object_id: [u8; 16],
    /// Evidence digest.
    pub evidence_digest: [u8; 32],
    /// Definite external erasure-commit digest.
    pub erasure_commit_digest: [u8; 32],
    /// Completion timestamp.
    pub erased_at_unix_ms: u64,
    /// Signed erasure receipt digest.
    pub receipt_digest: [u8; 32],
}

/// Payload-free signed retention decision.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerRetentionRecordV1 {
    /// Quarantine identifier.
    pub quarantine_id: [u8; 16],
    /// Object identifier.
    pub object_id: [u8; 16],
    /// Evidence digest.
    pub evidence_digest: [u8; 32],
    /// Earliest time at which supervised erasure may be considered.
    pub retain_until_unix_ms: u64,
    /// Whether an active legal hold took precedence at evaluation time.
    pub legal_hold_precedence: bool,
    /// Evaluation timestamp.
    pub evaluated_at_unix_ms: u64,
    /// Signed retention receipt digest.
    pub receipt_digest: [u8; 32],
}

/// Payload-free audit/status projection.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EvidenceViewerAuditStatusV1 {
    /// Checkpoint schema version.
    pub version: u16,
    /// Retained challenge count.
    pub challenge_count: u64,
    /// Retained session count.
    pub session_count: u64,
    /// Retained receipt count.
    pub receipt_count: u64,
    /// Active legal-hold count.
    pub active_legal_hold_count: u64,
    /// Completed erasure count.
    pub erasure_count: u64,
    /// Retained signed retention-decision count.
    pub retention_count: u64,
    /// Exact signed checkpoint and receipt-chain head represented by the
    /// counters above.
    pub checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ChallengeRecordV1 {
    challenge_id: [u8; 16],
    challenge_digest: [u8; 32],
    binding_digest: [u8; 32],
    case_id: String,
    round_id: String,
    quarantine_id: [u8; 16],
    viewer_account_digest: [u8; 32],
    role: EvidenceViewerRoleV1,
    purpose_digest: [u8; 32],
    policy_digest: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
    finalized_at_unix_ms: u64,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    consumed_at_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct IdempotencyRecordV1 {
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
    outcome_digest: [u8; 32],
}

/// Durable write-ahead intent for one irreversible erasure.
///
/// The intent is committed before crossing the KMS/object-store boundary and
/// is retained until the signed receipt and terminal erasure record are
/// durably committed. This makes crash recovery an exact idempotent replay
/// instead of a second irreversible operation.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EvidenceViewerErasureIntentV1 {
    operation_id: [u8; 32],
    quarantine_id: [u8; 16],
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
    case_id: String,
    round_id: String,
    actor_account: String,
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
    requested_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EvidenceViewerCompactionArchivePayloadV1 {
    version: u16,
    challenges: Vec<ChallengeRecordV1>,
    sessions: Vec<EvidenceViewerSessionSecurityRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EvidenceViewerCompactionArchiveArtifactV1 {
    version: u16,
    head: EvidenceViewerSignedCompactionArchiveHeadV1,
    payload: EvidenceViewerCompactionArchivePayloadV1,
}

/// Payload-free minimum default-retention boundary retained after session
/// compaction.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EvidenceViewerDefaultRetentionFloorV1 {
    quarantine_id: [u8; 16],
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
    basis_session_expires_at_unix_ms: u64,
    retain_until_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EvidenceViewerCheckpointV1 {
    version: u16,
    challenges: Vec<ChallengeRecordV1>,
    sessions: Vec<EvidenceViewerSessionSecurityRecordV1>,
    receipts: Vec<EvidenceViewerSignedReceiptV1>,
    legal_holds: Vec<EvidenceViewerLegalHoldV1>,
    retentions: Vec<EvidenceViewerRetentionRecordV1>,
    default_retention_floors: Vec<EvidenceViewerDefaultRetentionFloorV1>,
    erasure_intents: Vec<EvidenceViewerErasureIntentV1>,
    erasures: Vec<EvidenceViewerErasureRecordV1>,
    idempotency: Vec<IdempotencyRecordV1>,
    compaction_archive_head: Option<EvidenceViewerSignedCompactionArchiveHeadV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct EvidenceViewerCheckpointEnvelopeV1 {
    version: u16,
    checkpoint: EvidenceViewerCheckpointV1,
    checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
}

impl Default for EvidenceViewerCheckpointV1 {
    fn default() -> Self {
        Self {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            challenges: Vec::new(),
            sessions: Vec::new(),
            receipts: Vec::new(),
            legal_holds: Vec::new(),
            retentions: Vec::new(),
            default_retention_floors: Vec::new(),
            erasure_intents: Vec::new(),
            erasures: Vec::new(),
            idempotency: Vec::new(),
            compaction_archive_head: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EvidenceViewerStateV1 {
    challenges: BTreeMap<[u8; 16], ChallengeRecordV1>,
    sessions: BTreeMap<[u8; 16], EvidenceViewerSessionSecurityRecordV1>,
    receipts: Vec<EvidenceViewerSignedReceiptV1>,
    legal_holds: BTreeMap<[u8; 16], EvidenceViewerLegalHoldV1>,
    retentions: BTreeMap<[u8; 16], EvidenceViewerRetentionRecordV1>,
    default_retention_floors: BTreeMap<[u8; 16], EvidenceViewerDefaultRetentionFloorV1>,
    erasure_intents: BTreeMap<[u8; 16], EvidenceViewerErasureIntentV1>,
    erasures: BTreeMap<[u8; 16], EvidenceViewerErasureRecordV1>,
    idempotency: BTreeMap<[u8; 32], IdempotencyRecordV1>,
    compaction_archive_head: Option<EvidenceViewerSignedCompactionArchiveHeadV1>,
    checkpoint_anchor: Option<EvidenceViewerSignedCheckpointAnchorV1>,
    checkpoint_record: Option<EvidenceViewerCheckpointStoreRecordV1>,
    durability_uncertain: bool,
    authoritative_race_adopted: bool,
}

type VerifiedEvidenceViewerCheckpointV1 = (
    EvidenceViewerCheckpointStoreRecordV1,
    EvidenceViewerCheckpointV1,
    EvidenceViewerSignedCheckpointAnchorV1,
);

/// Production evidence-viewer service.
pub struct EvidenceViewerServiceV1 {
    config: EvidenceViewerConfigV1,
    deps: QualifiedEvidenceViewerRuntimeDepsV1,
    checkpoint_store: QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerCheckpointStoreV1>,
    node: NodeHandle,
    state: Mutex<EvidenceViewerStateV1>,
}

impl fmt::Debug for EvidenceViewerServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvidenceViewerServiceV1")
            .field("config", &self.config)
            .field("deps", &self.deps)
            .field("checkpoint_store", &self.checkpoint_store)
            .field("node", &"<opaque-node-handle>")
            .field("state", &"<payload-free-checkpoint>")
            .finish()
    }
}

/// Evidence-viewer operation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvidenceViewerErrorV1 {
    /// Governed configuration is invalid.
    #[error("invalid evidence-viewer configuration")]
    InvalidConfig,
    /// Request metadata is malformed or out of bounds.
    #[error("invalid evidence-viewer request")]
    InvalidRequest,
    /// Canonical requester lacks an exact finalized assignment or explicit role.
    #[error("evidence-viewer authorization denied")]
    Forbidden,
    /// Challenge, assertion, or grant was replayed, stale, or substituted.
    #[error("evidence-viewer authentication rejected")]
    AuthenticationRejected,
    /// The session or object does not exist.
    #[error("evidence-viewer resource not found")]
    NotFound,
    /// The session expired or was revoked.
    #[error("evidence-viewer session inactive")]
    SessionInactive,
    /// Legal hold takes precedence over erasure.
    #[error("evidence erasure denied by active legal hold")]
    LegalHoldPrecedence,
    /// Governed retention has not yet reached its erasure boundary.
    #[error("evidence erasure denied by active retention")]
    RetentionActive,
    /// A configured resource bound was reached.
    #[error("evidence-viewer resource exhausted")]
    ResourceExhausted,
    /// Runtime provider is unavailable or saturated.
    #[error("evidence-viewer runtime dependency unavailable")]
    RuntimeUnavailable,
    /// Durable checkpoint is malformed, forged, non-canonical, or inconsistent.
    #[error("invalid evidence-viewer checkpoint")]
    InvalidCheckpoint,
    /// The caller's exact signed-checkpoint expectation no longer matches.
    #[error("evidence-viewer checkpoint changed")]
    CheckpointChanged,
    /// Durable state could not be committed.
    #[error("evidence-viewer checkpoint unavailable")]
    CheckpointUnavailable,
    /// The runtime lock is poisoned.
    #[error("evidence-viewer state unavailable")]
    StateUnavailable,
}

#[derive(Debug)]
enum EvidenceViewerCheckpointCommitFailureV1 {
    Unchanged,
    Stale,
    Raced(Box<VerifiedEvidenceViewerCheckpointV1>),
    Ambiguous,
    Unavailable,
}

impl EvidenceViewerCheckpointCommitFailureV1 {
    const fn service_error(&self) -> EvidenceViewerErrorV1 {
        match self {
            Self::Unchanged | Self::Stale | Self::Raced(_) => {
                EvidenceViewerErrorV1::CheckpointChanged
            }
            Self::Ambiguous | Self::Unavailable => EvidenceViewerErrorV1::CheckpointUnavailable,
        }
    }

    const fn makes_state_uncertain(&self) -> bool {
        matches!(self, Self::Ambiguous)
    }
}

impl EvidenceViewerServiceV1 {
    /// Reject provider-less startup for an enabled production service.
    ///
    /// # Errors
    ///
    /// Always returns [`EvidenceViewerErrorV1::CheckpointUnavailable`].
    /// Call [`Self::open_with_checkpoint_store`] with an exact qualified
    /// deployment-owned authority.
    pub fn open(
        _config: EvidenceViewerConfigV1,
        _deps: EvidenceViewerRuntimeDepsV1,
        _node: NodeHandle,
    ) -> Result<Self, EvidenceViewerErrorV1> {
        Err(EvidenceViewerErrorV1::CheckpointUnavailable)
    }

    /// Open against an exact qualified authoritative checkpoint store.
    ///
    /// The external CAS head is authoritative. The configured local file is
    /// accepted only as an exact or one-generation-behind verified cache and
    /// can never initialize or replace the external head.
    ///
    /// # Errors
    ///
    /// Fails closed before local checkpoint access when any provider is
    /// missing, substituted, stale, test-marked, or otherwise unqualified.
    /// Also rejects forged, non-canonical, rolled-back, forked, or
    /// ambiguously committed checkpoint state.
    pub fn open_with_checkpoint_store(
        config: EvidenceViewerConfigV1,
        deps: EvidenceViewerRuntimeDepsV1,
        node: NodeHandle,
        checkpoint_store_handle: String,
        expected_checkpoint_store_qualification: EvidenceViewerRuntimeProviderQualificationV1,
        checkpoint_store: Arc<dyn EvidenceViewerCheckpointStoreV1>,
    ) -> Result<Self, EvidenceViewerErrorV1> {
        config.validate()?;
        let deps = QualifiedEvidenceViewerRuntimeDepsV1::try_new(&config, deps)
            .map_err(map_provider_qualification_error)?;
        let checkpoint_store = QualifiedEvidenceViewerProviderV1::try_new(
            &checkpoint_store_handle,
            expected_checkpoint_store_qualification,
            checkpoint_store,
        )
        .map_err(map_provider_qualification_error)?;
        let service = Self {
            config,
            deps,
            checkpoint_store,
            node,
            state: Mutex::new(EvidenceViewerStateV1::default()),
        };
        let authoritative = service.load_authoritative_checkpoint()?;
        let local = read_local_checkpoint_store_record(&service.config, &service.checkpoint_store)?;
        let refresh_local_cache = match authoritative {
            Some((record, checkpoint, checkpoint_anchor)) => {
                validate_checkpoint_cache_lineage(local.as_ref(), &record)?;
                {
                    let mut state = service
                        .state
                        .lock()
                        .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
                    *state = state_from_checkpoint(
                        &service.config,
                        checkpoint,
                        checkpoint_anchor,
                        record.clone(),
                    )?;
                }
                local.as_ref() != Some(&record)
            }
            None => {
                if local.is_some() {
                    return Err(EvidenceViewerErrorV1::CheckpointChanged);
                }
                let mut state = service
                    .state
                    .lock()
                    .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
                service.persist_locked(&mut state)?;
                false
            }
        };
        service.verify_current_compaction_archive_readback()?;
        service.reconcile_erasure_intents()?;
        if refresh_local_cache {
            let record = service
                .state
                .lock()
                .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?
                .checkpoint_record
                .clone()
                .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
            write_local_checkpoint_store_record(&service.config, &record)?;
        }
        Ok(service)
    }

    /// Return the exact governed policy.
    #[must_use]
    pub fn config(&self) -> &EvidenceViewerConfigV1 {
        &self.config
    }

    /// Issue a single-use, case-bound WebAuthn challenge.
    ///
    /// # Errors
    ///
    /// Rejects malformed requests, missing objects, unauthorized finalized
    /// assignments, reused idempotency keys, resource exhaustion, provider
    /// failure, and checkpoint failure.
    pub fn issue_challenge(
        &self,
        request: EvidenceViewerChallengeRequestV1,
    ) -> Result<EvidenceViewerChallengeIssuedV1, EvidenceViewerErrorV1> {
        validate_challenge_request(&request)?;
        let object = self.object_record(request.quarantine_id)?;
        let authorization = self.authorize(
            &request.case_id,
            &request.round_id,
            &request.viewer_account,
            request.role,
            object.payload_digest,
        )?;
        if authorization.finalized_at_unix_ms > request.now_unix_ms {
            return Err(EvidenceViewerErrorV1::Forbidden);
        }
        let purpose_digest = text_digest(&request.purpose);
        let binding_digest = challenge_binding_digest(
            &request,
            EvidenceViewerChallengeBindingContextV1 {
                object_id: object.object_id,
                evidence_digest: object.payload_digest,
                purpose_digest,
                policy_digest: authorization.policy_digest,
                finalized_height: authorization.finalized_height,
                finalized_block_hash: authorization.finalized_block_hash,
                finalized_at_unix_ms: authorization.finalized_at_unix_ms,
            },
        );
        let request_digest =
            request_binding_digest(b"challenge", &request.idempotency_key, &binding_digest);
        {
            let state = self
                .state
                .lock()
                .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
            self.ensure_authoritative_state_locked(&state)?;
            if let Some(existing) = state.idempotency.get(&request.idempotency_key) {
                if existing.request_digest == request_digest {
                    return Err(EvidenceViewerErrorV1::AuthenticationRejected);
                }
                return Err(EvidenceViewerErrorV1::InvalidRequest);
            }
            if state.challenges.len() >= self.config.max_challenges
                || state.idempotency.len() >= self.config.max_idempotency_records
            {
                return Err(EvidenceViewerErrorV1::ResourceExhausted);
            }
            if state.erasures.contains_key(&request.quarantine_id) {
                return Err(EvidenceViewerErrorV1::InvalidRequest);
            }
        }
        let expires_at_unix_ms = request
            .now_unix_ms
            .checked_add(self.config.challenge_ttl_ms)
            .ok_or(EvidenceViewerErrorV1::InvalidRequest)?;
        let challenge = self
            .deps
            .webauthn
            .issue_challenge(binding_digest, expires_at_unix_ms)
            .map_err(map_external_error)?;
        let challenge_digest = challenge.digest();
        let challenge_id = digest_id16(challenge_digest);
        let record = ChallengeRecordV1 {
            challenge_id,
            challenge_digest,
            binding_digest,
            case_id: request.case_id.clone(),
            round_id: request.round_id.clone(),
            quarantine_id: request.quarantine_id,
            viewer_account_digest: text_digest(&request.viewer_account),
            role: request.role,
            purpose_digest,
            policy_digest: authorization.policy_digest,
            finalized_height: authorization.finalized_height,
            finalized_block_hash: authorization.finalized_block_hash,
            finalized_at_unix_ms: authorization.finalized_at_unix_ms,
            issued_at_unix_ms: request.now_unix_ms,
            expires_at_unix_ms,
            consumed_at_unix_ms: None,
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        if let Some(existing) = state.idempotency.get(&request.idempotency_key) {
            if existing.request_digest == request_digest {
                return Err(EvidenceViewerErrorV1::AuthenticationRejected);
            }
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state.challenges.len() >= self.config.max_challenges
            || state.idempotency.len() >= self.config.max_idempotency_records
        {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        if state.erasures.contains_key(&request.quarantine_id) {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state.challenges.contains_key(&challenge_id) {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        let previous = state.clone();
        state.challenges.insert(challenge_id, record);
        state.idempotency.insert(
            request.idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key: request.idempotency_key,
                request_digest,
                outcome_digest: challenge_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            return Err(error);
        }
        Ok(EvidenceViewerChallengeIssuedV1 {
            challenge_id,
            challenge,
            expires_at_unix_ms,
        })
    }

    /// Verify WebAuthn and create one case-bound session and initial grant.
    ///
    /// # Errors
    ///
    /// Fails closed for replay, stale challenge, substituted assignment,
    /// invalid assertion, resource exhaustion, signer/grant failure, local
    /// object mismatch, or checkpoint failure.
    pub fn create_session(
        &self,
        request: EvidenceViewerSessionRequestV1,
    ) -> Result<EvidenceViewerSessionIssuedV1, EvidenceViewerErrorV1> {
        validate_session_request(&request)?;
        let object = self.object_record(request.quarantine_id)?;
        let purpose_digest = text_digest(&request.purpose);
        let challenge_digest = request.challenge.digest();
        let challenge_id = digest_id16(challenge_digest);
        let challenge_record = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?
            .challenges
            .get(&challenge_id)
            .cloned()
            .ok_or(EvidenceViewerErrorV1::AuthenticationRejected)?;
        let initial_authorization = self.authorize(
            &request.case_id,
            &request.round_id,
            &request.viewer_account,
            request.role,
            object.payload_digest,
        )?;
        if initial_authorization.finalized_at_unix_ms > request.now_unix_ms
            || !authorization_extends_challenge(&initial_authorization, &challenge_record)
        {
            return Err(EvidenceViewerErrorV1::Forbidden);
        }
        let binding_request = EvidenceViewerChallengeRequestV1 {
            case_id: request.case_id.clone(),
            round_id: request.round_id.clone(),
            quarantine_id: request.quarantine_id,
            viewer_account: request.viewer_account.clone(),
            role: request.role,
            purpose: request.purpose.clone(),
            idempotency_key: [0; 32],
            now_unix_ms: request.now_unix_ms,
        };
        let binding_digest = challenge_binding_digest(
            &binding_request,
            EvidenceViewerChallengeBindingContextV1 {
                object_id: object.object_id,
                evidence_digest: object.payload_digest,
                purpose_digest,
                policy_digest: challenge_record.policy_digest,
                finalized_height: challenge_record.finalized_height,
                finalized_block_hash: challenge_record.finalized_block_hash,
                finalized_at_unix_ms: challenge_record.finalized_at_unix_ms,
            },
        );
        let assertion_digest = *blake3::hash(&request.webauthn_assertion).as_bytes();
        let session_request_digest = session_request_digest(
            &request,
            challenge_digest,
            assertion_digest,
            object.object_id,
            object.payload_digest,
            binding_digest,
            initial_authorization.policy_digest,
        );
        {
            let state = self
                .state
                .lock()
                .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
            self.ensure_authoritative_state_locked(&state)?;
            if state.sessions.len() >= self.config.max_sessions
                || state.idempotency.len() >= self.config.max_idempotency_records
                || state.receipts.len() >= self.config.max_receipts
            {
                return Err(EvidenceViewerErrorV1::ResourceExhausted);
            }
            if let Some(existing) = state.idempotency.get(&request.idempotency_key) {
                if existing.request_digest == session_request_digest {
                    return Err(EvidenceViewerErrorV1::AuthenticationRejected);
                }
                return Err(EvidenceViewerErrorV1::InvalidRequest);
            }
            if state
                .sessions
                .values()
                .any(|session| session.webauthn_assertion_digest == assertion_digest)
            {
                return Err(EvidenceViewerErrorV1::AuthenticationRejected);
            }
            if state.erasures.contains_key(&request.quarantine_id) {
                return Err(EvidenceViewerErrorV1::InvalidRequest);
            }
            let current = state
                .challenges
                .get(&challenge_id)
                .cloned()
                .ok_or(EvidenceViewerErrorV1::AuthenticationRejected)?;
            if current != challenge_record {
                return Err(EvidenceViewerErrorV1::AuthenticationRejected);
            }
        }
        if challenge_record.challenge_digest != challenge_digest
            || challenge_record.binding_digest != binding_digest
            || challenge_record.case_id != request.case_id
            || challenge_record.round_id != request.round_id
            || challenge_record.quarantine_id != request.quarantine_id
            || challenge_record.viewer_account_digest != text_digest(&request.viewer_account)
            || challenge_record.role != request.role
            || challenge_record.purpose_digest != purpose_digest
            || challenge_record.consumed_at_unix_ms.is_some()
            || request.now_unix_ms < challenge_record.issued_at_unix_ms
            || request.now_unix_ms >= challenge_record.expires_at_unix_ms
        {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        let webauthn = self
            .deps
            .webauthn
            .verify_and_consume(
                request.challenge.expose(),
                &request.webauthn_assertion,
                binding_digest,
                &self.config.webauthn_rp_id,
                &self.config.webauthn_allowed_origins,
                request.now_unix_ms,
            )
            .map_err(|_| EvidenceViewerErrorV1::AuthenticationRejected)?;
        if is_zero_digest(webauthn.attestation_digest)
            || is_zero_digest(webauthn.credential_id_digest)
            || webauthn.authenticator_counter == 0
        {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        let authorization = self.authorize(
            &request.case_id,
            &request.round_id,
            &request.viewer_account,
            request.role,
            object.payload_digest,
        )?;
        if authorization.finalized_at_unix_ms > request.now_unix_ms
            || !authorization_anchor_extends(
                &authorization,
                initial_authorization.policy_digest,
                initial_authorization.finalized_height,
                initial_authorization.finalized_block_hash,
                initial_authorization.finalized_at_unix_ms,
            )
        {
            return Err(EvidenceViewerErrorV1::Forbidden);
        }
        let expires_at_unix_ms = request
            .now_unix_ms
            .checked_add(self.config.session_ttl_ms)
            .ok_or(EvidenceViewerErrorV1::InvalidRequest)?;
        let watermark_metadata_digest = watermark_digest(
            &request.case_id,
            &request.round_id,
            &request.viewer_account,
            request.role,
            object.object_id,
            request.now_unix_ms,
        );
        let session_nonce_digest = challenge_digest;
        let session_input = ModerationEvidenceViewerSessionInput {
            quarantine_id: request.quarantine_id,
            requested_by: request.viewer_account.clone(),
            viewer_account: request.viewer_account.clone(),
            viewer_role: request.role.as_str().to_owned(),
            purpose: PAYLOAD_FREE_PURPOSE_LABEL_V1.to_owned(),
            attestation_digest: webauthn.attestation_digest,
            watermark_metadata_digest,
            session_nonce_digest,
            issued_at_unix_ms: request.now_unix_ms,
            expires_at_unix_ms,
            legal_hold_id: None,
            notes: None,
            raw_evidence_included: false,
            signed_url_included: false,
            session_token_included: false,
            watermark_secret_included: false,
        };
        let local_session = evidence_viewer_session_record_from_input(session_input, &object)
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        let grant_expires_at_unix_ms = request
            .now_unix_ms
            .checked_add(self.config.grant_ttl_ms)
            .map(|expiry| expiry.min(expires_at_unix_ms))
            .ok_or(EvidenceViewerErrorV1::InvalidRequest)?;
        let claims = EvidenceViewerGrantClaimsV1 {
            session_id: local_session.session_id,
            case_id: request.case_id.clone(),
            round_id: request.round_id.clone(),
            quarantine_id: request.quarantine_id,
            viewer_account: request.viewer_account.clone(),
            role: request.role,
            purpose_digest,
            generation: 1,
            issued_at_unix_ms: request.now_unix_ms,
            expires_at_unix_ms: grant_expires_at_unix_ms,
        };
        let grant = self
            .deps
            .grants
            .issue(&claims)
            .map_err(map_external_error)?;
        let security_record = EvidenceViewerSessionSecurityRecordV1 {
            local_session,
            case_id: request.case_id.clone(),
            round_id: request.round_id.clone(),
            role: request.role,
            purpose_digest,
            credential_id_digest: webauthn.credential_id_digest,
            webauthn_assertion_digest: assertion_digest,
            authenticator_counter: webauthn.authenticator_counter,
            policy_digest: authorization.policy_digest,
            finalized_height: authorization.finalized_height,
            finalized_block_hash: authorization.finalized_block_hash,
            finalized_at_unix_ms: authorization.finalized_at_unix_ms,
            grant_generation: 1,
            active_grant_issued_at_unix_ms: request.now_unix_ms,
            active_grant_digest: grant.digest(),
            active_grant_expires_at_unix_ms: grant_expires_at_unix_ms,
            revoked: false,
        };
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                let _ = self.deps.grants.revoke(grant.digest());
                return Err(EvidenceViewerErrorV1::StateUnavailable);
            }
        };
        if let Err(error) = self.ensure_authoritative_state_locked(&state) {
            let _ = self.deps.grants.revoke(grant.digest());
            return Err(error);
        }
        if let Err(error) = ensure_session_commit_slot(
            &state,
            &self.config,
            request.idempotency_key,
            session_request_digest,
            assertion_digest,
            security_record.local_session.session_id,
            request.quarantine_id,
        ) {
            let _ = self.deps.grants.revoke(grant.digest());
            return Err(error);
        }
        let previous = state.clone();
        let challenge = state
            .challenges
            .get_mut(&challenge_id)
            .ok_or(EvidenceViewerErrorV1::AuthenticationRejected)?;
        if challenge.consumed_at_unix_ms.is_some() {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        challenge.consumed_at_unix_ms = Some(request.now_unix_ms);
        state.sessions.insert(
            security_record.local_session.session_id,
            security_record.clone(),
        );
        let receipt = match self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::SessionIssued,
                session_id: Some(security_record.local_session.session_id),
                case_id: Some(security_record.case_id.clone()),
                round_id: Some(security_record.round_id.clone()),
                quarantine_id: security_record.local_session.quarantine_id,
                object_id: security_record.local_session.object_id,
                evidence_digest: security_record.local_session.evidence_digest,
                actor_account: &security_record.local_session.viewer_account,
                idempotency_key: request.idempotency_key,
                request_digest: session_request_digest,
                range: None,
                issued_at_unix_ms: request.now_unix_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                *state = previous;
                let _ = self.deps.grants.revoke(grant.digest());
                return Err(error);
            }
        };
        state.idempotency.insert(
            request.idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key: request.idempotency_key,
                request_digest: session_request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            let _ = self.deps.grants.revoke(grant.digest());
            return Err(error);
        }
        Ok(EvidenceViewerSessionIssuedV1 {
            session: security_record,
            grant,
            receipt,
        })
    }

    /// Return a payload-free case manifest and rotate the grant.
    ///
    /// # Errors
    ///
    /// Rejects unknown/inactive sessions, invalid or replayed grants,
    /// substituted accounts, authorization revocation, resource exhaustion,
    /// and checkpoint/provider failures.
    pub fn manifest(
        &self,
        session_id: [u8; 16],
        viewer_account: &str,
        grant: &OpaqueEvidenceViewerSecretV1,
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<EvidenceViewerManifestOutcomeV1, EvidenceViewerErrorV1> {
        let session = self.active_session(session_id, viewer_account, now_unix_ms)?;
        let authorization = self.reauthorize_session(&session, now_unix_ms)?;
        let object = self.object_record(session.local_session.quarantine_id)?;
        if object.object_id != session.local_session.object_id
            || object.payload_digest != session.local_session.evidence_digest
        {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        let rotated = self.rotate_grant(&session, grant, now_unix_ms)?;
        let viewer_digest = text_digest(&session.local_session.viewer_account);
        let visible_watermark = format!(
            "CONFIDENTIAL · {} · {} · {}",
            session.role.as_str(),
            hex::encode(&viewer_digest[..8]),
            hex::encode(session.local_session.session_id)
        );
        let manifest = EvidenceViewerManifestV1 {
            version: EVIDENCE_VIEWER_MANIFEST_VERSION_V1,
            session_id,
            case_id: session.case_id.clone(),
            round_id: session.round_id.clone(),
            quarantine_id: session.local_session.quarantine_id,
            object_id: session.local_session.object_id,
            evidence_digest: session.local_session.evidence_digest,
            payload_len: object.payload_len,
            content_type: object.content_type.clone(),
            max_range_bytes: self.config.max_range_bytes,
            role: session.role,
            purpose_digest: session.purpose_digest,
            visible_watermark,
            watermark_metadata_digest: session.local_session.watermark_metadata_digest,
            expires_at_unix_ms: session.local_session.expires_at_unix_ms,
            finalized_height: authorization.finalized_height,
            finalized_block_hash: authorization.finalized_block_hash,
        };
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                let _ = self.deps.grants.revoke(rotated.token.digest());
                return Err(EvidenceViewerErrorV1::StateUnavailable);
            }
        };
        if let Err(error) = self.ensure_authoritative_state_locked(&state) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        if let Err(error) = ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            request_digest,
        ) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let previous = state.clone();
        if let Err(error) = apply_rotated_grant(&mut state, &session, &rotated) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        if let Err(error) = apply_reauthorized_anchor(&mut state, &session, &authorization) {
            *state = previous;
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let receipt = match self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::ManifestAccessed,
                session_id: Some(session_id),
                case_id: Some(session.case_id),
                round_id: Some(session.round_id),
                quarantine_id: session.local_session.quarantine_id,
                object_id: session.local_session.object_id,
                evidence_digest: session.local_session.evidence_digest,
                actor_account: viewer_account,
                idempotency_key,
                request_digest,
                range: None,
                issued_at_unix_ms: now_unix_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                *state = previous;
                let _ = self.deps.grants.revoke(rotated.token.digest());
                return Err(error);
            }
        };
        state.idempotency.insert(
            idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key,
                request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let _ = self.deps.grants.revoke(grant.digest());
        Ok(EvidenceViewerManifestOutcomeV1 {
            manifest,
            rotated_grant: rotated.token,
            receipt,
        })
    }

    /// Authenticate, decrypt, durably log, and return one bounded range.
    ///
    /// The decrypted bytes are returned only after the signed access receipt is
    /// committed. A crash before checkpoint completion therefore cannot release
    /// an unlogged response.
    ///
    /// # Errors
    ///
    /// Rejects invalid ranges, inactive sessions, stale/replayed grants,
    /// authorization changes, object substitution, provider failures, and
    /// checkpoint failures.
    #[allow(clippy::too_many_arguments)]
    pub fn read_range(
        &self,
        session_id: [u8; 16],
        viewer_account: &str,
        grant: &OpaqueEvidenceViewerSecretV1,
        start: u64,
        end: u64,
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<EvidenceViewerRangeOutcomeV1, EvidenceViewerErrorV1> {
        if start >= end
            || end.saturating_sub(start) > self.config.max_range_bytes
            || is_zero_digest(idempotency_key)
            || is_zero_digest(request_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let session = self.active_session(session_id, viewer_account, now_unix_ms)?;
        let authorization = self.reauthorize_session(&session, now_unix_ms)?;
        let rotated = self.rotate_grant(&session, grant, now_unix_ms)?;
        let range = self
            .node
            .read_moderation_quarantine_object_range(
                session.local_session.quarantine_id,
                start,
                end,
            )
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        if range.record.object_id != session.local_session.object_id
            || range.record.payload_digest != session.local_session.evidence_digest
            || range.start != start
            || range.end != end
            || u64::try_from(range.payload.len()).unwrap_or(u64::MAX) != end - start
        {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                let _ = self.deps.grants.revoke(rotated.token.digest());
                return Err(EvidenceViewerErrorV1::StateUnavailable);
            }
        };
        if let Err(error) = self.ensure_authoritative_state_locked(&state) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        if let Err(error) = ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            request_digest,
        ) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let previous = state.clone();
        if let Err(error) = apply_rotated_grant(&mut state, &session, &rotated) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        if let Err(error) = apply_reauthorized_anchor(&mut state, &session, &authorization) {
            *state = previous;
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let receipt = match self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::RangeAccessed,
                session_id: Some(session_id),
                case_id: Some(session.case_id),
                round_id: Some(session.round_id),
                quarantine_id: session.local_session.quarantine_id,
                object_id: session.local_session.object_id,
                evidence_digest: session.local_session.evidence_digest,
                actor_account: viewer_account,
                idempotency_key,
                request_digest,
                range: Some((start, end)),
                issued_at_unix_ms: now_unix_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                *state = previous;
                let _ = self.deps.grants.revoke(rotated.token.digest());
                return Err(error);
            }
        };
        state.idempotency.insert(
            idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key,
                request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let _ = self.deps.grants.revoke(grant.digest());
        Ok(EvidenceViewerRangeOutcomeV1 {
            range,
            watermark_metadata_digest: session.local_session.watermark_metadata_digest,
            rotated_grant: rotated.token,
            receipt,
        })
    }

    /// Record a bounded browser interaction and rotate the grant.
    ///
    /// # Errors
    ///
    /// Applies the same authentication, authorization, idempotency, signing,
    /// and durability checks as [`Self::read_range`].
    #[allow(clippy::too_many_arguments)]
    pub fn record_interaction(
        &self,
        session_id: [u8; 16],
        viewer_account: &str,
        grant: &OpaqueEvidenceViewerSecretV1,
        kind: ModerationEvidenceViewerAccessKind,
        event_metadata_digest: Option<[u8; 32]>,
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<(OpaqueEvidenceViewerSecretV1, EvidenceViewerSignedReceiptV1), EvidenceViewerErrorV1>
    {
        if matches!(kind, ModerationEvidenceViewerAccessKind::SessionExpired)
            || event_metadata_digest.is_some_and(is_zero_digest)
            || is_zero_digest(idempotency_key)
            || is_zero_digest(request_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let session = self.active_session(session_id, viewer_account, now_unix_ms)?;
        let authorization = self.reauthorize_session(&session, now_unix_ms)?;
        let rotated = self.rotate_grant(&session, grant, now_unix_ms)?;
        let mut event_hasher = blake3::Hasher::new();
        event_hasher.update(REQUEST_BINDING_DOMAIN_V1);
        event_hasher.update(kind.as_str().as_bytes());
        event_hasher.update(&request_digest);
        if let Some(digest) = event_metadata_digest {
            event_hasher.update(&digest);
        }
        let event_digest = *event_hasher.finalize().as_bytes();
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                let _ = self.deps.grants.revoke(rotated.token.digest());
                return Err(EvidenceViewerErrorV1::StateUnavailable);
            }
        };
        if let Err(error) = self.ensure_authoritative_state_locked(&state) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        if let Err(error) = ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            request_digest,
        ) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let previous = state.clone();
        if let Err(error) = apply_rotated_grant(&mut state, &session, &rotated) {
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        if let Err(error) = apply_reauthorized_anchor(&mut state, &session, &authorization) {
            *state = previous;
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let receipt = match self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::InteractionRecorded,
                session_id: Some(session_id),
                case_id: Some(session.case_id),
                round_id: Some(session.round_id),
                quarantine_id: session.local_session.quarantine_id,
                object_id: session.local_session.object_id,
                evidence_digest: session.local_session.evidence_digest,
                actor_account: viewer_account,
                idempotency_key,
                request_digest: event_digest,
                range: None,
                issued_at_unix_ms: now_unix_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                *state = previous;
                let _ = self.deps.grants.revoke(rotated.token.digest());
                return Err(error);
            }
        };
        state.idempotency.insert(
            idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key,
                request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            let _ = self.deps.grants.revoke(rotated.token.digest());
            return Err(error);
        }
        let _ = self.deps.grants.revoke(grant.digest());
        Ok((rotated.token, receipt))
    }

    /// Place a legal hold. Only an exact finalized legal authorization is
    /// accepted.
    ///
    /// # Errors
    ///
    /// Rejects non-legal roles, malformed authority digests, object mismatch,
    /// duplicate/conflicting holds, signing failure, and checkpoint failure.
    #[allow(clippy::too_many_arguments)]
    pub fn place_legal_hold(
        &self,
        case_id: &str,
        round_id: &str,
        quarantine_id: [u8; 16],
        legal_account: &str,
        authority_digest: [u8; 32],
        idempotency_key: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<(EvidenceViewerLegalHoldV1, EvidenceViewerSignedReceiptV1), EvidenceViewerErrorV1>
    {
        if is_zero_digest(authority_digest) || is_zero_digest(idempotency_key) || now_unix_ms == 0 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let object = self.object_record(quarantine_id)?;
        self.authorize(
            case_id,
            round_id,
            legal_account,
            EvidenceViewerRoleV1::Legal,
            object.payload_digest,
        )?;
        let hold_digest = legal_hold_digest(
            quarantine_id,
            object.object_id,
            object.payload_digest,
            authority_digest,
        );
        let hold = EvidenceViewerLegalHoldV1 {
            hold_id: digest_id16(hold_digest),
            quarantine_id,
            object_id: object.object_id,
            evidence_digest: object.payload_digest,
            authority_digest,
            placed_at_unix_ms: now_unix_ms,
            released_at_unix_ms: None,
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            hold_digest,
        )?;
        if state.erasures.contains_key(&quarantine_id) {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state.legal_holds.contains_key(&hold.hold_id) {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state.legal_holds.values().any(|existing| {
            existing.quarantine_id == quarantine_id && existing.released_at_unix_ms.is_none()
        }) {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state.legal_holds.len() >= self.config.max_sessions {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        let previous = state.clone();
        state.legal_holds.insert(hold.hold_id, hold.clone());
        let receipt = match self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::LegalHoldPlaced,
                session_id: None,
                case_id: Some(case_id.to_owned()),
                round_id: Some(round_id.to_owned()),
                quarantine_id,
                object_id: object.object_id,
                evidence_digest: object.payload_digest,
                actor_account: legal_account,
                idempotency_key,
                request_digest: hold_digest,
                range: None,
                issued_at_unix_ms: now_unix_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                *state = previous;
                return Err(error);
            }
        };
        state.idempotency.insert(
            idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key,
                request_digest: hold_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            return Err(error);
        }
        Ok((hold, receipt))
    }

    /// Release one active legal hold under an exact finalized legal role.
    ///
    /// # Errors
    ///
    /// Rejects unknown/already released holds, non-legal accounts, replayed or
    /// conflicting idempotency, signer failure, and checkpoint failure.
    #[allow(clippy::too_many_arguments)]
    pub fn release_legal_hold(
        &self,
        case_id: &str,
        round_id: &str,
        hold_id: [u8; 16],
        legal_account: &str,
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<(EvidenceViewerLegalHoldV1, EvidenceViewerSignedReceiptV1), EvidenceViewerErrorV1>
    {
        if hold_id == [0; 16]
            || is_zero_digest(idempotency_key)
            || is_zero_digest(request_digest)
            || now_unix_ms == 0
        {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let hold = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?
            .legal_holds
            .get(&hold_id)
            .cloned()
            .ok_or(EvidenceViewerErrorV1::NotFound)?;
        if hold.released_at_unix_ms.is_some() {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        self.authorize(
            case_id,
            round_id,
            legal_account,
            EvidenceViewerRoleV1::Legal,
            hold.evidence_digest,
        )?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            request_digest,
        )?;
        let previous = state.clone();
        let released = state
            .legal_holds
            .get_mut(&hold_id)
            .ok_or(EvidenceViewerErrorV1::NotFound)?;
        if released.released_at_unix_ms.is_some() || now_unix_ms < released.placed_at_unix_ms {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        released.released_at_unix_ms = Some(now_unix_ms);
        let released = released.clone();
        let receipt = match self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::LegalHoldReleased,
                session_id: None,
                case_id: Some(case_id.to_owned()),
                round_id: Some(round_id.to_owned()),
                quarantine_id: released.quarantine_id,
                object_id: released.object_id,
                evidence_digest: released.evidence_digest,
                actor_account: legal_account,
                idempotency_key,
                request_digest,
                range: None,
                issued_at_unix_ms: now_unix_ms,
            },
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                *state = previous;
                return Err(error);
            }
        };
        state.idempotency.insert(
            idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key,
                request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            return Err(error);
        }
        Ok((released, receipt))
    }

    /// Record one signed retention decision under an exact finalized legal
    /// authorization.
    ///
    /// An active legal hold is captured as precedence in the signed record and
    /// always keeps the object out of the due-erasure projection.
    ///
    /// # Errors
    ///
    /// Rejects malformed intervals, non-legal accounts, conflicting
    /// idempotency, signer failure, and checkpoint failure.
    #[allow(clippy::too_many_arguments)]
    pub fn record_retention(
        &self,
        case_id: &str,
        round_id: &str,
        quarantine_id: [u8; 16],
        legal_account: &str,
        retain_until_unix_ms: u64,
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<
        (
            EvidenceViewerRetentionRecordV1,
            EvidenceViewerSignedReceiptV1,
        ),
        EvidenceViewerErrorV1,
    > {
        if retain_until_unix_ms < now_unix_ms
            || now_unix_ms == 0
            || is_zero_digest(idempotency_key)
            || is_zero_digest(request_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let object = self.object_record(quarantine_id)?;
        self.authorize(
            case_id,
            round_id,
            legal_account,
            EvidenceViewerRoleV1::Legal,
            object.payload_digest,
        )?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            request_digest,
        )?;
        if state.erasures.contains_key(&quarantine_id) {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state.retentions.len() >= self.config.max_sessions
            && !state.retentions.contains_key(&quarantine_id)
        {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        let legal_hold_precedence = state
            .legal_holds
            .values()
            .any(|hold| hold.quarantine_id == quarantine_id && hold.released_at_unix_ms.is_none());
        let previous = state.clone();
        let receipt = self.append_receipt_locked(
            &mut state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::RetentionEvaluated,
                session_id: None,
                case_id: Some(case_id.to_owned()),
                round_id: Some(round_id.to_owned()),
                quarantine_id,
                object_id: object.object_id,
                evidence_digest: object.payload_digest,
                actor_account: legal_account,
                idempotency_key,
                request_digest,
                range: None,
                issued_at_unix_ms: now_unix_ms,
            },
        )?;
        let retention = EvidenceViewerRetentionRecordV1 {
            quarantine_id,
            object_id: object.object_id,
            evidence_digest: object.payload_digest,
            retain_until_unix_ms,
            legal_hold_precedence,
            evaluated_at_unix_ms: now_unix_ms,
            receipt_digest: receipt.receipt_digest,
        };
        state.retentions.insert(quarantine_id, retention.clone());
        state.idempotency.insert(
            idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key,
                request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            return Err(error);
        }
        Ok((retention, receipt))
    }

    /// Erase an object unless an active legal hold has precedence.
    ///
    /// Both successful erasure and legal-hold denial produce signed,
    /// payload-free receipts.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceViewerErrorV1::LegalHoldPrecedence`] after durably
    /// recording a denial receipt, returns
    /// [`EvidenceViewerErrorV1::RetentionActive`] without calling the erasure
    /// boundary before the governed deadline, and fails closed for
    /// authorization, dependency, signing, or checkpoint failures.
    #[allow(clippy::too_many_arguments)]
    pub fn erase(
        &self,
        case_id: &str,
        round_id: &str,
        quarantine_id: [u8; 16],
        legal_account: &str,
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        now_unix_ms: u64,
    ) -> Result<(EvidenceViewerErasureRecordV1, EvidenceViewerSignedReceiptV1), EvidenceViewerErrorV1>
    {
        if is_zero_digest(idempotency_key) || is_zero_digest(request_digest) || now_unix_ms == 0 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let object = self.object_record(quarantine_id)?;
        self.authorize(
            case_id,
            round_id,
            legal_account,
            EvidenceViewerRoleV1::Legal,
            object.payload_digest,
        )?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        ensure_new_idempotency(
            &state,
            self.config.max_idempotency_records,
            idempotency_key,
            request_digest,
        )?;
        if state.erasures.contains_key(&quarantine_id) {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if state
            .legal_holds
            .values()
            .any(|hold| hold.quarantine_id == quarantine_id && hold.released_at_unix_ms.is_none())
        {
            let previous = state.clone();
            let receipt = self.append_receipt_locked(
                &mut state,
                ReceiptSpecV1 {
                    kind: EvidenceViewerReceiptKindV1::ErasureDeniedLegalHold,
                    session_id: None,
                    case_id: Some(case_id.to_owned()),
                    round_id: Some(round_id.to_owned()),
                    quarantine_id,
                    object_id: object.object_id,
                    evidence_digest: object.payload_digest,
                    actor_account: legal_account,
                    idempotency_key,
                    request_digest,
                    range: None,
                    issued_at_unix_ms: now_unix_ms,
                },
            )?;
            state.idempotency.insert(
                idempotency_key,
                IdempotencyRecordV1 {
                    idempotency_key,
                    request_digest,
                    outcome_digest: receipt.receipt_digest,
                },
            );
            if let Err(error) = self.persist_locked(&mut state) {
                if can_restore_process_local_snapshot(&state) {
                    *state = previous;
                }
                return Err(error);
            }
            return Err(EvidenceViewerErrorV1::LegalHoldPrecedence);
        }
        let retain_until_unix_ms = retention_deadline_for(&state, &self.config, quarantine_id)
            .ok_or(EvidenceViewerErrorV1::RetentionActive)?;
        if now_unix_ms < retain_until_unix_ms {
            return Err(EvidenceViewerErrorV1::RetentionActive);
        }
        if state.erasures.len() >= self.config.max_sessions {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        if state.erasure_intents.len() >= self.config.max_sessions {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        let operation_id = erasure_operation_id(
            idempotency_key,
            request_digest,
            quarantine_id,
            object.object_id,
            object.payload_digest,
        );
        let intent = EvidenceViewerErasureIntentV1 {
            operation_id,
            quarantine_id,
            object_id: object.object_id,
            evidence_digest: object.payload_digest,
            case_id: case_id.to_owned(),
            round_id: round_id.to_owned(),
            actor_account: legal_account.to_owned(),
            idempotency_key,
            request_digest,
            requested_at_unix_ms: now_unix_ms,
        };
        let previous = state.clone();
        if state
            .erasure_intents
            .insert(quarantine_id, intent.clone())
            .is_some()
        {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            return Err(error);
        }
        // Retain the state lock across the irreversible boundary so a legal
        // hold cannot race between precedence evaluation and erasure commit.
        let erasure_commit_digest = match self.deps.erasure.erase(
            operation_id,
            quarantine_id,
            object.object_id,
            object.payload_digest,
        ) {
            Ok(digest) if !is_zero_digest(digest) => digest,
            Ok(_) => {
                state.durability_uncertain = true;
                return Err(EvidenceViewerErrorV1::RuntimeUnavailable);
            }
            Err(error) => {
                // The provider result can be ambiguous. Leave the durable
                // intent in place and force a restart/reconciliation before
                // any evidence can be served again.
                state.durability_uncertain = true;
                return Err(map_external_error(error));
            }
        };
        let (erasure, receipt) =
            match self.finalize_erasure_locked(&mut state, &intent, erasure_commit_digest) {
                Ok(result) => result,
                Err(error) => {
                    state.durability_uncertain = true;
                    return Err(error);
                }
            };
        if let Err(error) = self.persist_locked(&mut state) {
            // The irreversible boundary has already committed. The durable
            // write-ahead intent makes restart recovery an idempotent replay.
            state.durability_uncertain = true;
            return Err(error);
        }
        Ok((erasure, receipt))
    }

    fn reconcile_erasure_intents(&self) -> Result<(), EvidenceViewerErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        if state.erasure_intents.is_empty() {
            return Ok(());
        }
        let intents = state.erasure_intents.values().cloned().collect::<Vec<_>>();
        for intent in intents {
            let erasure_commit_digest = match self.deps.erasure.erase(
                intent.operation_id,
                intent.quarantine_id,
                intent.object_id,
                intent.evidence_digest,
            ) {
                Ok(digest) if !is_zero_digest(digest) => digest,
                Ok(_) => {
                    state.durability_uncertain = true;
                    return Err(EvidenceViewerErrorV1::RuntimeUnavailable);
                }
                Err(error) => {
                    state.durability_uncertain = true;
                    return Err(map_external_error(error));
                }
            };
            if let Err(error) =
                self.finalize_erasure_locked(&mut state, &intent, erasure_commit_digest)
            {
                state.durability_uncertain = true;
                return Err(error);
            }
        }
        if let Err(error) = self.persist_locked(&mut state) {
            state.durability_uncertain = true;
            return Err(error);
        }
        Ok(())
    }

    fn finalize_erasure_locked(
        &self,
        state: &mut EvidenceViewerStateV1,
        intent: &EvidenceViewerErasureIntentV1,
        erasure_commit_digest: [u8; 32],
    ) -> Result<(EvidenceViewerErasureRecordV1, EvidenceViewerSignedReceiptV1), EvidenceViewerErrorV1>
    {
        if is_zero_digest(erasure_commit_digest)
            || state.erasures.contains_key(&intent.quarantine_id)
            || state.idempotency.contains_key(&intent.idempotency_key)
            || state.legal_holds.values().any(|hold| {
                hold.quarantine_id == intent.quarantine_id && hold.released_at_unix_ms.is_none()
            })
            || state
                .erasure_intents
                .get(&intent.quarantine_id)
                .is_none_or(|pending| pending != intent)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        let receipt = self.append_receipt_locked(
            state,
            ReceiptSpecV1 {
                kind: EvidenceViewerReceiptKindV1::ErasureCompleted,
                session_id: None,
                case_id: Some(intent.case_id.clone()),
                round_id: Some(intent.round_id.clone()),
                quarantine_id: intent.quarantine_id,
                object_id: intent.object_id,
                evidence_digest: intent.evidence_digest,
                actor_account: &intent.actor_account,
                idempotency_key: intent.idempotency_key,
                request_digest: erasure_commit_digest,
                range: None,
                issued_at_unix_ms: intent.requested_at_unix_ms,
            },
        )?;
        let erasure = EvidenceViewerErasureRecordV1 {
            quarantine_id: intent.quarantine_id,
            object_id: intent.object_id,
            evidence_digest: intent.evidence_digest,
            erasure_commit_digest,
            erased_at_unix_ms: intent.requested_at_unix_ms,
            receipt_digest: receipt.receipt_digest,
        };
        state.erasures.insert(intent.quarantine_id, erasure.clone());
        for session in state
            .sessions
            .values_mut()
            .filter(|session| session.local_session.quarantine_id == intent.quarantine_id)
        {
            session.revoked = true;
            let _ = self.deps.grants.revoke(session.active_grant_digest);
        }
        state.idempotency.insert(
            intent.idempotency_key,
            IdempotencyRecordV1 {
                idempotency_key: intent.idempotency_key,
                request_digest: intent.request_digest,
                outcome_digest: receipt.receipt_digest,
            },
        );
        state.erasure_intents.remove(&intent.quarantine_id);
        state.default_retention_floors.remove(&intent.quarantine_id);
        Ok((erasure, receipt))
    }

    /// Return a bounded deterministic list of evidence objects whose governed
    /// retention deadline has elapsed and which are not protected by a legal
    /// hold.
    ///
    /// This method does not erase data. A supervised worker must pass each
    /// candidate through [`Self::erase`], preserving finalized legal
    /// authorization, idempotency, signed receipts, and the erasure boundary.
    ///
    /// # Errors
    ///
    /// Rejects invalid limits and unavailable or durability-uncertain state.
    pub fn retention_due(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<[u8; 16]>, EvidenceViewerErrorV1> {
        if now_unix_ms == 0 || limit == 0 || limit > 1_024 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        let active_holds = state
            .legal_holds
            .values()
            .filter(|hold| hold.released_at_unix_ms.is_none())
            .map(|hold| hold.quarantine_id)
            .collect::<BTreeSet<_>>();
        let candidates = state
            .sessions
            .values()
            .map(|session| session.local_session.quarantine_id)
            .chain(state.retentions.keys().copied())
            .chain(state.default_retention_floors.keys().copied())
            .collect::<BTreeSet<_>>();
        Ok(candidates
            .into_iter()
            .filter(|quarantine_id| {
                !active_holds.contains(quarantine_id)
                    && !state.erasures.contains_key(quarantine_id)
                    && retention_deadline_for(&state, &self.config, *quarantine_id)
                        .is_some_and(|retention_end| now_unix_ms >= retention_end)
            })
            .take(limit)
            .collect())
    }

    /// Return bounded signed receipts after an exclusive sequence.
    ///
    /// # Errors
    ///
    /// Rejects zero/over-sized limits and unavailable state.
    pub fn receipts(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<EvidenceViewerSignedReceiptV1>, EvidenceViewerErrorV1> {
        if limit == 0 || limit > 1_024 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        let after = after_sequence.unwrap_or(0);
        Ok(state
            .receipts
            .iter()
            .filter(|receipt| receipt.body.sequence > after)
            .take(limit)
            .cloned()
            .collect())
    }

    /// Refresh one fenced replica from the exact authoritative checkpoint head.
    ///
    /// The current in-memory head and the local cache must each be either the
    /// authoritative record or its exact signed predecessor. This explicit
    /// handoff never accepts an unrelated replacement or an unverified local
    /// checkpoint. Any current compaction head and its complete historical
    /// lineage are read back and verified before the local cache is replaced or
    /// the in-memory state becomes visible. Any retained erasure intent is then
    /// reconciled through its stable external operation identifier before
    /// success.
    ///
    /// # Errors
    ///
    /// Fails closed for a missing, forged, rolled-back, forked, unavailable, or
    /// more-than-one-generation-ahead authoritative head, missing or corrupt
    /// compaction history, a local-cache write failure, or an erasure-intent
    /// reconciliation failure.
    pub fn refresh_authoritative_checkpoint(
        &self,
    ) -> Result<EvidenceViewerSignedCheckpointAnchorV1, EvidenceViewerErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        let (record, checkpoint, checkpoint_anchor) = self
            .load_authoritative_checkpoint()?
            .ok_or(EvidenceViewerErrorV1::CheckpointChanged)?;
        validate_checkpoint_cache_lineage(state.checkpoint_record.as_ref(), &record)?;
        let local = read_local_checkpoint_store_record(&self.config, &self.checkpoint_store)?;
        validate_checkpoint_cache_lineage(local.as_ref(), &record)?;
        let refreshed_state =
            state_from_checkpoint(&self.config, checkpoint, checkpoint_anchor, record.clone())?;
        if let Some(head) = refreshed_state.compaction_archive_head.as_ref() {
            self.verify_compaction_archive_lineage(head)?;
        }
        if local.as_ref() != Some(&record) {
            write_local_checkpoint_store_record(&self.config, &record)?;
        }
        *state = refreshed_state;
        drop(state);

        self.reconcile_erasure_intents()?;
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        validated_checkpoint_anchor(&self.config, &state)
    }

    /// Durably archive and then prune a bounded expired-record prefix.
    ///
    /// The caller fences the exact signed checkpoint and archive predecessor.
    /// The deployment-owned archive is independently qualified before use. A
    /// deterministic operation identifier and signer-authenticated monotonic
    /// head bind the source checkpoint, predecessor, provider identity, cutoff,
    /// work bound, and exact canonical payload. The service requires exact
    /// canonical readback before changing local state or attempting the
    /// authoritative checkpoint CAS.
    ///
    /// Exact retries after an ambiguous archive install or a failed checkpoint
    /// commit reuse the same operation identifier and artifact. A completed
    /// transition may also be replayed while it remains the current archive
    /// head; the archive is read and verified again without another prune.
    ///
    /// # Errors
    ///
    /// Rejects invalid bounds, stale/forked fences, substituted or stale
    /// archives, forged/non-canonical/trailing readback, generation overflow,
    /// unavailable signing/archive/checkpoint providers, and empty eligible
    /// record sets.
    pub fn compact_expired_with_archive(
        &self,
        request: EvidenceViewerCompactionArchiveRequestV1,
    ) -> Result<EvidenceViewerSignedCompactionArchiveHeadV1, EvidenceViewerErrorV1> {
        validate_compaction_archive_request(&self.config, &request)?;
        let archive = &self.deps.compaction_archive;
        let mut state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;

        if let Some(head) = state.compaction_archive_head.as_ref()
            && compaction_archive_head_matches_request(head, &request, archive)
        {
            self.verify_compaction_archive_lineage(head)?;
            return Ok(head.clone());
        }

        let source_anchor = validated_checkpoint_anchor(&self.config, &state)?;
        let predecessor_head_digest = state
            .compaction_archive_head
            .as_ref()
            .map(|head| head.head_digest);
        if source_anchor != request.expected_checkpoint_anchor
            || predecessor_head_digest != request.expected_archive_head_digest
        {
            return Err(EvidenceViewerErrorV1::CheckpointChanged);
        }
        let source_record = state
            .checkpoint_record
            .as_ref()
            .cloned()
            .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
        if source_record.checkpoint_digest != source_anchor.checkpoint_digest {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        if let Some(predecessor) = state.compaction_archive_head.as_ref() {
            self.verify_compaction_archive_lineage(predecessor)?;
        }

        let maximum_records = usize::try_from(request.maximum_records)
            .map_err(|_| EvidenceViewerErrorV1::InvalidRequest)?;
        let challenges = state
            .challenges
            .values()
            .filter(|record| record.expires_at_unix_ms <= request.compacted_through_unix_ms)
            .take(maximum_records)
            .cloned()
            .collect::<Vec<_>>();
        let remaining = maximum_records.saturating_sub(challenges.len());
        let sessions = state
            .sessions
            .values()
            .filter(|record| {
                record.local_session.expires_at_unix_ms <= request.compacted_through_unix_ms
            })
            .take(remaining)
            .cloned()
            .collect::<Vec<_>>();
        if challenges.is_empty() && sessions.is_empty() {
            return Err(EvidenceViewerErrorV1::NotFound);
        }
        let projected_retention_floors =
            projected_default_retention_floors(&state, &self.config, &sessions)?;
        let payload = EvidenceViewerCompactionArchivePayloadV1 {
            version: EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1,
            challenges,
            sessions,
        };
        let (generation, predecessor_head_digest, predecessor_operation_id) =
            match state.compaction_archive_head.as_ref() {
                Some(predecessor) => (
                    predecessor
                        .generation
                        .checked_add(1)
                        .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?,
                    Some(predecessor.head_digest),
                    Some(predecessor.operation_id),
                ),
                None => (1, None, None),
            };
        let mut head = EvidenceViewerSignedCompactionArchiveHeadV1 {
            version: EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1,
            generation,
            predecessor_head_digest,
            predecessor_operation_id,
            operation_id: [0; 32],
            source_checkpoint_generation: source_record.generation,
            source_checkpoint_revision: source_record.revision,
            source_checkpoint_anchor: source_anchor,
            compacted_through_unix_ms: request.compacted_through_unix_ms,
            maximum_records: request.maximum_records,
            challenge_count: u32::try_from(payload.challenges.len())
                .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?,
            session_count: u32::try_from(payload.sessions.len())
                .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?,
            compacted_payload_digest: compaction_archive_payload_digest(&payload)?,
            archive_handle: archive.handle().to_owned(),
            archive_revision: archive.qualification().revision(),
            archive_policy_digest: archive.qualification().policy_digest(),
            archive_id: archive.archive_id,
            archive_public_key: archive.public_key,
            signer_handle: self.config.receipt_signer_handle.clone(),
            signer_public_key: self.config.receipt_signer_public_key,
            signature: [0; 64],
            head_digest: [0; 32],
            archive_signature: [0; 64],
        };
        head.operation_id = compaction_archive_operation_id(&head)?;
        head.signature = self
            .deps
            .receipt_signer
            .sign(&compaction_archive_signature_message(&head)?)
            .map_err(map_external_error)?;
        head.head_digest = compaction_archive_head_digest(&head)?;
        verify_compaction_archive_head_core(
            &head,
            &self.config.receipt_signer_handle,
            self.config.receipt_signer_public_key,
        )
        .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        if let Some(predecessor) = state.compaction_archive_head.as_ref() {
            verify_compaction_archive_lineage_link(&head, predecessor)?;
        }
        let artifact = EvidenceViewerCompactionArchiveArtifactV1 {
            version: EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1,
            head: head.clone(),
            payload,
        };
        let artifact_bytes =
            norito::to_bytes(&artifact).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        if artifact_bytes.is_empty()
            || len_u64(artifact_bytes.len()) > compaction_archive_max_bytes(&self.config)
        {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        let verified_artifact =
            verify_compaction_archive_artifact(&self.config, archive, &artifact_bytes)?;
        if verified_artifact != artifact {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }

        let install_result = archive.install(
            head.operation_id,
            compaction_archive_receipt_message(&head),
            &artifact_bytes,
        );
        let readback = archive.read(head.operation_id);
        let readback = match readback {
            Ok(Some(readback)) => readback,
            Ok(None) => {
                return Err(install_result.as_ref().err().copied().map_or(
                    EvidenceViewerErrorV1::RuntimeUnavailable,
                    map_archive_external_error,
                ));
            }
            Err(error) => return Err(map_archive_external_error(error)),
        };
        if let Ok(install_signature) = install_result
            && install_signature != readback.signature
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        let installed = verify_compaction_archive_artifact(
            &self.config,
            archive,
            &readback.canonical_artifact,
        )?;
        if installed != artifact {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        head.archive_signature = readback.signature;
        head.verify(
            &self.config.receipt_signer_handle,
            self.config.receipt_signer_public_key,
        )?;

        let previous = state.clone();
        let archived_challenge_digests = artifact
            .payload
            .challenges
            .iter()
            .map(|record| record.challenge_digest)
            .collect::<BTreeSet<_>>();
        if artifact
            .payload
            .challenges
            .iter()
            .any(|record| state.challenges.get(&record.challenge_id) != Some(record))
            || artifact
                .payload
                .sessions
                .iter()
                .any(|record| state.sessions.get(&record.local_session.session_id) != Some(record))
        {
            return Err(EvidenceViewerErrorV1::CheckpointChanged);
        }
        for challenge in &artifact.payload.challenges {
            let _ = state.challenges.remove(&challenge.challenge_id);
        }
        state.default_retention_floors = projected_retention_floors;
        for session in &artifact.payload.sessions {
            let _ = state.sessions.remove(&session.local_session.session_id);
        }
        for record in state.idempotency.values_mut() {
            if archived_challenge_digests.contains(&record.outcome_digest)
                || artifact.head.predecessor_head_digest == Some(record.outcome_digest)
            {
                record.outcome_digest = artifact.head.head_digest;
            }
        }
        state.compaction_archive_head = Some(head.clone());
        if let Err(error) = self.persist_locked(&mut state) {
            if can_restore_process_local_snapshot(&state) {
                *state = previous;
            }
            return Err(error);
        }
        Ok(head)
    }

    /// Run one bounded supervised compaction tick against the current exact
    /// checkpoint and archive head.
    ///
    /// The returned `None` means that no expired challenge or session was
    /// eligible. A concurrent checkpoint/archive change is surfaced to the
    /// supervisor so it can retry on the next configured cadence.
    ///
    /// # Errors
    ///
    /// Fails closed for a zero timestamp, stale authority, provider failure,
    /// invalid archive readback, or durable checkpoint failure.
    pub fn compact_expired_tick(
        &self,
        now_unix_ms: u64,
    ) -> Result<Option<EvidenceViewerSignedCompactionArchiveHeadV1>, EvidenceViewerErrorV1> {
        if now_unix_ms == 0 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let request = {
            let state = self
                .state
                .lock()
                .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
            self.ensure_authoritative_state_locked(&state)?;
            EvidenceViewerCompactionArchiveRequestV1 {
                expected_checkpoint_anchor: validated_checkpoint_anchor(&self.config, &state)?,
                expected_archive_head_digest: state
                    .compaction_archive_head
                    .as_ref()
                    .map(|head| head.head_digest),
                compacted_through_unix_ms: now_unix_ms,
                maximum_records: self.config.compaction_max_records,
            }
        };
        match self.compact_expired_with_archive(request) {
            Ok(head) => Ok(Some(head)),
            Err(EvidenceViewerErrorV1::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Return the bounded cadence for the shutdown-aware compaction worker.
    #[must_use]
    pub const fn compaction_interval_ms(&self) -> u64 {
        self.config.compaction_interval_ms
    }

    fn verify_current_compaction_archive_readback(&self) -> Result<(), EvidenceViewerErrorV1> {
        let head = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?
            .compaction_archive_head
            .clone();
        if let Some(head) = head.as_ref() {
            self.verify_compaction_archive_lineage(head)?;
        }
        Ok(())
    }

    fn verify_compaction_archive_lineage(
        &self,
        head: &EvidenceViewerSignedCompactionArchiveHeadV1,
    ) -> Result<(), EvidenceViewerErrorV1> {
        let mut current = self.load_verified_compaction_archive_head(head.operation_id)?;
        if current != *head {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        while current.generation > 1 {
            let predecessor_operation_id = current
                .predecessor_operation_id
                .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
            let predecessor =
                self.load_verified_compaction_archive_head(predecessor_operation_id)?;
            verify_compaction_archive_lineage_link(&current, &predecessor)?;
            current = predecessor;
        }
        Ok(())
    }

    fn load_verified_compaction_archive_head(
        &self,
        operation_id: [u8; 32],
    ) -> Result<EvidenceViewerSignedCompactionArchiveHeadV1, EvidenceViewerErrorV1> {
        let readback = self
            .deps
            .compaction_archive
            .read(operation_id)
            .map_err(map_archive_external_error)?
            .ok_or(EvidenceViewerErrorV1::RuntimeUnavailable)?;
        let artifact = verify_compaction_archive_artifact(
            &self.config,
            &self.deps.compaction_archive,
            &readback.canonical_artifact,
        )?;
        if artifact.head.operation_id != operation_id {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        let mut head = artifact.head;
        head.archive_signature = readback.signature;
        head.verify(
            &self.config.receipt_signer_handle,
            self.config.receipt_signer_public_key,
        )?;
        Ok(head)
    }

    /// Return the exact signed compaction-archive head retained by the current
    /// authoritative checkpoint.
    ///
    /// # Errors
    ///
    /// Fails closed for unavailable, stale, or malformed checkpoint state.
    pub fn compaction_archive_head(
        &self,
    ) -> Result<Option<EvidenceViewerSignedCompactionArchiveHeadV1>, EvidenceViewerErrorV1> {
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        if let Some(head) = state.compaction_archive_head.as_ref() {
            head.verify(
                &self.config.receipt_signer_handle,
                self.config.receipt_signer_public_key,
            )?;
        }
        Ok(state.compaction_archive_head.clone())
    }

    /// Return a bounded, exact-cursor projection of the durable signed receipt
    /// chain for transparency publication or replica readback.
    ///
    /// Unlike the retired local session/access registry, this projection is
    /// rebuilt exclusively from the authenticated production checkpoint. A
    /// consumer cursor must match both the sequence and digest already retained
    /// by this service, so rollback and same-sequence substitution fail closed.
    ///
    /// # Errors
    ///
    /// Rejects a zero/oversized limit, a malformed or unknown predecessor,
    /// unavailable state, and canonical projection-encoding failures.
    pub fn transparency_projection(
        &self,
        expected_checkpoint_digest: [u8; 32],
        predecessor: Option<EvidenceViewerReceiptCursorV1>,
        limit: usize,
    ) -> Result<EvidenceViewerTransparencyProjectionV1, EvidenceViewerErrorV1> {
        if is_zero_digest(expected_checkpoint_digest) || limit == 0 || limit > 1_024 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let page_limit = u16::try_from(limit).map_err(|_| EvidenceViewerErrorV1::InvalidRequest)?;
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        let checkpoint_anchor = validated_checkpoint_anchor(&self.config, &state)?;
        if checkpoint_anchor.checkpoint_digest != expected_checkpoint_digest {
            return Err(EvidenceViewerErrorV1::CheckpointChanged);
        }
        let start = match predecessor {
            None => 0,
            Some(cursor) => {
                if cursor.sequence == 0 || is_zero_digest(cursor.receipt_digest) {
                    return Err(EvidenceViewerErrorV1::InvalidRequest);
                }
                let index = usize::try_from(cursor.sequence.saturating_sub(1))
                    .map_err(|_| EvidenceViewerErrorV1::InvalidRequest)?;
                let receipt = state
                    .receipts
                    .get(index)
                    .ok_or(EvidenceViewerErrorV1::InvalidRequest)?;
                if receipt.body.sequence != cursor.sequence
                    || receipt.receipt_digest != cursor.receipt_digest
                {
                    return Err(EvidenceViewerErrorV1::InvalidRequest);
                }
                index
                    .checked_add(1)
                    .ok_or(EvidenceViewerErrorV1::InvalidRequest)?
            }
        };
        let receipts = state
            .receipts
            .iter()
            .skip(start)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let has_more = start
            .checked_add(receipts.len())
            .is_some_and(|consumed| consumed < state.receipts.len());
        let next_cursor = receipts.last().map(receipt_cursor).or(predecessor);
        let compaction_archive_head = state.compaction_archive_head.clone();
        let projection_digest = transparency_projection_digest(
            &checkpoint_anchor,
            compaction_archive_head.as_ref(),
            predecessor,
            page_limit,
            &receipts,
            next_cursor,
            has_more,
        )?;
        Ok(EvidenceViewerTransparencyProjectionV1 {
            version: EVIDENCE_VIEWER_TRANSPARENCY_PROJECTION_VERSION_V1,
            checkpoint_anchor,
            compaction_archive_head,
            predecessor,
            page_limit,
            receipts,
            next_cursor,
            has_more,
            projection_digest,
        })
    }

    /// Return a payload-free status projection.
    ///
    /// # Errors
    ///
    /// Fails if state is unavailable, durability is uncertain, or canonical
    /// status encoding fails.
    pub fn audit_status(&self) -> Result<EvidenceViewerAuditStatusV1, EvidenceViewerErrorV1> {
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        let checkpoint_anchor = validated_checkpoint_anchor(&self.config, &state)?;
        Ok(EvidenceViewerAuditStatusV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            challenge_count: len_u64(state.challenges.len()),
            session_count: len_u64(state.sessions.len()),
            receipt_count: len_u64(state.receipts.len()),
            active_legal_hold_count: len_u64(
                state
                    .legal_holds
                    .values()
                    .filter(|hold| hold.released_at_unix_ms.is_none())
                    .count(),
            ),
            erasure_count: len_u64(state.erasures.len()),
            retention_count: len_u64(state.retentions.len()),
            checkpoint_anchor,
        })
    }

    fn object_record(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectRecord, EvidenceViewerErrorV1> {
        self.node
            .export_moderation_quarantine_object_snapshot()
            .map_err(|error| match error {
                ModerationQuarantineObjectError::StateLockPoisoned => {
                    EvidenceViewerErrorV1::StateUnavailable
                }
                _ => EvidenceViewerErrorV1::RuntimeUnavailable,
            })?
            .objects
            .into_iter()
            .find(|record| record.quarantine_id == quarantine_id)
            .ok_or(EvidenceViewerErrorV1::NotFound)
    }

    fn authorize(
        &self,
        case_id: &str,
        round_id: &str,
        viewer_account: &str,
        role: EvidenceViewerRoleV1,
        evidence_digest: [u8; 32],
    ) -> Result<EvidenceViewerFinalizedAuthorizationV1, EvidenceViewerErrorV1> {
        let authorization = self
            .deps
            .authorization_reader
            .authorize(case_id, round_id, viewer_account, role, evidence_digest)
            .map_err(|error| match error {
                EvidenceViewerAuthorizationErrorV1::Denied => EvidenceViewerErrorV1::Forbidden,
                EvidenceViewerAuthorizationErrorV1::Unavailable
                | EvidenceViewerAuthorizationErrorV1::ResourceExhausted => {
                    EvidenceViewerErrorV1::RuntimeUnavailable
                }
            })?;
        if authorization.case_id != case_id
            || authorization.round_id != round_id
            || authorization.viewer_account != viewer_account
            || authorization.role != role
            || authorization.evidence_bundle_digest != evidence_digest
            || authorization.finalized_height == 0
            || is_zero_digest(authorization.finalized_block_hash)
            || authorization.finalized_at_unix_ms == 0
            || is_zero_digest(authorization.policy_digest)
        {
            return Err(EvidenceViewerErrorV1::Forbidden);
        }
        Ok(authorization)
    }

    fn active_session(
        &self,
        session_id: [u8; 16],
        viewer_account: &str,
        now_unix_ms: u64,
    ) -> Result<EvidenceViewerSessionSecurityRecordV1, EvidenceViewerErrorV1> {
        validate_label(viewer_account)?;
        if now_unix_ms == 0 {
            return Err(EvidenceViewerErrorV1::InvalidRequest);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| EvidenceViewerErrorV1::StateUnavailable)?;
        self.ensure_authoritative_state_locked(&state)?;
        let session = state
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or(EvidenceViewerErrorV1::NotFound)?;
        if session.local_session.viewer_account != viewer_account
            || session.revoked
            || now_unix_ms < session.local_session.issued_at_unix_ms
            || now_unix_ms >= session.local_session.expires_at_unix_ms
            || state
                .erasures
                .contains_key(&session.local_session.quarantine_id)
        {
            return Err(EvidenceViewerErrorV1::SessionInactive);
        }
        Ok(session)
    }

    fn reauthorize_session(
        &self,
        session: &EvidenceViewerSessionSecurityRecordV1,
        now_unix_ms: u64,
    ) -> Result<EvidenceViewerFinalizedAuthorizationV1, EvidenceViewerErrorV1> {
        let authorization = self.authorize(
            &session.case_id,
            &session.round_id,
            &session.local_session.viewer_account,
            session.role,
            session.local_session.evidence_digest,
        )?;
        if authorization.finalized_at_unix_ms > now_unix_ms
            || !authorization_anchor_extends(
                &authorization,
                session.policy_digest,
                session.finalized_height,
                session.finalized_block_hash,
                session.finalized_at_unix_ms,
            )
        {
            return Err(EvidenceViewerErrorV1::Forbidden);
        }
        Ok(authorization)
    }

    fn rotate_grant(
        &self,
        session: &EvidenceViewerSessionSecurityRecordV1,
        token: &OpaqueEvidenceViewerSecretV1,
        now_unix_ms: u64,
    ) -> Result<RotatedGrantV1, EvidenceViewerErrorV1> {
        if token.digest() != session.active_grant_digest
            || now_unix_ms >= session.active_grant_expires_at_unix_ms
        {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        let current_claims = grant_claims(session, now_unix_ms, false)?;
        self.deps
            .grants
            .verify(token.expose(), &current_claims, now_unix_ms)
            .map_err(|_| EvidenceViewerErrorV1::AuthenticationRejected)?;
        let generation = session
            .grant_generation
            .checked_add(1)
            .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(self.config.grant_ttl_ms)
            .map(|expiry| expiry.min(session.local_session.expires_at_unix_ms))
            .ok_or(EvidenceViewerErrorV1::InvalidRequest)?;
        if expires_at_unix_ms <= now_unix_ms {
            return Err(EvidenceViewerErrorV1::SessionInactive);
        }
        let claims = EvidenceViewerGrantClaimsV1 {
            generation,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
            ..current_claims
        };
        let replacement = self
            .deps
            .grants
            .issue(&claims)
            .map_err(map_external_error)?;
        Ok(RotatedGrantV1 {
            token: replacement,
            generation,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
        })
    }

    fn append_receipt_locked(
        &self,
        state: &mut EvidenceViewerStateV1,
        spec: ReceiptSpecV1<'_>,
    ) -> Result<EvidenceViewerSignedReceiptV1, EvidenceViewerErrorV1> {
        if state.receipts.len() >= self.config.max_receipts {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        let sequence = state
            .receipts
            .last()
            .map_or(Some(1), |receipt| receipt.body.sequence.checked_add(1))
            .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?;
        let previous_receipt_digest = state
            .receipts
            .last()
            .map_or([0; 32], |receipt| receipt.receipt_digest);
        let body = EvidenceViewerReceiptBodyV1 {
            version: EVIDENCE_VIEWER_RECEIPT_VERSION_V1,
            sequence,
            kind: spec.kind,
            session_id: spec.session_id,
            case_id: spec.case_id,
            round_id: spec.round_id,
            quarantine_id: spec.quarantine_id,
            object_id: spec.object_id,
            evidence_digest: spec.evidence_digest,
            actor_account_digest: text_digest(spec.actor_account),
            idempotency_key_digest: *blake3::hash(&spec.idempotency_key).as_bytes(),
            request_digest: spec.request_digest,
            range_start: spec.range.map(|range| range.0),
            range_end: spec.range.map(|range| range.1),
            issued_at_unix_ms: spec.issued_at_unix_ms,
            previous_receipt_digest,
        };
        let receipt_digest = receipt_body_digest(&body)?;
        let signature = self
            .deps
            .receipt_signer
            .sign(&receipt_signature_message(receipt_digest))
            .map_err(map_external_error)?;
        let receipt = EvidenceViewerSignedReceiptV1 {
            body,
            receipt_digest,
            signer_handle: self.config.receipt_signer_handle.clone(),
            signer_public_key: self.config.receipt_signer_public_key,
            signature,
        };
        receipt
            .verify(
                &self.config.receipt_signer_handle,
                self.config.receipt_signer_public_key,
            )
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        state.receipts.push(receipt.clone());
        Ok(receipt)
    }

    fn load_authoritative_checkpoint(
        &self,
    ) -> Result<Option<VerifiedEvidenceViewerCheckpointV1>, EvidenceViewerErrorV1> {
        self.checkpoint_store
            .load_latest()
            .map_err(map_checkpoint_store_external_error)?
            .map(|record| {
                let (checkpoint, checkpoint_anchor) =
                    verify_checkpoint_store_record(&self.config, &self.checkpoint_store, &record)?;
                Ok((record, checkpoint, checkpoint_anchor))
            })
            .transpose()
    }

    fn ensure_authoritative_state_locked(
        &self,
        state: &EvidenceViewerStateV1,
    ) -> Result<(), EvidenceViewerErrorV1> {
        ensure_durability(state)?;
        let expected = state
            .checkpoint_record
            .as_ref()
            .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
        let latest = self
            .checkpoint_store
            .load_latest()
            .map_err(map_checkpoint_store_external_error)?;
        if latest.as_ref() == Some(expected) {
            return Ok(());
        }
        if let Some(record) = latest.as_ref() {
            verify_checkpoint_store_record(&self.config, &self.checkpoint_store, record)?;
        }
        Err(EvidenceViewerErrorV1::CheckpointChanged)
    }

    fn sign_checkpoint_store_record(
        &self,
        checkpoint_digest: [u8; 32],
        checkpoint_bytes: Vec<u8>,
        predecessor: Option<&EvidenceViewerCheckpointStoreRecordV1>,
    ) -> Result<EvidenceViewerCheckpointStoreRecordV1, EvidenceViewerErrorV1> {
        let (generation, predecessor_revision, predecessor_checkpoint_digest) = match predecessor {
            Some(record) => (
                record
                    .generation
                    .checked_add(1)
                    .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?,
                Some(record.revision),
                Some(record.checkpoint_digest),
            ),
            None => (1, None, None),
        };
        let mut record = EvidenceViewerCheckpointStoreRecordV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_STORE_RECORD_VERSION_V1,
            generation,
            predecessor_revision,
            predecessor_checkpoint_digest,
            checkpoint_digest,
            checkpoint_bytes,
            checkpoint_store_handle: self.checkpoint_store.handle.clone(),
            checkpoint_store_revision: self.checkpoint_store.qualification.revision(),
            checkpoint_store_policy_digest: self.checkpoint_store.qualification.policy_digest(),
            signer_handle: self.config.receipt_signer_handle.clone(),
            signer_public_key: self.config.receipt_signer_public_key,
            signature: [0; 64],
            revision: [0; 32],
        };
        let signature = self
            .deps
            .receipt_signer
            .sign(&checkpoint_store_record_signature_message(&record))
            .map_err(map_external_error)?;
        record.signature = signature;
        record.revision = checkpoint_store_record_revision(&record);
        verify_checkpoint_store_record(&self.config, &self.checkpoint_store, &record)?;
        Ok(record)
    }

    fn commit_authoritative_checkpoint(
        &self,
        expected: Option<&EvidenceViewerCheckpointStoreRecordV1>,
        next: &EvidenceViewerCheckpointStoreRecordV1,
    ) -> Result<(), EvidenceViewerCheckpointCommitFailureV1> {
        verify_checkpoint_store_record(&self.config, &self.checkpoint_store, next)
            .map_err(|_| EvidenceViewerCheckpointCommitFailureV1::Unavailable)?;
        let before = self
            .checkpoint_store
            .load_latest()
            .map_err(|_| EvidenceViewerCheckpointCommitFailureV1::Unavailable)?;
        if let Some(record) = before.as_ref() {
            verify_checkpoint_store_record(&self.config, &self.checkpoint_store, record)
                .map_err(|_| EvidenceViewerCheckpointCommitFailureV1::Stale)?;
        }
        if before.as_ref() != expected {
            return Err(EvidenceViewerCheckpointCommitFailureV1::Stale);
        }

        let cas_result = self
            .checkpoint_store
            .compare_and_swap_latest(expected.map(|record| record.revision), next);
        let readback = self
            .checkpoint_store
            .load_latest()
            .map_err(|_| EvidenceViewerCheckpointCommitFailureV1::Ambiguous)?;
        let verified_readback = readback
            .as_ref()
            .map(|record| {
                verify_checkpoint_store_record(&self.config, &self.checkpoint_store, record)
            })
            .transpose()
            .map_err(|_| EvidenceViewerCheckpointCommitFailureV1::Ambiguous)?;
        if readback.as_ref() == Some(next) {
            return Ok(());
        }
        if readback.as_ref() == expected {
            return if cas_result.is_err() {
                Err(EvidenceViewerCheckpointCommitFailureV1::Unchanged)
            } else {
                Err(EvidenceViewerCheckpointCommitFailureV1::Ambiguous)
            };
        }
        if cas_result.is_ok() {
            return Err(EvidenceViewerCheckpointCommitFailureV1::Ambiguous);
        }
        let (Some(record), Some((checkpoint, checkpoint_anchor))) = (readback, verified_readback)
        else {
            return Err(EvidenceViewerCheckpointCommitFailureV1::Ambiguous);
        };
        if !checkpoint_store_record_is_direct_successor(expected, &record) {
            return Err(EvidenceViewerCheckpointCommitFailureV1::Ambiguous);
        }
        Err(EvidenceViewerCheckpointCommitFailureV1::Raced(Box::new((
            record,
            checkpoint,
            checkpoint_anchor,
        ))))
    }

    fn persist_locked(
        &self,
        state: &mut EvidenceViewerStateV1,
    ) -> Result<(), EvidenceViewerErrorV1> {
        state.authoritative_race_adopted = false;
        let checkpoint = checkpoint_from_state(state);
        validate_checkpoint(&self.config, &checkpoint)?;
        let checkpoint_digest = checkpoint_payload_digest(&checkpoint)?;
        let receipt_count = u64::try_from(checkpoint.receipts.len())
            .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?;
        let chain_head = checkpoint.receipts.last().map(receipt_cursor);
        let compaction_archive_head_digest = checkpoint
            .compaction_archive_head
            .as_ref()
            .map(|head| head.head_digest);
        let previous_record = state.checkpoint_record.clone();
        let (checkpoint_generation, predecessor_checkpoint_revision, predecessor_checkpoint_digest) =
            match previous_record.as_ref() {
                Some(record) => (
                    record
                        .generation
                        .checked_add(1)
                        .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?,
                    Some(record.revision),
                    Some(record.checkpoint_digest),
                ),
                None => (1, None, None),
            };
        let mut checkpoint_anchor = EvidenceViewerSignedCheckpointAnchorV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            checkpoint_generation,
            predecessor_checkpoint_revision,
            predecessor_checkpoint_digest,
            checkpoint_digest,
            receipt_count,
            chain_head,
            compaction_archive_head_digest,
            checkpoint_store_handle: self.checkpoint_store.handle.clone(),
            checkpoint_store_revision: self.checkpoint_store.qualification.revision(),
            checkpoint_store_policy_digest: self.checkpoint_store.qualification.policy_digest(),
            signer_handle: self.config.receipt_signer_handle.clone(),
            signer_public_key: self.config.receipt_signer_public_key,
            signature: [0; 64],
        };
        let signature = self
            .deps
            .receipt_signer
            .sign(&checkpoint_anchor_signature_message(&checkpoint_anchor))
            .map_err(map_external_error)?;
        checkpoint_anchor.signature = signature;
        checkpoint_anchor
            .verify(
                &self.config.receipt_signer_handle,
                self.config.receipt_signer_public_key,
            )
            .map_err(|_| EvidenceViewerErrorV1::RuntimeUnavailable)?;
        let envelope = EvidenceViewerCheckpointEnvelopeV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            checkpoint,
            checkpoint_anchor: checkpoint_anchor.clone(),
        };
        verify_checkpoint_envelope(&self.config, envelope.clone())?;
        let checkpoint_bytes =
            norito::to_bytes(&envelope).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        if len_u64(checkpoint_bytes.len()) > self.config.checkpoint_max_bytes {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        let next_record = self.sign_checkpoint_store_record(
            checkpoint_digest,
            checkpoint_bytes,
            previous_record.as_ref(),
        )?;
        match self.commit_authoritative_checkpoint(previous_record.as_ref(), &next_record) {
            Ok(()) => {}
            Err(EvidenceViewerCheckpointCommitFailureV1::Raced(authoritative)) => {
                let (record, checkpoint, checkpoint_anchor) = *authoritative;
                let mut adopted = match state_from_checkpoint(
                    &self.config,
                    checkpoint,
                    checkpoint_anchor,
                    record.clone(),
                ) {
                    Ok(adopted) => adopted,
                    Err(_) => {
                        state.durability_uncertain = true;
                        return Err(EvidenceViewerErrorV1::CheckpointUnavailable);
                    }
                };
                if let Some(head) = adopted.compaction_archive_head.as_ref()
                    && self.verify_compaction_archive_lineage(head).is_err()
                {
                    state.durability_uncertain = true;
                    return Err(EvidenceViewerErrorV1::CheckpointUnavailable);
                }
                if write_local_checkpoint_store_record(&self.config, &record).is_err() {
                    adopted.durability_uncertain = true;
                    *state = adopted;
                    return Err(EvidenceViewerErrorV1::CheckpointUnavailable);
                }
                adopted.authoritative_race_adopted = true;
                *state = adopted;
                return Err(EvidenceViewerErrorV1::CheckpointChanged);
            }
            Err(failure) => {
                if failure.makes_state_uncertain() {
                    state.durability_uncertain = true;
                }
                return Err(failure.service_error());
            }
        }

        state.checkpoint_anchor = Some(checkpoint_anchor);
        state.checkpoint_record = Some(next_record.clone());
        if write_local_checkpoint_store_record(&self.config, &next_record).is_err() {
            state.durability_uncertain = true;
            return Err(EvidenceViewerErrorV1::CheckpointUnavailable);
        }
        Ok(())
    }
}

struct ReceiptSpecV1<'a> {
    kind: EvidenceViewerReceiptKindV1,
    session_id: Option<[u8; 16]>,
    case_id: Option<String>,
    round_id: Option<String>,
    quarantine_id: [u8; 16],
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
    actor_account: &'a str,
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
    range: Option<(u64, u64)>,
    issued_at_unix_ms: u64,
}

struct RotatedGrantV1 {
    token: OpaqueEvidenceViewerSecretV1,
    generation: u64,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

fn apply_rotated_grant(
    state: &mut EvidenceViewerStateV1,
    expected: &EvidenceViewerSessionSecurityRecordV1,
    rotated: &RotatedGrantV1,
) -> Result<(), EvidenceViewerErrorV1> {
    if state
        .erasures
        .contains_key(&expected.local_session.quarantine_id)
    {
        return Err(EvidenceViewerErrorV1::SessionInactive);
    }
    let session = state
        .sessions
        .get_mut(&expected.local_session.session_id)
        .ok_or(EvidenceViewerErrorV1::NotFound)?;
    if session.revoked
        || session.grant_generation != expected.grant_generation
        || session.active_grant_issued_at_unix_ms != expected.active_grant_issued_at_unix_ms
        || session.active_grant_digest != expected.active_grant_digest
        || session.active_grant_expires_at_unix_ms != expected.active_grant_expires_at_unix_ms
    {
        return Err(EvidenceViewerErrorV1::AuthenticationRejected);
    }
    session.grant_generation = rotated.generation;
    session.active_grant_issued_at_unix_ms = rotated.issued_at_unix_ms;
    session.active_grant_digest = rotated.token.digest();
    session.active_grant_expires_at_unix_ms = rotated.expires_at_unix_ms;
    Ok(())
}

fn apply_reauthorized_anchor(
    state: &mut EvidenceViewerStateV1,
    expected: &EvidenceViewerSessionSecurityRecordV1,
    authorization: &EvidenceViewerFinalizedAuthorizationV1,
) -> Result<(), EvidenceViewerErrorV1> {
    let session = state
        .sessions
        .get_mut(&expected.local_session.session_id)
        .ok_or(EvidenceViewerErrorV1::NotFound)?;
    if session.case_id != expected.case_id
        || session.round_id != expected.round_id
        || session.local_session.viewer_account != expected.local_session.viewer_account
        || session.role != expected.role
        || session.local_session.evidence_digest != expected.local_session.evidence_digest
        || session.policy_digest != expected.policy_digest
        || session.finalized_height != expected.finalized_height
        || session.finalized_block_hash != expected.finalized_block_hash
        || session.finalized_at_unix_ms != expected.finalized_at_unix_ms
    {
        return Err(EvidenceViewerErrorV1::AuthenticationRejected);
    }
    if authorization.case_id != session.case_id
        || authorization.round_id != session.round_id
        || authorization.viewer_account != session.local_session.viewer_account
        || authorization.role != session.role
        || authorization.evidence_bundle_digest != session.local_session.evidence_digest
        || !authorization_anchor_extends(
            authorization,
            session.policy_digest,
            session.finalized_height,
            session.finalized_block_hash,
            session.finalized_at_unix_ms,
        )
    {
        return Err(EvidenceViewerErrorV1::Forbidden);
    }
    session.finalized_height = authorization.finalized_height;
    session.finalized_block_hash = authorization.finalized_block_hash;
    session.finalized_at_unix_ms = authorization.finalized_at_unix_ms;
    Ok(())
}

fn authorization_extends_challenge(
    authorization: &EvidenceViewerFinalizedAuthorizationV1,
    challenge: &ChallengeRecordV1,
) -> bool {
    authorization_anchor_extends(
        authorization,
        challenge.policy_digest,
        challenge.finalized_height,
        challenge.finalized_block_hash,
        challenge.finalized_at_unix_ms,
    )
}

fn authorization_anchor_extends(
    authorization: &EvidenceViewerFinalizedAuthorizationV1,
    expected_policy_digest: [u8; 32],
    expected_height: u64,
    expected_block_hash: [u8; 32],
    expected_finalized_at_unix_ms: u64,
) -> bool {
    if authorization.policy_digest != expected_policy_digest {
        return false;
    }
    match authorization.finalized_height.cmp(&expected_height) {
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => {
            authorization.finalized_block_hash == expected_block_hash
                && authorization.finalized_at_unix_ms == expected_finalized_at_unix_ms
        }
        std::cmp::Ordering::Greater => {
            authorization.finalized_at_unix_ms >= expected_finalized_at_unix_ms
        }
    }
}

fn grant_claims(
    session: &EvidenceViewerSessionSecurityRecordV1,
    now_unix_ms: u64,
    replacement: bool,
) -> Result<EvidenceViewerGrantClaimsV1, EvidenceViewerErrorV1> {
    let generation = if replacement {
        session
            .grant_generation
            .checked_add(1)
            .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?
    } else {
        session.grant_generation
    };
    Ok(EvidenceViewerGrantClaimsV1 {
        session_id: session.local_session.session_id,
        case_id: session.case_id.clone(),
        round_id: session.round_id.clone(),
        quarantine_id: session.local_session.quarantine_id,
        viewer_account: session.local_session.viewer_account.clone(),
        role: session.role,
        purpose_digest: session.purpose_digest,
        generation,
        issued_at_unix_ms: if replacement {
            now_unix_ms
        } else {
            session.active_grant_issued_at_unix_ms
        },
        expires_at_unix_ms: session.active_grant_expires_at_unix_ms,
    })
}

fn checkpoint_from_state(state: &EvidenceViewerStateV1) -> EvidenceViewerCheckpointV1 {
    EvidenceViewerCheckpointV1 {
        version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
        challenges: state.challenges.values().cloned().collect(),
        sessions: state.sessions.values().cloned().collect(),
        receipts: state.receipts.clone(),
        legal_holds: state.legal_holds.values().cloned().collect(),
        retentions: state.retentions.values().cloned().collect(),
        default_retention_floors: state.default_retention_floors.values().cloned().collect(),
        erasure_intents: state.erasure_intents.values().cloned().collect(),
        erasures: state.erasures.values().cloned().collect(),
        idempotency: state.idempotency.values().cloned().collect(),
        compaction_archive_head: state.compaction_archive_head.clone(),
    }
}

fn checkpoint_payload_digest(
    checkpoint: &EvidenceViewerCheckpointV1,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let bytes =
        norito::to_bytes(checkpoint).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHECKPOINT_DIGEST_DOMAIN_V1);
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn checkpoint_sequence_limit(config: &EvidenceViewerConfigV1) -> usize {
    config
        .max_challenges
        .saturating_add(config.max_sessions)
        .saturating_add(config.max_receipts)
        .saturating_add(config.max_idempotency_records)
        .saturating_add(config.max_sessions.saturating_mul(5))
}

fn checkpoint_store_record_max_bytes(config: &EvidenceViewerConfigV1) -> u64 {
    config
        .checkpoint_max_bytes
        .saturating_add(CHECKPOINT_STORE_RECORD_MAX_OVERHEAD_BYTES_V1)
}

fn checkpoint_store_record_sequence_limit(config: &EvidenceViewerConfigV1) -> usize {
    checkpoint_sequence_limit(config)
        .max(usize::try_from(config.checkpoint_max_bytes).unwrap_or(usize::MAX))
}

fn compaction_archive_max_bytes(config: &EvidenceViewerConfigV1) -> u64 {
    config
        .checkpoint_max_bytes
        .saturating_add(COMPACTION_ARCHIVE_MAX_OVERHEAD_BYTES_V1)
}

fn compaction_archive_sequence_limit(config: &EvidenceViewerConfigV1) -> usize {
    usize::try_from(config.compaction_max_records)
        .unwrap_or(usize::MAX)
        .max(1)
}

fn hash_optional_checkpoint_digest(hasher: &mut blake3::Hasher, digest: Option<[u8; 32]>) {
    match digest {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(&digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_checkpoint_store_record_fields(
    hasher: &mut blake3::Hasher,
    record: &EvidenceViewerCheckpointStoreRecordV1,
) {
    hasher.update(&record.version.to_le_bytes());
    hasher.update(&record.generation.to_le_bytes());
    hash_optional_checkpoint_digest(hasher, record.predecessor_revision);
    hash_optional_checkpoint_digest(hasher, record.predecessor_checkpoint_digest);
    hasher.update(&record.checkpoint_digest);
    hasher.update(&len_u64(record.checkpoint_bytes.len()).to_le_bytes());
    hasher.update(&record.checkpoint_bytes);
    hasher.update(&len_u64(record.checkpoint_store_handle.len()).to_le_bytes());
    hasher.update(record.checkpoint_store_handle.as_bytes());
    hasher.update(&record.checkpoint_store_revision.to_le_bytes());
    hasher.update(&record.checkpoint_store_policy_digest);
    hasher.update(&len_u64(record.signer_handle.len()).to_le_bytes());
    hasher.update(record.signer_handle.as_bytes());
    hasher.update(&record.signer_public_key);
}

fn checkpoint_store_record_signature_message(
    record: &EvidenceViewerCheckpointStoreRecordV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHECKPOINT_STORE_RECORD_SIGNATURE_DOMAIN_V1);
    hash_checkpoint_store_record_fields(&mut hasher, record);
    *hasher.finalize().as_bytes()
}

fn checkpoint_store_record_revision(record: &EvidenceViewerCheckpointStoreRecordV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHECKPOINT_STORE_RECORD_REVISION_DOMAIN_V1);
    hash_checkpoint_store_record_fields(&mut hasher, record);
    hasher.update(&record.signature);
    *hasher.finalize().as_bytes()
}

fn compaction_archive_payload_digest(
    payload: &EvidenceViewerCompactionArchivePayloadV1,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let bytes = norito::to_bytes(payload).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTION_ARCHIVE_PAYLOAD_DOMAIN_V1);
    hasher.update(&len_u64(bytes.len()).to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn hash_compaction_archive_head_fields(
    hasher: &mut blake3::Hasher,
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> Result<(), EvidenceViewerErrorV1> {
    hasher.update(&head.version.to_le_bytes());
    hasher.update(&head.generation.to_le_bytes());
    hash_optional_checkpoint_digest(hasher, head.predecessor_head_digest);
    hash_optional_checkpoint_digest(hasher, head.predecessor_operation_id);
    hasher.update(&head.source_checkpoint_generation.to_le_bytes());
    hasher.update(&head.source_checkpoint_revision);
    let anchor_bytes = norito::to_bytes(&head.source_checkpoint_anchor)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    hasher.update(&len_u64(anchor_bytes.len()).to_le_bytes());
    hasher.update(&anchor_bytes);
    hasher.update(&head.compacted_through_unix_ms.to_le_bytes());
    hasher.update(&head.maximum_records.to_le_bytes());
    hasher.update(&head.challenge_count.to_le_bytes());
    hasher.update(&head.session_count.to_le_bytes());
    hasher.update(&head.compacted_payload_digest);
    hasher.update(&len_u64(head.archive_handle.len()).to_le_bytes());
    hasher.update(head.archive_handle.as_bytes());
    hasher.update(&head.archive_revision.to_le_bytes());
    hasher.update(&head.archive_policy_digest);
    hasher.update(&head.archive_id);
    hasher.update(&head.archive_public_key);
    hasher.update(&len_u64(head.signer_handle.len()).to_le_bytes());
    hasher.update(head.signer_handle.as_bytes());
    hasher.update(&head.signer_public_key);
    Ok(())
}

fn compaction_archive_operation_id(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTION_ARCHIVE_OPERATION_DOMAIN_V1);
    hash_compaction_archive_head_fields(&mut hasher, head)?;
    Ok(*hasher.finalize().as_bytes())
}

fn compaction_archive_signature_message(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTION_ARCHIVE_SIGNATURE_DOMAIN_V1);
    hash_compaction_archive_head_fields(&mut hasher, head)?;
    hasher.update(&head.operation_id);
    Ok(*hasher.finalize().as_bytes())
}

fn compaction_archive_head_digest(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTION_ARCHIVE_HEAD_DOMAIN_V1);
    hash_compaction_archive_head_fields(&mut hasher, head)?;
    hasher.update(&head.operation_id);
    hasher.update(&head.signature);
    Ok(*hasher.finalize().as_bytes())
}

fn compaction_archive_receipt_message(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(COMPACTION_ARCHIVE_RECEIPT_DOMAIN_V1);
    hasher.update(&head.archive_id);
    hasher.update(&head.archive_public_key);
    hasher.update(&head.operation_id);
    hasher.update(&head.head_digest);
    *hasher.finalize().as_bytes()
}

fn verify_compaction_archive_head(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
    expected_signer_handle: &str,
    expected_signer_public_key: [u8; 32],
) -> Result<(), EvidenceViewerErrorV1> {
    verify_compaction_archive_head_core(head, expected_signer_handle, expected_signer_public_key)?;
    if head.archive_signature == [0; 64] {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    let key = PublicKey::from_bytes(Algorithm::Ed25519, &head.archive_public_key)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let signature = IrohaSignature::try_from_bytes(&head.archive_signature)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    signature
        .verify(&key, &compaction_archive_receipt_message(head))
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)
}

fn verify_compaction_archive_head_core(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
    expected_signer_handle: &str,
    expected_signer_public_key: [u8; 32],
) -> Result<(), EvidenceViewerErrorV1> {
    let lineage_is_valid = match head.generation {
        1 => head.predecessor_head_digest.is_none() && head.predecessor_operation_id.is_none(),
        2.. => {
            head.predecessor_head_digest
                .is_some_and(|digest| !is_zero_digest(digest))
                && head
                    .predecessor_operation_id
                    .is_some_and(|operation_id| !is_zero_digest(operation_id))
        }
        0 => false,
    };
    let record_count = head
        .challenge_count
        .checked_add(head.session_count)
        .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
    if head.version != EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1
        || !lineage_is_valid
        || is_zero_digest(head.operation_id)
        || head.source_checkpoint_generation == 0
        || head.source_checkpoint_generation != head.source_checkpoint_anchor.checkpoint_generation
        || is_zero_digest(head.source_checkpoint_revision)
        || head.compacted_through_unix_ms == 0
        || head.maximum_records == 0
        || head.maximum_records > EVIDENCE_VIEWER_MAX_COMPACTION_RECORDS_V1
        || record_count == 0
        || record_count > head.maximum_records
        || is_zero_digest(head.compacted_payload_digest)
        || !is_production_runtime_handle(&head.archive_handle)
        || head.archive_revision == 0
        || is_zero_digest(head.archive_policy_digest)
        || is_zero_digest(head.archive_id)
        || is_zero_digest(head.archive_public_key)
        || head.signer_handle != expected_signer_handle
        || !is_production_runtime_handle(&head.signer_handle)
        || head.signer_public_key != expected_signer_public_key
        || is_zero_digest(head.signer_public_key)
        || is_zero_digest(head.head_digest)
        || head.predecessor_head_digest == Some(head.head_digest)
        || head.predecessor_operation_id == Some(head.operation_id)
        || head.operation_id != compaction_archive_operation_id(head)?
        || head.head_digest != compaction_archive_head_digest(head)?
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    head.source_checkpoint_anchor
        .verify(expected_signer_handle, expected_signer_public_key)?;
    let key = PublicKey::from_bytes(Algorithm::Ed25519, &head.signer_public_key)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    PublicKey::from_bytes(Algorithm::Ed25519, &head.archive_public_key)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let signature = IrohaSignature::try_from_bytes(&head.signature)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    signature
        .verify(&key, &compaction_archive_signature_message(head)?)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)
}

fn verify_compaction_archive_lineage_link(
    successor: &EvidenceViewerSignedCompactionArchiveHeadV1,
    predecessor: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> Result<(), EvidenceViewerErrorV1> {
    if predecessor.generation.checked_add(1) != Some(successor.generation)
        || successor.predecessor_head_digest != Some(predecessor.head_digest)
        || successor.predecessor_operation_id != Some(predecessor.operation_id)
        || successor.source_checkpoint_generation <= predecessor.source_checkpoint_generation
        || successor.compacted_through_unix_ms < predecessor.compacted_through_unix_ms
        || successor.archive_handle != predecessor.archive_handle
        || successor.archive_revision != predecessor.archive_revision
        || successor.archive_policy_digest != predecessor.archive_policy_digest
        || successor.archive_id != predecessor.archive_id
        || successor.archive_public_key != predecessor.archive_public_key
        || successor.signer_handle != predecessor.signer_handle
        || successor.signer_public_key != predecessor.signer_public_key
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    Ok(())
}

fn verify_compaction_archive_artifact(
    config: &EvidenceViewerConfigV1,
    archive: &QualifiedEvidenceViewerCompactionArchiveV1,
    bytes: &[u8],
) -> Result<EvidenceViewerCompactionArchiveArtifactV1, EvidenceViewerErrorV1> {
    if bytes.is_empty() || len_u64(bytes.len()) > compaction_archive_max_bytes(config) {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    let artifact = decode_local_checkpoint_canonical::<EvidenceViewerCompactionArchiveArtifactV1>(
        bytes,
        compaction_archive_max_bytes(config),
        compaction_archive_sequence_limit(config),
    )
    .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    verify_compaction_archive_head_core(
        &artifact.head,
        &config.receipt_signer_handle,
        config.receipt_signer_public_key,
    )?;
    let record_count = u32::try_from(
        artifact
            .payload
            .challenges
            .len()
            .checked_add(artifact.payload.sessions.len())
            .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?,
    )
    .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    if artifact.version != EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1
        || artifact.payload.version != EVIDENCE_VIEWER_COMPACTION_ARCHIVE_VERSION_V1
        || artifact.head.archive_handle != archive.handle()
        || artifact.head.archive_revision != archive.qualification().revision()
        || artifact.head.archive_policy_digest != archive.qualification().policy_digest()
        || artifact.head.archive_id != archive.archive_id
        || artifact.head.archive_public_key != archive.public_key
        || artifact.head.archive_signature != [0; 64]
        || artifact.head.challenge_count
            != u32::try_from(artifact.payload.challenges.len())
                .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?
        || artifact.head.session_count
            != u32::try_from(artifact.payload.sessions.len())
                .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?
        || record_count == 0
        || record_count > artifact.head.maximum_records
        || artifact.head.compacted_payload_digest
            != compaction_archive_payload_digest(&artifact.payload)?
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    ensure_unique_sorted(
        artifact
            .payload
            .challenges
            .iter()
            .map(|record| record.challenge_id),
    )?;
    ensure_unique_sorted(
        artifact
            .payload
            .sessions
            .iter()
            .map(|record| record.local_session.session_id),
    )?;
    if artifact
        .payload
        .challenges
        .iter()
        .any(|record| record.expires_at_unix_ms > artifact.head.compacted_through_unix_ms)
        || artifact.payload.sessions.iter().any(|record| {
            record.local_session.expires_at_unix_ms > artifact.head.compacted_through_unix_ms
        })
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    Ok(artifact)
}

fn verify_checkpoint_store_record(
    config: &EvidenceViewerConfigV1,
    checkpoint_store: &QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerCheckpointStoreV1>,
    record: &EvidenceViewerCheckpointStoreRecordV1,
) -> Result<
    (
        EvidenceViewerCheckpointV1,
        EvidenceViewerSignedCheckpointAnchorV1,
    ),
    EvidenceViewerErrorV1,
> {
    let lineage_is_valid = match record.generation {
        1 => {
            record.predecessor_revision.is_none() && record.predecessor_checkpoint_digest.is_none()
        }
        2.. => {
            record
                .predecessor_revision
                .is_some_and(|digest| !is_zero_digest(digest))
                && record
                    .predecessor_checkpoint_digest
                    .is_some_and(|digest| !is_zero_digest(digest))
        }
        0 => false,
    };
    if record.version != EVIDENCE_VIEWER_CHECKPOINT_STORE_RECORD_VERSION_V1
        || !lineage_is_valid
        || is_zero_digest(record.checkpoint_digest)
        || record.checkpoint_bytes.is_empty()
        || len_u64(record.checkpoint_bytes.len()) > config.checkpoint_max_bytes
        || record.checkpoint_store_handle != checkpoint_store.handle
        || !is_production_runtime_handle(&record.checkpoint_store_handle)
        || record.checkpoint_store_revision != checkpoint_store.qualification.revision()
        || record.checkpoint_store_policy_digest != checkpoint_store.qualification.policy_digest()
        || record.signer_handle != config.receipt_signer_handle
        || !is_production_runtime_handle(&record.signer_handle)
        || record.signer_public_key != config.receipt_signer_public_key
        || is_zero_digest(record.revision)
        || record.revision != checkpoint_store_record_revision(record)
        || record.predecessor_revision == Some(record.revision)
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    let key = PublicKey::from_bytes(Algorithm::Ed25519, &record.signer_public_key)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let signature = IrohaSignature::try_from_bytes(&record.signature)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    signature
        .verify(&key, &checkpoint_store_record_signature_message(record))
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;

    let envelope = decode_local_checkpoint_canonical::<EvidenceViewerCheckpointEnvelopeV1>(
        &record.checkpoint_bytes,
        config.checkpoint_max_bytes,
        checkpoint_sequence_limit(config),
    )
    .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let (checkpoint, checkpoint_anchor) = verify_checkpoint_envelope(config, envelope)?;
    if checkpoint_anchor.checkpoint_generation != record.generation
        || checkpoint_anchor.predecessor_checkpoint_revision != record.predecessor_revision
        || checkpoint_anchor.predecessor_checkpoint_digest != record.predecessor_checkpoint_digest
        || checkpoint_anchor.checkpoint_digest != record.checkpoint_digest
        || checkpoint_anchor.checkpoint_store_handle != record.checkpoint_store_handle
        || checkpoint_anchor.checkpoint_store_revision != record.checkpoint_store_revision
        || checkpoint_anchor.checkpoint_store_policy_digest != record.checkpoint_store_policy_digest
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    if let Some(head) = checkpoint.compaction_archive_head.as_ref() {
        let source_is_direct_predecessor = head
            .source_checkpoint_generation
            .checked_add(1)
            .is_some_and(|generation| generation == record.generation);
        if head.source_checkpoint_generation >= record.generation
            || head.source_checkpoint_revision == record.revision
            || (source_is_direct_predecessor
                && (record.predecessor_revision != Some(head.source_checkpoint_revision)
                    || record.predecessor_checkpoint_digest
                        != Some(head.source_checkpoint_anchor.checkpoint_digest)))
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    Ok((checkpoint, checkpoint_anchor))
}

fn read_local_checkpoint_store_record(
    config: &EvidenceViewerConfigV1,
    checkpoint_store: &QualifiedEvidenceViewerProviderV1<dyn EvidenceViewerCheckpointStoreV1>,
) -> Result<Option<EvidenceViewerCheckpointStoreRecordV1>, EvidenceViewerErrorV1> {
    let Some(bytes) = read_local_checkpoint_bounded(
        &config.checkpoint_path,
        checkpoint_store_record_max_bytes(config),
    )
    .map_err(|_| EvidenceViewerErrorV1::CheckpointUnavailable)?
    else {
        return Ok(None);
    };
    let record = decode_local_checkpoint_canonical::<EvidenceViewerCheckpointStoreRecordV1>(
        &bytes,
        checkpoint_store_record_max_bytes(config),
        checkpoint_store_record_sequence_limit(config),
    )
    .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    verify_checkpoint_store_record(config, checkpoint_store, &record)?;
    Ok(Some(record))
}

fn write_local_checkpoint_store_record(
    config: &EvidenceViewerConfigV1,
    record: &EvidenceViewerCheckpointStoreRecordV1,
) -> Result<(), EvidenceViewerErrorV1> {
    let bytes = norito::to_bytes(record).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    if len_u64(bytes.len()) > checkpoint_store_record_max_bytes(config) {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    write_local_checkpoint_atomic_bounded(
        &config.checkpoint_path,
        &bytes,
        checkpoint_store_record_max_bytes(config),
    )
    .map_err(|_| EvidenceViewerErrorV1::CheckpointUnavailable)
}

fn validate_checkpoint_cache_lineage(
    local: Option<&EvidenceViewerCheckpointStoreRecordV1>,
    authoritative: &EvidenceViewerCheckpointStoreRecordV1,
) -> Result<(), EvidenceViewerErrorV1> {
    let Some(local) = local else {
        return Ok(());
    };
    if local == authoritative {
        return Ok(());
    }
    if local.generation.checked_add(1) == Some(authoritative.generation)
        && authoritative.predecessor_revision == Some(local.revision)
        && authoritative.predecessor_checkpoint_digest == Some(local.checkpoint_digest)
    {
        return Ok(());
    }
    Err(EvidenceViewerErrorV1::CheckpointChanged)
}

fn checkpoint_store_record_is_direct_successor(
    predecessor: Option<&EvidenceViewerCheckpointStoreRecordV1>,
    successor: &EvidenceViewerCheckpointStoreRecordV1,
) -> bool {
    match predecessor {
        Some(predecessor) => {
            predecessor.generation.checked_add(1) == Some(successor.generation)
                && successor.predecessor_revision == Some(predecessor.revision)
                && successor.predecessor_checkpoint_digest == Some(predecessor.checkpoint_digest)
        }
        None => {
            successor.generation == 1
                && successor.predecessor_revision.is_none()
                && successor.predecessor_checkpoint_digest.is_none()
        }
    }
}

fn validated_checkpoint_anchor(
    config: &EvidenceViewerConfigV1,
    state: &EvidenceViewerStateV1,
) -> Result<EvidenceViewerSignedCheckpointAnchorV1, EvidenceViewerErrorV1> {
    let anchor = state
        .checkpoint_anchor
        .clone()
        .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
    anchor.verify(
        &config.receipt_signer_handle,
        config.receipt_signer_public_key,
    )?;
    let checkpoint = checkpoint_from_state(state);
    if anchor.checkpoint_digest != checkpoint_payload_digest(&checkpoint)?
        || anchor.receipt_count
            != u64::try_from(state.receipts.len())
                .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?
        || anchor.chain_head != state.receipts.last().map(receipt_cursor)
        || anchor.compaction_archive_head_digest
            != state
                .compaction_archive_head
                .as_ref()
                .map(|head| head.head_digest)
        || state.checkpoint_record.as_ref().is_none_or(|record| {
            anchor.checkpoint_generation != record.generation
                || anchor.predecessor_checkpoint_revision != record.predecessor_revision
                || anchor.predecessor_checkpoint_digest != record.predecessor_checkpoint_digest
                || anchor.checkpoint_store_handle != record.checkpoint_store_handle
                || anchor.checkpoint_store_revision != record.checkpoint_store_revision
                || anchor.checkpoint_store_policy_digest != record.checkpoint_store_policy_digest
        })
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    Ok(anchor)
}

fn checkpoint_anchor_head_is_valid(
    receipt_count: u64,
    chain_head: Option<EvidenceViewerReceiptCursorV1>,
) -> bool {
    match (receipt_count, chain_head) {
        (0, None) => true,
        (0, Some(_)) | (_, None) => false,
        (count, Some(head)) => {
            head.sequence == count && head.sequence != 0 && !is_zero_digest(head.receipt_digest)
        }
    }
}

fn legal_hold_digest(
    quarantine_id: [u8; 16],
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
    authority_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.evidence-viewer.legal-hold.v1");
    hasher.update(&quarantine_id);
    hasher.update(&object_id);
    hasher.update(&evidence_digest);
    hasher.update(&authority_digest);
    *hasher.finalize().as_bytes()
}

fn checkpoint_anchor_signature_message(anchor: &EvidenceViewerSignedCheckpointAnchorV1) -> Vec<u8> {
    let mut message = Vec::with_capacity(
        CHECKPOINT_SIGNATURE_DOMAIN_V1.len()
            + std::mem::size_of::<u16>()
            + std::mem::size_of::<u64>()
            + 1
            + 32
            + 1
            + 32
            + anchor.checkpoint_digest.len()
            + std::mem::size_of::<u64>()
            + 1
            + std::mem::size_of::<u64>()
            + 32
            + 1
            + 32
            + std::mem::size_of::<u64>()
            + anchor.checkpoint_store_handle.len()
            + std::mem::size_of::<u64>()
            + 32
            + std::mem::size_of::<u64>()
            + anchor.signer_handle.len()
            + anchor.signer_public_key.len(),
    );
    message.extend_from_slice(CHECKPOINT_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&anchor.version.to_le_bytes());
    message.extend_from_slice(&anchor.checkpoint_generation.to_le_bytes());
    hash_optional_checkpoint_digest_bytes(&mut message, anchor.predecessor_checkpoint_revision);
    hash_optional_checkpoint_digest_bytes(&mut message, anchor.predecessor_checkpoint_digest);
    message.extend_from_slice(&anchor.checkpoint_digest);
    message.extend_from_slice(&anchor.receipt_count.to_le_bytes());
    hash_optional_receipt_cursor_bytes(&mut message, anchor.chain_head);
    match anchor.compaction_archive_head_digest {
        Some(digest) => {
            message.push(1);
            message.extend_from_slice(&digest);
        }
        None => message.push(0),
    }
    message.extend_from_slice(
        &u64::try_from(anchor.checkpoint_store_handle.len())
            .expect("evidence-viewer checkpoint-store handle length is bounded to 256 bytes")
            .to_le_bytes(),
    );
    message.extend_from_slice(anchor.checkpoint_store_handle.as_bytes());
    message.extend_from_slice(&anchor.checkpoint_store_revision.to_le_bytes());
    message.extend_from_slice(&anchor.checkpoint_store_policy_digest);
    message.extend_from_slice(
        &u64::try_from(anchor.signer_handle.len())
            .expect("evidence-viewer signer handle length is bounded to 256 bytes")
            .to_le_bytes(),
    );
    message.extend_from_slice(anchor.signer_handle.as_bytes());
    message.extend_from_slice(&anchor.signer_public_key);
    message
}

fn hash_optional_checkpoint_digest_bytes(output: &mut Vec<u8>, digest: Option<[u8; 32]>) {
    match digest {
        Some(digest) => {
            output.push(1);
            output.extend_from_slice(&digest);
        }
        None => output.push(0),
    }
}

fn hash_optional_receipt_cursor_bytes(
    output: &mut Vec<u8>,
    cursor: Option<EvidenceViewerReceiptCursorV1>,
) {
    match cursor {
        Some(cursor) => {
            output.push(1);
            output.extend_from_slice(&cursor.sequence.to_le_bytes());
            output.extend_from_slice(&cursor.receipt_digest);
        }
        None => output.push(0),
    }
}

fn verify_checkpoint_envelope(
    config: &EvidenceViewerConfigV1,
    envelope: EvidenceViewerCheckpointEnvelopeV1,
) -> Result<
    (
        EvidenceViewerCheckpointV1,
        EvidenceViewerSignedCheckpointAnchorV1,
    ),
    EvidenceViewerErrorV1,
> {
    if envelope.version != EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1
        || envelope.checkpoint_anchor.checkpoint_digest
            != checkpoint_payload_digest(&envelope.checkpoint)?
        || envelope.checkpoint_anchor.receipt_count
            != u64::try_from(envelope.checkpoint.receipts.len())
                .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?
        || envelope.checkpoint_anchor.chain_head
            != envelope.checkpoint.receipts.last().map(receipt_cursor)
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    envelope.checkpoint_anchor.verify(
        &config.receipt_signer_handle,
        config.receipt_signer_public_key,
    )?;
    validate_checkpoint(config, &envelope.checkpoint)?;
    Ok((envelope.checkpoint, envelope.checkpoint_anchor))
}

fn state_from_checkpoint(
    config: &EvidenceViewerConfigV1,
    checkpoint: EvidenceViewerCheckpointV1,
    checkpoint_anchor: EvidenceViewerSignedCheckpointAnchorV1,
    checkpoint_record: EvidenceViewerCheckpointStoreRecordV1,
) -> Result<EvidenceViewerStateV1, EvidenceViewerErrorV1> {
    validate_checkpoint(config, &checkpoint)?;
    Ok(EvidenceViewerStateV1 {
        challenges: checkpoint
            .challenges
            .into_iter()
            .map(|record| (record.challenge_id, record))
            .collect(),
        sessions: checkpoint
            .sessions
            .into_iter()
            .map(|record| (record.local_session.session_id, record))
            .collect(),
        receipts: checkpoint.receipts,
        legal_holds: checkpoint
            .legal_holds
            .into_iter()
            .map(|record| (record.hold_id, record))
            .collect(),
        retentions: checkpoint
            .retentions
            .into_iter()
            .map(|record| (record.quarantine_id, record))
            .collect(),
        default_retention_floors: checkpoint
            .default_retention_floors
            .into_iter()
            .map(|record| (record.quarantine_id, record))
            .collect(),
        erasure_intents: checkpoint
            .erasure_intents
            .into_iter()
            .map(|record| (record.quarantine_id, record))
            .collect(),
        erasures: checkpoint
            .erasures
            .into_iter()
            .map(|record| (record.quarantine_id, record))
            .collect(),
        idempotency: checkpoint
            .idempotency
            .into_iter()
            .map(|record| (record.idempotency_key, record))
            .collect(),
        compaction_archive_head: checkpoint.compaction_archive_head,
        checkpoint_anchor: Some(checkpoint_anchor),
        checkpoint_record: Some(checkpoint_record),
        durability_uncertain: false,
        authoritative_race_adopted: false,
    })
}

fn validate_checkpoint(
    config: &EvidenceViewerConfigV1,
    checkpoint: &EvidenceViewerCheckpointV1,
) -> Result<(), EvidenceViewerErrorV1> {
    if checkpoint.version != EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1
        || checkpoint.challenges.len() > config.max_challenges
        || checkpoint.sessions.len() > config.max_sessions
        || checkpoint.receipts.len() > config.max_receipts
        || checkpoint.idempotency.len() > config.max_idempotency_records
        || checkpoint.legal_holds.len() > config.max_sessions
        || checkpoint.retentions.len() > config.max_sessions
        || checkpoint.default_retention_floors.len() > config.max_sessions
        || checkpoint.erasure_intents.len() > config.max_sessions
        || checkpoint.erasures.len() > config.max_sessions
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    if let Some(head) = checkpoint.compaction_archive_head.as_ref() {
        if head.archive_handle != config.compaction_archive_handle
            || head.archive_revision != config.expected_compaction_archive_qualification.revision()
            || head.archive_policy_digest
                != config
                    .expected_compaction_archive_qualification
                    .policy_digest()
            || head.archive_id != config.compaction_archive_id
            || head.archive_public_key != config.compaction_archive_public_key
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        head.verify(
            &config.receipt_signer_handle,
            config.receipt_signer_public_key,
        )?;
    }
    ensure_unique_sorted(
        checkpoint
            .challenges
            .iter()
            .map(|record| record.challenge_id),
    )?;
    ensure_unique_sorted(
        checkpoint
            .sessions
            .iter()
            .map(|record| record.local_session.session_id),
    )?;
    ensure_unique_sorted(checkpoint.legal_holds.iter().map(|record| record.hold_id))?;
    ensure_unique_sorted(
        checkpoint
            .retentions
            .iter()
            .map(|record| record.quarantine_id),
    )?;
    ensure_unique_sorted(
        checkpoint
            .default_retention_floors
            .iter()
            .map(|record| record.quarantine_id),
    )?;
    ensure_unique_sorted(
        checkpoint
            .erasure_intents
            .iter()
            .map(|record| record.quarantine_id),
    )?;
    ensure_unique_sorted(
        checkpoint
            .erasures
            .iter()
            .map(|record| record.quarantine_id),
    )?;
    ensure_unique_sorted(
        checkpoint
            .idempotency
            .iter()
            .map(|record| record.idempotency_key),
    )?;
    let mut assertion_digests = BTreeSet::new();
    for challenge in &checkpoint.challenges {
        if challenge.challenge_id != digest_id16(challenge.challenge_digest)
            || is_zero_digest(challenge.challenge_digest)
            || is_zero_digest(challenge.binding_digest)
            || is_zero_digest(challenge.viewer_account_digest)
            || is_zero_digest(challenge.purpose_digest)
            || is_zero_digest(challenge.policy_digest)
            || challenge.finalized_height == 0
            || is_zero_digest(challenge.finalized_block_hash)
            || challenge.finalized_at_unix_ms == 0
            || challenge.finalized_at_unix_ms > challenge.issued_at_unix_ms
            || challenge.issued_at_unix_ms == 0
            || challenge.expires_at_unix_ms <= challenge.issued_at_unix_ms
            || challenge.consumed_at_unix_ms.is_some_and(|consumed| {
                consumed < challenge.issued_at_unix_ms || consumed >= challenge.expires_at_unix_ms
            })
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    for session in &checkpoint.sessions {
        if validate_evidence_viewer_session_record(&session.local_session).is_err()
            || session.local_session.session_id == [0; 16]
            || session.case_id.is_empty()
            || session.round_id.is_empty()
            || session.local_session.viewer_role != session.role.as_str()
            || session.local_session.purpose != PAYLOAD_FREE_PURPOSE_LABEL_V1
            || is_zero_digest(session.purpose_digest)
            || is_zero_digest(session.credential_id_digest)
            || is_zero_digest(session.webauthn_assertion_digest)
            || session.authenticator_counter == 0
            || is_zero_digest(session.policy_digest)
            || session.finalized_height == 0
            || is_zero_digest(session.finalized_block_hash)
            || session.finalized_at_unix_ms == 0
            || session.grant_generation == 0
            || session.active_grant_issued_at_unix_ms == 0
            || is_zero_digest(session.active_grant_digest)
            || session.active_grant_expires_at_unix_ms <= session.active_grant_issued_at_unix_ms
            || session.active_grant_expires_at_unix_ms > session.local_session.expires_at_unix_ms
            || session.active_grant_issued_at_unix_ms < session.local_session.issued_at_unix_ms
            || session.finalized_at_unix_ms > session.active_grant_issued_at_unix_ms
            || session
                .local_session
                .expires_at_unix_ms
                .saturating_sub(session.local_session.issued_at_unix_ms)
                > config.session_ttl_ms
            || !assertion_digests.insert(session.webauthn_assertion_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    let mut previous_digest = [0; 32];
    let mut expected_sequence = 1_u64;
    let mut receipts_by_digest = BTreeMap::new();
    for receipt in &checkpoint.receipts {
        receipt.verify(
            &config.receipt_signer_handle,
            config.receipt_signer_public_key,
        )?;
        if receipt.body.sequence != expected_sequence
            || receipt.body.previous_receipt_digest != previous_digest
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        if receipts_by_digest
            .insert(receipt.receipt_digest, receipt)
            .is_some()
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?;
        previous_digest = receipt.receipt_digest;
    }
    for session in &checkpoint.sessions {
        let mut issuance = checkpoint.receipts.iter().filter(|receipt| {
            receipt.body.kind == EvidenceViewerReceiptKindV1::SessionIssued
                && receipt.body.session_id == Some(session.local_session.session_id)
        });
        let Some(receipt) = issuance.next() else {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        };
        if issuance.next().is_some()
            || receipt.body.case_id.as_deref() != Some(session.case_id.as_str())
            || receipt.body.round_id.as_deref() != Some(session.round_id.as_str())
            || receipt.body.quarantine_id != session.local_session.quarantine_id
            || receipt.body.object_id != session.local_session.object_id
            || receipt.body.evidence_digest != session.local_session.evidence_digest
            || receipt.body.actor_account_digest
                != text_digest(&session.local_session.viewer_account)
            || receipt.body.issued_at_unix_ms != session.local_session.issued_at_unix_ms
            || (session.revoked
                && !checkpoint
                    .erasures
                    .iter()
                    .any(|record| record.quarantine_id == session.local_session.quarantine_id))
            || (!session.revoked
                && checkpoint
                    .erasures
                    .iter()
                    .any(|record| record.quarantine_id == session.local_session.quarantine_id))
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    for hold in &checkpoint.legal_holds {
        let placement_digest = legal_hold_digest(
            hold.quarantine_id,
            hold.object_id,
            hold.evidence_digest,
            hold.authority_digest,
        );
        let placement_count = checkpoint
            .receipts
            .iter()
            .filter(|receipt| {
                receipt.body.kind == EvidenceViewerReceiptKindV1::LegalHoldPlaced
                    && receipt.body.quarantine_id == hold.quarantine_id
                    && receipt.body.object_id == hold.object_id
                    && receipt.body.evidence_digest == hold.evidence_digest
                    && receipt.body.request_digest == placement_digest
                    && receipt.body.issued_at_unix_ms == hold.placed_at_unix_ms
            })
            .count();
        let release_count = hold.released_at_unix_ms.map_or(0, |released_at| {
            checkpoint
                .receipts
                .iter()
                .filter(|receipt| {
                    receipt.body.kind == EvidenceViewerReceiptKindV1::LegalHoldReleased
                        && receipt.body.quarantine_id == hold.quarantine_id
                        && receipt.body.object_id == hold.object_id
                        && receipt.body.evidence_digest == hold.evidence_digest
                        && receipt.body.issued_at_unix_ms == released_at
                })
                .count()
        });
        let unsigned_release = hold.released_at_unix_ms.is_none()
            && checkpoint.receipts.iter().any(|receipt| {
                receipt.body.kind == EvidenceViewerReceiptKindV1::LegalHoldReleased
                    && receipt.body.quarantine_id == hold.quarantine_id
                    && receipt.body.object_id == hold.object_id
                    && receipt.body.evidence_digest == hold.evidence_digest
                    && receipt.body.issued_at_unix_ms >= hold.placed_at_unix_ms
            });
        if hold.hold_id != digest_id16(placement_digest)
            || is_zero_digest(hold.evidence_digest)
            || is_zero_digest(hold.authority_digest)
            || hold.placed_at_unix_ms == 0
            || placement_count != 1
            || hold.released_at_unix_ms.is_some() && release_count != 1
            || unsigned_release
            || hold
                .released_at_unix_ms
                .is_some_and(|released| released < hold.placed_at_unix_ms)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    let mut erasure_operation_ids = BTreeSet::new();
    let mut erasure_idempotency_keys = BTreeSet::new();
    for intent in &checkpoint.erasure_intents {
        if intent.quarantine_id == [0; 16]
            || intent.object_id == [0; 16]
            || is_zero_digest(intent.evidence_digest)
            || is_zero_digest(intent.operation_id)
            || is_zero_digest(intent.idempotency_key)
            || is_zero_digest(intent.request_digest)
            || intent.requested_at_unix_ms == 0
            || validate_label(&intent.case_id).is_err()
            || validate_label(&intent.round_id).is_err()
            || validate_label(&intent.actor_account).is_err()
            || intent.operation_id
                != erasure_operation_id(
                    intent.idempotency_key,
                    intent.request_digest,
                    intent.quarantine_id,
                    intent.object_id,
                    intent.evidence_digest,
                )
            || !erasure_operation_ids.insert(intent.operation_id)
            || !erasure_idempotency_keys.insert(intent.idempotency_key)
            || checkpoint
                .erasures
                .iter()
                .any(|record| record.quarantine_id == intent.quarantine_id)
            || checkpoint
                .idempotency
                .iter()
                .any(|record| record.idempotency_key == intent.idempotency_key)
            || checkpoint.legal_holds.iter().any(|hold| {
                hold.quarantine_id == intent.quarantine_id && hold.released_at_unix_ms.is_none()
            })
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    for erasure in &checkpoint.erasures {
        let receipt = receipts_by_digest.get(&erasure.receipt_digest);
        if is_zero_digest(erasure.evidence_digest)
            || is_zero_digest(erasure.erasure_commit_digest)
            || is_zero_digest(erasure.receipt_digest)
            || erasure.erased_at_unix_ms == 0
            || receipt.is_none_or(|receipt| {
                receipt.body.kind != EvidenceViewerReceiptKindV1::ErasureCompleted
                    || receipt.body.quarantine_id != erasure.quarantine_id
                    || receipt.body.object_id != erasure.object_id
                    || receipt.body.evidence_digest != erasure.evidence_digest
                    || receipt.body.request_digest != erasure.erasure_commit_digest
                    || receipt.body.issued_at_unix_ms != erasure.erased_at_unix_ms
            })
            || checkpoint.legal_holds.iter().any(|hold| {
                hold.quarantine_id == erasure.quarantine_id && hold.released_at_unix_ms.is_none()
            })
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    for retention in &checkpoint.retentions {
        let receipt = receipts_by_digest.get(&retention.receipt_digest);
        if is_zero_digest(retention.evidence_digest)
            || is_zero_digest(retention.receipt_digest)
            || retention.evaluated_at_unix_ms == 0
            || retention.retain_until_unix_ms < retention.evaluated_at_unix_ms
            || receipt.is_none_or(|receipt| {
                receipt.body.kind != EvidenceViewerReceiptKindV1::RetentionEvaluated
                    || receipt.body.quarantine_id != retention.quarantine_id
                    || receipt.body.object_id != retention.object_id
                    || receipt.body.evidence_digest != retention.evidence_digest
                    || receipt.body.issued_at_unix_ms != retention.evaluated_at_unix_ms
            })
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    for floor in &checkpoint.default_retention_floors {
        if floor.quarantine_id == [0; 16]
            || floor.object_id == [0; 16]
            || is_zero_digest(floor.evidence_digest)
            || floor.basis_session_expires_at_unix_ms == 0
            || floor.retain_until_unix_ms
                != floor
                    .basis_session_expires_at_unix_ms
                    .checked_add(config.retention_after_expiry_ms)
                    .ok_or(EvidenceViewerErrorV1::InvalidCheckpoint)?
            || checkpoint
                .erasures
                .iter()
                .any(|record| record.quarantine_id == floor.quarantine_id)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    let challenge_by_digest = checkpoint
        .challenges
        .iter()
        .map(|challenge| (challenge.challenge_digest, challenge))
        .collect::<BTreeMap<_, _>>();
    let outcome_digests = checkpoint
        .idempotency
        .iter()
        .map(|record| record.outcome_digest)
        .collect::<BTreeSet<_>>();
    for record in &checkpoint.idempotency {
        if is_zero_digest(record.idempotency_key)
            || is_zero_digest(record.request_digest)
            || is_zero_digest(record.outcome_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        if let Some(receipt) = receipts_by_digest.get(&record.outcome_digest) {
            if receipt.body.idempotency_key_digest
                != *blake3::hash(&record.idempotency_key).as_bytes()
            {
                return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
            }
        } else if let Some(challenge) = challenge_by_digest.get(&record.outcome_digest) {
            if record.request_digest
                != request_binding_digest(
                    b"challenge",
                    &record.idempotency_key,
                    &challenge.binding_digest,
                )
            {
                return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
            }
        } else if checkpoint
            .compaction_archive_head
            .as_ref()
            .is_none_or(|head| record.outcome_digest != head.head_digest)
        {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
    }
    if checkpoint
        .receipts
        .iter()
        .any(|receipt| !outcome_digests.contains(&receipt.receipt_digest))
        || checkpoint
            .challenges
            .iter()
            .any(|challenge| !outcome_digests.contains(&challenge.challenge_digest))
    {
        return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
    }
    Ok(())
}

fn validate_compaction_archive_request(
    config: &EvidenceViewerConfigV1,
    request: &EvidenceViewerCompactionArchiveRequestV1,
) -> Result<(), EvidenceViewerErrorV1> {
    request.expected_checkpoint_anchor.verify(
        &config.receipt_signer_handle,
        config.receipt_signer_public_key,
    )?;
    if request
        .expected_archive_head_digest
        .is_some_and(is_zero_digest)
        || request.compacted_through_unix_ms == 0
        || request.maximum_records == 0
        || request.maximum_records > EVIDENCE_VIEWER_MAX_COMPACTION_RECORDS_V1
    {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    Ok(())
}

fn compaction_archive_head_matches_request(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
    request: &EvidenceViewerCompactionArchiveRequestV1,
    archive: &QualifiedEvidenceViewerCompactionArchiveV1,
) -> bool {
    head.source_checkpoint_anchor == request.expected_checkpoint_anchor
        && head.predecessor_head_digest == request.expected_archive_head_digest
        && head.compacted_through_unix_ms == request.compacted_through_unix_ms
        && head.maximum_records == request.maximum_records
        && head.archive_handle == archive.handle()
        && head.archive_revision == archive.qualification().revision()
        && head.archive_policy_digest == archive.qualification().policy_digest()
        && head.archive_id == archive.archive_id
        && head.archive_public_key == archive.public_key
}

fn validate_challenge_request(
    request: &EvidenceViewerChallengeRequestV1,
) -> Result<(), EvidenceViewerErrorV1> {
    validate_label(&request.case_id)?;
    validate_label(&request.round_id)?;
    validate_label(&request.viewer_account)?;
    validate_purpose(&request.purpose)?;
    if is_zero_digest(request.idempotency_key) || request.now_unix_ms == 0 {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    Ok(())
}

fn validate_session_request(
    request: &EvidenceViewerSessionRequestV1,
) -> Result<(), EvidenceViewerErrorV1> {
    validate_label(&request.case_id)?;
    validate_label(&request.round_id)?;
    validate_label(&request.viewer_account)?;
    validate_purpose(&request.purpose)?;
    if request.webauthn_assertion.is_empty()
        || request.webauthn_assertion.len() > EVIDENCE_VIEWER_MAX_WEBAUTHN_ASSERTION_BYTES_V1
        || is_zero_digest(request.idempotency_key)
        || request.now_unix_ms == 0
    {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    Ok(())
}

fn validate_label(value: &str) -> Result<(), EvidenceViewerErrorV1> {
    if value.is_empty()
        || value.len() > 256
        || value != value.trim()
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    Ok(())
}

fn validate_purpose(value: &str) -> Result<(), EvidenceViewerErrorV1> {
    if value.is_empty()
        || value.len() > 256
        || value != value.trim()
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    Ok(())
}

fn is_canonical_rp_id(rp_id: &str) -> bool {
    rp_id.len() <= 253
        && rp_id.contains('.')
        && rp_id == rp_id.to_ascii_lowercase()
        && rp_id.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && label
                    .bytes()
                    .last()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn validate_evidence_viewer_runtime_provider_handle(
    handle: &str,
    configured: bool,
) -> Result<(), EvidenceViewerRuntimeProviderQualificationErrorV1> {
    match validate_production_runtime_handle(handle) {
        Ok(()) => Ok(()),
        Err(ProductionRuntimeHandleError::InvalidSyntax) => Err(if configured {
            EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidConfiguredHandle
        } else {
            EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidProviderHandle
        }),
        Err(ProductionRuntimeHandleError::TestMarked) => Err(if configured {
            EvidenceViewerRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle
        } else {
            EvidenceViewerRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle
        }),
    }
}

fn qualify_evidence_viewer_runtime_provider<P: EvidenceViewerRuntimeProviderV1 + ?Sized>(
    expected_handle: &str,
    expected_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), EvidenceViewerRuntimeProviderQualificationErrorV1> {
    validate_evidence_viewer_runtime_provider_handle(provider.handle(), false)?;
    if provider.handle() != expected_handle {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::SubstitutedProvider);
    }
    let qualification = provider
        .qualification()
        .map_err(|_| EvidenceViewerRuntimeProviderQualificationErrorV1::UnavailableOrStale)?;
    if !qualification.is_valid() {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidQualification);
    }
    if qualification != expected_qualification {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::QualificationMismatch);
    }
    if provider.handle() != expected_handle {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    Ok(())
}

fn assert_evidence_viewer_runtime_provider_qualification<
    P: EvidenceViewerRuntimeProviderV1 + ?Sized,
>(
    expected_handle: &str,
    expected_qualification: EvidenceViewerRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), EvidenceViewerRuntimeProviderQualificationErrorV1> {
    if provider.handle() != expected_handle {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    let qualification = provider
        .qualification()
        .map_err(|_| EvidenceViewerRuntimeProviderQualificationErrorV1::UnavailableOrStale)?;
    if !qualification.is_valid() {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidQualification);
    }
    if provider.handle() != expected_handle || qualification != expected_qualification {
        return Err(EvidenceViewerRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    Ok(())
}

fn is_canonical_https_origin(origin: &str, rp_id: &str) -> bool {
    if origin.len() > 512
        || origin != origin.trim()
        || !origin.is_ascii()
        || origin
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let Ok(parsed) = Url::parse(origin) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.path() == "/"
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && parsed.origin().ascii_serialization() == origin
        && (host == rp_id || host.ends_with(&format!(".{rp_id}")))
}

fn ensure_unique_sorted<const N: usize>(
    values: impl Iterator<Item = [u8; N]>,
) -> Result<(), EvidenceViewerErrorV1> {
    let mut previous = None;
    for value in values {
        if previous.is_some_and(|prior| prior >= value) {
            return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
        }
        previous = Some(value);
    }
    Ok(())
}

fn ensure_new_idempotency(
    state: &EvidenceViewerStateV1,
    maximum_records: usize,
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
) -> Result<(), EvidenceViewerErrorV1> {
    if is_zero_digest(idempotency_key) || is_zero_digest(request_digest) {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    if let Some(existing) = state.idempotency.get(&idempotency_key) {
        if existing.request_digest == request_digest {
            return Err(EvidenceViewerErrorV1::AuthenticationRejected);
        }
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    if state.idempotency.len() >= maximum_records {
        return Err(EvidenceViewerErrorV1::ResourceExhausted);
    }
    Ok(())
}

fn ensure_session_commit_slot(
    state: &EvidenceViewerStateV1,
    config: &EvidenceViewerConfigV1,
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
    assertion_digest: [u8; 32],
    session_id: [u8; 16],
    quarantine_id: [u8; 16],
) -> Result<(), EvidenceViewerErrorV1> {
    ensure_durability(state)?;
    ensure_new_idempotency(
        state,
        config.max_idempotency_records,
        idempotency_key,
        request_digest,
    )?;
    if state.sessions.len() >= config.max_sessions
        || state.receipts.len() >= config.max_receipts
        || state.sessions.contains_key(&session_id)
    {
        return Err(EvidenceViewerErrorV1::ResourceExhausted);
    }
    if state
        .sessions
        .values()
        .any(|session| session.webauthn_assertion_digest == assertion_digest)
    {
        return Err(EvidenceViewerErrorV1::AuthenticationRejected);
    }
    if state.erasures.contains_key(&quarantine_id) {
        return Err(EvidenceViewerErrorV1::InvalidRequest);
    }
    Ok(())
}

fn ensure_durability(state: &EvidenceViewerStateV1) -> Result<(), EvidenceViewerErrorV1> {
    if state.durability_uncertain {
        Err(EvidenceViewerErrorV1::CheckpointUnavailable)
    } else {
        Ok(())
    }
}

fn can_restore_process_local_snapshot(state: &EvidenceViewerStateV1) -> bool {
    !state.durability_uncertain && !state.authoritative_race_adopted
}

fn retention_deadline_for(
    state: &EvidenceViewerStateV1,
    config: &EvidenceViewerConfigV1,
    quarantine_id: [u8; 16],
) -> Option<u64> {
    let explicit = state
        .retentions
        .get(&quarantine_id)
        .map(|record| record.retain_until_unix_ms);
    let compacted_floor = state
        .default_retention_floors
        .get(&quarantine_id)
        .map(|record| record.retain_until_unix_ms);
    let live_session_floor = state
        .sessions
        .values()
        .filter(|session| session.local_session.quarantine_id == quarantine_id)
        .map(|session| session.local_session.expires_at_unix_ms)
        .max()
        .and_then(|expiry| expiry.checked_add(config.retention_after_expiry_ms));
    [explicit, compacted_floor, live_session_floor]
        .into_iter()
        .flatten()
        .max()
}

fn projected_default_retention_floors(
    state: &EvidenceViewerStateV1,
    config: &EvidenceViewerConfigV1,
    compacted_sessions: &[EvidenceViewerSessionSecurityRecordV1],
) -> Result<BTreeMap<[u8; 16], EvidenceViewerDefaultRetentionFloorV1>, EvidenceViewerErrorV1> {
    let mut projected = state.default_retention_floors.clone();
    for session in compacted_sessions {
        let local = &session.local_session;
        if state.erasures.contains_key(&local.quarantine_id) {
            projected.remove(&local.quarantine_id);
            continue;
        }
        let retain_until_unix_ms = local
            .expires_at_unix_ms
            .checked_add(config.retention_after_expiry_ms)
            .ok_or(EvidenceViewerErrorV1::ResourceExhausted)?;
        if let Some(existing) = projected.get_mut(&local.quarantine_id) {
            if existing.object_id != local.object_id
                || existing.evidence_digest != local.evidence_digest
            {
                return Err(EvidenceViewerErrorV1::InvalidCheckpoint);
            }
            if local.expires_at_unix_ms > existing.basis_session_expires_at_unix_ms {
                existing.basis_session_expires_at_unix_ms = local.expires_at_unix_ms;
                existing.retain_until_unix_ms = retain_until_unix_ms;
            }
            continue;
        }
        if projected.len() >= config.max_sessions {
            return Err(EvidenceViewerErrorV1::ResourceExhausted);
        }
        projected.insert(
            local.quarantine_id,
            EvidenceViewerDefaultRetentionFloorV1 {
                quarantine_id: local.quarantine_id,
                object_id: local.object_id,
                evidence_digest: local.evidence_digest,
                basis_session_expires_at_unix_ms: local.expires_at_unix_ms,
                retain_until_unix_ms,
            },
        );
    }
    Ok(projected)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceViewerChallengeBindingContextV1 {
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
    purpose_digest: [u8; 32],
    policy_digest: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
    finalized_at_unix_ms: u64,
}

fn challenge_binding_digest(
    request: &EvidenceViewerChallengeRequestV1,
    context: EvidenceViewerChallengeBindingContextV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHALLENGE_BINDING_DOMAIN_V1);
    hash_text(&mut hasher, &request.case_id);
    hash_text(&mut hasher, &request.round_id);
    hasher.update(&request.quarantine_id);
    hasher.update(&context.object_id);
    hasher.update(&context.evidence_digest);
    hash_text(&mut hasher, &request.viewer_account);
    hash_text(&mut hasher, request.role.as_str());
    hasher.update(&context.purpose_digest);
    hasher.update(&context.policy_digest);
    hasher.update(&context.finalized_height.to_le_bytes());
    hasher.update(&context.finalized_block_hash);
    hasher.update(&context.finalized_at_unix_ms.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn session_request_digest(
    request: &EvidenceViewerSessionRequestV1,
    challenge_digest: [u8; 32],
    assertion_digest: [u8; 32],
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
    challenge_binding_digest: [u8; 32],
    policy_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SESSION_REQUEST_DOMAIN_V1);
    hash_text(&mut hasher, &request.case_id);
    hash_text(&mut hasher, &request.round_id);
    hasher.update(&request.quarantine_id);
    hasher.update(&object_id);
    hasher.update(&evidence_digest);
    hash_text(&mut hasher, &request.viewer_account);
    hash_text(&mut hasher, request.role.as_str());
    hash_text(&mut hasher, &request.purpose);
    hasher.update(&challenge_digest);
    hasher.update(&challenge_binding_digest);
    hasher.update(&assertion_digest);
    hasher.update(&policy_digest);
    hasher.update(&request.idempotency_key);
    *hasher.finalize().as_bytes()
}

fn request_binding_digest(label: &[u8], idempotency_key: &[u8; 32], body: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(REQUEST_BINDING_DOMAIN_V1);
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(idempotency_key);
    hasher.update(&(body.len() as u64).to_le_bytes());
    hasher.update(body);
    *hasher.finalize().as_bytes()
}

fn erasure_operation_id(
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
    quarantine_id: [u8; 16],
    object_id: [u8; 16],
    evidence_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ERASURE_OPERATION_DOMAIN_V1);
    hasher.update(&idempotency_key);
    hasher.update(&request_digest);
    hasher.update(&quarantine_id);
    hasher.update(&object_id);
    hasher.update(&evidence_digest);
    *hasher.finalize().as_bytes()
}

fn watermark_digest(
    case_id: &str,
    round_id: &str,
    viewer_account: &str,
    role: EvidenceViewerRoleV1,
    object_id: [u8; 16],
    issued_at_unix_ms: u64,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(WATERMARK_DOMAIN_V1);
    hash_text(&mut hasher, case_id);
    hash_text(&mut hasher, round_id);
    hash_text(&mut hasher, viewer_account);
    hash_text(&mut hasher, role.as_str());
    hasher.update(&object_id);
    hasher.update(&issued_at_unix_ms.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn receipt_body_digest(
    body: &EvidenceViewerReceiptBodyV1,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let bytes = norito::to_bytes(body).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(RECEIPT_BODY_DOMAIN_V1);
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn receipt_signature_message(receipt_digest: [u8; 32]) -> Vec<u8> {
    let mut message = Vec::with_capacity(RECEIPT_SIGNATURE_DOMAIN_V1.len() + receipt_digest.len());
    message.extend_from_slice(RECEIPT_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&receipt_digest);
    message
}

fn receipt_cursor(receipt: &EvidenceViewerSignedReceiptV1) -> EvidenceViewerReceiptCursorV1 {
    EvidenceViewerReceiptCursorV1 {
        sequence: receipt.body.sequence,
        receipt_digest: receipt.receipt_digest,
    }
}

fn transparency_projection_digest(
    checkpoint_anchor: &EvidenceViewerSignedCheckpointAnchorV1,
    compaction_archive_head: Option<&EvidenceViewerSignedCompactionArchiveHeadV1>,
    predecessor: Option<EvidenceViewerReceiptCursorV1>,
    page_limit: u16,
    receipts: &[EvidenceViewerSignedReceiptV1],
    next_cursor: Option<EvidenceViewerReceiptCursorV1>,
    has_more: bool,
) -> Result<[u8; 32], EvidenceViewerErrorV1> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TRANSPARENCY_PROJECTION_DOMAIN_V1);
    hasher.update(&EVIDENCE_VIEWER_TRANSPARENCY_PROJECTION_VERSION_V1.to_le_bytes());
    let checkpoint_anchor_bytes = norito::to_bytes(checkpoint_anchor)
        .map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
    hasher.update(
        &u64::try_from(checkpoint_anchor_bytes.len())
            .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?
            .to_le_bytes(),
    );
    hasher.update(&checkpoint_anchor_bytes);
    match compaction_archive_head {
        Some(head) => {
            hasher.update(&[1]);
            let bytes =
                norito::to_bytes(head).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
            hasher.update(
                &u64::try_from(bytes.len())
                    .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?
                    .to_le_bytes(),
            );
            hasher.update(&bytes);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hash_optional_receipt_cursor(&mut hasher, predecessor);
    hasher.update(&page_limit.to_le_bytes());
    hasher.update(
        &u64::try_from(receipts.len())
            .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?
            .to_le_bytes(),
    );
    for receipt in receipts {
        let bytes =
            norito::to_bytes(receipt).map_err(|_| EvidenceViewerErrorV1::InvalidCheckpoint)?;
        hasher.update(
            &u64::try_from(bytes.len())
                .map_err(|_| EvidenceViewerErrorV1::ResourceExhausted)?
                .to_le_bytes(),
        );
        hasher.update(&bytes);
    }
    hash_optional_receipt_cursor(&mut hasher, next_cursor);
    hasher.update(&[u8::from(has_more)]);
    Ok(*hasher.finalize().as_bytes())
}

fn hash_optional_receipt_cursor(
    hasher: &mut blake3::Hasher,
    cursor: Option<EvidenceViewerReceiptCursorV1>,
) {
    match cursor {
        Some(cursor) => {
            hasher.update(&[1]);
            hasher.update(&cursor.sequence.to_le_bytes());
            hasher.update(&cursor.receipt_digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn text_digest(value: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GRANT_CLAIMS_DOMAIN_V1);
    hash_text(&mut hasher, value);
    *hasher.finalize().as_bytes()
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn digest_id16(digest: [u8; 32]) -> [u8; 16] {
    let mut id = [0; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

fn is_zero_digest(digest: [u8; 32]) -> bool {
    digest.iter().all(|byte| *byte == 0)
}

fn len_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn map_provider_qualification_error(
    error: EvidenceViewerRuntimeProviderQualificationErrorV1,
) -> EvidenceViewerErrorV1 {
    match error {
        EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidConfiguredHandle
        | EvidenceViewerRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle
        | EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidConfiguredQualification
        | EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidProviderHandle
        | EvidenceViewerRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle
        | EvidenceViewerRuntimeProviderQualificationErrorV1::SubstitutedProvider
        | EvidenceViewerRuntimeProviderQualificationErrorV1::SignerPublicKeyChanged
        | EvidenceViewerRuntimeProviderQualificationErrorV1::ArchiveIdentityChanged
        | EvidenceViewerRuntimeProviderQualificationErrorV1::ArchivePublicKeyChanged => {
            EvidenceViewerErrorV1::InvalidConfig
        }
        EvidenceViewerRuntimeProviderQualificationErrorV1::UnavailableOrStale
        | EvidenceViewerRuntimeProviderQualificationErrorV1::InvalidQualification
        | EvidenceViewerRuntimeProviderQualificationErrorV1::QualificationMismatch
        | EvidenceViewerRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged => {
            EvidenceViewerErrorV1::RuntimeUnavailable
        }
    }
}

fn map_external_error(error: EvidenceViewerExternalErrorV1) -> EvidenceViewerErrorV1 {
    match error {
        EvidenceViewerExternalErrorV1::Unavailable
        | EvidenceViewerExternalErrorV1::Backpressure => EvidenceViewerErrorV1::RuntimeUnavailable,
        EvidenceViewerExternalErrorV1::Rejected => EvidenceViewerErrorV1::AuthenticationRejected,
    }
}

fn map_archive_external_error(_error: EvidenceViewerExternalErrorV1) -> EvidenceViewerErrorV1 {
    EvidenceViewerErrorV1::RuntimeUnavailable
}

fn map_checkpoint_store_external_error(
    _error: EvidenceViewerCheckpointStoreExternalErrorV1,
) -> EvidenceViewerErrorV1 {
    EvidenceViewerErrorV1::CheckpointUnavailable
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        fs,
        sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    };

    use ed25519_dalek::{Signer as _, SigningKey};
    use tempfile::TempDir;

    use super::*;
    use crate::{
        ModerationQuarantineKeyOperationErrorV1, ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1, ModerationQuarantineKeyWrapper,
        ModerationQuarantineObjectInput, ModerationScreeningInput, ModerationScreeningVerdict,
        config::StorageConfig,
    };

    const CASE_ID: &str = "case-1";
    const ROUND_ID: &str = "round-1";
    const JUROR_ACCOUNT: &str = "juror@review.example";
    const LEGAL_ACCOUNT: &str = "legal@review.example";
    const REVIEW_PURPOSE: &str = "appeal evidence review";
    const BASE_UNIX_MS: u64 = 1_800_000_100_000;
    const MOCK_PROVIDER_SECRET: &str = "MOCK-PROVIDER-SECRET-MUST-NOT-LEAK";
    const EVIDENCE_PAYLOAD: &[u8] = b"EVIDENCE-PAYLOAD-SECRET-MUST-NOT-LEAK";
    const TEST_CHECKPOINT_STORE_HANDLE: &str = "sealed:prod-evidence-checkpoints";
    const TEST_CHECKPOINT_STORE_QUALIFICATION: EvidenceViewerRuntimeProviderQualificationV1 =
        EvidenceViewerRuntimeProviderQualificationV1::new(1, [0xA5; 32]);
    const TEST_COMPACTION_ARCHIVE_HANDLE: &str = "object-lock:prod-evidence-archive";
    const TEST_COMPACTION_ARCHIVE_QUALIFICATION: EvidenceViewerRuntimeProviderQualificationV1 =
        EvidenceViewerRuntimeProviderQualificationV1::new(1, [0xA6; 32]);
    const TEST_COMPACTION_ARCHIVE_ID: [u8; 32] = [0xA7; 32];
    const TEST_COMPACTION_ARCHIVE_SIGNING_SEED: [u8; 32] = [0x52; 32];
    const TEST_QUARANTINE_KEY_PROVIDER_HANDLE: &str = "kms://moderation/quarantine/primary";
    const TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION:
        ModerationQuarantineKeyProviderQualificationV1 =
        ModerationQuarantineKeyProviderQualificationV1::new(1, [0x51; 32]);

    fn test_quarantine_key_provider_config()
    -> iroha_config::parameters::actual::SorafsModerationQuarantineKeyProviderBinding {
        iroha_config::parameters::actual::SorafsModerationQuarantineKeyProviderBinding {
            handle: TEST_QUARANTINE_KEY_PROVIDER_HANDLE.to_owned(),
            revision: TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION.revision(),
            policy_digest: TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION.policy_digest(),
        }
    }

    fn valid_config(public_key: [u8; 32]) -> EvidenceViewerConfigV1 {
        EvidenceViewerConfigV1 {
            checkpoint_path: PathBuf::from("/var/lib/iroha/sorafs/evidence-viewer.to"),
            checkpoint_max_bytes: 1_048_576,
            session_ttl_ms: EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1,
            grant_ttl_ms: 60_000,
            challenge_ttl_ms: 120_000,
            max_range_bytes: 1_048_576,
            max_challenges: 16,
            max_sessions: 16,
            max_receipts: 64,
            max_idempotency_records: 64,
            retention_after_expiry_ms: 86_400_000,
            webauthn_rp_id: "review.example".to_owned(),
            webauthn_allowed_origins: vec!["https://review.example".to_owned()],
            webauthn_handle: "webauthn:prod-evidence-viewer".to_owned(),
            expected_webauthn_qualification: EvidenceViewerRuntimeProviderQualificationV1::new(
                1, [0xA1; 32],
            ),
            grant_handle: "kms:prod-evidence-grants".to_owned(),
            expected_grant_qualification: EvidenceViewerRuntimeProviderQualificationV1::new(
                1, [0xA2; 32],
            ),
            erasure_handle: "kms:prod-evidence-erasure".to_owned(),
            expected_erasure_qualification: EvidenceViewerRuntimeProviderQualificationV1::new(
                1, [0xA4; 32],
            ),
            compaction_archive_handle: TEST_COMPACTION_ARCHIVE_HANDLE.to_owned(),
            expected_compaction_archive_qualification: TEST_COMPACTION_ARCHIVE_QUALIFICATION,
            compaction_archive_id: TEST_COMPACTION_ARCHIVE_ID,
            compaction_archive_public_key: SigningKey::from_bytes(
                &TEST_COMPACTION_ARCHIVE_SIGNING_SEED,
            )
            .verifying_key()
            .to_bytes(),
            compaction_interval_ms: 60_000,
            compaction_max_records: 256,
            receipt_signer_handle: "pkcs11:prod-evidence-receipts".to_owned(),
            expected_receipt_signer_qualification:
                EvidenceViewerRuntimeProviderQualificationV1::new(1, [0xA3; 32]),
            receipt_signer_public_key: public_key,
        }
    }

    #[derive(Clone)]
    struct MockAuthorizationPolicy {
        evidence_digest: [u8; 32],
        policy_digest: [u8; 32],
        finalized_height: u64,
        finalized_block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
        allowed: BTreeSet<(String, EvidenceViewerRoleV1)>,
    }

    struct MockAuthorizationReader {
        policy: Mutex<MockAuthorizationPolicy>,
    }

    impl fmt::Debug for MockAuthorizationReader {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockAuthorizationReader {
        fn new(evidence_digest: [u8; 32]) -> Self {
            Self {
                policy: Mutex::new(MockAuthorizationPolicy {
                    evidence_digest,
                    policy_digest: [0x91; 32],
                    finalized_height: 77,
                    finalized_block_hash: [0x92; 32],
                    finalized_at_unix_ms: BASE_UNIX_MS - 1_000,
                    allowed: BTreeSet::from([
                        (JUROR_ACCOUNT.to_owned(), EvidenceViewerRoleV1::Juror),
                        (LEGAL_ACCOUNT.to_owned(), EvidenceViewerRoleV1::Legal),
                    ]),
                }),
            }
        }

        fn set_allowed(&self, account: &str, role: EvidenceViewerRoleV1, allowed: bool) {
            let mut policy = self.policy.lock().expect("authorization policy lock");
            if allowed {
                policy.allowed.insert((account.to_owned(), role));
            } else {
                policy.allowed.remove(&(account.to_owned(), role));
            }
        }

        fn set_policy_digest(&self, policy_digest: [u8; 32]) {
            self.policy
                .lock()
                .expect("authorization policy lock")
                .policy_digest = policy_digest;
        }

        fn set_finalized_anchor(
            &self,
            finalized_height: u64,
            finalized_block_hash: [u8; 32],
            finalized_at_unix_ms: u64,
        ) {
            let mut policy = self.policy.lock().expect("authorization policy lock");
            policy.finalized_height = finalized_height;
            policy.finalized_block_hash = finalized_block_hash;
            policy.finalized_at_unix_ms = finalized_at_unix_ms;
        }
    }

    impl EvidenceViewerFinalizedAuthorizationReaderV1 for MockAuthorizationReader {
        fn authorize(
            &self,
            case_id: &str,
            round_id: &str,
            viewer_account: &str,
            role: EvidenceViewerRoleV1,
            evidence_bundle_digest: [u8; 32],
        ) -> Result<EvidenceViewerFinalizedAuthorizationV1, EvidenceViewerAuthorizationErrorV1>
        {
            let policy = self
                .policy
                .lock()
                .map_err(|_| EvidenceViewerAuthorizationErrorV1::Unavailable)?
                .clone();
            if case_id != CASE_ID
                || round_id != ROUND_ID
                || evidence_bundle_digest != policy.evidence_digest
                || !policy.allowed.contains(&(viewer_account.to_owned(), role))
            {
                return Err(EvidenceViewerAuthorizationErrorV1::Denied);
            }
            Ok(EvidenceViewerFinalizedAuthorizationV1 {
                case_id: case_id.to_owned(),
                round_id: round_id.to_owned(),
                viewer_account: viewer_account.to_owned(),
                role,
                evidence_bundle_digest,
                policy_digest: policy.policy_digest,
                finalized_height: policy.finalized_height,
                finalized_block_hash: policy.finalized_block_hash,
                finalized_at_unix_ms: policy.finalized_at_unix_ms,
            })
        }
    }

    #[derive(Clone)]
    struct MockChallenge {
        binding_digest: [u8; 32],
        expires_at_unix_ms: u64,
        consumed: bool,
    }

    struct MockProviderQualification {
        revision: AtomicU64,
        policy_digest: Mutex<[u8; 32]>,
        failure: Mutex<Option<EvidenceViewerRuntimeProviderReadinessErrorV1>>,
        policy_drift_after_operation: Mutex<Option<[u8; 32]>>,
    }

    impl MockProviderQualification {
        fn new(policy_byte: u8) -> Self {
            Self {
                revision: AtomicU64::new(1),
                policy_digest: Mutex::new([policy_byte; 32]),
                failure: Mutex::new(None),
                policy_drift_after_operation: Mutex::new(None),
            }
        }

        fn observe(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            if let Some(error) = self
                .failure
                .lock()
                .map_err(|_| EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable)?
                .as_ref()
                .copied()
            {
                return Err(error);
            }
            let policy_digest = *self
                .policy_digest
                .lock()
                .map_err(|_| EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable)?;
            Ok(EvidenceViewerRuntimeProviderQualificationV1::new(
                self.revision.load(Ordering::SeqCst),
                policy_digest,
            ))
        }

        fn set_revision(&self, revision: u64) {
            self.revision.store(revision, Ordering::SeqCst);
        }

        fn set_policy_digest(&self, policy_digest: [u8; 32]) {
            *self
                .policy_digest
                .lock()
                .expect("provider qualification policy lock") = policy_digest;
        }

        fn set_failure(&self, failure: Option<EvidenceViewerRuntimeProviderReadinessErrorV1>) {
            *self
                .failure
                .lock()
                .expect("provider qualification failure lock") = failure;
        }

        fn drift_policy_after_next_operation(&self, policy_digest: [u8; 32]) {
            *self
                .policy_drift_after_operation
                .lock()
                .expect("provider qualification drift lock") = Some(policy_digest);
        }

        fn operation_guard(&self) -> MockProviderOperationGuard<'_> {
            MockProviderOperationGuard(self)
        }
    }

    struct MockProviderOperationGuard<'a>(&'a MockProviderQualification);

    impl Drop for MockProviderOperationGuard<'_> {
        fn drop(&mut self) {
            if let Some(policy_digest) = self
                .0
                .policy_drift_after_operation
                .lock()
                .expect("provider qualification drift lock")
                .take()
            {
                self.0.set_policy_digest(policy_digest);
            }
        }
    }

    struct MockWebAuthn {
        handle: String,
        qualification: MockProviderQualification,
        sequence: AtomicU64,
        challenges: Mutex<BTreeMap<String, MockChallenge>>,
    }

    impl fmt::Debug for MockWebAuthn {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockWebAuthn {
        fn new(handle: &str) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: MockProviderQualification::new(0xA1),
                sequence: AtomicU64::new(0),
                challenges: Mutex::new(BTreeMap::new()),
            }
        }

        fn issue_call_count(&self) -> u64 {
            self.sequence.load(Ordering::SeqCst)
        }
    }

    impl EvidenceViewerRuntimeProviderV1 for MockWebAuthn {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            self.qualification.observe()
        }
    }

    impl EvidenceViewerWebAuthnBoundaryV1 for MockWebAuthn {
        fn issue_challenge(
            &self,
            binding_digest: [u8; 32],
            expires_at_unix_ms: u64,
        ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
            let token = format!("webauthn-{}-{sequence}", hex::encode(binding_digest));
            self.challenges
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?
                .insert(
                    token.clone(),
                    MockChallenge {
                        binding_digest,
                        expires_at_unix_ms,
                        consumed: false,
                    },
                );
            OpaqueEvidenceViewerSecretV1::new(token)
        }

        fn verify_and_consume(
            &self,
            challenge: &str,
            assertion: &[u8],
            binding_digest: [u8; 32],
            rp_id: &str,
            allowed_origins: &[String],
            now_unix_ms: u64,
        ) -> Result<EvidenceViewerWebAuthnResultV1, EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            if !assertion.starts_with(b"valid-webauthn-assertion")
                || rp_id != "review.example"
                || allowed_origins.len() != 1
                || allowed_origins[0] != "https://review.example"
            {
                return Err(EvidenceViewerExternalErrorV1::Rejected);
            }
            let mut challenges = self
                .challenges
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
            let expected = challenges
                .get_mut(challenge)
                .ok_or(EvidenceViewerExternalErrorV1::Rejected)?;
            if expected.consumed
                || expected.binding_digest != binding_digest
                || now_unix_ms >= expected.expires_at_unix_ms
            {
                return Err(EvidenceViewerExternalErrorV1::Rejected);
            }
            expected.consumed = true;
            let mut transcript = blake3::Hasher::new();
            transcript.update(challenge.as_bytes());
            transcript.update(assertion);
            transcript.update(&binding_digest);
            Ok(EvidenceViewerWebAuthnResultV1 {
                attestation_digest: *transcript.finalize().as_bytes(),
                credential_id_digest: [0xA2; 32],
                authenticator_counter: 7,
            })
        }
    }

    struct MockGrantBoundary {
        handle: String,
        qualification: MockProviderQualification,
        sequence: AtomicU64,
        issued: Mutex<BTreeMap<String, EvidenceViewerGrantClaimsV1>>,
        revoked: Mutex<BTreeSet<[u8; 32]>>,
    }

    impl fmt::Debug for MockGrantBoundary {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockGrantBoundary {
        fn new(handle: &str) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: MockProviderQualification::new(0xA2),
                sequence: AtomicU64::new(0),
                issued: Mutex::new(BTreeMap::new()),
                revoked: Mutex::new(BTreeSet::new()),
            }
        }

        fn was_revoked(&self, token: &str) -> bool {
            self.revoked
                .lock()
                .expect("grant revocation lock")
                .contains(blake3::hash(token.as_bytes()).as_bytes())
        }

        fn issued_tokens(&self) -> Vec<String> {
            self.issued
                .lock()
                .expect("issued grants lock")
                .keys()
                .cloned()
                .collect()
        }
    }

    impl EvidenceViewerRuntimeProviderV1 for MockGrantBoundary {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            self.qualification.observe()
        }
    }

    impl EvidenceViewerGrantBoundaryV1 for MockGrantBoundary {
        fn issue(
            &self,
            claims: &EvidenceViewerGrantClaimsV1,
        ) -> Result<OpaqueEvidenceViewerSecretV1, EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            let claims_bytes =
                norito::to_bytes(claims).map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
            let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
            let token = format!(
                "grant-{sequence}-{}",
                hex::encode(&blake3::hash(&claims_bytes).as_bytes()[..16])
            );
            self.issued
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?
                .insert(token.clone(), claims.clone());
            OpaqueEvidenceViewerSecretV1::new(token)
        }

        fn verify(
            &self,
            token: &str,
            claims: &EvidenceViewerGrantClaimsV1,
            now_unix_ms: u64,
        ) -> Result<(), EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            if self
                .revoked
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?
                .contains(blake3::hash(token.as_bytes()).as_bytes())
            {
                return Err(EvidenceViewerExternalErrorV1::Rejected);
            }
            let issued = self
                .issued
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
            if issued.get(token) != Some(claims)
                || now_unix_ms < claims.issued_at_unix_ms
                || now_unix_ms >= claims.expires_at_unix_ms
            {
                return Err(EvidenceViewerExternalErrorV1::Rejected);
            }
            Ok(())
        }

        fn revoke(&self, token_digest: [u8; 32]) -> Result<(), EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            self.revoked
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?
                .insert(token_digest);
            Ok(())
        }
    }

    struct MockReceiptSigner {
        handle: String,
        qualification: MockProviderQualification,
        signing_key: SigningKey,
        corrupt_signatures: AtomicBool,
        public_key_calls: AtomicUsize,
        sign_calls: AtomicUsize,
    }

    impl fmt::Debug for MockReceiptSigner {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockReceiptSigner {
        fn new(handle: &str, signing_key: SigningKey) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: MockProviderQualification::new(0xA3),
                signing_key,
                corrupt_signatures: AtomicBool::new(false),
                public_key_calls: AtomicUsize::new(0),
                sign_calls: AtomicUsize::new(0),
            }
        }

        fn set_corrupt_signatures(&self, corrupt: bool) {
            self.corrupt_signatures.store(corrupt, Ordering::SeqCst);
        }

        fn sign_call_count(&self) -> usize {
            self.sign_calls.load(Ordering::SeqCst)
        }

        fn public_key_call_count(&self) -> usize {
            self.public_key_calls.load(Ordering::SeqCst)
        }
    }

    impl EvidenceViewerRuntimeProviderV1 for MockReceiptSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            self.qualification.observe()
        }
    }

    impl EvidenceViewerReceiptSignerV1 for MockReceiptSigner {
        fn public_key(&self) -> [u8; 32] {
            let _operation = self.qualification.operation_guard();
            self.public_key_calls.fetch_add(1, Ordering::SeqCst);
            self.signing_key.verifying_key().to_bytes()
        }

        fn sign(&self, message: &[u8]) -> Result<[u8; 64], EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            self.sign_calls.fetch_add(1, Ordering::SeqCst);
            let mut signature = self.signing_key.sign(message).to_bytes();
            if self.corrupt_signatures.load(Ordering::SeqCst) {
                signature[0] ^= 1;
            }
            Ok(signature)
        }
    }

    type MockErasureCallV1 = ([u8; 32], [u8; 16], [u8; 16], [u8; 32]);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockErasureInjectedResult {
        Pass,
        Unavailable,
        ZeroDigest,
        CommitThenUnavailable,
    }

    struct MockErasureBoundary {
        handle: String,
        qualification: MockProviderQualification,
        calls: Mutex<Vec<MockErasureCallV1>>,
        commits: Mutex<BTreeMap<[u8; 32], [u8; 32]>>,
        injected_results: Mutex<VecDeque<MockErasureInjectedResult>>,
        commit_then_unavailable_once: AtomicBool,
    }

    impl fmt::Debug for MockErasureBoundary {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockErasureBoundary {
        fn new(handle: &str) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: MockProviderQualification::new(0xA4),
                calls: Mutex::new(Vec::new()),
                commits: Mutex::new(BTreeMap::new()),
                injected_results: Mutex::new(VecDeque::new()),
                commit_then_unavailable_once: AtomicBool::new(false),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().expect("erasure calls lock").len()
        }

        fn commit_count(&self) -> usize {
            self.commits.lock().expect("erasure commits lock").len()
        }

        fn commit_then_unavailable_once(&self) {
            self.commit_then_unavailable_once
                .store(true, Ordering::SeqCst);
        }

        fn inject_results(&self, results: &[MockErasureInjectedResult]) {
            *self
                .injected_results
                .lock()
                .expect("erasure injected results lock") = results.iter().copied().collect();
        }
    }

    impl EvidenceViewerRuntimeProviderV1 for MockErasureBoundary {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            self.qualification.observe()
        }
    }

    impl EvidenceViewerErasureBoundaryV1 for MockErasureBoundary {
        fn erase(
            &self,
            operation_id: [u8; 32],
            quarantine_id: [u8; 16],
            object_id: [u8; 16],
            evidence_digest: [u8; 32],
        ) -> Result<[u8; 32], EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            self.calls
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?
                .push((operation_id, quarantine_id, object_id, evidence_digest));
            let injected_result = {
                let mut injected_results = self
                    .injected_results
                    .lock()
                    .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
                injected_results
                    .pop_front()
                    .unwrap_or(MockErasureInjectedResult::Pass)
            };
            match injected_result {
                MockErasureInjectedResult::Unavailable => {
                    return Err(EvidenceViewerExternalErrorV1::Unavailable);
                }
                MockErasureInjectedResult::ZeroDigest => return Ok([0; 32]),
                MockErasureInjectedResult::Pass
                | MockErasureInjectedResult::CommitThenUnavailable => {}
            }
            let mut commits = self
                .commits
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
            let commit_digest = commits.get(&operation_id).copied().unwrap_or_else(|| {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"sorafs.evidence-viewer.test-erasure.v1");
                hasher.update(&operation_id);
                hasher.update(&quarantine_id);
                hasher.update(&object_id);
                hasher.update(&evidence_digest);
                let commit_digest = *hasher.finalize().as_bytes();
                commits.insert(operation_id, commit_digest);
                commit_digest
            });
            if injected_result == MockErasureInjectedResult::CommitThenUnavailable
                || self
                    .commit_then_unavailable_once
                    .swap(false, Ordering::SeqCst)
            {
                return Err(EvidenceViewerExternalErrorV1::Unavailable);
            }
            Ok(commit_digest)
        }
    }

    #[derive(Clone, Default)]
    enum MockCheckpointCasMode {
        #[default]
        Normal,
        AmbiguousCommit,
        AmbiguousNoCommit,
        RejectedNoCommit,
        RaceWith(Box<EvidenceViewerCheckpointStoreRecordV1>),
    }

    struct MockCheckpointStore {
        handle: String,
        qualification: MockProviderQualification,
        latest: Mutex<Option<EvidenceViewerCheckpointStoreRecordV1>>,
        next_cas_mode: Mutex<MockCheckpointCasMode>,
        load_calls: AtomicUsize,
        cas_calls: AtomicUsize,
    }

    impl fmt::Debug for MockCheckpointStore {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockCheckpointStore {
        fn new(handle: &str) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: MockProviderQualification::new(0xA5),
                latest: Mutex::new(None),
                next_cas_mode: Mutex::new(MockCheckpointCasMode::Normal),
                load_calls: AtomicUsize::new(0),
                cas_calls: AtomicUsize::new(0),
            }
        }

        fn current(&self) -> Option<EvidenceViewerCheckpointStoreRecordV1> {
            self.latest
                .lock()
                .expect("checkpoint-store latest lock")
                .clone()
        }

        fn replace_latest(&self, record: Option<EvidenceViewerCheckpointStoreRecordV1>) {
            *self.latest.lock().expect("checkpoint-store latest lock") = record;
        }

        fn set_next_cas_mode(&self, mode: MockCheckpointCasMode) {
            *self
                .next_cas_mode
                .lock()
                .expect("checkpoint-store CAS mode lock") = mode;
        }

        fn load_call_count(&self) -> usize {
            self.load_calls.load(Ordering::SeqCst)
        }

        fn cas_call_count(&self) -> usize {
            self.cas_calls.load(Ordering::SeqCst)
        }
    }

    impl EvidenceViewerRuntimeProviderV1 for MockCheckpointStore {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            self.qualification.observe()
        }
    }

    impl EvidenceViewerCheckpointStoreV1 for MockCheckpointStore {
        fn load_latest(
            &self,
        ) -> Result<
            Option<EvidenceViewerCheckpointStoreRecordV1>,
            EvidenceViewerCheckpointStoreExternalErrorV1,
        > {
            let _operation = self.qualification.operation_guard();
            self.load_calls.fetch_add(1, Ordering::SeqCst);
            let latest = self
                .latest
                .lock()
                .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?;
            Ok(latest.clone())
        }

        fn compare_and_swap_latest(
            &self,
            expected_revision: Option<[u8; 32]>,
            next: &EvidenceViewerCheckpointStoreRecordV1,
        ) -> Result<(), EvidenceViewerCheckpointStoreExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            self.cas_calls.fetch_add(1, Ordering::SeqCst);
            let mut latest = self
                .latest
                .lock()
                .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?;
            if latest.as_ref().map(|record| record.revision) != expected_revision {
                return Err(EvidenceViewerCheckpointStoreExternalErrorV1::Rejected);
            }
            let mode = std::mem::take(
                &mut *self
                    .next_cas_mode
                    .lock()
                    .map_err(|_| EvidenceViewerCheckpointStoreExternalErrorV1::Unavailable)?,
            );
            match mode {
                MockCheckpointCasMode::Normal => {
                    *latest = Some(next.clone());
                    Ok(())
                }
                MockCheckpointCasMode::AmbiguousCommit => {
                    *latest = Some(next.clone());
                    Err(EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous)
                }
                MockCheckpointCasMode::AmbiguousNoCommit => {
                    Err(EvidenceViewerCheckpointStoreExternalErrorV1::Ambiguous)
                }
                MockCheckpointCasMode::RejectedNoCommit => {
                    Err(EvidenceViewerCheckpointStoreExternalErrorV1::Rejected)
                }
                MockCheckpointCasMode::RaceWith(record) => {
                    *latest = Some(*record);
                    Err(EvidenceViewerCheckpointStoreExternalErrorV1::Rejected)
                }
            }
        }
    }

    struct MockCompactionArchive {
        handle: String,
        qualification: MockProviderQualification,
        archive_id: [u8; 32],
        signing_key: SigningKey,
        artifacts: Mutex<BTreeMap<[u8; 32], ([u8; 32], EvidenceViewerCompactionArchiveReadbackV1)>>,
        install_calls: AtomicUsize,
        read_calls: AtomicUsize,
        append_trailing_on_next_read: AtomicBool,
    }

    impl fmt::Debug for MockCompactionArchive {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(MOCK_PROVIDER_SECRET)
        }
    }

    impl MockCompactionArchive {
        fn new(handle: &str) -> Self {
            Self::with_identity(
                handle,
                TEST_COMPACTION_ARCHIVE_ID,
                TEST_COMPACTION_ARCHIVE_SIGNING_SEED,
            )
        }

        fn with_identity(handle: &str, archive_id: [u8; 32], signing_seed: [u8; 32]) -> Self {
            Self {
                handle: handle.to_owned(),
                qualification: MockProviderQualification::new(0xA6),
                archive_id,
                signing_key: SigningKey::from_bytes(&signing_seed),
                artifacts: Mutex::new(BTreeMap::new()),
                install_calls: AtomicUsize::new(0),
                read_calls: AtomicUsize::new(0),
                append_trailing_on_next_read: AtomicBool::new(false),
            }
        }

        fn install_call_count(&self) -> usize {
            self.install_calls.load(Ordering::SeqCst)
        }

        fn read_call_count(&self) -> usize {
            self.read_calls.load(Ordering::SeqCst)
        }

        fn retained_artifact_count(&self) -> usize {
            self.artifacts
                .lock()
                .expect("compaction archive artifacts lock")
                .len()
        }

        fn sole_operation_id(&self) -> [u8; 32] {
            *self
                .artifacts
                .lock()
                .expect("compaction archive artifacts lock")
                .keys()
                .next()
                .expect("installed compaction operation")
        }

        fn artifact(&self, operation_id: [u8; 32]) -> Vec<u8> {
            self.artifacts
                .lock()
                .expect("compaction archive artifacts lock")
                .get(&operation_id)
                .map(|(_, readback)| readback.canonical_artifact.clone())
                .expect("installed compaction artifact")
        }

        fn replace_artifact(&self, operation_id: [u8; 32], bytes: Vec<u8>) {
            self.artifacts
                .lock()
                .expect("compaction archive artifacts lock")
                .get_mut(&operation_id)
                .expect("installed compaction artifact")
                .1
                .canonical_artifact = bytes;
        }

        fn remove_artifact(&self, operation_id: [u8; 32]) {
            self.artifacts
                .lock()
                .expect("compaction archive artifacts lock")
                .remove(&operation_id)
                .expect("installed compaction artifact");
        }

        fn append_trailing_on_next_read(&self) {
            self.append_trailing_on_next_read
                .store(true, Ordering::SeqCst);
        }

        fn corrupt_signature(&self, operation_id: [u8; 32]) {
            let mut artifacts = self
                .artifacts
                .lock()
                .expect("compaction archive artifacts lock");
            let (_, readback) = artifacts
                .get_mut(&operation_id)
                .expect("installed compaction artifact");
            readback.signature[0] ^= 1;
        }
    }

    impl EvidenceViewerRuntimeProviderV1 for MockCompactionArchive {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            EvidenceViewerRuntimeProviderQualificationV1,
            EvidenceViewerRuntimeProviderReadinessErrorV1,
        > {
            self.qualification.observe()
        }
    }

    impl EvidenceViewerCompactionArchiveV1 for MockCompactionArchive {
        fn archive_id(&self) -> [u8; 32] {
            self.archive_id
        }

        fn signing_public_key(&self) -> [u8; 32] {
            self.signing_key.verifying_key().to_bytes()
        }

        fn install(
            &self,
            operation_id: [u8; 32],
            receipt_message: [u8; 32],
            canonical_artifact: &[u8],
        ) -> Result<[u8; 64], EvidenceViewerExternalErrorV1> {
            let _operation = self.qualification.operation_guard();
            self.install_calls.fetch_add(1, Ordering::SeqCst);
            let mut artifacts = self
                .artifacts
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?;
            match artifacts.get(&operation_id) {
                Some((existing_message, existing))
                    if *existing_message == receipt_message
                        && existing.canonical_artifact.as_slice() == canonical_artifact =>
                {
                    Ok(existing.signature)
                }
                Some(_) => Err(EvidenceViewerExternalErrorV1::Rejected),
                None => {
                    let signature = self.signing_key.sign(&receipt_message).to_bytes();
                    artifacts.insert(
                        operation_id,
                        (
                            receipt_message,
                            EvidenceViewerCompactionArchiveReadbackV1 {
                                canonical_artifact: canonical_artifact.to_vec(),
                                signature,
                            },
                        ),
                    );
                    Ok(signature)
                }
            }
        }

        fn read(
            &self,
            operation_id: [u8; 32],
        ) -> Result<Option<EvidenceViewerCompactionArchiveReadbackV1>, EvidenceViewerExternalErrorV1>
        {
            let _operation = self.qualification.operation_guard();
            self.read_calls.fetch_add(1, Ordering::SeqCst);
            let mut bytes = self
                .artifacts
                .lock()
                .map_err(|_| EvidenceViewerExternalErrorV1::Unavailable)?
                .get(&operation_id)
                .map(|(_, readback)| readback.clone());
            if self
                .append_trailing_on_next_read
                .swap(false, Ordering::SeqCst)
                && let Some(readback) = bytes.as_mut()
            {
                readback.canonical_artifact.push(0xA5);
            }
            Ok(bytes)
        }
    }

    #[derive(Debug)]
    struct TestQuarantineKeyWrapper {
        key_id: String,
        key: [u8; 32],
    }

    impl TestQuarantineKeyWrapper {
        fn nonce(&self, context_digest: [u8; 32]) -> [u8; 12] {
            let mut hasher = blake3::Hasher::new_keyed(&self.key);
            hasher.update(b"sorafs.evidence-viewer.test-wrapper.nonce.v1");
            hasher.update(self.key_id.as_bytes());
            hasher.update(&context_digest);
            let mut nonce = [0; 12];
            nonce.copy_from_slice(&hasher.finalize().as_bytes()[..12]);
            nonce
        }
    }

    impl ModerationQuarantineKeyWrapper for TestQuarantineKeyWrapper {
        fn provider_handle(&self) -> &str {
            TEST_QUARANTINE_KEY_PROVIDER_HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<
            ModerationQuarantineKeyProviderQualificationV1,
            ModerationQuarantineKeyProviderReadinessErrorV1,
        > {
            Ok(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION)
        }

        fn active_key_id(&self) -> &str {
            &self.key_id
        }

        fn wrap_dek(
            &self,
            context_digest: [u8; 32],
            dek: &[u8; 32],
        ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
            use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};

            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(self.key)
                .map_err(|error| {
                    ModerationQuarantineKeyOperationErrorV1::Rejected
                        .after_scrubbing_provider_diagnostic(error.to_string())
                })?
                .encrypt(
                    self.nonce(context_digest).as_slice(),
                    context_digest.as_slice(),
                    dek.as_slice(),
                )
                .map_err(|error| {
                    ModerationQuarantineKeyOperationErrorV1::Rejected
                        .after_scrubbing_provider_diagnostic(error.to_string())
                })
        }

        fn unwrap_dek(
            &self,
            key_id: &str,
            context_digest: [u8; 32],
            wrapped_dek: &[u8],
        ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
            use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};

            if key_id != self.key_id {
                return Err(ModerationQuarantineKeyOperationErrorV1::StaleOrRevoked);
            }
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(self.key)
                .map_err(|error| {
                    ModerationQuarantineKeyOperationErrorV1::Rejected
                        .after_scrubbing_provider_diagnostic(error.to_string())
                })?
                .decrypt(
                    self.nonce(context_digest).as_slice(),
                    context_digest.as_slice(),
                    wrapped_dek,
                )
                .map_err(|error| {
                    ModerationQuarantineKeyOperationErrorV1::Rejected
                        .after_scrubbing_provider_diagnostic(error.to_string())
                })?
                .try_into()
                .map_err(|_| ModerationQuarantineKeyOperationErrorV1::Rejected)
        }
    }

    struct EvidenceViewerFixture {
        _temp_dir: TempDir,
        config: EvidenceViewerConfigV1,
        deps: EvidenceViewerRuntimeDepsV1,
        node: NodeHandle,
        authorization: Arc<MockAuthorizationReader>,
        webauthn: Arc<MockWebAuthn>,
        grants: Arc<MockGrantBoundary>,
        signer: Arc<MockReceiptSigner>,
        erasure: Arc<MockErasureBoundary>,
        compaction_archive: Arc<MockCompactionArchive>,
        checkpoint_store: Arc<MockCheckpointStore>,
        quarantine_id: [u8; 16],
        object: ModerationQuarantineObjectRecord,
    }

    impl EvidenceViewerFixture {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().expect("create evidence-viewer temp dir");
            let root = temp_dir
                .path()
                .canonicalize()
                .expect("canonical evidence-viewer temp dir");
            let storage_config = StorageConfig::builder()
                .enabled(true)
                .data_dir(root.join("storage"))
                .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config()))
                .build();
            let key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> =
                Arc::new(TestQuarantineKeyWrapper {
                    key_id: "kms:test/evidence-quarantine".to_owned(),
                    key: [0xD1; 32],
                });
            let node = NodeHandle::try_new_with_quarantine_key_wrapper(storage_config, key_wrapper)
                .expect("start evidence-viewer test node");
            let screening = ModerationScreeningInput {
                subject: "cid:bafy-evidence-viewer".to_owned(),
                subject_digest: *blake3::hash(EVIDENCE_PAYLOAD).as_bytes(),
                manifest_id: [0x12; 16],
                runner_hash: [0x34; 32],
                combined_score_bps: 7_500,
                verdict: ModerationScreeningVerdict::Quarantine,
                screened_at_unix: BASE_UNIX_MS / 1_000 - 10,
                evidence_digest: Some([0xE1; 32]),
                policy_digest: Some([0xC1; 32]),
                notes: None,
            };
            let quarantine_id = node
                .record_moderation_screening_result(screening)
                .expect("record test quarantine")
                .quarantine
                .expect("quarantine verdict")
                .quarantine_id;
            let object = node
                .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                    quarantine_id,
                    payload: EVIDENCE_PAYLOAD.to_vec(),
                    captured_at_unix: BASE_UNIX_MS / 1_000 - 5,
                    content_type: Some("application/octet-stream".to_owned()),
                    notes: None,
                })
                .expect("seal test evidence");

            let authorization = Arc::new(MockAuthorizationReader::new(object.payload_digest));
            let webauthn = Arc::new(MockWebAuthn::new("webauthn:prod-evidence-viewer"));
            let grants = Arc::new(MockGrantBoundary::new("kms:prod-evidence-grants"));
            let signing_key = SigningKey::from_bytes(&[0x51; 32]);
            let signer = Arc::new(MockReceiptSigner::new(
                "pkcs11:prod-evidence-receipts",
                signing_key,
            ));
            let erasure = Arc::new(MockErasureBoundary::new("kms:prod-evidence-erasure"));
            let compaction_archive =
                Arc::new(MockCompactionArchive::new(TEST_COMPACTION_ARCHIVE_HANDLE));
            let checkpoint_store = Arc::new(MockCheckpointStore::new(TEST_CHECKPOINT_STORE_HANDLE));
            let mut config = valid_config(signer.public_key());
            config.checkpoint_path = root.join("evidence-viewer.to");
            let deps = EvidenceViewerRuntimeDepsV1 {
                authorization_reader: authorization.clone(),
                webauthn: webauthn.clone(),
                grants: grants.clone(),
                receipt_signer: signer.clone(),
                erasure: erasure.clone(),
                compaction_archive: compaction_archive.clone(),
            };
            Self {
                _temp_dir: temp_dir,
                config,
                deps,
                node,
                authorization,
                webauthn,
                grants,
                signer,
                erasure,
                compaction_archive,
                checkpoint_store,
                quarantine_id,
                object,
            }
        }

        fn open(&self) -> EvidenceViewerServiceV1 {
            self.open_with(self.config.clone(), self.deps.clone())
                .expect("open evidence-viewer service")
        }

        fn open_with(
            &self,
            config: EvidenceViewerConfigV1,
            deps: EvidenceViewerRuntimeDepsV1,
        ) -> Result<EvidenceViewerServiceV1, EvidenceViewerErrorV1> {
            EvidenceViewerServiceV1::open_with_checkpoint_store(
                config,
                deps,
                self.node.clone(),
                TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
                TEST_CHECKPOINT_STORE_QUALIFICATION,
                self.checkpoint_store.clone(),
            )
        }

        fn issue_challenge(
            &self,
            service: &EvidenceViewerServiceV1,
            account: &str,
            role: EvidenceViewerRoleV1,
            idempotency_key: [u8; 32],
            now_unix_ms: u64,
        ) -> EvidenceViewerChallengeIssuedV1 {
            service
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: self.quarantine_id,
                    viewer_account: account.to_owned(),
                    role,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key,
                    now_unix_ms,
                })
                .expect("issue evidence challenge")
        }

        fn create_session(
            &self,
            service: &EvidenceViewerServiceV1,
            challenge: &str,
            assertion: &[u8],
            idempotency_key: [u8; 32],
            now_unix_ms: u64,
        ) -> Result<EvidenceViewerSessionIssuedV1, EvidenceViewerErrorV1> {
            service.create_session(EvidenceViewerSessionRequestV1 {
                case_id: CASE_ID.to_owned(),
                round_id: ROUND_ID.to_owned(),
                quarantine_id: self.quarantine_id,
                viewer_account: JUROR_ACCOUNT.to_owned(),
                role: EvidenceViewerRoleV1::Juror,
                purpose: REVIEW_PURPOSE.to_owned(),
                challenge: OpaqueEvidenceViewerSecretV1::new(challenge.to_owned())
                    .expect("rebuild opaque challenge"),
                webauthn_assertion: assertion.to_vec(),
                idempotency_key,
                now_unix_ms,
            })
        }
    }

    fn opaque(value: &str) -> OpaqueEvidenceViewerSecretV1 {
        OpaqueEvidenceViewerSecretV1::new(value.to_owned()).expect("valid test token")
    }

    fn current_checkpoint_digest(service: &EvidenceViewerServiceV1) -> [u8; 32] {
        service
            .audit_status()
            .expect("read signed checkpoint anchor")
            .checkpoint_anchor
            .checkpoint_digest
    }

    fn test_erasure_intent(
        quarantine_id: [u8; 16],
        object_id: [u8; 16],
        evidence_digest: [u8; 32],
        idempotency_key: [u8; 32],
        request_digest: [u8; 32],
        requested_at_unix_ms: u64,
    ) -> EvidenceViewerErasureIntentV1 {
        EvidenceViewerErasureIntentV1 {
            operation_id: erasure_operation_id(
                idempotency_key,
                request_digest,
                quarantine_id,
                object_id,
                evidence_digest,
            ),
            quarantine_id,
            object_id,
            evidence_digest,
            case_id: CASE_ID.to_owned(),
            round_id: ROUND_ID.to_owned(),
            actor_account: LEGAL_ACCOUNT.to_owned(),
            idempotency_key,
            request_digest,
            requested_at_unix_ms,
        }
    }

    fn persist_test_erasure_intents(
        service: &EvidenceViewerServiceV1,
        intents: impl IntoIterator<Item = EvidenceViewerErasureIntentV1>,
    ) {
        let mut state = service.state.lock().expect("test erasure state lock");
        for intent in intents {
            assert!(
                state
                    .erasure_intents
                    .insert(intent.quarantine_id, intent)
                    .is_none(),
                "test erasure intents must use distinct quarantine ids"
            );
        }
        service
            .persist_locked(&mut state)
            .expect("persist test erasure intents");
    }

    fn assert_live_refresh_rejects_historical_archive_damage(
        corrupt_artifact: bool,
        expected_error: EvidenceViewerErrorV1,
    ) {
        let mut fixture = EvidenceViewerFixture::new();
        fixture.config.compaction_max_records = 1;
        let writer = fixture.open();
        for idempotency_key in [[0xE6; 32], [0xE7; 32]] {
            fixture.issue_challenge(
                &writer,
                JUROR_ACCOUNT,
                EvidenceViewerRoleV1::Juror,
                idempotency_key,
                BASE_UNIX_MS,
            );
        }
        let cutoff = BASE_UNIX_MS + fixture.config.challenge_ttl_ms;
        let first = writer
            .compact_expired_tick(cutoff)
            .expect("first live-refresh archive transition")
            .expect("first expired challenge");
        writer
            .compact_expired_tick(cutoff)
            .expect("second live-refresh archive transition")
            .expect("second expired challenge");

        let mut stale_config = fixture.config.clone();
        stale_config.checkpoint_path =
            fixture
                .config
                .checkpoint_path
                .with_file_name(if corrupt_artifact {
                    "archive-corrupt-refresh-replica.to"
                } else {
                    "archive-missing-refresh-replica.to"
                });
        let stale = fixture
            .open_with(stale_config.clone(), fixture.deps.clone())
            .expect("open replica before archive damage");
        let stale_state = stale.state.lock().expect("stale state lock").clone();
        let stale_cache =
            fs::read(&stale_config.checkpoint_path).expect("read pre-refresh replica cache");

        fixture.issue_challenge(
            &writer,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xE8; 32],
            cutoff + 1,
        );
        assert_eq!(
            fixture
                .checkpoint_store
                .current()
                .expect("authoritative direct successor")
                .generation,
            stale_state
                .checkpoint_record
                .as_ref()
                .expect("stale checkpoint record")
                .generation
                + 1
        );

        if corrupt_artifact {
            let mut corrupt = fixture.compaction_archive.artifact(first.operation_id);
            let last = corrupt
                .last_mut()
                .expect("non-empty historical archive artifact");
            *last ^= 1;
            fixture
                .compaction_archive
                .replace_artifact(first.operation_id, corrupt);
        } else {
            fixture
                .compaction_archive
                .remove_artifact(first.operation_id);
        }

        assert_eq!(
            stale
                .refresh_authoritative_checkpoint()
                .expect_err("historical archive damage must reject live refresh"),
            expected_error
        );
        assert_eq!(
            *stale.state.lock().expect("post-refresh stale state lock"),
            stale_state,
            "archive verification must precede in-memory adoption"
        );
        assert_eq!(
            fs::read(&stale_config.checkpoint_path).expect("read rejected refresh cache"),
            stale_cache,
            "archive verification must precede local-cache replacement"
        );
    }

    fn assert_receipt_chain(
        receipts: &[EvidenceViewerSignedReceiptV1],
        config: &EvidenceViewerConfigV1,
    ) {
        let mut previous = [0; 32];
        for (index, receipt) in receipts.iter().enumerate() {
            assert_eq!(receipt.body.sequence, index as u64 + 1);
            assert_eq!(receipt.body.previous_receipt_digest, previous);
            receipt
                .verify(
                    &config.receipt_signer_handle,
                    config.receipt_signer_public_key,
                )
                .expect("verify receipt signature");
            previous = receipt.receipt_digest;
        }
    }

    fn signed_receipt(signing_key: &SigningKey) -> EvidenceViewerSignedReceiptV1 {
        let body = EvidenceViewerReceiptBodyV1 {
            version: EVIDENCE_VIEWER_RECEIPT_VERSION_V1,
            sequence: 1,
            kind: EvidenceViewerReceiptKindV1::RangeAccessed,
            session_id: Some([0x11; 16]),
            case_id: Some("case-1".to_owned()),
            round_id: Some("round-1".to_owned()),
            quarantine_id: [0x22; 16],
            object_id: [0x33; 16],
            evidence_digest: [0x44; 32],
            actor_account_digest: [0x55; 32],
            idempotency_key_digest: [0x66; 32],
            request_digest: [0x77; 32],
            range_start: Some(0),
            range_end: Some(1024),
            issued_at_unix_ms: 1_800_000_000_000,
            previous_receipt_digest: [0; 32],
        };
        let receipt_digest = receipt_body_digest(&body).expect("receipt digest");
        EvidenceViewerSignedReceiptV1 {
            body,
            receipt_digest,
            signer_handle: "pkcs11:prod-evidence-receipts".to_owned(),
            signer_public_key: signing_key.verifying_key().to_bytes(),
            signature: signing_key
                .sign(&receipt_signature_message(receipt_digest))
                .to_bytes(),
        }
    }

    fn signed_checkpoint_envelope(
        signing_key: &SigningKey,
        config: &EvidenceViewerConfigV1,
        checkpoint: EvidenceViewerCheckpointV1,
    ) -> EvidenceViewerCheckpointEnvelopeV1 {
        let mut checkpoint_anchor = EvidenceViewerSignedCheckpointAnchorV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            checkpoint_generation: 1,
            predecessor_checkpoint_revision: None,
            predecessor_checkpoint_digest: None,
            checkpoint_digest: checkpoint_payload_digest(&checkpoint).expect("checkpoint digest"),
            receipt_count: u64::try_from(checkpoint.receipts.len()).expect("receipt count"),
            chain_head: checkpoint.receipts.last().map(receipt_cursor),
            compaction_archive_head_digest: checkpoint
                .compaction_archive_head
                .as_ref()
                .map(|head| head.head_digest),
            checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
            checkpoint_store_revision: TEST_CHECKPOINT_STORE_QUALIFICATION.revision(),
            checkpoint_store_policy_digest: TEST_CHECKPOINT_STORE_QUALIFICATION.policy_digest(),
            signer_handle: config.receipt_signer_handle.clone(),
            signer_public_key: config.receipt_signer_public_key,
            signature: [0; 64],
        };
        checkpoint_anchor.signature = signing_key
            .sign(&checkpoint_anchor_signature_message(&checkpoint_anchor))
            .to_bytes();
        EvidenceViewerCheckpointEnvelopeV1 {
            version: EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1,
            checkpoint,
            checkpoint_anchor,
        }
    }

    #[test]
    fn opaque_secrets_are_bounded_ascii_and_redacted() {
        let secret =
            OpaqueEvidenceViewerSecretV1::new("runtime-token-1".to_owned()).expect("valid token");
        assert_eq!(
            format!("{secret:?}"),
            "OpaqueEvidenceViewerSecretV1(<redacted>)"
        );
        assert!(OpaqueEvidenceViewerSecretV1::new("contains whitespace".to_owned()).is_err());
        assert!(OpaqueEvidenceViewerSecretV1::new("nön-ascii".to_owned()).is_err());
        assert!(
            OpaqueEvidenceViewerSecretV1::new(
                "x".repeat(EVIDENCE_VIEWER_MAX_OPAQUE_TOKEN_BYTES_V1.saturating_add(1))
            )
            .is_err()
        );
    }

    #[test]
    fn config_enforces_fifteen_minute_ceiling_and_https_origin() {
        let key = SigningKey::from_bytes(&[0x41; 32]);
        let mut config = valid_config(key.verifying_key().to_bytes());
        config.validate().expect("valid production policy");
        config.session_ttl_ms = EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1.saturating_add(1);
        assert_eq!(config.validate(), Err(EvidenceViewerErrorV1::InvalidConfig));
        config.session_ttl_ms = EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1;
        config.webauthn_allowed_origins = vec!["http://review.example".to_owned()];
        assert_eq!(config.validate(), Err(EvidenceViewerErrorV1::InvalidConfig));
        config.webauthn_allowed_origins = vec!["https://review.example/path".to_owned()];
        assert_eq!(config.validate(), Err(EvidenceViewerErrorV1::InvalidConfig));
        config.webauthn_allowed_origins = vec![
            "https://review.example".to_owned(),
            "https://review.example".to_owned(),
        ];
        assert_eq!(config.validate(), Err(EvidenceViewerErrorV1::InvalidConfig));

        for unsafe_handle in [
            "",
            "kms:prod evidence",
            "kms:test/evidence",
            "webauthn:mock-provider",
            "kms:dev/evidence",
            "kms:fake/evidence",
            "kms:dummy/evidence",
            "kms:null/evidence",
            "kms:placeholder/evidence",
            "https://operator:secret@review.example",
            "https://review.example/provider?token=secret",
            "https://review.example/provider#fragment",
            "https://review.example/%70rovider",
            "kms\\evidence",
        ] {
            let mut config = valid_config(key.verifying_key().to_bytes());
            config.erasure_handle = unsafe_handle.to_owned();
            assert_eq!(
                config.validate(),
                Err(EvidenceViewerErrorV1::InvalidConfig),
                "unsafe runtime handle must fail closed: {unsafe_handle:?}"
            );
        }
        for unsafe_handle in ["x".repeat(257), "kms:prød/evidence".to_owned()] {
            let mut config = valid_config(key.verifying_key().to_bytes());
            config.webauthn_handle = unsafe_handle;
            assert_eq!(config.validate(), Err(EvidenceViewerErrorV1::InvalidConfig));
        }
        let mut zero_revision = valid_config(key.verifying_key().to_bytes());
        zero_revision.expected_grant_qualification =
            EvidenceViewerRuntimeProviderQualificationV1::new(0, [0xA2; 32]);
        assert_eq!(
            zero_revision.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig)
        );
        let mut zero_policy_digest = valid_config(key.verifying_key().to_bytes());
        zero_policy_digest.expected_receipt_signer_qualification =
            EvidenceViewerRuntimeProviderQualificationV1::new(1, [0; 32]);
        assert_eq!(
            zero_policy_digest.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig)
        );
        let mut zero_archive_id = valid_config(key.verifying_key().to_bytes());
        zero_archive_id.compaction_archive_id = [0; 32];
        assert_eq!(
            zero_archive_id.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig)
        );
        let mut zero_archive_key = valid_config(key.verifying_key().to_bytes());
        zero_archive_key.compaction_archive_public_key = [0; 32];
        assert_eq!(
            zero_archive_key.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig)
        );
        for interval_ms in [
            EVIDENCE_VIEWER_MIN_COMPACTION_INTERVAL_MS_V1 - 1,
            EVIDENCE_VIEWER_MAX_COMPACTION_INTERVAL_MS_V1 + 1,
        ] {
            let mut invalid = valid_config(key.verifying_key().to_bytes());
            invalid.compaction_interval_ms = interval_ms;
            assert_eq!(
                invalid.validate(),
                Err(EvidenceViewerErrorV1::InvalidConfig)
            );
        }
        let mut invalid_compaction_bound = valid_config(key.verifying_key().to_bytes());
        invalid_compaction_bound.compaction_max_records =
            EVIDENCE_VIEWER_MAX_COMPACTION_RECORDS_V1 + 1;
        assert_eq!(
            invalid_compaction_bound.validate(),
            Err(EvidenceViewerErrorV1::InvalidConfig)
        );
    }

    #[test]
    fn case_bound_webauthn_session_rotates_grants_reauthorizes_and_survives_restart() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        assert!(
            !format!("{service:?}").contains(MOCK_PROVIDER_SECRET),
            "runtime provider Debug output must remain outside service logs"
        );
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x01; 32],
            BASE_UNIX_MS,
        );
        assert_eq!(
            challenge.expires_at_unix_ms,
            BASE_UNIX_MS + fixture.config.challenge_ttl_ms
        );
        let challenge_token = challenge.challenge.expose().to_owned();
        let issued = fixture
            .create_session(
                &service,
                &challenge_token,
                b"valid-webauthn-assertion",
                [0x02; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create case-bound session");
        assert_eq!(issued.session.case_id, CASE_ID);
        assert_eq!(issued.session.round_id, ROUND_ID);
        assert_eq!(issued.session.role, EvidenceViewerRoleV1::Juror);
        assert_eq!(
            issued.session.local_session.expires_at_unix_ms,
            BASE_UNIX_MS + 1 + EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1
        );
        assert_eq!(
            issued.receipt.body.kind,
            EvidenceViewerReceiptKindV1::SessionIssued
        );
        let session_id = issued.session.local_session.session_id;
        let initial_grant = issued.grant.expose().to_owned();

        let first_manifest = service
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&initial_grant),
                [0x03; 32],
                [0xA3; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("read first manifest");
        assert_eq!(first_manifest.manifest.case_id, CASE_ID);
        assert_eq!(first_manifest.manifest.round_id, ROUND_ID);
        assert_eq!(first_manifest.manifest.quarantine_id, fixture.quarantine_id);
        assert_eq!(first_manifest.manifest.object_id, fixture.object.object_id);
        assert_eq!(
            first_manifest.manifest.evidence_digest,
            fixture.object.payload_digest
        );
        assert_eq!(
            first_manifest.manifest.payload_len,
            EVIDENCE_PAYLOAD.len() as u64
        );
        assert!(
            first_manifest
                .manifest
                .visible_watermark
                .starts_with("CONFIDENTIAL · juror · ")
        );
        assert_eq!(
            first_manifest.receipt.body.kind,
            EvidenceViewerReceiptKindV1::ManifestAccessed
        );
        let second_grant = first_manifest.rotated_grant.expose().to_owned();
        assert_ne!(initial_grant, second_grant);
        assert!(fixture.grants.was_revoked(&initial_grant));
        assert_eq!(
            service
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&initial_grant),
                    [0x04; 32],
                    [0xA4; 32],
                    BASE_UNIX_MS + 3,
                )
                .expect_err("rotated grant must be single use"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );

        drop(service);
        let restarted = fixture.open();
        let second_manifest = restarted
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&second_grant),
                [0x05; 32],
                [0xA5; 32],
                BASE_UNIX_MS + 4,
            )
            .expect("continue rotating grant after restart");
        let third_grant = second_manifest.rotated_grant.expose().to_owned();
        assert!(fixture.grants.was_revoked(&second_grant));
        let receipts = restarted.receipts(None, 16).expect("read receipt chain");
        assert_eq!(receipts.len(), 3);
        assert_receipt_chain(&receipts, &fixture.config);

        fixture
            .authorization
            .set_allowed(JUROR_ACCOUNT, EvidenceViewerRoleV1::Juror, false);
        assert_eq!(
            restarted
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&third_grant),
                    [0x06; 32],
                    [0xA6; 32],
                    BASE_UNIX_MS + 5,
                )
                .expect_err("revoked finalized assignment must fail"),
            EvidenceViewerErrorV1::Forbidden
        );
        fixture
            .authorization
            .set_allowed(JUROR_ACCOUNT, EvidenceViewerRoleV1::Juror, true);
        fixture.authorization.set_policy_digest([0x93; 32]);
        assert_eq!(
            restarted
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&third_grant),
                    [0x07; 32],
                    [0xA7; 32],
                    BASE_UNIX_MS + 6,
                )
                .expect_err("policy substitution must fail"),
            EvidenceViewerErrorV1::Forbidden
        );
        fixture.authorization.set_policy_digest([0x91; 32]);
        let third_manifest = restarted
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&third_grant),
                [0x08; 32],
                [0xA8; 32],
                BASE_UNIX_MS + 7,
            )
            .expect("failed reauthorization must not consume the active grant");
        let fourth_grant = third_manifest.rotated_grant.expose().to_owned();
        assert_eq!(
            restarted
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&fourth_grant),
                    [0x09; 32],
                    [0xA9; 32],
                    issued.session.local_session.expires_at_unix_ms,
                )
                .expect_err("exact expiry boundary must fail"),
            EvidenceViewerErrorV1::SessionInactive,
            "the exact fifteen-minute boundary must be expired"
        );
    }

    #[test]
    fn signed_transparency_projection_is_authoritative_payload_free_and_restart_stable() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x81; 32],
            BASE_UNIX_MS,
        );
        let challenge_token = challenge.challenge.expose().to_owned();
        let issued = fixture
            .create_session(
                &service,
                &challenge_token,
                b"valid-webauthn-assertion-projection",
                [0x82; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create projected session");
        let initial_grant = issued.grant.expose().to_owned();
        let (rotated_grant, _) = service
            .record_interaction(
                issued.session.local_session.session_id,
                JUROR_ACCOUNT,
                &opaque(&initial_grant),
                ModerationEvidenceViewerAccessKind::Viewed,
                Some([0x83; 32]),
                [0x84; 32],
                [0x85; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("record signed interaction");
        let rotated_grant = rotated_grant.expose().to_owned();

        let legacy = fixture.node.moderation_evidence_viewer_snapshot();
        assert!(
            legacy.sessions.is_empty() && legacy.access_events.is_empty(),
            "the production viewer must not populate the competing local registry"
        );

        let checkpoint_digest = current_checkpoint_digest(&service);
        let signer_calls_before_reads = fixture.signer.sign_call_count();
        let first_page = service
            .transparency_projection(checkpoint_digest, None, 1)
            .expect("read first signed projection page");
        assert_eq!(first_page.receipts.len(), 1);
        assert!(first_page.has_more);
        assert_eq!(
            first_page.receipts[0].body.kind,
            EvidenceViewerReceiptKindV1::SessionIssued
        );
        first_page
            .verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            )
            .expect("verify first transparency page");
        let cursor = first_page.next_cursor.expect("first exact cursor");
        let second_page = service
            .transparency_projection(checkpoint_digest, Some(cursor), 16)
            .expect("read second signed projection page");
        assert_eq!(second_page.receipts.len(), 1);
        assert!(!second_page.has_more);
        assert_eq!(
            second_page.receipts[0].body.kind,
            EvidenceViewerReceiptKindV1::InteractionRecorded
        );
        second_page
            .verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            )
            .expect("verify second transparency page");
        let full_projection = service
            .transparency_projection(checkpoint_digest, None, 16)
            .expect("read complete signed projection");
        assert_eq!(full_projection.receipts.len(), 2);
        assert_receipt_chain(&full_projection.receipts, &fixture.config);
        full_projection
            .verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            )
            .expect("verify complete transparency page");

        let mut substituted_cursor = cursor;
        substituted_cursor.receipt_digest[0] ^= 1;
        assert_eq!(
            service
                .transparency_projection(checkpoint_digest, Some(substituted_cursor), 16)
                .expect_err("same-sequence digest substitution must fail"),
            EvidenceViewerErrorV1::InvalidRequest
        );
        assert_eq!(
            fixture.signer.sign_call_count(),
            signer_calls_before_reads,
            "audit reads must return the retained signed anchor without invoking the signer"
        );

        let encoded =
            norito::to_bytes(&full_projection).expect("encode payload-free transparency page");
        for secret in [
            std::str::from_utf8(EVIDENCE_PAYLOAD).expect("ASCII evidence fixture"),
            "valid-webauthn-assertion-projection",
            challenge_token.as_str(),
            initial_grant.as_str(),
            rotated_grant.as_str(),
            JUROR_ACCOUNT,
            MOCK_PROVIDER_SECRET,
        ] {
            assert!(
                !encoded
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes()),
                "payload-free projection leaked forbidden material"
            );
        }

        drop(service);
        let restarted = fixture.open();
        assert_eq!(
            restarted
                .transparency_projection(checkpoint_digest, None, 16)
                .expect("rebuild signed projection after restart"),
            full_projection
        );
        let legacy = fixture.node.moderation_evidence_viewer_snapshot();
        assert!(legacy.sessions.is_empty() && legacy.access_events.is_empty());
    }

    #[test]
    fn transparency_projection_binds_signed_checkpoint_limit_and_freshness() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let checkpoint_digest = current_checkpoint_digest(&service);
        let signer_calls_before_reads = fixture.signer.sign_call_count();
        assert_eq!(
            service
                .transparency_projection([0; 32], None, 16)
                .expect_err("zero checkpoint expectation must fail"),
            EvidenceViewerErrorV1::InvalidRequest
        );
        for invalid_limit in [0, 1_025] {
            assert_eq!(
                service
                    .transparency_projection(checkpoint_digest, None, invalid_limit)
                    .expect_err("out-of-bounds page limit must fail"),
                EvidenceViewerErrorV1::InvalidRequest
            );
        }
        assert_eq!(
            service
                .transparency_projection(
                    checkpoint_digest,
                    Some(EvidenceViewerReceiptCursorV1 {
                        sequence: 0,
                        receipt_digest: [0xF2; 32],
                    }),
                    16,
                )
                .expect_err("zero-sequence predecessor must fail"),
            EvidenceViewerErrorV1::InvalidRequest
        );

        let empty_page = service
            .transparency_projection(checkpoint_digest, None, 16)
            .expect("read signed empty checkpoint");
        assert_eq!(empty_page.checkpoint_anchor.receipt_count, 0);
        assert_eq!(empty_page.checkpoint_anchor.chain_head, None);
        assert_eq!(empty_page.next_cursor, None);
        assert!(!empty_page.has_more);
        empty_page
            .verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            )
            .expect("verify signed empty checkpoint");

        let differently_bounded = service
            .transparency_projection(checkpoint_digest, None, 17)
            .expect("read same checkpoint with a different bound");
        assert_ne!(
            differently_bounded.projection_digest, empty_page.projection_digest,
            "the exact requested page bound must be digest-bound"
        );
        assert_eq!(
            fixture.signer.sign_call_count(),
            signer_calls_before_reads,
            "status and projection reads must not invoke the signer"
        );

        let mut tampered_limit = empty_page.clone();
        tampered_limit.page_limit = 15;
        assert_eq!(
            tampered_limit.verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            ),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint)
        );
        let mut tampered_anchor = empty_page;
        tampered_anchor.checkpoint_anchor.receipt_count = 1;
        assert_eq!(
            tampered_anchor.verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            ),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint)
        );

        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xF1; 32],
            BASE_UNIX_MS,
        );
        assert_ne!(current_checkpoint_digest(&service), checkpoint_digest);
        assert_eq!(
            service
                .transparency_projection(checkpoint_digest, None, 16)
                .expect_err("a stale checkpoint expectation must not silently repaginate"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
    }

    #[test]
    fn finalized_reauthorization_rejects_same_height_forks_and_persists_monotonic_head() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x86; 32],
            BASE_UNIX_MS,
        );
        let challenge_token = challenge.challenge.expose().to_owned();

        fixture
            .authorization
            .set_finalized_anchor(77, [0x93; 32], BASE_UNIX_MS - 1_000);
        assert_eq!(
            fixture
                .create_session(
                    &service,
                    &challenge_token,
                    b"valid-webauthn-assertion-fork",
                    [0x87; 32],
                    BASE_UNIX_MS + 1,
                )
                .expect_err("same-height challenge fork must fail before WebAuthn consumption"),
            EvidenceViewerErrorV1::Forbidden
        );
        fixture
            .authorization
            .set_finalized_anchor(77, [0x92; 32], BASE_UNIX_MS - 1_000);
        let issued = fixture
            .create_session(
                &service,
                &challenge_token,
                b"valid-webauthn-assertion-fork",
                [0x87; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("exact challenge anchor remains usable");
        let session_id = issued.session.local_session.session_id;
        let first_grant = issued.grant.expose().to_owned();

        fixture
            .authorization
            .set_finalized_anchor(78, [0x94; 32], BASE_UNIX_MS - 500);
        let advanced = service
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&first_grant),
                [0x88; 32],
                [0x89; 32],
                BASE_UNIX_MS + 3,
            )
            .expect("strictly newer finalized anchor extends the session");
        assert_eq!(advanced.manifest.finalized_height, 78);
        assert_eq!(advanced.manifest.finalized_block_hash, [0x94; 32]);
        let second_grant = advanced.rotated_grant.expose().to_owned();

        fixture
            .authorization
            .set_finalized_anchor(78, [0x95; 32], BASE_UNIX_MS - 500);
        assert_eq!(
            service
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&second_grant),
                    [0x8A; 32],
                    [0x8B; 32],
                    BASE_UNIX_MS + 4,
                )
                .expect_err("same-height session fork must fail"),
            EvidenceViewerErrorV1::Forbidden
        );
        fixture
            .authorization
            .set_finalized_anchor(77, [0x92; 32], BASE_UNIX_MS - 1_000);
        assert_eq!(
            service
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&second_grant),
                    [0x8C; 32],
                    [0x8D; 32],
                    BASE_UNIX_MS + 5,
                )
                .expect_err("persisted finalized head must reject rollback"),
            EvidenceViewerErrorV1::Forbidden
        );

        drop(service);
        let restarted = fixture.open();
        fixture
            .authorization
            .set_finalized_anchor(79, [0x96; 32], BASE_UNIX_MS);
        let after_restart = restarted
            .manifest(
                session_id,
                JUROR_ACCOUNT,
                &opaque(&second_grant),
                [0x8E; 32],
                [0x8F; 32],
                BASE_UNIX_MS + 6,
            )
            .expect("new finalized head extends the persisted cursor after restart");
        assert_eq!(after_restart.manifest.finalized_height, 79);
        assert_eq!(after_restart.manifest.finalized_block_hash, [0x96; 32]);
    }

    #[test]
    fn missing_checkpoint_capacity_fails_closed_before_service_exposure() {
        let fixture = EvidenceViewerFixture::new();
        let mut config = fixture.config.clone();
        config.checkpoint_max_bytes = 1;
        assert_eq!(
            fixture
                .open_with(config, fixture.deps.clone())
                .expect_err("the initial signed checkpoint must be durable before exposure"),
            EvidenceViewerErrorV1::ResourceExhausted
        );
    }

    #[test]
    fn provider_less_open_fails_before_checkpoint_access() {
        let fixture = EvidenceViewerFixture::new();
        assert_eq!(
            EvidenceViewerServiceV1::open(
                fixture.config.clone(),
                fixture.deps.clone(),
                fixture.node.clone(),
            )
            .expect_err("provider-less startup must fail closed"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );
        assert_eq!(fixture.checkpoint_store.load_call_count(), 0);
        assert_eq!(fixture.checkpoint_store.cas_call_count(), 0);
        assert!(!fixture.config.checkpoint_path.exists());
    }

    #[test]
    fn poisoned_quarantine_object_index_fails_closed_instead_of_masking_not_found() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let object_index = fixture.node.moderation_quarantine_objects.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = object_index
                    .write()
                    .expect("acquire quarantine object index for poison test");
                panic!("poison quarantine object index");
            })
            .join()
            .is_err()
        );
        let provider_calls = fixture.webauthn.issue_call_count();
        assert_eq!(
            service
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Juror,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0xCE; 32],
                    now_unix_ms: BASE_UNIX_MS,
                })
                .expect_err("poisoned object state must remain distinguishable from absence"),
            EvidenceViewerErrorV1::StateUnavailable
        );
        assert_eq!(
            fixture.webauthn.issue_call_count(),
            provider_calls,
            "object-state failure must precede every external challenge operation"
        );
    }

    #[test]
    fn checkpoint_genesis_and_successor_use_cas_with_mandatory_readback() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let genesis = fixture
            .checkpoint_store
            .current()
            .expect("authoritative genesis");
        assert_eq!(genesis.generation, 1);
        assert_eq!(genesis.predecessor_revision, None);
        assert_eq!(genesis.predecessor_checkpoint_digest, None);
        assert_eq!(
            genesis.checkpoint_store_handle,
            TEST_CHECKPOINT_STORE_HANDLE
        );
        assert_eq!(
            genesis.checkpoint_store_revision,
            TEST_CHECKPOINT_STORE_QUALIFICATION.revision()
        );
        assert_eq!(
            genesis.checkpoint_store_policy_digest,
            TEST_CHECKPOINT_STORE_QUALIFICATION.policy_digest()
        );
        assert_eq!(genesis.revision, checkpoint_store_record_revision(&genesis));
        let (_, genesis_anchor) =
            verify_checkpoint_store_record(&fixture.config, &service.checkpoint_store, &genesis)
                .expect("genesis record signature and canonical checkpoint");
        assert_eq!(genesis_anchor.checkpoint_generation, genesis.generation);
        assert_eq!(genesis_anchor.predecessor_checkpoint_revision, None);
        assert_eq!(genesis_anchor.predecessor_checkpoint_digest, None);
        assert_eq!(
            genesis_anchor.checkpoint_store_handle,
            TEST_CHECKPOINT_STORE_HANDLE
        );
        assert_eq!(
            genesis_anchor.checkpoint_store_revision,
            TEST_CHECKPOINT_STORE_QUALIFICATION.revision()
        );
        assert_eq!(
            genesis_anchor.checkpoint_store_policy_digest,
            TEST_CHECKPOINT_STORE_QUALIFICATION.policy_digest()
        );
        assert_eq!(fixture.checkpoint_store.cas_call_count(), 1);
        assert_eq!(fixture.checkpoint_store.load_call_count(), 4);

        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC0; 32],
            BASE_UNIX_MS,
        );
        let successor = fixture
            .checkpoint_store
            .current()
            .expect("authoritative successor");
        assert_eq!(successor.generation, 2);
        assert_eq!(successor.predecessor_revision, Some(genesis.revision));
        assert_eq!(
            successor.predecessor_checkpoint_digest,
            Some(genesis.checkpoint_digest)
        );
        let (_, successor_anchor) =
            verify_checkpoint_store_record(&fixture.config, &service.checkpoint_store, &successor)
                .expect("successor record signature and canonical checkpoint");
        assert_eq!(successor_anchor.checkpoint_generation, successor.generation);
        assert_eq!(
            successor_anchor.predecessor_checkpoint_revision,
            Some(genesis.revision)
        );
        assert_eq!(
            successor_anchor.predecessor_checkpoint_digest,
            Some(genesis.checkpoint_digest)
        );
        assert_eq!(fixture.checkpoint_store.cas_call_count(), 2);
        assert_eq!(fixture.checkpoint_store.load_call_count(), 8);
    }

    #[test]
    fn stale_replica_fails_closed_until_verified_authoritative_refresh() {
        let fixture = EvidenceViewerFixture::new();
        let mut stale_config = fixture.config.clone();
        stale_config.checkpoint_path = fixture
            .config
            .checkpoint_path
            .with_file_name("stale-replica.norito");
        let mut writer_config = fixture.config.clone();
        writer_config.checkpoint_path = fixture
            .config
            .checkpoint_path
            .with_file_name("writer-replica.norito");
        let stale_replica = fixture
            .open_with(stale_config.clone(), fixture.deps.clone())
            .expect("open stale replica");
        let writer_replica = fixture
            .open_with(writer_config.clone(), fixture.deps.clone())
            .expect("open writer replica");
        let stale_genesis =
            fs::read(&stale_config.checkpoint_path).expect("read stale replica genesis cache");
        fixture.issue_challenge(
            &writer_replica,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC5; 32],
            BASE_UNIX_MS,
        );
        assert_eq!(
            fs::read(&stale_config.checkpoint_path).expect("read untouched stale cache"),
            stale_genesis,
            "a remote writer must not mutate another replica's local cache"
        );
        assert_eq!(
            stale_replica
                .audit_status()
                .expect_err("stale replica must not serve its process-local projection"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        let writer_status = writer_replica
            .audit_status()
            .expect("writer replica remains current");
        assert_eq!(writer_status.challenge_count, 1);
        let committed_cas_calls = fixture.checkpoint_store.cas_call_count();
        let issued_challenges = fixture.webauthn.issue_call_count();
        assert_eq!(
            stale_replica
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Juror,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0xC6; 32],
                    now_unix_ms: BASE_UNIX_MS + 1,
                })
                .expect_err("stale replica must not issue a WebAuthn challenge"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        assert_eq!(
            fixture.webauthn.issue_call_count(),
            issued_challenges,
            "a fenced replica must not issue an untracked runtime challenge"
        );
        assert_eq!(
            fixture.checkpoint_store.cas_call_count(),
            committed_cas_calls,
            "a fenced replica must not attempt a checkpoint CAS"
        );

        let refreshed_anchor = stale_replica
            .refresh_authoritative_checkpoint()
            .expect("verify and install the exact successor");
        assert_eq!(refreshed_anchor, writer_status.checkpoint_anchor);
        assert_eq!(
            fs::read(&stale_config.checkpoint_path).expect("read refreshed local cache"),
            fs::read(&writer_config.checkpoint_path).expect("read authoritative writer cache"),
            "explicit refresh must install the exact authoritative record"
        );
        assert_eq!(
            stale_replica
                .audit_status()
                .expect("refreshed replica serves the authoritative projection")
                .challenge_count,
            1
        );

        fixture.issue_challenge(
            &stale_replica,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC7; 32],
            BASE_UNIX_MS + 2,
        );
        assert_eq!(
            stale_replica
                .audit_status()
                .expect("refreshed replica can acquire the next CAS generation")
                .challenge_count,
            2
        );
        assert_eq!(
            writer_replica
                .audit_status()
                .expect_err("the previous writer is fenced after handoff"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        let writer_cache =
            fs::read(&writer_config.checkpoint_path).expect("read fenced writer cache");
        fixture.issue_challenge(
            &stale_replica,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC8; 32],
            BASE_UNIX_MS + 3,
        );
        assert_eq!(
            writer_replica
                .refresh_authoritative_checkpoint()
                .expect_err("refresh must not skip a signed predecessor"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        assert_eq!(
            fs::read(&writer_config.checkpoint_path).expect("read rejected writer cache"),
            writer_cache,
            "a rejected refresh must leave the local cache unchanged"
        );
    }

    #[test]
    fn signed_archive_compaction_installs_before_prune_and_replays_idempotently() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let archive = fixture.compaction_archive.clone();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xD1; 32],
            BASE_UNIX_MS,
        );
        fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"archive-compaction-assertion",
                [0xD0; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create expiring session");
        let request = EvidenceViewerCompactionArchiveRequestV1 {
            expected_checkpoint_anchor: service
                .audit_status()
                .expect("source audit status")
                .checkpoint_anchor,
            expected_archive_head_digest: None,
            compacted_through_unix_ms: BASE_UNIX_MS + 1 + EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1,
            maximum_records: 2,
        };

        fixture
            .checkpoint_store
            .set_next_cas_mode(MockCheckpointCasMode::RejectedNoCommit);
        assert_eq!(
            service
                .compact_expired_with_archive(request.clone())
                .expect_err("checkpoint failure follows durable archive install"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        assert_eq!(
            service
                .audit_status()
                .expect("failed checkpoint commit preserves source state")
                .challenge_count,
            1
        );
        assert_eq!(
            service
                .audit_status()
                .expect("failed checkpoint commit preserves source session")
                .session_count,
            1
        );
        assert_eq!(archive.retained_artifact_count(), 1);
        assert_eq!(archive.install_call_count(), 1);

        let head = service
            .compact_expired_with_archive(request.clone())
            .expect("exact archive replay and checkpoint commit");
        head.verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        )
        .expect("signed archive head");
        assert_eq!(head.generation, 1);
        assert_eq!(head.predecessor_head_digest, None);
        assert_eq!(head.predecessor_operation_id, None);
        assert_eq!(head.challenge_count, 1);
        assert_eq!(head.session_count, 1);
        assert_eq!(archive.retained_artifact_count(), 1);
        assert_eq!(archive.install_call_count(), 2);
        assert_eq!(
            service
                .audit_status()
                .expect("compacted authoritative status")
                .challenge_count,
            0
        );
        assert_eq!(
            service
                .audit_status()
                .expect("expired session was compacted")
                .session_count,
            0
        );
        assert_eq!(
            service
                .compaction_archive_head()
                .expect("read signed archive head"),
            Some(head.clone())
        );
        let compacted_checkpoint = current_checkpoint_digest(&service);
        let projection = service
            .transparency_projection(compacted_checkpoint, None, 16)
            .expect("project exact signed compaction state");
        assert_eq!(projection.compaction_archive_head, Some(head.clone()));
        assert_eq!(
            projection.checkpoint_anchor.compaction_archive_head_digest,
            Some(head.head_digest)
        );
        projection
            .verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            )
            .expect("verify archive-bound transparency projection");
        let mut omitted_archive = projection;
        omitted_archive.compaction_archive_head = None;
        assert_eq!(
            omitted_archive.verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            ),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint)
        );

        let install_calls = archive.install_call_count();
        let read_calls = archive.read_call_count();
        assert_eq!(
            service
                .compact_expired_with_archive(request.clone())
                .expect("completed transition is an exact readback replay"),
            head
        );
        assert_eq!(archive.install_call_count(), install_calls);
        assert_eq!(archive.read_call_count(), read_calls + 1);

        let webauthn_calls = fixture.webauthn.issue_call_count();
        assert_eq!(
            service
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Juror,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0xD1; 32],
                    now_unix_ms: BASE_UNIX_MS,
                })
                .expect_err("archived challenge idempotency remains fenced"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );
        assert_eq!(
            fixture.webauthn.issue_call_count(),
            webauthn_calls,
            "archive compaction must retain a replay tombstone"
        );

        let reopened = fixture.open();
        assert_eq!(
            reopened
                .compaction_archive_head()
                .expect("reopen signed archive head"),
            Some(head)
        );
        assert_eq!(
            reopened
                .audit_status()
                .expect("reopened compacted checkpoint")
                .challenge_count,
            0
        );
        assert_eq!(
            reopened
                .audit_status()
                .expect("reopened compacted session state")
                .session_count,
            0
        );
    }

    #[test]
    fn supervised_compaction_tick_honors_the_configured_work_bound() {
        let mut fixture = EvidenceViewerFixture::new();
        fixture.config.compaction_interval_ms = EVIDENCE_VIEWER_MIN_COMPACTION_INTERVAL_MS_V1;
        fixture.config.compaction_max_records = 1;
        let service = fixture.open();
        assert_eq!(
            service.compaction_interval_ms(),
            EVIDENCE_VIEWER_MIN_COMPACTION_INTERVAL_MS_V1
        );
        for idempotency_key in [[0xE1; 32], [0xE2; 32]] {
            fixture.issue_challenge(
                &service,
                JUROR_ACCOUNT,
                EvidenceViewerRoleV1::Juror,
                idempotency_key,
                BASE_UNIX_MS,
            );
        }
        let cutoff = BASE_UNIX_MS + fixture.config.challenge_ttl_ms;
        let first = service
            .compact_expired_tick(cutoff)
            .expect("first bounded compaction tick")
            .expect("one expired record");
        assert_eq!(first.challenge_count, 1);
        assert_eq!(first.session_count, 0);
        assert_eq!(
            service
                .audit_status()
                .expect("first bounded compaction status")
                .challenge_count,
            1
        );
        let second = service
            .compact_expired_tick(cutoff)
            .expect("second bounded compaction tick")
            .expect("second expired record");
        assert_eq!(second.generation, first.generation + 1);
        assert_eq!(second.predecessor_head_digest, Some(first.head_digest));
        assert_eq!(second.predecessor_operation_id, Some(first.operation_id));
        assert_eq!(second.challenge_count, 1);
        assert_eq!(
            service
                .audit_status()
                .expect("second bounded compaction status")
                .challenge_count,
            0
        );
        assert_eq!(
            service
                .compact_expired_tick(cutoff)
                .expect("empty bounded compaction tick"),
            None
        );
    }

    #[test]
    fn restart_verifies_every_archive_generation_and_rejects_lineage_gaps() {
        fn build_two_generation_archive(
            fixture: &EvidenceViewerFixture,
        ) -> (
            EvidenceViewerServiceV1,
            EvidenceViewerSignedCompactionArchiveHeadV1,
            EvidenceViewerSignedCompactionArchiveHeadV1,
        ) {
            let service = fixture.open();
            for idempotency_key in [[0xE4; 32], [0xE5; 32]] {
                fixture.issue_challenge(
                    &service,
                    JUROR_ACCOUNT,
                    EvidenceViewerRoleV1::Juror,
                    idempotency_key,
                    BASE_UNIX_MS,
                );
            }
            let cutoff = BASE_UNIX_MS + fixture.config.challenge_ttl_ms;
            let first = service
                .compact_expired_tick(cutoff)
                .expect("first historical archive transition")
                .expect("first expired record");
            let second = service
                .compact_expired_tick(cutoff)
                .expect("second historical archive transition")
                .expect("second expired record");
            (service, first, second)
        }

        let mut missing_fixture = EvidenceViewerFixture::new();
        missing_fixture.config.compaction_max_records = 1;
        let (missing_service, first, second) = build_two_generation_archive(&missing_fixture);
        assert_eq!(second.predecessor_head_digest, Some(first.head_digest));
        assert_eq!(
            second.predecessor_operation_id,
            Some(first.operation_id),
            "the signed successor must retain a recoverable predecessor operation"
        );
        verify_compaction_archive_lineage_link(&second, &first)
            .expect("contiguous authenticated generations");
        let mut jumped = second.clone();
        jumped.generation = jumped.generation.saturating_add(1);
        assert_eq!(
            verify_compaction_archive_lineage_link(&jumped, &first),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint),
            "a signed lineage link must reject generation jumps"
        );
        drop(missing_service);
        missing_fixture
            .compaction_archive
            .remove_artifact(first.operation_id);
        assert_eq!(
            missing_fixture
                .open_with(missing_fixture.config.clone(), missing_fixture.deps.clone(),)
                .expect_err("restart must fail when an old generation is missing"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );

        let mut corrupt_fixture = EvidenceViewerFixture::new();
        corrupt_fixture.config.compaction_max_records = 1;
        let (corrupt_service, corrupt_first, _) = build_two_generation_archive(&corrupt_fixture);
        drop(corrupt_service);
        let mut corrupt_bytes = corrupt_fixture
            .compaction_archive
            .artifact(corrupt_first.operation_id);
        let last = corrupt_bytes
            .last_mut()
            .expect("non-empty historical archive artifact");
        *last ^= 1;
        corrupt_fixture
            .compaction_archive
            .replace_artifact(corrupt_first.operation_id, corrupt_bytes);
        assert_eq!(
            corrupt_fixture
                .open_with(corrupt_fixture.config.clone(), corrupt_fixture.deps.clone(),)
                .expect_err("restart must fail when an old generation is corrupt"),
            EvidenceViewerErrorV1::InvalidCheckpoint
        );
    }

    #[test]
    fn live_refresh_rejects_a_missing_historical_archive_before_adoption() {
        assert_live_refresh_rejects_historical_archive_damage(
            false,
            EvidenceViewerErrorV1::RuntimeUnavailable,
        );
    }

    #[test]
    fn live_refresh_rejects_a_corrupt_historical_archive_before_adoption() {
        assert_live_refresh_rejects_historical_archive_damage(
            true,
            EvidenceViewerErrorV1::InvalidCheckpoint,
        );
    }

    #[test]
    fn compaction_archive_decode_rejects_a_maximal_nested_length_prefix() {
        let mut fixture = EvidenceViewerFixture::new();
        fixture.config.compaction_max_records = 7;
        let sequence_limit = compaction_archive_sequence_limit(&fixture.config);
        assert_eq!(sequence_limit, 7);

        let framed = norito::core::frame_bare_with_header_flags::<Vec<ChallengeRecordV1>>(
            &u64::MAX.to_le_bytes(),
            0,
        )
        .expect("frame malicious nested sequence prefix");
        let maximum_bytes =
            usize::try_from(compaction_archive_max_bytes(&fixture.config)).expect("byte limit");
        let limits = norito::DecodeLimits::new(
            sequence_limit,
            maximum_bytes,
            sequence_limit.saturating_mul(2),
            maximum_bytes.saturating_mul(4),
            64,
        );
        let error =
            norito::decode_from_bytes_with_limits::<Vec<ChallengeRecordV1>>(&framed, limits)
                .expect_err("maximal declared record count must fail before allocation");
        assert!(matches!(
            error,
            norito::core::Error::SequenceLengthExceeded {
                length: u64::MAX,
                limit: 7
            }
        ));
    }

    #[test]
    fn archive_policy_drift_during_install_never_prunes_state() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xE3; 32],
            BASE_UNIX_MS,
        );
        fixture
            .compaction_archive
            .qualification
            .drift_policy_after_next_operation([0xF2; 32]);
        assert_eq!(
            service
                .compact_expired_tick(BASE_UNIX_MS + fixture.config.challenge_ttl_ms)
                .expect_err("archive policy drift must fail closed"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        assert_eq!(
            service
                .audit_status()
                .expect("archive drift preserves source state")
                .challenge_count,
            1
        );
    }

    #[test]
    fn archive_compaction_rejects_forgery_forks_rollback_and_trailing_data() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let archive = fixture.compaction_archive.clone();
        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xD2; 32],
            BASE_UNIX_MS,
        );
        let request = EvidenceViewerCompactionArchiveRequestV1 {
            expected_checkpoint_anchor: service
                .audit_status()
                .expect("source audit status")
                .checkpoint_anchor,
            expected_archive_head_digest: None,
            compacted_through_unix_ms: BASE_UNIX_MS + 120_000,
            maximum_records: 1,
        };
        let substituted_archive = Arc::new(MockCompactionArchive::new(
            "object-lock:prod-evidence-archive-secondary",
        ));
        let mut substituted_deps = fixture.deps.clone();
        substituted_deps.compaction_archive = substituted_archive.clone();
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), substituted_deps)
                .expect_err("substituted archive identity must fail"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        assert_eq!(substituted_archive.install_call_count(), 0);

        let checkpoint_cas_calls = fixture.checkpoint_store.cas_call_count();
        archive.append_trailing_on_next_read();
        assert_eq!(
            service
                .compact_expired_with_archive(request.clone())
                .expect_err("trailing archive data must fail closed"),
            EvidenceViewerErrorV1::InvalidCheckpoint
        );
        assert_eq!(
            fixture.checkpoint_store.cas_call_count(),
            checkpoint_cas_calls
        );
        assert_eq!(
            service
                .audit_status()
                .expect("trailing readback cannot prune")
                .challenge_count,
            1
        );

        let operation_id = archive.sole_operation_id();
        let canonical = archive.artifact(operation_id);
        let mut forged =
            decode_local_checkpoint_canonical::<EvidenceViewerCompactionArchiveArtifactV1>(
                &canonical,
                compaction_archive_max_bytes(&fixture.config),
                compaction_archive_sequence_limit(&fixture.config),
            )
            .expect("decode canonical archive fixture");
        forged.head.signature[0] ^= 1;
        archive.replace_artifact(
            operation_id,
            norito::to_bytes(&forged).expect("encode forged canonical artifact"),
        );
        assert_eq!(
            service
                .compact_expired_with_archive(request.clone())
                .expect_err("forged signed readback must fail closed"),
            EvidenceViewerErrorV1::InvalidCheckpoint
        );
        assert_eq!(
            fixture.checkpoint_store.cas_call_count(),
            checkpoint_cas_calls
        );
        assert_eq!(
            service
                .audit_status()
                .expect("forged readback cannot prune")
                .challenge_count,
            1
        );

        archive.replace_artifact(operation_id, canonical);
        let head = service
            .compact_expired_with_archive(request)
            .expect("install verified artifact after adversarial readbacks");
        let mut forged_head = head.clone();
        forged_head.signature[0] ^= 1;
        assert_eq!(
            forged_head.verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            ),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint)
        );
        archive.corrupt_signature(head.operation_id);
        assert_eq!(
            service
                .compact_expired_with_archive(EvidenceViewerCompactionArchiveRequestV1 {
                    expected_checkpoint_anchor: head.source_checkpoint_anchor.clone(),
                    expected_archive_head_digest: head.predecessor_head_digest,
                    compacted_through_unix_ms: head.compacted_through_unix_ms,
                    maximum_records: head.maximum_records,
                })
                .expect_err("forged archive receipt signature must fail closed"),
            EvidenceViewerErrorV1::InvalidCheckpoint
        );
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), fixture.deps.clone())
                .expect_err("restart must re-read and verify the current archive artifact"),
            EvidenceViewerErrorV1::InvalidCheckpoint
        );

        let mut skipped_generation = head.clone();
        skipped_generation.generation = 2;
        skipped_generation.predecessor_head_digest = None;
        skipped_generation.operation_id = [0; 32];
        skipped_generation.signature = [0; 64];
        skipped_generation.head_digest = [0; 32];
        skipped_generation.archive_signature = [0; 64];
        skipped_generation.operation_id =
            compaction_archive_operation_id(&skipped_generation).expect("operation id");
        skipped_generation.signature = fixture
            .signer
            .signing_key
            .sign(
                &compaction_archive_signature_message(&skipped_generation)
                    .expect("archive signing message"),
            )
            .to_bytes();
        skipped_generation.head_digest =
            compaction_archive_head_digest(&skipped_generation).expect("archive head digest");
        skipped_generation.archive_signature = archive
            .signing_key
            .sign(&compaction_archive_receipt_message(&skipped_generation))
            .to_bytes();
        assert_eq!(
            skipped_generation.verify(
                &fixture.config.receipt_signer_handle,
                fixture.config.receipt_signer_public_key,
            ),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint),
            "even a correctly signed skipped generation must be rejected"
        );

        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xD3; 32],
            BASE_UNIX_MS + 1,
        );
        let current_anchor = service
            .audit_status()
            .expect("post-compaction source")
            .checkpoint_anchor;
        let install_calls = archive.install_call_count();
        for substituted_predecessor in [None, Some([0xD4; 32])] {
            assert_eq!(
                service
                    .compact_expired_with_archive(EvidenceViewerCompactionArchiveRequestV1 {
                        expected_checkpoint_anchor: current_anchor.clone(),
                        expected_archive_head_digest: substituted_predecessor,
                        compacted_through_unix_ms: BASE_UNIX_MS + 120_001,
                        maximum_records: 1,
                    })
                    .expect_err("rollback/forked archive predecessor must fail"),
                EvidenceViewerErrorV1::CheckpointChanged
            );
        }
        assert_eq!(
            archive.install_call_count(),
            install_calls,
            "forked or rolled-back fences must fail before archive installation"
        );
    }

    #[test]
    fn ambiguous_checkpoint_cas_is_resolved_only_by_exact_readback() {
        let committed_fixture = EvidenceViewerFixture::new();
        let committed_service = committed_fixture.open();
        committed_fixture
            .checkpoint_store
            .set_next_cas_mode(MockCheckpointCasMode::AmbiguousCommit);
        committed_fixture.issue_challenge(
            &committed_service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC1; 32],
            BASE_UNIX_MS,
        );
        assert_eq!(
            committed_service
                .audit_status()
                .expect("exact committed readback is accepted")
                .challenge_count,
            1
        );
        assert_eq!(
            committed_fixture
                .checkpoint_store
                .current()
                .expect("committed ambiguous successor")
                .generation,
            2
        );

        let unchanged_fixture = EvidenceViewerFixture::new();
        let unchanged_service = unchanged_fixture.open();
        let predecessor = unchanged_fixture
            .checkpoint_store
            .current()
            .expect("unchanged predecessor");
        unchanged_fixture
            .checkpoint_store
            .set_next_cas_mode(MockCheckpointCasMode::AmbiguousNoCommit);
        assert_eq!(
            unchanged_service
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: unchanged_fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Juror,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0xC2; 32],
                    now_unix_ms: BASE_UNIX_MS,
                })
                .expect_err("unchanged readback must not claim a commit"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        assert_eq!(
            unchanged_fixture.checkpoint_store.current(),
            Some(predecessor)
        );
        assert_eq!(
            unchanged_service
                .audit_status()
                .expect("definite unchanged predecessor permits rollback")
                .challenge_count,
            0
        );
    }

    #[test]
    fn two_live_replicas_adopt_a_verified_authoritative_cas_successor() {
        let fixture = EvidenceViewerFixture::new();
        let writer = fixture.open();
        let mut replica_config = fixture.config.clone();
        replica_config.checkpoint_path = fixture
            .config
            .checkpoint_path
            .with_file_name("evidence-viewer-race-replica.to");
        let replica = fixture
            .open_with(replica_config, fixture.deps.clone())
            .expect("open second live replica");
        let predecessor = fixture
            .checkpoint_store
            .current()
            .expect("race predecessor");
        let competing = replica
            .sign_checkpoint_store_record(
                predecessor.checkpoint_digest,
                predecessor.checkpoint_bytes.clone(),
                Some(&predecessor),
            )
            .expect("sign competing valid successor");
        fixture
            .checkpoint_store
            .set_next_cas_mode(MockCheckpointCasMode::RaceWith(Box::new(competing.clone())));

        assert_eq!(
            writer
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Juror,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0xC3; 32],
                    now_unix_ms: BASE_UNIX_MS,
                })
                .expect_err("competing authoritative successor must fail the local writer"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        assert_eq!(
            writer
                .audit_status()
                .expect("writer adopts the authenticated competing successor")
                .challenge_count,
            0
        );
        assert_eq!(
            writer
                .state
                .lock()
                .expect("writer state lock")
                .checkpoint_record
                .as_ref(),
            Some(&competing)
        );
        assert_eq!(
            read_local_checkpoint_store_record(&fixture.config, &writer.checkpoint_store)
                .expect("read automatically refreshed writer cache"),
            Some(competing.clone())
        );
        assert_eq!(
            replica
                .audit_status()
                .expect_err("the other live replica is now fenced"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        replica
            .refresh_authoritative_checkpoint()
            .expect("second replica adopts the exact direct successor");
        assert_eq!(
            replica
                .audit_status()
                .expect("both live replicas remain usable")
                .challenge_count,
            0
        );
    }

    #[test]
    fn checkpoint_race_with_a_generation_jump_remains_poisoned() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let predecessor = fixture
            .checkpoint_store
            .current()
            .expect("race predecessor");
        let mut jumped = service
            .sign_checkpoint_store_record(
                predecessor.checkpoint_digest,
                predecessor.checkpoint_bytes.clone(),
                Some(&predecessor),
            )
            .expect("sign candidate successor");
        jumped.generation = jumped.generation.saturating_add(1);
        jumped.signature = fixture
            .signer
            .signing_key
            .sign(&checkpoint_store_record_signature_message(&jumped))
            .to_bytes();
        jumped.revision = checkpoint_store_record_revision(&jumped);
        verify_checkpoint_store_record(&fixture.config, &service.checkpoint_store, &jumped)
            .expect("the isolated record is signed but its predecessor step is unverifiable");
        fixture
            .checkpoint_store
            .set_next_cas_mode(MockCheckpointCasMode::RaceWith(Box::new(jumped)));

        assert_eq!(
            service
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Juror,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0xCC; 32],
                    now_unix_ms: BASE_UNIX_MS,
                })
                .expect_err("generation-jump race must be treated as unverifiable"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );
        assert_eq!(
            service
                .audit_status()
                .expect_err("unverifiable race must poison process-local state"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );
    }

    #[test]
    fn restart_accepts_only_exact_or_single_predecessor_local_cache() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let genesis = fixture.checkpoint_store.current().expect("cache genesis");
        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC4; 32],
            BASE_UNIX_MS,
        );
        let authoritative = fixture.checkpoint_store.current().expect("cache successor");
        write_local_checkpoint_store_record(&fixture.config, &genesis)
            .expect("install one-generation-behind cache");
        drop(service);

        let restarted = fixture.open();
        assert_eq!(
            restarted
                .audit_status()
                .expect("single predecessor cache is repaired")
                .challenge_count,
            1
        );
        assert_eq!(
            read_local_checkpoint_store_record(&fixture.config, &restarted.checkpoint_store)
                .expect("read repaired cache"),
            Some(authoritative.clone())
        );
        drop(restarted);

        std::fs::remove_file(&fixture.config.checkpoint_path).expect("remove local cache");
        let cacheless_restart = fixture.open();
        assert_eq!(
            cacheless_restart
                .state
                .lock()
                .expect("cacheless restart state lock")
                .checkpoint_record
                .as_ref(),
            Some(&authoritative)
        );
    }

    #[test]
    fn restart_rejects_a_local_cache_more_than_one_generation_behind() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let genesis = fixture.checkpoint_store.current().expect("stale genesis");
        fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xC6; 32],
            BASE_UNIX_MS,
        );
        fixture.issue_challenge(
            &service,
            LEGAL_ACCOUNT,
            EvidenceViewerRoleV1::Legal,
            [0xC7; 32],
            BASE_UNIX_MS + 1,
        );
        assert_eq!(
            fixture
                .checkpoint_store
                .current()
                .expect("two-generation successor")
                .generation,
            3
        );
        write_local_checkpoint_store_record(&fixture.config, &genesis)
            .expect("install cache two generations behind");
        drop(service);
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), fixture.deps.clone())
                .expect_err("multi-generation stale cache must fail closed"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
    }

    #[test]
    fn tampered_authoritative_checkpoint_records_fail_before_local_cache_use() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        drop(service);
        let original = fixture
            .checkpoint_store
            .current()
            .expect("checkpoint record to tamper");
        let mut signature_tamper = original.clone();
        signature_tamper.signature[0] ^= 1;
        signature_tamper.revision = checkpoint_store_record_revision(&signature_tamper);
        let mut revision_tamper = original.clone();
        revision_tamper.revision[0] ^= 1;
        let mut checkpoint_tamper = original;
        checkpoint_tamper.checkpoint_bytes[0] ^= 1;
        checkpoint_tamper.revision = checkpoint_store_record_revision(&checkpoint_tamper);
        for tampered in [signature_tamper, revision_tamper, checkpoint_tamper] {
            fixture.checkpoint_store.replace_latest(Some(tampered));
            assert_eq!(
                fixture
                    .open_with(fixture.config.clone(), fixture.deps.clone())
                    .expect_err("forged authoritative record must fail startup"),
                EvidenceViewerErrorV1::InvalidCheckpoint
            );
        }
    }

    #[test]
    fn checkpoint_store_substitution_and_staleness_fail_before_cache_access() {
        let substituted_fixture = EvidenceViewerFixture::new();
        let substituted = Arc::new(MockCheckpointStore::new(
            "sealed:prod-unexpected-evidence-checkpoints",
        ));
        assert_eq!(
            EvidenceViewerServiceV1::open_with_checkpoint_store(
                substituted_fixture.config.clone(),
                substituted_fixture.deps.clone(),
                substituted_fixture.node.clone(),
                TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
                TEST_CHECKPOINT_STORE_QUALIFICATION,
                substituted.clone(),
            )
            .expect_err("substituted checkpoint store must fail startup"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        assert_eq!(substituted.load_call_count(), 0);
        assert!(!substituted_fixture.config.checkpoint_path.exists());

        let test_marked_fixture = EvidenceViewerFixture::new();
        let test_marked = Arc::new(MockCheckpointStore::new("sealed:test-evidence-checkpoints"));
        assert_eq!(
            EvidenceViewerServiceV1::open_with_checkpoint_store(
                test_marked_fixture.config.clone(),
                test_marked_fixture.deps.clone(),
                test_marked_fixture.node.clone(),
                "sealed:test-evidence-checkpoints".to_owned(),
                TEST_CHECKPOINT_STORE_QUALIFICATION,
                test_marked.clone(),
            )
            .expect_err("test-marked checkpoint store must fail startup"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        assert_eq!(test_marked.load_call_count(), 0);
        assert!(!test_marked_fixture.config.checkpoint_path.exists());

        let stale_fixture = EvidenceViewerFixture::new();
        let stale = Arc::new(MockCheckpointStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        stale.qualification.set_failure(Some(
            EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable,
        ));
        assert_eq!(
            EvidenceViewerServiceV1::open_with_checkpoint_store(
                stale_fixture.config.clone(),
                stale_fixture.deps.clone(),
                stale_fixture.node.clone(),
                TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
                TEST_CHECKPOINT_STORE_QUALIFICATION,
                stale.clone(),
            )
            .expect_err("stale checkpoint store must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        assert_eq!(stale.load_call_count(), 0);
        assert!(!stale_fixture.config.checkpoint_path.exists());
    }

    #[test]
    fn rejected_checkpoint_cas_rolls_back_receipt_grant_and_anchor() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xA0; 32],
            BASE_UNIX_MS,
        );
        let issued = fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"valid-webauthn-assertion-checkpoint",
                [0xA1; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create checkpoint rollback session");
        let initial_grant = issued.grant.expose().to_owned();
        let checkpoint_digest = current_checkpoint_digest(&service);
        let before = service
            .transparency_projection(checkpoint_digest, None, 16)
            .expect("projection before rejected CAS");
        fixture
            .checkpoint_store
            .set_next_cas_mode(MockCheckpointCasMode::RejectedNoCommit);

        fixture
            .authorization
            .set_finalized_anchor(78, [0xA2; 32], BASE_UNIX_MS);
        assert_eq!(
            service
                .manifest(
                    issued.session.local_session.session_id,
                    JUROR_ACCOUNT,
                    &opaque(&initial_grant),
                    [0xA3; 32],
                    [0xA4; 32],
                    BASE_UNIX_MS + 2,
                )
                .expect_err("rejected checkpoint CAS must suppress access response"),
            EvidenceViewerErrorV1::CheckpointChanged
        );
        assert_eq!(
            service
                .transparency_projection(checkpoint_digest, None, 16)
                .expect("projection rolls back after precommit failure"),
            before
        );
        let rolled_back_session = service
            .state
            .lock()
            .expect("service state lock")
            .sessions
            .get(&issued.session.local_session.session_id)
            .cloned()
            .expect("rolled-back session");
        assert_eq!(rolled_back_session.finalized_height, 77);
        assert_eq!(rolled_back_session.finalized_block_hash, [0x92; 32]);
        assert_eq!(rolled_back_session.grant_generation, 1);
        let issued_tokens = fixture.grants.issued_tokens();
        let replacement = issued_tokens
            .last()
            .expect("replacement grant was attempted");
        assert!(fixture.grants.was_revoked(replacement));
        assert!(!fixture.grants.was_revoked(&initial_grant));

        drop(service);
        let restarted = fixture.open();
        assert_eq!(
            restarted
                .transparency_projection(checkpoint_digest, None, 16)
                .expect("restart from pre-access checkpoint"),
            before
        );
        fixture
            .authorization
            .set_finalized_anchor(78, [0xA2; 32], BASE_UNIX_MS);
        restarted
            .manifest(
                issued.session.local_session.session_id,
                JUROR_ACCOUNT,
                &opaque(&initial_grant),
                [0xA3; 32],
                [0xA4; 32],
                BASE_UNIX_MS + 3,
            )
            .expect("original grant remains usable after rollback and restart");
    }

    #[test]
    fn challenge_assertion_role_account_and_purpose_substitution_are_denied() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x11; 32],
            BASE_UNIX_MS,
        );
        let challenge_token = challenge.challenge.expose().to_owned();
        let substituted = service.create_session(EvidenceViewerSessionRequestV1 {
            case_id: CASE_ID.to_owned(),
            round_id: ROUND_ID.to_owned(),
            quarantine_id: fixture.quarantine_id,
            viewer_account: JUROR_ACCOUNT.to_owned(),
            role: EvidenceViewerRoleV1::Juror,
            purpose: "substituted purpose".to_owned(),
            challenge: opaque(&challenge_token),
            webauthn_assertion: b"valid-webauthn-assertion".to_vec(),
            idempotency_key: [0x12; 32],
            now_unix_ms: BASE_UNIX_MS + 1,
        });
        assert_eq!(
            substituted.expect_err("purpose substitution must fail"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );

        let issued = fixture
            .create_session(
                &service,
                &challenge_token,
                b"valid-webauthn-assertion",
                [0x13; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("unmodified challenge remains usable");
        assert_eq!(
            fixture
                .create_session(
                    &service,
                    &challenge_token,
                    b"different-assertion",
                    [0x14; 32],
                    BASE_UNIX_MS + 3,
                )
                .expect_err("consumed challenge must not replay"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );

        let second_challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x15; 32],
            BASE_UNIX_MS + 4,
        );
        assert_eq!(
            fixture
                .create_session(
                    &service,
                    second_challenge.challenge.expose(),
                    b"valid-webauthn-assertion",
                    [0x16; 32],
                    BASE_UNIX_MS + 5,
                )
                .expect_err("assertion bytes must not be reused across challenges"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );
        let stale_challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x19; 32],
            BASE_UNIX_MS + 6,
        );
        assert_eq!(
            fixture
                .create_session(
                    &service,
                    stale_challenge.challenge.expose(),
                    b"valid-webauthn-assertion-stale",
                    [0x1A; 32],
                    stale_challenge.expires_at_unix_ms,
                )
                .expect_err("challenge must expire at its exact boundary"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );
        assert_eq!(
            service
                .manifest(
                    issued.session.local_session.session_id,
                    LEGAL_ACCOUNT,
                    &opaque(issued.grant.expose()),
                    [0x17; 32],
                    [0xB7; 32],
                    stale_challenge.expires_at_unix_ms + 1,
                )
                .expect_err("account substitution must fail"),
            EvidenceViewerErrorV1::SessionInactive
        );
        assert_eq!(
            service
                .issue_challenge(EvidenceViewerChallengeRequestV1 {
                    case_id: CASE_ID.to_owned(),
                    round_id: ROUND_ID.to_owned(),
                    quarantine_id: fixture.quarantine_id,
                    viewer_account: JUROR_ACCOUNT.to_owned(),
                    role: EvidenceViewerRoleV1::Legal,
                    purpose: REVIEW_PURPOSE.to_owned(),
                    idempotency_key: [0x18; 32],
                    now_unix_ms: stale_challenge.expires_at_unix_ms + 2,
                })
                .expect_err("role substitution must fail"),
            EvidenceViewerErrorV1::Forbidden
        );
    }

    #[test]
    fn legal_hold_precedes_retention_and_erasure_then_release_allows_exactly_once_commit() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x21; 32],
            BASE_UNIX_MS,
        );
        let issued = fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"valid-webauthn-assertion",
                [0x22; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create session protected by legal hold");
        let session_id = issued.session.local_session.session_id;
        let active_grant = issued.grant.expose().to_owned();

        let (hold, placement) = service
            .place_legal_hold(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                [0x23; 32],
                [0x24; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("place legal hold");
        assert_eq!(
            placement.body.kind,
            EvidenceViewerReceiptKindV1::LegalHoldPlaced
        );
        let retain_until = issued.session.local_session.expires_at_unix_ms
            + fixture.config.retention_after_expiry_ms;
        let (retention, _) = service
            .record_retention(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                retain_until,
                [0x25; 32],
                [0xB5; 32],
                BASE_UNIX_MS + 3,
            )
            .expect("record legal-hold-aware retention");
        assert!(retention.legal_hold_precedence);
        assert!(
            service
                .retention_due(retain_until, 16)
                .expect("evaluate retention under hold")
                .is_empty()
        );

        assert_eq!(
            service
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x26; 32],
                    [0xB6; 32],
                    BASE_UNIX_MS + 4,
                )
                .expect_err("active legal hold must prevent erasure"),
            EvidenceViewerErrorV1::LegalHoldPrecedence
        );
        assert_eq!(fixture.erasure.call_count(), 0);
        let denial_receipts = service.receipts(None, 16).expect("read denial receipt");
        assert_eq!(
            denial_receipts.last().map(|receipt| receipt.body.kind),
            Some(EvidenceViewerReceiptKindV1::ErasureDeniedLegalHold)
        );

        let (released, _) = service
            .release_legal_hold(
                CASE_ID,
                ROUND_ID,
                hold.hold_id,
                LEGAL_ACCOUNT,
                [0x27; 32],
                [0xB7; 32],
                BASE_UNIX_MS + 5,
            )
            .expect("release legal hold");
        assert_eq!(released.released_at_unix_ms, Some(BASE_UNIX_MS + 5));
        assert_eq!(
            service
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x28; 32],
                    [0xB8; 32],
                    BASE_UNIX_MS + 6,
                )
                .expect_err("retention deadline must prevent early erasure"),
            EvidenceViewerErrorV1::RetentionActive
        );
        assert_eq!(fixture.erasure.call_count(), 0);
        assert_eq!(
            service
                .retention_due(retain_until, 16)
                .expect("evaluate retention after release"),
            vec![fixture.quarantine_id]
        );
        let (erasure, receipt) = service
            .erase(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                [0x28; 32],
                [0xB8; 32],
                retain_until,
            )
            .expect("erase at exact retention boundary");
        assert_eq!(fixture.erasure.call_count(), 1);
        assert_eq!(
            receipt.body.kind,
            EvidenceViewerReceiptKindV1::ErasureCompleted
        );
        assert_eq!(erasure.receipt_digest, receipt.receipt_digest);
        assert_eq!(
            service
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x28; 32],
                    [0xB8; 32],
                    retain_until + 1,
                )
                .expect_err("completed erasure idempotency key must not replay"),
            EvidenceViewerErrorV1::AuthenticationRejected
        );
        assert_eq!(fixture.erasure.call_count(), 1);
        assert!(
            service
                .state
                .lock()
                .expect("service state lock")
                .sessions
                .get(&session_id)
                .expect("persisted session")
                .revoked
        );
        assert_eq!(
            service
                .manifest(
                    session_id,
                    JUROR_ACCOUNT,
                    &opaque(&active_grant),
                    [0x2A; 32],
                    [0xBA; 32],
                    retain_until + 2,
                )
                .expect_err("erasure must revoke active sessions"),
            EvidenceViewerErrorV1::SessionInactive
        );

        drop(service);
        let restarted = fixture.open();
        let status = restarted.audit_status().expect("restart audit projection");
        assert_eq!(status.session_count, 1);
        assert_eq!(status.receipt_count, 6);
        assert_eq!(status.active_legal_hold_count, 0);
        assert_eq!(status.retention_count, 1);
        assert_eq!(status.erasure_count, 1);
        let receipts = restarted.receipts(None, 16).expect("restart receipt chain");
        assert_receipt_chain(&receipts, &fixture.config);
        assert_eq!(
            receipts
                .iter()
                .map(|receipt| receipt.body.kind)
                .collect::<Vec<_>>(),
            vec![
                EvidenceViewerReceiptKindV1::SessionIssued,
                EvidenceViewerReceiptKindV1::LegalHoldPlaced,
                EvidenceViewerReceiptKindV1::RetentionEvaluated,
                EvidenceViewerReceiptKindV1::ErasureDeniedLegalHold,
                EvidenceViewerReceiptKindV1::LegalHoldReleased,
                EvidenceViewerReceiptKindV1::ErasureCompleted,
            ]
        );
    }

    #[test]
    fn default_retention_uses_latest_session_expiry_and_never_erases_without_a_deadline() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        assert_eq!(
            service
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x61; 32],
                    [0xD1; 32],
                    BASE_UNIX_MS,
                )
                .expect_err("object without a retention basis must not be erased"),
            EvidenceViewerErrorV1::RetentionActive
        );
        assert_eq!(fixture.erasure.call_count(), 0);

        let first_challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x62; 32],
            BASE_UNIX_MS + 1,
        );
        let first = fixture
            .create_session(
                &service,
                first_challenge.challenge.expose(),
                b"valid-webauthn-assertion-first",
                [0x63; 32],
                BASE_UNIX_MS + 2,
            )
            .expect("create first retained session");
        let second_challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x64; 32],
            BASE_UNIX_MS + 10,
        );
        let second = fixture
            .create_session(
                &service,
                second_challenge.challenge.expose(),
                b"valid-webauthn-assertion-second",
                [0x65; 32],
                BASE_UNIX_MS + 11,
            )
            .expect("create later retained session");
        let first_deadline = first.session.local_session.expires_at_unix_ms
            + fixture.config.retention_after_expiry_ms;
        let latest_deadline = second.session.local_session.expires_at_unix_ms
            + fixture.config.retention_after_expiry_ms;
        assert!(latest_deadline > first_deadline);
        assert!(
            service
                .retention_due(first_deadline, 16)
                .expect("evaluate first-session deadline")
                .is_empty(),
            "the later session must extend default retention"
        );
        assert_eq!(
            service
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x66; 32],
                    [0xD6; 32],
                    first_deadline,
                )
                .expect_err("latest session must extend erasure deadline"),
            EvidenceViewerErrorV1::RetentionActive
        );
        assert_eq!(fixture.erasure.call_count(), 0);
        assert_eq!(
            service
                .retention_due(latest_deadline, 16)
                .expect("evaluate exact latest deadline"),
            vec![fixture.quarantine_id]
        );
        service
            .erase(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                [0x67; 32],
                [0xD7; 32],
                latest_deadline,
            )
            .expect("erase at exact latest-session retention boundary");
        assert_eq!(fixture.erasure.call_count(), 1);
    }

    #[test]
    fn session_expiry_compaction_preserves_default_retention_until_erasure() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x68; 32],
            BASE_UNIX_MS,
        );
        let issued = fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"retention-floor-compaction-assertion",
                [0x69; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create session with a default retention floor");
        let session_expiry = issued.session.local_session.expires_at_unix_ms;
        let retention_deadline = session_expiry + fixture.config.retention_after_expiry_ms;
        let head = service
            .compact_expired_tick(session_expiry)
            .expect("archive expired session")
            .expect("expired session is eligible");
        assert_eq!(head.session_count, 1);
        {
            let state = service.state.lock().expect("compacted state lock");
            assert!(state.sessions.is_empty());
            assert_eq!(
                state
                    .default_retention_floors
                    .get(&fixture.quarantine_id)
                    .map(|floor| floor.retain_until_unix_ms),
                Some(retention_deadline)
            );
        }
        drop(service);

        let reopened = fixture.open();
        assert_eq!(
            reopened
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x6A; 32],
                    [0xDA; 32],
                    retention_deadline - 1,
                )
                .expect_err("compaction must not erase the default retention floor"),
            EvidenceViewerErrorV1::RetentionActive
        );
        assert_eq!(fixture.erasure.call_count(), 0);
        assert_eq!(
            reopened
                .retention_due(retention_deadline, 16)
                .expect("compacted object becomes due at the retained boundary"),
            vec![fixture.quarantine_id]
        );
        reopened
            .erase(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                [0x6B; 32],
                [0xDB; 32],
                retention_deadline,
            )
            .expect("erase at the retained boundary");
        assert_eq!(fixture.erasure.call_count(), 1);
        assert!(
            reopened
                .state
                .lock()
                .expect("post-erasure state lock")
                .default_retention_floors
                .get(&fixture.quarantine_id)
                .is_none(),
            "a completed erasure removes the no-longer-needed retention floor"
        );
    }

    #[test]
    fn erasure_before_compaction_does_not_recreate_a_retention_floor() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x6C; 32],
            BASE_UNIX_MS,
        );
        let issued = fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"erasure-before-compaction-assertion",
                [0x6D; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("create session before erasure");
        let retention_deadline = issued.session.local_session.expires_at_unix_ms
            + fixture.config.retention_after_expiry_ms;
        service
            .erase(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                [0x6E; 32],
                [0xDE; 32],
                retention_deadline,
            )
            .expect("erase before expired session compaction");
        let source_generation = fixture
            .checkpoint_store
            .current()
            .expect("post-erasure checkpoint")
            .generation;

        let head = service
            .compact_expired_tick(retention_deadline)
            .expect("compaction after terminal erasure")
            .expect("erased session remains eligible for archival");
        assert_eq!(head.session_count, 1);
        assert_eq!(fixture.compaction_archive.retained_artifact_count(), 1);
        assert_eq!(
            fixture
                .checkpoint_store
                .current()
                .expect("committable post-compaction checkpoint")
                .generation,
            source_generation + 1,
            "the installed archive must have a matching committed checkpoint"
        );
        {
            let state = service.state.lock().expect("post-compaction state lock");
            assert!(state.sessions.is_empty());
            assert!(state.erasures.contains_key(&fixture.quarantine_id));
            assert!(
                !state
                    .default_retention_floors
                    .contains_key(&fixture.quarantine_id),
                "terminal erasure must suppress compacted default-retention state"
            );
        }
        drop(service);

        let reopened = fixture.open();
        assert_eq!(
            reopened
                .audit_status()
                .expect("restart from erasure-compaction checkpoint")
                .erasure_count,
            1
        );
        assert!(
            reopened
                .state
                .lock()
                .expect("reopened erasure-compaction state")
                .default_retention_floors
                .is_empty()
        );
    }

    #[test]
    fn ambiguous_erasure_commit_recovers_from_durable_intent_without_repeating_commit() {
        let fixture = EvidenceViewerFixture::new();
        let service = fixture.open();
        let retain_until = BASE_UNIX_MS + 10;
        service
            .record_retention(
                CASE_ID,
                ROUND_ID,
                fixture.quarantine_id,
                LEGAL_ACCOUNT,
                retain_until,
                [0x71; 32],
                [0x72; 32],
                BASE_UNIX_MS,
            )
            .expect("record erasure recovery retention");

        fixture.erasure.commit_then_unavailable_once();
        assert_eq!(
            service
                .erase(
                    CASE_ID,
                    ROUND_ID,
                    fixture.quarantine_id,
                    LEGAL_ACCOUNT,
                    [0x73; 32],
                    [0x74; 32],
                    retain_until,
                )
                .expect_err("ambiguous external result must fail closed"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        assert_eq!(fixture.erasure.call_count(), 1);
        assert_eq!(fixture.erasure.commit_count(), 1);
        assert_eq!(
            service
                .audit_status()
                .expect_err("uncertain in-process state must remain unavailable"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );

        drop(service);
        let recovered = fixture.open();
        let status = recovered
            .audit_status()
            .expect("durable intent must reconcile on restart");
        assert_eq!(fixture.erasure.call_count(), 2);
        assert_eq!(
            fixture.erasure.commit_count(),
            1,
            "stable operation id must suppress a second irreversible commit"
        );
        assert_eq!(status.erasure_count, 1);
        assert_eq!(status.retention_count, 1);
        assert_eq!(status.receipt_count, 2);
        drop(recovered);

        let stable_restart = fixture.open();
        assert_eq!(
            stable_restart
                .audit_status()
                .expect("terminal erasure survives restart")
                .erasure_count,
            1
        );
        assert_eq!(fixture.erasure.call_count(), 2);
        assert_eq!(fixture.erasure.commit_count(), 1);
    }

    #[test]
    fn live_refresh_reconciliation_failures_poison_the_adopted_state() {
        for (label, injected_result, expected_commits) in [
            (
                "unavailable",
                MockErasureInjectedResult::Unavailable,
                0_usize,
            ),
            (
                "zero-digest",
                MockErasureInjectedResult::ZeroDigest,
                0_usize,
            ),
            (
                "ambiguous-commit",
                MockErasureInjectedResult::CommitThenUnavailable,
                1_usize,
            ),
        ] {
            let fixture = EvidenceViewerFixture::new();
            let writer = fixture.open();
            let mut stale_config = fixture.config.clone();
            stale_config.checkpoint_path = fixture
                .config
                .checkpoint_path
                .with_file_name(format!("erasure-reconcile-{label}.to"));
            let stale = fixture
                .open_with(stale_config, fixture.deps.clone())
                .expect("open pre-intent replica");
            let intent = test_erasure_intent(
                fixture.quarantine_id,
                fixture.object.object_id,
                fixture.object.payload_digest,
                [0x81; 32],
                [0x82; 32],
                BASE_UNIX_MS,
            );
            persist_test_erasure_intents(&writer, [intent]);
            fixture.erasure.inject_results(&[injected_result]);

            assert_eq!(
                stale
                    .refresh_authoritative_checkpoint()
                    .expect_err("failed reconciliation must reject refreshed service state"),
                EvidenceViewerErrorV1::RuntimeUnavailable,
                "{label}"
            );
            assert_eq!(fixture.erasure.commit_count(), expected_commits, "{label}");
            assert!(
                stale
                    .state
                    .lock()
                    .expect("failed reconciliation state lock")
                    .durability_uncertain,
                "{label}"
            );
            assert_eq!(
                stale
                    .audit_status()
                    .expect_err("subsequent reads must fail after uncertain reconciliation"),
                EvidenceViewerErrorV1::CheckpointUnavailable,
                "{label}"
            );
            assert_eq!(
                stale
                    .retention_due(BASE_UNIX_MS, 1)
                    .expect_err("subsequent projections must fail after reconciliation"),
                EvidenceViewerErrorV1::CheckpointUnavailable,
                "{label}"
            );
        }
    }

    #[test]
    fn partial_multi_intent_live_refresh_reconciliation_remains_poisoned() {
        let fixture = EvidenceViewerFixture::new();
        let writer = fixture.open();
        let mut stale_config = fixture.config.clone();
        stale_config.checkpoint_path = fixture
            .config
            .checkpoint_path
            .with_file_name("erasure-reconcile-partial.to");
        let stale = fixture
            .open_with(stale_config, fixture.deps.clone())
            .expect("open pre-intent replica");
        let first = test_erasure_intent(
            [0x01; 16],
            [0x11; 16],
            [0x21; 32],
            [0x31; 32],
            [0x41; 32],
            BASE_UNIX_MS,
        );
        let second = test_erasure_intent(
            [0xF1; 16],
            [0xF2; 16],
            [0xF3; 32],
            [0xF4; 32],
            [0xF5; 32],
            BASE_UNIX_MS + 1,
        );
        persist_test_erasure_intents(&writer, [first, second]);
        fixture.erasure.inject_results(&[
            MockErasureInjectedResult::Pass,
            MockErasureInjectedResult::Unavailable,
        ]);

        assert_eq!(
            stale
                .refresh_authoritative_checkpoint()
                .expect_err("second intent failure must reject a partially reconciled refresh"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        assert_eq!(fixture.erasure.call_count(), 2);
        assert_eq!(fixture.erasure.commit_count(), 1);
        {
            let state = stale
                .state
                .lock()
                .expect("partial reconciliation state lock");
            assert!(state.durability_uncertain);
            assert_eq!(state.erasures.len(), 1);
            assert_eq!(state.erasure_intents.len(), 1);
            assert_eq!(state.receipts.len(), 1);
            assert_eq!(state.idempotency.len(), 1);
        }
        let (_, authoritative_checkpoint, _) = stale
            .load_authoritative_checkpoint()
            .expect("read authoritative pending-intent checkpoint")
            .expect("authoritative pending-intent checkpoint");
        assert_eq!(authoritative_checkpoint.erasure_intents.len(), 2);
        assert!(authoritative_checkpoint.erasures.is_empty());
        assert_eq!(
            stale
                .audit_status()
                .expect_err("partial reconciliation must poison later service reads"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );
        assert_eq!(
            stale
                .receipts(None, 1)
                .expect_err("partial reconciliation must poison later receipt reads"),
            EvidenceViewerErrorV1::CheckpointUnavailable
        );
    }

    #[test]
    fn provider_identity_and_signature_drift_fail_closed_across_restart() {
        let fixture = EvidenceViewerFixture::new();
        let valid_service = fixture.open();
        fixture.issue_challenge(
            &valid_service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x31; 32],
            BASE_UNIX_MS,
        );
        drop(valid_service);

        let drifted_webauthn = EvidenceViewerRuntimeDepsV1 {
            webauthn: Arc::new(MockWebAuthn::new("webauthn:unexpected-provider")),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), drifted_webauthn)
                .expect_err("WebAuthn identity drift must fail closed"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        let drifted_grants = EvidenceViewerRuntimeDepsV1 {
            grants: Arc::new(MockGrantBoundary::new("kms:unexpected-grants")),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), drifted_grants)
                .expect_err("grant identity drift must fail closed"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        let drifted_erasure = EvidenceViewerRuntimeDepsV1 {
            erasure: Arc::new(MockErasureBoundary::new("kms:unexpected-erasure")),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), drifted_erasure)
                .expect_err("erasure identity drift must fail closed"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        let wrong_signer = Arc::new(MockReceiptSigner::new(
            "pkcs11:unexpected-receipts",
            SigningKey::from_bytes(&[0x52; 32]),
        ));
        let drifted_signer = EvidenceViewerRuntimeDepsV1 {
            receipt_signer: wrong_signer,
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), drifted_signer)
                .expect_err("receipt signer identity drift must fail closed"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        let substituted_archive_id = EvidenceViewerRuntimeDepsV1 {
            compaction_archive: Arc::new(MockCompactionArchive::with_identity(
                TEST_COMPACTION_ARCHIVE_HANDLE,
                [0xF1; 32],
                TEST_COMPACTION_ARCHIVE_SIGNING_SEED,
            )),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), substituted_archive_id)
                .expect_err("archive namespace substitution must fail closed"),
            EvidenceViewerErrorV1::InvalidConfig
        );
        let substituted_archive_key = EvidenceViewerRuntimeDepsV1 {
            compaction_archive: Arc::new(MockCompactionArchive::with_identity(
                TEST_COMPACTION_ARCHIVE_HANDLE,
                TEST_COMPACTION_ARCHIVE_ID,
                [0x53; 32],
            )),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), substituted_archive_key)
                .expect_err("archive verification-key substitution must fail closed"),
            EvidenceViewerErrorV1::InvalidConfig
        );

        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0x32; 32],
            BASE_UNIX_MS + 1,
        );
        fixture.signer.set_corrupt_signatures(true);
        assert_eq!(
            fixture
                .create_session(
                    &service,
                    challenge.challenge.expose(),
                    b"valid-webauthn-assertion",
                    [0x33; 32],
                    BASE_UNIX_MS + 2,
                )
                .expect_err("invalid receipt signature must abort the session"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        let issued_tokens = fixture.grants.issued_tokens();
        assert_eq!(issued_tokens.len(), 1);
        assert!(fixture.grants.was_revoked(&issued_tokens[0]));
        let status = service
            .audit_status()
            .expect("state rolled back after signer drift");
        assert_eq!(status.session_count, 0);
        assert_eq!(status.receipt_count, 0);
    }

    #[test]
    fn provider_qualification_fails_before_checkpoint_access() {
        let fixture = EvidenceViewerFixture::new();
        assert!(!fixture.config.checkpoint_path.exists());

        let test_marked_webauthn = Arc::new(MockWebAuthn::new("webauthn:test-evidence-viewer"));
        let test_marked_deps = EvidenceViewerRuntimeDepsV1 {
            webauthn: test_marked_webauthn,
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), test_marked_deps)
                .expect_err("test-marked WebAuthn provider must fail startup"),
            EvidenceViewerErrorV1::InvalidConfig
        );

        let mismatched_grants = Arc::new(MockGrantBoundary::new(&fixture.config.grant_handle));
        mismatched_grants.qualification.set_revision(2);
        let mismatched_deps = EvidenceViewerRuntimeDepsV1 {
            grants: mismatched_grants,
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), mismatched_deps)
                .expect_err("provider revision outside deployment policy must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );

        let stale_grants = Arc::new(MockGrantBoundary::new(&fixture.config.grant_handle));
        stale_grants.qualification.set_failure(Some(
            EvidenceViewerRuntimeProviderReadinessErrorV1::Unavailable,
        ));
        let stale_deps = EvidenceViewerRuntimeDepsV1 {
            grants: stale_grants,
            ..fixture.deps.clone()
        };
        let stale_error = fixture
            .open_with(fixture.config.clone(), stale_deps)
            .expect_err("stale grant provider must fail startup");
        assert_eq!(stale_error, EvidenceViewerErrorV1::RuntimeUnavailable);
        assert!(!format!("{stale_error:?} {stale_error}").contains(MOCK_PROVIDER_SECRET));

        let unavailable_signer = Arc::new(MockReceiptSigner::new(
            &fixture.config.receipt_signer_handle,
            SigningKey::from_bytes(&[0x51; 32]),
        ));
        unavailable_signer.qualification.set_failure(Some(
            EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected,
        ));
        let unavailable_signer_deps = EvidenceViewerRuntimeDepsV1 {
            receipt_signer: unavailable_signer.clone(),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), unavailable_signer_deps)
                .expect_err("unqualified receipt signer must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        assert_eq!(
            unavailable_signer.public_key_call_count(),
            0,
            "signer metadata must not be trusted before expected qualification"
        );

        let drifting_signer = Arc::new(MockReceiptSigner::new(
            &fixture.config.receipt_signer_handle,
            SigningKey::from_bytes(&[0x51; 32]),
        ));
        drifting_signer
            .qualification
            .drift_policy_after_next_operation([0xB3; 32]);
        let drifting_signer_deps = EvidenceViewerRuntimeDepsV1 {
            receipt_signer: drifting_signer.clone(),
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), drifting_signer_deps)
                .expect_err("qualification drift during signer-key read must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );
        assert_eq!(drifting_signer.public_key_call_count(), 1);
        assert_eq!(drifting_signer.sign_call_count(), 0);

        let invalid_signer = Arc::new(MockReceiptSigner::new(
            &fixture.config.receipt_signer_handle,
            SigningKey::from_bytes(&[0x51; 32]),
        ));
        invalid_signer.qualification.set_policy_digest([0; 32]);
        let invalid_signer_deps = EvidenceViewerRuntimeDepsV1 {
            receipt_signer: invalid_signer,
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), invalid_signer_deps)
                .expect_err("zero signer policy digest must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );

        let invalid_erasure = Arc::new(MockErasureBoundary::new(&fixture.config.erasure_handle));
        invalid_erasure.qualification.set_revision(0);
        let invalid_erasure_deps = EvidenceViewerRuntimeDepsV1 {
            erasure: invalid_erasure,
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), invalid_erasure_deps)
                .expect_err("zero erasure-provider revision must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );

        let stale_archive = Arc::new(MockCompactionArchive::new(
            &fixture.config.compaction_archive_handle,
        ));
        stale_archive.qualification.set_failure(Some(
            EvidenceViewerRuntimeProviderReadinessErrorV1::Rejected,
        ));
        let stale_archive_deps = EvidenceViewerRuntimeDepsV1 {
            compaction_archive: stale_archive,
            ..fixture.deps.clone()
        };
        assert_eq!(
            fixture
                .open_with(fixture.config.clone(), stale_archive_deps)
                .expect_err("unqualified immutable archive must fail startup"),
            EvidenceViewerErrorV1::RuntimeUnavailable
        );

        assert!(
            !fixture.config.checkpoint_path.exists(),
            "qualification must finish before checkpoint creation or loading"
        );
        assert_eq!(
            fixture.signer.sign_call_count(),
            0,
            "qualification failure must precede signer operations"
        );
    }

    // Keep provider-policy, redaction, and signed-state tamper regressions in this
    // test module so their libtest paths and private-helper access remain stable.
    include!("evidence_viewer/provider_security_tests.rs");
}
