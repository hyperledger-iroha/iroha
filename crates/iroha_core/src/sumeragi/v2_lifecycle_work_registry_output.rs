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
    /// row. Its volatile runtime root may have been reminted after the first
    /// service call, so it stutters without re-entering direct admission.
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
