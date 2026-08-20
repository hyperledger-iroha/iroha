impl RuntimeEffectOwnership {
    /// Replace the positional binding while retaining the same lifecycle root.
    /// This is used only when one completed local async stage atomically creates
    /// its next exact stage without returning through the serialized reducer.
    pub(crate) fn rebind_as_inherited_adapter_effect(
        &self,
        effect: &AdapterEffect,
    ) -> Result<Self, String> {
        let inherited = self.candidate_semantic_statement();
        let candidate = production_adapter_effect_candidate_binding(effect, inherited.as_ref())?;
        let candidate_count = u8::from(candidate.is_some());
        RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            RuntimeEffectCausality::Inherit,
            production_adapter_effect_kind(effect),
            &production_adapter_effect_semantic_identity(effect),
            candidate.as_ref(),
            1,
            1,
            candidate_count,
            candidate_count,
        )
        .map_err(|_| "Sumeragi v2 successor effect binding failed closed".to_owned())
    }
    pub(crate) fn rebind_same_adapter_effect(
        &self,
        effect: &AdapterEffect,
    ) -> Result<Self, String> {
        let inherited = self.candidate_semantic_statement();
        let candidate = production_adapter_effect_candidate_binding(effect, inherited.as_ref())?;
        let candidate_count = u8::from(candidate.is_some());
        RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            self.causality,
            production_adapter_effect_kind(effect),
            &production_adapter_effect_semantic_identity(effect),
            candidate.as_ref(),
            1,
            1,
            candidate_count,
            candidate_count,
        )
        .map_err(|_| "Sumeragi v2 exact effect rebind failed closed".to_owned())
    }
    /// Bind a route-neutral semantic retry under the candidate's incumbent owner.
    ///
    /// Independent authenticated reducer turns can converge on the same local
    /// candidate.  Their causal roots are intentionally different, while the
    /// candidate kind and complete six-coordinate semantic statement are the
    /// same.  The abstract `1 -> 1` transition must retain the first physical
    /// owner rather than either replacing it or failing the whole height.
    ///
    /// This helper validates both exact bindings, the incoming concrete effect,
    /// and equality of the complete candidate statement before copying only the
    /// incoming macro-step positions onto the immutable incumbent owner.
    pub(crate) fn adopt_incumbent_candidate_for_semantic_retry(
        &self,
        incoming: &Self,
        effect: &AdapterEffect,
    ) -> Result<Self, String> {
        if !self.validate_exact() || !incoming.validate_exact() {
            return Err("Sumeragi v2 semantic retry omitted an exact ownership binding".to_owned());
        }
        let incumbent_binding = &self.binding;
        let incoming_binding = &incoming.binding;
        if incumbent_binding.candidate_kind == RUNTIME_CANDIDATE_KIND_NONE
            || incumbent_binding.candidate_kind != incoming_binding.candidate_kind
            || incumbent_binding.candidate_statement != incoming_binding.candidate_statement
            || incumbent_binding.candidate_semantic_identity
                != incoming_binding.candidate_semantic_identity
        {
            return Err(
                "Sumeragi v2 semantic retry changed its candidate kind or statement".to_owned(),
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
            return Err("Sumeragi v2 semantic retry changed its exact incoming effect".to_owned());
        }
        let statement = incoming_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 semantic retry omitted its candidate statement".to_owned()
        })?;
        let candidate = production_adapter_effect_candidate_binding(effect, Some(&statement))?
            .ok_or_else(|| {
                "Sumeragi v2 semantic retry rebound to a non-candidate effect".to_owned()
            })?;
        let semantic_identity =
            runtime_effect_candidate_semantic_hash(candidate.kind, &candidate.semantic_identity);
        if candidate.kind != incumbent_binding.candidate_kind
            || candidate.statement != Some(statement)
            || Some(semantic_identity) != incumbent_binding.candidate_semantic_identity
        {
            return Err(
                "Sumeragi v2 semantic retry disagreed with its retained candidate".to_owned(),
            );
        }
        RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            self.causality,
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
            Some(&candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| "Sumeragi v2 semantic retry could not retain its incumbent owner".to_owned())
    }
    /// Bind a later StoreBody or ValidateBody carrier to the one physical
    /// asynchronous task already serving its exact body stage.
    ///
    /// Quorum authority is part of a candidate's route-neutral identity, so
    /// an ordinary body pipeline and a later Prepare/Commit-certified pipeline
    /// deliberately have different candidate identities. Persistence and
    /// deterministic validation, however, are each physically unique for one
    /// exact body. Once both bound carriers prove the same stage, body key, and
    /// comparable authority lattice, the retry retains the incumbent task
    /// owner. A weaker late carrier is therefore stale rather than an owner
    /// downgrade, while a conflicting commitment remains fail-closed.
    pub(crate) fn adopt_incumbent_body_stage_for_retry_or_authority(
        &self,
        incoming: &Self,
        effect: &AdapterEffect,
    ) -> Result<Self, String> {
        if !self.validate_exact() || !incoming.validate_exact() {
            return Err(
                "Sumeragi v2 body-stage carrier omitted an exact ownership binding".to_owned(),
            );
        }
        let incumbent_binding = &self.binding;
        let incoming_binding = &incoming.binding;
        let (effect_kind, candidate_kind) = match effect {
            AdapterEffect::StoreBody { .. } => (
                RUNTIME_EFFECT_KIND_STORE_BODY,
                RUNTIME_CANDIDATE_KIND_STORE_BODY,
            ),
            AdapterEffect::ValidateBody { .. } => (
                RUNTIME_EFFECT_KIND_VALIDATE_BODY,
                RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
            ),
            _ => {
                return Err(
                    "Sumeragi v2 body-stage adoption received a non-physical stage".to_owned(),
                );
            }
        };
        if incumbent_binding.effect_kind != effect_kind
            || incoming_binding.effect_kind != effect_kind
            || incumbent_binding.candidate_kind != candidate_kind
            || incoming_binding.candidate_kind != candidate_kind
        {
            return Err("Sumeragi v2 body-stage carrier changed its physical stage".to_owned());
        }
        let incoming_effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        if incoming_binding.effect_identity != incoming_effect_identity {
            return Err(
                "Sumeragi v2 body-stage carrier changed its exact incoming effect".to_owned(),
            );
        }
        let incumbent_statement = incumbent_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 incumbent body stage omitted its authority statement".to_owned()
        })?;
        let incoming_statement = incoming_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 incoming body stage omitted its authority statement".to_owned()
        })?;
        let relation = incumbent_statement
            .body_stage_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "Sumeragi v2 body-stage carrier changed its body key or authority commitment"
                    .to_owned()
            })?;
        // Retain the one physical task owner while advancing its authority
        // monotonically. A stale carrier cannot downgrade a stronger task,
        // but a Prepare/Commit upgrade must remain visible to the eventual
        // reducer completion.
        let (retained_statement, retained_semantic_identity) = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => (
                incoming_statement,
                incoming_binding.candidate_semantic_identity,
            ),
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => (
                incumbent_statement,
                incumbent_binding.candidate_semantic_identity,
            ),
        };
        let retained_semantic_identity = retained_semantic_identity.ok_or_else(|| {
            "Sumeragi v2 retained body stage omitted its candidate identity".to_owned()
        })?;
        let candidate =
            production_adapter_effect_candidate_binding(effect, Some(&retained_statement))?
                .ok_or_else(|| {
                    "Sumeragi v2 body-stage retry rebound to a non-candidate effect".to_owned()
                })?;
        if candidate.kind != candidate_kind
            || runtime_effect_candidate_semantic_hash(candidate.kind, &candidate.semantic_identity)
                != retained_semantic_identity
        {
            return Err(
                "Sumeragi v2 body-stage retry changed its retained semantic statement".to_owned(),
            );
        }
        RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            self.causality,
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
            Some(&candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| "Sumeragi v2 body-stage retry could not retain its incumbent owner".to_owned())
    }
    /// Bind one exact FetchBody carrier under its incumbent physical owner.
    ///
    /// Fetch authority is part of the route-neutral candidate statement, so a
    /// newly authenticated Prepare/Commit carrier has a different candidate
    /// identity from an already-live ordinary fetch. The physical fetch task,
    /// however, keeps one owner and advances authority monotonically. This
    /// helper validates that narrow authority relation and retains the
    /// incumbent owner/causality while copying the incoming macro-step
    /// positions. A stale carrier is still bound exactly here; the returned
    /// relation tells the task reducer to retain its stronger existing state.
    pub(crate) fn adopt_incumbent_fetch_for_retry_or_authority(
        &self,
        incoming: &Self,
        effect: &AdapterEffect,
    ) -> Result<(Self, RuntimeFetchAuthorityRelation), String> {
        if !self.validate_exact() || !incoming.validate_exact() {
            return Err(
                "Sumeragi v2 body-fetch carrier omitted an exact ownership binding".to_owned(),
            );
        }
        let incumbent_binding = &self.binding;
        let incoming_binding = &incoming.binding;
        if incumbent_binding.effect_kind != RUNTIME_EFFECT_KIND_FETCH_BODY
            || incoming_binding.effect_kind != RUNTIME_EFFECT_KIND_FETCH_BODY
            || incumbent_binding.candidate_kind != RUNTIME_CANDIDATE_KIND_FETCH_BODY
            || incoming_binding.candidate_kind != RUNTIME_CANDIDATE_KIND_FETCH_BODY
        {
            return Err(
                "Sumeragi v2 body-fetch lineage changed its effect or candidate kind".to_owned(),
            );
        }
        let incumbent_statement = incumbent_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 incumbent body-fetch omitted its authority statement".to_owned()
        })?;
        let incoming_statement = incoming_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 incoming body-fetch omitted its authority statement".to_owned()
        })?;
        let relation = incumbent_statement
            .fetch_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "Sumeragi v2 body-fetch carrier changed its coordinates or authority commitment"
                    .to_owned()
            })?;
        let effect_kind = production_adapter_effect_kind(effect);
        let effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        if effect_kind != RUNTIME_EFFECT_KIND_FETCH_BODY
            || incoming_binding.effect_identity != effect_identity
        {
            return Err(
                "Sumeragi v2 body-fetch carrier changed its exact incoming effect".to_owned(),
            );
        }
        let candidate = production_adapter_effect_candidate_binding(effect, None)?
            .filter(|candidate| candidate.kind == RUNTIME_CANDIDATE_KIND_FETCH_BODY)
            .ok_or_else(|| {
                "Sumeragi v2 body-fetch lineage rebound to a different effect kind".to_owned()
            })?;
        if candidate.statement != Some(incoming_statement) {
            return Err(
                "Sumeragi v2 body-fetch carrier disagreed with its incoming authority binding"
                    .to_owned(),
            );
        }
        let adopted = RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            self.causality,
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
            Some(&candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| {
            "Sumeragi v2 body-fetch carrier could not retain its incumbent owner".to_owned()
        })?;
        Ok((adopted, relation))
    }
    /// Bind a durable-Decision body-stage retry under its incumbent task owner.
    ///
    /// Storage and validation are physical work for one exact durable body. A
    /// CommitQC can arrive after an ordinary or Prepare-authorized stage has
    /// already started, so the reducer's Decision recovery emits a
    /// Commit-authorized `StoreBody` or `ValidateBody` with a different causal
    /// root. The physical task must not be duplicated or abandoned: validate
    /// the exact monotonic authority transition, retain the incumbent
    /// lifecycle root, and install the incoming Commit binding for the
    /// eventual reducer completion.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn adopt_incumbent_body_stage_for_durable_decision(
        &self,
        incoming: &Self,
        effect: &AdapterEffect,
        decision_round: wire::ConsensusRound,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Result<Self, String> {
        if !self.validate_exact() || !incoming.validate_exact() {
            return Err(
                "Sumeragi v2 Decision body stage omitted an exact ownership binding".to_owned(),
            );
        }
        let incumbent_binding = &self.binding;
        let incoming_binding = &incoming.binding;
        let (expected_effect_kind, expected_candidate_kind, effect_round, effect_subject) =
            match effect {
                AdapterEffect::StoreBody { round, subject, .. } => (
                    RUNTIME_EFFECT_KIND_STORE_BODY,
                    RUNTIME_CANDIDATE_KIND_STORE_BODY,
                    *round,
                    *subject,
                ),
                AdapterEffect::ValidateBody { round, subject, .. } => (
                    RUNTIME_EFFECT_KIND_VALIDATE_BODY,
                    RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
                    *round,
                    *subject,
                ),
                _ => {
                    return Err(
                        "Sumeragi v2 Decision body stage rebound to a different effect kind"
                            .to_owned(),
                    );
                }
            };
        if incumbent_binding.effect_kind != expected_effect_kind
            || incoming_binding.effect_kind != expected_effect_kind
            || incumbent_binding.candidate_kind != expected_candidate_kind
            || incoming_binding.candidate_kind != expected_candidate_kind
        {
            return Err(
                "Sumeragi v2 Decision body stage changed its effect or candidate kind".to_owned(),
            );
        }
        let incumbent_statement = incumbent_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 incumbent body stage omitted its authority statement".to_owned()
        })?;
        let incoming_statement = incoming_binding.candidate_statement.ok_or_else(|| {
            "Sumeragi v2 Decision body stage omitted its authority statement".to_owned()
        })?;
        if !incoming.binds_durable_decision_authority(
            decision_round,
            proposal_round,
            subject,
            execution_commitment,
        ) || incumbent_statement
            .commit_refinement_to(incoming_statement)
            .is_none()
        {
            return Err(
                "Sumeragi v2 Decision body stage changed its proposal or quorum authority"
                    .to_owned(),
            );
        }
        if effect_round != proposal_round || effect_subject != subject {
            return Err(
                "Sumeragi v2 Decision body stage changed its exact body coordinates".to_owned(),
            );
        }
        let effect_kind = production_adapter_effect_kind(effect);
        let effect_identity = runtime_effect_identity_hash(
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
        );
        if incoming_binding.effect_identity != effect_identity {
            return Err(
                "Sumeragi v2 Decision body stage changed its exact incoming effect".to_owned(),
            );
        }
        let candidate =
            production_adapter_effect_candidate_binding(effect, Some(&incoming_statement))?
                .filter(|candidate| candidate.kind == expected_candidate_kind)
                .ok_or_else(|| {
                    "Sumeragi v2 Decision body stage lost its exact candidate".to_owned()
                })?;
        if candidate.statement != Some(incoming_statement) {
            return Err(
                "Sumeragi v2 Decision body stage disagreed with its Commit binding".to_owned(),
            );
        }
        RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            self.causality,
            effect_kind,
            &production_adapter_effect_semantic_identity(effect),
            Some(&candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| {
            "Sumeragi v2 Decision body stage could not retain its incumbent owner".to_owned()
        })
    }
}
