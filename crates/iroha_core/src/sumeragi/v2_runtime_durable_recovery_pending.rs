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

    /// Project the complete inert binding used to coalesce retransmits of one
    /// storage-authenticated cold Validate carrier.
    ///
    /// The lifecycle registry calls this only while the exact recovered
    /// carrier, LedgerV1 row, and replay authority remain joined. The returned
    /// value is move-only and retains the complete pending projection; callers
    /// cannot reconstruct it from decoded coordinates or an effect digest.
    pub(in crate::sumeragi) fn project_recovered_durable_validate_retry_binding(
        &self,
        effect: &AdapterEffect,
        expected_decision: Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    ) -> Option<RecoveredDurableValidateRetryBindingV1> {
        if !matches!(effect, AdapterEffect::ValidateBody { .. }) || !self.validate_exact(effect) {
            return None;
        }
        let pending = Self {
            causal_lifecycle_key: self.causal_lifecycle_key.clone(),
            effect_kind: self.effect_kind,
            effect_identity: self.effect_identity.clone(),
            candidate_kind: self.candidate_kind,
            candidate_statement: self.candidate_statement,
            candidate_semantic_identity: self.candidate_semantic_identity.clone(),
            projection_hash: self.projection_hash.clone(),
        };
        let incumbent_statement = pending.candidate_statement?;
        let expected_retry_statement = match expected_decision {
            Some((decision_round, proposal_round, subject, execution_commitment)) => {
                let decision_statement = RuntimeCandidateSemanticStatement::new(
                    decision_round,
                    proposal_round,
                    Some(subject),
                    Some(wire::GlobalPhase::Commit),
                    Some(execution_commitment),
                );
                incumbent_statement.commit_refinement_to(decision_statement)?;
                decision_statement
            }
            None => incumbent_statement,
        };
        pending
            .validate_exact(effect)
            .then_some(RecoveredDurableValidateRetryBindingV1 {
                pending,
                expected_retry_statement,
                authority_ceiling_commitment: expected_retry_statement.execution_commitment(),
            })
    }
}

/// Move-only exact pending projection for one recovered durable Validate.
///
/// This is an inert retry fingerprint, not executable ownership. It can only
/// classify an independently authenticated runtime retransmit as a stutter;
/// it cannot enter lifecycle admission or dispatch a second Validate worker.
#[must_use = "a recovered durable Validate retry binding must remain attached to its registry owner"]
#[derive(Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDurableValidateRetryBindingV1 {
    pending: PendingRuntimeEffectBinding,
    expected_retry_statement: RuntimeCandidateSemanticStatement,
    authority_ceiling_commitment: Option<wire::ExecutionCommitment>,
}

/// Monotonic process-local frontier retained by one recovered retry seal.
///
/// The registry owner remains immutable. This frontier records only the
/// highest accepted Validate tag and body-stage authority, plus the first
/// authenticated commitment ceiling learned from Decision replay, a durable
/// marker, or an accepted quorum-authority refinement.
#[must_use = "a recovered Validate retry frontier must stay attached to its seal"]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDurableValidateRetryFrontierV1 {
    effect: AdapterEffect,
    statement: RuntimeCandidateSemanticStatement,
    authority_ceiling_commitment: Option<wire::ExecutionCommitment>,
}

impl RecoveredDurableValidateRetryFrontierV1 {
    /// Return only the retained Validate tag to focused tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn effect_tag_for_test(&self) -> EventTag {
        let AdapterEffect::ValidateBody { tag, .. } = &self.effect else {
            unreachable!("recovered retry frontier retains one Validate effect")
        };
        *tag
    }

    /// Return only the retained quorum phase to focused tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn phase_for_test(&self) -> Option<wire::GlobalPhase> {
        self.statement.phase()
    }

    /// Return only the sealed durable commitment ceiling to focused tests.
    #[cfg(test)]
    pub(in crate::sumeragi) const fn commitment_ceiling_for_test(
        &self,
    ) -> Option<wire::ExecutionCommitment> {
        self.authority_ceiling_commitment
    }

    /// Project a later durable marker/Decision commitment without granting a
    /// new quorum phase. Callers commit the replacement only after their own
    /// catalogs and service preflights have succeeded.
    pub(in crate::sumeragi) fn project_commitment_ceiling(
        &self,
        commitment: wire::ExecutionCommitment,
    ) -> Result<Self, &'static str> {
        if self
            .authority_ceiling_commitment
            .is_some_and(|expected| expected != commitment)
        {
            return Err("recovered Validate retry frontier changed its durable commitment");
        }
        let mut projected = self.clone();
        projected.authority_ceiling_commitment = Some(commitment);
        Ok(projected)
    }
}

impl RecoveredDurableValidateRetryBindingV1 {
    /// Return only the immutable recovered causal root to focused tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn causal_lifecycle_key_for_test(&self) -> iroha_crypto::Hash {
        *self.pending.causal_lifecycle_key()
    }

    /// Bind one exact durable marker commitment without inventing quorum phase.
    pub(in crate::sumeragi) fn bind_validated_marker_commitment(
        &mut self,
        commitment: wire::ExecutionCommitment,
    ) -> bool {
        if self
            .authority_ceiling_commitment
            .is_some_and(|expected| expected != commitment)
        {
            return false;
        }
        self.authority_ceiling_commitment = Some(commitment);
        true
    }

    /// Recheck one marker after the complete startup oracle has been sealed.
    pub(in crate::sumeragi) fn exactly_matches_validated_marker_commitment(
        &self,
        commitment: wire::ExecutionCommitment,
    ) -> bool {
        self.authority_ceiling_commitment == Some(commitment)
    }

    /// Construct the sole initial frontier after marker classification.
    pub(in crate::sumeragi) fn initial_frontier(
        &self,
        incumbent_effect: &AdapterEffect,
    ) -> Option<RecoveredDurableValidateRetryFrontierV1> {
        let statement = self.pending.candidate_statement?;
        self.pending.validate_exact(incumbent_effect).then_some(
            RecoveredDurableValidateRetryFrontierV1 {
                effect: incumbent_effect.clone(),
                statement,
                authority_ceiling_commitment: self.authority_ceiling_commitment,
            },
        )
    }

    /// Recheck one independently authenticated retransmit and advance only the
    /// seal's monotonic tag/authority frontier.
    pub(in crate::sumeragi) fn project_retry(
        &self,
        recovered_effect: &AdapterEffect,
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
        let (
            AdapterEffect::ValidateBody {
                tag: recovered_tag,
                round: recovered_round,
                subject: recovered_subject,
            },
            AdapterEffect::ValidateBody {
                tag: frontier_tag,
                round: frontier_round,
                subject: frontier_subject,
            },
            AdapterEffect::ValidateBody {
                tag: incoming_tag,
                round: incoming_round,
                subject: incoming_subject,
            },
        ) = (recovered_effect, &frontier.effect, effect)
        else {
            return Err(
                "recovered durable Validate retry received another effect stage".to_owned(),
            );
        };
        if (frontier_tag != recovered_tag && !frontier_tag.strictly_advances(*recovered_tag))
            || incoming_tag.height() != frontier_tag.height()
            || frontier_round != recovered_round
            || frontier_subject != recovered_subject
            || incoming_round != recovered_round
            || incoming_subject != recovered_subject
            || !self.pending.validate_exact(recovered_effect)
            || !incoming.validate_exact()
        {
            return Err(
                "recovered durable Validate retry changed its body, tag, or exact binding"
                    .to_owned(),
            );
        }
        let recovered_statement = self.pending.candidate_statement.ok_or_else(|| {
            "recovered durable Validate retry omitted its retained candidate statement".to_owned()
        })?;
        if (self.expected_retry_statement != recovered_statement
            && recovered_statement
                .commit_refinement_to(self.expected_retry_statement)
                .is_none())
            || !matches!(
                recovered_statement.body_stage_authority_relation_to(frontier.statement),
                Some(RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Upgrade)
            )
            || self
                .authority_ceiling_commitment
                .is_some_and(|expected| frontier.authority_ceiling_commitment != Some(expected))
            || frontier
                .statement
                .execution_commitment()
                .is_some_and(|commitment| frontier.authority_ceiling_commitment != Some(commitment))
        {
            return Err(
                "recovered durable Validate retry lost its sealed authority refinement".to_owned(),
            );
        }
        let incoming_binding = &incoming.binding;
        let incoming_statement = incoming_binding.candidate_statement.ok_or_else(|| {
            "recovered durable Validate retry omitted its incoming candidate statement".to_owned()
        })?;
        let frontier_relation = frontier
            .statement
            .body_stage_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "recovered durable Validate retry changed its body or authority commitment"
                    .to_owned()
            })?;
        let incoming_commitment = incoming_statement.execution_commitment();
        if frontier
            .authority_ceiling_commitment
            .zip(incoming_commitment)
            .is_some_and(|(expected, incoming)| expected != incoming)
        {
            return Err(
                "recovered durable Validate retry changed its sealed commitment ceiling".to_owned(),
            );
        }
        let retained_statement = match frontier_relation {
            RuntimeFetchAuthorityRelation::Upgrade => incoming_statement,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
                frontier.statement
            }
        };
        let retained_semantic_identity = runtime_effect_candidate_semantic_hash(
            RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
            &retained_statement.semantic_identity(),
        );
        let incoming_semantic_identity = runtime_effect_candidate_semantic_hash(
            RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
            &incoming_statement.semantic_identity(),
        );
        if self.pending.candidate_kind != RUNTIME_CANDIDATE_KIND_VALIDATE_BODY
            || incoming_binding.candidate_kind != RUNTIME_CANDIDATE_KIND_VALIDATE_BODY
            || incoming_binding.candidate_semantic_identity != Some(incoming_semantic_identity)
        {
            return Err(
                "recovered durable Validate retry changed its exact candidate authority".to_owned(),
            );
        }
        let effect_kind = production_adapter_effect_kind(effect);
        let effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        if incoming_binding.effect_kind != effect_kind
            || incoming_binding.effect_identity != effect_identity
        {
            return Err(
                "recovered durable Validate retry changed its exact incoming effect".to_owned(),
            );
        }
        let candidate =
            production_adapter_effect_candidate_binding(effect, Some(&incoming_statement))?
                .ok_or_else(|| {
                    "recovered durable Validate retry projected a non-candidate effect".to_owned()
                })?;
        let semantic_identity =
            runtime_effect_candidate_semantic_hash(candidate.kind, &candidate.semantic_identity);
        if candidate.kind != self.pending.candidate_kind
            || candidate.statement != Some(incoming_statement)
            || semantic_identity != incoming_semantic_identity
        {
            return Err(
                "recovered durable Validate retry disagreed with its retained candidate".to_owned(),
            );
        }
        let retained_candidate =
            production_adapter_effect_candidate_binding(effect, Some(&retained_statement))?
                .ok_or_else(|| {
                    "recovered durable Validate retry projected no retained candidate".to_owned()
                })?;
        if retained_candidate.kind != RUNTIME_CANDIDATE_KIND_VALIDATE_BODY
            || runtime_effect_candidate_semantic_hash(
                retained_candidate.kind,
                &retained_candidate.semantic_identity,
            ) != retained_semantic_identity
        {
            return Err(
                "recovered durable Validate retry changed its retained candidate authority"
                    .to_owned(),
            );
        }
        let ownership = RuntimeEffectOwnership::new_bound(
            incoming.owner.clone(),
            incoming.causality,
            production_adapter_effect_kind(effect),
            &production_adapter_effect_semantic_identity(effect),
            Some(&retained_candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| {
            "recovered durable Validate retry could not retain its authority lattice".to_owned()
        })?;
        Ok((
            RecoveredDurableValidateRetryFrontierV1 {
                effect: if incoming_tag.strictly_advances(*frontier_tag) {
                    effect.clone()
                } else {
                    frontier.effect.clone()
                },
                statement: retained_statement,
                authority_ceiling_commitment: frontier
                    .authority_ceiling_commitment
                    .or(incoming_commitment),
            },
            ownership,
        ))
    }
}
