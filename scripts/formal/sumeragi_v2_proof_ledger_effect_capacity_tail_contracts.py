# Executed lexically in check_sumeragi_v2_proof_ledger.py.

def _validate_retry_capacity_source_fidelity_errors(
    effects_path: Path,
    source: str,
    errors: list[str],
    generic_executor_context: tuple,
) -> None:
    """Bind bounded, non-evicting Validate retry authority accounting."""

    pending_work = _require_rust_item(
        effects_path,
        source,
        "pending_work",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        pending_work,
        generic_executor_context,
        "executable pending-work count",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        pending_work,
        """
self.pending_signatures
    .len()
    .checked_add(self.pending_fetches.len())
    .and_then(|total| total.checked_add(self.pending_stores.len()))
    .and_then(|total| total.checked_add(self.pending_durable_validate_admissions.len()))
    .and_then(|total| {
        total.checked_add(usize::from(self.pending_live_wal_sign_admission.is_some()))
    })
    .and_then(|total| total.checked_add(self.pending_lifecycle_output_admissions.len()))
    .and_then(|total| total.checked_add(self.pending_applications.len()))
    .unwrap_or(usize::MAX)
""",
        "pending_work must remain the exact executable and service-work count",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        pending_work,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256["pending_work"],
        "executable pending-work count",
        errors,
    )

    capacity_work = _require_rust_item(
        effects_path,
        source,
        "capacity_work",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        capacity_work,
        generic_executor_context,
        "Validate retry authority capacity projection",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        capacity_work,
        """
self.pending_work()
    .checked_sub(self.pending_durable_validate_admissions.len())
    .and_then(|total| total.checked_add(self.validate_retry_authority_count()))
    .unwrap_or(usize::MAX)
""",
        "capacity_work must replace paired pending Validate work with the complete retry-authority count",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        capacity_work,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256["capacity_work"],
        "Validate retry authority capacity projection",
        errors,
    )

    authority_count = _require_rust_item(
        effects_path,
        source,
        "validate_retry_authority_count",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        authority_count,
        generic_executor_context,
        "complete Validate retry authority count",
        errors,
    )
    for required, description in (
        (
            """
self.pending_durable_validate_admissions
    .keys()
    .all(|key| self.durable_validate_retry_seals.contains_key(key))
""",
            "every pending Validate admission must be paired with a durable retry seal",
        ),
        (
            """
self.durable_validate_retry_seals.keys().all(|key| {
    !self
        .published_lifecycle_validate_retry_markers
        .contains_key(key)
})
""",
            "live/recovered seals and published markers must remain key-disjoint",
        ),
        (
            """
self.durable_validate_retry_seals
    .len()
    .checked_add(self.published_lifecycle_validate_retry_markers.len())
    .unwrap_or(usize::MAX)
""",
            "the capacity count must include both retry-authority catalogs without eviction",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            authority_count,
            required,
            description,
            errors,
        )
    _require_rust_item_token_sha256(
        effects_path,
        authority_count,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256[
            "validate_retry_authority_count"
        ],
        "complete Validate retry authority count",
        errors,
    )

    recovered_absorb = _require_rust_item(
        effects_path,
        source,
        "absorb",
        errors,
    )
    recovered_install_context = (
        (
            "impl",
            "<",
            "R",
            ":",
            "EffectRuntime",
            ">",
            "PreparedRecoveredDurableValidateRetryInstallV1",
            "<",
            "'",
            "_",
            ",",
            "R",
            ">",
        ),
    )
    _require_rust_item_context(
        effects_path,
        recovered_absorb,
        recovered_install_context,
        "cold recovered Validate retry authority capacity preflight",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        recovered_absorb,
        """
let projected_work = self
    .executor
    .capacity_work()
    .checked_add(self.prepared.len())
    .unwrap_or(usize::MAX);
if projected_work >= self.executor.config.max_pending_work {
    return Err(EffectExecutorError::PendingWorkCapacity {
        capacity: self.executor.config.max_pending_work,
    });
}
""",
        "cold census absorption must backpressure before adding a new recovered retry authority",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        recovered_absorb,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256["recovered_absorb"],
        "cold recovered Validate retry authority capacity preflight",
        errors,
    )

    published_prepare = _require_rust_item(
        effects_path,
        source,
        "prepare_published_lifecycle_validate_retry_marker",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        published_prepare,
        generic_executor_context,
        "published Validate retry authority capacity preflight",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        published_prepare,
        """
self.ensure_pending_slot()?;
Ok(PreparedPublishedLifecycleValidateRetryMarkerV1 {
    durable_receipt: durable_receipt.clone(),
    marker: None,
})
""",
        "published Validate marker preparation must reserve capacity before exposing its publication token",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        published_prepare,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256[
            "prepare_published_lifecycle_validate_retry_marker"
        ],
        "published Validate retry authority capacity preflight",
        errors,
    )

    validate_body = _require_rust_item(
        effects_path,
        source,
        "validate_body",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        validate_body,
        generic_executor_context,
        "fresh live Validate retry authority capacity preflight",
        errors,
    )
    if validate_body is not None:
        validate_tokens = rust_code_tokens(validate_body.source)
        validate_fragments = tuple(
            rust_code_tokens(fragment)
            for fragment in (
                "self.pending_durable_validate_admissions.get(&key)",
                "self.published_lifecycle_validate_retry_markers.get_mut(&key)",
                "self.durable_validate_retry_seals.get_mut(&key)",
                "let receipt = self.durable_bodies.get(&key).cloned()",
                "self.ensure_pending_slot()?",
            )
        )
        positions = [
            _token_sequence_positions(validate_tokens, fragment)
            for fragment in validate_fragments
        ]
        if not (
            all(len(found) == 1 for found in positions)
            and [found[0] for found in positions]
            == sorted(found[0] for found in positions)
        ):
            errors.append(
                f"{effects_path}:{validate_body.line}: fresh live Validate "
                "must check every existing retry owner, retain the durable "
                "body, then reserve capacity before consuming replay authority"
            )
    _require_rust_item_token_sha256(
        effects_path,
        validate_body,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256["validate_body"],
        "fresh live Validate retry authority capacity preflight",
        errors,
    )

    for item_name, expected_source, description in (
        (
            "status",
            "pending_validations: self.validate_retry_authority_count()",
            "status must report every capacity-bearing Validate retry authority",
        ),
        (
            "can_admit_local_proposal",
            "self.capacity_work() < self.config.max_pending_work",
            "local Proposal admission must use the retry-authority capacity projection",
        ),
        (
            "ensure_pending_slot",
            "self.capacity_work() >= self.config.max_pending_work",
            "ordinary pending admission must use the retry-authority capacity projection",
        ),
        (
            "ensure_signature_slot",
            "let pending_work = self.capacity_work()",
            "signature admission and preemption must use the retry-authority capacity projection",
        ),
    ):
        item = _require_rust_item(effects_path, source, item_name, errors)
        _require_rust_item_context(
            effects_path,
            item,
            generic_executor_context,
            description,
            errors,
        )
        _require_rust_token_sequence(
            effects_path,
            item,
            expected_source,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            effects_path,
            item,
            _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256[item_name],
            description,
            errors,
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
        "FetchBody retry-authority capacity deferral",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments)]",),
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
if self.capacity_work() > self.config.max_pending_work {
    return Err(EffectExecutorError::Contract(
        "pending effect work exceeded its configured capacity".to_owned(),
    ));
}
if self.capacity_work() == self.config.max_pending_work {
""",
        "FetchBody capacity checks must count retained Validate retry authorities",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        begin_fetch,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256["begin_fetch"],
        "FetchBody retry-authority capacity deferral",
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
        "bounded transactional Validate retry overlay",
        errors,
    )
    if retain is not None:
        retain_tokens = rust_code_tokens(retain.source)
        for forbidden, description in (
            (
                "self.durable_validate_retry_seals.clone()",
                "durable Validate retry seal catalog",
            ),
            (
                "self.published_lifecycle_validate_retry_markers.clone()",
                "published Validate retry marker catalog",
            ),
        ):
            count = _token_sequence_count(
                retain_tokens,
                rust_code_tokens(forbidden),
            )
            if count != 0:
                errors.append(
                    f"{effects_path}:{retain.line}: retained-effect preflight "
                    f"must not clone the complete {description}; found {count} clone(s)"
                )
        for installed_mutation, description in (
            (
                "self.durable_validate_retry_seals.insert(",
                "durable Validate retry seal overlay",
            ),
            (
                "self.published_lifecycle_validate_retry_markers.insert(",
                "published Validate retry marker overlay",
            ),
        ):
            count = _token_sequence_count(
                retain_tokens,
                rust_code_tokens(installed_mutation),
            )
            if count != 1:
                errors.append(
                    f"{effects_path}:{retain.line}: {description} must have "
                    "exactly one installed-catalog commit after complete "
                    f"preflight; found {count} mutation(s)"
                )
        ordered_fragments = tuple(
            rust_code_tokens(fragment)
            for fragment in (
                "let mut validate_retry_seal_updates = BTreeMap::<",
                "let mut published_validate_retry_marker_updates = BTreeMap::<",
                "for (index, (effect, evidence)) in effects.iter().zip(&mut ownership).enumerate()",
                "published_validate_retry_marker_updates.insert((*round, *subject), projected)",
                "validate_retry_seal_updates.insert((*round, *subject), projected.seal)",
                "if !runtime_terminal_commits.is_empty()",
                "self.runtime.commit_body_pipeline_candidate_terminals(&terminals)",
                "for (key, seal) in validate_retry_seal_updates",
                "self.durable_validate_retry_seals.insert(key, seal)",
                "for (key, marker) in published_validate_retry_marker_updates",
                "self.published_lifecycle_validate_retry_markers.insert(key, marker)",
                "let retained = effects.into_iter()",
            )
        )
        positions = [
            _token_sequence_positions(retain_tokens, fragment)
            for fragment in ordered_fragments
        ]
        if not (
            all(len(found) == 1 for found in positions)
            and [found[0] for found in positions]
            == sorted(found[0] for found in positions)
        ):
            errors.append(
                f"{effects_path}:{retain.line}: touched-key Validate retry "
                "overlays must be projected through the complete bounded "
                "batch and committed only after every runtime terminal preflight"
            )
    _require_rust_item_token_sha256(
        effects_path,
        retain,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256[
            "retain_effect_batch_at_frontier"
        ],
        "bounded transactional Validate retry overlay",
        errors,
    )

    retry_release = _require_rust_item(
        effects_path,
        source,
        "release_validate_retry_lifecycle_ordinal",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retry_release,
        generic_executor_context,
        "selected live-Apply Validate retry tombstone conversion",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retry_release,
        """
let exact_live_apply_owns_selected_body = self
    .live_lifecycle_decision_apply
    .as_ref()
    .is_some_and(|owner| {
        selected_body == Some(key)
            && self.protected_decision == Some(owner.decision)
            && (owner.certificate.proposal_round, owner.subject) == key
            && (
                owner.validated_receipt.durable().round(),
                owner.validated_receipt.durable().subject(),
            ) == key
    });
""",
        "selected Validate release must authenticate the protected Decision, certificate, and durable receipt before retaining a live-Apply tombstone",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        retry_release,
        """
let discard = selected_body.is_some_and(|selected| {
    selected != key
        || (self.decision_body_drained && !exact_live_apply_owns_selected_body)
});
""",
        "drained Decision cleanup must retain only the exact selected live-Apply tombstone",
        errors,
    )
    if retry_release is not None:
        release_tokens = rust_code_tokens(retry_release.source)
        for authority_map, description in (
            (
                "self.durable_validate_retry_seals",
                "durable Validate retry seal",
            ),
            (
                "self.published_lifecycle_validate_retry_markers",
                "published lifecycle Validate retry marker",
            ),
        ):
            release_positions = _token_sequence_positions(
                release_tokens,
                rust_code_tokens(".release_lifecycle_ordinal(lifecycle_ordinal)"),
            )
            remove_positions = _token_sequence_positions(
                release_tokens,
                rust_code_tokens(f"{authority_map}.remove(&key)"),
            )
            if len(release_positions) != 2 or len(remove_positions) != 1:
                errors.append(
                    f"{effects_path}:{retry_release.line}: selected live-Apply "
                    f"tombstone conversion must retain the exact {description} "
                    "release/removal corridor"
                )
    _require_rust_item_token_sha256(
        effects_path,
        retry_release,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256[
            "release_validate_retry_lifecycle_ordinal"
        ],
        "selected live-Apply Validate retry tombstone conversion",
        errors,
    )

    bounded_regression = _require_rust_item(
        effects_path,
        source,
        "validate_retry_tombstone_capacity_is_bounded_across_long_view_churn",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        bounded_regression,
        (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
        "long-view Validate retry tombstone capacity regression",
        errors,
        expected_attributes=("#[test]",),
    )
    for required, description in (
        (
            "for view in 1..=64",
            "the regression must exercise bounded authority across sustained view churn",
        ),
        (
            """
assert!(
    executor
        .published_lifecycle_validate_retry_markers
        .is_empty()
);
assert_eq!(executor.pending_work(), 0);
assert_eq!(executor.capacity_work(), 1);
assert_eq!(executor.status().pending_validations, 1);
""",
            "the regression must keep exactly one capacity-bearing authority",
        ),
        (
            "Err(EffectExecutorError::PendingWorkCapacity { capacity: 1 })",
            "the regression must reject a distinct authority at capacity",
        ),
        (
            """
assert_eq!(executor.durable_validate_retry_seals[&key], frontier);
assert_eq!(executor.durable_validate_retry_seals.len(), 1);

executor
    .retain_effect_batch(vec![original_validate], vec![original_ownership])
""",
            "the regression must prove capacity pressure never evicts the incumbent frontier",
        ),
        (
            """
executor
    .retain_effect_batch(vec![original_validate], vec![original_ownership])
    .expect("the delayed original-view retry remains authorized at capacity");
assert!(executor.retained_effect_batch.is_none());
""",
            "the regression must retain delayed retry authority after the pressure attempt",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            bounded_regression,
            required,
            description,
            errors,
        )
    _require_rust_item_token_sha256(
        effects_path,
        bounded_regression,
        _VALIDATE_RETRY_CAPACITY_RUST_ITEM_SHA256[
            "validate_retry_tombstone_capacity_is_bounded_across_long_view_churn"
        ],
        "long-view Validate retry tombstone capacity regression",
        errors,
    )

def _effect_capacity_terminal_retirement_source_fidelity_errors(
    effects_path: Path,
    source: str,
    errors: list[str],
    generic_executor_context: tuple,
    production_executor_context: tuple,
) -> None:
    _validate_retry_capacity_source_fidelity_errors(
        effects_path,
        source,
        errors,
        generic_executor_context,
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
self.remote_proposal_replay.retain(|key, stage| {
    Some(*key) == protected_body
        && matches!(
            stage,
            RemoteProposalReplayStageV1::Store { .. }
                | RemoteProposalReplayStageV1::Stored { .. }
        )
});
""",
        "certified-view installation must retain only the protected in-flight or stored Proposal replay owner",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
self.authenticated_genesis_replay.retain(|key, stage| {
    Some(*key) == protected_body
        && matches!(
            stage,
            AuthenticatedGenesisReplayStageV1::Store { .. }
                | AuthenticatedGenesisReplayStageV1::Stored { .. }
        )
});
""",
        "certified-view installation must retain only the protected in-flight or stored authenticated-genesis replay owner",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        install_view,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256["install_view"],
        "certified-view protected Fetch transfer",
        errors,
    )
    if install_view is not None:
        install_tokens = rust_code_tokens(install_view.source)
        for authority_map, description in (
            (
                "self.durable_validate_retry_seals",
                "durable Validate retry authority",
            ),
            (
                "self.published_lifecycle_validate_retry_markers",
                "published lifecycle Validate retry authority",
            ),
        ):
            observed = sum(
                _token_sequence_count(
                    install_tokens,
                    rust_code_tokens(f"{authority_map}{operation}"),
                )
                for operation in (".retain(", ".remove(", ".clear(", " =")
            )
            if observed != 0:
                errors.append(
                    f"{effects_path}:{install_view.line}: certified-view "
                    f"installation must leave every {description} untouched; "
                    f"found {observed} pruning mutation(s)"
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

    reconcile_protected_lock = _require_rust_item(
        effects_path,
        source,
        "reconcile_protected_lock",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        reconcile_protected_lock,
        generic_executor_context,
        "protected-lock Validate retry authority preservation",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        reconcile_protected_lock,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "reconcile_protected_lock"
        ],
        "protected-lock Validate retry authority preservation",
        errors,
    )
    if reconcile_protected_lock is not None:
        reconciliation_tokens = rust_code_tokens(reconcile_protected_lock.source)
        for authority_map, description in (
            (
                "self.durable_validate_retry_seals",
                "durable Validate retry authority",
            ),
            (
                "self.published_lifecycle_validate_retry_markers",
                "published lifecycle Validate retry authority",
            ),
        ):
            observed = sum(
                _token_sequence_count(
                    reconciliation_tokens,
                    rust_code_tokens(f"{authority_map}{operation}"),
                )
                for operation in (".retain(", ".remove(", ".clear(", " =")
            )
            if observed != 0:
                errors.append(
                    f"{effects_path}:{reconcile_protected_lock.line}: protected-lock "
                    f"reconciliation must leave every {description} untouched; "
                    f"found {observed} pruning mutation(s)"
                )

    terminal_validate_tombstone = _require_rust_item(
        effects_path,
        source,
        "retain_terminal_validate_retry_tombstone",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        terminal_validate_tombstone,
        generic_executor_context,
        "terminal Validate retry tombstone preservation",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        terminal_validate_tombstone,
        """
match self
    .durable_validate_retry_seals
    .get(&key)
    .map(DurableValidateRetrySealV1::lifecycle_ordinal)
{
    None => Err(self.close(
        EffectExecutorError::Contract(
            "terminal Validate admission lost its transient retry authority".to_owned(),
        ),
        services,
    )),
    Some(Some(_)) => Err(self.close(
        EffectExecutorError::Contract(
            "terminal Validate admission retained an ordinal-bound live retry authority"
                .to_owned(),
        ),
        services,
    )),
    Some(None) => Ok(()),
}
""",
        "terminal Validate retry tombstone helper must fail closed unless the exact seal is unbound",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        terminal_validate_tombstone,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "retain_terminal_validate_retry_tombstone"
        ],
        "terminal Validate retry tombstone preservation",
        errors,
    )
    if terminal_validate_tombstone is not None:
        removal_count = _token_sequence_count(
            rust_code_tokens(terminal_validate_tombstone.source),
            rust_code_tokens("self.durable_validate_retry_seals.remove(&key)"),
        )
        if removal_count != 0:
            errors.append(
                f"{effects_path}:{terminal_validate_tombstone.line}: terminal "
                "Validate retry tombstone helper must not remove its unbound "
                f"retry authority; found {removal_count} removal(s)"
            )

    settle_validate_admissions = _require_rust_item(
        effects_path,
        source,
        "settle_pending_durable_validate_admissions",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        settle_validate_admissions,
        production_executor_context,
        "terminal durable Validate admission settlement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        settle_validate_admissions,
        """
ProductionDurableValidateAdmissionSettlementV1::Returned {
    decision:
        AdmissionDecision::ReplayTerminal { .. }
        | AdmissionDecision::StutterTerminal { .. },
    pending: _,
} => {
    self.retain_terminal_validate_retry_tombstone(key, services)?;
}
""",
        "returned terminal Validate admission must retain its exact unbound retry tombstone",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        settle_validate_admissions,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "settle_pending_durable_validate_admissions"
        ],
        "terminal durable Validate admission settlement",
        errors,
    )
    if settle_validate_admissions is not None:
        settlement_tokens = rust_code_tokens(settle_validate_admissions.source)
        helper_calls = _token_sequence_count(
            settlement_tokens,
            rust_code_tokens(
                "self.retain_terminal_validate_retry_tombstone(key, services)"
            ),
        )
        removals = _token_sequence_count(
            settlement_tokens,
            rust_code_tokens("self.durable_validate_retry_seals.remove(&key)"),
        )
        if helper_calls != 1 or removals != 0:
            errors.append(
                f"{effects_path}:{settle_validate_admissions.line}: returned "
                "terminal Validate admission must invoke the tombstone helper "
                "exactly once and must not prune retry authority; found "
                f"{helper_calls} helper call(s) and {removals} removal(s)"
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
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
            error,
        ) => panic!(
            "A lost its productive ingress before persistence: {}: {}",
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
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
            error,
        ) => panic!("A lost its exact Runtime handoff after dequeue: {error}"),
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
