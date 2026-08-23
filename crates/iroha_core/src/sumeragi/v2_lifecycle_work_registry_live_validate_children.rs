/// Reconstruct the active lifecycle context named by one already-projected child.
fn lifecycle_admission_candidate_context(candidate: &CandidateAdmission) -> LifecycleContext {
    LifecycleContext::new(candidate.key.context(), candidate.key.round().height())
}

/// Close one bound origin around the exact candidate already used to stage LedgerV1.
fn prepare_exact_bound_lifecycle_admission(
    candidate: CandidateAdmission,
    bound: BoundAdapterEffectV1,
) -> Result<PreparedLifecycleAdmissionV1, (CandidateAdmission, BoundAdapterEffectV1)> {
    let active_context = lifecycle_admission_candidate_context(&candidate);
    if !bound.exactly_authorizes_candidate(active_context, &candidate) {
        return Err((candidate, bound));
    }
    Ok(
        PreparedLifecycleAdmissionV1::from_returned_bound(active_context, candidate, bound)
            .expect("exact bound child retains one five-origin lifecycle admission"),
    )
}

/// Close one live-WAL Apply around the exact receipt-derived body frame used by LedgerV1.
fn prepare_exact_bound_live_wal_apply_admission(
    candidate: CandidateAdmission,
    bound: BoundAdapterEffectV1,
    receipt: &DurableBodyReceipt,
) -> Result<PreparedLifecycleAdmissionV1, (CandidateAdmission, BoundAdapterEffectV1)> {
    let active_context = lifecycle_admission_candidate_context(&candidate);
    let Some(frame) = projection::durable_body_frame_reference(active_context, receipt) else {
        return Err((candidate, bound));
    };
    let payload = DurablePayloadReference::BodyFrame(frame);
    if candidate.payload != payload || !frame.matches_key(candidate.key) {
        return Err((candidate, bound));
    }
    let live = match PreparedLiveWalAdmissionV1::apply_body_frame(bound, payload) {
        Ok(live) => live,
        Err(bound) => return Err((candidate, bound)),
    };
    if !live.exactly_authorizes_candidate(active_context, &candidate) {
        let PreparedLiveWalAdmissionV1 {
            bound,
            companion: _,
        } = live;
        return Err((candidate, bound));
    }
    Ok(
        PreparedLifecycleAdmissionV1::from_returned_live_wal(active_context, candidate, live)
            .expect("exact Apply body-frame owner retains one five-origin lifecycle admission"),
    )
}

fn invalid_report_admission_parts(
    admission: &PreparedLifecycleAdmissionV1,
) -> Option<(&BoundAdapterEffectV1, &CandidateAdmission)> {
    let PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(bound) = &admission.owner else {
        return None;
    };
    Some((bound, &admission.candidate))
}

fn live_wal_admission_parts(
    admission: &PreparedLifecycleAdmissionV1,
) -> Option<(&BoundAdapterEffectV1, &CandidateAdmission)> {
    let PreparedLifecycleAdmissionOwnerV1::LiveWal(live) = &admission.owner else {
        return None;
    };
    Some((&live.bound, &admission.candidate))
}

fn into_invalid_report_concrete(admission: PreparedLifecycleAdmissionV1) -> ConcreteLifecycleWork {
    let PreparedLifecycleAdmissionV1 { owner, candidate } = admission;
    let PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(bound) = owner else {
        unreachable!("prepared report work retained another admission origin")
    };
    debug_assert!(bound.exactly_authorizes_candidate(
        lifecycle_admission_candidate_context(&candidate),
        &candidate,
    ));
    let BoundAdapterEffectV1 {
        effect,
        pending,
        replay_origin,
    } = bound;
    let BoundAdapterReplayOriginV1::InvalidBodyReport(authority) = replay_origin else {
        unreachable!("prepared report work retained another replay origin")
    };
    ConcreteLifecycleWork::from_authorized_exact(effect, pending, authority)
        .expect("five-origin invalid-body admission remains exact")
}

/// One-shot authority for turning the replay-sealed invalid-body effect into
/// the same five-origin admission that staged its durable child.
pub(in crate::sumeragi) struct LiveValidateReportWorkProjectionPermit {
    candidate: CandidateAdmission,
    _linearity: LiveValidateReportWorkProjectionLinearity,
}
struct LiveValidateReportWorkProjectionLinearity;
impl Drop for LiveValidateReportWorkProjectionLinearity {
    fn drop(&mut self) {}
}
impl LiveValidateReportWorkProjectionPermit {
    fn new(candidate: CandidateAdmission) -> Self {
        Self {
            candidate,
            _linearity: LiveValidateReportWorkProjectionLinearity,
        }
    }
}

/// Opaque invalid-body row retaining the single prepared lifecycle admission.
#[must_use = "prepared live invalid-body report has not entered its reserved registry row"]
pub(in crate::sumeragi) struct PreparedLiveValidateReportRegistryWork {
    admission: PreparedLifecycleAdmissionV1,
}
impl PreparedLiveValidateReportRegistryWork {
    /// Accept only the bound owner and exact candidate minted by rejection evidence.
    pub(super) fn from_bound(
        permit: LiveValidateReportWorkProjectionPermit,
        bound: BoundAdapterEffectV1,
    ) -> Self {
        let admission = prepare_exact_bound_lifecycle_admission(permit.candidate, bound)
            .unwrap_or_else(|_| panic!("sealed report candidate and bound origin remain exact"));
        debug_assert!(matches!(
            &admission.owner,
            PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(_)
        ));
        Self { admission }
    }

    /// Match the staged report row, including exact owner, digest, and slot.
    pub(in crate::sumeragi) fn validates_publication(
        &self,
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return false;
        };
        let Some((bound, candidate)) = invalid_report_admission_parts(&self.admission) else {
            return false;
        };
        let active_context = lifecycle_admission_candidate_context(candidate);
        self.admission.validates(active_context)
            && candidate.work_class == LifecycleWorkClass::InvalidBodyReport
            && candidate.stage.kind() == LifecycleStageKind::ReportInvalidBody
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.causal_root == owner.causal_root()
            && matches!(
                &bound.replay_origin,
                BoundAdapterReplayOriginV1::InvalidBodyReport(authority)
                    if authority.is_invalid_body_report_origin()
            )
            && matches!(
                &bound.effect,
                AdapterEffect::ReportInvalidCertifiedBody { .. }
            )
            && digest == digest_from_hash(bound.pending.exact_effect_identity())
            && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0)
    }

    /// Consume the single admission into ordinary concrete work after all checks.
    fn into_concrete(self) -> ConcreteLifecycleWork {
        into_invalid_report_concrete(self.admission)
    }
}

/// One-shot authority for consuming a body-frame-completed live-WAL Apply seal.
pub(in crate::sumeragi) struct LiveValidateApplyWorkProjectionPermit {
    candidate: CandidateAdmission,
    _linearity: LiveValidateApplyWorkProjectionLinearity,
}
struct LiveValidateApplyWorkProjectionLinearity;
impl Drop for LiveValidateApplyWorkProjectionLinearity {
    fn drop(&mut self) {}
}
impl LiveValidateApplyWorkProjectionPermit {
    fn new(candidate: CandidateAdmission) -> Self {
        Self {
            candidate,
            _linearity: LiveValidateApplyWorkProjectionLinearity,
        }
    }
}

/// Closed process-local carrier for one live Validate-to-Apply successor.
///
/// The full validated receipt and detached Validate parent remain inseparable
/// from the exact WAL admission. This is deliberately not ordinary pending
/// adapter work and cannot be mistaken for the recovered Decision lineage.
struct DurableLiveWalApplyWork {
    admission: PreparedLiveWalAdmissionV1,
    candidate: CandidateAdmission,
    validated_receipt: ValidatedBodyReceipt,
    parent_address: ConcreteWorkAddress,
    parent: Box<ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    dispatch_key: Option<LifecycleDecisionApplyDispatchKeyV1>,
}

impl fmt::Debug for DurableLiveWalApplyWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableLiveWalApplyWork")
            .field("address", &self.address)
            .field("parent_address", &self.parent_address)
            .field("dispatched", &self.dispatch_key.is_some())
            .finish_non_exhaustive()
    }
}

impl DurableLiveWalApplyWork {
    fn context(&self) -> LifecycleContext {
        lifecycle_admission_candidate_context(&self.candidate)
    }

    fn digest(&self) -> LifecycleDigest {
        digest_from_hash(self.admission.bound.pending.exact_effect_identity())
    }

    fn exact_body_binding(&self) -> bool {
        let AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } = &self.admission.bound.effect
        else {
            return false;
        };
        let PreparedLiveWalCompanionV1::ApplyBodyFrame(payload) = &self.admission.companion else {
            return false;
        };
        let Some(frame) = projection::durable_body_frame_reference(
            self.context(),
            self.validated_receipt.durable(),
        ) else {
            return false;
        };
        *payload == DurablePayloadReference::BodyFrame(frame)
            && frame.matches_key(self.candidate.key)
            && self.validated_receipt.durable().subject() == *subject
            && certificate.subject == *subject
            && certificate.execution_commitment == self.validated_receipt.execution_commitment()
            && self
                .admission
                .exactly_authorizes_candidate(self.context(), &self.candidate)
    }

    fn validates_at(&self, address: ConcreteWorkAddress, digest: LifecycleDigest) -> bool {
        let parent_is_exact = self.parent.validates_at(self.parent_address)
            && matches!(
                &self.parent.kind,
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                    if completion.outcome.validated_receipt() == Some(&self.validated_receipt)
            );
        self.address == address
            && self.parent_address != address
            && self.parent_address.owner == address.owner
            && address.owner.causal_root() == self.candidate.causal_root
            && address.slot
                == PhysicalSlotId::for_capacity(LifecycleWorkClass::Apply.capacity_class(), 0)
            && digest == self.digest()
            && self.candidate.work_class == LifecycleWorkClass::Apply
            && self.candidate.stage.kind() == LifecycleStageKind::ApplyDecision
            && self.candidate.initial_state == InitialLifecycleState::Ready
            && self.exact_body_binding()
            && parent_is_exact
    }

    fn predecessor_is_exact_in_coordinator(&self, coordinator: &LifecycleCoordinator) -> bool {
        coordinator
            .records
            .get(&self.parent_address.ordinal)
            .is_some_and(|record| {
                record.owner == self.parent_address.owner
                    && record.ordinal == self.parent_address.ordinal
                    && record.work_class == LifecycleWorkClass::Validate
                    && record.state
                        == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
                    && record.physical_slots
                        == BTreeMap::from([(self.parent_address.slot, self.parent.digest())])
            })
            && coordinator
                .durable_records
                .get(&self.parent_address.ordinal)
                .is_some_and(|metadata| {
                    metadata.continuation
                        == super::schema::DurableContinuation::successor(
                            super::schema::DurableContinuationEdge::ValidateToApply,
                            self.address.ordinal,
                        )
                })
    }

    fn validates_in_ledger(&self, ledger: &super::ledger::LifecycleLedgerV1) -> bool {
        let (Some(parent_record), Some(apply_record)) = (
            ledger
                .records()
                .iter()
                .find(|record| record.ordinal() == self.parent_address.ordinal),
            ledger
                .records()
                .iter()
                .find(|record| record.ordinal() == self.address.ordinal),
        ) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &self.parent.kind
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
        self.validates_at(self.address, self.digest())
            && ledger.context() == self.context()
            && self.parent.validates_at(self.parent_address)
            && completion.outcome.validated_receipt() == Some(&self.validated_receipt)
            && parent_record.owner() == self.parent_address.owner
            && parent_record.ordinal() == self.parent_address.ordinal
            && parent_record.work_class() == Some(LifecycleWorkClass::Validate)
            && parent_stage.kind() == LifecycleStageKind::ValidateBody
            && parent_record.terminal() == Some(Some(super::TerminalOutcome::Advanced))
            && parent_record.continuation()
                == Some(super::schema::DurableContinuation::successor(
                    super::schema::DurableContinuationEdge::ValidateToApply,
                    self.address.ordinal,
                ))
            && projection::recovered_validate_no_successor_ledger_identity_is_authenticated(
                ledger.context(),
                parent_key,
                self.parent_address.owner.causal_root(),
                parent_record.reconstruction_source(),
                parent_stage,
                parent_payload,
                &completion.outcome,
            )
            && apply_record.key() == Some(self.candidate.key)
            && apply_record.owner() == self.address.owner
            && apply_record.ordinal() == self.address.ordinal
            && apply_record.work_class() == Some(LifecycleWorkClass::Apply)
            && apply_record.stage() == Some(self.candidate.stage)
            && apply_record.terminal() == Some(None)
            && apply_record.reconstruction_source() == self.candidate.reconstruction_source
            && apply_record.durable_payload() == Some(self.candidate.payload)
            && apply_record.continuation() == Some(super::schema::DurableContinuation::None)
            && apply_record.replay_matches_candidate(&self.candidate)
            && ledger
                .records()
                .iter()
                .filter(|record| {
                    record.owner() == self.address.owner
                        && record.ordinal() >= self.parent_address.ordinal
                })
                .count()
                == 2
    }

    fn matches_record(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        state: super::LifecycleState,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        self.validates_at(address, digest)
            && self.predecessor_is_exact_in_coordinator(coordinator)
            && coordinator.fault.is_none()
            && coordinator.active_context == self.context()
            && record.key == self.candidate.key
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == self.candidate.work_class
            && record.stage == self.candidate.stage
            && record.state == state
            && record.physical_slots == BTreeMap::from([(address.slot, digest)])
            && record.episode.slot_universe == std::collections::BTreeSet::from([address.slot])
            && record.episode.consumed_slots == record.episode.slot_universe
            && metadata.matches_admission(&self.candidate)
            && metadata.continuation == super::schema::DurableContinuation::None
            && coordinator.key_index.get(&record.key) == Some(&address.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
    }

    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.matches_record(address, digest, coordinator, super::LifecycleState::Ready)
            && coordinator.active_lease.is_none()
            && coordinator.ready_index.contains(&address.ordinal)
    }

    fn matches_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        self.matches_record(
            address,
            digest,
            coordinator,
            super::LifecycleState::Claimed(lease.id()),
        ) && coordinator.active_lease.as_ref() == Some(lease)
            && lease.key() == self.candidate.key
            && lease.owner() == address.owner
            && lease.ordinal() == address.ordinal
            && lease.work_class() == LifecycleWorkClass::Apply
            && lease.stage() == self.candidate.stage
            && lease.physical_slots() == &BTreeMap::from([(address.slot, digest)])
            && !coordinator.ready_index.contains(&address.ordinal)
    }

    fn project_task(
        &self,
        identity: LifecycleDecisionApplyDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_apply::LifecycleDecisionApplyTaskV1> {
        if !identity.matches_carrier(
            self.context(),
            self.address,
            self.digest(),
            LifecycleDecisionApplyLineageV1::Live,
        ) {
            return None;
        }
        let AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } = &self.admission.bound.effect
        else {
            return None;
        };
        crate::sumeragi::v2_apply::LifecycleDecisionApplyTaskV1::from_live_registry_projection(
            identity,
            *tag,
            *subject,
            certificate.clone(),
            self.validated_receipt.clone(),
        )
    }

    fn project_reconciliation(
        &self,
        dispatch_key: LifecycleDecisionApplyDispatchKeyV1,
    ) -> Option<LiveLifecycleDecisionApplyReconciliationAuthorityV1> {
        if !self.exact_body_binding()
            || !dispatch_key.matches_carrier(
                self.context(),
                self.address,
                self.digest(),
                LifecycleDecisionApplyLineageV1::Live,
            )
        {
            return None;
        }
        let AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } = &self.admission.bound.effect
        else {
            return None;
        };
        Some(LiveLifecycleDecisionApplyReconciliationAuthorityV1 {
            dispatch_key,
            tag: *tag,
            subject: *subject,
            certificate: certificate.clone(),
            validated_receipt: self.validated_receipt.clone(),
            pending_causal_key: *self.admission.bound.pending.causal_lifecycle_key(),
            pending_effect_identity: *self.admission.bound.pending.exact_effect_identity(),
            pending_candidate_statement: self.admission.bound.pending.candidate_statement(),
            _seal: LiveLifecycleDecisionApplyReconciliationAuthoritySealV1,
        })
    }

    fn project_completion(
        &self,
        permit: LifecycleDecisionApplyCompletionProjectionPermitV1,
        completion: &crate::sumeragi::v2_apply::LifecycleDecisionApplyCompletionV1,
    ) -> Option<crate::sumeragi::v2::LifecycleDecisionApplyAdapterCompletionAuthorityV1> {
        crate::sumeragi::v2::project_live_decision_apply_completion(
            permit,
            self.context(),
            self.address,
            self.digest(),
            &self.admission.bound.effect,
            &self.validated_receipt,
            completion,
        )
    }
}

/// Opaque Apply row retaining the single prepared lifecycle admission and receipt.
#[must_use = "prepared live Apply work has not entered its reserved registry row"]
pub(in crate::sumeragi) struct PreparedLiveValidateApplyRegistryWork {
    admission: PreparedLifecycleAdmissionV1,
    validated_receipt: ValidatedBodyReceipt,
}
impl PreparedLiveValidateApplyRegistryWork {
    /// Close exact effect, pending, WAL authority, and staged candidate together.
    pub(super) fn from_exact(
        permit: LiveValidateApplyWorkProjectionPermit,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        replay_authority: LifecycleReplayAuthorityV1,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        let bound = BoundAdapterEffectV1::bind_live_wal(effect, pending, replay_authority)
            .map_err(|(effect, pending, _)| (RegistryError::CorruptWork, effect, pending))?;
        match prepare_exact_bound_live_wal_apply_admission(
            permit.candidate,
            bound,
            receipt.durable(),
        ) {
            Ok(admission) => Ok(Self {
                admission,
                validated_receipt: receipt.clone(),
            }),
            Err((candidate, bound)) => {
                drop(candidate);
                let BoundAdapterEffectV1 {
                    effect,
                    pending,
                    replay_origin: _,
                } = bound;
                Err((RegistryError::CorruptWork, effect, pending))
            }
        }
    }

    /// Match the exact staged Apply row and inherited causal owner.
    pub(in crate::sumeragi) fn validates_publication(
        &self,
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return false;
        };
        let Some((bound, candidate)) = live_wal_admission_parts(&self.admission) else {
            return false;
        };
        let active_context = lifecycle_admission_candidate_context(candidate);
        self.admission.validates(active_context)
            && candidate.work_class == LifecycleWorkClass::Apply
            && candidate.stage.kind() == LifecycleStageKind::ApplyDecision
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.causal_root == owner.causal_root()
            && matches!(&bound.effect, AdapterEffect::Apply { .. })
            && matches!(
                &bound.effect,
                AdapterEffect::Apply {
                    subject,
                    certificate,
                    ..
                } if self.validated_receipt.durable().subject() == *subject
                    && certificate.subject == *subject
                    && certificate.execution_commitment
                        == self.validated_receipt.execution_commitment()
            )
            && digest == digest_from_hash(bound.pending.exact_effect_identity())
            && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
    }

    /// Join the closed Apply admission to its moved successful Validate parent.
    ///
    /// Failure returns both move-only inputs unchanged so the pre-fsync
    /// publication transaction can restore the incumbent registry row.
    fn into_typed_concrete(
        self,
        parent: ConcreteLifecycleWork,
        address: ConcreteWorkAddress,
        digest: LifecycleDigest,
    ) -> Result<ConcreteLifecycleWork, (Self, ConcreteLifecycleWork)> {
        let parent_address = match &parent.kind {
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                completion.address
            }
            _ => return Err((self, parent)),
        };
        if !self.validates_publication(address.owner, address.ordinal, address.slot, digest)
            || !parent.validates_at(parent_address)
            || parent_address == address
            || parent_address.owner != address.owner
            || !matches!(
                &parent.kind,
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                    if completion.outcome.validated_receipt() == Some(&self.validated_receipt)
            )
        {
            return Err((self, parent));
        }
        let Self {
            admission,
            validated_receipt,
        } = self;
        let PreparedLifecycleAdmissionV1 { owner, candidate } = admission;
        let PreparedLifecycleAdmissionOwnerV1::LiveWal(admission) = owner else {
            unreachable!("prepared live Apply retained another admission origin")
        };
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableLiveWalApply(DurableLiveWalApplyWork {
                admission,
                candidate,
                validated_receipt,
                parent_address,
                parent: Box::new(parent),
                address,
                dispatch_key: None,
            }),
        };
        if work.validates_at(address) {
            Ok(work)
        } else {
            let ConcreteLifecycleWork {
                kind: ConcreteLifecycleWorkKind::DurableLiveWalApply(carrier),
                ..
            } = work
            else {
                unreachable!("new live Apply retained its dedicated typed carrier")
            };
            let DurableLiveWalApplyWork {
                admission,
                candidate,
                validated_receipt,
                parent,
                ..
            } = carrier;
            Err((
                Self {
                    admission: PreparedLifecycleAdmissionV1 {
                        owner: PreparedLifecycleAdmissionOwnerV1::LiveWal(admission),
                        candidate,
                    },
                    validated_receipt,
                },
                *parent,
            ))
        }
    }
}

/// Opaque Sign row retaining the single prepared lifecycle admission.
#[must_use = "prepared live Sign work has not entered its reserved registry row"]
pub(in crate::sumeragi) struct PreparedLiveValidateSignRegistryWork {
    admission: PreparedLifecycleAdmissionV1,
}
impl PreparedLiveValidateSignRegistryWork {
    /// Close exact effect, pending, WAL authority, and staged candidate together.
    pub(super) fn from_exact(
        permit: LiveValidateSignWorkProjectionPermit,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        replay_authority: LifecycleReplayAuthorityV1,
    ) -> Result<Self, (RegistryError, AdapterEffect, PendingRuntimeEffectBinding)> {
        let bound = BoundAdapterEffectV1::bind_live_wal(effect, pending, replay_authority)
            .map_err(|(effect, pending, _)| (RegistryError::CorruptWork, effect, pending))?;
        match prepare_exact_bound_lifecycle_admission(permit.candidate, bound) {
            Ok(admission) => Ok(Self { admission }),
            Err((candidate, bound)) => {
                drop(candidate);
                let BoundAdapterEffectV1 {
                    effect,
                    pending,
                    replay_origin: _,
                } = bound;
                Err((RegistryError::CorruptWork, effect, pending))
            }
        }
    }

    /// Revalidate the still-closed effect, pending binding, and WAL candidate.
    pub(in crate::sumeragi) fn validates_exact(&self) -> bool {
        live_wal_admission_parts(&self.admission).is_some_and(|(bound, candidate)| {
            self.admission
                .validates(lifecycle_admission_candidate_context(candidate))
                && matches!(
                    &bound.effect,
                    AdapterEffect::Sign {
                        request: SignRequest::Vote(vote),
                        ..
                    } if matches!(
                        vote.phase,
                        wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit
                    )
                )
        })
    }

    /// Match the exact staged Sign row, including its inherited causal owner.
    pub(in crate::sumeragi) fn validates_publication(
        &self,
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return false;
        };
        let Some((bound, candidate)) = live_wal_admission_parts(&self.admission) else {
            return false;
        };
        self.validates_exact()
            && candidate.work_class == LifecycleWorkClass::SignVote
            && matches!(
                candidate.stage.kind(),
                LifecycleStageKind::SignPrepareVote | LifecycleStageKind::SignCommitVote
            )
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.causal_root == owner.causal_root()
            && digest == digest_from_hash(bound.pending.exact_effect_identity())
            && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
    }

    /// Consume this closed row into its prevalidated exclusive reservation.
    pub(in crate::sumeragi) fn install_into(
        self,
        reservation: LiveValidateSignRegistryReservation<'_>,
    ) {
        reservation.install_live_sign(self);
    }
}
