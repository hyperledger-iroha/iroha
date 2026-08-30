//! Crash-safe storage for exact Sumeragi v2 proposal bodies.
//!
//! Consensus votes authenticate a [`wire::BlockSubject`], including the hash of the
//! exact canonical [`SignedBlock`] wire bytes.  This store is the durability
//! boundary between reconstruction and the reducer's `BodyStored` input: a
//! receipt can only be obtained after the bytes, their canonically re-derived
//! RS16 manifest, and the directory entry have been synchronised. The first
//! release has one V1 frame layout and no predecessor-format reader or
//! migration path.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "recovered-body seams remain test-sealed until first release"
    )
)]
use super::v2_chunks::encode_payload;
use super::v2_core::EventTag;
use super::{
    v2::{
        PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1,
        RecoveredLifecycleNextVoteBodyAuthorityV1, RecoveredValidationAuthority,
        RecoveredWalVoteSign,
    },
    v2_apply::{V2ApplyError, V2ApplyService, VerifiedRecoveredFinalitySubject},
    v2_effects::{BodyStoreTask, EffectWorkId},
    v2_lifecycle_coordinator::{
        AuthenticatedRecoveredLifecycleOutputV1,
        AuthenticatedRecoveredReleasedValidateNoSuccessorV1,
        AuthenticatedRecoveredWalDecisionFetchProjection,
        AuthenticatedRecoveredWalValidateLedgerParent, LifecycleContext,
        RecoveredDecisionApplyReplayLineageV1, TerminalValidateNoSuccessorClaim,
    },
    v2_transport::AuthenticatedCertifiedBodyResponse,
};
use crate::kura::KuraV2CommitReceipt;
use iroha_config::parameters::defaults::sumeragi::V2_MAX_LIFECYCLE_RECORDS_PER_HEIGHT;
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::block::{
    CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire, decode_framed_signed_block,
};
use norito::codec::{Decode, DecodeAll as _, Encode};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
const STORE_MAGIC: &[u8; 8] = b"SUM2BODY";
const VALIDATED_MAGIC: &[u8; 8] = b"SUM2VALD";
const STORE_VERSION: u16 = 1;
const VALIDATION_OUTCOME_MARKER_VERSION: u16 = 1;
const FRAME_HEADER_LEN: usize = STORE_MAGIC.len() + size_of::<u16>() + size_of::<u64>();
const CHECKSUM_LEN: usize = 32;
const FRAME_PAYLOAD_MAX_BYTES: u64 =
    wire::MAX_EXECUTED_BLOCK_WIRE_BYTES + wire::MAX_DA_ENCODED_PAYLOAD_BYTES;
const VALIDATION_OUTCOME_MARKER_PAYLOAD_MAX_BYTES: u64 = 4 * 1024;
const VALIDATION_OUTCOME_MARKER_FRAME_MAX_BYTES: u64 =
    VALIDATION_OUTCOME_MARKER_PAYLOAD_MAX_BYTES + (FRAME_HEADER_LEN + CHECKSUM_LEN) as u64;
/// Test and inert read-only geometry used where no operator limit is consumed.
pub(crate) const DEFAULT_V2_BODY_STORE_MAX_BYTES_PER_HEIGHT: u64 = 1024 * 1024 * 1024;
const BODY_STORE_TEMPORARY_ENTRIES_PER_BODY: usize = 2;
const BODY_STORE_DIRECTORY_ENTRIES_PER_BODY: usize = 4;

#[derive(Clone, Copy)]
enum FramePayloadKind {
    Body,
    ValidationOutcomeMarker,
}

impl FramePayloadKind {
    const fn maximum_payload_bytes(self) -> u64 {
        match self {
            Self::Body => FRAME_PAYLOAD_MAX_BYTES,
            Self::ValidationOutcomeMarker => VALIDATION_OUTCOME_MARKER_PAYLOAD_MAX_BYTES,
        }
    }

    const fn too_large(self) -> V2BodyStoreError {
        match self {
            Self::Body => V2BodyStoreError::BodyTooLarge,
            Self::ValidationOutcomeMarker => V2BodyStoreError::ValidationMarkerTooLarge,
        }
    }

    fn maximum_frame_bytes(self) -> Result<u64, V2BodyStoreError> {
        self.maximum_payload_bytes()
            .checked_add(
                u64::try_from(FRAME_HEADER_LEN + CHECKSUM_LEN)
                    .expect("body-store frame overhead fits u64"),
            )
            .ok_or_else(|| self.too_large())
    }
}

fn body_store_leaf_has_canonical_name(name: &str, suffix: &str) -> bool {
    let Some(stem) = name.strip_suffix(suffix) else {
        return false;
    };
    let bytes = stem.as_bytes();
    let hash_digits = Hash::LENGTH * 2;
    let expected_len = 20 + 1 + 20 + 1 + hash_digits;
    if bytes.len() != expected_len || bytes[20] != b'-' || bytes[41] != b'-' {
        return false;
    }
    let decimal = |digits: &[u8], nonzero: bool| {
        digits.iter().all(u8::is_ascii_digit)
            && std::str::from_utf8(digits)
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|value| !nonzero || value != 0)
    };
    decimal(&bytes[..20], true)
        && decimal(&bytes[21..41], false)
        && bytes[42..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f'))
}

/// Per-height geometry for bounded durable body ownership.
///
/// Production construction fixes the semantic entry ceiling to the lifecycle
/// record limit. The byte ceiling covers complete final `.norito` body frames;
/// validation markers are separately size- and count-bounded, while atomic-write remnants are
/// count-bounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct V2BodyStoreCapacity {
    max_body_entries: usize,
    max_body_frame_bytes: u64,
    max_temporary_entries: usize,
    max_directory_entries: usize,
}

impl V2BodyStoreCapacity {
    /// Build the production capacity geometry for one height.
    pub(crate) fn new(max_bytes_per_height: u64) -> Result<Self, V2BodyStoreError> {
        Self::with_entry_limit(V2_MAX_LIFECYCLE_RECORDS_PER_HEIGHT, max_bytes_per_height)
    }

    fn with_entry_limit(
        max_body_entries: usize,
        max_body_frame_bytes: u64,
    ) -> Result<Self, V2BodyStoreError> {
        if max_body_entries == 0
            || max_body_entries > V2_MAX_LIFECYCLE_RECORDS_PER_HEIGHT
            || max_body_frame_bytes == 0
        {
            return Err(V2BodyStoreError::InvalidCapacityGeometry);
        }
        let max_temporary_entries = max_body_entries
            .checked_mul(BODY_STORE_TEMPORARY_ENTRIES_PER_BODY)
            .ok_or(V2BodyStoreError::InvalidCapacityGeometry)?;
        let max_directory_entries = max_body_entries
            .checked_mul(BODY_STORE_DIRECTORY_ENTRIES_PER_BODY)
            .ok_or(V2BodyStoreError::InvalidCapacityGeometry)?;
        Ok(Self {
            max_body_entries,
            max_body_frame_bytes,
            max_temporary_entries,
            max_directory_entries,
        })
    }

    #[cfg(test)]
    fn for_test(
        max_body_entries: usize,
        max_body_frame_bytes: u64,
    ) -> Result<Self, V2BodyStoreError> {
        Self::with_entry_limit(max_body_entries, max_body_frame_bytes)
    }
}
/// Metadata and exact canonical bytes persisted in one final body file.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct StoredBodyEnvelope {
    version: u16,
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest: wire::PayloadManifest,
    canonical_wire: Vec<u8>,
}
/// Closed durable result of deterministic validation for one exact body frame.
///
/// A merge-sidecar deferral is deliberately absent: it is a retry dependency,
/// not a terminal semantic outcome and therefore cannot become restart
/// authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum ValidationOutcomeMarkerKind {
    /// Deterministic execution succeeded with this exact commitment.
    Validated(wire::ExecutionCommitment),
    /// Deterministic execution rejected with this canonical closed code.
    Rejected(u8),
}
/// Versioned durable validation outcome bound to one exact body-store frame.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct ValidationOutcomeMarker {
    version: u16,
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
    body_frame_hash: Hash,
    outcome: ValidationOutcomeMarkerKind,
}
/// Non-forgeable acknowledgement that one exact body is durable locally.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableBodyReceipt {
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
    frame_hash: Hash,
}
/// Opaque proof that one exact recovered frame was reopened under the
/// genesis-authority signature policy for a parentless height-one context.
#[derive(Debug)]
#[must_use = "genesis-authority frame proof must remain with cold replay"]
pub(in crate::sumeragi) struct AuthenticatedGenesisBodyStoreFrameV1 {
    receipt: DurableBodyReceipt,
}
impl AuthenticatedGenesisBodyStoreFrameV1 {
    /// Compare this policy proof with the exact recovered durable receipt.
    pub(in crate::sumeragi) fn exactly_matches(&self, receipt: &DurableBodyReceipt) -> bool {
        &self.receipt == receipt
    }
}
/// Durable proof that one fully authenticated certified-Fetch response's exact
/// canonical body has crossed the local file-and-directory sync boundary.
///
/// This receipt deliberately retains the transport occurrence hashes without
/// serializing them into the body store. The nested body receipt remains the
/// sole authority for reloading the canonical bytes after restart.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct DurableCertifiedFetchBodyReceipt {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    durable_body: DurableBodyReceipt,
}
/// Opaque body-frame authority for the recovered Decision Fetch-to-Store cut.
///
/// Live completion and cold recovery are the only mints. Both paths bind the
/// exact canonical manifest to the already-fsynced body receipt before this
/// value can enter the adapter/registry successor projection.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "recovered Decision body authority must remain attached to Store settlement"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchStoreBodyAuthorityV1 {
    manifest: wire::PayloadManifest,
    durable: DurableBodyReceipt,
}
impl RecoveredDecisionFetchStoreBodyAuthorityV1 {
    /// Build one exact closed body authority for staged-address regressions.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        manifest: wire::PayloadManifest,
        durable: DurableBodyReceipt,
    ) -> Option<Self> {
        (durable.round() == manifest.round
            && durable.subject() == manifest.subject
            && durable.manifest_hash() == HashOf::new(&manifest))
        .then_some(Self { manifest, durable })
    }
    /// Bind one live authenticated response to its exact durable completion.
    pub(in crate::sumeragi) fn from_persisted_certified_response(
        authenticated: &AuthenticatedCertifiedBodyResponse,
        receipt: &DurableCertifiedFetchBodyReceipt,
    ) -> Option<Self> {
        let response = authenticated.response();
        (receipt.request_hash() == response.request_hash
            && receipt.response_hash() == HashOf::new(response)
            && receipt.durable_body().round() == response.manifest.round
            && receipt.durable_body().subject() == response.manifest.subject
            && receipt.durable_body().manifest_hash() == HashOf::new(&response.manifest))
        .then(|| Self {
            manifest: response.manifest.clone(),
            durable: receipt.durable_body().clone(),
        })
    }
    /// Borrow the canonical manifest only inside the fixed adapter preview.
    pub(in crate::sumeragi) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }
    /// Borrow the non-forgeable body receipt only inside fixed projections.
    pub(in crate::sumeragi) const fn durable(&self) -> &DurableBodyReceipt {
        &self.durable
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl DurableCertifiedFetchBodyReceipt {
    /// Build a byte-consistent durability projection for selector-only tests.
    #[cfg(test)]
    pub(crate) fn for_authenticated_response_test(
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Self {
        let response = authenticated.response();
        Self {
            request_hash: response.request_hash,
            response_hash: HashOf::new(response),
            durable_body: DurableBodyReceipt::for_test(
                response.manifest.round.context_id,
                response.manifest.round,
                response.manifest.subject,
                HashOf::new(&response.manifest),
            ),
        }
    }
    /// Hash of the exact authenticated request family served by the response.
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.request_hash
    }
    /// Hash of the complete authenticated response, including its responder.
    pub(crate) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
    /// Durable receipt for the response's exact canonical body-store frame.
    pub(crate) const fn durable_body(&self) -> &DurableBodyReceipt {
        &self.durable_body
    }
}
/// Non-forgeable acknowledgement that deterministic validation succeeded for
/// the exact durable body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct ValidatedBodyReceipt {
    durable: DurableBodyReceipt,
    execution_commitment: wire::ExecutionCommitment,
}
// DURABLE_BODY_VALIDATION_SURFACE_BEGIN
/// Canonical closed identity of a deterministic body rejection.
///
/// All non-sidecar validation failures currently drive the same
/// `ValidationCompleted { valid: false }` reducer transition. Keeping one
/// closed code prevents diagnostic formatting or unstable internal error
/// variants from becoming physical lifecycle identity.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodyValidationRejectionIdentity {
    /// Deterministic validation rejected the exact durable body.
    Rejected,
}
impl BodyValidationRejectionIdentity {
    /// Return the bounded code shared by durable markers and lifecycle digests.
    pub(crate) const fn canonical_code(&self) -> u8 {
        match self {
            Self::Rejected => 0,
        }
    }
    /// Decode the closed durable identity domain without accepting extensions.
    const fn from_canonical_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Rejected),
            _ => None,
        }
    }
}
/// Scheduler-free result of validating one exact durable body.
///
/// The private payload keeps construction inside this storage boundary. In
/// particular, every non-success result retains the same durable receipt that
/// was checked before the validator ran, so a caller cannot accidentally
/// rebind a diagnostic or merge-sidecar dependency to another proposal.
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableBodyValidationOutcome(DurableBodyValidationOutcomeBody);
/// Move-only ownership of one exact semantically revalidated restart marker.
///
/// The marker is removed from the process-local recovery catalog while this
/// cut exists. Dropping the cut before the recovered lifecycle parent accepts
/// it restores the exact marker; consuming it transfers the receipt directly
/// into an opaque validation outcome. No receipt accessor is provided.
#[must_use = "a detached recovered validation marker must be transferred or restored"]
pub(super) struct RecoveredValidatedBodyCut<'store> {
    store: &'store mut V2BodyStore,
    key: (wire::ConsensusRound, wire::BlockSubject),
    validated: Option<ValidatedBodyReceipt>,
}
/// Closed reason an authenticated WAL vote could not detach its body marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredValidatedBodyCutError {
    /// The body store belongs to another height context.
    ForeignContext,
    /// The exact proposal marker is absent or has not been revalidated.
    MissingMarker,
    /// The marker names another execution commitment.
    CommitmentMismatch,
}
/// Move-only ownership of the exact revalidated body named by a recovered Decision.
///
/// The complete body envelope, durable index entry, manifest, and successful
/// validation marker stay private and inseparable. The cut also retains the
/// identity and immutable context of the exact open store which minted it.
/// Dropping the cut before the restart-closed Apply transaction accepts it
/// restores every detached in-memory value exactly; it performs no storage
/// write and exposes no body, manifest, receipt, or validation-marker accessor.
#[must_use = "a detached recovered Decision body must be transferred or restored"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyBodyCut<'store> {
    store: &'store mut V2BodyStore,
    store_identity: V2BodyStoreInstanceIdentity,
    context: wire::HeightContext,
    key: (wire::ConsensusRound, wire::BlockSubject),
    envelope: StoredBodyEnvelope,
    durable: Option<DurableBodyReceipt>,
    manifest: Option<wire::PayloadManifest>,
    validated: Option<ValidatedBodyReceipt>,
}
/// Move-only recovered-Decision adapter preview with every authority kept whole.
///
/// The original Fetch projection retains its effect, pending binding, WAL
/// identity, replay evidence, and candidate internally. The body cut and replay
/// lineage remain inseparable from the staged adapter and its private
/// predecessor-derived Store/Validate/Apply pending chain. This value exposes
/// no effect, candidate, body, replay, pending, or parts accessor.
#[must_use = "a recovered Decision Apply preview must remain sealed through publication"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyAdapterPreviewV1<'store> {
    _fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
    _body: RecoveredDecisionApplyBodyCut<'store>,
    _replay: RecoveredDecisionApplyReplayLineageV1,
    _staged: super::v2::RecoveredDecisionApplyStagedAdapterV1,
}
/// Closed storage-ready Decision Apply preview.
///
/// The reducer-derived logical lineage remains attached to the same-store
/// detached body cut. The exact lifecycle transaction must restore the body
/// before authenticating the complete startup census and publishing LedgerV1.
#[must_use = "recovered Decision storage preview must enter exact publication"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyStoragePreviewV1<'store> {
    staged: Box<super::v2::RecoveredDecisionApplyStagedStorageV1>,
    body: RecoveredDecisionApplyBodyCut<'store>,
}
/// Storage-ready state after the exact body has been restored to its owner.
///
/// This value no longer borrows the body store. A persistence failure retains
/// all adapter and lifecycle authority here for restart-only handling.
#[must_use = "restored Decision Apply state must enter the single-fsync transaction"]
pub(in crate::sumeragi) struct RestoredRecoveredDecisionApplyStorageV1 {
    staged: Box<super::v2::RecoveredDecisionApplyStagedStorageV1>,
}
/// Opaque failure while closing the adapter preview for storage publication.
#[must_use = "failed recovered Decision storage projection requires restart"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyStoragePreviewErrorV1<'store> {
    _body: RecoveredDecisionApplyBodyCut<'store>,
    _projection: super::v2::RecoveredDecisionApplyStorageProjectionErrorV1,
}
#[cfg(all(test, feature = "bls"))]
impl RecoveredDecisionApplyAdapterPreviewV1<'_> {
    /// Recheck the sealed Fetch/body join and staged adapter state in tests.
    pub(in crate::sumeragi) fn validates_for_test(&self) -> bool {
        self._body.exactly_matches_decision(&self._fetch) && self._staged.validates()
    }
}
impl<'store> RecoveredDecisionApplyAdapterPreviewV1<'store> {
    /// Consume the reducer preview into the sole storage-ready closed form.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_storage_preview(
        self,
        verified: &super::v2::VerifiedHeightContext,
    ) -> Result<
        RecoveredDecisionApplyStoragePreviewV1<'store>,
        RecoveredDecisionApplyStoragePreviewErrorV1<'store>,
    > {
        let Self {
            _fetch: fetch,
            _body: body,
            _replay: replay,
            _staged: staged,
        } = self;
        let durable = body
            .durable
            .as_ref()
            .expect("a live Decision body cut retains its durable receipt");
        let validated = body
            .validated
            .as_ref()
            .expect("a live Decision body cut retains its validation receipt");
        match staged.into_storage_projection(verified, fetch, replay, durable, validated) {
            Ok(staged) => Ok(RecoveredDecisionApplyStoragePreviewV1 { staged, body }),
            Err(projection) => Err(RecoveredDecisionApplyStoragePreviewErrorV1 {
                _body: body,
                _projection: projection,
            }),
        }
    }
}
impl RecoveredDecisionApplyStoragePreviewV1<'_> {
    /// Recheck the same-store Fetch/body/lineage join without exposing parts.
    pub(in crate::sumeragi) fn validates(
        &self,
        verified: &super::v2::VerifiedHeightContext,
    ) -> bool {
        self.staged.validates(verified) && self.body.exactly_matches_decision(self.staged.fetch())
    }
    /// Borrow the opaque logical lineage for exact ledger staging.
    #[allow(
        dead_code,
        reason = "reviewed inspection seam retained beside the consuming restore path"
    )]
    pub(in crate::sumeragi) fn staged(&self) -> &super::v2::RecoveredDecisionApplyStagedStorageV1 {
        self.staged.as_ref()
    }
    /// Restore the exact body frame and marker, ending the store borrow before
    /// the external LedgerV1 publication can begin.
    pub(in crate::sumeragi) fn restore_body(self) -> RestoredRecoveredDecisionApplyStorageV1 {
        let Self { staged, body } = self;
        drop(body);
        RestoredRecoveredDecisionApplyStorageV1 { staged }
    }
}
impl RestoredRecoveredDecisionApplyStorageV1 {
    /// Borrow the exact staged lineage for the final pre-fsync recheck.
    pub(in crate::sumeragi) fn staged(&self) -> &super::v2::RecoveredDecisionApplyStagedStorageV1 {
        self.staged.as_ref()
    }
    /// Consume the restored state only inside the lifecycle publication tail.
    pub(in crate::sumeragi) fn into_staged(
        self,
    ) -> Box<super::v2::RecoveredDecisionApplyStagedStorageV1> {
        self.staged
    }
}
/// Opaque failed preview retaining every move-only input until fail-stop restart.
///
/// Dropping this value restores the detached body cut exactly. The adapter is
/// either the untouched input or the explicitly rolled-back cold state, and no
/// constituent can be recovered for an in-process retry.
#[must_use = "a failed recovered Decision Apply preview requires restart"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyAdapterPreviewError<'store> {
    reason: &'static str,
    _failure: RecoveredDecisionApplyAdapterPreviewFailure<'store>,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum RecoveredDecisionApplyAdapterPreviewFailure<'store> {
    Inputs {
        _adapter: super::v2::SumeragiV2Adapter,
        _fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        _body: RecoveredDecisionApplyBodyCut<'store>,
        _replay: RecoveredDecisionApplyReplayLineageV1,
    },
    Staging {
        _fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        _body: RecoveredDecisionApplyBodyCut<'store>,
        _replay: RecoveredDecisionApplyReplayLineageV1,
        _staging: super::v2::RecoveredDecisionApplyAdapterStagingError,
    },
}
impl RecoveredDecisionApplyAdapterPreviewError<'_> {
    /// Return one stable non-authorizing diagnostic without releasing inputs.
    pub(in crate::sumeragi) const fn reason(&self) -> &'static str {
        self.reason
    }
}
/// One-shot call capability for deriving the inert recovered-Decision replay family.
///
/// Construction stays beside the same-store body cut, so no caller can submit
/// arbitrary manifest or receipt parts to the WAL projection.
pub(in crate::sumeragi) struct RecoveredDecisionApplyReplayPermit {
    _linearity: RecoveredDecisionApplyReplayLinearity,
}
/// One-shot capability for the fixed recovered-Decision reducer preview.
pub(in crate::sumeragi) struct RecoveredDecisionApplyAdapterPreviewPermit {
    _linearity: RecoveredDecisionApplyAdapterPreviewLinearity,
}
struct RecoveredDecisionApplyAdapterPreviewLinearity;
impl Drop for RecoveredDecisionApplyAdapterPreviewLinearity {
    fn drop(&mut self) {}
}
struct RecoveredDecisionApplyReplayLinearity;
impl Drop for RecoveredDecisionApplyReplayLinearity {
    fn drop(&mut self) {}
}
/// Closed reason a revalidated same-store Decision body could not be detached.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub(in crate::sumeragi) enum RecoveredDecisionApplyBodyCutError {
    /// The Decision projection belongs to another immutable height context.
    #[error("recovered Decision body belongs to another height context")]
    ForeignContext,
    /// The store still contains checksummed markers which have not been revalidated.
    #[error("recovered Decision body markers require semantic revalidation")]
    UnrevalidatedMarkers,
    /// Deterministic replay rejected the exact body named by the Decision.
    #[error("recovered Decision body was deterministically rejected")]
    DeterministicRejection,
    /// No successful revalidated marker matches the exact Decision body.
    #[error("recovered Decision has no exact revalidated body marker")]
    MissingMarker,
    /// More than one successful marker claimed the exact Decision body.
    #[error("recovered Decision body marker is ambiguous")]
    AmbiguousMarker,
    /// The accepted in-memory frame, manifest, marker, or durable file drifted.
    #[error("recovered Decision body-store frame failed exact revalidation: {0}")]
    BodyStore(#[source] V2BodyStoreError),
}
#[derive(Debug, PartialEq, Eq)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum DurableBodyValidationOutcomeBody {
    Validated(ValidatedBodyReceipt),
    Rejected {
        durable: DurableBodyReceipt,
        identity: BodyValidationRejectionIdentity,
        reason: String,
    },
    DeferredMergeSidecar {
        durable: DurableBodyReceipt,
        reference: CertifiedMergeLedgerReference,
    },
}
#[cfg_attr(not(test), allow(dead_code))]
impl DurableBodyValidationOutcome {
    /// Exact durable body bound to this result.
    pub(crate) const fn durable_body(&self) -> &DurableBodyReceipt {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => receipt.durable(),
            DurableBodyValidationOutcomeBody::Rejected { durable, .. }
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { durable, .. } => durable,
        }
    }
    /// Durable success receipt, when deterministic validation succeeded.
    pub(crate) const fn validated_receipt(&self) -> Option<&ValidatedBodyReceipt> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => Some(receipt),
            DurableBodyValidationOutcomeBody::Rejected { .. }
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { .. } => None,
        }
    }
    /// Deterministic rejection diagnostic, when validation rejected the body.
    pub(crate) fn rejection_reason(&self) -> Option<&str> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Rejected { reason, .. } => Some(reason),
            DurableBodyValidationOutcomeBody::Validated(_)
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { .. } => None,
        }
    }
    /// Canonical closed identity of a deterministic rejection.
    pub(crate) const fn rejection_identity(&self) -> Option<&BodyValidationRejectionIdentity> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::Rejected { identity, .. } => Some(identity),
            DurableBodyValidationOutcomeBody::Validated(_)
            | DurableBodyValidationOutcomeBody::DeferredMergeSidecar { .. } => None,
        }
    }
    /// Exact certified merge reference whose absence deferred validation.
    pub(crate) const fn missing_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match &self.0 {
            DurableBodyValidationOutcomeBody::DeferredMergeSidecar { reference, .. } => {
                Some(reference)
            }
            DurableBodyValidationOutcomeBody::Validated(_)
            | DurableBodyValidationOutcomeBody::Rejected { .. } => None,
        }
    }
    /// Consume this closed result only when deterministic validation succeeded.
    ///
    /// A rejection or sidecar deferral is returned intact so the typed
    /// lifecycle completion transaction cannot accidentally discard or relabel
    /// it while attempting the success-only Validate path.
    pub(crate) fn into_validated_receipt(self) -> Result<ValidatedBodyReceipt, Self> {
        match self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => Ok(receipt),
            body => Err(Self(body)),
        }
    }
    /// Construct a sealed successful outcome for lifecycle boundary tests.
    #[cfg(test)]
    pub(crate) const fn validated_for_test(receipt: ValidatedBodyReceipt) -> Self {
        Self(DurableBodyValidationOutcomeBody::Validated(receipt))
    }
    /// Construct a sealed deterministic rejection for lifecycle boundary tests.
    #[cfg(test)]
    pub(crate) fn rejected_for_test(durable: DurableBodyReceipt) -> Self {
        Self(DurableBodyValidationOutcomeBody::Rejected {
            durable,
            identity: BodyValidationRejectionIdentity::Rejected,
            reason: "test-only deterministic rejection".to_owned(),
        })
    }
}
impl RecoveredValidatedBodyCut<'_> {
    /// Revalidate this detached marker against the same authenticated WAL vote.
    pub(super) fn exactly_matches_vote(&self, recovered: &RecoveredWalVoteSign) -> bool {
        self.validated.as_ref().is_some_and(|validated| {
            let vote = recovered.vote();
            self.key == (vote.proposal_round, vote.subject)
                && validated.durable().context_id() == vote.round.context_id
                && validated.durable().round() == vote.proposal_round
                && validated.durable().subject() == vote.subject
                && validated.execution_commitment() == vote.execution_commitment
        })
    }
    /// Match this one-shot marker to the exact BodyFrame retained by a durable
    /// recovered-WAL Validate parent.
    ///
    /// The fixed boolean join keeps both the receipt and ledger payload opaque;
    /// neither side can be detached or substituted after this comparison.
    pub(super) fn exactly_matches_ledger_parent(
        &self,
        active_context: LifecycleContext,
        parent: &AuthenticatedRecoveredWalValidateLedgerParent,
    ) -> bool {
        self.validated.as_ref().is_some_and(|validated| {
            parent.matches_durable_receipt(active_context, validated.durable())
        })
    }
    /// Transfer the exact marker into the sealed durable-validation outcome.
    pub(super) fn into_validation_outcome(mut self) -> DurableBodyValidationOutcome {
        let validated = self
            .validated
            .take()
            .expect("a live recovered validation cut retains one marker");
        DurableBodyValidationOutcome(DurableBodyValidationOutcomeBody::Validated(validated))
    }
}
impl Drop for RecoveredValidatedBodyCut<'_> {
    fn drop(&mut self) {
        let Some(validated) = self.validated.take() else {
            return;
        };
        let displaced = self.store.validated.insert(self.key, validated);
        debug_assert!(displaced.is_none());
    }
}
impl<'store> RecoveredDecisionApplyBodyCut<'store> {
    /// Recheck the opaque cut against the same authenticated recovered Decision.
    ///
    /// This comparison deliberately returns only a boolean. It cannot release
    /// any constituent which a caller could splice into another transition.
    pub(in crate::sumeragi) fn exactly_matches_decision(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        let Some(durable) = self.durable.as_ref() else {
            return false;
        };
        let Some(manifest) = self.manifest.as_ref() else {
            return false;
        };
        let Some(validated) = self.validated.as_ref() else {
            return false;
        };
        self.store
            .instance_identity()
            .same_instance(&self.store_identity)
            && self.store.context == self.context
            && self.key == (durable.round(), durable.subject())
            && self.context.id() == durable.context_id()
            && self.context.height == durable.round().height
            && validated.durable() == durable
            && projection.matches_validated_body(validated)
            && &self.envelope.manifest == manifest
            && self.envelope.context_id == durable.context_id()
            && self.envelope.round == durable.round()
            && self.envelope.subject == durable.subject()
            && HashOf::new(manifest) == durable.manifest_hash()
    }
    /// Derive the inert Store/Validate/Apply replay family without exposing body parts.
    pub(in crate::sumeragi) fn prepare_replay_lineage(
        &self,
        verified: &super::v2::VerifiedHeightContext,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Option<RecoveredDecisionApplyReplayLineageV1> {
        if !self.exactly_matches_decision(projection) {
            return None;
        }
        projection.project_decision_apply_replay_lineage(
            RecoveredDecisionApplyReplayPermit {
                _linearity: RecoveredDecisionApplyReplayLinearity,
            },
            verified,
            self.manifest.as_ref()?,
            self.durable.as_ref()?,
        )
    }
    /// Consume every exact input into one fixed three-event reducer fast-forward.
    ///
    /// The supplied replay lineage is re-derived and compared before the body
    /// parts reach the adapter. A typed failure keeps the complete projection,
    /// body cut, replay lineage, and original or rolled-back adapter opaque.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_adapter_preview(
        self,
        adapter: super::v2::SumeragiV2Adapter,
        verified: &super::v2::VerifiedHeightContext,
        projection: AuthenticatedRecoveredWalDecisionFetchProjection,
        replay: RecoveredDecisionApplyReplayLineageV1,
    ) -> Result<
        RecoveredDecisionApplyAdapterPreviewV1<'store>,
        RecoveredDecisionApplyAdapterPreviewError<'store>,
    > {
        let exact = self.exactly_matches_decision(&projection)
            && self
                .prepare_replay_lineage(verified, &projection)
                .is_some_and(|expected| expected == replay);
        if !exact {
            return Err(RecoveredDecisionApplyAdapterPreviewError {
                reason: "Decision body, Fetch projection, and replay lineage do not match",
                _failure: RecoveredDecisionApplyAdapterPreviewFailure::Inputs {
                    _adapter: adapter,
                    _fetch: projection,
                    _body: self,
                    _replay: replay,
                },
            });
        }
        match adapter.prepare_recovered_decision_apply_fast_forward(
            RecoveredDecisionApplyAdapterPreviewPermit {
                _linearity: RecoveredDecisionApplyAdapterPreviewLinearity,
            },
            verified,
            &projection,
            self.manifest
                .as_ref()
                .expect("a live Decision body cut retains its manifest"),
            self.durable
                .as_ref()
                .expect("a live Decision body cut retains its durable receipt"),
            self.validated
                .as_ref()
                .expect("a live Decision body cut retains its validation marker"),
        ) {
            Ok(staged) => Ok(RecoveredDecisionApplyAdapterPreviewV1 {
                _fetch: projection,
                _body: self,
                _replay: replay,
                _staged: staged,
            }),
            Err(staging) => {
                let _ = staging.error();
                Err(RecoveredDecisionApplyAdapterPreviewError {
                    reason: "Decision body reducer fast-forward is inconsistent",
                    _failure: RecoveredDecisionApplyAdapterPreviewFailure::Staging {
                        _fetch: projection,
                        _body: self,
                        _replay: replay,
                        _staging: staging,
                    },
                })
            }
        }
    }
    /// Compare the retained process identity and full immutable context in tests.
    #[cfg(all(test, feature = "bls"))]
    pub(in crate::sumeragi) fn exactly_matches_store_for_test(
        &self,
        identity: &V2BodyStoreInstanceIdentity,
        context: &wire::HeightContext,
    ) -> bool {
        self.store_identity.same_instance(identity) && &self.context == context
    }
}
impl Drop for RecoveredDecisionApplyBodyCut<'_> {
    fn drop(&mut self) {
        let durable = self
            .durable
            .take()
            .expect("a live recovered Decision body cut retains its durable frame");
        let manifest = self
            .manifest
            .take()
            .expect("a live recovered Decision body cut retains its manifest");
        let validated = self
            .validated
            .take()
            .expect("a live recovered Decision body cut retains its validation marker");
        debug_assert_eq!(&durable, validated.durable());
        debug_assert_eq!(&manifest, &self.envelope.manifest);
        assert!(
            !self.store.entries.contains_key(&self.key)
                && !self.store.manifests.contains_key(&self.key)
                && !self.store.validated.contains_key(&self.key),
            "a detached recovered Decision body cannot collide while restoring"
        );
        let displaced_durable = self.store.entries.insert(self.key, durable);
        let displaced_manifest = self.store.manifests.insert(self.key, manifest);
        let displaced_validated = self.store.validated.insert(self.key, validated);
        assert!(displaced_durable.is_none());
        assert!(displaced_manifest.is_none());
        assert!(displaced_validated.is_none());
    }
}
// DURABLE_BODY_VALIDATION_SURFACE_END
#[derive(Clone, Debug, PartialEq, Eq)]
struct QuarantinedValidationOutcome {
    durable: DurableBodyReceipt,
    outcome: ValidationOutcomeMarkerKind,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct RevalidatedRejectedBody {
    durable: DurableBodyReceipt,
    identity_code: u8,
    /// Volatile diagnostic text reproduced by the current validator.
    reason: String,
}
impl RevalidatedRejectedBody {
    fn sealed_outcome(&self) -> DurableBodyValidationOutcome {
        let identity = BodyValidationRejectionIdentity::from_canonical_code(self.identity_code)
            .expect("revalidated rejection codes are closed and canonical");
        DurableBodyValidationOutcome(DurableBodyValidationOutcomeBody::Rejected {
            durable: self.durable.clone(),
            identity,
            reason: self.reason.clone(),
        })
    }
}
/// Move-only ownership of all semantically revalidated terminal Validate outcomes.
///
/// The cut remains borrowed from one body-store instance and exposes only an
/// exact ledger-claim selector. Dropping it restores every detached outcome;
/// committing it consumes selected outcomes and restores every unselected one.
#[must_use = "a detached terminal Validate outcome catalog must be committed or restored"]
pub(super) struct RecoveredTerminalValidateOutcomeCatalogCut<'store> {
    store: &'store mut V2BodyStore,
    validated: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    rejected: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), RevalidatedRejectedBody>,
    selected_validated: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    selected_rejected:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), RevalidatedRejectedBody>,
}
/// Closed reason the terminal Validate outcome catalog cannot be detached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredTerminalValidateOutcomeCatalogError {
    /// At least one durable marker still awaits semantic replay.
    UnrevalidatedMarkers,
    /// One proposal key names both a success and a deterministic rejection.
    AmbiguousOutcome,
}
impl RecoveredTerminalValidateOutcomeCatalogCut<'_> {
    /// Select exactly one unselected outcome authenticated by the ledger claim.
    ///
    /// Zero or multiple matches fail without consuming any candidate. Already
    /// selected outcomes are not eligible for a second claim.
    pub(super) fn select_exact_terminal_validate(
        &mut self,
        claim: &TerminalValidateNoSuccessorClaim,
    ) -> bool {
        enum ExactMatch {
            Validated((wire::ConsensusRound, wire::BlockSubject)),
            Rejected((wire::ConsensusRound, wire::BlockSubject)),
        }
        let mut exact_match = None;
        for (key, validated) in &self.validated {
            let outcome = DurableBodyValidationOutcome(
                DurableBodyValidationOutcomeBody::Validated(validated.clone()),
            );
            if claim.matches_outcome(&outcome)
                && exact_match.replace(ExactMatch::Validated(*key)).is_some()
            {
                return false;
            }
        }
        for (key, rejected) in &self.rejected {
            let outcome = rejected.sealed_outcome();
            if claim.matches_outcome(&outcome)
                && exact_match.replace(ExactMatch::Rejected(*key)).is_some()
            {
                return false;
            }
        }
        match exact_match {
            Some(ExactMatch::Validated(key)) => {
                let validated = self
                    .validated
                    .remove(&key)
                    .expect("an exact catalog match remains unselected");
                let displaced = self.selected_validated.insert(key, validated);
                debug_assert!(displaced.is_none());
            }
            Some(ExactMatch::Rejected(key)) => {
                let rejected = self
                    .rejected
                    .remove(&key)
                    .expect("an exact catalog match remains unselected");
                let displaced = self.selected_rejected.insert(key, rejected);
                debug_assert!(displaced.is_none());
            }
            None => return false,
        }
        true
    }

    /// Select exactly one successful marker authenticated by the ledger claim.
    ///
    /// Rejections are deliberately outside this selector: a released Apply can
    /// be recovered only from a durable successful execution commitment.
    /// Selection remains rollback-safe and one-shot within this catalog cut.
    pub(super) fn select_exact_successful_terminal_validate(
        &mut self,
        claim: &TerminalValidateNoSuccessorClaim,
    ) -> bool {
        let mut exact_key = None;
        for (key, validated) in &self.validated {
            if claim.matches_validated_receipt(validated) && exact_key.replace(*key).is_some() {
                return false;
            }
        }
        let Some(key) = exact_key else {
            return false;
        };
        let validated = self
            .validated
            .remove(&key)
            .expect("an exact successful catalog match remains unselected");
        let displaced = self.selected_validated.insert(key, validated);
        debug_assert!(displaced.is_none());
        true
    }

    /// Select the sole revalidated rejection marker named by one cold report row.
    ///
    /// The output carrier retains the private manifest/frame/rejection binding;
    /// this catalog exposes no marker parts.  Selection is rollback-safe under
    /// `Drop` and shares the same selected map as terminal Validate recovery, so
    /// one durable rejection can never authorize two logical restart owners.
    pub(super) fn select_exact_invalid_body_report(
        &mut self,
        report: &AuthenticatedRecoveredLifecycleOutputV1,
    ) -> bool {
        if !report.requires_rejected_body_marker() {
            return false;
        }
        let mut exact_key = None;
        for (key, rejected) in &self.rejected {
            if report.exactly_matches_rejected_body_outcome(&rejected.sealed_outcome())
                && exact_key.replace(*key).is_some()
            {
                return false;
            }
        }
        let Some(key) = exact_key else {
            return false;
        };
        let rejected = self
            .rejected
            .remove(&key)
            .expect("an exact invalid-body marker remains unselected");
        let displaced = self.selected_rejected.insert(key, rejected);
        debug_assert!(displaced.is_none());
        true
    }
    /// Consume selected outcomes and restore every unselected catalog entry.
    pub(super) fn commit_selected(mut self) {
        self.restore_unselected();
        self.selected_validated.clear();
        self.selected_rejected.clear();
    }
    /// Commit all selected outcomes and transfer one exact success into a
    /// move-only released-Validate authority.
    ///
    /// The named claim must already have crossed
    /// [`Self::select_exact_successful_terminal_validate`]. Zero or multiple
    /// selected matches fail closed; dropping the cut then restores every
    /// detached marker.
    pub(super) fn commit_selected_with_released_validate(
        mut self,
        claim: TerminalValidateNoSuccessorClaim,
    ) -> Option<AuthenticatedRecoveredReleasedValidateNoSuccessorV1> {
        let mut exact_key = None;
        for (key, validated) in &self.selected_validated {
            if claim.matches_validated_receipt(validated) && exact_key.replace(*key).is_some() {
                return None;
            }
        }
        let key = exact_key?;
        let validated = self.selected_validated.get(&key)?.clone();
        let authority =
            AuthenticatedRecoveredReleasedValidateNoSuccessorV1::from_consumed_body_store_success(
                claim, validated,
            )?;
        self.selected_validated.remove(&key);
        self.restore_unselected();
        self.selected_validated.clear();
        self.selected_rejected.clear();
        Some(authority)
    }
    fn restore_unselected(&mut self) {
        for (key, validated) in std::mem::take(&mut self.validated) {
            let displaced = self.store.validated.insert(key, validated);
            debug_assert!(displaced.is_none());
        }
        for (key, rejected) in std::mem::take(&mut self.rejected) {
            let displaced = self.store.rejected.insert(key, rejected);
            debug_assert!(displaced.is_none());
        }
    }
}
impl Drop for RecoveredTerminalValidateOutcomeCatalogCut<'_> {
    fn drop(&mut self) {
        self.restore_unselected();
        for (key, validated) in std::mem::take(&mut self.selected_validated) {
            let displaced = self.store.validated.insert(key, validated);
            debug_assert!(displaced.is_none());
        }
        for (key, rejected) in std::mem::take(&mut self.selected_rejected) {
            let displaced = self.store.rejected.insert(key, rejected);
            debug_assert!(displaced.is_none());
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum SemanticReplayOutcome {
    Validated(wire::ExecutionCommitment),
    Rejected { identity_code: u8, reason: String },
    DeferredMergeSidecar,
}
/// Completion minted only after an exact body task reaches durable storage.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct BodyStoreCompletion {
    work_id: EffectWorkId,
    tag: EventTag,
    manifest: wire::PayloadManifest,
    receipt: DurableBodyReceipt,
}
impl BodyStoreCompletion {
    /// Stable asynchronous work identifier.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        self.work_id
    }
    /// Original reducer event tag.
    pub(crate) const fn tag(&self) -> EventTag {
        self.tag
    }
    /// Non-forgeable receipt for the exact durable bytes.
    pub(crate) const fn receipt(&self) -> &DurableBodyReceipt {
        &self.receipt
    }
    /// Exact manifest stored beside the durable bytes.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }
}
/// Typed classification supplied by deterministic body validators.
///
/// Only a missing, compact-reference-bound merge sidecar is recoverable. Every
/// other semantic error remains a terminal rejection of the exact body.
pub(crate) trait BodyValidationError: std::fmt::Display {
    /// Return the canonical reducer-level identity of a terminal rejection.
    ///
    /// Every current non-sidecar failure has identical `valid: false`
    /// semantics, so the safe default is the one closed rejection identity.
    fn rejection_identity(&self) -> BodyValidationRejectionIdentity {
        BodyValidationRejectionIdentity::Rejected
    }
    /// Return the exact missing sidecar reference when validation should defer.
    fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        None
    }
}
impl BodyValidationError for String {}
/// Authority whose single block signature must cover an exact proposal body.
///
/// Height-one genesis is signed by the configured genesis authority rather
/// than by the rotating consensus leader.  Keeping that distinction explicit
/// avoids either rejecting a valid genesis body or silently weakening the
/// signature check for ordinary blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BlockSignaturePolicy {
    /// Require the rotating leader selected by the immutable body-header view.
    RotatingLeader,
    /// Require signature index zero and the configured genesis public key.
    GenesisAuthority(PublicKey),
}
impl ValidatedBodyReceipt {
    /// Durable body receipt whose exact bytes passed validation.
    pub(crate) const fn durable(&self) -> &DurableBodyReceipt {
        &self.durable
    }
    /// Exact deterministic execution result fsynced with the validation marker.
    pub(crate) const fn execution_commitment(&self) -> wire::ExecutionCommitment {
        self.execution_commitment
    }
    #[cfg(test)]
    pub(crate) fn for_test(durable: DurableBodyReceipt) -> Self {
        let empty = Hash::new([]);
        let bind_frame = |domain: &[u8]| {
            let mut preimage = Vec::with_capacity(domain.len() + 1 + Hash::LENGTH);
            preimage.extend_from_slice(domain);
            preimage.push(0);
            preimage.extend_from_slice(durable.frame_hash.as_ref());
            Hash::new(preimage)
        };
        Self {
            execution_commitment: wire::ExecutionCommitment::new_without_merge_carrier(
                bind_frame(b"iroha:sumeragi:v2:test-parent-state-root:v1"),
                bind_frame(b"iroha:sumeragi:v2:test-post-state-root:v1"),
                empty,
                None,
                0,
                1,
                bind_frame(b"iroha:sumeragi:v2:test-executed-block-wire:v1"),
            )
            .expect("test execution commitment is canonical"),
            durable,
        }
    }
    #[cfg(test)]
    pub(crate) const fn for_test_with_commitment(
        durable: DurableBodyReceipt,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Self {
        Self {
            durable,
            execution_commitment,
        }
    }
}
impl DurableBodyReceipt {
    /// Frozen context which owns the body.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Proposal round which owns the body.
    pub(crate) const fn round(&self) -> wire::ConsensusRound {
        self.round
    }
    /// Exact certified subject represented by the bytes.
    pub(crate) const fn subject(&self) -> wire::BlockSubject {
        self.subject
    }
    /// Hash of the canonical manifest stored beside the body.
    pub(crate) const fn manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.manifest_hash
    }
    /// Hash of the complete checksummed body-store frame.
    ///
    /// This is the lossless 256-bit identity of the bytes acknowledged by the
    /// receipt. Comparisons mediated by this value rely on the repository's
    /// reviewed collision-resistance contract for [`Hash`].
    pub(crate) const fn frame_hash(&self) -> Hash {
        self.frame_hash
    }
    #[cfg(test)]
    pub(crate) fn for_test(
        context_id: wire::HeightContextId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        manifest_hash: HashOf<wire::PayloadManifest>,
    ) -> Self {
        Self {
            context_id,
            round,
            subject,
            manifest_hash,
            frame_hash: Hash::new(b"test-only durable body receipt"),
        }
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UnixFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(all(unix, not(target_os = "espidf")))]
impl UnixFileIdentity {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }
    }

    fn from_metadata(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;

        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[derive(Clone, Debug)]
struct BoundBodyStoreLeaf {
    name: OsString,
    identity: UnixFileIdentity,
    length: u64,
}

/// Descriptor-relative owner of one context directory below the Kura-bound
/// `sumeragi_v2/bodies` root.
///
/// All filesystem mutations use these retained handles. Paths are diagnostic
/// only and are rechecked against the opened identities before and after each
/// durability boundary.
#[derive(Debug)]
struct BoundV2BodyContextDirectory {
    body_root_path: PathBuf,
    context_path: PathBuf,
    context_name: OsString,
    #[cfg(all(unix, not(target_os = "espidf")))]
    body_root: File,
    #[cfg(all(unix, not(target_os = "espidf")))]
    context: File,
    #[cfg(all(unix, not(target_os = "espidf")))]
    body_root_identity: UnixFileIdentity,
    #[cfg(all(unix, not(target_os = "espidf")))]
    context_identity: UnixFileIdentity,
}

impl BoundV2BodyContextDirectory {
    fn from_kura_authority(
        kura: &crate::kura::Kura,
        authority: crate::kura::KuraV2BodyStoreDirectoryAuthority,
        context_name: OsString,
    ) -> Result<Self, V2BodyStoreError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            if !authority.matches_kura(kura) {
                return Err(V2BodyStoreError::StorageBinding {
                    path: kura.sumeragi_v2_storage_root().join("bodies"),
                    reason: "body-store authority belongs to another Kura instance",
                });
            }
            let (body_root_path, body_root) = authority
                .into_opened_directory_for(kura)
                .ok_or_else(|| V2BodyStoreError::StorageBinding {
                    path: kura.sumeragi_v2_storage_root().join("bodies"),
                    reason: "consumed body-store authority changed its Kura identity",
                })?;
            return Self::from_opened_body_root(body_root_path, body_root, context_name);
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (kura, authority, context_name);
            Err(V2BodyStoreError::UnsupportedStorageBinding {
                path: kura.sumeragi_v2_storage_root().join("bodies"),
            })
        }
    }

    #[cfg(all(test, unix, not(target_os = "espidf")))]
    fn bind_test_root(
        body_root_path: &Path,
        context_name: OsString,
    ) -> Result<Self, V2BodyStoreError> {
        use std::os::unix::fs::OpenOptionsExt as _;

        fs::create_dir_all(body_root_path).map_err(|source| V2BodyStoreError::Io {
            path: body_root_path.to_path_buf(),
            source,
        })?;
        let lexical =
            fs::symlink_metadata(body_root_path).map_err(|source| V2BodyStoreError::Io {
                path: body_root_path.to_path_buf(),
                source,
            })?;
        if lexical.file_type().is_symlink() || !lexical.is_dir() {
            return Err(V2BodyStoreError::StorageBinding {
                path: body_root_path.to_path_buf(),
                reason: "body-store root is not a direct directory",
            });
        }
        let body_root = OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC)
                    .bits() as i32,
            )
            .open(body_root_path)
            .map_err(|source| V2BodyStoreError::Io {
                path: body_root_path.to_path_buf(),
                source,
            })?;
        if UnixFileIdentity::from_metadata(&lexical)
            != UnixFileIdentity::from_metadata(&body_root.metadata().map_err(|source| {
                V2BodyStoreError::Io {
                    path: body_root_path.to_path_buf(),
                    source,
                }
            })?)
        {
            return Err(V2BodyStoreError::StorageBinding {
                path: body_root_path.to_path_buf(),
                reason: "body-store root changed while opening",
            });
        }
        Self::from_opened_body_root(body_root_path.to_path_buf(), body_root, context_name)
    }

    #[cfg(all(test, not(all(unix, not(target_os = "espidf")))))]
    fn bind_test_root(
        body_root_path: &Path,
        _context_name: OsString,
    ) -> Result<Self, V2BodyStoreError> {
        Err(V2BodyStoreError::UnsupportedStorageBinding {
            path: body_root_path.to_path_buf(),
        })
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn from_opened_body_root(
        body_root_path: PathBuf,
        body_root: File,
        context_name: OsString,
    ) -> Result<Self, V2BodyStoreError> {
        let body_root_metadata = body_root
            .metadata()
            .map_err(|source| V2BodyStoreError::Io {
                path: body_root_path.clone(),
                source,
            })?;
        let lexical =
            fs::symlink_metadata(&body_root_path).map_err(|source| V2BodyStoreError::Io {
                path: body_root_path.clone(),
                source,
            })?;
        let body_root_identity = UnixFileIdentity::from_metadata(&body_root_metadata);
        if !body_root_metadata.is_dir()
            || lexical.file_type().is_symlink()
            || !lexical.is_dir()
            || UnixFileIdentity::from_metadata(&lexical) != body_root_identity
        {
            return Err(V2BodyStoreError::StorageBinding {
                path: body_root_path,
                reason: "opened body-store root is no longer directly linked",
            });
        }
        let context_path = body_root_path.join(&context_name);
        let created = match rustix::fs::mkdirat(&body_root, &context_name, rustix::fs::Mode::RWXU) {
            Ok(()) => true,
            Err(rustix::io::Errno::EXIST) => false,
            Err(error) => {
                return Err(V2BodyStoreError::Io {
                    path: context_path,
                    source: std::io::Error::from(error),
                });
            }
        };
        let before = rustix::fs::statat(
            &body_root,
            &context_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
        .map_err(|source| V2BodyStoreError::Io {
            path: context_path.clone(),
            source,
        })?;
        if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::Directory {
            return Err(V2BodyStoreError::StorageBinding {
                path: context_path,
                reason: "body-store context entry is not a direct directory",
            });
        }
        let context = File::from(
            rustix::fs::openat(
                &body_root,
                &context_name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: context_path.clone(),
                source,
            })?,
        );
        let opened = context.metadata().map_err(|source| V2BodyStoreError::Io {
            path: context_path.clone(),
            source,
        })?;
        let after = rustix::fs::statat(
            &body_root,
            &context_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
        .map_err(|source| V2BodyStoreError::Io {
            path: context_path.clone(),
            source,
        })?;
        let context_identity = UnixFileIdentity::from_metadata(&opened);
        if !opened.is_dir()
            || UnixFileIdentity::from_stat(&before) != context_identity
            || UnixFileIdentity::from_stat(&after) != context_identity
        {
            return Err(V2BodyStoreError::StorageBinding {
                path: context_path,
                reason: "body-store context changed while opening",
            });
        }
        rustix::fs::flock(
            &context,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .map_err(std::io::Error::from)
        .map_err(|source| V2BodyStoreError::Io {
            path: context_path.clone(),
            source,
        })?;
        let bound = Self {
            body_root_path,
            context_path,
            context_name,
            body_root,
            context,
            body_root_identity,
            context_identity,
        };
        bound.verify_linked()?;
        if created {
            bound.sync_body_root()?;
        }
        Ok(bound)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_body_root_linked(&self) -> Result<(), V2BodyStoreError> {
        let linked_root =
            fs::symlink_metadata(&self.body_root_path).map_err(|source| V2BodyStoreError::Io {
                path: self.body_root_path.clone(),
                source,
            })?;
        let opened_root = self
            .body_root
            .metadata()
            .map_err(|source| V2BodyStoreError::Io {
                path: self.body_root_path.clone(),
                source,
            })?;
        if linked_root.file_type().is_symlink()
            || !linked_root.is_dir()
            || !opened_root.is_dir()
            || UnixFileIdentity::from_metadata(&linked_root) != self.body_root_identity
            || UnixFileIdentity::from_metadata(&opened_root) != self.body_root_identity
        {
            return Err(V2BodyStoreError::StorageBinding {
                path: self.body_root_path.clone(),
                reason: "body-store root identity changed after binding",
            });
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_linked(&self) -> Result<(), V2BodyStoreError> {
        self.verify_body_root_linked()?;
        let linked_context = rustix::fs::statat(
            &self.body_root,
            &self.context_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
        .map_err(|source| V2BodyStoreError::Io {
            path: self.context_path.clone(),
            source,
        })?;
        let opened_context = self
            .context
            .metadata()
            .map_err(|source| V2BodyStoreError::Io {
                path: self.context_path.clone(),
                source,
            })?;
        if rustix::fs::FileType::from_raw_mode(linked_context.st_mode)
            != rustix::fs::FileType::Directory
            || UnixFileIdentity::from_stat(&linked_context) != self.context_identity
            || !opened_context.is_dir()
            || UnixFileIdentity::from_metadata(&opened_context) != self.context_identity
        {
            return Err(V2BodyStoreError::StorageBinding {
                path: self.context_path.clone(),
                reason: "body-store directory identity changed after binding",
            });
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn fresh_context_handle(&self) -> Result<File, V2BodyStoreError> {
        self.verify_linked()?;
        let file = File::from(
            rustix::fs::openat(
                &self.body_root,
                &self.context_name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: self.context_path.clone(),
                source,
            })?,
        );
        let metadata = file.metadata().map_err(|source| V2BodyStoreError::Io {
            path: self.context_path.clone(),
            source,
        })?;
        if !metadata.is_dir() || UnixFileIdentity::from_metadata(&metadata) != self.context_identity
        {
            return Err(V2BodyStoreError::StorageBinding {
                path: self.context_path.clone(),
                reason: "body-store context changed while reopening its descriptor",
            });
        }
        self.verify_linked()?;
        Ok(file)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn inventory(
        &self,
        maximum_entries: usize,
    ) -> Result<Vec<BoundBodyStoreLeaf>, V2BodyStoreError> {
        use std::os::unix::ffi::OsStrExt as _;

        let context = self.fresh_context_handle()?;
        let entries = rustix::fs::Dir::read_from(&context)
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: self.context_path.clone(),
                source,
            })?;
        let mut leaves = Vec::new();
        for entry in entries {
            let entry =
                entry
                    .map_err(std::io::Error::from)
                    .map_err(|source| V2BodyStoreError::Io {
                        path: self.context_path.clone(),
                        source,
                    })?;
            let name = OsStr::from_bytes(entry.file_name().to_bytes());
            if matches!(name.as_bytes(), b"." | b"..") {
                continue;
            }
            if leaves.len() >= maximum_entries {
                return Err(V2BodyStoreError::DirectoryEntryCapacityExceeded {
                    capacity: maximum_entries,
                });
            }
            let name = name.to_os_string();
            let path = self.context_path.join(&name);
            let stat = rustix::fs::statat(&context, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)
                .map_err(|source| V2BodyStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
            if rustix::fs::FileType::from_raw_mode(stat.st_mode)
                != rustix::fs::FileType::RegularFile
                || stat.st_nlink as u64 != 1
                || stat.st_size < 0
            {
                return Err(V2BodyStoreError::UnexpectedEntry(path));
            }
            leaves.push(BoundBodyStoreLeaf {
                name,
                identity: UnixFileIdentity::from_stat(&stat),
                length: u64::try_from(stat.st_size)
                    .map_err(|_| V2BodyStoreError::UnexpectedEntry(path))?,
            });
        }
        self.verify_linked()?;
        Ok(leaves)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn inspect_leaf(
        &self,
        name: &OsStr,
        kind: FramePayloadKind,
    ) -> Result<BoundBodyStoreLeaf, V2BodyStoreError> {
        self.verify_linked()?;
        let path = self.context_path.join(name);
        let stat = rustix::fs::statat(&self.context, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: path.clone(),
                source,
            })?;
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
            || stat.st_nlink as u64 != 1
            || stat.st_size < 0
        {
            return Err(V2BodyStoreError::UnexpectedEntry(path));
        }
        let length = u64::try_from(stat.st_size).map_err(|_| kind.too_large())?;
        if length > kind.maximum_frame_bytes()? {
            return Err(kind.too_large());
        }
        Ok(BoundBodyStoreLeaf {
            name: name.to_os_string(),
            identity: UnixFileIdentity::from_stat(&stat),
            length,
        })
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_leaf(
        &self,
        leaf: &BoundBodyStoreLeaf,
        kind: FramePayloadKind,
    ) -> Result<File, V2BodyStoreError> {
        let current = self.inspect_leaf(&leaf.name, kind)?;
        let path = self.context_path.join(&leaf.name);
        if current.identity != leaf.identity || current.length != leaf.length {
            return Err(V2BodyStoreError::StorageBinding {
                path,
                reason: "body-store leaf changed between inventory and open",
            });
        }
        let file = File::from(
            rustix::fs::openat(
                &self.context,
                &leaf.name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: path.clone(),
                source,
            })?,
        );
        self.verify_leaf(&file, &leaf.name, Some(leaf))?;
        self.verify_linked()?;
        Ok(file)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_named_leaf(
        &self,
        name: &OsStr,
        kind: FramePayloadKind,
    ) -> Result<File, V2BodyStoreError> {
        let leaf = self.inspect_leaf(name, kind)?;
        self.open_leaf(&leaf, kind)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_leaf(
        &self,
        file: &File,
        name: &OsStr,
        expected: Option<&BoundBodyStoreLeaf>,
    ) -> Result<(), V2BodyStoreError> {
        use std::os::unix::fs::MetadataExt as _;

        let path = self.context_path.join(name);
        let opened = file.metadata().map_err(|source| V2BodyStoreError::Io {
            path: path.clone(),
            source,
        })?;
        let linked = rustix::fs::statat(&self.context, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: path.clone(),
                source,
            })?;
        let opened_identity = UnixFileIdentity::from_metadata(&opened);
        let linked_identity = UnixFileIdentity::from_stat(&linked);
        if !opened.is_file()
            || opened.nlink() != 1
            || rustix::fs::FileType::from_raw_mode(linked.st_mode)
                != rustix::fs::FileType::RegularFile
            || linked.st_nlink as u64 != 1
            || opened_identity != linked_identity
            || expected.is_some_and(|expected| {
                expected.identity != opened_identity || expected.length != opened.len()
            })
        {
            return Err(V2BodyStoreError::StorageBinding {
                path,
                reason: "body-store leaf changed while open",
            });
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn unlink_leaf(
        &self,
        leaf: &BoundBodyStoreLeaf,
        kind: FramePayloadKind,
    ) -> Result<(), V2BodyStoreError> {
        let file = self.open_leaf(leaf, kind)?;
        self.verify_leaf(&file, &leaf.name, Some(leaf))?;
        rustix::fs::unlinkat(&self.context, &leaf.name, rustix::fs::AtFlags::empty())
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: self.context_path.join(&leaf.name),
                source,
            })?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn publish_atomic(
        &self,
        destination: &OsStr,
        bytes: &[u8],
        kind: FramePayloadKind,
    ) -> Result<(), V2BodyStoreError> {
        if u64::try_from(bytes.len()).map_err(|_| kind.too_large())? > kind.maximum_frame_bytes()? {
            return Err(kind.too_large());
        }
        self.verify_linked()?;
        let mut temporary = destination.to_os_string();
        temporary.push(".tmp");
        let temporary_path = self.context_path.join(&temporary);
        let mut file = File::from(
            rustix::fs::openat(
                &self.context,
                &temporary,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .map_err(std::io::Error::from)
            .map_err(|source| V2BodyStoreError::Io {
                path: temporary_path.clone(),
                source,
            })?,
        );
        let opened = file.metadata().map_err(|source| V2BodyStoreError::Io {
            path: temporary_path.clone(),
            source,
        })?;
        let leaf = BoundBodyStoreLeaf {
            name: temporary.clone(),
            identity: UnixFileIdentity::from_metadata(&opened),
            length: u64::try_from(bytes.len()).map_err(|_| kind.too_large())?,
        };
        let publication = (|| {
            self.verify_leaf(&file, &temporary, None)?;
            file.write_all(bytes)
                .and_then(|()| file.flush())
                .and_then(|()| file.sync_all())
                .map_err(|source| V2BodyStoreError::Io {
                    path: temporary_path.clone(),
                    source,
                })?;
            self.verify_leaf(&file, &temporary, Some(&leaf))?;
            self.verify_linked()?;
            rename_body_store_leaf_noreplace(&self.context, &temporary, destination).map_err(
                |source| {
                    if source.kind() == std::io::ErrorKind::AlreadyExists {
                        V2BodyStoreError::AtomicDestinationExists(
                            self.context_path.join(destination),
                        )
                    } else {
                        V2BodyStoreError::Io {
                            path: self.context_path.join(destination),
                            source,
                        }
                    }
                },
            )?;
            self.verify_leaf(&file, destination, Some(&leaf))?;
            self.sync_context()?;
            self.verify_leaf(&file, destination, Some(&leaf))
        })();
        if publication.is_err()
            && rustix::fs::statat(
                &self.context,
                &temporary,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .is_ok_and(|stat| UnixFileIdentity::from_stat(&stat) == leaf.identity)
        {
            let _ = rustix::fs::unlinkat(&self.context, &temporary, rustix::fs::AtFlags::empty());
            let _ = self.context.sync_all();
        }
        publication
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn publish_atomic(
        &self,
        _destination: &OsStr,
        _bytes: &[u8],
        _kind: FramePayloadKind,
    ) -> Result<(), V2BodyStoreError> {
        Err(V2BodyStoreError::UnsupportedStorageBinding {
            path: self.context_path.clone(),
        })
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn sync_context(&self) -> Result<(), V2BodyStoreError> {
        self.verify_linked()?;
        self.context
            .sync_all()
            .map_err(|source| V2BodyStoreError::Io {
                path: self.context_path.clone(),
                source,
            })?;
        self.verify_linked()
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn sync_body_root(&self) -> Result<(), V2BodyStoreError> {
        self.verify_linked()?;
        self.body_root
            .sync_all()
            .map_err(|source| V2BodyStoreError::Io {
                path: self.body_root_path.clone(),
                source,
            })?;
        self.verify_linked()
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn retire_contents(&self, stop_after_marker_sync: bool) -> Result<(), V2BodyStoreError> {
        let mut temporary = Vec::new();
        let mut markers = Vec::new();
        let mut bodies = Vec::new();
        for leaf in self.inventory(
            BODY_STORE_DIRECTORY_ENTRIES_PER_BODY
                .saturating_mul(V2_MAX_LIFECYCLE_RECORDS_PER_HEIGHT),
        )? {
            let Some(name) = leaf.name.to_str() else {
                return Err(V2BodyStoreError::UnexpectedEntry(
                    self.context_path.join(&leaf.name),
                ));
            };
            if name.ends_with(".norito.tmp") {
                if !body_store_leaf_has_canonical_name(name, ".norito.tmp") {
                    return Err(V2BodyStoreError::UnexpectedEntry(
                        self.context_path.join(&leaf.name),
                    ));
                }
                if leaf.length > FramePayloadKind::Body.maximum_frame_bytes()? {
                    return Err(V2BodyStoreError::BodyTooLarge);
                }
                temporary.push((leaf, FramePayloadKind::Body));
            } else if name.ends_with(".validated.tmp") {
                if !body_store_leaf_has_canonical_name(name, ".validated.tmp") {
                    return Err(V2BodyStoreError::UnexpectedEntry(
                        self.context_path.join(&leaf.name),
                    ));
                }
                if leaf.length > FramePayloadKind::ValidationOutcomeMarker.maximum_frame_bytes()? {
                    return Err(V2BodyStoreError::ValidationMarkerTooLarge);
                }
                temporary.push((leaf, FramePayloadKind::ValidationOutcomeMarker));
            } else if name.ends_with(".validated") {
                if !body_store_leaf_has_canonical_name(name, ".validated") {
                    return Err(V2BodyStoreError::UnexpectedEntry(
                        self.context_path.join(&leaf.name),
                    ));
                }
                if leaf.length > FramePayloadKind::ValidationOutcomeMarker.maximum_frame_bytes()? {
                    return Err(V2BodyStoreError::ValidationMarkerTooLarge);
                }
                markers.push(leaf);
            } else if name.ends_with(".norito") {
                if !body_store_leaf_has_canonical_name(name, ".norito") {
                    return Err(V2BodyStoreError::UnexpectedEntry(
                        self.context_path.join(&leaf.name),
                    ));
                }
                if leaf.length > FramePayloadKind::Body.maximum_frame_bytes()? {
                    return Err(V2BodyStoreError::BodyTooLarge);
                }
                bodies.push(leaf);
            } else {
                return Err(V2BodyStoreError::UnexpectedEntry(
                    self.context_path.join(&leaf.name),
                ));
            }
        }
        temporary.sort_by(|left, right| left.0.name.cmp(&right.0.name));
        markers.sort_by(|left, right| left.name.cmp(&right.name));
        bodies.sort_by(|left, right| left.name.cmp(&right.name));
        for (leaf, kind) in &temporary {
            self.unlink_leaf(leaf, *kind)?;
        }
        for leaf in &markers {
            self.unlink_leaf(leaf, FramePayloadKind::ValidationOutcomeMarker)?;
        }
        self.sync_context()?;
        if stop_after_marker_sync {
            return Ok(());
        }
        for leaf in &bodies {
            self.unlink_leaf(leaf, FramePayloadKind::Body)?;
        }
        self.sync_context()?;
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn retire(self) -> Result<(), V2BodyStoreError> {
        self.retire_contents(false)?;
        if !self.inventory(1)?.is_empty() {
            return Err(V2BodyStoreError::StorageBinding {
                path: self.context_path.clone(),
                reason: "body-store context changed during retirement",
            });
        }
        self.verify_linked()?;
        rustix::fs::unlinkat(
            &self.body_root,
            &self.context_name,
            rustix::fs::AtFlags::REMOVEDIR,
        )
        .map_err(std::io::Error::from)
        .map_err(|source| V2BodyStoreError::Io {
            path: self.context_path.clone(),
            source,
        })?;
        self.body_root
            .sync_all()
            .map_err(|source| V2BodyStoreError::Io {
                path: self.body_root_path.clone(),
                source,
            })?;
        self.verify_body_root_linked()
    }

    #[cfg(all(test, unix, not(target_os = "espidf")))]
    fn simulate_crash_after_marker_sync_for_test(self) -> Result<(), V2BodyStoreError> {
        self.retire_contents(true)
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn retire(self) -> Result<(), V2BodyStoreError> {
        Err(V2BodyStoreError::UnsupportedStorageBinding {
            path: self.context_path,
        })
    }
}

#[cfg(all(
    unix,
    not(target_os = "espidf"),
    any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    )
))]
fn rename_body_store_leaf_noreplace(
    directory: &File,
    source: &OsStr,
    destination: &OsStr,
) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        directory,
        source,
        directory,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(all(
    unix,
    not(target_os = "espidf"),
    not(any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))
))]
fn rename_body_store_leaf_noreplace(
    _directory: &File,
    _source: &OsStr,
    _destination: &OsStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace body-store publication is unavailable",
    ))
}

/// Persistent exact-body store for one immutable height context.
///
/// The validation snapshot is part of that height ownership contract: callers
/// must not change world state or context-bound validation inputs while this
/// store is active. A committed-parent change retires the store and creates a
/// new height context. Validation receipts remain bound to one exact proposal
/// round and are never promoted across views.
pub(crate) struct V2BodyStore {
    identity: Arc<V2BodyStoreInstanceIdentityMarker>,
    context: wire::HeightContext,
    signature_policy: BlockSignaturePolicy,
    directory: PathBuf,
    bound_directory: Option<BoundV2BodyContextDirectory>,
    capacity: V2BodyStoreCapacity,
    body_frame_bytes: u64,
    /// Emergency Fast owners are empty and may neither publish nor retire files.
    emergency_read_only: bool,
    entries: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt>,
    manifests: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), wire::PayloadManifest>,
    /// Structurally authenticated restart markers which have not yet crossed
    /// deterministic candidate validation in this process.
    ///
    /// A checksum only detects accidental corruption; it is not authority to
    /// vote. Production preflight must promote every entry through
    /// [`Self::revalidate_recovered_markers`] before constructing the runtime.
    pending_revalidation:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), QuarantinedValidationOutcome>,
    /// Outcome seals retired from restart authority by a missing sidecar or
    /// because authenticated WAL/finality replay did not name their key.
    ///
    /// These entries are comparison-only: an ordinary bounded retry must run
    /// the validator again and reproduce the exact durable outcome before it
    /// can promote one. Rejection identities are exposed only through a
    /// deny-only catalog so a fresh local producer cannot launder an already
    /// rejected pre-intent body into Proposal authority.
    retired_revalidation:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), QuarantinedValidationOutcome>,
    validated: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt>,
    /// Semantically revalidated deterministic rejections retained for the
    /// body-store-instance-bound terminal Validate recovery join. The raw
    /// diagnostic string remains non-authoritative.
    rejected: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), RevalidatedRejectedBody>,
}
/// Move-only same-store input accepted by unified production lifecycle startup.
///
/// Construction consumes an already-open body store only after every recovered
/// terminal marker has crossed deterministic semantic replay. There is no
/// production constructor from a filesystem root and no raw parts API: the
/// unified owner must consume this exact instance or startup stops.
#[must_use = "the revalidated body-store cut must enter unified lifecycle startup"]
pub(crate) struct RevalidatedV2BodyStore(V2BodyStore);
/// Move-only freshly quarantined body-store input for recovered startup.
///
/// This cut can be minted only while no validation marker has already been
/// promoted, rejected, or retired in the open instance. Checksummed disk
/// markers may remain quarantined for the unified factory's exact semantic
/// replay, but a caller cannot prevalidate them with another callback and then
/// bypass that replay.
#[must_use = "the quarantined body-store cut must enter recovered lifecycle startup"]
pub(in crate::sumeragi) struct QuarantinedV2BodyStore(V2BodyStore);
impl QuarantinedV2BodyStore {
    /// Compare the still-owned store with one canonical lifecycle layout.
    pub(in crate::sumeragi) fn matches_lifecycle_storage_root(
        &self,
        root: &Path,
        context: &wire::HeightContext,
        signature_policy: &BlockSignaturePolicy,
    ) -> bool {
        self.0
            .matches_lifecycle_storage_root(root, context, signature_policy)
    }
    /// Filter and replay every quarantined marker with one exact Apply service.
    ///
    /// This is the only operation that can consume the recovered-startup
    /// quarantine. It fixes the authority order internally so no sibling can
    /// inject a callback, pre-promote a marker, or extract an intermediate
    /// store before semantic replay is sealed.
    pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(
        mut self,
        apply_service: &V2ApplyService,
        context: &wire::HeightContext,
        validation_authority: RecoveredValidationAuthority,
    ) -> Result<RevalidatedV2BodyStore, V2ApplyError> {
        if let Some(subject) = apply_service.recovered_finality_subject(context)? {
            self.0.retain_recovered_markers_for_subject(subject)?;
        }
        self.0
            .retain_recovered_markers_for_authority(validation_authority)?;
        self.0.revalidate_recovered_markers(|body| {
            apply_service.revalidate_recovered_candidate(context, body)
        })?;
        self.0.into_revalidated_startup().map_err(Into::into)
    }
}
impl RevalidatedV2BodyStore {
    /// Compare the complete immutable context without releasing the store.
    pub(in crate::sumeragi) fn matches_context(&self, context: &wire::HeightContext) -> bool {
        &self.0.context == context
    }
    /// Compare the still-owned store with one canonical context-addressed root.
    ///
    /// This is a fixed boolean oracle: neither the opened directory nor the
    /// signature policy crosses the sealed startup handoff. Production uses it
    /// to prove that the body owner came from the same Kura-derived storage
    /// layout as the lifecycle ledger before either owner is opened.
    #[allow(
        dead_code,
        reason = "reviewed comparison seam retained for revalidated-startup diagnostics"
    )]
    pub(in crate::sumeragi) fn matches_lifecycle_storage_root(
        &self,
        root: &Path,
        context: &wire::HeightContext,
        signature_policy: &BlockSignaturePolicy,
    ) -> bool {
        &self.0.context == context
            && &self.0.signature_policy == signature_policy
            && self.0.directory == root.join(hex::encode(context.id().0.as_ref()))
    }
    /// Return a comparison-only identity for this exact still-owned store.
    #[cfg(all(test, feature = "bls"))]
    pub(in crate::sumeragi) fn instance_identity(&self) -> V2BodyStoreInstanceIdentity {
        self.0.instance_identity()
    }
    /// Return whether this revalidated store owns the exact successful marker
    /// needed to replace a recovered Decision Fetch with Apply.
    pub(in crate::sumeragi) fn has_exact_recovered_decision_fetch_parent(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        self.0.has_exact_recovered_decision_fetch_parent(projection)
    }
    /// Return whether deterministic replay rejected the body named by a
    /// recovered Decision.
    pub(in crate::sumeragi) fn has_rejected_recovered_decision_body(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        self.0.has_rejected_recovered_decision_body(projection)
    }
    /// Detach the exact successful body frame named by a recovered Decision.
    ///
    /// Every fallible context, marker, index, manifest, checksum, canonical
    /// wire, and signature check completes before any in-memory value moves.
    /// Deterministic rejection is a distinct fatal classification and never
    /// becomes either Fetch or Apply authority.
    pub(in crate::sumeragi) fn detach_recovered_decision_apply_body(
        &mut self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<RecoveredDecisionApplyBodyCut<'_>, RecoveredDecisionApplyBodyCutError> {
        self.0
            .ensure_recovered_markers_revalidated()
            .map_err(|error| {
                debug_assert!(matches!(
                    error,
                    V2BodyStoreError::UnrevalidatedValidationMarkers
                ));
                RecoveredDecisionApplyBodyCutError::UnrevalidatedMarkers
            })?;
        let lifecycle_context = super::v2_lifecycle_coordinator::lifecycle_context(&self.0.context);
        if !projection.belongs_to_context(lifecycle_context) {
            return Err(RecoveredDecisionApplyBodyCutError::ForeignContext);
        }
        if self
            .0
            .rejected
            .values()
            .any(|rejected| projection.matches_durable_body(&rejected.durable))
        {
            return Err(RecoveredDecisionApplyBodyCutError::DeterministicRejection);
        }
        let mut matching = self
            .0
            .validated
            .iter()
            .filter(|(_, validated)| projection.matches_validated_body(validated));
        let Some((key, validated)) = matching.next() else {
            return Err(RecoveredDecisionApplyBodyCutError::MissingMarker);
        };
        if matching.next().is_some() {
            return Err(RecoveredDecisionApplyBodyCutError::AmbiguousMarker);
        }
        let key = *key;
        let durable = self
            .0
            .entries
            .get(&key)
            .ok_or(V2BodyStoreError::ReceiptMismatch)
            .map_err(RecoveredDecisionApplyBodyCutError::BodyStore)?;
        let manifest = self
            .0
            .manifests
            .get(&key)
            .ok_or(V2BodyStoreError::ReceiptMismatch)
            .map_err(RecoveredDecisionApplyBodyCutError::BodyStore)?;
        if validated.durable() != durable
            || key != (durable.round(), durable.subject())
            || manifest.round != durable.round()
            || manifest.subject != durable.subject()
            || HashOf::new(manifest) != durable.manifest_hash()
        {
            return Err(RecoveredDecisionApplyBodyCutError::BodyStore(
                V2BodyStoreError::ReceiptMismatch,
            ));
        }
        let envelope = self
            .0
            .load_envelope(durable)
            .map_err(RecoveredDecisionApplyBodyCutError::BodyStore)?;
        if envelope.manifest != *manifest
            || envelope.context_id != self.0.context.id()
            || !projection.matches_validated_body(validated)
        {
            return Err(RecoveredDecisionApplyBodyCutError::BodyStore(
                V2BodyStoreError::ReceiptMismatch,
            ));
        }
        let store_identity = self.0.instance_identity();
        let context = self.0.context.clone();
        let durable = self
            .0
            .entries
            .remove(&key)
            .expect("the exact Decision durable frame was checked before detach");
        let manifest = self
            .0
            .manifests
            .remove(&key)
            .expect("the exact Decision manifest was checked before detach");
        let validated = self
            .0
            .validated
            .remove(&key)
            .expect("the exact Decision marker was checked before detach");
        Ok(RecoveredDecisionApplyBodyCut {
            store: &mut self.0,
            store_identity,
            context,
            key,
            envelope,
            durable: Some(durable),
            manifest: Some(manifest),
            validated: Some(validated),
        })
    }
    /// Consume the sealed instance into the unified production owner.
    ///
    /// The expected context comparison is repeated at the transfer boundary so
    /// a revalidated store opened for another height cannot be substituted.
    pub(in crate::sumeragi) fn into_lifecycle_owner_store(
        self,
        expected_context: &wire::HeightContext,
    ) -> Result<V2BodyStore, V2BodyStoreError> {
        self.0.ensure_recovered_markers_revalidated()?;
        if &self.0.context != expected_context {
            return Err(V2BodyStoreError::ContextMismatch);
        }
        Ok(self.0)
    }
}
#[derive(Debug)]
struct V2BodyStoreInstanceIdentityMarker;
/// Comparison-only process identity for one exact open body-store instance.
///
/// This seal is not durable authority. It only proves that a lifecycle owner
/// and its height-local I/O worker refer to the same move-only store instance
/// during the live process transition.
#[derive(Clone, Debug)]
pub(crate) struct V2BodyStoreInstanceIdentity(Arc<V2BodyStoreInstanceIdentityMarker>);
impl V2BodyStoreInstanceIdentity {
    /// Return whether both seals were projected from the same open store.
    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
/// Move-only authority for one exact Certified-Serve body read from its live
/// body-store instance.
///
/// Construction remains inside [`V2BodyStore`]. The owned receipt and bytes
/// have therefore crossed the store's exact receipt, frame checksum, manifest,
/// and canonical body validation before a worker can return them to the
/// lifecycle owner. The comparison-only instance seal prevents an otherwise
/// valid readback from a different open store from settling this owner's work.
#[derive(Debug)]
#[must_use = "a Certified-Serve body readback must remain attached to worker completion"]
pub(in crate::sumeragi) struct DurableCertifiedServeBodyReadbackV1 {
    durable_body: DurableBodyReceipt,
    canonical_wire: Vec<u8>,
    store_identity: V2BodyStoreInstanceIdentity,
}
impl DurableCertifiedServeBodyReadbackV1 {
    /// Borrow the exact durable receipt revalidated by the worker-owned store.
    pub(in crate::sumeragi) const fn durable_body(&self) -> &DurableBodyReceipt {
        &self.durable_body
    }

    /// Borrow the exact canonical `SignedBlockWire` bytes loaded from the frame.
    pub(in crate::sumeragi) fn canonical_wire(&self) -> &[u8] {
        &self.canonical_wire
    }

    /// Compare this readback with the lifecycle owner's retained store seal.
    pub(in crate::sumeragi) fn matches_store_instance(
        &self,
        expected: &V2BodyStoreInstanceIdentity,
    ) -> bool {
        self.store_identity.same_instance(expected)
    }
}
/// Body-store-private one-shot permit for binding a cold adapter body lookup.
///
/// Construction stays in this module so another recovery path cannot bind a
/// preview to a caller-selected process identity.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyColdPreviewBindPermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyColdPreviewBindPermitLinearityV1,
}
struct RecoveredLifecycleNextVoteBodyColdPreviewBindPermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyColdPreviewBindPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyColdPreviewBindPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyColdPreviewBindPermitLinearityV1,
        }
    }
}
/// Body-store-private one-shot permit for promoting an exact cold body join.
///
/// The adapter accepts this permit only beside the lookup and receipt selected
/// from one already-revalidated store's private catalogs.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitLinearityV1,
}
struct RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitLinearityV1,
        }
    }
}
/// Body-store-private permit for retaining canonical cold Proposal output.
///
/// The same revalidated store that authenticates the next Vote constructs this
/// permit, preventing another sibling from attaching caller-selected chunks or
/// a foreign process identity to the durable Broadcast carrier.
pub(in crate::sumeragi) struct RecoveredLifecycleColdProposalOutputMintPermitV1 {
    _linearity: RecoveredLifecycleColdProposalOutputMintPermitLinearityV1,
}
struct RecoveredLifecycleColdProposalOutputMintPermitLinearityV1;
impl Drop for RecoveredLifecycleColdProposalOutputMintPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleColdProposalOutputMintPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleColdProposalOutputMintPermitLinearityV1,
        }
    }
}
/// Immutable post-finality deletion authority for one exact body directory.
///
/// Construction consumes the height-local store only after a matching Kura
/// receipt is available. Executing the plan may block on the filesystem and
/// therefore belongs exclusively to the runner's bounded cleanup worker.
pub(crate) struct V2BodyRetirementJob {
    directory: BoundV2BodyContextDirectory,
}
impl V2BodyRetirementJob {
    /// Delete the finalized height's durable candidate bodies and fsync the
    /// containing directory.
    pub(crate) fn execute(self) -> Result<(), V2BodyStoreError> {
        self.directory.retire()
    }
}
impl V2BodyStore {
    fn ensure_mutable(&self) -> Result<(), V2BodyStoreError> {
        if self.emergency_read_only {
            return Err(V2BodyStoreError::EmergencyFastReadOnly);
        }
        Ok(())
    }

    /// Consume a freshly opened store into recovered-startup quarantine.
    ///
    /// Pending disk markers are allowed because the unified factory must
    /// filter and replay them. Any already promoted, rejected, or retired
    /// marker proves that another validator has touched this open instance and
    /// therefore makes it ineligible for the production recovery boundary.
    pub(in crate::sumeragi) fn into_quarantined_recovered_startup(
        self,
    ) -> Result<QuarantinedV2BodyStore, V2BodyStoreError> {
        if !self.validated.is_empty()
            || !self.rejected.is_empty()
            || !self.retired_revalidation.is_empty()
        {
            return Err(V2BodyStoreError::RecoveredMarkersAlreadyPromoted);
        }
        Ok(QuarantinedV2BodyStore(self))
    }
    /// Project a comparison-only identity before moving this store to its worker.
    pub(crate) fn instance_identity(&self) -> V2BodyStoreInstanceIdentity {
        V2BodyStoreInstanceIdentity(Arc::clone(&self.identity))
    }
    /// Return whether this is the inert emergency owner which skipped disk inventory.
    pub(in crate::sumeragi) const fn emergency_read_only(&self) -> bool {
        self.emergency_read_only
    }
    /// Authenticate one cold reducer-produced next Vote against this store.
    ///
    /// The preview receives this store's process identity only through a
    /// private affine permit. Exactly one semantically revalidated success
    /// marker must match, and the validated, durable, and manifest catalogs
    /// must all retain the same key and values. No receipt, lookup coordinate,
    /// or raw catalog leaves this boundary.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn authenticate_recovered_lifecycle_next_vote_body(
        &self,
        preview: &mut PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1,
    ) -> Result<RecoveredLifecycleNextVoteBodyAuthorityV1, V2BodyStoreError> {
        self.ensure_recovered_markers_revalidated()?;
        let body_store_identity = self.instance_identity();
        let lookup = preview
            .project_next_vote_body_lookup(
                RecoveredLifecycleNextVoteBodyColdPreviewBindPermitV1::new(),
                body_store_identity.clone(),
            )
            .map_err(|_| V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)?;
        if !lookup.matches_height_context(&self.context) {
            return Err(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch);
        }
        let mut exact = self
            .validated
            .values()
            .filter(|validated| lookup.matches_validated_body(validated));
        let validated = exact
            .next()
            .cloned()
            .ok_or(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)?;
        if exact.next().is_some() {
            return Err(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch);
        }
        let durable = validated.durable();
        let key = (durable.round(), durable.subject());
        let manifest = self
            .manifests
            .get(&key)
            .ok_or(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)?;
        if self.validated.get(&key) != Some(&validated)
            || self.entries.get(&key) != Some(durable)
            || self.rejected.contains_key(&key)
            || HashOf::new(manifest) != durable.manifest_hash()
            || !lookup.matches_recovered_body(manifest, durable)
        {
            return Err(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch);
        }
        let canonical_wire = self.load_canonical_wire(durable)?;
        let payload = encode_payload(
            &self.context,
            durable.round(),
            durable.subject(),
            &canonical_wire,
        )
        .map_err(|_| V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)?;
        if payload.manifest() != manifest {
            return Err(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch);
        }
        preview
            .bind_cold_proposal_output(
                RecoveredLifecycleColdProposalOutputMintPermitV1::new(),
                payload,
                body_store_identity.clone(),
            )
            .map_err(|_| V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)?;
        RecoveredLifecycleNextVoteBodyAuthorityV1::from_exact_revalidated_body_store(
            RecoveredLifecycleNextVoteBodyColdAuthorityMintPermitV1::new(),
            lookup,
            validated,
            body_store_identity,
        )
        .ok_or(V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)
    }
    /// Return whether this already-open store belongs to the exact context.
    ///
    /// Production startup uses this when a durable recovery catalog must be
    /// inspected before the serialized runtime is constructed.
    pub(crate) fn matches_context(&self, context: &wire::HeightContext) -> bool {
        &self.context == context
    }
    /// Authenticate one exact recovered frame under the private genesis policy.
    pub(in crate::sumeragi) fn authenticate_genesis_authority_frame(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Option<AuthenticatedGenesisBodyStoreFrameV1> {
        (self.context.height == 1
            && self.context.parent_commit_qc.is_none()
            && self.context.snapshot_bootstrap.is_none()
            && matches!(
                &self.signature_policy,
                BlockSignaturePolicy::GenesisAuthority(_)
            )
            && self.owns_receipt(receipt)
            && self.load_envelope(receipt).is_ok())
        .then(|| AuthenticatedGenesisBodyStoreFrameV1 {
            receipt: receipt.clone(),
        })
    }
    /// Compare this exact opened store with one sealed lifecycle storage root.
    ///
    /// The caller receives only a boolean. The context-addressed directory and
    /// signature policy remain private to the store, so this cannot be used to
    /// reconstruct or redirect body ownership.
    pub(in crate::sumeragi) fn matches_lifecycle_storage_root(
        &self,
        root: &Path,
        context: &wire::HeightContext,
        signature_policy: &BlockSignaturePolicy,
    ) -> bool {
        &self.context == context
            && &self.signature_policy == signature_policy
            && self.directory == root.join(hex::encode(context.id().0.as_ref()))
    }
    /// Compare a receipt with the exact in-memory entry owned by this open store.
    ///
    /// This deliberately performs no filesystem I/O. A caller can therefore
    /// distinguish an unowned receipt, which is rejected before accepting any
    /// storage authority, from corruption discovered while reloading a receipt
    /// that this store already accepted and indexed.
    pub(super) fn owns_receipt(&self, receipt: &DurableBodyReceipt) -> bool {
        receipt.context_id == self.context.id()
            && receipt.round.context_id == receipt.context_id
            && receipt.round.height == self.context.height
            && self
                .entries
                .get(&(receipt.round, receipt.subject))
                .is_some_and(|known| known == receipt)
    }
    /// Replace an accepted frame after its receipt was minted.
    #[cfg(test)]
    pub(super) fn corrupt_owned_frame_for_test(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<(), V2BodyStoreError> {
        if !self.owns_receipt(receipt) {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let path = self.path_for(receipt.round, receipt.subject);
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|source| V2BodyStoreError::Io {
                path: path.clone(),
                source,
            })?;
        file.write_all(b"corrupt accepted body frame")
            .and_then(|()| file.sync_all())
            .map_err(|source| V2BodyStoreError::Io { path, source })
    }
    /// Open the active context directory and fail closed on every malformed
    /// final body file. Incomplete atomic-write files are unacknowledged and
    /// are removed after bounded inventory.
    #[cfg(test)]
    pub(crate) fn open(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
    ) -> Result<Self, V2BodyStoreError> {
        Self::open_with_policy(root, context, BlockSignaturePolicy::RotatingLeader)
    }
    /// Open a context directory with an explicit block-signature policy.
    ///
    /// The genesis-authority policy is valid only for the first height with no
    /// parent certificate. All successor heights use rotating-leader signing.
    #[cfg(test)]
    pub(crate) fn open_with_policy(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
    ) -> Result<Self, V2BodyStoreError> {
        let capacity = V2BodyStoreCapacity::new(DEFAULT_V2_BODY_STORE_MAX_BYTES_PER_HEIGHT)?;
        Self::open_with_policy_and_capacity(root, context, signature_policy, capacity)
    }
    /// Open a context directory with an explicit signature policy and capacity.
    #[cfg(test)]
    pub(crate) fn open_with_policy_and_capacity(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
        capacity: V2BodyStoreCapacity,
    ) -> Result<Self, V2BodyStoreError> {
        Self::validate_open_parameters(&context, &signature_policy)?;
        let context_name = OsString::from(hex::encode(context.id().0.as_ref()));
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let bound = BoundV2BodyContextDirectory::bind_test_root(root.as_ref(), context_name)?;
            return Self::open_bound(bound, context, signature_policy, capacity);
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = context_name;
            Err(V2BodyStoreError::UnsupportedStorageBinding {
                path: root.as_ref().to_path_buf(),
            })
        }
    }

    /// Open one context below a descriptor-bound body root minted by Kura.
    pub(crate) fn open_with_kura_authority_and_capacity(
        kura: &crate::kura::Kura,
        authority: crate::kura::KuraV2BodyStoreDirectoryAuthority,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
        capacity: V2BodyStoreCapacity,
    ) -> Result<Self, V2BodyStoreError> {
        Self::validate_open_parameters(&context, &signature_policy)?;
        let context_name = OsString::from(hex::encode(context.id().0.as_ref()));
        let bound =
            BoundV2BodyContextDirectory::from_kura_authority(kura, authority, context_name)?;
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            Self::open_bound(bound, context, signature_policy, capacity)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (bound, context, signature_policy, capacity);
            Err(V2BodyStoreError::UnsupportedStorageBinding {
                path: kura.sumeragi_v2_storage_root().join("bodies"),
            })
        }
    }

    fn validate_open_parameters(
        context: &wire::HeightContext,
        signature_policy: &BlockSignaturePolicy,
    ) -> Result<(), V2BodyStoreError> {
        context.validate()?;
        if matches!(signature_policy, BlockSignaturePolicy::GenesisAuthority(_))
            && (context.height != 1 || context.parent_commit_qc.is_some())
        {
            return Err(V2BodyStoreError::InvalidSignaturePolicy);
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_bound(
        bound_directory: BoundV2BodyContextDirectory,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
        capacity: V2BodyStoreCapacity,
    ) -> Result<Self, V2BodyStoreError> {
        let directory = bound_directory.context_path.clone();
        let mut store = Self {
            identity: Arc::new(V2BodyStoreInstanceIdentityMarker),
            context,
            signature_policy,
            directory,
            bound_directory: Some(bound_directory),
            capacity,
            body_frame_bytes: 0,
            emergency_read_only: false,
            entries: BTreeMap::new(),
            manifests: BTreeMap::new(),
            pending_revalidation: BTreeMap::new(),
            retired_revalidation: BTreeMap::new(),
            validated: BTreeMap::new(),
            rejected: BTreeMap::new(),
        };
        let mut body_frame_bytes = 0_u64;
        let mut body_leaves = Vec::new();
        let mut validation_marker_leaves = Vec::new();
        let mut temporary_leaves = Vec::new();
        let leaves = store
            .bound_directory
            .as_ref()
            .expect("mutable body store retains a bound directory")
            .inventory(capacity.max_directory_entries)?;
        for leaf in leaves {
            let path = store.directory.join(&leaf.name);
            let name = leaf
                .name
                .to_str()
                .ok_or_else(|| V2BodyStoreError::UnexpectedEntry(path.clone()))?;
            if name.ends_with(".norito.tmp") {
                if !body_store_leaf_has_canonical_name(name, ".norito.tmp") {
                    return Err(V2BodyStoreError::UnexpectedEntry(path));
                }
                if temporary_leaves.len() >= capacity.max_temporary_entries {
                    return Err(V2BodyStoreError::TemporaryEntryCapacityExceeded {
                        capacity: capacity.max_temporary_entries,
                    });
                }
                if leaf.length > FramePayloadKind::Body.maximum_frame_bytes()? {
                    return Err(V2BodyStoreError::BodyTooLarge);
                }
                temporary_leaves.push((leaf, FramePayloadKind::Body));
                continue;
            }
            if name.ends_with(".validated.tmp") {
                if !body_store_leaf_has_canonical_name(name, ".validated.tmp") {
                    return Err(V2BodyStoreError::UnexpectedEntry(path));
                }
                if temporary_leaves.len() >= capacity.max_temporary_entries {
                    return Err(V2BodyStoreError::TemporaryEntryCapacityExceeded {
                        capacity: capacity.max_temporary_entries,
                    });
                }
                if leaf.length > FramePayloadKind::ValidationOutcomeMarker.maximum_frame_bytes()? {
                    return Err(V2BodyStoreError::ValidationMarkerTooLarge);
                }
                temporary_leaves.push((leaf, FramePayloadKind::ValidationOutcomeMarker));
                continue;
            }
            match Path::new(name).extension().and_then(|value| value.to_str()) {
                Some("norito") => {
                    if !body_store_leaf_has_canonical_name(name, ".norito") {
                        return Err(V2BodyStoreError::UnexpectedEntry(path));
                    }
                    if body_leaves.len() >= capacity.max_body_entries {
                        return Err(V2BodyStoreError::BodyEntryCapacityExceeded {
                            capacity: capacity.max_body_entries,
                        });
                    }
                    if leaf.length > FramePayloadKind::Body.maximum_frame_bytes()? {
                        return Err(V2BodyStoreError::BodyTooLarge);
                    }
                    let next_body_frame_bytes = body_frame_bytes.checked_add(leaf.length).ok_or(
                        V2BodyStoreError::BodyByteCapacityExceeded {
                            capacity: capacity.max_body_frame_bytes,
                        },
                    )?;
                    if next_body_frame_bytes > capacity.max_body_frame_bytes {
                        return Err(V2BodyStoreError::BodyByteCapacityExceeded {
                            capacity: capacity.max_body_frame_bytes,
                        });
                    }
                    body_frame_bytes = next_body_frame_bytes;
                    body_leaves.push(leaf);
                }
                Some("validated") => {
                    if !body_store_leaf_has_canonical_name(name, ".validated") {
                        return Err(V2BodyStoreError::UnexpectedEntry(path));
                    }
                    if validation_marker_leaves.len() >= capacity.max_body_entries {
                        return Err(V2BodyStoreError::ValidationMarkerCapacityExceeded {
                            capacity: capacity.max_body_entries,
                        });
                    }
                    if leaf.length > VALIDATION_OUTCOME_MARKER_FRAME_MAX_BYTES {
                        return Err(V2BodyStoreError::ValidationMarkerTooLarge);
                    }
                    validation_marker_leaves.push(leaf);
                }
                _ => return Err(V2BodyStoreError::UnexpectedEntry(path)),
            }
        }
        if !temporary_leaves.is_empty() {
            temporary_leaves.sort_by(|left, right| left.0.name.cmp(&right.0.name));
            let bound = store
                .bound_directory
                .as_ref()
                .expect("mutable body store retains a bound directory");
            for (leaf, kind) in &temporary_leaves {
                bound.unlink_leaf(leaf, *kind)?;
            }
            bound.sync_context()?;
        }
        body_leaves.sort_by(|left, right| left.name.cmp(&right.name));
        for leaf in &body_leaves {
            let (envelope, frame_hash) = read_envelope_from_leaf(
                store
                    .bound_directory
                    .as_ref()
                    .expect("mutable body store retains a bound directory"),
                leaf,
            )?;
            store.validate_envelope(&envelope)?;
            if leaf.name != Self::file_name_for(envelope.round, envelope.subject) {
                return Err(V2BodyStoreError::UnexpectedEntry(
                    store.directory.join(&leaf.name),
                ));
            }
            let receipt = receipt_for(&envelope, frame_hash);
            let key = (envelope.round, envelope.subject);
            if store.entries.insert(key, receipt).is_some()
                || store.manifests.insert(key, envelope.manifest).is_some()
            {
                return Err(V2BodyStoreError::DuplicateBodyKey);
            }
        }
        validation_marker_leaves.sort_by(|left, right| left.name.cmp(&right.name));
        for leaf in &validation_marker_leaves {
            let marker = read_validation_outcome_marker_from_leaf(
                store
                    .bound_directory
                    .as_ref()
                    .expect("mutable body store retains a bound directory"),
                leaf,
            )?;
            if leaf.name != Self::validated_file_name_for(marker.round, marker.subject) {
                return Err(V2BodyStoreError::UnexpectedEntry(
                    store.directory.join(&leaf.name),
                ));
            }
            let key = (marker.round, marker.subject);
            let receipt = store
                .entries
                .get(&key)
                .cloned()
                .ok_or(V2BodyStoreError::OrphanedValidationMarker)?;
            store.validate_marker(&marker, &receipt)?;
            store.ensure_validation_outcome_consistent(&receipt, marker.outcome)?;
            if store
                .pending_revalidation
                .insert(
                    key,
                    QuarantinedValidationOutcome {
                        durable: receipt,
                        outcome: marker.outcome,
                    },
                )
                .is_some()
            {
                return Err(V2BodyStoreError::DuplicateValidationMarker);
            }
        }
        store.body_frame_bytes = body_frame_bytes;
        Ok(store)
    }
    /// Construct an empty, read-only owner without opening or inventorying the
    /// active context directory.
    ///
    /// Emergency Fast mode is locally passive, so recovered body authority is
    /// supplied only by the bounded WAL/finality joins. Ignored files remain
    /// untouched for a Strict restart, and every mutation entry point rejects
    /// this owner before it can replace one of those files.
    pub(crate) fn open_emergency_fast_read_only(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
    ) -> Result<Self, V2BodyStoreError> {
        context.validate()?;
        if matches!(signature_policy, BlockSignaturePolicy::GenesisAuthority(_))
            && (context.height != 1 || context.parent_commit_qc.is_some())
        {
            return Err(V2BodyStoreError::InvalidSignaturePolicy);
        }
        let capacity = V2BodyStoreCapacity::new(DEFAULT_V2_BODY_STORE_MAX_BYTES_PER_HEIGHT)?;
        Ok(Self {
            identity: Arc::new(V2BodyStoreInstanceIdentityMarker),
            directory: root.as_ref().join(hex::encode(context.id().0.as_ref())),
            bound_directory: None,
            context,
            signature_policy,
            capacity,
            body_frame_bytes: 0,
            emergency_read_only: true,
            entries: BTreeMap::new(),
            manifests: BTreeMap::new(),
            pending_revalidation: BTreeMap::new(),
            retired_revalidation: BTreeMap::new(),
            validated: BTreeMap::new(),
            rejected: BTreeMap::new(),
        })
    }
    /// Open an empty, context-addressed store for non-cryptographic lifecycle fixtures.
    ///
    /// Production must consume a Kura-minted directory authority through
    /// [`Self::open_with_kura_authority_and_capacity`]. This helper exists only
    /// because replay-authority unit fixtures deliberately retain structural
    /// certificates rather than usable voting signatures.
    #[cfg(test)]
    pub(in crate::sumeragi) fn open_lifecycle_fixture_for_test(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
    ) -> Result<Self, V2BodyStoreError> {
        let context_name = OsString::from(hex::encode(context.id().0.as_ref()));
        let bound_directory =
            BoundV2BodyContextDirectory::bind_test_root(root.as_ref(), context_name)?;
        let directory = bound_directory.context_path.clone();
        let capacity = V2BodyStoreCapacity::new(DEFAULT_V2_BODY_STORE_MAX_BYTES_PER_HEIGHT)?;
        Ok(Self {
            identity: Arc::new(V2BodyStoreInstanceIdentityMarker),
            context,
            signature_policy,
            directory,
            bound_directory: Some(bound_directory),
            capacity,
            body_frame_bytes: 0,
            emergency_read_only: false,
            entries: BTreeMap::new(),
            manifests: BTreeMap::new(),
            pending_revalidation: BTreeMap::new(),
            retired_revalidation: BTreeMap::new(),
            validated: BTreeMap::new(),
            rejected: BTreeMap::new(),
        })
    }
    /// Recover the durable receipt indexed by an exact round and subject.
    ///
    /// Receipts are reconstructed and fully revalidated while opening the
    /// store, so returning one here permits crash recovery without a needless
    /// network fetch.
    #[cfg(test)]
    pub(crate) fn receipt(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Option<DurableBodyReceipt> {
        self.entries.get(&(round, subject)).cloned()
    }
    /// Reload a recovered body's canonical manifest together with its durable
    /// receipt.
    ///
    /// The manifest is part of the reducer's subject registry. Returning it at
    /// the recovery boundary is essential when a replayed CommitQC requests a
    /// body without having first received the original proposal.
    pub(crate) fn recovered(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<Option<(wire::PayloadManifest, DurableBodyReceipt)>, V2BodyStoreError> {
        let key = (round, subject);
        let Some(receipt) = self.entries.get(&key).cloned() else {
            return Ok(None);
        };
        let manifest = self
            .manifests
            .get(&key)
            .cloned()
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        Ok(Some((manifest, receipt)))
    }
    /// Snapshot the in-memory recovery index reconstructed while opening.
    ///
    /// This performs no filesystem I/O and lets the serialized reducer owner
    /// recover local durability while the store itself moves to an asynchronous
    /// storage service.
    pub(crate) fn recovery_catalog(
        &self,
    ) -> Result<
        BTreeMap<
            (wire::ConsensusRound, wire::BlockSubject),
            (wire::PayloadManifest, DurableBodyReceipt),
        >,
        V2BodyStoreError,
    > {
        let mut catalog = BTreeMap::new();
        for (key, receipt) in &self.entries {
            let manifest = self
                .manifests
                .get(key)
                .cloned()
                .ok_or(V2BodyStoreError::ReceiptMismatch)?;
            catalog.insert(*key, (manifest, receipt.clone()));
        }
        Ok(catalog)
    }
    /// Re-run deterministic validation for every marker recovered from disk.
    ///
    /// Structurally valid marker bytes are deliberately quarantined while the
    /// store opens. This method promotes them atomically only after the exact
    /// durable bodies reproduce the exact persisted success commitment or
    /// deterministic rejection code. A typed missing-certified-sidecar result
    /// retires the affected marker authority without promoting it; the exact
    /// durable body remains available to the ordinary bounded validation and
    /// sidecar-fetch pipeline. Any success/rejection or commitment change fails
    /// closed. Bodies shared by several proposal rounds are executed once
    /// because validation consumes the signed body and immutable height
    /// context, not the manifest round; every round-local marker remains
    /// checked against that result.
    pub(crate) fn revalidate_recovered_markers<F, E>(
        &mut self,
        mut validator: F,
    ) -> Result<(), V2BodyStoreError>
    where
        F: FnMut(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        if self.pending_revalidation.is_empty() {
            return Ok(());
        }
        let mut replayed = BTreeMap::<wire::BlockSubject, SemanticReplayOutcome>::new();
        let mut promoted_validated = BTreeMap::new();
        let mut promoted_rejected = BTreeMap::new();
        let mut retired_missing_sidecar = BTreeMap::new();
        for (key, recovered) in &self.pending_revalidation {
            let receipt = self
                .entries
                .get(key)
                .ok_or(V2BodyStoreError::OrphanedValidationMarker)?;
            if &recovered.durable != receipt {
                return Err(V2BodyStoreError::ValidationMarkerMismatch);
            }
            let reproduced = if let Some(outcome) = replayed.get(&key.1) {
                outcome.clone()
            } else {
                let body = self.load(receipt)?;
                let outcome = match validator(&body) {
                    Ok(commitment) => {
                        commitment.validate()?;
                        SemanticReplayOutcome::Validated(commitment)
                    }
                    Err(error) if error.missing_certified_merge_sidecar().is_some() => {
                        SemanticReplayOutcome::DeferredMergeSidecar
                    }
                    Err(error) => SemanticReplayOutcome::Rejected {
                        identity_code: error.rejection_identity().canonical_code(),
                        reason: error.to_string(),
                    },
                };
                replayed.insert(key.1, outcome.clone());
                outcome
            };
            match (recovered.outcome, reproduced) {
                (_, SemanticReplayOutcome::DeferredMergeSidecar) => {
                    retired_missing_sidecar.insert(*key, recovered.clone());
                }
                (
                    ValidationOutcomeMarkerKind::Validated(expected),
                    SemanticReplayOutcome::Validated(actual),
                ) if expected == actual => {
                    promoted_validated.insert(
                        *key,
                        ValidatedBodyReceipt {
                            durable: receipt.clone(),
                            execution_commitment: actual,
                        },
                    );
                }
                (
                    ValidationOutcomeMarkerKind::Validated(_),
                    SemanticReplayOutcome::Validated(_),
                ) => return Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch),
                (
                    ValidationOutcomeMarkerKind::Rejected(expected_code),
                    SemanticReplayOutcome::Rejected {
                        identity_code,
                        reason,
                    },
                ) if expected_code == identity_code => {
                    promoted_rejected.insert(
                        *key,
                        RevalidatedRejectedBody {
                            durable: receipt.clone(),
                            identity_code,
                            reason,
                        },
                    );
                }
                _ => return Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch),
            }
        }
        self.validated.extend(promoted_validated);
        self.rejected.extend(promoted_rejected);
        self.retired_revalidation.extend(retired_missing_sidecar);
        self.pending_revalidation.clear();
        Ok(())
    }
    /// Retire restart vote authority for bodies other than a verified decision.
    ///
    /// Once Kura has a cryptographically verified finality artifact, losing
    /// candidates cannot be re-executed against the now-advanced world state
    /// and must never recover height-local vote authority. Their durable body
    /// bytes remain available for bounded cleanup. Their vote-authorizing
    /// marker capabilities are removed; a quarantined rejection is retained
    /// only as comparison-only denial against laundering the same local body.
    pub(crate) fn retain_recovered_markers_for_subject(
        &mut self,
        decision: VerifiedRecoveredFinalitySubject,
    ) -> Result<(), V2BodyStoreError> {
        if !decision.authorizes_context(&self.context) {
            return Err(V2BodyStoreError::RecoveredFinalityContextMismatch);
        }
        let subject = decision.subject();
        self.retain_pending_revalidation(|(_, candidate)| *candidate == subject)?;
        self.validated
            .retain(|(_, candidate), _| *candidate == subject);
        self.rejected
            .retain(|(_, candidate), _| *candidate == subject);
        Ok(())
    }
    /// Retain only marker authority named by authenticated WAL replay.
    ///
    /// Superseded view-local markers remain on disk as checksummed diagnostics,
    /// and their exact bodies remain available for certified serving or later
    /// bounded validation. They are excluded from synchronous semantic replay
    /// and cannot restore vote authority unless the live runtime validates the
    /// body again.
    pub(crate) fn retain_recovered_markers_for_authority(
        &mut self,
        authority: RecoveredValidationAuthority,
    ) -> Result<(), V2BodyStoreError> {
        if !authority.authorizes_context(&self.context) {
            return Err(V2BodyStoreError::RecoveredValidationAuthorityContextMismatch);
        }
        self.retain_pending_revalidation(|(round, subject)| {
            authority.authorizes(*round, *subject)
        })?;
        self.validated
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
        self.rejected
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
        Ok(())
    }
    /// Move every marker outside one authenticated startup frontier into the
    /// comparison-only retired catalog without discarding its rejection seal.
    fn retain_pending_revalidation(
        &mut self,
        mut retain: impl FnMut(&(wire::ConsensusRound, wire::BlockSubject)) -> bool,
    ) -> Result<(), V2BodyStoreError> {
        let pending = std::mem::take(&mut self.pending_revalidation);
        for (key, outcome) in pending {
            if retain(&key) {
                self.pending_revalidation.insert(key, outcome);
                continue;
            }
            match self.retired_revalidation.entry(key) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(outcome);
                }
                std::collections::btree_map::Entry::Occupied(slot) if slot.get() == &outcome => {}
                std::collections::btree_map::Entry::Occupied(_) => {
                    return Err(V2BodyStoreError::ConflictingValidationOutcome);
                }
            }
        }
        Ok(())
    }
    /// Require all restart markers to have crossed semantic revalidation.
    ///
    /// The serialized runtime calls this before restoring vote authority so a
    /// caller cannot accidentally treat a checksummed local file as a trusted
    /// validation receipt.
    pub(crate) fn ensure_recovered_markers_revalidated(&self) -> Result<(), V2BodyStoreError> {
        if self.pending_revalidation.is_empty() {
            Ok(())
        } else {
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        }
    }
    /// Consume this exact open store into the sole production-startup handoff.
    ///
    /// Checksummed restart markers cannot cross this boundary. The store is
    /// consumed even on error so there is no raw fallback path after a failed
    /// attempt to seal unrevalidated state.
    // The unified lifecycle-owner factory consumes this immediately after
    // semantic marker replay.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn into_revalidated_startup(
        self,
    ) -> Result<RevalidatedV2BodyStore, V2BodyStoreError> {
        self.ensure_recovered_markers_revalidated()?;
        Ok(RevalidatedV2BodyStore(self))
    }
    /// Snapshot semantically revalidated success receipts for WAL recovery.
    ///
    /// Deterministic rejections remain in a separate private map and cannot be
    /// mistaken for vote-signing authority through this existing API.
    pub(crate) fn validated_recovery_catalog(
        &self,
    ) -> BTreeMap<(wire::ConsensusRound, wire::BlockSubject), ValidatedBodyReceipt> {
        self.validated.clone()
    }
    /// Snapshot semantically replayed deterministic-rejection receipts.
    pub(crate) fn rejected_recovery_catalog(
        &self,
    ) -> BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt> {
        self.rejected
            .iter()
            .map(|(key, rejected)| (*key, rejected.durable.clone()))
            .collect()
    }
    /// Snapshot retired rejection receipts as comparison-only local-producer
    /// denial. These receipts have not crossed semantic replay in this process
    /// and therefore must never authorize a reducer `ValidationFailed` event.
    pub(crate) fn retired_rejected_recovery_catalog(
        &self,
    ) -> BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableBodyReceipt> {
        self.retired_revalidation
            .iter()
            .filter(|(_, retired)| {
                matches!(retired.outcome, ValidationOutcomeMarkerKind::Rejected(_))
            })
            .map(|(key, retired)| (*key, retired.durable.clone()))
            .collect()
    }
    /// Reconstruct the exact fsynced body authority for a recovered Decision Store.
    ///
    /// This cold path requires one unambiguous WAL-matching body frame and
    /// reopens its canonical envelope before returning. It does not require a
    /// validation marker: the Store row is the crash cut immediately after
    /// body persistence and before validation begins.
    pub(in crate::sumeragi) fn recovered_decision_fetch_store_body(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<RecoveredDecisionFetchStoreBodyAuthorityV1, V2BodyStoreError> {
        let mut matches = self
            .entries
            .iter()
            .filter(|(_, durable)| projection.matches_durable_body(durable));
        let Some((key, durable)) = matches.next() else {
            return Err(V2BodyStoreError::ReceiptMismatch);
        };
        if matches.next().is_some() {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let manifest = self
            .manifests
            .get(key)
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        if *key != (durable.round(), durable.subject())
            || manifest.round != durable.round()
            || manifest.subject != durable.subject()
            || HashOf::new(manifest) != durable.manifest_hash()
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let envelope = self.load_envelope(durable)?;
        if envelope.context_id != self.context.id()
            || envelope.round != durable.round()
            || envelope.subject != durable.subject()
            || envelope.manifest != *manifest
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        Ok(RecoveredDecisionFetchStoreBodyAuthorityV1 {
            manifest: manifest.clone(),
            durable: durable.clone(),
        })
    }
    /// Detect the exact validated predecessor reserved for Decision Apply publication.
    ///
    /// This read-only oracle exposes no receipt. A matching marker prevents the
    /// recovered Decision Fetch from being installed as a second executable
    /// owner; the publication composite consumes and replaces both atomically.
    pub(in crate::sumeragi) fn has_exact_recovered_decision_fetch_parent(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        self.validated
            .values()
            .any(|validated| projection.matches_validated_body(validated))
    }
    /// Detect a matching success marker still awaiting semantic replay.
    ///
    /// This is a rejection-only oracle. A quarantined marker is not Apply
    /// authority, but startup must not install a duplicate Decision Fetch before
    /// semantic revalidation can classify and consume the marker.
    pub(in crate::sumeragi) fn has_quarantined_recovered_decision_fetch_parent(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        self.pending_revalidation
            .values()
            .any(|pending| match &pending.outcome {
                ValidationOutcomeMarkerKind::Validated(commitment) => {
                    projection.matches_durable_body_and_commitment(&pending.durable, *commitment)
                }
                ValidationOutcomeMarkerKind::Rejected(_) => false,
            })
    }
    /// Detect a deterministic rejection for the body named by a Commit Decision.
    ///
    /// Both semantically replayed and quarantined rejection markers are
    /// invariant conflicts. They authorize neither refetch nor Apply and force
    /// startup to stop before LedgerV1 can change.
    pub(in crate::sumeragi) fn has_rejected_recovered_decision_body(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> bool {
        self.rejected
            .values()
            .any(|rejected| projection.matches_durable_body(&rejected.durable))
            || self.pending_revalidation.values().any(|pending| {
                matches!(&pending.outcome, ValidationOutcomeMarkerKind::Rejected(_))
                    && projection.matches_durable_body(&pending.durable)
            })
    }
    /// Detach the aggregate semantically revalidated terminal outcome catalog.
    ///
    /// Checks precede every move so a failed factory leaves the store intact.
    /// Comparison-only retired sidecar seals are neither eligible nor detached.
    pub(super) fn detach_terminal_validate_outcome_catalog(
        &mut self,
    ) -> Result<
        RecoveredTerminalValidateOutcomeCatalogCut<'_>,
        RecoveredTerminalValidateOutcomeCatalogError,
    > {
        if !self.pending_revalidation.is_empty() {
            return Err(RecoveredTerminalValidateOutcomeCatalogError::UnrevalidatedMarkers);
        }
        if self
            .validated
            .keys()
            .any(|key| self.rejected.contains_key(key))
        {
            return Err(RecoveredTerminalValidateOutcomeCatalogError::AmbiguousOutcome);
        }
        let validated = std::mem::take(&mut self.validated);
        let rejected = std::mem::take(&mut self.rejected);
        Ok(RecoveredTerminalValidateOutcomeCatalogCut {
            store: self,
            validated,
            rejected,
            selected_validated: BTreeMap::new(),
            selected_rejected: BTreeMap::new(),
        })
    }
    /// Detach the exact revalidated proposal marker named by one recovered WAL vote.
    ///
    /// This one-shot factory consults neither a scheduler lease nor a runtime
    /// lifecycle ordinal. The returned cut restores the marker on every
    /// pre-transfer failure and exposes no receipt or marker parts.
    pub(super) fn detach_recovered_validated_parent(
        &mut self,
        recovered: &RecoveredWalVoteSign,
    ) -> Result<RecoveredValidatedBodyCut<'_>, RecoveredValidatedBodyCutError> {
        let vote = recovered.vote();
        let tag_matches_vote = recovered.tag().height() == vote.round.height
            && match vote.phase {
                wire::GlobalPhase::Prepare => recovered.tag().view() == vote.round.view,
                wire::GlobalPhase::Commit => recovered.tag().view() >= vote.round.view,
            };
        if self.context.id() != vote.round.context_id
            || self.context.height != vote.round.height
            || !tag_matches_vote
        {
            return Err(RecoveredValidatedBodyCutError::ForeignContext);
        }
        let key = (vote.proposal_round, vote.subject);
        let Some(validated) = self.validated.remove(&key) else {
            return Err(RecoveredValidatedBodyCutError::MissingMarker);
        };
        if validated.execution_commitment() != vote.execution_commitment {
            let displaced = self.validated.insert(key, validated);
            debug_assert!(displaced.is_none());
            return Err(RecoveredValidatedBodyCutError::CommitmentMismatch);
        }
        Ok(RecoveredValidatedBodyCut {
            store: self,
            key,
            validated: Some(validated),
        })
    }
    /// Execute one exact-body persistence task on a storage-service thread.
    ///
    /// Only this method can mint [`BodyStoreCompletion`], and it does so only
    /// after [`Self::store`] has completed file and directory synchronisation.
    pub(crate) fn execute_store_task(
        &mut self,
        task: &BodyStoreTask,
    ) -> Result<BodyStoreCompletion, V2BodyStoreError> {
        let receipt = self.store(task.manifest().clone(), task.canonical_wire().to_vec())?;
        Ok(BodyStoreCompletion {
            work_id: task.id(),
            tag: task.tag(),
            manifest: task.manifest().clone(),
            receipt,
        })
    }
    // DURABLE_BODY_VALIDATION_API_BEGIN
    /// Execute deterministic validation against one exact durable body.
    ///
    /// The independently supplied manifest hash is checked against both the
    /// receipt and the store's canonical manifest before the callback can run.
    /// The complete checksummed frame is then reloaded, structurally validated,
    /// and decoded. A success or deterministic rejection result is minted only
    /// after its closed outcome marker has crossed the file-and-directory
    /// durability boundary. Missing-sidecar deferrals are never persisted.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn execute_durable_validation<F, E>(
        &mut self,
        durable: DurableBodyReceipt,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
        validator: F,
    ) -> Result<DurableBodyValidationOutcome, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        self.verify_receipt(&durable)?;
        let key = (durable.round(), durable.subject());
        let stored_manifest = self
            .manifests
            .get(&key)
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        if durable.context_id() != self.context.id()
            || durable.round().context_id != durable.context_id()
            || durable.round().height != self.context.height
            || stored_manifest.round != durable.round()
            || stored_manifest.subject != durable.subject()
            || durable.manifest_hash() != expected_manifest_hash
            || HashOf::new(stored_manifest) != expected_manifest_hash
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let envelope = self.load_envelope(&durable)?;
        if envelope.context_id != durable.context_id()
            || envelope.round != durable.round()
            || envelope.subject != durable.subject()
            || &envelope.manifest != stored_manifest
            || HashOf::new(&envelope.manifest) != expected_manifest_hash
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != &durable {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(DurableBodyValidationOutcome(
                DurableBodyValidationOutcomeBody::Validated(validated.clone()),
            ));
        }
        if let Some(rejected) = self.rejected.get(&key) {
            if rejected.durable != durable {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(rejected.sealed_outcome());
        }
        if let Some(execution_commitment) =
            self.reusable_validated_commitment_for_exact_body(&durable, &envelope)?
        {
            let validated = self.persist_validated_receipt(&durable, execution_commitment)?;
            return Ok(DurableBodyValidationOutcome(
                DurableBodyValidationOutcomeBody::Validated(validated),
            ));
        }
        let block = decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))?;
        match validator(&block) {
            Ok(execution_commitment) => {
                let validated = self.persist_validated_receipt(&durable, execution_commitment)?;
                Ok(DurableBodyValidationOutcome(
                    DurableBodyValidationOutcomeBody::Validated(validated),
                ))
            }
            Err(error) => {
                if let Some(reference) = error.missing_certified_merge_sidecar() {
                    return Ok(DurableBodyValidationOutcome(
                        DurableBodyValidationOutcomeBody::DeferredMergeSidecar {
                            durable,
                            reference: reference.clone(),
                        },
                    ));
                }
                let identity_code = error.rejection_identity().canonical_code();
                let rejected =
                    self.persist_rejected_outcome(&durable, identity_code, error.to_string())?;
                Ok(rejected.sealed_outcome())
            }
        }
    }
    /// Reuse an already authenticated semantic result for an unchanged body.
    ///
    /// A locked-body reproposal changes the manifest round but retains the
    /// exact signed block bytes and subject. Deterministic validation consumes
    /// those bytes under this immutable height context, not the proposal
    /// round. Reloading and comparing the checksummed canonical bytes keeps
    /// this shortcut behind the same body-store authority as ordinary
    /// validation. The caller still persists a fresh round-local marker before
    /// the result can authorize a vote.
    fn reusable_validated_commitment_for_exact_body(
        &self,
        durable: &DurableBodyReceipt,
        current: &StoredBodyEnvelope,
    ) -> Result<Option<wire::ExecutionCommitment>, V2BodyStoreError> {
        let Some(((source_round, source_subject), validated)) = self
            .validated
            .iter()
            .filter(|((round, subject), _)| {
                *subject == durable.subject()
                    && round.context_id == durable.context_id()
                    && round.height == durable.round().height
                    && round.view < durable.round().view
            })
            .max_by_key(|((round, _), _)| round.view)
        else {
            return Ok(None);
        };
        if *source_round != validated.durable().round()
            || *source_subject != validated.durable().subject()
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let previous = self.load_envelope(validated.durable())?;
        let same_manifest_body = previous.manifest.subject == current.manifest.subject
            && previous.manifest.payload_size_bytes == current.manifest.payload_size_bytes
            && previous.manifest.layout == current.manifest.layout
            && previous.manifest.chunk_hashes == current.manifest.chunk_hashes
            && previous.manifest.chunk_root == current.manifest.chunk_root;
        if !same_manifest_body || previous.canonical_wire != current.canonical_wire {
            return Err(V2BodyStoreError::ConflictingBody);
        }
        let commitment = validated.execution_commitment();
        commitment.validate()?;
        Ok(Some(commitment))
    }
    // DURABLE_BODY_VALIDATION_API_END
    /// Durably persist the exact body carried by an authenticated certified
    /// Fetch response and bind its complete transport occurrence.
    ///
    /// The canonical [`Self::store`] path validates the response manifest and
    /// body against this store's immutable context and returns only after a new
    /// file and its directory entry have been synchronised. Exact repeats reuse
    /// that already durable frame; a different body for the same round and
    /// subject fails closed under the existing store contract.
    // The bounded storage worker owns this blocking call. Its typed completion
    // retains the sealed receipt until the fresh-selector lifecycle transaction
    // either commits or closes output for restart.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn persist_authenticated_certified_fetch_response(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<DurableCertifiedFetchBodyReceipt, V2BodyStoreError> {
        let response = authenticated.response();
        let request_hash = response.request_hash;
        let response_hash = HashOf::new(response);
        let round = response.manifest.round;
        let subject = response.manifest.subject;
        let context_id = round.context_id;
        let manifest_hash = HashOf::new(&response.manifest);
        let durable_body = self.store(response.manifest.clone(), response.body.clone())?;
        if durable_body.context_id() != context_id
            || durable_body.round() != round
            || durable_body.subject() != subject
            || durable_body.manifest_hash() != manifest_hash
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        Ok(DurableCertifiedFetchBodyReceipt {
            request_hash,
            response_hash,
            durable_body,
        })
    }
    /// Validate and durably store canonical `SignedBlockWire` bytes.
    ///
    /// An identical body is idempotent. A different final file for the same
    /// round and subject fails closed instead of being replaced.
    pub(crate) fn store(
        &mut self,
        manifest: wire::PayloadManifest,
        canonical_wire: Vec<u8>,
    ) -> Result<DurableBodyReceipt, V2BodyStoreError> {
        self.ensure_mutable()?;
        let envelope = StoredBodyEnvelope {
            version: STORE_VERSION,
            context_id: self.context.id(),
            round: manifest.round,
            subject: manifest.subject,
            manifest,
            canonical_wire,
        };
        self.validate_envelope(&envelope)?;
        let key = (envelope.round, envelope.subject);
        if let Some(existing) = self.entries.get(&key) {
            let existing_envelope = self.load_envelope(existing)?;
            if existing_envelope == envelope {
                return Ok(existing.clone());
            }
            return Err(V2BodyStoreError::ConflictingBody);
        }
        if self.entries.len() >= self.capacity.max_body_entries {
            return Err(V2BodyStoreError::BodyEntryCapacityExceeded {
                capacity: self.capacity.max_body_entries,
            });
        }
        let encoded = envelope.encode();
        let framed = frame_payload(&encoded)?;
        let frame_bytes =
            u64::try_from(framed.len()).map_err(|_| V2BodyStoreError::BodyTooLarge)?;
        let next_body_frame_bytes = self.body_frame_bytes.checked_add(frame_bytes).ok_or(
            V2BodyStoreError::BodyByteCapacityExceeded {
                capacity: self.capacity.max_body_frame_bytes,
            },
        )?;
        if next_body_frame_bytes > self.capacity.max_body_frame_bytes {
            return Err(V2BodyStoreError::BodyByteCapacityExceeded {
                capacity: self.capacity.max_body_frame_bytes,
            });
        }
        let frame_hash = Hash::new(&framed);
        let name = Self::file_name_for(envelope.round, envelope.subject);
        self.bound_directory
            .as_ref()
            .ok_or_else(|| V2BodyStoreError::UnsupportedStorageBinding {
                path: self.directory.clone(),
            })?
            .publish_atomic(&name, &framed, FramePayloadKind::Body)?;
        let receipt = receipt_for(&envelope, frame_hash);
        self.entries.insert(key, receipt.clone());
        self.manifests.insert(key, envelope.manifest);
        self.body_frame_bytes = next_body_frame_bytes;
        Ok(receipt)
    }
    /// Load and revalidate the exact block represented by a durable receipt.
    pub(crate) fn load(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<SignedBlock, V2BodyStoreError> {
        let envelope = self.load_envelope(receipt)?;
        decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))
    }
    /// Load the exact canonical `SignedBlockWire` bytes represented by a
    /// durable receipt.
    ///
    /// Certified fetch responses and exact locked-subject recovery must
    /// preserve byte identity. Re-encoding a decoded block at those boundaries
    /// would be an unnecessary second source of truth, so callers receive the
    /// checksummed bytes from the final store frame itself.
    pub(crate) fn load_canonical_wire(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<Vec<u8>, V2BodyStoreError> {
        Ok(self.load_envelope(receipt)?.canonical_wire)
    }
    /// Read one exact Certified-Serve body and bind it to this live store.
    ///
    /// The returned move-only authority is minted only after the ordinary
    /// receipt and frame revalidation succeeds. It lets the lifecycle owner
    /// settle worker completion after launch has moved this store out of the
    /// owner, without weakening the existing exact body-store boundary.
    pub(in crate::sumeragi) fn read_durable_body_for_certified_serve(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<DurableCertifiedServeBodyReadbackV1, V2BodyStoreError> {
        let canonical_wire = self.load_canonical_wire(receipt)?;
        Ok(DurableCertifiedServeBodyReadbackV1 {
            durable_body: receipt.clone(),
            canonical_wire,
            store_identity: self.instance_identity(),
        })
    }
    /// Find the newest durable proposal round retaining one exact subject.
    ///
    /// Locked-candidate recovery uses this lookup to find the retained body for
    /// the exact subject. The BTreeMap order makes selection deterministic
    /// across restart, while the returned receipt still has to pass the normal
    /// frame checks before bytes can be loaded.
    pub(crate) fn latest_for_subject(
        &self,
        subject: wire::BlockSubject,
    ) -> Result<Option<(wire::PayloadManifest, DurableBodyReceipt)>, V2BodyStoreError> {
        let selected = self
            .entries
            .iter()
            .rev()
            .find(|((_, stored_subject), _)| *stored_subject == subject)
            .map(|(key, receipt)| (*key, receipt.clone()));
        let Some((key, receipt)) = selected else {
            return Ok(None);
        };
        self.verify_receipt(&receipt)?;
        let manifest = self
            .manifests
            .get(&key)
            .cloned()
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        Ok(Some((manifest, receipt)))
    }
    /// Test helper that runs a validator synchronously over the exact durable
    /// body and persists the same marker used by the production task API.
    #[cfg(test)]
    pub(crate) fn validate<F, E>(
        &mut self,
        receipt: &DurableBodyReceipt,
        validator: F,
    ) -> Result<ValidatedBodyReceipt, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: std::fmt::Display,
    {
        let key = (receipt.round, receipt.subject);
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(validated.clone());
        }
        let block = self.load(receipt)?;
        let execution_commitment = validator(&block)
            .map_err(|error| V2BodyStoreError::DeterministicValidation(error.to_string()))?;
        self.persist_validated_receipt(receipt, execution_commitment)
    }
    fn persist_validated_receipt(
        &mut self,
        receipt: &DurableBodyReceipt,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Result<ValidatedBodyReceipt, V2BodyStoreError> {
        self.ensure_mutable()?;
        self.verify_receipt(receipt)?;
        execution_commitment.validate()?;
        let key = (receipt.round, receipt.subject);
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            if validated.execution_commitment() != execution_commitment {
                return Err(V2BodyStoreError::ConflictingValidationCommitment);
            }
            return Ok(validated.clone());
        }
        if let Some(rejected) = self.rejected.get(&key) {
            if &rejected.durable != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Err(V2BodyStoreError::ConflictingValidationOutcome);
        }
        let outcome = ValidationOutcomeMarkerKind::Validated(execution_commitment);
        if let Some(recovered) = self.pending_revalidation.get(&key).cloned() {
            if &recovered.durable != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            match recovered.outcome {
                ValidationOutcomeMarkerKind::Validated(expected)
                    if expected == execution_commitment => {}
                ValidationOutcomeMarkerKind::Validated(_) => {
                    return Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch);
                }
                ValidationOutcomeMarkerKind::Rejected(_) => {
                    return Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch);
                }
            }
            self.pending_revalidation.remove(&key);
            let validated = ValidatedBodyReceipt {
                durable: recovered.durable,
                execution_commitment,
            };
            self.validated.insert(key, validated.clone());
            return Ok(validated);
        }
        if let Some(retired) = self.retired_revalidation.get(&key).cloned() {
            if &retired.durable != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            match retired.outcome {
                ValidationOutcomeMarkerKind::Validated(expected)
                    if expected == execution_commitment => {}
                ValidationOutcomeMarkerKind::Validated(_) => {
                    return Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch);
                }
                ValidationOutcomeMarkerKind::Rejected(_) => {
                    return Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch);
                }
            }
            self.retired_revalidation.remove(&key);
            let validated = ValidatedBodyReceipt {
                durable: retired.durable,
                execution_commitment,
            };
            self.validated.insert(key, validated.clone());
            return Ok(validated);
        }
        self.ensure_validation_outcome_consistent(receipt, outcome)?;
        let validated = ValidatedBodyReceipt {
            durable: receipt.clone(),
            execution_commitment,
        };
        let marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
            outcome,
        };
        write_validation_outcome_marker_bound(
            self.bound_directory.as_ref().ok_or_else(|| {
                V2BodyStoreError::UnsupportedStorageBinding {
                    path: self.directory.clone(),
                }
            })?,
            &Self::validated_file_name_for(receipt.round, receipt.subject),
            &marker,
        )?;
        self.validated.insert(key, validated.clone());
        Ok(validated)
    }
    fn persist_rejected_outcome(
        &mut self,
        receipt: &DurableBodyReceipt,
        identity_code: u8,
        reason: String,
    ) -> Result<RevalidatedRejectedBody, V2BodyStoreError> {
        self.ensure_mutable()?;
        self.verify_receipt(receipt)?;
        if BodyValidationRejectionIdentity::from_canonical_code(identity_code).is_none() {
            return Err(V2BodyStoreError::UnknownValidationRejectionIdentity(
                identity_code,
            ));
        }
        let key = (receipt.round, receipt.subject);
        if let Some(rejected) = self.rejected.get(&key) {
            if &rejected.durable != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            if rejected.identity_code != identity_code {
                return Err(V2BodyStoreError::ConflictingValidationOutcome);
            }
            return Ok(rejected.clone());
        }
        if let Some(validated) = self.validated.get(&key) {
            if validated.durable() != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Err(V2BodyStoreError::ConflictingValidationOutcome);
        }
        let outcome = ValidationOutcomeMarkerKind::Rejected(identity_code);
        if let Some(recovered) = self.pending_revalidation.get(&key).cloned() {
            if &recovered.durable != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            match recovered.outcome {
                ValidationOutcomeMarkerKind::Rejected(expected_code)
                    if expected_code == identity_code => {}
                _ => {
                    return Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch);
                }
            }
            self.pending_revalidation.remove(&key);
            let rejected = RevalidatedRejectedBody {
                durable: recovered.durable,
                identity_code,
                reason,
            };
            self.rejected.insert(key, rejected.clone());
            return Ok(rejected);
        }
        if let Some(retired) = self.retired_revalidation.get(&key).cloned() {
            if &retired.durable != receipt {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            match retired.outcome {
                ValidationOutcomeMarkerKind::Rejected(expected_code)
                    if expected_code == identity_code => {}
                _ => return Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch),
            }
            self.retired_revalidation.remove(&key);
            let rejected = RevalidatedRejectedBody {
                durable: retired.durable,
                identity_code,
                reason,
            };
            self.rejected.insert(key, rejected.clone());
            return Ok(rejected);
        }
        self.ensure_validation_outcome_consistent(receipt, outcome)?;
        let marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
            outcome,
        };
        write_validation_outcome_marker_bound(
            self.bound_directory.as_ref().ok_or_else(|| {
                V2BodyStoreError::UnsupportedStorageBinding {
                    path: self.directory.clone(),
                }
            })?,
            &Self::validated_file_name_for(receipt.round, receipt.subject),
            &marker,
        )?;
        let rejected = RevalidatedRejectedBody {
            durable: receipt.clone(),
            identity_code,
            reason,
        };
        self.rejected.insert(key, rejected.clone());
        Ok(rejected)
    }
    fn ensure_validation_outcome_consistent(
        &self,
        receipt: &DurableBodyReceipt,
        proposed: ValidationOutcomeMarkerKind,
    ) -> Result<(), V2BodyStoreError> {
        let owns_same_body = |round: &wire::ConsensusRound, subject: &wire::BlockSubject| {
            *subject == receipt.subject
                && round.context_id == receipt.round.context_id
                && round.height == receipt.round.height
        };
        let active_validated = self
            .validated
            .iter()
            .filter_map(|((round, subject), validated)| {
                owns_same_body(round, subject).then_some(ValidationOutcomeMarkerKind::Validated(
                    validated.execution_commitment(),
                ))
            });
        let active_rejected = self
            .rejected
            .iter()
            .filter_map(|((round, subject), rejected)| {
                owns_same_body(round, subject).then_some(ValidationOutcomeMarkerKind::Rejected(
                    rejected.identity_code,
                ))
            });
        let quarantined =
            self.pending_revalidation
                .iter()
                .filter_map(|((round, subject), recovered)| {
                    owns_same_body(round, subject).then_some(recovered.outcome)
                });
        let retired =
            self.retired_revalidation
                .iter()
                .filter_map(|((round, subject), recovered)| {
                    owns_same_body(round, subject).then_some(recovered.outcome)
                });
        for existing in active_validated
            .chain(active_rejected)
            .chain(quarantined)
            .chain(retired)
        {
            if existing == proposed {
                continue;
            }
            return Err(match (existing, proposed) {
                (
                    ValidationOutcomeMarkerKind::Validated(_),
                    ValidationOutcomeMarkerKind::Validated(_),
                ) => V2BodyStoreError::ConflictingValidationCommitment,
                _ => V2BodyStoreError::ConflictingValidationOutcome,
            });
        }
        Ok(())
    }
    /// Retire every losing and decided candidate after Kura durably finalizes
    /// the owning height context.
    ///
    /// Once a height has one immutable CommitQC artifact, no candidate body at
    /// that height can be voted on again. Context/height matching is therefore
    /// the authorization boundary for deleting the complete directory; only
    /// the decided candidate additionally matches the receipt's block hash.
    pub(crate) fn into_retirement_job(
        mut self,
        kura_receipt: &KuraV2CommitReceipt,
    ) -> Result<V2BodyRetirementJob, V2BodyStoreError> {
        self.ensure_mutable()?;
        if kura_receipt.context_id() != self.context.id()
            || kura_receipt.height() != self.context.height
        {
            return Err(V2BodyStoreError::KuraReceiptMismatch);
        }
        let directory = self.bound_directory.take().ok_or_else(|| {
            V2BodyStoreError::UnsupportedStorageBinding {
                path: self.directory.clone(),
            }
        })?;
        Ok(V2BodyRetirementJob { directory })
    }
    fn load_envelope(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<StoredBodyEnvelope, V2BodyStoreError> {
        self.verify_receipt(receipt)?;
        let directory = self.bound_directory.as_ref().ok_or_else(|| {
            V2BodyStoreError::UnsupportedStorageBinding {
                path: self.directory.clone(),
            }
        })?;
        let (envelope, frame_hash) = read_envelope_named(
            directory,
            &Self::file_name_for(receipt.round, receipt.subject),
        )?;
        if frame_hash != receipt.frame_hash {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        self.validate_envelope(&envelope)?;
        if receipt_for(&envelope, frame_hash) != *receipt {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        Ok(envelope)
    }
    fn verify_receipt(&self, receipt: &DurableBodyReceipt) -> Result<(), V2BodyStoreError> {
        if !self.owns_receipt(receipt) {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        Ok(())
    }
    fn validate_envelope(&self, envelope: &StoredBodyEnvelope) -> Result<(), V2BodyStoreError> {
        if envelope.version != STORE_VERSION {
            return Err(V2BodyStoreError::UnsupportedVersion(envelope.version));
        }
        if envelope.context_id != self.context.id()
            || envelope.round.context_id != envelope.context_id
            || envelope.round.height != self.context.height
            || envelope.manifest.round != envelope.round
            || envelope.manifest.subject != envelope.subject
        {
            return Err(V2BodyStoreError::ContextMismatch);
        }
        envelope.manifest.validate(&self.context)?;
        let body_len = u64::try_from(envelope.canonical_wire.len())
            .map_err(|_| V2BodyStoreError::BodyTooLarge)?;
        if body_len != envelope.manifest.payload_size_bytes {
            return Err(V2BodyStoreError::BodyLengthMismatch);
        }
        if Hash::new(&envelope.canonical_wire) != envelope.subject.payload_hash {
            return Err(V2BodyStoreError::PayloadHashMismatch);
        }
        let canonical_manifest = encode_payload(
            &self.context,
            envelope.round,
            envelope.subject,
            &envelope.canonical_wire,
        )
        .map_err(|_| V2BodyStoreError::NonCanonicalPayloadManifest)?;
        if canonical_manifest.manifest() != &envelope.manifest {
            return Err(V2BodyStoreError::NonCanonicalPayloadManifest);
        }
        let block = decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))?;
        if !block.is_resultless_proposal() {
            return Err(V2BodyStoreError::ResultBearingProposal);
        }
        let reencoded = block
            .encode_wire()
            .map_err(|error| V2BodyStoreError::BlockEncode(error.to_string()))?;
        if reencoded != envelope.canonical_wire {
            return Err(V2BodyStoreError::NonCanonicalBlockWire);
        }
        let header = block.header();
        let body_origin_view = header.view_change_index();
        let view_matches = match &self.signature_policy {
            // An unchanged locked body keeps its original header and signature
            // when a later leader re-proposes it. Authenticate that original
            // leader and forbid only bodies originating in a future view.
            BlockSignaturePolicy::RotatingLeader => body_origin_view <= envelope.round.view,
            // Genesis is the sole exception: height one retains the fixed
            // authority-signed view-zero body across certified view changes.
            // `open_with_policy` separately rejects this policy outside an
            // unparented height-one context.
            BlockSignaturePolicy::GenesisAuthority(_) => {
                header.height().get() == 1 && body_origin_view == 0
            }
        };
        if header.height().get() != envelope.round.height
            || !view_matches
            || block.hash() != envelope.subject.block_hash
            || header.prev_block_hash() != envelope.subject.parent_block_hash
        {
            return Err(V2BodyStoreError::BlockSubjectMismatch);
        }
        let expected_parent = self
            .context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| certificate.subject.block_hash)
            .or_else(|| {
                self.context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            });
        if header.prev_block_hash() != expected_parent {
            return Err(V2BodyStoreError::ParentMismatch);
        }
        let (expected_index, expected_key) = match &self.signature_policy {
            BlockSignaturePolicy::RotatingLeader => {
                let leader = self.context.leader(body_origin_view);
                let leader_index =
                    usize::try_from(leader).map_err(|_| V2BodyStoreError::LeaderOutOfRange)?;
                let leader_key = self
                    .context
                    .roster
                    .get(leader_index)
                    .ok_or(V2BodyStoreError::LeaderOutOfRange)?
                    .validator
                    .public_key();
                (u64::from(leader), leader_key)
            }
            BlockSignaturePolicy::GenesisAuthority(public_key) => (0, public_key),
        };
        let mut signatures = block.signatures();
        let signature = signatures
            .next()
            .ok_or(V2BodyStoreError::MissingExpectedSignature)?;
        if signatures.next().is_some() || signature.index() != expected_index {
            return Err(V2BodyStoreError::InvalidExpectedSignatureSet);
        }
        signature
            .signature()
            .verify_hash(expected_key, block.hash())
            .map_err(|_| V2BodyStoreError::InvalidExpectedSignature)?;
        Ok(())
    }
    fn validate_marker(
        &self,
        marker: &ValidationOutcomeMarker,
        receipt: &DurableBodyReceipt,
    ) -> Result<(), V2BodyStoreError> {
        if marker.version != VALIDATION_OUTCOME_MARKER_VERSION {
            return Err(V2BodyStoreError::UnsupportedValidationOutcomeMarkerVersion(
                marker.version,
            ));
        }
        if marker.context_id != self.context.id()
            || marker.round.context_id != marker.context_id
            || marker.round.height != self.context.height
            || marker.context_id != receipt.context_id
            || marker.round != receipt.round
            || marker.subject != receipt.subject
            || marker.manifest_hash != receipt.manifest_hash
            || marker.body_frame_hash != receipt.frame_hash
        {
            return Err(V2BodyStoreError::ValidationMarkerMismatch);
        }
        match marker.outcome {
            ValidationOutcomeMarkerKind::Validated(execution_commitment) => {
                execution_commitment.validate()?;
            }
            ValidationOutcomeMarkerKind::Rejected(identity_code) => {
                if BodyValidationRejectionIdentity::from_canonical_code(identity_code).is_none() {
                    return Err(V2BodyStoreError::UnknownValidationRejectionIdentity(
                        identity_code,
                    ));
                }
            }
        }
        Ok(())
    }
    fn path_for(&self, round: wire::ConsensusRound, subject: wire::BlockSubject) -> PathBuf {
        self.directory.join(Self::file_name_for(round, subject))
    }
    fn file_name_for(round: wire::ConsensusRound, subject: wire::BlockSubject) -> OsString {
        let key_hash = Hash::new((round, subject).encode());
        OsString::from(format!(
            "{:020}-{:020}-{}.norito",
            round.height,
            round.view,
            hex::encode(key_hash.as_ref())
        ))
    }
    fn validated_path_for(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> PathBuf {
        self.directory
            .join(Self::validated_file_name_for(round, subject))
    }
    fn validated_file_name_for(
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> OsString {
        let mut path = PathBuf::from(Self::file_name_for(round, subject));
        path.set_extension("validated");
        path.into_os_string()
    }
}
fn receipt_for(envelope: &StoredBodyEnvelope, frame_hash: Hash) -> DurableBodyReceipt {
    DurableBodyReceipt {
        context_id: envelope.context_id,
        round: envelope.round,
        subject: envelope.subject,
        manifest_hash: HashOf::new(&envelope.manifest),
        frame_hash,
    }
}
fn frame_payload(payload: &[u8]) -> Result<Vec<u8>, V2BodyStoreError> {
    frame_payload_with_magic(STORE_MAGIC, payload, FramePayloadKind::Body)
}
fn frame_payload_with_magic(
    magic: &[u8; 8],
    payload: &[u8],
    kind: FramePayloadKind,
) -> Result<Vec<u8>, V2BodyStoreError> {
    let payload_len = u64::try_from(payload.len()).map_err(|_| kind.too_large())?;
    if payload_len > kind.maximum_payload_bytes() {
        return Err(kind.too_large());
    }
    let capacity = FRAME_HEADER_LEN
        .checked_add(payload.len())
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or_else(|| kind.too_large())?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(capacity)
        .map_err(|_| kind.too_large())?;
    frame.extend_from_slice(magic);
    frame.extend_from_slice(&STORE_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(payload);
    frame.extend_from_slice(Hash::new(payload).as_ref());
    Ok(frame)
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn read_envelope_from_leaf(
    directory: &BoundV2BodyContextDirectory,
    leaf: &BoundBodyStoreLeaf,
) -> Result<(StoredBodyEnvelope, Hash), V2BodyStoreError> {
    let mut file = directory.open_leaf(leaf, FramePayloadKind::Body)?;
    let (payload, frame_hash) = read_frame_payload_with_hash(
        &mut file,
        &directory.context_path.join(&leaf.name),
        STORE_MAGIC,
        FramePayloadKind::Body,
    )?;
    directory.verify_leaf(&file, &leaf.name, Some(leaf))?;
    let mut cursor = payload.as_slice();
    let envelope = StoredBodyEnvelope::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::EnvelopeDecode(error.to_string()))?;
    Ok((envelope, frame_hash))
}

#[cfg(all(unix, not(target_os = "espidf")))]
fn read_envelope_named(
    directory: &BoundV2BodyContextDirectory,
    name: &OsStr,
) -> Result<(StoredBodyEnvelope, Hash), V2BodyStoreError> {
    let leaf = directory.inspect_leaf(name, FramePayloadKind::Body)?;
    read_envelope_from_leaf(directory, &leaf)
}

#[cfg(not(all(unix, not(target_os = "espidf"))))]
fn read_envelope_named(
    directory: &BoundV2BodyContextDirectory,
    _name: &OsStr,
) -> Result<(StoredBodyEnvelope, Hash), V2BodyStoreError> {
    Err(V2BodyStoreError::UnsupportedStorageBinding {
        path: directory.context_path.clone(),
    })
}

fn read_frame_payload(
    file: &mut File,
    path: &Path,
    magic: &[u8; 8],
    kind: FramePayloadKind,
) -> Result<Vec<u8>, V2BodyStoreError> {
    read_frame_payload_with_hash(file, path, magic, kind).map(|(payload, _)| payload)
}
fn read_frame_payload_with_hash(
    file: &mut File,
    path: &Path,
    magic: &[u8; 8],
    kind: FramePayloadKind,
) -> Result<(Vec<u8>, Hash), V2BodyStoreError> {
    let opened_len = file
        .metadata()
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    let maximum_frame_bytes = kind
        .maximum_payload_bytes()
        .checked_add(
            u64::try_from(FRAME_HEADER_LEN + CHECKSUM_LEN).expect("frame overhead fits u64"),
        )
        .ok_or_else(|| kind.too_large())?;
    if opened_len > maximum_frame_bytes {
        return Err(kind.too_large());
    }
    let mut header = [0_u8; FRAME_HEADER_LEN];
    file.read_exact(&mut header)
        .map_err(|_| V2BodyStoreError::CorruptFrame)?;
    if &header[..magic.len()] != magic {
        return Err(V2BodyStoreError::CorruptFrame);
    }
    let version_offset = magic.len();
    let version = u16::from_le_bytes(
        header[version_offset..version_offset + size_of::<u16>()]
            .try_into()
            .map_err(|_| V2BodyStoreError::CorruptFrame)?,
    );
    if version != STORE_VERSION {
        return Err(V2BodyStoreError::UnsupportedVersion(version));
    }
    let length_offset = version_offset + size_of::<u16>();
    let payload_len = u64::from_le_bytes(
        header[length_offset..length_offset + size_of::<u64>()]
            .try_into()
            .map_err(|_| V2BodyStoreError::CorruptFrame)?,
    );
    if payload_len > kind.maximum_payload_bytes() {
        return Err(kind.too_large());
    }
    let payload_len = usize::try_from(payload_len).map_err(|_| kind.too_large())?;
    let expected_len = FRAME_HEADER_LEN
        .checked_add(payload_len)
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or_else(|| kind.too_large())?;
    if opened_len != u64::try_from(expected_len).map_err(|_| kind.too_large())? {
        return Err(V2BodyStoreError::CorruptFrame);
    }
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(payload_len)
        .map_err(|_| kind.too_large())?;
    payload.resize(payload_len, 0);
    file.read_exact(&mut payload)
        .map_err(|_| V2BodyStoreError::CorruptFrame)?;
    let mut checksum = [0_u8; CHECKSUM_LEN];
    file.read_exact(&mut checksum)
        .map_err(|_| V2BodyStoreError::CorruptFrame)?;
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?
        != 0
    {
        return Err(V2BodyStoreError::CorruptFrame);
    }
    if Hash::new(&payload).as_ref() != checksum.as_slice() {
        return Err(V2BodyStoreError::ChecksumMismatch);
    }
    let frame_hash = Hash::new_from_chunks(&[&header, &payload, &checksum]);
    Ok((payload, frame_hash))
}
fn write_validation_outcome_marker_bound(
    directory: &BoundV2BodyContextDirectory,
    name: &OsStr,
    marker: &ValidationOutcomeMarker,
) -> Result<(), V2BodyStoreError> {
    let payload = marker.encode();
    let frame = frame_payload_with_magic(
        VALIDATED_MAGIC,
        &payload,
        FramePayloadKind::ValidationOutcomeMarker,
    )?;
    directory.publish_atomic(name, &frame, FramePayloadKind::ValidationOutcomeMarker)
}

#[cfg(all(unix, not(target_os = "espidf")))]
fn read_validation_outcome_marker_from_leaf(
    directory: &BoundV2BodyContextDirectory,
    leaf: &BoundBodyStoreLeaf,
) -> Result<ValidationOutcomeMarker, V2BodyStoreError> {
    let mut file = directory.open_leaf(leaf, FramePayloadKind::ValidationOutcomeMarker)?;
    let payload = read_frame_payload(
        &mut file,
        &directory.context_path.join(&leaf.name),
        VALIDATED_MAGIC,
        FramePayloadKind::ValidationOutcomeMarker,
    )?;
    directory.verify_leaf(&file, &leaf.name, Some(leaf))?;
    let mut cursor = payload.as_slice();
    ValidationOutcomeMarker::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::ValidationMarkerDecode(error.to_string()))
}

#[cfg(test)]
fn write_validation_outcome_marker(
    path: &Path,
    marker: &ValidationOutcomeMarker,
) -> Result<(), V2BodyStoreError> {
    let payload = marker.encode();
    let frame = frame_payload_with_magic(
        VALIDATED_MAGIC,
        &payload,
        FramePayloadKind::ValidationOutcomeMarker,
    )?;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(&frame)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
fn read_validation_outcome_marker(
    path: &Path,
) -> Result<ValidationOutcomeMarker, V2BodyStoreError> {
    let mut file = File::open(path).map_err(|source| V2BodyStoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let payload = read_frame_payload(
        &mut file,
        path,
        VALIDATED_MAGIC,
        FramePayloadKind::ValidationOutcomeMarker,
    )?;
    let mut cursor = payload.as_slice();
    ValidationOutcomeMarker::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::ValidationMarkerDecode(error.to_string()))
}
/// Exact-body persistence or validation failure.
#[derive(Debug, Error)]
pub(crate) enum V2BodyStoreError {
    /// Filesystem operation failed.
    #[error("Sumeragi v2 body-store I/O failed at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A retained directory or leaf no longer names the object opened earlier.
    #[error("Sumeragi v2 body-store binding failed at {path}: {reason}")]
    StorageBinding {
        /// Affected diagnostic path.
        path: PathBuf,
        /// Closed binding failure reason.
        reason: &'static str,
    },
    /// The platform cannot provide the required descriptor-relative storage API.
    #[error("descriptor-relative Sumeragi v2 body storage is unsupported at {path}")]
    UnsupportedStorageBinding {
        /// Requested body-store path.
        path: PathBuf,
    },
    /// An immutable final entry appeared before no-replace publication.
    #[error("Sumeragi v2 body-store destination already exists: {}", .0.display())]
    AtomicDestinationExists(PathBuf),
    /// Frozen height context is structurally invalid.
    #[error("invalid Sumeragi v2 body-store height context: {0}")]
    Context(#[from] wire::ValidationError),
    /// Configured per-height body capacity cannot form a bounded geometry.
    #[error("invalid Sumeragi v2 body-store capacity geometry")]
    InvalidCapacityGeometry,
    /// Context directory exceeds the bounded inventory geometry.
    #[error("Sumeragi v2 body-store directory exceeds {capacity} entries")]
    DirectoryEntryCapacityExceeded {
        /// Maximum entries inventoried for one height directory.
        capacity: usize,
    },
    /// Atomic-write remnants exceed their bounded inventory allowance.
    #[error("Sumeragi v2 body-store directory exceeds {capacity} temporary entries")]
    TemporaryEntryCapacityExceeded {
        /// Maximum temporary entries inventoried for one height.
        capacity: usize,
    },
    /// Durable semantic body count reached the lifecycle ceiling.
    #[error("Sumeragi v2 body store exceeds {capacity} durable bodies per height")]
    BodyEntryCapacityExceeded {
        /// Maximum durable body entries for one height.
        capacity: usize,
    },
    /// Durable validation-marker count exceeds the bounded body geometry.
    #[error("Sumeragi v2 body store exceeds {capacity} validation markers per height")]
    ValidationMarkerCapacityExceeded {
        /// Maximum validation markers for one height.
        capacity: usize,
    },
    /// One validation marker exceeds the fixed first-release frame ceiling.
    #[error("Sumeragi v2 validation marker exceeds its fixed frame-size ceiling")]
    ValidationMarkerTooLarge,
    /// Complete durable body frames exceed the configured per-height byte cap.
    #[error("Sumeragi v2 body store exceeds {capacity} body-frame bytes per height")]
    BodyByteCapacityExceeded {
        /// Maximum complete body-frame bytes for one height.
        capacity: u64,
    },
    /// Stored file uses an unsupported framing or envelope version.
    #[error("unsupported Sumeragi v2 body-store version {0}")]
    UnsupportedVersion(u16),
    /// Stored file framing, length, or trailing bytes are invalid.
    #[error("corrupt Sumeragi v2 body-store frame")]
    CorruptFrame,
    /// Stored frame checksum does not match its payload.
    #[error("Sumeragi v2 body-store checksum mismatch")]
    ChecksumMismatch,
    /// Norito envelope could not be decoded exactly.
    #[error("invalid Sumeragi v2 body-store envelope: {0}")]
    EnvelopeDecode(String),
    /// Durable validation marker could not be decoded exactly.
    #[error("invalid Sumeragi v2 validation marker: {0}")]
    ValidationMarkerDecode(String),
    /// Validation outcome payload uses an unsupported schema version.
    #[error("unsupported Sumeragi v2 validation outcome marker version {0}")]
    UnsupportedValidationOutcomeMarkerVersion(u16),
    /// Rejection marker names a code outside the closed canonical domain.
    #[error("unknown Sumeragi v2 validation rejection identity {0}")]
    UnknownValidationRejectionIdentity(u8),
    /// Stored metadata does not belong to the active context.
    #[error("Sumeragi v2 body-store context, round, subject, or manifest mismatch")]
    ContextMismatch,
    /// The opened body store is outside the canonical lifecycle storage owner.
    #[error("Sumeragi v2 body-store publication target mismatch")]
    StoreRootMismatch,
    /// Canonical body size cannot be represented safely.
    #[error("Sumeragi v2 body is too large")]
    BodyTooLarge,
    /// Manifest size differs from the exact wire byte length.
    #[error("Sumeragi v2 body length differs from its manifest")]
    BodyLengthMismatch,
    /// Exact wire byte hash differs from the proposal subject.
    #[error("Sumeragi v2 body payload hash mismatch")]
    PayloadHashMismatch,
    /// The stored manifest is not the unique RS16 encoding of the exact body.
    #[error("non-canonical Sumeragi v2 payload manifest")]
    NonCanonicalPayloadManifest,
    /// Exact body could not be decoded as `SignedBlockWire`.
    #[error("invalid canonical Sumeragi v2 block body: {0}")]
    BlockDecode(String),
    /// Decoded body could not be re-encoded canonically.
    #[error("failed to encode canonical Sumeragi v2 block body: {0}")]
    BlockEncode(String),
    /// Accepted bytes were not the unique canonical encoding.
    #[error("non-canonical Sumeragi v2 block wire bytes")]
    NonCanonicalBlockWire,
    /// Proposal ingress contained execution results or a result-root commitment.
    #[error("Sumeragi v2 proposal body must be resultless")]
    ResultBearingProposal,
    /// Header height/view/hash/parent differs from the proposal subject.
    #[error("Sumeragi v2 block header differs from its proposal subject")]
    BlockSubjectMismatch,
    /// Header parent differs from the frozen parent CommitQC.
    #[error("Sumeragi v2 block parent differs from the frozen height context")]
    ParentMismatch,
    /// Frozen leader index is not representable in the roster.
    #[error("Sumeragi v2 leader is outside the frozen roster")]
    LeaderOutOfRange,
    /// Selected signature policy is incompatible with the frozen height.
    #[error("Sumeragi v2 genesis signature policy is valid only at height one without a parent")]
    InvalidSignaturePolicy,
    /// Emergency Fast startup owns an inert body store and cannot mutate it.
    #[error("Sumeragi v2 body store is read-only during emergency Fast startup")]
    EmergencyFastReadOnly,
    /// Body contains no expected block signature.
    #[error("Sumeragi v2 body is missing its expected block signature")]
    MissingExpectedSignature,
    /// Body contains the wrong signature index or more than one block signature.
    #[error("Sumeragi v2 body must contain exactly its expected block signature")]
    InvalidExpectedSignatureSet,
    /// Expected block signature is cryptographically invalid.
    #[error("invalid Sumeragi v2 block signature")]
    InvalidExpectedSignature,
    /// Test-only synchronous deterministic validation rejected the body.
    #[cfg(test)]
    #[error("deterministic Sumeragi v2 body validation failed: {0}")]
    DeterministicValidation(String),
    /// Two final files map to one semantic round/subject key.
    #[error("duplicate Sumeragi v2 durable body key")]
    DuplicateBodyKey,
    /// More than one durable validation marker maps to a semantic body key.
    #[error("duplicate Sumeragi v2 durable validation marker")]
    DuplicateValidationMarker,
    /// Validation marker has no matching exact durable body.
    #[error("orphaned Sumeragi v2 validation marker")]
    OrphanedValidationMarker,
    /// Byte-identical bodies in one height context produced different execution commitments.
    #[error("conflicting Sumeragi v2 execution commitments for one exact body")]
    ConflictingValidationCommitment,
    /// Byte-identical bodies produced different closed validation outcomes.
    #[error("conflicting Sumeragi v2 validation outcomes for one exact body")]
    ConflictingValidationOutcome,
    /// Validation marker is not bound to the matching exact body frame.
    #[error("Sumeragi v2 validation marker differs from its durable body")]
    ValidationMarkerMismatch,
    /// Recovered marker commitment differs from deterministic replay.
    #[error("recovered Sumeragi v2 validation commitment differs from semantic replay")]
    RecoveredValidationCommitmentMismatch,
    /// Recovered success/rejection identity differs from deterministic replay.
    #[error("recovered Sumeragi v2 validation outcome differs from semantic replay")]
    RecoveredValidationOutcomeMismatch,
    /// The recovered-startup input was already touched by another validator.
    #[error("recovered Sumeragi v2 validation markers were already promoted before startup")]
    RecoveredMarkersAlreadyPromoted,
    /// Verified finality capability belongs to a different height context.
    #[error("verified Sumeragi v2 recovery finality belongs to a different height context")]
    RecoveredFinalityContextMismatch,
    /// WAL replay authority belongs to a different immutable height context.
    #[error("recovered Sumeragi v2 validation authority belongs to a different height context")]
    RecoveredValidationAuthorityContextMismatch,
    /// Runtime construction attempted to restore unvalidated local markers.
    #[error("recovered Sumeragi v2 validation markers require semantic replay")]
    UnrevalidatedValidationMarkers,
    /// A cold next-Vote lookup did not name one exact revalidated body owner.
    #[error("recovered Sumeragi v2 next-Vote body marker is not exact")]
    RecoveredLifecycleNextVoteBodyMismatch,
    /// Context directory contains an unrecognized final entry.
    #[error("unexpected Sumeragi v2 body-store entry: {}", .0.display())]
    UnexpectedEntry(PathBuf),
    /// Caller attempted to replace a durable body for the same subject.
    #[error("conflicting Sumeragi v2 durable body")]
    ConflictingBody,
    /// Receipt was not minted by this active store or no longer exists.
    #[error("Sumeragi v2 durable body receipt mismatch")]
    ReceiptMismatch,
    /// Kura receipt does not durably cover this body.
    #[error("Kura finality receipt does not match the pending Sumeragi v2 body")]
    KuraReceiptMismatch,
}
include!("v2_body_store_tests.rs");
