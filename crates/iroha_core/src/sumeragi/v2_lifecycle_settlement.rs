//! Durable settlement gate for lifecycle turns.

use super::{
    CoordinatorFault, LifecycleCoordinator, LifecycleStageKind, LifecycleState, LifecycleWorkClass,
    TerminalOutcome, TurnLease, TurnOutcome, WaitSource,
    replay_authority::CertifiedServeTerminalReplayAuthorityPairV1, serve_and_producer_keys_match,
};

impl LifecycleCoordinator {
    /// Settle once; an invalid lease remains visible and fails closed.
    pub(crate) fn settle_turn(&mut self, lease: TurnLease, outcome: TurnOutcome) {
        self.settle_turn_inner(lease, outcome, None);
    }

    /// Settle a Certified-Serve terminal from one sealed post-fsync replay
    /// family replacement.
    pub(super) fn settle_turn_with_durable_serve_terminal(
        &mut self,
        lease: TurnLease,
        terminal: CertifiedServeTerminalReplayAuthorityPairV1,
    ) {
        let outcome = TurnOutcome::Terminal(terminal.terminal_outcome());
        self.settle_turn_inner(lease, outcome, Some(terminal));
    }

    fn settle_turn_inner(
        &mut self,
        lease: TurnLease,
        outcome: TurnOutcome,
        durable_serve_terminal: Option<CertifiedServeTerminalReplayAuthorityPairV1>,
    ) {
        if self.ledger_store.is_none() {
            self.reduce_settle_turn(lease, outcome, durable_serve_terminal);
            return;
        }
        let durable_transition =
            matches!(outcome, TurnOutcome::Advanced | TurnOutcome::Terminal(_));
        let mut next = self.stage_durable_transaction();
        next.reduce_settle_turn(lease, outcome, durable_serve_terminal);
        if next.fault.is_none() && durable_transition && next.persist_durable_projection().is_err()
        {
            return self.latch_settlement_fault(CoordinatorFault::DurabilityFailure);
        }
        *self = next;
    }

    /// Apply one already-gated settlement to the pure lifecycle state.
    pub(super) fn reduce_settle_turn(
        &mut self,
        lease: TurnLease,
        outcome: TurnOutcome,
        durable_serve_terminal: Option<CertifiedServeTerminalReplayAuthorityPairV1>,
    ) {
        self.reduce_settle_turn_inner(lease, outcome, durable_serve_terminal, false);
    }

    /// Terminalize one body parent on a staged coordinator before attaching
    /// its typed continuation and publishing the complete durable projection.
    pub(super) fn reduce_settle_body_parent_for_continuation(&mut self, lease: TurnLease) {
        self.reduce_settle_turn_inner(lease, TurnOutcome::Advanced, None, true);
    }

    fn reduce_settle_turn_inner(
        &mut self,
        lease: TurnLease,
        outcome: TurnOutcome,
        durable_serve_terminal: Option<CertifiedServeTerminalReplayAuthorityPairV1>,
        allow_unlinked_body_advanced: bool,
    ) {
        if self.fault.is_some() {
            return;
        }
        if self.active_lease.as_ref() != Some(&lease)
            || !self.records.get(&lease.ordinal).is_some_and(|record| {
                record.owner == lease.owner && record.state == LifecycleState::Claimed(lease.id)
            })
        {
            return self.latch_settlement_fault(CoordinatorFault::StaleLease);
        }
        if lease.output_reservation().is_some() {
            // TODO: The sole rejected-Validate transaction must either consume
            // this reservation into its exact report child or release it on a
            // typed non-report outcome. Generic settlement cannot distinguish
            // those cuts and therefore retains the active lease fail-closed.
            return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
        }
        if matches!(
            lease.work_class,
            LifecycleWorkClass::Fetch | LifecycleWorkClass::Store | LifecycleWorkClass::Validate
        ) && matches!(
            outcome,
            TurnOutcome::Advanced | TurnOutcome::Terminal(TerminalOutcome::Advanced)
        ) && !allow_unlinked_body_advanced
        {
            return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
        }

        match (lease.work_class, outcome, durable_serve_terminal) {
            (LifecycleWorkClass::CertifiedServe, TurnOutcome::Terminal(terminal), Some(replay)) => {
                if replay.terminal_outcome() != terminal {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                }
                let Some(producer_ordinal) = self.producer_debts.get(&lease.ordinal).copied()
                else {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                };
                let Some(serve_record) = self.records.get(&lease.ordinal) else {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                };
                let Some(serve_metadata) = self.durable_records.get(&lease.ordinal) else {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                };
                let Some(producer_record) = self.records.get(&producer_ordinal) else {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                };
                let Some(producer_metadata) = self.durable_records.get(&producer_ordinal) else {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                };
                if !replay.exactly_advances_pending_records(
                    self.active_context,
                    serve_record,
                    serve_metadata,
                    producer_record,
                    producer_metadata,
                ) {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                }
                let (payload, pair_outcome, serve_replay, producer_replay) =
                    replay.consume_terminal_rebind();
                if pair_outcome != terminal {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                }
                let serve_metadata = self
                    .durable_records
                    .get_mut(&lease.ordinal)
                    .expect("terminal preflight retained Serve metadata");
                serve_metadata.payload = payload;
                serve_metadata.replay_authority = serve_replay;
                self.durable_records
                    .get_mut(&producer_ordinal)
                    .expect("terminal preflight retained ProducerTurn metadata")
                    .replay_authority = producer_replay;
            }
            (
                LifecycleWorkClass::CertifiedServe,
                TurnOutcome::Advanced | TurnOutcome::Terminal(_),
                _,
            )
            | (_, _, Some(_)) => {
                return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
            }
            (_, _, None) => {}
        }

        let result = match outcome {
            TurnOutcome::Advanced => self.finish_terminal(lease.ordinal, TerminalOutcome::Advanced),
            TurnOutcome::Terminal(outcome) => self.finish_terminal(lease.ordinal, outcome),
            TurnOutcome::Blocked(wait) => {
                if !matches!(
                    wait.source,
                    WaitSource::External(_) | WaitSource::Recovery(_)
                ) {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidReadyEvent);
                }
                let known = self
                    .observed_generation
                    .get(&wait.source)
                    .copied()
                    .unwrap_or(0);
                if wait.observed_generation == u64::MAX || wait.observed_generation < known {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidReadyEvent);
                }
                self.advance_observed_generation(wait.source, wait.observed_generation);
                self.records
                    .get_mut(&lease.ordinal)
                    .expect("active lease retains its record")
                    .state = LifecycleState::Waiting(wait);
                Ok(())
            }
            TurnOutcome::Replenished(slot) => self.finish_replenishment(lease.ordinal, slot),
        };
        if let Err(fault) = result {
            return self.latch_settlement_fault(fault);
        }
        self.active_lease = None;
    }

    fn latch_settlement_fault(&mut self, fault: CoordinatorFault) {
        self.fault = Some(fault);
    }

    /// Finish one already-preflighted terminal transition and its paired
    /// Certified-Serve/ProducerTurn bookkeeping.
    pub(super) fn finish_terminal(
        &mut self,
        ordinal: u128,
        outcome: TerminalOutcome,
    ) -> Result<(), CoordinatorFault> {
        let record = self
            .records
            .get(&ordinal)
            .ok_or(CoordinatorFault::StaleLease)?;
        let (work_class, key, owner, stage) =
            (record.work_class, record.key, record.owner, record.stage);
        let metadata = self
            .durable_records
            .get(&ordinal)
            .ok_or(CoordinatorFault::InvalidTerminalOutcome)?;
        let terminal_payload = metadata
            .payload
            .terminalized(outcome)
            .filter(|payload| payload.matches_terminal(work_class, Some(outcome)))
            .ok_or(CoordinatorFault::InvalidTerminalOutcome)?;
        let terminal_replay = metadata
            .terminalized_replay_authority(
                self.active_context,
                key,
                work_class,
                stage,
                terminal_payload,
            )
            .ok_or(CoordinatorFault::InvalidTerminalOutcome)?;
        let paired_ordinal = match work_class {
            LifecycleWorkClass::CertifiedServe => {
                let producer = self
                    .producer_debts
                    .get(&ordinal)
                    .copied()
                    .ok_or(CoordinatorFault::CapacityAccounting)?;
                let paired = self
                    .records
                    .get(&producer)
                    .ok_or(CoordinatorFault::CapacityAccounting)?;
                (ordinal.checked_add(1) == Some(producer)
                    && paired.work_class == LifecycleWorkClass::ProducerTurn
                    && paired.stage.kind == LifecycleStageKind::ProducerTurn
                    && serve_and_producer_keys_match(key, paired.key)
                    && paired.owner == owner
                    && !matches!(paired.state, LifecycleState::Terminal(_)))
                .then_some(producer)
                .ok_or(CoordinatorFault::CapacityAccounting)?
            }
            LifecycleWorkClass::ProducerTurn => {
                let serve = self
                    .producer_debts
                    .iter()
                    .find_map(|(serve, producer)| (*producer == ordinal).then_some(*serve))
                    .ok_or(CoordinatorFault::CapacityAccounting)?;
                let paired = self
                    .records
                    .get(&serve)
                    .ok_or(CoordinatorFault::CapacityAccounting)?;
                (serve.checked_add(1) == Some(ordinal)
                    && paired.work_class == LifecycleWorkClass::CertifiedServe
                    && paired.stage.kind == LifecycleStageKind::CertifiedServe
                    && serve_and_producer_keys_match(paired.key, key)
                    && paired.owner == owner)
                    .then_some(serve)
                    .ok_or(CoordinatorFault::CapacityAccounting)?
            }
            _ => ordinal,
        };
        if work_class == LifecycleWorkClass::CertifiedServe
            && self
                .durable_records
                .get(&paired_ordinal)
                .is_none_or(|producer_metadata| {
                    !terminal_replay.same_persisted_family(&producer_metadata.replay_authority)
                })
        {
            return Err(CoordinatorFault::InvalidTerminalOutcome);
        }
        self.ready_index.remove(&ordinal);
        self.records
            .get_mut(&ordinal)
            .expect("terminalized record remains present")
            .state = LifecycleState::Terminal(outcome);
        let metadata = self
            .durable_records
            .get_mut(&ordinal)
            .expect("durable metadata is bijective with records");
        metadata.payload = terminal_payload;
        metadata.replay_authority = terminal_replay;
        self.release_capacity(work_class.capacity_class())?;

        if work_class == LifecycleWorkClass::CertifiedServe {
            let producer = paired_ordinal;
            if outcome == TerminalOutcome::Cancelled {
                let producer_record = self
                    .records
                    .get(&producer)
                    .ok_or(CoordinatorFault::CapacityAccounting)?;
                let producer_class = producer_record.work_class.capacity_class();
                self.records
                    .get_mut(&producer)
                    .expect("producer debt names a retained record")
                    .state = LifecycleState::Terminal(TerminalOutcome::Cancelled);
                self.ready_index.remove(&producer);
                self.release_capacity(producer_class)?;
                self.producer_debts.remove(&ordinal);
            } else {
                self.make_ready(producer);
            }
        } else if work_class == LifecycleWorkClass::ProducerTurn {
            self.producer_debts.remove(&paired_ordinal);
        } else if work_class == LifecycleWorkClass::EnterView
            && outcome == TerminalOutcome::Advanced
        {
            self.supersede_lower_enter_views(key)?;
        }
        Ok(())
    }
}
