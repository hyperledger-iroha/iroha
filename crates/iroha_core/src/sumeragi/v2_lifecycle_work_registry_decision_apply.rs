/// Closed concrete carrier for one live Decision-backed Apply.
///
/// The carrier retains the original WAL Fetch, all three body successors, and
/// the final pending binding. It has no generic adapter-effect extraction path.
struct DurableRecoveredDecisionApplyWork {
    carrier: DurableDecisionApplyRegistryCarrierV1,
    address: ConcreteWorkAddress,
    dispatch_key: Option<RecoveredDecisionApplyDispatchKeyV1>,
}
/// Exact origin retained by the shared Decision Apply worker owner.
enum DurableDecisionApplyRegistryCarrierV1 {
    /// Apply reconstructed with the complete recovered Decision body family.
    Recovered(RecoveredDecisionApplyRegistryCarrierV1),
    /// Apply emitted by the ordinary durable Validate completion transaction.
    Validate(DurableValidateApplyRegistryCarrierV1),
}
/// Ordinary Validate-to-Apply carrier retaining all replay and body authority.
struct DurableValidateApplyRegistryCarrierV1 {
    context: LifecycleContext,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    validated_receipt: ValidatedBodyReceipt,
    candidate: CandidateAdmission,
    replay_evidence: DurableValidateApplyReplayEvidenceV1,
}
impl DurableValidateApplyRegistryCarrierV1 {
    fn from_exact(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        validated_receipt: ValidatedBodyReceipt,
        candidate: CandidateAdmission,
        replay_evidence: DurableValidateApplyReplayEvidenceV1,
    ) -> Result<
        Self,
        (
            RegistryError,
            AdapterEffect,
            PendingRuntimeEffectBinding,
            ValidatedBodyReceipt,
            CandidateAdmission,
            DurableValidateApplyReplayEvidenceV1,
        ),
    > {
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(validated_receipt.durable().context_id().0.as_ref());
        let context = LifecycleContext::new(
            LifecycleDigest::new(context_id),
            validated_receipt.durable().round().height,
        );
        if !replay_evidence.validates_registry_carrier(
            context,
            &effect,
            &pending,
            &validated_receipt,
            &candidate,
        ) {
            return Err((
                RegistryError::CorruptWork,
                effect,
                pending,
                validated_receipt,
                candidate,
                replay_evidence,
            ));
        }
        Ok(Self {
            context,
            effect,
            pending,
            validated_receipt,
            candidate,
            replay_evidence,
        })
    }
    fn validates(&self) -> bool {
        self.replay_evidence.validates_registry_carrier(
            self.context,
            &self.effect,
            &self.pending,
            &self.validated_receipt,
            &self.candidate,
        )
    }
    fn installed_digest(&self) -> LifecycleDigest {
        digest_from_hash(self.pending.exact_effect_identity())
    }
    fn project_apply_task(
        &self,
        identity: RecoveredDecisionApplyDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1> {
        let AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } = &self.effect
        else {
            return None;
        };
        (self.validates() && identity.matches_carrier(self.context, self.installed_digest())).then(
            || {
                crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1::from_registry_projection(
                    identity,
                    *subject,
                    certificate.clone(),
                    self.validated_receipt.clone(),
                )
            },
        )
    }
}
impl DurableDecisionApplyRegistryCarrierV1 {
    fn context(&self) -> LifecycleContext {
        match self {
            Self::Recovered(carrier) => carrier.context(),
            Self::Validate(carrier) => carrier.context,
        }
    }
    fn installed_digest(&self) -> LifecycleDigest {
        match self {
            Self::Recovered(carrier) => carrier.installed_digest(),
            Self::Validate(carrier) => carrier.installed_digest(),
        }
    }
    fn validates_digest(&self, installed_digest: LifecycleDigest) -> bool {
        match self {
            Self::Recovered(carrier) => {
                carrier.installed_digest() == installed_digest
                    && carrier.lineage().is_exact(carrier.context())
            }
            Self::Validate(carrier) => {
                carrier.installed_digest() == installed_digest && carrier.validates()
            }
        }
    }
    fn exactly_matches_candidate(&self, candidate: &CandidateAdmission) -> bool {
        match self {
            Self::Recovered(carrier) => carrier.exactly_matches_candidate(candidate),
            Self::Validate(carrier) => carrier.validates() && &carrier.candidate == candidate,
        }
    }
    fn validates_in_ledger(
        &self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
        ordinal: u128,
    ) -> bool {
        match self {
            Self::Recovered(carrier) => carrier.validates_in_ledger(verified, ledger, ordinal),
            Self::Validate(carrier) => {
                super::projection::lifecycle_context(verified.context()) == carrier.context
                    && carrier.validates()
                    && ledger.exactly_matches_live_candidate(ordinal, &carrier.candidate)
            }
        }
    }
    fn project_recovered_apply_task(
        &self,
        identity: RecoveredDecisionApplyDispatchIdentityV1,
    ) -> Option<crate::sumeragi::v2_apply::RecoveredDecisionApplyTaskV1> {
        match self {
            Self::Recovered(carrier) => carrier.project_recovered_apply_task(identity),
            Self::Validate(carrier) => carrier.project_apply_task(identity),
        }
    }
    fn project_recovered_apply_completion(
        &self,
        permit: RecoveredDecisionApplyCompletionProjectionPermit,
        completion: &crate::sumeragi::v2_apply::RecoveredDecisionApplyCompletionV1,
    ) -> Option<crate::sumeragi::v2::RecoveredDecisionApplyAdapterCompletionAuthorityV1> {
        match self {
            Self::Recovered(carrier) => {
                carrier.project_recovered_apply_completion(permit, completion)
            }
            Self::Validate(carrier) => {
                crate::sumeragi::v2::RecoveredDecisionApplyAdapterCompletionAuthorityV1::from_validate_apply_registry(
                    permit,
                    &carrier.replay_evidence,
                    &carrier.effect,
                    &carrier.pending,
                    &carrier.validated_receipt,
                    completion,
                )
            }
        }
    }
}
impl fmt::Debug for DurableRecoveredDecisionApplyWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableRecoveredDecisionApplyWork")
            .field("address", &self.address)
            .field("dispatched", &self.dispatch_key.is_some())
            .finish_non_exhaustive()
    }
}
impl DurableRecoveredDecisionApplyWork {
    fn validates_digest(&self, installed_digest: LifecycleDigest) -> bool {
        self.carrier.validates_digest(installed_digest)
    }
    fn validates_at(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.address == address
            && self.address.slot
                == PhysicalSlotId::for_capacity(LifecycleWorkClass::Apply.capacity_class(), 0)
            && self.validates_digest(installed_digest)
    }
    fn matches_current_ready_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())
        else {
            return false;
        };
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
        self.validates_at(address, installed_digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == self.carrier.context()
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.work_class == LifecycleWorkClass::Apply
            && record.state == super::LifecycleState::Ready
            && slot == address.slot
            && digest == installed_digest
            && metadata.matches_admission(&candidate)
            && self.carrier.exactly_matches_candidate(&candidate)
            && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && coordinator.ready_index.contains(&record.ordinal)
    }
    fn matches_claimed_record(
        &self,
        address: ConcreteWorkAddress,
        installed_digest: LifecycleDigest,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        let (Some(record), Some(metadata)) = (
            coordinator.records.get(&address.ordinal),
            coordinator.durable_records.get(&address.ordinal),
        ) else {
            return false;
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())
        else {
            return false;
        };
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
        self.validates_at(address, installed_digest)
            && coordinator.fault.is_none()
            && coordinator.active_context == self.carrier.context()
            && coordinator.active_lease.as_ref() == Some(lease)
            && record.owner == address.owner
            && record.ordinal == address.ordinal
            && record.key == lease.key()
            && record.owner == lease.owner()
            && record.work_class == LifecycleWorkClass::Apply
            && record.work_class == lease.work_class()
            && record.stage == lease.stage()
            && record.state == super::LifecycleState::Claimed(lease.id())
            && lease.ordinal() == address.ordinal
            && lease.physical_slots() == &record.physical_slots
            && slot == address.slot
            && digest == installed_digest
            && metadata.matches_admission(&candidate)
            && self.carrier.exactly_matches_candidate(&candidate)
            && coordinator.key_index.get(&record.key) == Some(&record.ordinal)
            && coordinator.owner_index.get(&record.owner.causal_root()) == Some(&record.owner)
            && !coordinator.ready_index.contains(&record.ordinal)
    }
}
/// Closed service demand authenticated for one Ready recovered Decision Apply.
///
/// The classifier has exactly one first-release outcome: execution must enter
/// the bounded height-local I/O worker before the coordinator may claim the
/// Apply. Keeping this as a typed outcome prevents callers from supplying an
/// unbound boolean capacity hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyRecoveredDecisionApplyDemand {
    /// Reserve one bounded I/O command position before claiming the Apply.
    BoundedIo,
}
/// Opaque proof that one Ready row is the exact recovered Decision Apply carrier.
///
/// Construction is private to the concrete registry classifier. The retained
/// carrier, body receipt, effect, pending binding, address, and digest never
/// leave the registry; the scheduler can inspect only the typed service demand
/// and an opaque key for reserving the exact worker position.
#[must_use = "a Ready recovered Decision Apply attestation must enter scheduler classification"]
pub(super) struct ReadyRecoveredDecisionApplyAttestation {
    demand: ReadyRecoveredDecisionApplyDemand,
    dispatch_key: RecoveredDecisionApplyDispatchKeyV1,
    _seal: ReadyRecoveredDecisionApplyAttestationSeal,
}
struct ReadyRecoveredDecisionApplyAttestationSeal;
impl Drop for ReadyRecoveredDecisionApplyAttestationSeal {
    fn drop(&mut self) {}
}
impl ReadyRecoveredDecisionApplyAttestation {
    /// Return the sole typed service demand without exposing carrier parts.
    pub(super) const fn demand(&self) -> ReadyRecoveredDecisionApplyDemand {
        self.demand
    }
    /// Return the queue key derived from the exact Ready carrier location.
    pub(super) const fn dispatch_key(&self) -> RecoveredDecisionApplyDispatchKeyV1 {
        self.dispatch_key
    }
    /// Recheck that this attestation still belongs to the exact Ready row.
    pub(super) fn matches_ready_record(&self, record: &super::LifecycleRecord) -> bool {
        record.state == super::LifecycleState::Ready
            && record.work_class == LifecycleWorkClass::Apply
            && record.key.phase() == LifecyclePhase::Apply
            && record.stage.kind() == LifecycleStageKind::ApplyDecision
            && record.stage.predecessor_scope() == PredecessorScope::Independent
            && record.physical_slots.len() == 1
            && record
                .physical_slots
                .first_key_value()
                .and_then(|(&slot, &digest)| {
                    ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                        .map(|address| (address, digest))
                })
                .is_some_and(|(address, digest)| {
                    self.dispatch_key.context == record.key.context()
                        && self.dispatch_key.height == record.key.round().height()
                        && self.dispatch_key.owner == address.owner
                        && self.dispatch_key.ordinal == address.ordinal
                        && self.dispatch_key.slot == address.slot
                        && self.dispatch_key.digest == digest
                })
    }
}
/// One-shot permit for converting authenticated Decision/Validate Apply replay.
pub(in crate::sumeragi) struct ValidateApplyRegistryWorkProjectionPermit {
    _linearity: ValidateApplyRegistryWorkProjectionLinearity,
}
struct ValidateApplyRegistryWorkProjectionLinearity;
impl Drop for ValidateApplyRegistryWorkProjectionLinearity {
    fn drop(&mut self) {}
}
impl ValidateApplyRegistryWorkProjectionPermit {
    pub(super) fn new() -> Self {
        Self {
            _linearity: ValidateApplyRegistryWorkProjectionLinearity,
        }
    }
}
/// Closed Apply carrier prepared before LifecycleLedgerV1 fsync.
#[must_use = "prepared Validate Apply work has not been installed"]
pub(in crate::sumeragi) struct PreparedValidateApplyRegistryWork {
    carrier: DurableValidateApplyRegistryCarrierV1,
    digest: LifecycleDigest,
}
/// Preflighted parent-retirement/Apply-child vacancy cut.
pub(in crate::sumeragi) struct LiveValidateApplyRegistryReservation<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    parent_address: ConcreteWorkAddress,
    parent_digest: LifecycleDigest,
    child_address: ConcreteWorkAddress,
    child_digest: LifecycleDigest,
}
impl PreparedValidateApplyRegistryWork {
    pub(super) fn from_exact(
        _permit: ValidateApplyRegistryWorkProjectionPermit,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        validated_receipt: ValidatedBodyReceipt,
        candidate: CandidateAdmission,
        replay_evidence: DurableValidateApplyReplayEvidenceV1,
    ) -> Result<
        Self,
        (
            RegistryError,
            AdapterEffect,
            PendingRuntimeEffectBinding,
            ValidatedBodyReceipt,
            CandidateAdmission,
            DurableValidateApplyReplayEvidenceV1,
        ),
    > {
        let carrier = DurableValidateApplyRegistryCarrierV1::from_exact(
            effect,
            pending,
            validated_receipt,
            candidate,
            replay_evidence,
        )?;
        let digest = carrier.installed_digest();
        Ok(Self { carrier, digest })
    }
    pub(super) fn validates_publication(
        &self,
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        ConcreteWorkAddress::new(owner, ordinal, slot).is_some()
            && slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
            && self.carrier.validates()
            && self.digest == digest
            && self.carrier.candidate.causal_root == owner.causal_root()
    }
    pub(in crate::sumeragi) fn install_into(
        self,
        reservation: LiveValidateApplyRegistryReservation<'_>,
    ) {
        assert!(self.validates_publication(
            reservation.child_address.owner,
            reservation.child_address.ordinal,
            reservation.child_address.slot,
            reservation.child_digest,
        ));
        let parent = reservation
            .entries
            .remove(&reservation.parent_address)
            .expect("preflighted Apply publication retains its Validate parent");
        assert_eq!(parent.digest(), reservation.parent_digest);
        let work = ConcreteLifecycleWork {
            digest: self.digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(
                DurableRecoveredDecisionApplyWork {
                    carrier: DurableDecisionApplyRegistryCarrierV1::Validate(self.carrier),
                    address: reservation.child_address,
                    dispatch_key: None,
                },
            ),
        };
        assert!(work.validates_at(reservation.child_address));
        assert!(
            reservation
                .entries
                .insert(reservation.child_address, work)
                .is_none(),
            "preflighted Apply child address remains vacant"
        );
        drop(parent);
    }
}
