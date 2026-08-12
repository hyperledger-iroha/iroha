//! Sealed durable-open and authenticated restart reconciliation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use iroha_config::parameters::actual::SumeragiV2Config;
use thiserror::Error;

use super::{
    AdmissionDecision, AdmissionRequest, CandidateAdmission, CoordinatorFault,
    DurablePayloadReference, LifecycleContext, LifecycleCoordinator, LifecycleDigest, LifecycleKey,
    LifecycleState, LifecycleWorkClass, RolloverSnapshot, TerminalOutcome,
    authority::{self, AuthenticatedEpisodeAuthority},
    ledger::{
        LifecycleLedgerError, LifecycleLedgerRecordV1, LifecycleLedgerStoreV1, LifecycleLedgerV1,
    },
};
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_certified_serve_payload_store::{
        AuthenticatedCertifiedServePayloadRecoveryCut,
        AuthenticatedRecoveredCertifiedServePayloadState, CertifiedServePayloadId,
        CertifiedServePayloadNegativeOutcome, CertifiedServePayloadStoreError,
        CertifiedServePayloadStoreV1, DurableCertifiedServeAdmissionReceipt,
    },
};

/// Move-only, post-authentication join between durable logical rows and their
/// exact storage-reconstructed work.
///
/// Constructors stay inside the lifecycle authority. Production storage code
/// receives this value only after the exhaustive effect classifier, body/WAL
/// reconciliation, and Certified-Serve payload resolver have authenticated all
/// of its parts. The move-only payload cut may retain authenticated store-only
/// crash tails; durable open removes those orphans only after every ledger
/// Serve resolves exactly and the reconciled ledger has been published.
#[derive(Debug)]
#[must_use]
pub(crate) struct AuthenticatedLifecycleRecoveryCut {
    context: LifecycleContext,
    candidates: BTreeMap<LifecycleKey, CandidateAdmission>,
    serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
}

impl AuthenticatedLifecycleRecoveryCut {
    /// Assemble an exact cut from already authenticated, sealed projections.
    /// Duplicate semantic keys are rejected rather than overwritten.
    pub(super) fn from_authenticated_parts(
        context: LifecycleContext,
        candidates: impl IntoIterator<Item = CandidateAdmission>,
        serve_payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Option<Self> {
        if digest_bytes(serve_payloads.context_id().0.as_ref()) != context.id()
            || serve_payloads.height() != context.height()
        {
            return None;
        }
        let mut candidate_map = BTreeMap::new();
        for candidate in candidates {
            if matches!(
                candidate.work_class,
                LifecycleWorkClass::CertifiedServe | LifecycleWorkClass::ProducerTurn
            ) || candidate_map.insert(candidate.key, candidate).is_some()
            {
                return None;
            }
        }
        Some(Self {
            context,
            candidates: candidate_map,
            serve_payloads,
        })
    }
}

/// Failure to open the sole durable lifecycle authority for one height.
#[derive(Debug, Error)]
#[error("{0}")]
pub(crate) struct LifecycleOpenError(LifecycleOpenErrorKind);

#[derive(Debug, Error)]
enum LifecycleOpenErrorKind {
    #[error("verified height context cannot derive bounded lifecycle authority")]
    InvalidAuthority,
    #[error("authenticated lifecycle recovery cut is inconsistent: {0}")]
    InvalidRecovery(&'static str),
    #[error(transparent)]
    Ledger(#[from] LifecycleLedgerError),
    #[error(transparent)]
    PayloadStore(#[from] CertifiedServePayloadStoreError),
}

impl From<LifecycleOpenErrorKind> for LifecycleOpenError {
    fn from(error: LifecycleOpenErrorKind) -> Self {
        Self(error)
    }
}

impl From<LifecycleLedgerError> for LifecycleOpenError {
    fn from(error: LifecycleLedgerError) -> Self {
        Self(LifecycleOpenErrorKind::Ledger(error))
    }
}

impl From<CertifiedServePayloadStoreError> for LifecycleOpenError {
    fn from(error: CertifiedServePayloadStoreError) -> Self {
        Self(LifecycleOpenErrorKind::PayloadStore(error))
    }
}

impl LifecycleCoordinator {
    /// Open the sole durable coordinator from a verified height context.
    ///
    /// The persisted ledger owns the ordinal high-water mark. Every live row
    /// must join exactly one authenticated recovery candidate (ProducerTurn
    /// rows join through their adjacent Serve), and every Serve row must join
    /// its payload-store reference. Rebinding and payload-store-ahead terminal
    /// cuts are persisted before this method returns. Authenticated payloads
    /// with no ledger owner are then durably pruned through `payload_store`.
    pub(crate) fn open_from_verified_height_context(
        verified: &VerifiedHeightContext,
        config: &SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        let authority =
            authority::production_authority(verified, config, reply_route_source_capacity)
                .ok_or(LifecycleOpenErrorKind::InvalidAuthority)?;
        Self::open_with_authority(authority, ledger_root, payload_store, recovery)
    }

    pub(super) fn open_with_authority(
        authority: AuthenticatedEpisodeAuthority,
        ledger_root: &Path,
        payload_store: &mut CertifiedServePayloadStoreV1,
        mut recovery: AuthenticatedLifecycleRecoveryCut,
    ) -> Result<Self, LifecycleOpenError> {
        let context = authority.context();
        if recovery.context != context {
            return Err(LifecycleOpenErrorKind::InvalidRecovery("foreign recovery context").into());
        }
        payload_store.validate_authenticated_cut(&recovery.serve_payloads)?;
        let (store, ledger) = LifecycleLedgerStoreV1::open(ledger_root, context)?;
        let records_by_key = decoded_records_by_key(&ledger)?;
        let (serve_candidates, terminal_updates, retained_serve_payloads) =
            resolve_serve_payloads(context, &ledger, &records_by_key, &recovery.serve_payloads)?;
        for candidate in serve_candidates {
            if recovery
                .candidates
                .insert(candidate.key, candidate)
                .is_some()
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "Serve projection collided with non-Serve recovery work",
                )
                .into());
            }
        }

        let mut physical_universes = ledger
            .records()
            .iter()
            .map(|record| (record.ordinal(), BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut candidates_by_ordinal = BTreeMap::new();
        let mut producer_coverage = BTreeSet::new();
        for (_, mut candidate) in std::mem::take(&mut recovery.candidates) {
            candidate.canonicalize_geometry().map_err(|_| {
                LifecycleOpenErrorKind::InvalidRecovery("invalid physical geometry")
            })?;
            let record = records_by_key.get(&candidate.key).copied().ok_or(
                LifecycleOpenErrorKind::InvalidRecovery(
                    "recovered candidate has no durable semantic row",
                ),
            )?;
            validate_candidate_record(record, &candidate)?;
            let ordinal = record.ordinal();
            if candidates_by_ordinal
                .insert(ordinal, candidate.clone())
                .is_some()
            {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "multiple candidates cover one durable row",
                )
                .into());
            }
            if record.terminal().flatten().is_none() {
                let (_, universe, _) = candidate.physical_geometry.normalized().map_err(|_| {
                    LifecycleOpenErrorKind::InvalidRecovery("invalid primary geometry")
                })?;
                if !authority.admits_slots(candidate.work_class.capacity_class(), &universe) {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "primary geometry exceeds authenticated capacity",
                    )
                    .into());
                }
                physical_universes.insert(ordinal, universe);
            }
            match (candidate.work_class, candidate.producer_turn.as_ref()) {
                (LifecycleWorkClass::CertifiedServe, Some(producer)) => {
                    let producer_ordinal =
                        ordinal
                            .checked_add(1)
                            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                                "producer ordinal overflowed",
                            ))?;
                    let producer_record = ledger_record_at(&ledger, producer_ordinal).ok_or(
                        LifecycleOpenErrorKind::InvalidRecovery(
                            "Serve has no adjacent durable producer",
                        ),
                    )?;
                    if producer_record.key() != Some(producer.key)
                        || producer_record.owner() != record.owner()
                        || producer_record.work_class() != Some(LifecycleWorkClass::ProducerTurn)
                        || producer_record.stage() != Some(producer.stage)
                        || producer_record.reconstruction_source() != producer.reconstruction_source
                    {
                        return Err(LifecycleOpenErrorKind::InvalidRecovery(
                            "producer companion changed durable semantics",
                        )
                        .into());
                    }
                    if producer_record.terminal().flatten().is_none() {
                        let (_, universe, _) =
                            producer.physical_geometry.normalized().map_err(|_| {
                                LifecycleOpenErrorKind::InvalidRecovery("invalid producer geometry")
                            })?;
                        if !authority.admits_slots(
                            LifecycleWorkClass::ProducerTurn.capacity_class(),
                            &universe,
                        ) || !producer_coverage.insert(producer_ordinal)
                        {
                            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                                "producer geometry or coverage is invalid",
                            )
                            .into());
                        }
                        physical_universes.insert(producer_ordinal, universe);
                    }
                }
                (LifecycleWorkClass::CertifiedServe, None) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "recovered Serve lacks its producer companion",
                    )
                    .into());
                }
                (_, Some(_)) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "non-Serve candidate carries a producer companion",
                    )
                    .into());
                }
                (_, None) => {}
            }
        }

        let mut required_candidates = BTreeSet::new();
        let mut required_producers = BTreeSet::new();
        for record in ledger.records() {
            let terminal = record
                .terminal()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "durable terminal cannot be decoded",
                ))?;
            match (record.work_class(), terminal) {
                (Some(LifecycleWorkClass::ProducerTurn), None) => {
                    required_producers.insert(record.ordinal());
                }
                (Some(_), None) => {
                    required_candidates.insert(record.ordinal());
                }
                (Some(LifecycleWorkClass::CertifiedServe), Some(_)) => {
                    if record
                        .ordinal()
                        .checked_add(1)
                        .and_then(|ordinal| ledger_record_at(&ledger, ordinal))
                        .is_some_and(|producer| producer.terminal().flatten().is_none())
                    {
                        required_candidates.insert(record.ordinal());
                    }
                }
                (Some(_), Some(_)) => {}
                (None, _) => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "durable work class cannot be decoded",
                    )
                    .into());
                }
            }
        }
        if required_candidates != candidates_by_ordinal.keys().copied().collect()
            || required_producers != producer_coverage
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "live durable record coverage is not exact",
            )
            .into());
        }

        let snapshot = ledger.recovery_snapshot(physical_universes)?;
        let mut coordinator =
            LifecycleCoordinator::new_with_authority(authority, ledger.high_water());
        coordinator.reconcile_restart(snapshot);
        if coordinator.fault == Some(CoordinatorFault::RecoveryRejected) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "coordinator rejected the reconstructed durable state",
            )
            .into());
        }
        for (ordinal, candidate) in candidates_by_ordinal {
            if matches!(
                coordinator.records[&ordinal].state,
                LifecycleState::Terminal(_)
            ) {
                coordinator.rebind_terminal_serve_producer(ordinal, candidate)?;
                continue;
            }
            match coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)) {
                AdmissionDecision::Retry {
                    ordinal: rebound, ..
                } if rebound == ordinal => {}
                _ => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "recovered candidate did not rebind exactly",
                    )
                    .into());
                }
            }
        }
        for (ordinal, outcome, expected_payload) in terminal_updates {
            coordinator.finish_terminal(ordinal, outcome).map_err(|_| {
                LifecycleOpenErrorKind::InvalidRecovery(
                    "payload-store terminal cut could not settle its Serve",
                )
            })?;
            if coordinator.durable_records[&ordinal].payload != expected_payload {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal payload projection changed during settlement",
                )
                .into());
            }
        }
        if coordinator.records.values().any(|record| {
            matches!(
                record.state,
                LifecycleState::Waiting(wait)
                    if matches!(wait.source, super::WaitSource::Recovery(_))
            )
        }) {
            return Err(
                LifecycleOpenErrorKind::InvalidRecovery("recovery work remains unbound").into(),
            );
        }
        store.persist(&super::ledger::LifecycleLedgerV1::from_coordinator(
            &coordinator,
        )?)?;
        payload_store
            .prune_authenticated_orphans(&recovery.serve_payloads, &retained_serve_payloads)?;
        coordinator.ledger_store = Some(store);
        Ok(coordinator)
    }

    /// Retire one context and durably open its immediate verified successor.
    ///
    /// With an attached ledger, payload-store cancellation receipts are
    /// required for every live Serve. The retired tombstones are persisted
    /// first; the successor's empty, high-water-preserving ledger is then
    /// published. Either crash cut is idempotently recoverable.
    #[cfg(test)]
    pub(crate) fn rollover(&mut self, snapshot: RolloverSnapshot) {
        self.rollover_inner(snapshot, None);
    }

    /// Retire one durable context together with every payload-first Serve
    /// capacity fence, then open its immediate successor.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn rollover_with_payload_store(
        &mut self,
        snapshot: RolloverSnapshot,
        payload_store: &mut CertifiedServePayloadStoreV1,
    ) {
        self.rollover_inner(snapshot, Some(payload_store));
    }

    fn rollover_inner(
        &mut self,
        snapshot: RolloverSnapshot,
        payload_store: Option<&mut CertifiedServePayloadStoreV1>,
    ) {
        if self.fault.is_some() {
            return;
        }
        if !self.rollover_snapshot_is_exact(&snapshot) {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        if self.ledger_store.is_none() {
            let mut next = self.stage_durable_transaction();
            if snapshot.successor_ledger_root.is_some()
                || !snapshot.serve_cancellations.is_empty()
                || next.retire_for_rollover(&snapshot).is_err()
            {
                self.fault = Some(CoordinatorFault::InvalidRollover);
                return;
            }
            next.activate_successor(snapshot);
            *self = next;
            return;
        }
        let Some(successor_root) = snapshot.successor_ledger_root.as_deref() else {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        };
        if !self.serve_cancellation_receipts_are_exact(&snapshot) {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        let Some(serve_wait_rollbacks) = self.serve_wait_rollback_receipts() else {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        };

        let mut retired = self.stage_durable_transaction();
        if retired.retire_for_rollover(&snapshot).is_err() {
            self.fault = Some(CoordinatorFault::InvalidRollover);
            return;
        }
        if !serve_wait_rollbacks.is_empty()
            && payload_store
                .ok_or(())
                .and_then(|store| {
                    store
                        .rollback_pending_batch(&serve_wait_rollbacks)
                        .map_err(|_| ())
                })
                .is_err()
        {
            self.fault = Some(CoordinatorFault::DurabilityFailure);
            return;
        }
        let retired_projection = match LifecycleLedgerV1::from_coordinator(&retired) {
            Ok(ledger) => ledger,
            Err(_) => {
                self.fault = Some(CoordinatorFault::DurabilityFailure);
                return;
            }
        };
        if retired
            .ledger_store
            .as_ref()
            .expect("durable rollover retains its predecessor store")
            .persist(&retired_projection)
            .is_err()
        {
            self.fault = Some(CoordinatorFault::DurabilityFailure);
            return;
        }

        let successor_store =
            match LifecycleLedgerStoreV1::open(successor_root, snapshot.successor_context) {
                Ok((store, existing))
                    if existing.records().is_empty()
                        && existing.producer_debts().is_empty()
                        && (existing.high_water() == 0
                            || existing.high_water() == snapshot.retained_high_water) =>
                {
                    store
                }
                Ok(_) | Err(_) => {
                    retired.fault = Some(CoordinatorFault::DurabilityFailure);
                    *self = retired;
                    return;
                }
            };
        let mut successor = LifecycleCoordinator::new_with_authority(
            snapshot.successor_authority.clone(),
            snapshot.retained_high_water,
        );
        let successor_projection = match LifecycleLedgerV1::from_coordinator(&successor) {
            Ok(ledger) => ledger,
            Err(_) => {
                retired.fault = Some(CoordinatorFault::DurabilityFailure);
                *self = retired;
                return;
            }
        };
        if successor_store.persist(&successor_projection).is_err() {
            retired.fault = Some(CoordinatorFault::DurabilityFailure);
            *self = retired;
            return;
        }
        successor.ledger_store = Some(successor_store);
        *self = successor;
    }

    fn rollover_snapshot_is_exact(&self, snapshot: &RolloverSnapshot) -> bool {
        let live_ordinals = self
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (!matches!(record.state, LifecycleState::Terminal(_))).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        let pending_keys = self
            .admission_waits
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        self.active_lease.is_none()
            && self.active_context == snapshot.retired_context
            && snapshot.successor_context.id != snapshot.retired_context.id
            && snapshot.successor_predecessor == snapshot.retired_context.id
            && snapshot.successor_authority.context() == snapshot.successor_context
            && snapshot.retired_context.height.checked_add(1)
                == Some(snapshot.successor_context.height)
            && snapshot.retained_high_water == self.high_water
            && snapshot.retire_ordinals == live_ordinals
            && snapshot.retire_admission_keys == pending_keys
    }

    fn serve_cancellation_receipts_are_exact(&self, snapshot: &RolloverSnapshot) -> bool {
        let mut cancellations = BTreeMap::new();
        for receipt in &snapshot.serve_cancellations {
            if receipt.outcome() != CertifiedServePayloadNegativeOutcome::Cancelled {
                return false;
            }
            let request = digest_bytes(receipt.id().request_hash().as_ref());
            let certificate = digest_bytes(receipt.certificate_hash().as_ref());
            if cancellations.insert(request, certificate).is_some() {
                return false;
            }
        }
        let mut expected = BTreeMap::new();
        for record in self.records.values().filter(|record| {
            record.work_class == LifecycleWorkClass::CertifiedServe
                && !matches!(record.state, LifecycleState::Terminal(_))
        }) {
            let DurablePayloadReference::CertifiedServePending {
                request,
                certificate,
            } = self.durable_records[&record.ordinal].payload
            else {
                return false;
            };
            if expected.insert(request, certificate).is_some() {
                return false;
            }
        }
        expected == cancellations
    }

    fn serve_wait_rollback_receipts(&self) -> Option<Vec<DurableCertifiedServeAdmissionReceipt>> {
        let mut receipts = Vec::new();
        for waiting in self.admission_waits.values() {
            match (waiting.candidate.work_class, waiting.serve_payload_receipt) {
                (LifecycleWorkClass::CertifiedServe, Some(receipt)) => receipts.push(receipt),
                (LifecycleWorkClass::CertifiedServe, None) | (_, Some(_)) => return None,
                (_, None) => {}
            }
        }
        Some(receipts)
    }

    fn retire_for_rollover(&mut self, snapshot: &RolloverSnapshot) -> Result<(), CoordinatorFault> {
        for ordinal in &snapshot.retire_ordinals {
            if !self
                .records
                .get(ordinal)
                .is_some_and(|record| matches!(record.state, LifecycleState::Terminal(_)))
            {
                self.finish_terminal(*ordinal, TerminalOutcome::Cancelled)?;
            }
        }
        for key in &snapshot.retire_admission_keys {
            self.admission_waits.remove(key);
        }
        if !self.producer_debts.is_empty()
            || self.capacity_used.values().any(|used| *used != 0)
            || self
                .records
                .values()
                .any(|record| !matches!(record.state, LifecycleState::Terminal(_)))
        {
            return Err(CoordinatorFault::InvalidRollover);
        }
        Ok(())
    }

    fn activate_successor(&mut self, snapshot: RolloverSnapshot) {
        self.records.clear();
        self.key_index.clear();
        self.ready_index.clear();
        self.owner_index.clear();
        self.durable_records.clear();
        self.producer_debts.clear();
        self.observed_generation.clear();
        self.capacity_generation
            .values_mut()
            .for_each(|generation| *generation = 0);
        self.next_lease = Some(1);
        self.capacity_geometry = snapshot.successor_authority.capacity_geometry().clone();
        self.episode_authority = snapshot.successor_authority;
        self.active_context = snapshot.successor_context;
    }

    fn rebind_terminal_serve_producer(
        &mut self,
        serve_ordinal: u128,
        mut candidate: CandidateAdmission,
    ) -> Result<(), LifecycleOpenError> {
        candidate.canonicalize_geometry().map_err(|_| {
            LifecycleOpenErrorKind::InvalidRecovery("invalid terminal Serve geometry")
        })?;
        let serve =
            self.records
                .get(&serve_ordinal)
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve record disappeared",
                ))?;
        if serve.work_class != LifecycleWorkClass::CertifiedServe
            || !matches!(serve.state, LifecycleState::Terminal(_))
            || serve.key != candidate.key
            || serve.owner.causal_root() != candidate.causal_root
            || serve.stage != candidate.stage
            || !self.durable_records[&serve_ordinal]
                .payload
                .same_admission_material(candidate.payload)
            || !self.retry_companion_matches(serve, &candidate)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Serve recovery companion changed semantics",
            )
            .into());
        }
        let producer_ordinal = self.producer_debts.get(&serve_ordinal).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("terminal Serve has no live producer debt"),
        )?;
        let producer =
            candidate
                .producer_turn
                .as_ref()
                .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve lacks producer geometry",
                ))?;
        let (physical, universe, consumed) = producer
            .physical_geometry
            .normalized()
            .map_err(|_| LifecycleOpenErrorKind::InvalidRecovery("invalid producer geometry"))?;
        let record = self.records.get_mut(&producer_ordinal).ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("terminal Serve producer disappeared"),
        )?;
        if record.episode.slot_universe != universe
            || !record.physical_slots.is_empty()
            || !record.episode.consumed_slots.is_empty()
            || !matches!(record.state, LifecycleState::Ready)
        {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "terminal Serve producer cannot be rebound",
            )
            .into());
        }
        record.physical_slots = physical;
        record.episode.consumed_slots = consumed;
        Ok(())
    }
}

fn decoded_records_by_key(
    ledger: &super::ledger::LifecycleLedgerV1,
) -> Result<BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>, LifecycleOpenError> {
    let mut records = BTreeMap::new();
    for record in ledger.records() {
        let key = record.key().ok_or(LifecycleOpenErrorKind::InvalidRecovery(
            "durable key cannot be decoded",
        ))?;
        if records.insert(key, record).is_some() {
            return Err(
                LifecycleOpenErrorKind::InvalidRecovery("duplicate durable semantic key").into(),
            );
        }
    }
    Ok(records)
}

fn ledger_record_at(
    ledger: &super::ledger::LifecycleLedgerV1,
    ordinal: u128,
) -> Option<&LifecycleLedgerRecordV1> {
    ledger
        .records()
        .binary_search_by_key(&ordinal, LifecycleLedgerRecordV1::ordinal)
        .ok()
        .and_then(|index| ledger.records().get(index))
}

fn digest_bytes(bytes: &[u8]) -> LifecycleDigest {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(bytes);
    LifecycleDigest::new(digest)
}

fn validate_candidate_record(
    record: &LifecycleLedgerRecordV1,
    candidate: &CandidateAdmission,
) -> Result<(), LifecycleOpenError> {
    if record.owner().causal_root() != candidate.causal_root
        || record.work_class() != Some(candidate.work_class)
        || record.stage() != Some(candidate.stage)
        || record.reconstruction_source() != candidate.reconstruction_source
        || record
            .durable_payload()
            .is_none_or(|payload| !payload.same_admission_material(candidate.payload))
        || candidate.initial_state != super::InitialLifecycleState::Ready
    {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "recovered candidate changed durable semantics",
        )
        .into());
    }
    Ok(())
}

type TerminalUpdate = (u128, TerminalOutcome, DurablePayloadReference);

#[allow(clippy::too_many_lines)]
fn resolve_serve_payloads(
    context: LifecycleContext,
    ledger: &LifecycleLedgerV1,
    records: &BTreeMap<LifecycleKey, &LifecycleLedgerRecordV1>,
    recovered: &AuthenticatedCertifiedServePayloadRecoveryCut,
) -> Result<
    (
        Vec<CandidateAdmission>,
        Vec<TerminalUpdate>,
        BTreeSet<CertifiedServePayloadId>,
    ),
    LifecycleOpenError,
> {
    if digest_bytes(recovered.context_id().0.as_ref()) != context.id()
        || recovered.height() != context.height()
    {
        return Err(
            LifecycleOpenErrorKind::InvalidRecovery("foreign Certified-Serve payload cut").into(),
        );
    }
    let mut recovered_by_request = BTreeMap::new();
    for payload in recovered.iter() {
        let request = digest_bytes(payload.id().request_hash().as_ref());
        if recovered_by_request.insert(request, payload).is_some() {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "duplicate authenticated Serve request identity",
            )
            .into());
        }
    }

    let mut candidates = Vec::new();
    let mut updates = Vec::new();
    let mut retained = BTreeSet::new();
    for (key, record) in records {
        if record.work_class() != Some(LifecycleWorkClass::CertifiedServe) {
            continue;
        }
        let durable = record
            .durable_payload()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve payload cannot be decoded",
            ))?;
        let request = durable
            .request()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve ledger row lost its signed-request identity",
            ))?;
        let payload = recovered_by_request.get(&request).copied().ok_or(
            LifecycleOpenErrorKind::InvalidRecovery("Serve payload is missing from storage"),
        )?;
        retained.insert(payload.id());
        let (candidate, resolved, projected_terminal) =
            super::projection::recovered_certified_serve_projection(context, payload)
                .map_err(|_| {
                    LifecycleOpenErrorKind::InvalidRecovery(
                        "authenticated Serve payload could not be projected",
                    )
                })?
                .into_parts();
        if candidate.key != *key {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve six-field key changed its body/request identity",
            )
            .into());
        }
        if !durable.same_admission_material(resolved) {
            return Err(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve payload changed request or certificate identity",
            )
            .into());
        }
        let ledger_terminal = record
            .terminal()
            .ok_or(LifecycleOpenErrorKind::InvalidRecovery(
                "Serve terminal cannot be decoded",
            ))?;
        if durable == resolved {
            if ledger_terminal != projected_terminal {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "Serve payload state disagrees with its ledger terminal",
                )
                .into());
            }
        } else {
            let outcome = match (durable, resolved, projected_terminal) {
                (
                    DurablePayloadReference::CertifiedServePending { .. },
                    DurablePayloadReference::CertifiedServeCompleted { response, .. },
                    Some(TerminalOutcome::Completed(Some(projected_response))),
                ) if response == projected_response => TerminalOutcome::Completed(Some(response)),
                (
                    DurablePayloadReference::CertifiedServePending { .. },
                    DurablePayloadReference::CertifiedServeNegative { outcome, .. },
                    Some(projected),
                ) if outcome.terminal() == projected => projected,
                _ => {
                    return Err(LifecycleOpenErrorKind::InvalidRecovery(
                        "Serve payload storage regressed or conflicts with the ledger",
                    )
                    .into());
                }
            };
            if ledger_terminal.is_some() {
                return Err(LifecycleOpenErrorKind::InvalidRecovery(
                    "terminal Serve payload disagrees with its ledger tombstone",
                )
                .into());
            }
            updates.push((record.ordinal(), outcome, resolved));
        }

        let producer_is_live = record
            .ordinal()
            .checked_add(1)
            .and_then(|ordinal| ledger_record_at(ledger, ordinal))
            .is_some_and(|producer| {
                producer.work_class() == Some(LifecycleWorkClass::ProducerTurn)
                    && producer.terminal() == Some(None)
            });
        if ledger_terminal.is_none() || producer_is_live {
            candidates.push(candidate);
        }
    }
    if recovered.iter().any(|payload| {
        !retained.contains(&payload.id())
            && !matches!(
                payload.state(),
                AuthenticatedRecoveredCertifiedServePayloadState::Pending
            )
    }) {
        return Err(LifecycleOpenErrorKind::InvalidRecovery(
            "terminal Serve payload has no durable ledger owner",
        )
        .into());
    }
    Ok((candidates, updates, retained))
}
