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
use norito::codec::{Decode, DecodeAll, Encode};
use thiserror::Error;

use super::schema::{MAX_LIFECYCLE_RECORDS_PER_HEIGHT, serve_and_producer_keys_match};
use super::{
    CausalRoot, DurablePayloadReference, DurableServeNegativeOutcome, LifecycleContext,
    LifecycleCoordinator, LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRound,
    LifecycleStage, LifecycleStageKind, LifecycleState, LifecycleWorkClass, OwnerId,
    PhysicalSlotId, PredecessorScope, RecoveredLifecycleRecord, RecoverySnapshot, TerminalOutcome,
};

const LEDGER_FILE: &str = "lifecycle-ledger-v1.norito";
const LEGACY_SERVE_V5_FILE: &str = "certified-serve-state.norito";
const LEDGER_MAGIC: &[u8; 8] = b"SUMV2LC1";
const LEDGER_VERSION: u16 = 1;
const HASH_BYTES: usize = 32;
const HEADER_BYTES: usize = LEDGER_MAGIC.len() + 2 + 8 + HASH_BYTES;
const MAX_LEDGER_FRAME_BYTES: u64 = 64 * 1024 * 1024;

const PAYLOAD_NONE: u16 = 0;
const PAYLOAD_CERTIFIED_SERVE_PENDING: u16 = 1;
const PAYLOAD_CERTIFIED_SERVE_COMPLETED: u16 = 2;
const PAYLOAD_CERTIFIED_SERVE_NEGATIVE: u16 = 3;
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
        })
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

    fn validate(&self, context: LifecycleContext, high_water: u128) -> bool {
        let Some(key) = self.key() else {
            return false;
        };
        let Some(work_class) = self.work_class() else {
            return false;
        };
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
            })
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

    /// Borrow the canonical Serve-to-producer debts.
    pub(super) fn producer_debts(&self) -> &[LifecycleProducerDebtV1] {
        &self.producer_debts
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

/// Typed LifecycleLedgerV1 load or persistence failure.
#[derive(Debug, Error)]
pub(super) enum LifecycleLedgerError {
    /// The retired Serve v5 file requires the authorized fresh reset.
    #[error("legacy Sumeragi certified-Serve v5 state at {0} requires the authorized fresh reset")]
    LegacyV5RequiresReset(PathBuf),
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

/// Crash-safe, bounded store for one height-local LifecycleLedgerV1.
#[derive(Clone, Debug)]
pub(super) struct LifecycleLedgerStoreV1 {
    path: PathBuf,
    context: LifecycleContext,
    max_records: usize,
    max_frame_bytes: u64,
}

impl LifecycleLedgerStoreV1 {
    /// Open a height-local ledger under the coordinator's sealed size bounds,
    /// rejecting the retired v5 Serve format.
    pub(super) fn open(
        root: &Path,
        context: LifecycleContext,
    ) -> Result<(Self, LifecycleLedgerV1), LifecycleLedgerError> {
        let legacy_path = root.join(LEGACY_SERVE_V5_FILE);
        match fs::symlink_metadata(&legacy_path) {
            Ok(_) => return Err(LifecycleLedgerError::LegacyV5RequiresReset(legacy_path)),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(LifecycleLedgerError::Io(format!(
                    "failed to inspect retired Serve v5 state {}: {error}",
                    legacy_path.display()
                )));
            }
        }
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
        LifecycleKey::new(
            context().id(),
            LifecycleRound::new(7, u64::from(seed)),
            Some(LifecycleRound::new(7, u64::from(seed))),
            Some(digest(seed)),
            phase,
            None,
        )
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

    fn serve_pair() -> (LifecycleLedgerRecordV1, LifecycleLedgerRecordV1) {
        let serve = LifecycleLedgerRecordV1::new(
            key(2, LifecyclePhase::Serve),
            owner(1),
            1,
            LifecycleWorkClass::CertifiedServe,
            stage(LifecycleStageKind::CertifiedServe),
            Some(TerminalOutcome::Completed(Some(digest(23)))),
            digest(20),
            DurablePayloadReference::CertifiedServeCompleted {
                request: digest(21),
                certificate: digest(22),
                response: digest(23),
            },
        )
        .expect("valid Serve ledger record");
        let producer = LifecycleLedgerRecordV1::new(
            key(2, LifecyclePhase::ProducerTurn),
            owner(1),
            2,
            LifecycleWorkClass::ProducerTurn,
            stage(LifecycleStageKind::ProducerTurn),
            None,
            digest(20),
            DurablePayloadReference::None,
        )
        .expect("valid producer ledger record");
        (serve, producer)
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
    fn store_rejects_legacy_v5_without_a_migration_path() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(root.path().join(LEGACY_SERVE_V5_FILE), b"SUMV2SRV").expect("legacy fixture");
        let error = LifecycleLedgerStoreV1::open(root.path(), context())
            .expect_err("legacy v5 must fail startup");
        assert!(matches!(
            &error,
            LifecycleLedgerError::LegacyV5RequiresReset(_)
        ));
        assert!(error.to_string().contains("authorized fresh reset"));
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
