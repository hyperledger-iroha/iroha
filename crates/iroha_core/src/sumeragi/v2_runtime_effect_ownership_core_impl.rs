impl RuntimeEffectOwnerAssignment {
    fn inherit(owner: RuntimeLifecycleOwner) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Inherit,
        }
    }

    fn fresh_root(owner: RuntimeLifecycleOwner, kind: RuntimeFreshRootKind) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Fresh(kind),
        }
    }

    #[cfg(test)]
    pub(crate) fn fresh_for_test(tag: EventTag, lifecycle_ordinal: u128) -> Self {
        Self::fresh_for_test_with_semantic_identity(tag, lifecycle_ordinal, b"test-runtime-effect")
    }

    #[cfg(test)]
    pub(crate) fn fresh_for_test_with_semantic_identity(
        tag: EventTag,
        lifecycle_ordinal: u128,
        semantic_identity: &[u8],
    ) -> Self {
        let kind = RuntimeFreshRootKind::StartupRecovery;
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            tag,
            CommandClass::Progress,
            kind,
            semantic_identity,
        );
        Self::fresh_root(
            RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                .expect("fresh test owner binds its first lifecycle ordinal"),
            kind,
        )
    }
}

impl RuntimeEffectOwnership {
    /// Select a fresh test lifecycle for atomic batch binding.
    #[cfg(test)]
    pub(crate) fn fresh_for_test(
        tag: EventTag,
        lifecycle_ordinal: u128,
    ) -> RuntimeEffectOwnerAssignment {
        RuntimeEffectOwnerAssignment::fresh_for_test(tag, lifecycle_ordinal)
    }

    /// Select a semantically named fresh test lifecycle for atomic batch binding.
    #[cfg(test)]
    pub(crate) fn fresh_for_test_with_semantic_identity(
        tag: EventTag,
        lifecycle_ordinal: u128,
        semantic_identity: &[u8],
    ) -> RuntimeEffectOwnerAssignment {
        RuntimeEffectOwnerAssignment::fresh_for_test_with_semantic_identity(
            tag,
            lifecycle_ordinal,
            semantic_identity,
        )
    }

    fn validate_exact(&self) -> bool {
        self.owner.validate_exact() && self.binding.validate_exact(&self.owner, self.causality)
    }
    /// Return whether this non-forgeable sidecar names one exact production
    /// adapter effect, including all of the effect's concrete coordinates.
    pub(crate) fn exactly_binds_adapter_effect(&self, effect: &AdapterEffect) -> bool {
        let binding = &self.binding;
        let effect_kind = production_adapter_effect_kind(effect);
        self.validate_exact()
            && binding.effect_kind == effect_kind
            && binding.effect_identity
                == runtime_effect_identity_hash(
                    effect_kind,
                    &production_adapter_effect_semantic_identity(effect),
                )
    }
    /// Mint the ordinal-free lifecycle-admission binding for this exact effect.
    ///
    /// The operation fails closed when the supplied effect differs from the
    /// mandatory binding; there is no unbound or best-effort conversion path.
    pub(crate) fn exact_pending_adapter_effect_binding(
        &self,
        effect: &AdapterEffect,
    ) -> Result<PendingRuntimeEffectBinding, EnqueueError> {
        if !self.exactly_binds_adapter_effect(effect) {
            return Err(EnqueueError::FailClosed);
        }
        let binding = &self.binding;
        let causal_lifecycle_key = self.owner.causal_origin.lifecycle_key;
        let projection_hash = pending_runtime_effect_binding_projection_hash(
            &causal_lifecycle_key,
            binding.effect_kind,
            &binding.effect_identity,
            binding.candidate_kind,
            binding.candidate_statement,
            binding.candidate_semantic_identity.as_ref(),
        );
        let pending = PendingRuntimeEffectBinding {
            causal_lifecycle_key,
            effect_kind: binding.effect_kind,
            effect_identity: binding.effect_identity,
            candidate_kind: binding.candidate_kind,
            candidate_statement: binding.candidate_statement,
            candidate_semantic_identity: binding.candidate_semantic_identity,
            projection_hash,
        };
        pending
            .validate_exact(effect)
            .then_some(pending)
            .ok_or(EnqueueError::FailClosed)
    }
    /// Clone the opaque authenticated-Proposal replay envelope only for its exact Fetch.
    pub(in crate::sumeragi) fn exact_remote_proposal_fetch_replay(
        &self,
        effect: &AdapterEffect,
    ) -> Option<RemoteProposalFetchReplayEvidenceV1> {
        let replay = self.remote_proposal_fetch_replay.as_ref()?;
        let pending = self.exact_pending_adapter_effect_binding(effect).ok()?;
        replay
            .exactly_matches_fetch_pending(effect, &pending)
            .then(|| replay.clone())
    }
    /// Attach genuine authenticated-Proposal replay evidence to one test Fetch owner.
    ///
    /// This uses the same frozen fair-ingress projection and private production
    /// mint as runtime dispatch; it does not construct replay source parts.
    #[cfg(test)]
    pub(crate) fn bind_authenticated_remote_proposal_replay_for_test(
        &mut self,
        proposal: wire::Proposal,
        effect: &AdapterEffect,
    ) -> bool {
        if self.remote_proposal_fetch_replay.is_some() {
            return false;
        }
        let authenticated = AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal),
        ));
        let message = authenticated.wire_envelope_for_test();
        let mut admitted = super::fair_v2_ingress_admit_for_test(
            super::InboundBlockMessage::from_authenticated_peer(
                super::message::BlockMessage::V2(message.clone()),
                super::authenticated_peer_for_test(),
            ),
        );
        let Some(fair_ingress) = admitted.take_ingress_ownership() else {
            return false;
        };
        let Some(ingress) =
            RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, fair_ingress)
        else {
            return false;
        };
        let Some(origin) = AuthenticatedRemoteProposalDispatchOrigin::new(authenticated, ingress)
        else {
            return false;
        };
        let Ok(pending) = self.exact_pending_adapter_effect_binding(effect) else {
            return false;
        };
        let Some(replay) = origin.bind_exact_fetch(effect, pending) else {
            return false;
        };
        self.remote_proposal_fetch_replay = Some(replay);
        true
    }
    /// Return whether this exact predecessor capability authorizes one body
    /// pipeline successor. The mapping is deliberately closed: matching body
    /// coordinates alone never authorize a lifecycle-owner handoff.
    fn exactly_authorizes_body_pipeline_successor(
        &self,
        predecessor: &AdapterEffect,
        tag: EventTag,
        successor: &BodyPipelineCompletionEvidence,
    ) -> bool {
        if !self.exactly_binds_adapter_effect(predecessor) {
            return false;
        }
        match (predecessor, successor) {
            (
                AdapterEffect::StoreBody {
                    tag: predecessor_tag,
                    round,
                    subject,
                },
                BodyPipelineCompletionEvidence::BodyStored {
                    round: successor_round,
                    subject: successor_subject,
                    receipt,
                },
            ) => {
                *predecessor_tag == tag
                    && *round == *successor_round
                    && *subject == *successor_subject
                    && receipt.round() == *round
                    && receipt.subject() == *subject
            }
            (
                AdapterEffect::ValidateBody {
                    tag: predecessor_tag,
                    round,
                    subject,
                },
                BodyPipelineCompletionEvidence::LocalProposalReady {
                    manifest,
                    durable_receipt,
                    validated_receipt,
                },
            ) => {
                *predecessor_tag == tag
                    && *round == manifest.round
                    && *subject == manifest.subject
                    && durable_receipt.round() == *round
                    && durable_receipt.subject() == *subject
                    && durable_receipt.manifest_hash() == iroha_crypto::HashOf::new(manifest)
                    && validated_receipt.durable() == durable_receipt
            }
            _ => false,
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn new_bound(
        owner: RuntimeLifecycleOwner,
        causality: RuntimeEffectCausality,
        effect_kind: u8,
        effect_semantic_identity: &[u8],
        candidate: Option<&RuntimeEffectCandidateSemantic>,
        effect_position: u8,
        effect_count: u8,
        candidate_position: u8,
        candidate_count: u8,
    ) -> Result<Self, EnqueueError> {
        let parent = matches!(causality, RuntimeEffectCausality::Inherit).then_some(&owner);
        let binding = RuntimeEffectCandidateBinding::new(
            &owner,
            causality,
            parent,
            effect_kind,
            effect_semantic_identity,
            candidate,
            effect_position,
            effect_count,
            candidate_position,
            candidate_count,
        )?;
        let ownership = Self {
            owner,
            causality,
            binding,
            remote_proposal_fetch_replay: None,
        };
        ownership
            .validate_exact()
            .then_some(ownership)
            .ok_or(EnqueueError::FailClosed)
    }

    fn binding(&self) -> &RuntimeEffectCandidateBinding {
        &self.binding
    }
    #[cfg(test)]
    pub(crate) fn candidate_identity(&self) -> Option<iroha_crypto::Hash> {
        self.binding.candidate_identity
    }
    /// Route-neutral semantic lifecycle used by the single-owner admission gate.
    pub(crate) fn candidate_semantic_identity(&self) -> Option<iroha_crypto::Hash> {
        self.binding.candidate_semantic_identity
    }
    /// Typed semantic statement retained through causal completion handoffs.
    fn candidate_semantic_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
        self.binding.candidate_statement
    }
    /// Return whether this exact effect binding carries one durable Decision's
    /// complete Commit authority statement.
    pub(crate) fn binds_durable_decision_authority(
        &self,
        decision_round: wire::ConsensusRound,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> bool {
        self.validate_exact()
            && self.candidate_semantic_statement()
                == Some(RuntimeCandidateSemanticStatement::new(
                    decision_round,
                    proposal_round,
                    Some(subject),
                    Some(wire::GlobalPhase::Commit),
                    Some(execution_commitment),
                ))
    }
    fn binds_exact_fetch_body_manifest(&self, manifest: &wire::PayloadManifest) -> bool {
        let binding = &self.binding;
        self.validate_exact()
            && binding.effect_kind == RUNTIME_EFFECT_KIND_FETCH_BODY
            && binding.candidate_kind == RUNTIME_CANDIDATE_KIND_FETCH_BODY
            && binding
                .candidate_statement
                .is_some_and(|statement| statement.binds_exact_body_manifest(manifest))
    }
    fn binds_body_pipeline_completion_predecessor(
        &self,
        completion: &BodyPipelineCompletionEvidence,
    ) -> bool {
        let binding = &self.binding;
        let (effect_kind, candidate_kind, round, subject) = match completion {
            BodyPipelineCompletionEvidence::LocalProposalReady { manifest, .. } => (
                RUNTIME_EFFECT_KIND_VALIDATE_BODY,
                RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
                manifest.round,
                manifest.subject,
            ),
            BodyPipelineCompletionEvidence::BodyAvailable { manifest } => (
                RUNTIME_EFFECT_KIND_FETCH_BODY,
                RUNTIME_CANDIDATE_KIND_FETCH_BODY,
                manifest.round,
                manifest.subject,
            ),
            BodyPipelineCompletionEvidence::BodyStored { round, subject, .. } => (
                RUNTIME_EFFECT_KIND_STORE_BODY,
                RUNTIME_CANDIDATE_KIND_STORE_BODY,
                *round,
                *subject,
            ),
        };
        self.validate_exact()
            && binding.effect_kind == effect_kind
            && binding.candidate_kind == candidate_kind
            && binding.candidate_statement.is_some_and(|statement| {
                statement.validate_exact()
                    && statement.context_id == round.context_id
                    && statement.proposal_round == round
                    && statement.subject == Some(subject)
            })
            && match completion {
                BodyPipelineCompletionEvidence::BodyAvailable { manifest } => {
                    self.binds_exact_fetch_body_manifest(manifest)
                }
                BodyPipelineCompletionEvidence::LocalProposalReady { .. }
                | BodyPipelineCompletionEvidence::BodyStored { .. } => true,
            }
    }
    /// Immutable owner carried into an asynchronous task or completion.
    pub(crate) const fn owner(&self) -> &RuntimeLifecycleOwner {
        &self.owner
    }
    /// Frozen inherit/fresh classification. Retries and rebinds retain it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn causality(&self) -> RuntimeEffectCausality {
        self.causality
    }
}
