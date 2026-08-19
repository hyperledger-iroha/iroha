# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.


def _promotion_target_invocation(
    contract: PromotionProofTargetContract, start_line: int, end_line: int
) -> list[str]:
    """Return the exact code-owned TLAPM argument vector for one target."""

    # Pinned tlapm_args.ml defines --toolbox START END as an inclusive locus
    # selection.  Pinned tlapm_lib.ml retains an obligation only when its whole
    # start/stop locus lies inside that interval; --strict exits 12 when an
    # explicit selection contains no obligations.
    return [
        "--toolbox",
        str(start_line),
        str(end_line),
        "--strict",
        "--nofp",
        "--threads",
        "1",
        "--cache-dir",
        _formal_evidence_logical_path(
            "tlaps-cache", "targets", contract.obligation_id
        ),
        f"{contract.provider_module}.tla",
    ]


ASYNC_STRONG_TYPE_INVARIANT_EXACT_BODY = (
    "/\\ StrongInductiveInvariant /\\ AsyncSchedulerTypeInvariant "
    "/\\ AsyncProducerTypeInvariant "
    "/\\ AsyncServeProducerTurnTypeInvariant "
    "/\\ AsyncServeProducerTurnOwnershipInvariant "
    "/\\ AsyncServiceActivationPairInvariant "
    "/\\ AsyncControlServiceStateTypeInvariant "
    "/\\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant "
    "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant "
    "/\\ AsyncCertifiedResponseClaimIngressOwnershipInvariant "
    "/\\ AsyncLeaderWireIngressCarrierOwnershipInvariant "
    "/\\ AsyncOrdinaryIngressCarrierOwnershipInvariant "
    "/\\ ReceivedTimeoutVotePoolInvariant "
    "/\\ AsyncRecoveryTypeInvariant /\\ AsyncRestartAuthorityInvariant "
    "/\\ AsyncRecoveryExecutionInvariant "
    "/\\ AsyncHistoricalLockRestartAuthorityTypeInvariant "
    "/\\ HistoricalLockRestartAuthoritySourceRetentionInvariant "
    "/\\ AsyncGstRecoveryPhaseInvariant "
    "/\\ AsyncSerializedBusyKernelInvariant"
)


def _async_extended_strong_type_induction_errors(
    path: Path, source: str
) -> list[str]:
    """Keep every producer/timeout conjunct in init, stutter, and Next induction."""

    errors: list[str] = []

    def normalized_theorem(symbol: str) -> tuple[str, int] | None:
        theorem = _top_level_theorem_body(
            source, symbol, preserve_string_contents=True
        )
        if theorem is None:
            return None
        body, line = theorem
        return " ".join(body.split()), line

    init = normalized_theorem("AsyncInitEstablishesStrongTypeInvariant")
    if init is not None:
        body, line = init
        bridges = (
            "<2>3f. AsyncProducerTypeInvariant BY <1>1, "
            "AsyncInitEstablishesProducerTypeInvariant",
            "<2>3p. /\\ AsyncServeProducerTurnTypeInvariant "
            "/\\ AsyncServeProducerTurnOwnershipInvariant BY <1>1, "
            "AsyncInitEstablishesServeProducerTurnInvariants",
            "<2>3t. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant "
            "BY <1>1, AsyncInitEstablishesTimeoutRecoveryCurrentBoundary",
        )
        if any(body.count(bridge) != 1 for bridge in bridges):
            errors.append(
                f"{path}:{line}: AsyncInitEstablishesStrongTypeInvariant must "
                "establish every producer-journal, one-shot Serve producer, "
                "and timeout-recovery boundary conjunct"
            )

    stutter = normalized_theorem(
        "AsyncAllVarsStutterPreservesStrongTypeInvariant"
    )
    if stutter is not None:
        body, line = stutter
        projection = (
            "/\\ AsyncProducerTypeInvariant "
            "/\\ AsyncServeProducerTurnTypeInvariant "
            "/\\ AsyncServeProducerTurnOwnershipInvariant "
            "/\\ AsyncServiceActivationPairInvariant "
            "/\\ AsyncControlServiceStateTypeInvariant "
            "/\\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant"
        )
        steps = (
            "<2>4e. AsyncProducerTypeInvariant' BY <1>1, <2>1, "
            "AsyncProducerVarsFramePreservesTypeInvariant DEF AsyncAllVars",
            "<2>8p. /\\ AsyncServeProducerTurnTypeInvariant' "
            "/\\ AsyncServeProducerTurnOwnershipInvariant' "
            "BY <1>1, <2>1, Isa DEF AsyncAllVars, AsyncSchedulerVars, "
            "AsyncServeProducerTurnTypeInvariant, "
            "AsyncServeProducerTurnOwnershipInvariant, "
            "AsyncServeIngressLifecycleOwnerIdentities, "
            "AsyncServeIngressAdmissionIdentities, "
            "AsyncServeOffQueueReservations, AsyncServeJobQueued, "
            "AsyncIoServeIdentities",
            "<2>8t. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant' "
            "BY <1>1, <2>1, Isa DEF AsyncAllVars, AsyncSchedulerVars, vars, "
            "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant, "
            "AsyncTimeoutRecoveryEpisodes, AsyncTimeoutRecoveryEpisodesIn, "
            "NodeHasDecision, AsyncNodeHasDecisionIn",
        )
        qed = (
            "<2> QED BY <2>3, <2>4, <2>4aa, <2>4a, <2>4b, <2>4bb, "
            "<2>4c, <2>4d, <2>4e, <2>5, <2>6, <2>7, <2>8, <2>8p, "
            "<2>8t DEF AsyncStrongTypeInvariant"
        )
        if body.count(projection) != 1:
            errors.append(
                f"{path}:{line}: "
                "AsyncAllVarsStutterPreservesStrongTypeInvariant must project "
                "every producer and timeout-boundary conjunct"
            )
        if any(body.count(step) != 1 for step in steps):
            errors.append(
                f"{path}:{line}: "
                "AsyncAllVarsStutterPreservesStrongTypeInvariant must prime "
                "every producer and timeout-boundary conjunct"
            )
        if body.count(qed) != 1:
            errors.append(
                f"{path}:{line}: "
                "AsyncAllVarsStutterPreservesStrongTypeInvariant must retain "
                "every new conjunct as an exact QED dependency"
            )

    next_step = normalized_theorem("AsyncNextPreservesStrongTypeInvariant")
    if next_step is not None:
        body, line = next_step
        projection = (
            "<2>2m. /\\ AsyncProducerTypeInvariant "
            "/\\ AsyncServeProducerTurnTypeInvariant "
            "/\\ AsyncServeProducerTurnOwnershipInvariant "
            "/\\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant "
            "BY <1>1 DEF AsyncStrongTypeInvariant"
        )
        producer_type = (
            "<2>4f. AsyncProducerTypeInvariant' BY <1>1, <2>2, "
            "AsyncProducerProjectionPreservesTypeInvariant DEF AsyncNext"
        )
        serve_type = (
            "<2>4d. /\\ AsyncServeProducerTurnTypeInvariant' "
            "/\\ AsyncServeProducerTurnOwnershipInvariant' BY <1>1, "
            "AsyncNextPreservesServeProducerTurnInvariants"
        )
        boundary = (
            "<2>4e. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant' "
            "BY <1>1, <2>2h, "
            "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant"
        )
        qed = (
            "<2> QED BY <2>2l, <2>2m, <2>3, <2>4, <2>4a, <2>4b, "
            "<2>4c, <2>4d, <2>4e, <2>4f, <2>5, <2>6, <2>7, <2>8, "
            "<2>9, <2>10, <2>11, <2>12, <2>13 "
            "DEF AsyncStrongTypeInvariant"
        )
        if body.count(projection) != 1:
            errors.append(
                f"{path}:{line}: AsyncNextPreservesStrongTypeInvariant must "
                "project every producer and timeout-recovery boundary conjunct "
                "before priming it"
            )
        if body.count(producer_type) != 1:
            errors.append(
                f"{path}:{line}: AsyncNextPreservesStrongTypeInvariant must "
                "prime AsyncProducerTypeInvariant through the exact producer "
                "projection transition"
            )
        if body.count(serve_type) != 1:
            errors.append(
                f"{path}:{line}: AsyncNextPreservesStrongTypeInvariant must "
                "prime AsyncServeProducerTurnTypeInvariant through the "
                "exact one-shot transition"
            )
        if body.count(serve_type) != 1:
            errors.append(
                f"{path}:{line}: AsyncNextPreservesStrongTypeInvariant must "
                "prime AsyncServeProducerTurnOwnershipInvariant through "
                "the reviewed admission, retirement, and reset cases"
            )
        if body.count(boundary) != 1:
            errors.append(
                f"{path}:{line}: AsyncNextPreservesStrongTypeInvariant must "
                "prime AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant "
                "through retained, created, and vote-updated episodes"
            )
        if body.count(qed) != 1:
            errors.append(
                f"{path}:{line}: AsyncNextPreservesStrongTypeInvariant must "
                "retain every producer and timeout-boundary prime as an "
                "exact QED dependency"
            )
    return errors


def _proof_obligation_architecture_errors(
    obligations: list[Any], module_sources: dict[str, str]
) -> list[str]:
    """Bind release obligations to the non-circular production proof layers."""

    errors: list[str] = []
    by_id = {
        obligation.get("id"): obligation
        for obligation in obligations
        if isinstance(obligation, dict) and _nonempty_string(obligation.get("id"))
    }

    for obligation_id, (module, symbol) in FIXED_PROOF_OBLIGATION_TARGETS.items():
        obligation = by_id.get(obligation_id)
        if obligation is None:
            errors.append(f"proof ledger is missing required obligation {obligation_id}")
            continue
        where = f"proof obligation {obligation_id}"
        if obligation.get("module") != module:
            errors.append(f"{where} must use {module}")
        if obligation.get("symbol") != symbol:
            errors.append(f"{where} must use the direct theorem {symbol}")

    for support_id, (module, symbols) in (
        SUPPORT_PROOF_OBLIGATION_INVENTORY.items()
    ):
        consumer_id = SUPPORT_PROOF_CONSUMER_BY_ID[support_id]
        consumer = by_id.get(consumer_id)
        if consumer is None:
            errors.append(
                f"proof support leaf {support_id} is missing reviewed consumer "
                f"{consumer_id}"
            )
        else:
            expected_module, expected_symbol = (
                REQUIRED_PROOF_OBLIGATION_INVENTORY[consumer_id]
            )
            if (
                consumer.get("module") != expected_module
                or consumer.get("symbol") != expected_symbol
            ):
                errors.append(
                    f"proof support leaf {support_id} must be transitively "
                    f"consumed by exact reviewed obligation {consumer_id}"
                )
        source = module_sources.get(module)
        if source is None:
            continue
        stripped = strip_tla_comments(source)
        for symbol in symbols.split(" / "):
            declaration = re.compile(
                rf"(?m)^[ \t]*(?:(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)"
                rf"[ \t]+)?{re.escape(symbol)}"
                rf"\s*(?:\([^)=\n]*\))?\s*=="
            )
            if declaration.search(stripped) is None:
                errors.append(
                    f"{module}.tla: proof support leaf {support_id} is missing "
                    f"reviewed declaration {symbol}"
                )

    def check_direct_theorem(
        obligation_id: str,
        symbol: str,
        *,
        module: str,
        required_spec: str,
        forbidden: tuple[str, ...],
        exact_statement: str | None = None,
    ) -> None:
        obligation = by_id.get(obligation_id)
        if obligation is None:
            errors.append(f"proof ledger is missing required obligation {obligation_id}")
            return
        where = f"proof obligation {obligation_id}"
        if obligation.get("module") != module:
            errors.append(f"{where} must use {module}")
        if obligation.get("symbol") != symbol:
            errors.append(f"{where} must use the direct theorem {symbol}")
        source = module_sources.get(module)
        if source is None:
            return
        declaration = re.compile(
            rf"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            rf"{re.escape(symbol)}\s*=="
        )
        if declaration.search(strip_tla_comments(source)) is None:
            errors.append(
                f"{where} must be one closed theorem universally quantifying initialContext"
            )
        extracted = _top_level_theorem_body(source, symbol)
        if extracted is None:
            return
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized_statement = " ".join(statement.split())
        statement_tokens = _exact_tla_call_statement_tokens(statement)
        exact_statement_tokens = (
            _exact_tla_call_statement_tokens(exact_statement)
            if exact_statement is not None
            else None
        )
        if exact_statement is not None and (
            exact_statement_tokens is None
            or statement_tokens != exact_statement_tokens
        ):
            errors.append(
                f"{module}.tla:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {normalized_statement!r}"
            )
        if re.search(
            rf"\b{re.escape(required_spec)}\s*\(\s*initialContext\s*\)", body
        ) is None:
            errors.append(
                f"{module}.tla:{line}: {symbol} must directly require "
                f"{required_spec}(initialContext)"
            )
        for retired in forbidden:
            if re.search(rf"\b{re.escape(retired)}\b", body):
                errors.append(
                    f"{module}.tla:{line}: {symbol} may not depend on "
                    f"legacy global-barrier operator {retired}"
                )

    def check_closed_theorem(
        obligation_id: str,
        symbol: str,
        *,
        module: str,
        exact_statement: str,
        forbidden: tuple[str, ...],
    ) -> None:
        if obligation_id in SUPPORT_PROOF_OBLIGATION_INVENTORY:
            consumer_id = SUPPORT_PROOF_CONSUMER_BY_ID[obligation_id]
            consumer = by_id.get(consumer_id)
            where = f"proof support leaf {obligation_id}"
            if consumer is None:
                errors.append(
                    f"{where} is missing reviewed consumer {consumer_id}"
                )
                return
        else:
            obligation = by_id.get(obligation_id)
            if obligation is None:
                errors.append(
                    f"proof ledger is missing required obligation {obligation_id}"
                )
                return
            where = f"proof obligation {obligation_id}"
            if obligation.get("module") != module:
                errors.append(f"{where} must use {module}")
            if obligation.get("symbol") != symbol:
                errors.append(f"{where} must use the direct theorem {symbol}")
        source = module_sources.get(module)
        if source is None:
            return
        declaration = re.compile(
            rf"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            rf"{re.escape(symbol)}\s*=="
        )
        if declaration.search(strip_tla_comments(source)) is None:
            errors.append(f"{where} must be one closed, unparameterized theorem")
        extracted = _top_level_theorem_body(source, symbol)
        if extracted is None:
            return
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized_statement = " ".join(statement.split())
        if normalized_statement != exact_statement:
            errors.append(
                f"{module}.tla:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {normalized_statement!r}"
            )
        for retired in forbidden:
            if re.search(rf"\b{re.escape(retired)}\b", body):
                errors.append(
                    f"{module}.tla:{line}: {symbol} may not depend on "
                    f"legacy global-barrier operator {retired}"
                )

    def check_exact_operator(
        module: str,
        symbol: str,
        exact_body: str,
    ) -> None:
        source = module_sources.get(module)
        if source is None:
            return
        extracted = _top_level_operator_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module}.tla: missing exact release property operator {symbol}"
            )
            return
        body, line = extracted
        normalized_body = " ".join(body.split())
        if normalized_body != exact_body:
            errors.append(
                f"{module}.tla:{line}: {symbol} must equal only "
                f"{exact_body!r}; found {normalized_body!r}"
            )

    def check_exact_supporting_theorem(
        module: str,
        symbol: str,
        exact_statement: str,
    ) -> None:
        source = module_sources.get(module)
        if source is None:
            return
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module}.tla: missing exact supporting theorem {symbol}"
            )
            return
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
            maxsplit=1,
        )[0]
        normalized_statement = " ".join(statement.split())
        if normalized_statement != exact_statement:
            errors.append(
                f"{module}.tla:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {normalized_statement!r}"
            )

    def check_theorem_proof_tokens(
        module: str,
        symbol: str,
        required_tokens: tuple[str, ...],
    ) -> None:
        source = module_sources.get(module)
        if source is None:
            return
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module}.tla: missing proof-dependency theorem {symbol}"
            )
            return
        body, line = extracted
        marker = re.search(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
        )
        proof = "" if marker is None else body[marker.start() :]
        missing = [
            token
            for token in required_tokens
            if not _tla_dependency_present(proof, token)
        ]
        if missing:
            errors.append(
                f"{module}.tla:{line}: {symbol} must retain reviewed proof "
                f"dependencies {missing!r}"
            )

    def check_theorem_forbidden_proof_tokens(
        module: str,
        symbol: str,
        forbidden_tokens: tuple[str, ...],
    ) -> None:
        source = module_sources.get(module)
        if source is None:
            return
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module}.tla: missing proof-boundary theorem {symbol}"
            )
            return
        body, line = extracted
        marker = re.search(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
        )
        proof = "" if marker is None else body[marker.start() :]
        present = [
            token
            for token in forbidden_tokens
            if _tla_dependency_present(proof, token)
        ]
        if present:
            errors.append(
                f"{module}.tla:{line}: {symbol} may not consume conditional "
                "proof bridges without their premises; "
                f"forbidden={present!r}"
            )

    def check_action_temporal_repair_contracts() -> None:
        """Pin reviewed action formulas without depending on source layout."""

        def compact_shape(fragment: str) -> str:
            # These reviewed surfaces contain no string literals.  Removing
            # whitespace makes the contract insensitive to indentation and
            # line wrapping while preserving every temporal/action token.
            return "".join(fragment.split())

        def require_temporal_property(
            module: str,
            symbol: str,
            expected_body: str,
        ) -> None:
            source = module_sources.get(module)
            if source is None:
                return
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed temporal action property "
                    f"{symbol}"
                )
                return
            body, line = extracted
            if compact_shape(body) != compact_shape(expected_body):
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the reviewed "
                    "temporal action shape"
                )

        def require_compact_theorem_statement(
            module: str,
            symbol: str,
            expected_statement: str,
        ) -> None:
            source = module_sources.get(module)
            if source is None:
                return
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed action-temporal theorem {symbol}"
                )
                return
            body, line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            if compact_shape(statement) != compact_shape(expected_statement):
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the exact "
                    "reviewed action-temporal statement"
                )

        def require_direct_proof_step(
            module: str,
            symbol: str,
            description: str,
            expected_step: str,
        ) -> None:
            source = module_sources.get(module)
            if source is None:
                return
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed action-temporal theorem {symbol}"
                )
                return
            body, line = extracted
            expected_shape = compact_shape(expected_step)
            observed_shapes: list[str] = []
            for label in re.finditer(r"(?m)^[ \t]*<\d+>\d+\.[ \t]*", body):
                tail = body[label.end() :]
                proof_marker = re.search(r"(?m)^[ \t]*BY\b", tail)
                if proof_marker is None:
                    continue
                observed_shapes.append(
                    compact_shape(tail[: proof_marker.start()])
                )
            if expected_shape not in observed_shapes:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the reviewed "
                    f"direct action proof step {description}"
                )

        def require_unique_direct_proof_step(
            module: str,
            symbol: str,
            description: str,
            expected_step: str,
        ) -> None:
            source = module_sources.get(module)
            if source is None:
                return
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed action-temporal theorem {symbol}"
                )
                return
            body, line = extracted
            expected_shape = compact_shape(expected_step)
            observed_shapes: list[str] = []
            for label in re.finditer(r"(?m)^[ \t]*<\d+>\d+\.[ \t]*", body):
                tail = body[label.end() :]
                proof_marker = re.search(r"(?m)^[ \t]*BY\b", tail)
                if proof_marker is None:
                    continue
                observed_shapes.append(
                    compact_shape(tail[: proof_marker.start()])
                )
            count = observed_shapes.count(expected_shape)
            if count != 1:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain exactly one "
                    f"reviewed direct proof step {description}; found {count}"
                )

        def require_unique_operator_fragment(
            module: str,
            symbol: str,
            description: str,
            expected_fragment: str,
        ) -> None:
            source = module_sources.get(module)
            if source is None:
                return
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed temporal action property "
                    f"{symbol}"
                )
                return
            body, line = extracted
            count = compact_shape(body).count(compact_shape(expected_fragment))
            if count != 1:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain exactly one "
                    f"reviewed operator fragment {description}; found {count}"
                )

        authority_module = (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs"
        )
        temporal_properties = {
            "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty": (
                "specification => "
                "[][AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider]"
                "_AsyncAllVars"
            ),
            "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty": (
                "specification => "
                "[][AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider]"
                "_AsyncAllVars"
            ),
            (
                "AdequateLeaderFixedPipelineOriginHistoryAnd"
                "NoResurrectionProviderProperty"
            ): (
                "specification => "
                "[][AdequateLeaderFixedPipelineOriginHistoryAnd"
                "NoResurrectionProvider]_AsyncAllVars"
            ),
            "AdequateLeaderFixedCutPerActionProviderProperty": (
                "specification => "
                "[][AdequateLeaderFixedCutPerActionProvider]_AsyncAllVars"
            ),
            (
                "AdequateLeaderFixedPipelineOriginEpisode"
                "SelectedOwnerStepProviderProperty"
            ): (
                "specification => "
                "[][AdequateLeaderFixedPipelineOriginEpisode"
                "SelectedOwnerStepProvider]_AsyncAllVars"
            ),
            "AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty": (
                "specification => "
                "[][AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider]"
                "_AsyncAllVars"
            ),
            "AdequateLeaderFixedSelectedActionClockCarryProviderProperty": (
                "specification => "
                r"/\ [][AdequateLeaderFixedSelectedCandidateActionCarries"
                "AbsoluteCeiling]_AsyncAllVars "
                r"/\ [][AdequateLeaderFixedSelectedEntryActionCarries"
                "AbsoluteCeiling]_AsyncAllVars"
            ),
            "AdequateLeaderFixedGlobalBlockerProviderProperty": (
                r"/\ (specification => "
                r"[][(/\ AdequateLeaderFixedGlobalBlockerEntryProvider "
                r"/\ AdequateLeaderFixedGlobalProducerEpisodeEntryProvider "
                r"/\ AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider "
                r"/\ AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider "
                r"/\ AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider "
                r"/\ AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider)]"
                "_AsyncAllVars) "
                r"/\ AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty("
                "specification)"
            ),
            "AdequateLeaderFixedSubjectReplacementCutCarryProviderProperty": (
                "specification => "
                "[][AdequateLeaderFixedSubjectReplacementCutCarryProvider]"
                "_AsyncAllVars"
            ),
            "AdequateLeaderRetainedProducerPacketActionProviderProperty": (
                r"/\ initialContext \in ContextRecords "
                r"/\ (specification => "
                r"[][(/\ AdequateLeaderRetainedProducerPacketPrefixEntryProvider("
                "initialContext) "
                r"/\ AdequateLeaderRetainedProducerPacketConcreteOwnerProvider("
                "initialContext) "
                r"/\ AdequateLeaderRetainedProducerPacketSelectedOwnerStepProvider("
                "initialContext))]_AsyncAllVars)"
            ),
        }
        for symbol, expected_body in temporal_properties.items():
            require_temporal_property(
                authority_module,
                symbol,
                expected_body,
            )

        pre_candidate_theorem = (
            "AdequateLeaderFixedPreCandidateSelectionAndFairnessSupplyEntryService"
        )
        require_direct_proof_step(
            authority_module,
            pre_candidate_theorem,
            "selected-owner action implication",
            r"""
/\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
     initialContext, target, leaderContext,
     leader, leaderView, receipt,
     route, token, entryDebt, owner, packet, sourceRank)
/\ ~AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
     initialContext, target, leaderContext,
     leader, leaderView, receipt, route, sourceRank)
/\ <<AdequateLeaderFixedSelectedServiceOwnerAction(owner)>>_AsyncAllVars
=> AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
     initialContext, target, leaderContext,
     leader, leaderView, receipt, route, sourceRank)'
""",
        )
        require_direct_proof_step(
            authority_module,
            pre_candidate_theorem,
            "AsyncNext frame implication",
            r"""
/\ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
     initialContext, target, leaderContext,
     leader, leaderView, receipt,
     route, token, entryDebt, owner, packet, sourceRank)
/\ ~AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
     initialContext, target, leaderContext,
     leader, leaderView, receipt, route, sourceRank)
/\ [AsyncNext]_AsyncAllVars
=> \/ AdequateLeaderFixedPreCandidateEntryStrictRankGoal(
        initialContext, target, leaderContext,
        leader, leaderView, receipt, route, sourceRank)'
   \/ AdequateLeaderFixedSelectedPreCandidateEntryFrontier(
        initialContext, target, leaderContext,
        leader, leaderView, receipt,
        route, token, entryDebt, owner, packet, sourceRank)'
""",
        )

        chain_module = "SumeragiV2ChainEpochRefinement"
        irreversible_theorem = (
            "IndexedChainSpecKeepsServiceActivationRestrictionIrreversible"
        )
        require_compact_theorem_statement(
            chain_module,
            irreversible_theorem,
            r"""
IndexedChainSpec
  => \A initialContext \in AdmissibleContextRecords:
       [](IndexedAsync(initialContext)!AsyncServiceActivationRestricted
            => []IndexedAsync(initialContext)!
                  AsyncServiceActivationRestricted)
""",
        )
        check_theorem_proof_tokens(
            chain_module,
            irreversible_theorem,
            (
                "IndexedChainSpecEstablishesCompositionInvariant",
                "IndexedStepKeepsServiceActivationRestrictionIrreversible",
                "IndexedChainSpec",
                "PTL",
            ),
        )
        require_direct_proof_step(
            chain_module,
            irreversible_theorem,
            "service-activation irreversible implication",
            r"""
IndexedAsync(initialContext)!AsyncServiceActivationRestricted
  /\ [IndexedChainNext]_IndexedChainVars
  => (IndexedAsync(initialContext)!AsyncServiceActivationRestricted)'
""",
        )

        tick_theorem = "IndexedPostGstTickFairnessTransfersLocally"
        require_compact_theorem_statement(
            chain_module,
            tick_theorem,
            r"""
\A initialContext \in AdmissibleContextRecords:
  IndexedChainSpec
    => WF_(IndexedAsyncStateAt(initialContext))(
         IndexedPostGstTick(initialContext))
""",
        )
        check_theorem_proof_tokens(
            chain_module,
            tick_theorem,
            (
                "IndexedChainSpecEstablishesCompositionInvariant",
                "IndexedPostGstHistoricalFairOccurrencesEnableProduct",
                "IndexedPostGstTickProductStepProjectsExactOccurrence",
                "IndexedFairness",
                "ExpandENABLED",
                "PTL",
            ),
        )
        require_direct_proof_step(
            chain_module,
            tick_theorem,
            "post-GST tick projection implication",
            r"""
IndexedCore(initialContext, 7)
  /\ IndexedTickStep(initialContext)
  => <<IndexedPostGstTick(initialContext)>>_(
       IndexedAsyncStateAt(initialContext))
""",
        )

        historical_target_frame_theorem = (
            "IndexedSuccessorActivationStepPreservesHistoricalRecoveryTargets"
        )
        require_compact_theorem_statement(
            chain_module,
            historical_target_frame_theorem,
            r"""
\A parentContext \in AdmissibleContextRecords,
   node \in ValidatorIds:
  IndexedSuccessorActivationProgressStep(parentContext, node)
    => \A initialContext \in AdmissibleContextRecords:
         UNCHANGED IndexedScheduler(initialContext, 44)
""",
        )
        check_theorem_proof_tokens(
            chain_module,
            historical_target_frame_theorem,
            (
                "IndexedSuccessorActivationProgressStep",
                "SuccessorActivationEnvironmentStutter",
                "SuccessorActivationEnvironmentActivatesNode",
                "IndexedAsync!AsyncEnterIndexedServiceActivation",
                "IndexedAsync!AsyncActivateServiceNode",
                "IndexedAsync!AsyncServiceActivationFrameVars",
                "IndexedAsync!AsyncSchedulerExceptServiceActivation",
                "IndexedScheduler",
                "Isa",
            ),
        )
        check_theorem_proof_tokens(
            "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
            "IndexedNonOpenProductStepCannotCreateHistoricalTarget",
            (historical_target_frame_theorem,),
        )

        historical_closure_module = (
            "SumeragiV2HistoricalRecoveryTemporalClosureProofs"
        )
        strict_ancestor_theorem = (
            "IndexedHistoricalLowerAuthorityProgressGivesStrictAncestorAdvance"
        )
        require_compact_theorem_statement(
            historical_closure_module,
            strict_ancestor_theorem,
            r"""
\A targetContext \in AdmissibleContextRecords:
  /\ IndexedHistoricalRecoveryAuthorityProgressBelow(targetContext)
  /\ IndexedHistoricalRecoveryEntryCompletionBelow(targetContext)
  => IndexedStrictAncestorRecoveryAdvance(targetContext)
""",
        )
        check_theorem_proof_tokens(
            historical_closure_module,
            strict_ancestor_theorem,
            (
                "IndexedHistoricalRecoveryAuthorityProgressBelow",
                "IndexedHistoricalRecoveryEntryCompletionBelow",
                "IndexedAdmissibleTargetHasAdmissibleAncestors",
                "HistoricalRecoveryComplete",
                "IndexedNodePastContext",
                "PTL",
            ),
        )
        check_theorem_proof_tokens(
            historical_closure_module,
            "IndexedHistoricalDiscoveryOwnedOutcomeDropsCertificateRankFour",
            (
                "IndexedHistoricalTransport!HistoricalCommitRequestOccurrence",
                "IndexedScheduler",
            ),
        )
        certificate_rank_theorem = (
            "IndexedChainSpecClosesHistoricalCertificateDiscoveryRank"
        )
        require_unique_direct_proof_step(
            historical_closure_module,
            certificate_rank_theorem,
            "certificate-stage GST core projection",
            r"""
[](IndexedHistoricalCertificateStageAt(initialContext, node, 4)
  => /\ IndexedCore(initialContext, 7)
     /\ IndexedHistoricalTransport(initialContext)!
          HistoricalRecoveryTarget(node))
""",
        )
        require_unique_direct_proof_step(
            historical_closure_module,
            certificate_rank_theorem,
            "owned-discovery GST core projection",
            r"""
(IndexedCore(initialContext, 7)
   /\ IndexedHistoricalTransport(initialContext)!
        HistoricalRecoveryTarget(node))
  ~> IndexedHistoricalDiscoveryOwnedOutcome(initialContext, node)
""",
        )
        require_unique_operator_fragment(
            historical_closure_module,
            "IndexedAdequateLeaderLocalFairBehaviorAt",
            "adequate-leader Tick GST core projection",
            r"""
WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)(
  IndexedCore(initialContext, 7)
    /\ IndexedAdequateLeaderWitness(initialContext)!AsyncTick)
""",
        )
        require_unique_direct_proof_step(
            historical_closure_module,
            "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster",
            "local-source GST core projection",
            "IndexedCore(initialContext, 7)",
        )

    def check_historical_import_and_projection_contract() -> None:
        for module, expected_extends in (
            HISTORICAL_TEMPORAL_REVIEWED_EXTENDS.items()
        ):
            source = module_sources.get(module)
            if source is None:
                continue
            actual_extends = _module_extends(source)
            if actual_extends != expected_extends:
                errors.append(
                    f"{module}.tla must EXTEND exactly "
                    f"{list(expected_extends)!r}; "
                    f"found {list(actual_extends)!r}"
                )
            forbidden_imports = (
                HISTORICAL_TEMPORAL_FORBIDDEN_DIRECT_IMPORTS.get(
                    module, ()
                )
            )
            present_forbidden = tuple(
                imported
                for imported in forbidden_imports
                if imported in actual_extends
            )
            if present_forbidden:
                errors.append(
                    f"{module}.tla may not directly import circular "
                    "historical/height proof modules "
                    f"{list(present_forbidden)!r}"
                )

        expected_mappings = tuple(
            f"{field} <- IndexedCore(initialContext, {index})"
            for index, field in enumerate(
                HISTORICAL_INDEXED_CORE_FIELDS,
                start=1,
            )
        ) + tuple(
            f"{field} <- IndexedScheduler(initialContext, {index})"
            for index, field in enumerate(
                HISTORICAL_INDEXED_SCHEDULER_FIELDS,
                start=1,
            )
        ) + tuple(
            f"{field} <- IndexedRecovery(initialContext, {index})"
            for index, field in enumerate(
                HISTORICAL_INDEXED_RECOVERY_FIELDS,
                start=1,
            )
        ) + tuple(
            f"{field} <- IndexedProducer(initialContext, {index})"
            for index, field in enumerate(
                HISTORICAL_INDEXED_PRODUCER_FIELDS,
                start=1,
            )
        ) + (
            "asyncFixedCorridorDeadlines <- "
            "IndexedFixedCorridorDeadlines(initialContext)",
            "asyncServeProducerTurnReady <- "
            "IndexedServeProducerTurnDue(initialContext)",
        )
        for module, symbol, instance_module in (
            HISTORICAL_INDEXED_INSTANCE_CONTRACTS
        ):
            source = module_sources.get(module)
            if source is None:
                continue
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed historical instance "
                    f"{symbol}"
                )
                continue
            body, line = extracted
            normalized_body = " ".join(body.split())
            expected_body = (
                f"INSTANCE {instance_module} WITH "
                + ", ".join(expected_mappings)
            )
            if normalized_body != expected_body:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must instantiate exactly "
                    f"{instance_module} with the reviewed 49 Core, 46 "
                    "scheduler, 5 recovery, 3 producer-journal, and "
                    "fixed-corridor/Serve-producer-debt projections"
                )

    def check_historical_finite_runner_provider_contract() -> None:
        """Keep finite-runner providers below their liveness consumers."""

        audited = {
            (module, symbol)
            for module, symbols in (
                HISTORICAL_FINITE_RUNNER_PROVIDER_THEOREMS.items()
            )
            for symbol in symbols
        }
        unscoped_required = sorted(
            set(HISTORICAL_FINITE_RUNNER_PROVIDER_REQUIRED_PROOF_TOKENS)
            - audited
        )
        if unscoped_required:
            errors.append(
                "historical finite-runner/provider dependency contract has "
                f"unaudited entries {unscoped_required!r}"
            )

        exact_forbidden = set(
            HISTORICAL_FINITE_RUNNER_PROVIDER_FORBIDDEN_DEPENDENCIES
        )
        forbidden_patterns = tuple(
            re.compile(pattern)
            for pattern in (
                HISTORICAL_FINITE_RUNNER_PROVIDER_FORBIDDEN_DEPENDENCY_PATTERNS
            )
        )
        for module, symbols in (
            HISTORICAL_FINITE_RUNNER_PROVIDER_THEOREMS.items()
        ):
            source = module_sources.get(module)
            if source is None:
                continue
            for symbol in symbols:
                extracted = _top_level_theorem_body(
                    source,
                    symbol,
                    preserve_string_contents=True,
                )
                if extracted is None:
                    errors.append(
                        f"{module}.tla: missing reviewed historical "
                        f"finite-runner/provider theorem {symbol}"
                    )
                    continue
                body, line = extracted
                proof_marker = re.search(
                    r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                    body,
                )
                proof = (
                    ""
                    if proof_marker is None
                    else body[proof_marker.start() :]
                )
                required = (
                    HISTORICAL_FINITE_RUNNER_PROVIDER_REQUIRED_PROOF_TOKENS.get(
                        (module, symbol), ()
                    )
                )
                missing = tuple(
                    dependency
                    for dependency in required
                    if not _tla_dependency_present(proof, dependency)
                )
                if proof_marker is None or missing:
                    errors.append(
                        f"{module}.tla:{line}: {symbol} must retain reviewed "
                        "historical finite-runner/provider proof "
                        f"dependencies; missing={missing!r}, "
                        f"has_proof={proof_marker is not None}"
                    )

                forbidden = sorted(
                    {
                        token
                        for token in tla_code_tokens(body)
                        if token in exact_forbidden
                        or any(
                            pattern.fullmatch(token)
                            for pattern in forbidden_patterns
                        )
                    }
                )
                if forbidden:
                    errors.append(
                        f"{module}.tla:{line}: {symbol} may not depend on "
                        "indexed height/application liveness or "
                        "all-responsive-joined assumptions; "
                        f"forbidden={forbidden!r}"
                    )

    def check_historical_local_authority_provider_contract() -> None:
        """Pin historical authority to one local owner and exact providers."""

        module = "SumeragiV2HistoricalRecoveryTemporalClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return

        for symbol in HISTORICAL_LOCAL_AUTHORITY_REQUIRED_OPERATORS:
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed local historical "
                    f"authority operator {symbol}"
                )
                continue
            body, line = extracted
            expected = HISTORICAL_LOCAL_AUTHORITY_EXACT_OPERATOR_BODIES[symbol]
            observed = " ".join(body.split())
            if observed != expected:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the exact "
                    f"reviewed local authority alias body {expected!r}; "
                    f"found {observed!r}"
                )

        for symbol, required in {
            "IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty": (
                "AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty",
                "IndexedChainSpec",
            ),
            "IndexedAdequateLeaderLocalFairBehaviorAt": (
                "PostGstServiceIoWorker",
            ),
        }.items():
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            missing = tuple(
                token
                for token in required
                if not _tla_dependency_present(body, token)
            )
            if missing:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain exact "
                    "fixed-deadline/fair-owner dependencies; "
                    f"missing={missing!r}"
                )

        exact_statements = {
            "IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst": (
                "IndexedChainSpec => \\A initialContext \\in "
                "AdmissibleContextRecords: /\\ initialContext \\in "
                "JoinedContexts /\\ IndexedResponsiveActiveRosterAt("
                "initialContext) ~> IndexedCore(initialContext, 7)"
            ),
            "IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec": (
                "\\A initialContext \\in AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!"
                "AsyncLiveSpecAt(initialContext) => "
                "IndexedAdequateLeaderWitness(initialContext)! "
                "AsyncLiveSpecAt(initialContext)"
            ),
            "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster": (
                "\\A initialContext \\in AdmissibleContextRecords, "
                "target \\in ValidatorIds: /\\ IndexedCompositionInvariant "
                "/\\ IndexedAdequateLeaderWitness(initialContext)! "
                "AdequateLeaderLocalTargetDecisionSource(target) "
                "=> IndexedAllResponsiveJoined(initialContext)"
            ),
            "IndexedAdequateLeaderFixedDeadlineSourceJoinsResponsiveRoster": (
                "\\A initialContext \\in AdmissibleContextRecords, "
                "target \\in ValidatorIds, "
                "leaderContext \\in ContextRecords, "
                "leaderView \\in Views, "
                "startTime, deadline \\in Nat: "
                "/\\ IndexedCompositionInvariant "
                "/\\ IndexedAdequateLeaderWitness(initialContext)! "
                "AdequateLeaderFixedCorridorDeadlineSource( target, "
                "leaderContext, leaderView, startTime, deadline) "
                "/\\ ~IndexedAdequateLeaderWitness(initialContext)!"
                "NodeHasDecision(target) "
                "=> IndexedAllResponsiveJoined(initialContext)"
            ),
            (
                "IndexedLiveChainSpecProvidesLocalAdequateLeader"
                "FreshSelfCorridorExposure"
            ): (
                "IndexedLiveChainSpec => "
                "IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty"
            ),
            (
                "IndexedLiveChainSpecProvidesLocalAdequateLeader"
                "FixedDeadlineAndResponsiveDissemination"
            ): (
                "IndexedLiveChainSpec => "
                "IndexedLocalAdequateLeaderFixedDeadlineAnd"
                "ResponsiveDisseminationProperty"
            ),
            "IndexedAdequateLeaderCompletedProvidersSupplyLocalSemanticKernel": (
                "/\\ IndexedLiveChainSpec /\\ "
                "IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty "
                "/\\ IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty "
                "/\\ IndexedLocalAdequateLeaderProducerTransportClosureProperty "
                "/\\ IndexedLocalAdequateLeader"
                "ProducerTransportOccurrenceClosureProperty "
                "/\\ IndexedLocalAdequateLeaderRetainedProducer"
                "NonDescentEpisodeStepProperty "
                "/\\ IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty "
                "=> IndexedLocalAdequateLeaderSemanticKernelProperty"
            ),
            (
                "IndexedAdequateLeaderRetainedProducer"
                "StepAndOccurrenceClosureCompose"
            ): (
                "/\\ IndexedLocalAdequateLeader"
                "ProducerTransportOccurrenceClosureProperty "
                "/\\ IndexedLocalAdequateLeaderRetainedProducer"
                "NonDescentEpisodeStepProperty "
                "=> IndexedLocalAdequateLeaderRetainedProducer"
                "OccurrenceClosureProperty"
            ),
            (
                "IndexedAdequateLeaderFixedDeadlineDissemination"
                "AndExposureSupplyLocalConvergence"
            ): (
                "/\\ IndexedLocalAdequateLeaderFixedDeadlineAnd"
                "ResponsiveDisseminationProperty "
                "/\\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty "
                "=> IndexedLocalAdequateLeaderDecisionConvergenceProperty"
            ),
        }
        for symbol, expected in exact_statements.items():
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            observed = " ".join(statement.split())
            if observed != expected:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the exact "
                    f"reviewed local statement {expected!r}; found "
                    f"{observed!r}"
                )

        retired_joined_gst = "IndexedChainSpecJoinedContextEventuallySetsExactGst"
        if re.search(
            rf"(?m)^[ \t]*THEOREM[ \t]+{retired_joined_gst}\b",
            strip_tla_comments(source, preserve_string_contents=True),
        ):
            errors.append(
                f"{module}.tla: retired one-joined-context GST theorem "
                f"{retired_joined_gst} must not be resurrected"
            )

        network_source = module_sources.get("SumeragiV2AsyncNetwork")
        if network_source is not None:
            set_gst = _top_level_operator_body(
                network_source,
                "AsyncSetGST",
                preserve_string_contents=True,
            )
            if set_gst is None:
                errors.append(
                    "SumeragiV2AsyncNetwork.tla: missing exact AsyncSetGST "
                    "guard for local GST composition"
                )
            else:
                body, line = set_gst
                guard = "Responsive \\subseteq AsyncActiveServiceNodes"
                if " ".join(body.split()).count(guard) != 1:
                    errors.append(
                        "SumeragiV2AsyncNetwork.tla:"
                        f"{line}: AsyncSetGST must retain the exact responsive "
                        "active-roster guard exactly once"
                    )

        chain_source = module_sources.get("SumeragiV2ChainEpochRefinement")
        if chain_source is not None:
            for symbol, required in {
                "IndexedCompositionInvariant": (
                    "IndexedPostGstResponsiveActiveRosterCoherence",
                ),
                "IndexedActionPreservesCompositionInvariant": (
                    "IndexedActionPreservesPostGstResponsiveActiveRosterCoherence",
                ),
                "IndexedStepPreservesCompositionInvariant": (
                    "IndexedPostGstResponsiveActiveRosterCoherence",
                ),
                "IndexedPostGstResponsiveRosterIsActive": (
                    "IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive",
                    "IndexedPostGstResponsiveActiveRosterCoherence",
                ),
            }.items():
                extracted = (
                    _top_level_operator_body(
                        chain_source,
                        symbol,
                        preserve_string_contents=True,
                    )
                    if symbol == "IndexedCompositionInvariant"
                    else _top_level_theorem_body(
                        chain_source,
                        symbol,
                        preserve_string_contents=True,
                    )
                )
                if extracted is None:
                    errors.append(
                        "SumeragiV2ChainEpochRefinement.tla: missing GST "
                        f"active-roster derivation {symbol}"
                    )
                    continue
                body, line = extracted
                missing = tuple(
                    token
                    for token in required
                    if not _tla_dependency_present(body, token)
                )
                if missing:
                    errors.append(
                        "SumeragiV2ChainEpochRefinement.tla:"
                        f"{line}: {symbol} must retain GST active-roster "
                        f"dependencies; missing={missing!r}"
                    )

        for symbol, required in (
            HISTORICAL_LOCAL_AUTHORITY_REQUIRED_PROOF_TOKENS.items()
        ):
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing reviewed local historical "
                    f"authority theorem {symbol}"
                )
                continue
            body, line = extracted
            marker = re.search(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
            )
            proof = "" if marker is None else body[marker.start() :]
            missing = tuple(
                dependency
                for dependency in required
                if not _tla_dependency_present(proof, dependency)
            )
            if marker is None or missing:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain reviewed "
                    "local historical authority proof dependencies; "
                    f"missing={missing!r}, has_proof={marker is not None}"
                )

        exposure_symbol = (
            "IndexedLiveChainSpecProvidesLocalAdequateLeader"
            "FreshSelfCorridorExposure"
        )
        exposure = _top_level_theorem_body(
            source,
            exposure_symbol,
            preserve_string_contents=True,
        )
        if exposure is not None:
            body, line = exposure
            marker = re.search(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
            )
            proof = "" if marker is None else body[marker.start() :]
            forbidden_exposure_dependencies = (
                "IndexedLocalAdequateLeaderAuthorityBoundDecisionCarryProperty",
                "AdequateLeaderAuthorityDeadlineActiveReceiptDecisionCarryProperty",
                "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
                "AdequateLeaderAuthorityDeadlineQuantitativeProviderBundle",
                "AdequateLeaderAuthorityBoundReceiptCarrySuppliesLocalSemanticKernel",
                "AdequateLeaderAuthorityBoundReceiptCarrySuppliesFreshSelfLeaderDecision",
            )
            proof_tokens = set(tla_code_tokens(proof))
            prohibited = tuple(
                sorted(
                    {
                        dependency
                        for dependency in forbidden_exposure_dependencies
                        if _tla_dependency_present(proof, dependency)
                    }
                    | {
                        token
                        for token in proof_tokens
                        if token.startswith("AdequateLeaderAuthority")
                        or "AuthorityCarry" in token
                    }
                )
            )
            if prohibited:
                errors.append(
                    f"{module}.tla:{line}: {exposure_symbol} must derive "
                    "source-conditioned local exposure without authority "
                    f"carry or bundle assumptions; prohibited={prohibited!r}"
                )
            normalized_proof = " ".join(
                strip_tla_comments(
                    proof,
                    preserve_string_contents=True,
                ).split()
            )
            normalized_proof = re.sub(r"!\s+", "!", normalized_proof)
            source_condition = (
                "IndexedAdequateLeaderWitness(initialContext)!"
                "AdequateLeaderLocalTargetDecisionSource(target)"
            )
            required_source_cases = (
                f"CASE <>{source_condition}",
                f"CASE ~<>{source_condition}",
                "TRUE ~> IndexedAllResponsiveJoined(initialContext)",
            )
            invalid_case_counts = {
                fragment: normalized_proof.count(fragment)
                for fragment in required_source_cases
                if normalized_proof.count(fragment) != 1
            }
            if invalid_case_counts:
                errors.append(
                    f"{module}.tla:{line}: {exposure_symbol} must retain "
                    "the exact source-occurrence/vacuity split and derive "
                    "the stable local activation premise; "
                    f"counts={invalid_case_counts!r}"
                )

        aggregate_dependency_source = strip_tla_comments(
            source,
            preserve_string_contents=True,
        )
        for reviewed_exposure_symbol in (
            "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster",
            "IndexedAdequateLeaderFixedDeadlineSourceJoinsResponsiveRoster",
            (
                "IndexedLiveChainSpecProvidesLocalAdequateLeader"
                "FreshSelfCorridorExposure"
            ),
            (
                "IndexedLiveChainSpecProvidesLocalAdequateLeader"
                "FixedDeadlineAndResponsiveDissemination"
            ),
        ):
            reviewed_exposure = _top_level_theorem_body(
                aggregate_dependency_source,
                reviewed_exposure_symbol,
                preserve_string_contents=True,
            )
            if reviewed_exposure is not None:
                reviewed_body, _ = reviewed_exposure
                reviewed_forbidden = sorted(
                    (
                        set(tla_code_tokens(reviewed_body))
                        & set(HISTORICAL_LOCAL_AUTHORITY_FORBIDDEN_DEPENDENCIES)
                    )
                    - {"IndexedAllResponsiveJoined"}
                )
                if reviewed_forbidden:
                    errors.append(
                        f"{module}.tla: {reviewed_exposure_symbol} may use "
                        "IndexedAllResponsiveJoined only for its reviewed "
                        "source-conditioned activation step and may not import "
                        "other aggregate liveness dependencies; "
                        f"forbidden={reviewed_forbidden!r}"
                    )
                aggregate_dependency_source = (
                    aggregate_dependency_source.replace(
                        reviewed_body,
                        "\n",
                        1,
                    )
                )
        forbidden = sorted(
            set(tla_code_tokens(aggregate_dependency_source))
            & set(HISTORICAL_LOCAL_AUTHORITY_FORBIDDEN_DEPENDENCIES)
        )
        if forbidden:
            errors.append(
                f"{module}.tla: local historical authority may not import "
                "aggregate GST/application/one-height/height dependencies; "
                f"forbidden={forbidden!r}"
            )

    def check_adequate_leader_authority_deadline_contract() -> None:
        """Pin the conditional quantitative boundary and repaired rank seams."""

        module = "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs"
        source = module_sources.get(module)
        if source is None:
            return

        def require_operator(
            symbol: str,
            *,
            exact: str | None = None,
            required: tuple[str, ...] = (),
            forbidden: tuple[str, ...] = (),
        ) -> str | None:
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing authority-deadline operator {symbol}"
                )
                return None
            body, line = extracted
            normalized = " ".join(body.split())
            if exact is not None and normalized != exact:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the exact "
                    f"authority-deadline body {exact!r}; found {normalized!r}"
                )
            missing = tuple(
                token
                for token in required
                if not _tla_dependency_present(body, token)
            )
            present = tuple(
                token
                for token in forbidden
                if _tla_dependency_present(body, token)
            )
            if missing or present:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the reviewed "
                    "authority-deadline dependencies; "
                    f"missing={missing!r}, forbidden={present!r}"
                )
            return body

        def require_theorem(
            symbol: str,
            *,
            exact_statement: str | None = None,
            required: tuple[str, ...] = (),
            forbidden: tuple[str, ...] = (),
        ) -> None:
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing authority-deadline theorem {symbol}"
                )
                return
            body, line = extracted
            parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )
            statement = " ".join(parts[0].split())
            proof = "" if len(parts) == 1 else parts[1]
            if exact_statement is not None and statement != exact_statement:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the exact "
                    f"authority-deadline statement {exact_statement!r}; "
                    f"found {statement!r}"
                )
            missing = tuple(
                token
                for token in required
                if not _tla_dependency_present(proof, token)
            )
            present = tuple(
                token
                for token in forbidden
                if _tla_dependency_present(proof, token)
            )
            if len(parts) == 1 or missing or present:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the reviewed "
                    "authority-deadline proof chain; "
                    f"missing={missing!r}, forbidden={present!r}, "
                    f"has_proof={len(parts) > 1}"
                )

        exact_arithmetic = {
            "AdequateLeaderFixedOriginSlotCapacity": "AsyncChunkCount + 8",
            "AdequateLeaderFixedPerOriginSlotEpisodeCharge": (
                "AdequateLeaderFixedCandidateCharge + 1"
            ),
            "AdequateLeaderFixedProtocolTokenCharge": (
                "AdequateLeaderFixedOriginSlotCapacity "
                "* AdequateLeaderFixedPerOriginSlotEpisodeCharge"
            ),
            "AdequateLeaderFixedLeaderPipelineActionBudget": (
                "4 * N * AdequateLeaderFixedProtocolTokenCharge"
            ),
            "AdequateLeaderFixedConfiguredPipelineBudgetCompatibility": (
                "AdequateLeaderFixedLeaderPipelineActionBudget "
                "<= AsyncProposalPipelineBudget"
            ),
            "AdequateLeaderFixedConfiguredDeadlineCompatibility": (
                "AdequateLeaderFixedLeaderPipelineClockBudget "
                "+ AdequateLeaderFixedCommitQcDisseminationClockBudget "
                "<= AsyncFixedCorridorServiceBudget"
            ),
            "AdequateLeaderFixedRetireOwner": (
                '[ownerKind |-> "Retire", node |-> 0, '
                "source |-> AsyncUntrustedSource, slot |-> slot]"
            ),
            "AdequateLeaderFixedCommitQcTransportCharge": (
                "AsyncOneWayTransportBudget"
            ),
            "AdequateLeaderFixedCommitQcIoCharge": (
                "AsyncIoDrainBudget * AsyncDeliveryBound"
            ),
            "AdequateLeaderFixedCommitQcRunnerCharge": (
                "2 * AsyncRuntimeCycleBudget * AsyncDeliveryBound"
            ),
            "AdequateLeaderFixedCommitQcRetransmitCharge": (
                "3 * AsyncRetransmitPeriod"
            ),
            "AdequateLeaderFixedCommitQcCompletionCharge": (
                "AsyncCompletionReserve"
            ),
            "AdequateLeaderFixedCommitQcDisseminationClockBudget": (
                "AdequateLeaderFixedCommitQcTransportCharge "
                "+ AdequateLeaderFixedCommitQcIoCharge "
                "+ AdequateLeaderFixedCommitQcRunnerCharge "
                "+ AdequateLeaderFixedCommitQcRetransmitCharge "
                "+ AdequateLeaderFixedCommitQcCompletionCharge"
            ),
        }
        for symbol, exact in exact_arithmetic.items():
            require_operator(symbol, exact=exact)

        require_theorem(
            "AdequateLeaderFixedLeaderPipelineBudgetMatchesConfiguration",
            exact_statement=(
                "AdequateLeaderFixedLeaderPipelineActionBudget "
                "= AsyncProposalPipelineBudget"
            ),
            required=(
                "AdequateLeaderFixedProtocolTokenCharge",
                "AdequateLeaderFixedOriginSlotCapacity",
                "AdequateLeaderFixedPerOriginSlotEpisodeCharge",
                "AsyncProposalPipelineBudget",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedDeadlineBudgetMatchesConfiguration",
            exact_statement=(
                "AdequateLeaderFixedLeaderPipelineClockBudget "
                "+ AdequateLeaderFixedCommitQcDisseminationClockBudget "
                "= AsyncFixedCorridorServiceBudget"
            ),
            required=(
                "AdequateLeaderFixedLeaderPipelineBudgetMatchesConfiguration",
                "AdequateLeaderFixedCommitQcDisseminationClockBudget",
                "AdequateLeaderFixedCommitQcTransportCharge",
                "AdequateLeaderFixedCommitQcIoCharge",
                "AdequateLeaderFixedCommitQcRunnerCharge",
                "AdequateLeaderFixedCommitQcRetransmitCharge",
                "AdequateLeaderFixedCommitQcCompletionCharge",
                "AsyncFixedCorridorServiceBudget",
            ),
        )

        selected_service_owner_set = require_operator(
            "AdequateLeaderFixedSelectedServiceOwnerSet",
            required=(
                "AdequateLeaderFixedRetireOwner",
                "AsyncLeaderWireLifecycleSlotSet",
                "AdequateLeaderFixedTickOwner",
                '"RunHistoricalRecovery"',
                '"RunHistoricalServer"',
                '"ServiceHistoricalIo"',
                '"AdmitHistorical"',
                '"ResolveLocalProducer"',
                '"ServiceConditionalProducer"',
                '"ServiceVolatileProducer"',
            ),
        )
        if selected_service_owner_set is not None:
            for owner_kind in (
                "ResolveLocalProducer",
                "ServiceConditionalProducer",
                "ServiceVolatileProducer",
            ):
                if selected_service_owner_set.count(f'"{owner_kind}"') != 1:
                    errors.append(
                        f"{module}.tla: "
                        "AdequateLeaderFixedSelectedServiceOwnerSet must "
                        "retain reviewed authority-deadline dependencies: "
                        f"contain exactly one {owner_kind} fair owner"
                    )
        selected_service_owner_action = require_operator(
            "AdequateLeaderFixedSelectedServiceOwnerAction",
            required=(
                "PostGstRetireLeaderWireLifecycleSlot",
                "PostGstRunHistoricalRecoveryNode",
                "PostGstRunHistoricalServer",
                "PostGstServiceHistoricalRecoveryIoWorker",
                "PostGstAdmitHistoricalRecoveryPacket",
                "PostGstResolveLocalCandidateProducerContinuation",
                "PostGstServiceConditionalTransportProducerContinuation",
                "PostGstServiceVolatileBodyProducerContinuation",
                "OTHER",
                "FALSE",
            ),
        )
        if selected_service_owner_action is not None:
            normalized_action = " ".join(selected_service_owner_action.split())
            volatile_dispatch = (
                'owner.ownerKind = "ServiceVolatileProducer" -> '
                "PostGstServiceVolatileBodyProducerContinuation(owner.node)"
            )
            if volatile_dispatch not in normalized_action:
                errors.append(
                    f"{module}.tla: "
                    "AdequateLeaderFixedSelectedServiceOwnerAction must "
                    "retain reviewed authority-deadline dependencies and "
                    "dispatch ServiceVolatileProducer to its exact PostGst "
                    "fair action"
                )
            if "[] OTHER -> FALSE" not in normalized_action:
                errors.append(
                    f"{module}.tla: "
                    "AdequateLeaderFixedSelectedServiceOwnerAction must "
                    "retain reviewed authority-deadline dependencies and "
                    "reject unknown owner kinds with OTHER -> FALSE"
                )
        require_theorem(
            "AdequateLeaderFixedSelectedOwnerUsesExactAsyncFairness",
            required=(
                "AdequateLeaderFixedRetireOwner",
                "AdequateLeaderFixedSelectedServiceOwnerSet",
                "AdequateLeaderFixedSelectedServiceOwnerAction",
            ),
        )

        require_operator(
            "AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProvider",
            required=(
                "AdequateLeaderFixedDiscoveredPipelineOriginsForToken",
                "AdequateLeaderFixedLivePipelineOriginsForToken",
                "AdequateLeaderFrozenTargetCorridor",
                "AdequateLeaderFixedAuthorityPipelineRemainingTokens",
            ),
            forbidden=(
                "AdequateLeaderFixedDiscoveredPipelineOriginSlotsForToken",
                "AdequateLeaderFixedLivePipelineOriginSlotsForToken",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineOriginEqualCountReplacementAction",
            required=(
                "AdequateLeaderTargetEqualCountOwnerReplacementAction",
                "AdequateLeaderFixedDiscoveredPipelineOriginsForToken",
                "AdequateLeaderFixedLivePipelineOriginsForToken",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction",
            required=(
                "AdequateLeaderTargetCountIncreasingReplenishmentAction",
                "AdequateLeaderFixedDiscoveredPipelineOriginsForToken",
                "AdequateLeaderFixedLivePipelineOriginsForToken",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineOriginNonDescentEpisodeAction",
            required=(
                "AdequateLeaderFixedPipelineOriginEqualCountReplacementAction",
                "AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget",
            required=(
                "AdequateLeaderFixedSelectedOccurrenceLifecycleActive",
                "AdequateLeaderTargetEpisodeKnownOwnerSet",
                "AdequateLeaderTargetLiveOwnerIdentitySet",
                "AdequateLeaderTargetNonDescentEpisodeBudget",
            ),
            forbidden=(
                "AdequateLeaderTargetNonDescentEpisodeAtBudget",
                "AdequateLeaderFixedSemanticStrictDescentCarriesPhysicalRankProvider",
                "AdequateLeaderFixedSemanticStrictDescentCarriesPhysicalRankProviderProperty",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineOriginEpisodeFrontier",
            required=(
                "AdequateLeaderFixedPipelineOriginEpisodeDebtAtBudget",
                "AdequateLeaderFixedPipelineEpisodeCurrentRankAdmissible",
                "sourceOccurrenceRank",
                "sourceOccurrenceOwner",
                "sourceCutoffOrdinal",
                "sourceDormantPotential",
                "knownDormantPotential",
                "known",
                "sourceRank",
                "budget",
                "token",
                "episodeTarget",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal",
            required=(
                "AdequateLeaderFixedPipelineStrictRankGoal",
                "AdequateLeaderFixedPipelineOriginEpisodeFrontier",
                "sourceOccurrenceRank",
                "sourceOccurrenceOwner",
                "sourceRank",
                "token",
                "episodeTarget",
            ),
            forbidden=(
                "AdequateLeaderFixedPipelineOriginCountIncreasingReplenishmentAction",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedPipelineOriginEpisodeStepClosesNonDescentEpisode",
            exact_statement=(
                "\\A specification: "
                "AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty( "
                "specification) "
                "=> "
                "AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty( "
                "specification)"
            ),
            required=(
                "AdequateLeaderFixedPipelineOriginEpisodeBudgetOrderingIsWellFounded",
                "WellFoundedLeadsTo",
                "AdequateLeaderFixedPipelineOriginEpisodeBudgetDescentGoal",
                "AdequateLeaderFixedPipelineOriginEpisodeBudgetOrdering",
                "AdequateLeaderFixedPipelineOriginEpisodeBudgetCarrier",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedOriginEpisodeClosureSuppliesSelectedOwnerService",
            exact_statement=(
                "\\A specification: "
                "/\\ (specification => []AsyncStrongTypeInvariant) "
                "/\\ "
                "AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty( "
                "specification) "
                "=> AdequateLeaderFixedSelectedOwnerServiceProperty(specification)"
            ),
            required=(
                "AdequateLeaderFixedSelectedFrontierStartsPinnedEpisodeOrStrictRank",
                "AdequateLeaderFixedPipelineOriginNonDescentEpisodeClosureProperty",
            ),
            forbidden=(
                "AdequateLeaderFixedSemanticStrictDescentCarriesPhysicalRankProvider",
                "AdequateLeaderFixedSemanticStrictDescentCarriesPhysicalRankProviderProperty",
            ),
        )

        require_operator(
            "AdequateLeaderFixedDeterministicHeadOrGateToken",
            required=(
                "AdequateLeaderFixedSelectedServiceOwnerAction",
                "AdequateLeaderFixedCandidateOwnsPipelineToken",
                "NodeHasDecision",
                "AdequateLeaderFrozenTargetCorridor",
            ),
        )
        require_operator(
            "AdequateLeaderFixedPreCandidateTokenIsExact",
            required=(
                "AdequateLeaderFixedDeterministicHeadOrGateToken",
                "AsyncCandidateProducerContinuationSelectedResolutionRecord",
                "AsyncLeaderWireLifecycleRecordsForSlot",
                "AdequateLeaderFixedWireItemProtocolToken",
            ),
        )
        for symbol in (
            "AdequateLeaderFixedPreCandidateEntryRankCell",
            "AdequateLeaderFixedSelectedPreCandidateEntryFrontier",
            "AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider",
        ):
            require_operator(
                symbol,
                forbidden=("AdequateLeaderFreshSynchronizedTargetCorridor",),
            )

        require_operator(
            "AdequateLeaderFixedPreCandidateRouteTyped",
            required=(
                "AdequateLeaderFixedPreCandidateRouteIdentityCarrier",
            ),
        )
        require_operator(
            "AdequateLeaderFixedCandidatePipelineServiceCellIdentity",
            exact=(
                '[kind |-> "Candidate", token |-> token, '
                "candidate |-> candidate, cutoffOrdinal |-> cutoffOrdinal, "
                "semanticRank |-> semanticRank]"
            ),
        )
        require_operator(
            "AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity",
            exact=(
                '[kind |-> "PreCandidate", token |-> token, '
                "route |-> route]"
            ),
        )
        require_operator(
            "AdequateLeaderFixedPipelineServiceRankFrontierForCell",
            required=(
                "AdequateLeaderFixedCandidatePipelineServiceCellIdentity",
                "AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity",
                "AdequateLeaderFixedPipelineRankCell",
                "AdequateLeaderFixedPreCandidateEntryRankCell",
                "cellIdentity",
            ),
        )
        require_operator(
            "AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell",
            required=(
                "AdequateLeaderFixedCandidatePipelineServiceCellIdentity",
                "AdequateLeaderFixedPreCandidatePipelineServiceCellIdentity",
                "AdequateLeaderFixedSelectedPipelineRankFrontier",
                "AdequateLeaderFixedSelectedPreCandidateEntryFrontier",
                "cellIdentity",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalLifecycleSnapshotCarrier",
            required=(
                "producerRequests",
                "ingressEpisodes",
                "candidateRoots",
                "schedulerCuts",
                "physicalCuts",
                "predecessors",
                "ExactDecisionTargetNeutralCausalRootSet",
            ),
            forbidden=(
                "candidateStart",
                "serveStart",
                "candidateCeiling",
                "serveCeiling",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerSnapshotCarrier",
            required=(
                "AdequateLeaderFixedPipelineServiceCellIdentityCarrier",
                "AdequateLeaderFixedGlobalLifecycleSnapshotCarrier",
                "serviceCellIdentity",
                "fixedClock",
            ),
        )
        require_operator(
            "AdequateLeaderFixedConfiguredGlobalBlockerSnapshot",
            required=(
                "serviceCellIdentity",
                "serviceCellIdentity |-> serviceCellIdentity",
                "ExactDecisionTargetNeutralFixedClockSnapshot",
            ),
            forbidden=(
                "candidateStart",
                "serveStart",
                "candidateCeiling",
                "serveCeiling",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerSnapshotActive",
            required=(
                "AdequateLeaderFixedGlobalBlockerSnapshotCarrier",
                "ExactDecisionTargetNeutralSnapshotActive",
                "snapshot.fixedClock",
            ),
        )
        require_operator(
            "AdequateLeaderFixedConcreteGlobalBlockerRank",
            required=(
                "ExactDecisionTargetNeutralConcreteFixedClockRankForSnapshot",
                "snapshot.fixedClock",
            ),
            forbidden=("ExactDecisionTargetNeutralConcreteFixedClockRank",),
        )
        require_operator(
            "AdequateLeaderFixedGlobalProducerEpisodeRank",
            required=(
                "ExactDecisionTargetNeutralProducerEpisodeRank",
                "snapshot.fixedClock",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalProducerEpisodeRankCarrier",
            exact="ExactDecisionTargetNeutralProducerEpisodeCarrier",
        )
        require_operator(
            "AdequateLeaderFixedGlobalProducerEpisodeRankOrdering",
            exact="ExactDecisionTargetNeutralProducerEpisodeOrdering",
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerSelectionGoal",
            required=(
                "AdequateLeaderFixedPipelineServiceRankDescentGoal",
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell",
                "serviceCellIdentity",
            ),
            forbidden=(
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontier",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerStrictRankGoal",
            exact=(
                "\\/ AdequateLeaderFixedGlobalBlockerSelectionGoal( "
                "initialContext, target, leaderContext, leader, leaderView, "
                "receipt, sourceRank, snapshot.serviceCellIdentity) "
                "\\/ \\E lowerRank \\in SetLessThan( blockerRank, "
                "AdequateLeaderFixedGlobalBlockerRankOrdering, "
                "AdequateLeaderFixedGlobalBlockerRankCarrier): "
                "AdequateLeaderFixedGlobalBlockerAtRank( initialContext, "
                "target, leaderContext, leader, leaderView, receipt, "
                "sourceRank, snapshot, clockValue, lowerRank)"
            ),
            forbidden=(
                "CHOOSE",
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontier",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerPending",
            required=(
                "AdequateLeaderFixedPipelineServiceRankFrontierForCell",
                "snapshot.serviceCellIdentity",
                "AdequateLeaderFixedGlobalBlockerSelectionGoal",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerEntryProvider",
            required=(
                "AdequateLeaderFixedPipelineServiceRankFrontierForCell",
                "AdequateLeaderFixedPipelineServiceCellIdentityCarrier",
                "AdequateLeaderFixedConfiguredGlobalBlockerSnapshot",
                "serviceCellIdentity",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerSelectionClosureProperty",
            required=(
                "AdequateLeaderFixedPipelineServiceRankFrontierForCell",
                "AdequateLeaderFixedPipelineServiceCellIdentityCarrier",
                "AdequateLeaderFixedGlobalBlockerSelectionGoal",
                "serviceCellIdentity",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedPipelineServiceRankFrontierHasExactCell",
            exact_statement=(
                "\\A initialContext, target, leaderContext, leader, "
                "leaderView, receipt, sourceRank: "
                "AdequateLeaderFixedPipelineServiceRankFrontier( "
                "initialContext, target, leaderContext, leader, leaderView, "
                "receipt, sourceRank) => \\E cellIdentity \\in "
                "AdequateLeaderFixedPipelineServiceCellIdentityCarrier: "
                "AdequateLeaderFixedPipelineServiceRankFrontierForCell( "
                "initialContext, target, leaderContext, leader, leaderView, "
                "receipt, sourceRank, cellIdentity)"
            ),
            required=(
                "AdequateLeaderFixedPipelineServiceRankFrontier",
                "AdequateLeaderFixedPipelineServiceRankFrontierForCell",
                "AdequateLeaderFixedPipelineServiceCellIdentityCarrier",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedSelectedExactCellProjectsToServiceFrontier",
            exact_statement=(
                "\\A initialContext, target, leaderContext, leader, "
                "leaderView, receipt, sourceRank, cellIdentity: "
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell( "
                "initialContext, target, leaderContext, leader, leaderView, "
                "receipt, sourceRank, cellIdentity) => "
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontier( "
                "initialContext, target, leaderContext, leader, leaderView, "
                "receipt, sourceRank)"
            ),
            required=(
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontierForCell",
                "AdequateLeaderFixedSelectedPipelineServiceRankFrontier",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedGlobalSelectionAndSelectedServiceSupplyPipelineRankDescent",
            exact_statement=(
                "\\A specification: "
                "/\\ AdequateLeaderFixedGlobalBlockerSelectionClosureProperty( "
                "specification) "
                "/\\ AdequateLeaderFixedSelectedPipelineServiceRankDescentProperty( "
                "specification) => "
                "AdequateLeaderFixedPipelineServiceRankDescentProperty("
                "specification)"
            ),
            required=(
                "AdequateLeaderFixedPipelineServiceRankFrontierHasExactCell",
                "AdequateLeaderFixedSelectedExactCellProjectsToServiceFrontier",
            ),
        )
        require_theorem(
            "AdequateLeaderFixedSelectedActionsCarryPipelineRank",
            exact_statement=(
                "/\\ AsyncStrongTypeInvariant "
                "/\\ AsyncProgressOwnershipInvariant "
                "/\\ AsyncCandidateServiceTombstoneLifecycleInvariant "
                "/\\ "
                "AsyncCandidateProducerContinuationExternalCoverageInvariant "
                "/\\ "
                "AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
                "=> /\\ AdequateLeaderFixedCutPerActionProvider "
                "/\\ AdequateLeaderFixedPreCandidateSelectedOwnerStepProvider "
                "/\\ "
                "AdequateLeaderFixedSelectedCandidateActionCarriesAbsoluteCeiling "
                "/\\ "
                "AdequateLeaderFixedSelectedEntryActionCarriesAbsoluteCeiling"
            ),
            required=(
                "AdequateLeaderFixedPipelineOriginHistoryFollowsAsyncStep",
                "AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut",
                "AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut",
                "AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget",
                "CandidateProducerContinuationFrozenOriginsCannotReplenish",
                "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
                "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
                "AdequateLeaderTargetEqualCountReplacementIntroducesAndRetires",
                "AdequateLeaderTargetCountIncreaseIntroducesOwnerIdentity",
            ),
        )
        for symbol in (
            "AsyncLiveProvidesAdequateLeaderFixedCutPerAction",
            "AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep",
            "AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry",
        ):
            require_theorem(
                symbol,
                required=(
                    "AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage",
                    "AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity",
                    "AdequateLeaderFixedSelectedActionsCarryPipelineRank",
                ),
            )
        require_theorem(
            "AdequateLeaderFixedGlobalBlockerFactsSupplyProviders",
            exact_statement=(
                "/\\ AsyncStrongTypeInvariant "
                "/\\ AsyncProgressOwnershipInvariant "
                "/\\ AsyncCandidateServiceTombstoneLifecycleInvariant "
                "/\\ "
                "AsyncCandidateProducerContinuationExternalCoverageInvariant "
                "/\\ "
                "AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
                "/\\ PostGstReplayQuarantineExcluded "
                "=> /\\ AdequateLeaderFixedGlobalBlockerEntryProvider "
                "/\\ AdequateLeaderFixedGlobalProducerEpisodeEntryProvider "
                "/\\ AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider "
                "/\\ "
                "AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider "
                "/\\ "
                "AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider "
                "/\\ "
                "AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider"
            ),
            required=(
                "ExactDecisionTargetNeutralSnapshotIsFinite",
                "ExactDecisionTargetNeutralEpisodeRankIsInCarrier",
                "ExactDecisionTargetNeutralActiveSnapshotConcreteRankIsInCarrier",
                "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
                "ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock",
                "ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock",
                "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
                "ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame",
                "ExactDecisionTargetNeutralSnapshotProducerEpisodeDoesNotReplenish",
                "ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank",
                "CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner",
                "CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends",
                "OverdueResponsivePacketEnablesConcreteProgress",
                "DueNodeServiceEnablesConcreteGateProgress",
                "HistoricalDiscoveryServeExitEitherLowersOrReplenishes",
                "HistoricalDiscoveryServeFairActionLowersOccurrenceDebt",
                "AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut",
                "AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut",
                "CandidateProducerContinuationFrozenOriginsCannotReplenish",
            ),
        )
        require_operator(
            "AdequateLeaderFixedGlobalBlockerProviderProperty",
            required=(
                "AdequateLeaderFixedGlobalBlockerEntryProvider",
                "AdequateLeaderFixedGlobalProducerEpisodeEntryProvider",
                "AdequateLeaderFixedGlobalBlockerConcreteOwnerProvider",
                "AdequateLeaderFixedGlobalBlockerSelectedOwnerStepProvider",
                "AdequateLeaderFixedGlobalBlockerRetainedEpisodeCarryProvider",
                "AdequateLeaderFixedGlobalBlockerBottomForcesGoalProvider",
                "AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty",
            ),
            forbidden=(
                "AdequateLeaderFixedGlobalBlockerOrdinalCeilingCarryProvider",
                "AdequateLeaderFixedGlobalBlockerLastOrdinalForcesGoalProvider",
            ),
        )
        require_theorem(
            "AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders",
            required=(
                "AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage",
                "AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity",
                (
                    "AsyncLiveProvidesAdequateLeaderRetainedProducer"
                    "NonDescentEpisodeStep"
                ),
                (
                    "AdequateLeaderFiniteRetainedProducerBudgetCloses"
                    "NonDescentEpisode"
                ),
                "AdequateLeaderFixedGlobalBlockerFactsSupplyProviders",
            ),
        )

        retired_authority_deadline_dependencies = (
            "AdequateLeaderFixedCommitQcStageCarrier",
            "AdequateLeaderFixedCommitQcDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcWalDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcReceivedReducerDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcDeliveryCandidateDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcIngressDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcPacketDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcRetainedDeadlineKernelProperty",
            "AdequateLeaderFixedCommitQcSixDeadlineKernelProperties",
            "AdequateLeaderAuthorityDeadlineReceiptCoverageInitProvider",
            "AdequateLeaderAuthorityDeadlineReceiptCoverageArmingProvider",
            (
                "AdequateLeaderAuthorityDeadlineReceiptCoverage"
                "PreservationOrRetirementProvider"
            ),
            "AdequateLeaderAuthorityDeadlineReachabilityInitProvider",
            "AdequateLeaderAuthorityDeadlineReachabilityStepProvider",
            "AdequateLeaderAuthorityDeadlineReachabilityArmingProvider",
            "AdequateLeaderAuthorityDeadlineQuantitativeProviderBundle",
            (
                "AdequateLeaderAuthorityDeadlineQuantitativeBundle"
                "SuppliesArbitraryActiveService"
            ),
            (
                "AdequateLeaderAuthorityDeadlineQuantitativeBundle"
                "SuppliesDecisionCarry"
            ),
            "AdequateLeaderFixedGlobalBlockerOrdinalCeilingCarryProvider",
            "AdequateLeaderFixedGlobalBlockerLastOrdinalForcesGoalProvider",
            "ExactDecisionTargetNeutralConcreteFixedClockRank",
            "ExactDecisionTargetNeutralPacketDependencyRank",
            "ExactDecisionTargetNeutralOrdinalCeilingsCarryUntilStrictRankGoal",
            "ExactDecisionTargetNeutralNonGoalEpisodeHasRemainingOrdinal",
            "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
            "ExactDecisionTargetNeutralLastOrdinalForcesStrictRankGoal",
            "candidateStart",
            "serveStart",
            "candidateCeiling",
            "serveCeiling",
        )
        source_tokens = set(tla_code_tokens(source))
        resurrected = tuple(
            token
            for token in retired_authority_deadline_dependencies
            if token in source_tokens
        )
        if resurrected:
            errors.append(
                f"{module}.tla: retired stored-receipt or staged CommitQC "
                "authority-deadline seams must not be resurrected; "
                f"present={resurrected!r}"
            )

        require_operator(
            "AdequateLeaderAuthorityDeadlineReceiptSet",
            exact=(
                "[deadlineReceipt: AsyncFixedCorridorDeadlineReceiptSet, "
                "subject: Subjects]"
            ),
            forbidden=("AsyncActiveFixedCorridorDeadlineReceipts",),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineReceipt",
            exact=(
                "[deadlineReceipt |-> AsyncFixedCorridorDeadlineReceipt( "
                "leader, leaderContext, leaderView, startTime), "
                "subject |-> subject]"
            ),
            forbidden=("AsyncActiveFixedCorridorDeadlineReceipts",),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow",
            exact=(
                "\\A node \\in "
                "AdequateLeaderFrozenResponsiveRoster(leaderContext): "
                "receipt.deadlineReceipt.deadline "
                "<= asyncNodeDeadlines[node]"
            ),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineFreshSelfWindowActive",
            exact=(
                "/\\ target = leader "
                "/\\ receipt \\in AdequateLeaderAuthorityDeadlineReceiptSet "
                "/\\ receipt.deadlineReceipt = "
                "AsyncFixedCorridorDeadlineReceipt( leader, leaderContext, "
                "leaderView, receipt.deadlineReceipt.armedAt) "
                "/\\ AdequateLeaderFrozenTargetCorridor( leader, "
                "leaderContext, leader, leaderView) "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow( "
                "leaderContext, receipt) "
                "/\\ receipt.deadlineReceipt.armedAt <= asyncNow "
                "/\\ asyncNow < receipt.deadlineReceipt.deadline"
            ),
            forbidden=(
                "AsyncActiveFixedCorridorDeadlineReceipts",
                "AdequateLeaderAuthorityDeadlineReceiptActive",
            ),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineFixedSubjectWindowActive",
            exact=(
                "/\\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive( "
                "target, leaderContext, leader, leaderView, receipt) "
                "/\\ AsyncProposalSubject(leader) = receipt.subject"
            ),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineFreshSource",
            exact=(
                "/\\ target = leader "
                "/\\ receipt = AdequateLeaderAuthorityDeadlineReceipt( "
                "leader, leaderContext, leaderView, "
                "receipt.deadlineReceipt.armedAt, "
                "AsyncProposalSubject(leader)) "
                "/\\ AdequateLeaderFixedCorridorDeadlineSource( leader, "
                "leaderContext, leaderView, "
                "receipt.deadlineReceipt.armedAt, "
                "receipt.deadlineReceipt.deadline) "
                "/\\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive( "
                "target, leaderContext, leader, leaderView, receipt)"
            ),
            forbidden=(
                "AsyncActiveFixedCorridorDeadlineReceipts",
                "AdequateLeaderAuthorityDeadlineReceiptActive",
            ),
        )
        require_theorem(
            "AdequateLeaderAuthorityDeadlineFreshSourceOwnsFrozenRosterWindow",
            exact_statement=(
                "\\A target, leaderContext, leader, leaderView, receipt: "
                "AdequateLeaderAuthorityDeadlineFreshSource( target, "
                "leaderContext, leader, leaderView, receipt) "
                "=> "
                "AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow( "
                "leaderContext, receipt)"
            ),
            required=(
                "AdequateLeaderAuthorityDeadlineFreshSource",
                "AdequateLeaderAuthorityDeadlineFreshSelfWindowActive",
                "AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow",
            ),
        )
        require_theorem(
            "AdequateLeaderFreshReceiptOwnsCompatibleConfiguredCeiling",
            exact_statement=(
                "\\A target, leaderContext, leader, leaderView, receipt: "
                "/\\ AdequateLeaderFixedConfiguredDeadlineCompatibility "
                "/\\ AdequateLeaderAuthorityDeadlineFreshSource( target, "
                "leaderContext, leader, leaderView, receipt) "
                "=> /\\ receipt.deadlineReceipt.deadline "
                "= receipt.deadlineReceipt.armedAt "
                "+ AsyncFixedCorridorServiceBudget + 1 "
                "/\\ receipt.deadlineReceipt.armedAt "
                "< receipt.deadlineReceipt.deadline "
                "/\\ receipt.deadlineReceipt.armedAt "
                "+ AdequateLeaderFixedLeaderPipelineClockBudget "
                "+ AdequateLeaderFixedCommitQcDisseminationClockBudget "
                "<= receipt.deadlineReceipt.deadline - 1"
            ),
            required=(
                "AdequateLeaderAuthorityDeadlineFreshSource",
                "AdequateLeaderAuthorityDeadlineReceipt",
                "AdequateLeaderAuthorityDeadlineFreshSelfWindowActive",
                "AdequateLeaderFixedCorridorDeadlineSource",
                "AsyncFixedCorridorDeadlineReceipt",
                "AdequateLeaderFixedConfiguredDeadlineCompatibility",
            ),
            forbidden=retired_authority_deadline_dependencies,
        )
        require_operator(
            "AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProvider",
            exact=(
                "\\A source, target \\in AsyncCurrentResponsiveVoters, "
                "qc \\in QcRecordSet: DecisionSourceAt(source, qc) "
                "=> \\/ NodeHasDecision(target) "
                "\\/ /\\ TimeoutDecisionKernelSource(source, target, qc) "
                "/\\ CommitCertificateDelivery(source, target, qc)"
            ),
        )
        require_theorem(
            "AsyncLiveProvidesAdequateLeaderFixedDecisionSourceTargetDeliveryRetention",
            exact_statement=(
                "\\A initialContext: "
                "AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProviderProperty( "
                "AsyncLiveSpecAt(initialContext))"
            ),
            required=(
                "AsyncLiveDecisionAuthorityRetainsExactTargetDelivery",
                "AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProviderProperty",
                "AdequateLeaderFixedDecisionSourceTargetDeliveryRetentionProvider",
            ),
        )

        require_operator(
            "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider",
            exact=(
                "\\A target \\in ValidatorIds: "
                "/\\ NodeHasDecision(target) "
                "/\\ [AsyncNext]_AsyncAllVars "
                "=> NodeHasDecision(target)'"
            ),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider",
            exact=(
                "\\A target \\in ValidatorIds, "
                "leaderContext \\in ContextRecords, "
                "leader \\in ValidatorIds, "
                "leaderView \\in Views, "
                "receipt \\in AdequateLeaderAuthorityDeadlineReceiptSet: "
                "/\\ AdequateLeaderFrozenTargetCorridor( target, "
                "leaderContext, leader, leaderView) "
                "/\\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive( "
                "target, leaderContext, leader, leaderView, receipt) "
                "/\\ ~NodeHasDecision(target) "
                "/\\ [AsyncNext]_AsyncAllVars "
                "/\\ asyncNow' < receipt.deadlineReceipt.deadline "
                "=> \\/ NodeHasDecision(target)' "
                "\\/ /\\ (AdequateLeaderFrozenTargetCorridor( target, "
                "leaderContext, leader, leaderView))' "
                "/\\ "
                "(AdequateLeaderAuthorityDeadlineFreshSelfWindowActive( "
                "target, leaderContext, leader, leaderView, receipt))'"
            ),
            forbidden=(
                "AsyncActiveFixedCorridorDeadlineReceipts",
                "AdequateLeaderAuthorityDeadlineReceiptActive",
            ),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty",
            exact=(
                "specification => []("
                "\\A target \\in ValidatorIds, "
                "leaderContext \\in ContextRecords, "
                "leader \\in ValidatorIds, "
                "leaderView \\in Views, "
                "receipt \\in AdequateLeaderAuthorityDeadlineReceiptSet: "
                "/\\ AdequateLeaderFrozenTargetCorridor( "
                "target, leaderContext, leader, leaderView) "
                "/\\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive( "
                "target, leaderContext, leader, leaderView, receipt) "
                "=> [](/\\ asyncNow < receipt.deadlineReceipt.deadline "
                "/\\ ~NodeHasDecision(target) "
                "=> /\\ AdequateLeaderFrozenTargetCorridor( "
                "target, leaderContext, leader, leaderView) "
                "/\\ AdequateLeaderAuthorityDeadlineFreshSelfWindowActive( "
                "target, leaderContext, leader, leaderView, receipt)))"
            ),
            forbidden=(
                "AsyncActiveFixedCorridorDeadlineReceipts",
                "AdequateLeaderAuthorityDeadlineReceiptActive",
            ),
        )

        require_theorem(
            "AdequateLeaderAuthorityDeadlineStepCarryPreventsPrematureExit",
            exact_statement=(
                "\\A specification: "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty( "
                "specification) "
                "=> "
                "AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty( "
                "specification)"
            ),
            required=(
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty",
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty",
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider",
                "AdequateLeaderAuthorityDeadlineNoPrematureExitSafetyProperty",
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider",
            ),
            forbidden=retired_authority_deadline_dependencies,
        )
        require_theorem(
            "AsyncSpecProvidesAdequateLeaderAuthorityDeadlineDecisionRetention",
            exact_statement=(
                "\\A initialContext: "
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty( "
                "AsyncSpecAt(initialContext))"
            ),
            required=(
                "AdequateLeaderAsyncBracketStepPreservesTargetDecision",
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty",
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProvider",
            ),
        )
        require_theorem(
            "AdequateLeaderAuthorityDeadlineWindowFollowsAsyncStep",
            exact_statement=(
                "/\\ AsyncStrongTypeInvariant "
                "/\\ AsyncProgressOwnershipInvariant "
                "/\\ AsyncCandidateServiceTombstoneLifecycleInvariant "
                "=> AdequateLeaderAuthorityDeadlineNoPrematureExitStepProvider"
            ),
            required=(
                "DualQuorumIntersectionHasHonest",
                "AdequateLeaderAsyncBracketStepPreservesTargetDecision",
                "AsyncFixedCorridorDeadlineAsyncNextClockIsMonotone",
                "AsyncBracketNextPreservesStrongTypeInvariant",
                "AsyncBracketNextPreservesProgressOwnership",
                "AsyncNextPreservesCandidateServiceTombstoneLifecycle",
                "AdequateLeaderAuthorityDeadlineFreshSelfWindowActive",
                "AdequateLeaderAuthorityDeadlineReceiptOwnsFrozenRosterWindow",
                "AdequateLeaderFrozenTargetCorridor",
                "AdequateLeaderFrozenResponsiveRoster",
            ),
            forbidden=retired_authority_deadline_dependencies,
        )
        require_theorem(
            "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineNoPrematureExitStep",
            exact_statement=(
                "\\A initialContext: "
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty( "
                "AsyncLiveSpecAt(initialContext))"
            ),
            required=(
                "AsyncLiveSpecProjectsAsyncSpec",
                "AsyncSpecAlwaysStrongTypeInvariant",
                "AsyncSpecAlwaysProgressOwnershipInvariant",
                "AsyncSpecAlwaysCandidateServiceTombstoneLifecycle",
                "AdequateLeaderAuthorityDeadlineWindowFollowsAsyncStep",
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty",
            ),
        )
        require_operator(
            "AdequateLeaderAuthorityDeadlineFreshSelfSafetyActionProviderProperties",
            exact=(
                "/\\ "
                "AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty( "
                "specification) "
                "/\\ AdequateLeaderFixedCutPerActionProviderProperty(specification) "
                "/\\ "
                "AdequateLeaderFixedSelectedActionClockCarryProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty( "
                "specification)"
            ),
            forbidden=retired_authority_deadline_dependencies,
        )
        require_theorem(
            "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineFreshSelfSafetyActions",
            exact_statement=(
                "\\A initialContext: "
                "AdequateLeaderAuthorityDeadlineFreshSelfSafetyActionProviderProperties( "
                "AsyncLiveSpecAt(initialContext))"
            ),
            required=(
                "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineImmediateSourceEntry",
                "AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry",
                "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory",
                "AsyncLiveProvidesAdequateLeaderFixedCutPerAction",
                "AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry",
                "AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep",
                "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineNoPrematureExitStep",
                "AdequateLeaderAuthorityDeadlineFreshSelfSafetyActionProviderProperties",
            ),
        )
        bundle = require_operator(
            "AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle",
            exact=(
                "/\\ ModelConfiguration "
                "/\\ AdequateLeaderFixedConfiguredPipelineBudgetCompatibility "
                "/\\ AdequateLeaderFixedConfiguredDeadlineCompatibility "
                "/\\ (specification => []AsyncStrongTypeInvariant) "
                "/\\ AdequateLeaderAsyncNextBehaviorProperty(specification) "
                "/\\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification) "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineImmediateSourceEntryProviderProperty( "
                "specification) "
                "/\\ AdequateLeaderFixedSubjectReplacementProviderProperties( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPipelineTokenOwnershipAndTailCarryProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPipelineOriginHistoryAndNoResurrectionProviderProperty( "
                "specification) "
                "/\\ AdequateLeaderFixedCutPerActionProviderProperty(specification) "
                "/\\ "
                "AdequateLeaderFixedSelectedActionClockCarryProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPreCandidateSelectedOwnerStepProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderRetainedProducerNonDescentEpisodeClosureProperty( "
                "specification) "
                "/\\ AdequateLeaderFixedGlobalBlockerProviderProperty(specification) "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineNoPrematureExitStepProviderProperty( "
                "specification) "
                "/\\ "
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty( "
                "specification)"
            ),
            forbidden=(
                "AdequateLeaderFixedSelectedOwnerServiceProperty",
                "AdequateLeaderFixedPreCandidateEntryServiceProperty",
                "AdequateLeaderFixedPipelineServiceRankClosureProperty",
                "AdequateLeaderFixedCorridorDeadlineServiceProperty",
                "AdequateLeaderAuthorityDeadlineStoredReceiptServiceProperty",
                "AdequateLeaderAuthorityDeadlineActiveReceiptDecisionCarryProperty",
                "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
                "AsyncActiveFixedCorridorDeadlineReceipts",
                *retired_authority_deadline_dependencies,
            ),
        )
        if bundle is not None and _tla_dependency_present(
            bundle,
            "AsyncLiveSpecAt",
        ):
            errors.append(
                f"{module}.tla: conditional quantitative provider bundle "
                "may not disguise an unproved AsyncLive supplier as a "
                "primitive conjunct"
            )

        require_theorem(
            "AsyncLiveSpecSuppliesAdequateLeaderConfiguredBudget",
            exact_statement=(
                "\\A initialContext: AsyncLiveSpecAt(initialContext) "
                "=> /\\ ModelConfiguration "
                "/\\ AdequateLeaderFixedConfiguredPipelineBudgetCompatibility "
                "/\\ AdequateLeaderFixedConfiguredDeadlineCompatibility"
            ),
            required=(
                "AdequateLeaderFixedConfiguredBudgetCompatibilityIsDischarged",
                "AsyncLiveSpecAt",
                "AsyncSpecAt",
                "AsyncInitAt",
                "AsyncBaseInitAt",
                "InitAt",
            ),
        )
        require_theorem(
            "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineDecisionRetention",
            exact_statement=(
                "\\A initialContext: "
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty( "
                "AsyncLiveSpecAt(initialContext))"
            ),
            required=(
                "AsyncLiveSpecProjectsAsyncSpec",
                "AsyncSpecProvidesAdequateLeaderAuthorityDeadlineDecisionRetention",
                "AdequateLeaderAuthorityDeadlineDecisionRetentionStepProviderProperty",
            ),
        )
        require_theorem(
            (
                "AsyncLiveSpecSuppliesAdequateLeaderAuthorityDeadline"
                "FreshSelfQuantitativeProviderBundle"
            ),
            exact_statement=(
                "\\A initialContext: AsyncLiveSpecAt(initialContext) "
                "=> "
                "AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle( "
                "AsyncLiveSpecAt(initialContext))"
            ),
            required=(
                "AsyncLiveSpecSuppliesAdequateLeaderConfiguredBudget",
                "AsyncLiveSpecProjectsAsyncSpec",
                "AsyncSpecAlwaysStrongTypeInvariant",
                "AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness",
                "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineImmediateSourceEntry",
                "AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementProviders",
                "AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry",
                "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory",
                "AsyncLiveProvidesAdequateLeaderFixedCutPerAction",
                "AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry",
                "AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep",
                (
                    "AsyncLiveProvidesAdequateLeaderFixedPipelineOrigin"
                    "NonDescentEpisodeStep"
                ),
                (
                    "AsyncLiveProvidesAdequateLeaderRetainedProducer"
                    "NonDescentEpisodeStep"
                ),
                (
                    "AdequateLeaderFiniteRetainedProducerBudgetCloses"
                    "NonDescentEpisode"
                ),
                "AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders",
                (
                    "AsyncLiveProvidesAdequateLeaderAuthorityDeadline"
                    "NoPrematureExitStep"
                ),
                (
                    "AsyncLiveProvidesAdequateLeaderAuthorityDeadline"
                    "DecisionRetention"
                ),
                (
                    "AdequateLeaderAuthorityDeadlineFreshSelf"
                    "QuantitativeProviderBundle"
                ),
            ),
            forbidden=(
                "AdequateLeaderFixedSelectedOwnerServiceProperty",
                "AdequateLeaderFixedPreCandidateEntryServiceProperty",
                "AdequateLeaderFixedCorridorDeadlineServiceProperty",
                "AdequateLeaderAuthorityDeadlineStoredReceiptServiceProperty",
                "AdequateLeaderAuthorityDeadlineActiveReceiptDecisionCarryProperty",
                "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
                "AsyncActiveFixedCorridorDeadlineReceipts",
                *retired_authority_deadline_dependencies,
            ),
        )

    def check_historical_local_import_lineage_residual() -> None:
        """Require the exact three-arm local-import provenance composition."""

        module = "SumeragiV2HistoricalRecoveryTemporalClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        retired_residuals = (
            "IndexedHistoricalCertificateReceivedQcLocalImportResidualProperty",
            "IndexedHistoricalCertificateDecisionWalLocalImportResidualProperty",
            "IndexedHistoricalCertificateLocalImportPhysicalKernelsCloseCandidateEntry",
        )
        for retired in retired_residuals:
            if re.search(rf"\b{re.escape(retired)}\b", stripped):
                errors.append(
                    f"{module}.tla: retired proofless local-import residual "
                    f"{retired} must not be resurrected"
                )

        generic_provider = (
            "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry"
        )
        generic_claimants: list[tuple[str, int]] = []
        declarations = re.finditer(
            r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"([A-Za-z_][A-Za-z0-9_]*)\s*==",
            stripped,
        )
        for declaration in declarations:
            symbol = declaration.group(1)
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )
            statement = " ".join(parts[0].split())
            if "=>" not in statement:
                continue
            _, consequent = statement.rsplit("=>", 1)
            if (
                "IndexedHistoricalCertificateLocalImportCandidateEntryProperty"
                in consequent
            ):
                generic_claimants.append((symbol, line))

        if [symbol for symbol, _ in generic_claimants] != [generic_provider]:
            errors.append(
                f"{module}.tla: exact local-import candidate entry must have "
                f"the sole reviewed theorem provider {generic_provider}; "
                f"found {generic_claimants!r}"
            )

        provider = _top_level_theorem_body(
            source,
            generic_provider,
            preserve_string_contents=True,
        )
        if provider is None:
            return
        body, line = provider
        marker = re.search(r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body)
        proof = "" if marker is None else body[marker.start() :]
        required_physical_providers = (
            "IndexedChainSpecClosesHistoricalCertificateReceivedQcLocalImportEntry",
            "IndexedChainSpecClosesHistoricalCertificateDecisionWalLocalImportEntry",
            "IndexedChainSpecClosesHistoricalCertificateExactCommandLocalImport",
            "IndexedHistoricalCertificateLocalImportSplitsPhysicalSources",
        )
        missing = tuple(
            dependency
            for dependency in required_physical_providers
            if not _tla_dependency_present(proof, dependency)
        )
        if missing:
            errors.append(
                f"{module}.tla:{line}: {generic_provider} must compose the "
                "received-QC, non-rebroadcast Decision-WAL, and exact-command "
                f"physical provenance providers; missing={missing!r}"
            )

    def check_dedicated_open_historical_recovery_branch() -> None:
        module = "SumeragiV2ProgressWitnessFinalClosureProofs"
        symbol = "AsyncNonRunnerPreservesFinalProgressWitnessClosure"
        source = module_sources.get(module)
        if source is None:
            return
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            return
        body, line = extracted
        branch = re.search(
            r"(?ms)^[ \t]*<2>3\.\s+CASE\s+\\E\s+node\s+\\in\s+"
            r"ValidatorIds:\s*OpenHistoricalRecovery\(node\).*?"
            r"(?=^[ \t]*<2>4\.\s+CASE\b)",
            body,
        )
        expected = (
            "<2>3. CASE \\E node \\in ValidatorIds: "
            "OpenHistoricalRecovery(node) "
            "BY <1>1, <2>3, "
            "OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure"
        )
        found = None if branch is None else " ".join(branch.group(0).split())
        if found != expected:
            errors.append(
                f"{module}.tla:{line}: {symbol} must keep "
                "OpenHistoricalRecovery on its dedicated preservation branch "
                "and may not fold it into the monotone-carrier frame path; "
                f"expected {expected!r}, found {found!r}"
            )

    def check_exact_target_neutral_contract() -> None:
        module = "SumeragiV2ExactDecisionStageServiceClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return
        actual_extends = _module_extends(source)
        if actual_extends != EXACT_TARGET_NEUTRAL_REVIEWED_EXTENDS:
            errors.append(
                f"{module}.tla must EXTEND exactly "
                f"{list(EXACT_TARGET_NEUTRAL_REVIEWED_EXTENDS)!r}; "
                f"found {list(actual_extends)!r}"
            )

        observed_operators = set(
            re.findall(
                r"(?m)^(ExactDecisionTargetNeutral[A-Za-z0-9_]*)"
                r"\s*(?:\(|==)",
                strip_tla_comments(source),
            )
        )
        operator_contract_tokens: list[str] = []
        for symbol in sorted(observed_operators):
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            operator_contract_tokens.append(symbol)
            operator_contract_tokens.extend(tla_code_tokens(extracted[0]))
        operator_contract_sha256 = hashlib.sha256(
            "\0".join(operator_contract_tokens).encode("utf-8")
        ).hexdigest()
        if (
            len(observed_operators)
            != EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_COUNT
            or operator_contract_sha256
            != EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_SHA256
        ):
            errors.append(
                f"{module}.tla target-neutral operator inventory/body "
                "contract must stay fully token-sealed; "
                f"expected_count={EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_COUNT}, "
                f"found_count={len(observed_operators)}, "
                f"expected_sha256={EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_SHA256}, "
                f"found_sha256={operator_contract_sha256}"
            )

        observed_theorems = set(
            re.findall(
                r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)"
                r"[ \t]+(ExactDecisionTargetNeutral[A-Za-z0-9_]*)"
                r"\s*(?:\(|==)",
                strip_tla_comments(source),
            )
        )
        theorem_contract_tokens: list[str] = []
        for symbol in sorted(observed_theorems):
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, _line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            theorem_contract_tokens.append(symbol)
            theorem_contract_tokens.extend(tla_code_tokens(statement))
        theorem_contract_sha256 = hashlib.sha256(
            "\0".join(theorem_contract_tokens).encode("utf-8")
        ).hexdigest()
        if (
            len(observed_theorems)
            != EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_COUNT
            or theorem_contract_sha256
            != EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_SHA256
        ):
            errors.append(
                f"{module}.tla target-neutral theorem inventory/statement "
                "contract must stay fully token-sealed; "
                f"expected_count={EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_COUNT}, "
                f"found_count={len(observed_theorems)}, "
                f"expected_sha256={EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_SHA256}, "
                f"found_sha256={theorem_contract_sha256}"
            )

        stripped_source = strip_tla_comments(
            source,
            preserve_string_contents=True,
        )
        for retired_symbol in EXACT_TARGET_NEUTRAL_RETIRED_SYMBOLS:
            if _symbol_exists(stripped_source, retired_symbol):
                errors.append(
                    f"{module}.tla: retired target-neutral compatibility "
                    f"symbol {retired_symbol} may not be restored"
                )

        retired_snapshot_tokens = (
            "candidateStart",
            "serveStart",
            "candidateCeiling",
            "serveCeiling",
            "AsyncCandidateProducerEpisodeBudget",
            "AsyncServeLifecycleFamilyBudget",
        )
        target_neutral_tokens: set[str] = set()
        for symbol in (*sorted(observed_operators), *sorted(observed_theorems)):
            extractor = (
                _top_level_operator_body
                if symbol in observed_operators
                else _top_level_theorem_body
            )
            extracted = extractor(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is not None:
                target_neutral_tokens.update(tla_code_tokens(extracted[0]))
        present_retired_snapshot_tokens = tuple(
            token
            for token in retired_snapshot_tokens
            if token in target_neutral_tokens
        )
        if present_retired_snapshot_tokens:
            errors.append(
                f"{module}.tla: target-neutral release rank may not restore "
                "the synthetic candidateStart/serveStart/ceiling budget "
                "kernel; "
                f"found={present_retired_snapshot_tokens!r}"
            )

        def require_target_operator_tokens(
            symbol: str,
            required: tuple[str, ...],
            *,
            forbidden: tuple[str, ...] = (),
        ) -> None:
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{module}.tla: missing exact target-neutral rank "
                    f"operator {symbol}"
                )
                return
            body, line = extracted
            tokens = set(tla_code_tokens(body))
            missing = tuple(token for token in required if token not in tokens)
            present = tuple(token for token in forbidden if token in tokens)
            if missing or present:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must retain the frozen "
                    "past scheduler cut and exact causal-occurrence/Serve "
                    f"rank contract; missing={missing!r}, forbidden={present!r}"
                )

        current_cut = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralCurrentPastSchedulerCut",
            preserve_string_contents=True,
        )
        expected_current_cut = (
            "AsyncNextCandidateLifecycleOrdinal(node) - 1"
        )
        if current_cut is None or " ".join(current_cut[0].split()) != expected_current_cut:
            line = "?" if current_cut is None else str(current_cut[1])
            errors.append(
                f"{module}.tla:{line}: "
                "ExactDecisionTargetNeutralCurrentPastSchedulerCut must be "
                "the immutable predecessor of the current lifecycle "
                "high-watermark"
            )

        # Leader-wire logical age is not a physical producer allowance.  The
        # proofless rank freezes the receiver-local physical admission cut and
        # charges only records which were already Active strictly below that
        # cut.  A packetless Dormant record contributes zero; a later replay
        # receives an ordinal at or above the frozen cut and therefore cannot
        # recharge this coordinate after the outer packet rank descends.
        current_physical_cuts = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralCurrentPhysicalCuts",
            preserve_string_contents=True,
        )
        expected_physical_cuts = (
            "[node \\in Responsive |-> AsyncNextIngressPhysicalOrdinal(node)]"
        )
        if (
            current_physical_cuts is None
            or " ".join(current_physical_cuts[0].split())
            != expected_physical_cuts
        ):
            line = (
                "?"
                if current_physical_cuts is None
                else str(current_physical_cuts[1])
            )
            errors.append(
                f"{module}.tla:{line}: "
                "ExactDecisionTargetNeutralCurrentPhysicalCuts must freeze "
                "the receiver-local physical admission high-watermark"
            )

        frozen_active_leader_wire = _top_level_operator_body(
            source,
            (
                "ExactDecisionTargetNeutralFrozenActiveLeaderWireRecords"
                "ForSnapshot"
            ),
            preserve_string_contents=True,
        )
        if frozen_active_leader_wire is None:
            errors.append(
                f"{module}.tla: missing exact target-neutral frozen active "
                "leader-wire physical-prefix operator"
            )
        else:
            body, line = frozen_active_leader_wire
            tokens = set(tla_code_tokens(body))
            required = (
                "asyncLeaderWireLifecycles",
                "recipient",
                "schedulerOrdinal",
                "schedulerCuts",
                "AsyncLeaderWireLifecycleActive",
                "physicalAdmissionOrdinal",
                "physicalCuts",
            )
            missing = tuple(token for token in required if token not in tokens)
            normalized = " ".join(body.split())
            required_fragments = (
                "record.schedulerOrdinal <= snapshot.schedulerCuts[node]",
                "record.physicalAdmissionOrdinal < snapshot.physicalCuts[node]",
            )
            missing_fragments = tuple(
                fragment
                for fragment in required_fragments
                if fragment not in normalized
            )
            forbidden = tuple(
                token
                for token in (
                    "AsyncLeaderWireLifecycleDormant",
                    "AsyncLeaderWireRetryableDormant",
                    "AsyncLeaderWireActionInertDormant",
                    "RetryableItems",
                )
                if token in tokens
            )
            if missing or missing_fragments or forbidden:
                errors.append(
                    f"{module}.tla:{line}: frozen leader-wire proofless debt "
                    "must charge only Active records strictly below the "
                    "frozen physical admission cut; packetless Dormant and "
                    "post-cut admissions contribute zero; "
                    f"missing={missing!r}, missing_fragments="
                    f"{missing_fragments!r}, forbidden={forbidden!r}"
                )

        leader_wire_stage = _top_level_operator_body(
            source,
            (
                "ExactDecisionTargetNeutralLeaderWireRemainingStage"
                "ForSnapshot"
            ),
            preserve_string_contents=True,
        )
        if leader_wire_stage is None:
            errors.append(
                f"{module}.tla: missing exact target-neutral leader-wire "
                "remaining-stage operator"
            )
        else:
            body, line = leader_wire_stage
            normalized = " ".join(body.split())
            required_fragments = (
                "record \\in "
                "ExactDecisionTargetNeutralFrozenActiveLeaderWireRecordsForSnapshot( "
                "snapshot, node)",
                "AsyncLeaderWireLifecycleIngressProtected(record) -> 1",
                "[] OTHER -> 0",
            )
            missing = tuple(
                fragment
                for fragment in required_fragments
                if fragment not in normalized
            )
            forbidden_tokens = tuple(
                token
                for token in (
                    "AsyncLeaderWireLifecycleDormant",
                    "AsyncFrozenLeaderWireBarrierRemainingStage",
                )
                if token in tla_code_tokens(body)
            )
            if missing or forbidden_tokens or re.search(r"->\s*2\b", body):
                errors.append(
                    f"{module}.tla:{line}: leader-wire stage debt must be one "
                    "only for a frozen Active ingress-protected carrier and "
                    "zero otherwise; all-Dormant positive debt is forbidden; "
                    f"missing={missing!r}, forbidden={forbidden_tokens!r}"
                )

        chargeable_leader_wire_candidates = _top_level_operator_body(
            source,
            (
                "ExactDecisionTargetNeutralChargeableLeaderWireCandidates"
                "ForSnapshot"
            ),
            preserve_string_contents=True,
        )
        if chargeable_leader_wire_candidates is None:
            errors.append(
                f"{module}.tla: missing exact target-neutral chargeable "
                "leader-wire candidate operator"
            )
        else:
            body, line = chargeable_leader_wire_candidates
            tokens = set(tla_code_tokens(body))
            required = (
                "AsyncLeaderWireRuntimeCandidate",
                (
                    "ExactDecisionTargetNeutralFrozenActiveLeaderWireRecords"
                    "ForSnapshot"
                ),
            )
            forbidden = (
                "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
                "AsyncLeaderWireLifecycleDormant",
            )
            missing = tuple(token for token in required if token not in tokens)
            present = tuple(token for token in forbidden if token in tokens)
            if missing or present:
                errors.append(
                    f"{module}.tla:{line}: latent leader-wire Candidates may "
                    "enter the proofless rank only through an Active pre-cut "
                    "physical record; missing="
                    f"{missing!r}, forbidden={present!r}"
                )

        proofless_candidate_owners = _top_level_operator_body(
            source,
            (
                "ExactDecisionTargetNeutralProoflessCandidateOwners"
                "ForSnapshot"
            ),
            preserve_string_contents=True,
        )
        if proofless_candidate_owners is not None:
            body, line = proofless_candidate_owners
            tokens = set(tla_code_tokens(body))
            required = (
                "ExactDecisionTargetNeutralCausalCandidatesForSnapshot",
                "ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot",
                "ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot",
                "ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot",
            )
            missing = tuple(token for token in required if token not in tokens)
            forbidden = tuple(token for token in (
                "AsyncCausalEpisodeCandidates",
                "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates",
                "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
            ) if token in tokens)
            if missing or forbidden:
                errors.append(
                    f"{module}.tla:{line}: proofless candidate ownership must "
                    "use the exact pre-cut causal, continuation, leader-wire, "
                    "and ordinary-ingress projections, not generic latent sets; "
                    f"missing={missing!r}, forbidden_present="
                    f"{forbidden!r}"
                )

        rejected_persistent_producer_tokens = tuple(
            token
            for token, present in (
                (
                    '"LeaderWireProducer"',
                    '"LeaderWireProducer"' in stripped_source,
                ),
                (
                    "ExactDecisionTargetNeutralLiveRetainedLeaderWireProducer"
                    "IdentitySet",
                    (
                        "ExactDecisionTargetNeutralLiveRetainedLeaderWireProducer"
                        "IdentitySet"
                    )
                    in target_neutral_tokens,
                ),
                (
                    "ExactDecisionTargetNeutralRetainedLeaderWireProducer"
                    "IdentitiesForSnapshot",
                    (
                        "ExactDecisionTargetNeutralRetainedLeaderWireProducer"
                        "IdentitiesForSnapshot"
                    )
                    in target_neutral_tokens,
                ),
                (
                    "ExactDecisionTargetNeutralRetainedLeaderWireProducer"
                    "Prepaid",
                    (
                        "ExactDecisionTargetNeutralRetainedLeaderWireProducer"
                        "Prepaid"
                    )
                    in target_neutral_tokens,
                ),
            )
            if present
        )
        if rejected_persistent_producer_tokens:
            errors.append(
                f"{module}.tla: rejected persistent/prepaid leader-wire "
                "producer tags may recharge after packet policy-drop; use "
                "only the frozen physical admission cut; present="
                f"{rejected_persistent_producer_tokens!r}"
            )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralCandidateOwnersForSnapshot",
            (
                "AsyncCandidateLifecycleOrdinal",
                "schedulerCuts",
                "ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut",
                "ExactDecisionTargetNeutralCausalRoot",
                "candidateRoots",
                "ProtectedCandidateOwned",
            ),
            forbidden=("candidateStart", "candidateCeiling"),
        )
        candidate_owners = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralCandidateOwnersForSnapshot",
            preserve_string_contents=True,
        )
        if candidate_owners is not None:
            body, line = candidate_owners
            normalized = " ".join(body.split())
            required_fragments = (
                "AsyncCandidateLifecycleOrdinal(candidate) <= "
                "snapshot.schedulerCuts[recipient]",
                "ExactDecisionTargetNeutralCausalRoot( candidate.node, "
                "candidate.causalOrigin) \\in snapshot.candidateRoots",
            )
            missing = tuple(
                fragment for fragment in required_fragments if fragment not in normalized
            )
            if missing:
                errors.append(
                    f"{module}.tla:{line}: "
                    "ExactDecisionTargetNeutralCandidateOwnersForSnapshot "
                    "must exclude future same-root descendants using both "
                    f"the frozen cut and causal-root set; missing={missing!r}"
                )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot",
            (
                (
                    "ExactDecisionTargetNeutralExactCandidateOccurrenceBudget"
                    "ForSnapshot"
                ),
                "ExactDecisionTargetNeutralServeWorkBudgetForSnapshot",
                "ExactDecisionTargetNeutralServeReachDebtForSnapshot",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralCausalCandidatesForSnapshot",
            (
                "AsyncCausalEpisodeCandidates",
                "schedulerCuts",
                "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot",
            ),
        )
        causal_candidates = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralCausalCandidatesForSnapshot",
            preserve_string_contents=True,
        )
        if causal_candidates is not None:
            body, line = causal_candidates
            normalized = " ".join(body.split())
            required_fragments = (
                "AsyncCausalEpisodeCandidates(node, snapshot.schedulerCuts[node])",
                "candidate.causalOrigin \\in "
                "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot( "
                "snapshot, node)",
            )
            missing = tuple(
                fragment for fragment in required_fragments if fragment not in normalized
            )
            if missing:
                errors.append(
                    f"{module}.tla:{line}: "
                    "ExactDecisionTargetNeutralCausalCandidatesForSnapshot "
                    "frozen causal candidates must "
                    "retain both the immutable scheduler cut and predecessor "
                    f"origin set; missing={missing!r}"
                )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot",
            (
                "ExactDecisionTargetNeutralCausalCandidatesForSnapshot",
                "AsyncCausalExactRemainingOccurrenceBudget",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot",
            (
                "AsyncCausalEpisodeServeIngressIdentities",
                "schedulerCuts",
                "AsyncServeIngressAdmissionOrdinal",
                "physicalCuts",
            ),
        )
        serve_ingress_identities = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot",
            preserve_string_contents=True,
        )
        if serve_ingress_identities is not None:
            body, line = serve_ingress_identities
            normalized = " ".join(body.split())
            required_fragments = (
                "AsyncCausalEpisodeServeIngressIdentities( node, "
                "snapshot.schedulerCuts[node])",
                "AsyncServeIngressAdmissionOrdinal(node, identity) < "
                "snapshot.physicalCuts[node]",
            )
            missing = tuple(
                fragment for fragment in required_fragments if fragment not in normalized
            )
            if missing:
                errors.append(
                    f"{module}.tla:{line}: "
                    "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot "
                    "frozen Serve identities must "
                    "retain both the immutable scheduler prefix and strict "
                    f"physical admission cut; missing={missing!r}"
                )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralServeWorkTokensForSnapshot",
            (
                "AsyncCausalEpisodeServeWorkTokens",
                "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralServeReachDebtForSnapshot",
            (
                "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot",
                "DrainableIngressTurnReachRank",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralFixedPredecessorSetForSnapshot",
            (
                "ExactDecisionTargetNeutralSchedulerCutTokens",
                "candidateIdentities",
                "serveIdentities",
            ),
            forbidden=("candidateStart", "serveStart"),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralFixedClockSnapshot",
            (
                "producerRequests",
                "ingressEpisodes",
                "candidateRoots",
                "schedulerCuts",
                "physicalCuts",
                "predecessors",
                "ExactDecisionTargetNeutralFrozenCausalRoots",
                "ExactDecisionTargetNeutralSchedulerCutTokens",
                "ExactDecisionTargetNeutralCurrentPhysicalCuts",
            ),
            forbidden=(
                "candidateStart",
                "serveStart",
                "candidateCeiling",
                "serveCeiling",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralFrozenSnapshotCarriers",
            (
                "schedulerCuts",
                "physicalCuts",
                "predecessors",
            ),
            forbidden=(
                "candidateStart",
                "serveStart",
                "candidateCeiling",
                "serveCeiling",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralSnapshotActive",
            (
                "schedulerCuts",
                "physicalCuts",
                "AsyncNextCandidateLifecycleOrdinal",
                "AsyncNextIngressPhysicalOrdinal",
                "candidateRoots",
                "ExactDecisionTargetNeutralRetainedCausalRootsForSnapshot",
                "ExactDecisionTargetNeutralSchedulerCutTokens",
            ),
            forbidden=(
                "candidateStart",
                "serveStart",
                "candidateCeiling",
                "serveCeiling",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralProducerEpisodeCarrier",
            (
                "Nat",
                "ExactDecisionTargetNeutralComposedCausalEpisodeCarrier",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralProducerEpisodeOrdering",
            (
                "LexPairOrdering",
                "ExactDecisionTargetNeutralComposedCausalEpisodeOrdering",
                "ExactDecisionTargetNeutralComposedCausalEpisodeCarrier",
                "OpToRel",
            ),
        )
        require_target_operator_tokens(
            "ExactDecisionTargetNeutralRankCellOutcome",
            (
                "SetLessThan",
                "ExactDecisionTargetNeutralProducerEpisodeOrdering",
                "ExactDecisionTargetNeutralProducerEpisodeCarrier",
            ),
        )

        fair_owner_kinds = {
            "ResolveLocalCandidateProducerContinuation": (
                "PostGstResolveLocalCandidateProducerContinuation",
                "LocalCandidateProducerContinuationResolutionUsesReviewedFairAction",
            ),
            "ServiceConditionalTransportProducerContinuation": (
                "PostGstServiceConditionalTransportProducerContinuation",
                "ConditionalTransportProducerContinuationServiceUsesReviewedFairAction",
            ),
            "ServiceVolatileBodyProducerContinuation": (
                "PostGstServiceVolatileBodyProducerContinuation",
                "VolatileBodyProducerContinuationServiceUsesReviewedFairAction",
            ),
        }
        fair_owner_set = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralFairOwnerSet",
            preserve_string_contents=True,
        )
        fair_action = _top_level_operator_body(
            source,
            "ExactDecisionTargetNeutralFairAction",
            preserve_string_contents=True,
        )
        fairness_theorem = _top_level_theorem_body(
            source,
            "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
            preserve_string_contents=True,
        )
        if fair_owner_set is not None:
            body, line = fair_owner_set
            for owner_kind in fair_owner_kinds:
                if body.count(f'"{owner_kind}"') != 1:
                    errors.append(
                        f"{module}.tla:{line}: "
                        "ExactDecisionTargetNeutralFairOwnerSet must include "
                        f"exactly one individually fair {owner_kind} owner"
                    )
            if body.count("node \\in AsyncVotersAt(initialContext)") < 4:
                errors.append(
                    f"{module}.tla:{line}: producer-continuation fair owners "
                    "must range over AsyncVotersAt(initialContext)"
                )
        if fair_action is not None:
            body, line = fair_action
            normalized = " ".join(body.split())
            for owner_kind, (action, _provider) in fair_owner_kinds.items():
                expected = (
                    f'owner.ownerKind = "{owner_kind}" -> '
                    f"{action}(owner.node)"
                )
                if expected not in normalized:
                    errors.append(
                        f"{module}.tla:{line}: "
                        "ExactDecisionTargetNeutralFairAction must map "
                        f"{owner_kind} only to {action}(owner.node)"
                    )
            if "[] OTHER -> FALSE" not in normalized:
                errors.append(
                    f"{module}.tla:{line}: "
                    "ExactDecisionTargetNeutralFairAction must reject unknown "
                    "owner kinds with OTHER -> FALSE"
                )
        if fairness_theorem is not None:
            body, line = fairness_theorem
            for owner_kind, (_action, provider) in fair_owner_kinds.items():
                if not _tla_dependency_present(body, provider):
                    errors.append(
                        f"{module}.tla:{line}: "
                        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness "
                        f"must cite {provider} for {owner_kind}"
                    )

        allowed_fairness_theorems = {
            "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
            "ExactDecisionTargetNeutralFairEpisodeStep",
        }
        for symbol in sorted(observed_operators):
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            for token in EXACT_TARGET_NEUTRAL_FORBIDDEN_TOKENS:
                if re.search(rf"\b{re.escape(token)}\b", body):
                    errors.append(
                        f"{module}.tla:{line}: {symbol} may not depend on "
                        f"non-local or circular target-neutral shortcut {token}"
                    )
        for symbol in sorted(observed_theorems):
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            for token in EXACT_TARGET_NEUTRAL_FORBIDDEN_TOKENS:
                if re.search(rf"\b{re.escape(token)}\b", body):
                    errors.append(
                        f"{module}.tla:{line}: {symbol} may not depend on "
                        f"non-local or circular target-neutral shortcut {token}"
                    )
            if (
                symbol not in allowed_fairness_theorems
                and re.search(r"\b(?:WF|SF)_AsyncAllVars\b", body)
            ):
                errors.append(
                    f"{module}.tla:{line}: {symbol} may not introduce "
                    "fairness outside the reviewed target-neutral owner "
                    "composition"
                )

        # Producer episodes are no longer natural-number allowances.  The
        # exact ingress count is paired with the complete causal-occurrence
        # rank, so every consumer must stay in the reviewed product carrier
        # and use its well-founded ordering.  A stale ``budget \in Nat`` (or
        # numeric comparison of that budget) can make the downstream theorem
        # vacuous because no tuple-valued state satisfies its antecedent.
        producer_carrier = (
            "ExactDecisionTargetNeutralProducerEpisodeCarrier"
        )
        producer_ordering = (
            "ExactDecisionTargetNeutralProducerEpisodeOrdering"
        )
        producer_path_tokens = {
            "ExactDecisionTargetNeutralProducerEpisodeAtBudget",
            "ExactDecisionTargetNeutralProducerEpisodeBudget",
            "ExactDecisionTargetNeutralProducerEpisodeRank",
            "ExactDecisionTargetNeutralProducerEpisodeBottom",
            "ExactDecisionTargetNeutralRankCellOutcome",
        }
        theorem_bodies: dict[str, tuple[str, int]] = {}
        for symbol in sorted(observed_theorems):
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            theorem_bodies[symbol] = (body, line)
            tokens = set(tla_code_tokens(body))
            if producer_path_tokens.isdisjoint(tokens):
                continue

            if re.search(r"\bbudget\s*\\in\s*Nat\b", body):
                errors.append(
                    f"{module}.tla:{line}: {symbol} must quantify the tuple "
                    f"producer budget in {producer_carrier}, not Nat"
                )

            # Only flag comparisons whose operand is the producer budget (or
            # its rank expression); clock, scheduler-cut, and physical ordinal
            # comparisons in the same theorem remain legitimate Naturals.
            body_tokens = tla_code_tokens(body)
            producer_value_tokens = {
                "ExactDecisionTargetNeutralProducerEpisodeBudget",
                "ExactDecisionTargetNeutralProducerEpisodeRank",
            }
            numeric_budget_comparison = False
            for index, token in enumerate(body_tokens):
                if token not in {"<", "<=", ">", ">="}:
                    continue
                # The tokenizer intentionally keeps punctuation simple.  Do
                # not mistake proof-step labels (`<2>`) or function arrows
                # (`Responsive -> Nat`) for numeric comparisons.
                if (
                    token == "<"
                    and index + 2 < len(body_tokens)
                    and body_tokens[index + 1].isdigit()
                    and body_tokens[index + 2] == ">"
                ):
                    continue
                if token == ">" and index > 0 and body_tokens[index - 1] == "-":
                    continue
                before = body_tokens[max(0, index - 10) : index]
                after = body_tokens[index + 1 : index + 11]
                if (
                    any(name in before[-2:] for name in ("budget", "lowerBudget"))
                    or any(name in after[:2] for name in ("budget", "lowerBudget"))
                    or not producer_value_tokens.isdisjoint(before)
                    or not producer_value_tokens.isdisjoint(after)
                ):
                    numeric_budget_comparison = True
                    break
            if numeric_budget_comparison:
                errors.append(
                    f"{module}.tla:{line}: {symbol} may not compare the "
                    "tuple producer budget with numeric <, <=, >, or >=; "
                    f"use SetLessThan with {producer_ordering} and "
                    f"{producer_carrier} (or membership in that ordering)"
                )

            producer_nat_conclusion = re.search(
                r"ExactDecisionTargetNeutralProducerEpisode(?:Budget|Rank)"
                r"\s*\([^)]*\)\s*'?\s*\\in\s*Nat\b",
                body,
            )
            if producer_nat_conclusion is not None:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must type every producer "
                    f"episode result in {producer_carrier}, not Nat"
                )

            # Any theorem which carries a named producer-cell budget must make
            # the product carrier explicit.  Bottom-cell theorems pass the
            # concrete nested bottom and therefore have no `budget` variable.
            if (
                "ExactDecisionTargetNeutralProducerEpisodeAtBudget" in tokens
                and "budget" in tokens
                and not re.search(
                    rf"\bbudget\s*\\in\s*{re.escape(producer_carrier)}\b",
                    body,
                )
            ):
                errors.append(
                    f"{module}.tla:{line}: {symbol} carries a producer-cell "
                    f"budget without exact {producer_carrier} membership"
                )

        # Materialized identity-set containment is only a resurrection
        # diagnostic.  It cannot establish non-ascent for same-root causal
        # descendants that were not materialized at snapshot time.  Keep a
        # separate exact-occurrence structural theorem and require the
        # composed producer step—and therefore the temporal rank-cell path—to
        # consume it.
        structural_symbol = (
            "ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame"
        )
        structural = theorem_bodies.get(structural_symbol)
        if structural is None:
            errors.append(
                f"{module}.tla: exact target-neutral producer closure must "
                f"declare {structural_symbol}"
            )
        else:
            body, line = structural
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            required_structural_tokens = (
                "ExactDecisionTargetNeutralCausalEpisodeRankForSnapshot",
                "AsyncCausalEpisodeStructuralRankOrdering",
                "AsyncNext",
            )
            missing = tuple(
                token
                for token in required_structural_tokens
                if token not in tla_code_tokens(statement)
            )
            if missing or "=" not in tla_code_tokens(statement):
                errors.append(
                    f"{module}.tla:{line}: {structural_symbol} must state "
                    "exact causal-occurrence descent-or-frame, not only "
                    "materialized identity-set containment; "
                    f"missing={missing!r}"
                )
            proof = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )
            proof_text = "" if len(proof) == 1 else proof[1]
            required_rank_definitions = (
                "ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot",
                "ExactDecisionTargetNeutralServeWorkBudgetForSnapshot",
                "ExactDecisionTargetNeutralServeReachDebtForSnapshot",
                "AsyncCausalEpisodeStructuralRankOrdering",
            )
            missing = tuple(
                token
                for token in required_rank_definitions
                if token not in tla_code_tokens(proof_text)
            )
            if missing:
                errors.append(
                    f"{module}.tla:{line}: {structural_symbol} must unfold "
                    "the exact candidate-occurrence and Serve-work rank; "
                    f"missing={missing!r}"
                )

        composed_symbol = (
            "ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame"
        )
        composed = theorem_bodies.get(composed_symbol)
        if composed is not None and not _tla_dependency_present(
            composed[0], structural_symbol
        ):
            errors.append(
                f"{module}.tla:{composed[1]}: {composed_symbol} must depend "
                f"on {structural_symbol}; materialized identity-set subset "
                "alone does not close causal non-ascent"
            )

        retained_symbol = (
            "ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish"
        )
        retained = theorem_bodies.get(retained_symbol)
        structural_provider_symbols = {
            structural_symbol,
            composed_symbol,
            "ExactDecisionTargetNeutralProducerEpisodeStepIsDescentOrFrame",
        }
        if retained is not None and not any(
            _tla_dependency_present(retained[0], dependency)
            for dependency in structural_provider_symbols
        ):
            errors.append(
                f"{module}.tla:{retained[1]}: {retained_symbol} must consume "
                "the exact-occurrence structural non-ascent provider; "
                "materialized identity-set subset alone is insufficient"
            )

    def check_adequate_candidate_retirement_provider_contract() -> None:
        """Require the positive A -> B -> A serviced-memory provider chain."""

        module = "SumeragiV2AdequateLeaderServiceClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        forbidden_theorems = (
            "AdequateLeaderCandidateRetirementInstallsServicedMemory",
            "AdequateLeaderCandidateRetirementStartsClosedMemory",
            "AdequateLeaderServicedCandidateIdentityHasTerminalWitness",
            "AdequateLeaderCandidateSuccessfulServiceRetirementReclaimsMarker",
        )
        for symbol in forbidden_theorems:
            if _symbol_exists(stripped, symbol, theorem_only=True):
                errors.append(
                    f"{module}.tla: removed unsound candidate-retirement "
                    f"provider {symbol} may not be restored"
                )

        retired_nonterminal_residual = (
            "AdequateLeaderTargetCandidateNonterminalRetirementMemoryResidualProperty"
        )
        if _symbol_exists(stripped, retired_nonterminal_residual):
            errors.append(
                f"{module}.tla: retired proofless candidate-memory residual "
                f"{retired_nonterminal_residual} may not replace the proved "
                "successful-service memory property"
            )
        for forbidden_token in (
            "AsyncCandidateAppliedMarkerReclaimableAfterIn",
            "AsyncCandidateSuccessfulServiceReclaimsTransientMarker",
        ):
            if forbidden_token in tla_code_tokens(stripped):
                errors.append(
                    f"{module}.tla: successful candidate service must retain "
                    f"positive memory until strict exit; forbidden reclaimed-"
                    f"marker dependency {forbidden_token}"
                )
        negated_memory = re.search(
            r"~\s*AsyncCandidateServiceTombstoned\s*\(",
            stripped,
        )
        if negated_memory is not None:
            line = stripped.count("\n", 0, negated_memory.start()) + 1
            errors.append(
                f"{module}.tla:{line}: successful candidate service may not "
                "derive negated serviced memory"
            )

        providers_by_property: dict[str, list[str]] = {
            "AdequateLeaderTargetCandidateIdentityTombstoneProperty": [],
            "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty": [],
            "AdequateLeaderTargetCandidateTerminalTombstoneProperty": [],
        }
        declarations = re.finditer(
            r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"([A-Za-z_][A-Za-z0-9_]*)\s*==",
            stripped,
        )
        for declaration in declarations:
            symbol = declaration.group(1)
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, _line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            statement_tokens = tla_code_tokens(statement)
            for property_name in providers_by_property:
                if property_name in statement_tokens:
                    providers_by_property[property_name].append(symbol)

        expected_providers = {
            "AdequateLeaderTargetCandidateIdentityTombstoneProperty": [
                "AsyncSpecProvidesAdequateLeaderTargetCandidateIdentityTombstones"
            ],
            "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty": [
                "AsyncSpecProvidesAdequateLeaderTargetCandidateSuccessfulServiceMemory"
            ],
            "AdequateLeaderTargetCandidateTerminalTombstoneProperty": [
                "AsyncSpecProvidesAdequateLeaderTargetCandidateTerminalTombstones"
            ],
        }
        for property_name, expected in expected_providers.items():
            observed = providers_by_property[property_name]
            if observed != expected:
                errors.append(
                    f"{module}.tla: {property_name} must have sole reviewed "
                    f"positive provider {expected[0]}; found {observed!r}"
                )

    def check_retained_lock_proofless_provider_contract() -> None:
        """Keep every local retained-lock provider conditional and unpromoted."""

        module = "SumeragiV2LockedBodyReproposalProgressProofs"
        source = module_sources.get(module)
        if source is None:
            return
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        corridor_leaves = (
            "RetainedLockSourceAuthorityExposureProperty",
            "RetainedLockPrepareAuthorityTransportProperty",
            "RetainedLockTargetLeaderFreshActivationProperty",
            "RetainedLockLeaderProducerOriginProperty",
        )
        producer_leaves = (
            "RetainedLockSameOriginLifecycleDispositionClosureProperty",
            "RetainedLockCrossOriginProducerReplacementClosureProperty",
            "RetainedLockProducerExactReentryClosureProperty",
        )
        legacy_leaf = "RetainedLockRankHandoffProperty"
        tracked_properties = (
            *corridor_leaves,
            *producer_leaves,
            legacy_leaf,
        )
        theorem_users: dict[str, list[str]] = {
            property_name: [] for property_name in tracked_properties
        }
        for declaration in re.finditer(
            r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"([A-Za-z_][A-Za-z0-9_]*)\s*==",
            stripped,
        ):
            symbol = declaration.group(1)
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, _line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            statement_tokens = tla_code_tokens(statement)
            for property_name in tracked_properties:
                if property_name in statement_tokens:
                    theorem_users[property_name].append(symbol)

        conditional_compositor = (
            "RetainedLockSeparatedProducerProvidersCloseEpisodeResidual"
        )
        corridor_compositors = [
            "DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader",
            "DirectRetainedLockOwnerNeutralDecompositionReachesHigherClosure",
        ]
        for proofless_leaf in corridor_leaves:
            if theorem_users[proofless_leaf] != corridor_compositors:
                errors.append(
                    f"{module}.tla: {proofless_leaf} must remain a proofless "
                    "premise used only by the reviewed conditional corridor "
                    f"compositors {corridor_compositors!r}; theorem users="
                    f"{theorem_users[proofless_leaf]!r}"
                )

        for proofless_leaf in producer_leaves:
            expected = [conditional_compositor]
            if theorem_users[proofless_leaf] != expected:
                errors.append(
                    f"{module}.tla: {proofless_leaf} must remain a proofless "
                    "premise used only by conditional compositor "
                    f"{conditional_compositor}; theorem users="
                    f"{theorem_users[proofless_leaf]!r}"
                )

        expected_legacy_users = [
            "RetainedLockRankHandoffClosesExactOrigin",
            "RetainedLockRankHandoffClosesRankedFrontier",
        ]
        if theorem_users[legacy_leaf] != expected_legacy_users:
            errors.append(
                f"{module}.tla: RetainedLockRankHandoffProperty is the legacy "
                "conditional compatibility surface and may not be promoted; "
                f"theorem users must be {expected_legacy_users!r}, found "
                f"{theorem_users[legacy_leaf]!r}"
            )

    def check_historical_nonpacket_provider_contract() -> None:
        """Pin the local fairness closure as the sole non-packet provider."""

        module = "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        providers: list[str] = []
        for declaration in re.finditer(
            r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"([A-Za-z_][A-Za-z0-9_]*)\s*==",
            stripped,
        ):
            symbol = declaration.group(1)
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, _line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                body,
                maxsplit=1,
            )[0]
            if (
                "IndexedHistoricalFixedClockNonPacketServiceProperty"
                in tla_code_tokens(statement)
            ):
                providers.append(symbol)
        expected = [
            "IndexedChainSpecClosesHistoricalFixedClockNonPacketService"
        ]
        if providers != expected:
            errors.append(
                f"{module}.tla: IndexedHistoricalFixedClockNonPacketServiceProperty "
                f"must have sole reviewed provider {expected[0]}; found "
                f"{providers!r}"
            )

    def check_timeout_direct_residual_scope() -> None:
        """Reject a finite-counter assumption hidden in timeout liveness."""

        module = "SumeragiV2TimeoutViewProgressProofs"
        source = module_sources.get(module)
        if source is None:
            return
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        match = re.search(r"\bAsyncInstallGenerationBudget\b", stripped)
        if match is not None:
            line = stripped.count("\n", 0, match.start()) + 1
            errors.append(
                f"{module}.tla:{line}: timeout liveness may not assume the "
                "diagnostic AsyncInstallGenerationBudget; live generations "
                "are Nat and production physical nonexhaustion must derive "
                "from strict same-view Prepare-rank ascent"
            )

    def check_retained_lock_unbounded_view_release_scope() -> None:
        """Keep finite TLC view bounds and circular closure out of reproposal."""

        lower_module = "SumeragiV2LockedBodyReproposalProgressProofs"
        lower_source = module_sources.get(lower_module)
        if lower_source is not None:
            stripped = strip_tla_comments(
                lower_source,
                preserve_string_contents=True,
            )
            for forbidden in (
                "AsyncMaximumView",
                "RetainedLockLeaderViewEpisodeAtBudget",
                "RetainedLockFiniteViewBudgetClosesLeaderEpisodes",
            ):
                match = re.search(rf"\b{re.escape(forbidden)}\b", stripped)
                if match is not None:
                    line = stripped.count("\n", 0, match.start()) + 1
                    errors.append(
                        f"{lower_module}.tla:{line}: retained-lock liveness "
                        "may not turn the configured/TLC maximum view into a "
                        f"production rank; found {forbidden}"
                    )

        adequate_module = "SumeragiV2AdequateLeaderServiceClosureProofs"
        adequate_source = module_sources.get(adequate_module)
        if adequate_source is not None:
            stripped = strip_tla_comments(
                adequate_source,
                preserve_string_contents=True,
            )
            for forbidden in (
                "LockedBodyReproposalProgressProperty",
                "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
                "ResponsiveDecisionConvergenceClosesLockedBodyReproposal",
            ):
                match = re.search(rf"\b{re.escape(forbidden)}\b", stripped)
                if match is not None:
                    line = stripped.count("\n", 0, match.start()) + 1
                    errors.append(
                        f"{adequate_module}.tla:{line}: adequate-leader "
                        "convergence may not consume the retained-lock release "
                        f"theorem and create a circular proof; found {forbidden}"
                    )

        temporal_module = "SumeragiV2AsyncTemporalClosureProofs"
        temporal_source = module_sources.get(temporal_module)
        if temporal_source is None:
            return
        for symbol in (
            "AsyncTemporalClosureRotatingLeaderProgressObligation",
            "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
        ):
            extracted = _top_level_theorem_body(
                temporal_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            tokens = tla_code_tokens(body)
            for forbidden in (
                "RotatingLeaderProgressObligation",
                "LockedBodyReproposalProgressObligation",
            ):
                if forbidden in tokens:
                    errors.append(
                        f"{temporal_module}.tla:{line}: {symbol} may not "
                        "substitute the proofless compatibility facade for "
                        f"its reviewed closure proof; found {forbidden}"
                    )
        for symbol in (
            "ResponsiveDecisionConvergenceClosesLockedBodyReproposal",
            "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
        ):
            extracted = _top_level_theorem_body(
                temporal_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            body, line = extracted
            tokens = tla_code_tokens(body)
            for forbidden in (
                "DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader",
                "DirectRetainedLockOwnerNeutralDecompositionReachesHigherClosure",
                "RetainedLockOutcomeOrHigherLeaderProgressProperty",
                "RetainedLockSourceAuthorityExposureProperty",
                "RetainedLockPrepareAuthorityTransportProperty",
                "RetainedLockTargetLeaderFreshActivationProperty",
                "RetainedLockLeaderProducerOriginProperty",
                "RetainedLockRankHandoffProperty",
                "RetainedLockSameOriginLifecycleDispositionClosureProperty",
                "RetainedLockCrossOriginProducerReplacementClosureProperty",
                "RetainedLockProducerExactReentryClosureProperty",
                "RetainedLockProducerNonDescentEpisodeClosureProperty",
                "RetainedLockOwnerNeutralRankHandoffProperty",
                "RetainedLockSeparatedProducerProvidersCloseEpisodeResidual",
                "RetainedLockNonDescentClosureClosesRankHandoff",
                "RetainedLockSeparatedProducerProvidersCloseOwnerNeutralRankHandoff",
                "RetainedLockOwnerNeutralRankHandoffClosesFixedCorridor",
                "RetainedLockOwnerNeutralRankHandoffClosesRankedFrontier",
                "RetainedLockRankHandoffClosesExactOrigin",
                "RetainedLockRankHandoffClosesRankedFrontier",
                "RetainedLockLeaderViewEpisodeAtBudget",
            ):
                if forbidden in tokens:
                    errors.append(
                        f"{temporal_module}.tla:{line}: {symbol} must close "
                        "only after independent rotating-leader convergence, "
                        f"not through a bounded or still-conditional shard; "
                        f"found {forbidden}"
                    )

    def check_application_release_scope() -> None:
        """Keep the aggregate application facade out of the release theorem."""

        module = "SumeragiV2AsyncTemporalClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return
        symbol = "AsyncTemporalClosureApplicationCompletionProgressObligation"
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            return
        body, line = extracted
        tokens = tla_code_tokens(body)
        forbidden = tuple(
            token
            for token in (
                "ApplicationCompletionProgressObligation",
                "ApplicationLivenessObligation",
                "OneHeightCompletionObligation",
                "ResponsiveDecisionApplicationFromExactCorridor",
            )
            if token in tokens
        )
        if forbidden:
            errors.append(
                f"{module}.tla:{line}: {symbol} must consume the exact "
                "Decision-stage service proof rather than an aggregate or "
                f"compatibility facade; found={forbidden!r}"
            )

    def check_install_generation_budget_is_diagnostic_only() -> None:
        """Keep the checked counter boundary out of every liveness claim."""

        diagnostic_operator = (
            "SumeragiV2AsyncNetwork",
            "AsyncInstallGenerationBudget",
        )
        diagnostic_theorem = (
            "SumeragiV2AsyncProgressOwnershipProofs",
            "InstallGenerationBudgetExcludesExhaustion",
        )
        forbidden_tokens = (
            "AsyncInstallGenerationBudget",
            "IndexedInstallGenerationBudgetPremise",
            "InstallGenerationBudgetExcludesExhaustion",
            "RecoveryGenerationBudget",
            "AsyncGstEventually",
        )
        for module, source in module_sources.items():
            stripped = strip_tla_comments(
                source,
                preserve_string_contents=True,
            )
            for symbol in re.findall(
                r"(?m)^([A-Za-z_][A-Za-z0-9_]*)\s*"
                r"(?:\([^)=\n]*\))?\s*==",
                stripped,
            ):
                extracted = _top_level_operator_body(
                    source,
                    symbol,
                    preserve_string_contents=True,
                )
                if extracted is None or (module, symbol) == diagnostic_operator:
                    continue
                body, line = extracted
                present = tuple(
                    token
                    for token in forbidden_tokens
                    if token in tla_code_tokens(body)
                )
                if present:
                    errors.append(
                        f"{module}.tla:{line}: {symbol} may not turn the "
                        "diagnostic install-generation/restart boundary into "
                        f"a liveness premise or manufacture GST; found={present!r}"
                    )
            for declaration in re.finditer(
                r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)"
                r"[ \t]+([A-Za-z_][A-Za-z0-9_]*)\s*==",
                stripped,
            ):
                symbol = declaration.group(1)
                extracted = _top_level_theorem_body(
                    source,
                    symbol,
                    preserve_string_contents=True,
                )
                if extracted is None or (module, symbol) == diagnostic_theorem:
                    continue
                body, line = extracted
                present = tuple(
                    token
                    for token in forbidden_tokens
                    if token in tla_code_tokens(body)
                )
                if present:
                    errors.append(
                        f"{module}.tla:{line}: {symbol} may not assume or "
                        "derive liveness or GST from the diagnostic install-"
                        f"generation/restart boundary; found={present!r}"
                    )

    def check_exact_replenishment_compatibility_surface() -> None:
        """Keep the diagnostic replenishment union out of convergence premises."""

        module = "SumeragiV2ExactDecisionStageServiceClosureProofs"
        source = module_sources.get(module)
        if source is None:
            return
        token = "ExactDecisionRequestIngressRankReplenishmentResidual"
        stripped = strip_tla_comments(source, preserve_string_contents=True)
        theorem_users: list[str] = []
        theorem_names = set(
            re.findall(
                r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)"
                r"[ \t]+([A-Za-z_][A-Za-z0-9_]*)\s*==",
                stripped,
            )
        )
        for symbol in sorted(theorem_names):
            extracted = _top_level_theorem_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is not None and token in tla_code_tokens(extracted[0]):
                theorem_users.append(symbol)
        expected_theorem_users = [
            "ExactDecisionRequestIngressReplenishmentHasConcreteActionWitness"
        ]
        if theorem_users != expected_theorem_users:
            errors.append(
                f"{module}.tla: {token} is a compatibility action-witness "
                "surface, not a lifecycle-convergence premise; theorem users "
                f"must be {expected_theorem_users!r}, found {theorem_users!r}"
            )

        operator_users: list[str] = []
        for symbol in re.findall(
            r"(?m)^([A-Za-z_][A-Za-z0-9_]*)\s*"
            r"(?:\([^)=\n]*\))?\s*==",
            stripped,
        ):
            if symbol == token:
                continue
            extracted = _top_level_operator_body(
                source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is not None and token in tla_code_tokens(extracted[0]):
                operator_users.append(symbol)
        if operator_users:
            errors.append(
                f"{module}.tla: {token} may not feed another property or "
                f"rank premise; operator users={operator_users!r}"
            )

    for obligation_id, symbol in ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS.items():
        property_wrapper = ARBITRARY_CONTEXT_SAFETY_PROPERTY_WRAPPERS[obligation_id]
        check_direct_theorem(
            obligation_id,
            symbol,
            module="SumeragiV2Proofs",
            required_spec="CoreSpecAt",
            forbidden=("Spec", "NextV2", "AdvanceContext"),
            exact_statement=(
                f"\\A initialContext: "
                f"{property_wrapper}(CoreSpecAt(initialContext))"
            ),
        )
    for obligation_id, symbol in ASYNC_LIVENESS_OBLIGATIONS.items():
        property_wrapper = ASYNC_LIVENESS_PROPERTY_WRAPPERS[obligation_id]
        required_spec = (
            "AsyncLiveSpecAt"
            if obligation_id
            in {
                "timeout-view-liveness",
                "rotating-leader-liveness",
                "locked-body-reproposal",
            }
            else "AsyncSpecAt"
        )
        exact_statement = ASYNC_LIVENESS_EXACT_STATEMENTS.get(
            obligation_id,
            f"\\A initialContext: "
            f"{property_wrapper}({required_spec}(initialContext))",
        )
        check_direct_theorem(
            obligation_id,
            symbol,
            module=ASYNC_LIVENESS_OBLIGATION_MODULES.get(
                obligation_id,
                "SumeragiV2AsyncLivenessProofs",
            ),
            required_spec=required_spec,
            forbidden=("Spec", "NextV2", "AdvanceContext"),
            exact_statement=exact_statement,
        )
    check_direct_theorem(
        "progress-witness-preservation",
        "ProgressWitnessObligation",
        module="SumeragiV2AsyncTemporalClosureProofs",
        required_spec="AsyncSpecAt",
        forbidden=("Spec", "NextV2", "AdvanceContext"),
        exact_statement=(
            "\\A initialContext: "
            "AsyncProgressWitnessAndHistoricalRecoveryProperty("
            "AsyncSpecAt(initialContext))"
        ),
    )
    for obligation_id, (symbol, property_wrapper) in CHAIN_SAFETY_OBLIGATIONS.items():
        check_closed_theorem(
            obligation_id,
            symbol,
            module="SumeragiV2ChainEpochProofs",
            exact_statement=f"{property_wrapper}(ChainEpochSpec)",
            forbidden=(
                "AsyncSpec",
                "Spec",
                "NextV2",
                "AdvanceContext",
                "CommonAppliedSubject",
            ),
        )
    for obligation_id, exact_statement in (
        EXACT_FIXED_PROOF_OBLIGATION_STATEMENTS.items()
    ):
        target = FIXED_PROOF_OBLIGATION_TARGETS.get(
            obligation_id,
            SUPPORT_PROOF_OBLIGATION_INVENTORY.get(obligation_id),
        )
        if target is None:
            errors.append(
                f"exact fixed proof contract has no reviewed target "
                f"{obligation_id}"
            )
            continue
        module, symbol = target
        check_closed_theorem(
            obligation_id,
            symbol,
            module=module,
            exact_statement=exact_statement,
            forbidden=(),
        )
    for (
        module,
        symbol,
    ), exact_body in EXACT_FIXED_PROOF_PROPERTY_OPERATOR_BODIES.items():
        if (
            module == "SumeragiV2ExactDecisionStageServiceClosureProofs"
            and symbol.startswith("ExactDecisionTargetNeutral")
        ):
            continue
        check_exact_operator(module, symbol, exact_body)
    for (
        module,
        symbol,
    ), exact_statement in EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS.items():
        if (
            module == "SumeragiV2ExactDecisionStageServiceClosureProofs"
            and symbol.startswith("ExactDecisionTargetNeutral")
        ):
            continue
        check_exact_supporting_theorem(module, symbol, exact_statement)
    for (
        module,
        symbol,
    ), required_tokens in FIXED_PROOF_REQUIRED_PROOF_TOKENS.items():
        if (
            module == "SumeragiV2ExactDecisionStageServiceClosureProofs"
            and symbol.startswith("ExactDecisionTargetNeutral")
        ):
            continue
        check_theorem_proof_tokens(module, symbol, required_tokens)
    for (
        module,
        symbol,
    ), forbidden_tokens in FIXED_PROOF_FORBIDDEN_PROOF_TOKENS.items():
        if (
            module == "SumeragiV2ExactDecisionStageServiceClosureProofs"
            and symbol.startswith("ExactDecisionTargetNeutral")
        ):
            continue
        check_theorem_forbidden_proof_tokens(
            module,
            symbol,
            forbidden_tokens,
        )
    for symbol, required_tokens in (
        EXACT_TARGET_NEUTRAL_REQUIRED_PROOF_TOKENS.items()
    ):
        check_theorem_proof_tokens(
            "SumeragiV2ExactDecisionStageServiceClosureProofs",
            symbol,
            required_tokens,
        )
    check_historical_import_and_projection_contract()
    check_historical_finite_runner_provider_contract()
    check_historical_local_authority_provider_contract()
    check_action_temporal_repair_contracts()
    check_adequate_leader_authority_deadline_contract()
    check_historical_local_import_lineage_residual()
    check_adequate_candidate_retirement_provider_contract()
    check_retained_lock_proofless_provider_contract()
    check_historical_nonpacket_provider_contract()
    check_timeout_direct_residual_scope()
    check_retained_lock_unbounded_view_release_scope()
    check_application_release_scope()
    check_install_generation_budget_is_diagnostic_only()
    check_exact_replenishment_compatibility_surface()
    exact_historical_physical_boundaries = {
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs": (
            "IndexedHistoricalFixedClockExactResidualsEstablishPrerequisiteSurface",
            "IndexedHistoricalCommitTransportKernelsCloseExactLeaf",
            "IndexedHistoricalDecisionTransportKernelsCloseExactLeaf",
        ),
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs": (
            "IndexedHistoricalFixedClockExactResidualsCloseCertificateDiscoveryRank",
            "IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual",
            "IndexedHistoricalDecisionPhysicalKernelsCloseTargetRank",
            "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
        ),
    }
    compatibility_fixed_clock_tokens = (
        "IndexedHistoricalFixedClockTemporalLeafProperties",
        "IndexedHistoricalFixedClockLeavesEstablishPrerequisiteSurface",
        "IndexedHistoricalFixedClockLeavesCloseCertificateDiscoveryRank",
        "HistoricalTemporalFixedClockLeaves",
    )
    for module, symbols in exact_historical_physical_boundaries.items():
        source = module_sources.get(module)
        if source is None:
            continue
        for symbol in symbols:
            theorem = _top_level_theorem_body(
                source, symbol, preserve_string_contents=True
            )
            if theorem is None:
                continue
            body, line = theorem
            forbidden = [
                token
                for token in compatibility_fixed_clock_tokens
                if _tla_dependency_present(body, token)
            ]
            if forbidden:
                errors.append(
                    f"{module}.tla:{line}: {symbol} must consume the exact "
                    "five-group historical physical residual inventory, not "
                    "the broad fixed-clock compatibility surface; "
                    f"forbidden={forbidden!r}"
                )
    check_dedicated_open_historical_recovery_branch()
    check_exact_target_neutral_contract()
    for module, forbidden_tokens in FIXED_PROOF_FORBIDDEN_MODULE_TOKENS.items():
        source = module_sources.get(module)
        if source is None:
            continue
        stripped = strip_tla_comments(
            source,
            preserve_string_contents=True,
        )
        for token in forbidden_tokens:
            match = re.search(rf"\b{re.escape(token)}\b", stripped)
            if match is None:
                continue
            line = stripped.count("\n", 0, match.start()) + 1
            errors.append(
                f"{module}.tla:{line}: release proof may not derive receipt "
                f"agreement from chain-history projection {token}"
            )
    return errors
