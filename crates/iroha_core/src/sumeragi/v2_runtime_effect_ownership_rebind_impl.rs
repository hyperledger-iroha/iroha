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
        let mut rebound = RuntimeEffectOwnership::new_bound(
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
        .map_err(|_| "Sumeragi v2 exact effect rebind failed closed".to_owned())?;
        if let Some(replay) = self.exact_remote_proposal_fetch_replay(effect) {
            let pending = rebound
                .exact_pending_adapter_effect_binding(effect)
                .map_err(|_| {
                    "Sumeragi v2 exact Fetch rebind lost its pending binding".to_owned()
                })?;
            if !replay.exactly_matches_fetch_pending(effect, &pending) {
                return Err(
                    "Sumeragi v2 exact Fetch rebind changed its authenticated replay root"
                        .to_owned(),
                );
            }
            rebound.remote_proposal_fetch_replay = Some(replay);
        }
        Ok(rebound)
    }
    /// Retag the consumer of one unchanged Fetch task while retaining its
    /// immutable lifecycle owner and, for an ordinary Proposal Fetch, its
    /// authenticated replay envelope.
    pub(crate) fn rebind_fetch_consumer(
        &self,
        previous_effect: &AdapterEffect,
        rebound_effect: &AdapterEffect,
    ) -> Result<Self, String> {
        let replay = self.remote_proposal_fetch_replay.as_ref().ok_or_else(|| {
            "Sumeragi v2 Fetch consumer rebind omitted authenticated Proposal replay authority"
                .to_owned()
        })?;
        let previous_pending = self
            .exact_pending_adapter_effect_binding(previous_effect)
            .map_err(|_| {
                "Sumeragi v2 Fetch consumer rebind changed its incumbent effect".to_owned()
            })?;
        let projected_pending = previous_pending
            .project_fetch_consumer_rebind(previous_effect, rebound_effect)
            .ok_or_else(|| {
                "Sumeragi v2 Fetch consumer rebind changed immutable acquisition work".to_owned()
            })?;
        let inherited = self.candidate_semantic_statement();
        let candidate =
            production_adapter_effect_candidate_binding(rebound_effect, inherited.as_ref())?;
        let candidate_count = u8::from(candidate.is_some());
        let mut rebound = RuntimeEffectOwnership::new_bound(
            self.owner.clone(),
            self.causality,
            production_adapter_effect_kind(rebound_effect),
            &production_adapter_effect_semantic_identity(rebound_effect),
            candidate.as_ref(),
            1,
            1,
            candidate_count,
            candidate_count,
        )
        .map_err(|_| "Sumeragi v2 Fetch consumer rebind failed closed".to_owned())?;
        let rebound_pending = rebound
            .exact_pending_adapter_effect_binding(rebound_effect)
            .map_err(|_| "Sumeragi v2 Fetch consumer rebind lost its pending binding".to_owned())?;
        if rebound_pending != projected_pending {
            return Err("Sumeragi v2 Fetch consumer rebind changed its lifecycle root".to_owned());
        }
        let replay = replay
            .rebind_exact_consumer(
                previous_effect,
                &previous_pending,
                rebound_effect,
                rebound_pending,
            )
            .ok_or_else(|| {
                "Sumeragi v2 ordinary Proposal Fetch consumer rebind changed its authenticated replay owner"
                    .to_owned()
            })?;
        rebound.remote_proposal_fetch_replay = Some(replay);
        Ok(rebound)
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
        let mut adopted = RuntimeEffectOwnership::new_bound(
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
            "Sumeragi v2 semantic retry could not retain its incumbent owner".to_owned()
        })?;
        if let Some(replay) = self.exact_remote_proposal_fetch_replay(effect) {
            let pending = adopted
                .exact_pending_adapter_effect_binding(effect)
                .map_err(|_| {
                    "Sumeragi v2 semantic Fetch retry lost its pending binding".to_owned()
                })?;
            if !replay.exactly_matches_fetch_pending(effect, &pending) {
                return Err(
                    "Sumeragi v2 semantic Fetch retry changed its authenticated replay root"
                        .to_owned(),
                );
            }
            adopted.remote_proposal_fetch_replay = Some(replay);
        }
        Ok(adopted)
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
        let mut adopted = RuntimeEffectOwnership::new_bound(
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
        if relation == RuntimeFetchAuthorityRelation::Same
            && let Some(replay) = self.exact_remote_proposal_fetch_replay(effect)
        {
            let pending = adopted
                .exact_pending_adapter_effect_binding(effect)
                .map_err(|_| {
                    "Sumeragi v2 same-authority Fetch adoption lost its pending binding".to_owned()
                })?;
            if !replay.exactly_matches_fetch_pending(effect, &pending) {
                return Err(
                    "Sumeragi v2 same-authority Fetch adoption changed its authenticated replay root"
                        .to_owned(),
                );
            }
            adopted.remote_proposal_fetch_replay = Some(replay);
        }
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

impl PendingRuntimeEffectBinding {
    /// Project one strictly later consumer tag for the same immutable Fetch.
    pub(in crate::sumeragi) fn project_fetch_consumer_rebind(
        &self,
        previous: &AdapterEffect,
        successor: &AdapterEffect,
    ) -> Option<Self> {
        let (
            AdapterEffect::FetchBody {
                tag: previous_tag,
                round: previous_round,
                subject: previous_subject,
                manifest: previous_manifest,
                certified_sources: previous_sources,
                certificate: previous_certificate,
            },
            AdapterEffect::FetchBody {
                tag: successor_tag,
                round: successor_round,
                subject: successor_subject,
                manifest: successor_manifest,
                certified_sources: successor_sources,
                certificate: successor_certificate,
            },
        ) = (previous, successor)
        else {
            return None;
        };
        if !self.validate_exact(previous)
            || !successor_tag.strictly_advances(*previous_tag)
            || previous_round != successor_round
            || previous_subject != successor_subject
            || previous_manifest != successor_manifest
            || previous_sources != successor_sources
            || previous_certificate != successor_certificate
            || previous_manifest.is_none()
            || !previous_sources.is_empty()
            || previous_certificate.is_some()
        {
            return None;
        }
        let candidate = production_adapter_effect_candidate_binding(
            successor,
            self.candidate_statement.as_ref(),
        )
        .ok()??;
        let projected =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        (projected.validate_exact(successor)
            && projected.effect_kind == self.effect_kind
            && projected.candidate_kind == self.candidate_kind
            && projected.candidate_statement == self.candidate_statement
            && projected.candidate_semantic_identity == self.candidate_semantic_identity)
            .then_some(projected)
    }

    /// Project an ordinary Proposal Store into the exact Commit-refined
    /// `ValidateBody` owner retained after a durable Decision.
    ///
    /// The concrete Store and Validate stages keep the immutable causal root
    /// minted by the authenticated Proposal. Only the candidate statement may
    /// change, and only through the ordinary-to-Commit edge of the reviewed
    /// authority lattice. The supplied Commit binding is itself opaque and
    /// must equal the complete reconstruction; matching body coordinates are
    /// therefore insufficient to splice a foreign lifecycle root.
    pub(crate) fn project_store_validate_successor_with_commit_refinement(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        commit_pending: &Self,
    ) -> Option<Self> {
        if !commit_pending.validate_exact(successor) {
            return None;
        }
        let ordinary_statement = self.candidate_statement?;
        let commit_statement = commit_pending.candidate_statement?;
        ordinary_statement.commit_refinement_to(commit_statement)?;
        let projected = self.project_store_validate_successor_with_authority_refinement(
            predecessor,
            successor,
            commit_pending,
        )?;
        (&projected == commit_pending).then_some(projected)
    }

    /// Project one exact Store lineage into a same-body Validate whose
    /// consumer tag and quorum authority advance monotonically.
    pub(in crate::sumeragi) fn project_store_validate_successor_with_authority_refinement(
        &self,
        predecessor: &AdapterEffect,
        successor: &AdapterEffect,
        successor_pending: &Self,
    ) -> Option<Self> {
        let (
            AdapterEffect::StoreBody {
                tag: predecessor_tag,
                round: predecessor_round,
                subject: predecessor_subject,
            },
            AdapterEffect::ValidateBody {
                tag: successor_tag,
                round: successor_round,
                subject: successor_subject,
            },
        ) = (predecessor, successor)
        else {
            return None;
        };
        if !self.validate_exact(predecessor)
            || !successor_pending.validate_exact(successor)
            || predecessor_tag.height() != successor_tag.height()
            || (predecessor_tag != successor_tag
                && !successor_tag.strictly_advances(*predecessor_tag))
            || predecessor_round != successor_round
            || predecessor_subject != successor_subject
        {
            return None;
        }
        let predecessor_statement = self.candidate_statement?;
        let successor_statement = successor_pending.candidate_statement?;
        if !matches!(
            predecessor_statement.body_stage_authority_relation_to(successor_statement)?,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Upgrade
        ) {
            return None;
        }
        let candidate =
            production_adapter_effect_candidate_binding(successor, Some(&successor_statement))
                .ok()??;
        if candidate.kind != RUNTIME_CANDIDATE_KIND_VALIDATE_BODY
            || candidate.statement != Some(successor_statement)
        {
            return None;
        }
        let projected =
            Self::from_effect_candidate(self.causal_lifecycle_key, successor, Some(&candidate));
        (projected.validate_exact(successor) && &projected == successor_pending)
            .then_some(projected)
    }
}
