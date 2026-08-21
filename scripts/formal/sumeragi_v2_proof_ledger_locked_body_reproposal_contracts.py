def _locked_body_reproposal_source_fidelity_errors(
    formal_dir: Path, repo_root: Path = ROOT_DIR
) -> list[str]:
    """Bind the justified-high model guard to the exact production body path."""

    errors: list[str] = []

    def read_regular(path: Path, description: str) -> str | None:
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: {description} must be a regular source file")
            return None
        try:
            return path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read {description}: {error}")
            return None

    core_path = formal_dir / "SumeragiV2Core.tla"
    inductive_path = formal_dir / "SumeragiV2InductiveProofs.tla"
    core_source = read_regular(core_path, "locked-body core model")
    inductive_source = read_regular(
        inductive_path, "locked-body inductive proof"
    )
    if core_source is not None:
        stripped_core = strip_tla_comments(
            core_source, preserve_string_contents=True
        )
        exact_operators = {
            "ProposalJustified": (
                "\\/ /\\ proposal.view = 0 "
                "/\\ proposal.timeoutCertificate = NoTimeoutCertificate "
                "/\\ proposal.highestPrepareQc = NoPrepareQC "
                "/\\ proposal.justifyRank = NoRank "
                "/\\ proposal.justifySubject = context.parent "
                "\\/ /\\ proposal.view > 0 "
                "/\\ lastInstalledTc[node] # NoTimeoutCertificate "
                "/\\ [node |-> node, tc |-> lastInstalledTc[node]] "
                "\\in installedTCs "
                "/\\ lastInstalledTc[node].context = context "
                "/\\ lastInstalledTc[node].view + 1 = proposal.view "
                "/\\ TCValid(lastInstalledTc[node]) "
                "/\\ proposal.timeoutCertificate = lastInstalledTc[node] "
                "/\\ proposal.highestPrepareQc = "
                "lastInstalledTc[node].highestPrepareQc "
                "/\\ proposal.justifyRank = "
                "PrepareQcRank(proposal.highestPrepareQc) "
                "/\\ proposal.justifySubject = "
                "PrepareQcSubject(proposal.highestPrepareQc) "
                "/\\ proposal.justifyRank < proposal.view"
            ),
            "LocalProposalJustification": (
                "LET roundView == nodeView[node] "
                "IN IF roundView = 0 "
                "THEN [timeoutCertificate |-> NoTimeoutCertificate, "
                "highestPrepareQc |-> NoPrepareQC] "
                "ELSE LET tc == lastInstalledTc[node] "
                "IN [timeoutCertificate |-> tc, "
                "highestPrepareQc |-> tc.highestPrepareQc]"
            ),
            "LocalProposalFor": (
                "LET justification == LocalProposalJustification(node) "
                "IN Proposal(context, nodeView[node], subject, node, "
                "justification.timeoutCertificate, "
                "justification.highestPrepareQc)"
            ),
            "LocalProposalReproposesJustifiedHigh": (
                "\\/ proposal.justifyRank = NoRank "
                "\\/ proposal.subject = proposal.justifySubject"
            ),
        }
        for symbol, expected in exact_operators.items():
            declarations = re.findall(
                rf"(?m)^{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==",
                stripped_core,
            )
            if len(declarations) != 1:
                errors.append(
                    f"{core_path}: require exactly one top-level {symbol} "
                    f"operator; found {len(declarations)}"
                )
                continue
            extracted = _top_level_operator_body(
                core_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(f"{core_path}: cannot extract {symbol}")
                continue
            body, line = extracted
            observed = " ".join(body.split())
            if observed != expected:
                errors.append(
                    f"{core_path}:{line}: {symbol} must retain the exact "
                    f"justified-high subject contract {expected!r}; found "
                    f"{observed!r}"
                )

        begin_declarations = re.findall(
            r"(?m)^BeginLocalProposal\s*\([^)=]*\)\s*==", stripped_core
        )
        if len(begin_declarations) != 1:
            errors.append(
                f"{core_path}: require exactly one top-level BeginLocalProposal "
                f"action; found {len(begin_declarations)}"
            )
        else:
            extracted = _top_level_operator_body(
                core_source,
                "BeginLocalProposal",
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(f"{core_path}: cannot extract BeginLocalProposal")
            else:
                body, line = extracted
                body_tokens = " ".join(body.split())
                exact_guard = (
                    "/\\ ProposalWireValidFor(node, proposal) "
                    "/\\ LocalProposalReproposesJustifiedHigh(proposal)"
                )
                observed_count = body_tokens.count(exact_guard)
                if observed_count != 1:
                    errors.append(
                        f"{core_path}:{line}: BeginLocalProposal must enforce "
                        "the exact justified-high subject guard immediately after "
                        "wire validation exactly once; found "
                        f"{observed_count} guarded paths"
                    )

    if inductive_source is not None:
        stripped_inductive = strip_tla_comments(
            inductive_source, preserve_string_contents=True
        )
        theorem_symbol = "BeginLocalProposalReproposesExactJustifiedHigh"
        declarations = re.findall(
            rf"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            rf"{theorem_symbol}\s*(?:\([^)=]*\))?\s*==",
            stripped_inductive,
        )
        theorem = _top_level_theorem_body(
            inductive_source,
            theorem_symbol,
            preserve_string_contents=True,
        )
        if len(declarations) != 1 or theorem is None:
            errors.append(
                f"{inductive_path}: require exactly one top-level "
                f"{theorem_symbol} theorem; found {len(declarations)}"
            )
        else:
            body, line = theorem
            parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )
            expected_statement = (
                "\\A node, subject: BeginLocalProposal(node, subject) "
                "=> LET justification == LocalProposalJustification(node) "
                "IN \\/ justification.rank = NoRank "
                "\\/ subject = justification.subject"
            )
            observed_statement = " ".join(parts[0].split())
            if observed_statement != expected_statement:
                errors.append(
                    f"{inductive_path}:{line}: {theorem_symbol} must state the "
                    "exact nontrivial justified-high subject postcondition; "
                    f"found {observed_statement!r}"
                )
            expected_proof = (
                "SMT DEF BeginLocalProposal, "
                "LocalProposalReproposesJustifiedHigh, LocalProposalFor, "
                "LocalProposalJustification, Proposal"
            )
            observed_proof = " ".join(parts[1].split()) if len(parts) == 2 else ""
            if observed_proof != expected_proof:
                errors.append(
                    f"{inductive_path}:{line}: {theorem_symbol} must retain its "
                    "exact action-to-justification proof dependency"
                )

    directive_path = repo_root / "crates/iroha_core/src/sumeragi/v2.rs"
    directive_source = read_regular(
        directive_path, "local-proposal directive production source"
    )
    if directive_source is not None:
        directive_structural = mask_rust_comments_and_literals(directive_source)
        directive_declarations = re.findall(
            r"(?m)^[ \t]*pub[ \t]*\([ \t]*crate[ \t]*\)[ \t]+"
            r"struct[ \t]+LocalProposalDirective\b",
            directive_structural,
        )
        if len(directive_declarations) != 1:
            errors.append(
                f"{directive_path}: require exactly one crate-visible "
                "LocalProposalDirective declaration; found "
                f"{len(directive_declarations)}"
            )
        _require_rust_source_token_sequence(
            directive_path,
            directive_source,
            """
pub(crate) struct LocalProposalDirective {
    tag: reducer::EventTag,
    leader: wire::ValidatorIndex,
    locked_round: Option<wire::ConsensusRound>,
    locked_subject: Option<wire::BlockSubject>,
    decided_subject: Option<wire::BlockSubject>,
}
""",
            "LocalProposalDirective production fields must remain private",
            errors,
        )
        directive_for_test = _require_qualified_rust_item(
            directive_path,
            directive_source,
            "LocalProposalDirective",
            "for_test",
            errors,
            "test-only exact local-proposal directive constructor",
            expected_attributes=("#[cfg(test)]",),
        )
        _require_rust_item_token_sha256(
            directive_path,
            directive_for_test,
            _LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256[
                "local_proposal_directive_for_test"
            ],
            "test-only exact local-proposal directive constructor",
            errors,
        )

        recovered_attempt_declarations = re.findall(
            r"(?m)^[ \t]*pub[ \t]*\([ \t]*in[ \t]+crate::sumeragi[ \t]*\)"
            r"[ \t]+struct[ \t]+RecoveredLifecycleLocalProposalAttemptV1\b",
            directive_structural,
        )
        if len(recovered_attempt_declarations) != 1:
            errors.append(
                f"{directive_path}: require exactly one sumeragi-visible opaque "
                "RecoveredLifecycleLocalProposalAttemptV1 declaration; found "
                f"{len(recovered_attempt_declarations)}"
            )
        _require_rust_source_token_sequence(
            directive_path,
            directive_source,
            """
pub(in crate::sumeragi) struct RecoveredLifecycleLocalProposalAttemptV1 {
    tag: reducer::EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
}
""",
            "recovered local-Proposal ownership must remain opaque and comparison-only",
            errors,
        )
        recovered_attempt_structs = rust_struct_items(
            directive_source, "RecoveredLifecycleLocalProposalAttemptV1"
        )
        if len(recovered_attempt_structs) != 1:
            errors.append(
                f"{directive_path}: require exactly one real "
                "RecoveredLifecycleLocalProposalAttemptV1 struct; found "
                f"{len(recovered_attempt_structs)}"
            )
        else:
            _require_rust_item_context(
                directive_path,
                recovered_attempt_structs[0],
                (),
                "opaque recovered local-Proposal ownership type",
                errors,
                expected_attributes=(
                    '#[must_use = "recovered local Proposal ownership must initialize runner proposal state"]',
                ),
            )
        recovered_attempt_methods = tuple(
            item.name
            for item in _rust_all_function_items(directive_source)
            if item.brace_context
            == (("impl", "RecoveredLifecycleLocalProposalAttemptV1"),)
        )
        if recovered_attempt_methods != (
            "from_authenticated_durable_current_round",
            "from_control",
            "exactly_matches_directive",
            "for_test",
        ):
            errors.append(
                f"{directive_path}: opaque recovered local-Proposal ownership "
                "must expose only its reviewed mint, comparison oracle, and "
                f"test fixture; found {recovered_attempt_methods!r}"
            )
        recovered_attempt_for_test = _require_qualified_rust_item(
            directive_path,
            directive_source,
            "RecoveredLifecycleLocalProposalAttemptV1",
            "for_test",
            errors,
            "test-only opaque recovered local-Proposal fixture",
            expected_attributes=("#[cfg(test)]",),
        )
        _require_rust_item_token_sha256(
            directive_path,
            recovered_attempt_for_test,
            _LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256[
                "recovered_local_proposal_attempt_for_test"
            ],
            "test-only opaque recovered local-Proposal fixture",
            errors,
        )

    prepared_path = (
        repo_root / "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"
    )
    prepared_source = read_regular(
        prepared_path, "affine prepared local-Proposal ownership source"
    )
    if prepared_source is not None:
        _require_rust_source_token_sequence(
            prepared_path,
            prepared_source,
            """
pub(in crate::sumeragi) struct ProductionLifecyclePreparedLocalProposalStateV1 {
    runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1,
    context_id: wire::HeightContextId,
    directive: super::super::v2::LocalProposalDirective,
}
""",
            "prepared local-Proposal receipt must retain private affine runner, context, and directive ownership",
            errors,
        )
        prepared_structs = rust_struct_items(
            prepared_source, "ProductionLifecyclePreparedLocalProposalStateV1"
        )
        if len(prepared_structs) != 1:
            errors.append(
                f"{prepared_path}: require exactly one real "
                "ProductionLifecyclePreparedLocalProposalStateV1 struct; found "
                f"{len(prepared_structs)}"
            )
        else:
            _require_rust_item_context(
                prepared_path,
                prepared_structs[0],
                (),
                "affine prepared local-Proposal ownership type",
                errors,
                expected_attributes=(
                    '#[must_use = "prepared local-Proposal state must enter lifecycle activation"]',
                ),
            )

    production_specs = (
        (
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            (
                (
                    "on_resume_after_replay",
                    (("impl", "Reducer"),),
                    (),
                    "on_resume_after_replay",
                    "replay proposal authorization boundary",
                ),
                (
                    "durable_proposal_is_active",
                    (("impl", "Reducer"),),
                    (),
                    "durable_proposal_is_active",
                    "durable proposal replay authorization kernel",
                ),
                (
                    "proposal_is_safe_for_durable_lock",
                    (("impl", "Reducer"),),
                    (),
                    "proposal_is_safe_for_durable_lock",
                    "shared live-and-replay safe-value kernel",
                ),
                (
                    "safe_to_prepare",
                    (("impl", "Reducer"),),
                    (),
                    "safe_to_prepare",
                    "live proposal safe-value call path",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            (
                (
                    "apply",
                    (("impl", "DurableState"),),
                    ("#[allow(clippy::too_many_lines)]",),
                    "wal_apply",
                    "atomic durable WAL application wrapper",
                ),
                (
                    "apply_in_place",
                    (("impl", "DurableState"),),
                    ("#[allow(clippy::too_many_lines)]",),
                    "wal_apply_in_place",
                    "durable proposal/timeout replay kernel",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            (
                (
                    "local_proposal_directive",
                    (("impl", "SumeragiV2Adapter"),),
                    (),
                    "local_proposal_directive",
                    "durable lock-to-runner directive projection",
                ),
                (
                    "from_authenticated_durable_current_round",
                    (("impl", "RecoveredLifecycleLocalProposalAttemptV1"),),
                    (),
                    "recovered_local_proposal_attempt_from_durable_round",
                    "durable-current-round opaque local-Proposal attempt mint",
                ),
                (
                    "from_control",
                    (("impl", "RecoveredLifecycleLocalProposalAttemptV1"),),
                    (),
                    "recovered_local_proposal_attempt_from_control",
                    "WAL-authenticated opaque local-Proposal attempt mint",
                ),
                (
                    "exactly_matches_directive",
                    (("impl", "RecoveredLifecycleLocalProposalAttemptV1"),),
                    (),
                    "recovered_local_proposal_attempt_exactly_matches_directive",
                    "opaque recovered local-Proposal directive comparison",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            (
                (
                    "can_schedule_local_proposal",
                    (
                        (
                            "impl",
                            "V2EffectExecutor",
                            "<",
                            "SerializedV2Runtime",
                            ">",
                        ),
                    ),
                    (),
                    "can_schedule_local_proposal",
                    "serialized active-view one-shot producer reservation",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            (
                (
                    "bind_recovered_local_proposal",
                    (("impl", "ProductionLifecyclePreActivationRunnerBorrowV1"),),
                    (),
                    "bind_recovered_local_proposal",
                    "one-shot recovered Proposal bind into runner-local state",
                ),
                (
                    "from_recovered_lifecycle_attempt",
                    (("impl", "LocalProposalState"),),
                    ("#[cfg_attr(not(test), allow(dead_code))]",),
                    "from_recovered_lifecycle_attempt",
                    "runner-local recovered Proposal attempt initialization",
                ),
                (
                    "locked_body_recovery_plan",
                    (),
                    (),
                    "locked_body_recovery_plan",
                    "locked-body acquisition and reproposal eligibility projection",
                ),
                (
                    "local_consensus_duties",
                    (),
                    (),
                    "local_consensus_duties",
                    "independent lane-author and successor-global duty projection",
                ),
                (
                    "schedule_local_proposal",
                    (),
                    ("#[allow(clippy::too_many_arguments)]",),
                    "schedule_local_proposal",
                    "exact locked-body proposal scheduler",
                ),
                (
                    "submit_exact_body",
                    (),
                    (),
                    "submit_exact_body",
                    "locked-body subject admission boundary",
                ),
                (
                    "encode_exact_local_body",
                    (),
                    (),
                    "encode_exact_local_body",
                    "canonical locked-body subject validation kernel",
                ),
                (
                    "submit_encoded_body",
                    (),
                    (),
                    "submit_encoded_body",
                    "encoded-body executor admission bridge",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
            (
                (
                    "initialize_recovered_local_proposal",
                    (("impl", "LaunchedProductionLifecycleV1"),),
                    ("#[allow(dead_code, clippy::result_large_err)]",),
                    "initialize_recovered_local_proposal",
                    "closed-ingress affine recovered Proposal join",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
            (
                (
                    "exactly_matches",
                    (("impl", "ProductionLifecyclePreparedLocalProposalStateV1"),),
                    (),
                    "prepared_local_proposal_exactly_matches",
                    "affine prepared Proposal context and directive receipt",
                ),
                (
                    "activate_with",
                    (("impl", "LaunchedProductionLifecycleV1"),),
                    (),
                    "activate_with_prepared_local_proposal",
                    "activation revalidation of the affine Proposal receipt",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            (
                (
                    "run_non_pending_lifecycle_loop",
                    (),
                    ("#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",),
                    "run_non_pending_lifecycle_loop",
                    "lifecycle runner recovered Proposal initialization handoff",
                ),
                (
                    "run_lifecycle_active_height",
                    (),
                    ("#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",),
                    "run_lifecycle_active_height",
                    "active locked-body refinement request",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner_tests.rs",
            (
                (
                    "recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade",
                    (),
                    ("#[test]",),
                    "recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade",
                    "opaque recovered-Proposal same-view one-shot regression",
                ),
                (
                    "locked_body_recovery_is_independent_of_reproposal_gates",
                    (),
                    ("#[test]",),
                    "locked_body_recovery_is_independent_of_reproposal_gates",
                    "locked-body acquisition/reproposal split regression",
                ),
                (
                    "lane_production_duty_survives_successor_global_roster_removal",
                    (),
                    ("#[test]",),
                    "lane_production_duty_survives_successor_global_roster_removal",
                    "successor-roster-independent lane duty regression",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs",
            (
                (
                    "prepared_local_proposal_state_is_affine_and_context_directive_bound",
                    (),
                    ("#[test]",),
                    "prepared_local_proposal_state_is_affine_and_context_directive_bound",
                    "affine prepared local-Proposal receipt regression",
                ),
            ),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_candidate.rs",
            (
                (
                    "validate_request",
                    (),
                    (),
                    "validate_request",
                    "fresh-candidate locked-body rejection",
                ),
            ),
        ),
    )
    production_items: dict[str, tuple[Path, RustItem | None]] = {}
    for relative, specs in production_specs:
        path = repo_root / relative
        if relative == "crates/iroha_core/src/sumeragi/v2_runner_tests.rs":
            path, source = _read_reviewed_rust_source(
                repo_root,
                relative,
                errors,
                "locked-body runner regression source",
            )
        else:
            source = read_regular(path, "locked-body production refinement source")
        if source is None:
            continue
        for item_name, context, attributes, digest_key, description in specs:
            item = _require_rust_item(path, source, item_name, errors)
            _require_rust_item_context(
                path,
                item,
                context,
                description,
                errors,
                expected_attributes=attributes,
            )
            _require_rust_item_token_sha256(
                path,
                item,
                _LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256[digest_key],
                description,
                errors,
            )
            production_items[digest_key] = (path, item)

    def require_sequence(
        digest_key: str, source: str, description: str, *, count: int = 1
    ) -> None:
        path_item = production_items.get(digest_key)
        if path_item is None:
            return
        path, item = path_item
        _require_rust_token_sequence(
            path, item, source, description, errors, count=count
        )

    def require_order(
        digest_key: str, markers: tuple[str, ...], description: str
    ) -> None:
        path_item = production_items.get(digest_key)
        if path_item is None:
            return
        path, item = path_item
        if item is None:
            return
        item_tokens = rust_code_tokens(item.source)
        cursor = 0
        for marker in markers:
            marker_tokens = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(
                        cursor, len(item_tokens) - len(marker_tokens) + 1
                    )
                    if item_tokens[index : index + len(marker_tokens)]
                    == marker_tokens
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{path}:{item.line}: {description} must preserve the "
                    "exact reviewed production order"
                )
                return
            cursor = position + len(marker_tokens)

    require_sequence(
        "on_resume_after_replay",
        """
} else if let Some(proposal) = self
    .durable
    .proposal_intent(round)
    .filter(|proposal| Self::durable_proposal_is_active(&self.durable, proposal))
{
    self.signature_queue
        .push_back(SignableMessage::Proposal(proposal.clone()));
}
""",
        "replay must filter a durable proposal through current lock authorization before signing",
    )
    require_sequence(
        "durable_proposal_is_active",
        """
durable.decision().is_none()
    && proposal.round() == Round::new(durable.height(), durable.current_view())
    && durable.timeout_intent(proposal.round()).is_none()
    && durable.proposal_intent(proposal.round()) == Some(proposal)
    && exact_justification
    && Self::proposal_is_safe_for_durable_lock(durable, proposal)
""",
        "durable proposal replay authorization must invoke the shared safe-value kernel",
    )
    require_sequence(
        "proposal_is_safe_for_durable_lock",
        """
let Some(locked) = durable.locked() else {
    return true;
};
let subject = proposal.manifest().subject();
if locked.subject() == subject {
    return true;
}
let ProposalJustification::Timeout(certificate) = proposal.justification() else {
    return false;
};
certificate.highest_prepare().is_some_and(|highest| {
    highest.phase() == Phase::Prepare
        && highest.subject() == subject
        && highest.round().view() > locked.round().view()
})
""",
        "the shared safe-value kernel must require the lock subject or a strictly higher matching PrepareQC",
    )
    require_sequence(
        "safe_to_prepare",
        """
Self::proposal_is_safe_for_durable_lock(&self.durable, proposal)
""",
        "live proposal admission must invoke the same safe-value kernel as replay",
    )
    require_sequence(
        "recovered_local_proposal_attempt_from_durable_round",
        """
let tag = adapter.reducer.current_tag();
let round = reducer::Round::new(tag.height(), tag.view());
let Some(proposal) = adapter.reducer.durable_state().proposal_intent(round) else {
    return Ok(None);
};
let wire_round = adapter.registry.round_to_wire(proposal.round());
let subject = adapter.registry.subject(proposal.manifest().subject())?;
if wire_round.context_id != adapter.wire_context.id()
    || wire_round.height != tag.height()
    || wire_round.view != tag.view()
{
    return Err(AdapterError::RecoveredStartupEffectMismatch);
}
Ok(Some(Self { tag, round: wire_round, subject, }))
""",
        "only the replay-authenticated durable current-round ProposalIntent may mint the startup attempt",
    )
    require_sequence(
        "recovered_local_proposal_attempt_from_control",
        """
let AdapterEffect::Sign {
    tag,
    request: SignRequest::Proposal(proposal),
} = &control.effect
else {
    return None;
};
Some(Self {
    tag: *tag,
    round: proposal.round,
    subject: proposal.subject,
})
""",
        "only a WAL-authenticated Proposal control Sign may mint the opaque recovered attempt",
    )
    require_sequence(
        "recovered_local_proposal_attempt_exactly_matches_directive",
        """
self.tag == current.tag()
    && self.round.height == self.tag.height()
    && self.round.view == self.tag.view()
    && current.decided_subject().is_none()
    && current.locked_body().is_none_or(|(locked_round, _)| {
        self.round.context_id == locked_round.context_id
            && self.round.height == locked_round.height
    })
""",
        "the opaque recovered attempt must bind tag, view, context, height, and decision state while a same-view lock upgrade stays one-shot",
    )
    require_sequence(
        "bind_recovered_local_proposal",
        """
let Some(local_proposal) = self.local_proposal.as_mut() else {
    return false;
};
if !local_proposal.state.is_pristine() {
    return false;
}
local_proposal.state =
    LocalProposalState::from_recovered_lifecycle_attempt(true, directive);
true
""",
        "runner-local Proposal state must accept the affine recovered owner exactly once from pristine state",
    )
    require_sequence(
        "from_recovered_lifecycle_attempt",
        """
Self {
    attempted: already_attempted.then_some(LocalProposalOwner::from(current)),
    ..Self::default()
}
""",
        "the recovered attempt must initialize only the exact current runner owner",
    )
    require_sequence(
        "initialize_recovered_local_proposal",
        """
let recovered = self.recovered_local_proposal_attempt.take();
let (context_id, directive, runner) =
    self.with_runner_setup_transaction(move |executor, _services| {
        let directive = executor
            .local_proposal_directive()
            .map_err(ProductionLifecyclePreActivationErrorV1::LocalProposalDirective)?;
        match recovered {
            Some(recovered) if recovered.exactly_matches_directive(directive) => {
                if !runner.bind_recovered_local_proposal(directive) {
                    return Err(
                        ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine,
                    );
                }
            }
            Some(_) => {
                return Err(
                    ProductionLifecyclePreActivationErrorV1::RecoveredProposalMismatch,
                );
            }
            None if !runner.local_proposal_state_is_pristine() => {
                return Err(
                    ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine,
                );
            }
            None => {}
        }
        Ok((executor.context().id(), directive, runner))
    })?;
""",
        "preactivation must consume and compare the opaque attempt inside one closed-ingress transaction",
    )
    require_sequence(
        "initialize_recovered_local_proposal",
        """
let prepared = super::ProductionLifecyclePreparedLocalProposalStateV1 {
    runner,
    context_id,
    directive,
};
Ok((directive, prepared))
""",
        "the preactivation join must return only the reducer directive and affine prepared state",
    )
    require_order(
        "initialize_recovered_local_proposal",
        (
            "self.recovered_local_proposal_attempt.take()",
            "self.with_runner_setup_transaction(",
            ".local_proposal_directive()",
            "recovered.exactly_matches_directive(directive)",
            "runner.bind_recovered_local_proposal(directive)",
            "ProductionLifecyclePreparedLocalProposalStateV1 {",
            "Ok((directive, prepared))",
        ),
        "closed-ingress affine recovered Proposal handoff",
    )
    require_sequence(
        "prepared_local_proposal_exactly_matches",
        """
self.context_id == context_id
    && self.directive == directive
    && self
        .runner
        .prepared_local_proposal_exactly_matches(directive)
""",
        "the affine prepared owner must retain its exact context, directive, and runner state",
    )
    require_order(
        "activate_with_prepared_local_proposal",
        (
            "self.recovered_local_proposal_attempt.is_some()",
            ".local_proposal_directive()",
            "local_proposal.exactly_matches(self.executor.context().id(), current_directive)",
            ".arm_live_clocks(clock_activation, now)",
            "ActivatedProductionLifecycleV1 {",
            "local_proposal,",
        ),
        "activation must reject an unconsumed attempt and revalidate the affine receipt before clocks",
    )
    require_order(
        "run_non_pending_lifecycle_loop",
        (
            "preactivation.initialize_recovered_local_proposal(setup_runner)",
            "preactivation.activate(height_started_at, local_proposal)",
            "run_lifecycle_active_height(",
            "activated,",
            "active_runner,",
        ),
        "the ordinary lifecycle must initialize and consume the affine Proposal owner before live service",
    )
    require_sequence(
        "run_lifecycle_active_height",
        """
let lock_outcome = lane_work.mark_global_body_locked(locked_round, locked)?;
if lock_outcome == GlobalBodyLockOutcome::Inserted && local_validator.is_some() {
    services
        .request_locked_candidate(executor.current_tag(), locked_round, locked)
        .map_err(V2RunnerError::Service)?;
}
""",
        "locked-body refinement evidence must be requested only by a local validator at exact first admission",
    )
    require_sequence(
        "locked_body_recovery_plan",
        """
let request = directive
    .decided_subject()
    .is_none()
    .then(|| directive.locked_body())
    .flatten()
    .map(|(round, subject)| (directive.tag(), round, subject));
""",
        "locked-body acquisition must project the exact undecided lock independently of reproposal eligibility",
    )
    require_sequence(
        "locked_body_recovery_plan",
        """
LockedBodyRecoveryPlan {
    request,
    may_repropose: request.is_some()
        && directive.leader() == local_validator
        && attempted != Some(owner)
        && can_admit_local_proposal,
}
""",
        "locked-body reproposal eligibility must retain leader, owner, and capacity gates without suppressing acquisition",
    )
    require_sequence(
        "local_consensus_duties",
        """
LocalConsensusDuties {
    autonomous_lane_view: (directive.decided_subject().is_none()
        && directive.locked_body().is_none())
    .then_some(directive.tag().view()),
    global_validator,
}
""",
        "lane-author duty must remain independent of successor-global validator membership while locks and decisions suppress fresh work",
    )
    require_sequence(
        "wal_apply",
        """
let mut next = self.clone();
next.apply_in_place(context, local_validator, entry)?;
*self = next;
Ok(())
""",
        "WAL application must atomically commit the reviewed in-place transition",
    )
    require_sequence(
        "wal_apply_in_place",
        """
ProposalJustification::Timeout(certificate)
    if proposal.round().view() > 0
        && certificate.validate(context).is_ok()
        && self.is_exact_local_proposal_timeout_justification(
            proposal.round().view(),
            certificate,
        )
        && certificate.round().view().checked_add(1)
            == Some(proposal.round().view())
        && certificate.highest_prepare().is_none_or(|highest| {
            highest.subject() == proposal.manifest().subject()
        }) =>
{
    certificate.highest_prepare()
}
""",
        "WAL replay must bind a Timeout justification high certificate to the proposal subject",
    )
    require_sequence(
        "wal_apply_in_place",
        """
if let Some(locked) = &self.locked
    && locked.subject() != proposal.manifest().subject()
    && proposal_high.is_none_or(|highest| {
        highest.phase() != Phase::Prepare
            || highest.subject() != proposal.manifest().subject()
            || highest.round().view() <= locked.round().view()
    })
{
    return Err(ReplayError::InvalidProposalIntent);
}
""",
        "WAL replay must reject an unrelated proposal unless a strictly higher matching PrepareQC supersedes the lock",
    )
    require_sequence(
        "wal_apply_in_place",
        """
if let Some(highest) = &selected {
    match &self.highest_prepare {
        None => self.highest_prepare = Some(highest.clone()),
        Some(existing) if highest.round().view() > existing.round().view() => {
            self.highest_prepare = Some(highest.clone());
        }
        Some(existing)
            if highest.round().view() == existing.round().view()
                && highest.subject() != existing.subject() =>
        {
            return Err(ReplayError::ConflictingHighestPrepare);
        }
        Some(_) => {}
    }

    match &self.locked {
        None => self.locked = Some(highest.clone()),
        Some(locked) if highest.round().view() > locked.round().view() => {
            self.locked = Some(highest.clone());
        }
        Some(locked)
            if highest.round().view() == locked.round().view()
                && highest.subject() != locked.subject() =>
        {
            return Err(ReplayError::LockRegression);
        }
        Some(_) => {}
    }
}
""",
        "InstallTimeout must promote its selected high PrepareQC into the non-regressing durable lock",
    )
    require_sequence(
        "wal_apply_in_place",
        """
let strict_same_round_upgrade =
    self.is_strict_same_round_timeout_upgrade(certificate);
if certificate.round().view() < self.current_view && !strict_same_round_upgrade {
    return Err(ReplayError::ViewRegression);
}
""",
        "InstallTimeout must admit only the source-shared strict immediate-predecessor high-QC upgrade",
    )
    require_sequence(
        "local_proposal_directive",
        """
let locked = durable.locked();
let locked_round =
    locked.map(|certificate| self.registry.round_to_wire(certificate.round()));
let locked_subject = locked
    .map(|certificate| self.registry.subject(certificate.subject()))
    .transpose()?;
""",
        "the runner directive must project the exact durable lock round and subject",
    )
    require_sequence(
        "local_proposal_directive",
        """
Ok(LocalProposalDirective {
    tag: self.reducer.current_tag(),
    leader,
    locked_round,
    locked_subject,
    decided_subject,
})
""",
        "the adapter must return the durable lock projection without reconstruction",
    )
    require_sequence(
        "can_schedule_local_proposal",
        """
self.ensure_open()?;
if !self.can_admit_local_proposal() {
    return Ok(false);
}
let tag = self.current_tag();
self.runtime
    .local_proposal_admission_available(tag)
    .map_err(EffectExecutorError::Runtime)
""",
        "proposal scheduling must combine queue capacity with the serialized active-view one-shot producer reservation",
    )
    require_sequence(
        "schedule_local_proposal",
        """
let directive = executor.local_proposal_directive()?;
let duties = local_consensus_duties(directive, local_validator);
if let Some(active_view) = duties.autonomous_lane_view {
    lane_work.schedule_autonomous_lane_production(active_view, candidate_limits)?;
}
let Some(local_validator) = duties.global_validator else {
    return Ok(());
};
let owner = proposal_state.reconcile(LocalProposalOwner::from(directive));
""",
        "lane-author service must precede the successor-global observer return",
    )
    require_sequence(
        "schedule_local_proposal",
        "executor.can_schedule_local_proposal()?",
        "every locked-load and fresh candidate observation must retain the serialized one-shot producer gate",
        count=3,
    )
    require_sequence(
        "schedule_local_proposal",
        """
let recovery_plan = locked_body_recovery_plan(
    directive,
    local_validator,
    proposal_state.attempted,
    executor.can_schedule_local_proposal()?,
);
if let Some((tag, locked_round, locked)) = recovery_plan.request {
    services
        .request_locked_candidate(tag, locked_round, locked)
        .map_err(V2RunnerError::Service)?;
}
while let Some(loaded) = services.take_loaded_candidate() {
""",
        "locked-body acquisition must remain independent of reproposal gates and precede loaded-body observation",
    )
    require_sequence(
        "schedule_local_proposal",
        """
if loaded.tag() != current.tag()
    || current.locked_body() != Some((loaded_round, loaded_subject))
{
""",
        "the runner must discard a load which no longer matches the exact current lock",
    )
    require_sequence(
        "schedule_local_proposal",
        """
let current_recovery_plan = locked_body_recovery_plan(
    current,
    local_validator,
    proposal_state.attempted,
    executor.can_schedule_local_proposal()?,
);
if !current_recovery_plan.may_repropose {
    continue;
}
""",
        "loaded locked-body reproposal must recheck the current leader, owner, and capacity gates",
    )
    require_sequence(
        "schedule_local_proposal",
        """
executor.retain_locked_body_for_recovery(
    current.tag(),
    loaded_round,
    loaded_subject,
    canonical_wire.clone(),
    services,
)?;
submit_exact_body(
    context,
    current,
    canonical_wire,
    executor,
    services,
    proposal_state,
)?;
proposal_state.attempted = Some(current_owner);
""",
        "reproposal must retain the exact-origin recovery carrier before submitting the unchanged locked body",
    )
    require_sequence(
        "schedule_local_proposal",
        """
submit_exact_body(
    context,
    current,
    canonical_wire,
    executor,
    services,
    proposal_state,
)?;
""",
        "a current locked-body load must enter the exact-body admission path",
    )
    require_sequence(
        "schedule_local_proposal",
        """
if directive.locked_body().is_some() {
    return Ok(());
}
""",
        "a locked directive must never fall through to fresh or genesis candidate construction",
    )
    require_sequence(
        "submit_exact_body",
        """
let payload = encode_exact_local_body(
    context,
    directive.tag(),
    directive.locked_subject(),
    &canonical_wire,
)?;
""",
        "exact-body submission must call the canonical encoder with the durable locked subject",
    )
    require_sequence(
        "encode_exact_local_body",
        """
if locked_subject.is_some_and(|locked| locked != subject) {
    return Err(V2RunnerError::LockedBodyMismatch);
}
""",
        "the canonical exact-body encoder must reject bytes whose subject differs from the durable lock",
    )
    require_sequence(
        "submit_exact_body",
        """
submit_encoded_body(
    LocalProposalOwner::from(directive),
    canonical_wire,
    payload,
    executor,
    services,
    proposal_state,
)
""",
        "the verified exact body must retain its directive owner through encoding",
    )
    require_sequence(
        "schedule_local_proposal",
        """
submit_exact_body(
    context,
    directive,
    canonical_height_one_proposal_wire(body)?,
    executor,
    services,
    proposal_state,
)?;
""",
        "genesis proposal submission must use the same exact-body admission kernel",
    )
    require_sequence(
        "submit_encoded_body",
        """
let manifest = services
    .register_outbound_payload(owner.tag, payload)
    .map_err(V2RunnerError::Service)?;
proposal_state.submitted = Some((owner, manifest.subject));
executor.admit_local_proposal(owner.tag, manifest, canonical_wire, services)?;
""",
        "encoded proposal admission must preserve the exact owner, manifest subject, and canonical bytes",
    )
    require_sequence(
        "validate_request",
        """
if request.directive.locked_subject().is_some() {
    return Err(CandidateError::LockedBodyMustBeReproposed);
}
""",
        "fresh candidate construction must reject every directive which owns a locked subject",
    )
    require_sequence(
        "locked_body_recovery_is_independent_of_reproposal_gates",
        """
for (local_validator, attempted, can_admit) in [
    (nonleader, None, true),
    (leader, Some(owner), true),
    (leader, None, false),
] {
    let plan = locked_body_recovery_plan(directive, local_validator, attempted, can_admit);
    assert_eq!(plan.request, expected_request);
    assert!(!plan.may_repropose, "body recovery must survive every local reproposal gate");
}
let eligible = locked_body_recovery_plan(directive, leader, None, true);
assert_eq!(eligible.request, expected_request);
assert!(eligible.may_repropose);
""",
        "the locked-body regression must separate unconditional exact acquisition from leader/owner/capacity reproposal gates",
    )
    require_sequence(
        "locked_body_recovery_is_independent_of_reproposal_gates",
        """
assert_eq!(
    locked_body_recovery_plan(decided, leader, None, true),
    LockedBodyRecoveryPlan {
        request: None,
        may_repropose: false,
    }
);
""",
        "the locked-body regression must prove that a decision retires acquisition and reproposal",
    )
    require_sequence(
        "lane_production_duty_survives_successor_global_roster_removal",
        """
assert_eq!(
    local_consensus_duties(directive, None),
    LocalConsensusDuties {
        autonomous_lane_view: Some(tag.view()),
        global_validator: None,
    },
    "successor-global observer status must not suppress independently frozen lane-author work"
);
""",
        "the lane-duty regression must retain lane-author work after successor-global roster removal",
    )
    require_sequence(
        "lane_production_duty_survives_successor_global_roster_removal",
        """
assert_eq!(
    local_consensus_duties(locked, None).autonomous_lane_view,
    None,
    "a global lock still suppresses fresh lane payload production"
);
""",
        "the lane-duty regression must prove that a global lock suppresses fresh lane work",
    )
    require_sequence(
        "lane_production_duty_survives_successor_global_roster_removal",
        """
assert_eq!(
    local_consensus_duties(decided, None).autonomous_lane_view,
    None,
    "a terminal global decision retires fresh lane payload production"
);
""",
        "the lane-duty regression must prove that a terminal decision retires fresh lane work",
    )
    require_sequence(
        "recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade",
        """
let recovered =
    super::super::v2::RecoveredLifecycleLocalProposalAttemptV1::for_test(tag, round, subject);
assert!(recovered.exactly_matches_directive(unlocked));
assert_eq!(
    LocalProposalState::from_recovered_lifecycle_attempt(true, unlocked).attempted,
    Some(LocalProposalOwner::from(unlocked))
);
assert!(
    LocalProposalState::from_recovered_lifecycle_attempt(false, unlocked)
        .attempted
        .is_none()
);
let mut setup = ProductionLifecyclePreActivationRunnerBorrowV1::for_test();
assert!(setup.bind_recovered_local_proposal(unlocked));
assert!(
    !setup.bind_recovered_local_proposal(unlocked),
    "a second bind must reject the already-owned runner state"
);
assert!(setup.already_attempted(unlocked));
let exact_lock = directive(Some(subject), None);
assert!(recovered.exactly_matches_directive(exact_lock));

let upgraded_lock = directive(Some(proposal_subject(b"upgraded replay lock")), None);
assert!(
    recovered.exactly_matches_directive(upgraded_lock),
    "a same-view lock upgrade cannot reopen that view's one proposal slot"
);
let mismatched_round = super::super::v2::RecoveredLifecycleLocalProposalAttemptV1::for_test(
    tag,
    wire::ConsensusRound { view: 2, ..round },
    subject,
);
assert!(!mismatched_round.exactly_matches_directive(unlocked));

let decided = directive(Some(subject), Some(subject));
assert!(!recovered.exactly_matches_directive(decided));
""",
        "the recovered-attempt regression must prove affine same-view suppression across a lock upgrade while rejecting foreign rounds and decisions",
    )
    require_sequence(
        "prepared_local_proposal_state_is_affine_and_context_directive_bound",
        """
let prepared = ProductionLifecyclePreparedLocalProposalStateV1 {
    runner,
    context_id,
    directive,
};
assert!(prepared.exactly_matches(context_id, directive));

let foreign_context = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
    b"foreign prepared local Proposal lifecycle context",
)));
assert!(!prepared.exactly_matches(foreign_context, directive));
let foreign_directive = super::super::v2::LocalProposalDirective::for_test(
    crate::sumeragi::v2_core::EventTag::new(1, 4, crate::sumeragi::v2_core::Generation::new(5)),
    0,
    None,
    None,
    None,
);
assert!(!prepared.exactly_matches(context_id, foreign_directive));
""",
        "the affine prepared-state regression must reject foreign context and directive receipts",
    )
    return errors
_LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256 = {
    "on_resume_after_replay": "5110f5c231d638f1b0ec7964f2fab3b91470cfeeae5a59ec320e45a478ca64cd",
    "durable_proposal_is_active": "dd2dd74ab7257442e5ce7017be1f4579d8716a162960a043763180a0f1016525",
    "proposal_is_safe_for_durable_lock": "612b97156d1c4a80df60024efa73404adc5df4265464b116be5c8b11b9247619",
    "safe_to_prepare": "9a6ac51a6cfcd6ffb4a7327372330e5754c5451fbd0a2a2cfb3218a42475c3df",
    "wal_apply": "ace7adf6ef605c6a1e37fb087522cc91e8bb66b55b62f45a22a44b101571c3f8",
    "wal_apply_in_place": "7473b0680ec743e30070bc5dcb5ca9d1c7934199852c861fb5e0e9796e7ab709",
    "local_proposal_directive": "ae8489fea82bd72963f4343745cd838bdc30be67e8f95e0a5e4c1b76a003796e",
    "local_proposal_directive_for_test": "9bb8bae3eeab780523915b4a196526ae4c45cba8f27336c093bbaa5420ae1709",
    "recovered_local_proposal_attempt_from_durable_round": "c7b029d9129f9dd338b0eac4bb9467b4424961a9dc925e9b77ef1e62fee1af5a",
    "recovered_local_proposal_attempt_from_control": "cd9907f75daf6fe867390c45b8a0809da7815906a879535e35eb7b172cf27a82",
    "recovered_local_proposal_attempt_exactly_matches_directive": "2a6d517103a2b2bb466799630170efc0305905af96a06ea582117a539ed9ba7b",
    "recovered_local_proposal_attempt_for_test": "a1e981b01c6ad83028ffc8f4b5bb6575f0c56058716d54a3239d04c6c93d8f02",
    "bind_recovered_local_proposal": "890ae2dc854b552ca4ca5b4e81ce08524379c632dc63edddee296e485e4ee228",
    "from_recovered_lifecycle_attempt": "d9d0aa23a83d8990f4db9abd9f2298b019453c3a4660c9c6c5a9baa69ec45549",
    "initialize_recovered_local_proposal": "8295ba9249430d0f8f6c3ecc65b11b48873adc3b9f3408c313f1425f2a68ff01",
    "prepared_local_proposal_exactly_matches": "666d62c1d4104e6f0e07548434bd2d789514d23531e598032b743ed66e2afc5a",
    "activate_with_prepared_local_proposal": "dbfce16c82f866d3f833f0d04c490851d72ce48fad28e178e16ef059355ddadc",
    "run_non_pending_lifecycle_loop": "cf1175fb9703fb722dea4460957dd63847e4a6ec98b5394c66823f8aed0576cf",
    "run_lifecycle_active_height": "8a93a2ae69c84618d44942bcbf9013c47d219630e56d8e5288af9336354d29b2",
    "recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade": "e82974c532219438b4b7af86dbd17c2c3ffeea7631b591b28708c5c3c66da452",
    "prepared_local_proposal_state_is_affine_and_context_directive_bound": "e2e8af92151f3b187cdb8eca6f40bb67a5472060ed6fdc79b5fea0127d02c3b3",
    "schedule_local_proposal": "f83b871c9f775d84674d6b536f7ff41396a563583bbd1906bcb33b8446c21c43",
    "locked_body_recovery_plan": "9f3f04e35b943a2bc09756833f08a782c050cccdd6c59aa2997eb1e9f0c1cf7b",
    "local_consensus_duties": "32480f07ba6f9eed6bbdfad70fc53c07e9e6d53c79cf7f0a80ff68ced7621c8e",
    "locked_body_recovery_is_independent_of_reproposal_gates": "e25524bcbcb9fba0308bdec85d063850b46285a00445ce49cedcca399a0ec0ec",
    "lane_production_duty_survives_successor_global_roster_removal": "eaa18cc0026251f66f3a77a916d1b84825fe5b6453443eef8b482e544272a9b6",
    "can_schedule_local_proposal": "d8e65dc370921393e55ef931f3513650c13f0ef60881bef50368c8e8c6aac919",
    "submit_exact_body": "bd38de84a86fd4769bf7784324b1ff94c875072d2cab99325897d1390962128e",
    "encode_exact_local_body": "34de57c479e25668c7e77efa06fe00df53d3602157a32fd188984565d6091a22",
    "submit_encoded_body": "78a7d2e2d5cb1e67cfa502ee54e6d5051b1ed0e7b21b24a451742a88e820a39b",
    "validate_request": "b341fab7be9687fd5db733999adab1a4c78cb7cdddda66e9b6bf9ca6424e6ce3",
}
