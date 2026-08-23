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
    let PreparedLifecycleAdmissionOwnerV1::LiveWal(live) = &admission.owner else {
        return None;
    };
    Some((&live.bound, &admission.candidate))
}

impl DurableLiveDecisionApplyCarrierV1 {
    fn validated_receipt(&self) -> Option<&ValidatedBodyReceipt> {
        self.validate.outcome.validated_receipt().filter(|receipt| {
            self.validate.outcome.rejection_identity().is_none()
                && self.validate.outcome.missing_merge_sidecar().is_none()
                && validate_validated_receipt_authority(&self.validate.incumbent, receipt).is_ok()
        })
    }

    fn exact_body_binding(&self) -> bool {
        let Some((bound, candidate)) = live_wal_admission_parts(&self.admission) else {
            return false;
        };
        let Some(validated) = self.validated_receipt() else {
            return false;
        };
        let AdapterEffect::ValidateBody {
            tag: validate_tag,
            round: validate_round,
            subject: validate_subject,
        } = &self.validate.incumbent.effect
        else {
            return false;
        };
        let AdapterEffect::Apply {
            tag: apply_tag,
            subject: apply_subject,
            certificate,
        } = &bound.effect
        else {
            return false;
        };
        let active_context = lifecycle_admission_candidate_context(candidate);
        let expected_payload =
            projection::durable_body_frame_reference(active_context, validated.durable())
                .map(DurablePayloadReference::BodyFrame);
        let expected_apply_pending = self
            .validate
            .incumbent
            .pending
            .project_validate_apply_successor(&self.validate.incumbent.effect, &bound.effect);
        let expected_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Ok((physical, universe, consumed)) = candidate.physical_geometry.normalized() else {
            return false;
        };
        self.validate.validates(self.validate_digest)
            && matches!(
                &self.admission.owner,
                PreparedLifecycleAdmissionOwnerV1::LiveWal(PreparedLiveWalAdmissionV1 {
                    companion: PreparedLiveWalCompanionV1::None,
                    ..
                })
            )
            && self.admission.validates(active_context)
            && expected_payload == Some(candidate.payload)
            && candidate.work_class == LifecycleWorkClass::Apply
            && candidate.key.phase() == LifecyclePhase::Apply
            && candidate.stage.kind() == LifecycleStageKind::ApplyDecision
            && candidate.stage.predecessor_scope() == PredecessorScope::Independent
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.causal_root == self.validate.address.owner.causal_root()
            && candidate.reconstruction_source == self.validate.address.owner.causal_root().digest()
            && candidate.producer_turn.is_none()
            && physical == BTreeMap::from([(expected_slot, self.installed_digest())])
            && universe == std::collections::BTreeSet::from([expected_slot])
            && consumed == universe
            && matches!(&bound.replay_origin, BoundAdapterReplayOriginV1::LiveWal(_))
            && bound.pending.exactly_binds_adapter_effect(&bound.effect)
            && expected_apply_pending.is_some_and(|pending| pending == bound.pending)
            && validate_tag == apply_tag
            && validate_subject == apply_subject
            && certificate.phase == wire::GlobalPhase::Commit
            && certificate.proposal_round == *validate_round
            && certificate.subject == *validate_subject
            && certificate.execution_commitment == validated.execution_commitment()
    }

    fn context(&self) -> LifecycleContext {
        lifecycle_admission_candidate_context(&self.admission.candidate)
    }

    fn installed_digest(&self) -> LifecycleDigest {
        live_wal_admission_parts(&self.admission)
            .map(|(bound, _)| digest_from_hash(bound.pending.exact_effect_identity()))
            .unwrap_or_else(|| LifecycleDigest::new([0; 32]))
    }

    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.exact_body_binding()
            && address.owner == self.validate.address.owner
            && address.ordinal > self.validate.address.ordinal
            && address.slot
                == PhysicalSlotId::for_capacity(LifecycleWorkClass::Apply.capacity_class(), 0)
            && installed_digest == self.installed_digest()
    }

    fn validates(&self, verified: &VerifiedHeightContext) -> bool {
        let mut context = [0_u8; 32];
        context.copy_from_slice(verified.context().id().0.as_ref());
        let Some((bound, _)) = live_wal_admission_parts(&self.admission) else {
            return false;
        };
        let AdapterEffect::Apply { certificate, .. } = &bound.effect else {
            return false;
        };
        self.exact_body_binding()
            && self.context()
                == LifecycleContext::new(LifecycleDigest::new(context), verified.context().height)
            && verified.verify_quorum_certificate(certificate).is_ok()
    }

    fn exactly_matches_candidate(&self, candidate: &CandidateAdmission) -> bool {
        self.exact_body_binding() && candidate == &self.admission.candidate
    }

    fn ledger_record_matches_candidate(
        record: &super::ledger::LifecycleLedgerRecordV1,
        candidate: &CandidateAdmission,
        owner: OwnerId,
        terminal: Option<TerminalOutcome>,
        continuation: super::schema::DurableContinuation,
    ) -> bool {
        record.key() == Some(candidate.key)
            && record.owner() == owner
            && record.work_class() == Some(candidate.work_class)
            && record.stage() == Some(candidate.stage)
            && record.terminal() == Some(terminal)
            && record.reconstruction_source() == candidate.reconstruction_source
            && record.durable_payload() == Some(candidate.payload)
            && record.continuation() == Some(continuation)
            && record.replay_matches_candidate(candidate)
    }

    fn validates_in_ledger(
        &self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
        installed_apply_ordinal: u128,
    ) -> bool {
        if !self.validates(verified) || ledger.context() != self.context() {
            return false;
        }
        let Ok(validate_candidate) = self.validate.incumbent.project_candidate(verified) else {
            return false;
        };
        let mut validate_records = ledger
            .records()
            .iter()
            .filter(|record| record.ordinal() == self.validate.address.ordinal);
        let Some(validate_record) = validate_records.next() else {
            return false;
        };
        if validate_records.next().is_some() {
            return false;
        }
        let mut apply_records = ledger
            .records()
            .iter()
            .filter(|record| record.ordinal() == installed_apply_ordinal);
        let Some(apply_record) = apply_records.next() else {
            return false;
        };
        if apply_records.next().is_some() {
            return false;
        }
        let owner = self.validate.address.owner;
        Self::ledger_record_matches_candidate(
            validate_record,
            &validate_candidate,
            owner,
            Some(TerminalOutcome::Advanced),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::ValidateToApply,
                installed_apply_ordinal,
            ),
        ) && Self::ledger_record_matches_candidate(
            apply_record,
            &self.admission.candidate,
            owner,
            None,
            super::schema::DurableContinuation::None,
        )
    }

    fn project_apply_task(
        &self,
        identity: RecoveredDecisionApplyDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1> {
        let validated = self.validated_receipt()?;
        let (bound, _) = live_wal_admission_parts(&self.admission)?;
        let AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } = &bound.effect
        else {
            return None;
        };
        if !self.exact_body_binding()
            || !identity.matches_carrier(self.context(), self.installed_digest())
        {
            return None;
        }
        Some(
            crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1::from_registry_projection(
                identity,
                *tag,
                *subject,
                certificate.clone(),
                validated.clone(),
            ),
        )
    }

    fn project_apply_completion(
        &self,
        permit: RecoveredDecisionApplyCompletionProjectionPermit,
        completion: &crate::sumeragi::v2_apply::RecoveredDecisionApplyCompletionV1,
    ) -> Option<crate::sumeragi::v2::RecoveredDecisionApplyAdapterCompletionAuthorityV1> {
        let validated = self.validated_receipt()?;
        let (bound, _) = live_wal_admission_parts(&self.admission)?;
        self.exact_body_binding().then_some(())?;
        crate::sumeragi::v2::RecoveredDecisionApplyAdapterCompletionAuthorityV1::from_registry_projection(
            permit,
            self.context(),
            self.installed_digest(),
            &bound.effect,
            validated,
            completion,
        )
    }
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
        if !self.validates_publication(address.owner, address.ordinal, address.slot, digest)
            || !parent.validates_at(match &parent.kind {
                ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => {
                    completion.address
                }
                _ => return Err((self, parent)),
            })
        {
            return Err((self, parent));
        }
        let ConcreteLifecycleWork {
            digest: validate_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(validate),
        } = parent
        else {
            unreachable!("prechecked live Apply parent retained Validate completion")
        };
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(
                DurableRecoveredDecisionApplyWork {
                    carrier: LifecycleDecisionApplyCarrierV1::Live(
                        DurableLiveDecisionApplyCarrierV1 {
                            admission: self.admission,
                            validate,
                            validate_digest,
                        },
                    ),
                    address,
                    dispatch_key: None,
                },
            ),
        };
        if work.validates_at(address) {
            Ok(work)
        } else {
            let ConcreteLifecycleWork {
                kind:
                    ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(
                        DurableRecoveredDecisionApplyWork {
                            carrier: LifecycleDecisionApplyCarrierV1::Live(carrier),
                            ..
                        },
                    ),
                ..
            } = work
            else {
                unreachable!("new live Apply retained its typed carrier")
            };
            Err((
                Self {
                    admission: carrier.admission,
                },
                ConcreteLifecycleWork {
                    digest: carrier.validate_digest,
                    kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(carrier.validate),
                },
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
