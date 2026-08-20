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
    /// The exact row exists but an older Ready row or an active claim owns the turn.
    Deferred,
    /// The exact row is the next lifecycle-owned output allowed to execute.
    Ready(PreparedLifecycleOutputRegistryRetirementV1),
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
        let mut matches = self.entries.iter().filter(|(address, work)| {
            pending_output_matches_work(&execution.effect, &pending, work)
                && address.owner.causal_root()
                    == super::CausalRoot::new(digest_from_hash(pending.causal_lifecycle_key()))
        });
        let Some((&address, work)) = matches.next() else {
            return Ok(LifecycleOutputRegistryJoinV1::Missing);
        };
        if matches.next().is_some()
            || !lifecycle_output_row_matches(
                coordinator,
                address,
                work,
                &execution.effect,
                &pending,
            )
        {
            return Err(RegistryError::CorruptWork);
        }
        let record = coordinator
            .records
            .get(&address.ordinal)
            .ok_or(RegistryError::Missing)?;
        if coordinator.active_lease.is_some()
            || record.state != super::LifecycleState::Ready
            || coordinator.ready_index.first().copied() != Some(address.ordinal)
        {
            return Ok(LifecycleOutputRegistryJoinV1::Deferred);
        }
        Ok(LifecycleOutputRegistryJoinV1::Ready(
            PreparedLifecycleOutputRegistryRetirementV1 {
                address,
                digest: work.digest,
                key: record.key,
            },
        ))
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

    /// Remove the already-fsynced exact output carrier. All fallible checks
    /// happen in [`Self::lifecycle_output_terminal_is_exact`].
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
