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
    let PreparedLifecycleAdmissionOwnerV1::LiveWal(bound) = &admission.owner else {
        return None;
    };
    Some((bound, &admission.candidate))
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

fn into_live_wal_concrete(admission: PreparedLifecycleAdmissionV1) -> ConcreteLifecycleWork {
    let PreparedLifecycleAdmissionV1 { owner, candidate } = admission;
    let PreparedLifecycleAdmissionOwnerV1::LiveWal(bound) = owner else {
        unreachable!("prepared WAL work retained another admission origin")
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
    let BoundAdapterReplayOriginV1::LiveWal(authority) = replay_origin else {
        unreachable!("prepared WAL work retained another replay origin")
    };
    ConcreteLifecycleWork::from_authorized_exact(effect, pending, authority)
        .expect("five-origin live-WAL admission remains exact")
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

/// Opaque Apply row retaining the single prepared lifecycle admission.
#[must_use = "prepared live Apply work has not entered its reserved registry row"]
pub(in crate::sumeragi) struct PreparedLiveValidateApplyRegistryWork {
    admission: PreparedLifecycleAdmissionV1,
}
impl PreparedLiveValidateApplyRegistryWork {
    /// Close exact effect, pending, WAL authority, and staged candidate together.
    pub(super) fn from_exact(
        permit: LiveValidateApplyWorkProjectionPermit,
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
            && digest == digest_from_hash(bound.pending.exact_effect_identity())
            && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
    }

    /// Consume this closed row into its prevalidated exclusive reservation.
    pub(in crate::sumeragi) fn install_into(
        self,
        reservation: LiveValidateApplyRegistryReservation<'_>,
    ) {
        reservation.install_live_apply(into_live_wal_concrete(self.admission));
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
        reservation.install_live_sign(into_live_wal_concrete(self.admission));
    }
}
