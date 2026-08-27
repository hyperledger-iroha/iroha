/// Closed durable form of one admitted `StoreBody` effect.
///
/// The expected manifest hash is transferred independently from the
/// authenticated parent response. It deliberately remains distinct from the
/// body-store receipt's own manifest hash so validation proves agreement
/// between the transport family and the durable catalog entry.
#[derive(Debug)]
pub(super) struct DurableStoreBody {
    address: ConcreteWorkAddress,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    replay_evidence: CertifiedStoreReplayEvidenceV1,
}
impl DurableStoreBody {
    /// Reconstruct the exact durable Store child of one authenticated Fetch.
    pub(super) fn from_recovered_certified_fetch(
        completion: CertifiedFetchCompletion,
        verified: &VerifiedHeightContext,
        ordinal: u128,
    ) -> Result<
        (
            Self,
            CandidateAdmission,
            CertifiedBodyPipelineColdReplayStepV1,
        ),
        (),
    > {
        let owner = completion.address.owner;
        let Some((tag, manifest)) = completion.replay_evidence.adapter_preview_inputs(
            &completion.incumbent_effect,
            &completion.incumbent_pending,
            &completion.durable_receipt,
        ) else {
            return Err(());
        };
        let manifest = manifest.clone();
        let effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let Some(pending) = completion
            .incumbent_pending
            .project_certified_fetch_store_successor(&completion.incumbent_effect, &effect)
        else {
            return Err(());
        };
        let Some(replay_evidence) = completion.replay_evidence.project_store(
            &completion.incumbent_effect,
            &completion.incumbent_pending,
            &completion.durable_receipt,
            &effect,
        ) else {
            return Err(());
        };
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Store.capacity_class(), 0);
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return Err(());
        };
        let digest = digest_from_hash(pending.exact_effect_identity());
        let expected_manifest_hash = HashOf::new(&manifest);
        let carrier = Self {
            address,
            effect: effect.clone(),
            pending,
            durable_receipt: completion.durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        if !carrier.validates(digest) {
            return Err(());
        }
        let Ok(candidate) = carrier.project_candidate(verified) else {
            return Err(());
        };
        let Some(replay) = CertifiedBodyPipelineColdReplayStepV1::body_available(
            completion.address.ordinal,
            tag,
            manifest,
            effect,
        ) else {
            return Err(());
        };
        Ok((carrier, candidate, replay))
    }

    pub(super) fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        let AdapterEffect::StoreBody { round, subject, .. } = &self.effect else {
            return false;
        };
        ConcreteWorkAddress::new(self.address.owner, self.address.ordinal, self.address.slot)
            == Some(self.address)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && installed_digest == digest_from_hash(self.pending.exact_effect_identity())
            && self.durable_receipt.context_id() == round.context_id
            && self.durable_receipt.round() == *round
            && self.durable_receipt.subject() == *subject
            && self.durable_receipt.manifest_hash() == self.expected_manifest_hash
            && self
                .replay_evidence
                .exactly_matches_store(&self.effect, &self.durable_receipt)
    }
    pub(super) const fn address(&self) -> ConcreteWorkAddress {
        self.address
    }
    pub(super) fn ready_digest(&self) -> LifecycleDigest {
        digest_from_hash(self.pending.exact_effect_identity())
    }
    fn matches_recovered_record(
        &self,
        active_context: LifecycleContext,
        record: &LifecycleRecord,
        metadata: &DurableRecordMetadata,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.validates(installed_digest)
            && self.replay_evidence.exactly_matches_recovered_record(
                active_context,
                record,
                metadata,
                installed_digest,
                &self.effect,
                &self.durable_receipt,
                &self.pending,
            )
    }
    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.replay_evidence.project_installed_store_candidate(
            InstalledBodyCandidateProjectionPermit::new(),
            verified,
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }
}
/// Closed durable form of one admitted `ValidateBody` effect.
///
/// The body receipt remains attached to the exact causal lineage that moved
/// through Fetch and Store. The independently transferred manifest hash is a
/// second authority coordinate: it is never reconstructed from the receipt.
#[derive(Debug)]
pub(super) struct DurableValidateBody {
    address: ConcreteWorkAddress,
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    replay_evidence: DurableValidateReplayEvidenceV1,
}

/// Move-only launch authority for one exact recovered Ready Validate retry.
///
/// The concrete registry constructs this only while the exact Ready carrier,
/// coordinator row, durable metadata, pending statement, and replay authority
/// agree. Commit-refined carriers additionally bind the runtime's replayed
/// Decision. It is an inert retransmit owner and cannot dispatch work.
#[must_use = "a recovered durable Validate retry owner must be installed before live clocks"]
#[derive(Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDurableValidateRetryOwnerV1 {
    expected_decision: Option<(
        wire::ConsensusRound,
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )>,
    effect: AdapterEffect,
    durable_receipt: DurableBodyReceipt,
    binding: RecoveredDurableValidateRetryBindingV1,
    lifecycle_ordinal: u128,
    owner_class: ColdValidateRetryOwnerClassV1,
}

impl RecoveredDurableValidateRetryOwnerV1 {
    /// Construct one closed owner for focused executor tests.
    ///
    /// Production has no constructor outside the complete registry census;
    /// this helper still requires the exact pending projection used there.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        effect: AdapterEffect,
        durable_receipt: DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
        lifecycle_ordinal: u128,
        expected_decision: Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Option<Self> {
        if lifecycle_ordinal == 0 {
            return None;
        }
        let binding =
            pending.project_recovered_durable_validate_retry_binding(&effect, expected_decision)?;
        Some(Self {
            expected_decision,
            effect,
            durable_receipt,
            binding,
            lifecycle_ordinal,
            owner_class: ColdValidateRetryOwnerClassV1::AdmissionCensus,
        })
    }

    /// Return the exact recovered logical row retained by the registry.
    pub(in crate::sumeragi) const fn lifecycle_ordinal(&self) -> u128 {
        self.lifecycle_ordinal
    }

    /// Return the exact replayed Decision which bounds authority refinement.
    pub(in crate::sumeragi) const fn expected_decision(
        &self,
    ) -> Option<(
        wire::ConsensusRound,
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )> {
        self.expected_decision
    }

    /// Return the sole map key this inert owner may occupy.
    pub(in crate::sumeragi) fn key(&self) -> (wire::ConsensusRound, wire::BlockSubject) {
        match &self.effect {
            AdapterEffect::ValidateBody { round, subject, .. } => (*round, *subject),
            _ => unreachable!("registry projected only a durable Validate retry"),
        }
    }

    /// Borrow the exact authenticated body-store receipt joined by recovery.
    pub(in crate::sumeragi) const fn durable_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_receipt
    }

    /// Return only the immutable recovered causal root to focused tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn causal_lifecycle_key_for_test(&self) -> iroha_crypto::Hash {
        self.binding.causal_lifecycle_key_for_test()
    }

    /// Check whether startup must defer this exact validated marker to the
    /// still-Ready lifecycle worker which owns the same durable body.
    pub(in crate::sumeragi) fn bind_validated_marker(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        validated: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
    ) -> bool {
        self.key() == key
            && validated.durable() == &self.durable_receipt
            && self
                .binding
                .bind_validated_marker_commitment(validated.execution_commitment())
    }

    /// Recheck an already-bound marker during the atomic executor preflight.
    pub(in crate::sumeragi) fn exactly_matches_validated_marker(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        validated: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
    ) -> bool {
        self.key() == key
            && validated.durable() == &self.durable_receipt
            && self
                .binding
                .exactly_matches_validated_marker_commitment(validated.execution_commitment())
    }

    /// Construct the only initial monotonic retry frontier for this owner.
    pub(in crate::sumeragi) fn initial_retry_frontier(
        &self,
    ) -> Option<RecoveredDurableValidateRetryFrontierV1> {
        self.binding.initial_frontier(&self.effect)
    }

    /// Recheck a runtime retransmit without exposing the recovered pending binding.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        frontier: &RecoveredDurableValidateRetryFrontierV1,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<
        (
            RecoveredDurableValidateRetryFrontierV1,
            RuntimeEffectOwnership,
        ),
        String,
    > {
        self.binding
            .project_retry(&self.effect, frontier, effect, incoming)
    }
}

/// Complete move-only launch census for all recovered Ready Validate retries.
///
/// Only the concrete registry can mint this value. Startup may query its
/// closed marker-deferral predicate and must then consume the same value into
/// the executor before services or clocks open; no owner iterator or parts
/// projection exists.
#[must_use = "the complete recovered Validate retry census must be consumed during executor open"]
#[derive(Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDurableValidateRetryCensusV1 {
    owners:
        BTreeMap<(wire::ConsensusRound, wire::BlockSubject), RecoveredDurableValidateRetryOwnerV1>,
}

impl RecoveredDurableValidateRetryCensusV1 {
    /// Report census cardinality without exposing owners to focused tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn len_for_test(&self) -> usize {
        self.owners.len()
    }

    /// Report the deterministic seal/marker partition without exposing owners.
    #[cfg(test)]
    pub(in crate::sumeragi) fn owner_class_counts_for_test(&self) -> (usize, usize) {
        self.owners
            .values()
            .fold((0, 0), |(admission, published), owner| {
                match owner.owner_class {
                    ColdValidateRetryOwnerClassV1::AdmissionCensus => (admission + 1, published),
                    ColdValidateRetryOwnerClassV1::PublishedStoreSuccessor => {
                        (admission, published + 1)
                    }
                }
            })
    }

    /// Corrupt only the last member's durable receipt for atomic-install tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn corrupt_last_durable_receipt_for_test(
        &mut self,
        replacement: DurableBodyReceipt,
    ) -> bool {
        if self.owners.len() < 2 {
            return false;
        }
        let Some(key) = self.owners.last_key_value().map(|(&key, _)| key) else {
            return false;
        };
        if (replacement.round(), replacement.subject()) == key {
            return false;
        }
        let owner = self
            .owners
            .get_mut(&key)
            .expect("last recovered Validate retry owner remains present");
        owner.durable_receipt = replacement;
        true
    }

    /// Decide whether one exact validated marker remains owned by a Ready
    /// lifecycle Validate row. A key collision with different durable
    /// authority is corruption, not a non-match.
    pub(in crate::sumeragi) fn classify_and_bind_validated_marker(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        validated: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
    ) -> Result<bool, &'static str> {
        match self.owners.get_mut(&key) {
            None => Ok(false),
            Some(owner) => owner
                .bind_validated_marker(key, validated)
                .then_some(true)
                .ok_or("Ready Validate marker changed its complete recovered authority"),
        }
    }

    /// Consume every owner into one atomic executor installation.
    pub(in crate::sumeragi) fn install_into_executor<
        R: crate::sumeragi::v2_effects::EffectRuntime,
    >(
        self,
        executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<R>,
    ) -> Result<(), crate::sumeragi::v2_effects::EffectExecutorError> {
        let mut installation = executor.prepare_recovered_durable_validate_retry_install()?;
        for owner in self.owners.into_values() {
            match owner.owner_class {
                ColdValidateRetryOwnerClassV1::AdmissionCensus => installation.absorb(owner)?,
                ColdValidateRetryOwnerClassV1::PublishedStoreSuccessor => {
                    // Its validated marker was already classified above. The
                    // launch marker pass installs the mutually exclusive
                    // direct Store-successor owner before clocks are armed.
                }
            }
        }
        installation.commit()
    }

    /// Construct the empty complete census for unit tests which open no
    /// lifecycle registry. Production has no empty/subset constructor.
    #[cfg(test)]
    pub(in crate::sumeragi) fn empty_for_test() -> Self {
        Self {
            owners: BTreeMap::new(),
        }
    }

    /// Carry one exact admission owner across a synthetic volatile completion.
    #[cfg(test)]
    fn from_admission_owner_for_test(owner: RecoveredDurableValidateRetryOwnerV1) -> Self {
        assert_eq!(
            owner.owner_class,
            ColdValidateRetryOwnerClassV1::AdmissionCensus
        );
        Self {
            owners: BTreeMap::from([(owner.key(), owner)]),
        }
    }
}

/// Closed failure while joining cold Validate retry authority at launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredDurableValidateRetryOwnerErrorV1 {
    /// The runtime Decision is outside the recovered height or is malformed.
    InvalidDecision,
    /// More than one recovered Validate carrier names one logical body owner.
    MultipleCarriers,
    /// The matching row, registry carrier, or replay authority is not exact.
    InvalidCarrier,
}

impl DurableValidateBody {
    fn matches_recovered_decision_installed_seal(
        &self,
        digest: LifecycleDigest,
        seal: &super::wal_recovery::RecoveredDecisionValidateInstalledSealV1,
    ) -> bool {
        self.validates(digest)
            && seal.exactly_matches_carrier(
                self.address,
                digest,
                &self.effect,
                &self.pending,
                &self.durable_receipt,
                self.expected_manifest_hash,
                &self.replay_evidence,
            )
    }
    /// Reconstruct one standalone local-body or signed-Proposal Validate row.
    pub(super) fn from_recovered_standalone_validate(
        owner: OwnerId,
        ordinal: u128,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        durable_receipt: DurableBodyReceipt,
        replay_evidence: DurableValidateReplayEvidenceV1,
        verified: &VerifiedHeightContext,
    ) -> Result<(Self, CandidateAdmission), ()> {
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Validate.capacity_class(), 0);
        let Some(address) = ConcreteWorkAddress::new(owner, ordinal, slot) else {
            return Err(());
        };
        let digest = digest_from_hash(pending.exact_effect_identity());
        let expected_manifest_hash = durable_receipt.manifest_hash();
        let carrier = Self {
            address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        if !carrier.validates(digest) {
            return Err(());
        }
        let candidate = carrier.project_candidate(verified).map_err(|_| ())?;
        Ok((carrier, candidate))
    }
    /// Reconstruct the exact durable Validate child of one authenticated Store.
    pub(super) fn from_recovered_certified_store(
        store: DurableStoreBody,
        verified: &VerifiedHeightContext,
        ordinal: u128,
    ) -> Result<
        (
            Self,
            CandidateAdmission,
            CertifiedBodyPipelineColdReplayStepV1,
        ),
        (),
    > {
        let (tag, round, subject) = match &store.effect {
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            } => (*tag, *round, *subject),
            _ => return Err(()),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let Some(pending) = store
            .pending
            .project_store_validate_successor(&store_effect, &effect)
        else {
            return Err(());
        };
        let Some(replay_evidence) = store.replay_evidence.project_validate(
            &store_effect,
            &store.durable_receipt,
            &effect,
            &pending,
        ) else {
            return Err(());
        };
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Validate.capacity_class(), 0);
        let Some(address) = ConcreteWorkAddress::new(store.address.owner, ordinal, slot) else {
            return Err(());
        };
        let digest = digest_from_hash(pending.exact_effect_identity());
        let durable_receipt = store.durable_receipt.clone();
        let carrier = Self {
            address,
            effect: effect.clone(),
            pending,
            durable_receipt,
            expected_manifest_hash: store.expected_manifest_hash,
            replay_evidence: DurableValidateReplayEvidenceV1::certified(replay_evidence),
        };
        if !carrier.validates(digest) {
            return Err(());
        }
        let Ok(candidate) = carrier.project_candidate(verified) else {
            return Err(());
        };
        let Some(replay) = CertifiedBodyPipelineColdReplayStepV1::body_stored(
            store.address.ordinal,
            tag,
            store.durable_receipt,
            effect,
        ) else {
            return Err(());
        };
        Ok((carrier, candidate, replay))
    }

    pub(super) fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        let AdapterEffect::ValidateBody { round, subject, .. } = &self.effect else {
            return false;
        };
        ConcreteWorkAddress::new(self.address.owner, self.address.ordinal, self.address.slot)
            == Some(self.address)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            && self.pending.exactly_binds_adapter_effect(&self.effect)
            && installed_digest == digest_from_hash(self.pending.exact_effect_identity())
            && self.durable_receipt.context_id() == round.context_id
            && self.durable_receipt.round() == *round
            && self.durable_receipt.subject() == *subject
            && self.durable_receipt.manifest_hash() == self.expected_manifest_hash
            && self.replay_evidence.exactly_matches_validate_pending(
                &self.effect,
                &self.durable_receipt,
                &self.pending,
            )
    }
    pub(super) const fn address(&self) -> ConcreteWorkAddress {
        self.address
    }
    pub(super) fn ready_digest(&self) -> LifecycleDigest {
        digest_from_hash(self.pending.exact_effect_identity())
    }
    fn matches_recovered_record(
        &self,
        active_context: LifecycleContext,
        record: &LifecycleRecord,
        metadata: &DurableRecordMetadata,
        installed_digest: LifecycleDigest,
    ) -> bool {
        self.validates(installed_digest)
            && self.replay_evidence.exactly_matches_recovered_record(
                active_context,
                record,
                metadata,
                installed_digest,
                &self.effect,
                &self.durable_receipt,
                &self.pending,
            )
    }
    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.replay_evidence.project_installed_validate_candidate(
            InstalledBodyCandidateProjectionPermit::new(),
            verified,
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }
}
/// Same-address closed result of one completed durable body validation.
///
/// The exact incumbent carrier is moved into this value rather than cloned or
/// reconstructed. Its original digest remains a separate authority coordinate
/// while the installed row uses the outcome-bound replacement digest.
#[derive(Debug)]
struct DurableValidateCompletion {
    address: ConcreteWorkAddress,
    incumbent: DurableValidateBody,
    incumbent_digest: LifecycleDigest,
    outcome: DurableBodyValidationOutcome,
}
impl DurableValidateCompletion {
    fn validates(&self, installed_digest: LifecycleDigest) -> bool {
        self.incumbent.address == self.address
            && self.incumbent.validates(self.incumbent_digest)
            && self.address.owner.causal_root()
                == super::CausalRoot::new(digest_from_hash(
                    self.incumbent.pending.causal_lifecycle_key(),
                ))
            && self
                .incumbent
                .pending
                .exactly_binds_adapter_effect(&self.incumbent.effect)
            && self.outcome.durable_body() == &self.incumbent.durable_receipt
            && self.incumbent.durable_receipt.manifest_hash()
                == self.incumbent.expected_manifest_hash
            && self.outcome.validated_receipt().is_none_or(|receipt| {
                validate_validated_receipt_authority(&self.incumbent, receipt).is_ok()
            })
            && durable_validate_completion_digest(
                self.incumbent_digest,
                self.incumbent.expected_manifest_hash,
                &self.outcome,
            ) == Some(installed_digest)
            && installed_digest != self.incumbent_digest
    }
}
// DURABLE_VALIDATE_COMPLETION_CARRIER_END
