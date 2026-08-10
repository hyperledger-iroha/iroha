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
                    "run_inner",
                    (),
                    ("#[allow(clippy::too_many_lines)]",),
                    "run_inner",
                    "startup replay-owner handoff",
                ),
                (
                    "from_replayed_proposal",
                    (("impl", "LocalProposalState"),),
                    (),
                    "from_replayed_proposal",
                    "exact replayed-proposal lock-owner authorization kernel",
                ),
                (
                    "replayed_proposal_sign",
                    (),
                    (),
                    "replayed_proposal_sign",
                    "replayed proposal tag/round/subject projection",
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
                (
                    "replayed_proposal_sign_reserves_only_the_exact_current_lock_owner",
                    (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
                    ("#[test]",),
                    "replayed_proposal_sign_reserves_only_the_exact_current_lock_owner",
                    "exact replayed-proposal lock-owner regression",
                ),
                (
                    "locked_body_recovery_is_independent_of_reproposal_gates",
                    (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
                    ("#[test]",),
                    "locked_body_recovery_is_independent_of_reproposal_gates",
                    "locked-body acquisition/reproposal split regression",
                ),
                (
                    "lane_production_duty_survives_successor_global_roster_removal",
                    (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
                    ("#[test]",),
                    "lane_production_duty_survives_successor_global_roster_removal",
                    "successor-roster-independent lane duty regression",
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
            if digest_key != "run_inner":
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
        "run_inner",
        """
let replayed_proposal = replayed_proposal_sign(&startup_effects);
""",
        "startup must retain the replayed proposal tag, round, and subject before consuming effects",
    )
    require_sequence(
        "run_inner",
        """
let mut local_proposal_state =
    LocalProposalState::from_replayed_proposal(replayed_proposal, initial_directive);
""",
        "startup must bind the retained replay identity to the exact current proposal owner",
    )
    require_order(
        "run_inner",
        (
            "let replayed_proposal = replayed_proposal_sign(&startup_effects);",
            "executor.consume_effects(std::mem::take(&mut startup_effects), &mut services)?;",
            "let initial_directive = reconcile_executor_locked_body(&mut executor, &mut services)?;",
            "LocalProposalState::from_replayed_proposal(replayed_proposal, initial_directive)",
        ),
        "startup replay-owner handoff",
    )
    require_sequence(
        "run_inner",
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
        "from_replayed_proposal",
        """
replayed.tag == owner.tag
    && replayed.round.height == replayed.tag.height()
    && replayed.round.view == replayed.tag.view()
    && owner.decided_subject.is_none()
    && owner
        .locked_body
        .is_none_or(|(locked_round, locked_subject)| {
            replayed.round.context_id == locked_round.context_id
                && replayed.round.height == locked_round.height
                && replayed.subject == locked_subject
        })
""",
        "runner replay authorization must bind tag, round, context, height, and locked subject",
    )
    require_sequence(
        "replayed_proposal_sign",
        """
AdapterEffect::Sign {
    tag,
    request: SignRequest::Proposal(proposal),
} => Some(ReplayedProposalSign {
    tag: *tag,
    round: proposal.round,
    subject: proposal.subject,
}),
""",
        "replayed proposal extraction must preserve its exact tag, round, and subject",
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
        "replayed_proposal_sign_reserves_only_the_exact_current_lock_owner",
        """
let foreign_lock = directive(Some(proposal_subject(b"foreign replay lock")), None);
assert!(
    LocalProposalState::from_replayed_proposal(Some(replayed), foreign_lock)
        .attempted
        .is_none(),
    "an equal-tag proposal for another subject cannot reserve the current lock owner"
);
let mismatched_round = ReplayedProposalSign {
    round: wire::ConsensusRound { view: 2, ..round },
    ..replayed
};
assert!(
    LocalProposalState::from_replayed_proposal(Some(mismatched_round), unlocked)
        .attempted
        .is_none(),
    "the replayed proposal round must match its reducer tag"
);
let decided = directive(Some(subject), Some(subject));
assert!(
    LocalProposalState::from_replayed_proposal(Some(replayed), decided)
        .attempted
        .is_none(),
    "a decision retires every replayed proposal reservation"
);
""",
        "the replay-owner regression must reject foreign subjects, mismatched rounds, and decided lifecycles",
    )
    return errors
