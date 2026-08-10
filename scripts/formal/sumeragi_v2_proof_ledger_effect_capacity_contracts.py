# Executed lexically in check_sumeragi_v2_proof_ledger.py.

def _effect_capacity_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind retryable exact-Fetch capacity and completion to production flow."""

    effects_path = (
        repo_root
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_effects.rs"
    )
    if not effects_path.is_file() or effects_path.is_symlink():
        return [
            f"{effects_path}: production effect executor must be a regular "
            "source file for certified-request capacity refinement"
        ]

    source = effects_path.read_text(encoding="utf-8")
    errors: list[str] = []
    generic_executor_context = (
        (
            "impl",
            "<",
            "R",
            ":",
            "EffectRuntime",
            ">",
            "V2EffectExecutor",
            "<",
            "R",
            ">",
        ),
    )
    production_executor_context = (
        ("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),
    )

    retry_policy = _require_rust_item(
        effects_path,
        source,
        "candidate_retry_is_redispatched",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retry_policy,
        generic_executor_context,
        "closed eleven-class adapter-effect retry policy",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        retry_policy,
        """
fn candidate_retry_is_redispatched(effect: &AdapterEffect) -> Option<bool> {
    match effect {
        AdapterEffect::Sign {
            request:
                SignRequest::Proposal(_) | SignRequest::Vote(_) | SignRequest::TimeoutVote(_),
            ..
        } => Some(false),
        AdapterEffect::FetchBody { .. }
        | AdapterEffect::StoreBody { .. }
        | AdapterEffect::ValidateBody { .. }
        | AdapterEffect::Apply { .. } => Some(true),
        AdapterEffect::Broadcast(_)
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
    }
}
""",
        "closed retry policy must classify all three Sign and eight other adapter-effect classes",
        errors,
    )

    retained_owners = _require_rust_item(
        effects_path,
        source,
        "retained_candidate_owners",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retained_owners,
        generic_executor_context,
        "route-neutral live and terminal candidate owner inventory",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retained_owners,
        """
let identity = ownership.candidate_semantic_identity().ok_or_else(|| {
    EffectExecutorError::Contract(
        "pending asynchronous work omitted its route-neutral candidate identity"
            .to_owned(),
    )
})?;
match owners.get(&identity) {
    Some(existing) if existing != ownership => Err(EffectExecutorError::Contract(
        "one semantic candidate lifecycle had conflicting exact owners".to_owned(),
    )),
    Some(_) => Ok(()),
    None => {
        owners.insert(identity, ownership.clone());
        Ok(())
    }
""",
        "candidate owner inventory must reject semantic owner replacement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retained_owners,
        """
for pending in self.pending_applications.values() {
    insert(pending.task.ownership())?;
}
if let Some(finality) = &self.finality_completion {
    insert(&finality.ownership)?;
}
""",
        "durable Apply tombstone must retain the same candidate owner after completion",
        errors,
    )

    retain = _require_rust_item(
        effects_path,
        source,
        "retain_effect_batch_at_frontier",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retain,
        generic_executor_context,
        "exact adapter candidate admission disposition gate",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
let mut candidate_semantic_identity = evidence.candidate_semantic_identity();
let mut exact_incumbent = candidate_semantic_identity
    .as_ref()
    .and_then(|identity| retained_candidate_owners.get(identity))
    .cloned();
""",
        "candidate admission must resolve the incumbent exact owner before constructing refinement evidence",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
if let Some(incumbent) = exact_incumbent.as_ref()
    && incumbent != &*evidence
{
    return Err(EffectExecutorError::Contract(
        "one semantic candidate lifecycle attempted exact owner replacement"
            .to_owned(),
    ));
}
""",
        "same semantic candidate lifecycle must reject exact owner replacement before refinement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
let runtime_terminal_ownership = self
    .runtime
    .plan_body_pipeline_candidate_terminal(effect, evidence)
    .map_err(EffectExecutorError::Runtime)?;
let runtime_terminal_incumbent = runtime_terminal_ownership.is_some();
if runtime_terminal_incumbent && exact_incumbent.is_some() {
    return Err(EffectExecutorError::Contract(
        "one body candidate was owned by both executor work and a runtime terminal"
            .to_owned(),
    ));
}
if let Some(adopted) = runtime_terminal_ownership {
    let adopted_identity = adopted.candidate_semantic_identity().ok_or_else(|| {
        EffectExecutorError::Contract(
            "one runtime body terminal omitted its candidate identity".to_owned(),
        )
    })?;
    if retained_candidate_owners.contains_key(&adopted_identity) {
        return Err(EffectExecutorError::Contract(
            "one body candidate was owned by both executor work and a runtime terminal"
                .to_owned(),
        ));
    }
    *evidence = adopted;
    candidate_semantic_identity = Some(adopted_identity);
    exact_incumbent = None;
}
""",
        "runtime body terminals must plan and adopt the incumbent owner without committing before refinement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
if let Some(incumbent) = lineage_only_incumbent {
    let (adopted, relation) = incumbent
        .adopt_incumbent_fetch_for_retry_or_authority(evidence, effect)
        .map_err(EffectExecutorError::Contract)?;
    admission = production_adapter_effect_candidate_admission_disposition(
        effect,
        1,
        candidate_owner_count_after,
    )
    .map_err(EffectExecutorError::Contract)?;
    let adopted_projection = production_adapter_effect_candidate_trace_projection(
        effect,
        &adopted,
        effect_position,
        effect_count,
        candidate.as_ref().map_or(0, |_| candidate_position),
        candidate_count,
        1,
        candidate_owner_count_after,
        true,
    )
    .map_err(EffectExecutorError::Contract)?;
    let _authorized_fetch_refinement =
        check_production_effect_to_candidate_transition(adopted_projection)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "coalesced body-fetch authority refinement failed its incumbent-owner refinement"
                        .to_owned(),
                )
            })?;
    *evidence = adopted;
    fetch_authority_relation = Some(relation);
    if let Some(identity) = candidate_semantic_identity {
        retained_candidate_owners.insert(identity, evidence.clone());
    }
}
""",
        "Fetch authority upgrades must adopt, re-prove, and publish the incumbent physical owner before redispatch",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
if let Some(incumbent) = body_stage_only_incumbent {
    let adopted = incumbent
        .adopt_incumbent_body_stage_for_retry_or_authority(evidence, effect)
        .map_err(EffectExecutorError::Contract)?;
    admission = production_adapter_effect_candidate_admission_disposition(
        effect,
        1,
        candidate_owner_count_after,
    )
    .map_err(EffectExecutorError::Contract)?;
    let adopted_projection = production_adapter_effect_candidate_trace_projection(
        effect,
        &adopted,
        effect_position,
        effect_count,
        candidate.as_ref().map_or(0, |_| candidate_position),
        candidate_count,
        1,
        candidate_owner_count_after,
        true,
    )
    .map_err(EffectExecutorError::Contract)?;
    let _authorized_body_stage_refinement =
        check_production_effect_to_candidate_transition(adopted_projection)
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "coalesced body-stage authority refinement failed its incumbent-owner refinement"
                        .to_owned(),
                )
            })?;
    *evidence = adopted;
}
""",
        "Store and Validate authority upgrades must adopt and re-prove the incumbent physical owner before redispatch",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
match (admission, candidate_semantic_identity) {
    (RuntimeCandidateAdmissionDisposition::FirstAdmission, Some(identity)) => {
        retained_candidate_owners.insert(identity, evidence.clone());
        retain_effect.push(true);
    }
    (RuntimeCandidateAdmissionDisposition::CoalescedRetry, Some(_)) => {
        let redispatch = if runtime_terminal_incumbent {
            false
        } else {
            Self::candidate_retry_is_redispatched(effect).ok_or_else(|| {
                EffectExecutorError::Contract(
                    "candidate retry omitted its closed adapter-effect policy"
                        .to_owned(),
                )
            })?
        };
        retain_effect.push(redispatch);
    }
    (RuntimeCandidateAdmissionDisposition::NonCandidate, None) => {
        if Self::candidate_retry_is_redispatched(effect).is_some() {
            return Err(EffectExecutorError::Contract(
                "non-candidate effect entered the candidate retry table".to_owned(),
            ));
        }
        retain_effect.push(true);
    }
""",
        "candidate admission must project only 0-to-1, 1-to-1, and 0-to-0 dispositions while runtime-owned terminals stutter",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
if runtime_terminal_incumbent {
    runtime_terminal_commits.push(index);
}
""",
        "runtime body-terminal authority must be scheduled only after its complete effect-level refinement and disposition gate",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retain,
        """
if !runtime_terminal_commits.is_empty() {
    let terminals = runtime_terminal_commits
        .iter()
        .map(|index| (&effects[*index], &ownership[*index]))
        .collect::<Vec<_>>();
    self.runtime
        .commit_body_pipeline_candidate_terminals(&terminals)
        .map_err(EffectExecutorError::Runtime)?;
}
""",
        "runtime body-terminal authority may commit atomically only after the complete macro-step positional gate",
        errors,
    )
    if retain is not None:
        retain_tokens = rust_code_tokens(retain.source)
        exact_incumbent_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("let mut exact_incumbent = candidate_semantic_identity"),
        )
        count_positions = _token_sequence_positions(
            retain_tokens, rust_code_tokens("let candidate_owner_count_before =")
        )
        admission_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens(
                "production_adapter_effect_candidate_admission_disposition("
            ),
        )
        projection_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens(
                "production_adapter_effect_candidate_trace_projection("
            ),
        )
        checked_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("check_production_effect_to_candidate_transition("),
        )
        owner_replacement_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens(
                "if let Some(incumbent) = exact_incumbent.as_ref() "
                "&& incumbent != &*evidence"
            ),
        )
        terminal_plan_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("plan_body_pipeline_candidate_terminal"),
        )
        terminal_adoption_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens(
                "if let Some(adopted) = runtime_terminal_ownership"
            ),
        )
        terminal_schedule_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("runtime_terminal_commits.push(index)"),
        )
        terminal_commit_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("commit_body_pipeline_candidate_terminals"),
        )
        fetch_adoption_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("adopt_incumbent_fetch_for_retry_or_authority"),
        )
        body_stage_adoption_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens(
                "adopt_incumbent_body_stage_for_retry_or_authority"
            ),
        )
        match_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens("match (admission, candidate_semantic_identity)"),
        )
        publication_positions = _token_sequence_positions(
            retain_tokens,
            rust_code_tokens(
                "retained_candidate_owners.insert(identity, evidence.clone())"
            ),
        )
        ordered = (
            len(exact_incumbent_positions) == 1
            and len(owner_replacement_positions) == 1
            and len(terminal_plan_positions) == 1
            and len(terminal_adoption_positions) == 1
            and len(terminal_schedule_positions) == 1
            and len(terminal_commit_positions) == 1
            and len(count_positions) == 1
            and len(admission_positions) == 3
            and len(projection_positions) == 3
            and len(checked_positions) == 3
            and len(fetch_adoption_positions) == 1
            and len(body_stage_adoption_positions) == 1
            and len(match_positions) == 1
            and len(publication_positions) == 2
            and exact_incumbent_positions[0]
            < owner_replacement_positions[0]
            < terminal_plan_positions[0]
            < terminal_adoption_positions[0]
            < count_positions[0]
            < admission_positions[0]
            < projection_positions[0]
            < checked_positions[0]
            < fetch_adoption_positions[0]
            < admission_positions[1]
            < projection_positions[1]
            < checked_positions[1]
            < body_stage_adoption_positions[0]
            < admission_positions[2]
            < projection_positions[2]
            < checked_positions[2]
            < match_positions[0]
            < publication_positions[-1]
            < terminal_schedule_positions[0]
            < terminal_commit_positions[0]
        )
        if ordered:
            def brace_depth_at(position: int) -> int:
                depth = 0
                for token in retain_tokens[:position]:
                    if token == "{":
                        depth += 1
                    elif token == "}":
                        depth -= 1
                return depth

            ordered = (
                brace_depth_at(terminal_schedule_positions[0])
                == brace_depth_at(terminal_plan_positions[0]) + 1
                and brace_depth_at(terminal_commit_positions[0])
                == brace_depth_at(terminal_plan_positions[0])
            )
        if not ordered:
            errors.append(
                f"{effects_path}:{retain.line}: exact owner replacement rejection, count "
                "classification, non-mutating terminal planning, authority refinements, "
                "owner publication, deferred terminal scheduling, and atomic batch commit "
                "must retain the reviewed "
                "pre-refinement order"
            )

    begin_apply = _require_rust_item(
        effects_path,
        source,
        "begin_apply",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        begin_apply,
        generic_executor_context,
        "same-owner Apply retry and durable tombstone handler",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_apply,
        """
if let Some(finality) = self.finality_completion.as_ref() {
    if !finality.matches_apply(tag, &self.context, subject, &certificate, &ownership) {
""",
        "durable Apply tombstone must compare the retained lifecycle owner",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_apply,
        """
let same_decision = existing.task.tag == tag
    && existing.task.subject == subject
    && existing.task.ownership() == &ownership
    && existing
        .task
        .certificate
        .as_ref()
        .same_commit_decision(certificate.as_ref());
""",
        "in-flight Apply retry must retain the incumbent owner and semantic decision",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_apply,
        """
return services
    .enqueue_apply(existing.task.clone())
    .map_err(service_error);
""",
        "same-owner Apply retry must re-enqueue only the incumbent task",
        errors,
    )

    complete_application = _require_rust_item(
        effects_path,
        source,
        "complete_application",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        complete_application,
        generic_executor_context,
        "Apply completion owner tombstone installation",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        complete_application,
        """
let ownership = task.ownership().clone();
if let Err(error) = self
    .runtime
    .enqueue_application_completed_with_owner(tag, subject, &ownership)
""",
        "Apply completion must publish through its incumbent owner",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        complete_application,
        """
self.finality_completion = Some(FinalityCompletion {
    tag,
    receipt: completion.receipt,
    artifact: completion.artifact,
    ownership,
});
""",
        "durable finality tombstone must retain the completed Apply owner",
        errors,
    )

    consume_effects = _require_rust_item(
        effects_path,
        source,
        "consume_effects",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        consume_effects,
        generic_executor_context,
        "runtime ownership boundary fail-closed handoff",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        consume_effects,
        """
let frontier = self
    .runtime
    .reconciliation_frontier()
    .map_err(EffectExecutorError::Runtime)
    .map_err(|error| self.close(error, services))?;
if let Err(error) = self.preflight_effect_batch_frontier(&effects, frontier) {
    return Err(self.close(error, services));
}
let ownership = match self.runtime.take_effect_ownership(&effects) {
    Ok(ownership) => ownership,
    Err(error) => {
        return Err(self.close(EffectExecutorError::Runtime(error), services));
    }
};
if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {
    return Err(self.close(error, services));
}
if let Err(error) = self.commit_reconciliation_frontier(frontier, services) {
""",
        "executor must retain the complete batch before committing its atomically observed reducer frontier",
        errors,
    )

    effect_runtime_context = (
        ("impl", "EffectRuntime", "for", "SerializedV2Runtime"),
    )
    for item_name, expected_source, description in (
        (
            "plan_body_pipeline_candidate_terminal",
            """
fn plan_body_pipeline_candidate_terminal(
    &mut self,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> Result<Option<RuntimeEffectOwnership>, String> {
    SerializedV2Runtime::plan_body_pipeline_candidate_terminal(self, effect, ownership)
}
""",
            "production EffectRuntime body-terminal plan delegation",
        ),
        (
            "commit_body_pipeline_candidate_terminals",
            """
fn commit_body_pipeline_candidate_terminals(
    &mut self,
    terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
) -> Result<(), String> {
    SerializedV2Runtime::commit_body_pipeline_candidate_terminals(self, terminals)
}
""",
            "production EffectRuntime atomic body-terminal commit delegation",
        ),
    ):
        items = tuple(
            item
            for item in rust_items(source, item_name)
            if item.brace_context == effect_runtime_context
        )
        if len(items) != 1:
            errors.append(
                f"{effects_path}: require exactly one contextual production "
                f"EffectRuntime::{item_name}; found {len(items)}"
            )
            continue
        _require_rust_item_context(
            effects_path,
            items[0],
            effect_runtime_context,
            description,
            errors,
        )
        _require_exact_rust_tokens(
            effects_path,
            items[0],
            expected_source,
            description,
            errors,
        )

    runtime_path = effects_path.with_name("v2_runtime.rs")
    if not runtime_path.is_file() or runtime_path.is_symlink():
        errors.append(
            f"{runtime_path}: runtime candidate admission source must be a regular file"
        )
    else:
        runtime_source = runtime_path.read_text(encoding="utf-8")
        serialized_runtime_context = (
            (
                "impl",
                "<",
                "D",
                ":",
                "RuntimeDriver",
                ">",
                "SerializedV2Runtime",
                "<",
                "D",
                ">",
            ),
        )
        production_serialized_runtime_context = (
            ("impl", "SerializedV2Runtime", "<", "SumeragiV2Adapter", ">"),
        )
        bounded_ingress_context = (
            ("impl", "BoundedIngress", "<", "AdapterCommand", ">"),
        )
        retransmit_ownership_helper = _require_rust_item(
            runtime_path,
            runtime_source,
            "retain_retransmit_effect_ownership_for_test",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            retransmit_ownership_helper,
            serialized_runtime_context,
            "test-only production retransmit lifecycle ownership binder",
            errors,
            expected_attributes=("#[cfg(test)]",),
        )
        _require_rust_item_token_sha256(
            runtime_path,
            retransmit_ownership_helper,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "retain_retransmit_effect_ownership_for_test"
            ],
            "test-only production retransmit lifecycle ownership binder",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            retransmit_ownership_helper,
            """
self.retain_effect_ownership(
    RuntimeEffectSource::Retransmit,
    None,
    None,
    effects
)
""",
            "test binder must delegate to the exact production retransmit ownership path",
            errors,
        )
        _require_rust_source_token_sequence(
            runtime_path,
            runtime_source,
            """
pub(crate) enum RuntimeCandidateAdmissionDisposition {
    FirstAdmission,
    CoalescedRetry,
    NonCandidate,
}
""",
            "candidate admission disposition must remain private and non-serialized",
            errors,
        )
        _require_rust_source_token_sequence(
            runtime_path,
            runtime_source,
            """
CoalesceOwned {
    causal_lifecycle_key: iroha_crypto::Hash,
    admission_ordinal: u128,
},
""",
            "exact terminal coalescence must carry its retained lifecycle key and ordinal",
            errors,
        )
        disposition = _require_rust_item(
            runtime_path,
            runtime_source,
            "production_adapter_effect_candidate_admission_disposition",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            disposition,
            (),
            "exact candidate owner-count disposition",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            disposition,
            """
pub(crate) fn production_adapter_effect_candidate_admission_disposition(
    effect: &AdapterEffect,
    candidate_owner_count_before: u8,
    candidate_owner_count_after: u8,
) -> Result<RuntimeCandidateAdmissionDisposition, String> {
    match (
        production_adapter_effect_candidate_semantic_identity(effect).is_some(),
        candidate_owner_count_before,
        candidate_owner_count_after,
    ) {
        (true, 0, 1) => Ok(RuntimeCandidateAdmissionDisposition::FirstAdmission),
        (true, 1, 1) => Ok(RuntimeCandidateAdmissionDisposition::CoalescedRetry),
        (false, 0, 0) => Ok(RuntimeCandidateAdmissionDisposition::NonCandidate),
        _ => Err(
            "Sumeragi v2 candidate admission used a non-exact owner-count transition".to_owned(),
        ),
    }
}
""",
            "candidate admission must expose only exact 0-to-1, 1-to-1, and 0-to-0 transitions",
            errors,
        )

        terminal_effective_statement = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeBodyCompletionOwnershipPlan",
            "effective_statement",
            errors,
            "replacement-first body-terminal authority selection",
        )
        _require_exact_rust_tokens(
            runtime_path,
            terminal_effective_statement,
            """
fn effective_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
    self.replacement_statement.or(self.retained_statement)
}
""",
            "body-terminal authority selection must prefer a checked replacement over the retained statement",
            errors,
        )

        terminal_adoption = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeBodyCompletionOwnershipPlan",
            "adopt_effect_ownership",
            errors,
            "non-mutating body-terminal incumbent-owner adoption",
        )
        _require_exact_rust_tokens(
            runtime_path,
            terminal_adoption,
            """
fn adopt_effect_ownership(
    &self,
    effect: &AdapterEffect,
    incoming: &RuntimeEffectOwnership,
) -> Result<RuntimeEffectOwnership, String> {
    if !self.retained_owner.validate_exact()
        || !incoming.validate_bound_exact()
        || !incoming.exactly_binds_adapter_effect(effect)
    {
        return Err(
            "Sumeragi v2 body terminal retry omitted exact effect ownership".to_owned(),
        );
    }
    let incoming_binding = incoming
        .binding()
        .expect("validated terminal retry has one positional binding");
    let retained_statement = self.effective_statement().ok_or_else(|| {
        "Sumeragi v2 body terminal retry omitted its retained authority statement".to_owned()
    })?;
    let candidate =
        production_adapter_effect_candidate_binding(effect, Some(&retained_statement))?
            .ok_or_else(|| {
                "Sumeragi v2 body terminal retry rebound to a non-candidate effect".to_owned()
            })?;
    if candidate.statement != Some(retained_statement)
        || candidate.kind != incoming_binding.candidate_kind
    {
        return Err(
            "Sumeragi v2 body terminal retry changed its retained candidate statement"
                .to_owned(),
        );
    }

    let ownership = match incoming.causality() {
        RuntimeEffectCausality::Inherit => {
            RuntimeEffectOwnership::inherited(self.retained_owner.clone())
        }
        RuntimeEffectCausality::Fresh(kind) => {
            RuntimeEffectOwnership::fresh(self.retained_owner.clone(), kind)
        }
    };
    let parent = matches!(ownership.causality(), RuntimeEffectCausality::Inherit)
        .then(|| self.retained_owner.clone());
    ownership
        .bind_runtime_effect(
            parent.as_ref(),
            production_adapter_effect_kind(effect),
            &production_adapter_effect_semantic_identity(effect),
            Some(&candidate),
            incoming_binding.effect_position,
            incoming_binding.effect_count,
            incoming_binding.candidate_position,
            incoming_binding.candidate_count,
        )
        .map_err(|_| {
            "Sumeragi v2 body terminal retry could not retain its incumbent owner".to_owned()
        })
}
""",
            "body-terminal adoption must rebind the exact candidate and positions under the immutable incumbent owner",
            errors,
        )
        for sequence, description in (
            (
                """
if !self.retained_owner.validate_exact()
    || !incoming.validate_bound_exact()
    || !incoming.exactly_binds_adapter_effect(effect)
{
    return Err(
        "Sumeragi v2 body terminal retry omitted exact effect ownership".to_owned(),
    );
}
""",
                "body-terminal adoption must validate the incumbent and complete incoming effect binding",
            ),
            (
                """
let retained_statement = self.effective_statement().ok_or_else(|| {
    "Sumeragi v2 body terminal retry omitted its retained authority statement".to_owned()
})?;
let candidate =
    production_adapter_effect_candidate_binding(effect, Some(&retained_statement))?
""",
                "body-terminal adoption must derive its candidate from the retained effective authority",
            ),
            (
                """
let ownership = match incoming.causality() {
    RuntimeEffectCausality::Inherit => {
        RuntimeEffectOwnership::inherited(self.retained_owner.clone())
    }
    RuntimeEffectCausality::Fresh(kind) => {
        RuntimeEffectOwnership::fresh(self.retained_owner.clone(), kind)
    }
};
""",
                "body-terminal adoption must retain the runtime terminal owner for every causality class",
            ),
            (
                """
ownership
    .bind_runtime_effect(
        parent.as_ref(),
        production_adapter_effect_kind(effect),
        &production_adapter_effect_semantic_identity(effect),
        Some(&candidate),
        incoming_binding.effect_position,
        incoming_binding.effect_count,
        incoming_binding.candidate_position,
        incoming_binding.candidate_count,
    )
""",
                "body-terminal adoption must copy only the checked incoming batch positions under the incumbent owner",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                terminal_adoption,
                sequence,
                description,
                errors,
            )

        terminal_resolution = _require_rust_item(
            runtime_path,
            runtime_source,
            "resolve_body_pipeline_completion_owner",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_resolution,
            production_serialized_runtime_context,
            "body-terminal owner and authority resolution",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            terminal_resolution,
            """
match relation {
    RuntimeFetchAuthorityRelation::Upgrade => incoming_statement,
    RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
        if retained_owner != *ownership.owner() {
            self.latch_fail_closed(
                "coalesced body completion changed its exact lifecycle owner",
            );
            return Err(EnqueueError::FailClosed);
        }
        None
    }
}
""",
            "same or stale terminal retries must reject a foreign owner while an authority upgrade remains separately reviewed",
            errors,
        )

        terminal_prepare = _require_rust_item(
            runtime_path,
            runtime_source,
            "prepare_body_pipeline_completion_refinements",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_prepare,
            production_serialized_runtime_context,
            "non-mutating body-terminal authority batch preparation",
            errors,
        )
        for sequence, description in (
            (
                """
let mut prepared = RuntimePreparedBodyCompletionRefinements::default();
let mut targets = BTreeSet::new();
for plan in plans {
    let Some(replacement) = plan.replacement_statement else {
        continue;
    };
    let Some(incumbent) = plan.retained_statement else {
        self.latch_fail_closed(
            "authority upgrade omitted its incumbent body completion statement",
        );
        return Err(EnqueueError::FailClosed);
    };
    if !replacement.validate_exact() || !targets.insert(plan.target) {
        self.latch_fail_closed(
            "authority upgrade had invalid or duplicate body completion targets",
        );
        return Err(EnqueueError::FailClosed);
    }
""",
                "body-terminal preparation must reject invalid authority or duplicate targets before staging refinements",
            ),
            (
                """
let matches = match self
    .ingress
    .exact_body_pipeline_completion_refinement_matches(
        plan.tag,
        &plan.candidate,
        plan.target,
        &plan.retained_owner,
        incumbent,
    ) {
    Ok(matches) => matches,
    Err(error) => {
        self.latch_fail_closed(
            "authority upgrade could not validate its ingress completion owner",
        );
        return Err(error);
    }
};
if !matches {
    self.latch_fail_closed(
        "authority upgrade changed its ingress body completion owner",
    );
    return Err(EnqueueError::FailClosed);
}
prepared.ingress.push((plan.target, replacement));
""",
                "body-terminal preparation must stage only an exact ingress owner and incumbent authority",
            ),
            (
                """
if self
    .driver
    .deferred_body_pipeline_completion_exact_owner_ordinals(
        plan.tag,
        &plan.candidate,
    )
    != vec![ordinal]
{
    self.latch_fail_closed(
        "authority upgrade changed its deferred body completion target",
    );
    return Err(EnqueueError::FailClosed);
}
let Some(existing) = self.deferred_lifecycle_ownership.get(&ordinal) else {
    self.latch_fail_closed(
        "authority upgrade lost its deferred body completion owner",
    );
    return Err(EnqueueError::FailClosed);
};
if existing.owner != plan.retained_owner
    || existing.candidate_semantic_statement != Some(incumbent)
{
    self.latch_fail_closed(
        "authority upgrade changed its deferred body completion owner",
    );
    return Err(EnqueueError::FailClosed);
}
let upgraded = match existing
    .clone()
    .with_candidate_semantic_statement(Some(replacement))
{
    Ok(upgraded) => upgraded,
    Err(error) => {
        self.latch_fail_closed(
            "authority upgrade invalidated its deferred body completion owner",
        );
        return Err(error);
    }
};
if prepared.deferred.insert(ordinal, upgraded).is_some() {
    self.latch_fail_closed(
        "authority upgrade duplicated its deferred body completion owner",
    );
    return Err(EnqueueError::FailClosed);
}
""",
                "body-terminal preparation must stage only one exact deferred owner and incumbent authority",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                terminal_prepare,
                sequence,
                description,
                errors,
            )
        if terminal_prepare is not None:
            prepare_tokens = rust_code_tokens(terminal_prepare.source)
            for forbidden in (
                "self.ingress.commands.iter_mut()",
                "self.ingress.reserved_body_available.as_mut()",
                "self.deferred_lifecycle_ownership.get_mut(",
            ):
                if _token_sequence_count(prepare_tokens, rust_code_tokens(forbidden)):
                    errors.append(
                        f"{runtime_path}:{terminal_prepare.line}: body-terminal "
                        "preparation may not mutate live completion authority "
                        f"through {forbidden}"
                    )

        terminal_prepared_commit = _require_rust_item(
            runtime_path,
            runtime_source,
            "commit_prepared_body_pipeline_completion_refinements",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_prepared_commit,
            production_serialized_runtime_context,
            "assignment-only prepared body-terminal authority commit",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            terminal_prepared_commit,
            """
let ingress_targets_exist = prepared.ingress.iter().all(|(target, _)| match target {
    RuntimeBodyCompletionStorageTarget::Queued(admission_ordinal) => self
        .ingress
        .commands
        .iter()
        .any(|queued| queued.admission_ordinal == Some(*admission_ordinal)),
    RuntimeBodyCompletionStorageTarget::Reserved(admission_ordinal) => self
        .ingress
        .reserved_body_available
        .as_ref()
        .is_some_and(|reservation| {
            reservation.admission_ordinal == Some(*admission_ordinal)
        }),
    RuntimeBodyCompletionStorageTarget::Deferred(_) => false,
});
if !ingress_targets_exist
    || !prepared
        .deferred
        .keys()
        .all(|ordinal| self.deferred_lifecycle_ownership.contains_key(ordinal))
{
    self.latch_fail_closed(
        "prevalidated authority upgrade lost its body completion target",
    );
    return Err(EnqueueError::FailClosed);
}
""",
            "prepared body-terminal commit must validate every ingress and deferred target before assignment",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            terminal_prepared_commit,
            """
for (target, replacement) in prepared.ingress {
    match target {
        RuntimeBodyCompletionStorageTarget::Queued(admission_ordinal) => {
            self.ingress
                .commands
                .iter_mut()
                .find(|queued| queued.admission_ordinal == Some(admission_ordinal))
                .expect("prevalidated queued body completion target remains serialized")
                .candidate_semantic_statement = Some(replacement);
        }
        RuntimeBodyCompletionStorageTarget::Reserved(admission_ordinal) => {
            let reservation = self
                .ingress
                .reserved_body_available
                .as_mut()
                .expect("prevalidated body reservation remains serialized");
            debug_assert_eq!(reservation.admission_ordinal, Some(admission_ordinal));
            reservation.candidate_semantic_statement = Some(replacement);
        }
        RuntimeBodyCompletionStorageTarget::Deferred(_) => {
            unreachable!("deferred completion targets use the deferred refinement commit")
        }
    }
}
for (ordinal, replacement) in prepared.deferred {
    *self
        .deferred_lifecycle_ownership
        .get_mut(&ordinal)
        .expect("prevalidated deferred body completion target remains serialized") =
        replacement;
}
""",
            "prepared body-terminal commit must use only the reviewed assignment tail",
            errors,
        )
        if terminal_prepared_commit is not None:
            commit_tokens = rust_code_tokens(terminal_prepared_commit.source)
            preflight_positions = _token_sequence_positions(
                commit_tokens,
                rust_code_tokens("if !ingress_targets_exist || !prepared.deferred.keys().all("),
            )
            mutation_positions = tuple(
                position
                for mutation in (
                    "self.ingress.commands.iter_mut()",
                    "self.ingress.reserved_body_available.as_mut()",
                    "self.deferred_lifecycle_ownership.get_mut(",
                )
                for position in _token_sequence_positions(
                    commit_tokens, rust_code_tokens(mutation)
                )
            )
            if not (
                len(preflight_positions) == 1
                and len(mutation_positions) == 3
                and preflight_positions[0] < min(mutation_positions)
            ):
                errors.append(
                    f"{runtime_path}:{terminal_prepared_commit.line}: prepared "
                    "body-terminal commit must validate every target before its "
                    "three assignment-only mutation seams"
                )

        terminal_lookup = _require_rust_item(
            runtime_path,
            runtime_source,
            "body_pipeline_candidate_terminal_ownership_plan",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_lookup,
            production_serialized_runtime_context,
            "read-only body-terminal ownership lookup",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            terminal_lookup,
            """
fn body_pipeline_candidate_terminal_ownership_plan(
    &mut self,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> Result<Option<RuntimeBodyCompletionOwnershipPlan>, String> {
    if self.fail_closed {
        return Err("Sumeragi v2 body terminal query entered a closed runtime".to_owned());
    }
    if !matches!(
        effect,
        AdapterEffect::StoreBody { .. } | AdapterEffect::ValidateBody { .. }
    ) {
        return Ok(None);
    }
    if !ownership.exactly_binds_adapter_effect(effect) {
        self.latch_fail_closed("body terminal query omitted its exact predecessor capability");
        return Err("Sumeragi v2 body terminal query failed closed".to_owned());
    }

    let mut candidates = self
        .ingress
        .commands
        .iter()
        .filter_map(|queued| {
            let evidence = queued.command.body_pipeline_completion_evidence()?;
            ownership
                .exactly_authorizes_body_pipeline_successor(effect, queued.tag, &evidence)
                .then_some((queued.tag, evidence))
        })
        .collect::<Vec<_>>();
    for (_, tag, evidence) in self.driver.deferred_body_pipeline_terminal_candidates() {
        if !ownership.exactly_authorizes_body_pipeline_successor(effect, tag, &evidence) {
            continue;
        }
        candidates.push((tag, evidence));
    }
    let [(tag, candidate)] = candidates.as_slice() else {
        return if candidates.is_empty() {
            Ok(None)
        } else {
            self.latch_fail_closed(
                "one body candidate retained multiple terminal completion owners",
            );
            Err("Sumeragi v2 body candidate has duplicate terminal owners".to_owned())
        };
    };
    let plan = self
        .resolve_body_pipeline_completion_owner(*tag, candidate, ownership)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            self.latch_fail_closed("body terminal query lost its serialized completion owner");
            "Sumeragi v2 body terminal query lost ownership".to_owned()
        })?;
    Ok(Some(plan))
}
""",
            "body-terminal lookup must select exactly one authorized live or deferred owner without committing it",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            terminal_lookup,
            """
if !matches!(
    effect,
    AdapterEffect::StoreBody { .. } | AdapterEffect::ValidateBody { .. }
) {
    return Ok(None);
}
if !ownership.exactly_binds_adapter_effect(effect) {
    self.latch_fail_closed("body terminal query omitted its exact predecessor capability");
    return Err("Sumeragi v2 body terminal query failed closed".to_owned());
}
""",
            "body-terminal lookup must reject a missing exact Store or Validate predecessor before scanning terminals",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            terminal_lookup,
            """
let mut candidates = self
    .ingress
    .commands
    .iter()
    .filter_map(|queued| {
        let evidence = queued.command.body_pipeline_completion_evidence()?;
        ownership
            .exactly_authorizes_body_pipeline_successor(effect, queued.tag, &evidence)
            .then_some((queued.tag, evidence))
    })
    .collect::<Vec<_>>();
for (_, tag, evidence) in self.driver.deferred_body_pipeline_terminal_candidates() {
    if !ownership.exactly_authorizes_body_pipeline_successor(effect, tag, &evidence) {
        continue;
    }
    candidates.push((tag, evidence));
}
let [(tag, candidate)] = candidates.as_slice() else {
    return if candidates.is_empty() {
        Ok(None)
    } else {
        self.latch_fail_closed(
            "one body candidate retained multiple terminal completion owners",
        );
        Err("Sumeragi v2 body candidate has duplicate terminal owners".to_owned())
    };
};
""",
            "body-terminal lookup must select exactly one fully authorized queued or deferred terminal",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            terminal_lookup,
            """
let plan = self
    .resolve_body_pipeline_completion_owner(*tag, candidate, ownership)
    .map_err(|error| error.to_string())?
    .ok_or_else(|| {
        self.latch_fail_closed("body terminal query lost its serialized completion owner");
        "Sumeragi v2 body terminal query lost ownership".to_owned()
    })?;
Ok(Some(plan))
""",
            "body-terminal lookup must return an uncommitted exact ownership plan",
            errors,
        )
        if terminal_lookup is not None:
            lookup_tokens = rust_code_tokens(terminal_lookup.source)
            for forbidden, description in (
                (
                    "prepare_body_pipeline_completion_refinements",
                    "prepare a terminal authority mutation",
                ),
                (
                    "commit_prepared_body_pipeline_completion_refinements",
                    "commit terminal authority before the executor gate",
                ),
            ):
                if _token_sequence_count(lookup_tokens, rust_code_tokens(forbidden)):
                    errors.append(
                        f"{runtime_path}:{terminal_lookup.line}: read-only body-terminal "
                        f"lookup may not {description}"
                    )

        terminal_plan = _require_rust_item(
            runtime_path,
            runtime_source,
            "plan_body_pipeline_candidate_terminal",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_plan,
            production_serialized_runtime_context,
            "body-terminal incumbent-owner plan",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            terminal_plan,
            """
pub(crate) fn plan_body_pipeline_candidate_terminal(
    &mut self,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> Result<Option<RuntimeEffectOwnership>, String> {
    self.body_pipeline_candidate_terminal_ownership_plan(effect, ownership)?
        .map(|plan| plan.adopt_effect_ownership(effect, ownership))
        .transpose()
}
""",
            "body-terminal plan must adopt the incumbent owner without committing runtime state",
            errors,
        )

        terminal_commits = _require_rust_item(
            runtime_path,
            runtime_source,
            "commit_body_pipeline_candidate_terminals",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_commits,
            production_serialized_runtime_context,
            "checked atomic body-terminal authority commit",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            terminal_commits,
            """
pub(crate) fn commit_body_pipeline_candidate_terminals(
    &mut self,
    terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
) -> Result<(), String> {
let mut plans = Vec::with_capacity(terminals.len());
for (effect, ownership) in terminals {
    let plan = self
        .body_pipeline_candidate_terminal_ownership_plan(effect, ownership)?
        .ok_or_else(|| {
            self.latch_fail_closed(
                "body terminal refinement lost its serialized completion owner",
            );
            "Sumeragi v2 body terminal refinement lost ownership".to_owned()
        })?;
    plans.push(plan);
}
let prepared = self
    .prepare_body_pipeline_completion_refinements(&plans)
    .map_err(|error| error.to_string())?;
self.commit_prepared_body_pipeline_completion_refinements(prepared)
    .map_err(|error| error.to_string())
}
""",
            "body-terminal batch commit must revalidate every target, prepare atomically, and then commit the checked authorities",
            errors,
        )

        terminal_commit = _require_rust_item(
            runtime_path,
            runtime_source,
            "commit_body_pipeline_candidate_terminal",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            terminal_commit,
            production_serialized_runtime_context,
            "checked single body-terminal authority commit wrapper",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            terminal_commit,
            """
pub(crate) fn commit_body_pipeline_candidate_terminal(
    &mut self,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> Result<(), String> {
    self.commit_body_pipeline_candidate_terminals(&[(effect, ownership)])
}
""",
            "single body-terminal commit must delegate to the atomic batch boundary",
            errors,
        )

        statement_structs = rust_struct_items(
            runtime_source, "RuntimeCandidateSemanticStatement"
        )
        if len(statement_structs) != 1:
            errors.append(
                f"{runtime_path}: require exactly one real internal Rust struct "
                "item named RuntimeCandidateSemanticStatement; found "
                f"{len(statement_structs)}"
            )
            statement_struct = None
        else:
            statement_struct = statement_structs[0]
            _require_rust_item_context(
                runtime_path,
                statement_struct,
                (),
                "typed non-serialized candidate semantic statement",
                errors,
                expected_attributes=(
                    "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
                ),
            )
            if _rust_item_header_tokens(statement_struct) != rust_code_tokens(
                "pub(crate) struct RuntimeCandidateSemanticStatement"
            ):
                errors.append(
                    f"{runtime_path}:{statement_struct.line}: candidate semantic "
                    "statement must remain crate-private and non-serialized"
                )
            _require_exact_rust_tokens(
                runtime_path,
                statement_struct,
                """
pub(crate) struct RuntimeCandidateSemanticStatement {
    context_id: wire::HeightContextId,
    round: wire::ConsensusRound,
    proposal_round: wire::ConsensusRound,
    subject: Option<wire::BlockSubject>,
    phase: Option<wire::GlobalPhase>,
    execution_commitment: Option<wire::ExecutionCommitment>,
}
""",
                "candidate semantic statement must contain exactly the six frozen coordinates",
                errors,
            )

        statement_new = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeCandidateSemanticStatement",
            "new",
            errors,
            "candidate semantic statement constructor",
        )
        _require_exact_rust_tokens(
            runtime_path,
            statement_new,
            """
fn new(
    round: wire::ConsensusRound,
    proposal_round: wire::ConsensusRound,
    subject: Option<wire::BlockSubject>,
    phase: Option<wire::GlobalPhase>,
    execution_commitment: Option<wire::ExecutionCommitment>,
) -> Self {
    Self {
        context_id: round.context_id,
        round,
        proposal_round,
        subject,
        phase,
        execution_commitment,
    }
}
""",
            "candidate semantic statement must freeze context from its consensus round",
            errors,
        )
        statement_validate = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeCandidateSemanticStatement",
            "validate_exact",
            errors,
            "candidate semantic statement exact validator",
        )
        _require_exact_rust_tokens(
            runtime_path,
            statement_validate,
            """
fn validate_exact(self) -> bool {
    self.context_id == self.round.context_id
        && self.context_id == self.proposal_round.context_id
        && self.round.height == self.proposal_round.height
        && self.phase.is_some() == self.execution_commitment.is_some()
        && self.phase.is_none_or(|_| self.subject.is_some())
        && self
            .execution_commitment
            .is_none_or(|_| self.subject.is_some())
}
""",
            "candidate semantic statement must validate context, height, subject, phase, and commitment as one exact tuple",
            errors,
        )
        statement_refinement = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeCandidateSemanticStatement",
            "commit_refinement_to",
            errors,
            "candidate authority refinement gate",
        )
        _require_exact_rust_tokens(
            runtime_path,
            statement_refinement,
            """
fn commit_refinement_to(
    self,
    successor: Self
) -> Option<RuntimeCandidateAuthorityRefinement> {
    if !self.validate_exact()
        || !successor.validate_exact()
        || successor.phase != Some(wire::GlobalPhase::Commit)
        || self.context_id != successor.context_id
        || self.round != successor.round
        || self.proposal_round != successor.proposal_round
        || self.subject != successor.subject
    {
        return None;
    }
    match (self.phase, self.execution_commitment) {
        (None, None) => Some(RuntimeCandidateAuthorityRefinement::AcquireCommit),
        (Some(wire::GlobalPhase::Prepare), Some(commitment))
            if successor.execution_commitment == Some(commitment) =>
        {
            Some(RuntimeCandidateAuthorityRefinement::PromotePrepare)
        }
        (Some(wire::GlobalPhase::Commit), Some(_)) if self == successor => {
            Some(RuntimeCandidateAuthorityRefinement::RetainCommit)
        }
        _ => None,
    }
}
""",
            "candidate authority refinement must freeze identity and admit only local acquisition, matching Prepare promotion, or exact Commit retention",
            errors,
        )
        _require_rust_source_token_sequence(
            runtime_path,
            runtime_source,
            """
enum RuntimeCandidateAuthorityRefinement {
    AcquireCommit,
    PromotePrepare,
    RetainCommit,
}
""",
            "candidate authority refinement outcomes must remain internal and closed",
            errors,
        )
        statement_identity = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeCandidateSemanticStatement",
            "semantic_identity",
            errors,
            "six-coordinate candidate semantic identity",
        )
        _require_exact_rust_tokens(
            runtime_path,
            statement_identity,
            """
fn semantic_identity(self) -> Vec<u8> {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"iroha:sumeragi:v2:tla-candidate-semantic:v2");
    append_runtime_identity_field(&mut identity, &self.context_id.encode());
    append_runtime_identity_field(&mut identity, &self.round.encode());
    append_runtime_identity_field(&mut identity, &self.proposal_round.encode());
    append_optional_runtime_identity_bytes(
        &mut identity,
        self.subject.map(|subject| subject.encode()),
    );
    append_optional_runtime_identity_bytes(
        &mut identity,
        self.phase.map(|phase| phase.encode()),
    );
    append_optional_runtime_identity_bytes(
        &mut identity,
        self.execution_commitment
            .map(|commitment| commitment.encode()),
    );
    identity
}
""",
            "candidate semantic identity must encode exactly the frozen six-coordinate statement under the v2 domain",
            errors,
        )
        if statement_identity is not None:
            expected_domain = (
                'b"iroha:sumeragi:v2:tla-candidate-semantic:v2"'
            )
            observed_byte_literals = re.findall(
                r'b"(?:\\.|[^"\\])*"', statement_identity.source
            )
            if observed_byte_literals != [expected_domain]:
                errors.append(
                    f"{runtime_path}:{statement_identity.line}: candidate semantic "
                    "identity must encode exactly the frozen six-coordinate "
                    "statement under the v2 domain; expected one byte literal "
                    f"{expected_domain!r}, found {observed_byte_literals!r}"
                )

        _require_rust_source_token_sequence(
            runtime_path,
            runtime_source,
            """
pub(crate) struct RuntimeEffectCandidateSemantic {
    kind: u8,
    semantic_identity: Vec<u8>,
    statement: Option<RuntimeCandidateSemanticStatement>,
}
""",
            "every production candidate binding must retain its independently typed statement",
            errors,
        )
        binding_structs = rust_struct_items(
            runtime_source, "RuntimeEffectCandidateBinding"
        )
        if len(binding_structs) != 1:
            errors.append(
                f"{runtime_path}: require exactly one real Rust struct item named "
                "RuntimeEffectCandidateBinding; found "
                f"{len(binding_structs)}"
            )
        else:
            _require_rust_token_sequence(
                runtime_path,
                binding_structs[0],
                "candidate_statement: Option<RuntimeCandidateSemanticStatement>",
                "effect-to-candidate binding must store the typed statement independently of its hashes",
                errors,
            )
        binding_projection_parts_structs = rust_struct_items(
            runtime_source, "RuntimeEffectCandidateBindingProjectionParts"
        )
        if len(binding_projection_parts_structs) != 1:
            errors.append(
                f"{runtime_path}: require exactly one real Rust struct item named "
                "RuntimeEffectCandidateBindingProjectionParts; found "
                f"{len(binding_projection_parts_structs)}"
            )
        else:
            binding_projection_parts_struct = binding_projection_parts_structs[0]
            _require_rust_item_context(
                runtime_path,
                binding_projection_parts_struct,
                (),
                "immutable candidate binding projection-parts value",
                errors,
                expected_attributes=("#[derive(Clone, Copy)]",),
            )
            _require_exact_rust_tokens(
                runtime_path,
                binding_projection_parts_struct,
                """
struct RuntimeEffectCandidateBindingProjectionParts<'a> {
    owner_projection_hash: &'a iroha_crypto::Hash,
    parent_owner_projection_hash: Option<&'a iroha_crypto::Hash>,
    effect_kind: u8,
    effect_identity: &'a iroha_crypto::Hash,
    candidate_kind: u8,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    candidate_semantic_identity: Option<&'a iroha_crypto::Hash>,
    candidate_identity: Option<&'a iroha_crypto::Hash>,
    effect_position: u8,
    effect_count: u8,
    candidate_position: u8,
    candidate_count: u8,
}
""",
                "candidate binding projection parts must borrow exactly the complete immutable preimage",
                errors,
            )
        required_statement = _require_rust_item(
            runtime_path,
            runtime_source,
            "runtime_candidate_kind_requires_statement",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            required_statement,
            (),
            "closed production candidate statement requirement",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            required_statement,
            """
fn runtime_candidate_kind_requires_statement(kind: u8) -> bool {
    matches!(
        kind,
        RUNTIME_CANDIDATE_KIND_SIGN_PROPOSAL
            | RUNTIME_CANDIDATE_KIND_SIGN_VOTE
            | RUNTIME_CANDIDATE_KIND_SIGN_TIMEOUT
            | RUNTIME_CANDIDATE_KIND_FETCH_BODY
            | RUNTIME_CANDIDATE_KIND_STORE_BODY
            | RUNTIME_CANDIDATE_KIND_VALIDATE_BODY
            | RUNTIME_CANDIDATE_KIND_APPLY
    )
}
""",
            "all seven production candidate kinds must require typed statement evidence",
            errors,
        )

        statement_builder = _require_rust_item(
            runtime_path,
            runtime_source,
            "production_adapter_effect_candidate_statement",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            statement_builder,
            (),
            "closed adapter candidate statement builder",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            statement_builder,
            """
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
            RuntimeCandidateSemanticStatement::new(
                *round,
                *round,
                Some(*subject),
                None,
                None
            ),
        ),
        AdapterEffect::ValidateBody { round, subject, .. } => (
            RUNTIME_CANDIDATE_KIND_VALIDATE_BODY,
            RuntimeCandidateSemanticStatement::new(
                *round,
                *round,
                Some(*subject),
                None,
                None
            ),
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
""",
            "all eleven adapter-effect classes must map through the exact seven-candidate statement table",
            errors,
        )

        candidate_binding = _require_rust_item(
            runtime_path,
            runtime_source,
            "production_adapter_effect_candidate_binding",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            candidate_binding,
            (),
            "candidate statement inheritance and refinement gate",
            errors,
        )
        for sequence, description in (
            (
                """
AdapterEffect::FetchBody {
    round,
    subject,
    certificate: Some(certificate),
    ..
} if certificate.proposal_round != *round || certificate.subject != *subject => {
    return Err(
        "Sumeragi v2 certified Fetch disagreed with its proposal-round body key"
            .to_owned(),
    );
}
""",
                "certified Fetch must reject a mismatched proposal round or subject",
            ),
            (
                """
AdapterEffect::Apply {
    subject,
    certificate,
    ..
} if certificate.phase != wire::GlobalPhase::Commit
    || certificate.subject != *subject =>
{
    return Err("Sumeragi v2 Apply omitted its exact Commit authority".to_owned());
}
""",
                "Apply must reject non-Commit or foreign-subject authority",
            ),
            (
                """
if !statement.validate_exact() {
    return Err(
        "Sumeragi v2 candidate statement had inconsistent context or height".to_owned(),
    );
}
""",
                "every derived statement must pass exact validation before inheritance",
            ),
            (
                """
if !parent.validate_exact() {
    return Err("Sumeragi v2 causal parent lost its exact candidate statement".to_owned());
}
""",
                "every inherited parent statement must be exact before refinement",
            ),
            (
                """
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
    statement = *parent;
}
""",
                "Store and Validate must inherit the exact frozen Fetch statement",
            ),
            (
                """
AdapterEffect::Apply { .. } => {
    if parent.commit_refinement_to(statement).is_none() {
        return Err(
            "Sumeragi v2 Apply changed its inherited candidate authority".to_owned(),
        );
    }
}
""",
                "Apply must pass the authority refinement gate before statement publication",
            ),
            (
                """
let semantic_identity = statement.semantic_identity();
Ok(Some(RuntimeEffectCandidateSemantic {
    kind,
    semantic_identity,
    statement: Some(statement),
}))
""",
                "candidate binding must publish matching typed and byte semantic evidence",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                candidate_binding,
                sequence,
                description,
                errors,
            )

        semantic = _require_rust_item(
            runtime_path,
            runtime_source,
            "production_adapter_effect_candidate_semantic_identity",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            semantic,
            (),
            "route-neutral adapter candidate semantic identity",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            semantic,
            """
pub(crate) fn production_adapter_effect_candidate_semantic_identity(
    effect: &AdapterEffect,
) -> Option<(u8, Vec<u8>)> {
    let (kind, statement) = production_adapter_effect_candidate_statement(effect)?;
    statement
        .validate_exact()
        .then(|| (kind, statement.semantic_identity()))
}
""",
            "candidate semantic identity must be derived only from a validated typed statement",
            errors,
        )

        production_binding_context = (
            ("impl", "RuntimeDriver", "for", "SumeragiV2Adapter"),
        )
        production_bindings = [
            item
            for item in rust_items(
                runtime_source, "effect_candidate_semantic_binding"
            )
            if item.brace_context == production_binding_context
        ]
        if len(production_bindings) != 1:
            errors.append(
                f"{runtime_path}: require exactly one production RuntimeDriver "
                "candidate semantic binding override; found "
                f"{len(production_bindings)}"
            )
        else:
            production_binding = production_bindings[0]
            _require_rust_item_context(
                runtime_path,
                production_binding,
                production_binding_context,
                "production typed candidate semantic binding override",
                errors,
            )
            _require_exact_rust_tokens(
                runtime_path,
                production_binding,
                """
fn effect_candidate_semantic_binding(
    &self,
    effect: &Self::Effect,
    inherited: Option<&RuntimeCandidateSemanticStatement>,
) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
    let effective_inherited = match effect {
        AdapterEffect::StoreBody { round, subject, .. }
        | AdapterEffect::ValidateBody { round, subject, .. } => {
            match self
                .replayed_decision_key()
                .map_err(|error| error.to_string())?
            {
                Some((decision_round, proposal_round, decision_subject, commitment))
                    if *round == proposal_round && *subject == decision_subject =>
                {
                    let decision_statement = RuntimeCandidateSemanticStatement::new(
                        decision_round,
                        proposal_round,
                        Some(decision_subject),
                        Some(wire::GlobalPhase::Commit),
                        Some(commitment),
                    );
                    if inherited.is_some_and(|parent| {
                        parent.commit_refinement_to(decision_statement).is_none()
                    }) {
                        return Err(
                            "Sumeragi v2 durable Decision body recovery conflicted with its causal authority"
                                .to_owned(),
                        );
                    }
                    Some(decision_statement)
                }
                Some(_) | None => inherited.copied(),
            }
        }
        AdapterEffect::Sign { .. }
        | AdapterEffect::Broadcast(_)
        | AdapterEffect::FetchBody { .. }
        | AdapterEffect::Apply { .. }
        | AdapterEffect::EnterView { .. }
        | AdapterEffect::ReportEquivocation { .. }
        | AdapterEffect::ReportInvalidCertifiedBody { .. } => inherited.copied(),
    };
    production_adapter_effect_candidate_binding(effect, effective_inherited.as_ref())
}
""",
                "production RuntimeDriver must route every candidate through the typed inheritance and refinement gate",
                errors,
            )
            _require_rust_token_sequence(
                runtime_path,
                production_binding,
                """
Some((decision_round, proposal_round, decision_subject, commitment))
    if *round == proposal_round && *subject == decision_subject =>
{
    let decision_statement = RuntimeCandidateSemanticStatement::new(
        decision_round,
        proposal_round,
        Some(decision_subject),
        Some(wire::GlobalPhase::Commit),
        Some(commitment),
    );
""",
                "durable Decision body recovery must reconstruct Commit authority only for the exact proposal round and subject",
                errors,
            )
            _require_rust_token_sequence(
                runtime_path,
                production_binding,
                """
if inherited.is_some_and(|parent| {
    parent.commit_refinement_to(decision_statement).is_none()
}) {
    return Err(
        "Sumeragi v2 durable Decision body recovery conflicted with its causal authority"
            .to_owned(),
    );
}
Some(decision_statement)
""",
                "durable Decision body recovery must reject an incompatible inherited authority before publication",
                errors,
            )
            _require_rust_token_sequence(
                runtime_path,
                production_binding,
                """
Some(_) | None => inherited.copied(),
""",
                "nonmatching or absent durable Decision recovery must preserve only the inherited authority",
                errors,
            )

        binding_projection = _require_rust_item(
            runtime_path,
            runtime_source,
            "runtime_effect_candidate_binding_projection_hash",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            binding_projection,
            (),
            "typed candidate binding projection",
            errors,
        )
        _require_exact_rust_tokens(
            runtime_path,
            binding_projection,
            """
fn runtime_effect_candidate_binding_projection_hash(
    owner: &RuntimeLifecycleOwner,
    causality: RuntimeEffectCausality,
    parts: RuntimeEffectCandidateBindingProjectionParts<'_>,
) -> iroha_crypto::Hash {
    let mut projection = Vec::new();
    projection.extend_from_slice(b"iroha:sumeragi:v2:runtime-effect-binding:v1");
    append_runtime_identity_field(&mut projection, owner.projection_hash.as_ref());
    projection.push(runtime_effect_causality_code(causality));
    projection.push(runtime_effect_fresh_root_code(causality));
    append_runtime_identity_field(&mut projection, parts.owner_projection_hash.as_ref());
    append_optional_runtime_hash(&mut projection, parts.parent_owner_projection_hash);
    projection.push(parts.effect_kind);
    append_runtime_identity_field(&mut projection, parts.effect_identity.as_ref());
    projection.push(parts.candidate_kind);
match parts.candidate_statement {
    None => projection.push(0),
    Some(statement) => {
        projection.push(1);
        append_runtime_identity_field(&mut projection, &statement.semantic_identity());
    }
}
append_optional_runtime_hash(&mut projection, parts.candidate_semantic_identity);
    append_optional_runtime_hash(&mut projection, parts.candidate_identity);
    projection.push(parts.effect_position);
    projection.push(parts.effect_count);
    projection.push(parts.candidate_position);
    projection.push(parts.candidate_count);
    iroha_crypto::Hash::new(projection)
}
""",
            "candidate binding projection must independently commit the typed statement before its semantic hash",
            errors,
        )

        binding_parts = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeEffectCandidateBinding",
            "projection_parts",
            errors,
            "immutable candidate binding projection parts",
        )
        _require_exact_rust_tokens(
            runtime_path,
            binding_parts,
            """
fn projection_parts(&self) -> RuntimeEffectCandidateBindingProjectionParts<'_> {
    RuntimeEffectCandidateBindingProjectionParts {
        owner_projection_hash: &self.owner_projection_hash,
        parent_owner_projection_hash: self.parent_owner_projection_hash.as_ref(),
        effect_kind: self.effect_kind,
        effect_identity: &self.effect_identity,
        candidate_kind: self.candidate_kind,
        candidate_statement: self.candidate_statement,
        candidate_semantic_identity: self.candidate_semantic_identity.as_ref(),
        candidate_identity: self.candidate_identity.as_ref(),
        effect_position: self.effect_position,
        effect_count: self.effect_count,
        candidate_position: self.candidate_position,
        candidate_count: self.candidate_count,
    }
}
""",
            "candidate binding validation must borrow every immutable projection field exactly once",
            errors,
        )

        binding_new = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeEffectCandidateBinding",
            "new",
            errors,
            "typed candidate binding constructor",
            expected_attributes=("#[allow(clippy::too_many_arguments)]",),
        )
        for sequence, description in (
            (
                """
candidate.statement.is_none_or(|statement| {
    statement.validate_exact()
        && statement.semantic_identity().as_slice()
            == candidate.semantic_identity.as_slice()
})
&& (!runtime_candidate_kind_requires_statement(candidate.kind)
    || candidate.statement.is_some())
""",
                "candidate binding construction must require matching typed evidence for every production candidate kind",
            ),
            (
                """
(
    candidate.kind,
    candidate.statement,
    Some(semantic_identity),
    Some(candidate_identity),
)
""",
                "candidate binding construction must retain the validated typed statement",
            ),
            (
                """
candidate_kind,
candidate_statement,
candidate_semantic_identity,
candidate_identity,
""",
                "candidate binding construction must publish statement and both derived identities together",
            ),
            (
                """
let projection_hash = runtime_effect_candidate_binding_projection_hash(
    owner,
    causality,
    RuntimeEffectCandidateBindingProjectionParts {
        owner_projection_hash: &owner_projection_hash,
        parent_owner_projection_hash: parent_owner_projection_hash.as_ref(),
        effect_kind,
        effect_identity: &effect_identity,
        candidate_kind,
        candidate_statement,
        candidate_semantic_identity: candidate_semantic_identity.as_ref(),
        candidate_identity: candidate_identity.as_ref(),
        effect_position,
        effect_count,
        candidate_position,
        candidate_count,
    },
);
""",
                "candidate binding construction must hash immutable projection parts before publication",
            ),
            (
                """
let binding = Self {
    owner_projection_hash,
    parent_owner_projection_hash,
    effect_kind,
    effect_identity,
    candidate_kind,
    candidate_statement,
    candidate_semantic_identity,
    candidate_identity,
    effect_position,
    effect_count,
    candidate_position,
    candidate_count,
    projection_hash,
};
""",
                "candidate binding construction must publish one immutable fully hashed binding",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                binding_new,
                sequence,
                description,
                errors,
            )
        _require_rust_token_sequence(
            runtime_path,
            binding_new,
            "let mut binding = Self",
            "candidate binding construction must not mutate a partially initialized binding",
            errors,
            count=0,
        )
        _require_rust_token_sequence(
            runtime_path,
            binding_new,
            "projection_hash: iroha_crypto::Hash::new([])",
            "candidate binding construction must not publish an empty projection-hash sentinel",
            errors,
            count=0,
        )

        binding_validate = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeEffectCandidateBinding",
            "validate_exact",
            errors,
            "typed candidate binding validator",
        )
        _require_rust_token_sequence(
            runtime_path,
            binding_validate,
            """
(
    candidate_kind,
    candidate_statement,
    Some(semantic_identity),
    Some(candidate_identity),
) => {
    candidate_kind != RUNTIME_CANDIDATE_KIND_NONE
        && self.candidate_count != 0
        && self.candidate_position != 0
        && self.candidate_position <= self.candidate_count
        && (!runtime_candidate_kind_requires_statement(candidate_kind)
            || candidate_statement.is_some())
        && candidate_statement.is_none_or(|statement| {
            statement.validate_exact()
                && *semantic_identity
                    == runtime_effect_candidate_semantic_hash(
                        candidate_kind,
                        &statement.semantic_identity(),
                    )
        })
        && *candidate_identity
            == runtime_effect_candidate_identity_hash(
                owner,
                candidate_kind,
                semantic_identity,
            )
}
""",
            "candidate binding validation must rederive semantic and concrete identity from the retained typed statement and owner",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            binding_validate,
            """
self.projection_hash
    == runtime_effect_candidate_binding_projection_hash(
        owner,
        causality,
        self.projection_parts(),
    )
""",
            "candidate binding validation must rederive the immutable complete projection hash",
            errors,
        )

        ownership_statement = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeEffectOwnership",
            "candidate_semantic_statement",
            errors,
            "typed candidate ownership accessor",
        )
        _require_exact_rust_tokens(
            runtime_path,
            ownership_statement,
            """
fn candidate_semantic_statement(&self) -> Option<RuntimeCandidateSemanticStatement> {
    self.binding
        .as_ref()
        .and_then(|binding| binding.candidate_statement)
}
""",
            "effect ownership must expose exactly the statement frozen in its validated binding",
            errors,
        )
        for rebind_name, description in (
            (
                "rebind_as_inherited_adapter_effect",
                "causal successor rebind must inherit the incumbent statement",
            ),
            (
                "rebind_same_adapter_effect",
                "same-effect retry rebind must inherit the incumbent statement",
            ),
        ):
            rebind = _require_qualified_rust_item(
                runtime_path,
                runtime_source,
                "RuntimeEffectOwnership",
                rebind_name,
                errors,
                description,
            )
            _require_rust_token_sequence(
                runtime_path,
                rebind,
                """
let inherited = self.candidate_semantic_statement();
let candidate = production_adapter_effect_candidate_binding(effect, inherited.as_ref())?;
""",
                description,
                errors,
            )

        trace_projection = _require_rust_item(
            runtime_path,
            runtime_source,
            "production_adapter_effect_candidate_trace_projection",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            trace_projection,
            (),
            "typed effect-to-candidate trace projection",
            errors,
            expected_attributes=("#[allow(clippy::too_many_arguments)]",),
        )
        _require_rust_token_sequence(
            runtime_path,
            trace_projection,
            """
let candidate = production_adapter_effect_candidate_binding(
    effect,
    binding.candidate_statement.as_ref()
)?;
""",
            "effect-to-candidate refinement must recompute from the independently retained typed statement",
            errors,
        )

        retain_runtime_effects = _require_rust_item(
            runtime_path,
            runtime_source,
            "retain_effect_ownership",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            retain_runtime_effects,
            serialized_runtime_context,
            "typed statement effect-batch retention",
            errors,
        )
        for sequence, description in (
            (
                """
fn retain_effect_ownership(
    &mut self,
    source: RuntimeEffectSource,
    parent: Option<&RuntimeLifecycleOwner>,
    parent_statement: Option<&RuntimeCandidateSemanticStatement>,
    effects: &[D::Effect],
) -> Result<(), EnqueueError>
""",
                "effect-batch retention must receive the selected parent's typed statement",
            ),
            (
                """
self.driver.effect_candidate_semantic_binding(
    effect,
    matches!(causality, RuntimeEffectCausality::Inherit)
        .then_some(parent_statement)
        .flatten(),
)
""",
                "only causally inherited effects may receive the parent statement",
            ),
            (
                """
let evidence = evidence.bind_runtime_effect(
    matches!(causality, RuntimeEffectCausality::Inherit)
        .then_some(parent)
        .flatten(),
    D::effect_refinement_kind(effect),
    &D::effect_semantic_identity(effect),
    candidate.as_ref(),
""",
                "effect retention must bind the derived typed candidate before publication",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                retain_runtime_effects,
                sequence,
                description,
                errors,
            )

        enqueue_successor = _require_rust_item(
            runtime_path,
            runtime_source,
            "enqueue_with_lifecycle_owner",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            enqueue_successor,
            serialized_runtime_context,
            "candidate statement causal-successor ingress handoff",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            enqueue_successor,
            """
tagged.candidate_semantic_statement = ownership.candidate_semantic_statement();
if !tagged.validate_admission_identity() {
    self.latch_fail_closed("causal-successor candidate statement was invalid");
    return Err(EnqueueError::FailClosed);
}
let result = self.enqueue_after_clock_reservation(tagged);
""",
            "causal-successor ingress must copy and validate the exact statement before publication",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            enqueue_successor,
            """
let preflight = self.command_admission_preflight(tag, class, &command)?;
if self.owned_preflight_is_coalesced(tag, &command, preflight, ownership)? {
    return Ok(());
}
let mut tagged = match preflight {
""",
            "owned causal successors must validate a retained coalescence owner before admission",
            errors,
        )

        owned_coalescence = _require_rust_item(
            runtime_path,
            runtime_source,
            "owned_preflight_is_coalesced",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            owned_coalescence,
            serialized_runtime_context,
            "exact retained-owner coalescence gate",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            owned_coalescence,
            """
RuntimeCommandAdmissionPreflight::CoalesceOwned {
    causal_lifecycle_key,
    admission_ordinal,
} => {
    let owner = ownership.owner();
    if owner.causal_origin().lifecycle_key != causal_lifecycle_key
        || owner.lifecycle_ordinal() != admission_ordinal
    {
        self.latch_fail_closed(
            "coalesced runtime command changed its retained lifecycle owner",
        );
        return Err(EnqueueError::FailClosed);
    }
    Ok(true)
}
""",
            "terminal coalescence must compare both retained lifecycle coordinates and fail closed on replacement",
            errors,
        )

        owned_completion = _require_rust_item(
            runtime_path,
            runtime_source,
            "body_pipeline_completion_is_owned_by",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            owned_completion,
            production_serialized_runtime_context,
            "in-flight exact body-completion owner gate",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            owned_completion,
            """
let Some(plan) = self.resolve_body_pipeline_completion_owner(tag, candidate, ownership)?
else {
    return Ok(None);
};
let result = (plan.retained_owner.clone(), plan.effective_statement());
let prepared =
    self.prepare_body_pipeline_completion_refinements(std::slice::from_ref(&plan))?;
self.commit_prepared_body_pipeline_completion_refinements(prepared)?;
Ok(Some(result))
""",
            "in-flight body completion coalescence must resolve, refine, and return its one exact incumbent owner",
            errors,
        )

        ingress_completion_owners = _require_rust_item(
            runtime_path,
            runtime_source,
            "exact_body_pipeline_completion_owners",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            ingress_completion_owners,
            bounded_ingress_context,
            "in-flight ingress body-completion owner inventory",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            ingress_completion_owners,
            """
queued.command.body_pipeline_completion_ownership(candidate) == Some(true)
""",
            "in-flight ingress inventory must return only full-evidence exact owners",
            errors,
        )

        enqueue_owned_completion = _require_rust_item(
            runtime_path,
            runtime_source,
            "enqueue_body_pipeline_completion_with_owner",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            enqueue_owned_completion,
            production_serialized_runtime_context,
            "owned body-completion enqueue gate",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            enqueue_owned_completion,
            """
if self
    .body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
    .is_some()
{
    return Ok(());
}
self.enqueue_with_lifecycle_owner(tag, CommandClass::Completion, command, ownership)
""",
            "owned body-completion retries must compare the incumbent before queue coalescence",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            owned_coalescence,
            """
RuntimeCommandAdmissionPreflight::Coalesce
    if tag != self.driver.current_tag() =>
{
    Ok(true)
}
RuntimeCommandAdmissionPreflight::Coalesce
    if self
        .driver
        .owned_terminal_completion_matches_effect(tag, command, ownership) =>
{
    Ok(true)
}
RuntimeCommandAdmissionPreflight::Coalesce => {
    self.latch_fail_closed(
        "owned runtime command reached current-tag coalescence without an exact owner",
    );
    Err(EnqueueError::FailClosed)
}
""",
            "only stale callbacks may use ownerless coalescence on an owned ingress path",
            errors,
        )

        take_runtime_ownership = _require_rust_item(
            runtime_path,
            runtime_source,
            "take_effect_ownership",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            take_runtime_ownership,
            serialized_runtime_context,
            "runtime effect-ownership fail-closed handoff",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            take_runtime_ownership,
            """
let Some(ownership) = self.pending_effect_ownership.take() else {
    self.latch_fail_closed("effect batch omitted its lifecycle ownership");
    return Err("Sumeragi v2 effect batch omitted its lifecycle ownership".to_owned());
};
""",
            "a missing nonempty runtime effect sidecar must latch fail closed before returning",
            errors,
        )

        coalesced_body = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "BodyAvailableReservation",
            "coalesced_with_owner",
            errors,
            "owned coalesced BodyAvailable token",
        )
        _require_rust_token_sequence(
            runtime_path,
            coalesced_body,
            """
reservation.lifecycle_ordinal = Some(ownership.owner().lifecycle_ordinal());
reservation.causal_origin = Some(ownership.owner().causal_origin().clone());
reservation.candidate_semantic_statement = ownership.candidate_semantic_statement();
Ok(reservation)
""",
            "coalesced BodyAvailable tokens must retain owner coordinates and the candidate statement",
            errors,
        )

        tagged_structs = rust_struct_items(runtime_source, "TaggedCommand")
        if len(tagged_structs) != 1:
            errors.append(
                f"{runtime_path}: require exactly one real Rust struct item named "
                f"TaggedCommand; found {len(tagged_structs)}"
            )
        else:
            _require_rust_token_sequence(
                runtime_path,
                tagged_structs[0],
                "candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>",
                "queued runtime commands must retain the candidate statement",
                errors,
            )
        tagged_context = (
            (
                "impl",
                "<",
                "C",
                ":",
                "ExactRuntimeCommandIdentity",
                ">",
                "TaggedCommand",
                "<",
                "C",
                ">",
            ),
        )
        tagged_candidates = [
            item
            for item in rust_items(runtime_source, "validate_admission_identity")
            if item.brace_context == tagged_context
        ]
        if len(tagged_candidates) != 1:
            errors.append(
                f"{runtime_path}: require exactly one TaggedCommand candidate "
                "statement admission validator; found "
                f"{len(tagged_candidates)}"
            )
            tagged_validate = None
        else:
            tagged_validate = tagged_candidates[0]
            _require_rust_item_context(
                runtime_path,
                tagged_validate,
                tagged_context,
                "queued candidate statement admission validation",
                errors,
            )
        _require_rust_token_sequence(
            runtime_path,
            tagged_validate,
            "self.validate_cached_admission_identity()",
            "deep queued admission validation must retain the constant-size candidate seal",
            errors,
        )
        tagged_cached_candidates = [
            item
            for item in rust_items(runtime_source, "validate_cached_admission_identity")
            if item.brace_context == tagged_context
        ]
        if len(tagged_cached_candidates) != 1:
            errors.append(
                f"{runtime_path}: require exactly one TaggedCommand cached "
                "candidate-statement admission validator; found "
                f"{len(tagged_cached_candidates)}"
            )
            tagged_cached_validate = None
        else:
            tagged_cached_validate = tagged_cached_candidates[0]
            _require_rust_item_context(
                runtime_path,
                tagged_cached_validate,
                tagged_context,
                "queued constant-size candidate statement admission validation",
                errors,
            )
        _require_rust_token_sequence(
            runtime_path,
            tagged_cached_validate,
            """
self.candidate_semantic_statement.is_none_or(|statement| {
    statement.validate_exact() && statement.round.height == self.tag.height()
})
""",
            "queued candidate statements must remain exact and height-scoped",
            errors,
        )

        reservation_structs = rust_struct_items(
            runtime_source, "BodyAvailableReservation"
        )
        if len(reservation_structs) != 1:
            errors.append(
                f"{runtime_path}: require exactly one real Rust struct item "
                "named BodyAvailableReservation; found "
                f"{len(reservation_structs)}"
            )
        else:
            _require_rust_token_sequence(
                runtime_path,
                reservation_structs[0],
                "candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>",
                "BodyAvailable capacity reservation must retain the candidate statement",
                errors,
            )
        reserve_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "reserve_canonical_body_available_internal",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            reserve_body,
            bounded_ingress_context,
            "candidate statement BodyAvailable capacity reservation",
            errors,
        )
        for sequence, description in (
            (
                "prospective.candidate_semantic_statement = candidate_semantic_statement;",
                "BodyAvailable preflight must install the inherited statement on the prospective command",
            ),
            (
                "existing.candidate_semantic_statement == candidate_semantic_statement",
                "BodyAvailable coalescing must compare the complete inherited statement",
            ),
            (
                "reservation.candidate_semantic_statement = candidate_semantic_statement;",
                "BodyAvailable reservation publication must retain the inherited statement",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                reserve_body,
                sequence,
                description,
                errors,
            )
        commit_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "commit_canonical_body_available",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            commit_body,
            bounded_ingress_context,
            "candidate statement BodyAvailable materialization",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            commit_body,
            """
command.candidate_semantic_statement = reservation.candidate_semantic_statement;
command.restored_producer_stage = reservation.restored_producer_stage;
command.causal_origin = reservation
    .causal_origin
    .clone()
    .ok_or(EnqueueError::FailClosed)?;
if !command.validate_admission_identity() {
    return Err(EnqueueError::FailClosed);
}
""",
            "BodyAvailable materialization must copy and validate the reserved statement before ingress",
            errors,
        )
        reserve_owned_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "reserve_body_available_with_owner",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            reserve_owned_body,
            production_serialized_runtime_context,
            "owned BodyAvailable statement handoff",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
if self.ingress.reserved_body_available.is_some() {
    if !self.body_pipeline_completion_is_owned(tag, &evidence)? {
        self.latch_fail_closed(
            "owned body-available retry differed from its unpublished exact owner",
        );
        return Err(EnqueueError::DuplicateCompletionOwnership);
    }
    let existing = self
        .ingress
        .reserved_body_available
        .as_ref()
        .expect("unpublished body owner remains serialized")
        .clone();
""",
            "owned BodyAvailable retry must select the sole exact unpublished physical owner before adapter preflight",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
let exact_retry = (|| -> Result<bool, EnqueueError> {
    let physical_admission_ordinal = existing
        .admission_ordinal
        .ok_or(EnqueueError::FailClosed)?;
    let lifecycle_ordinal = existing
        .lifecycle_ordinal
        .ok_or(EnqueueError::FailClosed)?;
    let retained_owner = existing
        .lifecycle_owner()
        .ok_or(EnqueueError::FailClosed)?;
    let restored = existing.restored_producer_stage.is_some();
    Ok(existing.tag == tag
        && existing.manifest == manifest
        && existing.owns_new_slot
        && existing.candidate_semantic_statement
            == ownership.candidate_semantic_statement()
        && existing
            .restored_producer_stage
            .is_none_or(RuntimeDormantLocalFifoReservation::is_known_stage)
        && (!restored || ownership.binds_exact_fetch_body_manifest(&manifest))
        && (&retained_owner == ownership.owner() || restored)
        && self
            .ingress
            .lifecycle_ordinals
            .recognizes_minted(physical_admission_ordinal)
            .map_err(|_| EnqueueError::FailClosed)?
        && self
            .ingress
            .lifecycle_ordinals
            .recognizes_minted(lifecycle_ordinal)
            .map_err(|_| EnqueueError::FailClosed)?
        && existing.dormant_replacement.as_ref().is_none_or(|replacement| {
            self.ingress
                .dormant_local_fifo_reservations
                .contains(replacement)
        }))
})();
""",
            "unpublished BodyAvailable reclaim must preserve tag, manifest, statement, physical/logical ordinals, restored authorization, and dormant backing",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
if let Some((retained_owner, retained_statement)) =
    self.body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
{
    return BodyAvailableReservation::coalesced_with_lifecycle_owner(
        tag,
        manifest,
        retained_owner,
        retained_statement,
    );
}
""",
            "an inexact unpublished retry must retain the exact incumbent lifecycle owner and effective authority",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
match exact_retry {
    Ok(true) => return Ok(existing),
    Ok(false) => self.latch_fail_closed(
        "owned body-available retry changed its unpublished exact owner",
    ),
    Err(error) => {
        self.latch_fail_closed(
            "owned body-available retry lost its unpublished exact owner",
        );
        return Err(error);
    }
}
return Err(EnqueueError::DuplicateCompletionOwnership);
}
if let Some((retained_owner, retained_statement)) =
    self.body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
{
    return BodyAvailableReservation::coalesced_with_lifecycle_owner(
        tag,
        manifest,
        retained_owner,
        retained_statement,
    );
}
let preflight =
    self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
if self.owned_preflight_is_coalesced(tag, &command, preflight, ownership)? {
    return BodyAvailableReservation::coalesced_with_owner(tag, manifest, ownership);
}
""",
            "an inexact unpublished retry must fail closed before ordinary queue or tombstone coalescence",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
let candidate_statement = ownership.candidate_semantic_statement();
""",
            "owned BodyAvailable reservation must receive the incumbent effect statement",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
let preflight =
    self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
if self.owned_preflight_is_coalesced(tag, &command, preflight, ownership)? {
    return BodyAvailableReservation::coalesced_with_owner(tag, manifest, ownership);
}
""",
            "owned BodyAvailable tombstone coalescence must pass the exact retained-owner gate",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
RuntimeCommandAdmissionPreflight::Coalesce
| RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
    self.latch_fail_closed(
        "unpublished body-available owner disagreed with adapter preflight",
    );
    return Err(EnqueueError::FailClosed);
}
""",
            "an owned retry may not coalesce through a different adapter or tombstone owner",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
let candidate_statement = ownership.candidate_semantic_statement();
""",
            "owned BodyAvailable reservation must receive the incumbent effect statement",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
if restored_owner.is_some() && !ownership.binds_exact_fetch_body_manifest(&manifest) {
    self.latch_fail_closed(
        "restored body-available retry changed its frozen candidate coordinates",
    );
    return Err(EnqueueError::FailClosed);
}
let owner = restored_owner
    .as_ref()
    .map_or_else(|| ownership.owner(), |(owner, _)| owner);
""",
            "restored BodyAvailable must authorize the stage-7 bridge by exact FetchBody coordinates and retain the persisted logical owner",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            reserve_owned_body,
            """
let result = self.ingress.reserve_canonical_body_available_internal(
    tag,
    manifest,
    Some(owner),
    candidate_statement,
    restored_owner
        .as_ref()
        .map(|(_, producer_stage)| *producer_stage),
);
""",
            "owned BodyAvailable reservation must publish the exact candidate statement and any restored stage under the selected logical owner",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            reserve_owned_body,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "reserve_body_available_with_owner"
            ],
            "owned BodyAvailable exact lifecycle reservation",
            errors,
        )

        rebind_unpublished_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "rebind_unpublished_body_available",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            rebind_unpublished_body,
            production_serialized_runtime_context,
            "unpublished BodyAvailable certified-view transfer",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            rebind_unpublished_body,
            """
reservation.tag == previous
    && reservation.manifest.round == round
    && reservation.manifest.subject == subject
""",
            "unpublished completion rebind must select the sole exact fetch coordinates",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            rebind_unpublished_body,
            """
if self.body_available_is_uniquely_owned(rebound, &manifest)? {
    self.latch_fail_closed(
        "unpublished body completion rebind found a second destination owner",
    );
""",
            "unpublished completion rebind must reject a second destination owner before transfer",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            rebind_unpublished_body,
            "self.rebind_body_available(previous, rebound, &manifest)",
            "unpublished completion rebind must delegate full-manifest ownership transfer",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            rebind_unpublished_body,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "rebind_unpublished_body_available"
            ],
            "unpublished BodyAvailable exact-coordinate rebind",
            errors,
        )

        retire_unpublished_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "retire_unpublished_body_available",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            retire_unpublished_body,
            production_serialized_runtime_context,
            "unpublished BodyAvailable fetch retirement",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            retire_unpublished_body,
            """
reservation.tag == tag
    && reservation.manifest.round == round
    && reservation.manifest.subject == subject
""",
            "unpublished completion retirement must select the sole exact fetch coordinates",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            retire_unpublished_body,
            "self.retire_body_available(tag, &manifest)",
            "unpublished completion retirement must delegate full-manifest exact-owner cleanup",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            retire_unpublished_body,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "retire_unpublished_body_available"
            ],
            "unpublished BodyAvailable exact-coordinate retirement",
            errors,
        )

        restored_retirement_identity = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RestoredProducerRetirement",
            "from_body_owner",
            errors,
            "restart-restored BodyAvailable retirement identity",
        )
        _require_rust_token_sequence(
            runtime_path,
            restored_retirement_identity,
            """
let admission_ordinal = lifecycle_ordinal.ok_or(EnqueueError::FailClosed)?;
let causal_lifecycle_key = causal_origin
    .restored_producer_lifecycle_key
    .ok_or(EnqueueError::FailClosed)?;
if producer_stage != RuntimeDormantLocalFifoReservation::BODY_AVAILABLE_STAGE
    || admission_ordinal == 0
    || !causal_origin.validate_exact()
    || causal_origin.root_lifecycle_ordinal != Some(admission_ordinal)
    || causal_origin.lifecycle_key != causal_lifecycle_key
{
    return Err(EnqueueError::FailClosed);
}
""",
            "restored BodyAvailable retirement must exact-match lifecycle key, nonzero ordinal, and stage 7",
            errors,
        )

        restored_retirement_inventory = _require_rust_item(
            runtime_path,
            runtime_source,
            "restored_body_available_retirement",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            restored_retirement_inventory,
            bounded_ingress_context,
            "restart-restored BodyAvailable runtime-owner inventory",
            errors,
        )
        for sequence, description in (
            (
                """
owners.push(RestoredProducerRetirement::from_body_owner(
    reservation
        .causal_origin
        .as_ref()
        .ok_or(EnqueueError::FailClosed)?,
    reservation.lifecycle_ordinal,
    reservation.restored_producer_stage,
)?);
""",
                "an unpublished restored BodyAvailable token must expose its exact persistent retirement tuple",
            ),
            (
                """
owners.push(RestoredProducerRetirement::from_body_owner(
    &queued.causal_origin,
    queued.lifecycle_ordinal,
    queued.restored_producer_stage,
)?);
""",
                "a queued restored BodyAvailable command must expose its exact persistent retirement tuple",
            ),
            (
                """
match owners.as_slice() {
    [] => Ok(None),
    [owner] => Ok(*owner),
    _ => Err(EnqueueError::DuplicateCompletionOwnership),
}
""",
                "restored producer retirement must select no more than one serialized BodyAvailable owner",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                restored_retirement_inventory,
                sequence,
                description,
                errors,
            )

        retire_restored_body_producer = _require_rust_item(
            runtime_path,
            runtime_source,
            "retire_restored_body_producer",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            retire_restored_body_producer,
            production_serialized_runtime_context,
            "persistent restart-restored BodyAvailable producer retirement",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            retire_restored_body_producer,
            """
match self.driver.retire_restored_producer_continuation(
    retirement.causal_lifecycle_key,
    retirement.admission_ordinal,
    retirement.producer_stage,
) {
    Ok(true) => Ok(()),
    Ok(false) => {
        self.latch_fail_closed(
            "restored body completion retirement lost its durable producer owner",
        );
""",
            "volatile BodyAvailable release must require successful persistent retirement of the exact restored producer",
            errors,
        )

        rebind_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "rebind_body_available",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            rebind_body,
            production_serialized_runtime_context,
            "destination-owned BodyAvailable rebind coalescence",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            rebind_body,
            """
let transferred = if destination_owned {
    let source_persistent =
        self.body_available_has_persistent_producer(previous, manifest)?;
    let destination_persistent =
        self.body_available_has_persistent_producer(rebound, manifest)?;
    if source_persistent && destination_persistent {
""",
            "destination-owned rebind must classify both persistent roots before coalescing either owner",
            errors,
        )

        retire_exact_body = _require_rust_item(
            runtime_path,
            runtime_source,
            "retire_body_available",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            retire_exact_body,
            production_serialized_runtime_context,
            "exact BodyAvailable terminal retirement",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            retire_exact_body,
            """
let restored = match self
    .ingress
    .restored_body_available_retirement(tag, |queued| queued == manifest)
{
    Ok(restored) => restored,
    Err(error) => {
        self.latch_fail_closed(
            "body completion retirement found corrupt restored producer metadata",
        );
        return Err(error.to_string());
    }
};
let deferred = match self.driver.retire_deferred_body_available(tag, manifest) {
    Ok(retired) => retired,
    Err(error) => {
        let error = error.to_string();
        self.latch_fail_closed(
            "body completion retirement could not persist deferred producer release",
        );
        return Err(format!(
            "Sumeragi v2 deferred body producer retirement failed: {error}"
        ));
    }
};
self.retire_restored_body_producer(restored)?;
let ingress = self.ingress.retire_canonical_body_available(tag, manifest);
""",
            "exact BodyAvailable retirement must persist whichever producer backs the sole owner before volatile removal",
            errors,
        )

        retire_body_pipeline_candidates = [
            item
            for item in rust_items(
                runtime_source, "retire_body_pipeline_completions"
            )
            if item.brace_context == production_serialized_runtime_context
        ]
        if len(retire_body_pipeline_candidates) != 1:
            errors.append(
                f"{runtime_path}: require exactly one production serialized "
                "runtime body-pipeline retirement item; found "
                f"{len(retire_body_pipeline_candidates)}"
            )
            retire_body_pipeline = None
        else:
            retire_body_pipeline = retire_body_pipeline_candidates[0]
            _require_rust_item_context(
                runtime_path,
                retire_body_pipeline,
                production_serialized_runtime_context,
                "exact body-pipeline terminal retirement",
                errors,
            )
        _require_rust_token_sequence(
            runtime_path,
            retire_body_pipeline,
            """
let expected = match expected.validate_unique() {
    Ok(expected) => expected,
    Err(error) => {
        self.latch_fail_closed(
            "body pipeline completion retirement found duplicate owners",
        );
        return Err(error);
    }
};
let restored = match self
    .ingress
    .restored_body_available_retirement(tag, |manifest| {
        manifest.round == round && manifest.subject == subject
    }) {
""",
            "body-pipeline retirement must reject duplicate serialized owners before selecting restored stage-7 metadata",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            retire_body_pipeline,
            """
let deferred = match self
    .driver
    .retire_deferred_body_pipeline_completions(tag, round, subject)
{
    Ok(retired) => retired,
    Err(error) => {
        let error = error.to_string();
        self.latch_fail_closed(
            "body pipeline retirement could not persist deferred producer release",
        );
        return Err(format!(
            "Sumeragi v2 deferred body producer retirement failed: {error}"
        ));
    }
};
self.retire_restored_body_producer(restored)?;
let ingress = self
    .ingress
    .retire_body_pipeline_completions(tag, round, subject);
""",
            "body-pipeline retirement must persist the sole deferred or restored producer before releasing volatile owners",
            errors,
        )

        unpublished_token_regression = _require_rust_item(
            runtime_path,
            runtime_source,
            "unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            unpublished_token_regression,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner"
            ],
            "unpublished BodyAvailable exact-coordinate lifecycle regression",
            errors,
        )
        for required, description in (
            (
                """
!runtime
    .rebind_unpublished_body_available(
        initial,
        rebound,
        manifest.round,
        foreign_subject,
    )
    .expect("foreign coordinates cannot select the reserved token")
""",
                "unpublished-token regression must reject foreign coordinates without selecting the token",
            ),
            (
                """
runtime
    .rebind_unpublished_body_available(
        initial,
        rebound,
        manifest.round,
        manifest.subject,
    )
    .expect("the unpublished token is a serialized body owner")
""",
                "unpublished-token regression must transfer the exact coordinate owner",
            ),
            (
                """
runtime
    .reserve_body_available_with_owner(
        rebound,
        manifest.clone(),
        &fetch_ownership
            .rebind_same_adapter_effect(&AdapterEffect::FetchBody {
                tag: rebound,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            })
            .expect("rebind the exact Fetch consumer"),
    )
    .expect("rebound exact owned retry reclaims the immutable root token")
""",
                "unpublished-token regression must reclaim through the same lifecycle owner rebound to the new Fetch consumer",
            ),
            (
                """
let retry = runtime
    .reserve_body_available_with_owner(
        rebound,
        manifest.clone(),
        &fetch_ownership
            .rebind_same_adapter_effect(&AdapterEffect::FetchBody {
                tag: rebound,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            })
            .expect("rebind the exact Fetch consumer"),
    )
    .expect("rebound exact owned retry reclaims the immutable root token");
assert_eq!(retry, rebound_reservation);
assert_eq!(
runtime
    .ingress
    .lifecycle_ordinals
    .next_ordinal_for_test()
    .expect("inspect source after rebound retry"),
source_before_rebind,
"retry cannot remint the rebound token",
""",
                "unpublished-token regression must preserve its physical admission source across rebind and retry",
            ),
            (
                """
runtime
    .retire_unpublished_body_available(
        rebound,
        manifest.round,
        manifest.subject,
    )
    .expect("terminal supersession retires the exact unpublished owner")
""",
                "unpublished-token regression must retire by the rebound exact coordinates",
            ),
            (
                """
assert!(runtime.ingress.reserved_body_available.is_none());
assert_eq!(runtime.queued_commands(), 0);
assert!(!runtime.fail_closed);
""",
                "unpublished-token regression must release capacity and stay open after terminal retirement",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                unpublished_token_regression,
                required,
                description,
                errors,
            )

        deferred_structs = rust_struct_items(
            runtime_source, "RuntimeDeferredLifecycleOwnership"
        )
        if len(deferred_structs) != 1:
            errors.append(
                f"{runtime_path}: require exactly one real Rust struct item named "
                "RuntimeDeferredLifecycleOwnership; found "
                f"{len(deferred_structs)}"
            )
        else:
            _require_rust_token_sequence(
                runtime_path,
                deferred_structs[0],
                "candidate_semantic_statement: Option<RuntimeCandidateSemanticStatement>",
                "adapter-deferred ownership must retain the candidate statement",
                errors,
            )
        deferred_validate = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeDeferredLifecycleOwnership",
            "validate_exact",
            errors,
            "adapter-deferred candidate statement validation",
        )
        _require_rust_token_sequence(
            runtime_path,
            deferred_validate,
            """
self.candidate_semantic_statement
    .is_none_or(RuntimeCandidateSemanticStatement::validate_exact)
""",
            "adapter-deferred ownership must validate every retained candidate statement",
            errors,
        )
        deferred_install = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeDeferredLifecycleOwnership",
            "with_candidate_semantic_statement",
            errors,
            "adapter-deferred candidate statement installation",
        )
        _require_exact_rust_tokens(
            runtime_path,
            deferred_install,
            """
fn with_candidate_semantic_statement(
    mut self,
    statement: Option<RuntimeCandidateSemanticStatement>,
) -> Result<Self, EnqueueError> {
    self.candidate_semantic_statement = statement;
    self.validate_exact()
        .then_some(self)
        .ok_or(EnqueueError::FailClosed)
}
""",
            "adapter-deferred ownership must validate a statement before installing it",
            errors,
        )
        deferred_rebase = _require_qualified_rust_item(
            runtime_path,
            runtime_source,
            "RuntimeDeferredLifecycleOwnership",
            "rebase_deferred_ingress",
            errors,
            "adapter-deferred ingress rebase statement retention",
        )
        _require_rust_token_sequence(
            runtime_path,
            deferred_rebase,
            "candidate_semantic_statement: self.candidate_semantic_statement,",
            "deferred ingress rebasing must not rewrite the retained candidate statement",
            errors,
        )
        accept_dispatch = _require_rust_item(
            runtime_path,
            runtime_source,
            "accept_driver_dispatch",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            accept_dispatch,
            serialized_runtime_context,
            "adapter-deferred candidate statement handoff",
            errors,
        )
        for sequence, description in (
            (
                "existing.candidate_semantic_statement != parent_statement",
                "an existing deferred occurrence must reject candidate statement replacement",
            ),
            (
                "ownership.with_candidate_semantic_statement(parent_statement)",
                "a new deferred occurrence must install the selected parent statement",
            ),
        ):
            _require_rust_token_sequence(
                runtime_path,
                accept_dispatch,
                sequence,
                description,
                errors,
            )
        dispatch_deferred = _require_rust_item(
            runtime_path,
            runtime_source,
            "dispatch_one_adapter_deferred",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            dispatch_deferred,
            serialized_runtime_context,
            "adapter-deferred candidate statement service handoff",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            dispatch_deferred,
            """
let parent_statement = lifecycle_ownership.candidate_semantic_statement;
""",
            "deferred service must recover the exact retained candidate statement",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            dispatch_deferred,
            """
self.retain_effect_ownership(
    RuntimeEffectSource::Deferred,
    Some(&lifecycle_owner),
    parent_statement.as_ref(),
    &effects,
)
""",
            "deferred service effects must inherit the exact retained candidate statement",
            errors,
        )

        for item_name, selected_kind, description in (
            (
                "dispatch_one_fence_dependency",
                "RuntimeEffectSource::Fifo",
                "fence-completion candidate statement handoff",
            ),
            (
                "step_recovery",
                "RuntimeEffectSource::Fifo",
                "recovery FIFO candidate statement handoff",
            ),
        ):
            dispatch = _require_rust_item(
                runtime_path,
                runtime_source,
                item_name,
                errors,
            )
            _require_rust_item_context(
                runtime_path,
                dispatch,
                serialized_runtime_context,
                description,
                errors,
            )
            _require_rust_token_sequence(
                runtime_path,
                dispatch,
                "let parent_statement = command.candidate_semantic_statement;",
                f"{description} must recover the statement from the selected command",
                errors,
            )
            _require_rust_token_sequence(
                runtime_path,
                dispatch,
                f"""
self.retain_effect_ownership(
    {selected_kind},
    Some(&owner),
    parent_statement.as_ref(),
    &effects,
)
""",
                f"{description} must pass the statement into successor effect binding",
                errors,
            )

        live_step = _require_rust_item(
            runtime_path,
            runtime_source,
            "step",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            live_step,
            serialized_runtime_context,
            "live FIFO candidate statement handoff",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            live_step,
            """
let parent_statement = command.candidate_semantic_statement;
let retry_command = command.clone();
""",
            "live FIFO dispatch must recover the statement from the selected command",
            errors,
        )
        _require_rust_token_sequence(
            runtime_path,
            live_step,
            """
(
    effects,
    RuntimeEffectSource::Fifo,
    owner,
    parent_statement,
    producer_handoff,
    retained_deferred_ingress,
)
""",
            "live dispatch must carry the selected statement to finalization",
            errors,
        )
        finish_dispatched = _require_rust_item(
            runtime_path,
            runtime_source,
            "finish_dispatched_step",
            errors,
        )
        _require_rust_item_context(
            runtime_path,
            finish_dispatched,
            serialized_runtime_context,
            "live FIFO candidate statement finalization",
            errors,
            expected_attributes=("#[allow(clippy::too_many_arguments)]",),
        )
        _require_rust_token_sequence(
            runtime_path,
            finish_dispatched,
            """
self.retain_effect_ownership(
    effect_source,
    Some(&effect_parent),
    effect_parent_statement.as_ref(),
    &effects,
)
""",
            "live finalization must pass the selected statement into successor effect binding",
            errors,
        )

    adapter_path = effects_path.with_name("v2.rs")
    if not adapter_path.is_file() or adapter_path.is_symlink():
        errors.append(
            f"{adapter_path}: durable producer tombstone source must be a regular file"
        )
    else:
        adapter_source = adapter_path.read_text(encoding="utf-8")
        deferred_exact_owners = _require_rust_item(
            adapter_path,
            adapter_source,
            "deferred_body_pipeline_completion_exact_owner_ordinals",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            deferred_exact_owners,
            (("impl", "SumeragiV2Adapter"),),
            "Busy-deferred exact completion owner inventory",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            deferred_exact_owners,
            """
input.completion_evidence.as_ref() == Some(candidate)
    && deferred_body_pipeline_completion_stage(input, tag, round, subject)
        == Some(expected_stage)
""",
            "Busy-deferred owner inventory must require the exact stage and full completion evidence",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            deferred_exact_owners,
            ".map(|input| input.admission_ordinal)",
            "Busy-deferred owner inventory must return the runtime ownership-map key",
            errors,
        )
        adapter_preflight = _require_rust_item(
            adapter_path,
            adapter_source,
            "preflight_runtime_command_admission",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            adapter_preflight,
            (("impl", "SumeragiV2Adapter"),),
            "durable producer-tombstone admission preflight",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_preflight,
            """
let serviced = self.serviced_candidates.contains_key(&key);
let matching = self
    .producer_continuations
    .iter()
    .filter(|(_, record)| record.identity().candidate() == key)
    .collect::<Vec<_>>();
""",
            "terminal preflight must join the service marker to its exact producer record",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_preflight,
            """
let identity = record.identity();
if serviced
    || record.status() != ProducerContinuationStatus::Reserved
    || !self
        .restored_dormant_producer_continuations
        .contains(address)
    || self.durable_producer_continuations.get(address) != Some(record)
{
    return Preflight::CoalesceOwned {
        causal_lifecycle_key: identity.causal_lifecycle_key(),
        admission_ordinal: identity.admission_ordinal(),
    };
}
""",
            "live and terminal producer coalescence must return the immutable retained owner",
            errors,
        )

        retire_restored_producer = _require_rust_item(
            adapter_path,
            adapter_source,
            "retire_restored_producer_continuation",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            retire_restored_producer,
            (("impl", "SumeragiV2Adapter"),),
            "persistent stage-7 producer-record retirement",
            errors,
        )
        for sequence, description in (
            (
                """
self.ensure_ingress()?;
if admission_ordinal == 0
    || producer_stage != ServicedCandidateStage::BodyAvailable as u8
    || self.selected_producer_lifecycle.is_some()
{
    return Err(self.fail_serviced_candidate_store(
        "restored producer retirement carried an invalid stage, ordinal, or active selection"
            .to_owned(),
    ));
}
""",
                "persistent producer retirement must accept only an inactive nonzero stage-7 owner",
            ),
            (
                """
let matches = self
    .producer_continuations
    .iter()
    .filter_map(|(address, record)| {
        let identity = record.identity();
        (identity.causal_lifecycle_key() == causal_lifecycle_key
    && identity.admission_ordinal() == admission_ordinal
    && identity.stage() == producer_stage)
    .then_some((*address, record.clone()))
    })
    .collect::<Vec<_>>();
""",
                "persistent producer retirement must join the exact lifecycle key, ordinal, and stage",
            ),
            (
                """
let [(address, record)] = matches.as_slice() else {
    return match matches.len() {
        0 => Ok(false),
        _ => Err(self.fail_serviced_candidate_store(
            "restored producer retirement matched multiple bounded addresses".to_owned(),
        )),
    };
};
self.persist_restored_body_producer_retirement(*address, record)?;
Ok(true)
""",
                "persistent producer retirement must select one exact record before delegating durable removal",
            ),
        ):
            _require_rust_token_sequence(
                adapter_path,
                retire_restored_producer,
                sequence,
                description,
                errors,
            )

        persist_restored_body_producer = _require_rust_item(
            adapter_path,
            adapter_source,
            "persist_restored_body_producer_retirement",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            persist_restored_body_producer,
            (("impl", "SumeragiV2Adapter"),),
            "shared persist-first stage-7 producer-record retirement",
            errors,
        )
        for sequence, description in (
            (
                """
if record.status() != ProducerContinuationStatus::Reserved
    || record.source_class() != ProducerContinuationSourceClass::VolatileBody
    || record.identity().address() != address
    || record.identity().stage() != ServicedCandidateStage::BodyAvailable as u8
    || self.durable_producer_continuations.get(&address) != Some(record)
    || !self
        .restored_dormant_producer_continuations
        .contains(&address)
    || self
        .deferred_producer_continuations
        .values()
        .any(|reservation| reservation.address == address)
    || self.pending_producer_handoffs.contains_key(&address)
{
    return Err(self.fail_serviced_candidate_store(
        "restored producer retirement did not own one exact dormant durable record"
            .to_owned(),
    ));
}
""",
                "persistent producer retirement must own one exact dormant durable stage-7 volatile-body record with no live alias",
            ),
            (
                """
let process_previous = self
    .producer_continuations
    .remove(&address)
    .expect("matched process producer remains present");
let durable_previous = self
    .durable_producer_continuations
    .remove(&address)
    .expect("matched durable producer remains present");
let dormant_removed = self
    .restored_dormant_producer_continuations
    .remove(&address);
debug_assert!(dormant_removed);
if let Err(reason) = self
    .serviced_candidate_store
    .persist_with_producer_continuations(
        &self.durable_serviced_candidates,
        &self.durable_producer_continuations,
        self.serviced_candidates_decision_reclaimed,
    )
{
    self.producer_continuations
        .insert(address, process_previous);
    self.durable_producer_continuations
        .insert(address, durable_previous);
    if dormant_removed {
        self.restored_dormant_producer_continuations.insert(address);
    }
    return Err(self.fail_serviced_candidate_store(reason));
}
Ok(())
""",
                "stage-7 retirement must persist process/durable/dormant removal and roll all memory back on persistence failure",
            ),
        ):
            _require_rust_token_sequence(
                adapter_path,
                persist_restored_body_producer,
                sequence,
                description,
                errors,
            )

        persistent_retirement_helper = _require_rust_item(
            adapter_path,
            adapter_source,
            "assert_restored_stage_seven_retirement_does_not_resurrect",
            errors,
        )
        for sequence, description in (
            (
                """
manifest: (marker != 0xBD).then_some(manifest.clone()),
""",
                "the restart regression must make BD the unique manifest-less restored Fetch case",
            ),
            (
                """
if !reserve_completion {
    assert!(
        runtime
            .retire_restored_body_fetch_parent(&reconstructed_fetch, &fetch_ownership)
            .expect("persist terminal restored fetch-parent retirement")
    );
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
    assert!(
        !runtime
            .driver()
            .producer_continuations
            .contains_key(&restored_address)
            && !runtime
                .driver()
                .durable_producer_continuations
                .contains_key(&restored_address)
            && !runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address),
        "terminal fetch cancellation must remove its dormant stage-7 parent"
    );
    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after terminal restored fetch cancellation");
    assert!(restarted_again.producer_continuations.is_empty());
    return;
}
""",
                "pre-reservation restored Fetch cancellation must persist its dormant stage-7 parent before returning",
            ),
            (
                """
let retired = if materialize_before_retirement {
    runtime
        .commit_body_available(reservation)
        .expect("materialize restored completion before pipeline retirement");
    runtime
        .retire_body_pipeline_completions(restarted_tag, round, body_subject)
        .map(|retired| retired.body_available())
} else {
    runtime.retire_unpublished_body_available(restarted_tag, round, body_subject)
};
""",
                "the restart regression must exercise unpublished and queued stage-7 retirement",
            ),
            (
                """
if let Some((path, bytes)) = sabotaged_snapshot {
    assert!(
        retired.is_err(),
        "a failed durable release cannot publish volatile token retirement"
    );
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before - 1,
        "failed persistence retains the exact unpublished physical owner"
    );
    assert!(runtime.driver().fail_closed);
    assert_eq!(
        runtime
            .driver()
            .producer_continuations
            .get(&restored_address),
        runtime
            .driver()
            .durable_producer_continuations
            .get(&restored_address),
        "failed persistence restores both in-memory producer aliases"
    );
    assert!(
        runtime
            .driver()
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );
""",
                "the injected persistence failure must retain the volatile token and restore every in-memory producer alias",
            ),
            (
                """
assert!(retired.expect("persist and retire the restored body completion"));
assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
assert!(
!runtime
    .driver()
    .producer_continuations
    .contains_key(&restored_address)
    && !runtime
        .driver()
        .durable_producer_continuations
        .contains_key(&restored_address)
    && !runtime
        .driver()
        .restored_dormant_producer_continuations
        .contains(&restored_address),
    "terminal runtime retirement must persistently release the restored producer"
);
""",
                "reserved stage-7 retirement must observe process, durable, dormant, and capacity release before reopening",
            ),
            (
                """
drop(runtime.into_driver());
let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
    directory.path().join("safety.wal"),
    verified_genesis(context()),
    Some(0),
    reducer::Generation::new(3),
    [0x11; 32],
    fingerprints(),
    Box::new(TestAggregator),
    deferred_admission_ordinals(),
)
.expect("reopen after terminal stage-7 retirement");
assert!(
    restarted_again.producer_continuations.is_empty()
        && restarted_again.durable_producer_continuations.is_empty()
        && restarted_again
            .restored_dormant_producer_continuations
            .is_empty(),
""",
                "the stage-7 retirement regression must perform a second restart from persisted state",
            ),
            (
                """
restarted_again.producer_continuations.is_empty()
    && restarted_again.durable_producer_continuations.is_empty()
    && restarted_again
        .restored_dormant_producer_continuations
        .is_empty()
""",
                "the second restart must prove that terminally retired stage-7 ownership cannot resurrect",
            ),
        ):
            _require_rust_token_sequence(
                adapter_path,
                persistent_retirement_helper,
                sequence,
                description,
                errors,
            )
        _require_rust_token_sequence(
            adapter_path,
            persistent_retirement_helper,
            """
!runtime
    .driver()
    .producer_continuations
    .contains_key(&restored_address)
    && !runtime
        .driver()
        .durable_producer_continuations
        .contains_key(&restored_address)
    && !runtime
        .driver()
        .restored_dormant_producer_continuations
        .contains(&restored_address)
""",
            "the restart regression must observe both terminal-fetch and reserved-token process/durable/dormant removal cuts",
            errors,
            count=2,
        )

        persistent_retirement_regression = _require_rust_item(
            adapter_path,
            adapter_source,
            "restored_body_available_terminal_retirement_is_persistent_before_token_release",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            persistent_retirement_regression,
            """
assert_restored_stage_seven_retirement_does_not_resurrect(0xB8, true, false, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xB9, true, true, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBA, true, false, true);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBB, false, false, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBD, false, false, false);
""",
            "the public regression must cover unpublished, materialized, failed, manifest-bound pre-reservation, and manifest-less pre-reservation stage-7 retirement",
            errors,
        )

    drain = _require_rust_item(
        effects_path,
        source,
        "drain_retained_effect_batch",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        drain,
        generic_executor_context,
        "certified-request retained-effect retry method",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        drain,
        """
let pending_work_producer = Self::pending_work_producer(&owned.effect);
match self.consume_one(owned.effect, owned.ownership, services) {
""",
        "certified-request retry must classify and dispatch the exact retained owned effect",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        drain,
        """
Err(
    EffectExecutorError::PendingWorkCapacity { .. }
    | EffectExecutorError::CertifiedRequestCapacity { .. },
) => {
    debug_assert!(pending_work_producer.is_some());
    break;
}
Err(error) => return Err(error),
""",
        "both retained-effect capacity errors must preserve the exact FIFO head before fail-closed fallback",
        errors,
    )
    if drain is not None:
        certified_capacity_count = _token_sequence_count(
            rust_code_tokens(drain.source),
            rust_code_tokens("EffectExecutorError::CertifiedRequestCapacity"),
        )
        if certified_capacity_count != 1:
            errors.append(
                f"{effects_path}:{drain.line}: retained-effect dispatch must "
                "retry CertifiedRequestCapacity exactly once beside "
                f"PendingWorkCapacity; found {certified_capacity_count} arm(s)"
            )

    begin_fetch = _require_rust_item(
        effects_path,
        source,
        "begin_fetch",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        begin_fetch,
        generic_executor_context,
        "source-faithful certified-request capacity deferral method",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments)]",),
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
} else if let Some(certificate) = certificate {
    let plan = match self.plan_certified_fetch_request(
        existing_id,
        round,
        subject,
        certificate,
        services,
    ) {
""",
        "existing ordinary Fetch Q-capacity upgrade planning",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
let request_plan = if let Some(certificate) = certificate {
    match self.plan_certified_fetch_request(
        work.id,
        round,
        subject,
        certificate,
        services
    ) {
""",
        "genuinely new Fetch Q-capacity request planning",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
Err(EffectExecutorError::CertifiedRequestCapacity { capacity }) => {
    iroha_logger::debug!(
        height = round.height,
        view = round.view,
        capacity,
        "deferred certified Sumeragi v2 body-fetch authority upgrade at request capacity"
    );
    return Err(EffectExecutorError::CertifiedRequestCapacity { capacity });
}
Err(error) => return Err(error),
""",
        "an existing Fetch Q-capacity upgrade must retain and retry its exact lifecycle without partial authority installation",
        errors,
        count=2,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
Err(EffectExecutorError::CertifiedRequestCapacity { capacity }) => {
    iroha_logger::debug!(
        height = round.height,
        view = round.view,
        capacity,
        "deferred certified Sumeragi v2 body fetch at request capacity"
    );
    return Err(EffectExecutorError::CertifiedRequestCapacity { capacity });
}
Err(error) => return Err(error),
""",
        "a new Fetch Q-capacity admission must retain and retry its exact lifecycle without partial authority installation",
        errors,
        count=2,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
let same_lifecycle = existing.task.ownership == ownership;
if existing.task.tag != tag {
    return Err(EffectExecutorError::Contract(
        "conflicting retransmission for one body-fetch round/subject".to_owned(),
    ));
}
if !same_lifecycle {
    return Err(EffectExecutorError::Contract(
        "body-fetch retry or authority upgrade changed its exact lifecycle owner"
            .to_owned(),
    ));
}
""",
        "Fetch owner replacement must fail before request, refinement, or service planning",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
let merged_ownership = existing
    .task
    .ownership
    .rebind_same_adapter_effect(&merged_effect)
    .map_err(EffectExecutorError::Contract)?;
let merged = BodyFetchTask {
    id: existing_id,
    tag,
    round,
    subject,
    manifest: merged_manifest,
    sources: merged_sources,
    certified_request: merged_request,
    ownership: merged_ownership,
};
""",
        "coalesced Fetch retries must rebind the concrete effect while retaining the incumbent owner",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
if merged == existing.task {
    services.enqueue_body_fetch(merged).map_err(service_error)?;
    return Ok(());
}
services
    .enqueue_body_fetch(merged.clone())
    .map_err(service_error)?;
""",
        "same-owner Fetch retries and upgrades must reach the idempotent service seam after the early owner gate",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
services
    .enqueue_body_fetch(merged.clone())
    .map_err(service_error)?;
if let Some(plan) = request_plan {
    self.commit_certified_fetch_request(plan);
}
self.commit_body_pipeline_owner(owner_plan);
let pending = self
    .pending_fetches
    .get_mut(&existing_id)
    .expect("serialized body-fetch owner remains present after admission");
pending.task = merged;
pending.request_hash = request_hash;
return Ok(());
""",
        "a successful same-owner Fetch authority upgrade must atomically install P/Q state and drain its retry",
        errors,
    )
    if begin_fetch is not None:
        _require_rust_item_token_sha256(
            effects_path,
            begin_fetch,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256["begin_fetch"],
            "idempotent exact Fetch admission lifecycle",
            errors,
        )
        begin_fetch_tokens = rust_code_tokens(begin_fetch.source)
        owner_barrier_tokens = rust_code_tokens("if !same_lifecycle")
        request_plan_tokens = rust_code_tokens("self.plan_certified_fetch_request(")
        refinement_tokens = rust_code_tokens(
            "existing.task.ownership.rebind_same_adapter_effect(&merged_effect)"
        )
        barrier_tokens = rust_code_tokens("if merged == existing.task")
        retry_enqueue_tokens = rust_code_tokens(
            "services.enqueue_body_fetch(merged).map_err(service_error)?"
        )
        upgrade_enqueue_tokens = rust_code_tokens(
            "services.enqueue_body_fetch(merged.clone()).map_err(service_error)?"
        )
        barrier_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(barrier_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(barrier_tokens)]
            == barrier_tokens
        ]
        retry_enqueue_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(retry_enqueue_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(retry_enqueue_tokens)]
            == retry_enqueue_tokens
        ]
        upgrade_enqueue_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(upgrade_enqueue_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(upgrade_enqueue_tokens)]
            == upgrade_enqueue_tokens
        ]
        owner_barrier_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(owner_barrier_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(owner_barrier_tokens)]
            == owner_barrier_tokens
        ]
        request_plan_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(request_plan_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(request_plan_tokens)]
            == request_plan_tokens
        ]
        refinement_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(refinement_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(refinement_tokens)]
            == refinement_tokens
        ]
        if not (
            len(owner_barrier_positions) == 1
            and len(request_plan_positions) == 2
            and len(refinement_positions) == 1
            and owner_barrier_positions[0] < min(request_plan_positions)
            and owner_barrier_positions[0] < refinement_positions[0]
        ):
            errors.append(
                f"{effects_path}:{begin_fetch.line}: begin_fetch must reject "
                "one foreign incumbent owner before either request planner "
                "and before candidate refinement evidence"
            )
        if not (
            len(barrier_positions) == 1
            and len(retry_enqueue_positions) == 1
            and len(upgrade_enqueue_positions) == 1
            and barrier_positions[0] < retry_enqueue_positions[0]
            and retry_enqueue_positions[0] < upgrade_enqueue_positions[0]
        ):
            errors.append(
                f"{effects_path}:{begin_fetch.line}: begin_fetch must keep one "
                "merged == existing.task barrier, one same-owner retry "
                "enqueue inside it, and one later same-owner authority-upgrade enqueue"
            )
        for forbidden_source in (
            "self.retained_effect_batch",
            "self.retain_effect_batch",
        ):
            retained_count = _token_sequence_count(
                begin_fetch_tokens,
                rust_code_tokens(forbidden_source),
            )
            if retained_count != 0:
                errors.append(
                    f"{effects_path}:{begin_fetch.line}: Q-capacity deferrals "
                    "must not mutate the outer executor's exact retained FIFO "
                    "owner inside begin_fetch; "
                    f"found {retained_count} occurrence(s) of {forbidden_source}"
                )

    production_effect_runtime_context = (
        ("impl", "EffectRuntime", "for", "SerializedV2Runtime"),
    )
    for item_name, seal_name, delegate, description in (
        (
            "rebind_unpublished_body_available",
            "effect_runtime_rebind_unpublished_body_available",
            """
SerializedV2Runtime::rebind_unpublished_body_available(
    self, previous, rebound, round, subject,
)
.map_err(|error| error.to_string())
""",
            "production EffectRuntime unpublished BodyAvailable rebind forwarding",
        ),
        (
            "retire_unpublished_body_available",
            "effect_runtime_retire_unpublished_body_available",
            """
SerializedV2Runtime::retire_unpublished_body_available(
    self, tag, round, subject
)
.map_err(|error| error.to_string())
""",
            "production EffectRuntime unpublished BodyAvailable retirement forwarding",
        ),
    ):
        matching = tuple(
            item
            for item in rust_items(source, item_name)
            if item.brace_context == production_effect_runtime_context
        )
        if len(matching) != 1:
            errors.append(
                f"{effects_path}: require exactly one production EffectRuntime "
                f"item named {item_name}; found {len(matching)}"
            )
            continue
        production_delegate = matching[0]
        _require_rust_item_context(
            effects_path,
            production_delegate,
            production_effect_runtime_context,
            description,
            errors,
        )
        _require_rust_token_sequence(
            effects_path,
            production_delegate,
            delegate,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            effects_path,
            production_delegate,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[seal_name],
            description,
            errors,
        )

    for item_name, expected_source, description in (
        (
            "plan_body_pipeline_candidate_terminal",
            """
fn plan_body_pipeline_candidate_terminal(
    &mut self,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> Result<Option<RuntimeEffectOwnership>, String> {
    SerializedV2Runtime::plan_body_pipeline_candidate_terminal(self, effect, ownership)
}
""",
            "production EffectRuntime body-terminal incumbent-owner plan forwarding",
        ),
        (
            "commit_body_pipeline_candidate_terminals",
            """
fn commit_body_pipeline_candidate_terminals(
    &mut self,
    terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
) -> Result<(), String> {
    SerializedV2Runtime::commit_body_pipeline_candidate_terminals(self, terminals)
}
""",
            "production EffectRuntime atomic body-terminal authority commit forwarding",
        ),
    ):
        matching = tuple(
            item
            for item in rust_items(source, item_name)
            if item.brace_context == production_effect_runtime_context
        )
        if len(matching) != 1:
            errors.append(
                f"{effects_path}: require exactly one production EffectRuntime "
                f"item named {item_name}; found {len(matching)}"
            )
            continue
        production_delegate = matching[0]
        _require_rust_item_context(
            effects_path,
            production_delegate,
            production_effect_runtime_context,
            description,
            errors,
        )
        _require_exact_rust_tokens(
            effects_path,
            production_delegate,
            expected_source,
            description,
            errors,
        )

    commit_fetch_retirement = _require_rust_item(
        effects_path,
        source,
        "commit_pending_fetch_retirement",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        commit_fetch_retirement,
        generic_executor_context,
        "exact pending-Fetch terminal retirement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_retirement,
        """
let retired_completion = self
    .runtime
    .retire_unpublished_body_available(
        plan.pending.task.tag,
        plan.pending.task.round,
        plan.pending.task.subject,
    )
    .map_err(EffectExecutorError::Runtime)?;
if !retired_completion {
    let effect = plan.pending.task.adapter_effect();
    self.runtime
        .retire_restored_body_fetch_parent(&effect, plan.pending.task.ownership())
        .map_err(EffectExecutorError::Runtime)?;
}
let work_id = plan.pending.task.id();
let removed = self.pending_fetches.remove(&work_id);
""",
        "pending Fetch retirement must release its token or restored stage-7 parent before local ownership",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_retirement,
        """
if let Some(certified) = plan.certified {
    self.commit_certified_fetch_retirement(certified);
}
Ok(())
""",
        "pending Fetch retirement must release its certified request only after runtime and local ownership",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        commit_fetch_retirement,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "commit_pending_fetch_retirement"
        ],
        "exact pending-Fetch terminal retirement",
        errors,
    )

    reject_noncanonical = _require_rust_item(
        effects_path,
        source,
        "reject_noncanonical_reconstruction",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        reject_noncanonical,
        generic_executor_context,
        "noncanonical Fetch terminal retirement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        reject_noncanonical,
        """
let retirement = self
    .plan_pending_fetch_retirement(&pending)
    .map_err(|error| self.fail_closed_transport(error, services))?;
""",
        "noncanonical reconstruction must preflight the shared exact Fetch retirement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        reject_noncanonical,
        """
self.commit_pending_fetch_retirement(retirement)
    .map_err(|error| self.fail_closed_transport(error, services))?;
self.body_pipeline_owners.remove(&key);
""",
        "noncanonical reconstruction must retire the unpublished token before its pipeline owner",
        errors,
    )

    install_view = _require_rust_item(
        effects_path,
        source,
        "install_view",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        install_view,
        generic_executor_context,
        "certified-view protected Fetch transfer",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        install_view,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256["install_view"],
        "certified-view protected Fetch transfer",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
self.runtime
    .rebind_unpublished_body_available(
        pending.task.tag,
        tag,
        pending.task.round,
        pending.task.subject,
    )
    .map_err(EffectExecutorError::Runtime)?;
let work_id = pending.task.id();
""",
        "protected Fetch transfer must retag its unpublished completion before local mutation",
        errors,
    )
    if install_view is not None:
        install_tokens = rust_code_tokens(install_view.source)
        ordered_fragments = tuple(
            rust_code_tokens(fragment)
            for fragment in (
                "services.rebind_body_fetch(&pending.task, rebound.clone())",
                "self.runtime.rebind_unpublished_body_available(",
                "current.task = rebound",
            )
        )
        ordered_positions: list[int] = []
        order_is_exact = True
        for fragment in ordered_fragments:
            positions = [
                index
                for index in range(len(install_tokens) - len(fragment) + 1)
                if install_tokens[index : index + len(fragment)] == fragment
            ]
            if len(positions) != 1:
                order_is_exact = False
            else:
                ordered_positions.append(positions[0])
        if not (
            order_is_exact
            and ordered_positions == sorted(ordered_positions)
        ):
            errors.append(
                f"{effects_path}:{install_view.line}: EnterView must transfer "
                "the external Fetch service, then its unpublished runtime "
                "token, then the local pending-Fetch consumer"
            )

    commit_fetch_completion = _require_rust_item(
        effects_path,
        source,
        "commit_fetch_completion",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        commit_fetch_completion,
        generic_executor_context,
        "fallible exact Fetch completion publication",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_completion,
        """
self.runtime
    .commit_body_available(plan.runtime_reservation)?;
self.commit_body_pipeline_owner(plan.owner);
match plan.ready {
""",
        "runtime BodyAvailable commit must precede every local Fetch-owner mutation",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_completion,
        """
self.pending_fetches.remove(&plan.work_id);
if let Some(retirement) = plan.certified_retirement {
    self.commit_certified_fetch_retirement(retirement);
}
Ok(())
""",
        "local Fetch and certified-request retirement must follow runtime publication",
        errors,
    )
    if commit_fetch_completion is not None:
        _require_rust_item_token_sha256(
            effects_path,
            commit_fetch_completion,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "commit_fetch_completion"
            ],
            "fallible exact Fetch completion publication",
            errors,
        )
        completion_tokens = rust_code_tokens(commit_fetch_completion.source)
        completion_fragments = tuple(
            rust_code_tokens(fragment)
            for fragment in (
                "self.runtime.commit_body_available(plan.runtime_reservation)?",
                "self.commit_body_pipeline_owner(plan.owner)",
                "match plan.ready",
                "self.pending_fetches.remove(&plan.work_id)",
                "self.commit_certified_fetch_retirement(retirement)",
            )
        )
        completion_positions: list[int] = []
        completion_cardinality_ok = True
        for fragment in completion_fragments:
            positions = [
                index
                for index in range(
                    len(completion_tokens) - len(fragment) + 1
                )
                if completion_tokens[index : index + len(fragment)] == fragment
            ]
            if len(positions) != 1:
                completion_cardinality_ok = False
            else:
                completion_positions.append(positions[0])
        if not (
            completion_cardinality_ok
            and completion_positions == sorted(completion_positions)
        ):
            errors.append(
                f"{effects_path}:{commit_fetch_completion.line}: exact Fetch "
                "completion must publish BodyAvailable before local owner, "
                "ready-body, Fetch, and certified-request retirement"
            )

    saturation_regression = _require_rust_item(
        effects_path,
        source,
        "production_capacity_saturation_admits_response_and_reconstructible_fetch",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        saturation_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "production_capacity_saturation_admits_response_and_reconstructible_fetch"
        ],
        "production capacity saturation with exact retransmit lifecycle ownership",
        errors,
    )
    for required, description in (
        (
            """
fixture
    .executor
    .runtime
    .retain_retransmit_effect_ownership_for_test(&effects)
    .expect("bind production retransmit lifecycle ownership");
assert_eq!(
    fixture
        .executor
        .consume_effects(effects, &mut services)
        .expect("fill production certified-request ownership"),
    1
);
""",
            "every synthetic saturated Fetch must carry production retransmit ownership",
        ),
        (
            """
fixture
    .executor
    .runtime
    .retain_retransmit_effect_ownership_for_test(&fetch_b_effects)
    .expect("bind deferred production retransmit lifecycle ownership");
assert_eq!(
    fixture
        .executor
        .consume_effects(fetch_b_effects, &mut services)
        .expect("defer Fetch B at production certified-request capacity"),
    0
);
""",
            "the deferred saturated Fetch must carry the same production ownership",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            saturation_regression,
            required,
            description,
            errors,
        )

    rebind_token_regression = _require_rust_item(
        effects_path,
        source,
        "ready_body_backpressure_retains_exact_ingress_until_capacity_retry",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        rebind_token_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "ready_body_backpressure_retains_exact_ingress_until_capacity_retry"
        ],
        "protected-view unpublished BodyAvailable rebind regression",
        errors,
    )
    for required, description in (
        (
            """
assert_eq!(retained_runtime_token.tag(), tag(0));
assert_eq!(retained_runtime_token.manifest(), &fixture.manifest);
""",
            "protected-view regression must begin with the typed-Retryable exact token in the old incarnation",
        ),
        (
            """
protected_body: Some((fixture.manifest.round, fixture.manifest.subject)),
""",
            "protected-view regression must install a TC which protects the token's exact body coordinates",
        ),
        (
            """
assert_eq!(rebound_runtime_token.tag(), tag(1));
assert_eq!(rebound_runtime_token.manifest(), &fixture.manifest);
assert_eq!(
    rebound_runtime_token.owns_new_slot(),
    retained_runtime_token.owns_new_slot(),
    "view rebinding cannot replace the physical reservation",
);
""",
            "protected-view regression must move the token without replacing its physical reservation",
        ),
        (
            """
assert!(executor.runtime.completions.is_empty());
assert!(executor.has_retained_certified_body_response());
assert_eq!(
executor
    .retained_certified_body_response_scheduler_ordinal()
    .expect("read rebound response ordinal"),
Some(retained_scheduler_ordinal),
"EnterView cannot remint the retained leader-wire ticket",
""",
            "protected-view regression must preserve the retained leader-wire scheduler ordinal",
        ),
        (
            """
[RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
    if *completion_tag == tag(1) && manifest == &fixture.manifest
""",
            "protected-view regression must materialize exactly one completion in the rebound incarnation",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            rebind_token_regression,
            required,
            description,
            errors,
        )

    tc_token_regression = _require_rust_item(
        effects_path,
        source,
        "tc_retires_unprotected_retryable_body_token_before_the_next_fetch",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        tc_token_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "tc_retires_unprotected_retryable_body_token_before_the_next_fetch"
        ],
        "unprotected TC unpublished BodyAvailable retirement regression",
        errors,
    )
    for required, description in (
        (
            "assert!(executor.pending_fetches.is_empty());",
            "unprotected TC regression must retire the stale Fetch",
        ),
        (
            """
executor
    .body_ownership_projection()
    .runtime_body_reservation
    .is_none()
""",
            "unprotected TC regression must release the unpublished Completion token",
        ),
        (
            """
[RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
    if *completion_tag == tag(1) && manifest == &manifest_b
""",
            "unprotected TC regression must publish one distinct successor completion",
        ),
        (
            """
Err(EffectTransportError::Authentication(
    V2TransportError::UnsolicitedResponse(_)
))
""",
            "unprotected TC regression must reject the retired response carrier instead of resurrecting its token",
        ),
        (
            "assert!(!executor.status().fail_closed);",
            "unprotected TC regression must preserve an open runtime after exact retirement and successor publication",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            tc_token_regression,
            required,
            description,
            errors,
        )

    sign_token_regression = _require_rust_item(
        effects_path,
        source,
        "durable_sign_preemption_retires_a_retryable_body_token",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        sign_token_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "durable_sign_preemption_retires_a_retryable_body_token"
        ],
        "durable Sign unpublished BodyAvailable retirement regression",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        sign_token_regression,
        """
executor
    .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
    .expect("durable signing preempts the retryable fetch and its token");
""",
        "durable-Sign regression must exercise pending-Fetch preemption",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        sign_token_regression,
        """
assert!(executor.pending_fetches.is_empty());
assert!(executor.certified_work.is_empty());
assert!(executor.outstanding_requests.is_empty());
assert!(executor.runtime.reserved_body_available.is_none());
assert!(executor.runtime.completions.is_empty());
assert_eq!(executor.pending_signatures.len(), 1);
assert!(!executor.status().fail_closed);
""",
        "durable-Sign regression must release the complete stale Fetch/token/request owner set and admit one signature without fail-close",
        errors,
    )

    classification = _require_rust_item(
        effects_path,
        source,
        "network_ingress_requires_reducer_order",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        classification,
        generic_executor_context,
        "exhaustive retained-ingress reducer-order classifier",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        classification,
        """
fn network_ingress_requires_reducer_order(
    payload: &wire::ConsensusMessageV2Payload
) -> bool {
    match payload {
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => true,
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => false,
    }
}
""",
        "exhaustive transport/reducer ingress classification",
        errors,
    )

    certified_escape = _require_rust_item(
        effects_path,
        source,
        "network_ingress_is_certified_fence_escape",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        certified_escape,
        (),
        "closed authenticated fence-escape classifier",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        certified_escape,
        """
pub(crate) const fn network_ingress_is_certified_fence_escape(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    match payload {
        wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => true,
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            matches!(certificate.phase, wire::GlobalPhase::Commit)
        }
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
            matches!(response.certificate.phase, wire::GlobalPhase::Commit)
        }
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        | wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => false,
    }
}
""",
        "only TC, direct CommitQC, and discovery CommitQC may escape a hung signer",
        errors,
    )

    retained_dispatch = _require_rust_item(
        effects_path,
        source,
        "retained_dispatch_allows_network_ingress",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retained_dispatch,
        generic_executor_context,
        "retained dispatch transport-completion bypass",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        retained_dispatch,
        """
fn retained_dispatch_allows_network_ingress(
    &self,
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    network_ingress_is_certified_fence_escape(payload)
        || (self.retained_certified_body_response.is_none()
            && (self.retained_effect_batch.is_none() && self.parked_effect_batch.is_none()
                || !Self::network_ingress_requires_reducer_order(payload)))
}
""",
        "retained dispatch transport completion and certified fence-escape policy",
        errors,
    )

    for item_name, required, description in (
        (
            "enqueue_network_with_ingress_ownership",
            """
if self.fatal_reason.is_some() || self.output_guard.restart_required() {
    return Err(NetworkIngressError::FailClosed);
}
let result = self
    .runtime
    .enqueue_network_with_ingress_ownership(message, ingress_ownership);
if matches!(&result, Err(NetworkIngressError::FailClosed)) {
    self.output_guard.activate_restart_required();
    self.fatal_reason.get_or_insert_with(|| {
        "Sumeragi v2 runtime rejected authenticated ingress ownership".to_owned()
    });
}
result
""",
            "public ownership-aware network admission and fail-stop latch",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            """
if self.fatal_reason.is_some() || self.output_guard.restart_required() {
    return false;
}
let retained_dispatch_allows =
    self.retained_dispatch_allows_network_ingress(&message.payload);
let timeout_vote_recovery_episode = !retained_dispatch_allows
    && self
        .runtime
        .can_admit_timeout_vote_recovery_episode(message, ingress_ownership);
(retained_dispatch_allows || timeout_vote_recovery_episode)
    && self
        .runtime
        .can_admit_network_message_with_ingress_ownership(message, ingress_ownership)
""",
            "public ownership-aware retained-debt capacity preflight",
        ),
    ):
        item = _require_rust_item(effects_path, source, item_name, errors)
        _require_rust_item_context(
            effects_path,
            item,
            production_executor_context,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            effects_path,
            item,
            _EFFECT_CAPACITY_PRODUCTION_RUST_ITEM_SHA256[item_name],
            description,
            errors,
        )
        _require_rust_token_sequence(
            effects_path,
            item,
            required,
            description,
            errors,
        )
    return errors
