/// Exact installed output row authorized for post-service terminal removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PreparedLifecycleOutputRegistryRetirementV1 {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    key: LifecycleKey,
}
impl PreparedLifecycleOutputRegistryRetirementV1 {
    /// Return the immutable ordinal of the exact output row being retired.
    pub(super) const fn ordinal(self) -> u128 {
        self.address.ordinal
    }
}

/// Classification of one runtime output against the concrete lifecycle census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LifecycleOutputRegistryJoinV1 {
    /// No exact concrete row exists; direct-signed admission may be attempted.
    Missing,
    /// A byte-identical retransmit remains owned by its durable recovered
    /// Broadcast carrier and must not enter generic service I/O.
    RecoveredBroadcastOwned,
    /// A byte-identical direct output already has an exact terminal durable
    /// row. Logical admission always stutters on that row; only a sealed fresh
    /// periodic-retransmit occurrence may repeat physical Broadcast service.
    TerminalDirectOutputDuplicate,
    /// A process-local carrier was reinstalled at the same exact address as an
    /// already-durable terminal direct output. Only the volatile carrier must
    /// be retired; the `Terminal(Advanced)` ledger row remains unchanged.
    TerminalInstalledDuplicate(PreparedLifecycleOutputRegistryRetirementV1),
    /// The exact row exists but an older Ready row or an active claim owns the turn.
    Deferred,
    /// The exact row is the next lifecycle-owned output allowed to execute.
    Ready(PreparedLifecycleOutputRegistryRetirementV1),
}

/// Exact concrete kind retained beneath one schedulable Ready Broadcast row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyLifecycleBroadcastCarrierV1 {
    /// An ordinary runtime output remains owned by generic output settlement.
    RetainedDirectOutput(ReadyRetainedDirectBroadcastAttestationV1),
    /// A typed recovered Sign successor owns one dedicated refanout transaction.
    RecoveredRefanout,
}

/// Classification of one exact Broadcast row admitted to the current
/// scheduler census.
///
/// Unlike [`ReadyLifecycleBroadcastCarrierV1`], this seal may describe a
/// direct output that the caller's exact reducer-fence observation will wake
/// during the same planning transaction. Recovered refanout remains
/// Ready-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SchedulableLifecycleBroadcastCarrierV1 {
    /// An ordinary runtime output remains owned by generic output settlement.
    RetainedDirectOutput(SchedulableRetainedDirectBroadcastAttestationV1),
    /// A typed recovered Sign successor owns one dedicated refanout transaction.
    RecoveredRefanout,
}

/// Registry seal for one Ready direct Broadcast which scheduling may observe
/// but must never claim through a recovered-refanout or fresh-I/O path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReadyRetainedDirectBroadcastAttestationV1 {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
}
impl ReadyRetainedDirectBroadcastAttestationV1 {
    /// Rejoin the seal to the same immutable Ready row before minting a passive
    /// scheduler input.
    pub(super) fn matches_ready_record(self, record: &super::LifecycleRecord) -> bool {
        record.state == super::LifecycleState::Ready
            && record.work_class == LifecycleWorkClass::Broadcast
            && record.owner == self.address.owner
            && record.ordinal == self.address.ordinal
            && exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
                == Some((self.address.slot, self.digest))
    }
}

/// Registry seal for one exact direct Broadcast in the current scheduler
/// census, including its pre-planning Ready or reducer-fence Waiting state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SchedulableRetainedDirectBroadcastAttestationV1 {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    state: super::LifecycleState,
}
impl SchedulableRetainedDirectBroadcastAttestationV1 {
    /// Rejoin the seal to the unchanged row before minting a scheduler input.
    pub(super) fn matches_schedulable_record(self, record: &super::LifecycleRecord) -> bool {
        record.state == self.state
            && record.work_class == LifecycleWorkClass::Broadcast
            && record.owner == self.address.owner
            && record.ordinal == self.address.ordinal
            && exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
                == Some((self.address.slot, self.digest))
    }
}

/// Move-only authority to settle one exact Ready direct Broadcast after Apply.
///
/// The pending-map key is copied from the already-installed `PendingAdapter`
/// carrier. Callers can neither select another pending output nor widen this
/// authority to diagnostic output classes.
#[must_use = "the attested direct Broadcast must be settled or failed closed"]
pub(in crate::sumeragi) struct PreparedApplyTerminalDirectBroadcastV1 {
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    pending_key: LifecycleOutputAdmissionKeyV1,
    _linearity: ApplyTerminalDirectBroadcastLinearityV1,
}

struct ApplyTerminalDirectBroadcastLinearityV1;

impl Drop for ApplyTerminalDirectBroadcastLinearityV1 {
    fn drop(&mut self) {}
}

impl PreparedApplyTerminalDirectBroadcastV1 {
    /// Return the immutable lifecycle ordinal of the attested Broadcast row.
    pub(in crate::sumeragi) const fn ordinal(&self) -> u128 {
        self.address.ordinal
    }

    /// Return the exact executor pending-map key copied from the installed carrier.
    pub(in crate::sumeragi) const fn pending_key(&self) -> LifecycleOutputAdmissionKeyV1 {
        self.pending_key
    }
}

/// Proof that the sole executor-pending output is the already-installed exact
/// CommitQC Broadcast immediately after one exact Ready live Decision Apply.
///
/// This proof does not authorize output service. It only closes the scheduling
/// cycle in which the globally earlier Apply must terminalize before the
/// ordinary post-Apply Broadcast corridor can service this exact row.
#[must_use = "the attested post-Apply output census must stay bound to its Apply"]
pub(in crate::sumeragi) struct AttestedLifecycleDecisionApplySuccessorOutputsV1 {
    live_apply: LiveLifecycleDecisionApplyReconciliationAuthorityV1,
    output_address: ConcreteWorkAddress,
    output_digest: LifecycleDigest,
    pending_key: LifecycleOutputAdmissionKeyV1,
    _seal: LifecycleDecisionApplySuccessorOutputsSealV1,
}

#[derive(Debug)]
struct LifecycleDecisionApplySuccessorOutputsSealV1;

impl AttestedLifecycleDecisionApplySuccessorOutputsV1 {
    /// Return the immutable Apply dispatch key governing this suffix.
    pub(in crate::sumeragi) const fn dispatch_key(
        &self,
    ) -> LifecycleDecisionApplyDispatchKeyV1 {
        self.live_apply.dispatch_key()
    }

    /// Recheck the sole retained reducer suffix against the exact live Apply.
    pub(in crate::sumeragi) fn exactly_matches_retransmit_apply(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.live_apply.exactly_matches_retransmit_apply(effect)
    }

    /// Return the exact number of pending direct outputs in the frozen census.
    pub(in crate::sumeragi) const fn pending_count(&self) -> usize {
        1
    }

    /// Recheck the whole current pending-map key census without exposing output bytes.
    #[allow(single_use_lifetimes)]
    pub(in crate::sumeragi) fn exactly_matches_pending_keys<'a>(
        &self,
        keys: impl Iterator<Item = &'a LifecycleOutputAdmissionKeyV1>,
    ) -> bool {
        keys.copied().eq(std::iter::once(self.pending_key))
    }

    /// Rejoin the terminal output preparation to the exact row and pending key
    /// authenticated before Apply dispatch.
    pub(in crate::sumeragi) fn exactly_matches_terminal_preparation(
        &self,
        prepared: &PreparedApplyTerminalDirectBroadcastV1,
    ) -> bool {
        self.output_address == prepared.address
            && self.output_digest == prepared.digest
            && self.pending_key == prepared.pending_key
    }
}

fn terminal_direct_output_matches_record(
    coordinator: &LifecycleCoordinator,
    ordinal: u128,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
) -> bool {
    let Some(expected_class) = lifecycle_output_work_class(effect) else {
        return false;
    };
    let Some(authority) = exact_direct_signed_admission_authority(effect, pending) else {
        return false;
    };
    let (Some(record), Some(metadata)) = (
        coordinator.records.get(&ordinal),
        coordinator.durable_records.get(&ordinal),
    ) else {
        return false;
    };
    let expected_slot = PhysicalSlotId::for_capacity(expected_class.capacity_class(), 0);
    let expected_digest = digest_from_hash(pending.exact_effect_identity());
    coordinator.fault.is_none()
        && coordinator.active_context.id() == record.key.context()
        && coordinator.active_context.height() == record.key.round().height()
        && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && record.ordinal == ordinal
        && record.work_class == expected_class
        && record
            .work_class
            .accepts_stage(record.key.phase(), record.stage)
        && record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
        && record.physical_slots == BTreeMap::from([(expected_slot, expected_digest)])
        && record.episode.slot_universe == std::collections::BTreeSet::from([expected_slot])
        && record.episode.consumed_slots == record.episode.slot_universe
        && metadata.reconstruction_source == record.owner.causal_root().digest()
        && metadata.payload == DurablePayloadReference::None
        && metadata.continuation == super::schema::DurableContinuation::None
        && metadata.replay_authority == authority
        && authority.structurally_matches_record(
            coordinator.active_context,
            record.key,
            record.work_class,
            record.stage,
            metadata.payload,
        )
}

fn pending_output_matches_work(
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
    work: &ConcreteLifecycleWork,
) -> bool {
    work.validate_exact()
        && matches!(
            &work.kind,
            ConcreteLifecycleWorkKind::PendingAdapter {
                effect: installed_effect,
                pending: installed_pending,
                replay_authority: _,
            } if installed_effect == effect
                && installed_pending == pending
        )
}

fn recovered_broadcast_output_matches_work(
    coordinator: &LifecycleCoordinator,
    address: ConcreteWorkAddress,
    work: &ConcreteLifecycleWork,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
) -> bool {
    let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) = &work.kind
    else {
        return false;
    };
    broadcast.exactly_matches_runtime_retransmit(address, work.digest, coordinator, effect, pending)
}

fn lifecycle_output_work_class(effect: &AdapterEffect) -> Option<LifecycleWorkClass> {
    match effect {
        AdapterEffect::Broadcast(_) => Some(LifecycleWorkClass::Broadcast),
        AdapterEffect::ReportEquivocation { .. } => Some(LifecycleWorkClass::EquivocationReport),
        AdapterEffect::ReportInvalidCertifiedBody { .. } => {
            Some(LifecycleWorkClass::InvalidBodyReport)
        }
        AdapterEffect::Sign { .. }
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::Apply { .. }
        | AdapterEffect::EnterView { .. } => None,
    }
}

fn lifecycle_output_row_matches(
    coordinator: &LifecycleCoordinator,
    address: ConcreteWorkAddress,
    work: &ConcreteLifecycleWork,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
) -> bool {
    let Some(expected_class) = lifecycle_output_work_class(effect) else {
        return false;
    };
    let (Some(record), Some(metadata)) = (
        coordinator.records.get(&address.ordinal),
        coordinator.durable_records.get(&address.ordinal),
    ) else {
        return false;
    };
    let ConcreteLifecycleWorkKind::PendingAdapter {
        replay_authority, ..
    } = &work.kind
    else {
        return false;
    };
    coordinator.fault.is_none()
        && coordinator.active_context.id() == record.key.context()
        && coordinator.active_context.height() == record.key.round().height()
        && record.owner == address.owner
        && record.ordinal == address.ordinal
        && record.owner.causal_root() == work.causal_root()
        && record.work_class == expected_class
        && record
            .work_class
            .accepts_stage(record.key.phase(), record.stage)
        && record.physical_slots == BTreeMap::from([(address.slot, work.digest)])
        && address.slot.capacity_class() == Some(expected_class.capacity_class())
        && record.episode.slot_universe == std::collections::BTreeSet::from([address.slot])
        && record.episode.consumed_slots == record.episode.slot_universe
        && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
        && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
        && metadata.reconstruction_source == record.owner.causal_root().digest()
        && metadata.payload == DurablePayloadReference::None
        && metadata.continuation == super::schema::DurableContinuation::None
        && metadata.replay_authority == *replay_authority
        && replay_authority.structurally_matches_record(
            coordinator.active_context,
            record.key,
            record.work_class,
            record.stage,
            metadata.payload,
        )
        && pending_output_matches_work(effect, pending, work)
}

impl ConcreteLifecycleWorkRegistry {
    /// Authenticate the sole exact CommitQC output immediately behind one
    /// exact Ready live Apply.
    ///
    /// Every pending owner must rejoin one unique installed `PendingAdapter`
    /// Broadcast row which is Ready, indexed, and strictly later than Apply.
    /// Missing/direct-terminal/recovered/diagnostic aliases are rejected.
    #[allow(single_use_lifetimes)]
    pub(super) fn attest_lifecycle_decision_apply_successor_outputs<'a>(
        &self,
        coordinator: &LifecycleCoordinator,
        authority: LiveLifecycleDecisionApplyReconciliationAuthorityV1,
        mut pending_outputs: impl ExactSizeIterator<Item = &'a PendingLifecycleOutputAdmissionV1>,
    ) -> Option<AttestedLifecycleDecisionApplySuccessorOutputsV1> {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return None;
        }
        let dispatch_key = authority.dispatch_key();
        if dispatch_key.lineage() != LifecycleDecisionApplyLineageV1::Live {
            return None;
        }
        let apply_ordinal = dispatch_key.lifecycle_ordinal();
        let apply_record = coordinator.records.get(&apply_ordinal)?;
        let apply_attestation = self
            .attest_ready_lifecycle_decision_apply(coordinator, apply_ordinal)
            .ok()?;
        if apply_attestation.dispatch_key() != dispatch_key
            || apply_record.state != super::LifecycleState::Ready
            || coordinator.ready_index.first().copied() != Some(apply_ordinal)
        {
            return None;
        }

        if pending_outputs.len() != 1 {
            return None;
        }
        let pending_output = pending_outputs.next()?;
        if pending_outputs.next().is_some() {
            return None;
        }
        let effect = &pending_output.effect;
        let pending = &pending_output.pending;
        let AdapterEffect::Broadcast(message) = effect else {
            return None;
        };
        let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &message.payload
        else {
            return None;
        };
        if certificate != authority.certificate()
            || certificate.phase != wire::GlobalPhase::Commit
        {
            return None;
        }
        let mut installed = self.entries.iter().filter(|(address, work)| {
            pending_output_matches_work(effect, pending, work)
                && address.owner.causal_root()
                    == super::CausalRoot::new(digest_from_hash(pending.causal_lifecycle_key()))
        });
        let (&address, work) = installed.next()?;
        if installed.next().is_some()
            || self.entries.iter().any(|(candidate, candidate_work)| {
                recovered_broadcast_output_matches_work(
                    coordinator,
                    *candidate,
                    candidate_work,
                    effect,
                    pending,
                )
            })
            || coordinator.records.keys().copied().any(|ordinal| {
                terminal_direct_output_matches_record(coordinator, ordinal, effect, pending)
            })
        {
            return None;
        }
        let record = coordinator.records.get(&address.ordinal)?;
        if address.ordinal <= apply_ordinal
            || coordinator.ready_index.iter().copied().nth(1) != Some(address.ordinal)
            || record.state != super::LifecycleState::Ready
            || record.work_class != LifecycleWorkClass::Broadcast
            || record.key.phase() != LifecyclePhase::BroadcastCommitQc
            || record.stage.kind() != LifecycleStageKind::BroadcastCommitQc
            || !lifecycle_output_row_matches(coordinator, address, work, effect, pending)
        {
            return None;
        }
        Some(AttestedLifecycleDecisionApplySuccessorOutputsV1 {
            live_apply: authority,
            output_address: address,
            output_digest: work.digest,
            pending_key: pending_output.key(),
            _seal: LifecycleDecisionApplySuccessorOutputsSealV1,
        })
    }

    fn owner_held_output_ordinals(
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> std::collections::BTreeSet<u128> {
        coordinator
            .records
            .values()
            .filter(|record| {
                !matches!(record.state, super::LifecycleState::Terminal(_))
                    && !extra.contains_record(record)
                    && matches!(
                        record.work_class,
                        LifecycleWorkClass::Broadcast
                            | LifecycleWorkClass::EquivocationReport
                            | LifecycleWorkClass::InvalidBodyReport
                    )
            })
            .map(|record| record.ordinal)
            .collect()
    }

    /// Classify one exact Ready Broadcast by its closed concrete carrier.
    ///
    /// Logical `Broadcast` is deliberately insufficient: ordinary direct
    /// output and recovered refanout share that work class but have disjoint
    /// execution owners. The returned direct-output seal authorizes only a
    /// passive full-census scheduler row; it does not transfer generic output
    /// settlement authority.
    pub(super) fn attest_ready_lifecycle_broadcast_carrier(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<ReadyLifecycleBroadcastCarrierV1, RegistryError> {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(RegistryError::CorruptWork);
        }
        let record = coordinator
            .records
            .get(&ordinal)
            .ok_or(RegistryError::Missing)?;
        if record.state != super::LifecycleState::Ready
            || record.work_class != LifecycleWorkClass::Broadcast
        {
            return Err(RegistryError::CorruptWork);
        }
        let (slot, digest) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
                .ok_or(RegistryError::InvalidAdmissionShape)?;
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(RegistryError::InvalidAddress)?;
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if work.digest != digest {
            return Err(RegistryError::DigestMismatch);
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::PendingAdapter {
                effect,
                pending,
                replay_authority: _,
            } if lifecycle_output_row_matches(coordinator, address, work, effect, pending) => {
                Ok(ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(
                    ReadyRetainedDirectBroadcastAttestationV1 { address, digest },
                ))
            }
            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast)
                if broadcast.matches_current_ready_record(address, digest, coordinator) =>
            {
                Ok(ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout)
            }
            ConcreteLifecycleWorkKind::PendingAdapter { .. }
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)
            | ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_)
            | ConcreteLifecycleWorkKind::DurableStoreBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateCompletion(_)
            | ConcreteLifecycleWorkKind::DurableLiveWalApply(_)
            | ConcreteLifecycleWorkKind::DurableLiveWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(_)
            | ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            | ConcreteLifecycleWorkKind::DurableProducerTurn(_) => Err(RegistryError::CorruptWork),
        }
    }

    /// Classify one exact Broadcast admitted to the current scheduler census.
    ///
    /// Ready rows retain the existing closed carrier classification. The sole
    /// additional state is a direct output Waiting on the exact context-scoped
    /// reducer fence supplied by the caller; that wait must be absent from the
    /// Ready index and strictly older than the observed generation. A recovered
    /// Broadcast can never borrow this direct-output wake path.
    pub(super) fn attest_schedulable_lifecycle_broadcast_carrier(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
        fence: Option<crate::sumeragi::v2::LifecycleReducerFenceObservationV1>,
    ) -> Result<SchedulableLifecycleBroadcastCarrierV1, RegistryError> {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(RegistryError::CorruptWork);
        }
        let record = coordinator
            .records
            .get(&ordinal)
            .ok_or(RegistryError::Missing)?;
        if record.work_class != LifecycleWorkClass::Broadcast {
            return Err(RegistryError::CorruptWork);
        }
        match record.state {
            super::LifecycleState::Ready => {
                if !coordinator.ready_index.contains(&ordinal) {
                    return Err(RegistryError::CorruptWork);
                }
                return self
                    .attest_ready_lifecycle_broadcast_carrier(coordinator, ordinal)
                    .map(|carrier| match carrier {
                        ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation) => {
                            SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(
                                SchedulableRetainedDirectBroadcastAttestationV1 {
                                    address: attestation.address,
                                    digest: attestation.digest,
                                    state: record.state,
                                },
                            )
                        }
                        ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout => {
                            SchedulableLifecycleBroadcastCarrierV1::RecoveredRefanout
                        }
                    });
            }
            super::LifecycleState::Waiting(wait)
                if fence.is_some_and(|fence| {
                    !coordinator.ready_index.contains(&ordinal)
                        && fence.source()
                            == super::projection::reducer_fence_wait_source(
                                coordinator.active_context,
                            )
                        && wait.source() == fence.source()
                        && wait.observed_generation() < fence.generation()
                        && coordinator.observed_generation.get(&wait.source())
                            == Some(&wait.observed_generation())
                }) => {}
            super::LifecycleState::Waiting(_)
            | super::LifecycleState::Claimed(_)
            | super::LifecycleState::Terminal(_) => {
                return Err(RegistryError::CorruptWork);
            }
        }
        let (slot, digest) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
                .ok_or(RegistryError::InvalidAdmissionShape)?;
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(RegistryError::InvalidAddress)?;
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if work.digest != digest {
            return Err(RegistryError::DigestMismatch);
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::PendingAdapter {
                effect,
                pending,
                replay_authority: _,
            } if lifecycle_output_row_matches(coordinator, address, work, effect, pending) => Ok(
                SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(
                    SchedulableRetainedDirectBroadcastAttestationV1 {
                        address,
                        digest,
                        state: record.state,
                    },
                ),
            ),
            ConcreteLifecycleWorkKind::PendingAdapter { .. }
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)
            | ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_)
            | ConcreteLifecycleWorkKind::DurableStoreBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateCompletion(_)
            | ConcreteLifecycleWorkKind::DurableLiveWalApply(_)
            | ConcreteLifecycleWorkKind::DurableLiveWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(_)
            | ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            | ConcreteLifecycleWorkKind::DurableProducerTurn(_) => Err(RegistryError::CorruptWork),
        }
    }

    /// Bind the global Ready minimum to its exact installed direct-Broadcast pending key.
    pub(super) fn prepare_apply_terminal_direct_broadcast(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<PreparedApplyTerminalDirectBroadcastV1, RegistryError> {
        let SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation) = self
            .attest_schedulable_lifecycle_broadcast_carrier(coordinator, ordinal, None)?
        else {
            return Err(RegistryError::CorruptWork);
        };
        let record = coordinator
            .records
            .get(&ordinal)
            .ok_or(RegistryError::Missing)?;
        if record.state != super::LifecycleState::Ready
            || coordinator.ready_index.first().copied() != Some(ordinal)
            || !attestation.matches_schedulable_record(record)
        {
            return Err(RegistryError::CorruptWork);
        }
        let work = self
            .entries
            .get(&attestation.address)
            .ok_or(RegistryError::Missing)?;
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect,
            pending,
            replay_authority: _,
        } = &work.kind
        else {
            return Err(RegistryError::CorruptWork);
        };
        if !matches!(effect, AdapterEffect::Broadcast(_))
            || work.digest != attestation.digest
            || !lifecycle_output_row_matches(
                coordinator,
                attestation.address,
                work,
                effect,
                pending,
            )
        {
            return Err(RegistryError::CorruptWork);
        }
        Ok(PreparedApplyTerminalDirectBroadcastV1 {
            address: attestation.address,
            digest: attestation.digest,
            pending_key: LifecycleOutputAdmissionKeyV1 {
                causal_lifecycle_key: *pending.causal_lifecycle_key().as_ref(),
                effect_identity: *pending.exact_effect_identity().as_ref(),
            },
            _linearity: ApplyTerminalDirectBroadcastLinearityV1,
        })
    }

    /// Rejoin a move-only pending output to the exact attested Ready Broadcast.
    pub(super) fn apply_terminal_direct_broadcast_pending_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        prepared: &PreparedApplyTerminalDirectBroadcastV1,
        pending: &PendingLifecycleOutputAdmissionV1,
    ) -> bool {
        let Some(record) = coordinator.records.get(&prepared.address.ordinal) else {
            return false;
        };
        let Some(work) = self.entries.get(&prepared.address) else {
            return false;
        };
        record.state == super::LifecycleState::Ready
            && coordinator.active_lease.is_none()
            && coordinator.ready_index.first().copied() == Some(prepared.address.ordinal)
            && work.digest == prepared.digest
            && pending.key() == prepared.pending_key
            && matches!(&pending.effect, AdapterEffect::Broadcast(_))
            && lifecycle_output_row_matches(
                coordinator,
                prepared.address,
                work,
                &pending.effect,
                &pending.pending,
            )
    }

    /// Join one runtime output to its sole exact installed lifecycle row.
    ///
    /// A matching row that is not the oldest Ready row remains deferred. This
    /// keeps service I/O behind the coordinator's immutable ordinal order while
    /// allowing a truly absent direct-signed output to enter admission.
    pub(super) fn join_lifecycle_output(
        &self,
        coordinator: &LifecycleCoordinator,
        execution: &PreparedLifecycleOutputExecutionV1,
    ) -> Result<LifecycleOutputRegistryJoinV1, RegistryError> {
        let pending = execution.exact_pending_binding();
        let (installed, installed_count) = {
            let mut matches = self.entries.iter().filter(|(address, work)| {
                pending_output_matches_work(&execution.effect, &pending, work)
                    && address.owner.causal_root()
                        == super::CausalRoot::new(digest_from_hash(pending.causal_lifecycle_key()))
            });
            let installed = matches.next();
            let count = usize::from(installed.is_some()).saturating_add(matches.count());
            (installed, count)
        };
        let (recovered_broadcast, recovered_broadcast_count) = {
            let mut matches = self.entries.iter().filter(|(address, work)| {
                recovered_broadcast_output_matches_work(
                    coordinator,
                    **address,
                    work,
                    &execution.effect,
                    &pending,
                )
            });
            let recovered_broadcast = matches.next();
            let count = usize::from(recovered_broadcast.is_some()).saturating_add(matches.count());
            (recovered_broadcast, count)
        };
        let (terminal_direct_output, terminal_direct_output_count) = {
            let mut matches = coordinator.records.keys().copied().filter(|ordinal| {
                terminal_direct_output_matches_record(
                    coordinator,
                    *ordinal,
                    &execution.effect,
                    &pending,
                )
            });
            let terminal = matches.next();
            let count = usize::from(terminal.is_some()).saturating_add(matches.count());
            (terminal, count)
        };
        let same_address_terminal_installed = installed_count == 1
            && recovered_broadcast_count == 0
            && terminal_direct_output_count == 1
            && installed
                .is_some_and(|(address, _)| terminal_direct_output == Some(address.ordinal));
        if installed_count > 1
            || recovered_broadcast_count > 1
            || terminal_direct_output_count > 1
            || (!same_address_terminal_installed
                && usize::from(installed.is_some())
                    .saturating_add(usize::from(recovered_broadcast.is_some()))
                    .saturating_add(usize::from(terminal_direct_output.is_some()))
                    > 1)
        {
            return Err(RegistryError::AmbiguousLifecycleOutputOwnership {
                installed: installed_count,
                recovered_broadcast: recovered_broadcast_count,
                terminal_direct_output: terminal_direct_output_count,
            });
        }
        let Some((&address, work)) = installed else {
            return Ok(if recovered_broadcast.is_some() {
                LifecycleOutputRegistryJoinV1::RecoveredBroadcastOwned
            } else if terminal_direct_output.is_some() {
                LifecycleOutputRegistryJoinV1::TerminalDirectOutputDuplicate
            } else {
                LifecycleOutputRegistryJoinV1::Missing
            });
        };
        if !lifecycle_output_row_matches(coordinator, address, work, &execution.effect, &pending) {
            return Err(RegistryError::CorruptWork);
        }
        let record = coordinator
            .records
            .get(&address.ordinal)
            .ok_or(RegistryError::Missing)?;
        let retirement = PreparedLifecycleOutputRegistryRetirementV1 {
            address,
            digest: work.digest,
            key: record.key,
        };
        if same_address_terminal_installed {
            return Ok(LifecycleOutputRegistryJoinV1::TerminalInstalledDuplicate(
                retirement,
            ));
        }
        if coordinator.active_lease.is_some()
            || record.state != super::LifecycleState::Ready
            || coordinator.ready_index.first().copied() != Some(address.ordinal)
        {
            return Ok(LifecycleOutputRegistryJoinV1::Deferred);
        }
        Ok(LifecycleOutputRegistryJoinV1::Ready(retirement))
    }

    /// Seal the exact generic retransmit used to exercise recovered-Broadcast
    /// output settlement without exposing the carrier or its message.
    #[cfg(test)]
    pub(super) fn recovered_broadcast_runtime_retransmit_for_test(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
        tag: crate::sumeragi::v2_core::EventTag,
        source_ordinal: u128,
    ) -> Option<PendingLifecycleOutputAdmissionV1> {
        let record = coordinator.records.get(&ordinal)?;
        if record.physical_slots.len() != 1 {
            return None;
        }
        let (&slot, &digest) = record.physical_slots.first_key_value()?;
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)?;
        let work = self.entries.get(&address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        (work.digest == digest).then_some(())?;
        broadcast.runtime_retransmit_for_test(address, digest, coordinator, tag, source_ordinal)
    }

    /// Recheck a staged terminal successor before its irreversible LedgerV1 fsync.
    pub(super) fn lifecycle_output_terminal_is_exact(
        &self,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        prepared: PreparedLifecycleOutputRegistryRetirementV1,
        execution: &PreparedLifecycleOutputExecutionV1,
    ) -> bool {
        let Some(work) = self.entries.get(&prepared.address) else {
            return false;
        };
        let pending = execution.exact_pending_binding();
        let (Some(current_record), Some(staged_record)) = (
            current.records.get(&prepared.address.ordinal),
            staged.records.get(&prepared.address.ordinal),
        ) else {
            return false;
        };
        lifecycle_output_row_matches(current, prepared.address, work, &execution.effect, &pending)
            && work.digest == prepared.digest
            && current_record.key == prepared.key
            && current_record.state == super::LifecycleState::Ready
            && staged_record.key == prepared.key
            && staged_record.owner == current_record.owner
            && staged_record.ordinal == current_record.ordinal
            && staged_record.work_class == current_record.work_class
            && staged_record.stage == current_record.stage
            && staged_record.physical_slots == current_record.physical_slots
            && staged_record.episode == current_record.episode
            && staged_record.state
                == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
            && !staged.ready_index.contains(&prepared.address.ordinal)
            && staged.key_index == current.key_index
            && staged.owner_index == current.owner_index
            && staged.active_lease.is_none()
            && staged.records.len() == current.records.len()
            && current.records.iter().all(|(ordinal, record)| {
                *ordinal == prepared.address.ordinal || staged.records.get(ordinal) == Some(record)
            })
    }

    /// Recheck a process-local carrier attached to the same exact address as
    /// its already-fsynced `Terminal(Advanced)` direct-output row.
    pub(super) fn lifecycle_output_terminal_installed_is_exact(
        &self,
        coordinator: &LifecycleCoordinator,
        prepared: PreparedLifecycleOutputRegistryRetirementV1,
        execution: &PreparedLifecycleOutputExecutionV1,
    ) -> bool {
        let Some(work) = self.entries.get(&prepared.address) else {
            return false;
        };
        let pending = execution.exact_pending_binding();
        let installed_count = self
            .entries
            .iter()
            .filter(|(address, work)| {
                pending_output_matches_work(&execution.effect, &pending, work)
                    && address.owner.causal_root()
                        == super::CausalRoot::new(digest_from_hash(pending.causal_lifecycle_key()))
            })
            .count();
        let recovered_count = self
            .entries
            .iter()
            .filter(|(address, work)| {
                recovered_broadcast_output_matches_work(
                    coordinator,
                    **address,
                    work,
                    &execution.effect,
                    &pending,
                )
            })
            .count();
        let terminal_count = coordinator
            .records
            .keys()
            .copied()
            .filter(|ordinal| {
                terminal_direct_output_matches_record(
                    coordinator,
                    *ordinal,
                    &execution.effect,
                    &pending,
                )
            })
            .count();
        lifecycle_output_row_matches(
            coordinator,
            prepared.address,
            work,
            &execution.effect,
            &pending,
        ) && terminal_direct_output_matches_record(
            coordinator,
            prepared.address.ordinal,
            &execution.effect,
            &pending,
        ) && work.digest == prepared.digest
            && coordinator
                .records
                .get(&prepared.address.ordinal)
                .is_some_and(|record| record.key == prepared.key)
            && installed_count == 1
            && recovered_count == 0
            && terminal_count == 1
    }

    /// Remove the already-fsynced exact output carrier. All fallible checks
    /// happen in the applicable terminal preflight immediately before fsync.
    pub(super) fn publish_lifecycle_output_terminal_after_fsync(
        &mut self,
        prepared: PreparedLifecycleOutputRegistryRetirementV1,
    ) {
        let work = self
            .entries
            .remove(&prepared.address)
            .expect("preflighted lifecycle output carrier remains installed after fsync");
        assert_eq!(work.digest, prepared.digest);
        assert!(work.validate_exact());
    }
}
