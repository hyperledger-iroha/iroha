//! Versioned durable ledger for Sumeragi v2 lifecycle ownership.
//!
//! The ledger persists only restart-stable logical state. Readiness, leases,
//! wait generations, physical carriers, and scheduler episodes are rebuilt
//! from authenticated storage after restart and never appear in this format.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll, Encode};
use thiserror::Error;

use super::replay_authority::{
    CertifiedServeTerminalReplayAuthorityPairV1, LifecycleReplayAuthorityV1,
};
use super::schema::{
    DurableBodyFrameReference, DurableContinuation, DurableContinuationEdge,
    MAX_LIFECYCLE_RECORDS_PER_HEIGHT, serve_and_producer_keys_match,
};
use super::wal_recovery::{
    AuthenticatedWalVoteLifecycleRepair, DurableAuthenticatedWalVoteLifecycleRepair,
};
use super::{
    CandidateAdmission, CausalRoot, DurablePayloadReference, DurableServeNegativeOutcome,
    InitialLifecycleState, LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey,
    LifecyclePhase, LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleState,
    LifecycleWorkClass, OwnerId, PhysicalSlotId, PredecessorScope, RecoveredLifecycleRecord,
    RecoverySnapshot, TerminalOutcome,
};
use super::{
    body_pipeline_transition::{
        durable_continuation_payload_is_exact, durable_continuation_successor_is_exact,
        durable_validate_payload_is_exact,
    },
    projection,
};
use crate::sumeragi::{
    v2::{RecoveredWalFrameIdentity, RecoveredWalVoteSign, VerifiedHeightContext},
    v2_body_store::DurableBodyReceipt,
    v2_core::EventTag,
    v2_runtime::{PendingRuntimeEffectBinding, RecoveredWalCandidateProjectionPermit},
};

const LEDGER_FILE: &str = "lifecycle-ledger-v1.norito";
const LEDGER_MAGIC: &[u8; 8] = b"SUMV2LC1";
const LEDGER_VERSION: u16 = 1;
const HASH_BYTES: usize = 32;
const HEADER_BYTES: usize = LEDGER_MAGIC.len() + 2 + 8 + HASH_BYTES;
const MAX_LEDGER_FRAME_BYTES: u64 = 64 * 1024 * 1024;

const PAYLOAD_NONE: u16 = 0;
const PAYLOAD_CERTIFIED_SERVE_PENDING: u16 = 1;
const PAYLOAD_CERTIFIED_SERVE_COMPLETED: u16 = 2;
const PAYLOAD_CERTIFIED_SERVE_NEGATIVE: u16 = 3;
const PAYLOAD_BODY_FRAME: u16 = 4;
const NEGATIVE_CANCELLED: u8 = 0;
const NEGATIVE_REJECTED: u8 = 1;
const NEGATIVE_FAILED: u8 = 2;

/// Canonical small reference envelope for Certified-Serve state.
///
/// The body itself remains in the body store. `lifecycle_subject` mirrors the
/// key's domain-separated `(block subject, exact signed request hash)` Serve
/// subject; the remaining fields authenticate the request, certificate
/// authorization, and optional terminal receipt without opaque bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct CertifiedServePayloadReferenceV1 {
    lifecycle_subject: [u8; 32],
    request_hash: [u8; 32],
    certificate_hash: [u8; 32],
    completed_response: Option<[u8; 32]>,
    negative_kind: Option<u8>,
    negative_code: Option<u16>,
}

impl CertifiedServePayloadReferenceV1 {
    /// Construct a reference for an admitted response that is not terminal.
    const fn pending(
        lifecycle_subject: LifecycleDigest,
        request_hash: LifecycleDigest,
        certificate_hash: LifecycleDigest,
    ) -> Self {
        Self {
            lifecycle_subject: *lifecycle_subject.as_bytes(),
            request_hash: *request_hash.as_bytes(),
            certificate_hash: *certificate_hash.as_bytes(),
            completed_response: None,
            negative_kind: None,
            negative_code: None,
        }
    }

    /// Construct a reference for a completed certified response.
    const fn completed(
        lifecycle_subject: LifecycleDigest,
        request_hash: LifecycleDigest,
        certificate_hash: LifecycleDigest,
        completed_response: LifecycleDigest,
    ) -> Self {
        Self {
            lifecycle_subject: *lifecycle_subject.as_bytes(),
            request_hash: *request_hash.as_bytes(),
            certificate_hash: *certificate_hash.as_bytes(),
            completed_response: Some(*completed_response.as_bytes()),
            negative_kind: None,
            negative_code: None,
        }
    }

    /// Construct a reference for a terminal negative certified response.
    const fn negative(
        lifecycle_subject: LifecycleDigest,
        request_hash: LifecycleDigest,
        certificate_hash: LifecycleDigest,
        outcome: DurableServeNegativeOutcome,
    ) -> Self {
        let (negative_kind, negative_code) = match outcome {
            DurableServeNegativeOutcome::Cancelled => (NEGATIVE_CANCELLED, None),
            DurableServeNegativeOutcome::Rejected(code) => (NEGATIVE_REJECTED, Some(code)),
            DurableServeNegativeOutcome::Failed(code) => (NEGATIVE_FAILED, Some(code)),
        };
        Self {
            lifecycle_subject: *lifecycle_subject.as_bytes(),
            request_hash: *request_hash.as_bytes(),
            certificate_hash: *certificate_hash.as_bytes(),
            completed_response: None,
            negative_kind: Some(negative_kind),
            negative_code,
        }
    }

    const fn matches_kind(self, kind: u16) -> bool {
        matches!(
            (
                kind,
                self.completed_response,
                self.negative_kind,
                self.negative_code,
            ),
            (PAYLOAD_CERTIFIED_SERVE_PENDING, None, None, None)
                | (PAYLOAD_CERTIFIED_SERVE_COMPLETED, Some(_), None, None)
                | (
                    PAYLOAD_CERTIFIED_SERVE_NEGATIVE,
                    None,
                    Some(NEGATIVE_CANCELLED),
                    None,
                )
                | (
                    PAYLOAD_CERTIFIED_SERVE_NEGATIVE,
                    None,
                    Some(NEGATIVE_REJECTED | NEGATIVE_FAILED),
                    Some(_),
                )
        )
    }

    const fn lifecycle_subject(self) -> LifecycleDigest {
        LifecycleDigest::new(self.lifecycle_subject)
    }

    const fn negative_outcome(self) -> Option<DurableServeNegativeOutcome> {
        match (self.negative_kind, self.negative_code) {
            (Some(NEGATIVE_CANCELLED), None) => Some(DurableServeNegativeOutcome::Cancelled),
            (Some(NEGATIVE_REJECTED), Some(code)) => {
                Some(DurableServeNegativeOutcome::Rejected(code))
            }
            (Some(NEGATIVE_FAILED), Some(code)) => Some(DurableServeNegativeOutcome::Failed(code)),
            _ => None,
        }
    }
}

/// Canonical reference to one fsynced body-store frame.
///
/// This is deliberately not the complete replay authority. An ordinary body
/// still needs its authenticated proposal provenance and a certified body
/// still needs its exact QC. Keeping the byte identity in LedgerV1 ensures a
/// later replay-source join cannot silently substitute another local frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct BodyFramePayloadReferenceV1 {
    context: [u8; 32],
    round_height: u64,
    round_view: u64,
    subject: [u8; 32],
    manifest_hash: [u8; 32],
    frame_hash: [u8; 32],
}

impl BodyFramePayloadReferenceV1 {
    const fn from_schema(reference: DurableBodyFrameReference) -> Self {
        Self {
            context: *reference.context.as_bytes(),
            round_height: reference.round.height(),
            round_view: reference.round.view(),
            subject: *reference.subject.as_bytes(),
            manifest_hash: *reference.manifest.as_bytes(),
            frame_hash: *reference.frame.as_bytes(),
        }
    }

    const fn to_schema(self) -> DurableBodyFrameReference {
        DurableBodyFrameReference::new(
            LifecycleDigest::new(self.context),
            LifecycleRound::new(self.round_height, self.round_view),
            LifecycleDigest::new(self.subject),
            LifecycleDigest::new(self.manifest_hash),
            LifecycleDigest::new(self.frame_hash),
        )
    }
}

/// Durable payload reference associated with a lifecycle record.
///
/// Certified-Serve references contain canonical Norito bytes for a small
/// typed reference envelope. Canonical block bodies remain in the body store.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(super) struct LifecyclePayloadReferenceV1 {
    kind: u16,
    digest: [u8; 32],
    canonical_reference: Vec<u8>,
}

impl LifecyclePayloadReferenceV1 {
    /// Construct the empty reference used by non-Serve records.
    pub(super) const fn none() -> Self {
        Self {
            kind: PAYLOAD_NONE,
            digest: [0; 32],
            canonical_reference: Vec::new(),
        }
    }

    /// Construct a reference to one exact fsynced body-store frame.
    pub(super) fn body_frame(reference: DurableBodyFrameReference) -> Self {
        let canonical_reference = BodyFramePayloadReferenceV1::from_schema(reference).encode();
        let digest = Hash::new(&canonical_reference);
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(digest.as_ref());
        Self {
            kind: PAYLOAD_BODY_FRAME,
            digest: bytes,
            canonical_reference,
        }
    }

    /// Construct a pending Certified-Serve payload reference.
    pub(super) fn certified_serve_pending(
        lifecycle_subject: LifecycleDigest,
        request_hash: LifecycleDigest,
        certificate_hash: LifecycleDigest,
    ) -> Self {
        Self::certified_serve(
            PAYLOAD_CERTIFIED_SERVE_PENDING,
            CertifiedServePayloadReferenceV1::pending(
                lifecycle_subject,
                request_hash,
                certificate_hash,
            ),
        )
    }

    /// Construct a completed Certified-Serve response reference.
    pub(super) fn certified_serve_completed(
        lifecycle_subject: LifecycleDigest,
        request_hash: LifecycleDigest,
        certificate_hash: LifecycleDigest,
        completed_response: LifecycleDigest,
    ) -> Self {
        Self::certified_serve(
            PAYLOAD_CERTIFIED_SERVE_COMPLETED,
            CertifiedServePayloadReferenceV1::completed(
                lifecycle_subject,
                request_hash,
                certificate_hash,
                completed_response,
            ),
        )
    }

    /// Construct a terminal negative Certified-Serve reference.
    pub(super) fn certified_serve_negative(
        lifecycle_subject: LifecycleDigest,
        request_hash: LifecycleDigest,
        certificate_hash: LifecycleDigest,
        outcome: DurableServeNegativeOutcome,
    ) -> Self {
        Self::certified_serve(
            PAYLOAD_CERTIFIED_SERVE_NEGATIVE,
            CertifiedServePayloadReferenceV1::negative(
                lifecycle_subject,
                request_hash,
                certificate_hash,
                outcome,
            ),
        )
    }

    fn certified_serve(kind: u16, reference: CertifiedServePayloadReferenceV1) -> Self {
        debug_assert!(reference.matches_kind(kind));
        let canonical_reference = reference.encode();
        let digest = Hash::new(&canonical_reference);
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(digest.as_ref());
        Self {
            kind,
            digest: bytes,
            canonical_reference,
        }
    }

    fn from_schema(key: LifecycleKey, payload: DurablePayloadReference) -> Option<Self> {
        match payload {
            DurablePayloadReference::None => Some(Self::none()),
            DurablePayloadReference::BodyFrame(reference) => reference
                .matches_key(key)
                .then(|| Self::body_frame(reference)),
            DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            } => Some(Self::certified_serve_pending(
                key.subject()?,
                request,
                certificate,
            )),
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response,
            } => Some(Self::certified_serve_completed(
                key.subject()?,
                request,
                certificate,
                response,
            )),
            DurablePayloadReference::CertifiedServeNegative {
                request,
                certificate,
                outcome,
            } => Some(Self::certified_serve_negative(
                key.subject()?,
                request,
                certificate,
                outcome,
            )),
        }
    }

    fn validate(&self) -> bool {
        match self.kind {
            PAYLOAD_NONE => self.digest == [0; 32] && self.canonical_reference.is_empty(),
            PAYLOAD_BODY_FRAME => {
                let mut bytes = self.canonical_reference.as_slice();
                let Ok(reference) = BodyFramePayloadReferenceV1::decode_all(&mut bytes) else {
                    return false;
                };
                reference.encode() == self.canonical_reference
                    && Hash::new(&self.canonical_reference).as_ref() == self.digest.as_slice()
            }
            PAYLOAD_CERTIFIED_SERVE_PENDING
            | PAYLOAD_CERTIFIED_SERVE_COMPLETED
            | PAYLOAD_CERTIFIED_SERVE_NEGATIVE => {
                let mut bytes = self.canonical_reference.as_slice();
                let Ok(reference) = CertifiedServePayloadReferenceV1::decode_all(&mut bytes) else {
                    return false;
                };
                reference.matches_kind(self.kind)
                    && reference.encode() == self.canonical_reference
                    && Hash::new(&self.canonical_reference).as_ref() == self.digest.as_slice()
            }
            _ => false,
        }
    }

    fn to_schema(&self, key: LifecycleKey) -> Option<DurablePayloadReference> {
        if self.kind == PAYLOAD_NONE {
            return self.validate().then_some(DurablePayloadReference::None);
        }
        if self.kind == PAYLOAD_BODY_FRAME {
            let mut bytes = self.canonical_reference.as_slice();
            let reference = BodyFramePayloadReferenceV1::decode_all(&mut bytes)
                .ok()?
                .to_schema();
            return (self.validate() && reference.matches_key(key))
                .then_some(DurablePayloadReference::BodyFrame(reference));
        }
        let mut bytes = self.canonical_reference.as_slice();
        let reference = CertifiedServePayloadReferenceV1::decode_all(&mut bytes).ok()?;
        if !self.validate() || key.subject() != Some(reference.lifecycle_subject()) {
            return None;
        }
        let request = LifecycleDigest::new(reference.request_hash);
        let certificate = LifecycleDigest::new(reference.certificate_hash);
        match (
            self.kind,
            reference.completed_response,
            reference.negative_outcome(),
        ) {
            (PAYLOAD_CERTIFIED_SERVE_PENDING, None, None) => {
                Some(DurablePayloadReference::CertifiedServePending {
                    request,
                    certificate,
                })
            }
            (PAYLOAD_CERTIFIED_SERVE_COMPLETED, Some(response), None) => {
                Some(DurablePayloadReference::CertifiedServeCompleted {
                    request,
                    certificate,
                    response: LifecycleDigest::new(response),
                })
            }
            (PAYLOAD_CERTIFIED_SERVE_NEGATIVE, None, Some(outcome)) => {
                Some(DurablePayloadReference::CertifiedServeNegative {
                    request,
                    certificate,
                    outcome,
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedLifecycleKeyV1 {
    context: [u8; 32],
    round_height: u64,
    round_view: u64,
    proposal_height: Option<u64>,
    proposal_view: Option<u64>,
    subject: Option<[u8; 32]>,
    phase_code: u16,
    execution_commitment: Option<[u8; 32]>,
}

impl PersistedLifecycleKeyV1 {
    fn from_schema(key: LifecycleKey) -> Self {
        let proposal_round = key.proposal_round();
        Self {
            context: *key.context().as_bytes(),
            round_height: key.round().height(),
            round_view: key.round().view(),
            proposal_height: proposal_round.map(LifecycleRound::height),
            proposal_view: proposal_round.map(LifecycleRound::view),
            subject: key.subject().map(|digest| *digest.as_bytes()),
            phase_code: phase_code(key.phase()),
            execution_commitment: key.execution_commitment().map(|digest| *digest.as_bytes()),
        }
    }

    fn to_schema(self) -> Option<LifecycleKey> {
        let proposal_round = match (self.proposal_height, self.proposal_view) {
            (Some(height), Some(view)) => Some(LifecycleRound::new(height, view)),
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => return None,
        };
        Some(LifecycleKey::new(
            LifecycleDigest::new(self.context),
            LifecycleRound::new(self.round_height, self.round_view),
            proposal_round,
            self.subject.map(LifecycleDigest::new),
            decode_phase(self.phase_code)?,
            self.execution_commitment.map(LifecycleDigest::new),
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedTerminalV1 {
    kind: u16,
    completed_digest: Option<[u8; 32]>,
    detail_code: Option<u16>,
}

impl PersistedTerminalV1 {
    fn from_schema(outcome: TerminalOutcome) -> Self {
        match outcome {
            TerminalOutcome::Advanced => Self {
                kind: 1,
                completed_digest: None,
                detail_code: None,
            },
            TerminalOutcome::Completed(digest) => Self {
                kind: 2,
                completed_digest: digest.map(|digest| *digest.as_bytes()),
                detail_code: None,
            },
            TerminalOutcome::Cancelled => Self {
                kind: 3,
                completed_digest: None,
                detail_code: None,
            },
            TerminalOutcome::Rejected(code) => Self {
                kind: 4,
                completed_digest: None,
                detail_code: Some(code),
            },
            TerminalOutcome::Failed(code) => Self {
                kind: 5,
                completed_digest: None,
                detail_code: Some(code),
            },
        }
    }

    fn to_schema(self) -> Option<TerminalOutcome> {
        match (self.kind, self.completed_digest, self.detail_code) {
            (1, None, None) => Some(TerminalOutcome::Advanced),
            (2, digest, None) => Some(TerminalOutcome::Completed(digest.map(LifecycleDigest::new))),
            (3, None, None) => Some(TerminalOutcome::Cancelled),
            (4, None, Some(code)) => Some(TerminalOutcome::Rejected(code)),
            (5, None, Some(code)) => Some(TerminalOutcome::Failed(code)),
            _ => None,
        }
    }
}

/// Canonical wire representation of one typed durable continuation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedDurableContinuationV1 {
    code: u8,
    successor_ordinal: Option<u128>,
}

impl PersistedDurableContinuationV1 {
    const NONE: u8 = 0;
    const ADVANCED_NO_SUCCESSOR: u8 = 1;
    const FETCH_TO_STORE: u8 = 2;
    const STORE_TO_VALIDATE: u8 = 3;
    const VALIDATE_TO_APPLY: u8 = 4;
    const VALIDATE_TO_INVALID_BODY_REPORT: u8 = 5;
    const VALIDATE_TO_SIGN_PREPARE: u8 = 6;
    const VALIDATE_TO_SIGN_COMMIT: u8 = 7;

    const fn from_schema(continuation: DurableContinuation) -> Self {
        match continuation {
            DurableContinuation::None => Self {
                code: Self::NONE,
                successor_ordinal: None,
            },
            DurableContinuation::AdvancedNoSuccessor => Self {
                code: Self::ADVANCED_NO_SUCCESSOR,
                successor_ordinal: None,
            },
            DurableContinuation::AdvancedSuccessor { edge, ordinal } => Self {
                code: match edge {
                    DurableContinuationEdge::FetchToStore => Self::FETCH_TO_STORE,
                    DurableContinuationEdge::StoreToValidate => Self::STORE_TO_VALIDATE,
                    DurableContinuationEdge::ValidateToApply => Self::VALIDATE_TO_APPLY,
                    DurableContinuationEdge::ValidateToInvalidBodyReport => {
                        Self::VALIDATE_TO_INVALID_BODY_REPORT
                    }
                    DurableContinuationEdge::ValidateToSignPrepare => {
                        Self::VALIDATE_TO_SIGN_PREPARE
                    }
                    DurableContinuationEdge::ValidateToSignCommit => Self::VALIDATE_TO_SIGN_COMMIT,
                },
                successor_ordinal: Some(ordinal),
            },
        }
    }

    const fn to_schema(self) -> Option<DurableContinuation> {
        match (self.code, self.successor_ordinal) {
            (Self::NONE, None) => Some(DurableContinuation::None),
            (Self::ADVANCED_NO_SUCCESSOR, None) => Some(DurableContinuation::AdvancedNoSuccessor),
            (Self::FETCH_TO_STORE, Some(ordinal)) => Some(DurableContinuation::successor(
                DurableContinuationEdge::FetchToStore,
                ordinal,
            )),
            (Self::STORE_TO_VALIDATE, Some(ordinal)) => Some(DurableContinuation::successor(
                DurableContinuationEdge::StoreToValidate,
                ordinal,
            )),
            (Self::VALIDATE_TO_APPLY, Some(ordinal)) => Some(DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                ordinal,
            )),
            (Self::VALIDATE_TO_INVALID_BODY_REPORT, Some(ordinal)) => {
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::ValidateToInvalidBodyReport,
                    ordinal,
                ))
            }
            (Self::VALIDATE_TO_SIGN_PREPARE, Some(ordinal)) => {
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::ValidateToSignPrepare,
                    ordinal,
                ))
            }
            (Self::VALIDATE_TO_SIGN_COMMIT, Some(ordinal)) => Some(DurableContinuation::successor(
                DurableContinuationEdge::ValidateToSignCommit,
                ordinal,
            )),
            _ => None,
        }
    }
}

/// One restart-stable lifecycle record in `LifecycleLedgerV1`.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(super) struct LifecycleLedgerRecordV1 {
    key: PersistedLifecycleKeyV1,
    causal_root: [u8; 32],
    owner_first_ordinal: u128,
    ordinal: u128,
    work_class_code: u16,
    stage_kind_code: u16,
    predecessor_code: u8,
    terminal: Option<PersistedTerminalV1>,
    reconstruction_source: [u8; 32],
    payload_reference: LifecyclePayloadReferenceV1,
    replay_authority: LifecycleReplayAuthorityV1,
    continuation: PersistedDurableContinuationV1,
}

/// Opaque LedgerV1 proof of the exact Validate parent named by a recovered WAL vote.
///
/// The proof retains the complete WAL identity and the durable owner/address
/// projection. It has no constructor or parts API; the runtime may use it only
/// to reconstruct the ordinal-free predecessor binding, and the registry may
/// use it only to exact-check its direct sealed reconstruction.
#[must_use = "a recovered WAL ledger parent must be consumed by sealed startup reconstruction"]
pub(crate) struct AuthenticatedRecoveredWalValidateLedgerParent {
    key: LifecycleKey,
    owner: OwnerId,
    ordinal: u128,
    payload: DurablePayloadReference,
    replay_authority: LifecycleReplayAuthorityV1,
    inherited_prepare_authority: bool,
    wal_identity: RecoveredWalFrameIdentity,
    tag: EventTag,
    vote: wire::Vote,
}

impl AuthenticatedRecoveredWalValidateLedgerParent {
    /// Revalidate the exact adapter-authenticated WAL identity retained here.
    pub(crate) fn exactly_matches_recovered_vote(&self, recovered: &RecoveredWalVoteSign) -> bool {
        self.wal_identity.exactly_matches(recovered.wal_identity())
            && recovered.replay_evidence_is_exact()
            && self.tag == recovered.tag()
            && self.vote == *recovered.vote()
    }

    /// Return the restart-stable runtime causal key preserved by LedgerV1.
    pub(crate) fn runtime_causal_lifecycle_key(&self) -> Hash {
        Hash::prehashed(*self.owner.causal_root().digest().as_bytes())
    }

    /// Return whether the durable Validate inherited the exact Prepare authority.
    pub(crate) const fn inherited_prepare_authority(&self) -> bool {
        self.inherited_prepare_authority
    }

    /// Return the durable owner without exposing its runtime binding.
    pub(super) const fn owner(&self) -> OwnerId {
        self.owner
    }

    /// Return the durable parent ordinal for internal address reconstruction.
    pub(super) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    /// Match one semantically revalidated body receipt to the exact frame
    /// reference retained by this ledger parent.
    ///
    /// This equality oracle deliberately exposes neither the persisted frame
    /// reference nor receipt parts. Recovered-WAL reconstruction uses it before
    /// consuming the one-shot body marker, so a vote-compatible marker from a
    /// substituted durable frame cannot cross the ledger/body authority join.
    pub(crate) fn matches_durable_receipt(
        &self,
        active_context: LifecycleContext,
        durable: &DurableBodyReceipt,
    ) -> bool {
        projection::durable_body_frame_reference(active_context, durable)
            .map(DurablePayloadReference::BodyFrame)
            == Some(self.payload)
    }

    /// Match the reconstructed parent candidate against the complete ledger seal.
    pub(super) fn matches_candidate(&self, candidate: &CandidateAdmission) -> bool {
        candidate.initial_state == InitialLifecycleState::Ready
            && candidate.key == self.key
            && candidate.causal_root == self.owner.causal_root()
            && candidate.work_class == LifecycleWorkClass::Validate
            && candidate.stage
                == LifecycleStage::new(
                    LifecycleStageKind::ValidateBody,
                    PredecessorScope::Independent,
                )
            && candidate.reconstruction_source == self.owner.causal_root().digest()
            && candidate.payload == self.payload
            && candidate.replay_authority == self.replay_authority
            && candidate.producer_turn.is_none()
    }

    /// Construct the exact recovered Validate candidate from this persisted seal.
    ///
    /// The persisted replay authority never leaves the ledger-parent wrapper;
    /// it is attached only after the runtime effect and pending binding project
    /// to the same owner and logical coordinates.
    pub(in crate::sumeragi) fn project_recovered_candidate(
        &self,
        _permit: RecoveredWalCandidateProjectionPermit,
        verified: &VerifiedHeightContext,
        effect: &crate::sumeragi::v2::AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        let active_context = projection::lifecycle_context(verified.context());
        let projected = projection::authority_free_admission_projection(
            active_context,
            verified,
            effect,
            pending,
        )
        .ok()?;
        self.replay_authority
            .structurally_matches_record(
                active_context,
                projected.key,
                projected.work_class,
                projected.stage,
                self.payload,
            )
            .then_some(())?;
        let candidate = CandidateAdmission::new(
            projected.key,
            projected.causal_root,
            projected.work_class,
            projected.stage,
            projected.initial_state,
            projected.reconstruction_source,
            self.payload,
            self.replay_authority.clone(),
            projected.physical_geometry,
            None,
        );
        self.matches_candidate(&candidate).then_some(candidate)
    }
}

impl LifecycleLedgerRecordV1 {
    /// Construct one durable logical lifecycle record.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        key: LifecycleKey,
        owner: OwnerId,
        ordinal: u128,
        work_class: LifecycleWorkClass,
        stage: LifecycleStage,
        terminal: Option<TerminalOutcome>,
        reconstruction_source: LifecycleDigest,
        payload: DurablePayloadReference,
        replay_authority: LifecycleReplayAuthorityV1,
        continuation: DurableContinuation,
    ) -> Result<Self, LifecycleLedgerError> {
        let payload_reference =
            LifecyclePayloadReferenceV1::from_schema(key, payload).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "durable payload cannot be projected from its lifecycle key".to_owned(),
                )
            })?;
        Ok(Self {
            key: PersistedLifecycleKeyV1::from_schema(key),
            causal_root: *owner.causal_root().digest().as_bytes(),
            owner_first_ordinal: owner.first_admission_ordinal(),
            ordinal,
            work_class_code: work_class_code(work_class),
            stage_kind_code: stage_kind_code(stage.kind()),
            predecessor_code: predecessor_code(stage.predecessor_scope()),
            terminal: terminal.map(PersistedTerminalV1::from_schema),
            reconstruction_source: *reconstruction_source.as_bytes(),
            payload_reference,
            replay_authority,
            continuation: PersistedDurableContinuationV1::from_schema(continuation),
        })
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_exact_replay_fixture(
        key: LifecycleKey,
        owner: OwnerId,
        ordinal: u128,
        work_class: LifecycleWorkClass,
        stage: LifecycleStage,
        terminal: Option<TerminalOutcome>,
        reconstruction_source: LifecycleDigest,
        payload: DurablePayloadReference,
        continuation: DurableContinuation,
    ) -> Result<Self, LifecycleLedgerError> {
        Self::new(
            key,
            owner,
            ordinal,
            work_class,
            stage,
            terminal,
            reconstruction_source,
            payload,
            tests::replay_authority_for(key, stage, payload),
            continuation,
        )
    }

    /// Decode the stable semantic key.
    pub(super) fn key(&self) -> Option<LifecycleKey> {
        self.key.to_schema()
    }

    /// Decode the stable owner.
    pub(super) fn owner(&self) -> OwnerId {
        OwnerId::new(
            CausalRoot::new(LifecycleDigest::new(self.causal_root)),
            self.owner_first_ordinal,
        )
    }

    /// Return the immutable admission ordinal.
    pub(super) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    /// Decode the exhaustive work class.
    pub(super) fn work_class(&self) -> Option<LifecycleWorkClass> {
        decode_work_class(self.work_class_code)
    }

    /// Decode the exact immutable execution stage.
    pub(super) fn stage(&self) -> Option<LifecycleStage> {
        Some(LifecycleStage::new(
            decode_stage_kind(self.stage_kind_code)?,
            decode_predecessor(self.predecessor_code)?,
        ))
    }

    /// Decode the optional terminal tombstone.
    pub(super) fn terminal(&self) -> Option<Option<TerminalOutcome>> {
        match self.terminal {
            None => Some(None),
            Some(terminal) => Some(Some(terminal.to_schema()?)),
        }
    }

    /// Return the authenticated reconstruction source.
    pub(super) const fn reconstruction_source(&self) -> LifecycleDigest {
        LifecycleDigest::new(self.reconstruction_source)
    }

    pub(super) fn durable_payload(&self) -> Option<DurablePayloadReference> {
        self.key()
            .and_then(|key| self.payload_reference.to_schema(key))
    }

    /// Decode the exact typed durable continuation.
    pub(super) const fn continuation(&self) -> Option<DurableContinuation> {
        self.continuation.to_schema()
    }

    /// Compare a reconstructed admission with this row's inert persisted
    /// authority without exposing the decoded envelope.
    pub(super) fn replay_matches_candidate(&self, candidate: &CandidateAdmission) -> bool {
        if self.work_class() == Some(LifecycleWorkClass::CertifiedServe) {
            self.replay_authority
                .same_persisted_family(&candidate.replay_authority)
        } else {
            self.replay_authority == candidate.replay_authority
        }
    }

    /// Compare the adjacent reconstructed ProducerTurn with this row's exact
    /// separately encoded replay authority.
    pub(super) fn replay_matches_producer(&self, producer: &super::ProducerTurnAdmission) -> bool {
        self.replay_authority == producer.replay_authority
    }

    /// Prove that this Pending Serve row and its adjacent ProducerTurn are the
    /// exact predecessor of one authenticated terminal payload-store frame.
    pub(super) fn replay_is_exact_pending_predecessor(
        &self,
        context: LifecycleContext,
        producer: &Self,
        terminal: &CertifiedServeTerminalReplayAuthorityPairV1,
    ) -> bool {
        let (
            Some(serve_key),
            Some(serve_stage),
            Some(serve_payload),
            Some(producer_key),
            Some(producer_stage),
            Some(producer_payload),
        ) = (
            self.key(),
            self.stage(),
            self.durable_payload(),
            producer.key(),
            producer.stage(),
            producer.durable_payload(),
        )
        else {
            return false;
        };
        self.work_class() == Some(LifecycleWorkClass::CertifiedServe)
            && self.terminal() == Some(None)
            && producer.work_class() == Some(LifecycleWorkClass::ProducerTurn)
            && producer.terminal() == Some(None)
            && terminal.exactly_advances_pending_coordinates(
                context,
                serve_key,
                self.owner(),
                self.ordinal,
                serve_stage,
                self.reconstruction_source(),
                serve_payload,
                &self.replay_authority,
                producer_key,
                producer.owner(),
                producer.ordinal,
                producer_stage,
                producer.reconstruction_source(),
                producer_payload,
                &producer.replay_authority,
            )
    }

    fn validate(&self, context: LifecycleContext, high_water: u128) -> bool {
        let Some(key) = self.key() else {
            return false;
        };
        let Some(work_class) = self.work_class() else {
            return false;
        };
        let terminal = self.terminal().flatten();
        let Some(continuation) = self.continuation() else {
            return false;
        };
        let successor_shape_is_valid =
            continuation.matches_record(work_class, terminal, self.ordinal, high_water);
        self.ordinal > 0
            && self.ordinal <= high_water
            && self.owner_first_ordinal > 0
            && self.owner_first_ordinal <= self.ordinal
            && key.context() == context.id()
            && key.round().height() == context.height()
            && key
                .proposal_round()
                .is_none_or(|round| round.height() == context.height())
            && self.stage().is_some()
            && self.terminal().is_some()
            && work_shape_is_valid(work_class, key, self.stage().expect("stage checked above"))
            && self.durable_payload().is_some_and(|payload| {
                payload
                    .matches_terminal(work_class, self.terminal().expect("terminal checked above"))
                    && self.replay_authority.structurally_matches_record(
                        context,
                        key,
                        work_class,
                        self.stage().expect("stage checked above"),
                        payload,
                    )
            })
            && successor_shape_is_valid
    }
}

/// Durable adjacent Serve-to-producer obligation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(super) struct LifecycleProducerDebtV1 {
    serve_ordinal: u128,
    producer_ordinal: u128,
}

impl LifecycleProducerDebtV1 {
    /// Construct an adjacent producer-turn debt.
    pub(super) const fn new(serve_ordinal: u128, producer_ordinal: u128) -> Self {
        Self {
            serve_ordinal,
            producer_ordinal,
        }
    }

    /// Return the Serve ordinal.
    pub(super) const fn serve_ordinal(self) -> u128 {
        self.serve_ordinal
    }

    /// Return the producer-turn ordinal.
    pub(super) const fn producer_ordinal(self) -> u128 {
        self.producer_ordinal
    }
}

/// Complete version-one durable lifecycle ledger.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(super) struct LifecycleLedgerV1 {
    format_version: u16,
    context: [u8; 32],
    height: u64,
    high_water: u128,
    records: Vec<LifecycleLedgerRecordV1>,
    producer_debts: Vec<LifecycleProducerDebtV1>,
}

impl LifecycleLedgerV1 {
    pub(super) fn from_coordinator(
        coordinator: &LifecycleCoordinator,
    ) -> Result<Self, LifecycleLedgerError> {
        let records = coordinator
            .records
            .values()
            .map(|record| {
                let metadata = coordinator
                    .durable_records
                    .get(&record.ordinal)
                    .ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "logical record has no durable reconstruction metadata".to_owned(),
                        )
                    })?;
                let terminal = match record.state {
                    LifecycleState::Terminal(outcome) => Some(outcome),
                    LifecycleState::Waiting(_)
                    | LifecycleState::Ready
                    | LifecycleState::Claimed(_) => None,
                };
                LifecycleLedgerRecordV1::new(
                    record.key,
                    record.owner,
                    record.ordinal,
                    record.work_class,
                    record.stage,
                    terminal,
                    metadata.reconstruction_source,
                    metadata.payload,
                    metadata.replay_authority.clone(),
                    metadata.continuation,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            coordinator.active_context,
            coordinator.high_water,
            records,
            coordinator.producer_debts.clone(),
        )
    }

    pub(super) fn recovery_snapshot(
        &self,
        mut physical_slot_universes: BTreeMap<u128, BTreeSet<PhysicalSlotId>>,
    ) -> Result<RecoverySnapshot, LifecycleLedgerError> {
        if physical_slot_universes.len() != self.records.len()
            || self
                .records
                .iter()
                .any(|record| !physical_slot_universes.contains_key(&record.ordinal))
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "storage reconciliation does not cover every durable record exactly once"
                    .to_owned(),
            ));
        }
        let records = self
            .records
            .iter()
            .map(|record| {
                let key = record.key().ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "durable record key cannot be decoded".to_owned(),
                    )
                })?;
                Ok(RecoveredLifecycleRecord::new(
                    key,
                    record.owner(),
                    record.ordinal(),
                    record.work_class().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable work class cannot be decoded".to_owned(),
                        )
                    })?,
                    record.stage().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable stage cannot be decoded".to_owned(),
                        )
                    })?,
                    record.terminal().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable terminal cannot be decoded".to_owned(),
                        )
                    })?,
                    record.reconstruction_source(),
                    record.durable_payload().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable payload cannot be decoded".to_owned(),
                        )
                    })?,
                    record.replay_authority.clone(),
                    record.continuation().ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "durable continuation cannot be decoded".to_owned(),
                        )
                    })?,
                    physical_slot_universes
                        .remove(&record.ordinal)
                        .expect("exact coverage checked above"),
                ))
            })
            .collect::<Result<Vec<_>, LifecycleLedgerError>>()?;
        let producer_debts = self
            .producer_debts
            .iter()
            .map(|debt| (debt.serve_ordinal(), debt.producer_ordinal()))
            .collect();
        Ok(RecoverySnapshot::new(
            self.context(),
            self.high_water(),
            records,
            producer_debts,
        ))
    }

    /// Construct and validate a canonical durable ledger.
    pub(super) fn new(
        context: LifecycleContext,
        high_water: u128,
        mut records: Vec<LifecycleLedgerRecordV1>,
        producer_debts: BTreeMap<u128, u128>,
    ) -> Result<Self, LifecycleLedgerError> {
        records.sort_by_key(LifecycleLedgerRecordV1::ordinal);
        let producer_debts = producer_debts
            .into_iter()
            .map(|(serve, producer)| LifecycleProducerDebtV1::new(serve, producer))
            .collect();
        let ledger = Self {
            format_version: LEDGER_VERSION,
            context: *context.id().as_bytes(),
            height: context.height(),
            high_water,
            records,
            producer_debts,
        };
        ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        Ok(ledger)
    }

    /// Construct the empty ledger for a validated height context.
    pub(super) fn empty(context: LifecycleContext) -> Self {
        Self {
            format_version: LEDGER_VERSION,
            context: *context.id().as_bytes(),
            height: context.height(),
            high_water: 0,
            records: Vec::new(),
            producer_debts: Vec::new(),
        }
    }

    /// Return the typed persisted context.
    pub(super) const fn context(&self) -> LifecycleContext {
        LifecycleContext::new(LifecycleDigest::new(self.context), self.height)
    }

    /// Return the durable ordinal high-water mark.
    pub(super) const fn high_water(&self) -> u128 {
        self.high_water
    }

    /// Borrow the canonical ordinal-ordered records.
    pub(super) fn records(&self) -> &[LifecycleLedgerRecordV1] {
        &self.records
    }

    #[cfg(test)]
    /// Substitute one structurally valid but foreign replay origin generation.
    pub(super) fn with_foreign_replay_authority_for_test(&self, ordinal: u128) -> Option<Self> {
        let mut changed = self.clone();
        let record = changed
            .records
            .iter_mut()
            .find(|record| record.ordinal == ordinal)?;
        record.replay_authority = record
            .replay_authority
            .with_foreign_origin_generation_for_test()?;
        changed.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).ok()?;
        Some(changed)
    }

    /// Borrow the canonical Serve-to-producer debts.
    pub(super) fn producer_debts(&self) -> &[LifecycleProducerDebtV1] {
        &self.producer_debts
    }

    /// Authenticate the unique live or already-repaired Validate parent of one WAL vote.
    ///
    /// This read-only projection binds the complete WAL identity to the exact
    /// LedgerV1 owner and admission ordinal. It accepts only the two crash
    /// states surrounding the existing fsync seam: a live uncontinued parent,
    /// or the exact Advanced-parent/live-Sign-child pair.
    pub(super) fn authenticate_recovered_wal_validate_parent(
        &self,
        recovered: &RecoveredWalVoteSign,
    ) -> Option<AuthenticatedRecoveredWalValidateLedgerParent> {
        let vote = recovered.vote();
        let mut context_bytes = [0_u8; 32];
        context_bytes.copy_from_slice(vote.round.context_id.0.as_ref());
        let wal_authority_is_exact = match vote.phase {
            wire::GlobalPhase::Prepare => recovered.prepare_certificate().is_none(),
            wire::GlobalPhase::Commit => recovered.prepare_certificate().is_some_and(|prepare| {
                prepare.phase == wire::GlobalPhase::Prepare
                    && prepare.round == vote.round
                    && prepare.proposal_round == vote.proposal_round
                    && prepare.subject == vote.subject
                    && prepare.execution_commitment == vote.execution_commitment
            }),
        };
        if !recovered.wal_identity().is_exact()
            || !recovered.replay_evidence_is_exact()
            || !wal_authority_is_exact
            || self.context().id() != LifecycleDigest::new(context_bytes)
            || self.context().height() != vote.round.height
            || vote.proposal_round.context_id != vote.round.context_id
            || vote.proposal_round.height != vote.round.height
            || recovered.tag().height() != vote.round.height
            || recovered.tag().view() != vote.round.view
        {
            return None;
        }
        let subject = projection::block_subject(vote.subject);
        let commitment = projection::execution_commitment(vote.execution_commitment);
        let round = LifecycleRound::new(vote.round.height, vote.round.view);
        let proposal_round =
            LifecycleRound::new(vote.proposal_round.height, vote.proposal_round.view);
        let mut parents = self.records.iter().filter(|record| {
            let Some(key) = record.key() else {
                return false;
            };
            let authority_is_exact = match vote.phase {
                wire::GlobalPhase::Prepare => key.execution_commitment().is_none(),
                wire::GlobalPhase::Commit => key
                    .execution_commitment()
                    .is_none_or(|candidate| candidate == commitment),
            };
            key.context() == self.context().id()
                && key.round() == round
                && key.proposal_round() == Some(proposal_round)
                && key.subject() == Some(subject)
                && key.phase() == LifecyclePhase::Validate
                && authority_is_exact
                && record.work_class() == Some(LifecycleWorkClass::Validate)
                && record.stage()
                    == Some(LifecycleStage::new(
                        LifecycleStageKind::ValidateBody,
                        PredecessorScope::Independent,
                    ))
                && record.reconstruction_source() == record.owner().causal_root().digest()
                && record
                    .durable_payload()
                    .is_some_and(|payload| durable_validate_payload_is_exact(key, payload))
        });
        let parent = parents.next()?;
        if parents.next().is_some() {
            return None;
        }
        let parent_key = parent.key()?;
        let inherited_prepare_authority = parent_key.execution_commitment().is_some();
        let edge = match vote.phase {
            wire::GlobalPhase::Prepare => DurableContinuationEdge::ValidateToSignPrepare,
            wire::GlobalPhase::Commit => DurableContinuationEdge::ValidateToSignCommit,
        };
        let child_phase = match vote.phase {
            wire::GlobalPhase::Prepare => LifecyclePhase::Prepare,
            wire::GlobalPhase::Commit => LifecyclePhase::Commit,
        };
        let child_stage = match vote.phase {
            wire::GlobalPhase::Prepare => LifecycleStageKind::SignPrepareVote,
            wire::GlobalPhase::Commit => LifecycleStageKind::SignCommitVote,
        };
        let child_key = LifecycleKey::new(
            self.context().id(),
            round,
            Some(proposal_round),
            Some(subject),
            child_phase,
            Some(commitment),
        );
        match (parent.terminal()?, parent.continuation()?) {
            (None, DurableContinuation::None) => {
                if self
                    .records
                    .iter()
                    .any(|record| record.key() == Some(child_key))
                {
                    return None;
                }
            }
            (Some(TerminalOutcome::Advanced), continuation) => {
                let (observed_edge, child_ordinal) = continuation.successor_parts()?;
                let child = self
                    .records
                    .iter()
                    .find(|record| record.ordinal() == child_ordinal)?;
                if observed_edge != edge
                    || child.key() != Some(child_key)
                    || child.owner() != parent.owner()
                    || child.work_class() != Some(LifecycleWorkClass::SignVote)
                    || child.stage()
                        != Some(LifecycleStage::new(
                            child_stage,
                            PredecessorScope::Independent,
                        ))
                    || child.terminal() != Some(None)
                    || child.reconstruction_source() != parent.reconstruction_source()
                    || child.durable_payload() != Some(DurablePayloadReference::None)
                    || child.continuation() != Some(DurableContinuation::None)
                {
                    return None;
                }
            }
            _ => return None,
        }
        Some(AuthenticatedRecoveredWalValidateLedgerParent {
            key: parent_key,
            owner: parent.owner(),
            ordinal: parent.ordinal(),
            payload: parent.durable_payload()?,
            replay_authority: parent.replay_authority.clone(),
            inherited_prepare_authority,
            wal_identity: recovered.wal_identity(),
            tag: recovered.tag(),
            vote: vote.clone(),
        })
    }

    /// Purely stage one adapter-authenticated WAL-ahead Validate-to-Sign repair.
    ///
    /// The only mutable shape is an exact live Validate parent with no child.
    /// It becomes `Advanced` and names a newly appended, same-owner Sign row at
    /// `high_water + 1`. An already repaired exact pair stutters. Every other
    /// parent/child arrangement fails before the returned ledger can be
    /// persisted by the future startup transaction.
    pub(super) fn stage_authenticated_wal_vote_repair(
        &self,
        repair: &AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !repair.concrete_pair_is_exact() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered WAL repair lost its concrete effect binding".to_owned(),
            ));
        }
        let parent_candidate = repair.parent();
        let child_candidate = repair.child();
        let parent_index = self
            .records
            .iter()
            .position(|record| record.key() == Some(parent_candidate.key))
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered WAL vote has no durable Validate parent".to_owned(),
                )
            })?;
        let parent = &self.records[parent_index];
        if !record_matches_recovery_candidate(parent, parent_candidate)
            || parent.work_class() != Some(LifecycleWorkClass::Validate)
            || parent.stage().is_none_or(|stage| {
                stage.kind() != LifecycleStageKind::ValidateBody
                    || stage.predecessor_scope() != PredecessorScope::Independent
            })
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered WAL vote changed its durable Validate parent".to_owned(),
            ));
        }

        let existing_child = self
            .records
            .iter()
            .find(|record| record.key() == Some(child_candidate.key));
        let continuation = parent.continuation().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered WAL parent continuation cannot be decoded".to_owned(),
            )
        })?;
        let terminal = parent.terminal().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered WAL parent terminal cannot be decoded".to_owned(),
            )
        })?;

        if let Some((edge, child_ordinal)) = continuation.successor_parts() {
            let child = existing_child.ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered WAL continuation lost its Sign child".to_owned(),
                )
            })?;
            if terminal != Some(TerminalOutcome::Advanced)
                || edge != repair.edge()
                || child.ordinal() != child_ordinal
                || child.owner() != parent.owner()
                || !record_matches_recovery_candidate(child, child_candidate)
                || child.terminal() != Some(None)
                || child.continuation() != Some(DurableContinuation::None)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "recovered WAL continuation conflicts with the durable Sign pair".to_owned(),
                ));
            }
            return Ok((self.clone(), child_ordinal, false));
        }

        if terminal.is_some()
            || continuation != DurableContinuation::None
            || existing_child.is_some()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered WAL vote does not match a live uncontinued Validate".to_owned(),
            ));
        }
        let child_ordinal = self.high_water.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger("recovered WAL Sign ordinal exhausted".to_owned())
        })?;
        let mut staged = self.clone();
        staged.records[parent_index].terminal =
            Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced));
        staged.records[parent_index].continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(repair.edge(), child_ordinal),
        );
        staged.records.push(LifecycleLedgerRecordV1::new(
            child_candidate.key,
            parent.owner(),
            child_ordinal,
            child_candidate.work_class,
            child_candidate.stage,
            None,
            child_candidate.reconstruction_source,
            child_candidate.payload,
            child_candidate.replay_authority.clone(),
            DurableContinuation::None,
        )?);
        staged.high_water = child_ordinal;
        staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        Ok((staged, child_ordinal, true))
    }

    fn validate(&self, max_records: usize) -> Result<(), LifecycleLedgerError> {
        if self.format_version != LEDGER_VERSION || self.records.len() > max_records {
            return Err(LifecycleLedgerError::InvalidLedger(
                "format version or record bound is invalid".to_owned(),
            ));
        }
        let context = self.context();
        let mut ordinals = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut owners = BTreeMap::new();
        let mut serve_requests = BTreeSet::new();
        let mut continuation_successors = BTreeSet::new();
        if self
            .records
            .windows(2)
            .any(|window| window[0].ordinal >= window[1].ordinal)
            || self
                .producer_debts
                .windows(2)
                .any(|window| window[0].serve_ordinal >= window[1].serve_ordinal)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "records or producer debts are not canonically ordered".to_owned(),
            ));
        }
        for record in &self.records {
            if !record.validate(context, self.high_water)
                || !ordinals.insert(record.ordinal)
                || !keys.insert(record.key)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "record identity, context, or schema is invalid".to_owned(),
                ));
            }
            let owner = record.owner();
            if owners
                .insert(owner.causal_root(), owner)
                .is_some_and(|known| known != owner)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "one causal root names multiple lifecycle owners".to_owned(),
                ));
            }
            if record.work_class() == Some(LifecycleWorkClass::CertifiedServe)
                && record
                    .durable_payload()
                    .and_then(DurablePayloadReference::request)
                    .is_none_or(|request| !serve_requests.insert(request))
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "one exact signed Serve request names multiple lifecycle records".to_owned(),
                ));
            }
        }
        for record in &self.records {
            let continuation = record
                .continuation()
                .expect("records validated before successor edges");
            if continuation != DurableContinuation::None
                && record.reconstruction_source() != record.owner().causal_root().digest()
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "durable continuation is not bound to its causal owner".to_owned(),
                ));
            }
            if continuation == DurableContinuation::AdvancedNoSuccessor
                && !durable_validate_payload_is_exact(
                    record
                        .key()
                        .expect("records validated before continuation payloads"),
                    record
                        .durable_payload()
                        .expect("records validated before continuation payloads"),
                )
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "advanced Validate without a successor lost its exact body frame".to_owned(),
                ));
            }
            let Some((edge, successor_ordinal)) = continuation.successor_parts() else {
                continue;
            };
            let successor = self
                .records
                .binary_search_by_key(&successor_ordinal, |candidate| candidate.ordinal)
                .ok()
                .and_then(|index| self.records.get(index));
            if !continuation_successors.insert(successor_ordinal)
                || successor.is_none_or(|successor| {
                    successor.owner() != record.owner()
                        || successor.reconstruction_source() != record.reconstruction_source()
                        || !durable_continuation_payload_is_exact(
                            edge,
                            record
                                .durable_payload()
                                .expect("records validated before successor edges"),
                            successor
                                .durable_payload()
                                .expect("records validated before successor edges"),
                        )
                        || !durable_continuation_successor_is_exact(
                            edge,
                            record
                                .work_class()
                                .expect("records validated before successor edges"),
                            record
                                .key()
                                .expect("records validated before successor edges"),
                            record
                                .stage()
                                .expect("records validated before successor edges"),
                            successor
                                .work_class()
                                .expect("records validated before successor edges"),
                            successor
                                .key()
                                .expect("records validated before successor edges"),
                            successor
                                .stage()
                                .expect("records validated before successor edges"),
                        )
                })
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "advanced body-stage successor is missing, aliased, or semantically foreign"
                        .to_owned(),
                ));
            }
        }
        for record in &self.records {
            let owner = record.owner();
            if self
                .records
                .binary_search_by_key(&owner.first_admission_ordinal(), |candidate| {
                    candidate.ordinal
                })
                .ok()
                .and_then(|index| self.records.get(index))
                .is_none_or(|first| first.owner() != owner)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "owner first ordinal has no matching tombstone or record".to_owned(),
                ));
            }
        }
        self.validate_debts()
    }

    fn validate_debts(&self) -> Result<(), LifecycleLedgerError> {
        let mut serves = BTreeSet::new();
        let mut producers = BTreeSet::new();
        for debt in &self.producer_debts {
            if !serves.insert(debt.serve_ordinal)
                || !producers.insert(debt.producer_ordinal)
                || debt.serve_ordinal.checked_add(1) != Some(debt.producer_ordinal)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "producer debt is non-adjacent or non-bijective".to_owned(),
                ));
            }
            let serve = self
                .records
                .binary_search_by_key(&debt.serve_ordinal, |record| record.ordinal)
                .ok()
                .and_then(|index| self.records.get(index));
            let producer = self
                .records
                .binary_search_by_key(&debt.producer_ordinal, |record| record.ordinal)
                .ok()
                .and_then(|index| self.records.get(index));
            if serve.and_then(LifecycleLedgerRecordV1::work_class)
                != Some(LifecycleWorkClass::CertifiedServe)
                || producer.and_then(LifecycleLedgerRecordV1::work_class)
                    != Some(LifecycleWorkClass::ProducerTurn)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "producer debt does not name a Serve/producer pair".to_owned(),
                ));
            }
        }
        for record in &self.records {
            let work_class = record.work_class().expect("records validated before debts");
            let terminal = record.terminal().expect("records validated before debts");
            match work_class {
                LifecycleWorkClass::CertifiedServe => {
                    let Some(producer_ordinal) = record.ordinal.checked_add(1) else {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "Serve ordinal cannot address its producer".to_owned(),
                        ));
                    };
                    let Some(producer) = self
                        .records
                        .binary_search_by_key(&producer_ordinal, |candidate| candidate.ordinal)
                        .ok()
                        .and_then(|index| self.records.get(index))
                    else {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "Serve record has no adjacent producer record".to_owned(),
                        ));
                    };
                    if producer.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                        || producer.owner() != record.owner()
                        || producer.reconstruction_source() != record.reconstruction_source()
                        || !producer
                            .replay_authority
                            .same_persisted_family(&record.replay_authority)
                        || !serve_and_producer_keys_match(
                            record.key().expect("records validated before debts"),
                            producer.key().expect("records validated before debts"),
                        )
                        || (terminal == Some(TerminalOutcome::Cancelled)
                            && producer.terminal() != Some(Some(TerminalOutcome::Cancelled)))
                        || (terminal.is_none() && !serves.contains(&record.ordinal))
                        || serves.contains(&record.ordinal)
                            != producer
                                .terminal()
                                .expect("records validated before debts")
                                .is_none()
                    {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "Serve/producer atomic pair is inconsistent".to_owned(),
                        ));
                    }
                }
                LifecycleWorkClass::ProducerTurn => {
                    let live = terminal.is_none();
                    let serve = record.ordinal.checked_sub(1).and_then(|ordinal| {
                        self.records
                            .binary_search_by_key(&ordinal, |candidate| candidate.ordinal)
                            .ok()
                            .and_then(|index| self.records.get(index))
                    });
                    if serve.and_then(LifecycleLedgerRecordV1::work_class)
                        != Some(LifecycleWorkClass::CertifiedServe)
                        || serve.is_none_or(|serve| serve.owner() != record.owner())
                        || serve.is_none_or(|serve| {
                            !serve_and_producer_keys_match(
                                serve.key().expect("records validated before debts"),
                                record.key().expect("records validated before debts"),
                            )
                        })
                        || producers.contains(&record.ordinal) != live
                    {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "producer debt does not match producer terminality".to_owned(),
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Focused projection of one real authenticated WAL repair through LedgerV1.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WalVoteLedgerRepairTestSummary {
    child_ordinal: u128,
    edge: DurableContinuationEdge,
    first_changed: bool,
    repeat_changed: bool,
    parent_advanced: bool,
    child_live: bool,
    high_water: u128,
    durable_frame_bound: bool,
    reopened_exact: bool,
}

#[cfg(test)]
impl WalVoteLedgerRepairTestSummary {
    /// Assemble the closed observations from the outer recovery fsync fixture.
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        child_ordinal: u128,
        edge: DurableContinuationEdge,
        first_changed: bool,
        repeat_changed: bool,
        parent_advanced: bool,
        child_live: bool,
        high_water: u128,
        durable_frame_bound: bool,
        reopened_exact: bool,
    ) -> Self {
        Self {
            child_ordinal,
            edge,
            first_changed,
            repeat_changed,
            parent_advanced,
            child_live,
            high_water,
            durable_frame_bound,
            reopened_exact,
        }
    }

    /// Return whether the first stage repaired the live parent.
    pub(crate) const fn first_changed(&self) -> bool {
        self.first_changed
    }

    /// Return whether the exact repeated stage changed the ledger.
    pub(crate) const fn repeat_changed(&self) -> bool {
        self.repeat_changed
    }

    /// Return whether the repaired parent is the exact Advanced tombstone.
    pub(crate) const fn parent_advanced(&self) -> bool {
        self.parent_advanced
    }

    /// Return whether the typed Sign child remains live.
    pub(crate) const fn child_live(&self) -> bool {
        self.child_live
    }

    /// Return the repaired child ordinal.
    pub(crate) const fn child_ordinal(&self) -> u128 {
        self.child_ordinal
    }

    /// Return whether the repair names the exact Validate-to-Prepare-Sign edge.
    pub(crate) const fn is_prepare_edge(&self) -> bool {
        matches!(self.edge, DurableContinuationEdge::ValidateToSignPrepare)
    }

    /// Return whether the repair names the exact Validate-to-Commit-Sign edge.
    pub(crate) const fn is_commit_edge(&self) -> bool {
        matches!(self.edge, DurableContinuationEdge::ValidateToSignCommit)
    }

    /// Return the repaired durable ordinal high-water mark.
    pub(crate) const fn high_water(&self) -> u128 {
        self.high_water
    }

    /// Return whether a nonzero complete-frame hash was bound post-fsync.
    pub(crate) const fn durable_frame_bound(&self) -> bool {
        self.durable_frame_bound
    }

    /// Return whether reopening reproduced the exact repaired ledger.
    pub(crate) const fn reopened_exact(&self) -> bool {
        self.reopened_exact
    }
}

fn record_matches_recovery_candidate(
    record: &LifecycleLedgerRecordV1,
    candidate: &CandidateAdmission,
) -> bool {
    candidate.initial_state == InitialLifecycleState::Ready
        && record.key() == Some(candidate.key)
        && record.owner().causal_root() == candidate.causal_root
        && record.work_class() == Some(candidate.work_class)
        && record.stage() == Some(candidate.stage)
        && record.reconstruction_source() == candidate.reconstruction_source
        && record
            .durable_payload()
            .is_some_and(|payload| payload.same_admission_material(candidate.payload))
        && record.replay_authority == candidate.replay_authority
}

/// Typed LifecycleLedgerV1 load or persistence failure.
#[derive(Debug, Error)]
pub(super) enum LifecycleLedgerError {
    /// A filesystem operation failed.
    #[error("{0}")]
    Io(String),
    /// Frame bytes were malformed or noncanonical.
    #[error("invalid LifecycleLedgerV1 frame: {0}")]
    InvalidFrame(String),
    /// Decoded logical state violated a durable invariant.
    #[error("invalid LifecycleLedgerV1 state: {0}")]
    InvalidLedger(String),
}

/// Post-fsync receipt for one exact WAL-ahead Validate-to-Sign ledger repair.
///
/// Construction is private to [`LifecycleLedgerStoreV1`]. The receipt binds
/// both semantic keys, the typed edge and child ordinal, and the complete
/// framed ledger bytes which were published before it was returned.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DurableWalVoteLedgerRepairReceipt {
    store_path: PathBuf,
    context: LifecycleContext,
    parent_key: LifecycleKey,
    child_key: LifecycleKey,
    edge: DurableContinuationEdge,
    child_ordinal: u128,
    ledger_frame_hash: LifecycleDigest,
}

impl DurableWalVoteLedgerRepairReceipt {
    /// Return whether this receipt names one exact authenticated repair.
    pub(super) fn matches(&self, repair: &AuthenticatedWalVoteLifecycleRepair) -> bool {
        self.context.id() == repair.parent().key.context()
            && self.context.height() == repair.parent().key.round().height()
            && self.parent_key == repair.parent().key
            && self.child_key == repair.child().key
            && self.edge == repair.edge()
            && self.child_ordinal != 0
    }

    /// Return the durable child ordinal named by the published ledger.
    pub(super) const fn child_ordinal(&self) -> u128 {
        self.child_ordinal
    }

    /// Return the hash of the complete canonical ledger frame.
    pub(super) const fn ledger_frame_hash(&self) -> LifecycleDigest {
        self.ledger_frame_hash
    }

    /// Return whether the receipt belongs to this exact opened ledger store.
    pub(super) fn belongs_to(&self, store: &LifecycleLedgerStoreV1) -> bool {
        store
            .load()
            .ok()
            .is_some_and(|ledger| self.belongs_to_loaded(store, &ledger))
    }

    /// Validate this receipt against one already-loaded frame from its store.
    /// Keeping this comparison load-free lets the Sign-install preflight bind
    /// the frame hash and repaired-pair shape to the same read.
    pub(super) fn belongs_to_loaded(
        &self,
        store: &LifecycleLedgerStoreV1,
        ledger: &LifecycleLedgerV1,
    ) -> bool {
        self.store_path == store.path
            && self.context == store.context
            && ledger.context() == self.context
            && encode_frame(ledger, store.max_frame_bytes)
                .ok()
                .is_some_and(|frame| {
                    LifecycleDigest::new(Hash::new(frame).into()) == self.ledger_frame_hash
                })
    }
}

/// Crash-safe, bounded store for one height-local LifecycleLedgerV1.
#[derive(Clone, Debug)]
pub(super) struct LifecycleLedgerStoreV1 {
    path: PathBuf,
    context: LifecycleContext,
    max_records: usize,
    max_frame_bytes: u64,
}

impl LifecycleLedgerStoreV1 {
    /// Open a height-local ledger under the coordinator's sealed size bounds.
    pub(super) fn open(
        root: &Path,
        context: LifecycleContext,
    ) -> Result<(Self, LifecycleLedgerV1), LifecycleLedgerError> {
        ensure_durable_ledger_directory(root)?;
        let store = Self {
            path: root.join(LEDGER_FILE),
            context,
            max_records: MAX_LIFECYCLE_RECORDS_PER_HEIGHT,
            max_frame_bytes: MAX_LEDGER_FRAME_BYTES,
        };
        let ledger = store.load()?;
        Ok((store, ledger))
    }

    fn load(&self) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(LifecycleLedgerV1::empty(self.context));
            }
            Err(error) => {
                return Err(LifecycleLedgerError::Io(format!(
                    "failed to inspect lifecycle ledger {}: {error}",
                    self.path.display()
                )));
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(LifecycleLedgerError::InvalidFrame(
                "ledger path is not a regular file".to_owned(),
            ));
        }
        if metadata.len() > self.max_frame_bytes {
            return Err(LifecycleLedgerError::InvalidFrame(
                "ledger exceeds its configured byte bound".to_owned(),
            ));
        }
        let read_limit = self.max_frame_bytes.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidFrame("ledger read bound overflowed".to_owned())
        })?;
        let mut bytes = Vec::new();
        File::open(&self.path)
            .and_then(|file| file.take(read_limit).read_to_end(&mut bytes))
            .map_err(|error| {
                LifecycleLedgerError::Io(format!(
                    "failed to read lifecycle ledger {}: {error}",
                    self.path.display()
                ))
            })?;
        let ledger = decode_frame(&bytes, self.max_frame_bytes)?;
        if ledger.context() != self.context {
            return Err(LifecycleLedgerError::InvalidLedger(
                "ledger belongs to another height context".to_owned(),
            ));
        }
        ledger.validate(self.max_records)?;
        Ok(ledger)
    }

    /// Reload and authenticate one already-fsynced WAL repair as an exact
    /// repaired-pair stutter.
    ///
    /// This is a read-only post-fsync/install preflight. It deliberately does
    /// not expose the loaded ledger: callers learn only whether the complete
    /// current frame contains the exact authenticated parent/child pair and
    /// durable child ordinal they already own.
    pub(super) fn revalidates_durable_authenticated_wal_vote_repair(
        &self,
        durable: &DurableAuthenticatedWalVoteLifecycleRepair,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        if !durable.belongs_to_loaded(self, &loaded) {
            return false;
        }
        let Ok((staged, observed_child_ordinal, changed)) =
            loaded.stage_authenticated_wal_vote_repair(durable.repair())
        else {
            return false;
        };
        !changed && observed_child_ordinal == durable.child_ordinal() && staged == loaded
    }

    /// Atomically replace the ledger after validating all durable invariants.
    pub(super) fn persist(&self, ledger: &LifecycleLedgerV1) -> Result<(), LifecycleLedgerError> {
        if ledger.context() != self.context {
            return Err(LifecycleLedgerError::InvalidLedger(
                "cannot persist a foreign height context".to_owned(),
            ));
        }
        ledger.validate(self.max_records)?;
        let bytes = encode_frame(ledger, self.max_frame_bytes)?;
        let parent = self.path.parent().ok_or_else(|| {
            LifecycleLedgerError::Io("ledger path has no parent directory".to_owned())
        })?;
        let temporary = self.path.with_extension("norito.tmp");
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(LifecycleLedgerError::InvalidFrame(
                    "ledger temporary path is not a regular file".to_owned(),
                ));
            }
            Ok(_) => {
                fs::remove_file(&temporary).map_err(|error| {
                    LifecycleLedgerError::Io(format!(
                        "failed to discard lifecycle ledger temporary file {}: {error}",
                        temporary.display()
                    ))
                })?;
                sync_ledger_directory(parent)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(LifecycleLedgerError::Io(format!(
                    "failed to inspect lifecycle ledger temporary file {}: {error}",
                    temporary.display()
                )));
            }
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                LifecycleLedgerError::Io(format!(
                    "failed to create lifecycle ledger temporary file {}: {error}",
                    temporary.display()
                ))
            })?;
        file.write_all(&bytes)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|error| {
                LifecycleLedgerError::Io(format!(
                    "failed to sync lifecycle ledger temporary file {}: {error}",
                    temporary.display()
                ))
            })?;
        fs::rename(&temporary, &self.path).map_err(|error| {
            LifecycleLedgerError::Io(format!(
                "failed to publish lifecycle ledger {}: {error}",
                self.path.display()
            ))
        })?;
        sync_ledger_directory(parent)?;
        Ok(())
    }

    /// Stage and fsync one authenticated WAL-ahead lifecycle repair.
    ///
    /// The receipt is minted only after the complete replacement frame and
    /// owning directory are synced. Exact repeats are persisted idempotently
    /// and receive the same frame-bound receipt.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn persist_authenticated_wal_vote_repair(
        &self,
        ledger: &LifecycleLedgerV1,
        repair: AuthenticatedWalVoteLifecycleRepair,
    ) -> Result<
        (
            LifecycleLedgerV1,
            DurableAuthenticatedWalVoteLifecycleRepair,
            bool,
        ),
        (LifecycleLedgerError, AuthenticatedWalVoteLifecycleRepair),
    > {
        let loaded = match self.load() {
            Ok(loaded) => loaded,
            Err(error) => return Err((error, repair)),
        };
        if &loaded != ledger {
            return Err((
                LifecycleLedgerError::InvalidLedger(
                    "WAL repair attempted to replace a stale ledger snapshot".to_owned(),
                ),
                repair,
            ));
        }
        let (staged, child_ordinal, changed) =
            match loaded.stage_authenticated_wal_vote_repair(&repair) {
                Ok(staged) => staged,
                Err(error) => return Err((error, repair)),
            };
        let frame = match encode_frame(&staged, self.max_frame_bytes) {
            Ok(frame) => frame,
            Err(error) => return Err((error, repair)),
        };
        if let Err(error) = self.persist(&staged) {
            return Err((error, repair));
        }
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: self.path.clone(),
            context: self.context,
            parent_key: repair.parent().key,
            child_key: repair.child().key,
            edge: repair.edge(),
            child_ordinal,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        debug_assert!(receipt.belongs_to(self));
        let durable = match repair.bind_durable_ledger_receipt(receipt) {
            Ok(durable) => durable,
            Err((repair, _receipt)) => {
                return Err((
                    LifecycleLedgerError::InvalidLedger(
                        "post-fsync WAL repair receipt did not bind its authority".to_owned(),
                    ),
                    repair,
                ));
            }
        };
        Ok((staged, durable, changed))
    }
}

fn sync_ledger_directory(directory: &Path) -> Result<(), LifecycleLedgerError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            LifecycleLedgerError::Io(format!(
                "failed to sync lifecycle ledger directory {}: {error}",
                directory.display()
            ))
        })
}

fn ensure_durable_ledger_directory(root: &Path) -> Result<(), LifecycleLedgerError> {
    ensure_durable_ledger_directory_with(root, &mut sync_ledger_directory)
}

fn ensure_durable_ledger_directory_with<Sync>(
    root: &Path,
    sync: &mut Sync,
) -> Result<(), LifecycleLedgerError>
where
    Sync: FnMut(&Path) -> Result<(), LifecycleLedgerError>,
{
    let parent = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(LifecycleLedgerError::InvalidFrame(
                    "ledger root is not a regular directory".to_owned(),
                ));
            }
            sync(root)?;
            if parent != root {
                sync(parent)?;
            }
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(LifecycleLedgerError::Io(format!(
                "failed to inspect lifecycle ledger root {}: {error}",
                root.display()
            )));
        }
    }

    ensure_durable_ledger_directory_with(parent, sync)?;
    match fs::create_dir(root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(LifecycleLedgerError::Io(format!(
                "failed to create lifecycle ledger root {}: {error}",
                root.display()
            )));
        }
    }
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        LifecycleLedgerError::Io(format!(
            "failed to inspect created lifecycle ledger root {}: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LifecycleLedgerError::InvalidFrame(
            "ledger root is not a regular directory".to_owned(),
        ));
    }
    sync(root)?;
    sync(parent)?;
    Ok(())
}

impl LifecycleCoordinator {
    pub(super) fn stage_durable_transaction(&self) -> Self {
        Self {
            episode_authority: self.episode_authority.clone(),
            active_context: self.active_context,
            records: self.records.clone(),
            key_index: self.key_index.clone(),
            owner_index: self.owner_index.clone(),
            ready_index: self.ready_index.clone(),
            admission_waits: self.admission_waits.clone(),
            active_lease: self.active_lease.clone(),
            high_water: self.high_water,
            next_lease: self.next_lease,
            durable_records: self.durable_records.clone(),
            capacity_geometry: self.capacity_geometry.clone(),
            capacity_used: self.capacity_used.clone(),
            capacity_generation: self.capacity_generation.clone(),
            observed_generation: self.observed_generation.clone(),
            producer_debts: self.producer_debts.clone(),
            ledger_store: self.ledger_store.clone(),
            fault: self.fault,
        }
    }

    pub(super) fn persist_durable_projection(&self) -> Result<(), LifecycleLedgerError> {
        let Some(store) = self.ledger_store.as_ref() else {
            return Ok(());
        };
        store.persist(&LifecycleLedgerV1::from_coordinator(self)?)
    }

    #[cfg(test)]
    pub(super) fn attach_empty_test_ledger(
        &mut self,
        root: &Path,
    ) -> Result<(), LifecycleLedgerError> {
        if self.ledger_store.is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "coordinator already owns a lifecycle ledger store".to_owned(),
            ));
        }
        let (store, existing) = LifecycleLedgerStoreV1::open(root, self.active_context)?;
        if existing.high_water != 0 || !existing.records.is_empty() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "test ledger attachment requires a new empty store".to_owned(),
            ));
        }
        store.persist(&LifecycleLedgerV1::from_coordinator(self)?)?;
        self.ledger_store = Some(store);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn redirect_test_ledger_to_missing_parent(&mut self, root: &Path) {
        self.ledger_store
            .as_mut()
            .expect("test ledger is attached")
            .path = root.join("missing-parent").join(LEDGER_FILE);
    }
}

fn encode_frame(
    ledger: &LifecycleLedgerV1,
    max_frame_bytes: u64,
) -> Result<Vec<u8>, LifecycleLedgerError> {
    let payload = ledger.encode();
    let payload_len = u64::try_from(payload.len()).map_err(|_| {
        LifecycleLedgerError::InvalidFrame("payload length is not representable".to_owned())
    })?;
    let frame_len = u64::try_from(HEADER_BYTES)
        .expect("header length fits u64")
        .checked_add(payload_len)
        .ok_or_else(|| LifecycleLedgerError::InvalidFrame("frame length overflowed".to_owned()))?;
    if frame_len > max_frame_bytes {
        return Err(LifecycleLedgerError::InvalidFrame(
            "frame exceeds its configured byte bound".to_owned(),
        ));
    }
    let digest = Hash::new(&payload);
    let mut frame =
        Vec::with_capacity(usize::try_from(frame_len).map_err(|_| {
            LifecycleLedgerError::InvalidFrame("frame is not addressable".to_owned())
        })?);
    frame.extend_from_slice(LEDGER_MAGIC);
    frame.extend_from_slice(&LEDGER_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(digest.as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_frame(
    bytes: &[u8],
    max_frame_bytes: u64,
) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_frame_bytes
        || bytes.len() < HEADER_BYTES
        || bytes.get(..LEDGER_MAGIC.len()) != Some(LEDGER_MAGIC.as_slice())
    {
        return Err(LifecycleLedgerError::InvalidFrame(
            "header or byte bound is invalid".to_owned(),
        ));
    }
    let version_offset = LEDGER_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| LifecycleLedgerError::InvalidFrame("version is truncated".to_owned()))?,
    );
    if version != LEDGER_VERSION {
        return Err(LifecycleLedgerError::InvalidFrame(format!(
            "unsupported frame version {version}"
        )));
    }
    let length_offset = version_offset + 2;
    let payload_len = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| LifecycleLedgerError::InvalidFrame("length is truncated".to_owned()))?,
    );
    let payload_len = usize::try_from(payload_len)
        .map_err(|_| LifecycleLedgerError::InvalidFrame("payload is not addressable".to_owned()))?;
    let digest_offset = length_offset + 8;
    let payload_offset = digest_offset + HASH_BYTES;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err(LifecycleLedgerError::InvalidFrame(
            "frame length is inconsistent".to_owned(),
        ));
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
        return Err(LifecycleLedgerError::InvalidFrame(
            "checksum mismatch".to_owned(),
        ));
    }
    let mut cursor = payload;
    let ledger = LifecycleLedgerV1::decode_all(&mut cursor).map_err(|error| {
        LifecycleLedgerError::InvalidFrame(format!("Norito decode failed: {error}"))
    })?;
    if ledger.encode() != payload {
        return Err(LifecycleLedgerError::InvalidFrame(
            "payload is not canonically encoded".to_owned(),
        ));
    }
    Ok(ledger)
}

fn work_shape_is_valid(
    work_class: LifecycleWorkClass,
    key: LifecycleKey,
    stage: LifecycleStage,
) -> bool {
    work_class.accepts_stage(key.phase(), stage)
}

fn phase_code(phase: LifecyclePhase) -> u16 {
    match phase {
        LifecyclePhase::Proposal => 1,
        LifecyclePhase::Prepare => 2,
        LifecyclePhase::Commit => 3,
        LifecyclePhase::Timeout => 4,
        LifecyclePhase::Fetch => 5,
        LifecyclePhase::Store => 6,
        LifecyclePhase::Validate => 7,
        LifecyclePhase::Apply => 8,
        LifecyclePhase::BroadcastProposal => 9,
        LifecyclePhase::BroadcastPrepareVote => 10,
        LifecyclePhase::BroadcastCommitVote => 11,
        LifecyclePhase::BroadcastPrepareQc => 12,
        LifecyclePhase::BroadcastCommitQc => 13,
        LifecyclePhase::BroadcastTimeoutVote => 14,
        LifecyclePhase::BroadcastTc => 15,
        LifecyclePhase::EnterView => 16,
        LifecyclePhase::DiagnosticProposalEquivocation => 17,
        LifecyclePhase::DiagnosticVoteEquivocation => 18,
        LifecyclePhase::DiagnosticTimeoutEquivocation => 19,
        LifecyclePhase::DiagnosticInvalidBody => 20,
        LifecyclePhase::Serve => 21,
        LifecyclePhase::ProducerTurn => 22,
    }
}

fn decode_phase(code: u16) -> Option<LifecyclePhase> {
    Some(match code {
        1 => LifecyclePhase::Proposal,
        2 => LifecyclePhase::Prepare,
        3 => LifecyclePhase::Commit,
        4 => LifecyclePhase::Timeout,
        5 => LifecyclePhase::Fetch,
        6 => LifecyclePhase::Store,
        7 => LifecyclePhase::Validate,
        8 => LifecyclePhase::Apply,
        9 => LifecyclePhase::BroadcastProposal,
        10 => LifecyclePhase::BroadcastPrepareVote,
        11 => LifecyclePhase::BroadcastCommitVote,
        12 => LifecyclePhase::BroadcastPrepareQc,
        13 => LifecyclePhase::BroadcastCommitQc,
        14 => LifecyclePhase::BroadcastTimeoutVote,
        15 => LifecyclePhase::BroadcastTc,
        16 => LifecyclePhase::EnterView,
        17 => LifecyclePhase::DiagnosticProposalEquivocation,
        18 => LifecyclePhase::DiagnosticVoteEquivocation,
        19 => LifecyclePhase::DiagnosticTimeoutEquivocation,
        20 => LifecyclePhase::DiagnosticInvalidBody,
        21 => LifecyclePhase::Serve,
        22 => LifecyclePhase::ProducerTurn,
        _ => return None,
    })
}

fn work_class_code(work_class: LifecycleWorkClass) -> u16 {
    match work_class {
        LifecycleWorkClass::SignProposal => 1,
        LifecycleWorkClass::SignVote => 2,
        LifecycleWorkClass::SignTimeout => 3,
        LifecycleWorkClass::Fetch => 4,
        LifecycleWorkClass::Store => 5,
        LifecycleWorkClass::Validate => 6,
        LifecycleWorkClass::Apply => 7,
        LifecycleWorkClass::Broadcast => 8,
        LifecycleWorkClass::EnterView => 9,
        LifecycleWorkClass::EquivocationReport => 10,
        LifecycleWorkClass::InvalidBodyReport => 11,
        LifecycleWorkClass::CertifiedServe => 12,
        LifecycleWorkClass::ProducerTurn => 13,
    }
}

fn decode_work_class(code: u16) -> Option<LifecycleWorkClass> {
    Some(match code {
        1 => LifecycleWorkClass::SignProposal,
        2 => LifecycleWorkClass::SignVote,
        3 => LifecycleWorkClass::SignTimeout,
        4 => LifecycleWorkClass::Fetch,
        5 => LifecycleWorkClass::Store,
        6 => LifecycleWorkClass::Validate,
        7 => LifecycleWorkClass::Apply,
        8 => LifecycleWorkClass::Broadcast,
        9 => LifecycleWorkClass::EnterView,
        10 => LifecycleWorkClass::EquivocationReport,
        11 => LifecycleWorkClass::InvalidBodyReport,
        12 => LifecycleWorkClass::CertifiedServe,
        13 => LifecycleWorkClass::ProducerTurn,
        _ => return None,
    })
}

fn stage_kind_code(kind: LifecycleStageKind) -> u16 {
    match kind {
        LifecycleStageKind::SignProposal => 1,
        LifecycleStageKind::SignPrepareVote => 2,
        LifecycleStageKind::SignCommitVote => 3,
        LifecycleStageKind::SignTimeoutVote => 4,
        LifecycleStageKind::FetchBody => 5,
        LifecycleStageKind::StoreBody => 6,
        LifecycleStageKind::ValidateBody => 7,
        LifecycleStageKind::ApplyDecision => 8,
        LifecycleStageKind::BroadcastProposal => 9,
        LifecycleStageKind::BroadcastPrepareVote => 10,
        LifecycleStageKind::BroadcastCommitVote => 11,
        LifecycleStageKind::BroadcastPrepareQc => 12,
        LifecycleStageKind::BroadcastCommitQc => 13,
        LifecycleStageKind::BroadcastTimeoutVote => 14,
        LifecycleStageKind::BroadcastTc => 15,
        LifecycleStageKind::EnterView => 16,
        LifecycleStageKind::ReportProposalEquivocation => 17,
        LifecycleStageKind::ReportVoteEquivocation => 18,
        LifecycleStageKind::ReportTimeoutEquivocation => 19,
        LifecycleStageKind::ReportInvalidBody => 20,
        LifecycleStageKind::CertifiedServe => 21,
        LifecycleStageKind::ProducerTurn => 22,
    }
}

fn decode_stage_kind(code: u16) -> Option<LifecycleStageKind> {
    Some(match code {
        1 => LifecycleStageKind::SignProposal,
        2 => LifecycleStageKind::SignPrepareVote,
        3 => LifecycleStageKind::SignCommitVote,
        4 => LifecycleStageKind::SignTimeoutVote,
        5 => LifecycleStageKind::FetchBody,
        6 => LifecycleStageKind::StoreBody,
        7 => LifecycleStageKind::ValidateBody,
        8 => LifecycleStageKind::ApplyDecision,
        9 => LifecycleStageKind::BroadcastProposal,
        10 => LifecycleStageKind::BroadcastPrepareVote,
        11 => LifecycleStageKind::BroadcastCommitVote,
        12 => LifecycleStageKind::BroadcastPrepareQc,
        13 => LifecycleStageKind::BroadcastCommitQc,
        14 => LifecycleStageKind::BroadcastTimeoutVote,
        15 => LifecycleStageKind::BroadcastTc,
        16 => LifecycleStageKind::EnterView,
        17 => LifecycleStageKind::ReportProposalEquivocation,
        18 => LifecycleStageKind::ReportVoteEquivocation,
        19 => LifecycleStageKind::ReportTimeoutEquivocation,
        20 => LifecycleStageKind::ReportInvalidBody,
        21 => LifecycleStageKind::CertifiedServe,
        22 => LifecycleStageKind::ProducerTurn,
        _ => return None,
    })
}

const fn predecessor_code(scope: PredecessorScope) -> u8 {
    match scope {
        PredecessorScope::Independent => 0,
        PredecessorScope::ReadyOrdinalPrefix => 1,
        PredecessorScope::ProducerHandoffBarrier => 2,
    }
}

const fn decode_predecessor(code: u8) -> Option<PredecessorScope> {
    match code {
        0 => Some(PredecessorScope::Independent),
        1 => Some(PredecessorScope::ReadyOrdinalPrefix),
        2 => Some(PredecessorScope::ProducerHandoffBarrier),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> LifecycleDigest {
        LifecycleDigest::new([byte; 32])
    }

    fn context() -> LifecycleContext {
        LifecycleContext::new(digest(1), 7)
    }

    fn key(seed: u8, phase: LifecyclePhase) -> LifecycleKey {
        let stage = match phase {
            LifecyclePhase::Serve => LifecycleStageKind::CertifiedServe,
            LifecyclePhase::ProducerTurn => LifecycleStageKind::ProducerTurn,
            _ => panic!("ledger key fixture only covers Serve/ProducerTurn"),
        };
        super::super::replay_authority::exact_record_fixture(context(), stage, seed).key
    }

    fn stage(kind: LifecycleStageKind) -> LifecycleStage {
        LifecycleStage::new(
            kind,
            if kind == LifecycleStageKind::ProducerTurn {
                PredecessorScope::ProducerHandoffBarrier
            } else {
                PredecessorScope::ReadyOrdinalPrefix
            },
        )
    }

    fn owner(first: u128) -> OwnerId {
        OwnerId::new(CausalRoot::new(digest(9)), first)
    }

    fn body_key(
        phase: LifecyclePhase,
        _execution_commitment: Option<LifecycleDigest>,
    ) -> LifecycleKey {
        let stage = match phase {
            LifecyclePhase::Fetch => LifecycleStageKind::FetchBody,
            LifecyclePhase::Store => LifecycleStageKind::StoreBody,
            LifecyclePhase::Validate => LifecycleStageKind::ValidateBody,
            LifecyclePhase::Apply => LifecycleStageKind::ApplyDecision,
            LifecyclePhase::Prepare => LifecycleStageKind::SignPrepareVote,
            LifecyclePhase::Commit => LifecycleStageKind::SignCommitVote,
            LifecyclePhase::DiagnosticInvalidBody => LifecycleStageKind::ReportInvalidBody,
            _ => panic!("ledger body fixture received a non-body phase"),
        };
        super::super::replay_authority::exact_record_fixture(context(), stage, 3).key
    }

    fn body_stage(kind: LifecycleStageKind) -> LifecycleStage {
        LifecycleStage::new(kind, PredecessorScope::Independent)
    }

    pub(super) fn replay_authority_for(
        key: LifecycleKey,
        stage: LifecycleStage,
        payload: DurablePayloadReference,
    ) -> LifecycleReplayAuthorityV1 {
        let seed = u8::try_from(key.round().view()).expect("fixture view fits u8");
        let record_context = LifecycleContext::new(key.context(), key.round().height());
        let case = super::super::replay_authority::exact_record_fixture(
            record_context,
            stage.kind(),
            seed,
        );
        if stage.kind() == LifecycleStageKind::CertifiedServe {
            return case
                .authority
                .terminalized_certified_serve(record_context, key, stage, payload)
                .unwrap_or(case.authority);
        }
        case.authority
    }

    fn exact_body_payload(stage: LifecycleStageKind) -> DurablePayloadReference {
        super::super::replay_authority::exact_record_fixture(context(), stage, 3).payload
    }

    #[test]
    fn body_frame_reference_roundtrips_and_is_bound_to_the_body_key() {
        let key = body_key(LifecyclePhase::Store, None);
        let payload = exact_body_payload(LifecycleStageKind::StoreBody);
        let encoded = LifecyclePayloadReferenceV1::from_schema(key, payload)
            .expect("exact body reference projects into LedgerV1");
        assert!(encoded.validate());
        assert_eq!(encoded.to_schema(key), Some(payload));

        let foreign_key = LifecycleKey::new(
            key.context(),
            key.round(),
            key.proposal_round(),
            Some(digest(43)),
            LifecyclePhase::Store,
            key.execution_commitment(),
        );
        assert!(LifecyclePayloadReferenceV1::from_schema(foreign_key, payload).is_none());
        assert_eq!(encoded.to_schema(foreign_key), None);

        let record = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            key,
            owner(1),
            1,
            LifecycleWorkClass::Store,
            body_stage(LifecycleStageKind::StoreBody),
            None,
            digest(9),
            payload,
            DurableContinuation::None,
        )
        .expect("body-bound Store record");
        LifecycleLedgerV1::new(context(), 1, vec![record], BTreeMap::new())
            .expect("body-bound Store ledger");

        let validate_key = body_key(LifecyclePhase::Validate, None);
        let validate_reference = DurableBodyFrameReference::new(
            context().id(),
            validate_key
                .proposal_round()
                .expect("Validate proposal round"),
            validate_key.subject().expect("Validate subject"),
            digest(41),
            digest(42),
        );
        let invalid_validate = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            validate_key,
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Rejected(7)),
            digest(9),
            DurablePayloadReference::BodyFrame(validate_reference),
            DurableContinuation::None,
        )
        .expect("construct invalid terminal Validate fixture");
        assert_invalid_records(1, vec![invalid_validate]);

        let mut corrupted = encoded;
        *corrupted
            .canonical_reference
            .last_mut()
            .expect("body reference has canonical bytes") ^= 1;
        assert!(!corrupted.validate());
        assert_eq!(corrupted.to_schema(key), None);
    }

    #[test]
    fn recovered_validate_parent_matches_only_its_exact_durable_body_frame() {
        let context_hash = Hash::new(b"recovered Validate body-frame context");
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(*context_hash.as_ref()),
            context().height(),
        );
        let (replay, durable) = super::super::replay_authority::exact_body_record_fixture(
            active_context,
            LifecycleStageKind::ValidateBody,
            3,
        );
        let round = durable.round();
        let subject = durable.subject();
        let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"parent state"),
            Hash::new(b"post state"),
            Hash::new(b"ordinary writes"),
            1,
            Hash::new(b"executed block"),
        );
        let parent = AuthenticatedRecoveredWalValidateLedgerParent {
            key: replay.key,
            owner: owner(1),
            ordinal: 1,
            payload: replay.payload,
            replay_authority: replay.authority,
            inherited_prepare_authority: false,
            wal_identity: RecoveredWalFrameIdentity::for_test(0, 1, [0xA5; 32]),
            tag: EventTag::new(
                round.height,
                round.view,
                crate::sumeragi::v2_core::Generation::new(0),
            ),
            vote: wire::Vote {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: commitment,
                signer: 0,
                signature: Vec::new(),
            },
        };
        assert!(parent.matches_durable_receipt(active_context, &durable));

        let substituted = DurableBodyReceipt::for_test(
            durable.context_id(),
            round,
            subject,
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                b"substituted recovered Validate manifest",
            )),
        );
        assert!(!parent.matches_durable_receipt(active_context, &substituted));
    }

    fn validate_successor_pair(
        edge: DurableContinuationEdge,
    ) -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        let (child_phase, child_class, child_stage) = match edge {
            DurableContinuationEdge::ValidateToApply => (
                LifecyclePhase::Apply,
                LifecycleWorkClass::Apply,
                LifecycleStageKind::ApplyDecision,
            ),
            DurableContinuationEdge::ValidateToInvalidBodyReport => (
                LifecyclePhase::DiagnosticInvalidBody,
                LifecycleWorkClass::InvalidBodyReport,
                LifecycleStageKind::ReportInvalidBody,
            ),
            DurableContinuationEdge::ValidateToSignPrepare => (
                LifecyclePhase::Prepare,
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignPrepareVote,
            ),
            DurableContinuationEdge::ValidateToSignCommit => (
                LifecyclePhase::Commit,
                LifecycleWorkClass::SignVote,
                LifecycleStageKind::SignCommitVote,
            ),
            DurableContinuationEdge::FetchToStore | DurableContinuationEdge::StoreToValidate => {
                panic!("Validate fixture requires a Validate continuation edge")
            }
        };
        let parent_key = body_key(LifecyclePhase::Validate, None);
        let body_frame = exact_body_payload(LifecycleStageKind::ValidateBody);
        let parent = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            parent_key,
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            body_frame,
            DurableContinuation::successor(edge, 2),
        )
        .expect("valid advanced Validate ledger row");
        let child_payload = match edge {
            DurableContinuationEdge::ValidateToApply => {
                exact_body_payload(LifecycleStageKind::ApplyDecision)
            }
            DurableContinuationEdge::ValidateToInvalidBodyReport
            | DurableContinuationEdge::ValidateToSignPrepare
            | DurableContinuationEdge::ValidateToSignCommit => DurablePayloadReference::None,
            DurableContinuationEdge::FetchToStore | DurableContinuationEdge::StoreToValidate => {
                unreachable!("Validate fixture excludes pre-Validate edges")
            }
        };
        let child = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(child_phase, Some(digest(41))),
            owner(1),
            2,
            child_class,
            body_stage(child_stage),
            None,
            digest(9),
            child_payload,
            DurableContinuation::None,
        )
        .expect("valid live Apply ledger row");
        (parent, child)
    }

    fn validate_apply_pair() -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        validate_successor_pair(DurableContinuationEdge::ValidateToApply)
    }

    fn complete_body_pipeline_chain() -> Vec<LifecycleLedgerRecordV1> {
        let commitment = Some(digest(41));
        [
            (
                LifecyclePhase::Fetch,
                LifecycleWorkClass::Fetch,
                LifecycleStageKind::FetchBody,
                1,
                Some(TerminalOutcome::Advanced),
                DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
            ),
            (
                LifecyclePhase::Store,
                LifecycleWorkClass::Store,
                LifecycleStageKind::StoreBody,
                2,
                Some(TerminalOutcome::Advanced),
                DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 3),
            ),
            (
                LifecyclePhase::Validate,
                LifecycleWorkClass::Validate,
                LifecycleStageKind::ValidateBody,
                3,
                Some(TerminalOutcome::Advanced),
                DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 4),
            ),
            (
                LifecyclePhase::Apply,
                LifecycleWorkClass::Apply,
                LifecycleStageKind::ApplyDecision,
                4,
                None,
                DurableContinuation::None,
            ),
        ]
        .into_iter()
        .map(
            |(phase, work_class, stage_kind, ordinal, terminal, continuation)| {
                let key = body_key(phase, commitment);
                let payload = exact_body_payload(stage_kind);
                LifecycleLedgerRecordV1::new_exact_replay_fixture(
                    key,
                    owner(1),
                    ordinal,
                    work_class,
                    body_stage(stage_kind),
                    terminal,
                    digest(9),
                    payload,
                    continuation,
                )
                .expect("valid complete body-pipeline ledger row")
            },
        )
        .collect()
    }

    fn assert_invalid_records(high_water: u128, records: Vec<LifecycleLedgerRecordV1>) {
        assert!(matches!(
            LifecycleLedgerV1::new(context(), high_water, records, BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn durable_body_successor_edges_reject_mixed_or_substituted_frames() {
        let frame_for = |key: LifecycleKey, byte: u8| {
            DurablePayloadReference::BodyFrame(DurableBodyFrameReference::new(
                key.context(),
                key.proposal_round().expect("body proposal round"),
                key.subject().expect("body subject"),
                digest(41),
                digest(byte),
            ))
        };

        let commitment = Some(digest(41));
        let store_key = body_key(LifecyclePhase::Store, commitment);
        let validate_key = body_key(LifecyclePhase::Validate, commitment);
        let store_frame = frame_for(store_key, 42);
        let foreign_frame = frame_for(validate_key, 43);
        let store = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            store_key,
            owner(1),
            1,
            LifecycleWorkClass::Store,
            body_stage(LifecycleStageKind::StoreBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            store_frame,
            DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 2),
        )
        .expect("construct body-bound Store parent");
        let foreign_validate = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            validate_key,
            owner(1),
            2,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            None,
            digest(9),
            foreign_frame,
            DurableContinuation::None,
        )
        .expect("construct substituted Validate child");
        assert_invalid_records(2, vec![store.clone(), foreign_validate]);
        let missing_validate = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            validate_key,
            owner(1),
            2,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            None,
            digest(9),
            DurablePayloadReference::None,
            DurableContinuation::None,
        )
        .expect("construct mixed Validate child");
        assert_invalid_records(2, vec![store, missing_validate]);

        let (mut validate, mut apply) = validate_apply_pair();
        let validate_key = validate.key().expect("decode Validate key");
        let apply_key = apply.key().expect("decode Apply key");
        validate.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(validate_key, frame_for(validate_key, 44))
                .expect("encode Validate body frame");
        apply.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(apply_key, frame_for(apply_key, 45))
                .expect("encode substituted Apply body frame");
        assert_invalid_records(2, vec![validate, apply]);
    }

    #[test]
    fn payload_free_store_and_apply_rows_are_not_ledger_v1() {
        for (phase, work_class, stage_kind) in [
            (
                LifecyclePhase::Store,
                LifecycleWorkClass::Store,
                LifecycleStageKind::StoreBody,
            ),
            (
                LifecyclePhase::Apply,
                LifecycleWorkClass::Apply,
                LifecycleStageKind::ApplyDecision,
            ),
        ] {
            let record = LifecycleLedgerRecordV1::new_exact_replay_fixture(
                body_key(phase, Some(digest(41))),
                owner(1),
                1,
                work_class,
                body_stage(stage_kind),
                None,
                digest(9),
                DurablePayloadReference::None,
                DurableContinuation::None,
            )
            .expect("construct a locally decodable payload-free body-stage row");
            assert_invalid_records(1, vec![record]);
        }
    }

    fn serve_pair() -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        let pending = super::super::replay_authority::exact_record_fixture(
            context(),
            LifecycleStageKind::CertifiedServe,
            2,
        )
        .payload;
        let DurablePayloadReference::CertifiedServePending {
            request,
            certificate,
        } = pending
        else {
            unreachable!("canonical Serve fixture has pending durable material")
        };
        let serve = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            key(2, LifecyclePhase::Serve),
            owner(1),
            1,
            LifecycleWorkClass::CertifiedServe,
            stage(LifecycleStageKind::CertifiedServe),
            Some(TerminalOutcome::Completed(Some(digest(23)))),
            digest(20),
            DurablePayloadReference::CertifiedServeCompleted {
                request,
                certificate,
                response: digest(23),
            },
            DurableContinuation::None,
        )
        .expect("valid Serve ledger record");
        let producer = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            key(2, LifecyclePhase::ProducerTurn),
            owner(1),
            2,
            LifecycleWorkClass::ProducerTurn,
            stage(LifecycleStageKind::ProducerTurn),
            None,
            digest(20),
            DurablePayloadReference::None,
            DurableContinuation::None,
        )
        .expect("valid producer ledger record");
        (serve, producer)
    }

    #[test]
    fn serve_debt_rejects_individually_valid_foreign_producer_family() {
        let (serve, mut producer) = serve_pair();
        producer.replay_authority =
            super::super::replay_authority::foreign_certified_serve_family_authority_fixture(
                context(),
                LifecycleStageKind::ProducerTurn,
                2,
            );
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn frame_roundtrip_is_canonical_and_preserves_high_water() {
        let (serve, producer) = serve_pair();
        let ledger = LifecycleLedgerV1::new(
            context(),
            9,
            vec![producer, serve],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid ledger");
        let frame = encode_frame(&ledger, 1024 * 1024).expect("encode frame");
        let decoded = decode_frame(&frame, 1024 * 1024).expect("decode frame");
        assert_eq!(decoded, ledger);
        assert_eq!(decoded.high_water(), 9);
        assert_eq!(decoded.records()[0].ordinal(), 1);
        assert_eq!(decoded.records()[1].ordinal(), 2);
    }

    #[test]
    fn advanced_validate_roundtrip_authenticates_its_exact_apply_successor() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (parent, child) = validate_apply_pair();
        let ledger = LifecycleLedgerV1::new(context(), 2, vec![parent, child], BTreeMap::new())
            .expect("exact Validate-to-Apply successor is durable");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open ledger store");
        assert!(empty.records().is_empty());
        store.persist(&ledger).expect("persist successor edge");
        let (_, reopened) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("reopen successor edge");
        assert_eq!(reopened, ledger);
        assert_eq!(
            reopened.records()[0].continuation(),
            Some(DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                2,
            ))
        );

        let snapshot = reopened
            .recovery_snapshot(BTreeMap::from([(1, BTreeSet::new()), (2, BTreeSet::new())]))
            .expect("recovery retains the typed successor edge");
        assert_eq!(
            snapshot.records[0].continuation,
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 2)
        );
        assert_eq!(snapshot.records[1].continuation, DurableContinuation::None);
    }

    #[test]
    fn complete_body_pipeline_chain_roundtrips_all_successor_edges() {
        let ledger = LifecycleLedgerV1::new(
            context(),
            4,
            complete_body_pipeline_chain(),
            BTreeMap::new(),
        )
        .expect("all three exact body-pipeline edges form one durable chain");
        assert_eq!(
            ledger
                .records()
                .iter()
                .map(LifecycleLedgerRecordV1::continuation)
                .collect::<Vec<_>>(),
            vec![
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    2,
                )),
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::StoreToValidate,
                    3,
                )),
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::ValidateToApply,
                    4,
                )),
                Some(DurableContinuation::None),
            ]
        );
        let snapshot = ledger
            .recovery_snapshot((1..=4).map(|ordinal| (ordinal, BTreeSet::new())).collect())
            .expect("complete body-pipeline chain survives recovery projection");
        assert_eq!(
            snapshot
                .records
                .iter()
                .map(|record| record.continuation)
                .collect::<Vec<_>>(),
            vec![
                DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
                DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 3),
                DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 4),
                DurableContinuation::None,
            ]
        );
    }

    #[test]
    fn all_validate_continuations_roundtrip_with_canonical_wire_shapes() {
        for edge in [
            DurableContinuationEdge::ValidateToApply,
            DurableContinuationEdge::ValidateToInvalidBodyReport,
            DurableContinuationEdge::ValidateToSignPrepare,
            DurableContinuationEdge::ValidateToSignCommit,
        ] {
            let (parent, child) = validate_successor_pair(edge);
            let ledger = LifecycleLedgerV1::new(context(), 2, vec![parent, child], BTreeMap::new())
                .expect("typed Validate successor edge is valid");
            let frame = encode_frame(&ledger, 1024 * 1024).expect("encode typed continuation");
            let decoded = decode_frame(&frame, 1024 * 1024).expect("decode typed continuation");
            assert_eq!(
                decoded.records()[0].continuation(),
                Some(DurableContinuation::successor(edge, 2))
            );
        }

        let no_successor = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(LifecyclePhase::Validate, None),
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            exact_body_payload(LifecycleStageKind::ValidateBody),
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct no-successor Validate tombstone");
        let ledger = LifecycleLedgerV1::new(context(), 1, vec![no_successor], BTreeMap::new())
            .expect("Validate may finish without a child");
        assert_eq!(
            ledger.records()[0].continuation(),
            Some(DurableContinuation::AdvancedNoSuccessor)
        );

        let payload_free_no_successor = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(LifecyclePhase::Validate, None),
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            DurablePayloadReference::None,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("the local row shape is checked again by the complete ledger relation");
        assert_invalid_records(1, vec![payload_free_no_successor]);

        let payload_free_live = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            body_key(LifecyclePhase::Validate, None),
            owner(1),
            1,
            LifecycleWorkClass::Validate,
            body_stage(LifecycleStageKind::ValidateBody),
            None,
            digest(9),
            DurablePayloadReference::None,
            DurableContinuation::None,
        )
        .expect("the complete ledger rejects a payload-free live Validate row");
        assert_invalid_records(1, vec![payload_free_live]);
    }

    #[test]
    fn persisted_continuation_rejects_unknown_and_noncanonical_option_shapes() {
        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1 {
            code: PersistedDurableContinuationV1::VALIDATE_TO_APPLY,
            successor_ordinal: None,
        };
        assert_invalid_records(2, vec![parent, child.clone()]);

        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1 {
            code: PersistedDurableContinuationV1::NONE,
            successor_ordinal: Some(2),
        };
        assert_invalid_records(2, vec![parent, child.clone()]);

        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1 {
            code: u8::MAX,
            successor_ordinal: Some(2),
        };
        assert_invalid_records(2, vec![parent, child]);
    }

    #[test]
    fn advanced_validate_rejects_missing_or_foreign_successor_edges() {
        let (mut parent, child) = validate_apply_pair();
        parent.continuation =
            PersistedDurableContinuationV1::from_schema(DurableContinuation::None);
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![parent, child.clone()], BTreeMap::new(),),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (mut parent, child) = validate_apply_pair();
        parent.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 3),
        );
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 3, vec![parent, child], BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (parent, mut foreign_owner) = validate_apply_pair();
        foreign_owner.causal_root = *digest(55).as_bytes();
        foreign_owner.owner_first_ordinal = 2;
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![parent, foreign_owner], BTreeMap::new(),),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (parent, mut foreign_lineage) = validate_apply_pair();
        foreign_lineage.key.subject = Some(*digest(56).as_bytes());
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![parent, foreign_lineage], BTreeMap::new(),),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn typed_continuation_rejects_live_backward_and_unauthenticated_edges() {
        let (mut live_parent, child) = validate_apply_pair();
        live_parent.terminal = None;
        assert_invalid_records(2, vec![live_parent, child]);

        let (mut cancelled_parent, child) = validate_apply_pair();
        cancelled_parent.terminal =
            Some(PersistedTerminalV1::from_schema(TerminalOutcome::Cancelled));
        assert_invalid_records(2, vec![cancelled_parent, child]);

        let (mut backward_parent, child) = validate_apply_pair();
        backward_parent.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 1),
        );
        assert_invalid_records(2, vec![backward_parent, child]);

        let (parent, mut foreign_child_source) = validate_apply_pair();
        foreign_child_source.reconstruction_source = *digest(57).as_bytes();
        assert_invalid_records(2, vec![parent, foreign_child_source]);

        let (mut foreign_parent_source, mut foreign_child_source) = validate_apply_pair();
        foreign_parent_source.reconstruction_source = *digest(58).as_bytes();
        foreign_child_source.reconstruction_source = *digest(58).as_bytes();
        assert_invalid_records(2, vec![foreign_parent_source, foreign_child_source]);

        let (mut absent_proposal_parent, mut absent_proposal_child) = validate_apply_pair();
        absent_proposal_parent.key.proposal_height = None;
        absent_proposal_parent.key.proposal_view = None;
        absent_proposal_child.key.proposal_height = None;
        absent_proposal_child.key.proposal_view = None;
        assert_invalid_records(2, vec![absent_proposal_parent, absent_proposal_child]);

        let (parent, mut foreign_scope) = validate_apply_pair();
        foreign_scope.predecessor_code = predecessor_code(PredecessorScope::ReadyOrdinalPrefix);
        assert_invalid_records(2, vec![parent, foreign_scope]);

        let (mut inherited_commitment, mut substituted_commitment) = validate_apply_pair();
        inherited_commitment.key.execution_commitment = Some(*digest(41).as_bytes());
        substituted_commitment.key.execution_commitment = Some(*digest(42).as_bytes());
        assert_invalid_records(2, vec![inherited_commitment, substituted_commitment]);

        let (parent, mut linked_live_apply) = validate_apply_pair();
        linked_live_apply.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 2),
        );
        assert_invalid_records(2, vec![parent, linked_live_apply]);

        let (mut wrong_edge, child) = validate_apply_pair();
        wrong_edge.continuation = PersistedDurableContinuationV1::from_schema(
            DurableContinuation::successor(DurableContinuationEdge::ValidateToSignPrepare, 2),
        );
        assert_invalid_records(2, vec![wrong_edge, child]);

        let mut no_child = validate_apply_pair().0;
        no_child.continuation =
            PersistedDurableContinuationV1::from_schema(DurableContinuation::AdvancedNoSuccessor);
        no_child.reconstruction_source = *digest(58).as_bytes();
        assert_invalid_records(1, vec![no_child]);

        let mut chain = complete_body_pipeline_chain();
        let mut fetch_without_child = chain.remove(0);
        fetch_without_child.continuation =
            PersistedDurableContinuationV1::from_schema(DurableContinuation::AdvancedNoSuccessor);
        assert_invalid_records(1, vec![fetch_without_child]);
    }

    #[test]
    fn first_ledger_directory_creation_fails_closed_until_parent_sync() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let parent = temporary.path().join("fresh-parent");
        let ledger_root = parent.join("ledger");
        let mut injected = false;
        let result = ensure_durable_ledger_directory_with(&ledger_root, &mut |path| {
            if !injected && ledger_root.exists() && path == parent {
                injected = true;
                return Err(LifecycleLedgerError::Io(
                    "injected parent synchronisation failure".to_owned(),
                ));
            }
            sync_ledger_directory(path)
        });
        assert!(matches!(result, Err(LifecycleLedgerError::Io(_))));
        assert!(injected);

        let (_store, ledger) = LifecycleLedgerStoreV1::open(&ledger_root, context())
            .expect("retry synchronises the existing root before exposure");
        assert_eq!(ledger.context(), context());
        assert_eq!(ledger.high_water(), 0);
        assert!(ledger.records().is_empty());
    }

    #[test]
    fn store_roundtrip_rejects_corrupt_and_foreign_frames() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open empty store");
        assert_eq!(empty, LifecycleLedgerV1::empty(context()));
        let (serve, producer) = serve_pair();
        let ledger = LifecycleLedgerV1::new(
            context(),
            2,
            vec![serve, producer],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid ledger");
        store.persist(&ledger).expect("persist ledger");
        let (_, loaded) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("reload ledger");
        assert_eq!(loaded, ledger);

        let mut frame = fs::read(root.path().join(LEDGER_FILE)).expect("ledger frame");
        *frame.last_mut().expect("nonempty frame") ^= 0x80;
        fs::write(root.path().join(LEDGER_FILE), frame).expect("corrupt fixture");
        assert!(matches!(
            LifecycleLedgerStoreV1::open(root.path(), context()),
            Err(LifecycleLedgerError::InvalidFrame(_))
        ));
    }

    #[test]
    fn durable_repair_receipt_reloads_the_current_store_frame() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open empty store");
        assert!(empty.records().is_empty());
        let (serve, producer) = serve_pair();
        let ledger = LifecycleLedgerV1::new(
            context(),
            2,
            vec![serve.clone(), producer.clone()],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid receipt frame");
        store.persist(&ledger).expect("persist receipt frame");
        let frame = encode_frame(&ledger, store.max_frame_bytes).expect("encode receipt frame");
        let receipt = DurableWalVoteLedgerRepairReceipt {
            store_path: store.path.clone(),
            context: context(),
            parent_key: serve.key().expect("Serve key"),
            child_key: producer.key().expect("producer key"),
            edge: DurableContinuationEdge::ValidateToSignPrepare,
            child_ordinal: 2,
            ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()),
        };
        assert!(receipt.belongs_to(&store));

        let replaced = LifecycleLedgerV1::new(
            context(),
            3,
            vec![serve, producer],
            BTreeMap::from([(1, 2)]),
        )
        .expect("valid later frame");
        store.persist(&replaced).expect("replace receipt frame");
        assert!(
            !receipt.belongs_to(&store),
            "a receipt for earlier bytes cannot authorize a later same-path frame"
        );
    }

    #[test]
    fn store_discards_regular_temp_residue_and_rejects_nonregular_temp_paths() {
        let root = tempfile::tempdir().expect("temporary directory");
        let (store, empty) =
            LifecycleLedgerStoreV1::open(root.path(), context()).expect("open empty store");
        let temporary = root.path().join(LEDGER_FILE).with_extension("norito.tmp");
        fs::write(&temporary, b"interrupted ledger write").expect("temporary crash residue");
        store
            .persist(&empty)
            .expect("regular crash residue is safely replaced");
        assert!(!temporary.exists());

        fs::create_dir(&temporary).expect("nonregular temporary path");
        assert!(matches!(
            store.persist(&empty),
            Err(LifecycleLedgerError::InvalidFrame(_))
        ));
    }

    #[test]
    fn malformed_owner_and_producer_debt_are_rejected() {
        let (serve, mut producer) = serve_pair();
        producer.owner_first_ordinal = 2;
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve.clone(), producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
        let (_, producer) = serve_pair();
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                3,
                vec![serve, producer],
                BTreeMap::from([(1, 3)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (serve, mut producer) = serve_pair();
        producer.predecessor_code = predecessor_code(PredecessorScope::ReadyOrdinalPrefix);
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (serve, mut producer) = serve_pair();
        producer.reconstruction_source = *digest(99).as_bytes();
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));

        let (serve, mut producer) = serve_pair();
        producer.key = PersistedLifecycleKeyV1::from_schema(key(3, LifecyclePhase::ProducerTurn));
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn completed_and_cancelled_atomic_pairs_are_valid_without_debt() {
        let (mut serve, producer) = serve_pair();
        serve.terminal = None;
        serve.payload_reference =
            LifecyclePayloadReferenceV1::certified_serve_pending(digest(2), digest(21), digest(22));
        LifecycleLedgerV1::new(
            context(),
            2,
            vec![serve, producer],
            BTreeMap::from([(1, 2)]),
        )
        .expect("live atomic pair");

        let (serve, mut producer) = serve_pair();
        producer.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced));
        LifecycleLedgerV1::new(context(), 2, vec![serve, producer], BTreeMap::new())
            .expect("completed atomic pair");

        let (mut serve, mut producer) = serve_pair();
        serve.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Cancelled));
        serve.payload_reference = LifecyclePayloadReferenceV1::certified_serve_negative(
            digest(2),
            digest(21),
            digest(22),
            DurableServeNegativeOutcome::Cancelled,
        );
        producer.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Cancelled));
        LifecycleLedgerV1::new(context(), 2, vec![serve, producer], BTreeMap::new())
            .expect("cancelled atomic pair");
    }

    #[test]
    fn negative_terminal_kinds_are_not_interchangeable() {
        let (mut serve, producer) = serve_pair();
        serve.terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Rejected(
            7,
        )));
        serve.payload_reference = LifecyclePayloadReferenceV1::certified_serve_negative(
            digest(2),
            digest(21),
            digest(22),
            DurableServeNegativeOutcome::Failed(7),
        );
        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn one_signed_serve_request_cannot_own_two_lifecycle_pairs() {
        let (serve, producer) = serve_pair();
        let (mut duplicate_serve, mut duplicate_producer) = serve_pair();
        duplicate_serve.key = PersistedLifecycleKeyV1::from_schema(key(4, LifecyclePhase::Serve));
        duplicate_serve.causal_root = *digest(10).as_bytes();
        duplicate_serve.owner_first_ordinal = 3;
        duplicate_serve.ordinal = 3;
        duplicate_serve.payload_reference = LifecyclePayloadReferenceV1::certified_serve_completed(
            digest(4),
            digest(21),
            digest(22),
            digest(23),
        );
        duplicate_producer.key =
            PersistedLifecycleKeyV1::from_schema(key(4, LifecyclePhase::ProducerTurn));
        duplicate_producer.causal_root = *digest(10).as_bytes();
        duplicate_producer.owner_first_ordinal = 3;
        duplicate_producer.ordinal = 4;

        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                4,
                vec![serve, producer, duplicate_serve, duplicate_producer],
                BTreeMap::from([(1, 2), (3, 4)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn orphan_serve_or_producer_records_are_rejected() {
        let (serve, producer) = serve_pair();
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![serve], BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
        assert!(matches!(
            LifecycleLedgerV1::new(context(), 2, vec![producer], BTreeMap::new()),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }

    #[test]
    fn opaque_or_noncanonical_certified_serve_references_are_rejected() {
        let (mut serve, producer) = serve_pair();
        serve.payload_reference.canonical_reference = vec![1, 2, 3];
        let digest = Hash::new(&serve.payload_reference.canonical_reference);
        serve
            .payload_reference
            .digest
            .copy_from_slice(digest.as_ref());

        assert!(matches!(
            LifecycleLedgerV1::new(
                context(),
                2,
                vec![serve, producer],
                BTreeMap::from([(1, 2)]),
            ),
            Err(LifecycleLedgerError::InvalidLedger(_))
        ));
    }
}
