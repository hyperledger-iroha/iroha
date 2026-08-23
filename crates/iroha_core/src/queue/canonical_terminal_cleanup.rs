impl Queue {
    /// Reauthenticate and complete every still-Pending group from one canonical
    /// carrier under a single all-group Queue preflight.
    ///
    /// All move-only Kura Pending tokens and independently reconstructed
    /// `ApplyCarrier` authorities are consumed into immutable gates before the
    /// first Queue mutation. Consequently a malformed later group cannot let an
    /// earlier group cross `Commit`, QueuePlan tombstone, or `ForgetCommit`.
    pub(crate) fn authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes(
        &self,
        groups: Vec<(
            AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
            AutonomousLaneQueueCarrierCleanupAuthorization,
        )>,
    ) -> Result<LaneQueueCarrierCleanupResult, LaneQueueReservationError> {
        self.authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers(
            vec![groups],
            1,
        )
    }
    /// Reauthenticate multiple canonical carriers while preserving each carrier's
    /// independent execution-entrypoint bound and one global semantic preflight.
    ///
    /// `anchored_carrier_bound` is the number of distinct committed Queue groups
    /// that selected these carriers from the immutable startup snapshot. A carrier
    /// may include absent sibling groups, so the flattened key count is not capped
    /// by Queue capacity; the carrier count is capped by those exact local anchors.
    pub(crate) fn authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers(
        &self,
        carriers: Vec<
            Vec<(
                AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization,
                AutonomousLaneQueueCarrierCleanupAuthorization,
            )>,
        >,
        anchored_carrier_bound: usize,
    ) -> Result<LaneQueueCarrierCleanupResult, LaneQueueReservationError> {
        let mut prepared_carriers = Vec::with_capacity(carriers.len());
        for groups in carriers {
            let mut prepared = Vec::with_capacity(groups.len());
            for (pending, carrier) in groups {
                let (reservation_group, ordered_keys, source_outcome_hash) =
                pending.consume_for_queue().ok_or_else(|| {
                    LaneQueueReservationError::InvalidIdentity(
                        "Kura canonical Pending authority has malformed ordered reservation identity"
                            .to_owned(),
                    )
                })?;
                let derived =
                    lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter())
                        .map_err(|reason| {
                            LaneQueueReservationError::InvalidIdentity(reason.to_owned())
                        })?;
                if derived != reservation_group {
                    return Err(LaneQueueReservationError::InvalidIdentity(
                        "Kura canonical Pending authority changed its reservation group".to_owned(),
                    ));
                }
                let cleanup_gate = LaneQueueCarrierCleanupGate::from_authorization(
                    derived,
                    carrier,
                    source_outcome_hash,
                )?;
                prepared.push(PreparedLaneQueueCarrierCleanupGroup {
                    ordered_keys,
                    group_binding: derived,
                    cleanup_gate,
                });
            }
            prepared_carriers.push(prepared);
        }
        self.commit_prepared_lane_reservation_carriers(prepared_carriers, anchored_carrier_bound)
    }
    fn validate_lane_queue_carrier_cleanup_batch_bounds(
        &self,
        carrier_reservation_counts: &[usize],
        anchored_carrier_bound: usize,
    ) -> Result<usize, LaneQueueReservationError> {
        if carrier_reservation_counts.is_empty()
            || carrier_reservation_counts.iter().any(|count| *count == 0)
            || anchored_carrier_bound == 0
            || anchored_carrier_bound > self.capacity.get()
            || carrier_reservation_counts.len() > anchored_carrier_bound
        {
            return Err(LaneQueueReservationError::InvalidIdentity(
                "canonical Queue cleanup requires a non-empty carrier set bounded by exact startup anchors"
                    .to_owned(),
            ));
        }
        let mut aggregate = 0_usize;
        for count in carrier_reservation_counts {
            if *count > iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS {
                return Err(LaneQueueReservationError::InvalidIdentity(format!(
                    "canonical Queue cleanup carrier reservation count exceeds hard limit {}",
                    iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS,
                )));
            }
            aggregate = aggregate.checked_add(*count).ok_or_else(|| {
                LaneQueueReservationError::InvalidIdentity(
                    "canonical Queue cleanup aggregate reservation count overflowed".to_owned(),
                )
            })?;
        }
        Ok(aggregate)
    }
    #[cfg(test)]
    fn commit_prepared_lane_reservation_groups(
        &self,
        groups: Vec<PreparedLaneQueueCarrierCleanupGroup>,
    ) -> Result<LaneQueueCarrierCleanupResult, LaneQueueReservationError> {
        self.commit_prepared_lane_reservation_carriers(vec![groups], 1)
    }
    fn commit_prepared_lane_reservation_carriers(
        &self,
        carriers: Vec<Vec<PreparedLaneQueueCarrierCleanupGroup>>,
        anchored_carrier_bound: usize,
    ) -> Result<LaneQueueCarrierCleanupResult, LaneQueueReservationError> {
        let carrier_reservation_counts = carriers
            .iter()
            .map(|groups| {
                groups.iter().try_fold(0_usize, |total, group| {
                    total.checked_add(group.ordered_keys.len())
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                LaneQueueReservationError::InvalidIdentity(
                    "canonical Queue cleanup carrier reservation count overflowed".to_owned(),
                )
            })?;
        let _aggregate = self.validate_lane_queue_carrier_cleanup_batch_bounds(
            &carrier_reservation_counts,
            anchored_carrier_bound,
        )?;
        let group_count = carriers
            .iter()
            .try_fold(0_usize, |total, carrier| total.checked_add(carrier.len()))
            .ok_or_else(|| {
                LaneQueueReservationError::InvalidIdentity(
                    "canonical Queue cleanup group count overflowed".to_owned(),
                )
            })?;
        let mut terminal_evidence = Vec::new();
        terminal_evidence
            .try_reserve_exact(group_count)
            .map_err(|_| {
                LaneQueueReservationError::InvalidIdentity(
                    "canonical Queue cleanup evidence allocation exceeds platform bounds"
                        .to_owned(),
                )
            })?;
        if self.transaction_selection_durability_faulted() {
            return Err(LaneQueueReservationError::DurabilityFault);
        }
        let cleanup_hashes = carriers
            .iter()
            .flatten()
            .flat_map(|group| group.ordered_keys.iter())
            .map(|key| key.entrypoint_hash)
            .collect::<Vec<_>>();
        let _reservation_transition_guard = self.lane_reservation_transition_lock.lock();
        // A producer can finish its QueuePlan/reservation durability boundary
        // concurrently with carrier application. That boundary is temporary,
        // not conflicting ownership. Wait without holding the Queue mutation
        // lock, then close the check/publication race exactly as committed hash
        // removal does.
        self.wait_for_durability_transitions(&cleanup_hashes);
        let mut queue_guard = self.push_remove_lock.lock();
        while cleanup_hashes
            .iter()
            .any(|hash| self.durability_transition_active(hash))
        {
            drop(queue_guard);
            self.wait_for_durability_transitions(&cleanup_hashes);
            queue_guard = self.push_remove_lock.lock();
        }
        if self.transaction_selection_durability_faulted() {
            return Err(LaneQueueReservationError::DurabilityFault);
        }
        let store = self.lane_reservations.lock();
        let global_selection_owners = self.global_selection_owners.lock();
        let active_durability_transitions = self.durability_transitions.lock();
        let fee_admission_reservations = self.fee_admission_reservations.lock();
        let fifo_snapshot = self.fifo_snapshot_locked();
        let fifo_hashes = fifo_snapshot.iter().copied().collect::<HashSet<_>>();
        if fifo_hashes.len() != fifo_snapshot.len() {
            return Err(LaneQueueReservationError::InvalidIdentity(
                "canonical Queue cleanup found a duplicate physical FIFO owner".to_owned(),
            ));
        }
        let mut fifo_ordinal_owners = BTreeMap::new();
        for entry in &self.fifo_order_by_hash {
            let hash = *entry.key();
            let fifo_order = *entry.value();
            fifo_order
                .validate()
                .map_err(|reason| LaneQueueReservationError::InvalidIdentity(reason.to_owned()))?;
            if let Some(existing) = fifo_ordinal_owners.insert(fifo_order.ordinal, hash)
                && existing != hash
            {
                return Err(LaneQueueReservationError::InvalidIdentity(format!(
                    "FIFO ordinal {} is owned by both {existing} and {hash}",
                    fifo_order.ordinal
                )));
            }
        }
        let ownership = LaneQueueCarrierCleanupOwnerSnapshot {
            global_selection_owners: &global_selection_owners,
            active_durability_transitions: &active_durability_transitions,
            fee_admission_reservations: &fee_admission_reservations,
            fifo_hashes: &fifo_hashes,
        };
        let mut journal_preflight = LaneQueueCarrierCleanupJournalPreflight::default();
        let mut seen_group_hashes = BTreeSet::new();
        let mut seen_group_identities = BTreeSet::new();
        let mut seen_group_slot_keys = BTreeSet::new();
        let mut seen_hashes = HashSet::new();
        let mut seen_entrypoints = BTreeSet::new();
        for group in carriers.iter().flatten() {
            if group.ordered_keys.is_empty()
                || usize::try_from(group.group_binding.reservation_count).ok()
                    != Some(group.ordered_keys.len())
                || group.ordered_keys.iter().any(|key| {
                    LaneQueueReservationGroupIdentityV1::from_key(key)
                        != group.group_binding.identity
                })
                || !group
                    .cleanup_gate
                    .authenticates_applied_group(group.group_binding)
            {
                return Err(LaneQueueReservationError::InvalidIdentity(
                    "carrier cleanup group differs from its exact applied-state binding".to_owned(),
                ));
            }
            if !seen_group_hashes.insert(group.group_binding.reservation_group_hash)
                || !seen_group_identities.insert(group.group_binding.identity)
                || !seen_group_slot_keys.insert(group.group_binding.identity.slot_key())
            {
                return Err(LaneQueueReservationError::InvalidIdentity(
                    "carrier cleanup carriers duplicate one reservation group hash, attempt identity, or lane slot"
                        .to_owned(),
                ));
            }
            for key in &group.ordered_keys {
                if !seen_hashes.insert(key.entrypoint_hash)
                    || !seen_entrypoints.insert(key.entrypoint_hash.clone())
                {
                    return Err(LaneQueueReservationError::InvalidIdentity(
                        "carrier cleanup groups duplicate one transaction or entrypoint owner"
                            .to_owned(),
                    ));
                }
            }
            self.preflight_lane_reservation_group_locked(
                &store,
                &group.ordered_keys,
                &ownership,
                &fifo_ordinal_owners,
                &mut journal_preflight,
                true,
            )?;
        }
        drop(ownership);
        drop(fee_admission_reservations);
        drop(active_durability_transitions);
        drop(global_selection_owners);
        let durability_transition = self
            .begin_durability_transition_locked(
                carriers
                    .iter()
                    .flatten()
                    .flat_map(|group| group.ordered_keys.iter())
                    .map(|key| key.entrypoint_hash),
            )
            .map_err(|hash| LaneQueueReservationError::Conflict { hash })?;
        drop(store);
        drop(queue_guard);
        self.preflight_lane_reservation_plan_journal(&journal_preflight)?;
        // Replica validators retain ordinary FIFO/QueuePlan ownership because only the
        // deterministic producer publishes a durable lane reservation. The exact replica set
        // was classified during the all-group read-only preflight above. Tombstone the complete
        // QueuePlan batch atomically as the first irreversible mutation, then remove its
        // in-memory projection while the same lane and per-hash transition fences remain held.
        self.tombstone_committed_replica_plan_journal(&journal_preflight.replica_keys)?;
        self.remove_preflighted_committed_replica_owners(
            &journal_preflight.replica_keys,
            &durability_transition,
        )?;
        let mut finalized = 0usize;
        for group in carriers.into_iter().flatten() {
            let PreparedLaneQueueCarrierCleanupGroup {
                ordered_keys,
                group_binding,
                cleanup_gate,
            } = group;
            let (group_finalized, evidence) = self.commit_lane_reservation(
                &ordered_keys,
                group_binding,
                None,
                cleanup_gate,
                &durability_transition,
            )?;
            finalized = finalized.saturating_add(group_finalized);
            terminal_evidence.push(evidence.ok_or_else(|| {
                LaneQueueReservationError::InvalidIdentity(
                    "complete canonical cleanup did not mint terminal Queue evidence".to_owned(),
                )
            })?);
        }
        drop(durability_transition);
        Ok(LaneQueueCarrierCleanupResult {
            finalized_reservations: finalized,
            terminal_evidence,
        })
    }
}
