//! Versioned durable ledger for Sumeragi v2 lifecycle ownership.
//!
//! The ledger persists only restart-stable logical state. Readiness, leases,
//! wait generations, physical carriers, and scheduler episodes are rebuilt
//! from authenticated storage after restart and never appear in this format.
use super::projection::{AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError};
use super::replay_authority::{
    AuthenticatedRecoveredDurableCertifiedBodyPipelineCensusV1,
    AuthenticatedRecoveredDurableCertifiedBodyPipelineEntryV1,
    AuthenticatedRecoveredDurableCertifiedFetchV1,
    AuthenticatedRecoveredDurableStandaloneValidateV1, CertifiedServeTerminalReplayAuthorityPairV1,
    LifecycleReplayAuthorityV1, RecoveredDecisionApplyCandidateLineageV1,
    authenticate_recovered_durable_certified_fetch,
    authenticate_recovered_durable_standalone_validate,
    recovered_decision_body_continuation_is_exact,
    seal_recovered_durable_certified_body_pipeline_census, signed_broadcast_continuation_is_exact,
};
use super::schema::{
    DurableBodyFrameReference, DurableContinuation, DurableContinuationEdge,
    MAX_LIFECYCLE_RECORDS_PER_HEIGHT, serve_and_producer_keys_match,
};
use super::wal_recovery::{
    AuthenticatedRecoveredWalControlProjection, AuthenticatedRecoveredWalDecisionFetchProjection,
    AuthenticatedWalVoteLifecycleRepair, DurableAuthenticatedWalVoteLifecycleRepair,
    RecoveredDecisionFetchStoreProjectionV1, RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    RecoveredLifecycleSignedBroadcastProjectionV1,
};
use super::{
    CandidateAdmission, CausalRoot, DurablePayloadReference, DurableServeNegativeOutcome,
    InitialLifecycleState, LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey,
    LifecyclePhase, LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleState,
    LifecycleWorkClass, LifecycleWorkRegistryHolder, OwnerId, PhysicalSlotId, PredecessorScope,
    ProductionLifecycleOwnerV1, RecoveredLifecycleRecord, RecoveredWalProductionOwnerOpenV1,
    RecoverySnapshot, TerminalOutcome,
};
#[cfg(test)]
use super::{TurnOutcome, TurnPlan};
use super::{
    authority,
    body_pipeline_transition::{
        durable_continuation_payload_is_exact, durable_continuation_successor_is_exact,
        durable_validate_payload_is_exact,
    },
    open::{
        AuthenticatedLifecycleRecoveryCut, CompleteTipServeRetirementReconciliationV1,
        LifecycleOpenError, LifecycleRecoveryAssemblyError,
    },
    projection,
};
use crate::sumeragi::{
    v2::{
        ProductionLifecycleAdapterStartupV1, ProductionRecoveredLifecycleOwnerAssemblyPermitV1,
        RecoveredWalFrameIdentity, RecoveredWalVoteSign, VerifiedHeightContext,
    },
    v2_body_store::{BlockSignaturePolicy, DurableBodyReceipt, V2BodyStore},
    v2_certified_serve_payload_store::{
        AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadRecoveryError,
        CertifiedServePayloadStoreError, CertifiedServePayloadStoreV1,
        CertifiedServeTerminalPersistenceError,
    },
    v2_core::EventTag,
    v2_runtime::{PendingRuntimeEffectBinding, RecoveredWalCandidateProjectionPermit},
};
use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll, Encode};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
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
    const SIGN_PROPOSAL_TO_BROADCAST: u8 = 8;
    const SIGN_PREPARE_TO_BROADCAST: u8 = 9;
    const SIGN_COMMIT_TO_BROADCAST: u8 = 10;
    const SIGN_TIMEOUT_TO_BROADCAST: u8 = 11;
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
                    DurableContinuationEdge::SignProposalToBroadcast => {
                        Self::SIGN_PROPOSAL_TO_BROADCAST
                    }
                    DurableContinuationEdge::SignPrepareToBroadcast => {
                        Self::SIGN_PREPARE_TO_BROADCAST
                    }
                    DurableContinuationEdge::SignCommitToBroadcast => {
                        Self::SIGN_COMMIT_TO_BROADCAST
                    }
                    DurableContinuationEdge::SignTimeoutToBroadcast => {
                        Self::SIGN_TIMEOUT_TO_BROADCAST
                    }
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
            (Self::SIGN_PROPOSAL_TO_BROADCAST, Some(ordinal)) => {
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::SignProposalToBroadcast,
                    ordinal,
                ))
            }
            (Self::SIGN_PREPARE_TO_BROADCAST, Some(ordinal)) => {
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::SignPrepareToBroadcast,
                    ordinal,
                ))
            }
            (Self::SIGN_COMMIT_TO_BROADCAST, Some(ordinal)) => {
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::SignCommitToBroadcast,
                    ordinal,
                ))
            }
            (Self::SIGN_TIMEOUT_TO_BROADCAST, Some(ordinal)) => {
                Some(DurableContinuation::successor(
                    DurableContinuationEdge::SignTimeoutToBroadcast,
                    ordinal,
                ))
            }
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
/// Move-only all-row finalization successor prepared from one exact frame.
///
/// Its fields remain private to the ledger module, so siblings cannot combine
/// an arbitrary current frame with a caller-built terminal successor before
/// the exact publication transaction.
#[must_use = "staged finalization retirement must cross its exact LedgerV1 publication"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct StagedFinalizationRetirementV1 {
    current: LifecycleLedgerV1,
    retired: LifecycleLedgerV1,
}

/// Move-only proof that the exact all-row finalization successor is durable.
///
/// The publication token retains both the pre-fsync and retired frames. Only
/// its consuming commit may clear the concrete registry and logical
/// coordinator, so no sibling can name a post-publication tail using raw
/// caller-supplied ledgers.
#[must_use = "published finalization retirement must commit its in-memory owners"]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct PublishedFinalizationRetirementV1 {
    coordinator: LifecycleCoordinator,
    current: LifecycleLedgerV1,
    retired: LifecycleLedgerV1,
    retained_floor: PublishedFinalizedLifecycleRetainedFloorV1,
}

/// Exact durable ordinal floor published by one finalized lifecycle owner.
///
/// The token retains the physically present retired frame and its opened store.
/// It is minted only after fsync/readback and can initialize only the sealed
/// canonical successor target derived by the same live Kura lineage.
#[must_use = "the finalized lifecycle floor must bind its canonical successor"]
pub(in crate::sumeragi) struct PublishedFinalizedLifecycleRetainedFloorV1 {
    store: LifecycleLedgerStoreV1,
    ledger: LifecycleLedgerV1,
    present: AuthenticatedPresentLifecycleFrameV1,
}

/// Exact successor frame initialized from one finalized predecessor floor.
///
/// This capability stays inside recovered lifecycle storage authority until
/// the production owner opens the same coordinator frame. It exposes neither
/// the floor nor either filesystem target.
#[must_use = "the authenticated successor floor must join the opened lifecycle owner"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredLifecycleSuccessorFloorV1 {
    store: LifecycleLedgerStoreV1,
    ledger: LifecycleLedgerV1,
    retained_high_water: u128,
}

impl PublishedFinalizationRetirementV1 {
    fn authenticates_source(&self) -> bool {
        LifecycleLedgerV1::from_coordinator(&self.coordinator)
            .is_ok_and(|ledger| ledger == self.current)
    }

    /// Consume the exact logical and concrete owners after durable publication.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn consume_owners(
        self,
        mut registry: LifecycleWorkRegistryHolder,
    ) -> PublishedFinalizedLifecycleRetainedFloorV1 {
        assert!(self.authenticates_source());
        assert!(
            registry
                .registry_mut()
                .exactly_covers_finalization_work(&self.coordinator),
            "published finalization must consume the preflighted concrete census",
        );
        assert_eq!(self.current.context(), self.retired.context());
        assert_eq!(self.current.high_water(), self.retired.high_water());
        assert_eq!(self.current.records().len(), self.retired.records().len());
        assert!(self.retired.producer_debts.is_empty());
        assert!(
            self.retired
                .records()
                .iter()
                .all(|record| record.terminal().is_some_and(|terminal| terminal.is_some()))
        );
        let retained_floor = self.retained_floor;
        drop(registry);
        drop(self.coordinator);
        retained_floor
    }
}

impl PublishedFinalizedLifecycleRetainedFloorV1 {
    /// Consume this finalized frame into the exact sealed H+1 storage target.
    pub(in crate::sumeragi) fn initialize_successor(
        self,
        target: crate::sumeragi::v2::RecoveredLifecycleSuccessorFloorTargetV1,
    ) -> Result<AuthenticatedRecoveredLifecycleSuccessorFloorV1, LifecycleLedgerError> {
        let predecessor_root = self.store.path.parent().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle floor has no canonical predecessor root".to_owned(),
            )
        })?;
        if !self.present.exactly_matches(&self.store, &self.ledger)
            || !target.authorizes_predecessor(predecessor_root, self.ledger.context())
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "finalized lifecycle floor does not bind the sealed successor".to_owned(),
            ));
        }
        let retained_high_water = self.ledger.high_water();
        let (successor_root, successor_context) = target.into_successor_target();
        let (store, ledger) = open_initialized_or_descendant_lifecycle_successor(
            &successor_root,
            successor_context,
            retained_high_water,
        )?;
        Ok(AuthenticatedRecoveredLifecycleSuccessorFloorV1 {
            store,
            ledger,
            retained_high_water,
        })
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Join the live-rollover floor with the exact coordinator opened at H+1.
    pub(in crate::sumeragi) fn authenticate_recovered_successor_floor(
        self,
        floor: AuthenticatedRecoveredLifecycleSuccessorFloorV1,
    ) -> Result<Self, LifecycleLedgerError> {
        let opened = LifecycleLedgerV1::from_coordinator(&self.coordinator)?;
        let Some(owner_store) = self.coordinator.ledger_store.as_ref() else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered successor floor opened without an attached ledger store".to_owned(),
            ));
        };
        if self.verified.context().height != opened.context().height()
            || self.verified.context().id().0.as_ref() != opened.context().id().as_bytes()
            || !owner_store.same_publication_target(&floor.store)
            || opened != floor.ledger
            || floor.store.load()? != floor.ledger
            || (floor.ledger.records().is_empty()
                && floor.ledger.high_water() != floor.retained_high_water)
            || floor.ledger.high_water() < floor.retained_high_water
            || floor.ledger.records().iter().any(|record| {
                record.ordinal() <= floor.retained_high_water
                    || record.owner().first_admission_ordinal() <= floor.retained_high_water
            })
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "opened lifecycle owner changed its authenticated successor floor".to_owned(),
            ));
        }
        Ok(self)
    }
}

/// One-shot proof that decoded replay data is still enclosed by its exact
/// opened LedgerV1 row while joining the authenticated body-store frame.
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct DurableCertifiedFetchLedgerJoinPermit {
    _linearity: DurableCertifiedFetchLedgerJoinLinearity,
}
struct DurableCertifiedFetchLedgerJoinLinearity;
impl Drop for DurableCertifiedFetchLedgerJoinLinearity {
    fn drop(&mut self) {}
}
impl DurableCertifiedFetchLedgerJoinPermit {
    fn new() -> Self {
        Self {
            _linearity: DurableCertifiedFetchLedgerJoinLinearity,
        }
    }
}
/// One-shot proof that decoded standalone Validate replay data remains
/// enclosed by its exact opened LedgerV1 row while joining the authenticated
/// body-store frame.
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct DurableStandaloneValidateLedgerJoinPermit {
    _linearity: DurableStandaloneValidateLedgerJoinLinearity,
}
struct DurableStandaloneValidateLedgerJoinLinearity;
impl Drop for DurableStandaloneValidateLedgerJoinLinearity {
    fn drop(&mut self) {}
}
impl DurableStandaloneValidateLedgerJoinPermit {
    fn new() -> Self {
        Self {
            _linearity: DurableStandaloneValidateLedgerJoinLinearity,
        }
    }
}
/// One-shot proof that the recovery projection contains every live ordinary
/// certified-body pipeline row selected from one exact opened LedgerV1.
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct DurableCertifiedBodyPipelineLedgerCensusPermit
{
    _linearity: DurableCertifiedBodyPipelineLedgerCensusLinearity,
    ledger_frame_identity: LifecycleDigest,
    live_ordinals: BTreeSet<u128>,
}
struct DurableCertifiedBodyPipelineLedgerCensusLinearity;
impl Drop for DurableCertifiedBodyPipelineLedgerCensusLinearity {
    fn drop(&mut self) {}
}
impl DurableCertifiedBodyPipelineLedgerCensusPermit {
    fn new(ledger: &LifecycleLedgerV1, live_ordinals: BTreeSet<u128>) -> Option<Self> {
        if live_ordinals.iter().any(|ordinal| {
            ledger
                .records
                .binary_search_by_key(ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| ledger.records.get(index))
                .is_none_or(|record| {
                    record.terminal() != Some(None)
                        || !matches!(
                            record.work_class(),
                            Some(
                                LifecycleWorkClass::Fetch
                                    | LifecycleWorkClass::Store
                                    | LifecycleWorkClass::Validate
                            )
                        )
                        || !matches!(
                            record.durable_payload(),
                            Some(DurablePayloadReference::BodyFrame(_))
                        )
                })
        }) {
            return None;
        }
        Some(Self {
            _linearity: DurableCertifiedBodyPipelineLedgerCensusLinearity,
            ledger_frame_identity: ledger.frame_identity(),
            live_ordinals,
        })
    }
    /// Consume the census permit into its exact frame and selected live rows.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn into_parts(
        self,
    ) -> (LifecycleDigest, BTreeSet<u128>) {
        (self.ledger_frame_identity, self.live_ordinals)
    }
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
    /// Decode one live signed Broadcast only as an inert recovered-WAL child.
    ///
    /// This keeps the replay envelope inside its checksummed LedgerV1 row.
    /// The returned value is not execution authority: its recovered WAL parent
    /// must still reconstruct the exact pending binding and the verified height
    /// must authenticate the signature before cold startup can advance.
    pub(super) fn project_recovered_signed_broadcast_child(
        &self,
        context: LifecycleContext,
    ) -> Option<super::replay_authority::DurableRecoveredSignedBroadcastChildV1> {
        super::replay_authority::project_durable_recovered_signed_broadcast_child(
            context,
            self.key()?,
            self.work_class()?,
            self.stage()?,
            self.terminal()?,
            self.reconstruction_source(),
            self.owner(),
            self.durable_payload()?,
            self.continuation()?,
            &self.replay_authority,
        )
    }
    /// Authenticate this row's source before opening its exact body-store frame.
    fn authenticate_durable_certified_fetch_origin<F>(
        &self,
        verified: &VerifiedHeightContext,
        authenticate_body: F,
    ) -> Result<Option<AuthenticatedRecoveredDurableCertifiedFetchV1>, DurableBodyFrameRecoveryError>
    where
        F: FnOnce() -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError>,
    {
        let Some(key) = self.key() else {
            return Ok(None);
        };
        let Some(stage) = self.stage() else {
            return Ok(None);
        };
        let Some(payload) = self.durable_payload() else {
            return Ok(None);
        };
        let Some(()) = (self.work_class() == Some(LifecycleWorkClass::Fetch)
            && self.reconstruction_source() == self.owner().causal_root().digest())
        .then_some(()) else {
            return Ok(None);
        };
        authenticate_recovered_durable_certified_fetch(
            DurableCertifiedFetchLedgerJoinPermit::new(),
            verified,
            key,
            self.owner(),
            self.ordinal(),
            stage,
            self.reconstruction_source(),
            payload,
            &self.replay_authority,
            authenticate_body,
        )
    }
    /// Authenticate this standalone Validate row's exact LocalBody or signed
    /// remote-Proposal source before opening its retained body-store frame.
    fn authenticate_durable_standalone_validate_origin<F>(
        &self,
        verified: &VerifiedHeightContext,
        authenticate_body: F,
    ) -> Result<
        Option<AuthenticatedRecoveredDurableStandaloneValidateV1>,
        DurableBodyFrameRecoveryError,
    >
    where
        F: FnOnce() -> Result<AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError>,
    {
        let Some(key) = self.key() else {
            return Ok(None);
        };
        let Some(stage) = self.stage() else {
            return Ok(None);
        };
        let Some(payload) = self.durable_payload() else {
            return Ok(None);
        };
        let Some(()) = (self.work_class() == Some(LifecycleWorkClass::Validate)
            && self.reconstruction_source() == self.owner().causal_root().digest())
        .then_some(()) else {
            return Ok(None);
        };
        authenticate_recovered_durable_standalone_validate(
            DurableStandaloneValidateLedgerJoinPermit::new(),
            verified,
            key,
            self.owner(),
            self.ordinal(),
            stage,
            self.reconstruction_source(),
            payload,
            &self.replay_authority,
            authenticate_body,
        )
    }
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
    /// Corrupt only the persisted work-class code for a storage-classifier test.
    #[cfg(test)]
    pub(super) fn with_work_class_for_test(mut self, work_class: LifecycleWorkClass) -> Self {
        self.work_class_code = work_class_code(work_class);
        self
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
/// Closed original-Sign lineage of one durable Broadcast-plus-next-Sign pair.
///
/// The phase case retains the exact Validate ordinal which introduced the
/// historical Prepare Sign. The control case has no body-stage predecessor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) enum RecoveredLifecycleSignedBroadcastAndSignParentV1 {
    /// A Proposal Sign advanced directly to its signed Proposal Broadcast.
    ControlProposal,
    /// A Validate advanced to the Prepare Sign which produced the Broadcast.
    PhasePrepare {
        /// Exact durable Validate ordinal in the historical causal owner.
        validate_ordinal: u128,
    },
}
/// Opaque LedgerV1 classification of one committed Broadcast-plus-next-Sign pair.
///
/// This projection authenticates the exact durable row shape and retains the
/// complete ledger-frame identity. Cold owner assembly joins it to the
/// historical WAL request, current WAL vote, body marker, and verified roster
/// before either live child enters the registry.
#[must_use = "a classified durable Broadcast-plus-next-Sign pair must remain frame-bound"]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1 {
    ledger_frame_identity: LifecycleDigest,
    parent: RecoveredLifecycleSignedBroadcastAndSignParentV1,
    parent_ordinal: u128,
    broadcast_ordinal: u128,
    next_sign_ordinal: u128,
}
/// One-pass indexes used by the durable Broadcast-plus-next-Sign classifier.
///
/// Values in `successor_parents` retain the first parent record index and the
/// complete incoming-edge count. The classifier therefore detects ambiguity
/// without rescanning the bounded ledger for every eligible Broadcast.
struct RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1 {
    successor_parents: BTreeMap<u128, (usize, usize)>,
    owner_record_counts: BTreeMap<OwnerId, usize>,
}
impl RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1 {
    fn new(records: &[LifecycleLedgerRecordV1]) -> Self {
        let mut successor_parents = BTreeMap::new();
        let mut owner_record_counts = BTreeMap::new();
        for (index, record) in records.iter().enumerate() {
            owner_record_counts
                .entry(record.owner())
                .and_modify(|count: &mut usize| *count = count.saturating_add(1))
                .or_insert(1);
            if let Some((_, successor_ordinal)) = record
                .continuation()
                .and_then(DurableContinuation::successor_parts)
            {
                successor_parents
                    .entry(successor_ordinal)
                    .and_modify(|(_, count): &mut (usize, usize)| {
                        *count = count.saturating_add(1);
                    })
                    .or_insert((index, 1));
            }
        }
        Self {
            successor_parents,
            owner_record_counts,
        }
    }
    fn unique_parent_index(&self, successor_ordinal: u128) -> Option<usize> {
        self.successor_parents
            .get(&successor_ordinal)
            .and_then(|&(index, count)| (count == 1).then_some(index))
    }
    fn has_incoming_edge(&self, ordinal: u128) -> bool {
        self.successor_parents.contains_key(&ordinal)
    }
    fn owner_record_count(&self, owner: OwnerId) -> usize {
        self.owner_record_counts.get(&owner).copied().unwrap_or(0)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1 {
    /// Return the closed historical parent classification.
    pub(in crate::sumeragi) const fn parent(
        &self,
    ) -> RecoveredLifecycleSignedBroadcastAndSignParentV1 {
        self.parent
    }
    /// Return the original Sign ordinal advanced by the recovered completion.
    pub(in crate::sumeragi) const fn parent_ordinal(&self) -> u128 {
        self.parent_ordinal
    }
    /// Return the signed Broadcast ordinal created by the recovered completion.
    pub(in crate::sumeragi) const fn broadcast_ordinal(&self) -> u128 {
        self.broadcast_ordinal
    }
    /// Return the independently WAL-owned next Sign ordinal.
    pub(in crate::sumeragi) const fn next_sign_ordinal(&self) -> u128 {
        self.next_sign_ordinal
    }
    /// Reauthenticate this projection against one unchanged complete ledger frame.
    pub(in crate::sumeragi) fn exactly_matches_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {
        ledger
            .project_recovered_lifecycle_signed_broadcast_and_sign_at(self.broadcast_ordinal)
            .as_ref()
            == Some(self)
    }
}
/// Complete version-one durable lifecycle ledger.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(in crate::sumeragi) struct LifecycleLedgerV1 {
    format_version: u16,
    context: [u8; 32],
    height: u64,
    high_water: u128,
    records: Vec<LifecycleLedgerRecordV1>,
    producer_debts: Vec<LifecycleProducerDebtV1>,
}
/// Move-only CompleteTip predecessor join to the Kura-bound lifecycle store.
///
/// This cut owns the full Kura CompleteTip evidence, the exact opened LedgerV1
/// store handle, and the byte-equivalent frame. The store target is compared
/// with the lifecycle root retained when the same `Kura` instance authenticated
/// CompleteTip, so a copied frame at a caller-selected root cannot enter this
/// cut. It proves the terminal recovered-Decision body chain, the exact empty
/// height-one ledger owned by authenticated signed genesis, or an exact
/// physically present non-genesis frame superseded by canonical finality. It
/// does not publish the successor or claim that unrelated live
/// rows, Serve payloads, leases, waits, debts, or capacity have been retired.
/// The outer canonical predecessor-storage transaction supplies and discharges
/// those remaining durable owners before it can mint successor activation.
#[must_use = "the CompleteTip predecessor store join must enter retirement or be dropped"]
struct AuthenticatedCompleteTipTerminalApplyStoreJoinV1 {
    complete_tip: crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ledger_store: LifecycleLedgerStoreV1,
    ledger: LifecycleLedgerV1,
    predecessor_evidence: CompleteTipPredecessorLifecycleEvidenceV1,
}
/// Closed durable lineage accepted for one CompleteTip predecessor.
///
/// Non-genesis empty retirement is intentionally distinct from the genesis
/// exception and retains the store-minted proof that the canonical frame was
/// physically present. No raw boolean or caller-built empty ledger can enter
/// this enum.
#[allow(variant_size_differences)]
enum CompleteTipPredecessorLifecycleEvidenceV1 {
    TerminalApply(u128),
    EmptyGenesis,
    CanonicalFrame(AuthenticatedPresentLifecycleFrameV1),
}
impl CompleteTipPredecessorLifecycleEvidenceV1 {
    fn exactly_matches(
        &self,
        store: &LifecycleLedgerStoreV1,
        ledger: &LifecycleLedgerV1,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> bool {
        match self {
            Self::TerminalApply(apply_ordinal) => ledger
                .authenticate_complete_tip_terminal_apply(complete_tip)
                .is_ok_and(|ordinal| ordinal == *apply_ordinal),
            Self::EmptyGenesis => {
                ledger.high_water() == 0
                    && ledger.records().is_empty()
                    && ledger.producer_debts.is_empty()
                    && complete_tip.authorizes_empty_genesis_lifecycle(ledger.context())
            }
            Self::CanonicalFrame(present) => {
                present.authorizes_canonical_retired_predecessor(ledger, complete_tip)
                    && present.exactly_matches(store, ledger)
            }
        }
    }

    fn authorizes_staged_retirement(
        &self,
        store: &LifecycleLedgerStoreV1,
        current: &LifecycleLedgerV1,
        retired: &LifecycleLedgerV1,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> bool {
        if !self.exactly_matches(store, current, complete_tip)
            || current.context() != retired.context()
            || current.high_water() != retired.high_water()
            || current.records().len() != retired.records().len()
            || !retired.producer_debts.is_empty()
            || retired
                .records()
                .iter()
                .any(|record| record.terminal() == Some(None))
        {
            return false;
        }
        match self {
            Self::TerminalApply(apply_ordinal) => retired
                .authenticate_complete_tip_terminal_apply(complete_tip)
                .is_ok_and(|retired_ordinal| retired_ordinal == *apply_ordinal),
            Self::EmptyGenesis => current == retired,
            Self::CanonicalFrame(_) => true,
        }
    }
}
/// Opaque failure while authenticating all canonical CompleteTip predecessor stores.
#[derive(Debug, Error)]
#[error("failed to authenticate canonical CompleteTip predecessor lifecycle storage: {kind}")]
pub(in crate::sumeragi) struct CompleteTipPredecessorStorageErrorV1 {
    #[source]
    kind: CompleteTipPredecessorStorageErrorKindV1,
}
#[derive(Debug, Error)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum CompleteTipPredecessorStorageErrorKindV1 {
    #[error(transparent)]
    Ledger(#[from] LifecycleLedgerError),
    #[error(transparent)]
    PayloadStore(#[from] CertifiedServePayloadStoreError),
    #[error(transparent)]
    PayloadRecovery(#[from] CertifiedServePayloadRecoveryError),
    #[error(transparent)]
    LifecycleOpen(#[from] LifecycleOpenError),
    #[error(transparent)]
    TerminalPersistence(#[from] CertifiedServeTerminalPersistenceError),
}
macro_rules! complete_tip_predecessor_storage_error_from {
    ($source:ty, $variant:ident) => {
        impl From<$source> for CompleteTipPredecessorStorageErrorV1 {
            fn from(source: $source) -> Self {
                Self {
                    kind: CompleteTipPredecessorStorageErrorKindV1::$variant(source),
                }
            }
        }
    };
}
complete_tip_predecessor_storage_error_from!(LifecycleLedgerError, Ledger);
complete_tip_predecessor_storage_error_from!(CertifiedServePayloadStoreError, PayloadStore);
complete_tip_predecessor_storage_error_from!(CertifiedServePayloadRecoveryError, PayloadRecovery);
complete_tip_predecessor_storage_error_from!(LifecycleOpenError, LifecycleOpen);
complete_tip_predecessor_storage_error_from!(
    CertifiedServeTerminalPersistenceError,
    TerminalPersistence
);
/// Complete authenticated disk owners needed by CompleteTip retirement.
///
/// The cut retains the exact terminal or authenticated empty-retired
/// predecessor ledger, the co-located Serve payload-store instance, and the
/// complete retirement-authenticated Serve census. Completed response
/// signatures remain bound to their manifest body hashes without reopening
/// body bytes which normal finality may already have deleted. It exposes no
/// path, raw frame, request, or activation parts and performs no retirement
/// publication by itself.
#[must_use = "the authenticated CompleteTip predecessor stores must enter retirement"]
pub(in crate::sumeragi) struct AuthenticatedCompleteTipPredecessorStorageV1 {
    terminal: AuthenticatedCompleteTipTerminalApplyStoreJoinV1,
    successor: CanonicalCompleteTipSuccessorLedgerTargetV1,
    payload_store: CertifiedServePayloadStoreV1,
    serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    retained_serve_payloads:
        BTreeSet<crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadId>,
}
/// Purely staged proof that one exact recovered timeout Broadcast was retired.
///
/// The proof is minted only by the roster- and WAL-authenticated supersession
/// classifier. It carries no publication authority by itself; the control-Sign
/// owner-open transaction must consume it after its exact CAS has reloaded the
/// complete successor frame.
#[must_use = "a staged timeout supersession must be published or discarded"]
struct StagedRecoveredTimeoutSupersessionSuccessorV1 {
    context: LifecycleContext,
    predecessor_frame_identity: LifecycleDigest,
    reconciled_frame_identity: LifecycleDigest,
}
impl StagedRecoveredTimeoutSupersessionSuccessorV1 {
    fn new(opened: &LifecycleLedgerV1, reconciled: &LifecycleLedgerV1) -> Option<Self> {
        let context = opened.context();
        let predecessor_frame_identity = opened.frame_identity();
        let reconciled_frame_identity = reconciled.frame_identity();
        (context == reconciled.context() && predecessor_frame_identity != reconciled_frame_identity)
            .then_some(Self {
                context,
                predecessor_frame_identity,
                reconciled_frame_identity,
            })
    }

    /// Check the complete successor which the specialized store CAS may publish.
    fn exactly_matches_successor(
        &self,
        store: &LifecycleLedgerStoreV1,
        opened: &LifecycleLedgerV1,
        reconciled: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
        projection: &AuthenticatedRecoveredWalControlProjection,
        control_ordinal: u128,
    ) -> bool {
        let Ok((expected, expected_ordinal, _staged_control)) =
            reconciled.stage_authenticated_wal_control_sign(projection)
        else {
            return false;
        };
        opened.context() == self.context
            && reconciled.context() == self.context
            && successor.context() == self.context
            && opened.frame_identity() == self.predecessor_frame_identity
            && reconciled.frame_identity() == self.reconciled_frame_identity
            && expected == *successor
            && expected_ordinal == control_ordinal
            && projection.exactly_matches_ledger_at(successor, control_ordinal)
            && store.context == self.context
    }

    /// Mint only after the specialized store method completed CAS and reload.
    fn into_authenticated(
        self,
        store: &LifecycleLedgerStoreV1,
        successor: &LifecycleLedgerV1,
    ) -> AuthenticatedRecoveredTimeoutSupersessionSuccessorV1 {
        AuthenticatedRecoveredTimeoutSupersessionSuccessorV1 {
            store: store.clone(),
            context: self.context,
            predecessor_frame_identity: self.predecessor_frame_identity,
            successor_frame_identity: successor.frame_identity(),
        }
    }
}
/// Move-only proof of one exact timeout-supersession owner-open publication.
///
/// This is the sole exception to CompleteTip's frozen-nonempty-successor rule.
/// It remains sealed inside the production lifecycle owner until that owner is
/// joined to the exact retired predecessor which froze the old frame.
#[must_use = "a timeout-supersession successor proof must be consumed by owner binding"]
pub(super) struct AuthenticatedRecoveredTimeoutSupersessionSuccessorV1 {
    store: LifecycleLedgerStoreV1,
    context: LifecycleContext,
    predecessor_frame_identity: LifecycleDigest,
    successor_frame_identity: LifecycleDigest,
}
impl AuthenticatedRecoveredTimeoutSupersessionSuccessorV1 {
    fn authorizes_complete_tip_owner_join(
        &self,
        retirement_store: &LifecycleLedgerStoreV1,
        owner_store: &LifecycleLedgerStoreV1,
        frozen: &LifecycleLedgerV1,
        loaded: &LifecycleLedgerV1,
        coordinator: &LifecycleLedgerV1,
    ) -> bool {
        self.store.same_publication_target(retirement_store)
            && self.store.same_publication_target(owner_store)
            && self.context == retirement_store.context
            && self.context == owner_store.context
            && frozen.context() == self.context
            && loaded.context() == self.context
            && coordinator.context() == self.context
            && frozen.frame_identity() == self.predecessor_frame_identity
            && loaded.frame_identity() == self.successor_frame_identity
            && coordinator.frame_identity() == self.successor_frame_identity
            && retirement_store.load().ok().as_ref() == Some(loaded)
            && owner_store.load().ok().as_ref() == Some(loaded)
            && self.store.load().ok().as_ref() == Some(loaded)
    }

    /// Construct the already-authenticated half of the compositional bind test.
    #[cfg(test)]
    fn for_exact_store_successor_test(
        store: &LifecycleLedgerStoreV1,
        predecessor: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
    ) -> Self {
        assert_eq!(predecessor.context(), successor.context());
        assert_eq!(store.context, successor.context());
        assert_eq!(store.load().ok().as_ref(), Some(successor));
        Self {
            store: store.clone(),
            context: successor.context(),
            predecessor_frame_identity: predecessor.frame_identity(),
            successor_frame_identity: successor.frame_identity(),
        }
    }

    /// Corrupt only the sealed context for a fail-closed bind regression.
    #[cfg(test)]
    fn with_context_for_test(mut self, context: LifecycleContext) -> Self {
        self.context = context;
        self
    }
}
/// Complete durable proof that one canonical CompleteTip predecessor retired.
///
/// This token is minted only after the exact predecessor frame is fully
/// terminal and debt-free, its canonical successor target has been initialized
/// or authenticated as a later descendant, and both stores reload the frames
/// retained here. It deliberately exposes neither path nor the underlying
/// generic successor activation authority.
#[must_use = "retired CompleteTip authority must be retained until the successor lifecycle owner consumes it"]
pub(in crate::sumeragi) struct RetiredRecoveredCompleteTipActivationAuthorityV1 {
    complete_tip: crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    predecessor_frame_identity: LifecycleDigest,
    successor_frame_identity: LifecycleDigest,
    retained_high_water: u128,
    predecessor_store: LifecycleLedgerStoreV1,
    predecessor_ledger: LifecycleLedgerV1,
    successor_store: LifecycleLedgerStoreV1,
    successor_ledger: LifecycleLedgerV1,
}
impl std::fmt::Debug for RetiredRecoveredCompleteTipActivationAuthorityV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetiredRecoveredCompleteTipActivationAuthorityV1")
            .field("predecessor", &self.complete_tip.predecessor())
            .field("retained_high_water", &self.retained_high_water)
            .field(
                "predecessor_frame_identity",
                &self.predecessor_frame_identity,
            )
            .field("successor_frame_identity", &self.successor_frame_identity)
            .finish_non_exhaustive()
    }
}
impl RetiredRecoveredCompleteTipActivationAuthorityV1 {
    /// Return the exact durable predecessor named by this retirement.
    pub(in crate::sumeragi) const fn predecessor(
        &self,
    ) -> crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity {
        self.complete_tip.predecessor()
    }
    /// Return the frozen successor context authenticated by CompleteTip.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn successor_context_id(&self) -> wire::HeightContextId {
        self.complete_tip.successor_context_id()
    }
    /// Return the ordinal floor inherited by the canonical successor ledger.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn retained_high_water(&self) -> u128 {
        self.retained_high_water
    }
    fn frame_descends_from_retained_floor(&self, ledger: &LifecycleLedgerV1) -> bool {
        ledger.context() == self.successor_store.context
            && if ledger.records.is_empty() {
                ledger.producer_debts.is_empty() && ledger.high_water == self.retained_high_water
            } else {
                ledger.high_water >= self.retained_high_water
                    && ledger.records.iter().all(|record| {
                        record.ordinal() > self.retained_high_water
                            && record.owner().first_admission_ordinal() > self.retained_high_water
                    })
            }
    }
    fn successor_descends_from_retirement(&self) -> bool {
        self.successor_ledger.frame_identity() == self.successor_frame_identity
            && self.frame_descends_from_retained_floor(&self.successor_ledger)
    }
    fn predecessor_remains_exact(&self) -> bool {
        self.predecessor_ledger.frame_identity() == self.predecessor_frame_identity
            && self
                .predecessor_store
                .is_authorized_complete_tip_predecessor_target(&self.complete_tip)
            && self.predecessor_store.load().ok().as_ref() == Some(&self.predecessor_ledger)
    }
    /// Authenticate the sole startup publication allowed after retirement.
    ///
    /// A production owner may repair an exact recovered WAL row while opening
    /// the H+1 store.  Retirement necessarily precedes that open, so an
    /// initially empty successor can advance before the owner is joined.  The
    /// adoption is deliberately one-way and narrow: only the exact initialized
    /// empty frame can move, and every row in the replacement must begin above
    /// the predecessor's retained ordinal floor. A nonempty retirement-time
    /// frame remains frozen here; `bind_successor_owner` admits only the
    /// separate move-only proof of an exact timeout-supersession owner-open CAS.
    fn authorizes_owner_open_successor(&self, successor: &LifecycleLedgerV1) -> bool {
        if successor == &self.successor_ledger {
            return self.successor_descends_from_retirement();
        }
        self.successor_descends_from_retirement()
            && self.successor_ledger.records.is_empty()
            && self.successor_ledger.producer_debts.is_empty()
            && self.successor_ledger.high_water == self.retained_high_water
            && successor.context() == self.successor_store.context
            && !successor.records.is_empty()
            && successor.high_water >= self.retained_high_water
            && successor.records.iter().all(|record| {
                record.ordinal() > self.retained_high_water
                    && record.owner().first_admission_ordinal() > self.retained_high_water
            })
    }
    /// Reauthenticate the retained canonical H+1 ledger at its Kura-derived target.
    pub(in crate::sumeragi) fn authorizes_retained_successor(&self) -> bool {
        let Some(successor_root) = self.successor_store.path.parent() else {
            return false;
        };
        self.predecessor_remains_exact()
            && self.successor_descends_from_retirement()
            && self.complete_tip.authorizes_successor_lifecycle_target(
                successor_root,
                self.successor_ledger.context(),
            )
            && self.successor_store.load().ok().as_ref() == Some(&self.successor_ledger)
    }
    /// Reauthenticate the retained canonical H+1 ledger and one prepared status.
    ///
    /// This is a comparison oracle rather than an extraction surface: the
    /// retired CompleteTip authority, store handle, and ledger frame remain
    /// sealed together. The runner calls it once before opening ingress and
    /// the consuming status bridge calls it again immediately before
    /// publication. This rejects observed copied, replaced, or foreign-context
    /// state at the owner-private quiescent boundary; it does not claim to
    /// defeat an actively replacing same-UID process between filesystem reads.
    pub(in crate::sumeragi) fn authorizes_successor_status(
        &self,
        successor: &wire::SumeragiV2Status,
    ) -> bool {
        self.authorizes_retained_successor()
            && self.successor_ledger.context().height() == successor.height
            && self.complete_tip.successor_context_id() == successor.height_context_id
            && self.complete_tip.predecessor().height().checked_add(1) == Some(successor.height)
            && successor.last_committed_height == self.complete_tip.predecessor().height()
    }
    fn matches_successor_owner_ledger(
        &self,
        owner: &mut ProductionLifecycleOwnerV1,
        successor_ledger: &LifecycleLedgerV1,
    ) -> bool {
        let Some(successor_root) = self.successor_store.path.parent() else {
            return false;
        };
        let Some(owner_store) = owner.coordinator.ledger_store.as_ref() else {
            return false;
        };
        let Some(body_store) = owner.body_store.as_ref() else {
            return false;
        };
        let Some(adapter_startup) = owner.adapter_startup.as_ref() else {
            return false;
        };
        if owner.body_store_identity.is_some()
            || owner.coordinator.fault.is_some()
            || owner.coordinator.active_lease.is_some()
            || !self.predecessor_remains_exact()
            || !self
                .complete_tip
                .authorizes_successor_kura(owner.kura_binding.as_ref())
            || !self
                .complete_tip
                .authorizes_verified_successor(&owner.verified)
            || !self
                .complete_tip
                .authorizes_successor_lifecycle_target(successor_root, successor_ledger.context())
            || !self
                .complete_tip
                .authorizes_successor_body_store(body_store, &owner.verified)
            || !adapter_startup.authorizes_verified_context(&owner.verified)
            || !owner
                .payload_store
                .matches_lifecycle_storage_root(successor_root, owner.verified.context())
            || owner
                .payload_store
                .validate_authenticated_cut(&owner.serve_payloads)
                .is_err()
            || !super::open::authenticated_serve_payloads_match_ledger(
                successor_ledger,
                &owner.serve_payloads,
            )
            || !owner_store.same_publication_target(&self.successor_store)
            || self.successor_store.load().ok().as_ref() != Some(successor_ledger)
            || owner_store.load().ok().as_ref() != Some(successor_ledger)
            || LifecycleLedgerV1::from_coordinator(&owner.coordinator)
                .ok()
                .as_ref()
                != Some(successor_ledger)
        {
            return false;
        }
        let registry = owner.registry.registry_mut();
        registry.exactly_covers_recovered_ready_work(&owner.coordinator)
            || registry.exactly_covers_recovered_ready_work_and_wal_authority(&owner.coordinator)
    }
    fn exactly_matches_successor_owner(&self, owner: &mut ProductionLifecycleOwnerV1) -> bool {
        self.successor_descends_from_retirement()
            && self.matches_successor_owner_ledger(owner, &self.successor_ledger)
    }
    /// Consume retirement and the exact unlaunched H+1 owner into one seal.
    ///
    /// The join reopens the retained Kura-derived LedgerV1 target, compares the
    /// complete coordinator projection and concrete registry, and binds the
    /// co-located Serve-payload owner plus canonical body-store owner. Failure
    /// consumes both inputs and requires restart; no path or storage part is
    /// returned.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn bind_successor_owner(
        mut self,
        mut owner: ProductionLifecycleOwnerV1,
    ) -> Result<BoundRecoveredCompleteTipSuccessorOwnerV1, CompleteTipSuccessorOwnerBindErrorV1>
    {
        let Ok(successor_ledger) = self.successor_store.load() else {
            return Err(CompleteTipSuccessorOwnerBindErrorV1);
        };
        let retirement_frame_authorizes = self.authorizes_owner_open_successor(&successor_ledger);
        let timeout_supersession_authorizes = if retirement_frame_authorizes {
            false
        } else if !self.successor_descends_from_retirement()
            || !self.frame_descends_from_retained_floor(&successor_ledger)
            || !self.predecessor_remains_exact()
        {
            false
        } else {
            let Some(owner_store) = owner.coordinator.ledger_store.as_ref() else {
                return Err(CompleteTipSuccessorOwnerBindErrorV1);
            };
            let Ok(coordinator_ledger) = LifecycleLedgerV1::from_coordinator(&owner.coordinator)
            else {
                return Err(CompleteTipSuccessorOwnerBindErrorV1);
            };
            owner
                .timeout_supersession_successor
                .as_ref()
                .is_some_and(|successor| {
                    successor.authorizes_complete_tip_owner_join(
                        &self.successor_store,
                        owner_store,
                        &self.successor_ledger,
                        &successor_ledger,
                        &coordinator_ledger,
                    )
                })
        };
        if (retirement_frame_authorizes && owner.timeout_supersession_successor.is_some())
            || (!retirement_frame_authorizes && !timeout_supersession_authorizes)
            || !self.matches_successor_owner_ledger(&mut owner, &successor_ledger)
        {
            return Err(CompleteTipSuccessorOwnerBindErrorV1);
        }
        if timeout_supersession_authorizes {
            drop(
                owner
                    .timeout_supersession_successor
                    .take()
                    .expect("authenticated timeout supersession was observed above"),
            );
        }
        // Freeze the owner-authenticated publication, not the retirement-time
        // snapshot. The sole nonempty replacement path consumed the exact
        // owner-open timeout-supersession witness above. Every later
        // runner/status check is strict against this new frame and therefore
        // still rejects post-bind drift.
        self.successor_frame_identity = successor_ledger.frame_identity();
        self.successor_ledger = successor_ledger;
        if !self.exactly_matches_successor_owner(&mut owner) {
            return Err(CompleteTipSuccessorOwnerBindErrorV1);
        }
        Ok(BoundRecoveredCompleteTipSuccessorOwnerV1 {
            owner,
            retirement: self,
        })
    }
}
/// Fail-stop rejection of a CompleteTip/H+1 lifecycle-owner join.
#[derive(Debug, Error)]
#[error("retired CompleteTip authority does not match the exact canonical H+1 lifecycle owner")]
#[must_use = "a failed H+1 owner join requires process restart"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct CompleteTipSuccessorOwnerBindErrorV1;
/// Opaque exact join of retired CompleteTip authority and the unlaunched H+1 owner.
///
/// The sole launch method consumes this seal into the existing lifecycle
/// launch transaction and retains retirement in another opaque wrapper. The
/// lifecycle runner retains that wrapper through clock/ingress arming and
/// publishes status only through a dedicated typed CompleteTip activation tail.
/// No generic owner, adapter, store, frame, registry, or activation parts are
/// exposed here.
#[must_use = "the bound CompleteTip successor owner must remain sealed until launch"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct BoundRecoveredCompleteTipSuccessorOwnerV1 {
    owner: ProductionLifecycleOwnerV1,
    retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,
}
// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_BEGIN
impl BoundRecoveredCompleteTipSuccessorOwnerV1 {
    /// Consume the exact H+1 owner into the generic launch transaction while
    /// keeping its retired-H authority sealed for final runner activation.
    ///
    /// A failed launch consumes the complete join and requires restart. A
    /// successful launch cannot be detached from its retirement authority or
    /// used to publish the generic adapter status.
    #[allow(dead_code, clippy::result_large_err)]
    #[inline(never)]
    pub(in crate::sumeragi) fn launch(
        self,
        inputs: super::launch::ProductionLifecycleLaunchInputsV1,
    ) -> Result<
        Box<LaunchedRecoveredCompleteTipSuccessorLifecycleV1>,
        super::launch::ProductionLifecycleLaunchErrorV1,
    > {
        let Self { owner, retirement } = self;
        let launched = owner.launch(inputs)?;
        Ok(Box::new(LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {
            launched,
            retirement,
        }))
    }
}
/// Opaque running H+1 lifecycle stack joined to its retired-H authority.
///
/// Its sole activation method consumes both halves, arms live clocks, activates
/// the completion observer, and publishes H+1 only through the retained retired
/// CompleteTip authority. The resulting generic activated owner contains no
/// predecessor authority and cannot repeat that one-shot publication.
#[must_use = "the launched CompleteTip successor must remain sealed until final activation"]
#[allow(dead_code)]
pub(in crate::sumeragi) struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {
    launched: Box<super::launch::LaunchedProductionLifecycleV1>,
    retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,
}

impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {
    /// Borrow the sealed H+1 executor and services while ingress stays closed.
    ///
    /// The retired-H authority remains inside this wrapper for the whole
    /// transaction. The callback therefore cannot detach the generic launched
    /// owner or publish H+1 before the dedicated CompleteTip activation.
    #[allow(dead_code, clippy::type_complexity)]
    pub(in crate::sumeragi) fn with_runner_setup<R, E>(
        &mut self,
        runner: &mut super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        operation: impl FnOnce(
            &mut super::super::v2_effects::V2EffectExecutor<
                super::super::v2_runtime::SerializedV2Runtime,
            >,
            &mut super::super::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<super::launch::ProductionLifecyclePreActivationErrorV1>,
    {
        self.launched.with_runner_setup(runner, operation)
    }

    /// Temporarily recover canonical bodies without separating retired H from H+1.
    #[allow(dead_code, clippy::type_complexity, clippy::result_large_err)]
    pub(in crate::sumeragi) fn with_canonical_body_recovery_ingress<R, E>(
        &mut self,
        runner: &mut super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
        activation: &mut super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        operation: impl FnOnce(
            &super::super::v2_runner::ProductionLifecycleCanonicalRecoveryIngressV1<'_>,
            &mut super::super::v2_effects::V2EffectExecutor<
                super::super::v2_runtime::SerializedV2Runtime,
            >,
            &mut super::super::v2_worker::ProductionV2Services,
        ) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<super::launch::ProductionLifecyclePreActivationErrorV1>,
    {
        self.launched
            .with_complete_tip_canonical_body_recovery_ingress(runner, activation, operation)
    }

    /// Consume the sealed H/H+1 stack during orderly operator shutdown.
    ///
    /// The successor is never published and the retired predecessor evidence
    /// remains durable for cold restart. Both halves are consumed together so
    /// no generic launched owner can outlive the CompleteTip join.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_clean_shutdown(
        self,
        runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
    ) -> Result<(), super::launch::ProductionLifecycleShutdownErrorV1> {
        let Self {
            launched,
            retirement,
        } = self;
        launched.into_complete_tip_clean_shutdown(runner, retirement)
    }

    /// Bind recovered local-Proposal ownership before consuming the H/H+1 join.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn initialize_recovered_local_proposal(
        &mut self,
        runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    ) -> Result<
        (
            super::super::v2::LocalProposalDirective,
            super::launch::ProductionLifecyclePreparedLocalProposalStateV1,
        ),
        super::launch::ProductionLifecyclePreActivationErrorV1,
    > {
        self.launched.initialize_recovered_local_proposal(runner)
    }

    /// Consume the sealed H/H+1 join into one exact live-height activation.
    #[allow(dead_code, clippy::result_large_err)]
    pub(in crate::sumeragi) fn activate(
        self,
        now: std::time::Instant,
        runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1,
        local_proposal: super::launch::ProductionLifecyclePreparedLocalProposalStateV1,
    ) -> Result<
        super::launch::ActivatedProductionLifecycleV1,
        super::launch::ProductionLifecycleActivationErrorV1,
    > {
        let Self {
            launched,
            retirement,
        } = self;
        launched.activate_recovered_complete_tip(now, runner, retirement, local_proposal)
    }
}
// COMPLETE_TIP_BOUND_SUCCESSOR_LAUNCH_END
#[cfg(test)]
impl BoundRecoveredCompleteTipSuccessorOwnerV1 {
    fn remains_exact_for_test(&mut self) -> bool {
        self.retirement
            .exactly_matches_successor_owner(&mut self.owner)
    }
}
/// Private Kura-derived target for the empty CompleteTip successor ledger.
struct CanonicalCompleteTipSuccessorLedgerTargetV1 {
    root: PathBuf,
    context: LifecycleContext,
}
impl CanonicalCompleteTipSuccessorLedgerTargetV1 {
    /// Initialize the canonical successor or authenticate a later descendant.
    ///
    /// An untouched empty file advances to the predecessor's retained ordinal
    /// high-water. A later nonempty successor is accepted read-only only when
    /// every record and owner begins strictly above that floor; this proves the
    /// frame descends from this rollover instead of a zero-based independent
    /// lifecycle history.
    fn open_initialized_or_descendant(
        &self,
        retained_high_water: u128,
    ) -> Result<(LifecycleLedgerStoreV1, LifecycleLedgerV1), LifecycleLedgerError> {
        open_initialized_or_descendant_lifecycle_successor(
            &self.root,
            self.context,
            retained_high_water,
        )
    }
}

/// Initialize a canonical successor at the predecessor floor or authenticate
/// an already-live descendant whose complete ordinal lineage begins above it.
fn open_initialized_or_descendant_lifecycle_successor(
    root: &Path,
    context: LifecycleContext,
    retained_high_water: u128,
) -> Result<(LifecycleLedgerStoreV1, LifecycleLedgerV1), LifecycleLedgerError> {
    let (store, current) = LifecycleLedgerStoreV1::open(root, context)?;
    let successor = if current.records.is_empty() {
        if !current.producer_debts.is_empty()
            || (current.high_water != 0 && current.high_water != retained_high_water)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "empty lifecycle successor has a foreign ordinal high-water".to_owned(),
            ));
        }
        let initialized =
            LifecycleLedgerV1::new(context, retained_high_water, Vec::new(), BTreeMap::new())?;
        store.persist_exact_successor(&current, &initialized)?;
        initialized
    } else {
        if current.high_water < retained_high_water
            || current.records.iter().any(|record| {
                record.ordinal() <= retained_high_water
                    || record.owner().first_admission_ordinal() <= retained_high_water
            })
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "nonempty lifecycle successor does not descend above the retained ordinal floor"
                    .to_owned(),
            ));
        }
        current
    };
    if store.load()? != successor {
        return Err(LifecycleLedgerError::InvalidLedger(
            "lifecycle successor changed during canonical authentication".to_owned(),
        ));
    }
    Ok((store, successor))
}
impl AuthenticatedCompleteTipPredecessorStorageV1 {
    fn is_exact(&self) -> Result<bool, CompleteTipPredecessorStorageErrorV1> {
        if !self.terminal.is_exact()?
            || !self
                .terminal
                .complete_tip
                .authorizes_successor_lifecycle_target(&self.successor.root, self.successor.context)
        {
            return Ok(false);
        }
        self.payload_store
            .validate_authenticated_cut(&self.serve_payloads)?;
        let retained = super::open::authenticate_complete_tip_serve_census(
            &self.terminal.ledger,
            &self.serve_payloads,
        )?;
        Ok(retained == self.retained_serve_payloads)
    }
    /// Retire the canonical predecessor and authenticate its successor target.
    ///
    /// The write order is intentionally one-way and crash-idempotent: first
    /// prune/cancel the authenticated Serve payload cut, then publish the
    /// fully terminal predecessor LedgerV1 frame, and only then initialize or
    /// authenticate the canonical successor ledger. Any error consumes this
    /// capability and aborts startup; retry begins by reopening the durable
    /// state, where exact already-published predecessor and successor frames
    /// stutter without rewrite.
    pub(in crate::sumeragi) fn retire(
        self,
    ) -> Result<
        RetiredRecoveredCompleteTipActivationAuthorityV1,
        CompleteTipPredecessorStorageErrorV1,
    > {
        if !self.is_exact()? {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip predecessor stores changed before retirement".to_owned(),
            )
            .into());
        }
        let Self {
            terminal,
            successor,
            mut payload_store,
            serve_payloads,
            retained_serve_payloads,
        } = self;
        let refreshed_serve_payloads =
            payload_store.retire_authenticated_cut(serve_payloads, &retained_serve_payloads)?;
        let serve_reconciliation = super::open::reconcile_complete_tip_serve_retirement(
            &terminal.ledger,
            refreshed_serve_payloads,
        )?;
        let staged = terminal
            .ledger
            .stage_finalized_height_all_row_retirement(serve_reconciliation)?;
        let StagedFinalizationRetirementV1 {
            current: staged_current,
            retired,
        } = staged;
        debug_assert_eq!(staged_current, terminal.ledger);
        let retained_predecessor_is_exact =
            terminal.predecessor_evidence.authorizes_staged_retirement(
                &terminal.ledger_store,
                &terminal.ledger,
                &retired,
                &terminal.complete_tip,
            );
        if !retained_predecessor_is_exact {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip all-row retirement changed its predecessor authority".to_owned(),
            )
            .into());
        }
        terminal
            .ledger_store
            .persist_exact_successor(&terminal.ledger, &retired)?;
        if terminal.ledger_store.load()? != retired {
            return Err(LifecycleLedgerError::InvalidLedger(
                "retired CompleteTip predecessor changed after publication".to_owned(),
            )
            .into());
        }
        let (successor_store, successor_ledger) =
            successor.open_initialized_or_descendant(retired.high_water())?;
        let token = RetiredRecoveredCompleteTipActivationAuthorityV1 {
            complete_tip: terminal.complete_tip,
            predecessor_frame_identity: retired.frame_identity(),
            successor_frame_identity: successor_ledger.frame_identity(),
            retained_high_water: retired.high_water(),
            predecessor_store: terminal.ledger_store,
            predecessor_ledger: retired,
            successor_store,
            successor_ledger,
        };
        Ok(token)
    }
}
/// Consume CompleteTip while opening and authenticating every predecessor disk owner.
#[allow(clippy::too_many_arguments)]
pub(in crate::sumeragi) fn open_complete_tip_predecessor_storage(
    predecessor_root: &Path,
    successor_root: &Path,
    successor_context: LifecycleContext,
    body_store_root: &Path,
    verified_predecessor: VerifiedHeightContext,
    signature_policy: BlockSignaturePolicy,
    local_signer: &KeyPair,
    complete_tip: crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
) -> Result<AuthenticatedCompleteTipPredecessorStorageV1, CompleteTipPredecessorStorageErrorV1> {
    if !complete_tip.authorizes_predecessor_storage_inputs(
        predecessor_root,
        successor_root,
        successor_context,
        body_store_root,
        &verified_predecessor,
        &signature_policy,
    ) {
        return Err(LifecycleLedgerError::InvalidLedger(
            "CompleteTip predecessor storage inputs changed after Kura authentication".to_owned(),
        )
        .into());
    }
    let context = projection::lifecycle_context(verified_predecessor.context());
    let (ledger_store, opened_ledger) = LifecycleLedgerStoreV1::open(predecessor_root, context)?;
    let present_frame = ledger_store.authenticate_present_frame(&opened_ledger)?;
    let (ledger, repaired_live_apply, predecessor_evidence) =
        opened_ledger.stage_complete_tip_terminal_apply_recovery(&complete_tip, present_frame)?;
    if repaired_live_apply {
        ledger_store.persist_exact_successor(&opened_ledger, &ledger)?;
        if ledger_store.load()? != ledger {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered CompleteTip terminal Apply changed after publication".to_owned(),
            )
            .into());
        }
    }
    let terminal = ledger.into_complete_tip_terminal_apply_store_join(
        ledger_store,
        complete_tip,
        predecessor_evidence,
    )?;
    let (payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(predecessor_root, verified_predecessor.context())?;
    let serve_payloads =
        recovered.authenticate_for_complete_tip_retirement(&verified_predecessor, local_signer)?;
    let retained_serve_payloads =
        super::open::authenticate_complete_tip_serve_census(&terminal.ledger, &serve_payloads)?;
    let cut = AuthenticatedCompleteTipPredecessorStorageV1 {
        terminal,
        successor: CanonicalCompleteTipSuccessorLedgerTargetV1 {
            root: successor_root.to_path_buf(),
            context: successor_context,
        },
        payload_store,
        serve_payloads,
        retained_serve_payloads,
    };
    if !cut.is_exact()? {
        return Err(LifecycleLedgerError::InvalidLedger(
            "CompleteTip predecessor stores changed during authentication".to_owned(),
        )
        .into());
    }
    Ok(cut)
}
impl AuthenticatedCompleteTipTerminalApplyStoreJoinV1 {
    fn is_exact(&self) -> Result<bool, LifecycleLedgerError> {
        if self.ledger_store.context != self.ledger.context() {
            return Ok(false);
        }
        let opened = self.ledger_store.load()?;
        let predecessor_is_exact = self.predecessor_evidence.exactly_matches(
            &self.ledger_store,
            &self.ledger,
            &self.complete_tip,
        );
        Ok(opened == self.ledger && predecessor_is_exact)
    }
}
/// Move-only storage recovery cut for every ordinary durable body-pipeline row.
///
/// The exact opened LedgerV1 frame, height context, body-store instance, and
/// authenticated all-row census remain inseparable. There is deliberately no
/// parts, clone, candidate, work, or registry-install API: the unified startup
/// transaction consumes this value directly while opening the coordinator and
/// installing the concrete registry in one boundary.
#[must_use = "the exact storage recovery cut must enter the startup composite"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct AuthenticatedDurableCertifiedBodyPipelineStorageRecoveryCutV1
{
    verified: VerifiedHeightContext,
    ledger_store: LifecycleLedgerStoreV1,
    ledger: LifecycleLedgerV1,
    body_store: V2BodyStore,
    census: AuthenticatedRecoveredDurableCertifiedBodyPipelineCensusV1,
}
/// Startup-fatal failure from the sole V1 lifecycle storage-owner transaction.
///
/// Every input is consumed by the failed call. The diagnostic deliberately
/// exposes no retry token, storage part, recovered row, or ordinal surface.
#[derive(Debug, Error)]
#[error("{kind}")]
#[must_use = "failed production lifecycle startup requires process fail-stop"]
pub(crate) struct ProductionLifecycleStartupErrorV1 {
    kind: ProductionLifecycleStartupErrorKindV1,
}
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
enum ProductionLifecycleStartupErrorKindV1 {
    #[error("authenticated lifecycle storage instances changed before startup")]
    InvalidStorageCut,
    #[error("production lifecycle capacity authority is invalid")]
    InvalidAuthority,
    #[error("the lifecycle ledger changed after body-pipeline authentication")]
    LedgerFrameMismatch,
    #[error("lifecycle ledger open failed: {0}")]
    Ledger(#[source] LifecycleLedgerError),
    #[error("durable body-pipeline authentication failed: {0}")]
    BodyPipeline(#[source] DurableCertifiedBodyPipelineRecoveryError),
    #[error("Certified-Serve payload recovery changed before startup: {0}")]
    ServePayload(#[source] CertifiedServePayloadStoreError),
    #[error("the complete body-pipeline census could not enter its startup phase")]
    InvalidBodyPipelineCensus,
    #[error("lifecycle recovery assembly failed: {0}")]
    Recovery(#[source] LifecycleRecoveryAssemblyError),
    #[error("the recovered body-pipeline census cannot enter an empty registry")]
    RegistryInstall,
    #[error("lifecycle coordinator open failed: {0}")]
    CoordinatorOpen(#[source] LifecycleOpenError),
    #[error("the opened coordinator and exact concrete registry differ")]
    RegistryCoordinatorMismatch,
}
impl ProductionLifecycleStartupErrorV1 {
    fn new(kind: ProductionLifecycleStartupErrorKindV1) -> Self {
        Self { kind }
    }
}
/// Startup-fatal failure from exact recovered control-Sign storage repair.
///
/// The classification intentionally carries no projection, effect, pending
/// binding, ledger row, carrier, address, digest, or retry authority.
#[must_use = "failed recovered control startup requires process restart"]
pub(in crate::sumeragi) struct ProductionRecoveredWalControlStartupErrorV1 {
    reason: &'static str,
    assembly_detail: Option<String>,
}
impl ProductionRecoveredWalControlStartupErrorV1 {
    const fn new(reason: &'static str) -> Self {
        Self {
            reason,
            assembly_detail: None,
        }
    }
    /// Preserve the typed, non-authorizing storage-census discriminator.
    pub(super) fn from_assembly(error: LifecycleRecoveryAssemblyError) -> Self {
        let assembly_detail = error.kind().to_string();
        iroha_logger::error!(
            detail = %assembly_detail,
            "recovered control storage census assembly failed"
        );
        Self {
            reason: "recovered control storage census assembly failed",
            assembly_detail: Some(assembly_detail),
        }
    }
    /// Return the stable non-authorizing failure classification.
    pub(in crate::sumeragi) const fn reason(&self) -> &'static str {
        self.reason
    }

    /// Borrow the typed assembler discriminator without exposing authority.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn assembly_detail(&self) -> Option<&str> {
        self.assembly_detail.as_deref()
    }
}
/// Startup-fatal failure from exact recovered Decision-Fetch storage repair.
///
/// Diagnostics expose no projection, effect, pending binding, row, carrier,
/// address, digest, or retry authority.
#[must_use = "failed recovered Decision Fetch startup requires process restart"]
pub(in crate::sumeragi) struct ProductionRecoveredWalDecisionFetchStartupErrorV1 {
    reason: &'static str,
}
impl ProductionRecoveredWalDecisionFetchStartupErrorV1 {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }
    /// Return the stable non-authorizing failure classification.
    pub(in crate::sumeragi) const fn reason(&self) -> &'static str {
        self.reason
    }
}
/// Startup-fatal failure from exact recovered Decision-to-Apply publication.
///
/// The diagnostic exposes no adapter, WAL, body, ledger, registry, effect,
/// pending-binding, candidate, or retry authority. Every failure requires a
/// fresh process restart from the still-canonical durable stores.
#[must_use = "failed recovered Decision Apply startup requires process restart"]
pub(in crate::sumeragi) struct ProductionRecoveredDecisionApplyStartupErrorV1 {
    reason: &'static str,
}
impl ProductionRecoveredDecisionApplyStartupErrorV1 {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }
    /// Return the stable non-authorizing failure classification.
    pub(in crate::sumeragi) const fn reason(&self) -> &'static str {
        self.reason
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl AuthenticatedDurableCertifiedBodyPipelineStorageRecoveryCutV1 {
    fn is_exact(&self) -> bool {
        self.ledger.context() == projection::lifecycle_context(self.verified.context())
            && self.ledger_store.context == self.ledger.context()
            && self
                .ledger_store
                .load()
                .is_ok_and(|opened| opened == self.ledger)
            && self.body_store.matches_context(self.verified.context())
            && self.census.exactly_matches_opened_ledger(&self.ledger)
    }
    /// Consume all durable storage authority into the sole production owner.
    ///
    /// Every context, frame, payload-store, census, and empty-registry check
    /// precedes terminal-outcome consumption. The logical recovery cut then
    /// consumes every ordinary body-pipeline candidate; its concrete peers enter the fresh
    /// registry before coordinator preparation. The exact registry/coordinator
    /// join is checked before either durable open publication occurs.
    // The lifecycle factory reaches this sole constructor only after its
    // authenticated storage, registry, and replay joins succeed.
    #[allow(dead_code, clippy::result_large_err, clippy::too_many_arguments)]
    pub(super) fn open_production_owner(
        self,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleStartupErrorV1> {
        let authority =
            authority::production_authority(&self.verified, config, reply_route_source_capacity)
                .ok_or_else(|| {
                    ProductionLifecycleStartupErrorV1::new(
                        ProductionLifecycleStartupErrorKindV1::InvalidAuthority,
                    )
                })?;
        self.open_owner_with_authority(authority, payload_store, serve_payloads, adapter_startup)
    }
    #[allow(clippy::result_large_err)]
    fn open_owner_with_authority(
        self,
        authority: super::authority::AuthenticatedEpisodeAuthority,
        mut payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleStartupErrorV1> {
        if !self.is_exact() {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::InvalidStorageCut,
            ));
        }
        let Self {
            verified,
            ledger_store,
            ledger,
            mut body_store,
            census,
        } = self;
        payload_store
            .validate_authenticated_cut(&serve_payloads)
            .map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::ServePayload(error),
                )
            })?;
        let reopened = ledger_store.load().map_err(|error| {
            ProductionLifecycleStartupErrorV1::new(ProductionLifecycleStartupErrorKindV1::Ledger(
                error,
            ))
        })?;
        if reopened != ledger {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::LedgerFrameMismatch,
            ));
        }
        let body_pipeline = census.into_startup(&ledger).ok_or_else(|| {
            ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::InvalidBodyPipelineCensus,
            )
        })?;
        let (body_pipeline, adapter_startup) = body_pipeline
            .replay_adapter_startup(adapter_startup)
            .map_err(|_| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::InvalidBodyPipelineCensus,
                )
            })?;
        let mut registry = LifecycleWorkRegistryHolder::empty();
        if !body_pipeline.preflights_empty_registry(registry.registry_mut()) {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::RegistryInstall,
            ));
        }
        let (mut recovery, body_pipeline) =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_body_pipeline_startup(
                ledger,
                serve_payloads,
                &mut body_store,
                body_pipeline,
            )
            .map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::Recovery(error),
                )
            })?;
        body_pipeline
            .install_into_empty_registry(registry.registry_mut())
            .map_err(|_| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::RegistryInstall,
                )
            })?;
        let prepared = LifecycleCoordinator::prepare_with_authenticated_store_borrowed(
            authority,
            ledger_store,
            &payload_store,
            &recovery,
        )
        .map_err(|error| {
            ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::CoordinatorOpen(error),
            )
        })?;
        let coordinator = prepared
            .commit_with_registry(registry.registry_mut(), &mut payload_store, &mut recovery)
            .map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::CoordinatorOpen(error.into_error()),
                )
            })?;
        if !registry
            .registry_mut()
            .exactly_covers_recovered_ready_work(&coordinator)
        {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::RegistryCoordinatorMismatch,
            ));
        }
        Ok(ProductionLifecycleOwnerV1 {
            verified,
            coordinator,
            registry,
            payload_store,
            serve_payloads: recovery.into_serve_payloads(),
            body_store: Some(body_store),
            body_store_identity: None,
            kura_binding: None,
            apply_service: None,
            adapter_startup: Some(adapter_startup),
            timeout_supersession_successor: None,
        })
    }
    #[cfg(test)]
    fn open_owner_for_test(
        self,
        payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleStartupErrorV1> {
        let effect_capacity = self
            .ledger
            .records()
            .iter()
            .filter(|record| {
                record.terminal() == Some(None)
                    && matches!(
                        record.work_class(),
                        Some(
                            LifecycleWorkClass::Fetch
                                | LifecycleWorkClass::Store
                                | LifecycleWorkClass::Validate
                        )
                    )
            })
            .count();
        let serve_capacity = self
            .ledger
            .records()
            .iter()
            .filter(|record| {
                record.terminal() == Some(None)
                    && record.work_class() == Some(LifecycleWorkClass::CertifiedServe)
            })
            .count();
        let authority = authority::lifecycle_storage_owner_test_authority(
            &self.verified,
            effect_capacity,
            serve_capacity,
        )
        .ok_or_else(|| {
            ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::InvalidAuthority,
            )
        })?;
        self.open_owner_with_authority(
            authority,
            payload_store,
            serve_payloads,
            ProductionLifecycleAdapterStartupV1::fixture_for_test(),
        )
    }
    #[cfg(test)]
    fn corrupt_fetch_census_for_test(&mut self) {
        self.census.corrupt_first_completion_for_test();
    }
}
impl ProductionLifecycleOwnerV1 {
    /// Prove the exact recovered Decision four-row chain and sole live Apply.
    #[cfg(test)]
    pub(in crate::sumeragi) fn recovered_decision_apply_summary_for_test(
        &mut self,
    ) -> Option<(usize, u128)> {
        if !self
            .adapter_startup
            .as_ref()
            .is_some_and(ProductionLifecycleAdapterStartupV1::is_exact_for_test)
            || !self
                .registry
                .registry_mut()
                .exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator)
        {
            return None;
        }
        let ledger = LifecycleLedgerV1::from_coordinator(&self.coordinator).ok()?;
        let apply = ledger.records().iter().find(|record| {
            record.work_class() == Some(LifecycleWorkClass::Apply)
                && record.terminal() == Some(None)
        })?;
        let owner = apply.owner();
        let owner_records = ledger
            .records()
            .iter()
            .filter(|record| record.owner() == owner)
            .count();
        (owner_records == 4).then_some((owner_records, apply.ordinal()))
    }
    /// Open the exact no-WAL-vote storage branch without exposing ledger or
    /// recovery parts to the adapter startup caller.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_storage_only_recovered_startup(
        verified: VerifiedHeightContext,
        ledger_root: &Path,
        body_store: V2BodyStore,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<Self, ProductionLifecycleStartupErrorV1> {
        let context = projection::lifecycle_context(verified.context());
        let (ledger_store, ledger) =
            LifecycleLedgerStoreV1::open(ledger_root, context).map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::Ledger(error),
                )
            })?;
        let storage = ledger
            .into_durable_certified_body_pipeline_storage_recovery_cut(
                verified,
                ledger_store,
                body_store,
            )
            .map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::BodyPipeline(error),
                )
            })?;
        storage.open_production_owner(
            config,
            reply_route_source_capacity,
            payload_store,
            serve_payloads,
            adapter_startup,
        )
    }
    /// Repair/coalesce and open one exact Proposal/Timeout control Sign.
    ///
    /// Projection exactness is checked before this method opens LedgerV1.
    /// Absence publishes only the deterministic checked successor; an exact
    /// existing row normally remains byte-for-byte untouched while its volatile
    /// carrier is reconstructed. The sole exception atomically terminalizes one
    /// roster-authenticated, strict-lower-view timeout Broadcast before the
    /// current Sign is staged. Every other live row remains unchanged and owned
    /// by the closed storage census; when no exact supersession or Sign staging
    /// is eligible, its rejection performs no lifecycle publication.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_recovered_control_startup(
        verified: VerifiedHeightContext,
        projection: AuthenticatedRecoveredWalControlProjection,
        ledger_root: &Path,
        body_store: V2BodyStore,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<Self, ProductionRecoveredWalControlStartupErrorV1> {
        if !projection.is_exact(&verified) {
            return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                "recovered control projection is not exact",
            ));
        }
        let context = projection::lifecycle_context(verified.context());
        let (ledger_store, opened) =
            LifecycleLedgerStoreV1::open(ledger_root, context).map_err(|_error| {
                ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control LedgerV1 open failed",
                )
            })?;
        #[inline(never)]
        #[allow(
            clippy::items_after_statements,
            clippy::result_large_err,
            clippy::too_many_arguments,
            clippy::too_many_lines
        )]
        fn open_recovered_control_signed_startup(
            verified: VerifiedHeightContext,
            projection: AuthenticatedRecoveredWalControlProjection,
            ledger_store: LifecycleLedgerStoreV1,
            opened: LifecycleLedgerV1,
            broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
            parent_ordinal: u128,
            child_ordinal: u128,
            mut body_store: V2BodyStore,
            config: &SumeragiV2Config,
            reply_route_source_capacity: usize,
            mut payload_store: CertifiedServePayloadStoreV1,
            serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
            adapter_startup: ProductionLifecycleAdapterStartupV1,
        ) -> Result<ProductionLifecycleOwnerV1, ProductionRecoveredWalControlStartupErrorV1>
        {
            let mut matching_pairs = opened
                .recovered_lifecycle_signed_broadcast_and_sign_pairs()
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control Broadcast-and-Sign pair classification failed",
                    )
                })?
                .into_iter()
                .filter(|pair| {
                    pair.parent()
                        == RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
                        && pair.parent_ordinal() == parent_ordinal
                        && pair.broadcast_ordinal() == child_ordinal
                });
            let pair_hint = matching_pairs.next();
            if matching_pairs.next().is_some() {
                return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control Broadcast matched multiple durable successor pairs",
                ));
            }
            if let Some(pair_hint) = pair_hint {
                let mut cold_preview = projection
                    .prepare_cold_signed_broadcast_and_sign(&verified, adapter_startup, &broadcast)
                    .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
                let body = body_store
                    .authenticate_recovered_lifecycle_next_vote_body(&mut cold_preview)
                    .map_err(|_error| {
                        ProductionRecoveredWalControlStartupErrorV1::new(
                            "recovered control next Vote lost its exact body-store authority",
                        )
                    })?;
                let seal = cold_preview
                    .seal_recovered_lifecycle_next_wal_vote(body)
                    .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
                let (cold_startup, mut combined) = projection
                    .project_authenticated_cold_signed_broadcast_and_sign(&verified, seal)
                    .ok_or_else(|| {
                        ProductionRecoveredWalControlStartupErrorV1::new(
                            "recovered control cold pair changed its WAL/body authority",
                        )
                    })?;
                let pair = opened
                    .authenticate_recovered_control_signed_broadcast_and_sign(
                        &verified,
                        &projection,
                        &combined,
                    )
                    .map_err(|_error| {
                        ProductionRecoveredWalControlStartupErrorV1::new(
                            "recovered control cold pair changed its exact durable rows",
                        )
                    })?;
                if pair != pair_hint {
                    return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control cold pair changed after executable projection",
                    ));
                }
                let adapter_authority = combined
                    .project_cold_adapter_replay_authority(&verified)
                    .ok_or_else(|| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control cold pair cannot advance the exact adapter",
                    )
                })?;
                let adapter_startup = cold_startup
                    .advance_recovered_lifecycle_signed_broadcast_and_sign(
                        &verified,
                        adapter_authority,
                    )
                    .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
                if !ledger_store.revalidates_recovered_control_signed_broadcast_and_sign(
                    &verified,
                    &projection,
                    &combined,
                    &pair,
                ) {
                    return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control cold pair changed after adapter advance",
                    ));
                }
                let body_pipeline = opened
                    .authenticate_durable_certified_body_pipeline_startup(&verified, &body_store)
                    .map_err(|_error| {
                        ProductionRecoveredWalControlStartupErrorV1::new(
                            "recovered control cold pair body-pipeline census authentication failed",
                        )
                    })?;
                let (body_pipeline, adapter_startup) = body_pipeline
                    .replay_adapter_startup(adapter_startup)
                    .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
                let (recovery, body_pipeline) = AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_control_broadcast_and_sign_and_body_pipeline_startup(
                    opened.clone(),
                    serve_payloads,
                    &mut body_store,
                    &projection,
                    &pair,
                    &combined,
                    body_pipeline,
                )
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control cold pair storage census assembly failed",
                    )
                })?;
                let mut registry = LifecycleWorkRegistryHolder::empty();
                let mut installed = registry
                    .registry_mut()
                    .install_recovered_control_signed_broadcast_and_sign(
                        &verified,
                        &ledger_store,
                        &opened,
                        projection,
                        combined,
                        pair,
                    )
                    .map_err(|error| {
                        ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                    })?;
                installed
                    .install_body_pipeline(body_pipeline)
                    .map_err(|error| {
                        ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                    })?;
                let authority = authority::production_authority(
                    &verified,
                    config,
                    reply_route_source_capacity,
                )
                .ok_or_else(|| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "verified height cannot derive recovered control cold-pair authority",
                    )
                })?;
                let (coordinator, recovery) = installed
                    .open_with_exact_store_authority(
                        authority,
                        ledger_store,
                        &mut payload_store,
                        recovery,
                    )
                    .map_err(|error| {
                        ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                    })?;
                return Ok(ProductionLifecycleOwnerV1 {
                    verified,
                    coordinator,
                    registry,
                    payload_store,
                    serve_payloads: recovery.into_serve_payloads(),
                    body_store: Some(body_store),
                    body_store_identity: None,
                    kura_binding: None,
                    apply_service: None,
                    adapter_startup: Some(adapter_startup),
                    timeout_supersession_successor: None,
                });
            }
            let adapter_authority = projection
                .project_cold_adapter_authority(&verified, &broadcast)
                .ok_or_else(|| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control Broadcast cannot replay the exact cold adapter",
                    )
                })?;
            let adapter_startup = adapter_startup
                .advance_recovered_lifecycle_signed_broadcast(&verified, adapter_authority)
                .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
            if !ledger_store.revalidates_recovered_control_signed_broadcast(
                &verified,
                &projection,
                &broadcast,
                parent_ordinal,
                child_ordinal,
            ) {
                return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control Broadcast changed after cold adapter replay",
                ));
            }
            let body_pipeline = opened
                .authenticate_durable_certified_body_pipeline_startup(&verified, &body_store)
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control Broadcast body-pipeline census authentication failed",
                    )
                })?;
            let (body_pipeline, adapter_startup) = body_pipeline
                .replay_adapter_startup(adapter_startup)
                .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
            let (recovery, body_pipeline) = AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_control_broadcast_and_body_pipeline_startup(
                opened.clone(),
                serve_payloads,
                &mut body_store,
                &projection,
                &broadcast,
                body_pipeline,
            )
            .map_err(|_error| {
                ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control Broadcast storage census assembly failed",
                )
            })?;
            let mut registry = LifecycleWorkRegistryHolder::empty();
            let mut installed = registry
                .registry_mut()
                .install_recovered_control_signed_broadcast(
                    &verified,
                    &ledger_store,
                    &opened,
                    projection,
                    broadcast,
                    parent_ordinal,
                    child_ordinal,
                )
                .map_err(|error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                })?;
            installed
                .install_body_pipeline(body_pipeline)
                .map_err(|error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                })?;
            let authority =
                authority::production_authority(&verified, config, reply_route_source_capacity)
                    .ok_or_else(|| {
                        ProductionRecoveredWalControlStartupErrorV1::new(
                            "verified height cannot derive recovered control Broadcast authority",
                        )
                    })?;
            let (coordinator, recovery) = installed
                .open_with_exact_store_authority(
                    authority,
                    ledger_store,
                    &mut payload_store,
                    recovery,
                )
                .map_err(|error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                })?;
            Ok(ProductionLifecycleOwnerV1 {
                verified,
                coordinator,
                registry,
                payload_store,
                serve_payloads: recovery.into_serve_payloads(),
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(adapter_startup),
                timeout_supersession_successor: None,
            })
        }
        if let Ok((broadcast, parent_ordinal, child_ordinal)) =
            opened.authenticate_recovered_control_signed_broadcast(&verified, &projection)
        {
            return open_recovered_control_signed_startup(
                verified,
                projection,
                ledger_store,
                opened,
                broadcast,
                parent_ordinal,
                child_ordinal,
                body_store,
                config,
                reply_route_source_capacity,
                payload_store,
                serve_payloads,
                adapter_startup,
            );
        }
        #[inline(never)]
        #[allow(
            clippy::items_after_statements,
            clippy::result_large_err,
            clippy::too_many_arguments
        )]
        fn open_recovered_control_sign_startup(
            verified: VerifiedHeightContext,
            projection: AuthenticatedRecoveredWalControlProjection,
            ledger_store: LifecycleLedgerStoreV1,
            opened: LifecycleLedgerV1,
            mut body_store: V2BodyStore,
            config: &SumeragiV2Config,
            reply_route_source_capacity: usize,
            mut payload_store: CertifiedServePayloadStoreV1,
            serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
            adapter_startup: ProductionLifecycleAdapterStartupV1,
        ) -> Result<ProductionLifecycleOwnerV1, ProductionRecoveredWalControlStartupErrorV1>
        {
            let (reconciled, staged_timeout_supersession) = opened
                .reconcile_superseded_timeout_broadcast(&verified, &projection)
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control timeout supersession invariant failed",
                    )
                })?;
            let (repaired, ordinal, staged) = reconciled
                .stage_authenticated_wal_control_sign(&projection)
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control durable row is absent-or-exact invariant failed",
                    )
                })?;
            let timeout_supersession_successor = if let Some(staged_supersession) =
                staged_timeout_supersession
            {
                Some(
                        ledger_store
                            .persist_recovered_timeout_supersession_successor(
                                staged_supersession,
                                &opened,
                                &reconciled,
                                &repaired,
                                &projection,
                                ordinal,
                            )
                            .map_err(|_error| {
                                ProductionRecoveredWalControlStartupErrorV1::new(
                                    "recovered control timeout supersession successor publication failed",
                                )
                            })?,
                    )
            } else {
                if staged {
                    ledger_store
                        .persist_exact_successor(&opened, &repaired)
                        .map_err(|_error| {
                            ProductionRecoveredWalControlStartupErrorV1::new(
                                "recovered control LedgerV1 successor publication failed",
                            )
                        })?;
                }
                None
            };
            let reopened = ledger_store.load().map_err(|_error| {
                ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control LedgerV1 reopen changed the exact row",
                )
            })?;
            if reopened != repaired || !projection.exactly_matches_ledger_at(&repaired, ordinal) {
                return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control LedgerV1 reopen changed the exact row",
                ));
            }
            let body_pipeline = repaired
                .authenticate_durable_certified_body_pipeline_startup(&verified, &body_store)
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control body-pipeline census authentication failed",
                    )
                })?;
            let (body_pipeline, adapter_startup) = body_pipeline
                .replay_adapter_startup(adapter_startup)
                .map_err(ProductionRecoveredWalControlStartupErrorV1::new)?;
            let (recovery, body_pipeline) =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_control_sign_and_body_pipeline_startup(
                repaired.clone(),
                serve_payloads,
                &mut body_store,
                &projection,
                body_pipeline,
            )
            .map_err(ProductionRecoveredWalControlStartupErrorV1::from_assembly)?;
            let mut registry = LifecycleWorkRegistryHolder::empty();
            let mut installed = registry
                .registry_mut()
                .install_recovered_wal_control_sign(&verified, &ledger_store, &repaired, projection)
                .map_err(|error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                })?;
            installed
                .install_body_pipeline(body_pipeline)
                .map_err(|error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                })?;
            let authority =
                authority::production_authority(&verified, config, reply_route_source_capacity)
                    .ok_or_else(|| {
                        ProductionRecoveredWalControlStartupErrorV1::new(
                            "verified height cannot derive recovered control lifecycle authority",
                        )
                    })?;
            let (coordinator, recovery) = installed
                .open_with_exact_store_authority(
                    authority,
                    ledger_store,
                    &mut payload_store,
                    recovery,
                )
                .map_err(|error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(error.reason())
                })?;
            Ok(ProductionLifecycleOwnerV1 {
                verified,
                coordinator,
                registry,
                payload_store,
                serve_payloads: recovery.into_serve_payloads(),
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(adapter_startup),
                timeout_supersession_successor,
            })
        }
        open_recovered_control_sign_startup(
            verified,
            projection,
            ledger_store,
            opened,
            body_store,
            config,
            reply_route_source_capacity,
            payload_store,
            serve_payloads,
            adapter_startup,
        )
    }
    /// Repair/coalesce and open one exact Decision-owned certified Fetch.
    ///
    /// Projection and semantically revalidated-body checks precede LedgerV1
    /// mutation. If the exact validated marker already exists, startup fails
    /// without installing a duplicate Fetch; the future sealed Apply composite
    /// must consume and replace both authorities atomically.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_recovered_decision_fetch_startup(
        verified: VerifiedHeightContext,
        projection: AuthenticatedRecoveredWalDecisionFetchProjection,
        ledger_root: &Path,
        mut body_store: V2BodyStore,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        mut payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<Self, ProductionRecoveredWalDecisionFetchStartupErrorV1> {
        if !projection.is_exact(&verified) {
            return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "recovered Decision Fetch projection is not exact",
            ));
        }
        if body_store.has_exact_recovered_decision_fetch_parent(&projection) {
            return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "recovered Decision Fetch has an exact validated marker requiring atomic Apply recovery",
            ));
        }
        if body_store.has_quarantined_recovered_decision_fetch_parent(&projection) {
            return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "recovered Decision Fetch has a matching quarantined marker requiring semantic replay before atomic Apply recovery",
            ));
        }
        if body_store.has_rejected_recovered_decision_body(&projection) {
            return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "recovered Commit Decision conflicts with a deterministic local body rejection",
            ));
        }
        let context = projection::lifecycle_context(verified.context());
        let (ledger_store, opened) =
            LifecycleLedgerStoreV1::open(ledger_root, context).map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Fetch LedgerV1 open failed",
                )
            })?;
        let (repaired, ordinal, changed) = match opened
            .stage_authenticated_wal_decision_fetch(&projection)
        {
            Ok(staged) => staged,
            Err(_) if opened.has_exact_recovered_decision_fetch_store_parent(&projection) => {
                return Self::open_recovered_decision_store_startup(
                    verified,
                    projection,
                    ledger_store,
                    opened,
                    body_store,
                    config,
                    reply_route_source_capacity,
                    payload_store,
                    serve_payloads,
                    adapter_startup,
                );
            }
            Err(_) => {
                return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision LedgerV1 is neither an exact live Fetch nor an exact advanced Store parent",
                ));
            }
        };
        if changed {
            ledger_store
                .persist_exact_successor(&opened, &repaired)
                .map_err(|_error| {
                    ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                        "recovered Decision Fetch LedgerV1 successor publication failed",
                    )
                })?;
        }
        if !ledger_store.load().is_ok_and(|loaded| loaded == repaired)
            || !projection.exactly_matches_ledger_at(&repaired, ordinal)
        {
            return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "recovered Decision Fetch LedgerV1 reopen changed the exact row",
            ));
        }
        let body_pipeline = repaired
            .authenticate_durable_certified_body_pipeline_startup(&verified, &body_store)
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Fetch body-pipeline census authentication failed",
                )
            })?;
        let (body_pipeline, adapter_startup) = body_pipeline
            .replay_adapter_startup(adapter_startup)
            .map_err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new)?;
        let (recovery, body_pipeline) =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_decision_fetch_and_body_pipeline_startup(
                repaired.clone(),
                serve_payloads,
                &mut body_store,
                &projection,
                body_pipeline,
            )
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Fetch storage census assembly failed",
                )
            })?;
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let mut installed = registry
            .registry_mut()
            .install_recovered_wal_decision_fetch(&verified, &ledger_store, &repaired, projection)
            .map_err(|error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(error.reason())
            })?;
        installed
            .install_body_pipeline(body_pipeline)
            .map_err(|error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(error.reason())
            })?;
        let authority = authority::production_authority(
            &verified,
            config,
            reply_route_source_capacity,
        )
        .ok_or_else(|| {
            ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "verified height cannot derive recovered Decision Fetch lifecycle authority",
            )
        })?;
        let (coordinator, recovery) = installed
            .open_with_exact_store_authority(authority, ledger_store, &mut payload_store, recovery)
            .map_err(|error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(error.reason())
            })?;
        Ok(Self {
            verified,
            coordinator,
            registry,
            payload_store,
            serve_payloads: recovery.into_serve_payloads(),
            body_store: Some(body_store),
            body_store_identity: None,
            kura_binding: None,
            apply_service: None,
            adapter_startup: Some(adapter_startup),
            timeout_supersession_successor: None,
        })
    }
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    fn open_recovered_decision_store_startup(
        verified: VerifiedHeightContext,
        projection: AuthenticatedRecoveredWalDecisionFetchProjection,
        ledger_store: LifecycleLedgerStoreV1,
        opened: LifecycleLedgerV1,
        mut body_store: V2BodyStore,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        mut payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<Self, ProductionRecoveredWalDecisionFetchStartupErrorV1> {
        let body = body_store
            .recovered_decision_fetch_store_body(&projection)
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Store lost its exact fsynced body frame",
                )
            })?;
        let (adapter_startup, store_projection) = adapter_startup
            .advance_recovered_decision_fetch_store(&verified, &projection, body)
            .map_err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new)?;
        let (fetch_ordinal, _store_ordinal) = opened
            .authenticate_recovered_decision_fetch_store(&projection, &store_projection)
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Fetch-to-Store durable prefix is not exact",
                )
            })?;
        if !ledger_store.load().is_ok_and(|loaded| loaded == opened)
            || !ledger_store.revalidates_recovered_decision_fetch_store(
                &projection,
                fetch_ordinal,
                &store_projection,
            )
        {
            return Err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "recovered Decision Store LedgerV1 reopen changed the exact prefix",
            ));
        }
        let body_pipeline = opened
            .authenticate_durable_certified_body_pipeline_startup(&verified, &body_store)
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Store body-pipeline census authentication failed",
                )
            })?;
        let (body_pipeline, adapter_startup) = body_pipeline
            .replay_adapter_startup(adapter_startup)
            .map_err(ProductionRecoveredWalDecisionFetchStartupErrorV1::new)?;
        let (recovery, body_pipeline) = AuthenticatedLifecycleRecoveryCut::
            assemble_storage_only_with_recovered_decision_store_and_body_pipeline_startup(
                opened.clone(),
                serve_payloads,
                &mut body_store,
                &projection,
                &store_projection,
                body_pipeline,
            )
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Store storage census assembly failed",
                )
            })?;
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let mut installed = registry
            .registry_mut()
            .install_recovered_wal_decision_store(
                &verified,
                &ledger_store,
                &opened,
                projection,
                store_projection,
            )
            .map_err(|error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(error.reason())
            })?;
        installed
            .install_body_pipeline(body_pipeline)
            .map_err(|error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(error.reason())
            })?;
        let authority = authority::production_authority(
            &verified,
            config,
            reply_route_source_capacity,
        )
        .ok_or_else(|| {
            ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                "verified height cannot derive recovered Decision Store lifecycle authority",
            )
        })?;
        let (coordinator, recovery) = installed
            .open_with_exact_store_authority(authority, ledger_store, &mut payload_store, recovery)
            .map_err(|error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(error.reason())
            })?;
        Ok(Self {
            verified,
            coordinator,
            registry,
            payload_store,
            serve_payloads: recovery.into_serve_payloads(),
            body_store: Some(body_store),
            body_store_identity: None,
            kura_binding: None,
            apply_service: None,
            adapter_startup: Some(adapter_startup),
            timeout_supersession_successor: None,
        })
    }
    /// Publish or exactly coalesce one recovered Decision body fast-forward.
    ///
    /// The input is the sole closed result of the authenticated WAL Fetch,
    /// same-store validated body, and private reducer Store→Validate→Apply
    /// preview. The exact predecessor ledger remains on disk while the complete
    /// four-row successor, Serve payloads, ordinary body census, coordinator, and
    /// dedicated Apply carrier are authenticated in memory. One exact-successor
    /// fsync then precedes only infallible ownership moves.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_recovered_decision_apply_startup(
        verified: VerifiedHeightContext,
        projection: Box<crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1>,
        effects: Vec<crate::sumeragi::v2::AdapterEffect>,
        ledger_root: &Path,
        mut body_store: V2BodyStore,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        mut payload_store: CertifiedServePayloadStoreV1,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<Self, ProductionRecoveredDecisionApplyStartupErrorV1> {
        if !projection.validates(&verified) || !effects.is_empty() {
            return Err(ProductionRecoveredDecisionApplyStartupErrorV1::new(
                "recovered Decision Apply projection retained inconsistent adapter state",
            ));
        }
        let context = projection::lifecycle_context(verified.context());
        let (ledger_store, predecessor) = LifecycleLedgerStoreV1::open(ledger_root, context)
            .map_err(|_error| {
                ProductionRecoveredDecisionApplyStartupErrorV1::new(
                    "recovered Decision Apply LedgerV1 open failed",
                )
            })?;
        let fetch_is_present = predecessor
            .records
            .iter()
            .any(|record| projection.fetch().names_record(record));
        let staged_predecessor = if fetch_is_present {
            predecessor.clone()
        } else {
            predecessor
                .stage_authenticated_wal_decision_fetch(projection.fetch())
                .map_err(|_error| {
                    ProductionRecoveredDecisionApplyStartupErrorV1::new(
                        "recovered Decision Apply Fetch parent is not exact",
                    )
                })?
                .0
        };
        let (successor, _apply_ordinal, _changed) = staged_predecessor
            .stage_recovered_decision_apply(projection.as_ref())
            .map_err(|_error| {
                ProductionRecoveredDecisionApplyStartupErrorV1::new(
                    "recovered Decision Apply four-row durable lineage is not exact",
                )
            })?;
        let body_pipeline = successor
            .authenticate_durable_certified_body_pipeline_startup(&verified, &body_store)
            .map_err(|_error| {
                ProductionRecoveredDecisionApplyStartupErrorV1::new(
                    "recovered Decision Apply body-pipeline census authentication failed",
                )
            })?;
        let (recovery, body_pipeline) = AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_decision_apply_and_body_pipeline_startup(
            successor.clone(),
            serve_payloads,
            &mut body_store,
            projection.as_ref(),
            body_pipeline,
        )
        .map_err(|_error| {
            ProductionRecoveredDecisionApplyStartupErrorV1::new(
                "recovered Decision Apply storage census assembly failed",
            )
        })?;
        let authority = authority::production_authority(
            &verified,
            config,
            reply_route_source_capacity,
        )
        .ok_or_else(|| {
            ProductionRecoveredDecisionApplyStartupErrorV1::new(
                "verified height cannot derive recovered Decision Apply lifecycle authority",
            )
        })?;
        let prepared = LifecycleCoordinator::prepare_with_authenticated_successor_store_borrowed(
            authority,
            ledger_store,
            predecessor,
            successor.clone(),
            &payload_store,
            &recovery,
        )
        .map_err(|_error| {
            ProductionRecoveredDecisionApplyStartupErrorV1::new(
                "recovered Decision Apply prospective coordinator open failed",
            )
        })?;
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let (adapter_startup, mut installed) = registry
            .registry_mut()
            .install_recovered_decision_apply(&verified, &successor, projection, effects)
            .map_err(|error| ProductionRecoveredDecisionApplyStartupErrorV1::new(error.reason()))?;
        let (body_pipeline, adapter_startup) = body_pipeline
            .replay_adapter_startup(adapter_startup)
            .map_err(ProductionRecoveredDecisionApplyStartupErrorV1::new)?;
        installed
            .install_body_pipeline(body_pipeline)
            .map_err(|error| ProductionRecoveredDecisionApplyStartupErrorV1::new(error.reason()))?;
        let (coordinator, recovery) = installed
            .open_with_prepared_successor(prepared, &mut payload_store, recovery)
            .map_err(|error| ProductionRecoveredDecisionApplyStartupErrorV1::new(error.reason()))?;
        Ok(Self {
            verified,
            coordinator,
            registry,
            payload_store,
            serve_payloads: recovery.into_serve_payloads(),
            body_store: Some(body_store),
            body_store_identity: None,
            kura_binding: None,
            apply_service: None,
            adapter_startup: Some(adapter_startup),
            timeout_supersession_successor: None,
        })
    }
    /// Bind one paired recovered-WAL open and adapter startup to exact owners.
    ///
    /// The consumed permit is constructible only by the private combined
    /// startup wrapper after the storage-authenticated open succeeds.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn from_recovered_wal_open(
        _permit: ProductionRecoveredLifecycleOwnerAssemblyPermitV1,
        opened: RecoveredWalProductionOwnerOpenV1,
        mut registry: LifecycleWorkRegistryHolder,
        payload_store: CertifiedServePayloadStoreV1,
        body_store: V2BodyStore,
        adapter_startup: ProductionLifecycleAdapterStartupV1,
    ) -> Result<Self, ProductionLifecycleStartupErrorV1> {
        let RecoveredWalProductionOwnerOpenV1 {
            coordinator,
            verified,
            serve_payloads,
            registry_identity,
            body_store_identity,
            payload_store_identity,
        } = opened;
        if coordinator.active_context() != projection::lifecycle_context(verified.context())
            || !body_store.matches_context(verified.context())
            || !body_store
                .instance_identity()
                .same_instance(&body_store_identity)
            || !payload_store
                .instance_identity()
                .same_instance(&payload_store_identity)
            || payload_store
                .validate_authenticated_cut(&serve_payloads)
                .is_err()
            || !registry
                .registry_mut()
                .instance_identity()
                .same_instance(&registry_identity)
        {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::InvalidStorageCut,
            ));
        }
        if !registry
            .registry_mut()
            .exactly_covers_recovered_ready_work_and_wal_authority(&coordinator)
        {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::RegistryCoordinatorMismatch,
            ));
        }
        Ok(Self {
            verified,
            coordinator,
            registry,
            payload_store,
            serve_payloads,
            body_store: Some(body_store),
            body_store_identity: None,
            kura_binding: None,
            apply_service: None,
            adapter_startup: Some(adapter_startup),
            timeout_supersession_successor: None,
        })
    }
}
#[cfg(test)]
impl ProductionLifecycleOwnerV1 {
    /// Whether owner-open retained the one-shot timeout-supersession successor proof.
    pub(in crate::sumeragi) fn has_timeout_supersession_successor_for_test(&self) -> bool {
        self.timeout_supersession_successor.is_some()
    }

    pub(in crate::sumeragi) fn exact_recovered_body_pipeline_join_for_test(&mut self) -> bool {
        if self.coordinator.active_context()
            != projection::lifecycle_context(self.verified.context())
        {
            return false;
        }
        let _retained_payload_store = &self.payload_store;
        let _retained_body_store = self
            .body_store
            .as_ref()
            .expect("an unlaunched production owner retains its exact body store");
        self.adapter_startup
            .as_ref()
            .is_some_and(ProductionLifecycleAdapterStartupV1::is_exact_for_test)
            && {
                let registry = self.registry.registry_mut();
                registry.exactly_covers_recovered_ready_work(&self.coordinator)
                    || registry
                        .exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator)
            }
    }
    /// Return the exact durable high-water/ordinal pair for the sole control row.
    pub(in crate::sumeragi) fn recovered_control_row_summary_for_test(
        &mut self,
    ) -> Option<(u128, u128)> {
        if !self
            .registry
            .registry_mut()
            .exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator)
        {
            return None;
        }
        let mut controls = self.coordinator.records.values().filter(|record| {
            matches!(
                record.work_class,
                LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout
            ) && record.state == LifecycleState::Ready
        });
        let control = controls.next()?;
        controls
            .next()
            .is_none()
            .then_some((self.coordinator.high_water, control.ordinal))
    }
    /// Return the durable high-water/ordinal pair for the sole WAL Decision Fetch.
    pub(in crate::sumeragi) fn recovered_decision_fetch_row_summary_for_test(
        &mut self,
    ) -> Option<(u128, u128)> {
        if !self
            .registry
            .registry_mut()
            .exactly_covers_recovered_ready_work_and_wal_authority(&self.coordinator)
        {
            return None;
        }
        let mut fetches = self.coordinator.records.values().filter(|record| {
            record.work_class == LifecycleWorkClass::Fetch
                && record.state == LifecycleState::Ready
                && self
                    .coordinator
                    .durable_records
                    .get(&record.ordinal)
                    .is_some_and(|metadata| metadata.payload == DurablePayloadReference::None)
        });
        let fetch = fetches.next()?;
        fetches
            .next()
            .is_none()
            .then_some((self.coordinator.high_water, fetch.ordinal))
    }
    fn live_fetch_count_for_test(&self) -> usize {
        self.coordinator
            .records
            .values()
            .filter(|record| {
                record.work_class == LifecycleWorkClass::Fetch
                    && !matches!(record.state, LifecycleState::Terminal(_))
            })
            .count()
    }
    fn live_body_pipeline_counts_for_test(&self) -> (usize, usize, usize) {
        let count = |work_class| {
            self.coordinator
                .records
                .values()
                .filter(|record| {
                    record.work_class == work_class
                        && !matches!(record.state, LifecycleState::Terminal(_))
                })
                .count()
        };
        (
            count(LifecycleWorkClass::Fetch),
            count(LifecycleWorkClass::Store),
            count(LifecycleWorkClass::Validate),
        )
    }
    fn certified_serve_and_producer_carrier_counts_for_test(&mut self) -> (usize, usize) {
        self.registry
            .registry_mut()
            .certified_serve_and_producer_carrier_counts()
    }
    fn claim_certified_serve_for_test(&mut self) -> super::TurnLease {
        assert!(
            self.registry
                .registry_mut()
                .exactly_covers_recovered_ready_work(&self.coordinator),
            "test claim requires the exact owner-coheld concrete census"
        );
        let ready = self.coordinator.ready_index.iter().map(|ordinal| {
            let record = &self.coordinator.records[ordinal];
            (
                *ordinal,
                super::SchedulerReadyInputs::new(record, None, [0; 6]),
            )
        });
        let TurnPlan::Execute(lease) = self.coordinator.plan_turn(
            super::SchedulerInputs::new([], ready)
                .expect("test owner has one exact Ready Serve row"),
        ) else {
            panic!("test owner must claim its exact Ready Serve row")
        };
        assert_eq!(lease.work_class(), LifecycleWorkClass::CertifiedServe);
        lease
    }
    fn terminal_validate_count_for_test(&self) -> usize {
        self.coordinator
            .records
            .values()
            .filter(|record| {
                record.work_class == LifecycleWorkClass::Validate
                    && matches!(record.state, LifecycleState::Terminal(_))
            })
            .count()
    }
}
/// Narrow comparison surface used by the terminal recovered-Decision ledger oracle.
///
/// The production implementation delegates to the sealed WAL/body projection. Keeping the
/// structural scan behind this private surface lets focused tests exercise terminal/live census
/// behavior without manufacturing the projection's move-only adapter and runtime authorities.
trait TerminalRecoveredDecisionApplyProjectionV1 {
    fn belongs_to_context(&self, context: LifecycleContext) -> bool;
    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool;
    fn exactly_matches_advanced_apply_parent(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool;
    fn exactly_matches_terminal_successor_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply: &LifecycleLedgerRecordV1,
    ) -> bool;
}
/// Narrow comparison surface for staging one live recovered-Decision chain.
///
/// Production delegates to the sealed WAL/body projection. The private trait
/// also lets ledger-local tests exercise crash-prefix and collision behavior
/// without manufacturing runtime or adapter ownership tokens.
trait RecoveredDecisionApplyStageProjectionV1 {
    fn belongs_to_context(&self, context: LifecycleContext) -> bool;
    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool;
    fn exactly_matches_live_fetch(&self, fetch: &LifecycleLedgerRecordV1) -> bool;
    fn exactly_matches_advanced_fetch(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool;
    fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1;
}

/// Borrowed comparison-only projection of one installed recovered Apply carrier.
struct RecoveredDecisionApplyCarrierLedgerProjectionV1<'a> {
    fetch: &'a AuthenticatedRecoveredWalDecisionFetchProjection,
    lineage: &'a RecoveredDecisionApplyCandidateLineageV1,
}

impl RecoveredDecisionApplyStageProjectionV1
    for RecoveredDecisionApplyCarrierLedgerProjectionV1<'_>
{
    fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.fetch.belongs_to_context(context)
    }

    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        self.fetch.names_record(record)
    }

    fn exactly_matches_live_fetch(&self, fetch: &LifecycleLedgerRecordV1) -> bool {
        self.fetch.exactly_matches_record(fetch)
    }

    fn exactly_matches_advanced_fetch(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool {
        self.fetch
            .exactly_matches_advanced_apply_parent(fetch, store_ordinal)
    }

    fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1 {
        self.lineage
    }
}

impl RecoveredDecisionApplyStageProjectionV1
    for crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1
{
    fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.fetch().belongs_to_context(context)
    }
    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        self.fetch().names_record(record)
    }
    fn exactly_matches_live_fetch(&self, fetch: &LifecycleLedgerRecordV1) -> bool {
        self.fetch().exactly_matches_record(fetch)
    }
    fn exactly_matches_advanced_fetch(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool {
        self.fetch()
            .exactly_matches_advanced_apply_parent(fetch, store_ordinal)
    }
    fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1 {
        self.lineage()
    }
}
impl TerminalRecoveredDecisionApplyProjectionV1
    for crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1
{
    fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.fetch().belongs_to_context(context)
    }
    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        self.fetch().names_record(record)
    }
    fn exactly_matches_advanced_apply_parent(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool {
        self.fetch()
            .exactly_matches_advanced_apply_parent(fetch, store_ordinal)
    }
    fn exactly_matches_terminal_successor_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply: &LifecycleLedgerRecordV1,
    ) -> bool {
        self.lineage()
            .exactly_matches_terminal_successor_records(owner, store, validate, apply)
    }
}
fn recovered_broadcast_and_next_sign_keys_are_exact(
    broadcast_key: LifecycleKey,
    broadcast_stage: LifecycleStage,
    next_sign_key: LifecycleKey,
    next_sign_stage: LifecycleStage,
) -> bool {
    let phase_and_commitment_are_exact = match (
        broadcast_key.phase(),
        broadcast_stage.kind(),
        next_sign_key.phase(),
        next_sign_stage.kind(),
    ) {
        (
            LifecyclePhase::BroadcastProposal,
            LifecycleStageKind::BroadcastProposal,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ) => {
            broadcast_key.execution_commitment().is_none()
                && next_sign_key.execution_commitment().is_some()
        }
        (
            LifecyclePhase::BroadcastPrepareVote,
            LifecycleStageKind::BroadcastPrepareVote,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        ) => {
            broadcast_key.execution_commitment().is_some()
                && broadcast_key.execution_commitment() == next_sign_key.execution_commitment()
        }
        _ => false,
    };
    phase_and_commitment_are_exact
        && broadcast_key.context() == next_sign_key.context()
        && broadcast_key.round() == next_sign_key.round()
        && broadcast_key.proposal_round() == next_sign_key.proposal_round()
        && broadcast_key.subject() == next_sign_key.subject()
}
include!("v2_lifecycle_ledger_operations.rs");
/// Startup-fatal failure from the consuming LedgerV1/body-pipeline join.
///
/// The failing call has consumed the opened ledger and body-store instances.
/// Recovery must abort this startup attempt; this reason is diagnostic only and
/// carries no retry, parts, or authority-recovery surface.
#[derive(Debug, Error)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(super) enum DurableCertifiedBodyPipelineRecoveryError {
    /// The opened ledger is not owned by the supplied authenticated height.
    #[error("durable certified-body ledger context is not the verified height context")]
    InvalidVerifiedContext,
    /// The consumed ledger store is foreign or no longer loads this frame.
    #[error("durable certified-body ledger store does not own the opened frame")]
    InvalidLedgerStore,
    /// The opened body-store instance is not owned by the verified height.
    #[error("durable certified-body store is not the verified height context")]
    InvalidBodyStoreContext,
    /// The selected row is not an exact current-V1 body-pipeline shape.
    #[error("durable certified-body ledger row is not recoverable")]
    InvalidLedgerRow,
    /// The exact body frame could not be authenticated in the opened store.
    #[error(transparent)]
    BodyFrame(#[from] DurableBodyFrameRecoveryError),
    /// The row replay family and authenticated body frame do not form one cut.
    #[error("durable certified-body replay join is inconsistent")]
    InvalidReplayJoin,
    /// Two live rows collide in owner, address, completion, or body identity.
    #[error("durable certified-body recovery census is ambiguous")]
    AmbiguousCensus,
    /// The sealed census no longer matches all of its retained storage owners.
    #[error("durable certified-body storage recovery cut is inconsistent")]
    InvalidStorageCut,
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
include!("v2_lifecycle_ledger_store.rs");
include!("v2_lifecycle_ledger_tests.rs");
