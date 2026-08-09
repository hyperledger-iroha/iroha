/// Sidecar metadata paired positionally with one reducer effect.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeEffectOwnership {
    owner: RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
    binding: Option<RuntimeEffectCandidateBinding>,
}

impl PartialEq for RuntimeEffectOwnership {
    // Equality names the immutable lifecycle owner, not the replaceable
    // positional effect binding. Fetch-route upgrades and later consumer-stage
    // rebinds must retain this equality while recomputing and validating the
    // complete binding before the next asynchronous owner is published.
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner && self.causality == other.causality
    }
}

impl Eq for RuntimeEffectOwnership {}

impl RuntimeEffectOwnership {
    fn inherited(owner: RuntimeLifecycleOwner) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Inherit,
            binding: None,
        }
    }

    fn fresh(owner: RuntimeLifecycleOwner, kind: RuntimeFreshRootKind) -> Self {
        Self {
            owner,
            causality: RuntimeEffectCausality::Fresh(kind),
            binding: None,
        }
    }

    fn validate_exact(&self) -> bool {
        self.owner.validate_exact()
            && self
                .binding
                .as_ref()
                .is_none_or(|binding| binding.validate_exact(&self.owner, self.causality))
    }

    fn validate_bound_exact(&self) -> bool {
        self.validate_exact() && self.binding.is_some()
    }

    /// Return whether this non-forgeable sidecar names one exact production
    /// adapter effect, including all of the effect's concrete coordinates.
    fn exactly_binds_adapter_effect(&self, effect: &AdapterEffect) -> bool {
        let Some(binding) = self.binding.as_ref() else {
            return false;
        };
        let effect_kind = production_adapter_effect_kind(effect);
        self.validate_bound_exact()
            && binding.effect_kind == effect_kind
            && binding.effect_identity
                == runtime_effect_identity_hash(
                    effect_kind,
                    &production_adapter_effect_semantic_identity(effect),
                )
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_runtime_effect(
        mut self,
        parent: Option<&RuntimeLifecycleOwner>,
        effect_kind: u8,
        effect_semantic_identity: &[u8],
        candidate: Option<&RuntimeEffectCandidateSemantic>,
        effect_position: u8,
        effect_count: u8,
        candidate_position: u8,
        candidate_count: u8,
    ) -> Result<Self, EnqueueError> {
        if self.binding.is_some() {
            return Err(EnqueueError::FailClosed);
        }
        self.binding = Some(RuntimeEffectCandidateBinding::new(
            &self.owner,
            self.causality,
            parent,
            effect_kind,
            effect_semantic_identity,
            candidate,
            effect_position,
            effect_count,
            candidate_position,
            candidate_count,
        )?);
        self.validate_bound_exact()
            .then_some(self)
            .ok_or(EnqueueError::FailClosed)
    }

    fn binding(&self) -> Option<&RuntimeEffectCandidateBinding> {
        self.binding.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn candidate_identity(&self) -> Option<iroha_crypto::Hash> {
        self.binding
            .as_ref()
            .and_then(|binding| binding.candidate_identity)
    }

    /// Route-neutral semantic lifecycle used by the single-owner admission gate.
    pub(crate) fn candidate_semantic_identity(&self) -> Option<iroha_crypto::Hash> {
        self.binding
            .as_ref()
            .and_then(|binding| binding.candidate_semantic_identity)
    }

    /// Typed semantic statement retained through causal completion handoffs.
    fn candidate_semantic_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
        self.binding
            .as_ref()
            .and_then(|binding| binding.candidate_statement)
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

    #[cfg(test)]
    pub(crate) fn fresh_for_test(tag: EventTag, lifecycle_ordinal: u128) -> Self {
        let kind = RuntimeFreshRootKind::StartupRecovery;
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            tag,
            CommandClass::Progress,
            kind,
            b"test-runtime-effect",
        );
        Self::fresh(
            RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                .expect("fresh test owner binds its first lifecycle ordinal"),
            kind,
        )
    }
}

fn append_optional_runtime_identity_bytes(identity: &mut Vec<u8>, value: Option<Vec<u8>>) {
    match value {
        None => identity.push(0),
        Some(value) => {
            identity.push(1);
            append_runtime_identity_field(identity, &value);
        }
    }
}

/// Closed classification of every production adapter effect.
pub(crate) const fn production_adapter_effect_kind(effect: &AdapterEffect) -> u8 {
    match effect {
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Proposal(_),
            ..
        } => RUNTIME_EFFECT_KIND_SIGN_PROPOSAL,
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Vote(_),
            ..
        } => RUNTIME_EFFECT_KIND_SIGN_VOTE,
        AdapterEffect::Sign {
            request: super::v2::SignRequest::TimeoutVote(_),
            ..
        } => RUNTIME_EFFECT_KIND_SIGN_TIMEOUT,
        AdapterEffect::FetchBody { .. } => RUNTIME_EFFECT_KIND_FETCH_BODY,
        AdapterEffect::StoreBody { .. } => RUNTIME_EFFECT_KIND_STORE_BODY,
        AdapterEffect::ValidateBody { .. } => RUNTIME_EFFECT_KIND_VALIDATE_BODY,
        AdapterEffect::Apply { .. } => RUNTIME_EFFECT_KIND_APPLY,
        AdapterEffect::Broadcast(_) => RUNTIME_EFFECT_KIND_BROADCAST,
        AdapterEffect::EnterView { .. } => RUNTIME_EFFECT_KIND_ENTER_VIEW,
        AdapterEffect::ReportEquivocation { .. } => RUNTIME_EFFECT_KIND_REPORT_EQUIVOCATION,
        AdapterEffect::ReportInvalidCertifiedBody { .. } => {
            RUNTIME_EFFECT_KIND_REPORT_INVALID_CERTIFIED_BODY
        }
    }
}

/// Canonical exact identity bytes for every field of one production effect.
///
/// These bytes are internal evidence only. They are never serialized as a wire
/// field and do not introduce a runtime configuration surface.
pub(crate) fn production_adapter_effect_semantic_identity(effect: &AdapterEffect) -> Vec<u8> {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"iroha:sumeragi:v2:adapter-effect-semantic:v1");
    identity.push(production_adapter_effect_kind(effect));
    match effect {
        AdapterEffect::Sign { tag, request } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &request.signature_preimage());
        }
        AdapterEffect::Broadcast(message) => {
            append_runtime_identity_field(&mut identity, &message.encode());
        }
        AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest,
            certified_sources,
            certificate,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &round.encode());
            append_runtime_identity_field(&mut identity, &subject.encode());
            append_optional_runtime_identity_bytes(
                &mut identity,
                manifest.as_ref().map(norito::codec::Encode::encode),
            );
            append_runtime_identity_field(&mut identity, &certified_sources.encode());
            append_optional_runtime_identity_bytes(
                &mut identity,
                certificate.as_ref().map(norito::codec::Encode::encode),
            );
        }
        AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        }
        | AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &round.encode());
            append_runtime_identity_field(&mut identity, &subject.encode());
        }
        AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &subject.encode());
            append_runtime_identity_field(&mut identity, &certificate.encode());
        }
        AdapterEffect::EnterView {
            tag,
            certificate,
            protected_body,
        } => {
            append_runtime_identity_tag(&mut identity, *tag);
            append_runtime_identity_field(&mut identity, &certificate.encode());
            match protected_body {
                None => identity.push(0),
                Some((round, subject)) => {
                    identity.push(1);
                    append_runtime_identity_field(&mut identity, &round.encode());
                    append_runtime_identity_field(&mut identity, &subject.encode());
                }
            }
        }
        AdapterEffect::ReportEquivocation {
            offender,
            round,
            kind,
        } => {
            append_runtime_identity_field(&mut identity, &offender.encode());
            append_runtime_identity_field(&mut identity, &round.encode());
            identity.push(match kind {
                super::v2_core::EquivocationKind::Vote => 1,
                super::v2_core::EquivocationKind::Timeout => 2,
                super::v2_core::EquivocationKind::Proposal => 3,
            });
        }
        AdapterEffect::ReportInvalidCertifiedBody {
            subject,
            certificate,
        } => {
            append_runtime_identity_field(&mut identity, &subject.encode());
            append_runtime_identity_field(&mut identity, &certificate.encode());
        }
    }
    identity
}

fn production_adapter_effect_candidate_statement(
    effect: &AdapterEffect,
) -> Option<(u8, RuntimeCandidateSemanticStatement)> {
    Some(match effect {
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Proposal(proposal),
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_SIGN_PROPOSAL,
            RuntimeCandidateSemanticStatement::new(
                proposal.round,
                proposal.round,
                Some(proposal.subject),
                None,
                None,
            ),
        ),
        AdapterEffect::Sign {
            request: super::v2::SignRequest::Vote(vote),
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_SIGN_VOTE,
            RuntimeCandidateSemanticStatement::new(
                vote.round,
                vote.proposal_round,
                Some(vote.subject),
                Some(vote.phase),
                Some(vote.execution_commitment),
            ),
        ),
        AdapterEffect::Sign {
            request: super::v2::SignRequest::TimeoutVote(vote),
            ..
        } => {
            let highest = vote.highest_prepare_qc.as_ref();
            (
                RUNTIME_CANDIDATE_KIND_SIGN_TIMEOUT,
                RuntimeCandidateSemanticStatement::new(
                    vote.round,
                    highest.map_or(vote.round, |certificate| certificate.proposal_round),
                    highest.map(|certificate| certificate.subject),
                    highest.map(|certificate| certificate.phase),
                    highest.map(|certificate| certificate.execution_commitment),
                ),
            )
        }
        AdapterEffect::FetchBody {
            round,
            subject,
            certificate,
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_FETCH_BODY,
            RuntimeCandidateSemanticStatement::new(
                certificate
                    .as_ref()
                    .map_or(*round, |certificate| certificate.round),
                certificate
                    .as_ref()
                    .map_or(*round, |certificate| certificate.proposal_round),
                Some(*subject),
                certificate.as_ref().map(|certificate| certificate.phase),
                certificate
                    .as_ref()
                    .map(|certificate| certificate.execution_commitment),
            ),
        ),
        AdapterEffect::StoreBody { round, subject, .. } => (
            RUNTIME_CANDIDATE_KIND_STORE_BODY,
            RuntimeCandidateSemanticStatement::new(*round, *round, Some(*subject), None, None),
        ),
        AdapterEffect::ValidateBody { round, subject, .. } => (
            RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
            RuntimeCandidateSemanticStatement::new(*round, *round, Some(*subject), None, None),
        ),
        AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } => (
            RUNTIME_CANDIDATE_KIND_APPLY,
            RuntimeCandidateSemanticStatement::new(
                certificate.round,
                certificate.proposal_round,
                Some(*subject),
                Some(certificate.phase),
                Some(certificate.execution_commitment),
            ),
        ),
        AdapterEffect::Broadcast(_)
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => return None,
    })
}

/// Bind one production candidate, optionally inheriting the exact body-stage
/// statement carried by its causal parent.
fn production_adapter_effect_candidate_binding(
    effect: &AdapterEffect,
    inherited: Option<&RuntimeCandidateSemanticStatement>,
) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
    match effect {
        AdapterEffect::FetchBody {
            round,
            subject,
            certificate: Some(certificate),
            ..
        } if certificate.proposal_round != *round || certificate.subject != *subject => {
            return Err(
                "Sumeragi v2 certified Fetch disagreed with its proposal-round body key".to_owned(),
            );
        }
        AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } if certificate.phase != wire::GlobalPhase::Commit || certificate.subject != *subject => {
            return Err("Sumeragi v2 Apply omitted its exact Commit authority".to_owned());
        }
        _ => {}
    }
    let Some((kind, mut statement)) = production_adapter_effect_candidate_statement(effect) else {
        return Ok(None);
    };
    if !statement.validate_exact() {
        return Err(
            "Sumeragi v2 candidate statement had inconsistent context or height".to_owned(),
        );
    }
    if let Some(parent) = inherited {
        if !parent.validate_exact() {
            return Err("Sumeragi v2 causal parent lost its exact candidate statement".to_owned());
        }
        match effect {
            AdapterEffect::StoreBody { round, subject, .. }
            | AdapterEffect::ValidateBody { round, subject, .. } => {
                if parent.context_id != round.context_id
                    || parent.proposal_round != *round
                    || parent.subject != Some(*subject)
                {
                    return Err(
                        "Sumeragi v2 body successor changed its frozen candidate statement"
                            .to_owned(),
                    );
                }
                // Store and Validate effects intentionally carry only their
                // concrete body key. Their abstract phase and execution
                // commitment remain the exact values frozen by Fetch.
                statement = *parent;
            }
            AdapterEffect::Apply { .. } => {
                if parent.commit_refinement_to(statement).is_none() {
                    return Err(
                        "Sumeragi v2 Apply changed its inherited candidate authority".to_owned(),
                    );
                }
            }
            AdapterEffect::Sign { .. }
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::Broadcast(_)
            | AdapterEffect::EnterView { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => {}
        }
    }
    let semantic_identity = statement.semantic_identity();
    Ok(Some(RuntimeEffectCandidateSemantic {
        kind,
        semantic_identity,
        statement: Some(statement),
    }))
}

/// Route-neutral TLA work kind and semantic payload for candidate-producing
/// effects.
///
/// The normalized payload is exactly the frozen context, round, proposal
/// round, optional subject, optional consensus phase, and optional execution
/// commitment. The candidate kind separately fixes the durable local stage.
/// Signer carriers, aggregate signatures, routes, manifests, and the
/// process-local reducer incarnation remain only in concrete effect identity.
pub(crate) fn production_adapter_effect_candidate_semantic_identity(
    effect: &AdapterEffect,
) -> Option<(u8, Vec<u8>)> {
    let (kind, statement) = production_adapter_effect_candidate_statement(effect)?;
    statement
        .validate_exact()
        .then(|| (kind, statement.semantic_identity()))
}
