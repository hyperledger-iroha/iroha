//! Crash-safe storage for exact Sumeragi v2 proposal bodies.
//!
//! Consensus votes authenticate a [`wire::BlockSubject`], including the hash of the
//! exact canonical [`SignedBlock`] wire bytes.  This store is the durability
//! boundary between reconstruction and the reducer's `BodyStored` input: a
//! receipt can only be obtained after the bytes, their metadata, and the
//! directory entry have been synchronised. The first release has one V1 frame
//! layout and no predecessor-format reader or migration path.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::v2_core::EventTag;
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::block::{
    CertifiedMergeLedgerReference, SignedBlock, consensus_v2 as wire, decode_framed_signed_block,
};
use norito::codec::{Decode, DecodeAll as _, Encode};
use thiserror::Error;

use super::v2_chunks::encode_payload;
use super::{
    v2::{
        PreparedRecoveredLifecycleSignedBroadcastAndSignColdPreviewV1,
        RecoveredLifecycleNextVoteBodyAuthorityV1, RecoveredValidationAuthority,
        RecoveredWalVoteSign,
    },
    v2_apply::{V2ApplyError, V2ApplyService, VerifiedRecoveredFinalitySubject},
    v2_effects::{BodyStoreTask, BodyValidationTask, EffectWorkId},
    v2_lifecycle_coordinator::{
        AuthenticatedRecoveredWalDecisionFetchProjection,
        AuthenticatedRecoveredWalValidateLedgerParent, LifecycleContext,
        RecoveredDecisionApplyReplayLineageV1, TerminalValidateNoSuccessorClaim,
    },
    v2_transport::AuthenticatedCertifiedBodyResponse,
};
use crate::kura::KuraV2CommitReceipt;

const STORE_MAGIC: &[u8; 8] = b"SUM2BODY";
const VALIDATED_MAGIC: &[u8; 8] = b"SUM2VALD";
const STORE_VERSION: u16 = 1;
const VALIDATION_OUTCOME_MARKER_VERSION: u16 = 1;
const FRAME_HEADER_LEN: usize = STORE_MAGIC.len() + size_of::<u16>() + size_of::<u64>();
const CHECKSUM_LEN: usize = 32;
const FRAME_PAYLOAD_MAX_BYTES: u64 =
    wire::MAX_EXECUTED_BLOCK_WIRE_BYTES + wire::MAX_DA_ENCODED_PAYLOAD_BYTES;

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
    staged: super::v2::RecoveredDecisionApplyStagedStorageV1,
    body: RecoveredDecisionApplyBodyCut<'store>,
}

/// Storage-ready state after the exact body has been restored to its owner.
///
/// This value no longer borrows the body store. A persistence failure retains
/// all adapter and lifecycle authority here for restart-only handling.
#[must_use = "restored Decision Apply state must enter the single-fsync transaction"]
pub(in crate::sumeragi) struct RestoredRecoveredDecisionApplyStorageV1 {
    staged: super::v2::RecoveredDecisionApplyStagedStorageV1,
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
    pub(in crate::sumeragi) const fn staged(
        &self,
    ) -> &super::v2::RecoveredDecisionApplyStagedStorageV1 {
        &self.staged
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
    pub(in crate::sumeragi) const fn staged(
        &self,
    ) -> &super::v2::RecoveredDecisionApplyStagedStorageV1 {
        &self.staged
    }

    /// Consume the restored state only inside the lifecycle publication tail.
    pub(in crate::sumeragi) fn into_staged(
        self,
    ) -> super::v2::RecoveredDecisionApplyStagedStorageV1 {
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
    /// A rejection or sidecar deferral is returned intact so the future typed
    /// lifecycle transaction cannot accidentally discard or relabel it while
    /// attempting the success-only Validate completion path.
    pub(crate) fn into_validated_receipt(self) -> Result<ValidatedBodyReceipt, Self> {
        match self.0 {
            DurableBodyValidationOutcomeBody::Validated(receipt) => Ok(receipt),
            body => Err(Self(body)),
        }
    }

    fn into_body(self) -> DurableBodyValidationOutcomeBody {
        self.0
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

    /// Consume selected outcomes and restore every unselected catalog entry.
    pub(super) fn commit_selected(mut self) {
        self.restore_unselected();
        self.selected_validated.clear();
        self.selected_rejected.clear();
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

/// Result minted by the body-store service after reloading and validating the
/// exact durable body represented by a validation task.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) enum BodyValidationCompletion {
    /// Deterministic semantic validation succeeded.
    Validated {
        /// Stable asynchronous work identifier.
        work_id: EffectWorkId,
        /// Non-forgeable validation receipt.
        receipt: ValidatedBodyReceipt,
    },
    /// Deterministic semantic validation rejected the exact body.
    Rejected {
        /// Stable asynchronous work identifier.
        work_id: EffectWorkId,
        /// Deterministic validator diagnostic.
        reason: String,
    },
    /// Validation is sound but cannot finish until the exact certified merge
    /// sidecar referenced by the durable body is fetched and authenticated.
    DeferredMergeSidecar {
        /// Stable asynchronous work identifier retained for the exact retry.
        work_id: EffectWorkId,
        /// Complete compact reference needed by the bounded sidecar transport.
        reference: CertifiedMergeLedgerReference,
    },
}

impl BodyValidationCompletion {
    /// Stable asynchronous work identifier.
    pub(crate) const fn work_id(&self) -> EffectWorkId {
        match self {
            Self::Validated { work_id, .. }
            | Self::Rejected { work_id, .. }
            | Self::DeferredMergeSidecar { work_id, .. } => *work_id,
        }
    }

    /// Non-forgeable success receipt, when validation succeeded.
    pub(crate) const fn validated_receipt(&self) -> Option<&ValidatedBodyReceipt> {
        match self {
            Self::Validated { receipt, .. } => Some(receipt),
            Self::Rejected { .. } | Self::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Deterministic rejection diagnostic, when validation failed.
    pub(crate) fn rejection_reason(&self) -> Option<&str> {
        match self {
            Self::Rejected { reason, .. } => Some(reason),
            Self::Validated { .. } | Self::DeferredMergeSidecar { .. } => None,
        }
    }

    /// Compact certified merge reference whose absence deferred validation.
    pub(crate) const fn missing_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
        match self {
            Self::DeferredMergeSidecar { reference, .. } => Some(reference),
            Self::Validated { .. } | Self::Rejected { .. } => None,
        }
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
    /// Outcome seals retired from restart authority by a missing sidecar.
    ///
    /// These entries are comparison-only: an ordinary bounded retry must run
    /// the validator again and reproduce the exact durable outcome before it
    /// can promote one. They are never exposed by a recovery catalog.
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
    directory: PathBuf,
    parent: Option<PathBuf>,
}

impl V2BodyRetirementJob {
    /// Delete the finalized height's durable candidate bodies and fsync the
    /// containing directory.
    pub(crate) fn execute(self) -> Result<(), V2BodyStoreError> {
        match fs::remove_dir_all(&self.directory) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(V2BodyStoreError::Io {
                    path: self.directory,
                    source,
                });
            }
        }
        if let Some(parent) = self.parent {
            sync_directory(&parent)?;
        }
        Ok(())
    }
}

impl V2BodyStore {
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
    /// final body file. Incomplete `.tmp` files are unacknowledged writes and
    /// are deliberately ignored.
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
    pub(crate) fn open_with_policy(
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
        let directory = root.as_ref().join(hex::encode(context.id().0.as_ref()));
        fs::create_dir_all(&directory).map_err(|source| V2BodyStoreError::Io {
            path: directory.clone(),
            source,
        })?;
        sync_directory(&directory)?;
        if let Some(parent) = directory.parent() {
            sync_directory(parent)?;
        }

        let mut store = Self {
            identity: Arc::new(V2BodyStoreInstanceIdentityMarker),
            context,
            signature_policy,
            directory,
            entries: BTreeMap::new(),
            manifests: BTreeMap::new(),
            pending_revalidation: BTreeMap::new(),
            retired_revalidation: BTreeMap::new(),
            validated: BTreeMap::new(),
            rejected: BTreeMap::new(),
        };
        let mut paths = fs::read_dir(&store.directory)
            .map_err(|source| V2BodyStoreError::Io {
                path: store.directory.clone(),
                source,
            })?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|source| V2BodyStoreError::Io {
                        path: store.directory.clone(),
                        source,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        for path in paths
            .iter()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("norito"))
        {
            let (envelope, frame_hash) = read_envelope(&path)?;
            store.validate_envelope(&envelope)?;
            let receipt = receipt_for(&envelope, frame_hash);
            let key = (envelope.round, envelope.subject);
            if store.entries.insert(key, receipt).is_some()
                || store.manifests.insert(key, envelope.manifest).is_some()
            {
                return Err(V2BodyStoreError::DuplicateBodyKey);
            }
        }
        for path in paths
            .iter()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("validated"))
        {
            let marker = read_validation_outcome_marker(path)?;
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
        for path in &paths {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| V2BodyStoreError::UnexpectedEntry(path.clone()))?;
            let extension = path.extension().and_then(|value| value.to_str());
            if name.ends_with(".tmp") || matches!(extension, Some("norito" | "validated")) {
                continue;
            }
            return Err(V2BodyStoreError::UnexpectedEntry(path.clone()));
        }
        Ok(store)
    }

    /// Open an empty, context-addressed store for non-cryptographic lifecycle fixtures.
    ///
    /// Production must use [`Self::open_with_policy`]. This helper exists only
    /// because replay-authority unit fixtures deliberately retain structural
    /// certificates rather than usable voting signatures.
    #[cfg(test)]
    pub(in crate::sumeragi) fn open_lifecycle_fixture_for_test(
        root: impl AsRef<Path>,
        context: wire::HeightContext,
        signature_policy: BlockSignaturePolicy,
    ) -> Result<Self, V2BodyStoreError> {
        let directory = root.as_ref().join(hex::encode(context.id().0.as_ref()));
        fs::create_dir_all(&directory).map_err(|source| V2BodyStoreError::Io {
            path: directory.clone(),
            source,
        })?;
        Ok(Self {
            identity: Arc::new(V2BodyStoreInstanceIdentityMarker),
            context,
            signature_policy,
            directory,
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
    /// bytes remain available for bounded cleanup; their quarantined and
    /// already-promoted marker capabilities are dropped from the in-memory
    /// recovery catalogs.
    pub(crate) fn retain_recovered_markers_for_subject(
        &mut self,
        decision: VerifiedRecoveredFinalitySubject,
    ) -> Result<(), V2BodyStoreError> {
        if !decision.authorizes_context(&self.context) {
            return Err(V2BodyStoreError::RecoveredFinalityContextMismatch);
        }
        let subject = decision.subject();
        self.pending_revalidation
            .retain(|(_, candidate), _| *candidate == subject);
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
        self.pending_revalidation
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
        self.validated
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
        self.rejected
            .retain(|(round, subject), _| authority.authorizes(*round, *subject));
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
    // TODO: Runner cutover must call this immediately after semantic marker
    // replay and move the result into the unified lifecycle-owner factory.
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
        let block = decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))?;

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
    // DURABLE_BODY_VALIDATION_API_END

    /// Execute deterministic validation against the exact durable task body.
    ///
    /// Filesystem loading, canonical decoding, and the validator callback all
    /// run in the caller's storage/validation service context, never on the
    /// serialized reducer owner. Production callers use
    /// `ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block` so the
    /// store-verified proposal-round signature is not redundantly rechecked
    /// while all transaction/state checks remain. Validation success is bound
    /// to the exact durable proposal round; a marker from another view cannot
    /// authorize this task. Final application re-executes and checks the
    /// commitment.
    pub(crate) fn execute_validation_task<F, E>(
        &mut self,
        task: &BodyValidationTask,
        validator: F,
    ) -> Result<BodyValidationCompletion, V2BodyStoreError>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        if task.round() != task.durable_receipt().round()
            || task.subject() != task.durable_receipt().subject()
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let work_id = task.id();
        let outcome = self.execute_durable_validation(
            task.durable_receipt().clone(),
            task.durable_receipt().manifest_hash(),
            validator,
        )?;
        match outcome.into_body() {
            DurableBodyValidationOutcomeBody::Validated(receipt) => {
                Ok(BodyValidationCompletion::Validated { work_id, receipt })
            }
            DurableBodyValidationOutcomeBody::Rejected { reason, .. } => {
                Ok(BodyValidationCompletion::Rejected { work_id, reason })
            }
            DurableBodyValidationOutcomeBody::DeferredMergeSidecar { reference, .. } => {
                Ok(BodyValidationCompletion::DeferredMergeSidecar { work_id, reference })
            }
        }
    }

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

        let encoded = envelope.encode();
        let framed = frame_payload(&encoded)?;
        let frame_hash = Hash::new(&framed);
        let path = self.path_for(envelope.round, envelope.subject);
        write_atomic_synced(&path, &framed)?;
        let receipt = receipt_for(&envelope, frame_hash);
        self.entries.insert(key, receipt.clone());
        self.manifests.insert(key, envelope.manifest);
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
        write_validation_outcome_marker(
            &self.validated_path_for(receipt.round, receipt.subject),
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
        write_validation_outcome_marker(
            &self.validated_path_for(receipt.round, receipt.subject),
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
        self,
        kura_receipt: &KuraV2CommitReceipt,
    ) -> Result<V2BodyRetirementJob, V2BodyStoreError> {
        if kura_receipt.context_id() != self.context.id()
            || kura_receipt.height() != self.context.height
        {
            return Err(V2BodyStoreError::KuraReceiptMismatch);
        }
        Ok(V2BodyRetirementJob {
            parent: self.directory.parent().map(Path::to_path_buf),
            directory: self.directory,
        })
    }

    fn load_envelope(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> Result<StoredBodyEnvelope, V2BodyStoreError> {
        self.verify_receipt(receipt)?;
        let path = self.path_for(receipt.round, receipt.subject);
        let (envelope, frame_hash) = read_envelope(&path)?;
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
        let key_hash = Hash::new((round, subject).encode());
        self.directory.join(format!(
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
        self.path_for(round, subject).with_extension("validated")
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
    frame_payload_with_magic(STORE_MAGIC, payload)
}

fn frame_payload_with_magic(magic: &[u8; 8], payload: &[u8]) -> Result<Vec<u8>, V2BodyStoreError> {
    let payload_len = u64::try_from(payload.len()).map_err(|_| V2BodyStoreError::BodyTooLarge)?;
    if payload_len > FRAME_PAYLOAD_MAX_BYTES {
        return Err(V2BodyStoreError::BodyTooLarge);
    }
    let capacity = FRAME_HEADER_LEN
        .checked_add(payload.len())
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or(V2BodyStoreError::BodyTooLarge)?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(capacity)
        .map_err(|_| V2BodyStoreError::BodyTooLarge)?;
    frame.extend_from_slice(magic);
    frame.extend_from_slice(&STORE_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(payload);
    frame.extend_from_slice(Hash::new(payload).as_ref());
    Ok(frame)
}

fn read_envelope(path: &Path) -> Result<(StoredBodyEnvelope, Hash), V2BodyStoreError> {
    let (payload, frame_hash) = read_frame_payload_with_hash(path, STORE_MAGIC)?;
    let mut cursor = payload.as_slice();
    let envelope = StoredBodyEnvelope::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::EnvelopeDecode(error.to_string()))?;
    Ok((envelope, frame_hash))
}

fn read_frame_payload(path: &Path, magic: &[u8; 8]) -> Result<Vec<u8>, V2BodyStoreError> {
    read_frame_payload_with_hash(path, magic).map(|(payload, _)| payload)
}

fn read_frame_payload_with_hash(
    path: &Path,
    magic: &[u8; 8],
) -> Result<(Vec<u8>, Hash), V2BodyStoreError> {
    let mut file = File::open(path).map_err(|source| V2BodyStoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let opened_len = file
        .metadata()
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    let maximum_frame_bytes = FRAME_PAYLOAD_MAX_BYTES
        .checked_add(
            u64::try_from(FRAME_HEADER_LEN + CHECKSUM_LEN).expect("frame overhead fits u64"),
        )
        .ok_or(V2BodyStoreError::BodyTooLarge)?;
    if opened_len > maximum_frame_bytes {
        return Err(V2BodyStoreError::BodyTooLarge);
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
    if payload_len > FRAME_PAYLOAD_MAX_BYTES {
        return Err(V2BodyStoreError::BodyTooLarge);
    }
    let payload_len = usize::try_from(payload_len).map_err(|_| V2BodyStoreError::BodyTooLarge)?;
    let expected_len = FRAME_HEADER_LEN
        .checked_add(payload_len)
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or(V2BodyStoreError::BodyTooLarge)?;
    if opened_len != u64::try_from(expected_len).map_err(|_| V2BodyStoreError::BodyTooLarge)? {
        return Err(V2BodyStoreError::CorruptFrame);
    }

    let mut payload = Vec::new();
    payload
        .try_reserve_exact(payload_len)
        .map_err(|_| V2BodyStoreError::BodyTooLarge)?;
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

fn write_validation_outcome_marker(
    path: &Path,
    marker: &ValidationOutcomeMarker,
) -> Result<(), V2BodyStoreError> {
    let payload = marker.encode();
    let frame = frame_payload_with_magic(VALIDATED_MAGIC, &payload)?;
    write_atomic_synced(path, &frame)
}

fn read_validation_outcome_marker(
    path: &Path,
) -> Result<ValidationOutcomeMarker, V2BodyStoreError> {
    let payload = read_frame_payload(path, VALIDATED_MAGIC)?;
    let mut cursor = payload.as_slice();
    ValidationOutcomeMarker::decode_all(&mut cursor)
        .map_err(|error| V2BodyStoreError::ValidationMarkerDecode(error.to_string()))
}

fn write_atomic_synced(path: &Path, bytes: &[u8]) -> Result<(), V2BodyStoreError> {
    let tmp_path = path.with_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map_or_else(|| "tmp".to_owned(), |extension| format!("{extension}.tmp")),
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)
        .map_err(|source| V2BodyStoreError::Io {
            path: tmp_path.clone(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| V2BodyStoreError::Io {
            path: tmp_path.clone(),
            source,
        })?;
    fs::rename(&tmp_path, path).map_err(|source| V2BodyStoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let parent = path.parent().ok_or(V2BodyStoreError::MissingParent)?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), V2BodyStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| V2BodyStoreError::Io {
            path: path.to_path_buf(),
            source,
        })
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
    /// Frozen height context is structurally invalid.
    #[error("invalid Sumeragi v2 body-store height context: {0}")]
    Context(#[from] wire::ValidationError),
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
    /// Atomic destination has no parent directory.
    #[error("Sumeragi v2 body-store path has no parent directory")]
    MissingParent,
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, fs, num::NonZeroU64, path::Path};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        NetworkId,
        block::{
            BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
            consensus_v2 as wire, decode_framed_signed_block,
        },
        merge::MergeQuorumCertificate,
        peer::PeerId,
    };
    use tempfile::TempDir;

    use super::{
        BlockSignaturePolicy, BodyValidationCompletion, BodyValidationError,
        BodyValidationRejectionIdentity, CHECKSUM_LEN, FRAME_PAYLOAD_MAX_BYTES,
        QuarantinedValidationOutcome, RecoveredTerminalValidateOutcomeCatalogError,
        RevalidatedRejectedBody, STORE_MAGIC, STORE_VERSION, V2BodyStore, V2BodyStoreError,
        VALIDATED_MAGIC, VALIDATION_OUTCOME_MARKER_VERSION, ValidatedBodyReceipt,
        ValidationOutcomeMarker, ValidationOutcomeMarkerKind, read_frame_payload_with_hash,
        write_validation_outcome_marker,
    };

    use crate::sumeragi::{
        v2::RecoveredValidationAuthority, v2_apply::VerifiedRecoveredFinalitySubject,
        v2_chunks::encode_payload, v2_effects::BodyValidationTask,
    };

    fn test_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x94; Hash::LENGTH]),
        ))
    }

    #[derive(Debug)]
    enum FixtureValidationError {
        MissingMergeSidecar(CertifiedMergeLedgerReference),
        Invalid(&'static str),
    }

    impl std::fmt::Display for FixtureValidationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MissingMergeSidecar(reference) => {
                    write!(formatter, "missing merge sidecar {}", reference.entry_hash)
                }
                Self::Invalid(reason) => formatter.write_str(reason),
            }
        }
    }

    impl BodyValidationError for FixtureValidationError {
        fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
            match self {
                Self::MissingMergeSidecar(reference) => Some(reference),
                Self::Invalid(_) => None,
            }
        }
    }

    fn context_and_keys() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: test_network_id(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"test nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_048_576,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 2,
            },
            leader_seed: [0x42; 32],
        };
        (context, keys)
    }

    fn missing_merge_reference(
        receipt: &super::DurableBodyReceipt,
    ) -> CertifiedMergeLedgerReference {
        let parent_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"body-store validation parent"));
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"body-store missing merge sidecar",
            )),
            encoded_len: 512,
            epoch_id: 7,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                receipt.round().view,
                7,
                receipt.round().height,
                parent_hash,
                test_network_id(),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"body-store validation certificate"),
            ),
        }
    }

    fn body_and_manifest(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        signing_key_index: Option<usize>,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("leader index");
        body_and_manifest_with_signature(
            context,
            &keys[signing_key_index.unwrap_or(leader_index)],
            u64::from(leader),
        )
    }

    fn body_and_manifest_with_signature(
        context: &wire::HeightContext,
        signing_key: &KeyPair,
        signature_index: u64,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        body_and_manifest_with_signature_and_views(context, signing_key, signature_index, 0, 0)
    }

    fn body_and_manifest_with_signature_and_views(
        context: &wire::HeightContext,
        signing_key: &KeyPair,
        signature_index: u64,
        proposal_view: u64,
        header_view: u64,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: proposal_view,
        };
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero height"),
            None,
            None,
            None,
            1_000,
            header_view,
        );
        let signature = SignatureOf::try_from_hash(signing_key.private_key(), header.hash())
            .expect("sign block header");
        let block = SignedBlock::presigned(
            BlockSignature::new(signature_index, signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block.encode_wire().expect("canonical block wire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(context, round, subject, &canonical_wire)
            .expect("encode canonical fixture payload")
            .manifest()
            .clone();
        (canonical_wire, manifest)
    }

    #[test]
    fn body_store_instance_identity_distinguishes_a_same_path_reopen() {
        let directory = TempDir::new().expect("temporary identity body store");
        let (context, _) = context_and_keys();
        let store = V2BodyStore::open(directory.path(), context.clone())
            .expect("open first body-store instance");
        let first = store.instance_identity();
        assert!(first.same_instance(&store.instance_identity()));

        let reopened = V2BodyStore::open(directory.path(), context)
            .expect("reopen the same body-store path independently");
        assert!(
            !first.same_instance(&reopened.instance_identity()),
            "path and context equality cannot substitute for move-only instance ownership"
        );
    }

    fn store_with_promoted_terminal_outcomes(
        directory: &Path,
        context: &wire::HeightContext,
        keys: &[KeyPair],
    ) -> V2BodyStore {
        let (validated_body, validated_manifest) = body_and_manifest(context, keys, None);
        let mut store =
            V2BodyStore::open(directory, context.clone()).expect("open terminal outcome store");
        let validated_receipt = store
            .store(validated_manifest, validated_body)
            .expect("persist terminal success body");
        let commitment =
            ValidatedBodyReceipt::for_test(validated_receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&validated_receipt, commitment)
            .expect("promote terminal success");

        let rejected_view = 1;
        let rejected_leader = context.leader(rejected_view);
        let rejected_leader_index =
            usize::try_from(rejected_leader).expect("rejected leader index");
        let (rejected_body, rejected_manifest) = body_and_manifest_with_signature_and_views(
            context,
            &keys[rejected_leader_index],
            u64::from(rejected_leader),
            rejected_view,
            rejected_view,
        );
        let rejected_receipt = store
            .store(rejected_manifest, rejected_body)
            .expect("persist terminal rejection body");
        let _rejected = store
            .persist_rejected_outcome(
                &rejected_receipt,
                BodyValidationRejectionIdentity::Rejected.canonical_code(),
                "volatile terminal rejection diagnostic".to_owned(),
            )
            .expect("promote terminal rejection");
        store
    }

    fn durable_files_snapshot(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, files: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
            let mut entries = fs::read_dir(directory)
                .expect("read body-store snapshot directory")
                .map(|entry| entry.expect("read body-store snapshot entry").path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root)
                            .expect("snapshot entry belongs to root")
                            .to_path_buf(),
                        fs::read(&path).expect("read body-store snapshot file"),
                    ));
                }
            }
        }

        let mut files = Vec::new();
        visit(root, root, &mut files);
        files
    }

    #[test]
    fn durable_body_roundtrips_and_reopens_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        assert!(store.matches_context(&context));
        let mut foreign_context = context.clone();
        foreign_context.height = foreign_context.height.saturating_add(1);
        assert!(!store.matches_context(&foreign_context));
        let receipt = store
            .store(manifest.clone(), body.clone())
            .expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let validated = store
            .validate(&receipt, |block| {
                (block.hash() == receipt.subject().block_hash)
                    .then_some(execution_commitment)
                    .ok_or("wrong block")
            })
            .expect("validate exact durable body");
        assert_eq!(validated.durable(), &receipt);
        assert_eq!(validated.execution_commitment(), execution_commitment);
        assert_eq!(
            store
                .load(&receipt)
                .expect("load exact body")
                .encode_wire()
                .unwrap(),
            body
        );
        assert_eq!(
            store
                .store(manifest.clone(), body)
                .expect("idempotent store returns same receipt"),
            receipt
        );

        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context).expect("replay store");
        assert_eq!(
            reopened.receipt(manifest.round, manifest.subject),
            Some(receipt.clone())
        );
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("reload recovered manifest"),
            Some((manifest, receipt.clone()))
        );
        assert_eq!(
            reopened
                .load(&receipt)
                .expect("receipt remains valid after replay")
                .hash(),
            receipt.subject().block_hash
        );
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
        let callback_ran = Cell::new(false);
        let _validated = reopened
            .validate(&receipt, |_| {
                callback_ran.set(true);
                Ok::<wire::ExecutionCommitment, &str>(execution_commitment)
            })
            .expect("durable validation marker resumes after semantic replay");
        assert!(callback_ran.get());
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("recovered marker crossed semantic replay");
        assert_eq!(
            reopened
                .validated_recovery_catalog()
                .get(&(receipt.round(), receipt.subject()))
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment),
        );
    }

    #[test]
    fn recovered_marker_cannot_restore_vote_authority_without_semantic_replay() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let expected = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, expected)
            .expect("persist legitimate validation marker");

        let forged = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"forged parent root"),
            Hash::new(b"forged post root"),
            Hash::new(b"forged ordinary writes"),
            1,
            Hash::new(b"forged executed block"),
        );
        assert_ne!(expected, forged);
        let marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
            outcome: ValidationOutcomeMarkerKind::Validated(forged),
        };
        write_validation_outcome_marker(
            &store.validated_path_for(receipt.round(), receipt.subject()),
            &marker,
        )
        .expect("substitute a checksum-valid local marker");
        drop(store);

        let mut reopened = V2BodyStore::open(directory.path(), context)
            .expect("structurally read substituted marker");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(expected)
            }),
            Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch)
        ));
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
    }

    #[test]
    fn recovered_marker_missing_sidecar_retires_authority_without_losing_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, execution_commitment)
            .expect("persist validation marker");
        drop(store);

        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "terminal recovered validation failure",
                ))
            }),
            Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch)
        ));
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));

        let reference = missing_merge_reference(&receipt);
        reopened
            .revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("missing sidecar retires marker authority without failing startup");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("no untrusted marker authority survives startup");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(reopened.retired_revalidation.len(), 1);
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("inspect retained exact body"),
            Some((manifest, receipt.clone()))
        );

        let task = BodyValidationTask::for_test(43, receipt.clone());
        let deferred = reopened
            .execute_validation_task(&task, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("ordinary validation defers on the exact missing sidecar");
        assert!(matches!(
            deferred,
            BodyValidationCompletion::DeferredMergeSidecar {
                reference: deferred_reference,
                ..
            } if deferred_reference == reference
        ));
        assert!(reopened.validated_recovery_catalog().is_empty());

        let validated = reopened
            .execute_validation_task(&task, |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("ordinary bounded retry validates after sidecar recovery");
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        assert!(reopened.retired_revalidation.is_empty());
    }

    #[test]
    fn wal_frontier_bounds_many_view_restart_validation_work() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in 0_u64..32 {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store view candidate");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist view validation marker");
            receipts.push((receipt, commitment));
        }
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen view catalog");
        let selected = [receipts[7].0.clone(), receipts[31].0.clone()];
        let authority = RecoveredValidationAuthority::for_test(
            &context,
            selected
                .iter()
                .map(|receipt| (receipt.round(), receipt.subject())),
        );
        assert_eq!(authority.len(), 2);
        reopened
            .retain_recovered_markers_for_authority(authority)
            .expect("WAL frontier belongs to the exact body context");

        let callback_count = Cell::new(0_usize);
        reopened
            .revalidate_recovered_markers(|block| {
                callback_count.set(callback_count.get().saturating_add(1));
                receipts
                    .iter()
                    .find_map(|(receipt, commitment)| {
                        (receipt.subject().block_hash == block.hash()).then_some(*commitment)
                    })
                    .ok_or_else(|| "replayed an unauthorized body".to_owned())
            })
            .expect("revalidate only the authenticated WAL frontier");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(reopened.validated_recovery_catalog().len(), 2);
        assert_eq!(
            reopened
                .recovery_catalog()
                .expect("retained body catalog")
                .len(),
            32,
            "superseded markers lose authority without deleting DA body evidence"
        );
    }

    #[test]
    fn wal_frontier_capability_cannot_cross_height_contexts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, commitment)
            .expect("persist validation marker");
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let pending_before = reopened.pending_revalidation.clone();
        let validated_before = reopened.validated.clone();
        let mut foreign_context = context;
        foreign_context.leader_seed[0] ^= 0x40;
        let foreign_round = wire::ConsensusRound {
            context_id: foreign_context.id(),
            height: foreign_context.height,
            view: receipt.round().view,
        };
        let authority = RecoveredValidationAuthority::for_test(
            &foreign_context,
            [(foreign_round, receipt.subject())],
        );

        assert!(matches!(
            reopened.retain_recovered_markers_for_authority(authority),
            Err(V2BodyStoreError::RecoveredValidationAuthorityContextMismatch)
        ));
        assert_eq!(reopened.pending_revalidation, pending_before);
        assert_eq!(reopened.validated, validated_before);
    }

    #[test]
    fn verified_decision_retires_losing_restart_marker_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in [0_u64, 1] {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store candidate body");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist candidate validation marker");
            receipts.push((receipt, commitment));
        }
        assert_ne!(receipts[0].0.subject(), receipts[1].0.subject());
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &context,
                receipts[0].0.subject(),
            ))
            .expect("verified decision belongs to the recovered context");
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(receipts[0].1)
            })
            .expect("revalidate only the verified decision");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("losing marker authority was retired");
        let catalog = reopened.validated_recovery_catalog();
        assert!(catalog.contains_key(&(receipts[0].0.round(), receipts[0].0.subject())));
        assert!(!catalog.contains_key(&(receipts[1].0.round(), receipts[1].0.subject())));
    }

    #[test]
    fn verified_decision_capability_cannot_cross_height_contexts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store candidate body");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, commitment)
            .expect("persist candidate validation marker");
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let pending_before = reopened.pending_revalidation.clone();
        let validated_before = reopened.validated.clone();
        let mut foreign_context = context.clone();
        foreign_context.leader_seed[0] ^= 0x80;
        assert_ne!(foreign_context.id(), context.id());

        let error = reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &foreign_context,
                receipt.subject(),
            ))
            .expect_err("foreign finality capability must fail closed");
        assert!(matches!(
            error,
            V2BodyStoreError::RecoveredFinalityContextMismatch
        ));
        assert_eq!(reopened.pending_revalidation, pending_before);
        assert_eq!(reopened.validated, validated_before);
    }

    #[test]
    fn verified_decision_retires_already_promoted_losing_marker_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in [0_u64, 1] {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store candidate body");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist candidate validation marker");
            receipts.push((receipt, commitment));
        }
        assert_ne!(receipts[0].0.subject(), receipts[1].0.subject());
        drop(store);

        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let _validated = reopened
            .validate(&receipts[1].0, |_| {
                Ok::<wire::ExecutionCommitment, &str>(receipts[1].1)
            })
            .expect("promote the losing recovered marker before finality filtering");
        assert!(
            reopened
                .validated_recovery_catalog()
                .contains_key(&(receipts[1].0.round(), receipts[1].0.subject()))
        );

        reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &context,
                receipts[0].0.subject(),
            ))
            .expect("verified decision belongs to the recovered context");
        assert!(reopened.validated_recovery_catalog().is_empty());
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(receipts[0].1)
            })
            .expect("revalidate only the verified decision");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("all losing marker authority was retired");
        let catalog = reopened.validated_recovery_catalog();
        assert!(catalog.contains_key(&(receipts[0].0.round(), receipts[0].0.subject())));
        assert!(!catalog.contains_key(&(receipts[1].0.round(), receipts[1].0.subject())));
    }

    #[test]
    fn non_v1_body_and_validation_frames_are_rejected() {
        const UNSUPPORTED_VERSION: u16 = 2;

        let body_directory = TempDir::new().expect("temporary body directory");
        let (body_context, body_keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&body_context, &body_keys, None);
        let mut body_store =
            V2BodyStore::open(body_directory.path(), body_context.clone()).expect("open store");
        let body_receipt = body_store.store(manifest, body).expect("store body");
        let body_path = body_store.path_for(body_receipt.round(), body_receipt.subject());
        drop(body_store);
        let mut body_frame = fs::read(&body_path).expect("read body frame");
        assert_eq!(
            u16::from_le_bytes(
                body_frame[STORE_MAGIC.len()..STORE_MAGIC.len() + size_of::<u16>()]
                    .try_into()
                    .expect("body frame version has fixed width"),
            ),
            STORE_VERSION,
        );
        body_frame[STORE_MAGIC.len()..STORE_MAGIC.len() + size_of::<u16>()]
            .copy_from_slice(&UNSUPPORTED_VERSION.to_le_bytes());
        fs::write(&body_path, body_frame).expect("write unsupported body frame");
        assert!(matches!(
            V2BodyStore::open(body_directory.path(), body_context),
            Err(V2BodyStoreError::UnsupportedVersion(UNSUPPORTED_VERSION))
        ));

        let marker_directory = TempDir::new().expect("temporary marker directory");
        let (marker_context, marker_keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&marker_context, &marker_keys, None);
        let mut marker_store = V2BodyStore::open(marker_directory.path(), marker_context.clone())
            .expect("open marker store");
        let marker_receipt = marker_store
            .store(manifest, body)
            .expect("store marker body");
        let commitment =
            ValidatedBodyReceipt::for_test(marker_receipt.clone()).execution_commitment();
        let _validated_receipt = marker_store
            .validate(&marker_receipt, |_| Ok::<_, &'static str>(commitment))
            .expect("persist validation marker");
        let marker_path =
            marker_store.validated_path_for(marker_receipt.round(), marker_receipt.subject());
        drop(marker_store);
        let mut marker_frame = fs::read(&marker_path).expect("read validation frame");
        assert_eq!(
            u16::from_le_bytes(
                marker_frame[VALIDATED_MAGIC.len()..VALIDATED_MAGIC.len() + size_of::<u16>()]
                    .try_into()
                    .expect("validation frame version has fixed width"),
            ),
            VALIDATION_OUTCOME_MARKER_VERSION,
        );
        marker_frame[VALIDATED_MAGIC.len()..VALIDATED_MAGIC.len() + size_of::<u16>()]
            .copy_from_slice(&UNSUPPORTED_VERSION.to_le_bytes());
        fs::write(&marker_path, marker_frame).expect("write unsupported validation frame");
        assert!(matches!(
            V2BodyStore::open(marker_directory.path(), marker_context),
            Err(V2BodyStoreError::UnsupportedVersion(UNSUPPORTED_VERSION))
        ));
    }

    #[test]
    fn rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, origin_manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open body store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store the origin-view body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(origin_receipt.clone()).execution_commitment();
        let origin_task = BodyValidationTask::for_test(51, origin_receipt.clone());
        let first_callback_ran = Cell::new(false);
        let origin_validation = store
            .execute_validation_task(&origin_task, |_| {
                first_callback_ran.set(true);
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("validate the exact body once");
        assert!(first_callback_ran.get());
        assert_eq!(
            origin_validation
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );

        drop(store);
        let mut store = V2BodyStore::open(directory.path(), context.clone())
            .expect("recover the exact origin-view validation marker");
        store
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(execution_commitment)
            })
            .expect("semantically replay the recovered origin marker");

        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode the exact body for its later view")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("the original leader signature authenticates an unchanged reproposal body");
        let callback_ran = Cell::new(false);
        let later_validation = store
            .execute_validation_task(&BodyValidationTask::for_test(52, later_receipt), |_| {
                callback_ran.set(true);
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("revalidate the unchanged body under its new proposal round");
        assert!(
            callback_ran.get(),
            "validation markers never promote across rounds"
        );
        assert_eq!(
            later_validation
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        assert!(
            store
                .validated_path_for(later_round, origin_manifest.subject)
                .exists()
        );
        assert_eq!(
            store
                .validated_recovery_catalog()
                .get(&(origin_manifest.round, origin_manifest.subject))
                .map(ValidatedBodyReceipt::durable),
            Some(&origin_receipt)
        );
    }

    #[test]
    fn locked_body_reproposal_cannot_change_rejection_into_success() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, origin_manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store origin body");
        let _rejected = store
            .execute_durable_validation(
                origin_receipt.clone(),
                origin_receipt.manifest_hash(),
                |_| {
                    Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                        "origin body is invalid",
                    ))
                },
            )
            .expect("persist origin rejection");

        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode unchanged body for later proposal round")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("store unchanged later-round body");
        let later_manifest_hash = later_receipt.manifest_hash();
        let success = ValidatedBodyReceipt::for_test(later_receipt.clone()).execution_commitment();
        assert!(matches!(
            store.execute_durable_validation(
                later_receipt.clone(),
                later_receipt.manifest_hash(),
                |_| Ok::<_, FixtureValidationError>(success),
            ),
            Err(V2BodyStoreError::ConflictingValidationOutcome)
        ));
        assert!(
            !store
                .validated_path_for(later_round, origin_manifest.subject)
                .exists(),
            "a conflicting outcome must not become durable"
        );

        let later_rejection = store
            .execute_durable_validation(later_receipt, later_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "later round reproduces rejection",
                ))
            })
            .expect("same closed rejection is consistent across proposal rounds");
        assert_eq!(
            later_rejection.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
    }

    #[test]
    fn genesis_cross_view_validation_is_reexecuted_and_conflicts_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xC4; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let (body, origin_manifest) =
            body_and_manifest_with_signature_and_views(&context, &genesis, 0, 0, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store the origin-view body");
        let origin_commitment =
            ValidatedBodyReceipt::for_test(origin_receipt.clone()).execution_commitment();
        let _ = store
            .persist_validated_receipt(&origin_receipt, origin_commitment)
            .expect("persist the origin-view validation witness");

        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode the exact body for its later view")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("durably bind the exact body to the later round");
        let conflicting_commitment =
            ValidatedBodyReceipt::for_test(later_receipt.clone()).execution_commitment();
        assert_ne!(origin_commitment, conflicting_commitment);

        let callback_ran = Cell::new(false);
        let later_task = BodyValidationTask::for_test(52, later_receipt.clone());
        let error = store
            .execute_validation_task(&later_task, |_| {
                callback_ran.set(true);
                Ok::<_, FixtureValidationError>(conflicting_commitment)
            })
            .expect_err("a prior-view marker must not bypass exact-round validation");
        assert!(
            callback_ran.get(),
            "the later proposal round must be revalidated"
        );
        assert!(matches!(
            error,
            V2BodyStoreError::ConflictingValidationCommitment
        ));
        let marker_path = store.validated_path_for(later_round, origin_manifest.subject);
        assert!(!marker_path.exists());

        let conflicting_marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: later_receipt.context_id,
            round: later_receipt.round,
            subject: later_receipt.subject,
            manifest_hash: later_receipt.manifest_hash,
            body_frame_hash: later_receipt.frame_hash,
            outcome: ValidationOutcomeMarkerKind::Validated(conflicting_commitment),
        };
        write_validation_outcome_marker(&marker_path, &conflicting_marker)
            .expect("write a syntactically valid conflicting marker");
        drop(store);

        let error = match V2BodyStore::open_with_policy(
            directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        ) {
            Ok(_) => panic!("recovery must reject conflicting exact-body commitments"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            V2BodyStoreError::ConflictingValidationCommitment
        ));
    }

    #[test]
    fn result_bearing_proposal_is_rejected_before_durable_admission() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut result_bearing = decode_framed_signed_block(&body).expect("decode fixture body");
        result_bearing
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach empty deterministic execution result");
        assert!(!result_bearing.is_resultless_proposal());
        let result_bearing_wire = result_bearing
            .encode_wire()
            .expect("encode result-bearing body");
        let subject = wire::BlockSubject {
            parent_block_hash: result_bearing.header().prev_block_hash(),
            block_hash: result_bearing.hash(),
            payload_hash: Hash::new(&result_bearing_wire),
        };
        let result_bearing_manifest =
            encode_payload(&context, manifest.round, subject, &result_bearing_wire)
                .expect("encode result-bearing fixture payload")
                .manifest()
                .clone();
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        assert!(matches!(
            store.store(result_bearing_manifest, result_bearing_wire),
            Err(V2BodyStoreError::ResultBearingProposal)
        ));
    }

    #[test]
    fn typed_validation_deferral_and_durable_rejection_never_mint_success_receipts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let task = BodyValidationTask::for_test(41, receipt.clone());
        let reference = missing_merge_reference(&receipt);
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();

        let deferred = store
            .execute_validation_task(&task, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("classify exact missing sidecar as deferred");
        assert!(matches!(
            deferred,
            BodyValidationCompletion::DeferredMergeSidecar {
                reference: deferred_reference,
                ..
            } if deferred_reference == reference
        ));
        assert!(store.validated_recovery_catalog().is_empty());
        assert!(
            !store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );

        let rejected = store
            .execute_validation_task(&task, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "invalid candidate",
                ))
            })
            .expect("return terminal deterministic rejection");
        assert_eq!(rejected.rejection_reason(), Some("invalid candidate"));
        assert!(store.validated_recovery_catalog().is_empty());
        assert!(
            store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );
        let marker_before_repeat =
            fs::read(store.validated_path_for(receipt.round(), receipt.subject()))
                .expect("rejection marker is durable before returning the outcome");

        let callback_ran = Cell::new(false);
        let repeated = store
            .execute_validation_task(&task, |_| {
                callback_ran.set(true);
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("exact repeat reuses the durable rejection");
        assert!(!callback_ran.get());
        assert_eq!(repeated.rejection_reason(), Some("invalid candidate"));
        assert_eq!(
            fs::read(store.validated_path_for(receipt.round(), receipt.subject()))
                .expect("read repeated rejection marker"),
            marker_before_repeat,
        );
        assert!(matches!(
            store.persist_validated_receipt(&receipt, execution_commitment),
            Err(V2BodyStoreError::ConflictingValidationOutcome)
        ));
        assert!(store.validated_recovery_catalog().is_empty());
    }

    #[test]
    fn durable_validation_persists_success_and_repeats_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let expected_manifest_hash = receipt.manifest_hash();
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();

        let validated = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("durable validation succeeds");
        assert_eq!(validated.durable_body(), &receipt);
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let marker_before_repeat = fs::read(&marker_path)
            .expect("success marker is durable before the outcome is returned");
        let files_before_repeat = durable_files_snapshot(directory.path());

        let validator_called = Cell::new(false);
        let repeated = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                validator_called.set(true);
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "idempotent validation must not rerun the callback",
                ))
            })
            .expect("repeat reuses the exact durable success");
        assert!(!validator_called.get());
        assert_eq!(repeated.durable_body(), &receipt);
        assert_eq!(repeated.validated_receipt(), validated.validated_receipt());
        assert_eq!(
            fs::read(marker_path).expect("read repeated success marker"),
            marker_before_repeat
        );
        assert_eq!(
            durable_files_snapshot(directory.path()),
            files_before_repeat
        );
        assert_eq!(
            repeated
                .into_validated_receipt()
                .expect("success-only extraction accepts the durable validation")
                .execution_commitment(),
            execution_commitment
        );
    }

    #[test]
    fn durable_validation_binds_rejection_and_typed_deferral_to_the_exact_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let expected_manifest_hash = receipt.manifest_hash();
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let files_before = durable_files_snapshot(directory.path());

        let reference = missing_merge_reference(&receipt);
        let deferred = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("return a reference-bound deferral outcome");
        assert_eq!(deferred.durable_body(), &receipt);
        assert_eq!(deferred.missing_merge_sidecar(), Some(&reference));
        assert!(deferred.validated_receipt().is_none());
        assert!(deferred.rejection_reason().is_none());
        assert!(deferred.rejection_identity().is_none());
        assert!(!marker_path.exists());
        assert!(store.validated_recovery_catalog().is_empty());
        assert_eq!(durable_files_snapshot(directory.path()), files_before);

        let rejected = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "candidate is invalid",
                ))
            })
            .expect("return a closed rejection outcome");
        assert_eq!(rejected.durable_body(), &receipt);
        assert_eq!(rejected.rejection_reason(), Some("candidate is invalid"));
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert!(rejected.validated_receipt().is_none());
        assert!(rejected.missing_merge_sidecar().is_none());
        assert!(marker_path.exists());
        let marker = super::read_validation_outcome_marker(&marker_path)
            .expect("decode durable rejection marker");
        assert_eq!(marker.version, VALIDATION_OUTCOME_MARKER_VERSION);
        assert_eq!(
            marker.outcome,
            ValidationOutcomeMarkerKind::Rejected(
                BodyValidationRejectionIdentity::Rejected.canonical_code()
            )
        );
        assert_ne!(durable_files_snapshot(directory.path()), files_before);
        let rejected = rejected
            .into_validated_receipt()
            .expect_err("rejection must remain intact on the success-only path");
        assert_eq!(rejected.durable_body(), &receipt);
        assert_eq!(rejected.rejection_reason(), Some("candidate is invalid"));
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert!(store.validated_recovery_catalog().is_empty());
    }

    #[test]
    fn durable_rejection_reopens_quarantined_and_promotes_only_after_exact_replay() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "first volatile rejection diagnostic",
                ))
            })
            .expect("persist deterministic rejection");
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        let marker_bytes = fs::read(&marker_path).expect("read durable rejection marker");
        assert!(
            !marker_bytes
                .windows(b"first volatile rejection diagnostic".len())
                .any(|window| window == b"first volatile rejection diagnostic"),
            "raw diagnostics must not enter durable authority"
        );
        drop(store);

        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        assert_eq!(reopened.pending_revalidation.len(), 1);
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(reopened.rejected.is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));

        let callback_count = Cell::new(0_usize);
        reopened
            .revalidate_recovered_markers(|_| {
                callback_count.set(callback_count.get().saturating_add(1));
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "reproduced volatile rejection diagnostic",
                ))
            })
            .expect("exact rejection code reproduces the durable outcome");
        assert_eq!(callback_count.get(), 1);
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("rejection marker crossed semantic replay");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(reopened.rejected.len(), 1);

        let repeat_callback_ran = Cell::new(false);
        let repeated = reopened
            .execute_durable_validation(
                receipt.clone(),
                receipt.manifest_hash(),
                |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                    repeat_callback_ran.set(true);
                    unreachable!("an exact durable rejection repeat must not rerun validation")
                },
            )
            .expect("reuse semantically revalidated rejection");
        assert!(!repeat_callback_ran.get());
        assert_eq!(
            repeated.rejection_reason(),
            Some("reproduced volatile rejection diagnostic")
        );
        assert_eq!(
            fs::read(marker_path).expect("read unchanged rejection marker"),
            marker_bytes
        );
    }

    #[test]
    fn recovered_rejection_rejects_outcome_change_and_retires_on_missing_sidecar() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("persist exact candidate body");
        let _rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "durable deterministic rejection",
                ))
            })
            .expect("persist rejection marker");
        drop(store);

        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        let success = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| { Ok::<_, FixtureValidationError>(success) }),
            Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch)
        ));
        assert_eq!(reopened.pending_revalidation.len(), 1);
        assert!(reopened.rejected.is_empty());
        assert!(reopened.validated_recovery_catalog().is_empty());

        let reference = missing_merge_reference(&receipt);
        reopened
            .revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("missing sidecar retires quarantined rejection authority");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("no quarantined marker authority survives deferral");
        assert!(reopened.rejected.is_empty());
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(reopened.retired_revalidation.len(), 1);
        assert!(matches!(
            reopened.execute_durable_validation(
                receipt.clone(),
                receipt.manifest_hash(),
                |_| Ok::<_, FixtureValidationError>(success),
            ),
            Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch)
        ));
        assert_eq!(reopened.retired_revalidation.len(), 1);
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("body remains available after marker retirement"),
            Some((manifest, receipt))
        );
    }

    #[test]
    fn durable_validation_preflight_errors_preserve_store_state_byte_for_byte() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");

        let entries_before = store.entries.clone();
        let manifests_before = store.manifests.clone();
        let pending_before = store.pending_revalidation.clone();
        let retired_before = store.retired_revalidation.clone();
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        let files_before = durable_files_snapshot(directory.path());
        let callback_called = Cell::new(false);
        let wrong_expected = HashOf::<wire::PayloadManifest>::from_untyped_unchecked(Hash::new(
            b"independently wrong expected manifest",
        ));
        let wrong_manifest = store.execute_durable_validation(
            receipt.clone(),
            wrong_expected,
            |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                callback_called.set(true);
                unreachable!("manifest mismatch must precede the validator")
            },
        );
        assert!(matches!(
            wrong_manifest,
            Err(V2BodyStoreError::ReceiptMismatch)
        ));
        assert!(!callback_called.get());
        assert_eq!(store.entries, entries_before);
        assert_eq!(store.manifests, manifests_before);
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.retired_revalidation, retired_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
        assert_eq!(durable_files_snapshot(directory.path()), files_before);

        let foreign_directory = TempDir::new().expect("foreign temporary directory");
        let mut foreign_context = context;
        foreign_context.network_id =
            crate::sumeragi::synthetic_network_id("foreign-sumeragi-v2-body-store");
        let (foreign_body, foreign_manifest) = body_and_manifest(&foreign_context, &keys, None);
        let mut foreign_store = V2BodyStore::open(foreign_directory.path(), foreign_context)
            .expect("open foreign store");
        let foreign_receipt = foreign_store
            .store(foreign_manifest, foreign_body)
            .expect("persist foreign body");
        let foreign_result = store.execute_durable_validation(
            foreign_receipt.clone(),
            foreign_receipt.manifest_hash(),
            |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                callback_called.set(true);
                unreachable!("foreign receipt must precede the validator")
            },
        );
        assert!(matches!(
            foreign_result,
            Err(V2BodyStoreError::ReceiptMismatch)
        ));
        assert!(!callback_called.get());
        assert_eq!(store.entries, entries_before);
        assert_eq!(store.manifests, manifests_before);
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.retired_revalidation, retired_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
    }

    #[test]
    fn terminal_validate_outcome_catalog_drop_restores_both_maps_and_retired_seals() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let (retired_key, retired) = store
            .validated
            .iter()
            .next()
            .map(|(key, validated)| {
                (
                    *key,
                    QuarantinedValidationOutcome {
                        durable: validated.durable().clone(),
                        outcome: ValidationOutcomeMarkerKind::Validated(
                            validated.execution_commitment(),
                        ),
                    },
                )
            })
            .expect("promoted success exists");
        store.retired_revalidation.insert(retired_key, retired);

        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        let retired_before = store.retired_revalidation.clone();
        {
            let _cut = store
                .detach_terminal_validate_outcome_catalog()
                .expect("detach aggregate terminal outcome catalog");
        }

        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
        assert_eq!(store.retired_revalidation, retired_before);
    }

    #[test]
    fn terminal_validate_outcome_catalog_commit_restores_all_unselected_entries() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();

        store
            .detach_terminal_validate_outcome_catalog()
            .expect("detach aggregate terminal outcome catalog")
            .commit_selected();

        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
    }

    #[test]
    fn terminal_validate_outcome_catalog_rejects_pending_markers_without_mutation() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let (pending_key, pending) = store
            .validated
            .iter()
            .next()
            .map(|(key, validated)| {
                (
                    *key,
                    QuarantinedValidationOutcome {
                        durable: validated.durable().clone(),
                        outcome: ValidationOutcomeMarkerKind::Validated(
                            validated.execution_commitment(),
                        ),
                    },
                )
            })
            .expect("promoted success exists");
        store.pending_revalidation.insert(pending_key, pending);
        let pending_before = store.pending_revalidation.clone();
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();

        let error = match store.detach_terminal_validate_outcome_catalog() {
            Ok(cut) => {
                drop(cut);
                panic!("pending semantic replay must prevent catalog detachment");
            }
            Err(error) => error,
        };
        assert_eq!(
            error,
            RecoveredTerminalValidateOutcomeCatalogError::UnrevalidatedMarkers
        );
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
    }

    #[test]
    fn terminal_validate_outcome_catalog_rejects_ambiguous_key_without_mutation() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let (ambiguous_key, ambiguous_rejection) = store
            .validated
            .iter()
            .next()
            .map(|(key, validated)| {
                (
                    *key,
                    RevalidatedRejectedBody {
                        durable: validated.durable().clone(),
                        identity_code: BodyValidationRejectionIdentity::Rejected.canonical_code(),
                        reason: "volatile ambiguous diagnostic".to_owned(),
                    },
                )
            })
            .expect("promoted success exists");
        store.rejected.insert(ambiguous_key, ambiguous_rejection);
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();

        let error = match store.detach_terminal_validate_outcome_catalog() {
            Ok(cut) => {
                drop(cut);
                panic!("ambiguous outcome key must prevent catalog detachment");
            }
            Err(error) => error,
        };
        assert_eq!(
            error,
            RecoveredTerminalValidateOutcomeCatalogError::AmbiguousOutcome
        );
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
    }

    #[test]
    fn terminal_validate_outcome_catalog_cut_is_opaque_and_move_only() {
        let source = include_str!("v2_body_store.rs");
        let implementation = source
            .split_once("impl RecoveredTerminalValidateOutcomeCatalogCut<'_>")
            .expect("terminal outcome catalog implementation exists")
            .1
            .split_once("impl Drop for RecoveredTerminalValidateOutcomeCatalogCut<'_>")
            .expect("drop restoration follows the catalog implementation")
            .0;

        assert_eq!(implementation.matches("pub(super) fn ").count(), 2);
        assert!(implementation.contains("fn select_exact_terminal_validate("));
        assert!(implementation.contains("fn commit_selected(mut self)"));
        for forbidden in [
            "pub(super) fn receipt",
            "pub(super) fn outcome",
            "pub(super) fn validated",
            "pub(super) fn rejected",
            "pub(super) fn into_",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "catalog cut must not expose {forbidden}"
            );
        }
        assert!(!source.contains("impl Clone for RecoveredTerminalValidateOutcomeCatalogCut"));
        assert!(!source.contains(
            "#[derive(Clone)]\npub(super) struct RecoveredTerminalValidateOutcomeCatalogCut"
        ));
    }

    #[test]
    fn durable_validation_surface_has_no_scheduler_identity_or_ordinal() {
        let source = include_str!("v2_body_store.rs");
        let section = |begin: &str, end: &str| {
            source
                .split_once(begin)
                .expect("durable validation section begins")
                .1
                .split_once(end)
                .expect("durable validation section ends")
                .0
        };
        let surface = section(
            "// DURABLE_BODY_VALIDATION_SURFACE_BEGIN",
            "// DURABLE_BODY_VALIDATION_SURFACE_END",
        );
        let api = section(
            "// DURABLE_BODY_VALIDATION_API_BEGIN",
            "// DURABLE_BODY_VALIDATION_API_END",
        );
        let error_classification = source
            .split("pub(crate) trait BodyValidationError")
            .nth(1)
            .expect("body validation error classification exists")
            .split("pub(crate) enum BlockSignaturePolicy")
            .next()
            .expect("signature policy follows validation error classification");

        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "lifecycle_ordinal",
            "work_id",
        ] {
            assert!(
                !surface.contains(forbidden) && !api.contains(forbidden),
                "new durable validation surface must not contain {forbidden}"
            );
        }
        assert!(
            surface
                .contains("struct DurableBodyValidationOutcome(DurableBodyValidationOutcomeBody);")
        );
        assert!(surface.contains(
            "#[derive(Debug, PartialEq, Eq)]\n#[must_use]\npub(crate) struct DurableBodyValidationOutcome"
        ));
        assert!(!surface.contains("impl Clone for DurableBodyValidationOutcome"));
        assert!(!surface.contains("impl Clone for RecoveredValidatedBodyCut"));
        assert!(surface.contains("enum DurableBodyValidationOutcomeBody"));
        assert!(surface.contains("enum BodyValidationRejectionIdentity"));
        assert!(surface.contains("identity: BodyValidationRejectionIdentity"));
        assert!(surface.contains("pub(crate) const fn rejection_identity"));
        assert!(api.contains("error.rejection_identity().canonical_code()"));
        assert!(api.contains("persist_rejected_outcome"));
        assert!(error_classification.contains("fn rejection_identity(&self)"));
        assert!(error_classification.contains("BodyValidationRejectionIdentity::Rejected"));
    }

    #[test]
    fn rejection_marker_version_code_and_frame_binding_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let _rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "durable rejection",
                ))
            })
            .expect("persist rejection marker");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let canonical_marker = super::read_validation_outcome_marker(&marker_path)
            .expect("decode canonical rejection marker");
        assert_eq!(
            canonical_marker.outcome,
            ValidationOutcomeMarkerKind::Rejected(
                BodyValidationRejectionIdentity::Rejected.canonical_code()
            )
        );
        drop(store);

        let mut wrong_frame = canonical_marker.clone();
        wrong_frame.body_frame_hash = Hash::new(b"foreign durable body frame");
        write_validation_outcome_marker(&marker_path, &wrong_frame)
            .expect("write checksum-valid foreign-frame marker");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::ValidationMarkerMismatch)
        ));

        let mut unknown_code = canonical_marker.clone();
        unknown_code.outcome = ValidationOutcomeMarkerKind::Rejected(u8::MAX);
        write_validation_outcome_marker(&marker_path, &unknown_code)
            .expect("write checksum-valid unknown rejection code");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::UnknownValidationRejectionIdentity(
                u8::MAX
            ))
        ));

        let mut unsupported_version = canonical_marker;
        unsupported_version.version = VALIDATION_OUTCOME_MARKER_VERSION.saturating_add(1);
        write_validation_outcome_marker(&marker_path, &unsupported_version)
            .expect("write checksum-valid unsupported marker version");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::UnsupportedValidationOutcomeMarkerVersion(version))
                if version == VALIDATION_OUTCOME_MARKER_VERSION.saturating_add(1)
        ));
    }

    #[test]
    fn corrupted_or_orphaned_validation_marker_fails_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .validate(&receipt, |_| Ok::<_, &'static str>(execution_commitment))
            .expect("persist validation marker");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let mut marker_bytes = fs::read(&marker_path).expect("read marker");
        *marker_bytes.last_mut().expect("nonempty marker") ^= 0x80;
        fs::write(&marker_path, marker_bytes).expect("corrupt marker");
        drop(store);
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::ChecksumMismatch)
        ));

        fs::remove_file(&marker_path).expect("remove corrupt marker");
        let reopened = V2BodyStore::open(directory.path(), context.clone()).expect("reopen body");
        let marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: receipt.context_id(),
            round: wire::ConsensusRound {
                view: receipt.round().view.saturating_add(1),
                ..receipt.round()
            },
            subject: receipt.subject(),
            manifest_hash: receipt.manifest_hash(),
            body_frame_hash: receipt.frame_hash,
            outcome: ValidationOutcomeMarkerKind::Validated(execution_commitment),
        };
        let orphan_path = reopened.validated_path_for(marker.round, marker.subject);
        write_validation_outcome_marker(&orphan_path, &marker).expect("write orphan marker");
        drop(reopened);
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::OrphanedValidationMarker)
        ));
    }

    #[test]
    fn declared_oversized_frame_is_rejected_before_payload_allocation() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("oversized.norito");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(STORE_MAGIC);
        bytes.extend_from_slice(&STORE_VERSION.to_le_bytes());
        bytes.extend_from_slice(&FRAME_PAYLOAD_MAX_BYTES.saturating_add(1).to_le_bytes());
        bytes.extend_from_slice(&[0_u8; CHECKSUM_LEN]);
        fs::write(&path, bytes).expect("write oversized frame header");

        assert!(matches!(
            read_frame_payload_with_hash(&path, STORE_MAGIC),
            Err(V2BodyStoreError::BodyTooLarge)
        ));
    }

    #[test]
    fn final_file_corruption_fails_closed_but_incomplete_temp_is_ignored() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let _receipt = store.store(manifest, body).expect("store body");
        let context_directory = directory.path().join(hex::encode(context.id().0.as_ref()));
        fs::write(context_directory.join("interrupted.norito.tmp"), b"partial")
            .expect("write incomplete temp file");
        V2BodyStore::open(directory.path(), context.clone())
            .expect("incomplete temp is unacknowledged");

        let final_path = fs::read_dir(&context_directory)
            .expect("list context directory")
            .map(|entry| entry.expect("directory entry").path())
            .find(|path| path.extension().and_then(|value| value.to_str()) == Some("norito"))
            .expect("durable final body");
        let mut bytes = fs::read(&final_path).expect("read final body");
        let last = bytes.last_mut().expect("non-empty frame");
        *last ^= 0x80;
        fs::write(&final_path, bytes).expect("corrupt final body");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::ChecksumMismatch)
        ));
    }

    #[test]
    fn wrong_leader_signature_is_rejected_before_durability() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let leader = usize::try_from(context.leader(0)).expect("leader index");
        let wrong = (leader + 1) % keys.len();
        let (body, manifest) = body_and_manifest(&context, &keys, Some(wrong));
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::InvalidExpectedSignature)
        ));
    }

    #[test]
    fn height_one_can_require_the_distinct_genesis_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let impostor = KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .expect("deterministic impostor key");
        let (body, manifest) = body_and_manifest_with_signature(&context, &genesis, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let _receipt = store
            .store(manifest, body)
            .expect("configured genesis signature is accepted");

        let other_directory = TempDir::new().expect("other temporary directory");
        let (body, manifest) = body_and_manifest_with_signature(&context, &impostor, 0);
        let mut store = V2BodyStore::open_with_policy(
            other_directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::InvalidExpectedSignature)
        ));
    }

    #[test]
    fn fixed_genesis_body_can_be_reproposed_after_a_certified_view_change() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xC3; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let (body, manifest) =
            body_and_manifest_with_signature_and_views(&context, &genesis, 0, 3, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");

        let _receipt = store
            .store(manifest, body)
            .expect("fixed signed genesis body is valid in a later proposal view");
    }

    #[test]
    fn rotating_leader_reproposal_authenticates_the_immutable_header_leader() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let origin_view = 1;
        let later_view = 4;
        let origin_leader = usize::try_from(context.leader(origin_view)).expect("leader index");
        let (body, manifest) = body_and_manifest_with_signature_and_views(
            &context,
            &keys[origin_leader],
            u64::try_from(origin_leader).expect("leader index fits u64"),
            later_view,
            origin_view,
        );
        let mut store = V2BodyStore::open(directory.path(), context).expect("open body store");

        let _ = store
            .store(manifest, body)
            .expect("a later reproposal retains the original header leader signature");
    }

    #[test]
    fn body_from_a_future_view_is_rejected() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let proposal_view = 2;
        let future_origin_view = 3;
        let future_leader =
            usize::try_from(context.leader(future_origin_view)).expect("leader index");
        let (body, manifest) = body_and_manifest_with_signature_and_views(
            &context,
            &keys[future_leader],
            u64::try_from(future_leader).expect("leader index fits u64"),
            proposal_view,
            future_origin_view,
        );
        let mut store = V2BodyStore::open(directory.path(), context).expect("open body store");

        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::BlockSubjectMismatch)
        ));
    }
}
