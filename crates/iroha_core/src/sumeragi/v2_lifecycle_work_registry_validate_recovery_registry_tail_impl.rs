// Lifecycle Decision Apply terminal and durable Validate registry transitions.

impl ConcreteLifecycleWorkRegistry {
    /// Bind one guarded Applied worker result to the exact in-flight carrier.
    pub(super) fn prepare_lifecycle_decision_apply_terminal_transition(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        completion: &crate::sumeragi::v2_apply::LifecycleDecisionApplyCompletionV1,
    ) -> Option<(
        PreparedLifecycleDecisionApplyTerminalTransitionV1,
        crate::sumeragi::v2::LifecycleDecisionApplyAdapterCompletionAuthorityV1,
    )> {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class() != LifecycleWorkClass::Apply
            || lease.key().phase() != LifecyclePhase::Apply
            || lease.stage().kind() != LifecycleStageKind::ApplyDecision
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return None;
        }
        let (&slot, &digest) = lease.physical_slots().first_key_value()?;
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)?;
        let work = self.entries.get(&address)?;
        let dispatch_key = completion.dispatch_key();
        if work.digest != digest || !work.validates_at(address) {
            return None;
        }
        let (lineage, authority) = match &work.kind {
            ConcreteLifecycleWorkKind::DurableLiveWalApply(apply)
                if apply.matches_claimed_record(address, digest, coordinator, lease)
                    && apply.dispatch_key == Some(dispatch_key)
                    && dispatch_key.matches(
                        coordinator.active_context,
                        address,
                        digest,
                        LifecycleDecisionApplyLineageV1::Live,
                    ) =>
            {
                (
                    LifecycleDecisionApplyLineageV1::Live,
                    apply.project_completion(
                        LifecycleDecisionApplyCompletionProjectionPermitV1::new(),
                        completion,
                    )?,
                )
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)
                if apply.matches_claimed_record(address, digest, coordinator, lease)
                    && apply.dispatch_key == Some(dispatch_key)
                    && dispatch_key.matches(
                        coordinator.active_context,
                        address,
                        digest,
                        LifecycleDecisionApplyLineageV1::Recovered,
                    ) =>
            {
                (
                    LifecycleDecisionApplyLineageV1::Recovered,
                    apply.carrier.project_recovered_apply_completion(
                        LifecycleDecisionApplyCompletionProjectionPermitV1::new(),
                        address,
                        completion,
                    )?,
                )
            }
            _ => return None,
        };
        Some((
            PreparedLifecycleDecisionApplyTerminalTransitionV1 {
                address,
                digest,
                dispatch_key,
                lineage,
                _linearity: LifecycleDecisionApplyTerminalTransitionLinearityV1,
            },
            authority,
        ))
    }
    /// Publish one exact lifecycle Apply terminal around LedgerV1 fsync.
    ///
    /// Every logical and physical check occurs before `publish`. Success is
    /// followed only by the infallible removal of the prevalidated carrier.
    pub(super) fn publish_lifecycle_decision_apply_terminal_transition<T, E>(
        &mut self,
        prepared: PreparedLifecycleDecisionApplyTerminalTransitionV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, LifecycleDecisionApplyTerminalPublicationErrorV1<E>> {
        let Some(work) = self.entries.get(&prepared.address) else {
            return Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Preflight(
                prepared,
            ));
        };
        let carrier_matches = match (&work.kind, prepared.lineage) {
            (
                ConcreteLifecycleWorkKind::DurableLiveWalApply(apply),
                LifecycleDecisionApplyLineageV1::Live,
            ) => {
                apply.dispatch_key == Some(prepared.dispatch_key)
                    && apply.matches_claimed_record(
                        prepared.address,
                        prepared.digest,
                        current,
                        lease,
                    )
            }
            (
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply),
                LifecycleDecisionApplyLineageV1::Recovered,
            ) => {
                apply.dispatch_key == Some(prepared.dispatch_key)
                    && apply.matches_claimed_record(
                        prepared.address,
                        prepared.digest,
                        current,
                        lease,
                    )
            }
            _ => false,
        };
        let exact_current = work.digest == prepared.digest
            && work.validates_at(prepared.address)
            && prepared.dispatch_key.matches(
                current.active_context,
                prepared.address,
                prepared.digest,
                prepared.lineage,
            )
            && carrier_matches;
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
            return Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Preflight(
                prepared,
            ));
        }
        match publish() {
            Ok(value) => {
                drop(
                    self.entries
                        .remove(&prepared.address)
                        .expect("lifecycle Apply preflight retained the exact carrier"),
                );
                Ok(value)
            }
            Err(error) => {
                Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Publication(error, prepared))
            }
        }
    }
    /// Prepare execution of one exact Ready durable Validate completion.
    ///
    /// The claimed lease must retain the original independent Validate
    /// lifecycle identity while its sole physical slot names the installed
    /// outcome-bound replacement digest. The retained incumbent is replayed
    /// through authenticated projection, and the complete closed outcome is
    /// revalidated before an exclusive, drop-inert registry borrow is issued.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_ready_durable_validate_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedReadyDurableValidateExecution<'_>, ReadyDurableValidateExecutionError> {
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
            return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
        }
        let address = self
            .validated_lease_address(lease, slot)
            .map_err(ReadyDurableValidateExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated Ready Validate completion address remains present");
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &work.kind else {
            return Err(ReadyDurableValidateExecutionError::WrongWorkKind);
        };
        let AdapterEffect::ValidateBody {
            tag: _,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        };
        let Some(candidate_statement) = completion.incumbent.pending.candidate_statement() else {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        };
        if !completion.validates(work.digest)
            || completion.address != address
            || completion.incumbent.address != address
            || candidate_statement.context_id() != round.context_id
            || candidate_statement.proposal_round() != *round
            || candidate_statement.subject() != Some(*subject)
            || completion.incumbent.durable_receipt.context_id() != round.context_id
            || completion.incumbent.durable_receipt.round() != *round
            || completion.incumbent.durable_receipt.subject() != *subject
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || completion.outcome.durable_body() != &completion.incumbent.durable_receipt
        {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        }
        let outcome_kind = match (
            completion.outcome.validated_receipt(),
            completion.outcome.rejection_identity(),
            completion.outcome.missing_merge_sidecar(),
        ) {
            (Some(receipt), None, None)
                if receipt.durable() == &completion.incumbent.durable_receipt
                    && receipt.durable().manifest_hash()
                        == completion.incumbent.expected_manifest_hash
                    && validate_validated_receipt_authority(&completion.incumbent, receipt)
                        .is_ok() =>
            {
                ReadyDurableValidateOutcomeKind::Validated
            }
            (None, Some(BodyValidationRejectionIdentity::Rejected), None) => {
                ReadyDurableValidateOutcomeKind::Rejected
            }
            _ => return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape),
        };
        let expected_reservation = match outcome_kind {
            ReadyDurableValidateOutcomeKind::Validated => None,
            ReadyDurableValidateOutcomeKind::Rejected => Some(CapacityClass::Consensus),
        };
        if lease
            .output_reservation()
            .map(|reservation| reservation.class())
            != expected_reservation
        {
            return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
        }
        let candidate = completion
            .incumbent
            .project_candidate(verified)
            .map_err(ReadyDurableValidateExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .ok_or(ReadyDurableValidateExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| ReadyDurableValidateExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let incumbent_slots = BTreeMap::from([(slot, completion.incumbent_digest)]);
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Validate
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots != incumbent_slots
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(ReadyDurableValidateExecutionError::InvalidProjection);
        }
        let validated_catalog_authority = match outcome_kind {
            ReadyDurableValidateOutcomeKind::Validated => {
                Some(ReadyValidatedExecutorCatalogAuthorityV1 {
                    validated: completion
                        .outcome
                        .validated_receipt()
                        .expect("validated outcome retains one exact receipt")
                        .clone(),
                })
            }
            ReadyDurableValidateOutcomeKind::Rejected => None,
        };
        Ok(PreparedReadyDurableValidateExecution {
            registry: self,
            address,
            outcome_kind,
            lease: lease.clone(),
            validated_catalog_authority,
            authenticated_manifest: None,
        })
    }
    /// Reattach one executed Validate outcome only if its original closed row
    /// remains byte-for-byte authoritative at the exact address and digest.
    ///
    /// Failure returns the complete move-only execution token. Success only
    /// establishes a new exclusive borrow; neither path changes the registry.
    // The sole outer consumer joins this reattachment with typed same-address
    // carrier installation and the coordinator Ready replacement. Waiting,
    // Ready, and physical carriers are excluded from the lifecycle ledger, so
    // that volatile cut deliberately performs no ledger rewrite.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn reattach_durable_validate_execution(
        &mut self,
        executed: ExecutedDurableValidateExecution,
    ) -> Result<
        PreparedDurableValidateCompletion<'_>,
        (
            DurableValidateExecutionError,
            ExecutedDurableValidateExecution,
        ),
    > {
        let request = &executed.request;
        let exact = (|| {
            if ConcreteWorkAddress::new(
                request.address.owner,
                request.address.ordinal,
                request.address.slot,
            ) != Some(request.address)
            {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::InvalidAddress,
                ));
            }
            let work = self.entries.get(&request.address).ok_or(
                DurableValidateExecutionError::Registry(RegistryError::Missing),
            )?;
            if !work.validates_at(request.address) {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork,
                ));
            }
            if request.address.owner.causal_root() != work.causal_root() {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::CausalOwnerMismatch,
                ));
            }
            if work.digest != request.incumbent_digest {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::DigestMismatch,
                ));
            }
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                return Err(DurableValidateExecutionError::WrongWorkKind);
            };
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                return Err(DurableValidateExecutionError::InvalidValidateShape);
            };
            if validate.address != request.address
                || *tag != request.tag
                || *round != request.round
                || *subject != request.subject
                || validate.durable_receipt != request.durable_receipt
                || validate.expected_manifest_hash != request.expected_manifest_hash
                || !validate
                    .pending
                    .exactly_binds_adapter_effect(&validate.effect)
                || validate.pending.causal_lifecycle_key() != &request.causal_lifecycle_key
                || validate.pending.candidate_statement() != request.candidate_statement
                || request.lifecycle_key.phase() != LifecyclePhase::Validate
                || request.lifecycle_stage.kind() != LifecycleStageKind::ValidateBody
                || request.lifecycle_stage.predecessor_scope() != PredecessorScope::Independent
            {
                return Err(DurableValidateExecutionError::InvalidValidateShape);
            }
            if executed.outcome.durable_body() != &request.durable_receipt {
                return Err(DurableValidateExecutionError::InvalidValidationReceipt);
            }
            if let Some(receipt) = executed.outcome.validated_receipt() {
                validate_validated_receipt_authority(validate, receipt)?;
            }
            Ok(())
        })();
        if let Err(error) = exact {
            return Err((error, executed));
        }
        Ok(PreparedDurableValidateCompletion {
            _registry: self,
            executed,
        })
    }
}
