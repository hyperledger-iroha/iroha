/// Process-local provenance retained by one fsynced live-WAL Sign row.
///
/// A Validate successor keeps the detached validated parent until the Sign is
/// durably advanced. A local Proposal keeps its consumed local-body lineage
/// beside the standalone WAL owner. Neither variant can be reconstructed from
/// volatile parts; cold open authenticates the corresponding durable WAL and
/// LedgerV1 rows instead.
enum DurableLiveWalSignOriginV1 {
    Validate {
        parent_address: ConcreteWorkAddress,
        parent: Box<ConcreteLifecycleWork>,
    },
    LocalProposal,
}

/// Closed process-local carrier for one exact post-fsync WAL Sign.
struct DurableLiveWalSignWork {
    admission: PreparedLiveWalAdmissionV1,
    candidate: CandidateAdmission,
    origin: DurableLiveWalSignOriginV1,
    address: ConcreteWorkAddress,
    dispatch_key: Option<RecoveredLifecycleSignDispatchKeyV1>,
}

impl fmt::Debug for DurableLiveWalSignWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableLiveWalSignWork")
            .field("address", &self.address)
            .field("dispatched", &self.dispatch_key.is_some())
            .finish_non_exhaustive()
    }
}

impl DurableLiveWalSignWork {
    fn class(&self) -> Option<RecoveredLifecycleSignClassV1> {
        match &self.admission.bound.effect {
            AdapterEffect::Sign {
                request: SignRequest::Vote(_),
                ..
            } => Some(RecoveredLifecycleSignClassV1::PhaseVote),
            AdapterEffect::Sign {
                request: SignRequest::Proposal(_),
                ..
            } => Some(RecoveredLifecycleSignClassV1::ControlProposal),
            AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            } => Some(RecoveredLifecycleSignClassV1::ControlTimeout),
            _ => None,
        }
    }

    fn validates_at(&self, address: ConcreteWorkAddress, digest: LifecycleDigest) -> bool {
        let active_context = LifecycleContext::new(
            self.candidate.key.context(),
            self.candidate.key.round().height(),
        );
        let geometry_is_exact = self.candidate.physical_geometry.normalized().is_ok_and(
            |(physical, universe, consumed)| {
                physical == BTreeMap::from([(address.slot, digest)])
                    && universe == std::collections::BTreeSet::from([address.slot])
                    && consumed == universe
            },
        );
        let origin_is_exact = match &self.origin {
            DurableLiveWalSignOriginV1::Validate {
                parent_address,
                parent,
            } => {
                self.class() == Some(RecoveredLifecycleSignClassV1::PhaseVote)
                    && *parent_address != address
                    && parent_address.owner == address.owner
                    && parent.validates_at(*parent_address)
                    && matches!(
                        &parent.kind,
                        ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                            if completion.outcome.validated_receipt().is_some()
                    )
                    && matches!(&self.admission.companion, PreparedLiveWalCompanionV1::None)
            }
            DurableLiveWalSignOriginV1::LocalProposal => {
                self.class() == Some(RecoveredLifecycleSignClassV1::ControlProposal)
                    && matches!(
                        &self.admission.companion,
                        PreparedLiveWalCompanionV1::LocalProposal(retained)
                            if retained.exactly_matches_live_wal_sign_effect(
                                &self.admission.bound.effect
                            )
                    )
            }
        };
        self.address == address
            && address.owner.causal_root() == self.candidate.causal_root
            && address.slot
                == PhysicalSlotId::for_capacity(self.candidate.work_class.capacity_class(), 0)
            && digest == digest_from_hash(self.admission.bound.pending.exact_effect_identity())
            && self
                .admission
                .exactly_authorizes_candidate(active_context, &self.candidate)
            && origin_is_exact
            && geometry_is_exact
    }

    fn predecessor_is_exact_in_coordinator(
        &self,
        address: ConcreteWorkAddress,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        match &self.origin {
            DurableLiveWalSignOriginV1::LocalProposal => true,
            DurableLiveWalSignOriginV1::Validate {
                parent_address,
                parent: predecessor,
            } => {
                let edge = match self.candidate.key.phase() {
                    LifecyclePhase::Prepare => {
                        super::schema::DurableContinuationEdge::ValidateToSignPrepare
                    }
                    LifecyclePhase::Commit => {
                        super::schema::DurableContinuationEdge::ValidateToSignCommit
                    }
                    _ => return false,
                };
                coordinator
                    .records
                    .get(&parent_address.ordinal)
                    .is_some_and(|record| {
                        record.owner == parent_address.owner
                            && record.ordinal == parent_address.ordinal
                            && record.work_class == LifecycleWorkClass::Validate
                            && record.state
                                == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
                            && record.physical_slots
                                == BTreeMap::from([(parent_address.slot, predecessor.digest())])
                    })
                    && coordinator
                        .durable_records
                        .get(&parent_address.ordinal)
                        .is_some_and(|metadata| {
                            metadata.continuation
                                == super::schema::DurableContinuation::successor(
                                    edge,
                                    address.ordinal,
                                )
                        })
            }
        }
    }

    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at(address, digest)
            && self.predecessor_is_exact_in_coordinator(address, coordinator)
            && coordinator.fault.is_none()
            && coordinator.active_context
                == LifecycleContext::new(
                    self.candidate.key.context(),
                    self.candidate.key.round().height(),
                )
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == self.candidate.work_class
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Ready
            && record.physical_slots == BTreeMap::from([(address.slot, digest)])
            && record.episode.slot_universe == std::collections::BTreeSet::from([address.slot])
            && record.episode.consumed_slots == record.episode.slot_universe
            && metadata.matches_admission(&self.candidate)
            && metadata.continuation == super::schema::DurableContinuation::None
            && coordinator.key_index.get(&record.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && coordinator.ready_index.contains(&address.ordinal)
    }

    fn matches_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        self.matches_current_record_state(address, digest, coordinator, lease)
    }

    fn matches_current_record_state(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at(address, digest)
            && self.predecessor_is_exact_in_coordinator(address, coordinator)
            && coordinator.fault.is_none()
            && coordinator.active_context
                == LifecycleContext::new(
                    self.candidate.key.context(),
                    self.candidate.key.round().height(),
                )
            && coordinator.active_lease.as_ref() == Some(lease)
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == self.candidate.work_class
            && record.stage == self.candidate.stage
            && record.state == super::LifecycleState::Claimed(lease.id())
            && lease.key() == record.key
            && lease.owner() == record.owner
            && lease.ordinal() == record.ordinal
            && lease.work_class() == record.work_class
            && lease.stage() == record.stage
            && lease.physical_slots() == &record.physical_slots
            && record.physical_slots == BTreeMap::from([(address.slot, digest)])
            && record.episode.slot_universe == std::collections::BTreeSet::from([address.slot])
            && record.episode.consumed_slots == record.episode.slot_universe
            && metadata.matches_admission(&self.candidate)
            && metadata.continuation == super::schema::DurableContinuation::None
            && coordinator.key_index.get(&record.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && !coordinator.ready_index.contains(&address.ordinal)
    }

    fn project_task(
        &self,
        identity: RecoveredLifecycleSignDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1> {
        let AdapterEffect::Sign { tag, request } = &self.admission.bound.effect else {
            return None;
        };
        crate::sumeragi::v2_worker::RecoveredLifecycleSignTaskV1::from_registry_projection(
            identity,
            *tag,
            request.clone(),
        )
    }

    fn validates_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        broadcast: &RecoveredLifecycleSignedBroadcastProjectionV1,
    ) -> bool {
        super::wal_recovery::live_wal_sign_matches_signed_broadcast(
            &self.admission.bound.effect,
            &self.admission.bound.pending,
            verified,
            broadcast,
        )
    }

    fn project_authenticated_signed_broadcast(
        &self,
        verified: &VerifiedHeightContext,
        authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastProjectionAuthorityV1,
    ) -> Option<(
        RecoveredLifecycleSignDispatchKeyV1,
        RecoveredLifecycleSignedBroadcastProjectionV1,
    )> {
        super::wal_recovery::project_live_wal_sign_signed_broadcast(
            &self.admission.bound.effect,
            &self.admission.bound.pending,
            verified,
            authority,
        )
    }

    fn project_authenticated_signed_broadcast_and_sign(
        &self,
        verified: &VerifiedHeightContext,
        authority: crate::sumeragi::v2::RecoveredLifecycleSignBroadcastAndSignAuthorityV1,
    ) -> Option<(
        RecoveredLifecycleSignDispatchKeyV1,
        RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    )> {
        super::wal_recovery::project_live_wal_sign_signed_broadcast_and_sign(
            &self.admission.bound.effect,
            &self.admission.bound.pending,
            verified,
            authority,
        )
    }

    fn exactly_matches_advanced_record(
        &self,
        context: LifecycleContext,
        record: &super::ledger::LifecycleLedgerRecordV1,
        child_ordinal: u128,
    ) -> bool {
        let edge = match self.candidate.stage.kind() {
            LifecycleStageKind::SignProposal => {
                super::schema::DurableContinuationEdge::SignProposalToBroadcast
            }
            LifecycleStageKind::SignPrepareVote => {
                super::schema::DurableContinuationEdge::SignPrepareToBroadcast
            }
            LifecycleStageKind::SignCommitVote => {
                super::schema::DurableContinuationEdge::SignCommitToBroadcast
            }
            LifecycleStageKind::SignTimeoutVote => {
                super::schema::DurableContinuationEdge::SignTimeoutToBroadcast
            }
            _ => return false,
        };
        self.validates_at(self.address, self.digest())
            && self.candidate.key.context() == context.id()
            && self.candidate.key.round().height() == context.height()
            && record.key() == Some(self.candidate.key)
            && record.owner() == self.address.owner
            && record.ordinal() == self.address.ordinal
            && record.work_class() == Some(self.candidate.work_class)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(self.candidate.payload)
            && record.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    edge,
                    child_ordinal,
                ))
            && record.replay_matches_candidate(&self.candidate)
    }

    fn exactly_matches_fresh_record(
        &self,
        context: LifecycleContext,
        record: &super::ledger::LifecycleLedgerRecordV1,
    ) -> bool {
        self.validates_at(self.address, self.digest())
            && self.candidate.key.context() == context.id()
            && self.candidate.key.round().height() == context.height()
            && record.key() == Some(self.candidate.key)
            && record.owner() == self.address.owner
            && record.ordinal() == self.address.ordinal
            && record.work_class() == Some(self.candidate.work_class)
            && record.stage() == Some(self.candidate.stage)
            && record.terminal() == Some(None)
            && record.reconstruction_source() == self.candidate.reconstruction_source
            && record.durable_payload() == Some(self.candidate.payload)
            && record.continuation() == Some(super::schema::DurableContinuation::None)
            && record.replay_matches_candidate(&self.candidate)
    }

    fn predecessor_is_exact_in_ledger(
        &self,
        ledger: &super::ledger::LifecycleLedgerV1,
        broadcast_ordinal: Option<u128>,
    ) -> bool {
        // The Broadcast caller separately authenticates the Sign and its exact
        // continuation target, so count only that closed lineage interval. A
        // later terminal retry may deliberately reuse the same causal owner and
        // must not retroactively invalidate the older durable Sign-to-Broadcast
        // edge. The fresh-Sign caller supplies no bound and retains the stricter
        // owner-tail census used before a Broadcast child exists.
        if broadcast_ordinal.is_some_and(|ordinal| ordinal < self.address.ordinal) {
            return false;
        }
        match &self.origin {
            DurableLiveWalSignOriginV1::LocalProposal => {
                ledger
                    .records()
                    .iter()
                    .filter(|record| {
                        record.owner() == self.address.owner
                            && record.ordinal() >= self.address.ordinal
                            && broadcast_ordinal
                                .is_none_or(|ordinal| record.ordinal() <= ordinal)
                    })
                    .count()
                    == if broadcast_ordinal.is_some() { 2 } else { 1 }
            }
            DurableLiveWalSignOriginV1::Validate {
                parent_address,
                parent,
            } => {
                let Some(parent_record) = ledger
                    .records()
                    .iter()
                    .find(|record| record.ordinal() == parent_address.ordinal)
                else {
                    return false;
                };
                let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &parent.kind
                else {
                    return false;
                };
                let (Some(parent_key), Some(parent_stage), Some(parent_payload)) = (
                    parent_record.key(),
                    parent_record.stage(),
                    parent_record.durable_payload(),
                ) else {
                    return false;
                };
                let edge = match self.candidate.key.phase() {
                    LifecyclePhase::Prepare => {
                        super::schema::DurableContinuationEdge::ValidateToSignPrepare
                    }
                    LifecyclePhase::Commit => {
                        super::schema::DurableContinuationEdge::ValidateToSignCommit
                    }
                    _ => return false,
                };
                parent.validates_at(*parent_address)
                    && completion.validates(parent.digest)
                    && parent_record.owner() == parent_address.owner
                    && parent_record.ordinal() == parent_address.ordinal
                    && parent_record.work_class() == Some(LifecycleWorkClass::Validate)
                    && parent_stage.kind() == LifecycleStageKind::ValidateBody
                    && parent_record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
                    && parent_record.continuation()
                        == Some(super::schema::DurableContinuation::successor(
                            edge,
                            self.address.ordinal,
                        ))
                    && projection::recovered_validate_no_successor_ledger_identity_is_authenticated(
                        ledger.context(),
                        parent_key,
                        parent_address.owner.causal_root(),
                        parent_record.reconstruction_source(),
                        parent_stage,
                        parent_payload,
                        &completion.outcome,
                    )
                    && ledger
                        .records()
                        .iter()
                        .filter(|record| {
                            record.owner() == self.address.owner
                                && record.ordinal() >= parent_address.ordinal
                                && broadcast_ordinal
                                    .is_none_or(|ordinal| record.ordinal() <= ordinal)
                        })
                        .count()
                        == if broadcast_ordinal.is_some() { 3 } else { 2 }
            }
        }
    }

    fn validates_in_ledger(&self, ledger: &super::ledger::LifecycleLedgerV1) -> bool {
        let Some(sign_record) = ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == self.address.ordinal)
        else {
            return false;
        };
        self.exactly_matches_fresh_record(ledger.context(), sign_record)
            && self.predecessor_is_exact_in_ledger(ledger, None)
    }

    fn digest(&self) -> LifecycleDigest {
        digest_from_hash(self.admission.bound.pending.exact_effect_identity())
    }

    fn causal_root(&self) -> super::CausalRoot {
        self.candidate.causal_root
    }
}

impl PreparedLiveWalAdmissionV1 {
    fn into_live_sign_work(
        self,
        candidate: CandidateAdmission,
        origin: DurableLiveWalSignOriginV1,
        address: ConcreteWorkAddress,
    ) -> Result<ConcreteLifecycleWork, (Self, CandidateAdmission, DurableLiveWalSignOriginV1)> {
        let digest = digest_from_hash(self.bound.pending.exact_effect_identity());
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableLiveWalSign(DurableLiveWalSignWork {
                admission: self,
                candidate,
                origin,
                address,
                dispatch_key: None,
            }),
        };
        if work.validates_at(address) {
            Ok(work)
        } else {
            let ConcreteLifecycleWorkKind::DurableLiveWalSign(work) = work.kind else {
                unreachable!("live Sign conversion retains its dedicated carrier")
            };
            Err((work.admission, work.candidate, work.origin))
        }
    }
}
