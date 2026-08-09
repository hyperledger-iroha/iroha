def _reply_writer_deadline_formal_source_fidelity_errors(
    formal_dir: Path, repo_root: Path = ROOT_DIR
) -> list[str]:
    """Pin the orthogonal deadline proof, its residual, and all TLC mutants."""

    errors: list[str] = []
    errors.extend(
        _dormant_reply_clock_mutation_contract_errors(
            formal_dir, repo_root
        )
    )
    sources: dict[str, str] = {}
    for name, expected_sha256 in (
        _REPLY_WRITER_DEADLINE_FORMAL_SOURCE_SHA256.items()
    ):
        path = formal_dir / name
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: reply-writer deadline formal source must be a regular file"
            )
            continue
        payload = path.read_bytes()
        observed_sha256 = hashlib.sha256(payload).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: reply-writer deadline formal source must match exact "
                f"reviewed SHA-256 {expected_sha256}; found {observed_sha256}"
            )
        if path.suffix == ".tla":
            sources[name] = payload.decode("utf-8")

    def require_operator_fragments(
        source_name: str,
        symbol: str,
        required: tuple[str, ...],
        *,
        forbidden: tuple[str, ...] = (),
    ) -> None:
        source = sources.get(source_name)
        if source is None:
            return
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        path = formal_dir / source_name
        if extracted is None:
            errors.append(
                f"{path}: missing reviewed reply-writer deadline operator {symbol}"
            )
            return
        body, line = extracted
        normalized = " ".join(body.split())
        for fragment in required:
            if " ".join(fragment.split()) not in normalized:
                errors.append(
                    f"{path}:{line}: {symbol} must retain reply-writer deadline "
                    f"contract fragment {fragment!r}"
                )
        for fragment in forbidden:
            if " ".join(fragment.split()) in normalized:
                errors.append(
                    f"{path}:{line}: {symbol} may not contain reply-writer "
                    f"deadline shortcut {fragment!r}"
                )

    def require_theorem_fragments(
        source_name: str,
        symbol: str,
        required: tuple[str, ...],
    ) -> None:
        source = sources.get(source_name)
        if source is None:
            return
        extracted = _top_level_theorem_body(
            source, symbol, preserve_string_contents=True
        )
        path = formal_dir / source_name
        if extracted is None:
            errors.append(
                f"{path}: missing reviewed reply-writer deadline theorem {symbol}"
            )
            return
        body, line = extracted
        normalized = " ".join(body.split())
        for fragment in required:
            if " ".join(fragment.split()) not in normalized:
                errors.append(
                    f"{path}:{line}: {symbol} must retain deductive "
                    f"reply-writer deadline fragment {fragment!r}"
                )

    model = "SumeragiV2ReplyWriterDeadline.tla"
    require_operator_fragments(
        model,
        "FirstExactActorDispatch",
        (
            "ExactUndispatched",
            "dispatchStarted' = TRUE",
            "deadlineSet' = TRUE",
            "deadlineBudget' = ScaledDeadline(timeoutAttempt)",
            "deadlineOrigin' = 0",
        ),
    )
    require_operator_fragments(
        model,
        "RetryFullPeerWriterQueue",
        (
            "dispatchRetries' = dispatchRetries + 1",
            "UNCHANGED <<kind, phase, outcome, cursor, timeoutAttempt, "
            "timedOutCount, dispatchStarted, deadlineSet, deadlineBudget, "
            "deadlineOrigin, deadlineDue",
        ),
    )
    require_operator_fragments(
        model,
        "ExactFlushReady",
        (
            "ExactActive",
            "phase = \"WriterPending\"",
            "ackReady",
            "ackTimeoutAttempt = timeoutAttempt",
            "writerFlushObserved",
        ),
    )
    require_operator_fragments(
        model,
        "PublishPeerWriterFlush",
        (
            "ExactActive",
            "phase = \"WriterPending\"",
            "~ackReady",
            "ackReady' = TRUE",
            "ackTimeoutAttempt' = timeoutAttempt",
            "writerFlushObserved' = TRUE",
        ),
    )
    require_operator_fragments(
        model,
        "PollPeerWriterFlush",
        (
            "ExactFlushReady",
            "outcome' = \"Flushed\"",
            "cursor' = 1",
            "ackReady' = FALSE",
            "ackTimeoutAttempt' = 0",
            "ackPublished' = TRUE",
            "UNCHANGED <<kind, dispatchStarted, dispatchRetries, "
            "writerFlushObserved",
        ),
    )
    require_operator_fragments(
        model,
        "ExactDeadlineDue",
        (
            "ExactActive",
            "deadlineDue",
            "~ackReady",
            "~writerFlushObserved",
        ),
    )
    require_operator_fragments(
        model,
        "ExpireExactDeadline",
        (
            "ExactDeadlineDue",
            "outcome' = \"TimedOut\"",
            "timeoutAttempt' = SaturatingIncrement(timeoutAttempt)",
            "ackPublished' = FALSE",
            "UNCHANGED <<kind, cursor, dispatchStarted, dispatchRetries, "
            "ackReady, writerFlushObserved",
            "IF currentConnection = occurrenceConnection "
            "THEN \"NoConnection\" ELSE currentConnection",
        ),
    )
    require_operator_fragments(
        model,
        "FlushAttemptIdentityInvariant",
        (
            "ackReady => ackTimeoutAttempt = timeoutAttempt",
            "~ackReady => ackTimeoutAttempt = 0",
        ),
    )
    require_operator_fragments(
        model,
        "WriterFlushAttemptIdentityAction",
        (
            "PublishPeerWriterFlush =>",
            "ackTimeoutAttempt' = timeoutAttempt",
        ),
    )
    require_operator_fragments(
        model,
        "FlushOutcomeInvariant",
        (
            "ackReady =>",
            "writerFlushObserved => kind = \"ExactReply\"",
            "kind = \"ExactReply\" => "
            "(writerFlushObserved <=> (ackReady \\/ ackPublished))",
            "ackPublished <=>",
            "outcome = \"Flushed\" => ackPublished",
        ),
    )
    require_operator_fragments(
        model,
        "TimeoutIsNotFlushAction",
        (
            "ExpireExactDeadline =>",
            "outcome' = \"TimedOut\"",
            "cursor' = cursor",
            "~writerFlushObserved'",
            "~ackPublished'",
        ),
    )
    require_operator_fragments(
        model,
        "RetireOldExactRoute",
        (
            "ExactActive",
            "dispatchStarted",
            "~ackReady",
            "occurrenceConnection = \"OldConnection\"",
            "outcome' = \"Closed\"",
        ),
    )
    require_operator_fragments(
        model,
        "ReadyFlushRetirementExclusionAction",
        (
            "TerminalFenceReadyWinsEveryDestructiveExitAction",
        ),
    )
    require_operator_fragments(
        model,
        "TerminalFenceReadyWinsEveryDestructiveExitAction",
        (
            "ExactFlushReady =>",
            "~ExpireExactDeadline",
            "~ClosePeerWriter",
            "~RetireOldExactRoute",
        ),
    )
    require_operator_fragments(
        model,
        "ReadyFlushSurvivesReplacementAction",
        (
            "ExactFlushReady",
            "InstallReplacementBeforeTerminal",
            "=> ExactFlushReady'",
        ),
    )
    require_operator_fragments(
        model,
        "WriterFlushObservationOriginAction",
        (
            "Next =>",
            "~writerFlushObserved",
            "writerFlushObserved'",
            "=> PublishPeerWriterFlush",
        ),
    )
    require_operator_fragments(
        model,
        "WriterFlushObservationMonotonicAction",
        (
            "ReplyWriterDeadlineInvariant",
            "Next",
            "writerFlushObserved",
            "=> writerFlushObserved'",
        ),
    )
    require_operator_fragments(
        model,
        "ExactOccurrenceReplacementIsolationAction",
        (
            "protectedReplacement = \"ReplacementConnection\"",
            "ExpireExactDeadline",
            "currentConnection' = \"ReplacementConnection\"",
            "protectedReplacement' = \"ReplacementConnection\"",
        ),
    )
    require_operator_fragments(
        model,
        "TopologyNeverAcquiresDeadlineAction",
        (
            "ReplyWriterDeadlineInvariant",
            "kind = \"Topology\"",
            "Next",
            "=> ~deadlineSet'",
        ),
    )
    require_operator_fragments(
        model,
        "ReplyWriterDeadlineSpec",
        (
            "Init",
            "[][Next]_replyWriterDeadlineVars",
            "WF_replyWriterDeadlineVars(FirstExactActorDispatch)",
            "WF_replyWriterDeadlineVars(ReachExactDeadline)",
            "WF_replyWriterDeadlineVars(ExpireExactDeadline)",
            "WF_replyWriterDeadlineVars(PollPeerWriterFlush)",
        ),
        forbidden=(
            "WF_replyWriterDeadlineVars(PublishPeerWriterFlush)",
            "SF_replyWriterDeadlineVars(PublishPeerWriterFlush)",
            "SF_replyWriterDeadlineVars(AdmitPeerWriter)",
        ),
    )
    require_operator_fragments(
        model,
        "ResponsiveReplyWriterSpec",
        (
            "ReplyWriterDeadlineSpec",
            "WF_replyWriterDeadlineVars(ReconnectExactReply)",
            "SF_replyWriterDeadlineVars(AdmitPeerWriter)",
            "SF_replyWriterDeadlineVars(PublishPeerWriterFlush)",
        ),
    )

    proofs = "SumeragiV2ReplyWriterDeadlineProofs.tla"
    require_theorem_fragments(
        proofs,
        "PublishedWriterReceiptBindsTimeoutAttempt",
        (
            "PublishPeerWriterFlush =>",
            "ackTimeoutAttempt' = timeoutAttempt",
            "BY DEF PublishPeerWriterFlush",
        ),
    )
    require_theorem_fragments(
        proofs,
        "PollRequiresMatchingReceiptTimeoutAttempt",
        (
            "PollPeerWriterFlush =>",
            "ackTimeoutAttempt = timeoutAttempt",
            "BY DEF PollPeerWriterFlush, ExactFlushReady",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ReadyReceiptDisablesRouteRetirement",
        (
            "ExactFlushReady =>",
            "~ExpireExactDeadline",
            "~ClosePeerWriter",
            "~RetireOldExactRoute",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ReadyReceiptSurvivesConnectionReplacement",
        (
            "ExactFlushReady",
            "InstallReplacementBeforeTerminal",
            "=> ExactFlushReady'",
        ),
    )
    require_theorem_fragments(
        proofs,
        "TerminalFenceReadyReceiptWinsEveryDestructiveExit",
        (
            "TerminalFenceReadyWinsEveryDestructiveExitAction",
            "BY ReadyReceiptDisablesRouteRetirement",
            "DEF TerminalFenceReadyWinsEveryDestructiveExitAction",
        ),
    )
    require_theorem_fragments(
        proofs,
        "WriterFlushObservationComesOnlyFromPublish",
        (
            "WriterFlushObservationOriginAction",
            "BY SMTT(30)",
            "DEF WriterFlushObservationOriginAction, Next",
            "PublishPeerWriterFlush",
            "ExpireExactDeadline",
            "ReconnectExactReply",
        ),
    )
    require_theorem_fragments(
        proofs,
        "WriterFlushObservationIsNeverErased",
        (
            "WriterFlushObservationMonotonicAction",
            "BY SMTT(30)",
            "DEF WriterFlushObservationMonotonicAction, Next",
            "PublishPeerWriterFlush",
            "ExpireExactDeadline",
            "ReconnectExactReply",
        ),
    )
    require_theorem_fragments(
        proofs,
        "TimeoutPublishesDistinctTerminalOutcome",
        (
            "ExpireExactDeadline =>",
            "outcome' = \"TimedOut\"",
            "cursor' = cursor",
            "~writerFlushObserved'",
            "~ackPublished'",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ReachableTopologyNeverAcquiresExactDeadline",
        (
            "ReplyWriterDeadlineSpec =>",
            "[](kind = \"Topology\" =>",
            "~deadlineSet",
            "deadlineBudget = 0",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ConditionalResponsiveWriterCursorLiveness",
        (
            "ReplyWriterDeadlineSpec",
            "ResponsiveWriterReceiptAssumption",
            "=> ResponsiveReplyWriterCursorLiveness",
            "BY ReadyReceiptLeadsToCursorAdvance, PTL",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ReplyWriterDeadlineModelObligation",
        (
            "ReplyWriterDeadlineSpec",
            "[]ReplyWriterDeadlineInvariant",
            "[][ReplyWriterDeadlineActionSafety]_replyWriterDeadlineVars",
            "LocalActorTermination",
            "WriterFlushObservationComesOnlyFromPublish",
            "WriterFlushObservationIsNeverErased",
            "WriterFlushObservationOriginAction",
            "WriterFlushObservationMonotonicAction",
            "PublishedWriterReceiptBindsTimeoutAttempt",
            "PollRequiresMatchingReceiptTimeoutAttempt",
            "WriterFlushAttemptIdentityAction",
            "TerminalFenceReadyReceiptWinsEveryDestructiveExit",
            "TerminalFenceReadyWinsEveryDestructiveExitAction",
            "BY ReplyWriterDeadlineLocalActorTermination",
        ),
    )
    require_theorem_fragments(
        proofs,
        "OutstandingWithoutReceiptIsNotOrphaned",
        (
            "OutstandingWithoutReceipt",
            "[Next]_replyWriterDeadlineVars",
            "=> OutstandingWithoutReceipt' \\/ writerFlushObserved'",
            "BY ReplyWriterDeadlineBracketPreservesInvariant",
        ),
    )
    require_theorem_fragments(
        proofs,
        "OutstandingWithoutReceiptLeadsToAdmissionOrReceipt",
        (
            "ResponsiveReplyWriterSpec =>",
            "OutstandingWithoutReceipt",
            "~> (writerFlushObserved \\/ ExactAdmissionWindow)",
            "ExactParkedTerminalLeadsToUndispatched",
            "UndispatchedLeadsToAdmissionWindow",
        ),
    )
    require_theorem_fragments(
        proofs,
        "AdmissionWindowEnablesAdmission",
        (
            "ExactAdmissionWindow =>",
            "ENABLED <<AdmitPeerWriter>>_replyWriterDeadlineVars",
        ),
    )
    require_theorem_fragments(
        proofs,
        "AdmissionCreatesPublicationWindow",
        (
            "ExactAdmissionWindow",
            "<<AdmitPeerWriter>>_replyWriterDeadlineVars",
            "=> ExactPublicationWindow'",
        ),
    )
    require_theorem_fragments(
        proofs,
        "PublicationWindowEnablesPublish",
        (
            "ExactPublicationWindow =>",
            "ENABLED <<PublishPeerWriterFlush>>_replyWriterDeadlineVars",
        ),
    )
    require_theorem_fragments(
        proofs,
        "PublishCreatesReceiptObservation",
        (
            "ExactPublicationWindow",
            "<<PublishPeerWriterFlush>>_replyWriterDeadlineVars",
            "=> writerFlushObserved'",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ResponsiveStrongFairnessToReceiptObservation",
        (
            "ResponsiveReplyWriterSpec =>",
            "ExactOutstanding ~> writerFlushObserved",
            "OutstandingWithoutReceiptLeadsToAdmissionOrReceipt",
            "AdmissionWindowEnablesAdmission",
            "AdmissionCreatesPublicationWindow",
            "PublicationWindowEnablesPublish",
            "PublishCreatesReceiptObservation",
        ),
    )
    require_theorem_fragments(
        proofs,
        "OutstandingReceiptObservationIsReady",
        (
            "ReplyWriterDeadlineInvariant",
            "ExactOutstanding",
            "writerFlushObserved",
            "=> ExactFlushReady",
            "FlushAttemptIdentityInvariant",
            "FlushOutcomeInvariant",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ResponsiveStrongFairnessToReceiptResidual",
        (
            "ResponsiveReplyWriterSpec => ResponsiveWriterReceiptAssumption",
            "BY ResponsiveStrongFairnessToReceiptObservation",
            "ReplyWriterDeadlineSpecAlwaysInvariant",
            "OutstandingReceiptObservationIsReady, PTL",
        ),
    )
    require_theorem_fragments(
        proofs,
        "ResponsiveReplyWriterCursorLivenessFromStrongFairness",
        (
            "ResponsiveReplyWriterSpec => ResponsiveReplyWriterCursorLiveness",
            "BY ResponsiveStrongFairnessToReceiptResidual",
            "ConditionalResponsiveWriterCursorLiveness, PTL",
        ),
    )

    mutation = "SumeragiV2ReplyWriterDeadlineMutation.tla"
    require_operator_fragments(
        mutation,
        "MutationModes",
        (
            "\"Fixed\"",
            "\"ResetDeadlineOnQueueRetry\"",
            "\"TimeoutAsFlushed\"",
            "\"IncrementAttemptOnClosed\"",
            "\"ResetAttemptOnReconnect\"",
            "\"UncappedAdaptiveDeadline\"",
            "\"TopologyAcquiresDeadline\"",
            "\"TerminateReplacementConnection\"",
            "\"TimeoutBeatsReadyFlush\"",
            "\"PublishWrongTimeoutAttempt\"",
            "\"CloseReadyFlushWithoutTerminalFence\"",
            "\"RetireReadyFlush\"",
            "\"EraseReadyFlushWitness\"",
        ),
    )
    require_operator_fragments(
        mutation,
        "PublishWrongTimeoutAttempt",
        (
            "ExactActive",
            "phase = \"WriterPending\"",
            "timeoutAttempt > 0",
            "~ackReady",
            "ackReady' = TRUE",
            "ackTimeoutAttempt' = timeoutAttempt - 1",
            "writerFlushObserved' = TRUE",
        ),
    )
    require_operator_fragments(
        mutation,
        "MutationPublish",
        (
            "MutationMode = \"PublishWrongTimeoutAttempt\"",
            "THEN PublishWrongTimeoutAttempt",
            "ELSE PublishPeerWriterFlush",
        ),
    )
    require_operator_fragments(
        mutation,
        "TimeoutAsFlushed",
        (
            "ExactDeadlineDue",
            "outcome' = \"Flushed\"",
            "cursor' = 1",
            "ackPublished' = TRUE",
            "UNCHANGED <<kind, timeoutAttempt, timedOutCount, dispatchStarted, "
            "dispatchRetries, ackReady, writerFlushObserved",
        ),
    )
    require_operator_fragments(
        mutation,
        "TimeoutBeatsReadyFlush",
        (
            "ExactActive",
            "deadlineDue",
            "ackReady",
            "outcome' = \"TimedOut\"",
            "ackReady' = FALSE",
            "UNCHANGED <<kind, cursor, dispatchStarted, dispatchRetries, "
            "writerFlushObserved",
        ),
    )
    require_operator_fragments(
        mutation,
        "CloseReadyFlushWithoutTerminalFence",
        (
            "ExactFlushReady",
            "outcome' = \"Closed\"",
            "routeWritable' = FALSE",
            "ackReady' = FALSE",
            "UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount, "
            "dispatchStarted, dispatchRetries, writerFlushObserved",
        ),
    )
    require_operator_fragments(
        mutation,
        "EraseReadyFlushWitness",
        (
            "ExactFlushReady",
            "outcome' = \"Closed\"",
            "routeWritable' = FALSE",
            "ackReady' = FALSE",
            "writerFlushObserved' = FALSE",
            "UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount, "
            "dispatchStarted, dispatchRetries, ackPublished",
        ),
    )
    require_operator_fragments(
        mutation,
        "MutationWriterFlushObservationMonotonicAction",
        (
            "MutationNext",
            "writerFlushObserved",
            "=> writerFlushObserved'",
        ),
    )
    require_operator_fragments(
        mutation,
        "MutationWriterFlushObservationMonotonicity",
        (
            "[][MutationWriterFlushObservationMonotonicAction]"
            "_replyWriterDeadlineVars",
        ),
    )
    require_operator_fragments(
        mutation,
        "RetireReadyFlush",
        (
            "ExactFlushReady",
            "outcome' = \"Closed\"",
            "routeWritable' = FALSE",
            "ackReady' = FALSE",
            "UNCHANGED <<kind, cursor, timeoutAttempt, timedOutCount, "
            "dispatchStarted, dispatchRetries, writerFlushObserved",
        ),
    )
    for symbol in (
        "ResetDeadlineOnQueueRetry",
        "TimeoutAsFlushed",
        "IncrementAttemptOnClosed",
        "ResetAttemptOnReconnect",
        "UncappedAdaptiveDeadline",
        "TopologyAcquiresDeadline",
        "TerminateReplacementConnection",
        "TimeoutBeatsReadyFlush",
        "PublishWrongTimeoutAttempt",
        "CloseReadyFlushWithoutTerminalFence",
        "RetireReadyFlush",
        "EraseReadyFlushWitness",
    ):
        if _top_level_operator_body(
            sources.get(mutation, ""), symbol, preserve_string_contents=True
        ) is None:
            errors.append(
                f"{formal_dir / mutation}: missing independent reply-writer "
                f"deadline mutant {symbol}"
            )

    runner_path = (
        repo_root
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_reply_writer_deadline_mutations.sh"
    )
    if not runner_path.is_file() or runner_path.is_symlink():
        errors.append(
            f"{runner_path}: reply-writer deadline mutation runner must be a regular file"
        )
    else:
        payload = runner_path.read_bytes()
        observed_sha256 = hashlib.sha256(payload).hexdigest()
        if observed_sha256 != _REPLY_WRITER_DEADLINE_MUTATION_RUNNER_SHA256:
            errors.append(
                f"{runner_path}: reply-writer deadline mutation runner must "
                "match exact reviewed SHA-256 "
                f"{_REPLY_WRITER_DEADLINE_MUTATION_RUNNER_SHA256}; "
                f"found {observed_sha256}"
            )
        runner = re.sub(
            r"[ \t]*\\\r?\n[ \t]*", " ", payload.decode("utf-8")
        )
        if runner.count("SumeragiV2ReplyWriterDeadlineProofs; do") != 1:
            errors.append(
                f"{runner_path}: reply-writer deadline runner must SANY-parse "
                "the proof module exactly once"
            )
        if runner.count('"-DTLA-Library=${TLAPM_STDLIB}"') != 3:
            errors.append(
                f"{runner_path}: both SANY calls and TLC must use the pinned "
                "TLAPS standard library"
            )
        fixed_cases = (
            (
                "deadline-local-termination-fixed",
                "SumeragiV2ReplyWriterDeadline.tla",
                "reply_writer_deadline_fixed.cfg",
                "331 states generated, 164 distinct states found, "
                "0 states left on queue.",
                "The depth of the complete state graph search is 20.",
            ),
            (
                "deadline-responsive-writer-fixed",
                "SumeragiV2ReplyWriterDeadline.tla",
                "reply_writer_deadline_responsive_fixed.cfg",
                "331 states generated, 164 distinct states found, "
                "0 states left on queue.",
                "The depth of the complete state graph search is 20.",
            ),
            (
                "deadline-mutation-fixed",
                "SumeragiV2ReplyWriterDeadlineMutation.tla",
                "reply_writer_deadline_mutation_fixed.cfg",
                "331 states generated, 164 distinct states found, "
                "0 states left on queue.",
                "The depth of the complete state graph search is 20.",
            ),
        )
        for label, module, config, states, depth in fixed_cases:
            pattern = (
                rf"\brun_case\s+{re.escape(label)}\s+"
                rf"{re.escape(module)}\s+{re.escape(config)}\s+0\s+"
                r'"Model checking completed\. No error has been found\."\s+'
                rf'"{re.escape(states)}"\s+"{re.escape(depth)}"'
            )
            if len(re.findall(pattern, runner)) != 1:
                errors.append(
                    f"{runner_path}: fixed reply-writer deadline runner must "
                    f"invoke {label} exactly once with TLC status 0 and its "
                    "exact generated/distinct/depth markers"
                )
        mutation_cases = (
            (
                "queue-retry-reset",
                "reply_writer_deadline_retry_reset_bug.cfg",
                12,
                "7 states generated, 7 distinct states found, "
                "2 states left on queue.",
                "The depth of the complete state graph search is 4.",
            ),
            (
                "timeout-as-flush",
                "reply_writer_deadline_timeout_as_flush_bug.cfg",
                12,
                "27 states generated, 22 distinct states found, "
                "12 states left on queue.",
                "The depth of the complete state graph search is 5.",
            ),
            (
                "closed-attempt-growth",
                "reply_writer_deadline_closed_attempt_bug.cfg",
                12,
                "22 states generated, 21 distinct states found, "
                "12 states left on queue.",
                "The depth of the complete state graph search is 5.",
            ),
            (
                "reconnect-attempt-reset",
                "reply_writer_deadline_reconnect_reset_bug.cfg",
                12,
                "67 states generated, 42 distinct states found, "
                "19 states left on queue.",
                "The depth of the complete state graph search is 6.",
            ),
            (
                "uncapped-attempt",
                "reply_writer_deadline_uncapped_attempt_bug.cfg",
                12,
                "225 states generated, 116 distinct states found, "
                "11 states left on queue.",
                "The depth of the complete state graph search is 11.",
            ),
            (
                "topology-deadline",
                "reply_writer_deadline_topology_deadline_bug.cfg",
                12,
                "6 states generated, 6 distinct states found, "
                "2 states left on queue.",
                "The depth of the complete state graph search is 3.",
            ),
            (
                "replacement-termination",
                "reply_writer_deadline_replacement_kill_bug.cfg",
                12,
                "66 states generated, 40 distinct states found, "
                "17 states left on queue.",
                "The depth of the complete state graph search is 6.",
            ),
            (
                "timeout-beats-ready-flush",
                "reply_writer_deadline_timeout_beats_flush_bug.cfg",
                12,
                "98 states generated, 52 distinct states found, "
                "13 states left on queue.",
                "The depth of the complete state graph search is 7.",
            ),
            (
                "wrong-timeout-attempt-flush",
                "reply_writer_deadline_wrong_attempt_flush_bug.cfg",
                12,
                "136 states generated, 66 distinct states found, "
                "10 states left on queue.",
                "The depth of the complete state graph search is 9.",
            ),
            (
                "inactive-close-beats-ready-flush",
                "reply_writer_deadline_close_ready_flush_bug.cfg",
                12,
                "55 states generated, 39 distinct states found, "
                "19 states left on queue.",
                "The depth of the complete state graph search is 6.",
            ),
            (
                "retirement-beats-ready-flush",
                "reply_writer_deadline_retire_ready_flush_bug.cfg",
                12,
                "46 states generated, 35 distinct states found, "
                "17 states left on queue.",
                "The depth of the complete state graph search is 6.",
            ),
            (
                "erase-ready-flush-witness",
                "reply_writer_deadline_erase_ready_witness_bug.cfg",
                13,
                "46 states generated, 34 distinct states found, "
                "17 states left on queue.",
                "The depth of the complete state graph search is 6.",
            ),
        )
        for label, config, status, states, depth in mutation_cases:
            case = f'"{label}|{config}|{status}|{states}|{depth}"'
            if runner.count(case) != 1:
                errors.append(
                    f"{runner_path}: reply-writer deadline runner must list "
                    f"{case} exactly once in the mutation matrix"
                )
        if runner.count(
            'readonly INVARIANT_MARKER="Error: Invariant '
            'ReplyWriterDeadlineInvariant is violated."'
        ) != 1:
            errors.append(
                f"{runner_path}: eleven state-invariant mutants must pin the "
                "exact TLC invariant-violation marker"
            )
        if runner.count(
            'readonly ACTION_MARKER="Error: Action property '
            'MutationWriterFlushObservationMonotonicity is violated."'
        ) != 1:
            errors.append(
                f"{runner_path}: witness-erasure mutant must pin the exact "
                "TLC monotonic action-property violation marker"
            )
        expected_loop = (
            "if [[ \"$expected_status\" -eq 12 ]]; then "
            "violation_marker=\"$INVARIANT_MARKER\" else "
            "violation_marker=\"$ACTION_MARKER\" fi "
            "run_case \"$label\" "
            "SumeragiV2ReplyWriterDeadlineMutation.tla \"$config\" "
            "\"$expected_status\" \"$violation_marker\" \"$state_marker\" "
            "\"$depth_marker\""
        )
        if " ".join(expected_loop.split()) not in " ".join(runner.split()):
            errors.append(
                f"{runner_path}: mutation loop must bind status 12 to the "
                "invariant marker, status 13 to the action marker, and freeze "
                "each case's exact state/depth signatures"
            )
    return errors
