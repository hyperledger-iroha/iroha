// Generic state-index and capacity helpers for `LifecycleCoordinator`.

impl LifecycleCoordinator {
    fn advance_observed_generation(&mut self, source: WaitSource, generation: u64) {
        debug_assert!(matches!(
            source,
            WaitSource::External(_) | WaitSource::Recovery(_)
        ));
        let known = self.observed_generation.entry(source).or_default();
        *known = (*known).max(generation);
        let known = *known;
        let stale: Vec<_> = self
            .records
            .iter()
            .filter_map(|(ordinal, record)| match record.state {
                LifecycleState::Waiting(wait)
                    if wait.source == source && wait.observed_generation < known =>
                {
                    Some(*ordinal)
                }
                LifecycleState::Waiting(_)
                | LifecycleState::Ready
                | LifecycleState::Claimed(_)
                | LifecycleState::Terminal(_) => None,
            })
            .collect();
        for ordinal in stale {
            self.make_ready(ordinal);
        }
    }
    fn first_capacity_wait(&self, delta: &BTreeMap<CapacityClass, usize>) -> Option<WaitToken> {
        let mut effective_used = self.capacity_used.clone();
        if let Some(reservation) = self
            .active_lease
            .as_ref()
            .and_then(TurnLease::output_reservation)
        {
            let used = effective_used.entry(reservation.class()).or_default();
            *used = used.checked_add(1).unwrap_or(usize::MAX);
        }
        first_capacity_wait(
            &effective_used,
            &self.capacity_geometry,
            &self.capacity_generation,
            delta,
        )
    }
    fn apply_capacity_delta(&mut self, delta: &BTreeMap<CapacityClass, usize>) {
        for (class, added) in delta {
            *self.capacity_used.entry(*class).or_default() += added;
        }
    }
    fn release_capacity(&mut self, class: CapacityClass) -> Result<(), CoordinatorFault> {
        let used = self.capacity_used.entry(class).or_default();
        *used = used
            .checked_sub(1)
            .ok_or(CoordinatorFault::CapacityAccounting)?;
        let generation = self.capacity_generation.entry(class).or_default();
        *generation = generation
            .checked_add(1)
            .ok_or(CoordinatorFault::CapacityAccounting)?;
        Ok(())
    }
    fn insert_record(&mut self, record: LifecycleRecord) {
        let ordinal = record.ordinal;
        let key = record.key;
        if record.state == LifecycleState::Ready {
            self.ready_index.insert(ordinal);
        }
        self.key_index.insert(key, ordinal);
        self.records.insert(ordinal, record);
    }
    fn make_ready(&mut self, ordinal: u128) {
        let record = self
            .records
            .get_mut(&ordinal)
            .expect("readiness publication names an existing record");
        if !matches!(record.state, LifecycleState::Terminal(_)) {
            record.state = LifecycleState::Ready;
            self.ready_index.insert(ordinal);
        }
    }
    fn replace_physical(
        &mut self,
        ordinal: u128,
        replacement: PhysicalReplacement,
    ) -> Result<(), CoordinatorFault> {
        if replacement.existing_slot != replacement.replacement.id {
            return Err(CoordinatorFault::InvalidPhysicalTransition);
        }
        let record = self
            .records
            .get_mut(&ordinal)
            .ok_or(CoordinatorFault::InvalidPhysicalTransition)?;
        if !record
            .physical_slots
            .contains_key(&replacement.existing_slot)
        {
            return Err(CoordinatorFault::InvalidPhysicalTransition);
        }
        if record.physical_slots.iter().any(|(slot, digest)| {
            *slot != replacement.existing_slot && *digest == replacement.replacement.digest
        }) {
            record.physical_slots.remove(&replacement.existing_slot);
        } else {
            record
                .physical_slots
                .insert(replacement.existing_slot, replacement.replacement.digest);
        }
        Ok(())
    }
    fn frozen_predecessors(&self, scope: PredecessorScope, ordinal: u128) -> BTreeSet<u128> {
        frozen_predecessors(&self.records, scope, ordinal)
    }
    fn ready_entry_is_eligible(&self, ordinal: u128, selectable_ready: &BTreeSet<u128>) -> bool {
        let record = self
            .records
            .get(&ordinal)
            .expect("ready index is bijective with lifecycle records");
        if record
            .episode
            .frozen_predecessors
            .iter()
            .any(|predecessor| selectable_ready.contains(predecessor))
        {
            return false;
        }
        !selectable_ready.iter().any(|candidate| {
            *candidate < ordinal
                && self.records.get(candidate).is_some_and(|record| {
                    record.stage.predecessor_scope == PredecessorScope::ProducerHandoffBarrier
                })
        })
    }
    fn finish_replenishment(
        &mut self,
        ordinal: u128,
        slot: PhysicalSlot,
    ) -> Result<(), CoordinatorFault> {
        let record = self
            .records
            .get_mut(&ordinal)
            .ok_or(CoordinatorFault::InvalidPhysicalTransition)?;
        if !record.episode.slot_universe.contains(&slot.id)
            || !record.episode.consumed_slots.insert(slot.id)
        {
            return Err(CoordinatorFault::InvalidPhysicalTransition);
        }
        if !record
            .physical_slots
            .values()
            .any(|digest| *digest == slot.digest)
        {
            record.physical_slots.insert(slot.id, slot.digest);
        }
        record.state = LifecycleState::Ready;
        self.ready_index.insert(ordinal);
        Ok(())
    }
    fn supersede_lower_enter_views(
        &mut self,
        installed: LifecycleKey,
    ) -> Result<(), CoordinatorFault> {
        for ordinal in lower_enter_view_ordinals(&self.records, installed) {
            self.finish_terminal(ordinal, TerminalOutcome::Cancelled)?;
        }
        self.retire_lower_enter_view_admission_waits(installed);
        Ok(())
    }
    fn retire_lower_enter_view_admission_waits(&mut self, installed: LifecycleKey) {
        self.admission_waits.retain(|_, waiting| {
            let candidate = &waiting.candidate;
            candidate.work_class != LifecycleWorkClass::EnterView
                || candidate.key.context != installed.context
                || candidate.key.round.height != installed.round.height
                || candidate.key.round.view >= installed.round.view
        });
    }
}
