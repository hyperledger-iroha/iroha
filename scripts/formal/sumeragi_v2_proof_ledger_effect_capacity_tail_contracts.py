# Executed lexically in check_sumeragi_v2_proof_ledger.py.

def _effect_capacity_terminal_retirement_source_fidelity_errors(
    effects_path: Path,
    source: str,
    errors: list[str],
    generic_executor_context: tuple,
    production_executor_context: tuple,
) -> None:
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
if retires_proposal_replay {
    self.remote_proposal_replay.remove(&key);
}
Ok(())
""",
        "pending Fetch retirement must release its certified request and exact Proposal replay only after runtime and local ownership",
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
        "noncanonical Fetch exact retry",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        reject_noncanonical,
        """
let _retirement = self
    .plan_pending_fetch_retirement(&pending)
    .map_err(|error| self.fail_closed_transport(error, services))?;
""",
        "noncanonical reconstruction must preflight every retained exact Fetch index",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        reject_noncanonical,
        """
if let Err(error) = services.complete_body_reconstruction_fetch(&pending.task) {
    return Err(self.fail_closed_transport(error, services));
}
if let Err(error) = services.enqueue_body_fetch(pending.task) {
    return Err(self.fail_closed_transport(error, services));
}
""",
        "noncanonical reconstruction must reset service work while retaining exact executor ownership",
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
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
self.authenticated_genesis_replay.retain(|key, stage| {
    Some(*key) == protected_body
        && matches!(stage, AuthenticatedGenesisReplayStageV1::Stored { .. })
});
""",
        "certified-view installation must retain only the protected stored authenticated-genesis replay owner",
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
protected.validate(&self.context).map_err(|error| {
    EffectExecutorError::Contract(format!(
        "EnterView protected lock is invalid: {error}"
    ))
})?;
if protected.phase != wire::GlobalPhase::Prepare
    || protected.proposal_round.context_id != self.context.id()
    || protected.proposal_round.height != self.context.height
    || protected.proposal_round.view >= tag.view()
{
""",
        "protected Fetch transfer must validate the full PrepareQC lock before use",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
if protected.round.view < highest.round.view
    || (protected.round.view == highest.round.view
        && (protected.round != highest.round
            || protected.proposal_round != highest.proposal_round
            || protected.phase != highest.phase
            || protected.subject != highest.subject
            || protected.execution_commitment != highest.execution_commitment))
{
""",
        "protected Fetch transfer must retain the exact highest PrepareQC identity",
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
if advances_proposal_replay {
    let Some(RemoteProposalReplayStageV1::Fetch { replay, .. }) =
        self.remote_proposal_replay.remove(&key)
    else {
        unreachable!("preflighted Proposal Fetch replay remains installed")
    };
    let previous = self
        .remote_proposal_replay
        .insert(key, RemoteProposalReplayStageV1::BodyAvailable(replay));
    debug_assert!(previous.is_none());
}
Ok(())
""",
        "local Fetch and certified-request retirement and Proposal replay advancement must follow runtime publication",
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
    0,
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
    _require_rust_token_sequence(
        effects_path,
        saturation_regression,
        """
owner
    .complete_certified_fetch_for_test(
        &mut fixture.executor,
        &mut production_services,
        &ingress,
        completion,
    )
    .unwrap_or_else(|error| match error {
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(
            error,
        ) => panic!(
            "A must publish Ready and retire its physical response: {}: {}",
            error.reason(),
            error.detail(),
        ),
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
            error,
        ) => panic!(
            "A reached a restart-only persistence failure: {}: {}",
            error.reason(),
            error.detail(),
        ),
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
            error,
        ) => panic!("A failed after the persistence commit: {error}"),
    });
""",
        "the saturated response must complete through the current lifecycle Phase-B persistence adapter",
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
executor.probe_certified_response_priority(&response_a, &responder),
Ok(CertifiedResponsePriorityProbe::PreflightRequired(_))
""",
            "unprotected TC regression must first authenticate the exact live response through the read-only priority probe",
        ),
        (
            """
executor.probe_certified_response_priority(&response_a, &responder),
Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(
    CertifiedResponsePriorityNonPriority::Unsolicited { request_hash }
)) if request_hash == request_hash_a
""",
            "unprotected TC regression must classify the retired response carrier as unsolicited instead of resurrecting its token",
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
assert!(!executor.output_guard.restart_required());
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
        || (self.retained_effect_batch.is_none() && self.parked_effect_batch.is_none()
            || !Self::network_ingress_requires_reducer_order(payload))
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
        executor_context = (
            production_executor_context
            if item_name == "enqueue_network_with_ingress_ownership"
            else generic_executor_context
        )
        matches = tuple(
            candidate
            for candidate in rust_items(source, item_name)
            if candidate.brace_context == executor_context
        )
        item = matches[0] if len(matches) == 1 else None
        if len(matches) != 1:
            errors.append(
                f"{effects_path}: require exactly one production executor item "
                f"{item_name}; found {len(matches)}"
            )
        _require_rust_item_context(
            effects_path,
            item,
            executor_context,
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
