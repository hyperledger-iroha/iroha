impl PendingRuntimeEffectBinding {
    /// Reconstruct the unique ordinal-free owner of a standalone durable Validate.
    ///
    /// The replay module mints the permit only after the canonical LocalBody,
    /// signed-Proposal, or authenticated-genesis authority has joined the exact
    /// BodyFrame. Certified genesis retains its exact QC statement so the
    /// reconstructed Validate key matches the live body-pipeline lineage.
    pub(in crate::sumeragi) fn from_durable_standalone_validate(
        _permit: DurableStandaloneValidatePendingMintPermit,
        causal_lifecycle_key: iroha_crypto::Hash,
        effect: &AdapterEffect,
        certified_predecessor: Option<&wire::QuorumCertificate>,
    ) -> Option<Self> {
        if !matches!(effect, AdapterEffect::ValidateBody { .. }) {
            return None;
        }
        let effect_kind = production_adapter_effect_kind(effect);
        let effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        let inherited = certified_predecessor.map(|certificate| {
            RuntimeCandidateSemanticStatement::new(
                certificate.round,
                certificate.proposal_round,
                Some(certificate.subject),
                Some(certificate.phase),
                Some(certificate.execution_commitment),
            )
        });
        let candidate =
            production_adapter_effect_candidate_binding(effect, inherited.as_ref()).ok()??;
        let candidate_semantic_identity = Some(runtime_effect_candidate_semantic_hash(
            candidate.kind,
            &candidate.semantic_identity,
        ));
        let projection_hash = pending_runtime_effect_binding_projection_hash(
            &causal_lifecycle_key,
            effect_kind,
            &effect_identity,
            candidate.kind,
            candidate.statement,
            candidate_semantic_identity.as_ref(),
        );
        let pending = Self {
            causal_lifecycle_key,
            effect_kind,
            effect_identity,
            candidate_kind: candidate.kind,
            candidate_statement: candidate.statement,
            candidate_semantic_identity,
            projection_hash,
        };
        pending.validate_exact(effect).then_some(pending)
    }

    /// Reconstruct the unique ordinal-free owner of one authenticated cold output.
    ///
    /// Output effects do not carry candidate statements in the serialized runtime
    /// binding. The one-shot permit is minted only while the complete signed or
    /// rejection replay envelope is authenticated against the frozen height and,
    /// for invalid-body reports, its exact rejected marker.
    pub(in crate::sumeragi) fn from_durable_lifecycle_output(
        _permit: DurableLifecycleOutputPendingMintPermit,
        causal_lifecycle_key: iroha_crypto::Hash,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        if !matches!(
            effect,
            AdapterEffect::Broadcast(_)
                | AdapterEffect::ReportEquivocation { .. }
                | AdapterEffect::ReportInvalidCertifiedBody { .. }
        ) {
            return None;
        }
        let effect_kind = production_adapter_effect_kind(effect);
        let effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        if production_adapter_effect_candidate_binding(effect, None)
            .ok()?
            .is_some()
        {
            return None;
        }
        let projection_hash = pending_runtime_effect_binding_projection_hash(
            &causal_lifecycle_key,
            effect_kind,
            &effect_identity,
            RUNTIME_CANDIDATE_KIND_NONE,
            None,
            None,
        );
        let pending = Self {
            causal_lifecycle_key,
            effect_kind,
            effect_identity,
            candidate_kind: RUNTIME_CANDIDATE_KIND_NONE,
            candidate_statement: None,
            candidate_semantic_identity: None,
            projection_hash,
        };
        pending.validate_exact(effect).then_some(pending)
    }
}
