#[allow(dead_code)]
impl<'a> PreparedCertifiedFetchExecution<'a> {
    /// Return the exact reducer tag and authenticated manifest accepted by the
    /// direct adapter preview. Both are derived from the installed completion;
    /// neither can be supplied independently by the caller.
    pub(super) fn adapter_preview_inputs(&self) -> (EventTag, &wire::PayloadManifest) {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared certified-Fetch completion remains installed");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            unreachable!("prepared certified-Fetch execution retains a closed completion")
        };
        completion
            .replay_evidence
            .adapter_preview_inputs(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
            )
            .expect("prepared certified-Fetch completion retains exact durable replay inputs")
    }
    /// Borrow the durable body proof retained by the exact completion.
    ///
    /// The receipt remains nested and non-decomposable; callers may use it only
    /// for the dedicated body-catalog equality check and canonical reload.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared certified-Fetch completion remains installed");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            unreachable!("prepared certified-Fetch execution retains a closed completion")
        };
        &completion.durable_receipt
    }
    /// Seal the ordinal-free pending binding for the exact Store effect emitted
    /// by the direct adapter preview.
    ///
    /// This pure projection checks the certified predecessor, exact tag/round/
    /// subject, inherited candidate statement, unchanged causal key, and a new
    /// physical effect identity. Neither success nor failure changes the
    /// installed completion.
    pub(super) fn seal_store_successor<'adapter>(
        self,
        adapter: crate::sumeragi::v2::PreparedCertifiedFetchStoreAdapterV1<'adapter>,
    ) -> Result<PreparedCertifiedFetchStoreSuccessor<'a, 'adapter>, CertifiedFetchExecutionError>
    {
        let successor = adapter.store_effect();
        let (
            store_effect,
            store_pending,
            store_digest,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared certified-Fetch completion remains installed");
            let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            if !completion.validates(work.digest) {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            }
            let Some(store_pending) = completion
                .incumbent_pending
                .project_certified_fetch_store_successor(&completion.incumbent_effect, successor)
            else {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            };
            if store_pending.causal_lifecycle_key()
                != completion.incumbent_pending.causal_lifecycle_key()
                || store_pending.candidate_statement()
                    != completion.incumbent_pending.candidate_statement()
                || store_pending.exact_effect_identity()
                    == completion.incumbent_pending.exact_effect_identity()
                || !store_pending.exactly_binds_adapter_effect(successor)
            {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            }
            let store_digest = digest_from_hash(store_pending.exact_effect_identity());
            let durable_body = completion.durable_receipt.clone();
            let Some(ready_projection) = completion.replay_evidence.project_durable_ready_fetch(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
            ) else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            let expected_manifest_hash = ready_projection.expected_manifest_hash();
            let Some(replay_evidence) = completion.replay_evidence.project_store(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
                successor,
            ) else {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            };
            (
                successor.clone(),
                store_pending,
                store_digest,
                durable_body,
                expected_manifest_hash,
                replay_evidence,
            )
        };
        Ok(PreparedCertifiedFetchStoreSuccessor {
            registry: self.registry,
            completion_address: self.address,
            store_effect,
            store_digest,
            store_pending,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
            adapter,
        })
    }
}
#[allow(dead_code)]
impl<'a> PreparedDurableStoreExecution<'a> {
    fn installed_work(&self) -> &ConcreteLifecycleWork {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Store carrier remains installed");
        work
    }
    /// Return the exact reducer coordinates accepted by the direct
    /// `BodyStored` adapter preview.
    pub(super) fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        match (&self.origin, &self.installed_work().kind) {
            (
                DurableStoreExecutionOriginV1::Certified,
                ConcreteLifecycleWorkKind::DurableStoreBody(store),
            ) => {
                let AdapterEffect::StoreBody {
                    tag,
                    round,
                    subject,
                } = &store.effect
                else {
                    unreachable!("prepared durable Store carrier retains its Store effect")
                };
                (*tag, *round, *subject)
            }
            (
                DurableStoreExecutionOriginV1::RecoveredDecision(_),
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store),
            ) => store
                .store
                .adapter_preview_inputs()
                .expect("prepared recovered Decision Store retains its Store effect"),
            _ => unreachable!("prepared durable Store execution retains its closed carrier"),
        }
    }
    /// Borrow the exact post-fsync body receipt retained by the Store carrier.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        match (&self.origin, &self.installed_work().kind) {
            (
                DurableStoreExecutionOriginV1::Certified,
                ConcreteLifecycleWorkKind::DurableStoreBody(store),
            ) => &store.durable_receipt,
            (
                DurableStoreExecutionOriginV1::RecoveredDecision(_),
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store),
            ) => store.store.durable_body_receipt(),
            _ => unreachable!("prepared durable Store execution retains its closed carrier"),
        }
    }
    /// Return the manifest hash transferred independently from the parent response.
    pub(super) fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        match (&self.origin, &self.installed_work().kind) {
            (
                DurableStoreExecutionOriginV1::Certified,
                ConcreteLifecycleWorkKind::DurableStoreBody(store),
            ) => store.expected_manifest_hash,
            (
                DurableStoreExecutionOriginV1::RecoveredDecision(_),
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store),
            ) => store.store.expected_manifest_hash(),
            _ => unreachable!("prepared durable Store execution retains its closed carrier"),
        }
    }
    /// Seal the ordinal-free pending binding for the exact Validate effect
    /// emitted by the direct `BodyStored` adapter preview.
    ///
    /// The Store's full inherited candidate statement and causal root must be
    /// unchanged, while the concrete effect identity must be replaced by the
    /// exact Validate identity. Neither success nor failure changes the Store
    /// row retained under the exclusive registry borrow.
    pub(super) fn seal_validate_successor<'adapter>(
        self,
        adapter: crate::sumeragi::v2::PreparedDurableStoreValidateAdapterV1<'adapter>,
    ) -> Result<PreparedDurableStoreValidateSuccessor<'a, 'adapter>, DurableStoreExecutionError>
    {
        let successor = adapter.validate_effect();
        let Self {
            registry,
            address,
            origin,
        } = self;
        let (
            parent,
            validate_effect,
            validate_pending,
            validate_digest,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
        ) = {
            let work = registry
                .entries
                .get(&address)
                .expect("prepared durable Store carrier remains installed");
            let (parent, validate_pending, durable_body, expected_manifest_hash, replay_evidence) =
                match (origin, &work.kind) {
                    (
                        DurableStoreExecutionOriginV1::Certified,
                        ConcreteLifecycleWorkKind::DurableStoreBody(store),
                    ) => {
                        if !store.validates(work.digest) {
                            return Err(DurableStoreExecutionError::InvalidStoreShape);
                        }
                        let Some(validate_pending) = store
                            .pending
                            .project_store_validate_successor(&store.effect, successor)
                        else {
                            return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
                        };
                        if validate_pending.causal_lifecycle_key()
                            != store.pending.causal_lifecycle_key()
                            || validate_pending.candidate_statement()
                                != store.pending.candidate_statement()
                            || validate_pending.exact_effect_identity()
                                == store.pending.exact_effect_identity()
                        {
                            return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
                        }
                        let Some(replay_evidence) = store.replay_evidence.project_validate(
                            &store.effect,
                            &store.durable_receipt,
                            successor,
                            &validate_pending,
                        ) else {
                            return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
                        };
                        (
                            DurableStoreValidateParentV1::Certified,
                            validate_pending,
                            store.durable_receipt.clone(),
                            store.expected_manifest_hash,
                            DurableValidateReplayEvidenceV1::certified(replay_evidence),
                        )
                    }
                    (
                        DurableStoreExecutionOriginV1::RecoveredDecision(authority),
                        ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore(store),
                    ) => {
                        if !store.validates_at(address, work.digest) {
                            return Err(DurableStoreExecutionError::InvalidStoreShape);
                        }
                        let Some((validate_pending, replay_evidence)) =
                            authority.project_validate_successor(&store.store, successor)
                        else {
                            return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
                        };
                        (
                            DurableStoreValidateParentV1::RecoveredDecision,
                            validate_pending,
                            store.store.durable_body_receipt().clone(),
                            store.store.expected_manifest_hash(),
                            replay_evidence,
                        )
                    }
                    _ => return Err(DurableStoreExecutionError::InvalidStoreShape),
                };
            if super::CausalRoot::new(digest_from_hash(validate_pending.causal_lifecycle_key()))
                != address.owner.causal_root()
                || !validate_pending.exactly_binds_adapter_effect(successor)
            {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            let validate_digest = digest_from_hash(validate_pending.exact_effect_identity());
            if validate_digest == work.digest {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            if durable_body.manifest_hash() != expected_manifest_hash
                || !replay_evidence.exactly_matches_validate_pending(
                    successor,
                    &durable_body,
                    &validate_pending,
                )
            {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            (
                parent,
                successor.clone(),
                validate_pending,
                validate_digest,
                durable_body,
                expected_manifest_hash,
                replay_evidence,
            )
        };
        Ok(PreparedDurableStoreValidateSuccessor {
            registry,
            store_address: address,
            parent,
            validate_effect,
            validate_digest,
            validate_pending,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
            adapter,
        })
    }
}
