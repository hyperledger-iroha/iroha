//! Durable settlement gate for lifecycle turns.

use super::{
    CoordinatorFault, LifecycleCoordinator, LifecycleState, LifecycleWorkClass, TerminalOutcome,
    TurnLease, TurnOutcome, WaitSource, schema::DurablePayloadReference,
};

impl LifecycleCoordinator {
    /// Settle once; an invalid lease remains visible and fails closed.
    pub(crate) fn settle_turn(&mut self, lease: TurnLease, outcome: TurnOutcome) {
        self.settle_turn_inner(lease, outcome, None);
    }

    /// Settle a Certified-Serve terminal after a post-fsync payload receipt
    /// has been projected into its exact durable ledger reference.
    pub(super) fn settle_turn_with_durable_serve_payload(
        &mut self,
        lease: TurnLease,
        outcome: TurnOutcome,
        payload: DurablePayloadReference,
    ) {
        self.settle_turn_inner(lease, outcome, Some(payload));
    }

    fn settle_turn_inner(
        &mut self,
        lease: TurnLease,
        outcome: TurnOutcome,
        durable_serve_payload: Option<DurablePayloadReference>,
    ) {
        if self.ledger_store.is_none() {
            self.reduce_settle_turn(lease, outcome, durable_serve_payload);
            return;
        }
        let durable_transition =
            matches!(outcome, TurnOutcome::Advanced | TurnOutcome::Terminal(_));
        let mut next = self.stage_durable_transaction();
        next.reduce_settle_turn(lease, outcome, durable_serve_payload);
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
        durable_serve_payload: Option<DurablePayloadReference>,
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

        match (lease.work_class, outcome, durable_serve_payload) {
            (
                LifecycleWorkClass::CertifiedServe,
                TurnOutcome::Terminal(terminal),
                Some(payload),
            ) => {
                let Some(metadata) = self.durable_records.get_mut(&lease.ordinal) else {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                };
                if !metadata.payload.same_admission_material(payload)
                    || !payload.matches_terminal(LifecycleWorkClass::CertifiedServe, Some(terminal))
                {
                    return self.latch_settlement_fault(CoordinatorFault::InvalidTerminalOutcome);
                }
                metadata.payload = payload;
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
}
