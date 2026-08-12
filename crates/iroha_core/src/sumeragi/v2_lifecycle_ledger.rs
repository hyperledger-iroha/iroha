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

use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll, Encode};
use thiserror::Error;

use super::projection::{AuthenticatedDurableBodyFrameRecovery, DurableBodyFrameRecoveryError};
use super::replay_authority::{
    AuthenticatedRecoveredDurableCertifiedFetchCensusV1,
    AuthenticatedRecoveredDurableCertifiedFetchV1, CertifiedServeTerminalReplayAuthorityPairV1,
    LifecycleReplayAuthorityV1, authenticate_recovered_durable_certified_fetch,
    recovered_decision_body_continuation_is_exact, seal_recovered_durable_certified_fetch_census,
};
use super::schema::{
    DurableBodyFrameReference, DurableContinuation, DurableContinuationEdge,
    MAX_LIFECYCLE_RECORDS_PER_HEIGHT, serve_and_producer_keys_match,
};
use super::wal_recovery::{
    AuthenticatedRecoveredWalControlProjection, AuthenticatedRecoveredWalDecisionFetchProjection,
    AuthenticatedWalVoteLifecycleRepair, DurableAuthenticatedWalVoteLifecycleRepair,
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
        RecoveredLifecycleOwnerKuraBindingV1, RecoveredWalFrameIdentity, RecoveredWalVoteSign,
        VerifiedHeightContext,
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

/// One-shot proof that the recovery projection contains every live durable
/// Fetch row from one exact opened LedgerV1.
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct DurableCertifiedFetchLedgerCensusPermit {
    _linearity: DurableCertifiedFetchLedgerCensusLinearity,
    ledger_frame_identity: LifecycleDigest,
}

struct DurableCertifiedFetchLedgerCensusLinearity;

impl Drop for DurableCertifiedFetchLedgerCensusLinearity {
    fn drop(&mut self) {}
}

impl DurableCertifiedFetchLedgerCensusPermit {
    fn new(ledger: &LifecycleLedgerV1) -> Self {
        Self {
            _linearity: DurableCertifiedFetchLedgerCensusLinearity,
            ledger_frame_identity: ledger.frame_identity(),
        }
    }

    /// Consume the census permit into its exact canonical ledger-frame identity.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn into_frame_identity(
        self,
    ) -> LifecycleDigest {
        self.ledger_frame_identity
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
    /// Authenticate this row's source before opening its exact body-store frame.
    fn authenticate_durable_certified_fetch<F>(
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
            && self.terminal() == Some(None)
            && self.continuation() == Some(DurableContinuation::None)
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
pub(in crate::sumeragi) struct LifecycleLedgerV1 {
    format_version: u16,
    context: [u8; 32],
    height: u64,
    high_water: u128,
    records: Vec<LifecycleLedgerRecordV1>,
    producer_debts: Vec<LifecycleProducerDebtV1>,
}

/// Move-only CompleteTip terminal-Apply join to the Kura-bound predecessor store.
///
/// This cut owns the full Kura CompleteTip evidence, the exact opened LedgerV1
/// store handle, and the byte-equivalent frame. The store target is compared
/// with the lifecycle root retained when the same `Kura` instance authenticated
/// CompleteTip, so a copied frame at a caller-selected root cannot enter this
/// cut. It still proves only the terminal recovered-Decision body chain: it does
/// not publish the successor or claim that unrelated live rows, Serve payloads,
/// leases, waits, debts, or capacity have been retired. The outer canonical
/// predecessor-storage transaction supplies and discharges those remaining
/// durable owners before it can mint successor activation.
#[must_use = "the CompleteTip terminal-Apply store join must enter retirement or be dropped"]
struct AuthenticatedCompleteTipTerminalApplyStoreJoinV1 {
    complete_tip: crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ledger_store: LifecycleLedgerStoreV1,
    ledger: LifecycleLedgerV1,
    apply_ordinal: u128,
}

/// Opaque failure while authenticating all canonical CompleteTip predecessor stores.
#[derive(Debug, Error)]
#[error("failed to authenticate canonical CompleteTip predecessor lifecycle storage")]
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
/// The cut retains the exact terminal predecessor ledger, the co-located Serve
/// payload-store instance, and the complete retirement-authenticated Serve
/// census. Completed response signatures remain bound to their manifest body
/// hashes without reopening body bytes which normal finality may already have
/// deleted. It exposes no path, raw frame, request, or activation parts and
/// performs no retirement publication by itself.
#[must_use = "the authenticated CompleteTip predecessor stores must enter retirement"]
pub(in crate::sumeragi) struct AuthenticatedCompleteTipPredecessorStorageV1 {
    terminal: AuthenticatedCompleteTipTerminalApplyStoreJoinV1,
    successor: CanonicalCompleteTipSuccessorLedgerTargetV1,
    payload_store: CertifiedServePayloadStoreV1,
    serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    retained_serve_payloads:
        BTreeSet<crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadId>,
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

    fn successor_descends_from_retirement(&self) -> bool {
        self.successor_ledger.context() == self.successor_store.context
            && self.successor_ledger.frame_identity() == self.successor_frame_identity
            && if self.successor_ledger.records.is_empty() {
                self.successor_ledger.producer_debts.is_empty()
                    && self.successor_ledger.high_water == self.retained_high_water
            } else {
                self.successor_ledger.high_water >= self.retained_high_water
                    && self.successor_ledger.records.iter().all(|record| {
                        record.ordinal() > self.retained_high_water
                            && record.owner().first_admission_ordinal() > self.retained_high_water
                    })
            }
    }

    /// Reauthenticate the retained canonical H+1 ledger at its Kura-derived target.
    pub(in crate::sumeragi) fn authorizes_retained_successor(&self) -> bool {
        let Some(successor_root) = self.successor_store.path.parent() else {
            return false;
        };
        self.predecessor_ledger.frame_identity() == self.predecessor_frame_identity
            && self
                .predecessor_store
                .is_authorized_complete_tip_predecessor_target(&self.complete_tip)
            && self.predecessor_store.load().ok().as_ref() == Some(&self.predecessor_ledger)
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
            && self
                .complete_tip
                .predecessor()
                .height()
                .checked_add(1)
                == Some(successor.height)
            && successor.last_committed_height == self.complete_tip.predecessor().height()
    }

    fn exactly_matches_successor_owner(&self, owner: &mut ProductionLifecycleOwnerV1) -> bool {
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
            || !self
                .complete_tip
                .authorizes_successor_kura(owner.kura_binding.as_ref())
            || !self.successor_descends_from_retirement()
            || !self
                .complete_tip
                .authorizes_verified_successor(&owner.verified)
            || !self.complete_tip.authorizes_successor_lifecycle_target(
                successor_root,
                self.successor_ledger.context(),
            )
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
                &self.successor_ledger,
                &owner.serve_payloads,
            )
            || !owner_store.same_publication_target(&self.successor_store)
            || self.successor_store.load().ok().as_ref() != Some(&self.successor_ledger)
            || owner_store.load().ok().as_ref() != Some(&self.successor_ledger)
            || LifecycleLedgerV1::from_coordinator(&owner.coordinator)
                .ok()
                .as_ref()
                != Some(&self.successor_ledger)
        {
            return false;
        }
        let registry = owner.registry.registry_mut();
        registry.exactly_covers_recovered_ready_work(&owner.coordinator)
            || registry.exactly_covers_recovered_ready_work_and_wal_authority(&owner.coordinator)
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
        self,
        mut owner: ProductionLifecycleOwnerV1,
    ) -> Result<BoundRecoveredCompleteTipSuccessorOwnerV1, CompleteTipSuccessorOwnerBindErrorV1>
    {
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
/// runner cutover must retain that wrapper through clock/ingress arming and
/// publish status only through a dedicated typed CompleteTip activation tail.
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
    pub(in crate::sumeragi) fn launch(
        self,
        inputs: super::launch::ProductionLifecycleLaunchInputsV1,
    ) -> Result<
        LaunchedRecoveredCompleteTipSuccessorLifecycleV1,
        super::launch::ProductionLifecycleLaunchErrorV1,
    > {
        let Self { owner, retirement } = self;
        let launched = owner.launch(inputs)?;
        Ok(LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {
            launched,
            retirement,
        })
    }
}

/// Opaque running H+1 lifecycle stack joined to its retired-H authority.
///
/// TODO: The generic lifecycle-coordinator replacement must consume this
/// complete wrapper while arming live clocks, opening authenticated ingress,
/// activating the completion observer, and publishing typed successor status.
/// The production restart bridge is deliberately narrower: it consumes the
/// retired authority after reauthenticating the canonical ledger/status pair
/// and neither launches nor credits this generic replacement.
#[must_use = "the launched CompleteTip successor must remain sealed until final activation"]
#[allow(dead_code)]
pub(in crate::sumeragi) struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 {
    launched: super::launch::LaunchedProductionLifecycleV1,
    retirement: RetiredRecoveredCompleteTipActivationAuthorityV1,
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
        let (store, current) = LifecycleLedgerStoreV1::open(&self.root, self.context)?;
        let successor = if current.records.is_empty() {
            if !current.producer_debts.is_empty()
                || (current.high_water != 0 && current.high_water != retained_high_water)
            {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "empty CompleteTip successor has a foreign ordinal high-water".to_owned(),
                ));
            }
            let initialized = LifecycleLedgerV1::new(
                self.context,
                retained_high_water,
                Vec::new(),
                BTreeMap::new(),
            )?;
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
                    "nonempty CompleteTip successor does not descend above the retained ordinal floor"
                        .to_owned(),
                ));
            }
            current
        };
        if store.load()? != successor {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip successor changed during canonical authentication".to_owned(),
            ));
        }
        Ok((store, successor))
    }
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
        let retired = terminal
            .ledger
            .stage_complete_tip_all_row_retirement(serve_reconciliation)?;
        if !retired
            .authenticate_complete_tip_terminal_apply(&terminal.complete_tip)
            .is_ok_and(|ordinal| ordinal == terminal.apply_ordinal)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip all-row retirement changed its terminal Apply authority".to_owned(),
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
    let (ledger_store, ledger) = LifecycleLedgerStoreV1::open(predecessor_root, context)?;
    let terminal =
        ledger.into_complete_tip_terminal_apply_store_join(ledger_store, complete_tip)?;
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
        Ok(opened == self.ledger
            && self
                .ledger
                .authenticate_complete_tip_terminal_apply(&self.complete_tip)
                .is_ok_and(|ordinal| ordinal == self.apply_ordinal))
    }
}

/// Move-only storage recovery cut for every durable Ready-Fetch row.
///
/// The exact opened LedgerV1 frame, height context, body-store instance, and
/// authenticated all-row census remain inseparable. There is deliberately no
/// parts, clone, candidate, work, or registry-install API: the unified startup
/// transaction consumes this value directly while opening the coordinator and
/// installing the concrete registry in one boundary.
#[must_use = "the exact storage recovery cut must enter the startup composite"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct AuthenticatedDurableCertifiedFetchStorageRecoveryCutV1
{
    verified: VerifiedHeightContext,
    ledger_store: LifecycleLedgerStoreV1,
    ledger: LifecycleLedgerV1,
    body_store: V2BodyStore,
    census: AuthenticatedRecoveredDurableCertifiedFetchCensusV1,
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
    #[error("the lifecycle ledger changed after Ready-Fetch authentication")]
    LedgerFrameMismatch,
    #[error("lifecycle ledger open failed: {0}")]
    Ledger(#[source] LifecycleLedgerError),
    #[error("durable Ready-Fetch authentication failed: {0}")]
    Fetch(#[source] DurableCertifiedFetchRecoveryError),
    #[error("Certified-Serve payload recovery changed before startup: {0}")]
    ServePayload(#[source] CertifiedServePayloadStoreError),
    #[error("the complete Ready-Fetch census could not enter its startup phase")]
    InvalidFetchCensus,
    #[error("lifecycle recovery assembly failed: {0}")]
    Recovery(#[source] LifecycleRecoveryAssemblyError),
    #[error("the recovered Ready-Fetch census cannot enter an empty registry")]
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
}

impl ProductionRecoveredWalControlStartupErrorV1 {
    const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Return the stable non-authorizing failure classification.
    pub(in crate::sumeragi) const fn reason(&self) -> &'static str {
        self.reason
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
impl AuthenticatedDurableCertifiedFetchStorageRecoveryCutV1 {
    fn is_exact(&self) -> bool {
        let live_body_fetch_count = self
            .ledger
            .records
            .iter()
            .filter(|record| {
                record.work_class() == Some(LifecycleWorkClass::Fetch)
                    && record.terminal() == Some(None)
                    && matches!(
                        record.durable_payload(),
                        Some(DurablePayloadReference::BodyFrame(_))
                    )
            })
            .count();
        self.ledger.context() == projection::lifecycle_context(self.verified.context())
            && self.ledger_store.context == self.ledger.context()
            && self
                .ledger_store
                .load()
                .is_ok_and(|opened| opened == self.ledger)
            && self.body_store.matches_context(self.verified.context())
            && self
                .census
                .exactly_matches_opened_ledger(&self.ledger, live_body_fetch_count)
    }

    /// Consume all durable storage authority into the sole production owner.
    ///
    /// Every context, frame, payload-store, census, and empty-registry check
    /// precedes terminal-outcome consumption. The logical recovery cut then
    /// consumes every Fetch candidate; its concrete peers enter the fresh
    /// registry before coordinator preparation. The exact registry/coordinator
    /// join is checked before either durable open publication occurs.
    // The runner cutover is deliberately separate; keep this sole production
    // constructor present until that owner is wired into startup.
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
        let fetches = census.into_startup(&ledger).ok_or_else(|| {
            ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::InvalidFetchCensus,
            )
        })?;
        let mut registry = LifecycleWorkRegistryHolder::empty();
        if !fetches.preflights_empty_registry(registry.registry_mut()) {
            return Err(ProductionLifecycleStartupErrorV1::new(
                ProductionLifecycleStartupErrorKindV1::RegistryInstall,
            ));
        }
        let (mut recovery, fetches) =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_durable_fetch_startup(
                ledger,
                serve_payloads,
                &mut body_store,
                fetches,
            )
            .map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::Recovery(error),
                )
            })?;
        fetches
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
            .into_durable_certified_fetch_storage_recovery_cut(verified, ledger_store, body_store)
            .map_err(|error| {
                ProductionLifecycleStartupErrorV1::new(
                    ProductionLifecycleStartupErrorKindV1::Fetch(error),
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
    /// existing row is left byte-for-byte untouched while its volatile carrier
    /// is reconstructed. All other row shapes fail closed.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_recovered_control_startup(
        verified: VerifiedHeightContext,
        projection: AuthenticatedRecoveredWalControlProjection,
        ledger_root: &Path,
        mut body_store: V2BodyStore,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        mut payload_store: CertifiedServePayloadStoreV1,
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
        let (repaired, ordinal, changed) = opened
            .stage_authenticated_wal_control_sign(&projection)
            .map_err(|_error| {
                ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control durable row is absent-or-exact invariant failed",
                )
            })?;
        if changed {
            ledger_store
                .persist_exact_successor(&opened, &repaired)
                .map_err(|_error| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "recovered control LedgerV1 successor publication failed",
                    )
                })?;
        }
        if !ledger_store.load().is_ok_and(|loaded| loaded == repaired)
            || !projection.exactly_matches_ledger_at(&repaired, ordinal)
        {
            return Err(ProductionRecoveredWalControlStartupErrorV1::new(
                "recovered control LedgerV1 reopen changed the exact row",
            ));
        }
        let fetches = repaired
            .authenticate_durable_certified_fetch_startup(&verified, &body_store)
            .map_err(|_error| {
                ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control Ready-Fetch census authentication failed",
                )
            })?;
        let (recovery, fetches) =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_control_sign_and_durable_fetch_startup(
                repaired.clone(),
                serve_payloads,
                &mut body_store,
                &projection,
                fetches,
            )
            .map_err(|_error| {
                ProductionRecoveredWalControlStartupErrorV1::new(
                    "recovered control storage census assembly failed",
                )
            })?;
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let mut installed = registry
            .registry_mut()
            .install_recovered_wal_control_sign(&verified, &ledger_store, &repaired, projection)
            .map_err(|error| ProductionRecoveredWalControlStartupErrorV1::new(error.reason()))?;
        installed
            .install_fetches(fetches)
            .map_err(|error| ProductionRecoveredWalControlStartupErrorV1::new(error.reason()))?;
        let authority =
            authority::production_authority(&verified, config, reply_route_source_capacity)
                .ok_or_else(|| {
                    ProductionRecoveredWalControlStartupErrorV1::new(
                        "verified height cannot derive recovered control lifecycle authority",
                    )
                })?;
        let (coordinator, recovery) = installed
            .open_with_exact_store_authority(authority, ledger_store, &mut payload_store, recovery)
            .map_err(|error| ProductionRecoveredWalControlStartupErrorV1::new(error.reason()))?;
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
        })
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
        let (repaired, ordinal, changed) = opened
            .stage_authenticated_wal_decision_fetch(&projection)
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Fetch durable row is absent-or-exact invariant failed",
                )
            })?;
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
        let fetches = repaired
            .authenticate_durable_certified_fetch_startup(&verified, &body_store)
            .map_err(|_error| {
                ProductionRecoveredWalDecisionFetchStartupErrorV1::new(
                    "recovered Decision Fetch Ready-Fetch census authentication failed",
                )
            })?;
        let (recovery, fetches) =
            AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_wal_decision_fetch_and_durable_fetch_startup(
                repaired.clone(),
                serve_payloads,
                &mut body_store,
                &projection,
                fetches,
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
        installed.install_fetches(fetches).map_err(|error| {
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
        })
    }

    /// Publish or exactly coalesce one recovered Decision body fast-forward.
    ///
    /// The input is the sole closed result of the authenticated WAL Fetch,
    /// same-store validated body, and private reducer Store→Validate→Apply
    /// preview. The exact predecessor ledger remains on disk while the complete
    /// four-row successor, Serve payloads, Ready-Fetch census, coordinator, and
    /// dedicated Apply carrier are authenticated in memory. One exact-successor
    /// fsync then precedes only infallible ownership moves.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_recovered_decision_apply_startup(
        verified: VerifiedHeightContext,
        projection: crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
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
        let (successor, _apply_ordinal, _changed) = predecessor
            .stage_recovered_decision_apply(&projection)
            .map_err(|_error| {
                ProductionRecoveredDecisionApplyStartupErrorV1::new(
                    "recovered Decision Apply four-row durable lineage is not exact",
                )
            })?;
        let fetches = successor
            .authenticate_durable_certified_fetch_startup(&verified, &body_store)
            .map_err(|_error| {
                ProductionRecoveredDecisionApplyStartupErrorV1::new(
                    "recovered Decision Apply Ready-Fetch census authentication failed",
                )
            })?;
        let (recovery, fetches) = AuthenticatedLifecycleRecoveryCut::assemble_storage_only_with_recovered_decision_apply_and_durable_fetch_startup(
            successor.clone(),
            serve_payloads,
            &mut body_store,
            &projection,
            fetches,
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
        installed
            .install_fetches(fetches)
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
        })
    }
}

#[cfg(test)]
impl ProductionLifecycleOwnerV1 {
    pub(in crate::sumeragi) fn exact_recovered_fetch_join_for_test(&mut self) -> bool {
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

impl LifecycleLedgerV1 {
    /// Hash the canonical V1 encoding, independent of its store path.
    pub(in crate::sumeragi::v2_lifecycle_coordinator) fn frame_identity(&self) -> LifecycleDigest {
        let mut preimage = Vec::from(&b"iroha:sumeragi:v2:lifecycle-ledger-frame:v1"[..]);
        preimage.extend_from_slice(&self.encode());
        LifecycleDigest::new(*Hash::new(preimage).as_ref())
    }

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

    /// Stage the exact all-row tombstone successor for CompleteTip retirement.
    ///
    /// Existing terminal rows remain byte-for-byte unchanged. Every live
    /// Certified-Serve row must consume one payload-store-authenticated
    /// terminal update together with its adjacent live ProducerTurn; an
    /// already-terminal Serve with a live ProducerTurn must consume the
    /// corresponding no-update coverage proof. Every other live row becomes a
    /// `Cancelled` tombstone without changing its immutable admission or replay
    /// material. No durable write occurs in this method.
    fn stage_complete_tip_all_row_retirement(
        &self,
        mut serve_reconciliation: CompleteTipServeRetirementReconciliationV1,
    ) -> Result<Self, LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !serve_reconciliation.authenticates_source(self) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip Serve retirement belongs to another ledger frame".to_owned(),
            ));
        }

        let mut consumed_producers = BTreeSet::new();
        let mut retired_records = Vec::with_capacity(self.records.len());
        for record in &self.records {
            if consumed_producers.remove(&record.ordinal()) {
                continue;
            }
            let work_class = record.work_class().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip retirement encountered an undecodable work class".to_owned(),
                )
            })?;
            let terminal = record.terminal().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip retirement encountered an undecodable terminal state".to_owned(),
                )
            })?;

            if work_class == LifecycleWorkClass::CertifiedServe {
                let producer_ordinal = record.ordinal().checked_add(1).ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "CompleteTip Serve producer ordinal exhausted".to_owned(),
                    )
                })?;
                let producer = self
                    .records
                    .binary_search_by_key(&producer_ordinal, LifecycleLedgerRecordV1::ordinal)
                    .ok()
                    .and_then(|index| self.records.get(index))
                    .ok_or_else(|| {
                        LifecycleLedgerError::InvalidLedger(
                            "CompleteTip Serve lost its adjacent ProducerTurn".to_owned(),
                        )
                    })?;
                if producer.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                    || producer.owner() != record.owner()
                    || producer.terminal().is_none()
                {
                    return Err(LifecycleLedgerError::InvalidLedger(
                        "CompleteTip Serve/ProducerTurn pair changed before retirement".to_owned(),
                    ));
                }

                match (
                    terminal,
                    producer.terminal().expect("producer terminal decoded"),
                ) {
                    (None, None) => {
                        let update = serve_reconciliation
                            .take_terminal_update_for_exact_pair(record, producer)
                            .ok_or_else(|| {
                                LifecycleLedgerError::InvalidLedger(
                                    "live CompleteTip Serve has no exact terminal payload update"
                                        .to_owned(),
                                )
                            })?;
                        let (payload, outcome, serve_replay, producer_replay) = update
                            .consume_for_exact_ledger_pair(record, producer)
                            .ok_or_else(|| {
                                LifecycleLedgerError::InvalidLedger(
                                    "CompleteTip Serve terminal update changed before staging"
                                        .to_owned(),
                                )
                            })?;
                        retired_records.push(Self::terminalized_record(
                            record,
                            outcome,
                            payload,
                            serve_replay,
                        )?);
                        retired_records.push(Self::terminalized_record(
                            producer,
                            TerminalOutcome::Cancelled,
                            producer.durable_payload().ok_or_else(|| {
                                LifecycleLedgerError::InvalidLedger(
                                    "CompleteTip ProducerTurn payload is undecodable".to_owned(),
                                )
                            })?,
                            producer_replay,
                        )?);
                        consumed_producers.insert(producer_ordinal);
                    }
                    (Some(_), None) => {
                        if !serve_reconciliation
                            .take_terminal_serve_live_producer_coverage(record, producer)
                        {
                            return Err(LifecycleLedgerError::InvalidLedger(
                                "terminal CompleteTip Serve has no exact live ProducerTurn coverage"
                                    .to_owned(),
                            ));
                        }
                        retired_records.push(record.clone());
                        retired_records.push(Self::cancelled_record(producer)?);
                        consumed_producers.insert(producer_ordinal);
                    }
                    (Some(_), Some(_)) => {
                        retired_records.push(record.clone());
                        retired_records.push(producer.clone());
                        consumed_producers.insert(producer_ordinal);
                    }
                    (None, Some(_)) => {
                        return Err(LifecycleLedgerError::InvalidLedger(
                            "live CompleteTip Serve has an already-terminal ProducerTurn"
                                .to_owned(),
                        ));
                    }
                }
                continue;
            }

            if work_class == LifecycleWorkClass::ProducerTurn {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "CompleteTip ProducerTurn was not consumed by its exact Serve owner".to_owned(),
                ));
            }
            retired_records.push(if terminal.is_some() {
                record.clone()
            } else {
                Self::cancelled_record(record)?
            });
        }

        if !consumed_producers.is_empty() || !serve_reconciliation.is_drained() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip Serve retirement census was not consumed exactly once".to_owned(),
            ));
        }
        let retired = Self::new(
            self.context(),
            self.high_water,
            retired_records,
            BTreeMap::new(),
        )?;
        if retired.high_water != self.high_water
            || retired.records.len() != self.records.len()
            || retired
                .records
                .iter()
                .any(|record| record.terminal() == Some(None))
            || !retired.producer_debts.is_empty()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip all-row retirement did not reach the exact quiescent frame".to_owned(),
            ));
        }
        Ok(retired)
    }

    fn cancelled_record(
        record: &LifecycleLedgerRecordV1,
    ) -> Result<LifecycleLedgerRecordV1, LifecycleLedgerError> {
        let payload = record.durable_payload().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "CompleteTip retirement encountered an undecodable payload".to_owned(),
            )
        })?;
        Self::terminalized_record(
            record,
            TerminalOutcome::Cancelled,
            payload,
            record.replay_authority.clone(),
        )
    }

    fn terminalized_record(
        record: &LifecycleLedgerRecordV1,
        outcome: TerminalOutcome,
        payload: DurablePayloadReference,
        replay_authority: LifecycleReplayAuthorityV1,
    ) -> Result<LifecycleLedgerRecordV1, LifecycleLedgerError> {
        if record.terminal() != Some(None)
            || record.continuation() != Some(DurableContinuation::None)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip retirement can terminalize only one live uncontinued row".to_owned(),
            ));
        }
        LifecycleLedgerRecordV1::new(
            record.key().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip retirement encountered an undecodable key".to_owned(),
                )
            })?,
            record.owner(),
            record.ordinal(),
            record.work_class().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip retirement encountered an undecodable work class".to_owned(),
                )
            })?,
            record.stage().ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip retirement encountered an undecodable stage".to_owned(),
                )
            })?,
            Some(outcome),
            record.reconstruction_source(),
            payload,
            replay_authority,
            DurableContinuation::None,
        )
    }

    /// Authenticate every live BodyFrame-backed Fetch against this exact frame.
    ///
    /// Unlike the consuming storage-cut constructor, this internal phase keeps
    /// the ledger and body store borrowed. Recovered-WAL startup uses it only
    /// after the exact Validate-to-Sign repair is fsynced, so the resulting
    /// census is bound to the final frame which the coordinator will open.
    pub(super) fn authenticate_durable_certified_fetch_startup(
        &self,
        verified: &VerifiedHeightContext,
        store: &V2BodyStore,
    ) -> Result<
        super::replay_authority::PreparedDurableCertifiedFetchStartupV1,
        DurableCertifiedFetchRecoveryError,
    > {
        self.authenticate_durable_certified_fetch_census(verified, store)?
            .into_startup(self)
            .ok_or(DurableCertifiedFetchRecoveryError::InvalidStorageCut)
    }

    fn authenticate_durable_certified_fetch_census(
        &self,
        verified: &VerifiedHeightContext,
        store: &V2BodyStore,
    ) -> Result<
        AuthenticatedRecoveredDurableCertifiedFetchCensusV1,
        DurableCertifiedFetchRecoveryError,
    > {
        if self.context() != projection::lifecycle_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidVerifiedContext);
        }
        if !store.matches_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidBodyStoreContext);
        }
        let mut entries = Vec::new();
        for record in self.records.iter().filter(|record| {
            record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.terminal() == Some(None)
                && matches!(
                    record.durable_payload(),
                    Some(DurablePayloadReference::BodyFrame(_))
                )
        }) {
            let Some(DurablePayloadReference::BodyFrame(reference)) = record.durable_payload()
            else {
                return Err(DurableCertifiedFetchRecoveryError::InvalidLedgerRow);
            };
            entries.push(
                record
                    .authenticate_durable_certified_fetch(verified, || {
                        projection::authenticate_durable_body_frame_recovery(
                            self.context(),
                            store,
                            reference,
                        )
                    })?
                    .ok_or(DurableCertifiedFetchRecoveryError::InvalidReplayJoin)?,
            );
        }
        let census = seal_recovered_durable_certified_fetch_census(
            DurableCertifiedFetchLedgerCensusPermit::new(self),
            entries,
        )
        .ok_or(DurableCertifiedFetchRecoveryError::AmbiguousCensus)?;
        Ok(census)
    }

    /// Consume this exact opened ledger and body store into one Ready-Fetch cut.
    ///
    /// Authentication censes every live BodyFrame-backed Fetch row before
    /// either storage owner is moved. Success retains both owners and the
    /// verified context beside the opaque census, preventing a caller from
    /// reminting individual rows or swapping a foreign store before the future
    /// coordinator-open/registry-install transaction consumes the whole cut.
    /// Every error is startup-fatal for these opened instances and consumes
    /// both storage owners; callers must abort this process startup rather than
    /// reopen either path and retry in-process with partially observed state.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn into_durable_certified_fetch_storage_recovery_cut(
        self,
        verified: VerifiedHeightContext,
        ledger_store: LifecycleLedgerStoreV1,
        store: V2BodyStore,
    ) -> Result<
        AuthenticatedDurableCertifiedFetchStorageRecoveryCutV1,
        DurableCertifiedFetchRecoveryError,
    > {
        if self.context() != projection::lifecycle_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidVerifiedContext);
        }
        if ledger_store.context != self.context()
            || !ledger_store.load().is_ok_and(|opened| opened == self)
        {
            return Err(DurableCertifiedFetchRecoveryError::InvalidLedgerStore);
        }
        if !store.matches_context(verified.context()) {
            return Err(DurableCertifiedFetchRecoveryError::InvalidBodyStoreContext);
        }
        let census = self.authenticate_durable_certified_fetch_census(&verified, &store)?;
        let cut = AuthenticatedDurableCertifiedFetchStorageRecoveryCutV1 {
            verified,
            ledger_store,
            ledger: self,
            body_store: store,
            census,
        };
        cut.is_exact()
            .then_some(cut)
            .ok_or(DurableCertifiedFetchRecoveryError::InvalidStorageCut)
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
        let tag_matches_vote = recovered.tag().height() == vote.round.height
            && match vote.phase {
                wire::GlobalPhase::Prepare => recovered.tag().view() == vote.round.view,
                wire::GlobalPhase::Commit => recovered.tag().view() >= vote.round.view,
            };
        if !recovered.wal_identity().is_exact()
            || !recovered.replay_evidence_is_exact()
            || !wal_authority_is_exact
            || self.context().id() != LifecycleDigest::new(context_bytes)
            || self.context().height() != vote.round.height
            || vote.proposal_round.context_id != vote.round.context_id
            || vote.proposal_round.height != vote.round.height
            || !tag_matches_vote
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

    /// Stage exactly one standalone Proposal/Timeout control Sign row.
    ///
    /// An exact existing row stutters without rewriting it. Absence appends
    /// the deterministic `high_water + 1` successor. A same-key row with any
    /// changed owner, metadata, payload, stage, replay authority, or terminal
    /// shape is a hard error and is never repaired in place.
    pub(super) fn stage_authenticated_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered control Sign belongs to another lifecycle context".to_owned(),
            ));
        }
        let mut matching = self
            .records
            .iter()
            .filter(|record| projection.names_record(record));
        if let Some(record) = matching.next() {
            if matching.next().is_some() || !projection.exactly_matches_record(record) {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "existing recovered control Sign row changed exact admission metadata"
                        .to_owned(),
                ));
            }
            return Ok((self.clone(), record.ordinal(), false));
        }

        let ordinal = self.high_water.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered control Sign ordinal exhausted".to_owned(),
            )
        })?;
        let mut staged = self.clone();
        staged.records.push(projection.fresh_record(ordinal)?);
        staged.high_water = ordinal;
        staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.exactly_matches_ledger_at(&staged, ordinal) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged recovered control Sign successor is not exact".to_owned(),
            ));
        }
        Ok((staged, ordinal, true))
    }

    /// Stage exactly one standalone recovered Decision Fetch row.
    ///
    /// An exact existing row is a read-only stutter. Absence appends only the
    /// deterministic `high_water + 1` successor. Same-key drift in owner,
    /// replay source, payload, stage, or terminal state is never repaired.
    pub(super) fn stage_authenticated_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch belongs to another lifecycle context".to_owned(),
            ));
        }
        let mut matching = self
            .records
            .iter()
            .filter(|record| projection.names_record(record));
        if let Some(record) = matching.next() {
            if matching.next().is_some() || !projection.exactly_matches_record(record) {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "existing recovered Decision Fetch row changed exact admission metadata"
                        .to_owned(),
                ));
            }
            return Ok((self.clone(), record.ordinal(), false));
        }

        let ordinal = self.high_water.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch ordinal exhausted".to_owned(),
            )
        })?;
        let mut staged = self.clone();
        staged.records.push(projection.fresh_record(ordinal)?);
        staged.high_water = ordinal;
        staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.exactly_matches_ledger_at(&staged, ordinal) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged recovered Decision Fetch successor is not exact".to_owned(),
            ));
        }
        Ok((staged, ordinal, true))
    }

    /// Stage or exactly coalesce the first-release recovered Decision body chain.
    ///
    /// The payload-free Decision Fetch must already be durable. A live exact
    /// Fetch advances directly to three adjacent BodyFrame successors in one
    /// prospective frame. An already complete four-row chain stutters without
    /// rewriting. Missing Fetch, partial prefixes, foreign same-owner rows, or
    /// any semantic drift fail closed; history is never synthesized.
    pub(super) fn stage_recovered_decision_apply(
        &self,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    ) -> Result<(Self, u128, bool), LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        self.reject_terminal_recovered_decision_apply(projection)?;
        let fetch_projection = projection.fetch();
        let lineage = projection.lineage();
        if !fetch_projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Apply belongs to another lifecycle context".to_owned(),
            ));
        }
        let matching = self
            .records
            .iter()
            .filter(|record| fetch_projection.names_record(record))
            .collect::<Vec<_>>();
        let [fetch] = matching.as_slice() else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "first-release recovered Decision Apply requires one exact durable Fetch parent"
                    .to_owned(),
            ));
        };
        let owner = fetch.owner();
        let owner_records = self
            .records
            .iter()
            .filter(|record| record.owner() == owner)
            .collect::<Vec<_>>();

        if fetch_projection.exactly_matches_record(fetch) {
            if owner_records.len() != 1 || fetch.ordinal() != owner.first_admission_ordinal() {
                return Err(LifecycleLedgerError::InvalidLedger(
                    "live Decision Fetch owner already names foreign lifecycle history".to_owned(),
                ));
            }
            let store_ordinal = self.high_water.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Store ordinal exhausted".to_owned(),
                )
            })?;
            let validate_ordinal = store_ordinal.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Validate ordinal exhausted".to_owned(),
                )
            })?;
            let apply_ordinal = validate_ordinal.checked_add(1).ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "recovered Decision Apply ordinal exhausted".to_owned(),
                )
            })?;
            let [store, validate, apply] = lineage
                .successor_records(owner, store_ordinal, validate_ordinal, apply_ordinal)
                .ok_or_else(|| {
                    LifecycleLedgerError::InvalidLedger(
                        "recovered Decision successors lost exact owner or lineage".to_owned(),
                    )
                })?;
            let mut staged = self.clone();
            let fetch_index = staged
                .records
                .iter()
                .position(|record| record.ordinal() == fetch.ordinal())
                .expect("the exact Fetch parent belongs to the cloned ledger");
            staged.records[fetch_index].terminal =
                Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced));
            staged.records[fetch_index].continuation =
                PersistedDurableContinuationV1::from_schema(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    store_ordinal,
                ));
            staged.records.extend([store, validate, apply]);
            staged.records.sort_by_key(LifecycleLedgerRecordV1::ordinal);
            staged.high_water = apply_ordinal;
            staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
            return Ok((staged, apply_ordinal, true));
        }

        let Some((DurableContinuationEdge::FetchToStore, store_ordinal)) = fetch
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision Fetch is neither live nor an exact complete parent".to_owned(),
            ));
        };
        let validate_ordinal = store_ordinal.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "coalesced recovered Decision Validate ordinal exhausted".to_owned(),
            )
        })?;
        let apply_ordinal = validate_ordinal.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "coalesced recovered Decision Apply ordinal exhausted".to_owned(),
            )
        })?;
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let (Some(store), Some(validate), Some(apply)) = (
            record_at(store_ordinal),
            record_at(validate_ordinal),
            record_at(apply_ordinal),
        ) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "recovered Decision body chain is a partial durable prefix".to_owned(),
            ));
        };
        if owner_records.len() != 4
            || !fetch_projection.exactly_matches_advanced_apply_parent(fetch, store_ordinal)
            || !lineage.exactly_matches_successor_records(owner, store, validate, apply)
            || apply_ordinal > self.high_water
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "coalesced recovered Decision body chain changed exact durable semantics"
                    .to_owned(),
            ));
        }
        Ok((self.clone(), apply_ordinal, false))
    }

    /// Authenticate an already terminal recovered Decision body chain.
    ///
    /// This oracle never feeds storage-only recovery: a terminal Apply must
    /// not be reconstructed as Ready. It only seals the exact four-row
    /// predecessor shape for the CompleteTip retirement transaction, whose
    /// caller must additionally join the full Kura artifact and receipt.
    pub(super) fn authenticate_terminal_recovered_decision_apply(
        &self,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    ) -> Result<u128, LifecycleLedgerError> {
        self.authenticate_terminal_recovered_decision_apply_projection(projection)
    }

    fn reject_terminal_recovered_decision_apply(
        &self,
        projection: &crate::sumeragi::v2::RecoveredDecisionApplyStagedStorageV1,
    ) -> Result<(), LifecycleLedgerError> {
        if self
            .authenticate_terminal_recovered_decision_apply(projection)
            .is_ok()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn reject_terminal_recovered_decision_apply_projection(
        &self,
        projection: &impl TerminalRecoveredDecisionApplyProjectionV1,
    ) -> Result<(), LifecycleLedgerError> {
        if self
            .authenticate_terminal_recovered_decision_apply_projection(projection)
            .is_ok()
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn authenticate_terminal_recovered_decision_apply_projection(
        &self,
        projection: &impl TerminalRecoveredDecisionApplyProjectionV1,
    ) -> Result<u128, LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        if !projection.belongs_to_context(self.context()) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply belongs to another lifecycle context".to_owned(),
            ));
        }
        let matching = self
            .records
            .iter()
            .filter(|record| projection.names_fetch_record(record))
            .collect::<Vec<_>>();
        let [fetch] = matching.as_slice() else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply requires one exact Fetch parent".to_owned(),
            ));
        };
        let Some((DurableContinuationEdge::FetchToStore, store_ordinal)) = fetch
            .continuation()
            .and_then(DurableContinuation::successor_parts)
        else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Fetch lost its Store continuation".to_owned(),
            ));
        };
        let validate_ordinal = store_ordinal.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Validate ordinal exhausted".to_owned(),
            )
        })?;
        let apply_ordinal = validate_ordinal.checked_add(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision Apply ordinal exhausted".to_owned(),
            )
        })?;
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let (Some(store), Some(validate), Some(apply)) = (
            record_at(store_ordinal),
            record_at(validate_ordinal),
            record_at(apply_ordinal),
        ) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain is incomplete".to_owned(),
            ));
        };
        let owner = fetch.owner();
        if self
            .records
            .iter()
            .filter(|record| record.owner() == owner)
            .count()
            != 4
            || !projection.exactly_matches_advanced_apply_parent(fetch, store_ordinal)
            || !projection.exactly_matches_terminal_successor_records(owner, store, validate, apply)
            || apply_ordinal > self.high_water
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal recovered Decision body chain changed exact durable semantics".to_owned(),
            ));
        }
        Ok(apply_ordinal)
    }

    /// Join one terminal recovered-Decision Apply row to the complete
    /// Kura-authenticated CompleteTip evidence retained for successor startup.
    ///
    /// This remains a predecessor-chain oracle, not retirement authority: it
    /// neither censes nor retires unrelated live rows, leases, waits, debt,
    /// capacity, Serve payloads, or either durable publication target. The
    /// consuming retirement transaction proves those independently before it
    /// mints the activation token.
    pub(in crate::sumeragi) fn authenticate_complete_tip_terminal_apply(
        &self,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> Result<u128, LifecycleLedgerError> {
        self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?;
        let predecessor = complete_tip.predecessor();
        if self.context().height() != predecessor.height() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal CompleteTip lifecycle ledger belongs to another height".to_owned(),
            ));
        }
        let mut terminal_applies = self.records.iter().filter(|record| {
            record.work_class() == Some(LifecycleWorkClass::Apply)
                && record.stage().is_some_and(|stage| {
                    stage.kind() == LifecycleStageKind::ApplyDecision
                        && stage.predecessor_scope() == PredecessorScope::Independent
                })
                && record.terminal() == Some(Some(TerminalOutcome::Advanced))
                && record.continuation() == Some(DurableContinuation::None)
                && complete_tip.authorizes_terminal_apply_replay(&record.replay_authority)
        });
        let apply = terminal_applies.next().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "CompleteTip finality has no exact terminal Decision Apply row".to_owned(),
            )
        })?;
        if terminal_applies.next().is_some() {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip finality names multiple terminal Decision Apply rows".to_owned(),
            ));
        }
        let apply_ordinal = apply.ordinal();
        let validate_ordinal = apply_ordinal.checked_sub(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal Decision Apply has no Validate predecessor".to_owned(),
            )
        })?;
        let store_ordinal = validate_ordinal.checked_sub(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal Decision Apply has no Store predecessor".to_owned(),
            )
        })?;
        let fetch_ordinal = store_ordinal.checked_sub(1).ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "terminal Decision Apply has no Fetch predecessor".to_owned(),
            )
        })?;
        let record_at = |ordinal| {
            self.records
                .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
                .ok()
                .and_then(|index| self.records.get(index))
        };
        let (Some(fetch), Some(store), Some(validate)) = (
            record_at(fetch_ordinal),
            record_at(store_ordinal),
            record_at(validate_ordinal),
        ) else {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal CompleteTip lifecycle body chain is incomplete".to_owned(),
            ));
        };
        let owner = apply.owner();
        if fetch.owner() != owner
            || store.owner() != owner
            || validate.owner() != owner
            || owner.first_admission_ordinal() != fetch_ordinal
            || [fetch, store, validate, apply]
                .iter()
                .any(|record| record.reconstruction_source() != owner.causal_root().digest())
            || self
                .records
                .iter()
                .filter(|record| record.owner() == owner)
                .count()
                != 4
            || fetch.work_class() != Some(LifecycleWorkClass::Fetch)
            || store.work_class() != Some(LifecycleWorkClass::Store)
            || validate.work_class() != Some(LifecycleWorkClass::Validate)
            || fetch.terminal() != Some(Some(TerminalOutcome::Advanced))
            || store.terminal() != Some(Some(TerminalOutcome::Advanced))
            || validate.terminal() != Some(Some(TerminalOutcome::Advanced))
            || fetch.continuation()
                != Some(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    store_ordinal,
                ))
            || store.continuation()
                != Some(DurableContinuation::successor(
                    DurableContinuationEdge::StoreToValidate,
                    validate_ordinal,
                ))
            || validate.continuation()
                != Some(DurableContinuation::successor(
                    DurableContinuationEdge::ValidateToApply,
                    apply_ordinal,
                ))
            || recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::FetchToStore,
                &fetch.replay_authority,
                fetch
                    .durable_payload()
                    .expect("validated ledger Fetch payload"),
                &store.replay_authority,
                store
                    .durable_payload()
                    .expect("validated ledger Store payload"),
            ) != Some(true)
            || recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::StoreToValidate,
                &store.replay_authority,
                store
                    .durable_payload()
                    .expect("validated ledger Store payload"),
                &validate.replay_authority,
                validate
                    .durable_payload()
                    .expect("validated ledger Validate payload"),
            ) != Some(true)
            || recovered_decision_body_continuation_is_exact(
                DurableContinuationEdge::ValidateToApply,
                &validate.replay_authority,
                validate
                    .durable_payload()
                    .expect("validated ledger Validate payload"),
                &apply.replay_authority,
                apply
                    .durable_payload()
                    .expect("validated ledger Apply payload"),
            ) != Some(true)
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "terminal CompleteTip lifecycle body chain changed exact durable semantics"
                    .to_owned(),
            ));
        }
        Ok(apply_ordinal)
    }

    /// Consume the exact opened predecessor store, frame, and CompleteTip proof
    /// into one non-decomposable authentication cut.
    ///
    /// The store is reloaded both before and after the terminal-chain join.
    /// Every failure consumes all three inputs and requires startup to restart;
    /// no caller can recover the CompleteTip activation or substitute a
    /// detached same-byte frame for the retained opened store handle.
    fn into_complete_tip_terminal_apply_store_join(
        self,
        ledger_store: LifecycleLedgerStoreV1,
        complete_tip: crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> Result<AuthenticatedCompleteTipTerminalApplyStoreJoinV1, LifecycleLedgerError> {
        if !ledger_store.is_authorized_complete_tip_predecessor_target(&complete_tip)
            || ledger_store.context != self.context()
            || ledger_store.load()? != self
        {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip predecessor store target or frame changed before authentication"
                    .to_owned(),
            ));
        }
        let apply_ordinal = self.authenticate_complete_tip_terminal_apply(&complete_tip)?;
        let cut = AuthenticatedCompleteTipTerminalApplyStoreJoinV1 {
            complete_tip,
            ledger_store,
            ledger: self,
            apply_ordinal,
        };
        if !cut.is_exact()? {
            return Err(LifecycleLedgerError::InvalidLedger(
                "CompleteTip predecessor cut changed during authentication".to_owned(),
            ));
        }
        Ok(cut)
    }

    /// Purely stage one adapter-authenticated WAL-ahead Validate-to-Sign repair.
    ///
    /// The only mutable shape is an exact live Validate parent with no child.
    /// It becomes `Advanced` and names a newly appended, same-owner Sign row at
    /// `high_water + 1`. An already repaired exact pair stutters. Every other
    /// parent/child arrangement fails before the unified startup transaction
    /// can persist the returned ledger.
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
                    let parent_payload = record
                        .durable_payload()
                        .expect("records validated before successor edges");
                    let successor_payload = successor
                        .durable_payload()
                        .expect("records validated before successor edges");
                    let payload_and_replay_are_exact =
                        recovered_decision_body_continuation_is_exact(
                            edge,
                            &record.replay_authority,
                            parent_payload,
                            &successor.replay_authority,
                            successor_payload,
                        )
                        .unwrap_or_else(|| {
                            durable_continuation_payload_is_exact(
                                edge,
                                parent_payload,
                                successor_payload,
                            )
                        });
                    successor.owner() != record.owner()
                        || successor.reconstruction_source() != record.reconstruction_source()
                        || !payload_and_replay_are_exact
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

/// Startup-fatal failure from the consuming LedgerV1/body-store Ready-Fetch join.
///
/// The failing call has consumed the opened ledger and body-store instances.
/// Recovery must abort this startup attempt; this reason is diagnostic only and
/// carries no retry, parts, or authority-recovery surface.
#[derive(Debug, Error)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(super) enum DurableCertifiedFetchRecoveryError {
    /// The opened ledger is not owned by the supplied authenticated height.
    #[error("durable Certified Fetch ledger context is not the verified height context")]
    InvalidVerifiedContext,
    /// The consumed ledger store is foreign or no longer loads this frame.
    #[error("durable Certified Fetch ledger store does not own the opened frame")]
    InvalidLedgerStore,
    /// The opened body-store instance is not owned by the verified height.
    #[error("durable Certified Fetch body store is not the verified height context")]
    InvalidBodyStoreContext,
    /// The selected row is not the live BodyFrame-backed Fetch shape.
    #[error("durable Certified Fetch ledger row is not recoverable")]
    InvalidLedgerRow,
    /// The exact body frame could not be authenticated in the opened store.
    #[error(transparent)]
    BodyFrame(#[from] DurableBodyFrameRecoveryError),
    /// The row replay family and authenticated body frame do not form one cut.
    #[error("durable Certified Fetch replay join is inconsistent")]
    InvalidReplayJoin,
    /// Two live rows collide in owner, address, completion, or body identity.
    #[error("durable Certified Fetch recovery census is ambiguous")]
    AmbiguousCensus,
    /// The sealed census no longer matches all of its retained storage owners.
    #[error("durable Certified Fetch storage recovery cut is inconsistent")]
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

/// Typed LifecycleLedgerV1 load or persistence failure.
#[derive(Debug, Error)]
pub(in crate::sumeragi) enum LifecycleLedgerError {
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
pub(in crate::sumeragi) struct LifecycleLedgerStoreV1 {
    path: PathBuf,
    context: LifecycleContext,
    max_records: usize,
    max_frame_bytes: u64,
}

impl LifecycleLedgerStoreV1 {
    fn is_authorized_complete_tip_predecessor_target(
        &self,
        complete_tip: &crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
    ) -> bool {
        self.path.parent().is_some_and(|root| {
            complete_tip.authorizes_predecessor_lifecycle_root(root)
                && self.path == root.join(LEDGER_FILE)
        })
    }

    /// Compare the complete immutable publication target of two open handles.
    pub(super) fn same_publication_target(&self, other: &Self) -> bool {
        self.path == other.path
            && self.context == other.context
            && self.max_records == other.max_records
            && self.max_frame_bytes == other.max_frame_bytes
    }

    /// Open a height-local ledger under the coordinator's sealed size bounds.
    pub(in crate::sumeragi) fn open(
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

    pub(super) fn load(&self) -> Result<LifecycleLedgerV1, LifecycleLedgerError> {
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

    /// Persist one exact staged successor only while the attached frame still
    /// equals the coordinator state from which it was derived.
    ///
    /// The equality read happens before any atomic replacement begins. An
    /// exact stutter confirms the already-fsynced frame without rewriting it;
    /// otherwise a successful return means `successor` is the exact fsynced V1
    /// frame replacing `current`. Callers may perform only infallible in-memory
    /// publication after this method returns.
    pub(super) fn persist_exact_successor(
        &self,
        current: &LifecycleLedgerV1,
        successor: &LifecycleLedgerV1,
    ) -> Result<(), LifecycleLedgerError> {
        if self.load()? != *current {
            return Err(LifecycleLedgerError::InvalidLedger(
                "attached lifecycle ledger changed before successor publication".to_owned(),
            ));
        }
        if current == successor {
            return Ok(());
        }
        self.persist(successor)
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

    /// Reopen and compare the complete exact control-Sign row without exposing it.
    pub(super) fn revalidates_authenticated_wal_control_sign(
        &self,
        projection: &AuthenticatedRecoveredWalControlProjection,
        ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        let Ok((staged, observed_ordinal, changed)) =
            loaded.stage_authenticated_wal_control_sign(projection)
        else {
            return false;
        };
        !changed
            && observed_ordinal == ordinal
            && staged == loaded
            && projection.exactly_matches_ledger_at(&loaded, ordinal)
    }

    /// Reopen and compare one already-fsynced Decision Fetch row.
    pub(super) fn revalidates_authenticated_wal_decision_fetch(
        &self,
        projection: &AuthenticatedRecoveredWalDecisionFetchProjection,
        ordinal: u128,
    ) -> bool {
        let Ok(loaded) = self.load() else {
            return false;
        };
        let Ok((staged, observed_ordinal, changed)) =
            loaded.stage_authenticated_wal_decision_fetch(projection)
        else {
            return false;
        };
        !changed
            && observed_ordinal == ordinal
            && staged == loaded
            && projection.exactly_matches_ledger_at(&loaded, ordinal)
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

    /// Fsync one staged successor against this coordinator's exact attached
    /// LedgerV1 frame.
    ///
    /// Unlike the generic durable helper, this first-release transaction never
    /// accepts an in-memory-only coordinator. The staged copy must retain the
    /// same store identity, and the on-disk frame must still equal the live
    /// coordinator projection before it can be replaced.
    pub(super) fn persist_exact_staged_successor(
        &self,
        staged: &Self,
    ) -> Result<(), LifecycleLedgerError> {
        let store = self.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "live lifecycle publication requires an attached LedgerV1 store".to_owned(),
            )
        })?;
        let staged_store = staged.ledger_store.as_ref().ok_or_else(|| {
            LifecycleLedgerError::InvalidLedger(
                "staged lifecycle successor lost its attached LedgerV1 store".to_owned(),
            )
        })?;
        if !store.same_publication_target(staged_store) {
            return Err(LifecycleLedgerError::InvalidLedger(
                "staged lifecycle successor changed its LedgerV1 store".to_owned(),
            ));
        }
        let current = LifecycleLedgerV1::from_coordinator(self)?;
        let successor = LifecycleLedgerV1::from_coordinator(staged)?;
        store.persist_exact_successor(&current, &successor)
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

/// Substitute one structurally valid but foreign control replay authority in a test frame.
#[cfg(test)]
pub(crate) fn substitute_recovered_control_replay_authority_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let controls = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = controls.as_slice() else {
        return false;
    };
    let Some(foreign) = ledger.records[*index]
        .replay_authority
        .with_foreign_origin_generation_for_test()
    else {
        return false;
    };
    ledger.records[*index].replay_authority = foreign;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}

/// Substitute a structurally valid foreign replay origin on the WAL Decision Fetch row.
#[cfg(test)]
pub(crate) fn substitute_recovered_decision_fetch_replay_authority_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let fetches = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.durable_payload() == Some(DurablePayloadReference::None))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = fetches.as_slice() else {
        return false;
    };
    let Some(foreign) = ledger.records[*index]
        .replay_authority
        .with_foreign_origin_generation_for_test()
    else {
        return false;
    };
    ledger.records[*index].replay_authority = foreign;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}

/// Substitute a valid foreign owner while retaining the exact Decision Fetch key.
#[cfg(test)]
pub(crate) fn substitute_recovered_decision_fetch_owner_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let fetches = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| {
            (record.work_class() == Some(LifecycleWorkClass::Fetch)
                && record.durable_payload() == Some(DurablePayloadReference::None))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let [index] = fetches.as_slice() else {
        return false;
    };
    let ordinal = ledger.records[*index].ordinal;
    let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xDF; 32])), ordinal);
    ledger.records[*index].causal_root = *owner.causal_root().digest().as_bytes();
    ledger.records[*index].owner_first_ordinal = owner.first_admission_ordinal();
    ledger.records[*index].reconstruction_source = *owner.causal_root().digest().as_bytes();
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}

/// Append a valid foreign terminal row which aliases the control row's owner.
#[cfg(test)]
pub(crate) fn append_same_owner_foreign_terminal_for_test(
    root: &Path,
    context: LifecycleContext,
) -> bool {
    let Ok((store, mut ledger)) = LifecycleLedgerStoreV1::open(root, context) else {
        return false;
    };
    let controls = ledger
        .records
        .iter()
        .filter(|record| {
            matches!(
                record.work_class(),
                Some(LifecycleWorkClass::SignProposal | LifecycleWorkClass::SignTimeout)
            )
        })
        .collect::<Vec<_>>();
    let [control] = controls.as_slice() else {
        return false;
    };
    let owner = control.owner();
    let Some(ordinal) = ledger.high_water.checked_add(1) else {
        return false;
    };
    let foreign = super::replay_authority::exact_record_fixture(
        context,
        LifecycleStageKind::ReportProposalEquivocation,
        0x7F,
    );
    let Ok(terminal) = LifecycleLedgerRecordV1::new(
        foreign.key,
        owner,
        ordinal,
        foreign.work_class,
        foreign.stage,
        Some(TerminalOutcome::Cancelled),
        owner.causal_root().digest(),
        foreign.payload,
        foreign.authority,
        DurableContinuation::None,
    ) else {
        return false;
    };
    ledger.records.push(terminal);
    ledger.high_water = ordinal;
    ledger.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT).is_ok() && store.persist(&ledger).is_ok()
}

#[cfg(test)]
/// Ledger-local behavior and source-surface regressions.
pub(crate) mod tests {
    use super::*;

    #[test]
    fn production_owner_parent_surface_is_declaration_only() {
        let source = include_str!("v2_lifecycle_coordinator.rs");
        let declaration = source
            .split_once("// PRODUCTION_LIFECYCLE_OWNER_DECLARATION_BEGIN")
            .expect("production owner declaration begin marker")
            .1
            .split_once("// PRODUCTION_LIFECYCLE_OWNER_DECLARATION_END")
            .expect("production owner declaration end marker")
            .0;
        assert!(declaration.lines().count() <= 25);
        assert!(!declaration.contains("fn "));
        for retained in [
            "VerifiedHeightContext",
            "LifecycleCoordinator",
            "LifecycleWorkRegistryHolder",
            "CertifiedServePayloadStoreV1",
            "AuthenticatedCertifiedServePayloadRecoveryCut",
            "V2BodyStore",
            "ProductionLifecycleAdapterStartupV1",
        ] {
            assert!(declaration.contains(retained), "owner dropped {retained}");
        }
    }

    #[cfg(feature = "bls")]
    /// BLS-backed storage-recovery and CompleteTip retirement regressions.
    pub(crate) mod durable_ready_fetch_recovery {
        use std::{collections::BTreeMap, fs, num::NonZeroU64, path::Path};

        use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
        use iroha_data_model::{
            block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
            peer::PeerId,
        };
        use tempfile::TempDir;

        use super::*;
        use crate::{
            kura::Kura,
            sumeragi::{
                v2_body_store::ValidatedBodyReceipt,
                v2_core::{EventTag, Generation},
                v2_transport::{
                    AuthenticatedCertifiedBodyRequest, authenticate_certified_body_request,
                },
            },
        };

        struct RecoveryFixture {
            verified: VerifiedHeightContext,
            keys: Vec<KeyPair>,
        }

        #[derive(Clone, Copy)]
        enum ServeTerminalFixture {
            Completed,
            Negative(
                crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome,
            ),
        }

        #[derive(Clone, Copy)]
        enum StagedTerminalDrift {
            Record,
            Index,
            Debt,
            Capacity,
            HighWater,
        }

        fn snapshot_files(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
            fn visit(
                root: &Path,
                directory: &Path,
                snapshot: &mut BTreeMap<std::path::PathBuf, Vec<u8>>,
            ) {
                let mut entries = fs::read_dir(directory)
                    .expect("read startup no-mutation fixture directory")
                    .collect::<Result<Vec<_>, _>>()
                    .expect("decode startup no-mutation directory entries");
                entries.sort_by_key(fs::DirEntry::path);
                for entry in entries {
                    let path = entry.path();
                    if path.is_dir() {
                        visit(root, &path, snapshot);
                    } else {
                        let relative = path
                            .strip_prefix(root)
                            .expect("snapshot path remains under fixture root")
                            .to_path_buf();
                        assert!(
                            snapshot
                                .insert(
                                    relative,
                                    fs::read(path).expect("read startup fixture file")
                                )
                                .is_none()
                        );
                    }
                }
            }
            let mut snapshot = BTreeMap::new();
            visit(root, root, &mut snapshot);
            snapshot
        }

        impl RecoveryFixture {
            fn new(network: &str, first_seed: u8) -> Self {
                let mut keys = (first_seed..first_seed + 4)
                    .map(|seed| {
                        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                            .expect("deterministic durable Ready-Fetch BLS key")
                    })
                    .collect::<Vec<_>>();
                keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
                let proofs = keys
                    .iter()
                    .map(|key| {
                        iroha_crypto::bls_normal_pop_prove(key.private_key())
                            .expect("durable Ready-Fetch proof of possession")
                    })
                    .collect::<Vec<_>>();
                let roster = keys
                    .iter()
                    .map(|key| wire::ValidatorPower {
                        validator: PeerId::new(key.public_key().clone()),
                        power: 1,
                    })
                    .collect::<Vec<_>>();
                let context = wire::HeightContext {
                    network_id: crate::sumeragi::synthetic_network_id(network),
                    protocol_version: wire::PROTOCOL_VERSION,
                    height: 1,
                    epoch: 1,
                    epoch_end_height: 100,
                    next_epoch_snapshot: None,
                    mode: wire::ConsensusMode::Permissioned,
                    parent_commit_qc: None,
                    snapshot_bootstrap: None,
                    quorum: wire::DualQuorum::from_roster(&roster)
                        .expect("four-validator durable Ready-Fetch quorum"),
                    roster,
                    nexus_amx_context_hash: Hash::new(b"durable Ready-Fetch nexus context"),
                    execution_policy_hash: Hash::new(b"durable Ready-Fetch execution policy"),
                    da_layout: wire::DataAvailabilityLayout {
                        encoding: wire::PayloadEncoding::ReedSolomon16,
                        chunk_size_bytes: 1024,
                        data_shards: 1,
                        parity_shards: 1,
                        max_payload_size_bytes: 512 * 1024,
                        max_chunk_count: 1024,
                    },
                    leader_seed: [0xA5; 32],
                };
                let verified = VerifiedHeightContext::genesis(context, proofs)
                    .expect("verified durable Ready-Fetch height context");
                Self { verified, keys }
            }

            fn lifecycle_context(&self) -> LifecycleContext {
                projection::lifecycle_context(self.verified.context())
            }

            fn open_store(&self, directory: &TempDir) -> V2BodyStore {
                V2BodyStore::open(directory.path(), self.verified.context().clone())
                    .expect("open durable Ready-Fetch body store")
            }

            #[allow(clippy::too_many_lines)]
            fn fetch_record(
                &self,
                store: &mut V2BodyStore,
                view: u64,
                marker: u8,
                ordinal: u128,
                certified_sources: Option<Vec<PeerId>>,
                corrupt_qc: bool,
            ) -> LifecycleLedgerRecordV1 {
                let context = self.verified.context();
                let round = wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view,
                };
                let leader = context.leader(view);
                let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
                let header = BlockHeader::new(
                    NonZeroU64::new(context.height).expect("fixture height is non-zero"),
                    None,
                    None,
                    None,
                    1_000 + u64::from(marker),
                    view,
                );
                let block_signature = SignatureOf::try_from_hash(
                    self.keys[leader_index].private_key(),
                    header.hash(),
                )
                .expect("sign durable Ready-Fetch block");
                let block = SignedBlock::presigned(
                    BlockSignature::new(u64::from(leader), block_signature),
                    header,
                    Vec::new(),
                );
                let body = block
                    .encode_wire()
                    .expect("encode canonical SignedBlockWire");
                let subject = wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: block.hash(),
                    payload_hash: Hash::new(&body),
                };
                let chunks = wire::encode_payload_chunks(context.da_layout, &body)
                    .expect("encode durable Ready-Fetch chunks");
                let manifest = wire::PayloadManifest::derive(
                    context,
                    round,
                    subject,
                    u64::try_from(body.len()).expect("fixture body length fits u64"),
                    &chunks,
                )
                .expect("derive durable Ready-Fetch manifest");
                let receipt = store
                    .store(manifest.clone(), body)
                    .expect("fsync durable Ready-Fetch body");
                let execution_commitment =
                    wire::ExecutionCommitment::without_topups_or_merge_carrier(
                        Hash::new([marker, 1]),
                        Hash::new([marker, 2]),
                        Hash::new([marker, 3]),
                        1,
                        Hash::new([marker, 4]),
                    );
                let signers = vec![0, 1, 2];
                let preimage = wire::Vote {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment,
                    signer: 0,
                    signature: Vec::new(),
                }
                .signature_preimage();
                let shares = signers
                    .iter()
                    .map(|signer| {
                        Signature::new(
                            self.keys[usize::try_from(*signer).expect("fixture signer fits usize")]
                                .private_key(),
                            &preimage,
                        )
                        .payload()
                        .to_vec()
                    })
                    .collect::<Vec<_>>();
                let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let mut certificate = wire::QuorumCertificate {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment,
                    signers,
                    aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                        .expect("aggregate durable Ready-Fetch PrepareQC"),
                };
                if corrupt_qc {
                    certificate.aggregate_signature[0] ^= 1;
                }
                let sources = certified_sources.unwrap_or_else(|| {
                    context
                        .roster
                        .iter()
                        .map(|entry| entry.validator.clone())
                        .collect()
                });
                let case = super::super::super::replay_authority::exact_durable_certified_fetch_record_fixture(
                    self.lifecycle_context(),
                    EventTag::new(context.height, view, Generation::new(u64::from(marker))),
                    certificate,
                    manifest,
                    sources,
                    &receipt,
                );
                let causal_root =
                    CausalRoot::new(LifecycleDigest::new(*Hash::new([marker, 0xF0]).as_ref()));
                let owner = OwnerId::new(causal_root, ordinal);
                LifecycleLedgerRecordV1::new(
                    case.key,
                    owner,
                    ordinal,
                    LifecycleWorkClass::Fetch,
                    LifecycleStage::new(
                        LifecycleStageKind::FetchBody,
                        PredecessorScope::Independent,
                    ),
                    None,
                    causal_root.digest(),
                    case.payload,
                    case.authority,
                    DurableContinuation::None,
                )
                .expect("construct durable Ready-Fetch LedgerV1 row")
            }

            fn terminal_validate_record(
                &self,
                store: &mut V2BodyStore,
                view: u64,
                marker: u8,
                ordinal: u128,
            ) -> LifecycleLedgerRecordV1 {
                let context = self.verified.context();
                let round = wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view,
                };
                let leader = context.leader(view);
                let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
                let header = BlockHeader::new(
                    NonZeroU64::new(context.height).expect("fixture height is non-zero"),
                    None,
                    None,
                    None,
                    2_000 + u64::from(marker),
                    view,
                );
                let signature = SignatureOf::try_from_hash(
                    self.keys[leader_index].private_key(),
                    header.hash(),
                )
                .expect("sign terminal Validate block");
                let block = SignedBlock::presigned(
                    BlockSignature::new(u64::from(leader), signature),
                    header,
                    Vec::new(),
                );
                let body = block
                    .encode_wire()
                    .expect("encode terminal Validate SignedBlockWire");
                let subject = wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: block.hash(),
                    payload_hash: Hash::new(&body),
                };
                let chunks = wire::encode_payload_chunks(context.da_layout, &body)
                    .expect("encode terminal Validate chunks");
                let manifest = wire::PayloadManifest::derive(
                    context,
                    round,
                    subject,
                    u64::try_from(body.len()).expect("fixture body length fits u64"),
                    &chunks,
                )
                .expect("derive terminal Validate manifest");
                let receipt = store
                    .store(manifest.clone(), body)
                    .expect("fsync terminal Validate body");
                let replay =
                    super::super::super::replay_authority::exact_local_body_record_fixture(
                        self.lifecycle_context(),
                        EventTag::new(context.height, view, Generation::new(u64::from(marker))),
                        manifest,
                        &receipt,
                        LifecycleStageKind::ValidateBody,
                    )
                    .expect("project exact terminal Validate replay row");
                let commitment =
                    ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
                let _validated = store
                    .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                        Ok::<_, String>(commitment)
                    })
                    .expect("persist terminal Validate success outcome");
                let causal_root = CausalRoot::new(LifecycleDigest::new([marker; 32]));
                LifecycleLedgerRecordV1::new(
                    replay.key,
                    OwnerId::new(causal_root, ordinal),
                    ordinal,
                    replay.work_class,
                    replay.stage,
                    Some(TerminalOutcome::Advanced),
                    causal_root.digest(),
                    replay.payload,
                    replay.authority,
                    DurableContinuation::AdvancedNoSuccessor,
                )
                .expect("construct terminal Validate ledger row")
            }

            fn authenticated_serve_request(
                &self,
                view: u64,
                marker: u8,
                requester_index: usize,
            ) -> AuthenticatedCertifiedBodyRequest {
                let subject = wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new([
                        marker, 0xA1,
                    ])),
                    payload_hash: Hash::new([marker, 0xA2]),
                };
                self.authenticated_serve_request_for_subject(view, marker, requester_index, subject)
            }

            fn authenticated_serve_request_for_subject(
                &self,
                view: u64,
                marker: u8,
                requester_index: usize,
                subject: wire::BlockSubject,
            ) -> AuthenticatedCertifiedBodyRequest {
                let context = self.verified.context();
                let round = wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view,
                };
                let execution_commitment =
                    wire::ExecutionCommitment::without_topups_or_merge_carrier(
                        Hash::new([marker, 0xB1]),
                        Hash::new([marker, 0xB2]),
                        Hash::new([marker, 0xB3]),
                        1,
                        Hash::new([marker, 0xB4]),
                    );
                let signers = vec![0, 1, 2];
                let preimage = wire::Vote {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment,
                    signer: 0,
                    signature: Vec::new(),
                }
                .signature_preimage();
                let shares = signers
                    .iter()
                    .map(|signer| {
                        Signature::new(
                            self.keys[usize::try_from(*signer).expect("fixture signer fits usize")]
                                .private_key(),
                            &preimage,
                        )
                        .payload()
                        .to_vec()
                    })
                    .collect::<Vec<_>>();
                let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
                let certificate = wire::QuorumCertificate {
                    round,
                    proposal_round: round,
                    phase: wire::GlobalPhase::Prepare,
                    subject,
                    execution_commitment,
                    signers,
                    aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                        .expect("aggregate Certified-Serve PrepareQC"),
                };
                let mut request = wire::CertifiedBodyRequest {
                    round,
                    subject,
                    certificate,
                    requester: PeerId::new(self.keys[requester_index].public_key().clone()),
                    signature: Vec::new(),
                };
                request.signature = Signature::new(
                    self.keys[requester_index].private_key(),
                    &request.signature_preimage(),
                )
                .payload()
                .to_vec();
                let requester = request.requester.clone();
                authenticate_certified_body_request(context, request, &requester, |context, qc| {
                    wire::finality::verify_quorum_certificate_with_validator_pops(
                        context,
                        qc,
                        self.verified.proofs_of_possession(),
                    )
                    .map_err(|error| error.to_string())
                })
                .expect("authenticate Certified-Serve request")
            }

            fn completed_serve_exchange(
                &self,
                store: &mut V2BodyStore,
                view: u64,
                marker: u8,
                requester_index: usize,
            ) -> (
                AuthenticatedCertifiedBodyRequest,
                crate::sumeragi::v2_body_store::DurableBodyReceipt,
                wire::CertifiedBodyResponse,
            ) {
                let context = self.verified.context();
                let round = wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view,
                };
                let leader = context.leader(view);
                let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
                let header = BlockHeader::new(
                    NonZeroU64::new(context.height).expect("fixture height is non-zero"),
                    None,
                    None,
                    None,
                    3_000 + u64::from(marker),
                    view,
                );
                let signature = SignatureOf::try_from_hash(
                    self.keys[leader_index].private_key(),
                    header.hash(),
                )
                .expect("sign completed Serve block");
                let block = SignedBlock::presigned(
                    BlockSignature::new(u64::from(leader), signature),
                    header,
                    Vec::new(),
                );
                let body = block
                    .encode_wire()
                    .expect("encode completed Serve SignedBlockWire");
                let subject = wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: block.hash(),
                    payload_hash: Hash::new(&body),
                };
                let chunks = wire::encode_payload_chunks(context.da_layout, &body)
                    .expect("encode completed Serve chunks");
                let manifest = wire::PayloadManifest::derive(
                    context,
                    round,
                    subject,
                    u64::try_from(body.len()).expect("fixture body length fits u64"),
                    &chunks,
                )
                .expect("derive completed Serve manifest");
                let durable_body = store
                    .store(manifest.clone(), body.clone())
                    .expect("fsync completed Serve body");
                let request = self.authenticated_serve_request_for_subject(
                    view,
                    marker,
                    requester_index,
                    subject,
                );
                let responder = 0;
                let mut response = wire::CertifiedBodyResponse {
                    request_hash: request.request_hash(),
                    manifest,
                    body,
                    responder,
                    signature: Vec::new(),
                };
                response.signature = Signature::new(
                    self.keys[usize::try_from(responder).expect("small responder")].private_key(),
                    &response.signature_preimage(),
                )
                .payload()
                .to_vec();
                (request, durable_body, response)
            }

            fn ledger(&self, records: Vec<LifecycleLedgerRecordV1>) -> LifecycleLedgerV1 {
                let high_water = records
                    .iter()
                    .map(LifecycleLedgerRecordV1::ordinal)
                    .max()
                    .unwrap_or(0);
                LifecycleLedgerV1::new(
                    self.lifecycle_context(),
                    high_water,
                    records,
                    BTreeMap::new(),
                )
                .expect("construct durable Ready-Fetch LedgerV1")
            }

            fn persist_ledger(
                &self,
                directory: &TempDir,
                ledger: &LifecycleLedgerV1,
            ) -> LifecycleLedgerStoreV1 {
                let (store, opened) =
                    LifecycleLedgerStoreV1::open(directory.path(), self.lifecycle_context())
                        .expect("open durable Ready-Fetch lifecycle ledger store");
                assert!(opened.records().is_empty());
                store
                    .persist(ledger)
                    .expect("persist durable Ready-Fetch lifecycle ledger");
                store
            }

            fn open_empty_serve_payloads(
                &self,
                directory: &TempDir,
                body_store: &V2BodyStore,
            ) -> (
                CertifiedServePayloadStoreV1,
                AuthenticatedCertifiedServePayloadRecoveryCut,
            ) {
                let (store, recovered) =
                    CertifiedServePayloadStoreV1::open(directory.path(), self.verified.context())
                        .expect("open empty Certified-Serve payload store");
                let authenticated = recovered
                    .authenticate(&self.verified, &self.keys[0], body_store)
                    .expect("authenticate empty Certified-Serve payload recovery");
                (store, authenticated)
            }

            fn open_empty_owner(
                &self,
                body_directory: &TempDir,
                payload_directory: &TempDir,
                ledger_directory: &TempDir,
            ) -> ProductionLifecycleOwnerV1 {
                let body_store = self.open_store(body_directory);
                let (payload_store, payloads) =
                    self.open_empty_serve_payloads(payload_directory, &body_store);
                let ledger = self.ledger(Vec::new());
                let ledger_store = self.persist_ledger(ledger_directory, &ledger);
                let cut = ledger
                    .into_durable_certified_fetch_storage_recovery_cut(
                        self.verified.clone(),
                        ledger_store,
                        body_store,
                    )
                    .expect("seal empty fresh-admission storage cut");
                cut.open_owner_for_test(payload_store, payloads)
                    .expect("open empty fresh-admission production owner")
            }

            fn open_completed_serve_owner(
                &self,
                body_directory: &TempDir,
                payload_directory: &TempDir,
                ledger_directory: &TempDir,
            ) -> (
                ProductionLifecycleOwnerV1,
                AuthenticatedCertifiedBodyRequest,
                crate::sumeragi::v2_body_store::DurableBodyReceipt,
                wire::CertifiedBodyResponse,
            ) {
                let mut body_store = self.open_store(body_directory);
                let (request, durable_body, response) =
                    self.completed_serve_exchange(&mut body_store, 0, 0xC1, 3);
                let (payload_store, payloads) =
                    self.open_empty_serve_payloads(payload_directory, &body_store);
                let ledger = self.ledger(Vec::new());
                let ledger_store = self.persist_ledger(ledger_directory, &ledger);
                let cut = ledger
                    .into_durable_certified_fetch_storage_recovery_cut(
                        self.verified.clone(),
                        ledger_store,
                        body_store,
                    )
                    .expect("seal completed-Serve production storage cut");
                let owner = cut
                    .open_owner_for_test(payload_store, payloads)
                    .expect("open completed-Serve production owner");
                (owner, request, durable_body, response)
            }

            fn open_terminal_serve_owner(
                &self,
                body_directory: &TempDir,
                payload_directory: &TempDir,
                ledger_directory: &TempDir,
                terminal: ServeTerminalFixture,
            ) -> (
                ProductionLifecycleOwnerV1,
                AuthenticatedCertifiedBodyRequest,
            ) {
                let mut body_store = self.open_store(body_directory);
                let (request, response) = match terminal {
                    ServeTerminalFixture::Completed => {
                        let (request, _durable_body, response) =
                            self.completed_serve_exchange(&mut body_store, 0, 0xD1, 3);
                        (request, Some(response))
                    }
                    ServeTerminalFixture::Negative(_) => {
                        (self.authenticated_serve_request(0, 0xD2, 3), None)
                    }
                };
                let (mut payload_store, recovery) = CertifiedServePayloadStoreV1::open(
                    payload_directory.path(),
                    self.verified.context(),
                )
                .expect("open terminal Serve payload store");
                assert!(recovery.is_empty());
                let pending = payload_store
                    .persist_pending_with_verified_retention(
                        &self.verified,
                        &self.keys[0],
                        &request,
                    )
                    .expect("persist terminal Serve Pending frame");
                let authority =
                    authority::lifecycle_storage_owner_test_authority(&self.verified, 1, 1)
                        .expect("construct terminal Serve lifecycle authority");
                let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
                assert!(matches!(
                    coordinator
                        .admit_certified_serve(&self.verified, &request, pending)
                        .expect("project terminal Serve request"),
                    super::super::super::AdmissionDecision::Admitted { .. }
                ));
                let ready = coordinator.ready_index.iter().map(|ordinal| {
                    let record = &coordinator.records[ordinal];
                    (
                        *ordinal,
                        super::super::super::SchedulerReadyInputs::new(record, None, [0; 6]),
                    )
                });
                let TurnPlan::Execute(lease) = coordinator.plan_turn(
                    super::super::super::SchedulerInputs::new([], ready)
                        .expect("terminal Serve has one exact Ready row"),
                ) else {
                    panic!("terminal Serve must own the selected turn")
                };
                match terminal {
                    ServeTerminalFixture::Completed => {
                        let response = response.expect("completed fixture retains response");
                        let completed = payload_store
                            .persist_completed(&request, &response)
                            .expect("persist completed Serve tombstone");
                        let producer = coordinator.producer_debts[&lease.ordinal];
                        let terminal =
                            CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
                                coordinator.active_context,
                                &coordinator.records[&lease.ordinal],
                                &coordinator.durable_records[&lease.ordinal],
                                &coordinator.records[&producer],
                                &coordinator.durable_records[&producer],
                                completed,
                            )
                            .expect("close completed Serve terminal family");
                        coordinator.reduce_settle_turn(
                            lease,
                            TurnOutcome::Terminal(terminal.terminal_outcome()),
                            Some(terminal),
                        );
                        assert_eq!(coordinator.fault(), None);
                    }
                    ServeTerminalFixture::Negative(outcome) => {
                        let negative = payload_store
                            .persist_negative(pending.id(), outcome)
                            .expect("persist negative Serve tombstone");
                        let producer = coordinator.producer_debts[&lease.ordinal];
                        let terminal =
                            CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
                                coordinator.active_context,
                                &coordinator.records[&lease.ordinal],
                                &coordinator.durable_records[&lease.ordinal],
                                &coordinator.records[&producer],
                                &coordinator.durable_records[&producer],
                                negative,
                            )
                            .expect("close negative Serve terminal family");
                        coordinator.reduce_settle_turn(
                            lease,
                            TurnOutcome::Terminal(terminal.terminal_outcome()),
                            Some(terminal),
                        );
                        assert_eq!(coordinator.fault(), None);
                    }
                }
                let ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                    .expect("project terminal Serve LedgerV1");
                let ledger_store = self.persist_ledger(ledger_directory, &ledger);
                drop(payload_store);
                let (payload_store, recovered) = CertifiedServePayloadStoreV1::open(
                    payload_directory.path(),
                    self.verified.context(),
                )
                .expect("reopen terminal Serve payload store");
                let payloads = recovered
                    .authenticate(&self.verified, &self.keys[0], &body_store)
                    .expect("authenticate terminal Serve payload");
                let cut = ledger
                    .into_durable_certified_fetch_storage_recovery_cut(
                        self.verified.clone(),
                        ledger_store,
                        body_store,
                    )
                    .expect("seal terminal Serve storage cut");
                let owner = cut
                    .open_owner_for_test(payload_store, payloads)
                    .expect("open terminal Serve production owner");
                (owner, request)
            }
        }

        /// Structural stand-in for the sealed adapter/WAL projection used only to exercise the
        /// ledger oracle's census behavior. It deliberately authorizes no runtime operation.
        struct TerminalDecisionProjectionFixture {
            context: LifecycleContext,
            fetch: LifecycleLedgerRecordV1,
            store: LifecycleLedgerRecordV1,
            validate: LifecycleLedgerRecordV1,
            apply: LifecycleLedgerRecordV1,
            subject: wire::BlockSubject,
            certificate: wire::QuorumCertificate,
        }

        impl TerminalRecoveredDecisionApplyProjectionV1 for TerminalDecisionProjectionFixture {
            fn belongs_to_context(&self, context: LifecycleContext) -> bool {
                self.context == context
            }

            fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
                record.key() == self.fetch.key()
            }

            fn exactly_matches_advanced_apply_parent(
                &self,
                fetch: &LifecycleLedgerRecordV1,
                store_ordinal: u128,
            ) -> bool {
                fetch == &self.fetch && store_ordinal == self.store.ordinal()
            }

            fn exactly_matches_terminal_successor_records(
                &self,
                owner: OwnerId,
                store: &LifecycleLedgerRecordV1,
                validate: &LifecycleLedgerRecordV1,
                apply: &LifecycleLedgerRecordV1,
            ) -> bool {
                owner == self.fetch.owner()
                    && store == &self.store
                    && validate == &self.validate
                    && apply == &self.apply
            }
        }

        fn terminal_decision_chain_fixture(
            fixture: &RecoveryFixture,
        ) -> (LifecycleLedgerV1, TerminalDecisionProjectionFixture) {
            terminal_decision_chain_fixture_with_seed(fixture, 0xE1)
        }

        fn terminal_decision_chain_fixture_with_seed(
            fixture: &RecoveryFixture,
            seed: u8,
        ) -> (LifecycleLedgerV1, TerminalDecisionProjectionFixture) {
            let context = fixture.lifecycle_context();
            let certified_sources = fixture
                .verified
                .context()
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect();
            let ([fetch_case, store_case, validate_case, apply_case], subject, certificate) =
                super::super::super::replay_authority::exact_recovered_decision_terminal_family_fixture(
                    context,
                    certified_sources,
                    seed,
                );
            assert_eq!(fetch_case.payload, DurablePayloadReference::None);
            let payload = store_case.payload;
            assert_eq!(validate_case.payload, payload);
            assert_eq!(apply_case.payload, payload);

            let causal_root = CausalRoot::new(LifecycleDigest::new(
                *Hash::new(b"terminal recovered Decision ledger fixture").as_ref(),
            ));
            let owner = OwnerId::new(causal_root, 1);
            let fetch = LifecycleLedgerRecordV1::new(
                fetch_case.key,
                owner,
                1,
                fetch_case.work_class,
                fetch_case.stage,
                Some(TerminalOutcome::Advanced),
                causal_root.digest(),
                fetch_case.payload,
                fetch_case.authority,
                DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
            )
            .expect("construct terminal Decision Fetch parent");
            let store = LifecycleLedgerRecordV1::new(
                store_case.key,
                owner,
                2,
                store_case.work_class,
                store_case.stage,
                Some(TerminalOutcome::Advanced),
                causal_root.digest(),
                store_case.payload,
                store_case.authority,
                DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 3),
            )
            .expect("construct terminal Decision Store row");
            let validate = LifecycleLedgerRecordV1::new(
                validate_case.key,
                owner,
                3,
                validate_case.work_class,
                validate_case.stage,
                Some(TerminalOutcome::Advanced),
                causal_root.digest(),
                validate_case.payload,
                validate_case.authority,
                DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 4),
            )
            .expect("construct terminal Decision Validate row");
            let apply = LifecycleLedgerRecordV1::new(
                apply_case.key,
                owner,
                4,
                apply_case.work_class,
                apply_case.stage,
                Some(TerminalOutcome::Advanced),
                causal_root.digest(),
                apply_case.payload,
                apply_case.authority,
                DurableContinuation::None,
            )
            .expect("construct terminal Decision Apply row");
            let projection = TerminalDecisionProjectionFixture {
                context,
                fetch: fetch.clone(),
                store: store.clone(),
                validate: validate.clone(),
                apply: apply.clone(),
                subject,
                certificate,
            };
            let ledger = LifecycleLedgerV1::new(
                context,
                4,
                vec![fetch, store, validate, apply],
                BTreeMap::new(),
            )
            .expect("construct exact terminal recovered Decision chain");
            (ledger, projection)
        }

        fn complete_tip_for_terminal_decision(
            fixture: &RecoveryFixture,
            projection: &TerminalDecisionProjectionFixture,
        ) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
            let artifact = wire::finality::V2FinalityArtifact::new(
                fixture.verified.context().clone(),
                projection.subject.clone(),
                projection.certificate.clone(),
                fixture.verified.proofs_of_possession().to_vec(),
            );
            let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
            let predecessor =
                crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
                    &artifact, &receipt,
                )
                .expect("terminal Decision finality and receipt authenticate");
            let successor_context_id = wire::HeightContextId(iroha_crypto::HashOf::<
                wire::HeightContext,
            >::from_untyped_unchecked(
                Hash::new(b"terminal Decision CompleteTip successor context"),
            ));
            let activation =
                crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
                    predecessor,
                    successor_context_id,
                );
            crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_test(
                artifact,
                receipt,
                successor_context_id,
                activation,
            )
            .expect("retain exact terminal Decision CompleteTip authority")
        }

        fn complete_tip_for_terminal_decision_at(
            fixture: &RecoveryFixture,
            projection: &TerminalDecisionProjectionFixture,
            predecessor_root: &Path,
        ) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
            let artifact = wire::finality::V2FinalityArtifact::new(
                fixture.verified.context().clone(),
                projection.subject.clone(),
                projection.certificate.clone(),
                fixture.verified.proofs_of_possession().to_vec(),
            );
            let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
            let predecessor =
                crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
                    &artifact, &receipt,
                )
                .expect("terminal Decision finality and receipt authenticate");
            let successor_context_id = wire::HeightContextId(iroha_crypto::HashOf::<
                wire::HeightContext,
            >::from_untyped_unchecked(
                Hash::new(b"terminal Decision CompleteTip successor context"),
            ));
            let activation =
                crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
                    predecessor,
                    successor_context_id,
                );
            crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_lifecycle_test(
                artifact,
                receipt,
                successor_context_id,
                activation,
                predecessor_root,
            )
            .expect("retain root-bound terminal Decision CompleteTip authority")
        }

        fn complete_tip_for_terminal_decision_on_kura(
            fixture: &RecoveryFixture,
            projection: &TerminalDecisionProjectionFixture,
            kura: &Kura,
        ) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
            let artifact = wire::finality::V2FinalityArtifact::new(
                fixture.verified.context().clone(),
                projection.subject.clone(),
                projection.certificate.clone(),
                fixture.verified.proofs_of_possession().to_vec(),
            );
            let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
            let predecessor =
                crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
                    &artifact, &receipt,
                )
                .expect("terminal Decision finality and receipt authenticate");
            let verified_successor = complete_tip_successor_fixture(fixture, projection);
            let successor_context_id = verified_successor.context().id();
            let activation =
                crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
                    predecessor,
                    successor_context_id,
                );
            crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_canonical_lifecycle_test(
                artifact,
                receipt,
                fixture.verified.clone(),
                crate::sumeragi::v2_body_store::BlockSignaturePolicy::RotatingLeader,
                successor_context_id,
                activation,
                kura,
            )
            .expect("retain Kura-bound terminal Decision CompleteTip authority")
        }

        fn complete_tip_successor_fixture(
            fixture: &RecoveryFixture,
            projection: &TerminalDecisionProjectionFixture,
        ) -> VerifiedHeightContext {
            let mut context = fixture.verified.context().clone();
            context.height = context
                .height
                .checked_add(1)
                .expect("fixture successor height is representable");
            context.parent_commit_qc = Some(projection.certificate.clone());
            context.snapshot_bootstrap = None;
            VerifiedHeightContext::successor_fixture_for_test(
                context,
                fixture.verified.proofs_of_possession().to_vec(),
                fixture.verified.context().clone(),
                fixture.verified.proofs_of_possession().to_vec(),
            )
        }

        /// Build one genuinely retired CompleteTip/H+1 pair for the runner's
        /// restart-activation boundary test.
        pub(crate) fn complete_tip_restart_activation_fixture() -> (
            std::sync::Arc<Kura>,
            std::path::PathBuf,
            wire::HeightContext,
            RetiredRecoveredCompleteTipActivationAuthorityV1,
        ) {
            let fixture = RecoveryFixture::new("complete-tip-runner-restart", 0x48);
            let (predecessor, projection) = terminal_decision_chain_fixture(&fixture);
            let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
            let successor_context = verified_successor.context().clone();
            let kura = Kura::blank_kura_for_testing();
            let predecessor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let (predecessor_store, empty) =
                LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
                    .expect("open canonical runner-restart predecessor");
            assert!(empty.records().is_empty());
            predecessor_store
                .persist(&predecessor)
                .expect("persist runner-restart terminal predecessor");
            let retirement =
                complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
                    .into_canonical_predecessor_storage(&fixture.keys[0])
                    .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
                    .expect("retire the exact runner-restart predecessor");
            (kura, predecessor_root, successor_context, retirement)
        }

        fn empty_successor_owner_for_complete_tip(
            retirement: &RetiredRecoveredCompleteTipActivationAuthorityV1,
            kura: &Kura,
            verified: VerifiedHeightContext,
            body_root: &Path,
            payload_root: &Path,
            ledger_store: LifecycleLedgerStoreV1,
        ) -> ProductionLifecycleOwnerV1 {
            assert!(retirement.successor_ledger.records().is_empty());
            let body_store = V2BodyStore::open_lifecycle_fixture_for_test(
                body_root,
                verified.context().clone(),
                BlockSignaturePolicy::RotatingLeader,
            )
            .expect("open fixture H+1 body owner");
            let (payload_store, serve_payloads) =
                CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
                    payload_root,
                    verified.context(),
                )
                .expect("open fixture H+1 Serve-payload owner");
            let authority = authority::lifecycle_storage_owner_test_authority(&verified, 0, 0)
                .expect("construct empty H+1 lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(
                authority,
                retirement.successor_ledger.high_water(),
            );
            coordinator.ledger_store = Some(ledger_store);
            ProductionLifecycleOwnerV1 {
                verified,
                coordinator,
                registry: LifecycleWorkRegistryHolder::empty(),
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: Some(RecoveredLifecycleOwnerKuraBindingV1::for_test(kura)),
                apply_service: None,
                adapter_startup: Some(ProductionLifecycleAdapterStartupV1::fixture_for_test()),
            }
        }

        fn unrelated_live_record(
            context: LifecycleContext,
            owner: OwnerId,
            ordinal: u128,
            seed: u8,
        ) -> LifecycleLedgerRecordV1 {
            let case = super::super::super::replay_authority::exact_record_fixture(
                context,
                LifecycleStageKind::SignPrepareVote,
                seed,
            );
            LifecycleLedgerRecordV1::new(
                case.key,
                owner,
                ordinal,
                case.work_class,
                case.stage,
                None,
                owner.causal_root().digest(),
                case.payload,
                case.authority,
                DurableContinuation::None,
            )
            .expect("construct unrelated live lifecycle row")
        }

        #[test]
        fn terminal_recovered_decision_oracle_accepts_the_exact_terminal_chain() {
            let fixture = RecoveryFixture::new("terminal-decision-exact", 0x31);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);

            assert_eq!(
                ledger
                    .authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .expect("authenticate exact terminal recovered Decision chain"),
                4
            );
        }

        #[test]
        fn complete_tip_terminal_join_binds_the_full_finality_family() {
            let fixture = RecoveryFixture::new("complete-tip-terminal-exact", 0x41);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let complete_tip = complete_tip_for_terminal_decision(&fixture, &projection);

            assert_eq!(
                ledger
                    .authenticate_complete_tip_terminal_apply(&complete_tip)
                    .expect("join exact CompleteTip finality to terminal Apply"),
                4
            );

            let (foreign, _) = terminal_decision_chain_fixture_with_seed(&fixture, 0xE2);
            assert!(
                foreign
                    .authenticate_complete_tip_terminal_apply(&complete_tip)
                    .is_err(),
                "another canonical Decision certificate cannot enter the CompleteTip join"
            );
        }

        #[test]
        fn complete_tip_terminal_join_rejects_foreign_apply_reconstruction_source() {
            let fixture = RecoveryFixture::new("complete-tip-terminal-source", 0x43);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let complete_tip = complete_tip_for_terminal_decision(&fixture, &projection);
            let mut records = ledger.records.clone();
            records[3].reconstruction_source = [0xFA; 32];
            let foreign_source = LifecycleLedgerV1::new(
                ledger.context(),
                ledger.high_water(),
                records,
                BTreeMap::new(),
            )
            .expect("terminal Apply source drift remains structurally decodable");

            assert!(
                foreign_source
                    .authenticate_complete_tip_terminal_apply(&complete_tip)
                    .is_err(),
                "terminal Apply must retain the exact body-family reconstruction owner"
            );
        }

        #[test]
        fn complete_tip_terminal_apply_store_join_consumes_the_exact_opened_frame() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-cut", 0x45);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let kura = Kura::blank_kura_for_testing();
            let predecessor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let complete_tip =
                complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref());
            let (store, empty) =
                LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
                    .expect("open canonical CompleteTip predecessor store");
            assert!(empty.records().is_empty());
            store
                .persist(&ledger)
                .expect("persist terminal CompleteTip predecessor");

            assert!(
                complete_tip
                    .into_canonical_predecessor_storage(&fixture.keys[0])
                    .and_then(|cut| cut.is_exact())
                    .is_ok_and(|exact| exact),
                "the capability must open its exact ledger, body, and Serve-payload owners"
            );
        }

        #[test]
        fn complete_tip_all_row_retirement_is_exact_and_restart_idempotent() {
            let fixture = RecoveryFixture::new("complete-tip-all-row-retirement", 0x46);
            let (terminal_chain, projection) = terminal_decision_chain_fixture(&fixture);
            let foreign_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x46; 32])), 5);
            let mut records = terminal_chain.records.clone();
            records.push(unrelated_live_record(
                terminal_chain.context(),
                foreign_owner,
                5,
                0xE5,
            ));
            let predecessor =
                LifecycleLedgerV1::new(terminal_chain.context(), 5, records, BTreeMap::new())
                    .expect("construct CompleteTip predecessor with unrelated live work");
            let kura = Kura::blank_kura_for_testing();
            let predecessor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let (predecessor_store, empty) =
                LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
                    .expect("open canonical all-row predecessor store");
            assert!(empty.records().is_empty());
            predecessor_store
                .persist(&predecessor)
                .expect("persist all-row CompleteTip predecessor");

            let complete_tip =
                complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref());
            let retired = complete_tip
                .into_canonical_predecessor_storage(&fixture.keys[0])
                .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
                .expect("retire every predecessor row and initialize successor");
            assert_eq!(retired.retained_high_water(), 5);
            let reopened_predecessor = predecessor_store
                .load()
                .expect("reload retired predecessor frame");
            assert!(reopened_predecessor.producer_debts().is_empty());
            assert!(
                reopened_predecessor
                    .records()
                    .iter()
                    .all(|record| record.terminal().is_some_and(|terminal| terminal.is_some()))
            );
            assert_eq!(
                reopened_predecessor.records()[4].terminal(),
                Some(Some(TerminalOutcome::Cancelled))
            );
            assert_eq!(
                reopened_predecessor.records()[..4],
                terminal_chain.records()[..4],
                "the exact CompleteTip Decision tombstones remain byte-identical"
            );
            assert_eq!(
                retired.predecessor_frame_identity,
                reopened_predecessor.frame_identity()
            );

            let successor_context_id = retired.successor_context_id();
            let mut successor_context_bytes = [0_u8; 32];
            successor_context_bytes.copy_from_slice(successor_context_id.0.as_ref());
            let successor_context = LifecycleContext::new(
                LifecycleDigest::new(successor_context_bytes),
                fixture.lifecycle_context().height() + 1,
            );
            let successor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(successor_context_id.0.as_ref()));
            let (successor_store, initialized_successor) =
                LifecycleLedgerStoreV1::open(&successor_root, successor_context)
                    .expect("reopen initialized CompleteTip successor");
            assert!(initialized_successor.records().is_empty());
            assert!(initialized_successor.producer_debts().is_empty());
            assert_eq!(initialized_successor.high_water(), 5);
            assert_eq!(
                retired.successor_frame_identity,
                initialized_successor.frame_identity()
            );

            let descendant_owner =
                OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x47; 32])), 6);
            let descendant = LifecycleLedgerV1::new(
                successor_context,
                6,
                vec![unrelated_live_record(
                    successor_context,
                    descendant_owner,
                    6,
                    0xE6,
                )],
                BTreeMap::new(),
            )
            .expect("construct later exact successor descendant");
            successor_store
                .persist_exact_successor(&initialized_successor, &descendant)
                .expect("publish later successor work above retained high-water");

            let retired_body_root = kura.sumeragi_v2_storage_root().join("bodies");
            std::fs::create_dir_all(&retired_body_root)
                .expect("materialize the obsolete predecessor body-owner root");
            std::fs::remove_dir_all(&retired_body_root)
                .expect("remove obsolete predecessor body owner after retirement");
            std::fs::write(
                &retired_body_root,
                b"retired body owner is no longer opened",
            )
            .expect("make any accidental predecessor body-store reopen fail");

            let repeated =
                complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
                    .into_canonical_predecessor_storage(&fixture.keys[0])
                    .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
                    .expect("exact retirement restart must stutter");
            assert_eq!(
                repeated.predecessor_frame_identity,
                retired.predecessor_frame_identity
            );
            assert_eq!(
                repeated.successor_frame_identity,
                descendant.frame_identity(),
                "restart must preserve a valid later successor without reopening obsolete predecessor bodies"
            );
            assert_eq!(repeated.predecessor(), retired.predecessor());
            assert_eq!(repeated.successor_context_id(), successor_context_id);
        }

        #[test]
        /// Prove retirement binds only the exact unlaunched H+1 owner.
        pub(crate) fn complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner() {
            let fixture = RecoveryFixture::new("complete-tip-successor-owner-bind", 0x49);
            let (predecessor, projection) = terminal_decision_chain_fixture(&fixture);
            let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
            let kura = Kura::blank_kura_for_testing();
            let predecessor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let (predecessor_store, empty) =
                LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
                    .expect("open canonical owner-binding predecessor");
            assert!(empty.records().is_empty());
            predecessor_store
                .persist(&predecessor)
                .expect("persist owner-binding predecessor");
            let retire = || {
                complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
                    .into_canonical_predecessor_storage(&fixture.keys[0])
                    .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
                    .expect("retire predecessor and authenticate exact H+1 target")
            };
            let body_root = kura.sumeragi_v2_storage_root().join("bodies");

            let retirement = retire();
            let foreign_root = TempDir::new().expect("foreign successor root");
            let (foreign_store, foreign_empty) = LifecycleLedgerStoreV1::open(
                foreign_root.path(),
                retirement.successor_ledger.context(),
            )
            .expect("open copied H+1 ledger target");
            assert!(foreign_empty.records().is_empty());
            foreign_store
                .persist(&retirement.successor_ledger)
                .expect("copy exact H+1 bytes to foreign target");
            let foreign_owner = empty_successor_owner_for_complete_tip(
                &retirement,
                kura.as_ref(),
                verified_successor.clone(),
                &body_root,
                foreign_root.path(),
                foreign_store,
            );
            assert!(
                retirement.bind_successor_owner(foreign_owner).is_err(),
                "byte-identical H+1 state at another publication target must fail closed"
            );

            let retirement = retire();
            let foreign_payload_root = TempDir::new().expect("foreign H+1 payload root");
            let foreign_payload_owner = empty_successor_owner_for_complete_tip(
                &retirement,
                kura.as_ref(),
                verified_successor.clone(),
                &body_root,
                foreign_payload_root.path(),
                retirement.successor_store.clone(),
            );
            assert!(
                retirement
                    .bind_successor_owner(foreign_payload_owner)
                    .is_err(),
                "the exact ledger cannot authorize a separately rooted Serve-payload owner"
            );

            let retirement = retire();
            let successor_root = retirement
                .successor_store
                .path
                .parent()
                .expect("canonical H+1 ledger has a parent root")
                .to_path_buf();
            let foreign_body_root = TempDir::new().expect("foreign H+1 body root");
            let foreign_body_owner = empty_successor_owner_for_complete_tip(
                &retirement,
                kura.as_ref(),
                verified_successor.clone(),
                foreign_body_root.path(),
                &successor_root,
                retirement.successor_store.clone(),
            );
            assert!(
                retirement.bind_successor_owner(foreign_body_owner).is_err(),
                "the exact ledger cannot authorize a separately rooted body owner"
            );

            let retirement = retire();
            let successor_root = retirement
                .successor_store
                .path
                .parent()
                .expect("canonical H+1 ledger has a parent root")
                .to_path_buf();
            let foreign_kura = Kura::blank_kura_for_testing();
            let foreign_kura_owner = empty_successor_owner_for_complete_tip(
                &retirement,
                foreign_kura.as_ref(),
                verified_successor.clone(),
                &body_root,
                &successor_root,
                retirement.successor_store.clone(),
            );
            assert!(
                retirement.bind_successor_owner(foreign_kura_owner).is_err(),
                "canonical H+1 storage cannot launch against another live Kura instance"
            );

            let retirement = retire();
            let successor_root = retirement
                .successor_store
                .path
                .parent()
                .expect("canonical H+1 ledger has a parent root")
                .to_path_buf();
            let exact_owner = empty_successor_owner_for_complete_tip(
                &retirement,
                kura.as_ref(),
                verified_successor,
                &body_root,
                &successor_root,
                retirement.successor_store.clone(),
            );
            let mut bound = retirement
                .bind_successor_owner(exact_owner)
                .expect("bind exact canonical unlaunched H+1 owner");
            assert!(bound.remains_exact_for_test());

            let old = bound.retirement.successor_ledger.clone();
            let next_ordinal = old
                .high_water()
                .checked_add(1)
                .expect("fixture H+1 ordinal remains representable");
            let owner = OwnerId::new(
                CausalRoot::new(LifecycleDigest::new([0x4A; 32])),
                next_ordinal,
            );
            let drifted = LifecycleLedgerV1::new(
                old.context(),
                next_ordinal,
                vec![unrelated_live_record(
                    old.context(),
                    owner,
                    next_ordinal,
                    0xEA,
                )],
                BTreeMap::new(),
            )
            .expect("construct post-bind H+1 storage drift");
            bound
                .retirement
                .successor_store
                .persist_exact_successor(&old, &drifted)
                .expect("publish test-only H+1 drift");
            assert!(
                !bound.remains_exact_for_test(),
                "the bound owner must detect canonical H+1 drift before launch"
            );
        }

        #[test]
        fn complete_tip_all_row_retirement_consumes_pending_serve_terminal_update() {
            let fixture = RecoveryFixture::new("complete-tip-serve-retirement", 0x4C);
            let (terminal_chain, projection) = terminal_decision_chain_fixture(&fixture);
            let kura = Kura::blank_kura_for_testing();
            let predecessor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let body_root = kura.sumeragi_v2_storage_root().join("bodies");
            let body_store = V2BodyStore::open(&body_root, fixture.verified.context().clone())
                .expect("open canonical CompleteTip body store");
            let request = fixture.authenticated_serve_request(0, 0x4C, 0);
            let (mut payload_store, recovered) =
                CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
                    .expect("open canonical CompleteTip Serve payload store");
            assert!(recovered.is_empty());
            let pending = payload_store
                .persist_pending_with_verified_retention(
                    &fixture.verified,
                    &fixture.keys[0],
                    &request,
                )
                .expect("persist retained CompleteTip Serve payload");
            let authority =
                authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
                    .expect("construct CompleteTip Serve lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 4);
            assert!(matches!(
                coordinator
                    .admit_certified_serve(&fixture.verified, &request, pending)
                    .expect("project retained CompleteTip Serve request"),
                super::super::super::AdmissionDecision::Admitted { ordinal: 5, .. }
            ));
            let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project live CompleteTip Serve pair");
            assert_eq!(serve_ledger.records().len(), 2);
            assert_eq!(
                serve_ledger.producer_debts(),
                &[LifecycleProducerDebtV1::new(5, 6)]
            );

            let mut records = terminal_chain.records.clone();
            records.extend_from_slice(serve_ledger.records());
            let predecessor = LifecycleLedgerV1::new(
                terminal_chain.context(),
                6,
                records,
                BTreeMap::from([(5, 6)]),
            )
            .expect("join terminal Decision chain with live Serve pair");
            let (predecessor_store, empty) =
                LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
                    .expect("open canonical Serve predecessor ledger");
            assert!(empty.records().is_empty());
            predecessor_store
                .persist(&predecessor)
                .expect("persist live Serve predecessor ledger");
            drop(payload_store);
            drop(body_store);

            complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
                .into_canonical_predecessor_storage(&fixture.keys[0])
                .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
                .expect("retire exact Pending Serve and its ProducerTurn");

            let retired = predecessor_store
                .load()
                .expect("reload Serve-retired predecessor ledger");
            assert!(retired.producer_debts().is_empty());
            assert_eq!(
                retired.records()[4].terminal(),
                Some(Some(TerminalOutcome::Cancelled))
            );
            assert_eq!(
                retired.records()[5].terminal(),
                Some(Some(TerminalOutcome::Cancelled))
            );
            let reopened_body = V2BodyStore::open(&body_root, fixture.verified.context().clone())
                .expect("reopen canonical CompleteTip body store");
            let (reopened_payload_store, recovered) =
                CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
                    .expect("reopen retired CompleteTip Serve payload store");
            let authenticated = recovered
                .authenticate(&fixture.verified, &fixture.keys[0], &reopened_body)
                .expect("authenticate retired CompleteTip Serve payload cut");
            reopened_payload_store
                .validate_authenticated_cut(&authenticated)
                .expect("retired Serve payload cut remains exact");
            assert!(authenticated.iter().all(|payload| matches!(
                payload.state(),
                crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedRecoveredCertifiedServePayloadState::Negative(
                    crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome::Cancelled,
                )
            )));
        }

        #[test]
        /// Prove CompleteTip retirement survives normal predecessor-body cleanup.
        pub(crate) fn complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work()
        {
            let fixture = RecoveryFixture::new("complete-tip-completed-serve-cleanup", 0x50);
            let (terminal_chain, projection) = terminal_decision_chain_fixture(&fixture);
            let kura = Kura::blank_kura_for_testing();
            let predecessor_root = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let body_root = kura.sumeragi_v2_storage_root().join("bodies");
            let mut body_store = V2BodyStore::open(&body_root, fixture.verified.context().clone())
                .expect("open canonical CompleteTip body store");
            let (request, durable_body, response) =
                fixture.completed_serve_exchange(&mut body_store, 0, 0x50, 3);
            let (mut payload_store, recovered) =
                CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
                    .expect("open canonical CompleteTip Serve payload store");
            assert!(recovered.is_empty());
            let pending = payload_store
                .persist_pending_with_verified_retention(
                    &fixture.verified,
                    &fixture.keys[0],
                    &request,
                )
                .expect("persist retained Completed-Serve request");

            let authority =
                authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
                    .expect("construct Completed-Serve lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 4);
            assert!(matches!(
                coordinator
                    .admit_certified_serve(&fixture.verified, &request, pending)
                    .expect("project retained Completed-Serve request"),
                super::super::super::AdmissionDecision::Admitted { ordinal: 5, .. }
            ));
            let pending_serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project the pre-completion Serve pair");
            let ready = coordinator.ready_index.iter().map(|ordinal| {
                let record = &coordinator.records[ordinal];
                (
                    *ordinal,
                    super::super::super::SchedulerReadyInputs::new(record, None, [0; 6]),
                )
            });
            let TurnPlan::Execute(lease) = coordinator.plan_turn(
                super::super::super::SchedulerInputs::new([], ready)
                    .expect("Completed Serve is the sole Ready row"),
            ) else {
                panic!("Completed Serve must own the selected turn")
            };
            let completed = payload_store
                .persist_completed_with_exact_body(&request, &durable_body, &body_store, &response)
                .expect("persist exact Completed-Serve tombstone");
            let serve_ordinal = lease.ordinal();
            let producer_ordinal = coordinator.producer_debts[&serve_ordinal];
            let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
                coordinator.active_context,
                &coordinator.records[&serve_ordinal],
                &coordinator.durable_records[&serve_ordinal],
                &coordinator.records[&producer_ordinal],
                &coordinator.durable_records[&producer_ordinal],
                completed,
            )
            .expect("close exact Completed-Serve replay family");
            coordinator.reduce_settle_turn(
                lease,
                TurnOutcome::Terminal(terminal.terminal_outcome()),
                Some(terminal),
            );
            assert_eq!(coordinator.fault(), None);
            let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project terminal Serve with live ProducerTurn");
            assert_eq!(serve_ledger.records().len(), 2);
            let response_digest =
                LifecycleDigest::new((*iroha_crypto::HashOf::new(&response).as_ref()).into());
            assert_eq!(
                serve_ledger.records()[0].terminal(),
                Some(Some(TerminalOutcome::Completed(Some(response_digest))))
            );
            assert_eq!(serve_ledger.records()[1].terminal(), Some(None));

            let unrelated_owner =
                OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x51; 32])), 7);
            let mut records = terminal_chain.records.clone();
            records.extend_from_slice(serve_ledger.records());
            records.push(unrelated_live_record(
                terminal_chain.context(),
                unrelated_owner,
                7,
                0xF0,
            ));
            let predecessor = LifecycleLedgerV1::new(
                terminal_chain.context(),
                7,
                records,
                BTreeMap::from([(serve_ordinal, producer_ordinal)]),
            )
            .expect("join terminal Decision, Completed Serve, and unrelated live work");
            let (predecessor_store, empty) =
                LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
                    .expect("open canonical Completed-Serve predecessor ledger");
            assert!(empty.records().is_empty());
            predecessor_store
                .persist(&predecessor)
                .expect("persist Completed-Serve predecessor ledger");
            drop(payload_store);
            drop(body_store);
            std::fs::remove_dir_all(&body_root)
                .expect("simulate normal post-finality predecessor body cleanup");

            {
                let (_bodyless_payload_store, recovered) = CertifiedServePayloadStoreV1::open(
                    &predecessor_root,
                    fixture.verified.context(),
                )
                .expect("reopen Completed metadata after body cleanup");
                let bodyless = recovered
                    .authenticate_for_complete_tip_retirement(&fixture.verified, &fixture.keys[0])
                    .expect("authenticate retirement-only Completed metadata");
                assert!(
                    super::super::super::open::authenticate_complete_tip_serve_census(
                        &pending_serve_ledger,
                        &bodyless,
                    )
                    .is_err(),
                    "bodyless metadata must not promote a Pending Serve ledger row"
                );
            }

            complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
                .into_canonical_predecessor_storage(&fixture.keys[0])
                .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
                .expect("retire Completed Serve after body cleanup");

            let retired = predecessor_store
                .load()
                .expect("reload Completed-Serve retired predecessor");
            assert!(retired.producer_debts().is_empty());
            assert!(
                retired
                    .records()
                    .iter()
                    .all(|record| { record.terminal().is_some_and(|terminal| terminal.is_some()) })
            );
            assert_eq!(retired.records()[4], serve_ledger.records()[0]);
            assert_eq!(
                retired.records()[5].terminal(),
                Some(Some(TerminalOutcome::Cancelled))
            );
            assert_eq!(
                retired.records()[6].terminal(),
                Some(Some(TerminalOutcome::Cancelled))
            );
            assert!(!body_root.exists());
            let (reopened_payload_store, recovered) =
                CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
                    .expect("reopen body-independent retired Serve payload store");
            let authenticated = recovered
                .authenticate_for_complete_tip_retirement(&fixture.verified, &fixture.keys[0])
                .expect("reauthenticate Completed metadata without body bytes");
            reopened_payload_store
                .validate_authenticated_cut(&authenticated)
                .expect("retired Completed payload cut remains exact");
            assert_eq!(authenticated.len(), 1);
        }

        #[test]
        fn complete_tip_terminal_apply_store_join_detects_later_same_store_drift() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-later-drift", 0x47);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let directory = TempDir::new().expect("temporary later-drift predecessor ledger");
            let complete_tip =
                complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
            let (store, empty) =
                LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
                    .expect("open later-drift predecessor store");
            store
                .persist(&ledger)
                .expect("persist terminal CompleteTip predecessor");
            let same_store_writer = store.clone();
            let cut = ledger
                .into_complete_tip_terminal_apply_store_join(store, complete_tip)
                .expect("authenticate exact predecessor before drift");
            same_store_writer
                .persist(&empty)
                .expect("replace predecessor after cut authentication");

            assert!(
                !cut.is_exact().expect("reload retained predecessor store"),
                "the retained cut must detect later writes through another handle"
            );
        }

        #[test]
        fn complete_tip_terminal_apply_store_join_is_not_an_all_row_retirement() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-chain-local", 0x48);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let foreign_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x48; 32])), 5);
            let mut records = ledger.records.clone();
            records.push(unrelated_live_record(
                ledger.context(),
                foreign_owner,
                5,
                0xE4,
            ));
            let chain_local = LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
                .expect("construct predecessor with unrelated live work");
            let directory = TempDir::new().expect("temporary chain-local predecessor ledger");
            let complete_tip =
                complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
            let (store, empty) =
                LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
                    .expect("open chain-local predecessor store");
            assert!(empty.records().is_empty());
            store
                .persist(&chain_local)
                .expect("persist chain-local predecessor");

            assert!(
                chain_local
                    .into_complete_tip_terminal_apply_store_join(store, complete_tip)
                    .is_ok(),
                "this prerequisite must not masquerade as exhaustive retirement"
            );
        }

        #[test]
        fn complete_tip_terminal_apply_store_join_rejects_store_drift() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-drift", 0x49);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let directory = TempDir::new().expect("temporary drifted predecessor ledger");
            let complete_tip =
                complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
            let (store, empty) =
                LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
                    .expect("open drifted CompleteTip predecessor store");
            store
                .persist(&ledger)
                .expect("persist terminal CompleteTip predecessor");
            store
                .persist(&empty)
                .expect("replace predecessor before cut authentication");

            assert!(
                ledger
                    .into_complete_tip_terminal_apply_store_join(store, complete_tip)
                    .is_err(),
                "a changed attached frame cannot mint predecessor authority"
            );
        }

        #[test]
        fn complete_tip_terminal_apply_store_join_rejects_an_identical_foreign_target() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-foreign-target", 0x4A);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let canonical_kura = Kura::blank_kura_for_testing();
            let foreign_kura = Kura::blank_kura_for_testing();
            let complete_tip = complete_tip_for_terminal_decision_on_kura(
                &fixture,
                &projection,
                canonical_kura.as_ref(),
            );
            let foreign_root = foreign_kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let (foreign_store, empty) =
                LifecycleLedgerStoreV1::open(&foreign_root, fixture.lifecycle_context())
                    .expect("open foreign predecessor store");
            assert!(empty.records().is_empty());
            foreign_store
                .persist(&ledger)
                .expect("copy exact terminal predecessor frame to foreign root");

            assert!(
                ledger
                    .into_complete_tip_terminal_apply_store_join(foreign_store, complete_tip)
                    .is_err(),
                "byte-identical ledger data cannot substitute for the Kura-bound target"
            );
        }

        #[test]
        fn complete_tip_successor_target_initializes_and_accepts_an_exact_descendant() {
            let context = LifecycleContext::new(LifecycleDigest::new([0xA1; 32]), 2);
            let directory = TempDir::new().expect("temporary CompleteTip successor target");
            let target = CanonicalCompleteTipSuccessorLedgerTargetV1 {
                root: directory.path().join("successor"),
                context,
            };
            let (store, initialized) = target
                .open_initialized_or_descendant(4)
                .expect("initialize successor at predecessor high-water");
            assert_eq!(initialized.high_water(), 4);
            assert!(initialized.records().is_empty());

            let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xA2; 32])), 5);
            let descendant = LifecycleLedgerV1::new(
                context,
                5,
                vec![unrelated_live_record(context, owner, 5, 0xA3)],
                BTreeMap::new(),
            )
            .expect("construct exact successor descendant");
            store
                .persist_exact_successor(&initialized, &descendant)
                .expect("publish descendant above retained ordinal floor");

            let (_, reopened) = target
                .open_initialized_or_descendant(4)
                .expect("preserve a valid nonempty descendant without rewriting it");
            assert_eq!(reopened, descendant);
        }

        #[test]
        fn complete_tip_successor_target_rejects_a_foreign_ordinal_floor() {
            let context = LifecycleContext::new(LifecycleDigest::new([0xB1; 32]), 2);
            let directory = TempDir::new().expect("temporary foreign-floor successor target");
            let target = CanonicalCompleteTipSuccessorLedgerTargetV1 {
                root: directory.path().join("successor"),
                context,
            };
            let (store, empty) = LifecycleLedgerStoreV1::open(&target.root, context)
                .expect("open foreign-floor successor");
            let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xB2; 32])), 4);
            let foreign = LifecycleLedgerV1::new(
                context,
                4,
                vec![unrelated_live_record(context, owner, 4, 0xB3)],
                BTreeMap::new(),
            )
            .expect("construct independently zero-based successor frame");
            store
                .persist_exact_successor(&empty, &foreign)
                .expect("persist foreign successor fixture");

            assert!(target.open_initialized_or_descendant(4).is_err());
        }

        #[test]
        fn terminal_recovered_decision_oracle_rejects_a_live_apply() {
            let fixture = RecoveryFixture::new("terminal-decision-live-apply", 0x35);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let mut records = ledger.records.clone();
            records[3].terminal = None;
            let live = LifecycleLedgerV1::new(
                ledger.context(),
                ledger.high_water(),
                records,
                BTreeMap::new(),
            )
            .expect("construct otherwise exact chain with a live Apply");

            assert!(
                live.authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .is_err()
            );
        }

        #[test]
        fn terminal_recovered_decision_oracle_rejects_extra_same_owner_history() {
            let fixture = RecoveryFixture::new("terminal-decision-same-owner", 0x39);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let owner = projection.fetch.owner();
            let mut records = ledger.records.clone();
            records.push(unrelated_live_record(ledger.context(), owner, 5, 0xE2));
            let with_extra_owner_history =
                LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
                    .expect("construct terminal chain with foreign same-owner history");

            assert!(
                with_extra_owner_history
                    .authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .is_err()
            );
        }

        #[test]
        fn terminal_recovered_decision_oracle_is_chain_local_and_allows_a_foreign_live_row() {
            let fixture = RecoveryFixture::new("terminal-decision-chain-local", 0x3D);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let foreign_root = CausalRoot::new(LifecycleDigest::new(
                *Hash::new(b"foreign live row outside terminal Decision chain").as_ref(),
            ));
            let foreign_owner = OwnerId::new(foreign_root, 5);
            let mut records = ledger.records.clone();
            records.push(unrelated_live_record(
                ledger.context(),
                foreign_owner,
                5,
                0xE3,
            ));
            let with_foreign_live =
                LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
                    .expect("construct terminal chain beside one foreign live row");

            assert_eq!(
                with_foreign_live
                    .authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .expect("the terminal oracle is intentionally limited to one owner chain"),
                4
            );
        }

        #[test]
        fn recovered_decision_stage_guard_routes_terminal_chain_to_complete_tip_retirement() {
            let fixture = RecoveryFixture::new("terminal-decision-stage-guard", 0x41);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);

            let error = ledger
                .reject_terminal_recovered_decision_apply_projection(&projection)
                .expect_err("terminal Apply cannot re-enter live recovered staging");
            assert!(matches!(
                error,
                LifecycleLedgerError::InvalidLedger(reason)
                    if reason == "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
            ));
        }

        fn admit_and_claim_serve(
            fixture: &RecoveryFixture,
            owner: &mut ProductionLifecycleOwnerV1,
            request: &AuthenticatedCertifiedBodyRequest,
        ) -> super::super::super::TurnLease {
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let admitted = owner.admit_selected_certified_serve(target, &fixture.keys[0], request);
            assert!(matches!(
                admitted.decision(),
                Some(super::super::super::AdmissionDecision::Admitted { .. })
            ));
            owner.claim_certified_serve_for_test()
        }

        #[test]
        fn consuming_storage_cut_censes_every_live_fetch_and_binds_exact_ledger_frame() {
            let fixture = RecoveryFixture::new("durable-ready-fetch-census", 0x31);
            let directory = TempDir::new().expect("temporary durable Ready-Fetch store");
            let mut store = fixture.open_store(&directory);
            let first = fixture.fetch_record(&mut store, 0, 0x41, 1, None, false);
            let second = fixture.fetch_record(&mut store, 1, 0x42, 2, None, false);
            let ledger = fixture.ledger(vec![first, second]);
            let ledger_directory =
                TempDir::new().expect("temporary durable Ready-Fetch lifecycle ledger");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);

            let mut cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    store,
                )
                .expect("all live durable Fetch rows form one consuming storage cut");
            assert_eq!(
                cut.ledger
                    .records
                    .iter()
                    .filter(|record| record.work_class() == Some(LifecycleWorkClass::Fetch))
                    .count(),
                2,
            );
            assert!(cut.is_exact(), "the opaque census covers both live rows");

            cut.ledger.high_water += 1;
            assert!(
                !cut.is_exact(),
                "the census cannot cross even a structurally harmless foreign ledger frame",
            );
        }

        #[test]
        fn production_owner_opens_empty_and_two_fetch_storage_atomically() {
            let empty_fixture = RecoveryFixture::new("empty-production-lifecycle-owner", 0x11);
            let empty_body_directory =
                TempDir::new().expect("temporary empty production body store");
            let empty_body_store = empty_fixture.open_store(&empty_body_directory);
            let empty_payload_directory =
                TempDir::new().expect("temporary empty production payload store");
            let (empty_payload_store, empty_payloads) = empty_fixture
                .open_empty_serve_payloads(&empty_payload_directory, &empty_body_store);
            let empty_ledger = empty_fixture.ledger(Vec::new());
            let empty_ledger_directory =
                TempDir::new().expect("temporary empty production ledger store");
            let empty_ledger_store =
                empty_fixture.persist_ledger(&empty_ledger_directory, &empty_ledger);
            let empty_cut = empty_ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    empty_fixture.verified.clone(),
                    empty_ledger_store,
                    empty_body_store,
                )
                .expect("seal empty production storage cut");
            let mut empty_owner = empty_cut
                .open_owner_for_test(empty_payload_store, empty_payloads)
                .expect("open empty production lifecycle owner");
            assert!(empty_owner.exact_recovered_fetch_join_for_test());
            assert_eq!(empty_owner.live_fetch_count_for_test(), 0);
            assert_eq!(empty_owner.plan_direct_registry_turn(), Ok(TurnPlan::Idle));

            let fixture = RecoveryFixture::new("two-fetch-production-lifecycle-owner", 0x21);
            let body_directory = TempDir::new().expect("temporary two-Fetch body store");
            let mut body_store = fixture.open_store(&body_directory);
            let first = fixture.fetch_record(&mut body_store, 0, 0x31, 1, None, false);
            let second = fixture.fetch_record(&mut body_store, 1, 0x32, 2, None, false);
            let payload_directory = TempDir::new().expect("temporary two-Fetch payload store");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(vec![first, second]);
            let ledger_directory = TempDir::new().expect("temporary two-Fetch ledger store");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal two-Fetch production storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open two-Fetch production lifecycle owner");
            assert!(owner.exact_recovered_fetch_join_for_test());
            assert_eq!(owner.live_fetch_count_for_test(), 2);
        }

        #[test]
        fn production_owner_keeps_terminal_validate_and_live_serve_together() {
            let fixture = RecoveryFixture::new("terminal-validate-live-serve-owner", 0x41);
            let body_directory = TempDir::new().expect("temporary coexistence body store");
            let mut body_store = fixture.open_store(&body_directory);
            let terminal_validate = fixture.terminal_validate_record(&mut body_store, 1, 0x51, 3);

            let payload_directory = TempDir::new().expect("temporary coexistence payload store");
            let (mut payload_store, _) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("open coexistence Certified-Serve payload store");
            let request = fixture.authenticated_serve_request(0, 0x52, 3);
            let receipt = payload_store
                .persist_pending_with_verified_retention(
                    &fixture.verified,
                    &fixture.keys[0],
                    &request,
                )
                .expect("persist coexistence Certified-Serve request");
            let authority =
                authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
                    .expect("construct coexistence lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
            assert!(matches!(
                coordinator
                    .admit_certified_serve(&fixture.verified, &request, receipt)
                    .expect("project coexistence Certified-Serve request"),
                super::super::super::AdmissionDecision::Admitted { .. }
            ));
            let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project coexistence Serve ledger");
            let mut records = serve_ledger.records.clone();
            records.push(terminal_validate);
            let producer_debts = serve_ledger
                .producer_debts
                .iter()
                .map(|debt| (debt.serve_ordinal(), debt.producer_ordinal()))
                .collect();
            let ledger =
                LifecycleLedgerV1::new(fixture.lifecycle_context(), 3, records, producer_debts)
                    .expect("construct terminal-Validate/live-Serve ledger");
            drop(payload_store);
            let (payload_store, recovered_payloads) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("reopen coexistence Certified-Serve payload store");
            let payloads = recovered_payloads
                .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
                .expect("authenticate coexistence Certified-Serve payload");
            let ledger_directory = TempDir::new().expect("temporary coexistence ledger store");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal coexistence storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open terminal-Validate/live-Serve production owner");
            assert!(owner.exact_recovered_fetch_join_for_test());
            assert_eq!(owner.live_fetch_count_for_test(), 0);
            assert_eq!(owner.terminal_validate_count_for_test(), 1);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1),
                "live Serve and dormant adjacent ProducerTurn both retain exact carriers",
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family(),
                "startup carriers retain the same whole replay family",
            );
        }

        #[test]
        fn fresh_certified_serve_publishes_exact_ledger_and_shared_pair_beside_fetch() {
            let fixture = RecoveryFixture::new("fresh-serve-owner", 0x81);
            let body_directory = TempDir::new().expect("temporary fresh Serve body store");
            let mut body_store = fixture.open_store(&body_directory);
            let fetch = fixture.fetch_record(&mut body_store, 0, 0x82, 1, None, false);
            let payload_directory = TempDir::new().expect("temporary fresh Serve payload store");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(vec![fetch]);
            let ledger_directory = TempDir::new().expect("temporary fresh Serve ledger store");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal fresh Serve storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open fresh Serve production owner");
            let request = fixture.authenticated_serve_request(1, 0x83, 3);
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );

            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(matches!(
                outcome.decision(),
                Some(super::super::super::AdmissionDecision::Admitted {
                    ordinal: 2,
                    producer_turn_ordinal: Some(3),
                    ..
                })
            ));
            assert!(!outcome.restart_required());
            let Ok(continuation) = outcome.into_safe_continuation() else {
                panic!("published fresh Serve must return its safe selector continuation")
            };
            assert!(continuation.failure().is_none());
            assert!(
                continuation
                    .into_target()
                    .matches_certified_serve_request(request.request_hash())
            );
            assert_eq!(owner.live_fetch_count_for_test(), 1);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family()
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&owner.coordinator)
            );
            let store = owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("fresh owner retains LedgerV1 store");
            assert_eq!(
                store.load().expect("reload fresh Serve LedgerV1"),
                LifecycleLedgerV1::from_coordinator(&owner.coordinator)
                    .expect("project fresh Serve coordinator")
            );

            let retry_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    2,
                );
            let retry =
                owner.admit_selected_certified_serve(retry_target, &fixture.keys[0], &request);
            assert!(matches!(
                retry.decision(),
                Some(super::super::super::AdmissionDecision::Retry { ordinal: 2, .. })
            ));
            assert!(retry.into_safe_continuation().is_ok());
            assert_eq!(owner.live_fetch_count_for_test(), 1);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1),
                "idempotent retry must preserve the unrelated Fetch and exact shared pair"
            );
        }

        #[test]
        fn terminal_owner_publishes_completed_and_reopens_exact_producer_carrier() {
            let fixture = RecoveryFixture::new("terminal-owner-completed", 0x85);
            let body_directory = TempDir::new().expect("temporary completed-owner body store");
            let payload_directory =
                TempDir::new().expect("temporary completed-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary completed-owner ledger store");
            let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
            );
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let serve_ordinal = lease.ordinal();
            let producer_ordinal = serve_ordinal + 1;

            owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &response)
                .expect("owner publishes exact completed Serve terminal");

            let response_digest =
                LifecycleDigest::new((*iroha_crypto::HashOf::new(&response).as_ref()).into());
            assert_eq!(
                owner.coordinator.records[&serve_ordinal].state,
                LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
            );
            assert_eq!(
                owner.coordinator.records[&producer_ordinal].state,
                LifecycleState::Ready
            );
            assert_eq!(owner.coordinator.active_lease, None);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&owner.coordinator)
            );
            let on_disk = owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("completed owner retains LedgerV1 store")
                .load()
                .expect("reload completed owner LedgerV1");
            assert_eq!(
                on_disk,
                LifecycleLedgerV1::from_coordinator(&owner.coordinator)
                    .expect("project completed owner coordinator")
            );
            drop(owner);

            let body_store = fixture.open_store(&body_directory);
            let (payload_store, recovered) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("reopen completed-owner payload store");
            let payloads = recovered
                .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
                .expect("authenticate completed-owner payloads");
            let (ledger_store, ledger) =
                LifecycleLedgerStoreV1::open(ledger_directory.path(), fixture.lifecycle_context())
                    .expect("reopen completed-owner LedgerV1");
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal completed-owner restart cut");
            let mut reopened = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("reopen completed production owner");
            assert_eq!(
                reopened.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            assert_eq!(
                reopened.coordinator.records[&serve_ordinal].state,
                LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
            );
            assert!(
                reopened
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&reopened.coordinator)
            );
        }

        #[test]
        fn terminal_owner_publishes_rejected_failed_and_cancelled_carrier_shapes() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            for (index, outcome) in [
                CertifiedServePayloadNegativeOutcome::Rejected(37),
                CertifiedServePayloadNegativeOutcome::Failed(41),
                CertifiedServePayloadNegativeOutcome::Cancelled,
            ]
            .into_iter()
            .enumerate()
            {
                let fixture = RecoveryFixture::new(
                    &format!("terminal-owner-negative-{index}"),
                    0x89 + u8::try_from(index).expect("small terminal fixture index") * 4,
                );
                let body_directory = TempDir::new().expect("temporary negative-owner body store");
                let payload_directory =
                    TempDir::new().expect("temporary negative-owner payload store");
                let ledger_directory =
                    TempDir::new().expect("temporary negative-owner ledger store");
                let mut owner = fixture.open_empty_owner(
                    &body_directory,
                    &payload_directory,
                    &ledger_directory,
                );
                let request = fixture.authenticated_serve_request(0, 0x90, 3);
                let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
                let serve_ordinal = lease.ordinal();
                let producer_ordinal = serve_ordinal + 1;

                owner
                    .settle_certified_serve_negative(lease, &request, outcome)
                    .expect("owner publishes exact negative Serve terminal");

                let expected = match outcome {
                    CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                        TerminalOutcome::Rejected(code)
                    }
                    CertifiedServePayloadNegativeOutcome::Failed(code) => {
                        TerminalOutcome::Failed(code)
                    }
                    CertifiedServePayloadNegativeOutcome::Cancelled => TerminalOutcome::Cancelled,
                };
                assert_eq!(
                    owner.coordinator.records[&serve_ordinal].state,
                    LifecycleState::Terminal(expected)
                );
                let cancelled = outcome == CertifiedServePayloadNegativeOutcome::Cancelled;
                assert_eq!(
                    owner.coordinator.records[&producer_ordinal].state,
                    if cancelled {
                        LifecycleState::Terminal(TerminalOutcome::Cancelled)
                    } else {
                        LifecycleState::Ready
                    }
                );
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    if cancelled { (0, 0) } else { (0, 1) }
                );
                assert_eq!(
                    owner.coordinator.producer_debts.get(&serve_ordinal),
                    (!cancelled).then_some(&producer_ordinal)
                );
                assert!(
                    owner
                        .registry
                        .registry_mut()
                        .exactly_covers_recovered_ready_work(&owner.coordinator)
                );
            }
        }

        #[test]
        fn terminal_owner_returns_foreign_request_and_body_before_publication() {
            let fixture = RecoveryFixture::new("terminal-owner-input-rejection", 0x99);
            let body_directory = TempDir::new().expect("temporary input-owner body store");
            let payload_directory = TempDir::new().expect("temporary input-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary input-owner ledger store");
            let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
            );
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            let payloads = snapshot_files(payload_directory.path());
            let foreign = fixture.authenticated_serve_request(1, 0x9A, 3);

            let mut foreign_lease = lease.clone();
            foreign_lease.ordinal = foreign_lease
                .ordinal
                .checked_add(2)
                .expect("small foreign lease ordinal");
            let error = owner
                .settle_certified_serve_completed(foreign_lease, &request, &durable_body, &response)
                .expect_err("foreign lease is rejected before terminal persistence");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::Coordinator
            );
            assert!(error.into_lease().is_ok());
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);

            let error = owner
                .settle_certified_serve_completed(lease, &foreign, &durable_body, &response)
                .expect_err("foreign request is rejected before terminal persistence");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::RequestAuthority
            );
            let lease = error
                .into_lease()
                .expect("prepublication rejection returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );

            let foreign_receipt = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
                fixture.verified.context().id(),
                response.manifest.round,
                response.manifest.subject,
                iroha_crypto::HashOf::new(&response.manifest),
            );
            let error = owner
                .settle_certified_serve_completed(lease, &request, &foreign_receipt, &response)
                .expect_err("foreign durable receipt is rejected before terminal persistence");
            assert!(!error.restart_required());
            let lease = error
                .into_lease()
                .expect("foreign body receipt returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );

            let mut foreign_body = response.clone();
            foreign_body.body.push(0);
            let error = owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &foreign_body)
                .expect_err("foreign response body is rejected before terminal persistence");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
            );
            let lease = error
                .into_lease()
                .expect("foreign response body returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);

            let retained_body_store = owner
                .body_store
                .take()
                .expect("unlaunched owner still retains its exact body store");
            let error = owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &response)
                .expect_err("completion without the retained body store is prepublication-safe");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::BodyStoreUnavailable
            );
            let lease = error
                .into_lease()
                .expect("unavailable body store returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            drop(retained_body_store);
        }

        #[test]
        fn terminal_owner_faults_on_corrupt_owned_body_after_receipt_mint() {
            let fixture = RecoveryFixture::new("terminal-owner-owned-body-corruption", 0x9B);
            let body_directory = TempDir::new().expect("temporary corrupt-owner body store");
            let payload_directory = TempDir::new().expect("temporary corrupt-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary corrupt-owner ledger store");
            let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
            );
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            let pending_payloads = snapshot_files(payload_directory.path());
            let ledger = owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("terminal owner retains LedgerV1 store")
                .load()
                .expect("load pre-corruption LedgerV1");
            owner
                .body_store
                .as_ref()
                .expect("unlaunched owner retains its exact body store")
                .corrupt_owned_frame_for_test(&durable_body)
                .expect("replace the already-accepted body frame");

            let error = owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &response)
                .expect_err("reload corruption after receipt ownership requires restart");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
            );
            assert!(
                error.into_lease().is_err(),
                "accepted-store corruption must not release a safe retry lease"
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_eq!(snapshot_files(payload_directory.path()), pending_payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family()
            );
            assert_eq!(
                owner
                    .coordinator
                    .ledger_store
                    .as_ref()
                    .expect("faulted owner retains LedgerV1 store")
                    .load()
                    .expect("reload unchanged LedgerV1"),
                ledger
            );
        }

        #[test]
        fn terminal_registry_rejects_every_arbitrary_staged_drift_before_callback() {
            for (index, drift) in [
                StagedTerminalDrift::Record,
                StagedTerminalDrift::Index,
                StagedTerminalDrift::Debt,
                StagedTerminalDrift::Capacity,
                StagedTerminalDrift::HighWater,
            ]
            .into_iter()
            .enumerate()
            {
                let fixture = RecoveryFixture::new(
                    &format!("terminal-staged-drift-{index}"),
                    0xB0 + u8::try_from(index).expect("small drift index") * 4,
                );
                let body_directory = TempDir::new().expect("temporary staged-drift body store");
                let payload_directory =
                    TempDir::new().expect("temporary staged-drift payload store");
                let ledger_directory = TempDir::new().expect("temporary staged-drift ledger store");
                let (mut owner, request, durable_body, response) = fixture
                    .open_completed_serve_owner(
                        &body_directory,
                        &payload_directory,
                        &ledger_directory,
                    );
                let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
                let serve_ordinal = lease.ordinal();
                let producer_ordinal = owner.coordinator.producer_debts[&serve_ordinal];
                let receipt = owner
                    .payload_store
                    .persist_completed_with_exact_body(
                        &request,
                        &durable_body,
                        owner
                            .body_store
                            .as_ref()
                            .expect("unlaunched owner retains body store"),
                        &response,
                    )
                    .expect("persist terminal receipt for staged-drift preflight");
                let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
                    owner.coordinator.active_context,
                    &owner.coordinator.records[&serve_ordinal],
                    &owner.coordinator.durable_records[&serve_ordinal],
                    &owner.coordinator.records[&producer_ordinal],
                    &owner.coordinator.durable_records[&producer_ordinal],
                    receipt,
                )
                .expect("seal exact terminal replay pair");
                let transition = owner
                    .registry
                    .registry_mut()
                    .prepare_certified_serve_terminal_transition(
                        &owner.coordinator,
                        &lease,
                        &request,
                        &terminal,
                    )
                    .expect("prepare exact terminal registry transition");
                let outcome = terminal.terminal_outcome();
                let mut staged = owner.coordinator.stage_durable_transaction();
                staged.reduce_settle_turn(
                    lease.clone(),
                    super::super::super::TurnOutcome::Terminal(outcome),
                    Some(terminal),
                );
                assert_eq!(staged.fault(), None);

                match drift {
                    StagedTerminalDrift::Record => {
                        let mut extra = staged.records[&producer_ordinal].clone();
                        extra.ordinal = u128::MAX - 1;
                        assert!(staged.records.insert(extra.ordinal, extra).is_none());
                    }
                    StagedTerminalDrift::Index => {
                        let key = staged.records[&serve_ordinal].key;
                        assert!(staged.key_index.remove(&key).is_some());
                    }
                    StagedTerminalDrift::Debt => {
                        assert!(staged.producer_debts.remove(&serve_ordinal).is_some());
                    }
                    StagedTerminalDrift::Capacity => {
                        *staged
                            .capacity_used
                            .get_mut(&super::super::super::CapacityClass::Effect)
                            .expect("effect capacity counter exists") += 1;
                    }
                    StagedTerminalDrift::HighWater => {
                        staged.high_water = staged
                            .high_water
                            .checked_add(1)
                            .expect("fixture high-water has room");
                    }
                }

                let records = owner.coordinator.records.clone();
                let durable_records = owner.coordinator.durable_records.clone();
                let mut callback_invoked = false;
                let result = owner
                    .registry
                    .registry_mut()
                    .publish_certified_serve_terminal_transition(
                        transition,
                        &owner.coordinator,
                        &staged,
                        &lease,
                        || {
                            callback_invoked = true;
                            Ok::<(), ()>(())
                        },
                    );
                assert!(matches!(
                    result,
                    Err(
                        super::super::super::work_registry::CertifiedServeTerminalRegistryPublicationError::Preflight(
                            _
                        )
                    )
                ));
                assert!(!callback_invoked);
                assert_eq!(owner.coordinator.records, records);
                assert_eq!(owner.coordinator.durable_records, durable_records);
                assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    (1, 1)
                );
                assert!(
                    owner
                        .registry
                        .registry_mut()
                        .one_certified_serve_pair_shares_replay_family()
                );
                assert!(
                    owner
                        .registry
                        .registry_mut()
                        .preflight_certified_serve_terminal_owner_state(
                            &owner.coordinator,
                            &lease,
                        )
                );
            }
        }

        #[test]
        fn terminal_owner_registry_mismatch_faults_before_payload_persistence() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("terminal-owner-registry-mismatch", 0x9D);
            let body_directory = TempDir::new().expect("temporary registry-owner body store");
            let payload_directory = TempDir::new().expect("temporary registry-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary registry-owner ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0x9E, 3);
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .remove_one_certified_serve_carrier_for_test()
            );
            let payloads = snapshot_files(payload_directory.path());

            let error = owner
                .settle_certified_serve_negative(
                    lease,
                    &request,
                    CertifiedServePayloadNegativeOutcome::Rejected(43),
                )
                .expect_err("private registry mismatch requires restart");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::Registry
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1),
                "terminal preflight must not mutate the already-mismatched registry"
            );
        }

        #[test]
        fn terminal_owner_ledger_drift_restores_both_current_carriers() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("terminal-owner-ledger-drift", 0xA1);
            let body_directory = TempDir::new().expect("temporary drift-owner body store");
            let payload_directory = TempDir::new().expect("temporary drift-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary drift-owner ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0xA2, 3);
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("terminal owner retains LedgerV1 store")
                .persist(&fixture.ledger(Vec::new()))
                .expect("drift the on-disk LedgerV1 before terminal publication");
            let pending_payloads = snapshot_files(payload_directory.path());

            let error = owner
                .settle_certified_serve_negative(
                    lease,
                    &request,
                    CertifiedServePayloadNegativeOutcome::Failed(47),
                )
                .expect_err("exact LedgerV1 drift rejects terminal successor");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::Ledger
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1),
                "Ledger failure restores the byte-for-byte current Serve/Producer pair"
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family()
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                pending_payloads,
                "the fsynced terminal payload remains as a startup reconciliation tail"
            );
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
        }

        #[test]
        fn terminal_owner_postrename_sync_failure_keeps_logical_and_registry_state() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("terminal-owner-postrename", 0xA5);
            let body_directory = TempDir::new().expect("temporary postrename-owner body store");
            let payload_directory =
                TempDir::new().expect("temporary postrename-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary postrename-owner ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0xA6, 3);
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            let pending_payloads = snapshot_files(payload_directory.path());
            owner
                .payload_store
                .fail_next_publish_directory_sync_for_test();

            let error = owner
                .settle_certified_serve_negative(
                    lease,
                    &request,
                    CertifiedServePayloadNegativeOutcome::Rejected(53),
                )
                .expect_err("post-rename sync ambiguity requires restart");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                pending_payloads,
                "ambiguous renamed terminal frame remains for startup"
            );
        }

        #[test]
        fn fresh_certified_serve_rejects_foreign_target_and_rolls_back_capacity_wait() {
            let fixture = RecoveryFixture::new("fresh-serve-preledger", 0x91);
            let body_directory = TempDir::new().expect("temporary preledger body store");
            let payload_directory = TempDir::new().expect("temporary preledger payload store");
            let ledger_directory = TempDir::new().expect("temporary preledger ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0x92, 3);
            let foreign = fixture.authenticated_serve_request(1, 0x93, 3);
            let foreign_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    foreign.request_hash(),
                    1,
                );
            let payload_before = snapshot_files(payload_directory.path());
            let foreign_outcome =
                owner.admit_selected_certified_serve(foreign_target, &fixture.keys[0], &request);
            assert_eq!(
                foreign_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::SelectorAuthority
                )
            );
            let Ok(foreign_continuation) = foreign_outcome.into_safe_continuation() else {
                panic!("foreign target rejection is a safe pre-persistence continuation")
            };
            let recovered_foreign_target = foreign_continuation.into_target();
            assert!(
                recovered_foreign_target.matches_certified_serve_request(foreign.request_hash())
            );
            assert!(
                !recovered_foreign_target.matches_certified_serve_request(request.request_hash())
            );
            assert_eq!(snapshot_files(payload_directory.path()), payload_before);

            let admitted_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    2,
                );
            assert!(
                owner
                    .admit_selected_certified_serve(admitted_target, &fixture.keys[0], &request)
                    .into_safe_continuation()
                    .is_ok()
            );
            let payload_after_first = snapshot_files(payload_directory.path());
            let waiting = fixture.authenticated_serve_request(2, 0x94, 3);
            let waiting_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    waiting.request_hash(),
                    3,
                );
            let waiting_outcome =
                owner.admit_selected_certified_serve(waiting_target, &fixture.keys[0], &waiting);
            assert!(matches!(
                waiting_outcome.decision(),
                Some(super::super::super::AdmissionDecision::WaitForCapacity(_))
            ));
            assert_eq!(
                waiting_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            let Ok(waiting_continuation) = waiting_outcome.into_safe_continuation() else {
                panic!("proven Pending rollback must release the selector continuation")
            };
            assert!(
                waiting_continuation
                    .into_target()
                    .matches_certified_serve_request(waiting.request_hash())
            );
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_after_first,
                "a proven pre-ledger capacity decline must synchronously remove only its fresh Pending frame"
            );
            assert_eq!(owner.coordinator.records.len(), 2);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
        }

        #[test]
        fn fresh_certified_serve_postledger_failure_retains_tail_and_requires_restart() {
            let fixture = RecoveryFixture::new("fresh-serve-restart", 0xA1);
            let body_directory = TempDir::new().expect("temporary restart body store");
            let payload_directory = TempDir::new().expect("temporary restart payload store");
            let ledger_directory = TempDir::new().expect("temporary restart ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let changed =
                LifecycleLedgerV1::new(fixture.lifecycle_context(), 1, Vec::new(), BTreeMap::new())
                    .expect("construct changed pre-publication LedgerV1");
            owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("fresh owner retains LedgerV1 store")
                .persist(&changed)
                .expect("replace LedgerV1 before exact successor publication");
            let request = fixture.authenticated_serve_request(0, 0xA2, 3);
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(outcome.restart_required());
            assert_eq!(
                outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Ledger
                )
            );
            let Err(retained) = outcome.into_safe_continuation() else {
                panic!("post-ledger failure must not release the selector target")
            };
            assert!(retained.restart_required());
            drop(retained);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 0),
                "failed LedgerV1 publication rolls back both staged registry carriers"
            );
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                BTreeMap::new(),
                "the authenticated post-fsync payload tail remains for restart recovery"
            );

            let reentry = fixture.authenticated_serve_request(1, 0xA3, 3);
            let reentry_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    reentry.request_hash(),
                    2,
                );
            let payload_before_reentry = snapshot_files(payload_directory.path());
            let reentry_outcome =
                owner.admit_selected_certified_serve(reentry_target, &fixture.keys[0], &reentry);
            assert!(reentry_outcome.restart_required());
            assert_eq!(
                reentry_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            assert!(reentry_outcome.into_safe_continuation().is_err());
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_before_reentry,
                "a faulted owner must retain the new selector target without touching payload storage"
            );
        }

        #[test]
        fn fresh_certified_serve_postrename_sync_failure_requires_restart() {
            let fixture = RecoveryFixture::new("fresh-serve-postrename-sync", 0xA5);
            let body_directory = TempDir::new().expect("temporary post-rename body store");
            let payload_directory = TempDir::new().expect("temporary post-rename payload store");
            let ledger_directory = TempDir::new().expect("temporary post-rename ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            owner
                .payload_store
                .fail_next_publish_directory_sync_for_test();
            let request = fixture.authenticated_serve_request(0, 0xA6, 3);
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );

            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(outcome.restart_required());
            assert_eq!(
                outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::PayloadStore
                )
            );
            assert!(outcome.into_safe_continuation().is_err());
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert!(owner.coordinator.records.is_empty());
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 0)
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                BTreeMap::new(),
                "the renamed frame is an opaque crash tail, never a retryable unchanged attempt"
            );
        }

        #[test]
        fn ledgerless_owner_requires_restart_before_selector_validation() {
            let fixture = RecoveryFixture::new("ledgerless-serve-owner", 0xA9);
            let body_directory = TempDir::new().expect("temporary ledgerless body store");
            let payload_directory = TempDir::new().expect("temporary ledgerless payload store");
            let ledger_directory = TempDir::new().expect("temporary ledgerless ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0xAA, 3);
            let foreign = fixture.authenticated_serve_request(1, 0xAB, 3);
            let foreign_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    foreign.request_hash(),
                    1,
                );
            let _detached_store = owner
                .coordinator
                .ledger_store
                .take()
                .expect("fresh owner starts with its exact LedgerV1 store");

            let outcome =
                owner.admit_selected_certified_serve(foreign_target, &fixture.keys[0], &request);
            assert!(outcome.restart_required());
            assert_eq!(
                outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            assert!(outcome.into_safe_continuation().is_err());
            assert_eq!(snapshot_files(payload_directory.path()), BTreeMap::new());
        }

        #[test]
        fn completed_certified_serve_tombstone_replays_without_a_serve_carrier() {
            let fixture = RecoveryFixture::new("completed-serve-replay", 0xB1);
            let body_directory = TempDir::new().expect("temporary completed body store");
            let payload_directory = TempDir::new().expect("temporary completed payload store");
            let ledger_directory = TempDir::new().expect("temporary completed ledger store");
            let (mut owner, request) = fixture.open_terminal_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
                ServeTerminalFixture::Completed,
            );
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(matches!(
                outcome.decision(),
                Some(super::super::super::AdmissionDecision::ReplayTerminal {
                    outcome: TerminalOutcome::Completed(Some(_)),
                    ..
                })
            ));
            assert!(outcome.into_safe_continuation().is_ok());
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );

            let foreign_retainer_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    2,
                );
            let foreign_retainer = owner.admit_selected_certified_serve(
                foreign_retainer_target,
                &fixture.keys[1],
                &request,
            );
            assert!(foreign_retainer.restart_required());
            assert_eq!(
                foreign_retainer.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            assert!(foreign_retainer.into_safe_continuation().is_err());
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
        }

        #[test]
        fn payload_store_ahead_terminal_startup_installs_only_the_live_producer() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("store-ahead-serve-replay", 0xB5);
            let body_directory = TempDir::new().expect("temporary store-ahead body store");
            let payload_directory = TempDir::new().expect("temporary store-ahead payload store");
            let ledger_directory = TempDir::new().expect("temporary store-ahead ledger store");
            let body_store = fixture.open_store(&body_directory);
            let request = fixture.authenticated_serve_request(0, 0xD3, 3);
            let (mut payload_store, recovery) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("open store-ahead Serve payload store");
            assert!(recovery.is_empty());
            let pending = payload_store
                .persist_pending_with_verified_retention(
                    &fixture.verified,
                    &fixture.keys[0],
                    &request,
                )
                .expect("persist store-ahead Pending frame");
            let authority =
                authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
                    .expect("construct store-ahead lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
            assert!(matches!(
                coordinator
                    .admit_certified_serve(&fixture.verified, &request, pending)
                    .expect("project store-ahead Serve request"),
                super::super::super::AdmissionDecision::Admitted { .. }
            ));
            let ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project Pending store-ahead LedgerV1");
            payload_store
                .persist_negative(
                    pending.id(),
                    CertifiedServePayloadNegativeOutcome::Rejected(29),
                )
                .expect("persist store-ahead negative tombstone");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            drop(payload_store);
            let (payload_store, recovered) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("reopen store-ahead Serve payload store");
            let payloads = recovered
                .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
                .expect("authenticate store-ahead Serve payload");
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal store-ahead storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open store-ahead production owner");

            assert_eq!(
                owner.coordinator.records[&1].state,
                LifecycleState::Terminal(TerminalOutcome::Rejected(29))
            );
            assert!(
                !owner.coordinator.records[&1].physical_slots.is_empty(),
                "store-ahead settlement retains non-executable former Pending geometry"
            );
            assert_eq!(owner.coordinator.records[&2].state, LifecycleState::Ready);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&owner.coordinator)
            );
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(matches!(
                outcome.decision(),
                Some(super::super::super::AdmissionDecision::StutterTerminal { .. })
            ));
            assert!(outcome.into_safe_continuation().is_ok());
        }

        #[test]
        fn negative_and_cancelled_certified_serve_tombstones_stutter_exactly() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            for (index, terminal) in [
                CertifiedServePayloadNegativeOutcome::Rejected(17),
                CertifiedServePayloadNegativeOutcome::Cancelled,
            ]
            .into_iter()
            .enumerate()
            {
                let fixture = RecoveryFixture::new(
                    &format!("negative-serve-replay-{index}"),
                    0xC1 + u8::try_from(index).expect("small fixture index") * 4,
                );
                let body_directory = TempDir::new().expect("temporary negative body store");
                let payload_directory = TempDir::new().expect("temporary negative payload store");
                let ledger_directory = TempDir::new().expect("temporary negative ledger store");
                let (mut owner, request) = fixture.open_terminal_serve_owner(
                    &body_directory,
                    &payload_directory,
                    &ledger_directory,
                    ServeTerminalFixture::Negative(terminal),
                );
                let expected_carriers = match terminal {
                    CertifiedServePayloadNegativeOutcome::Cancelled => (0, 0),
                    CertifiedServePayloadNegativeOutcome::Rejected(_)
                    | CertifiedServePayloadNegativeOutcome::Failed(_) => (0, 1),
                };
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    expected_carriers
                );
                let target =
                    super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                        fixture.verified.context(),
                        request.request_hash(),
                        1,
                    );
                let outcome =
                    owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
                assert!(matches!(
                    outcome.decision(),
                    Some(super::super::super::AdmissionDecision::StutterTerminal { .. })
                ));
                assert!(outcome.into_safe_continuation().is_ok());
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    expected_carriers
                );

                let foreign_retainer_target =
                    super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                        fixture.verified.context(),
                        request.request_hash(),
                        2,
                    );
                let foreign_retainer = owner.admit_selected_certified_serve(
                    foreign_retainer_target,
                    &fixture.keys[1],
                    &request,
                );
                assert!(foreign_retainer.restart_required());
                assert_eq!(
                    foreign_retainer.failure(),
                    Some(
                        super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                    )
                );
                assert!(foreign_retainer.into_safe_continuation().is_err());
                assert_eq!(
                    owner.coordinator.fault(),
                    Some(super::super::super::CoordinatorFault::DurabilityFailure)
                );
            }
        }

        #[test]
        fn production_owner_rejects_changed_store_and_corrupt_census_without_further_writes() {
            let fixture = RecoveryFixture::new("changed-production-owner-store", 0x61);
            let body_directory = TempDir::new().expect("temporary changed-store body root");
            let body_store = fixture.open_store(&body_directory);
            let payload_directory = TempDir::new().expect("temporary changed-store payload root");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(Vec::new());
            let ledger_directory = TempDir::new().expect("temporary changed-store ledger root");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal changed-store production cut");
            let changed =
                LifecycleLedgerV1::new(fixture.lifecycle_context(), 1, Vec::new(), BTreeMap::new())
                    .expect("construct same-context changed ledger frame");
            cut.ledger_store
                .persist(&changed)
                .expect("replace the retained store after cut mint");
            let ledger_after_external_change = snapshot_files(ledger_directory.path());
            let body_before_failure = snapshot_files(body_directory.path());
            let payload_before_failure = snapshot_files(payload_directory.path());
            let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
                panic!("same-store frame change must fail closed")
            };
            assert!(matches!(
                error.kind,
                ProductionLifecycleStartupErrorKindV1::InvalidStorageCut
                    | ProductionLifecycleStartupErrorKindV1::LedgerFrameMismatch
            ));
            assert_eq!(
                snapshot_files(ledger_directory.path()),
                ledger_after_external_change
            );
            assert_eq!(snapshot_files(body_directory.path()), body_before_failure);
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_before_failure
            );

            let fixture = RecoveryFixture::new("corrupt-production-owner-census", 0x71);
            let body_directory = TempDir::new().expect("temporary corrupt-census body root");
            let mut body_store = fixture.open_store(&body_directory);
            let fetch = fixture.fetch_record(&mut body_store, 0, 0x72, 1, None, false);
            let payload_directory = TempDir::new().expect("temporary corrupt-census payload root");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(vec![fetch]);
            let ledger_directory = TempDir::new().expect("temporary corrupt-census ledger root");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let mut cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal corrupt-census production cut");
            cut.corrupt_fetch_census_for_test();
            let ledger_before_failure = snapshot_files(ledger_directory.path());
            let body_before_failure = snapshot_files(body_directory.path());
            let payload_before_failure = snapshot_files(payload_directory.path());
            let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
                panic!("corrupt all-row Fetch census must fail closed")
            };
            assert!(matches!(
                error.kind,
                ProductionLifecycleStartupErrorKindV1::InvalidStorageCut
            ));
            assert_eq!(
                snapshot_files(ledger_directory.path()),
                ledger_before_failure
            );
            assert_eq!(snapshot_files(body_directory.path()), body_before_failure);
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_before_failure
            );
        }

        #[test]
        fn production_owner_rejects_an_unsupported_live_class_before_publication() {
            let fixture = RecoveryFixture::new("unsupported-live-production-owner", 0x81);
            let replay = super::super::super::replay_authority::exact_record_fixture(
                fixture.lifecycle_context(),
                LifecycleStageKind::SignProposal,
                0x82,
            );
            let causal_root = CausalRoot::new(LifecycleDigest::new([0x83; 32]));
            let record = LifecycleLedgerRecordV1::new(
                replay.key,
                OwnerId::new(causal_root, 1),
                1,
                replay.work_class,
                replay.stage,
                None,
                causal_root.digest(),
                replay.payload,
                replay.authority,
                DurableContinuation::None,
            )
            .expect("construct unsupported live SignProposal row");
            let ledger = fixture.ledger(vec![record]);
            let body_directory = TempDir::new().expect("temporary unsupported-live body root");
            let body_store = fixture.open_store(&body_directory);
            let payload_directory =
                TempDir::new().expect("temporary unsupported-live payload root");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger_directory = TempDir::new().expect("temporary unsupported-live ledger root");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal unsupported-live storage cut before exhaustive classification");
            let before = (
                snapshot_files(ledger_directory.path()),
                snapshot_files(body_directory.path()),
                snapshot_files(payload_directory.path()),
            );
            let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
                panic!("unsupported live class must fail closed")
            };
            assert!(matches!(
                error.kind,
                ProductionLifecycleStartupErrorKindV1::Recovery(_)
            ));
            assert_eq!(snapshot_files(ledger_directory.path()), before.0);
            assert_eq!(snapshot_files(body_directory.path()), before.1);
            assert_eq!(snapshot_files(payload_directory.path()), before.2);
        }

        #[test]
        fn consuming_storage_cut_rejects_foreign_context_store_sources_and_qc() {
            let fixture = RecoveryFixture::new("durable-ready-fetch-rejections", 0x51);
            let foreign = RecoveryFixture::new("foreign-durable-ready-fetch", 0x61);

            let exact_empty_ledger = fixture.ledger(Vec::new());
            let exact_empty_body_directory =
                TempDir::new().expect("temporary exact empty body store");
            let exact_empty_body_store = fixture.open_store(&exact_empty_body_directory);
            let foreign_ledger = foreign.ledger(Vec::new());
            let foreign_ledger_directory =
                TempDir::new().expect("temporary foreign lifecycle ledger store");
            let foreign_ledger_store =
                foreign.persist_ledger(&foreign_ledger_directory, &foreign_ledger);
            assert!(matches!(
                exact_empty_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    foreign_ledger_store,
                    exact_empty_body_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidLedgerStore)
            ));

            let foreign_context_directory =
                TempDir::new().expect("temporary foreign-context body store");
            let mut foreign_context_store = fixture.open_store(&foreign_context_directory);
            let foreign_context_record =
                fixture.fetch_record(&mut foreign_context_store, 0, 0x71, 1, None, false);
            let foreign_context_ledger = fixture.ledger(vec![foreign_context_record]);
            let foreign_context_ledger_directory =
                TempDir::new().expect("temporary foreign-context lifecycle ledger");
            let foreign_context_ledger_store =
                fixture.persist_ledger(&foreign_context_ledger_directory, &foreign_context_ledger);
            assert!(matches!(
                foreign_context_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    foreign.verified.clone(),
                    foreign_context_ledger_store,
                    foreign_context_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidVerifiedContext)
            ));

            let foreign_store_directory =
                TempDir::new().expect("temporary exact-context body store");
            let mut exact_store = fixture.open_store(&foreign_store_directory);
            let exact_record = fixture.fetch_record(&mut exact_store, 0, 0x72, 1, None, false);
            let foreign_body_directory =
                TempDir::new().expect("temporary foreign body-store context");
            let foreign_store = foreign.open_store(&foreign_body_directory);
            let exact_ledger = fixture.ledger(vec![exact_record]);
            let exact_ledger_directory =
                TempDir::new().expect("temporary exact-context lifecycle ledger");
            let exact_ledger_store = fixture.persist_ledger(&exact_ledger_directory, &exact_ledger);
            assert!(matches!(
                exact_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    exact_ledger_store,
                    foreign_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidBodyStoreContext)
            ));

            let wrong_sources_directory =
                TempDir::new().expect("temporary wrong-sources body store");
            let mut wrong_sources_store = fixture.open_store(&wrong_sources_directory);
            let wrong_sources = vec![fixture.verified.context().roster[0].validator.clone()];
            let wrong_sources_record = fixture.fetch_record(
                &mut wrong_sources_store,
                0,
                0x73,
                1,
                Some(wrong_sources),
                false,
            );
            assert!(
                wrong_sources_record
                    .authenticate_durable_certified_fetch(&fixture.verified, || -> Result<
                        AuthenticatedDurableBodyFrameRecovery,
                        DurableBodyFrameRecoveryError,
                    > {
                        panic!("body-store authority must not be minted before source rejection")
                    })
                    .expect("source rejection does not inspect the body store")
                    .is_none()
            );
            let wrong_sources_ledger = fixture.ledger(vec![wrong_sources_record]);
            let wrong_sources_ledger_directory =
                TempDir::new().expect("temporary wrong-sources lifecycle ledger");
            let wrong_sources_ledger_store =
                fixture.persist_ledger(&wrong_sources_ledger_directory, &wrong_sources_ledger);
            assert!(matches!(
                wrong_sources_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    wrong_sources_ledger_store,
                    wrong_sources_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidReplayJoin)
            ));

            let corrupt_qc_directory = TempDir::new().expect("temporary corrupt-QC body store");
            let mut corrupt_qc_store = fixture.open_store(&corrupt_qc_directory);
            let corrupt_qc_record =
                fixture.fetch_record(&mut corrupt_qc_store, 0, 0x74, 1, None, true);
            let corrupt_qc_ledger = fixture.ledger(vec![corrupt_qc_record]);
            let corrupt_qc_ledger_directory =
                TempDir::new().expect("temporary corrupt-QC lifecycle ledger");
            let corrupt_qc_ledger_store =
                fixture.persist_ledger(&corrupt_qc_ledger_directory, &corrupt_qc_ledger);
            assert!(matches!(
                corrupt_qc_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    corrupt_qc_ledger_store,
                    corrupt_qc_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidReplayJoin)
            ));
        }
    }

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
        super::super::replay_authority::exact_replay_authority_for_payload_fixture(
            record_context,
            stage.kind(),
            seed,
            payload,
        )
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
        let body_frame = exact_body_payload(LifecycleStageKind::StoreBody);
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
                LifecycleLedgerRecordV1::new_exact_replay_fixture(
                    key,
                    owner(1),
                    ordinal,
                    work_class,
                    body_stage(stage_kind),
                    terminal,
                    digest(9),
                    body_frame,
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
        let fetch_key = body_key(LifecyclePhase::Fetch, commitment);
        let store_key = body_key(LifecyclePhase::Store, commitment);
        let validate_key = body_key(LifecyclePhase::Validate, commitment);
        let store_frame = frame_for(store_key, 42);
        let foreign_frame = frame_for(validate_key, 43);
        let exact_fetch_frame = exact_body_payload(LifecycleStageKind::StoreBody);
        let fetch = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            fetch_key,
            owner(1),
            1,
            LifecycleWorkClass::Fetch,
            body_stage(LifecycleStageKind::FetchBody),
            Some(TerminalOutcome::Advanced),
            digest(9),
            exact_fetch_frame,
            DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
        )
        .expect("construct BodyFrame-backed Fetch parent");
        let store_child = LifecycleLedgerRecordV1::new_exact_replay_fixture(
            store_key,
            owner(1),
            2,
            LifecycleWorkClass::Store,
            body_stage(LifecycleStageKind::StoreBody),
            None,
            digest(9),
            exact_fetch_frame,
            DurableContinuation::None,
        )
        .expect("construct exact Store child");
        LifecycleLedgerV1::new(
            context(),
            2,
            vec![fetch.clone(), store_child.clone()],
            BTreeMap::new(),
        )
        .expect("Fetch-to-Store preserves the exact body frame");
        let mut payload_free_fetch = fetch.clone();
        payload_free_fetch.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(fetch_key, DurablePayloadReference::None)
                .expect("encode payload-free Fetch negative");
        assert_invalid_records(2, vec![payload_free_fetch, store_child.clone()]);
        let mut foreign_store = store_child;
        foreign_store.payload_reference =
            LifecyclePayloadReferenceV1::from_schema(store_key, frame_for(store_key, 43))
                .expect("encode substituted Store body frame");
        assert_invalid_records(2, vec![fetch, foreign_store]);
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
