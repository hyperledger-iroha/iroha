impl ConcreteLifecycleWorkRegistry {
    /// Join every exact cold Ready Validate carrier to its replay authority.
    ///
    /// This read-only launch cut deliberately returns no owner when the runtime
    /// has no Ready Validate row. Every carrier is accepted only after its
    /// complete logical, physical, replay, and candidate-authority coordinates
    /// are revalidated. A Commit-refined carrier must additionally equal the
    /// runtime's sole replayed Decision.
    pub(super) fn project_recovered_durable_validate_retry_census(
        &self,
        coordinator: &LifecycleCoordinator,
        decision: Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Result<RecoveredDurableValidateRetryCensusV1, RecoveredDurableValidateRetryOwnerErrorV1>
    {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidDecision);
        }
        if let Some((decision_round, proposal_round, _, _)) = decision
            && (decision_round != proposal_round
                || decision_round.context_id.0.as_ref()
                    != coordinator.active_context.id().as_bytes()
                || decision_round.height != coordinator.active_context.height())
        {
            return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidDecision);
        }
        let mut logical_keys = std::collections::BTreeSet::new();
        for work in self.entries.values() {
            let validate = match &work.kind {
                ConcreteLifecycleWorkKind::DurableValidateBody(validate) => validate,
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                    if !completion.validates(work.digest) {
                        return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
                    }
                    &completion.incumbent
                }
                _ => continue,
            };
            let statement = validate
                .pending
                .candidate_statement()
                .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?;
            let Some(subject) = statement.subject() else {
                return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
            };
            if !logical_keys.insert((statement.proposal_round(), subject)) {
                return Err(RecoveredDurableValidateRetryOwnerErrorV1::MultipleCarriers);
            }
        }
        let mut owners = BTreeMap::new();
        for (address, work) in &self.entries {
            let (validate, carrier_matches_record) = match &work.kind {
                ConcreteLifecycleWorkKind::DurableValidateBody(validate) => (
                    validate,
                    validate.matches_recovered_record(
                        coordinator.active_context,
                        coordinator
                            .records
                            .get(&address.ordinal)
                            .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?,
                        coordinator
                            .durable_records
                            .get(&address.ordinal)
                            .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?,
                        work.digest,
                    ),
                ),
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => (
                    &completion.incumbent,
                    completion.matches_recovered_record(
                        coordinator.active_context,
                        coordinator
                            .records
                            .get(&address.ordinal)
                            .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?,
                        coordinator
                            .durable_records
                            .get(&address.ordinal)
                            .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?,
                        work.digest,
                    ),
                ),
                _ => continue,
            };
            let record = coordinator
                .records
                .get(&address.ordinal)
                .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?;
            let statement = validate
                .pending
                .candidate_statement()
                .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?;
            let Some(subject) = statement.subject() else {
                return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
            };
            let expected_key = LifecycleKey::new(
                coordinator.active_context.id(),
                LifecycleRound::new(statement.round().height, statement.round().view),
                Some(LifecycleRound::new(
                    statement.proposal_round().height,
                    statement.proposal_round().view,
                )),
                Some(projection::block_subject(subject)),
                LifecyclePhase::Validate,
                statement
                    .execution_commitment()
                    .map(projection::execution_commitment),
            );
            let matching_decision = decision.filter(|(_, proposal_round, decision_subject, _)| {
                *proposal_round == statement.proposal_round() && *decision_subject == subject
            });
            match (statement.phase(), statement.execution_commitment()) {
                (Some(wire::GlobalPhase::Commit), Some(commitment)) => {
                    let carrier_decision = (
                        statement.round(),
                        statement.proposal_round(),
                        subject,
                        commitment,
                    );
                    if decision != Some(carrier_decision) {
                        return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
                    }
                }
                (Some(wire::GlobalPhase::Prepare), Some(_)) | (None, None) => {}
                _ => {
                    return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
                }
            }
            let AdapterEffect::ValidateBody {
                round,
                subject: effect_subject,
                ..
            } = &validate.effect
            else {
                return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
            };
            if *round != statement.proposal_round()
                || *effect_subject != subject
                || statement.context_id() != statement.round().context_id
                || statement.context_id() != statement.proposal_round().context_id
                || statement.context_id().0.as_ref() != coordinator.active_context.id().as_bytes()
                || statement.round().height != coordinator.active_context.height()
                || record.state != super::LifecycleState::Ready
                || record.work_class != LifecycleWorkClass::Validate
                || record.stage
                    != LifecycleStage::new(
                        LifecycleStageKind::ValidateBody,
                        PredecessorScope::Independent,
                    )
                || record.key != expected_key
                || record.ordinal != address.ordinal
                || record.owner != address.owner
                || record.physical_slots != BTreeMap::from([(address.slot, work.digest)])
                || record.episode.slot_universe != std::collections::BTreeSet::from([address.slot])
                || record.episode.consumed_slots != std::collections::BTreeSet::from([address.slot])
                || coordinator.key_index.get(&record.key) != Some(&record.ordinal)
                || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
                || !coordinator.ready_index.contains(&record.ordinal)
                || !work.validates_at(*address)
                || !carrier_matches_record
            {
                return Err(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier);
            }
            let binding = validate
                .pending
                .project_recovered_durable_validate_retry_binding(
                    &validate.effect,
                    matching_decision,
                )
                .ok_or(RecoveredDurableValidateRetryOwnerErrorV1::InvalidCarrier)?;
            let key = (*round, subject);
            let owner = RecoveredDurableValidateRetryOwnerV1 {
                expected_decision: matching_decision,
                effect: validate.effect.clone(),
                durable_receipt: validate.durable_receipt.clone(),
                binding,
                lifecycle_ordinal: address.ordinal,
            };
            if owners.insert(key, owner).is_some() {
                return Err(RecoveredDurableValidateRetryOwnerErrorV1::MultipleCarriers);
            }
        }
        Ok(RecoveredDurableValidateRetryCensusV1 { owners })
    }

    /// Seal the complete current Ready census when its oldest row is an
    /// executable ProducerTurn. `Ok(None)` means another Ready row must run
    /// first, or there is no Ready work.
    pub(super) fn attest_ready_producer_turn_census(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
        ledger: &super::ledger::LifecycleLedgerV1,
    ) -> Result<
        Option<ReadyProducerTurnCensusAttestationV1>,
        ReadyProducerTurnCensusAttestationErrorV1,
    > {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::CoordinatorUnavailable);
        }
        if !super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
            .is_ok_and(|current| &current == ledger)
        {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::LedgerMismatch);
        }
        let ready_records = coordinator
            .records
            .iter()
            .filter_map(|(&ordinal, record)| {
                (record.state == super::LifecycleState::Ready).then_some((ordinal, record.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let ready_ordinals = ready_records
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if ready_ordinals != coordinator.ready_index {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidReadyCensus);
        }
        let Some((&producer_ordinal, producer)) = ready_records.first_key_value() else {
            return Ok(None);
        };
        if producer.work_class != LifecycleWorkClass::ProducerTurn {
            return Ok(None);
        }
        if !self.exactly_covers_all_live_work(verified, coordinator) {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidRegistry);
        }
        let Some((&serve_ordinal, _)) = coordinator
            .producer_debts
            .iter()
            .find(|(_, producer)| **producer == producer_ordinal)
        else {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        };
        let (Some(serve), Some(serve_metadata), Some(producer_metadata)) = (
            coordinator.records.get(&serve_ordinal),
            coordinator.durable_records.get(&serve_ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        };
        let Some((slot, producer_digest)) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())
        else {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        };
        let Some(producer_address) =
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, slot)
        else {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        };
        let Some(work) = self.entries.get(&producer_address) else {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        };
        let ConcreteLifecycleWorkKind::DurableProducerTurn(carrier) = &work.kind else {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        };
        if serve_ordinal.checked_add(1) != Some(producer_ordinal)
            || !matches!(
                serve.state,
                super::LifecycleState::Terminal(outcome)
                    if outcome != super::TerminalOutcome::Cancelled
            )
            || producer.stage.kind() != LifecycleStageKind::ProducerTurn
            || producer.stage.predecessor_scope() != PredecessorScope::ProducerHandoffBarrier
            || !coordinator.ready_entry_is_eligible(producer_ordinal, &ready_ordinals)
            || !serve_ordinal_pair_is_exact(serve, producer)
            || !serve_metadata
                .replay_authority
                .same_persisted_family(&producer_metadata.replay_authority)
            || work.digest != producer_digest
            || !carrier.matches_record(producer, producer_metadata, producer_digest)
        {
            return Err(ReadyProducerTurnCensusAttestationErrorV1::InvalidProducer);
        }
        Ok(Some(ReadyProducerTurnCensusAttestationV1 {
            ledger_frame: ledger.frame_identity(),
            ready_records,
            producer_address,
            producer_digest,
            _linearity: ReadyProducerTurnCensusAttestationLinearityV1,
        }))
    }

    /// Consume the exact sealed Ready census with its matching active claim.
    pub(super) fn project_claimed_producer_turn(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
        ledger: &super::ledger::LifecycleLedgerV1,
        lease: TurnLease,
        attestation: ReadyProducerTurnCensusAttestationV1,
    ) -> Result<ClaimedProducerTurnV1, ClaimedProducerTurnErrorV1> {
        if !super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
            .is_ok_and(|current| &current == ledger)
        {
            return Err(ClaimedProducerTurnErrorV1::LedgerMismatch);
        }
        if !attestation.matches_claimed_census(coordinator, ledger, &lease) {
            return Err(ClaimedProducerTurnErrorV1::InvalidLease);
        }
        if !self.exactly_covers_all_live_work_with_active_producer(verified, coordinator, &lease) {
            return Err(ClaimedProducerTurnErrorV1::InvalidCarrier);
        }
        let record = coordinator
            .records
            .get(&lease.ordinal)
            .ok_or(ClaimedProducerTurnErrorV1::InvalidCarrier)?;
        let metadata = coordinator
            .durable_records
            .get(&lease.ordinal)
            .ok_or(ClaimedProducerTurnErrorV1::InvalidCarrier)?;
        let work = self
            .entries
            .get(&attestation.producer_address)
            .ok_or(ClaimedProducerTurnErrorV1::InvalidCarrier)?;
        let ConcreteLifecycleWorkKind::DurableProducerTurn(producer) = &work.kind else {
            return Err(ClaimedProducerTurnErrorV1::InvalidCarrier);
        };
        if work.digest != attestation.producer_digest
            || !producer.matches_claimed_record(record, metadata, work.digest, &lease)
        {
            return Err(ClaimedProducerTurnErrorV1::InvalidCarrier);
        }
        Ok(ClaimedProducerTurnV1 {
            lease,
            address: attestation.producer_address,
            digest: attestation.producer_digest,
            ledger_frame: attestation.ledger_frame,
            _linearity: ClaimedProducerTurnLinearityV1,
        })
    }

    /// Remove one exact fixture carrier without exposing the registry map.
    #[cfg(test)]
    pub(super) fn remove_exact_for_test(&mut self, address: ConcreteWorkAddress) -> bool {
        self.entries.remove(&address).is_some()
    }
    /// Replace only the volatile recovered-Fetch wait source for a negative
    /// exact-owner regression and return the prior source.
    #[cfg(test)]
    pub(super) fn replace_recovered_fetch_wait_source_for_test(
        &mut self,
        ordinal: u128,
        replacement: super::WaitSource,
    ) -> Option<super::WaitSource> {
        let (_, work) = self
            .entries
            .iter_mut()
            .find(|(address, _)| address.ordinal == ordinal)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &mut work.kind
        else {
            return None;
        };
        fetch.wait_source.replace(replacement)
    }

    /// Return whether one Broadcast carrier declares a paired next Vote.
    pub(super) fn recovered_lifecycle_signed_broadcast_declares_next_vote(
        &self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
    ) -> bool {
        let Some(record) = coordinator.records.get(&broadcast_ordinal) else {
            return false;
        };
        let Some((slot, _)) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
        else {
            return false;
        };
        let Some(address) = ConcreteWorkAddress::new(record.owner, broadcast_ordinal, slot) else {
            return false;
        };
        self.entries.get(&address).is_some_and(|work| {
            matches!(
                &work.kind,
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast)
                    if broadcast.paired_next_sign.is_some()
            )
        })
    }
    /// Return the exact paired next-Vote ordinal retained by one Ready Broadcast.
    pub(super) fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
        &self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
    ) -> Option<u128> {
        let record = coordinator.records.get(&broadcast_ordinal)?;
        let (slot, digest) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())?;
        let address = ConcreteWorkAddress::new(record.owner, broadcast_ordinal, slot)?;
        let work = self.entries.get(&address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        let (next, next_digest) = broadcast.paired_next_sign?;
        let next_record = coordinator.records.get(&next.ordinal)?;
        (work.digest == digest
            && broadcast.matches_current_ready_record(address, digest, coordinator)
            && next.ordinal == broadcast_ordinal.checked_add(1)?
            && next_record.state == super::LifecycleState::Ready
            && next_record.owner == next.owner
            && next_record.physical_slots.get(&next.slot) == Some(&next_digest))
        .then_some(next.ordinal)
    }
    /// Attest one live recovered signed Broadcast as a durable refanout source.
    pub(super) fn attest_ready_recovered_lifecycle_signed_broadcast(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<(), &'static str> {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err("recovered signed Broadcast coordinator is not idle");
        }
        let Some(record) = coordinator.records.get(&ordinal) else {
            return Err("recovered signed Broadcast row is absent");
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
        else {
            return Err("recovered signed Broadcast geometry changed");
        };
        let Some(address) = ConcreteWorkAddress::new(record.owner, ordinal, slot) else {
            return Err("recovered signed Broadcast address is invalid");
        };
        let Some(work) = self.entries.get(&address) else {
            return Err("recovered signed Broadcast carrier is absent");
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return Err("recovered signed Broadcast carrier class changed");
        };
        (work.digest == digest
            && broadcast.matches_current_ready_record(address, digest, coordinator))
        .then_some(())
        .ok_or("recovered signed Broadcast Ready owner changed")
    }
    /// Attest one adjacent Ready signed-Broadcast and next-WAL-Vote Sign pair.
    ///
    /// The Broadcast is authenticated first at the lower ordinal. Only an
    /// exact independently WAL-owned Vote carrier at the immediately following
    /// ordinal can yield the existing scheduler-facing Sign attestation. Both
    /// concrete carriers remain borrowed and no dispatch key is installed.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(
        &self,
        coordinator: &LifecycleCoordinator,
        broadcast_ordinal: u128,
        next_sign_ordinal: u128,
    ) -> Result<
        ReadyRecoveredLifecycleSignAttestationV1,
        ReadyRecoveredLifecycleSignAttestationErrorV1,
    > {
        if broadcast_ordinal.checked_add(1) != Some(next_sign_ordinal) {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let (Some(broadcast_record), Some(next_sign_record)) = (
            coordinator.records.get(&broadcast_ordinal),
            coordinator.records.get(&next_sign_ordinal),
        ) else {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        };
        if broadcast_record.owner == next_sign_record.owner {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        }
        self.attest_ready_recovered_lifecycle_signed_broadcast(coordinator, broadcast_ordinal)
            .map_err(|_| ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCarrier)?;
        let attestation =
            self.attest_ready_recovered_lifecycle_sign(coordinator, next_sign_ordinal)?;
        let Some((next_slot, next_digest)) = exact_single_record_slot(
            next_sign_record,
            LifecycleWorkClass::SignVote.capacity_class(),
        ) else {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        };
        let next_address =
            ConcreteWorkAddress::new(next_sign_record.owner, next_sign_ordinal, next_slot)
                .ok_or(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex)?;
        let next_work = self.entries.get(&next_address).ok_or(
            ReadyRecoveredLifecycleSignAttestationErrorV1::Registry(RegistryError::Missing),
        )?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(next_sign) =
            &next_work.kind
        else {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::WrongWorkKind);
        };
        if next_work.digest != next_digest
            || self.recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
                coordinator,
                broadcast_ordinal,
            ) != Some(next_sign_ordinal)
            || next_sign.dispatch_key.is_some()
            || !next_sign.matches_current_ready_record(next_address, next_digest, coordinator)
            || !attestation.matches_ready_record(next_sign_record)
        {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCarrier);
        }
        Ok(attestation)
    }
    /// Project the exact claimed Broadcast into its service-only refanout authority.
    pub(super) fn project_claimed_recovered_lifecycle_signed_broadcast_output(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Option<RecoveredLifecycleSignedBroadcastOutputAuthorityV1> {
        if lease.work_class() != LifecycleWorkClass::Broadcast || lease.physical_slots().len() != 1
        {
            return None;
        }
        let (&slot, &digest) = lease.physical_slots().first_key_value()?;
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)?;
        let work = self.entries.get(&address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
            &work.kind
        else {
            return None;
        };
        (work.digest == digest)
            .then(|| {
                broadcast.project_claimed_output_authority(address, digest, coordinator, lease)
            })
            .flatten()
    }
    /// Attest one Ready recovered PhaseVote or standalone control Sign.
    ///
    /// Classification keeps the concrete carrier closed and returns only its
    /// class-sensitive queue key plus the typed bounded-I/O demand.
    pub(super) fn attest_ready_recovered_lifecycle_sign(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<
        ReadyRecoveredLifecycleSignAttestationV1,
        ReadyRecoveredLifecycleSignAttestationErrorV1,
    > {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&ordinal),
            coordinator.durable_records.get(&ordinal),
        ) else {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        };
        let class = match record.work_class {
            LifecycleWorkClass::SignVote
                if matches!(
                    (record.key.phase(), record.stage.kind()),
                    (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote)
                        | (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote)
                ) =>
            {
                RecoveredLifecycleSignClassV1::PhaseVote
            }
            LifecycleWorkClass::SignProposal
                if record.key.phase() == LifecyclePhase::Proposal
                    && record.stage.kind() == LifecycleStageKind::SignProposal =>
            {
                RecoveredLifecycleSignClassV1::ControlProposal
            }
            LifecycleWorkClass::SignTimeout
                if record.key.phase() == LifecyclePhase::Timeout
                    && record.stage.kind() == LifecycleStageKind::SignTimeoutVote =>
            {
                RecoveredLifecycleSignClassV1::ControlTimeout
            }
            _ => {
                return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
            }
        };
        if record.state != super::LifecycleState::Ready
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || !coordinator.ready_index.contains(&ordinal)
            || coordinator.key_index.get(&record.key) != Some(&ordinal)
            || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || metadata.continuation != super::schema::DurableContinuation::None
        {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        };
        if record.physical_slots.len() != 1
            || slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
        {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex)?;
        if self
            .entries
            .keys()
            .filter(|candidate| candidate.owner == record.owner)
            .count()
            != 1
        {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let work = self.entries.get(&address).ok_or(
            ReadyRecoveredLifecycleSignAttestationErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let (carrier_matches, dispatch_key) = match (&work.kind, class) {
            (ConcreteLifecycleWorkKind::DurableLiveWalSign(sign), class)
                if sign.class() == Some(class) =>
            {
                (
                    sign.matches_current_ready_record(address, digest, coordinator),
                    sign.dispatch_key,
                )
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign),
                RecoveredLifecycleSignClassV1::PhaseVote,
            ) => (
                sign.matches_current_ready_record(address, digest, coordinator),
                sign.dispatch_key,
            ),
            (
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign),
                RecoveredLifecycleSignClassV1::PhaseVote,
            ) => (
                sign.matches_current_ready_record(address, digest, coordinator),
                sign.dispatch_key,
            ),
            (
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign),
                RecoveredLifecycleSignClassV1::ControlProposal
                | RecoveredLifecycleSignClassV1::ControlTimeout,
            ) => (
                sign.matches_current_ready_record(address, digest, coordinator),
                sign.dispatch_key,
            ),
            _ => return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::WrongWorkKind),
        };
        if !carrier_matches || dispatch_key.is_some() {
            return Err(ReadyRecoveredLifecycleSignAttestationErrorV1::InvalidCarrier);
        }
        Ok(ReadyRecoveredLifecycleSignAttestationV1 {
            demand: ReadyRecoveredLifecycleSignDemandV1::BoundedIo,
            dispatch_key: RecoveredLifecycleSignDispatchKeyV1::new(
                coordinator.active_context,
                address,
                digest,
                class,
            ),
            _seal: ReadyRecoveredLifecycleSignAttestationSealV1,
        })
    }
    /// Project one exact claimed recovered Sign into its dedicated worker task.
    pub(super) fn prepare_recovered_lifecycle_sign_dispatch(
        &mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Result<
        PreparedRecoveredLifecycleSignDispatch<'_>,
        RecoveredLifecycleSignDispatchProjectionErrorV1,
    > {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidLease);
        }
        let class = match lease.work_class() {
            LifecycleWorkClass::SignVote
                if matches!(
                    (lease.key().phase(), lease.stage().kind()),
                    (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote)
                        | (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote)
                ) =>
            {
                RecoveredLifecycleSignClassV1::PhaseVote
            }
            LifecycleWorkClass::SignProposal
                if lease.key().phase() == LifecyclePhase::Proposal
                    && lease.stage().kind() == LifecycleStageKind::SignProposal =>
            {
                RecoveredLifecycleSignClassV1::ControlProposal
            }
            LifecycleWorkClass::SignTimeout
                if lease.key().phase() == LifecyclePhase::Timeout
                    && lease.stage().kind() == LifecycleStageKind::SignTimeoutVote =>
            {
                RecoveredLifecycleSignClassV1::ControlTimeout
            }
            _ => return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidLease),
        };
        let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
            return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidLease);
        };
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0) {
            return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidLease);
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidLease)?;
        let work = self.entries.get_mut(&address).ok_or(
            RecoveredLifecycleSignDispatchProjectionErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let identity = RecoveredLifecycleSignDispatchIdentityV1::new(
            coordinator.active_context,
            address,
            digest,
            class,
        );
        let (carrier, task) = match (&mut work.kind, class) {
            (ConcreteLifecycleWorkKind::DurableLiveWalSign(sign), class)
                if sign.class() == Some(class) =>
            {
                if !sign.matches_claimed_record(address, digest, coordinator, lease) {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier);
                }
                if sign.dispatch_key.is_some() {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched);
                }
                let task = sign
                    .project_task(identity)
                    .ok_or(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier)?;
                (PreparedRecoveredLifecycleSignCarrier::Live(sign), task)
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign),
                RecoveredLifecycleSignClassV1::PhaseVote,
            ) => {
                if !sign.matches_claimed_record(address, digest, coordinator, lease) {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier);
                }
                if sign.dispatch_key.is_some() {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched);
                }
                let AdapterEffect::Sign { tag, request } = sign.repair.installed_child_effect()
                else {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier);
                };
                let task = crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1::from_registry_projection(
                    identity,
                    *tag,
                    request.clone(),
                )
                .ok_or(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier)?;
                (PreparedRecoveredLifecycleSignCarrier::PhaseVote(sign), task)
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign),
                RecoveredLifecycleSignClassV1::PhaseVote,
            ) => {
                if !sign.matches_current_claimed_record(address, digest, coordinator, lease) {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier);
                }
                if sign.dispatch_key.is_some() {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched);
                }
                let task = sign
                    .project_task(identity)
                    .ok_or(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier)?;
                (
                    PreparedRecoveredLifecycleSignCarrier::NextWalVote(sign),
                    task,
                )
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign),
                RecoveredLifecycleSignClassV1::ControlProposal
                | RecoveredLifecycleSignClassV1::ControlTimeout,
            ) => {
                if !sign.carrier.matches_claimed_record(coordinator, lease) {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier);
                }
                if sign.dispatch_key.is_some() {
                    return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched);
                }
                let task = sign
                    .carrier
                    .project_recovered_lifecycle_sign_task(identity)
                    .ok_or(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier)?;
                (PreparedRecoveredLifecycleSignCarrier::Control(sign), task)
            }
            _ => return Err(RecoveredLifecycleSignDispatchProjectionErrorV1::WrongWorkKind),
        };
        let key = task.dispatch_key();
        Ok(PreparedRecoveredLifecycleSignDispatch {
            carrier,
            task: Some(task),
            key,
        })
    }
    /// Bind one authenticated but certified-progress-superseded Sign result to
    /// its exact claimed registry carrier before durable cancellation.
    pub(super) fn prepare_recovered_lifecycle_sign_cancellation(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        key: RecoveredLifecycleSignDispatchKeyV1,
    ) -> Option<PreparedRecoveredLifecycleSignCancellationV1> {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return None;
        }
        let class = match lease.work_class() {
            LifecycleWorkClass::SignVote
                if matches!(
                    (lease.key().phase(), lease.stage().kind()),
                    (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote)
                        | (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote)
                ) =>
            {
                RecoveredLifecycleSignClassV1::PhaseVote
            }
            LifecycleWorkClass::SignProposal
                if lease.key().phase() == LifecyclePhase::Proposal
                    && lease.stage().kind() == LifecycleStageKind::SignProposal =>
            {
                RecoveredLifecycleSignClassV1::ControlProposal
            }
            LifecycleWorkClass::SignTimeout
                if lease.key().phase() == LifecyclePhase::Timeout
                    && lease.stage().kind() == LifecycleStageKind::SignTimeoutVote =>
            {
                RecoveredLifecycleSignClassV1::ControlTimeout
            }
            _ => return None,
        };
        let (&slot, &digest) = lease.physical_slots().first_key_value()?;
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0) {
            return None;
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)?;
        let work = self.entries.get(&address)?;
        if work.digest != digest
            || !work.validates_at(address)
            || !key.matches(coordinator.active_context, address, digest, class)
        {
            return None;
        }
        let exact_carrier = match (&work.kind, class) {
            (ConcreteLifecycleWorkKind::DurableLiveWalSign(sign), class)
                if sign.class() == Some(class) =>
            {
                sign.dispatch_key == Some(key)
                    && sign.matches_claimed_record(address, digest, coordinator, lease)
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign),
                RecoveredLifecycleSignClassV1::PhaseVote,
            ) => {
                sign.dispatch_key == Some(key)
                    && sign.matches_claimed_record(address, digest, coordinator, lease)
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign),
                RecoveredLifecycleSignClassV1::PhaseVote,
            ) => {
                sign.dispatch_key == Some(key)
                    && sign.matches_current_claimed_record(address, digest, coordinator, lease)
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign),
                RecoveredLifecycleSignClassV1::ControlProposal
                | RecoveredLifecycleSignClassV1::ControlTimeout,
            ) => {
                sign.dispatch_key == Some(key)
                    && sign.carrier.matches_claimed_record(coordinator, lease)
            }
            _ => false,
        };
        exact_carrier.then_some(PreparedRecoveredLifecycleSignCancellationV1 {
            address,
            digest,
            dispatch_key: key,
            _linearity: RecoveredLifecycleSignCancellationLinearityV1,
        })
    }
    /// Publish one exact superseded Sign cancellation around LedgerV1 fsync.
    ///
    /// Every logical and physical check precedes `publish`. Success is followed
    /// only by infallible removal of the prevalidated concrete carrier.
    pub(super) fn publish_recovered_lifecycle_sign_cancellation<T, E>(
        &mut self,
        prepared: PreparedRecoveredLifecycleSignCancellationV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, RecoveredLifecycleSignCancellationPublicationError<E>> {
        let Some(work) = self.entries.get(&prepared.address) else {
            return Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(prepared));
        };
        let class = match lease.work_class() {
            LifecycleWorkClass::SignVote
                if matches!(
                    (lease.key().phase(), lease.stage().kind()),
                    (LifecyclePhase::Prepare, LifecycleStageKind::SignPrepareVote)
                        | (LifecyclePhase::Commit, LifecycleStageKind::SignCommitVote)
                ) =>
            {
                Some(RecoveredLifecycleSignClassV1::PhaseVote)
            }
            LifecycleWorkClass::SignProposal
                if lease.key().phase() == LifecyclePhase::Proposal
                    && lease.stage().kind() == LifecycleStageKind::SignProposal =>
            {
                Some(RecoveredLifecycleSignClassV1::ControlProposal)
            }
            LifecycleWorkClass::SignTimeout
                if lease.key().phase() == LifecyclePhase::Timeout
                    && lease.stage().kind() == LifecycleStageKind::SignTimeoutVote =>
            {
                Some(RecoveredLifecycleSignClassV1::ControlTimeout)
            }
            _ => None,
        };
        let exact_current = class.is_some_and(|class| {
            current.fault.is_none()
                && current.active_lease.as_ref() == Some(lease)
                && lease.stage().predecessor_scope() == PredecessorScope::Independent
                && lease.physical_slots().len() == 1
                && work.digest == prepared.digest
                && work.validates_at(prepared.address)
                && prepared.dispatch_key.matches(
                    current.active_context,
                    prepared.address,
                    prepared.digest,
                    class,
                )
                && match (&work.kind, class) {
                    (ConcreteLifecycleWorkKind::DurableLiveWalSign(sign), class)
                        if sign.class() == Some(class) =>
                    {
                        sign.dispatch_key == Some(prepared.dispatch_key)
                            && sign.matches_claimed_record(
                                prepared.address,
                                prepared.digest,
                                current,
                                lease,
                            )
                    }
                    (
                        ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign),
                        RecoveredLifecycleSignClassV1::PhaseVote,
                    ) => {
                        sign.dispatch_key == Some(prepared.dispatch_key)
                            && sign.matches_claimed_record(
                                prepared.address,
                                prepared.digest,
                                current,
                                lease,
                            )
                    }
                    (
                        ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign),
                        RecoveredLifecycleSignClassV1::PhaseVote,
                    ) => {
                        sign.dispatch_key == Some(prepared.dispatch_key)
                            && sign.matches_current_claimed_record(
                                prepared.address,
                                prepared.digest,
                                current,
                                lease,
                            )
                    }
                    (
                        ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign),
                        RecoveredLifecycleSignClassV1::ControlProposal
                        | RecoveredLifecycleSignClassV1::ControlTimeout,
                    ) => {
                        sign.dispatch_key == Some(prepared.dispatch_key)
                            && sign.carrier.matches_claimed_record(current, lease)
                    }
                    _ => false,
                }
        });
        let mut expected = current.stage_durable_transaction();
        expected.reduce_cancel_superseded_sign(lease.clone());
        let same_ledger_target = matches!(
            (&expected.ledger_store, &staged.ledger_store),
            (Some(expected_store), Some(staged_store))
                if expected_store.same_publication_target(staged_store)
        );
        let exact_staged = expected.episode_authority == staged.episode_authority
            && expected.active_context == staged.active_context
            && expected.records == staged.records
            && expected.key_index == staged.key_index
            && expected.owner_index == staged.owner_index
            && expected.ready_index == staged.ready_index
            && expected.admission_waits == staged.admission_waits
            && expected.active_lease == staged.active_lease
            && expected.high_water == staged.high_water
            && expected.next_lease == staged.next_lease
            && expected.durable_records == staged.durable_records
            && expected.capacity_geometry == staged.capacity_geometry
            && expected.capacity_used == staged.capacity_used
            && expected.capacity_generation == staged.capacity_generation
            && expected.observed_generation == staged.observed_generation
            && expected.producer_debts == staged.producer_debts
            && expected.fault == staged.fault
            && same_ledger_target
            && staged.fault.is_none()
            && staged.active_lease.is_none()
            && staged
                .records
                .get(&prepared.address.ordinal)
                .is_some_and(|record| {
                    record.state
                        == super::LifecycleState::Terminal(super::TerminalOutcome::Cancelled)
                });
        if !exact_current || !exact_staged {
            return Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(prepared));
        }
        match publish() {
            Ok(value) => {
                drop(
                    self.entries
                        .remove(&prepared.address)
                        .expect("recovered Sign cancellation retained the exact carrier"),
                );
                Ok(value)
            }
            Err(error) => Err(
                RecoveredLifecycleSignCancellationPublicationError::Publication(error, prepared),
            ),
        }
    }
    /// Attest one exact ordinary Fetch or Store in the current Completion census.
    ///
    /// A row may already be Ready or may be prospectively woken by the exact
    /// context-scoped reducer-fence observation.  The concrete carrier, body
    /// frame, replay authority, reverse indexes, and sole Effect slot are all
    /// rejoined before the scheduler receives an opaque attestation.
    pub(super) fn attest_schedulable_certified_body_pipeline(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
        fence: Option<crate::sumeragi::v2::LifecycleReducerFenceObservationV1>,
    ) -> Result<ReadyCertifiedBodyPipelineAttestationV1, ReadyCertifiedBodyPipelineAttestationErrorV1>
    {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&ordinal),
            coordinator.durable_records.get(&ordinal),
        ) else {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex);
        };
        let schedulable = match record.state {
            super::LifecycleState::Ready => coordinator.ready_index.contains(&ordinal),
            super::LifecycleState::Waiting(wait) => fence.is_some_and(|fence| {
                !coordinator.ready_index.contains(&ordinal)
                    && wait.source() == fence.source()
                    && wait.observed_generation() < fence.generation()
            }),
            super::LifecycleState::Claimed(_) | super::LifecycleState::Terminal(_) => false,
        };
        if !schedulable
            || record.ordinal != ordinal
            || !matches!(
                record.work_class,
                LifecycleWorkClass::Fetch | LifecycleWorkClass::Store
            )
            || record.key.phase()
                != match record.work_class {
                    LifecycleWorkClass::Fetch => LifecyclePhase::Fetch,
                    LifecycleWorkClass::Store => LifecyclePhase::Store,
                    _ => unreachable!("filtered ordinary body work class"),
                }
            || record.stage.kind()
                != match record.work_class {
                    LifecycleWorkClass::Fetch => LifecycleStageKind::FetchBody,
                    LifecycleWorkClass::Store => LifecycleStageKind::StoreBody,
                    _ => unreachable!("filtered ordinary body work class"),
                }
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || !record.episode.frozen_predecessors.is_empty()
            || coordinator.key_index.get(&record.key) != Some(&ordinal)
            || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || metadata.continuation != super::schema::DurableContinuation::None
            || metadata.reconstruction_source != record.owner.causal_root().digest()
        {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex);
        };
        if record.physical_slots.len() != 1
            || slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            || record.episode.slot_universe != std::collections::BTreeSet::from([slot])
            || record.episode.consumed_slots != std::collections::BTreeSet::from([slot])
        {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex)?;
        if self
            .entries
            .keys()
            .filter(|candidate| candidate.owner == record.owner)
            .count()
            != 1
        {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let work = self.entries.get(&address).ok_or(
            ReadyCertifiedBodyPipelineAttestationErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let carrier_is_exact = match (&work.kind, record.work_class) {
            (
                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
                LifecycleWorkClass::Fetch,
            ) => {
                let candidate = CandidateAdmission::new(
                    record.key,
                    record.owner.causal_root(),
                    record.work_class,
                    record.stage,
                    InitialLifecycleState::Ready,
                    metadata.reconstruction_source,
                    metadata.payload,
                    metadata.replay_authority.clone(),
                    super::PhysicalGeometry::new([PhysicalSlot::new(slot, digest)], [slot]),
                    None,
                );
                completion.ready_digest() == Some(digest)
                    && completion.matches_recovered_candidate(&candidate)
            }
            (ConcreteLifecycleWorkKind::DurableStoreBody(store), LifecycleWorkClass::Store) => {
                store.matches_recovered_record(coordinator.active_context, record, metadata, digest)
            }
            (
                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_)
                | ConcreteLifecycleWorkKind::DurableStoreBody(_),
                _,
            ) => false,
            _ => {
                return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::WrongWorkKind);
            }
        };
        if !carrier_is_exact {
            return Err(ReadyCertifiedBodyPipelineAttestationErrorV1::InvalidCarrier);
        }
        Ok(ReadyCertifiedBodyPipelineAttestationV1 {
            address,
            digest,
            work_class: record.work_class,
            state: record.state,
        })
    }
    /// Attest one exact Ready recovered Decision Fetch and seal its request authority.
    ///
    /// The complete payload-free `FetchBody` effect remains inside its recovered
    /// WAL carrier. Success returns only the typed service demand, the dedicated
    /// lifecycle key, and a move-only authority which the executor can consume
    /// through the fixed certified-request authentication path.
    pub(super) fn attest_ready_recovered_decision_fetch(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<
        ReadyRecoveredDecisionFetchAttestationV1,
        ReadyRecoveredDecisionFetchAttestationErrorV1,
    > {
        if coordinator.fault.is_some() || coordinator.active_lease.is_some() {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&ordinal),
            coordinator.durable_records.get(&ordinal),
        ) else {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex);
        };
        if record.ordinal != ordinal
            || record.work_class != LifecycleWorkClass::Fetch
            || record.key.phase() != LifecyclePhase::Fetch
            || record.stage.kind() != LifecycleStageKind::FetchBody
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || record.state != super::LifecycleState::Ready
            || !record.episode.frozen_predecessors.is_empty()
            || !coordinator.ready_index.contains(&ordinal)
            || coordinator.key_index.get(&record.key) != Some(&ordinal)
            || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || metadata.continuation != super::schema::DurableContinuation::None
            || metadata.payload != DurablePayloadReference::None
        {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex);
        };
        if record.physical_slots.len() != 1
            || slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
        {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex)?;
        if self
            .entries
            .keys()
            .filter(|candidate| candidate.owner == record.owner)
            .count()
            != 1
        {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let work = self.entries.get(&address).ok_or(
            ReadyRecoveredDecisionFetchAttestationErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::WrongWorkKind);
        };
        if fetch.dispatch_key.is_some()
            || fetch.wait_source.is_some()
            || !fetch.matches_current_ready_record(address, digest, coordinator)
        {
            return Err(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCarrier);
        }
        let identity = RecoveredDecisionFetchDispatchIdentityV1::new(
            coordinator.active_context,
            address,
            digest,
        );
        let dispatch_key = identity.key();
        let request = fetch
            .carrier
            .project_recovered_decision_fetch_request(identity)
            .ok_or(ReadyRecoveredDecisionFetchAttestationErrorV1::InvalidCarrier)?;
        Ok(ReadyRecoveredDecisionFetchAttestationV1 {
            demand: ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor,
            dispatch_key,
            request: Some(request),
            _seal: ReadyRecoveredDecisionFetchAttestationSealV1,
        })
    }
    /// Join one exact claimed recovered Decision Fetch back to its closed carrier.
    pub(super) fn matches_claimed_dispatched_recovered_decision_fetch(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        key: RecoveredDecisionFetchDispatchKeyV1,
        wait_source: super::WaitSource,
    ) -> bool {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class() != LifecycleWorkClass::Fetch
            || lease.key().phase() != LifecyclePhase::Fetch
            || lease.stage().kind() != LifecycleStageKind::FetchBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return false;
        }
        let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
            return false;
        };
        let Some(address) = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot) else {
            return false;
        };
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            || !key.matches(coordinator.active_context, address, digest)
        {
            return false;
        }
        let Some(work) = self.entries.get(&address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return false;
        };
        work.validates_at(address)
            && work.digest == digest
            && fetch.dispatch_key == Some(key)
            && fetch.wait_source == Some(wait_source)
            && matches!(wait_source, super::WaitSource::External(_))
            && fetch.matches_claimed_record(address, digest, coordinator, lease)
    }
    /// Join one queue-selected response to its exact externally parked
    /// recovered Decision Fetch and installed request owner.
    pub(super) fn matches_waiting_dispatched_recovered_decision_fetch(
        &self,
        coordinator: &LifecycleCoordinator,
        key: RecoveredDecisionFetchDispatchKeyV1,
        wait_source: super::WaitSource,
    ) -> bool {
        if coordinator.fault.is_some()
            || coordinator.active_lease.is_some()
            || !matches!(wait_source, super::WaitSource::External(_))
        {
            return false;
        }
        let ordinal = key.lifecycle_ordinal();
        let Some(record) = coordinator.records.get(&ordinal) else {
            return false;
        };
        let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
            return false;
        };
        let Some(address) = ConcreteWorkAddress::new(record.owner, ordinal, slot) else {
            return false;
        };
        if record.work_class != LifecycleWorkClass::Fetch
            || record.key.phase() != LifecyclePhase::Fetch
            || record.stage.kind() != LifecycleStageKind::FetchBody
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || record.physical_slots.len() != 1
            || slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            || !key.matches(coordinator.active_context, address, digest)
            || coordinator.records.iter().any(|(candidate, other)| {
                *candidate != ordinal
                    && matches!(
                        other.state,
                        super::LifecycleState::Waiting(wait)
                            if wait.source() == wait_source
                    )
            })
        {
            return false;
        }
        let Some(work) = self.entries.get(&address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &work.kind else {
            return false;
        };
        work.validates_at(address)
            && work.digest == digest
            && fetch.dispatch_key == Some(key)
            && fetch.wait_source == Some(wait_source)
            && fetch.matches_waiting_record(address, digest, coordinator, wait_source)
    }
    /// Join one exact claimed recovered Decision Fetch back to its closed carrier.
    pub(super) fn prepare_recovered_decision_fetch_dispatch(
        &mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        key: RecoveredDecisionFetchDispatchKeyV1,
    ) -> Result<
        PreparedRecoveredDecisionFetchDispatchV1<'_>,
        RecoveredDecisionFetchDispatchProjectionErrorV1,
    > {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class() != LifecycleWorkClass::Fetch
            || lease.key().phase() != LifecyclePhase::Fetch
            || lease.stage().kind() != LifecycleStageKind::FetchBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::InvalidLease);
        }
        let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::InvalidLease);
        };
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0) {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::InvalidLease);
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredDecisionFetchDispatchProjectionErrorV1::InvalidLease)?;
        if !key.matches(coordinator.active_context, address, digest) {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::InvalidLease);
        }
        let work = self.entries.get_mut(&address).ok_or(
            RecoveredDecisionFetchDispatchProjectionErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) = &mut work.kind
        else {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::WrongWorkKind);
        };
        if !fetch.matches_claimed_record(address, digest, coordinator, lease) {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::InvalidCarrier);
        }
        if fetch.dispatch_key.is_some() || fetch.wait_source.is_some() {
            return Err(RecoveredDecisionFetchDispatchProjectionErrorV1::AlreadyDispatched);
        }
        Ok(PreparedRecoveredDecisionFetchDispatchV1 { work: fetch, key })
    }
    /// Project a comparison-only seal for this exact registry instance.
    pub(super) fn instance_identity(&self) -> ConcreteLifecycleWorkRegistryInstanceIdentity {
        ConcreteLifecycleWorkRegistryInstanceIdentity(std::sync::Arc::clone(&self.identity))
    }
    /// Whether this registry has no installed concrete authority.
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Consume one exact durable control projection into its dedicated carrier.
    ///
    /// Every projection, opened-frame, unique-row, standalone-owner, address,
    /// geometry, digest, and vacancy check precedes the sole insertion. The
    /// existing durable row is never rewritten here; a coalesced restart only
    /// reconstructs this volatile carrier.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_control_sign<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: AuthenticatedRecoveredWalControlProjection,
    ) -> Result<
        InstalledRecoveredWalControlSignRegistryCut<'registry>,
        RecoveredWalControlSignInstallError,
    > {
        if !self.entries.is_empty()
            || !projection.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let records = ledger
            .records()
            .iter()
            .filter(|record| projection.names_record(record))
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        if !projection.exactly_matches_record(record) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), record.ordinal(), slot) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        let carrier =
            match projection.into_durable_carrier(address.owner, address.ordinal, address.slot) {
                Ok(carrier) => carrier,
                Err(projection) => {
                    return Err(RecoveredWalControlSignInstallError {
                        failure: RecoveredWalControlSignInstallFailure::Projection {
                            _projection: projection,
                        },
                    });
                }
            };
        let digest = carrier.installed_digest();
        if !carrier.validates_in_store(store) || self.entries.contains_key(&address) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Carrier { _carrier: carrier },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(
                DurableRecoveredWalControlSignWork {
                    carrier,
                    address,
                    dispatch_key: None,
                },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalControlSignRegistryCut {
            registry: self,
            address,
            digest,
            next_sign: None,
            pair: None,
        })
    }
    /// Consume one exact Advanced control Sign and its live Broadcast child.
    ///
    /// The opened ledger first authenticates the parent continuation and child
    /// row against the recovered WAL projection and verified roster. This
    /// method retains the complete parent carrier beneath the child and
    /// installs only the Ready Broadcast address. No volatile Sign dispatch
    /// key is reconstructed during cold recovery.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(super) fn install_recovered_control_signed_broadcast<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        control: AuthenticatedRecoveredWalControlProjection,
        broadcast: RecoveredLifecycleSignedBroadcastProjectionV1,
        parent_ordinal: u128,
        child_ordinal: u128,
    ) -> Result<
        InstalledRecoveredWalControlSignRegistryCut<'registry>,
        RecoveredWalControlSignInstallError,
    > {
        if !self.entries.is_empty()
            || !control.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
            || !store.revalidates_recovered_control_signed_broadcast(
                verified,
                &control,
                &broadcast,
                parent_ordinal,
                child_ordinal,
            )
        {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastProjection {
                    _projection: control,
                    _broadcast: broadcast,
                },
            });
        }
        let (Some(parent_record), Some(child_record)) = (
            ledger
                .records()
                .binary_search_by_key(&parent_ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index)),
            ledger
                .records()
                .binary_search_by_key(&child_ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index)),
        ) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastProjection {
                    _projection: control,
                    _broadcast: broadcast,
                },
            });
        };
        let parent_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let (Some(parent_address), Some(child_address)) = (
            ConcreteWorkAddress::new(parent_record.owner(), parent_ordinal, parent_slot),
            ConcreteWorkAddress::new(parent_record.owner(), child_ordinal, child_slot),
        ) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastProjection {
                    _projection: control,
                    _broadcast: broadcast,
                },
            });
        };
        if child_record.owner() != parent_record.owner()
            || !broadcast.exactly_matches_record(child_record, parent_record.owner())
            || !broadcast.validates_at(verified, child_address, broadcast.digest())
        {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastProjection {
                    _projection: control,
                    _broadcast: broadcast,
                },
            });
        }
        let parent = match control.into_durable_carrier(
            parent_address.owner,
            parent_address.ordinal,
            parent_address.slot,
        ) {
            Ok(carrier) => carrier,
            Err(control) => {
                return Err(RecoveredWalControlSignInstallError {
                    failure: RecoveredWalControlSignInstallFailure::BroadcastProjection {
                        _projection: control,
                        _broadcast: broadcast,
                    },
                });
            }
        };
        if !parent.validates_signed_broadcast_in_store(verified, &broadcast, store, child_ordinal) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastCarrier {
                    _parent: parent,
                    _broadcast: broadcast,
                },
            });
        }
        let digest = broadcast.digest();
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::Control(
                        DurableRecoveredWalControlSignWork {
                            carrier: parent,
                            address: parent_address,
                            dispatch_key: None,
                        },
                    ),
                    broadcast,
                    verified: verified.clone(),
                    address: child_address,
                    paired_next_sign: None,
                },
            ),
        };
        assert!(work.validates_at(child_address));
        let previous = self.entries.insert(child_address, work);
        assert!(previous.is_none());
        Ok(InstalledRecoveredWalControlSignRegistryCut {
            registry: self,
            address: child_address,
            digest,
            next_sign: None,
            pair: None,
        })
    }
    /// Consume one exact control-Proposal Broadcast plus follow-on WAL Vote.
    ///
    /// The complete frame-bound pair is checked before either child enters the
    /// process-local registry. The combined executable projection splits only
    /// in the assertion-only installation tail: Broadcast retains the original
    /// control Sign carrier, while the next Vote Sign receives its independent
    /// WAL-derived owner and remains undispatched.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(super) fn install_recovered_control_signed_broadcast_and_sign<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        control: AuthenticatedRecoveredWalControlProjection,
        combined: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
        pair: super::ledger::RecoveredLifecycleSignedBroadcastAndSignLedgerProjectionV1,
    ) -> Result<
        InstalledRecoveredWalControlSignRegistryCut<'registry>,
        RecoveredWalControlSignInstallError,
    > {
        let preflight_is_exact = self.entries.is_empty()
            && control.is_exact(verified)
            && pair.parent()
                == super::ledger::RecoveredLifecycleSignedBroadcastAndSignParentV1::ControlProposal
            && pair.exactly_matches_ledger(ledger)
            && store.load().is_ok_and(|opened| opened == *ledger)
            && store.revalidates_recovered_control_signed_broadcast_and_sign(
                verified, &control, &combined, &pair,
            );
        let record_at = |ordinal| {
            ledger
                .records()
                .binary_search_by_key(&ordinal, |record| record.ordinal())
                .ok()
                .and_then(|index| ledger.records().get(index))
        };
        let Some(parent_record) = record_at(pair.parent_ordinal()) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection {
                    _projection: control,
                    _combined: combined,
                },
            });
        };
        let Some(broadcast_record) = record_at(pair.broadcast_ordinal()) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection {
                    _projection: control,
                    _combined: combined,
                },
            });
        };
        let Some(next_sign_record) = record_at(pair.next_sign_ordinal()) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection {
                    _projection: control,
                    _combined: combined,
                },
            });
        };
        let parent_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let broadcast_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let next_sign_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let addresses = (
            ConcreteWorkAddress::new(parent_record.owner(), parent_record.ordinal(), parent_slot),
            ConcreteWorkAddress::new(
                broadcast_record.owner(),
                broadcast_record.ordinal(),
                broadcast_slot,
            ),
            ConcreteWorkAddress::new(
                next_sign_record.owner(),
                next_sign_record.ordinal(),
                next_sign_slot,
            ),
        );
        let (Some(parent_address), Some(broadcast_address), Some(next_sign_address)) = addresses
        else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection {
                    _projection: control,
                    _combined: combined,
                },
            });
        };
        if !preflight_is_exact
            || !control.exactly_matches_advanced_record(parent_record, pair.broadcast_ordinal())
            || !combined.exactly_matches_fresh_records(
                ledger.context(),
                broadcast_record,
                next_sign_record,
            )
            || broadcast_address.owner != parent_address.owner
            || next_sign_address.owner == parent_address.owner
            || self.entries.contains_key(&broadcast_address)
            || self.entries.contains_key(&next_sign_address)
        {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection {
                    _projection: control,
                    _combined: combined,
                },
            });
        }
        let parent = match control.into_durable_carrier(
            parent_address.owner,
            parent_address.ordinal,
            parent_address.slot,
        ) {
            Ok(parent) => parent,
            Err(control) => {
                return Err(RecoveredWalControlSignInstallError {
                    failure: RecoveredWalControlSignInstallFailure::BroadcastAndSignProjection {
                        _projection: control,
                        _combined: combined,
                    },
                });
            }
        };
        let (broadcast, next_sign) = combined.into_registry_children(
            RecoveredLifecycleBroadcastAndSignRegistryCommitPermitV1::new(),
        );
        let broadcast_digest = broadcast.digest();
        let next_sign_digest = next_sign.digest();
        assert!(parent.validates_signed_broadcast_in_store(
            verified,
            &broadcast,
            store,
            broadcast_address.ordinal,
        ));
        let broadcast_work = ConcreteLifecycleWork {
            digest: broadcast_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                DurableRecoveredLifecycleSignedBroadcastWork {
                    parent: DurableRecoveredLifecycleSignParentV1::Control(
                        DurableRecoveredWalControlSignWork {
                            carrier: parent,
                            address: parent_address,
                            dispatch_key: None,
                        },
                    ),
                    broadcast,
                    verified: verified.clone(),
                    address: broadcast_address,
                    paired_next_sign: Some((next_sign_address, next_sign_digest)),
                },
            ),
        };
        let next_sign_work = ConcreteLifecycleWork {
            digest: next_sign_digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                DurableRecoveredLifecycleNextWalVoteSignWork {
                    projection: next_sign,
                    verified: verified.clone(),
                    address: next_sign_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(broadcast_work.validates_at(broadcast_address));
        assert!(next_sign_work.validates_at(next_sign_address));
        assert!(
            self.entries
                .insert(broadcast_address, broadcast_work)
                .is_none()
        );
        assert!(
            self.entries
                .insert(next_sign_address, next_sign_work)
                .is_none()
        );
        Ok(InstalledRecoveredWalControlSignRegistryCut {
            registry: self,
            address: broadcast_address,
            digest: broadcast_digest,
            next_sign: Some((next_sign_address, next_sign_digest)),
            pair: Some(pair),
        })
    }
    /// Consume one exact durable Decision Fetch projection into its carrier.
    ///
    /// All projection, row, owner, address, geometry, digest, store, and
    /// vacancy checks precede the sole insertion. An exact coalesced ledger
    /// row is read-only; this method reconstructs only process-local authority.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_decision_fetch<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<
        InstalledRecoveredWalDecisionFetchRegistryCut<'registry>,
        RecoveredWalDecisionFetchInstallError,
    > {
        if !self.entries.is_empty()
            || !projection.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let records = ledger
            .records()
            .iter()
            .filter(|record| projection.names_record(record))
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        if !projection.exactly_matches_record(record) {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), record.ordinal(), slot) else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        let carrier =
            match projection.into_durable_carrier(address.owner, address.ordinal, address.slot) {
                Ok(carrier) => carrier,
                Err(projection) => {
                    return Err(RecoveredWalDecisionFetchInstallError {
                        failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                            _projection: projection,
                        },
                    });
                }
            };
        let digest = carrier.installed_digest();
        if !carrier.validates_in_store(store) || self.entries.contains_key(&address) {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Carrier { _carrier: carrier },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(
                DurableRecoveredWalDecisionFetchWork {
                    carrier,
                    address,
                    dispatch_key: None,
                    wait_source: None,
                },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalDecisionFetchRegistryCut {
            registry: self,
            address,
            digest,
        })
    }
    /// Consume one advanced recovered Fetch and live Store into a dedicated carrier.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_decision_store<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        ledger_store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        fetch: AuthenticatedRecoveredWalDecisionFetchProjection,
        store: RecoveredDecisionFetchStoreProjectionV1,
    ) -> Result<
        InstalledRecoveredWalDecisionFetchRegistryCut<'registry>,
        RecoveredWalDecisionFetchInstallError,
    > {
        if !self.entries.is_empty()
            || !fetch.is_exact(verified)
            || !store.is_exact(verified)
            || !ledger_store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::StoreProjection {
                    _fetch: fetch,
                    _store: store,
                },
            });
        }
        let Ok((fetch_ordinal, store_ordinal)) =
            ledger.authenticate_recovered_decision_fetch_store(&fetch, &store)
        else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::StoreProjection {
                    _fetch: fetch,
                    _store: store,
                },
            });
        };
        let Some(fetch_record) = ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == fetch_ordinal)
        else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::StoreProjection {
                    _fetch: fetch,
                    _store: store,
                },
            });
        };
        let fetch_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let store_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(fetch_address) =
            ConcreteWorkAddress::new(fetch_record.owner(), fetch_ordinal, fetch_slot)
        else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::StoreProjection {
                    _fetch: fetch,
                    _store: store,
                },
            });
        };
        let Some(store_address) =
            ConcreteWorkAddress::new(fetch_record.owner(), store_ordinal, store_slot)
        else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::StoreProjection {
                    _fetch: fetch,
                    _store: store,
                },
            });
        };
        let carrier = match fetch.into_durable_carrier(
            fetch_address.owner,
            fetch_address.ordinal,
            fetch_address.slot,
        ) {
            Ok(carrier) => carrier,
            Err(fetch) => {
                return Err(RecoveredWalDecisionFetchInstallError {
                    failure: RecoveredWalDecisionFetchInstallFailure::StoreProjection {
                        _fetch: fetch,
                        _store: store,
                    },
                });
            }
        };
        let context = ledger.context();
        let digest = store.digest();
        if !carrier.validates_recovered_store_in_store(&store, ledger_store)
            || !store.validates_at(context, store_address, digest)
            || self.entries.contains_key(&store_address)
        {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::StoreCarrier {
                    _fetch: carrier,
                    _store: store,
                },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(
                DurableRecoveredDecisionStoreWork {
                    fetch: carrier,
                    store,
                    context,
                    address: store_address,
                },
            ),
        };
        debug_assert!(work.validates_at(store_address));
        let previous = self.entries.insert(store_address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalDecisionFetchRegistryCut {
            registry: self,
            address: store_address,
            digest,
        })
    }
    /// Consume one exact recovered Decision body projection into its Apply carrier.
    ///
    /// The supplied ledger is already the fully authenticated prospective
    /// successor. Its exact four-row lineage, final Apply ordinal, carrier
    /// digest, and empty-registry vacancy are all checked before insertion.
    /// Splitting the staged value is permit-gated here so neither the cold
    /// adapter nor the concrete carrier can be substituted by a caller.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_decision_apply<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: Box<RecoveredDecisionApplyStagedStorageV1>,
        effects: Vec<AdapterEffect>,
    ) -> Result<
        (
            ProductionLifecycleAdapterStartupV1,
            InstalledRecoveredDecisionApplyRegistryCut<'registry>,
        ),
        RecoveredDecisionApplyInstallError,
    > {
        if !self.entries.is_empty() || !projection.validates(verified) {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply failed exact registry preflight",
                projection,
                effects,
            ));
        }
        let (restaged, apply_ordinal, _) =
            match ledger.stage_recovered_decision_apply(projection.as_ref()) {
                Ok(staged) => staged,
                Err(_) => {
                    return Err(RecoveredDecisionApplyInstallError::projection(
                        "recovered Decision Apply ledger lineage is not exact",
                        projection,
                        effects,
                    ));
                }
            };
        if restaged != *ledger {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply prospective ledger is incomplete",
                projection,
                effects,
            ));
        }
        let Some(record) = ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == apply_ordinal)
        else {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply ledger has no final row",
                projection,
                effects,
            ));
        };
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), apply_ordinal, slot) else {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply address is invalid",
                projection,
                effects,
            ));
        };
        let authority = match projection.into_registry_carrier(
            RecoveredDecisionApplyRegistryProjectionPermit::new(),
            verified,
            effects,
        ) {
            Ok(parts) => parts,
            Err((projection, effects)) => {
                return Err(RecoveredDecisionApplyInstallError::projection(
                    "recovered Decision Apply retained residual adapter effects",
                    projection,
                    effects,
                ));
            }
        };
        self.validate_recovered_decision_apply_carrier(verified, address, authority)
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn validate_recovered_decision_apply_carrier<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        address: ConcreteWorkAddress,
        authority: Box<(
            ProductionLifecycleAdapterStartupV1,
            RecoveredDecisionApplyRegistryCarrierV1,
        )>,
    ) -> Result<
        (
            ProductionLifecycleAdapterStartupV1,
            InstalledRecoveredDecisionApplyRegistryCut<'registry>,
        ),
        RecoveredDecisionApplyInstallError,
    > {
        let digest = authority.1.installed_digest();
        if !authority.1.validates(verified) || self.entries.contains_key(&address) {
            return Err(RecoveredDecisionApplyInstallError::carrier(
                "recovered Decision Apply carrier disagrees with durable lineage",
                authority,
            ));
        }
        Ok(self.commit_recovered_decision_apply_carrier(address, digest, authority))
    }
    #[inline(never)]
    fn commit_recovered_decision_apply_carrier<'registry>(
        &'registry mut self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        authority: Box<(
            ProductionLifecycleAdapterStartupV1,
            RecoveredDecisionApplyRegistryCarrierV1,
        )>,
    ) -> (
        ProductionLifecycleAdapterStartupV1,
        InstalledRecoveredDecisionApplyRegistryCut<'registry>,
    ) {
        let (adapter, carrier) = *authority;
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(
                DurableRecoveredDecisionApplyWork {
                    carrier,
                    address,
                    dispatch_key: None,
                },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        (
            adapter,
            InstalledRecoveredDecisionApplyRegistryCut {
                registry: self,
                address,
                digest,
            },
        )
    }
    /// Install the startup Serve/Producer batch only after proving the exact
    /// prospective Fetch/(optional Sign)/Serve/Producer census. Rejection is
    /// before both registry mutation and the publication callback.
    pub(super) fn install_certified_serve_startup_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        coordinator: &LifecycleCoordinator,
        owner_held_outputs: &std::collections::BTreeSet<u128>,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_startup_registry(self, coordinator, owner_held_outputs) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        self.install_certified_serve_batch_before_publication(batch, publish)
    }
    /// Install one fresh adjacent Serve/Producer batch only after comparing the
    /// complete current and prospective concrete census. No raw ordinal or
    /// digest enters this boundary.
    pub(super) fn install_certified_serve_fresh_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        verified: &VerifiedHeightContext,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_fresh_registry(self, verified, current, staged) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        self.install_certified_serve_batch_before_publication(batch, publish)
    }
    /// Install a complete Certified-Serve/ProducerTurn batch immediately
    /// around one durable publication. The full registry and batch are checked
    /// before the first insertion. Publication failure removes every inserted
    /// carrier and returns the reconstructed move-only batch.
    pub(super) fn install_certified_serve_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_registry(self) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        let mut staged = StagedCertifiedServeRegistryBatch {
            entries: &mut self.entries,
            addresses: Vec::with_capacity(batch.entries.len()),
        };
        for (address, work) in batch.entries {
            staged.addresses.push(address);
            let displaced = staged.entries.insert(address, work);
            debug_assert!(displaced.is_none(), "complete preflight fixed vacancy");
            if displaced.is_some() {
                unreachable!("exclusive registry borrow cannot change after preflight")
            }
        }
        match publish() {
            Ok(published) => {
                staged.commit();
                Ok(published)
            }
            Err(error) => Err(CertifiedServeRegistryBatchPublicationError::Publication(
                error,
                staged.rollback(),
            )),
        }
    }
    /// Publish the exact terminal LedgerV1 successor while the registry's
    /// eventual Producer replacement is staged at the same address.
    ///
    /// Ledger failure restores the byte-for-byte incumbent before returning.
    /// Ledger success is followed only by infallible exact-address removals:
    /// Serve always leaves the registry, and cancellation removes Producer as
    /// well. No allocation or fallible callback occurs after Ledger fsync.
    pub(super) fn publish_certified_serve_terminal_transition<T, E>(
        &mut self,
        prepared: PreparedCertifiedServeTerminalRegistryTransitionV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeTerminalRegistryPublicationError<E>> {
        if !prepared.preflights_current(self, current, lease)
            || !prepared.preflights_exact_staged_successor(current, staged, lease)
        {
            return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        }
        if prepared.outcome == super::TerminalOutcome::Cancelled {
            if !prepared.preflights_cancelled_successor(staged) {
                return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                    prepared,
                ));
            }
            return match publish() {
                Ok(published) => {
                    drop(
                        self.entries
                            .remove(&prepared.serve_address)
                            .expect("terminal preflight retained the exact Serve carrier"),
                    );
                    drop(
                        self.entries
                            .remove(&prepared.producer_address)
                            .expect("cancel preflight retained the exact Producer carrier"),
                    );
                    Ok(published)
                }
                Err(error) => Err(CertifiedServeTerminalRegistryPublicationError::Publication(
                    error, prepared,
                )),
            };
        }
        let Some(replacement) = prepared.producer_replacement(staged) else {
            return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        };
        let incumbent = std::mem::replace(
            self.entries
                .get_mut(&prepared.producer_address)
                .expect("terminal preflight retained the exact Producer carrier"),
            replacement,
        );
        let staged_registry = StagedCertifiedServeTerminalProducer {
            entries: &mut self.entries,
            producer_address: prepared.producer_address,
            incumbent: Some(incumbent),
        };
        match publish() {
            Ok(published) => {
                staged_registry.commit();
                drop(
                    self.entries
                        .remove(&prepared.serve_address)
                        .expect("terminal preflight retained the exact Serve carrier"),
                );
                Ok(published)
            }
            Err(error) => {
                staged_registry.rollback();
                Err(CertifiedServeTerminalRegistryPublicationError::Publication(
                    error, prepared,
                ))
            }
        }
    }
    /// Prepare the sole carrier removal authorized by one claimed
    /// ProducerTurn and the complete active lifecycle census.
    pub(super) fn prepare_producer_turn_terminal_transition(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
        attempted: &AttemptedProducerTurnV1,
    ) -> Option<PreparedProducerTurnTerminalRegistryTransitionV1> {
        let claimed = attempted.claimed();
        let lease = claimed.lease();
        if !self.exactly_covers_all_live_work_with_active_producer(verified, coordinator, lease)
            || super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
                .ok()?
                .frame_identity()
                != claimed.ledger_frame
            || lease.ordinal() != claimed.address.ordinal
            || lease.work_class() != LifecycleWorkClass::ProducerTurn
            || lease.stage().kind() != LifecycleStageKind::ProducerTurn
            || lease.stage().predecessor_scope() != PredecessorScope::ProducerHandoffBarrier
            || lease.output_reservation().is_some()
        {
            return None;
        }
        let record = coordinator.records.get(&lease.ordinal())?;
        let metadata = coordinator.durable_records.get(&lease.ordinal())?;
        let (&serve_ordinal, _) = coordinator
            .producer_debts
            .iter()
            .find(|(_, producer)| **producer == lease.ordinal())?;
        let serve = coordinator.records.get(&serve_ordinal)?;
        let serve_metadata = coordinator.durable_records.get(&serve_ordinal)?;
        let work = self.entries.get(&claimed.address)?;
        let ConcreteLifecycleWorkKind::DurableProducerTurn(producer) = &work.kind else {
            return None;
        };
        if work.digest != claimed.digest
            || !producer.matches_claimed_record(record, metadata, work.digest, lease)
            || serve_ordinal.checked_add(1) != Some(record.ordinal)
            || !matches!(
                serve.state,
                super::LifecycleState::Terminal(outcome)
                    if outcome != super::TerminalOutcome::Cancelled
            )
            || !serve_ordinal_pair_is_exact(serve, record)
            || !serve_metadata
                .replay_authority
                .same_persisted_family(&metadata.replay_authority)
        {
            return None;
        }
        Some(PreparedProducerTurnTerminalRegistryTransitionV1 {
            address: claimed.address,
            digest: claimed.digest,
            ledger_frame: claimed.ledger_frame,
            _linearity: PreparedProducerTurnTerminalRegistryTransitionLinearityV1,
        })
    }

    /// Publish the exact ProducerTurn terminal successor around LedgerV1 fsync.
    /// Success is followed only by infallible removal of the prevalidated
    /// carrier; publication failure leaves the incumbent byte-for-byte intact.
    pub(super) fn publish_producer_turn_terminal_transition<T, E>(
        &mut self,
        prepared: PreparedProducerTurnTerminalRegistryTransitionV1,
        verified: &VerifiedHeightContext,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        attempted: &AttemptedProducerTurnV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, ProducerTurnTerminalRegistryPublicationError<E>> {
        let claimed = attempted.claimed();
        let lease = claimed.lease();
        let Some(work) = self.entries.get(&prepared.address) else {
            return Err(ProducerTurnTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        };
        let ConcreteLifecycleWorkKind::DurableProducerTurn(producer) = &work.kind else {
            return Err(ProducerTurnTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        };
        let exact_current = prepared.address == claimed.address
            && prepared.digest == claimed.digest
            && prepared.ledger_frame == claimed.ledger_frame
            && super::ledger::LifecycleLedgerV1::from_coordinator(current)
                .is_ok_and(|ledger| ledger.frame_identity() == prepared.ledger_frame)
            && self.exactly_covers_all_live_work_with_active_producer(verified, current, lease)
            && current
                .records
                .get(&lease.ordinal())
                .zip(current.durable_records.get(&lease.ordinal()))
                .is_some_and(|(record, metadata)| {
                    work.digest == prepared.digest
                        && work.validates_at(prepared.address)
                        && producer.matches_claimed_record(record, metadata, prepared.digest, lease)
                });
        let mut expected = current.stage_durable_transaction();
        expected.reduce_settle_turn(lease.clone(), super::TurnOutcome::Advanced, None);
        let same_ledger_target = matches!(
            (&expected.ledger_store, &staged.ledger_store),
            (Some(expected_store), Some(staged_store))
                if expected_store.same_publication_target(staged_store)
        );
        let exact_staged = expected.episode_authority == staged.episode_authority
            && expected.active_context == staged.active_context
            && expected.records == staged.records
            && expected.key_index == staged.key_index
            && expected.owner_index == staged.owner_index
            && expected.ready_index == staged.ready_index
            && expected.admission_waits == staged.admission_waits
            && expected.active_lease == staged.active_lease
            && expected.high_water == staged.high_water
            && expected.next_lease == staged.next_lease
            && expected.durable_records == staged.durable_records
            && expected.capacity_geometry == staged.capacity_geometry
            && expected.capacity_used == staged.capacity_used
            && expected.capacity_generation == staged.capacity_generation
            && expected.observed_generation == staged.observed_generation
            && expected.producer_debts == staged.producer_debts
            && expected.fault == staged.fault
            && same_ledger_target
            && staged.fault.is_none()
            && staged.active_lease.is_none()
            && staged
                .records
                .get(&prepared.address.ordinal)
                .is_some_and(|record| {
                    record.state
                        == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
                });
        if !exact_current || !exact_staged {
            return Err(ProducerTurnTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        }
        match publish() {
            Ok(value) => {
                drop(
                    self.entries
                        .remove(&prepared.address)
                        .expect("ProducerTurn preflight retained the exact carrier"),
                );
                Ok(value)
            }
            Err(error) => Err(ProducerTurnTerminalRegistryPublicationError::Publication(
                error, prepared,
            )),
        }
    }
    /// Whether the registry contains exactly one internally consistent
    /// recovered-WAL authority carrier and no other work.
    ///
    /// This is the only non-empty startup shape beside which the post-repair
    /// ordinary body-pipeline census may install. The phase-vote, control,
    /// Decision Fetch, recovered Decision Store, or Decision Apply carrier
    /// remains the exclusive durable authority for its causal owner; ordinary
    /// body carriers must use disjoint owners and addresses.
    pub(super) fn contains_only_exact_recovered_wal_authority(&self) -> bool {
        let Some(extra) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        extra.cardinality() != 0 && self.entries.len() == extra.cardinality()
    }
    /// Classify zero or one exact WAL-owned startup carrier.
    ///
    /// `None` from this function means ambiguity (including phase and control
    /// together), while `Some(None)` is the exact zero-carrier shape.
    fn exact_recovered_wal_registry_slot(&self) -> Option<RecoveredWalRegistrySlotV1> {
        let mut signs = self
            .entries
            .iter()
            .filter_map(|(&address, work)| match &work.kind {
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::PhaseVote(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::ControlSign(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::NextVote(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast)
                    if work.validates_at(address)
                        && broadcast.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::SignedBroadcast(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch)
                    if work.validates_at(address) && fetch.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::DecisionFetch(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store)
                    if work.validates_at(address) && store.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::DecisionStore(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)
                    if work.validates_at(address) && apply.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::DecisionApply(address))
                }
                _ => None,
            });
        let first = signs.next().unwrap_or(RecoveredWalRegistrySlotV1::None);
        let second = signs.next();
        if signs.next().is_some() {
            return None;
        }
        match (first, second) {
            (single, None) if !matches!(single, RecoveredWalRegistrySlotV1::NextVote(_)) => {
                Some(single)
            }
            (
                RecoveredWalRegistrySlotV1::SignedBroadcast(broadcast),
                Some(RecoveredWalRegistrySlotV1::NextVote(next_sign)),
            )
            | (
                RecoveredWalRegistrySlotV1::NextVote(next_sign),
                Some(RecoveredWalRegistrySlotV1::SignedBroadcast(broadcast)),
            ) if broadcast != next_sign && broadcast.owner != next_sign.owner => {
                Some(RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                    broadcast,
                    next_sign,
                })
            }
            _ => None,
        }
    }
    /// Preflight the complete ordinary body-pipeline batch beside the sole WAL authority.
    pub(super) fn preflights_recovered_body_pipeline_alongside_wal_authority(
        &self,
        works: &[(ConcreteWorkAddress, Option<LifecycleDigest>)],
    ) -> bool {
        let Some(extra) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        let mut addresses = std::collections::BTreeSet::new();
        let mut owners = std::collections::BTreeSet::new();
        extra.cardinality() != 0
            && self.contains_only_exact_recovered_wal_authority()
            && works.iter().all(|(address, digest)| {
                digest.is_some()
                    && !extra.contains_owner(address.owner)
                    && !self.entries.contains_key(address)
                    && addresses.insert(*address)
                    && owners.insert(address.owner)
            })
    }
    /// Install one sealed member of the authenticated ordinary body census.
    pub(super) fn install_recovered_durable_body_pipeline(
        &mut self,
        work: PreparedDurableCertifiedBodyPipelineWorkV1,
    ) -> Result<(), (RegistryError, PreparedDurableCertifiedBodyPipelineWorkV1)> {
        let address = work.address();
        let concrete = match work {
            PreparedDurableCertifiedBodyPipelineWorkV1::Fetch(completion) => {
                match ConcreteLifecycleWork::from_recovered_durable_fetch(completion) {
                    Ok(work) => work,
                    Err(completion) => {
                        return Err((
                            RegistryError::CorruptWork,
                            PreparedDurableCertifiedBodyPipelineWorkV1::Fetch(completion),
                        ));
                    }
                }
            }
            PreparedDurableCertifiedBodyPipelineWorkV1::Store(store) => {
                match ConcreteLifecycleWork::from_recovered_durable_store(store) {
                    Ok(work) => work,
                    Err(store) => {
                        return Err((
                            RegistryError::CorruptWork,
                            PreparedDurableCertifiedBodyPipelineWorkV1::Store(store),
                        ));
                    }
                }
            }
            PreparedDurableCertifiedBodyPipelineWorkV1::Validate(validate) => {
                match ConcreteLifecycleWork::from_recovered_durable_validate(validate) {
                    Ok(work) => work,
                    Err(validate) => {
                        return Err((
                            RegistryError::CorruptWork,
                            PreparedDurableCertifiedBodyPipelineWorkV1::Validate(validate),
                        ));
                    }
                }
            }
        };
        let digest = concrete.digest();
        self.install(address, digest, concrete)
            .map_err(|(error, work)| {
                let recovered = match work.kind {
                    ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                        PreparedDurableCertifiedBodyPipelineWorkV1::Fetch(completion)
                    }
                    ConcreteLifecycleWorkKind::DurableStoreBody(store) => {
                        PreparedDurableCertifiedBodyPipelineWorkV1::Store(store)
                    }
                    ConcreteLifecycleWorkKind::DurableValidateBody(validate) => {
                        PreparedDurableCertifiedBodyPipelineWorkV1::Validate(validate)
                    }
                    _ => unreachable!("recovered body install retains its closed carrier kind"),
                };
                (error, recovered)
            })
    }
    /// Verify complete equality between installed ordinary body carriers and
    /// all live coordinator Fetch, Store, and Validate rows.
    pub(super) fn exactly_covers_recovered_ready_body_pipeline(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let owner_held_outputs =
            Self::owner_held_output_ordinals(coordinator, RecoveredWalRegistrySlotV1::None);
        self.exactly_covers_recovered_ready_body_pipeline_with_extra_and_outputs(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
            &owner_held_outputs,
        )
    }
    fn exactly_covers_recovered_ready_body_pipeline_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        let owner_held_outputs = Self::owner_held_output_ordinals(coordinator, extra);
        self.exactly_covers_recovered_ready_body_pipeline_with_extra_and_outputs(
            coordinator,
            extra,
            &owner_held_outputs,
        )
    }
    fn exactly_covers_recovered_ready_body_pipeline_with_extra_and_outputs(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
        owner_held_outputs: &std::collections::BTreeSet<u128>,
    ) -> bool {
        let live_body_pipeline = coordinator
            .records
            .values()
            .filter(|record| {
                matches!(
                    record.work_class,
                    LifecycleWorkClass::Fetch
                        | LifecycleWorkClass::Store
                        | LifecycleWorkClass::Validate
                ) && !matches!(record.state, super::LifecycleState::Terminal(_))
                    && !extra.contains_record(record)
            })
            .collect::<Vec<_>>();
        self.entries.len() == live_body_pipeline.len() + extra.cardinality()
            && self.exact_optional_recovered_wal_authority(
                coordinator,
                extra,
                owner_held_outputs,
                &std::collections::BTreeSet::new(),
            )
            && live_body_pipeline.into_iter().all(|record| {
                if record.state != super::LifecycleState::Ready || record.physical_slots.len() != 1
                {
                    return false;
                }
                let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
                    return false;
                };
                if record.episode.consumed_slots != std::collections::BTreeSet::from([slot])
                    || record.episode.slot_universe != std::collections::BTreeSet::from([slot])
                {
                    return false;
                }
                let Some(address) = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                else {
                    return false;
                };
                let Some(metadata) = coordinator.durable_records.get(&record.ordinal) else {
                    return false;
                };
                self.entries.get(&address).is_some_and(|work| {
                    work.digest == digest
                        && work.validates_at(address)
                        && match (&work.kind, record.work_class) {
                            (
                                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
                                LifecycleWorkClass::Fetch,
                            ) => {
                                let candidate = CandidateAdmission::new(
                                    record.key,
                                    record.owner.causal_root(),
                                    record.work_class,
                                    record.stage,
                                    InitialLifecycleState::Ready,
                                    metadata.reconstruction_source,
                                    metadata.payload,
                                    metadata.replay_authority.clone(),
                                    super::PhysicalGeometry::new(
                                        [PhysicalSlot::new(slot, digest)],
                                        [slot],
                                    ),
                                    None,
                                );
                                completion.ready_digest() == Some(digest)
                                    && completion.matches_recovered_candidate(&candidate)
                            }
                            (
                                ConcreteLifecycleWorkKind::DurableStoreBody(store),
                                LifecycleWorkClass::Store,
                            ) => store.matches_recovered_record(
                                coordinator.active_context,
                                record,
                                metadata,
                                digest,
                            ),
                            (
                                ConcreteLifecycleWorkKind::DurableValidateBody(validate),
                                LifecycleWorkClass::Validate,
                            ) => validate.matches_recovered_record(
                                coordinator.active_context,
                                record,
                                metadata,
                                digest,
                            ),
                            _ => false,
                        }
                })
            })
    }
    fn serve_and_producer_carrier_count(&self) -> usize {
        self.entries
            .values()
            .filter(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
                        | ConcreteLifecycleWorkKind::DurableProducerTurn(_)
                )
            })
            .count()
    }
    /// Verify exact startup coverage for every live durable body, Serve, and
    /// ProducerTurn row, with no additional concrete carrier.
    pub(super) fn exactly_covers_recovered_ready_work(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let owner_held_outputs =
            Self::owner_held_output_ordinals(coordinator, RecoveredWalRegistrySlotV1::None);
        self.exactly_covers_recovered_ready_work_with_extra_and_outputs(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
            &owner_held_outputs,
        )
    }
    /// Verify startup coverage while authenticated cold outputs remain owner-held.
    pub(super) fn exactly_covers_recovered_ready_work_with_owner_held_outputs(
        &self,
        coordinator: &LifecycleCoordinator,
        owner_held_outputs: &std::collections::BTreeSet<u128>,
    ) -> bool {
        self.exactly_covers_recovered_ready_work_with_extra_and_outputs(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
            owner_held_outputs,
        )
    }
    /// Verify exact startup coverage beside the one recovered-WAL authority.
    pub(super) fn exactly_covers_recovered_ready_work_and_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        let owner_held_outputs = Self::owner_held_output_ordinals(coordinator, sign);
        !matches!(sign, RecoveredWalRegistrySlotV1::None)
            && self.exactly_covers_recovered_ready_work_with_extra_and_outputs(
                coordinator,
                sign,
                &owner_held_outputs,
            )
    }
    /// Return the exact Ready ordinal retained by recovered Decision Apply startup.
    ///
    /// The ordinary live Apply carrier is intentionally excluded: only the
    /// recovered WAL slot, its exact Ready row, and the complete unrelated
    /// recovered-work census can authorize pending-Kura direct-Apply startup.
    pub(super) fn exact_recovered_decision_apply_ready_ordinal(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> Option<u128> {
        let Some(extra @ RecoveredWalRegistrySlotV1::DecisionApply(address)) =
            self.exact_recovered_wal_registry_slot()
        else {
            return None;
        };
        self.exactly_covers_recovered_ready_work_with_extra(coordinator, extra)
            .then_some(address.ordinal)
    }
    /// Verify WAL-authority startup coverage beside authenticated cold outputs.
    pub(super) fn exactly_covers_recovered_ready_work_and_wal_authority_with_owner_held_outputs(
        &self,
        coordinator: &LifecycleCoordinator,
        owner_held_outputs: &std::collections::BTreeSet<u128>,
    ) -> bool {
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        !matches!(sign, RecoveredWalRegistrySlotV1::None)
            && self.exactly_covers_recovered_ready_work_with_extra_and_outputs(
                coordinator,
                sign,
                owner_held_outputs,
            )
    }

    /// Verify a bijection between every nonterminal logical row and every
    /// process-local concrete carrier across the exhaustive work-class set.
    ///
    /// This oracle is read-only and performs no selection. Fresh Serve
    /// admission uses it to prove that appending its adjacent pair cannot hide,
    /// replace, or skip unrelated live work of another class.
    #[allow(clippy::too_many_lines)]
    pub(super) fn exactly_covers_all_live_work(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.exactly_covers_all_live_work_with_optional_active_producer(verified, coordinator, None)
    }

    /// Verify the same exhaustive census while one exact ProducerTurn owns the
    /// coordinator's sole volatile lease.
    pub(super) fn exactly_covers_all_live_work_with_active_producer(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        self.exactly_covers_all_live_work_with_optional_active_producer(
            verified,
            coordinator,
            Some(lease),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn exactly_covers_all_live_work_with_optional_active_producer(
        &self,
        verified: &VerifiedHeightContext,
        coordinator: &LifecycleCoordinator,
        active_producer: Option<&TurnLease>,
    ) -> bool {
        let active_producer_is_exact = match (&coordinator.active_lease, active_producer) {
            (None, None) => true,
            (Some(active), Some(expected)) => {
                active == expected && active.work_class == LifecycleWorkClass::ProducerTurn
            }
            (None, Some(_)) | (Some(_), None) => false,
        };
        if coordinator.fault.is_some()
            || !active_producer_is_exact
            || coordinator.active_context != projection::lifecycle_context(verified.context())
            || coordinator.episode_authority.context() != coordinator.active_context
            || coordinator.episode_authority.capacity_geometry() != &coordinator.capacity_geometry
            || coordinator.records.len() != coordinator.durable_records.len()
            || coordinator.records.len() != coordinator.key_index.len()
        {
            return false;
        }
        let exact_capacity_classes = CapacityClass::ALL
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        if coordinator
            .capacity_generation
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            != exact_capacity_classes
            || coordinator
                .capacity_geometry
                .limits
                .keys()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                != exact_capacity_classes
        {
            return false;
        }
        if coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS
            || coordinator.admission_waits.iter().any(|(key, waiting)| {
                let candidate = &waiting.candidate;
                let mut canonical = candidate.clone();
                let WaitSource::Capacity(class) = waiting.wait_token.source else {
                    return true;
                };
                let candidate_slots_are_exact =
                    candidate
                        .physical_geometry
                        .normalized()
                        .is_ok_and(|(_, slots, _)| {
                            coordinator
                                .episode_authority
                                .universe_for(candidate.key)
                                .is_some()
                                && coordinator
                                    .episode_authority
                                    .admits_slots(candidate.work_class.capacity_class(), &slots)
                        });
                let producer_slots_are_exact =
                    candidate.producer_turn.as_ref().is_none_or(|producer| {
                        producer
                            .physical_geometry
                            .normalized()
                            .is_ok_and(|(_, slots, _)| {
                                coordinator
                                    .episode_authority
                                    .universe_for(producer.key)
                                    .is_some()
                                    && coordinator
                                        .episode_authority
                                        .admits_slots(CapacityClass::Producer, &slots)
                            })
                    });
                let producer_shape_is_invalid =
                    match (candidate.work_class, candidate.producer_turn.as_ref()) {
                        (LifecycleWorkClass::CertifiedServe, Some(producer)) => {
                            !super::schema::serve_and_producer_keys_match(
                                candidate.key,
                                producer.key,
                            ) || producer.stage.kind != LifecycleStageKind::ProducerTurn
                                || producer.reconstruction_source != candidate.reconstruction_source
                        }
                        (LifecycleWorkClass::CertifiedServe, None) | (_, Some(_)) => true,
                        (_, None) => false,
                    };
                key != &candidate.key
                    || coordinator.key_index.contains_key(key)
                    || candidate.work_class == LifecycleWorkClass::ProducerTurn
                    || !candidate
                        .work_class
                        .accepts_stage(candidate.key.phase, candidate.stage)
                    || !candidate
                        .payload
                        .matches_terminal(candidate.work_class, None)
                    || (candidate.work_class == LifecycleWorkClass::Validate
                        && !super::body_pipeline_transition::durable_validate_payload_is_exact(
                            candidate.key,
                            candidate.payload,
                        ))
                    || matches!(
                        candidate.initial_state,
                        InitialLifecycleState::Waiting(WaitToken {
                            source: WaitSource::Capacity(_)
                                | WaitSource::Recovery(_)
                                | WaitSource::ProducerTurn(_),
                            ..
                        })
                    )
                    || matches!(
                        candidate.initial_state,
                        InitialLifecycleState::Waiting(WaitToken {
                            observed_generation: u64::MAX,
                            ..
                        })
                    )
                    || canonical.canonicalize_geometry().is_err()
                    || canonical != *candidate
                    || !candidate.replay_authority_is_exact(coordinator.active_context)
                    || !candidate_slots_are_exact
                    || !producer_slots_are_exact
                    || producer_shape_is_invalid
                    || (class != candidate.work_class.capacity_class()
                        && !(class == CapacityClass::Producer && candidate.producer_turn.is_some()))
                    || waiting.wait_token.observed_generation
                        > coordinator.capacity_generation[&class]
                    || waiting.serve_payload_receipt.is_some()
                        && candidate.work_class != LifecycleWorkClass::CertifiedServe
            })
        {
            return false;
        }
        let Ok(exact_ledger) = super::ledger::LifecycleLedgerV1::from_coordinator(coordinator)
        else {
            return false;
        };
        let mut exact_owners = BTreeMap::new();
        if coordinator.records.iter().any(|(&ordinal, record)| {
            let frozen_predecessors_are_invalid =
                record
                    .episode
                    .frozen_predecessors
                    .iter()
                    .any(|predecessor| {
                        *predecessor >= ordinal || !coordinator.records.contains_key(predecessor)
                    })
                    || (matches!(
                        record.stage.predecessor_scope,
                        PredecessorScope::Independent
                    ) && !record.episode.frozen_predecessors.is_empty())
                    || (!matches!(
                        record.stage.predecessor_scope,
                        PredecessorScope::Independent
                    ) && coordinator
                        .records
                        .range(..ordinal)
                        .any(|(predecessor, prior)| {
                            !matches!(prior.state, super::LifecycleState::Terminal(_))
                                && !record.episode.frozen_predecessors.contains(predecessor)
                        }));
            let wait_state_is_invalid = match record.state {
                super::LifecycleState::Waiting(wait) => match wait.source {
                    WaitSource::Capacity(_) => true,
                    WaitSource::External(_) | WaitSource::Recovery(_) => {
                        wait.observed_generation == u64::MAX
                            || coordinator
                                .observed_generation
                                .get(&wait.source)
                                .copied()
                                .unwrap_or(0)
                                != wait.observed_generation
                    }
                    WaitSource::ProducerTurn(serve_ordinal) => {
                        record.work_class != LifecycleWorkClass::ProducerTurn
                            || wait.observed_generation != 0
                            || coordinator.producer_debts.get(&serve_ordinal) != Some(&ordinal)
                    }
                },
                super::LifecycleState::Ready | super::LifecycleState::Terminal(_) => false,
                super::LifecycleState::Claimed(lease_id) => active_producer.is_none_or(|lease| {
                    record.ordinal != lease.ordinal
                        || record.work_class != LifecycleWorkClass::ProducerTurn
                        || lease_id != lease.id
                }),
            };
            let unique_digests = record
                .physical_slots
                .values()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            record.ordinal != ordinal
                || coordinator.key_index.get(&record.key) != Some(&ordinal)
                || record.owner.first_admission_ordinal() == 0
                || record.owner.first_admission_ordinal() > ordinal
                || coordinator
                    .episode_authority
                    .universe_for(record.key)
                    .as_ref()
                    != Some(&record.episode.universe)
                || !coordinator.episode_authority.admits_slots(
                    record.work_class.capacity_class(),
                    &record.episode.slot_universe,
                )
                || !record
                    .physical_slots
                    .keys()
                    .all(|slot| record.episode.slot_universe.contains(slot))
                || !record
                    .episode
                    .consumed_slots
                    .is_subset(&record.episode.slot_universe)
                || unique_digests.len() != record.physical_slots.len()
                || frozen_predecessors_are_invalid
                || wait_state_is_invalid
                || exact_owners
                    .insert(record.owner.causal_root(), record.owner)
                    .is_some_and(|known| known != record.owner)
                || coordinator
                    .durable_records
                    .get(&ordinal)
                    .is_none_or(|metadata| {
                        !metadata.replay_authority.structurally_matches_record(
                            coordinator.active_context,
                            record.key,
                            record.work_class,
                            record.stage,
                            metadata.payload,
                        )
                    })
        }) || coordinator.owner_index != exact_owners
        {
            return false;
        }
        let exact_ready = coordinator
            .records
            .values()
            .filter_map(|record| {
                (record.state == super::LifecycleState::Ready).then_some(record.ordinal)
            })
            .collect::<std::collections::BTreeSet<_>>();
        if coordinator.ready_index != exact_ready {
            return false;
        }
        let exact_capacity_used = CapacityClass::ALL
            .into_iter()
            .map(|class| {
                (
                    class,
                    coordinator
                        .records
                        .values()
                        .filter(|record| {
                            record.work_class.capacity_class() == class
                                && !matches!(record.state, super::LifecycleState::Terminal(_))
                        })
                        .count(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if coordinator.capacity_used != exact_capacity_used
            || CapacityClass::ALL.into_iter().any(|class| {
                exact_capacity_used[&class] > coordinator.capacity_geometry.limit(class)
            })
        {
            return false;
        }
        let live = coordinator
            .records
            .iter()
            .filter(|(_, record)| !matches!(record.state, super::LifecycleState::Terminal(_)))
            .collect::<Vec<_>>();
        if self.entries.len() != live.len() {
            return false;
        }
        if !coordinator
            .producer_debts
            .iter()
            .all(|(&serve_ordinal, &producer_ordinal)| {
                let (Some(serve), Some(producer)) = (
                    coordinator.records.get(&serve_ordinal),
                    coordinator.records.get(&producer_ordinal),
                ) else {
                    return false;
                };
                let (Some(serve_metadata), Some(producer_metadata)) = (
                    coordinator.durable_records.get(&serve_ordinal),
                    coordinator.durable_records.get(&producer_ordinal),
                ) else {
                    return false;
                };
                if !serve_ordinal_pair_is_exact(serve, producer)
                    || !serve_metadata
                        .replay_authority
                        .same_persisted_family(&producer_metadata.replay_authority)
                {
                    return false;
                }
                if matches!(serve.state, super::LifecycleState::Terminal(_)) {
                    return true;
                }
                let (Some((serve_slot, _)), Some((producer_slot, _))) = (
                    exact_single_record_slot(
                        serve,
                        LifecycleWorkClass::CertifiedServe.capacity_class(),
                    ),
                    exact_single_record_slot(
                        producer,
                        LifecycleWorkClass::ProducerTurn.capacity_class(),
                    ),
                ) else {
                    return false;
                };
                let (Some(serve_address), Some(producer_address)) = (
                    ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot),
                    ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot),
                ) else {
                    return false;
                };
                matches!(
                    (
                        self.entries.get(&serve_address).map(|work| &work.kind),
                        self.entries.get(&producer_address).map(|work| &work.kind),
                    ),
                    (
                        Some(ConcreteLifecycleWorkKind::DurableCertifiedServe(serve)),
                        Some(ConcreteLifecycleWorkKind::DurableProducerTurn(producer)),
                    ) if Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)
                )
            })
        {
            return false;
        }
        let exact_next_vote_addresses = self
            .entries
            .iter()
            .filter_map(|(&address, work)| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
                )
                .then_some(address)
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut paired_next_vote_addresses = std::collections::BTreeSet::new();
        if self.entries.values().any(|work| {
            let ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) =
                &work.kind
            else {
                return false;
            };
            let Some((next_address, next_digest)) = broadcast.paired_next_sign else {
                return !broadcast.is_unpaired();
            };
            !broadcast.pairs_exact_next_sign(next_address, next_digest)
                || !paired_next_vote_addresses.insert(next_address)
                || self.entries.get(&next_address).is_none_or(|next_work| {
                    next_work.digest != next_digest
                        || !matches!(
                            &next_work.kind,
                            ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
                        )
                })
        }) || !paired_next_vote_addresses.is_subset(&exact_next_vote_addresses)
        {
            return false;
        }

        live.into_iter().all(|(&ordinal, record)| {
            if record.ordinal != ordinal
                || matches!(record.state, super::LifecycleState::Claimed(_))
                    && active_producer.is_none_or(|lease| {
                        record.ordinal != lease.ordinal
                            || record.state != super::LifecycleState::Claimed(lease.id)
                    })
                || coordinator.key_index.get(&record.key) != Some(&ordinal)
                || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
                || coordinator.high_water < ordinal
            {
                return false;
            }
            let Some(metadata) = coordinator.durable_records.get(&ordinal) else {
                return false;
            };
            if !metadata.replay_authority.structurally_matches_record(
                coordinator.active_context,
                record.key,
                record.work_class,
                record.stage,
                metadata.payload,
            ) {
                return false;
            }
            let Some((slot, digest)) =
                exact_single_record_slot(record, record.work_class.capacity_class())
            else {
                return false;
            };
            let Some(address) = ConcreteWorkAddress::new(record.owner, ordinal, slot) else {
                return false;
            };
            let Some(work) = self.entries.get(&address) else {
                return false;
            };
            if work.digest != digest || !work.validates_at(address) {
                return false;
            }

            let candidate_core_matches = |candidate: &CandidateAdmission| {
                let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized()
                else {
                    return false;
                };
                candidate.key == record.key
                    && candidate.causal_root == record.owner.causal_root()
                    && candidate.work_class == record.work_class
                    && candidate.stage == record.stage
                    && candidate.reconstruction_source == metadata.reconstruction_source
                    && candidate.producer_turn.is_none()
                    && physical.len() == 1
                    && physical.contains_key(&slot)
                    && record.episode.slot_universe == universe
                    && record.episode.consumed_slots == consumed
                    && metadata.matches_admission(candidate)
                    && metadata.continuation == super::schema::DurableContinuation::None
            };

            match &work.kind {
                ConcreteLifecycleWorkKind::PendingAdapter {
                    effect,
                    pending,
                    replay_authority,
                } => {
                    let Ok(projected) = projection::authority_free_admission_projection(
                        coordinator.active_context,
                        verified,
                        effect,
                        pending,
                    ) else {
                        return false;
                    };
                    let Ok((physical, universe, consumed)) =
                        projected.physical_geometry.normalized()
                    else {
                        return false;
                    };
                    let payload_is_exact = match (
                        projected.work_class,
                        projected.stage.kind(),
                        metadata.payload,
                    ) {
                        (
                            LifecycleWorkClass::Apply,
                            LifecycleStageKind::ApplyDecision,
                            DurablePayloadReference::BodyFrame(frame),
                        ) => frame.matches_key(record.key),
                        (LifecycleWorkClass::Apply, _, _) => false,
                        (_, _, DurablePayloadReference::None) => true,
                        _ => false,
                    };
                    let candidate = CandidateAdmission::new(
                        projected.key,
                        projected.causal_root,
                        projected.work_class,
                        projected.stage,
                        projected.initial_state,
                        projected.reconstruction_source,
                        metadata.payload,
                        metadata.replay_authority.clone(),
                        projected.physical_geometry,
                        None,
                    );
                    candidate.initial_state == InitialLifecycleState::Ready
                        && candidate_core_matches(&candidate)
                        && physical == record.physical_slots
                        && universe == record.episode.slot_universe
                        && consumed == record.episode.consumed_slots
                        && payload_is_exact
                        && replay_authority == &metadata.replay_authority
                }
                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) => {
                    let candidate = CandidateAdmission::new(
                        record.key,
                        record.owner.causal_root(),
                        record.work_class,
                        record.stage,
                        InitialLifecycleState::Ready,
                        metadata.reconstruction_source,
                        metadata.payload,
                        metadata.replay_authority.clone(),
                        super::PhysicalGeometry::new([PhysicalSlot::new(slot, digest)], [slot]),
                        None,
                    );
                    record.state == super::LifecycleState::Ready
                        && candidate_core_matches(&candidate)
                        && completion.matches_recovered_candidate(&candidate)
                }
                ConcreteLifecycleWorkKind::DurableStoreBody(store) => {
                    store.project_candidate(verified).is_ok_and(|candidate| {
                        candidate_core_matches(&candidate)
                            && candidate
                                .physical_geometry
                                .normalized()
                                .is_ok_and(|(physical, _, _)| physical == record.physical_slots)
                    })
                }
                ConcreteLifecycleWorkKind::DurableValidateBody(validate) => {
                    validate.project_candidate(verified).is_ok_and(|candidate| {
                        candidate_core_matches(&candidate)
                            && candidate
                                .physical_geometry
                                .normalized()
                                .is_ok_and(|(physical, _, _)| physical == record.physical_slots)
                    })
                }
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                    record.state == super::LifecycleState::Ready
                        && completion
                            .incumbent
                            .project_candidate(verified)
                            .is_ok_and(|candidate| candidate_core_matches(&candidate))
                }
                ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) => {
                    apply.dispatch_key.is_none()
                        && apply.validates_in_ledger(&exact_ledger)
                        && apply.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableLiveWalSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.validates_in_ledger(&exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.repair.validates_in_ledger(&exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.validates_in_ledger(&exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                    sign.dispatch_key.is_none()
                        && sign.carrier.validates_in_ledger(verified, &exact_ledger)
                        && sign.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(broadcast) => {
                    broadcast.validates_in_ledger(&exact_ledger)
                        && broadcast.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                    fetch.carrier.validates_in_ledger(verified, &exact_ledger)
                        && match (fetch.dispatch_key, fetch.wait_source) {
                            (None, None) => {
                                fetch.matches_current_ready_record(address, digest, coordinator)
                            }
                            (Some(key), Some(source)) => {
                                key.matches(coordinator.active_context, address, digest)
                                    && fetch.matches_waiting_record(
                                        address,
                                        digest,
                                        coordinator,
                                        source,
                                    )
                            }
                            (None, Some(_)) | (Some(_), None) => false,
                        }
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store) => {
                    store.store.is_exact(verified)
                        && store.fetch.validates(verified)
                        && store
                            .fetch
                            .validates_recovered_store_in_ledger(&store.store, &exact_ledger)
                        && store.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                    apply.dispatch_key.is_none()
                        && apply.carrier.validates_in_ledger(
                            verified,
                            &exact_ledger,
                            address.ordinal,
                        )
                        && apply.matches_current_ready_record(address, digest, coordinator)
                }
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => {
                    serve.matches_record(record, metadata, digest)
                }
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => active_producer
                    .map_or_else(
                        || producer.matches_record(record, metadata, digest),
                        |lease| {
                            if record.ordinal == lease.ordinal {
                                producer.matches_claimed_record(record, metadata, digest, lease)
                            } else {
                                producer.matches_record(record, metadata, digest)
                            }
                        },
                    ),
            }
        })
    }

    fn exactly_covers_recovered_ready_work_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        let owner_held_outputs = Self::owner_held_output_ordinals(coordinator, extra);
        self.exactly_covers_recovered_ready_work_with_extra_and_outputs(
            coordinator,
            extra,
            &owner_held_outputs,
        )
    }

    fn exactly_covers_ready_work_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
        owner_held_outputs: &std::collections::BTreeSet<u128>,
        active_serve: Option<&TurnLease>,
        refanned_broadcasts: &std::collections::BTreeSet<u128>,
    ) -> bool {
        let live = coordinator
            .records
            .values()
            .filter(|record| {
                matches!(
                    record.work_class,
                    LifecycleWorkClass::Fetch
                        | LifecycleWorkClass::Store
                        | LifecycleWorkClass::Validate
                        | LifecycleWorkClass::CertifiedServe
                        | LifecycleWorkClass::ProducerTurn
                ) && !matches!(record.state, super::LifecycleState::Terminal(_))
                    && !extra.contains_record(record)
            })
            .collect::<Vec<_>>();
        self.entries.len() == live.len() + extra.cardinality() + refanned_broadcasts.len()
            && self.exact_optional_recovered_wal_authority(
                coordinator,
                extra,
                owner_held_outputs,
                refanned_broadcasts,
            )
            && live.into_iter().all(|record| {
                let is_active_serve = active_serve.is_some_and(|lease| {
                    record.work_class == LifecycleWorkClass::CertifiedServe
                        && record.ordinal == lease.ordinal
                        && record.state == super::LifecycleState::Claimed(lease.id)
                });
                if record.work_class != LifecycleWorkClass::ProducerTurn
                    && record.state != super::LifecycleState::Ready
                    && !is_active_serve
                {
                    return false;
                }
                let Some((slot, digest)) =
                    exact_single_record_slot(record, record.work_class.capacity_class())
                else {
                    return false;
                };
                let Some(address) = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                else {
                    return false;
                };
                let Some(metadata) = coordinator.durable_records.get(&record.ordinal) else {
                    return false;
                };
                self.entries.get(&address).is_some_and(|work| {
                    work.digest == digest
                        && work.validates_at(address)
                        && match (&work.kind, record.work_class) {
                            (
                                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
                                LifecycleWorkClass::Fetch,
                            ) => {
                                let candidate = CandidateAdmission::new(
                                    record.key,
                                    record.owner.causal_root(),
                                    record.work_class,
                                    record.stage,
                                    InitialLifecycleState::Ready,
                                    metadata.reconstruction_source,
                                    metadata.payload,
                                    metadata.replay_authority.clone(),
                                    super::PhysicalGeometry::new(
                                        [PhysicalSlot::new(slot, digest)],
                                        [slot],
                                    ),
                                    None,
                                );
                                completion.ready_digest() == Some(digest)
                                    && completion.matches_recovered_candidate(&candidate)
                            }
                            (
                                ConcreteLifecycleWorkKind::DurableStoreBody(store),
                                LifecycleWorkClass::Store,
                            ) => store.matches_recovered_record(
                                coordinator.active_context,
                                record,
                                metadata,
                                digest,
                            ),
                            (
                                ConcreteLifecycleWorkKind::DurableValidateBody(validate),
                                LifecycleWorkClass::Validate,
                            ) => validate.matches_recovered_record(
                                coordinator.active_context,
                                record,
                                metadata,
                                digest,
                            ),
                            (
                                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve),
                                LifecycleWorkClass::CertifiedServe,
                            ) => active_serve.map_or_else(
                                || serve.matches_record(record, metadata, digest),
                                |lease| {
                                    if record.ordinal == lease.ordinal {
                                        serve
                                            .matches_claimed_record(record, metadata, digest, lease)
                                    } else {
                                        serve.matches_record(record, metadata, digest)
                                    }
                                },
                            ),
                            (
                                ConcreteLifecycleWorkKind::DurableProducerTurn(producer),
                                LifecycleWorkClass::ProducerTurn,
                            ) => producer.matches_record(record, metadata, digest),
                            _ => false,
                        }
                })
            })
    }
    fn exactly_covers_active_certified_serve_lease(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class != LifecycleWorkClass::CertifiedServe
        {
            return false;
        }
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        self.exactly_covers_ready_work_with_extra(
            coordinator,
            sign,
            &std::collections::BTreeSet::new(),
            Some(lease),
            &std::collections::BTreeSet::new(),
        )
    }
    /// Prove the complete private registry and exact active Serve lease without
    /// consulting caller-supplied request material.
    pub(super) fn preflight_certified_serve_terminal_owner_state(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if !self.exactly_covers_active_certified_serve_lease(coordinator, lease) {
            return false;
        }
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal) else {
            return false;
        };
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            coordinator.records.get(&lease.ordinal),
            coordinator.durable_records.get(&lease.ordinal),
            coordinator.records.get(&producer_ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        let (Some((serve_slot, _)), Some((producer_slot, _))) = (
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class()),
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class()),
        ) else {
            return false;
        };
        let (Some(serve_address), Some(producer_address)) = (
            ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot),
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot),
        ) else {
            return false;
        };
        let (Some(serve_work), Some(producer_work)) = (
            self.entries.get(&serve_address),
            self.entries.get(&producer_address),
        ) else {
            return false;
        };
        matches!(
            (&serve_work.kind, &producer_work.kind),
            (
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve_carrier),
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer_carrier),
            ) if serve_ordinal_pair_is_exact(serve, producer)
                && serve_metadata
                    .replay_authority
                    .same_persisted_family(&producer_metadata.replay_authority)
                && Arc::ptr_eq(
                    &serve_carrier.replay_evidence,
                    &producer_carrier.replay_evidence,
                )
        )
    }
    /// Join an exact signed request only after the complete owner-private state
    /// has independently passed preflight.
    pub(super) fn preflight_certified_serve_terminal_settlement(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        if !self.preflight_certified_serve_terminal_owner_state(coordinator, lease) {
            return false;
        }
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal) else {
            return false;
        };
        let (Some(serve_metadata), Some(producer_metadata)) = (
            coordinator.durable_records.get(&lease.ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        serve_metadata
            .replay_authority
            .exactly_matches_certified_serve_request(authenticated)
            && producer_metadata
                .replay_authority
                .exactly_matches_certified_serve_request(authenticated)
    }
    /// Close one post-fsync terminal replay family over the already-preflighted
    /// active Serve and adjacent Producer carriers.
    pub(super) fn prepare_certified_serve_terminal_transition(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        terminal: &CertifiedServeTerminalReplayAuthorityPairV1,
    ) -> Option<PreparedCertifiedServeTerminalRegistryTransitionV1> {
        if !self.preflight_certified_serve_terminal_settlement(coordinator, lease, authenticated) {
            return None;
        }
        let producer_ordinal = *coordinator.producer_debts.get(&lease.ordinal)?;
        let serve = coordinator.records.get(&lease.ordinal)?;
        let serve_metadata = coordinator.durable_records.get(&lease.ordinal)?;
        let producer = coordinator.records.get(&producer_ordinal)?;
        let producer_metadata = coordinator.durable_records.get(&producer_ordinal)?;
        if !terminal.exactly_advances_pending_records(
            coordinator.active_context,
            serve,
            serve_metadata,
            producer,
            producer_metadata,
        ) {
            return None;
        }
        let (serve_slot, _) =
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class())?;
        let (producer_slot, _) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())?;
        let serve_address = ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot)?;
        let producer_address =
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot)?;
        let serve_work = self.entries.get(&serve_address)?;
        let ConcreteLifecycleWorkKind::DurableCertifiedServe(serve_carrier) = &serve_work.kind
        else {
            return None;
        };
        let terminal_replay_evidence = terminal.terminal_carrier_replay_evidence()?;
        Some(PreparedCertifiedServeTerminalRegistryTransitionV1 {
            serve_address,
            producer_address,
            outcome: terminal.terminal_outcome(),
            pending_replay_evidence: Arc::clone(&serve_carrier.replay_evidence),
            terminal_replay_evidence: Arc::new(terminal_replay_evidence),
        })
    }
    fn exact_optional_recovered_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
        owner_held_outputs: &std::collections::BTreeSet<u128>,
        refanned_broadcasts: &std::collections::BTreeSet<u128>,
    ) -> bool {
        let exact_finalization_ledger = if refanned_broadcasts.is_empty() {
            None
        } else {
            let Ok(ledger) = super::ledger::LifecycleLedgerV1::from_coordinator(coordinator) else {
                return false;
            };
            Some(ledger)
        };
        if owner_held_outputs.iter().any(|ordinal| {
            coordinator.records.get(ordinal).is_none_or(|record| {
                !matches!(record.state, super::LifecycleState::Ready)
                    || !matches!(
                        record.work_class,
                        LifecycleWorkClass::Broadcast
                            | LifecycleWorkClass::EquivocationReport
                            | LifecycleWorkClass::InvalidBodyReport
                    )
                    || extra.contains_record(record)
            })
        }) {
            return false;
        }
        if !refanned_broadcasts.iter().all(|ordinal| {
            let Some(record) = coordinator.records.get(ordinal) else {
                return false;
            };
            if !matches!(record.state, super::LifecycleState::Waiting(_))
                || record.work_class != LifecycleWorkClass::Broadcast
                || owner_held_outputs.contains(ordinal)
                || extra.contains_record(record)
            {
                return false;
            }
            let Some((slot, digest)) =
                exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())
            else {
                return false;
            };
            let Some(address) = ConcreteWorkAddress::new(record.owner, *ordinal, slot) else {
                return false;
            };
            let Some(exact_ledger) = exact_finalization_ledger.as_ref() else {
                return false;
            };
            self.entries.get(&address).is_some_and(|work| {
                work.digest == digest
                    && work.validates_at(address)
                    && matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                            broadcast
                        ) if broadcast.validates_in_ledger(exact_ledger)
                            && broadcast.paired_next_sign_matches_terminal_record(
                                coordinator,
                                exact_ledger,
                            )
                            && broadcast.matches_current_finalization_record(
                                address,
                                digest,
                                coordinator,
                            )
                    )
            })
        }) {
            return false;
        }
        let unsupported_live = coordinator
            .records
            .values()
            .filter(|record| {
                !matches!(record.state, super::LifecycleState::Terminal(_))
                    && !owner_held_outputs.contains(&record.ordinal)
                    && !refanned_broadcasts.contains(&record.ordinal)
                    && !matches!(
                        record.work_class,
                        LifecycleWorkClass::Fetch
                            | LifecycleWorkClass::Store
                            | LifecycleWorkClass::Validate
                            | LifecycleWorkClass::CertifiedServe
                            | LifecycleWorkClass::ProducerTurn
                    )
            })
            .collect::<Vec<_>>();
        match extra {
            RecoveredWalRegistrySlotV1::None => unsupported_live.is_empty(),
            RecoveredWalRegistrySlotV1::PhaseVote(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && sign.matches_current_ready_record(address, work.digest, coordinator)
                    )
                })
            }
            RecoveredWalRegistrySlotV1::ControlSign(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && sign.matches_current_ready_record(address, work.digest, coordinator)
                    )
                })
            }
            RecoveredWalRegistrySlotV1::NextVote(_) => false,
            RecoveredWalRegistrySlotV1::SignedBroadcast(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal
                    || record.owner != address.owner
                    || record.work_class != LifecycleWorkClass::Broadcast
                {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                            broadcast
                        ) if record.physical_slots.get(&address.slot) == Some(&work.digest)
                            && broadcast.is_unpaired()
                            && broadcast.matches_current_ready_record(
                                address,
                                work.digest,
                                coordinator,
                            )
                    )
                })
            }
            RecoveredWalRegistrySlotV1::SignedBroadcastAndNextVote {
                broadcast,
                next_sign,
            } => {
                if unsupported_live.len() != 2 {
                    return false;
                }
                let record_at = |address: ConcreteWorkAddress| {
                    unsupported_live.iter().copied().find(|record| {
                        record.owner == address.owner && record.ordinal == address.ordinal
                    })
                };
                let (Some(broadcast_record), Some(next_sign_record)) =
                    (record_at(broadcast), record_at(next_sign))
                else {
                    return false;
                };
                if broadcast_record.work_class != LifecycleWorkClass::Broadcast
                    || next_sign_record.work_class != LifecycleWorkClass::SignVote
                    || next_sign_record.state != super::LifecycleState::Ready
                {
                    return false;
                }
                let Some(&next_sign_digest) = next_sign_record.physical_slots.get(&next_sign.slot)
                else {
                    return false;
                };
                let broadcast_exact = self.entries.get(&broadcast).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(
                            carrier
                        ) if broadcast_record.physical_slots.get(&broadcast.slot)
                            == Some(&work.digest)
                            && carrier.pairs_exact_next_sign(next_sign, next_sign_digest)
                            && carrier.matches_current_ready_record(
                                broadcast,
                                work.digest,
                                coordinator,
                            )
                    )
                });
                let next_sign_exact = self.entries.get(&next_sign).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(
                            carrier
                        ) if next_sign_record.physical_slots.get(&next_sign.slot)
                            == Some(&work.digest)
                            && carrier.matches_current_ready_record(
                                next_sign,
                                work.digest,
                                coordinator,
                            )
                    )
                });
                broadcast_exact && next_sign_exact
            }
            RecoveredWalRegistrySlotV1::DecisionFetch(address) => {
                if !unsupported_live.is_empty() {
                    return false;
                }
                let Some(record) = coordinator.records.get(&address.ordinal) else {
                    return false;
                };
                if record.owner != address.owner
                    || record.ordinal != address.ordinal
                    || record.work_class != LifecycleWorkClass::Fetch
                    || matches!(record.state, super::LifecycleState::Terminal(_))
                {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && match (fetch.dispatch_key, fetch.wait_source) {
                                    (None, None) => fetch.matches_current_ready_record(
                                        address,
                                        work.digest,
                                        coordinator,
                                    ),
                                    (Some(key), Some(source)) => key.matches(
                                        coordinator.active_context,
                                        address,
                                        work.digest,
                                    ) && fetch.matches_waiting_record(
                                        address,
                                        work.digest,
                                        coordinator,
                                        source,
                                    ),
                                    (None, Some(_)) | (Some(_), None) => false,
                                }
                    )
                })
            }
            RecoveredWalRegistrySlotV1::DecisionStore(address) => {
                let Some(record) = coordinator.records.get(&address.ordinal) else {
                    return false;
                };
                if !unsupported_live.is_empty()
                    || record.ordinal != address.ordinal
                    || record.owner != address.owner
                    || record.work_class != LifecycleWorkClass::Store
                    || record.state != super::LifecycleState::Ready
                {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && store.matches_current_ready_record(
                                    address,
                                    work.digest,
                                    coordinator,
                                )
                    )
                })
            }
            RecoveredWalRegistrySlotV1::DecisionApply(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && apply.matches_current_ready_record(
                                    address,
                                    work.digest,
                                    coordinator,
                                )
                    )
                })
            }
        }
    }
    /// Install one work value without overwriting an incumbent address.
    ///
    /// Failure returns the move-only value to the caller so a higher-level
    /// admission transaction can roll back without cloning physical work.
    pub(super) fn install(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        work: ConcreteLifecycleWork,
    ) -> Result<(), (RegistryError, ConcreteLifecycleWork)> {
        if ConcreteWorkAddress::new(address.owner, address.ordinal, address.slot) != Some(address) {
            return Err((RegistryError::InvalidAddress, work));
        }
        if !work.validates_at(address) {
            return Err((RegistryError::CorruptWork, work));
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err((RegistryError::CausalOwnerMismatch, work));
        }
        if work.digest != expected_digest {
            return Err((RegistryError::DigestMismatch, work));
        }
        if self.entries.contains_key(&address) {
            return Err((RegistryError::Occupied, work));
        }
        self.entries.insert(address, work);
        Ok(())
    }
    /// Install exact work, invoke durable publication, and synchronously undo
    /// the installation when publication fails or unwinds.
    ///
    /// The callback cannot access this exclusively borrowed registry, so the
    /// entry installed immediately before it remains the exact rollback target.
    pub(super) fn install_before_publication<T, E>(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        work: ConcreteLifecycleWork,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, RegistryPublicationError<E>> {
        if let Err((error, work)) = self.install(address, expected_digest, work) {
            return Err(RegistryPublicationError::Install(error, work));
        }
        let staged = StagedRegistryInstall {
            entries: &mut self.entries,
            address,
            armed: true,
        };
        match publish() {
            Ok(published) => {
                staged.commit();
                Ok(published)
            }
            Err(error) => {
                let work = staged.rollback();
                debug_assert!(work.validate_exact());
                debug_assert_eq!(work.digest, expected_digest);
                Err(RegistryPublicationError::Publication(error, work))
            }
        }
    }
    /// Install one mandatory replay-bound effect and publish its derived durable row.
    ///
    /// The bound owner and prepared candidate are rechecked together before
    /// constructing registry work.  Any pre-publication or reversible
    /// publication failure reconstructs the same move-only bound owner; no
    /// raw effect-plus-optional-owner pair crosses this boundary.
    pub(super) fn install_bound_before_publication<T, E>(
        &mut self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        bound: BoundAdapterEffectV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, BoundAdapterRegistryPublicationErrorV1<E>> {
        if !bound.exactly_authorizes_candidate(active_context, candidate) {
            return Err(BoundAdapterRegistryPublicationErrorV1::Install(
                RegistryError::CorruptWork,
                bound,
            ));
        }
        let BoundAdapterEffectV1 {
            effect,
            pending,
            replay_origin,
        } = bound;
        let authority = replay_origin.authority().clone();
        let work = match ConcreteLifecycleWork::from_authorized_exact(effect, pending, authority) {
            Ok(work) => work,
            Err((error, effect, pending)) => {
                return Err(BoundAdapterRegistryPublicationErrorV1::Install(
                    error,
                    BoundAdapterEffectV1 {
                        effect,
                        pending,
                        replay_origin,
                    },
                ));
            }
        };
        match self.install_before_publication(address, expected_digest, work, publish) {
            Ok(published) => Ok(published),
            Err(RegistryPublicationError::Install(error, work)) => {
                let (effect, pending) = work.into_pair();
                Err(BoundAdapterRegistryPublicationErrorV1::Install(
                    error,
                    BoundAdapterEffectV1 {
                        effect,
                        pending,
                        replay_origin,
                    },
                ))
            }
            Err(RegistryPublicationError::Publication(error, work)) => {
                let (effect, pending) = work.into_pair();
                Err(BoundAdapterRegistryPublicationErrorV1::Publication(
                    error,
                    BoundAdapterEffectV1 {
                        effect,
                        pending,
                        replay_origin,
                    },
                ))
            }
        }
    }

    /// Install one exact non-Apply live-WAL owner while retaining any mandatory
    /// local Proposal companion inside its typed Sign carrier.
    pub(super) fn install_live_wal_before_publication<T, E>(
        &mut self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        live: PreparedLiveWalAdmissionV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, LiveWalRegistryPublicationErrorV1<E>> {
        if !live.exactly_authorizes_candidate(active_context, candidate) {
            return Err(LiveWalRegistryPublicationErrorV1::Install(
                RegistryError::CorruptWork,
                live,
            ));
        }
        if candidate.work_class == LifecycleWorkClass::Apply {
            return Err(LiveWalRegistryPublicationErrorV1::Install(
                RegistryError::CorruptWork,
                live,
            ));
        }
        match &live.companion {
            PreparedLiveWalCompanionV1::None | PreparedLiveWalCompanionV1::ApplyBodyFrame(_) => {
                let PreparedLiveWalAdmissionV1 { bound, companion } = live;
                let BoundAdapterEffectV1 {
                    effect,
                    pending,
                    replay_origin,
                } = bound;
                let authority = replay_origin.authority().clone();
                let work = match ConcreteLifecycleWork::from_authorized_exact(
                    effect, pending, authority,
                ) {
                    Ok(work) => work,
                    Err((error, effect, pending)) => {
                        return Err(LiveWalRegistryPublicationErrorV1::Install(
                            error,
                            PreparedLiveWalAdmissionV1 {
                                bound: BoundAdapterEffectV1 {
                                    effect,
                                    pending,
                                    replay_origin,
                                },
                                companion,
                            },
                        ));
                    }
                };
                match self.install_before_publication(address, expected_digest, work, publish) {
                    Ok(published) => Ok(published),
                    Err(RegistryPublicationError::Install(error, work)) => {
                        let (effect, pending) = work.into_pair();
                        Err(LiveWalRegistryPublicationErrorV1::Install(
                            error,
                            PreparedLiveWalAdmissionV1 {
                                bound: BoundAdapterEffectV1 {
                                    effect,
                                    pending,
                                    replay_origin,
                                },
                                companion,
                            },
                        ))
                    }
                    Err(RegistryPublicationError::Publication(error, work)) => {
                        let (effect, pending) = work.into_pair();
                        Err(LiveWalRegistryPublicationErrorV1::Publication(
                            error,
                            PreparedLiveWalAdmissionV1 {
                                bound: BoundAdapterEffectV1 {
                                    effect,
                                    pending,
                                    replay_origin,
                                },
                                companion,
                            },
                        ))
                    }
                }
            }
            PreparedLiveWalCompanionV1::LocalProposal(_) => {
                let work = match live.into_live_sign_work(
                    candidate.clone(),
                    DurableLiveWalSignOriginV1::LocalProposal,
                    address,
                ) {
                    Ok(work) => work,
                    Err((live, _candidate, _origin)) => {
                        return Err(LiveWalRegistryPublicationErrorV1::Install(
                            RegistryError::CorruptWork,
                            live,
                        ));
                    }
                };
                if work.digest != expected_digest {
                    let ConcreteLifecycleWorkKind::DurableLiveWalSign(work) = work.kind else {
                        unreachable!("local Proposal conversion retains live Sign work")
                    };
                    return Err(LiveWalRegistryPublicationErrorV1::Install(
                        RegistryError::DigestMismatch,
                        work.admission,
                    ));
                }
                match self.install_before_publication(address, expected_digest, work, publish) {
                    Ok(published) => Ok(published),
                    Err(RegistryPublicationError::Install(error, work)) => {
                        let ConcreteLifecycleWorkKind::DurableLiveWalSign(work) = work.kind else {
                            unreachable!("local Proposal installation returns live Sign work")
                        };
                        Err(LiveWalRegistryPublicationErrorV1::Install(
                            error,
                            work.admission,
                        ))
                    }
                    Err(RegistryPublicationError::Publication(error, work)) => {
                        let ConcreteLifecycleWorkKind::DurableLiveWalSign(work) = work.kind else {
                            unreachable!("local Proposal rollback returns live Sign work")
                        };
                        Err(LiveWalRegistryPublicationErrorV1::Publication(
                            error,
                            work.admission,
                        ))
                    }
                }
            }
        }
    }
    /// Install one origin-specific durable Validate carrier and publish its row.
    ///
    /// Conversion happens only after the coordinator has minted the immutable
    /// address. Any reversible failure reconstructs the same local-body or
    /// remote-Proposal prepared owner from the returned closed carrier.
    pub(super) fn install_durable_validate_before_publication<T, E>(
        &mut self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        validate: PreparedDurableValidateAdmissionV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, DurableValidateRegistryPublicationErrorV1<E>> {
        if !validate.exactly_authorizes_candidate(active_context, candidate) {
            return Err(DurableValidateRegistryPublicationErrorV1::Install(
                RegistryError::CorruptWork,
                validate,
            ));
        }
        let (carrier, digest) = match validate.into_durable_validate_carrier(address) {
            Ok(carrier) => carrier,
            Err(validate) => {
                return Err(DurableValidateRegistryPublicationErrorV1::Install(
                    RegistryError::CorruptWork,
                    validate,
                ));
            }
        };
        if digest != expected_digest {
            let validate = PreparedDurableValidateAdmissionV1::from_returned_carrier(carrier)
                .expect("origin-specific conversion returns an origin-specific carrier");
            return Err(DurableValidateRegistryPublicationErrorV1::Install(
                RegistryError::DigestMismatch,
                validate,
            ));
        }
        let work = match ConcreteLifecycleWork::from_recovered_durable_validate(carrier) {
            Ok(work) => work,
            Err(carrier) => {
                let validate = PreparedDurableValidateAdmissionV1::from_returned_carrier(carrier)
                    .expect("origin-specific conversion returns an origin-specific carrier");
                return Err(DurableValidateRegistryPublicationErrorV1::Install(
                    RegistryError::CorruptWork,
                    validate,
                ));
            }
        };
        match self.install_before_publication(address, expected_digest, work, publish) {
            Ok(published) => Ok(published),
            Err(RegistryPublicationError::Install(error, work)) => {
                let ConcreteLifecycleWorkKind::DurableValidateBody(carrier) = work.kind else {
                    unreachable!("durable Validate installation returns its exact carrier")
                };
                let validate = PreparedDurableValidateAdmissionV1::from_returned_carrier(carrier)
                    .expect("origin-specific installation returns an origin-specific carrier");
                Err(DurableValidateRegistryPublicationErrorV1::Install(
                    error, validate,
                ))
            }
            Err(RegistryPublicationError::Publication(error, work)) => {
                let ConcreteLifecycleWorkKind::DurableValidateBody(carrier) = work.kind else {
                    unreachable!("durable Validate rollback returns its exact carrier")
                };
                let validate = PreparedDurableValidateAdmissionV1::from_returned_carrier(carrier)
                    .expect("origin-specific rollback returns an origin-specific carrier");
                Err(DurableValidateRegistryPublicationErrorV1::Publication(
                    error, validate,
                ))
            }
        }
    }
    /// Replace one exact address before invoking a reversible publication.
    ///
    /// The incumbent remains recoverable until the callback succeeds. A
    /// callback error removes the replacement and restores the byte-for-byte
    /// incumbent before returning the replacement to the caller. Unwinding
    /// also restores the incumbent through an RAII guard. This map is
    /// exclusively borrowed across the callback, so no other registry entry
    /// can observe the staged value or invalidate the rollback address.
    ///
    /// `Err` is valid only when the callback proves that its external target
    /// did not commit. A durability-ambiguous dequeue or publication must
    /// instead cross the process fail-stop boundary; restoring this volatile
    /// map cannot undo an external transition.
    /// This generic seam accepts pending adapter work only. Certified-Fetch
    /// completion must use the specialized conversion below, which moves the
    /// incumbent binding into its closed carrier rather than constructing an
    /// independent replacement proof.
    pub(super) fn replace_before_publication<T, E>(
        &mut self,
        address: ConcreteWorkAddress,
        expected_incumbent_digest: LifecycleDigest,
        expected_replacement_digest: LifecycleDigest,
        replacement: ConcreteLifecycleWork,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<(T, ConcreteLifecycleWork), RegistryReplacementError<E>> {
        if ConcreteWorkAddress::new(address.owner, address.ordinal, address.slot) != Some(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::InvalidAddress,
                replacement,
            ));
        }
        if !replacement.validates_at(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CorruptWork,
                replacement,
            ));
        }
        if !replacement.is_pending_adapter() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::WrongWorkKind,
                replacement,
            ));
        }
        if address.owner.causal_root() != replacement.causal_root() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CausalOwnerMismatch,
                replacement,
            ));
        }
        if replacement.digest != expected_replacement_digest {
            return Err(RegistryReplacementError::Validation(
                RegistryError::DigestMismatch,
                replacement,
            ));
        }
        let Some(incumbent) = self.entries.get(&address) else {
            return Err(RegistryReplacementError::Validation(
                RegistryError::Missing,
                replacement,
            ));
        };
        if !incumbent.validates_at(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CorruptWork,
                replacement,
            ));
        }
        if !incumbent.is_pending_adapter() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::WrongWorkKind,
                replacement,
            ));
        }
        if address.owner.causal_root() != incumbent.causal_root() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CausalOwnerMismatch,
                replacement,
            ));
        }
        if incumbent.digest != expected_incumbent_digest {
            return Err(RegistryReplacementError::Validation(
                RegistryError::DigestMismatch,
                replacement,
            ));
        }
        let incumbent = self
            .entries
            .insert(address, replacement)
            .expect("validated replacement address retains its incumbent");
        let staged = StagedRegistryReplacement {
            entries: &mut self.entries,
            address,
            incumbent: Some(incumbent),
        };
        match publish() {
            Ok(published) => Ok((published, staged.commit())),
            Err(error) => {
                let replacement = staged.rollback();
                debug_assert!(replacement.validate_exact());
                debug_assert_eq!(replacement.digest, expected_replacement_digest);
                Err(RegistryReplacementError::Publication(error, replacement))
            }
        }
    }
    /// Prepare an exact incumbent-to-completion conversion without mutation.
    ///
    /// The sealed selector capability is borrowed only for equality validation.
    /// It is deliberately not stored in the returned token: successful
    /// conversion moves the incumbent registry binding and never mints or
    /// retains a second causal proof. Raw response, responder, hash, queue
    /// identity, and pending-binding inputs are not accepted here.
    pub(super) fn prepare_certified_fetch_completion(
        &mut self,
        location: CertifiedFetchWaitingLocation,
        authority: CertifiedFetchCompletionAuthority<'_>,
    ) -> Result<PreparedCertifiedFetchCompletion<'_>, CertifiedFetchCompletionError> {
        let ingress_identity = authority.ingress_identity();
        let request_hash = authority.request_hash();
        let response_hash = authority.response_hash();
        let authenticated_responder = authority.authenticated_responder();
        let authenticated_response = authority.authenticated_response();
        let candidate_pending = authority.candidate_pending();
        let address = location.address();
        if ConcreteWorkAddress::new(location.owner, location.ordinal, location.slot)
            != Some(address)
        {
            return Err(CertifiedFetchCompletionError::InvalidLocation);
        }
        if ingress_identity.physical_admission_ordinal() == 0
            || !ingress_identity_matches_round(
                ingress_identity,
                authenticated_response.manifest.round,
            )
        {
            return Err(CertifiedFetchCompletionError::InvalidQueueIdentity);
        }
        if authenticated_response.request_hash != request_hash
            || HashOf::new(authenticated_response) != response_hash
        {
            return Err(CertifiedFetchCompletionError::ResponseFamilyMismatch);
        }
        let incumbent = self
            .entries
            .get(&address)
            .ok_or(CertifiedFetchCompletionError::MissingIncumbent)?;
        if !incumbent.validates_at(address) {
            return Err(CertifiedFetchCompletionError::CorruptIncumbent);
        }
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
            ..
        } = &incumbent.kind
        else {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        }
        if location.owner.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::ForeignCausalOwner);
        }
        if authority.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::CandidateBindingMismatch);
        }
        if incumbent.digest != location.incumbent_digest {
            return Err(CertifiedFetchCompletionError::IncumbentDigestMismatch);
        }
        if candidate_pending != incumbent_pending
            || !candidate_pending.exactly_binds_adapter_effect(incumbent_effect)
        {
            return Err(CertifiedFetchCompletionError::CandidateBindingMismatch);
        }
        if !fetch_effect_matches_response(incumbent_effect, authenticated_response) {
            return Err(CertifiedFetchCompletionError::ResponseFamilyMismatch);
        }
        let replay_origin = AuthenticatedCertifiedFetchReplayOriginV1::from_completion_authority(
            &authority,
            incumbent_effect,
        )
        .ok_or(CertifiedFetchCompletionError::InvalidReplayEvidence)?;
        Ok(PreparedCertifiedFetchCompletion {
            registry: self,
            location,
            ingress_identity,
            request_hash,
            response_hash,
            response_round: authenticated_response.manifest.round,
            response_subject: authenticated_response.manifest.subject,
            response_manifest_hash: HashOf::new(&authenticated_response.manifest),
            authenticated_responder: authenticated_responder.clone(),
            replay_origin,
        })
    }
    /// Prepare execution of one exact closed certified-Fetch completion.
    ///
    /// The lease must name the completion's immutable owner, record ordinal,
    /// sole physical slot, and installed response digest, and it must retain
    /// the coordinator's exact independent `FetchBody` stage. No row is taken
    /// or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_certified_fetch_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<PreparedCertifiedFetchExecution<'_>, CertifiedFetchExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Fetch
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(lease.work_class().capacity_class())
        {
            return Err(CertifiedFetchExecutionError::InvalidLeaseShape);
        }
        let address = self
            .validated_lease_address(lease, slot)
            .map_err(CertifiedFetchExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated certified-Fetch execution address remains present");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            return Err(CertifiedFetchExecutionError::WrongWorkKind);
        };
        let AdapterEffect::FetchBody {
            certificate: Some(certificate),
            ..
        } = &completion.incumbent_effect
        else {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        };
        let active_context =
            LifecycleContext::new(lease.key().context(), lease.key().round().height());
        if certified_fetch_lifecycle_key(
            active_context,
            certificate.round,
            certificate.proposal_round,
            certificate.subject,
            certificate.phase,
            certificate.execution_commitment,
        ) != Some(lease.key())
        {
            return Err(CertifiedFetchExecutionError::InvalidLeaseShape);
        }
        if !completion.validates(work.digest) {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        }
        Ok(PreparedCertifiedFetchExecution {
            registry: self,
            address,
        })
    }
    /// Prepare execution of one exact closed durable Store carrier.
    ///
    /// In addition to the address and digest checks shared by all registry
    /// leases, this replays the authenticated adapter projection under the
    /// supplied height context. The projected semantic key, causal owner, and
    /// complete one-slot physical geometry must be identical to the claimed
    /// Store lease. No row is taken or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_durable_store_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedDurableStoreExecution<'_>, DurableStoreExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Store
            || lease.key().phase() != LifecyclePhase::Store
            || lease.stage().kind() != LifecycleStageKind::StoreBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Store.capacity_class())
        {
            return Err(DurableStoreExecutionError::InvalidLeaseShape);
        }
        let address = self
            .validated_lease_address(lease, slot)
            .map_err(DurableStoreExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated durable Store execution address remains present");
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            return Err(DurableStoreExecutionError::WrongWorkKind);
        };
        if !store.validates(work.digest) {
            return Err(DurableStoreExecutionError::InvalidStoreShape);
        }
        let candidate = store
            .project_candidate(verified)
            .map_err(DurableStoreExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&store.durable_receipt)
            .ok_or(DurableStoreExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| DurableStoreExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Store
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots != *lease.physical_slots()
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(DurableStoreExecutionError::InvalidProjection);
        }
        Ok(PreparedDurableStoreExecution {
            registry: self,
            address,
        })
    }
    /// Prepare execution of one exact closed durable Validate carrier.
    ///
    /// The lease, installed carrier, verified projection, and normalized
    /// physical geometry must all describe the same independent one-slot
    /// `ValidateBody` work. No row is taken or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_durable_validate_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedDurableValidateExecution<'_>, DurableValidateExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Validate
            || lease.key().phase() != LifecyclePhase::Validate
            || lease.stage().kind() != LifecycleStageKind::ValidateBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(DurableValidateExecutionError::InvalidLeaseShape);
        }
        let address = self
            .validated_lease_address(lease, slot)
            .map_err(DurableValidateExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated durable Validate execution address remains present");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return Err(DurableValidateExecutionError::WrongWorkKind);
        };
        if !validate.validates(work.digest) {
            return Err(DurableValidateExecutionError::InvalidValidateShape);
        }
        let candidate = validate
            .project_candidate(verified)
            .map_err(DurableValidateExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&validate.durable_receipt)
            .ok_or(DurableValidateExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| DurableValidateExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Validate
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || projected_universe.len() != 1
            || projected_consumed.len() != 1
            || projected_slots != *lease.physical_slots()
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(DurableValidateExecutionError::InvalidProjection);
        }
        Ok(PreparedDurableValidateExecution {
            registry: self,
            address,
            lifecycle_key: lease.key(),
            lifecycle_stage: lease.stage(),
        })
    }
    /// Classify one exact Ready Validate carrier without granting scheduler authority.
    ///
    /// The caller supplies coordinator-owned address and digest coordinates.
    /// Successful classification proves only the process-local carrier shape;
    /// the coordinator must still bind it into its complete Ready census.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn classify_ready_validate_carrier(
        &self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
    ) -> Result<ReadyValidateCarrierSeal, ReadyValidateCarrierError> {
        let work = self
            .entries
            .get(&address)
            .ok_or(ReadyValidateCarrierError::Registry(RegistryError::Missing))?;
        if !work.validates_at(address) {
            return Err(ReadyValidateCarrierError::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != expected_digest {
            return Err(ReadyValidateCarrierError::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::DurableValidateBody(validate)
                if validate.validates(expected_digest) =>
            {
                let payload = durable_validate_body_payload(&validate.durable_receipt)
                    .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                Ok(ReadyValidateCarrierSeal {
                    address,
                    digest: expected_digest,
                    kind: ReadyValidateCarrierKind::ExecuteBody,
                    payload,
                })
            }
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                if completion.validates(expected_digest) =>
            {
                match (
                    completion.outcome.validated_receipt(),
                    completion.outcome.rejection_identity(),
                    completion.outcome.missing_merge_sidecar(),
                ) {
                    (Some(receipt), None, None)
                        if validate_validated_receipt_authority(&completion.incumbent, receipt)
                            .is_ok() =>
                    {
                        let payload =
                            durable_validate_body_payload(&completion.incumbent.durable_receipt)
                                .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                        Ok(ReadyValidateCarrierSeal {
                            address,
                            digest: expected_digest,
                            kind: ReadyValidateCarrierKind::ValidatedCompletion,
                            payload,
                        })
                    }
                    (None, Some(BodyValidationRejectionIdentity::Rejected), None) => {
                        let payload =
                            durable_validate_body_payload(&completion.incumbent.durable_receipt)
                                .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                        Ok(ReadyValidateCarrierSeal {
                            address,
                            digest: expected_digest,
                            kind: ReadyValidateCarrierKind::RejectedCompletion,
                            payload,
                        })
                    }
                    _ => Err(ReadyValidateCarrierError::InvalidCarrier),
                }
            }
            ConcreteLifecycleWorkKind::PendingAdapter { .. }
            | ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_)
            | ConcreteLifecycleWorkKind::DurableStoreBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateCompletion(_)
            | ConcreteLifecycleWorkKind::DurableLiveWalApply(_)
            | ConcreteLifecycleWorkKind::DurableLiveWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(_)
            | ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            | ConcreteLifecycleWorkKind::DurableProducerTurn(_) => {
                Err(ReadyValidateCarrierError::WrongWorkKind)
            }
        }
    }
    /// Attest one exact Ready lifecycle Decision Apply without exposing its carrier.
    ///
    /// This is a read-only join over the coordinator's complete logical row,
    /// durable metadata, reverse indexes, and the registry's immutable closed
    /// carrier. Success discloses only the typed bounded-I/O demand and opaque
    /// exact-position key needed by the production scheduler; it grants no
    /// execution or extraction authority.
    pub(super) fn attest_ready_lifecycle_decision_apply(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<
        ReadyLifecycleDecisionApplyAttestationV1,
        ReadyLifecycleDecisionApplyAttestationErrorV1,
    > {
        let Some(record) = coordinator.records.get(&ordinal) else {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCoordinatorIndex);
        };
        let Some(metadata) = coordinator.durable_records.get(&ordinal) else {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCoordinatorIndex);
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())
        else {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCoordinatorIndex);
        };
        if coordinator.fault.is_some()
            || coordinator.active_lease.is_some()
            || record.ordinal != ordinal
            || record.work_class != LifecycleWorkClass::Apply
            || record.key.phase() != LifecyclePhase::Apply
            || record.stage.kind() != LifecycleStageKind::ApplyDecision
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || record.state != super::LifecycleState::Ready
            || !record.episode.frozen_predecessors.is_empty()
            || coordinator
                .episode_authority
                .universe_for(record.key)
                .as_ref()
                != Some(&record.episode.universe)
            || !coordinator.episode_authority.admits_slots(
                record.work_class.capacity_class(),
                &record.episode.slot_universe,
            )
            || !coordinator.ready_index.contains(&ordinal)
            || coordinator.key_index.get(&record.key) != Some(&ordinal)
            || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || coordinator
                .records
                .values()
                .filter(|candidate| candidate.ordinal == ordinal)
                .count()
                != 1
            || coordinator
                .records
                .values()
                .filter(|candidate| candidate.key == record.key)
                .count()
                != 1
            || coordinator
                .key_index
                .values()
                .filter(|candidate| **candidate == ordinal)
                .count()
                != 1
            || coordinator
                .owner_index
                .values()
                .filter(|owner| **owner == record.owner)
                .count()
                != 1
            || metadata.continuation != super::schema::DurableContinuation::None
            || !matches!(metadata.payload, DurablePayloadReference::BodyFrame(_))
        {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCoordinatorIndex)?;
        if self
            .entries
            .keys()
            .filter(|candidate| candidate.owner == record.owner)
            .count()
            != 1
        {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCoordinatorIndex);
        }
        let work = self.entries.get(&address).ok_or(
            ReadyLifecycleDecisionApplyAttestationErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let (carrier_matches, lineage, dispatch_key) = match &work.kind {
            ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) => (
                apply.matches_current_ready_record(address, digest, coordinator),
                LifecycleDecisionApplyLineageV1::Live,
                apply.dispatch_key,
            ),
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => (
                apply.matches_current_ready_record(address, digest, coordinator),
                LifecycleDecisionApplyLineageV1::Recovered,
                apply.dispatch_key,
            ),
            _ => return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::WrongWorkKind),
        };
        if !carrier_matches || dispatch_key.is_some() {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCarrier);
        }
        Ok(ReadyLifecycleDecisionApplyAttestationV1 {
            demand: ReadyLifecycleDecisionApplyDemandV1::BoundedIo,
            dispatch_key: LifecycleDecisionApplyDispatchKeyV1::new(
                coordinator.active_context,
                address,
                digest,
                lineage,
            ),
            lineage,
            _seal: ReadyLifecycleDecisionApplyAttestationSealV1,
        })
    }
    /// Project the exact Ready live Apply into an executor-only cleanup authority.
    ///
    /// Recovered carriers return `None` and retain their cold-start contract.
    /// No queue identity, lease, or worker reservation is created here.
    pub(super) fn prepare_ready_live_decision_apply_reconciliation(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<
        Option<LiveLifecycleDecisionApplyReconciliationAuthorityV1>,
        ReadyLifecycleDecisionApplyAttestationErrorV1,
    > {
        let attestation = self.attest_ready_lifecycle_decision_apply(coordinator, ordinal)?;
        let dispatch_key = attestation.dispatch_key();
        if dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Recovered {
            return Ok(None);
        }
        let address =
            ConcreteWorkAddress::new(dispatch_key.owner, dispatch_key.ordinal, dispatch_key.slot)
                .ok_or(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCarrier)?;
        let work = self.entries.get(&address).ok_or(
            ReadyLifecycleDecisionApplyAttestationErrorV1::Registry(RegistryError::Missing),
        )?;
        let ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) = &work.kind else {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::WrongWorkKind);
        };
        if work.digest != dispatch_key.digest
            || !apply.matches_current_ready_record(address, work.digest, coordinator)
            || apply.dispatch_key.is_some()
        {
            return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCarrier);
        }
        apply
            .project_reconciliation(dispatch_key)
            .map(Some)
            .ok_or(ReadyLifecycleDecisionApplyAttestationErrorV1::InvalidCarrier)
    }
    /// Project one exact claimed lifecycle Decision Apply into its dedicated worker task.
    ///
    /// The coordinator must still retain the sole active lease and the registry
    /// must still contain the unchanged closed carrier at its exact Effect/0
    /// address. Success consumes a registry-minted move-only dispatch identity;
    /// no generic adapter effect, receipt, pending binding, or candidate parts
    /// cross this boundary.
    pub(super) fn prepare_lifecycle_decision_apply_dispatch(
        &mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Result<
        PreparedLifecycleDecisionApplyDispatchV1<'_>,
        LifecycleDecisionApplyDispatchProjectionErrorV1,
    > {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class() != LifecycleWorkClass::Apply
            || lease.key().phase() != LifecyclePhase::Apply
            || lease.stage().kind() != LifecycleStageKind::ApplyDecision
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidLease);
        }
        let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
            return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidLease);
        };
        if slot.capacity_class() != Some(LifecycleWorkClass::Apply.capacity_class()) {
            return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidLease);
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidLease)?;
        let work = self.entries.get_mut(&address).ok_or(
            LifecycleDecisionApplyDispatchProjectionErrorV1::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let (carrier, task) = match &mut work.kind {
            ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) => {
                if !apply.matches_claimed_record(address, digest, coordinator, lease) {
                    return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidCarrier);
                }
                if apply.dispatch_key.is_some() {
                    return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::AlreadyDispatched);
                }
                let identity = LifecycleDecisionApplyDispatchIdentityV1::new(
                    coordinator.active_context,
                    address,
                    digest,
                    LifecycleDecisionApplyLineageV1::Live,
                );
                let task = apply
                    .project_task(identity)
                    .ok_or(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidCarrier)?;
                (PreparedLifecycleDecisionApplyCarrierV1::Live(apply), task)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                if !apply.matches_claimed_record(address, digest, coordinator, lease) {
                    return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidCarrier);
                }
                if apply.dispatch_key.is_some() {
                    return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::AlreadyDispatched);
                }
                let identity = LifecycleDecisionApplyDispatchIdentityV1::new(
                    coordinator.active_context,
                    address,
                    digest,
                    LifecycleDecisionApplyLineageV1::Recovered,
                );
                let task = apply
                    .carrier
                    .project_recovered_apply_task(identity, address)
                    .ok_or(LifecycleDecisionApplyDispatchProjectionErrorV1::InvalidCarrier)?;
                (
                    PreparedLifecycleDecisionApplyCarrierV1::Recovered(apply),
                    task,
                )
            }
            _ => return Err(LifecycleDecisionApplyDispatchProjectionErrorV1::WrongWorkKind),
        };
        let key = task.dispatch_key();
        Ok(PreparedLifecycleDecisionApplyDispatchV1 {
            carrier,
            task: Some(task),
            key,
        })
    }
}

include!("v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs");

include!("v2_lifecycle_work_registry_validate_completion_impl.rs");
include!("v2_lifecycle_work_registry_access_impl.rs");
include!("v2_lifecycle_work_registry_validate_recovery_execution_impl.rs");
