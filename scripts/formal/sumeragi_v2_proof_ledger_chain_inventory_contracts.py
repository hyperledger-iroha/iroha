def _chain_source_fidelity_errors(formal_dir: Path) -> list[str]:
    """Keep chain composition per-node and independent of the old global barrier."""

    chain_path = formal_dir / "SumeragiV2ChainEpoch.tla"
    proof_path = formal_dir / "SumeragiV2ChainEpochProofs.tla"
    refinement_path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    errors: list[str] = []
    core_fields = HISTORICAL_INDEXED_CORE_FIELDS
    scheduler_fields = HISTORICAL_INDEXED_SCHEDULER_FIELDS
    scheduler_arity = len(scheduler_fields)
    recovery_fields = HISTORICAL_INDEXED_RECOVERY_FIELDS
    recovery_arity = len(recovery_fields)
    producer_fields = HISTORICAL_INDEXED_PRODUCER_FIELDS
    producer_arity = len(producer_fields)
    node_service_deadline_slot = scheduler_fields.index(
        "asyncNodeServiceDeadlines"
    ) + 1

    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if async_path.is_file():
        async_source = async_path.read_text(encoding="utf-8")
        async_scheduler = _top_level_operator_body(
            async_source, "AsyncSchedulerVars"
        )
        if async_scheduler is None:
            errors.append(f"{async_path}: missing AsyncSchedulerVars")
        else:
            body, line = async_scheduler
            tuple_match = re.fullmatch(r"\s*<<(.+)>>\s*", body, re.DOTALL)
            actual_scheduler_fields = (
                ()
                if tuple_match is None
                else tuple(
                    field.strip()
                    for field in tuple_match.group(1).split(",")
                )
            )
            if actual_scheduler_fields != scheduler_fields:
                errors.append(
                    f"{async_path}:{line}: AsyncSchedulerVars must match the "
                    "chain projection's exact ordered scheduler tuple; found "
                    f"{actual_scheduler_fields!r}"
                )
        async_recovery = _top_level_operator_body(async_source, "AsyncRecoveryVars")
        if async_recovery is None:
            errors.append(f"{async_path}: missing AsyncRecoveryVars")
        else:
            body, line = async_recovery
            tuple_match = re.fullmatch(r"\s*<<(.+)>>\s*", body, re.DOTALL)
            actual_recovery_fields = (
                ()
                if tuple_match is None
                else tuple(
                    field.strip() for field in tuple_match.group(1).split(",")
                )
            )
            if actual_recovery_fields != recovery_fields:
                errors.append(
                    f"{async_path}:{line}: AsyncRecoveryVars must match the "
                    "chain projection's exact ordered recovery tuple; found "
                    f"{actual_recovery_fields!r}"
                )
        async_producer = _top_level_operator_body(async_source, "AsyncProducerVars")
        if async_producer is None:
            errors.append(f"{async_path}: missing AsyncProducerVars")
        else:
            body, line = async_producer
            tuple_match = re.fullmatch(r"\s*<<(.+)>>\s*", body, re.DOTALL)
            actual_producer_fields = (
                ()
                if tuple_match is None
                else tuple(
                    field.strip() for field in tuple_match.group(1).split(",")
                )
            )
            if actual_producer_fields != producer_fields:
                errors.append(
                    f"{async_path}:{line}: AsyncProducerVars must match the "
                    "chain projection's exact ordered producer-journal tuple; "
                    f"found {actual_producer_fields!r}"
                )
        async_all_vars = _top_level_operator_body(async_source, "AsyncAllVars")
        expected_async_all_vars = (
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, "
            "AsyncProducerVars, asyncFixedCorridorDeadlines>>"
        )
        if async_all_vars is None:
            errors.append(f"{async_path}: missing AsyncAllVars")
        else:
            body, line = async_all_vars
            normalized = " ".join(body.split())
            if normalized != expected_async_all_vars:
                errors.append(
                    f"{async_path}:{line}: AsyncAllVars must equal only "
                    f"{expected_async_all_vars!r}; found {normalized!r}"
                )
        async_original_all_vars = _top_level_operator_body(
            async_source, "AsyncOriginalAllVars"
        )
        expected_async_original_all_vars = (
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, "
            "AsyncProducerVars>>"
        )
        if async_original_all_vars is None:
            errors.append(f"{async_path}: missing AsyncOriginalAllVars")
        else:
            body, line = async_original_all_vars
            normalized = " ".join(body.split())
            if normalized != expected_async_original_all_vars:
                errors.append(
                    f"{async_path}:{line}: AsyncOriginalAllVars must equal only "
                    f"{expected_async_original_all_vars!r}; "
                    f"found {normalized!r}"
                )

    if chain_path.is_file():
        raw_chain_source = chain_path.read_text(encoding="utf-8")
        source = strip_tla_comments(raw_chain_source)
        header = re.search(r"(?m)^EXTENDS\s+(.+)$", source)
        extended_modules = (
            set()
            if header is None
            else {module.strip() for module in header.group(1).split(",")}
        )
        if "SumeragiV2Core" not in extended_modules:
            errors.append(
                f"{chain_path}: chain/epoch state must extend SumeragiV2Core directly"
            )
        if re.search(r"\bSumeragiV2Reconfiguration\b", source):
            errors.append(
                f"{chain_path}: chain/epoch state may not inherit the global "
                "application-barrier model"
            )

        required_body_tokens = {
            "RecordCertifiedNext": (
                "certifiedHeight' = nextHeight",
                "UNCHANGED <<nodeHeight, nodeContext",
            ),
            "RecordAppliedNext": (
                "node == application.node",
                "nodeHeight[node]",
                "![node] = nextHeight",
                "![node] = ContextRecord(nextHeight, nextLineage)",
            ),
        }
        for symbol, tokens in required_body_tokens.items():
            extracted = _top_level_operator_body(source, symbol)
            if extracted is None:
                errors.append(f"{chain_path}: missing per-node chain operator {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            missing = [token for token in tokens if token not in normalized]
            if missing:
                errors.append(
                    f"{chain_path}:{line}: {symbol} omits required per-node "
                    f"chain behavior {missing}"
                )
            for forbidden in ("CommonAppliedSubject", "AdvanceContext", "NextV2"):
                if re.search(rf"\b{forbidden}\b", body):
                    errors.append(
                        f"{chain_path}:{line}: {symbol} may not use global-barrier "
                        f"operator {forbidden}"
                    )

        tlc_harness_contracts = {
            "ChainEpochNext": (
                "\\/ \\E decision \\in DecisionEvidenceSet: "
                "RecordCertifiedNext(decision) \\/ \\E decision \\in "
                "DecisionEvidenceSet: RecordKnownDecision(decision) "
                "\\/ \\E application \\in DecisionEvidenceSet: "
                "RecordAppliedNext(application) \\/ \\E application \\in "
                "DecisionEvidenceSet: RecordKnownApplication(application)"
            ),
            "ChainEpochSpec": (
                "ChainEpochInit /\\ [][ChainEpochNext]_ChainEpochVars"
            ),
            "CandidateHistoricalCommitCertificateSet": (
                '{QC(qcContext, roundView, "Commit", subject, signers): '
                "qcContext \\in ContextRecords, roundView \\in Views, "
                "subject \\in ValidSubjects, signers \\in SUBSET ValidatorIds}"
            ),
            "HistoricalCommitCertificateSet": (
                "{qc \\in CandidateHistoricalCommitCertificateSet: "
                "DualQuorum(qc.context.epoch, qc.signers)}"
            ),
            "CandidateDurableDecisionEvidenceSet": (
                "{[node |-> node, qc |-> qc]: node \\in ValidatorIds, "
                "qc \\in HistoricalCommitCertificateSet}"
            ),
            "DurableDecisionEvidenceSet": (
                "{decision \\in CandidateDurableDecisionEvidenceSet: "
                "decision \\in DecisionEvidenceSet}"
            ),
            "ChainEpochTlcVars": "<<vars, ChainEpochVars>>",
            "ChainEpochTlcInit": "Init /\\ ChainEpochInit",
            "ChainEpochTlcReceiptNext": (
                "\\/ \\E decision \\in DurableDecisionEvidenceSet: "
                "RecordCertifiedNext(decision) \\/ \\E decision \\in "
                "DurableDecisionEvidenceSet: RecordKnownDecision(decision) "
                "\\/ \\E application \\in DurableDecisionEvidenceSet: "
                "RecordAppliedNext(application) \\/ \\E application \\in "
                "DurableDecisionEvidenceSet: RecordKnownApplication(application)"
            ),
            "ChainEpochTlcNext": (
                "ChainEpochTlcReceiptNext /\\ UNCHANGED vars"
            ),
            "ChainEpochTlcSpec": (
                "ChainEpochTlcInit /\\ [][ChainEpochTlcNext]_ChainEpochTlcVars"
            ),
            "ChainEpochTlcInvariant": "TypeInvariant /\\ ChainEpochInvariant",
        }
        for symbol, exact_body in tlc_harness_contracts.items():
            extracted = _top_level_operator_body(
                raw_chain_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{chain_path}: missing full-state TLC chain harness {symbol}"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{chain_path}:{line}: {symbol} must equal only "
                    f"{exact_body!r}; found {normalized!r}"
                )

    if proof_path.is_file():
        proof_source = proof_path.read_text(encoding="utf-8")
        property_contracts = {
            "ChainPrefixProperty": (
                "specification => [](/\\ HistoryPrefixComparable "
                "/\\ NodeAppliedPrefixBacked)"
            ),
            "EpochBoundaryProperty": (
                "specification => [](/\\ PerNodeFrozenEpoch "
                "/\\ PerNodeParentFinality /\\ ForeignLineageRejected "
                "/\\ ForeignContextCertificateRejected)"
            ),
        }
        for symbol, exact_body in property_contracts.items():
            extracted = _top_level_operator_body(proof_source, symbol)
            if extracted is None:
                errors.append(f"{proof_path}: missing stable chain property {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{proof_path}:{line}: {symbol} must equal only "
                    f"{exact_body!r}; found {normalized!r}"
                )
        refinement = _top_level_theorem_body(
            proof_source, "ChainEpochTlcReceiptNextRefinesChainEpochNext"
        )
        exact_refinement = "ChainEpochTlcReceiptNext => ChainEpochNext"
        if refinement is None:
            errors.append(
                f"{proof_path}: missing checked TLC-to-deductive receipt refinement"
            )
        else:
            body, line = refinement
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )[0]
            normalized = " ".join(statement.split())
            if normalized != exact_refinement:
                errors.append(
                    f"{proof_path}:{line}: TLC receipt refinement must state only "
                    f"{exact_refinement!r}; found {normalized!r}"
                )

    if refinement_path.is_file():
        raw_source = refinement_path.read_text(encoding="utf-8")
        source = strip_tla_comments(raw_source)
        retired_shadows = (
            "asyncCertifiedHeight",
            "asyncDecidedAt",
            "asyncNodeHeight",
            "asyncNodeContext",
            "asyncDurableDecisionEvidence",
            "asyncDurableApplicationEvidence",
            "AsyncHistoryNext",
            "AsyncHistoryVars",
            "historicalCatchUpDecisions",
            "historicalCatchUpApplications",
            "historicalCatchUpStage",
        )
        for symbol in retired_shadows:
            for match in re.finditer(rf"\b{re.escape(symbol)}\b", source):
                line = source.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{refinement_path}:{line}: stale async chain shadow {symbol} "
                    "is prohibited"
                )
        for match in re.finditer(
            r"\b(?:HistoricalCatchUp|IndexedHistoricalCatchUp|IndexedCatchUp)"
            r"[A-Za-z0-9_]*\b",
            source,
        ):
            line = source.count("\n", 0, match.start()) + 1
            errors.append(
                f"{refinement_path}:{line}: standalone historical catch-up "
                f"state or transition {match.group(0)} is prohibited; recovery "
                "must remain inside the exact indexed Async product"
            )
        for match in re.finditer(
            r"\bIndexedReachedAncestorHasEveryResponsiveJoined\b", source
        ):
            line = source.count("\n", 0, match.start()) + 1
            errors.append(
                f"{refinement_path}:{line}: retired false static ancestor-join "
                "theorem is prohibited; height progress must use the temporal "
                "activation-to-join bridge"
            )
        for forbidden in ("CommonAppliedSubject", "AdvanceContext", "NextV2"):
            for match in re.finditer(rf"\b{forbidden}\b", source):
                line = source.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{refinement_path}:{line}: chain refinement may not depend on "
                    f"global-barrier operator {forbidden}"
                )

        exact_indexed_projection_helpers = {
            "IndexedDuplicatedGst": (
                "indexedAsyncState[initialContext][1]"
            ),
            "IndexedCore": (
                "indexedAsyncState[initialContext][2][component]"
            ),
            "IndexedScheduler": (
                "indexedAsyncState[initialContext][3][component]"
            ),
            "IndexedRecovery": (
                "indexedAsyncState[initialContext][4][component]"
            ),
            "IndexedProducer": (
                "indexedAsyncState[initialContext][5][component]"
            ),
            "IndexedFixedCorridorDeadlines": (
                "indexedAsyncState[initialContext][6]"
            ),
        }
        for symbol, expected_body in exact_indexed_projection_helpers.items():
            extracted = _top_level_operator_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing {symbol} projection"
                )
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != expected_body:
                errors.append(
                    f"{refinement_path}:{line}: {symbol} must equal only "
                    f"{expected_body!r}; found {normalized!r}"
                )

        indexed_async = _top_level_operator_body(raw_source, "IndexedAsync")
        if indexed_async is None:
            errors.append(
                f"{refinement_path}: missing indexed production-network instance"
            )
        else:
            body, line = indexed_async
            normalized = " ".join(body.split())
            if "INSTANCE SumeragiV2AsyncNetwork WITH" not in normalized:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync must directly "
                    "instantiate the authoritative SumeragiV2AsyncNetwork"
                )
            expected_core_mappings = tuple(
                f"{field} <- IndexedCore(initialContext, {index})"
                for index, field in enumerate(core_fields, start=1)
            )
            missing_core = [
                mapping
                for mapping in expected_core_mappings
                if mapping not in normalized
            ]
            if missing_core:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync Core tuple mapping "
                    f"does not match vars; missing {missing_core}"
                )
            expected_mappings = tuple(
                f"{field} <- IndexedScheduler(initialContext, {index})"
                for index, field in enumerate(scheduler_fields, start=1)
            )
            missing = [
                mapping for mapping in expected_mappings if mapping not in normalized
            ]
            if missing:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync scheduler tuple mapping "
                    f"does not match AsyncSchedulerVars; missing {missing}"
                )
            expected_recovery_mappings = tuple(
                f"{field} <- IndexedRecovery(initialContext, {index})"
                for index, field in enumerate(recovery_fields, start=1)
            )
            expected_producer_mappings = tuple(
                f"{field} <- IndexedProducer(initialContext, {index})"
                for index, field in enumerate(producer_fields, start=1)
            )
            expected_fixed_corridor_mapping = (
                "asyncFixedCorridorDeadlines <- "
                "IndexedFixedCorridorDeadlines(initialContext)"
            )
            missing_recovery = [
                mapping
                for mapping in expected_recovery_mappings
                if mapping not in normalized
            ]
            if missing_recovery:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync recovery tuple mapping "
                    "does not match AsyncRecoveryVars; missing "
                    f"{missing_recovery}"
                )
            missing_producer = [
                mapping
                for mapping in expected_producer_mappings
                if mapping not in normalized
            ]
            if missing_producer:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync producer-journal "
                    "tuple mapping does not match AsyncProducerVars; missing "
                    f"{missing_producer}"
                )
            exact_indexed_async = (
                "INSTANCE SumeragiV2AsyncNetwork WITH "
                + ", ".join(
                    expected_core_mappings
                    + expected_mappings
                    + expected_recovery_mappings
                    + expected_producer_mappings
                    + (expected_fixed_corridor_mapping,)
                )
            )
            if normalized != exact_indexed_async:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsync must use exactly "
                    "the reviewed ordered 49 Core, 46 scheduler, 5 recovery, "
                    "3 producer-journal, and fixed-corridor substitutions"
                )

        verification_context = re.search(
            r"(?m)^CONSTANTS?[ \t]+VerificationContext[ \t]*$", source
        )
        if verification_context is None:
            errors.append(
                f"{refinement_path}: missing proof-only VerificationContext constant"
            )

        verification_helpers = {
            "VerificationCore": "IndexedCore(VerificationContext, component)",
            "VerificationScheduler": (
                "IndexedScheduler(VerificationContext, component)"
            ),
            "VerificationRecovery": (
                "IndexedRecovery(VerificationContext, component)"
            ),
            "VerificationProducer": (
                "IndexedProducer(VerificationContext, component)"
            ),
            "VerificationFixedCorridorDeadlines": (
                "IndexedFixedCorridorDeadlines(VerificationContext)"
            ),
        }
        for symbol, expected_body in verification_helpers.items():
            extracted = _top_level_operator_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing proof-only {symbol} mapping"
                )
                continue
            helper_body, helper_line = extracted
            helper_normalized = " ".join(helper_body.split())
            if helper_normalized != expected_body:
                errors.append(
                    f"{refinement_path}:{helper_line}: {symbol} must equal only "
                    f"{expected_body!r}; found {helper_normalized!r}"
                )

        verification_async_proof = _top_level_operator_body(
            raw_source, "VerificationAsyncProof"
        )
        if verification_async_proof is None:
            errors.append(
                f"{refinement_path}: missing proof-only VerificationAsyncProof instance"
            )
        else:
            proof_body, proof_line = verification_async_proof
            proof_normalized = " ".join(proof_body.split())
            proof_prefix = "INSTANCE SumeragiV2AsyncTemporalClosureProofs WITH"
            if proof_prefix not in proof_normalized:
                errors.append(
                    f"{refinement_path}:{proof_line}: VerificationAsyncProof "
                    "must directly instantiate "
                    "SumeragiV2AsyncTemporalClosureProofs"
                )
            else:
                expected_proof_mapping = (
                    proof_prefix
                    + " "
                    + ", ".join(
                        tuple(
                            f"{field} <- VerificationCore({index})"
                            for index, field in enumerate(
                                core_fields, start=1
                            )
                        )
                        + tuple(
                            f"{field} <- VerificationScheduler({index})"
                            for index, field in enumerate(
                                scheduler_fields, start=1
                            )
                        )
                        + tuple(
                            f"{field} <- VerificationRecovery({index})"
                            for index, field in enumerate(
                                recovery_fields, start=1
                            )
                        )
                        + tuple(
                            f"{field} <- VerificationProducer({index})"
                            for index, field in enumerate(
                                producer_fields, start=1
                            )
                        )
                        + (
                            "asyncFixedCorridorDeadlines <- "
                            "VerificationFixedCorridorDeadlines",
                        )
                    )
                )
                if proof_normalized != expected_proof_mapping:
                    errors.append(
                        f"{refinement_path}:{proof_line}: "
                        "VerificationAsyncProof must use exactly the reviewed "
                        "ordered 49 Core, 46 scheduler, 5 recovery, 3 "
                        "producer-journal, and fixed-corridor "
                        "substitutions through VerificationCore, "
                        "VerificationScheduler, VerificationRecovery, "
                        "VerificationProducer, and "
                        "VerificationFixedCorridorDeadlines"
                    )

        indexed_shape = _top_level_operator_body(
            raw_source, "IndexedAsyncStateShape"
        )
        if indexed_shape is None:
            errors.append(f"{refinement_path}: missing IndexedAsyncStateShape")
        else:
            body, line = indexed_shape
            normalized = " ".join(body.split())
            required = (
                "Len(indexedAsyncState[initialContext]) = 6",
                "DOMAIN indexedAsyncState[initialContext] = 1..6",
                "indexedAsyncState[initialContext][1] = "
                "indexedAsyncState[initialContext][2][7]",
                f"Len(indexedAsyncState[initialContext][2]) = {len(core_fields)}",
                f"DOMAIN indexedAsyncState[initialContext][2] = 1..{len(core_fields)}",
                f"Len(indexedAsyncState[initialContext][3]) = {scheduler_arity}",
                f"DOMAIN indexedAsyncState[initialContext][3] = 1..{scheduler_arity}",
                f"Len(indexedAsyncState[initialContext][4]) = {recovery_arity}",
                f"DOMAIN indexedAsyncState[initialContext][4] = 1..{recovery_arity}",
                f"Len(indexedAsyncState[initialContext][5]) = {producer_arity}",
                f"DOMAIN indexedAsyncState[initialContext][5] = 1..{producer_arity}",
            )
            missing = [token for token in required if token not in normalized]
            if missing:
                errors.append(
                    f"{refinement_path}:{line}: IndexedAsyncStateShape has stale "
                    f"Core/scheduler/recovery/producer tuple arity {missing}"
                )

        exact_variables = _top_level_theorem_body(
            raw_source, "IndexedInstanceVariablesAreExact"
        )
        exact_variables_statement = (
            "IndexedAsyncStateShape => \\A initialContext \\in "
            "AdmissibleContextRecords: IndexedAsync(initialContext)!AsyncAllVars "
            "= IndexedAsyncStateAt(initialContext)"
        )
        if exact_variables is None:
            errors.append(
                f"{refinement_path}: missing IndexedInstanceVariablesAreExact"
            )
        else:
            body, line = exact_variables
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )[0]
            normalized_statement = " ".join(statement.split())
            if normalized_statement != exact_variables_statement:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedInstanceVariablesAreExact must state only "
                    f"{exact_variables_statement!r}; found "
                    f"{normalized_statement!r}"
                )
            missing_definitions = [
                definition
                for definition in (
                    "IndexedAsyncStateShape",
                    "IndexedAsyncStateAt",
                    "IndexedDuplicatedGst",
                    "IndexedCore",
                    "IndexedScheduler",
                    "IndexedRecovery",
                    "IndexedProducer",
                    "IndexedFixedCorridorDeadlines",
                )
                if definition not in body
            ]
            if missing_definitions:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedInstanceVariablesAreExact must unfold every exact "
                    f"tuple projection; missing {missing_definitions}"
                )

        joined_runner = _top_level_operator_body(
            raw_source, "IndexedJoinedRunnerStep"
        )
        exact_historical_server_branch = (
            "\\/ \\E node \\in Responsive: "
            "/\\ node \\in joinedByContext[initialContext] "
            "/\\ IndexedAsync(initialContext)!RunHistoricalServer(node)"
        )
        if joined_runner is None:
            errors.append(f"{refinement_path}: missing IndexedJoinedRunnerStep")
        else:
            body, line = joined_runner
            normalized = " ".join(body.split())
            if (
                normalized.count(exact_historical_server_branch) != 1
                or normalized.count(
                    "IndexedAsync(initialContext)!RunHistoricalServer(node)"
                )
                != 1
            ):
                errors.append(
                    f"{refinement_path}:{line}: IndexedJoinedRunnerStep must "
                    "contain exactly one Responsive, joined-context "
                    "RunHistoricalServer branch"
                )

        joined_non_runner = _top_level_operator_body(
            raw_source, "IndexedJoinedNonRunnerStep"
        )
        exact_io_worker_branch = (
            "\\/ \\E node \\in Responsive: "
            "/\\ node \\in joinedByContext[initialContext] "
            "/\\ IndexedAsync(initialContext)!ServiceIoWorker(node)"
        )
        exact_enqueue_control_branch = (
            "\\/ \\E node \\in IndexedAsync(initialContext)!"
            "AsyncCurrentResponsiveVoters: "
            "/\\ node \\in joinedByContext[initialContext] "
            "/\\ IndexedAsync(initialContext)!EnqueueIoLocalControl(node)"
        )
        if joined_non_runner is None:
            errors.append(
                f"{refinement_path}: missing IndexedJoinedNonRunnerStep"
            )
        else:
            body, line = joined_non_runner
            normalized = " ".join(body.split())
            direct_discovery_branch = (
                "\\/ \\E node \\in IndexedAsync(initialContext)! "
                "AsyncCurrentResponsiveVoters: "
                "/\\ IndexedNodeCurrentAt(initialContext, node) "
                "/\\ IndexedAsync(initialContext)! "
                "DirectCommitCertificateDiscoveryStep(node)"
            )
            if direct_discovery_branch not in normalized:
                errors.append(
                    f"{refinement_path}:{line}: indexed non-runner step must "
                    "restrict the exact DirectCommitCertificateDiscoveryStep "
                    "to the node's current joined context"
                )
            expected_frame = (
                "UNCHANGED IndexedScheduler(initialContext, "
                f"{node_service_deadline_slot})"
            )
            if expected_frame not in normalized:
                errors.append(
                    f"{refinement_path}:{line}: indexed non-runner frame must "
                    f"preserve scheduler slot {node_service_deadline_slot} "
                    "(asyncNodeServiceDeadlines)"
                )
            if (
                normalized.count(exact_io_worker_branch) != 1
                or normalized.count(
                    "IndexedAsync(initialContext)!ServiceIoWorker(node)"
                )
                != 1
            ):
                errors.append(
                    f"{refinement_path}:{line}: IndexedJoinedNonRunnerStep must "
                    "contain exactly one Responsive, joined-context "
                    "ServiceIoWorker branch"
                )
            if (
                normalized.count(exact_enqueue_control_branch) != 1
                or normalized.count(
                    "IndexedAsync(initialContext)!EnqueueIoLocalControl(node)"
                )
                != 1
            ):
                errors.append(
                    f"{refinement_path}:{line}: IndexedJoinedNonRunnerStep must "
                    "keep exactly one joined-context EnqueueIoLocalControl "
                    "branch restricted to AsyncCurrentResponsiveVoters"
                )

        joined_non_crash = _top_level_operator_body(
            raw_source,
            "IndexedJoinedNonCrashStep",
            preserve_string_contents=True,
        )
        exact_joined_non_crash = (
            "/\\ (IndexedJoinedRunnerStep(initialContext) "
            "\\/ IndexedJoinedNonRunnerStep(initialContext)) "
            "/\\ UNCHANGED <<IndexedCore(initialContext, 6), "
            "IndexedAsync(initialContext)!AsyncRecoveryControlVars>>"
        )
        if joined_non_crash is None:
            errors.append(
                f"{refinement_path}: missing IndexedJoinedNonCrashStep"
            )
        else:
            body, line = joined_non_crash
            normalized = re.sub(r"!\s+", "!", " ".join(body.split()))
            if normalized != exact_joined_non_crash:
                errors.append(
                    f"{refinement_path}:{line}: IndexedJoinedNonCrashStep must "
                    "retain the complete non-crash recovery-control frame; "
                    f"found={normalized!r}"
                )

        joined_async_next = _top_level_operator_body(
            raw_source, "IndexedJoinedAsyncNext", preserve_string_contents=True
        )
        exact_joined_async_next = (
            "/\\ (IndexedJoinedNonCrashStep(initialContext) "
            "\\/ \\E node \\in ValidatorIds: "
            "IndexedAsync(initialContext)!PreGstCrash(node)) "
            "/\\ IndexedAsync(initialContext)!"
            "AsyncHistoricalLockRestartAuthorityTransition "
            "/\\ IndexedAsync(initialContext)!AsyncProducerProjectionStep "
            "/\\ UNCHANGED IndexedScheduler(initialContext, 46) "
            "/\\ UNCHANGED <<IndexedCore(initialContext, 1), "
            "IndexedCore(initialContext, 2)>> "
            "/\\ [IndexedAsync(initialContext)!Next]_( "
            "IndexedAsync(initialContext)!vars)"
        )
        prohibited_joined_recovery_actions = (
            "PreGstResponsiveCrash",
            "PreGstResponsiveRestart",
            "PreGstResponsiveReplay",
            "ResponsiveReplayRunNode",
            "ResponsiveReplayServiceIoWorker",
            "DriveResponsiveReplayHead",
            "FinishResponsiveReplay",
            "RearmResponsiveRecovery",
        )
        if joined_async_next is None:
            errors.append(f"{refinement_path}: missing IndexedJoinedAsyncNext")
        else:
            body, line = joined_async_next
            normalized = re.sub(r"!\s+", "!", " ".join(body.split()))
            prohibited = [
                action
                for action in prohibited_joined_recovery_actions
                if action in normalized
            ]
            if normalized != exact_joined_async_next or prohibited:
                errors.append(
                    f"{refinement_path}:{line}: IndexedJoinedAsyncNext must "
                    "contain only joined non-crash work or non-responsive "
                    "PreGstCrash, apply the global historical-lock "
                    "restart-authority and producer-projection transitions, "
                    "and exclude responsive "
                    "crash/replay/rearm "
                    f"actions; prohibited={prohibited!r}, found={normalized!r}"
                )

        discovery_step = _top_level_operator_body(
            raw_source, "IndexedCommitCertificateDiscoveryStep"
        )
        expected_discovery_step = (
            "/\\ IndexedChainNext "
            "/\\ IndexedNodeCurrentAt(initialContext, node) "
            "/\\ IndexedAsync(initialContext)! "
            "PostGstCommitCertificateDiscovery(node)"
        )
        if discovery_step is None:
            errors.append(
                f"{refinement_path}: missing indexed current Commit-certificate "
                "discovery fairness action"
            )
        else:
            body, line = discovery_step
            normalized = " ".join(body.split())
            if normalized != expected_discovery_step:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedCommitCertificateDiscoveryStep must equal only the "
                    f"current exact discovery product step; found {normalized!r}"
                )

        indexed_fairness = _top_level_operator_body(raw_source, "IndexedFairness")
        exact_discovery_fairness = (
            "WF_IndexedChainVars( "
            "IndexedCommitCertificateDiscoveryStep( initialContext, node))"
        )
        if indexed_fairness is None:
            errors.append(f"{refinement_path}: missing IndexedFairness")
        else:
            body, line = indexed_fairness
            normalized = " ".join(body.split())
            if normalized.count(exact_discovery_fairness) != 1:
                errors.append(
                    f"{refinement_path}:{line}: IndexedFairness must contain "
                    "exactly one weak-fair current Commit-certificate discovery "
                    "product clause"
                )

        # Small unit-test fixtures exercise only the tuple projection above.
        # The production refinement is distinguished by its authoritative
        # indexed product action; once that action exists, every explicit
        # successor and exact-recovery contract below is mandatory.
        if _top_level_operator_body(raw_source, "IndexedProductActionAt") is None:
            return errors

        def require_chain_operator(
            symbol: str,
            *,
            required: tuple[str, ...] = (),
            forbidden: tuple[str, ...] = (),
            exact: str | None = None,
        ) -> str | None:
            extracted = _top_level_operator_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing explicit chain operator {symbol}"
                )
                return None
            operator_body, operator_line = extracted
            operator_normalized = " ".join(operator_body.split())
            if exact is not None and operator_normalized != exact:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} must equal only "
                    f"{exact!r}; found {operator_normalized!r}"
                )
            missing_tokens = [
                token for token in required if token not in operator_normalized
            ]
            if missing_tokens:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} omits exact "
                    f"successor/exact-recovery behavior {missing_tokens}"
                )
            present_forbidden = [
                token for token in forbidden if token in operator_normalized
            ]
            if present_forbidden:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} contains "
                    f"prohibited successor/exact-recovery behavior "
                    f"{present_forbidden}"
                )
            return operator_normalized

        def normalize_chain_contract(text: str) -> str:
            normalized = " ".join(text.split())
            normalized = re.sub(r"!\s+", "!", normalized)
            normalized = re.sub(r"\(\s+", "(", normalized)
            return re.sub(r"\s+\)", ")", normalized)

        def require_chain_theorem(symbol: str, exact: str) -> None:
            extracted = _top_level_theorem_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing explicit chain theorem {symbol}"
                )
                return
            theorem_body, theorem_line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                theorem_body,
                maxsplit=1,
            )[0]
            theorem_normalized = " ".join(statement.split())
            if theorem_normalized != exact:
                errors.append(
                    f"{refinement_path}:{theorem_line}: {symbol} must state only "
                    f"{exact!r}; found {theorem_normalized!r}"
                )

        def require_chain_theorem_contract(
            symbol: str,
            *,
            exact: str | None = None,
            required: tuple[str, ...] = (),
            forbidden: tuple[str, ...] = (),
            exact_counts: dict[str, int] | None = None,
            proof_required: tuple[str, ...] = (),
            proof_forbidden: tuple[str, ...] = (),
        ) -> None:
            extracted = _top_level_theorem_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing explicit chain theorem {symbol}"
                )
                return
            theorem_body, theorem_line = extracted
            theorem_parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                theorem_body,
                maxsplit=1,
            )
            theorem_statement = normalize_chain_contract(theorem_parts[0])
            theorem_proof = theorem_parts[1] if len(theorem_parts) == 2 else ""
            exact_normalized = (
                None if exact is None else normalize_chain_contract(exact)
            )
            if exact_normalized is not None and theorem_statement != exact_normalized:
                errors.append(
                    f"{refinement_path}:{theorem_line}: {symbol} must state only "
                    f"{exact_normalized!r}; found {theorem_statement!r}"
                )
            missing = [
                token
                for token in required
                if normalize_chain_contract(token) not in theorem_statement
            ]
            present_forbidden = [
                token
                for token in forbidden
                if normalize_chain_contract(token) in theorem_statement
            ]
            invalid_counts = {
                token: theorem_statement.count(normalize_chain_contract(token))
                for token, expected_count in (exact_counts or {}).items()
                if theorem_statement.count(normalize_chain_contract(token))
                != expected_count
            }
            if missing or present_forbidden or invalid_counts:
                errors.append(
                    f"{refinement_path}:{theorem_line}: {symbol} must retain "
                    "the exact indexed fairness domains and action partition; "
                    f"missing={missing!r}, prohibited={present_forbidden!r}, "
                    f"counts={invalid_counts!r}"
                )
            missing_proof = [
                token
                for token in proof_required
                if not _tla_dependency_present(theorem_proof, token)
            ]
            present_proof_forbidden = [
                token
                for token in proof_forbidden
                if _tla_dependency_present(theorem_proof, token)
            ]
            if (
                proof_required or proof_forbidden
            ) and (
                len(theorem_parts) != 2
                or missing_proof
                or present_proof_forbidden
            ):
                errors.append(
                    f"{refinement_path}:{theorem_line}: {symbol} proof must "
                    "retain the exact indexed fairness dependencies; "
                    f"missing={missing_proof!r}, "
                    f"prohibited={present_proof_forbidden!r}, "
                    f"has_proof={len(theorem_parts) == 2}"
                )

        def require_normalized_chain_operator_contract(
            symbol: str,
            exact: str,
            *,
            forbidden: tuple[str, ...] = (),
        ) -> None:
            extracted = _top_level_operator_body(
                raw_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{refinement_path}: missing explicit chain operator {symbol}"
                )
                return
            operator_body, operator_line = extracted
            normalized = normalize_chain_contract(operator_body)
            exact_normalized = normalize_chain_contract(exact)
            present_forbidden = [
                token
                for token in forbidden
                if normalize_chain_contract(token) in normalized
            ]
            if normalized != exact_normalized or present_forbidden:
                errors.append(
                    f"{refinement_path}:{operator_line}: {symbol} must equal "
                    f"only {exact_normalized!r}; "
                    f"prohibited={present_forbidden!r}, found={normalized!r}"
                )

        require_chain_theorem_contract(
            "IndexedDuplicatedGstProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: /\\ "
                "IndexedDuplicatedGst(initialContext) = "
                "IndexedCore(initialContext, 7) /\\ "
                "IndexedAsync(initialContext)!gst = "
                "IndexedDuplicatedGst(initialContext)"
            ),
            proof_required=(
                "IndexedAsyncStateShape",
                "IndexedDuplicatedGst",
                "IndexedCore",
            ),
        )
        require_chain_theorem_contract(
            "IndexedFortyNineFieldCoreProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!vars = <<"
                + ", ".join(
                    f"IndexedCore(initialContext, {index})"
                    for index in range(1, len(core_fields) + 1)
                )
                + ">>"
            ),
            proof_required=("IndexedAsync!vars", "IndexedCore"),
        )
        require_chain_theorem_contract(
            "IndexedFortySixFieldSchedulerProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!AsyncSchedulerVars = <<"
                + ", ".join(
                    f"IndexedScheduler(initialContext, {index})"
                    for index in range(1, scheduler_arity + 1)
                )
                + ">>"
            ),
            proof_required=(
                "IndexedAsync!AsyncSchedulerVars",
                "IndexedScheduler",
            ),
        )
        require_chain_theorem_contract(
            "IndexedFixedCorridorDeadlineProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!asyncFixedCorridorDeadlines = "
                "IndexedFixedCorridorDeadlines(initialContext)"
            ),
            proof_required=("IndexedFixedCorridorDeadlines",),
        )
        require_chain_theorem_contract(
            "IndexedThreeFieldProducerProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: /\\ "
                "IndexedAsync(initialContext)!AsyncProducerVars = "
                "indexedAsyncState[initialContext][5] /\\ "
                "indexedAsyncState[initialContext][5] = "
                "<<IndexedProducer(initialContext, 1), "
                "IndexedProducer(initialContext, 2), "
                "IndexedProducer(initialContext, 3)>>"
            ),
            proof_required=(
                "IndexedAsyncStateShape",
                "IndexedAsync!AsyncProducerVars",
                "IndexedProducer",
            ),
        )
        require_chain_theorem_contract(
            "VerificationThreeFieldProducerProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape /\\ VerificationContext \\in "
                "AdmissibleContextRecords => /\\ "
                "VerificationAsyncProof!AsyncProducerVars = "
                "indexedAsyncState[VerificationContext][5] /\\ "
                "indexedAsyncState[VerificationContext][5] = "
                "<<VerificationProducer(1), VerificationProducer(2), "
                "VerificationProducer(3)>>"
            ),
            proof_required=(
                "IndexedAsyncStateShape",
                "VerificationAsyncProof!AsyncProducerVars",
                "VerificationProducer",
                "IndexedProducer",
            ),
        )
        require_chain_theorem_contract(
            "VerificationInstanceVariablesAreExact",
            exact=(
                "/\\ IndexedAsyncStateShape /\\ VerificationContext \\in "
                "AdmissibleContextRecords => "
                "VerificationAsyncProof!AsyncAllVars = "
                "IndexedAsyncStateAt(VerificationContext)"
            ),
            proof_required=(
                "VerificationAsyncProof!AsyncAllVars",
                "VerificationAsyncProof!AsyncSchedulerVars",
                "VerificationAsyncProof!AsyncRecoveryVars",
                "VerificationAsyncProof!AsyncProducerVars",
                "VerificationAsyncProof!vars",
                "VerificationProducer",
                "VerificationFixedCorridorDeadlines",
                "IndexedDuplicatedGst",
                "IndexedCore",
                "IndexedScheduler",
                "IndexedRecovery",
                "IndexedProducer",
                "IndexedFixedCorridorDeadlines",
            ),
        )
        require_chain_theorem_contract(
            "IndexedLeaderWireLifecycleProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!asyncLeaderWireLifecycles = "
                "IndexedScheduler(initialContext, 42)"
            ),
            proof_required=("IndexedScheduler",),
        )
        require_chain_theorem_contract(
            "VerificationLeaderWireLifecycleProjectionIsExact",
            exact=(
                "/\\ IndexedAsyncStateShape /\\ VerificationContext \\in "
                "AdmissibleContextRecords => "
                "VerificationAsyncProof!asyncLeaderWireLifecycles = "
                "VerificationScheduler(42)"
            ),
            proof_required=("VerificationScheduler", "IndexedScheduler"),
        )
        require_chain_theorem_contract(
            "IndexedServiceActivationProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!AsyncSchedulerVars[46] = "
                "IndexedScheduler(initialContext, 46)"
            ),
            proof_required=(
                "IndexedAsync!AsyncSchedulerVars",
                "IndexedScheduler",
            ),
        )
        require_chain_theorem_contract(
            "VerificationServiceActivationProjectionIsExact",
            exact=(
                "/\\ IndexedAsyncStateShape /\\ VerificationContext \\in "
                "AdmissibleContextRecords => "
                "VerificationAsyncProof!AsyncSchedulerVars[46] = "
                "VerificationScheduler(46)"
            ),
            proof_required=(
                "VerificationAsyncProof!AsyncSchedulerVars",
                "VerificationScheduler",
                "IndexedScheduler",
            ),
        )

        require_chain_theorem_contract(
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            exact=(
                "IndexedAsyncStateShape => \\A initialContext \\in "
                "AdmissibleContextRecords: /\\ "
                "IndexedAsync(initialContext)!AsyncServeLifecycleVars = "
                "<<IndexedScheduler(initialContext, 11), "
                "IndexedScheduler(initialContext, 14), "
                "IndexedScheduler(initialContext, 15), "
                "IndexedScheduler(initialContext, 16), "
                "IndexedScheduler(initialContext, 17)>> /\\ "
                "IndexedAsync(initialContext)!AsyncServeIngressAdmissionVars = "
                "<<IndexedScheduler(initialContext, 12), "
                "IndexedScheduler(initialContext, 13)>>"
            ),
            proof_required=(
                "IndexedAsyncStateShape",
                "IndexedAsync!AsyncServeLifecycleVars",
                "IndexedAsync!AsyncServeIngressAdmissionVars",
                "IndexedScheduler",
            ),
        )
        require_chain_theorem_contract(
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            exact=(
                "/\\ IndexedAsyncStateShape /\\ VerificationContext \\in "
                "AdmissibleContextRecords => /\\ "
                "VerificationAsyncProof!AsyncServeLifecycleVars = "
                "<<VerificationScheduler(11), VerificationScheduler(14), "
                "VerificationScheduler(15), VerificationScheduler(16), "
                "VerificationScheduler(17)>> /\\ "
                "VerificationAsyncProof!AsyncServeIngressAdmissionVars = "
                "<<VerificationScheduler(12), VerificationScheduler(13)>>"
            ),
            proof_required=(
                "IndexedAsyncStateShape",
                "VerificationAsyncProof!AsyncServeLifecycleVars",
                "VerificationAsyncProof!AsyncServeIngressAdmissionVars",
                "VerificationScheduler",
                "IndexedScheduler",
            ),
        )
        require_normalized_chain_operator_contract(
            "AsyncLiveChainSpec",
            (
                "/\\ AsyncRepresentativeLiveConfiguration "
                "/\\ AsyncChainSpec"
            ),
        )
        require_chain_theorem_contract(
            "AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec",
            exact=(
                "AsyncLiveChainSpec "
                "=> AsyncLiveSpecAt(ContextRecord(0, <<>>))"
            ),
            proof_required=(
                "AsyncLiveChainSpec",
                "AsyncChainSpecProjectsAsyncSpec",
                "AsyncRepresentativeLiveConfiguration",
                "AsyncLiveSpecAt",
            ),
        )
        require_chain_theorem_contract(
            "GenesisHeightSuccessorHandoffFromOneHeightCompletion",
            exact=(
                "/\\ AsyncLiveChainSpec "
                "/\\ OneHeightCompletionLiveness(ContextRecord(0, <<>>)) "
                "=> GenesisHeightSuccessorHandoffProperty"
            ),
            proof_required=(
                "AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec",
                "AsyncLiveSpecAt(ContextRecord(0, <<>>))",
                "AsyncLiveChainSpec",
            ),
        )
        require_chain_theorem_contract(
            "GenesisHeightSuccessorHandoffObligation",
            exact=(
                "AsyncLiveChainSpec "
                "=> GenesisHeightSuccessorHandoffProperty"
            ),
            proof_required=(
                "AsyncTemporalClosureOneHeightCompletionObligation",
                "GenesisHeightSuccessorHandoffFromOneHeightCompletion",
            ),
            proof_forbidden=("OneHeightCompletionObligation",),
        )
        require_normalized_chain_operator_contract(
            "IndexedLiveChainSpec",
            (
                "/\\ AsyncRepresentativeLiveConfiguration "
                "/\\ IndexedChainSpec"
            ),
        )
        require_chain_operator(
            "IndexedGstEventuallyCondition",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords: "
                "IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext) "
                "=> <>IndexedCore(initialContext, 7)"
            ),
        )
        if _top_level_operator_body(
            raw_source,
            "IndexedInstallGenerationBudgetPremise",
            preserve_string_contents=True,
        ) is not None:
            errors.append(
                f"{refinement_path}: IndexedInstallGenerationBudgetPremise "
                "is an illicit finite-counter liveness assumption; retain "
                "AsyncInstallGenerationBudget only as a diagnostic predicate"
            )
        require_chain_theorem_contract(
            "IndexedLiveChainSpecProjectsIndexedChainSpec",
            exact="IndexedLiveChainSpec => IndexedChainSpec",
            proof_required=("IndexedLiveChainSpec",),
        )
        require_chain_theorem_contract(
            "IndexedLiveInstanceActivationObligation",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords: "
                "(/\\ IndexedLiveChainSpec "
                "/\\ TRUE ~> IndexedAllResponsiveJoined(initialContext)) "
                "=> IndexedAsync(initialContext)!"
                "AsyncLiveSpecAt(initialContext)"
            ),
            proof_required=(
                "IndexedLiveChainSpec",
                "AsyncRepresentativeLiveConfiguration",
                "IndexedInstanceActivationObligation",
                "IndexedAsync!AsyncLiveSpecAt",
            ),
        )
        require_normalized_chain_operator_contract(
            "VerificationOneHeightCompletion",
            (
                "IndexedAsync(VerificationContext)!"
                "AsyncLiveSpecAt(VerificationContext) "
                "=> (IndexedCore(VerificationContext, 7) "
                "~> IndexedAsync(VerificationContext)!"
                "AsyncAllResponsiveAppliedAt(VerificationContext))"
            ),
        )
        require_chain_theorem_contract(
            "VerificationOneHeightCompletionObligation",
            exact="VerificationOneHeightCompletion",
            proof_required=(
                "VerificationAsyncProof!"
                "AsyncTemporalClosureOneHeightCompletionObligation",
                "VerificationAsyncProof!AsyncLiveSpecAt",
                "IndexedAsync!AsyncLiveSpecAt",
            ),
            proof_forbidden=(
                "VerificationAsyncProof!OneHeightCompletionObligation",
            ),
        )
        require_chain_theorem_contract(
            "VerificationFrontierActivatedInstanceEventuallyApplies",
            exact=(
                "/\\ IndexedLiveChainSpec "
                "/\\ VerificationOneHeightCompletion "
                "/\\ VerificationContext \\in AdmissibleContextRecords "
                "/\\ (IndexedAsync(VerificationContext)!"
                "AsyncLiveSpecAt(VerificationContext) "
                "=> <>IndexedCore(VerificationContext, 7)) "
                "/\\ []~JoinedCanonicalDescendant(VerificationContext) "
                "=> IndexedAllResponsiveJoined(VerificationContext) "
                "~> IndexedAsync(VerificationContext)!"
                "AsyncAllResponsiveAppliedAt(VerificationContext)"
            ),
            proof_required=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "IndexedLiveInstanceActivationObligation",
            ),
        )
        require_chain_theorem_contract(
            "VerificationActivatedFrontierEventuallyEscapes",
            exact=(
                "/\\ IndexedLiveChainSpec "
                "/\\ VerificationOneHeightCompletion "
                "/\\ VerificationContext \\in AdmissibleContextRecords "
                "/\\ (IndexedAsync(VerificationContext)!"
                "AsyncLiveSpecAt(VerificationContext) "
                "=> <>IndexedCore(VerificationContext, 7)) "
                "=> IndexedAllResponsiveJoined(VerificationContext) "
                "~> VerificationFrontierEscape"
            ),
            proof_required=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "VerificationFrontierActivatedInstanceEventuallyApplies",
            ),
        )
        require_chain_theorem_contract(
            "VerificationJoinedTargetEventuallyReachesAndEscapes",
            exact=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedExactHistoricalRecoveryProgress "
                "/\\ IndexedSuccessorActivationProgress "
                "/\\ VerificationOneHeightCompletion "
                "/\\ VerificationContext \\in AdmissibleContextRecords "
                "/\\ (IndexedAsync(VerificationContext)!"
                "AsyncLiveSpecAt(VerificationContext) "
                "=> <>IndexedCore(VerificationContext, 7)) "
                "=> IndexedTargetJoined(VerificationContext) "
                "~> (/\\ IndexedTargetJoined(VerificationContext) "
                "/\\ IndexedResponsiveHeightReached("
                "VerificationContext.height) "
                "/\\ VerificationFrontierEscape)"
            ),
            proof_required=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "VerificationActivatedFrontierEventuallyEscapes",
            ),
        )

        composition_invariant = _top_level_operator_body(
            raw_source,
            "IndexedCompositionInvariant",
            preserve_string_contents=True,
        )
        if composition_invariant is not None:
            invariant_body, invariant_line = composition_invariant
            embedded_budget_tokens = [
                token
                for token in (
                    "AsyncInstallGenerationBudget",
                    "IndexedInstallGenerationBudgetPremise",
                    "IndexedLiveChainSpec",
                )
                if token in invariant_body
            ]
            if embedded_budget_tokens:
                errors.append(
                    f"{refinement_path}:{invariant_line}: "
                    "IndexedCompositionInvariant may not embed a live spec or "
                    "the diagnostic install-generation boundary; "
                    f"found={embedded_budget_tokens!r}"
                )

        for theorem_symbol, theorem_kind, _, _ in _top_level_declarations(
            raw_source
        ):
            if theorem_kind != "theorem":
                continue
            extracted = _top_level_theorem_body(
                raw_source,
                theorem_symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                continue
            theorem_body, theorem_line = extracted
            statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                theorem_body,
                maxsplit=1,
            )[0]
            forbidden_generation_assumptions = tuple(
                token
                for token in (
                    "IndexedInstallGenerationBudgetPremise",
                    "AsyncInstallGenerationBudget",
                )
                if token in tla_code_tokens(statement)
            )
            if forbidden_generation_assumptions:
                errors.append(
                    f"{refinement_path}:{theorem_line}: {theorem_symbol} may "
                    "not state a finite install-generation liveness premise; "
                    f"found={forbidden_generation_assumptions!r}"
                )
            proof = theorem_body[len(statement) :]
            forbidden_generation_dependencies = tuple(
                token
                for token in (
                    "IndexedInstallGenerationBudgetPremise",
                    "AsyncInstallGenerationBudget",
                )
                if token in tla_code_tokens(proof)
            )
            if forbidden_generation_dependencies:
                errors.append(
                    f"{refinement_path}:{theorem_line}: {theorem_symbol} may "
                    "not depend on a finite install-generation liveness "
                    f"premise; found={forbidden_generation_dependencies!r}"
                )

        responsive_recovery_dormant = _top_level_operator_body(
            raw_source,
            "IndexedResponsiveRecoveryDormant",
            preserve_string_contents=True,
        )
        exact_responsive_recovery_dormant = (
            "\\A initialContext \\in AdmissibleContextRecords: "
            'IndexedRecovery(initialContext, 1) = "Eligible"'
        )
        if responsive_recovery_dormant is None:
            errors.append(
                f"{refinement_path}: missing explicit chain operator "
                "IndexedResponsiveRecoveryDormant"
            )
        else:
            body, line = responsive_recovery_dormant
            normalized = normalize_chain_contract(body)
            if normalized != exact_responsive_recovery_dormant:
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedResponsiveRecoveryDormant must pin every indexed "
                    "instance to the initialized Eligible recovery phase; "
                    f"found={normalized!r}"
                )

        responsive_recovery_actions = (
            "PreGstResponsiveRestart",
            "PreGstResponsiveReplay",
            "ResponsiveReplayRunNode",
            "ResponsiveReplayServiceIoWorker",
            "DriveResponsiveReplayHead",
            "FinishResponsiveReplay",
        )
        require_chain_theorem_contract(
            "JoinedAsyncStepRefinesExactAsyncStep",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords: "
                "IndexedJoinedAsyncNext(initialContext) "
                "=> IndexedAsync(initialContext)!AsyncNext"
            ),
            proof_required=(
                "JoinedRunnerIsExactAsyncWork",
                "JoinedNonRunnerIsExactAsyncWork",
                "IndexedJoinedAsyncNext",
                "IndexedJoinedNonCrashStep",
            ),
        )
        exact_responsive_recovery_actions_disabled = (
            "\\A initialContext \\in AdmissibleContextRecords: "
            "/\\ ~ENABLED "
            "<<IndexedAsync(initialContext)!PreGstResponsiveRestart>>_("
            "IndexedAsyncStateAt(initialContext)) "
            "/\\ ~ENABLED "
            "<<IndexedAsync(initialContext)!PreGstResponsiveReplay>>_("
            "IndexedAsyncStateAt(initialContext)) "
            "/\\ ~ENABLED "
            "<<IndexedAsync(initialContext)!ResponsiveReplayRunNode>>_("
            "IndexedAsyncStateAt(initialContext)) "
            "/\\ ~ENABLED "
            "<<IndexedAsync(initialContext)!"
            "ResponsiveReplayServiceIoWorker>>_("
            "IndexedAsyncStateAt(initialContext)) "
            "/\\ ~ENABLED "
            "<<IndexedAsync(initialContext)!DriveResponsiveReplayHead>>_("
            "IndexedAsyncStateAt(initialContext)) "
            "/\\ ~ENABLED "
            "<<IndexedAsync(initialContext)!FinishResponsiveReplay>>_("
            "IndexedAsyncStateAt(initialContext))"
        )
        recovery_actions_disabled = _top_level_operator_body(
            raw_source,
            "IndexedResponsiveRecoveryActionsDisabled",
            preserve_string_contents=True,
        )
        if recovery_actions_disabled is None:
            errors.append(
                f"{refinement_path}: missing explicit chain operator "
                "IndexedResponsiveRecoveryActionsDisabled"
            )
        else:
            body, line = recovery_actions_disabled
            normalized = normalize_chain_contract(body)
            actual_disabled_actions = tuple(
                re.findall(
                    r"<<IndexedAsync\(initialContext\)!\s*"
                    r"([A-Za-z][A-Za-z0-9_]*)>>_",
                    body,
                )
            )
            if (
                normalized
                != normalize_chain_contract(
                    exact_responsive_recovery_actions_disabled
                )
                or actual_disabled_actions != responsive_recovery_actions
            ):
                errors.append(
                    f"{refinement_path}:{line}: "
                    "IndexedResponsiveRecoveryActionsDisabled must contain "
                    "exactly the six reviewed always-disabled recovery actions "
                    f"{responsive_recovery_actions!r}; "
                    f"found={actual_disabled_actions!r}"
                )

        require_chain_theorem_contract(
            "IndexedInitEstablishesResponsiveRecoveryDormancy",
            exact=(
                "IndexedChainInit => IndexedResponsiveRecoveryDormant"
            ),
            proof_required=(
                "IndexedAsync!AsyncRecoveryInit",
                "IndexedRecovery",
            ),
        )
        require_chain_theorem_contract(
            "IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords: "
                'IndexedRecovery(initialContext, 1) = "Eligible" '
                "/\\ IndexedJoinedAsyncNext(initialContext) "
                "=> IndexedRecovery(initialContext, 1)' = \"Eligible\""
            ),
            proof_required=(
                "IndexedJoinedNonCrashStep",
                "IndexedAsync!PreGstCrash",
                "IndexedAsync!AsyncRecoveryControlVars",
            ),
        )
        require_chain_theorem_contract(
            "IndexedProductActionPreservesResponsiveRecoveryDormancy",
            exact=(
                "\\A selectedContext \\in AdmissibleContextRecords: "
                "IndexedResponsiveRecoveryDormant "
                "/\\ IndexedProductActionAt(selectedContext) "
                "=> IndexedResponsiveRecoveryDormant'"
            ),
            proof_required=(
                "IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility",
            ),
        )
        require_chain_theorem_contract(
            "IndexedSuccessorActivationStepPreservesRecoveryState",
            exact=(
                "\\A parentContext \\in AdmissibleContextRecords, "
                "node \\in ValidatorIds: "
                "IndexedSuccessorActivationProgressStep(parentContext, node) "
                "=> \\A initialContext \\in AdmissibleContextRecords: "
                "UNCHANGED indexedAsyncState[initialContext][4]"
            ),
            proof_required=(
                "SuccessorActivationEnvironmentStutter",
                "IndexedAsync!AsyncEnterIndexedServiceActivation",
                "IndexedAsync!AsyncActivateServiceNode",
                "IndexedAsync!AsyncRecoveryVars",
                "IndexedRecovery",
            ),
        )
        require_chain_theorem_contract(
            "IndexedActionPreservesResponsiveRecoveryDormancy",
            exact=(
                "IndexedResponsiveRecoveryDormant /\\ IndexedChainNext "
                "=> IndexedResponsiveRecoveryDormant'"
            ),
            proof_required=(
                "IndexedProductActionPreservesResponsiveRecoveryDormancy",
                "IndexedSuccessorActivationStepPreservesRecoveryState",
            ),
        )
        require_chain_theorem_contract(
            "IndexedStepPreservesResponsiveRecoveryDormancy",
            exact=(
                "IndexedResponsiveRecoveryDormant "
                "/\\ [IndexedChainNext]_IndexedChainVars "
                "=> IndexedResponsiveRecoveryDormant'"
            ),
            proof_required=(
                "IndexedActionPreservesResponsiveRecoveryDormancy",
            ),
        )
        require_chain_theorem_contract(
            "IndexedChainSpecKeepsResponsiveRecoveryDormant",
            exact=(
                "IndexedChainSpec => []IndexedResponsiveRecoveryDormant"
            ),
            proof_required=(
                "IndexedInitEstablishesResponsiveRecoveryDormancy",
                "IndexedStepPreservesResponsiveRecoveryDormancy",
                "PTL",
            ),
        )
        require_chain_theorem_contract(
            "IndexedResponsiveRecoveryDormancyDisablesFairActions",
            exact=(
                "IndexedResponsiveRecoveryDormant "
                "=> IndexedResponsiveRecoveryActionsDisabled"
            ),
            proof_required=(
                "ExpandENABLED",
                "Isa",
                *(
                    f"IndexedAsync!{action}"
                    for action in responsive_recovery_actions
                ),
            ),
        )
        require_chain_theorem_contract(
            "IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions",
            exact=(
                "IndexedChainSpec "
                "=> []IndexedResponsiveRecoveryActionsDisabled"
            ),
            proof_required=(
                "IndexedChainSpecKeepsResponsiveRecoveryDormant",
                "IndexedResponsiveRecoveryDormancyDisablesFairActions",
                "PTL",
            ),
        )
        exact_responsive_recovery_fairness = (
            "IndexedChainSpec "
            "=> \\A initialContext \\in AdmissibleContextRecords: "
            "/\\ WF_(IndexedAsyncStateAt(initialContext))("
            "IndexedAsync(initialContext)!PreGstResponsiveRestart) "
            "/\\ WF_(IndexedAsyncStateAt(initialContext))("
            "IndexedAsync(initialContext)!PreGstResponsiveReplay) "
            "/\\ WF_(IndexedAsyncStateAt(initialContext))("
            "IndexedAsync(initialContext)!ResponsiveReplayRunNode) "
            "/\\ WF_(IndexedAsyncStateAt(initialContext))("
            "IndexedAsync(initialContext)!"
            "ResponsiveReplayServiceIoWorker) "
            "/\\ WF_(IndexedAsyncStateAt(initialContext))("
            "IndexedAsync(initialContext)!DriveResponsiveReplayHead) "
            "/\\ WF_(IndexedAsyncStateAt(initialContext))("
            "IndexedAsync(initialContext)!FinishResponsiveReplay)"
        )
        require_chain_theorem_contract(
            "IndexedResponsiveRecoveryFairnessIsVacuous",
            exact=exact_responsive_recovery_fairness,
            exact_counts={
                f"IndexedAsync(initialContext)!{action}": 1
                for action in responsive_recovery_actions
            },
            proof_required=(
                "IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions",
                "IndexedResponsiveRecoveryActionsDisabled",
                "PTL",
            ),
        )

        voter_node_domain = (
            "\\A node \\in IndexedAsync(initialContext)!"
            "AsyncVotersAt(initialContext):"
        )
        responsive_node_domain = "\\A node \\in Responsive:"
        ordinary_packet_domain = (
            "\\A recipient \\in Responsive, "
            "source \\in IndexedAsync(initialContext)!AsyncIngressSources:"
        )
        historical_packet_domain = (
            "\\A recipient \\in ValidatorIds, "
            "source \\in IndexedAsync(initialContext)!AsyncIngressSources:"
        )
        fairness_bridge_contracts = {
            "IndexedFairActionsRemainEnabledInProduct": (
                (
                    voter_node_domain,
                    (
                        f"{responsive_node_domain} "
                        "node \\in joinedByContext[initialContext] "
                        "=> /\\ (ENABLED IndexedAsync(initialContext)!"
                        "PostGstRunHistoricalServer(node)"
                    ),
                    ordinary_packet_domain,
                    historical_packet_domain,
                ),
                {
                    "PostGstRunHistoricalServer(node)": 1,
                    "PostGstServiceIoWorker(node)": 1,
                    "PostGstAdmitHiddenPacket(recipient, source)": 1,
                    "PostGstAdmitHistoricalRecoveryPacket(recipient, source)": 1,
                },
            ),
            "IndexedFairProductStepsProjectExactOccurrences": (
                (
                    voter_node_domain,
                    (
                        f"{responsive_node_domain} "
                        "/\\ (IndexedHistoricalServerStep(initialContext, node)"
                    ),
                    ordinary_packet_domain,
                    historical_packet_domain,
                ),
                {
                    "PostGstRunHistoricalServer(node)": 1,
                    "PostGstServiceIoWorker(node)": 1,
                    "PostGstAdmitHiddenPacket(recipient, source)": 1,
                    "PostGstAdmitHistoricalRecoveryPacket(recipient, source)": 1,
                },
            ),
            "IndexedFairExactOccurrencesEnableProductOccurrences": (
                (
                    voter_node_domain,
                    (
                        f"{responsive_node_domain} "
                        "/\\ (ENABLED <<IndexedAsync(initialContext)!"
                        "PostGstRunHistoricalServer(node)"
                    ),
                    ordinary_packet_domain,
                    historical_packet_domain,
                ),
                {
                    "PostGstRunHistoricalServer(node)": 1,
                    "PostGstServiceIoWorker(node)": 1,
                    "PostGstAdmitHiddenPacket(recipient, source)": 1,
                    "PostGstAdmitHistoricalRecoveryPacket(recipient, source)": 1,
                },
            ),
        }
        for symbol, (required_domains, exact_action_counts) in (
            fairness_bridge_contracts.items()
        ):
            require_chain_theorem_contract(
                symbol,
                required=required_domains,
                exact_counts=exact_action_counts,
            )

        def require_chain_fairness_transfer(
            symbol: str,
            domain: str,
            actions: tuple[str, ...],
        ) -> None:
            require_chain_theorem_contract(
                symbol,
                required=(domain,),
                exact_counts={f"{action}(": 1 for action in actions},
            )
            extracted = _top_level_theorem_body(
                raw_source, symbol, preserve_string_contents=True
            )
            if extracted is None:
                return
            theorem_body, theorem_line = extracted
            theorem_statement = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                theorem_body,
                maxsplit=1,
            )[0]
            actual_actions = tuple(
                re.findall(
                    r"IndexedAsync\(initialContext\)!\s*"
                    r"(PostGst[A-Za-z0-9_]+)\(",
                    theorem_statement,
                )
            )
            if actual_actions != actions:
                errors.append(
                    f"{refinement_path}:{theorem_line}: {symbol} must transfer "
                    f"exactly the indexed Async fair actions {actions!r}; "
                    f"found {actual_actions!r}"
                )

        require_chain_fairness_transfer(
            "IndexedNodeFairnessTransfers",
            voter_node_domain,
            ("PostGstRunNode", "PostGstCommitCertificateDiscovery"),
        )
        require_chain_fairness_transfer(
            "IndexedResponsiveServiceFairnessTransfers",
            responsive_node_domain,
            ("PostGstRunHistoricalServer", "PostGstServiceIoWorker"),
        )
        require_chain_fairness_transfer(
            "IndexedPacketFairnessTransfers",
            ordinary_packet_domain,
            ("PostGstAdmitHiddenPacket",),
        )
        require_chain_fairness_transfer(
            "IndexedHistoricalRecoveryPacketFairnessTransfers",
            historical_packet_domain,
            ("PostGstAdmitHistoricalRecoveryPacket",),
        )

        require_chain_theorem_contract(
            "IndexedInstanceActivationObligation",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords: "
                "(/\\ IndexedChainSpec "
                "/\\ TRUE ~> IndexedAllResponsiveJoined(initialContext)) "
                "=> IndexedAsync(initialContext)!AsyncSpecAt(initialContext)"
            ),
            proof_required=(
                "IndexedResponsiveRecoveryFairnessIsVacuous",
                *responsive_recovery_actions,
                "IndexedNodeFairnessTransfers",
                "IndexedResponsiveServiceFairnessTransfers",
                "IndexedHistoricalRecoveryFairnessTransfers",
                "IndexedPacketFairnessTransfers",
                "IndexedHistoricalRecoveryPacketFairnessTransfers",
                ordinary_packet_domain,
                historical_packet_domain,
            ),
            proof_forbidden=(
                "\\A recipient \\in ValidatorIds, source \\in ValidatorIds:",
                (
                    "\\A recipient \\in IndexedAsync(initialContext)!"
                    "AsyncVotersAt(initialContext), "
                    "source \\in IndexedAsync(initialContext)!"
                    "AsyncVotersAt(initialContext):"
                ),
            ),
        )

        require_chain_theorem(
            "IndexedFreshReceiptActionHasProductExtension",
            (
                "\\A initialContext \\in AdmissibleContextRecords: "
                "(IndexedCompositionInvariant "
                "/\\ initialContext \\in JoinedContexts "
                "/\\ ENABLED IndexedFreshReceiptAsyncAction(initialContext)) "
                "=> ENABLED "
                "(/\\ IndexedProductActionAt(initialContext) "
                "/\\ IndexedFreshReceiptAsyncAction(initialContext))"
            ),
        )
        receipt_product_operators = {
            "IndexedCurrentDecisions": (
                "{decision \\in IndexedDecisions(initialContext): "
                "/\\ decision.qc.context = initialContext "
                "/\\ decision.qc.height = initialContext.height}"
            ),
            "IndexedCurrentApplications": (
                "{application \\in IndexedApplications(initialContext): "
                "/\\ application.qc.context = initialContext "
                "/\\ application.qc.height = initialContext.height}"
            ),
            "IndexedDecisionEvidence": (
                "UNION {IndexedCurrentDecisions(initialContext): "
                "initialContext \\in AdmissibleContextRecords}"
            ),
            "IndexedApplicationEvidence": (
                "UNION {IndexedCurrentApplications(initialContext): "
                "initialContext \\in AdmissibleContextRecords}"
            ),
            "IndexedDecisionReceiptProjection": (
                "durableDecisionEvidence = IndexedDecisionEvidence"
            ),
            "IndexedApplicationReceiptProjection": (
                "durableApplicationEvidence = IndexedApplicationEvidence"
            ),
            "IndexedTotalReceiptProjection": (
                "/\\ IndexedDecisionReceiptProjection "
                "/\\ IndexedApplicationReceiptProjection"
            ),
            "NewIndexedDecisionReceipt": (
                "/\\ decision \\notin IndexedDecisions(initialContext) "
                "/\\ IndexedDecisions(initialContext)' = "
                "IndexedDecisions(initialContext) \\cup {decision} "
                "/\\ IndexedApplications(initialContext)' = "
                "IndexedApplications(initialContext)"
            ),
            "NewIndexedApplicationReceipt": (
                "/\\ application \\notin IndexedApplications(initialContext) "
                "/\\ IndexedApplications(initialContext)' = "
                "IndexedApplications(initialContext) \\cup {application} "
                "/\\ IndexedDecisions(initialContext)' = "
                "IndexedDecisions(initialContext)"
            ),
            "NoNewIndexedDurableReceipt": (
                "/\\ IndexedDecisions(initialContext)' = "
                "IndexedDecisions(initialContext) "
                "/\\ IndexedApplications(initialContext)' = "
                "IndexedApplications(initialContext)"
            ),
            "IndexedReceiptFreeChainStutter": (
                "/\\ NoNewIndexedDurableReceipt(initialContext) "
                "/\\ UNCHANGED <<joinedByContext, "
                "SuccessorActivationVars, Chain!ChainEpochVars>>"
            ),
            "IndexedReceiptFreeAsyncAction": (
                "/\\ IndexedJoinedAsyncNext(initialContext) "
                "/\\ NoNewIndexedDurableReceipt(initialContext)"
            ),
            "IndexedFreshReceiptAsyncAction": (
                "/\\ IndexedJoinedAsyncNext(initialContext) "
                "/\\ \\/ \\E decision \\in Chain!DecisionEvidenceSet: "
                "NewIndexedDecisionReceipt(initialContext, decision) "
                "\\/ \\E application \\in Chain!DecisionEvidenceSet: "
                "NewIndexedApplicationReceipt(initialContext, application)"
            ),
            "IndexedReceiptClassification": (
                "\\/ IndexedReceiptFreeChainStutter(initialContext) "
                "\\/ \\E decision \\in Chain!DecisionEvidenceSet: "
                "IndexedDecisionReceiptHandoff(initialContext, decision) "
                "\\/ \\E application \\in Chain!DecisionEvidenceSet: "
                "IndexedApplicationReceiptHandoff(initialContext, application)"
            ),
            "IndexedDecisionReceiptHandoff": (
                "/\\ NewIndexedDecisionReceipt(initialContext, decision) "
                "/\\ UNCHANGED <<joinedByContext, SuccessorActivationVars>> "
                "/\\ \\/ Chain!RecordCertifiedNext(decision) "
                "\\/ Chain!RecordKnownDecision(decision)"
            ),
            "IndexedApplicationReceiptHandoff": (
                "/\\ NewIndexedApplicationReceipt(initialContext, application) "
                "/\\ \\/ /\\ ExactNodeLocationAt("
                "initialContext, application.node) "
                "/\\ Chain!RecordAppliedNext(application) "
                "/\\ QueueSuccessorActivation("
                "initialContext, application.node) "
                "/\\ UNCHANGED joinedByContext "
                "\\/ /\\ Chain!RecordKnownApplication(application) "
                "/\\ UNCHANGED <<joinedByContext, SuccessorActivationVars>>"
            ),
        }
        for symbol, exact_body in receipt_product_operators.items():
            require_chain_operator(symbol, exact=exact_body)
        require_chain_operator(
            "IndexedSuccessorActivationProgress",
            exact=(
                "\\A parentContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "IndexedSuccessorActivationPending(parentContext, node) "
                "~> SuccessorPublicationOrSuperseded(parentContext, node)"
            ),
        )
        require_chain_operator(
            "IndexedJoinedThroughLocalHeight",
            exact=(
                "\\A node \\in ValidatorIds, blockHeight \\in Heights: "
                "blockHeight <= nodeHeight[node] "
                "=> /\\ CanonicalIndexedContext(blockHeight) "
                "\\in AdmissibleContextRecords "
                "/\\ \\/ node \\in joinedByContext[ "
                "CanonicalIndexedContext(blockHeight)] "
                "\\/ /\\ blockHeight = nodeHeight[node] "
                "/\\ blockHeight > 0 "
                "/\\ LET parentContext == "
                "CanonicalIndexedContext(blockHeight - 1) "
                "IN /\\ successorActivationStatus[parentContext][node] "
                '\\in {"Queued", "Running"} '
                "/\\ \\E application \\in Chain!DecisionEvidenceSet: "
                "ExactDurableParentApplication( "
                "parentContext, node, application)"
            ),
        )
        require_chain_operator(
            "IndexedActivationPendingIntoContext",
            exact=(
                "IF initialContext.height = 0 THEN FALSE "
                "ELSE /\\ initialContext = "
                "CanonicalIndexedContext(initialContext.height) "
                "/\\ IndexedSuccessorActivationPending( "
                "CanonicalIndexedContext(initialContext.height - 1), node)"
            ),
        )
        require_chain_theorem(
            "IndexedActivationPendingIntoContextEventuallyJoins",
            (
                "/\\ IndexedChainSpec "
                "/\\ IndexedSuccessorActivationProgress "
                "=> \\A initialContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "IndexedActivationPendingIntoContext(initialContext, node) "
                "~> node \\in joinedByContext[initialContext]"
            ),
        )
        require_chain_theorem(
            "IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode",
            (
                "/\\ IndexedChainSpec "
                "/\\ IndexedSuccessorActivationProgress "
                "=> \\A targetContext \\in AdmissibleContextRecords: "
                "\\A blockHeight \\in 0..targetContext.height: "
                "(IndexedTargetJoined(targetContext) "
                "/\\ IndexedResponsiveHeightReached(blockHeight)) "
                "~> IndexedAllResponsiveJoined( "
                "IndexedAncestorContext(targetContext, blockHeight))"
            ),
        )
        require_chain_theorem_contract(
            "HeightLivenessFromOneHeightAndExactRecoveryProgress",
            exact=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "/\\ IndexedExactHistoricalRecoveryProgress "
                "/\\ IndexedSuccessorActivationProgress "
                "/\\ VerificationOneHeightCompletion "
                "=> IndexedHeightLivenessProperty"
            ),
            proof_required=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "VerificationJoinedTargetEventuallyReachesAndEscapes",
                "VerificationReachedEscapeEventuallyCompletes",
            ),
        )
        require_chain_operator(
            "IndexedHeightLivenessReleaseTarget",
            exact=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "=> IndexedHeightLivenessProperty"
            ),
        )
        if _top_level_theorem_body(
            raw_source,
            "HeightLivenessObligation",
            preserve_string_contents=True,
        ) is not None:
            errors.append(
                f"{refinement_path}: HeightLivenessObligation must live in the "
                "child chain-liveness module so it can consume the successor "
                "starvation theorem"
            )

        require_chain_operator(
            "SuccessorActivationRequiredPrerequisites",
            exact=(
                '{"DeferredStatus", "AdapterReady", "RuntimeReady", '
                '"ServicesReady", "StartupApplied", "ClocksArmed", '
                '"IngressOpen"}'
            ),
        )
        require_chain_operator(
            "QueueSuccessorActivation",
            required=(
                'successorActivationStatus[parentContext][node] = "Idle"',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                '![parentContext][node] = "Queued"',
                '![parentContext][node] = "Published"',
                "UNCHANGED <<successorActivationTokens,",
            ),
            forbidden=("joinedByContext'",),
        )
        require_chain_operator(
            "IndexedApplicationReceiptHandoff",
            required=(
                "Chain!RecordAppliedNext(application)",
                "QueueSuccessorActivation(initialContext, application.node)",
                "UNCHANGED joinedByContext",
                "Chain!RecordKnownApplication(application)",
            ),
            forbidden=("joinedByContext'",),
        )
        require_chain_operator(
            "ExactSuccessorActivationToken",
            required=(
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "SuccessorActivationToken( kind, parentContext, node, successorContext) \\in successorActivationTokens",
            ),
            forbidden=("successorContext.height =",),
        )
        require_chain_operator(
            "SuccessorActivationMarker",
            required=(
                "parentContext |-> parentContext",
                "successorContext |-> successorContext",
                "successorHeight |-> successorContext.height",
                "generation |-> 0",
                "view |-> 0",
                'transition |-> "SuccessorHeightActivated"',
            ),
        )
        require_chain_operator(
            "BeginSuccessorActivation",
            required=(
                'LET token == SuccessorActivationToken( "Applied", parentContext, node, successorContext)',
                'successorActivationStatus[parentContext][node] = "Queued"',
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                "successorActivationPrerequisites[parentContext][node] = {}",
                "token \\notin successorActivationTokens",
                '![parentContext][node] = "Running"',
                "ExactDurableParentApplication(parentContext, node, application)",
            ),
        )
        require_chain_operator(
            "BindAppliedSuccessorActivationToken",
            required=(
                '"Applied", parentContext, node, successorContext',
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "ExactDurableParentApplication(parentContext, node, application)",
            ),
        )

        phase_contracts = {
            "OpenDeferredSuccessorAdapter": (
                "successorActivationPrerequisites[parentContext][node] = {}",
                "SuccessorActivationAdapterPrerequisites",
            ),
            "ConstructSuccessorRuntime": (
                "= SuccessorActivationAdapterPrerequisites",
                "SuccessorActivationRuntimePrerequisites",
            ),
            "StartSuccessorServices": (
                "= SuccessorActivationRuntimePrerequisites",
                "SuccessorActivationServicePrerequisites",
            ),
            "ApplySuccessorStartupEffects": (
                "= SuccessorActivationServicePrerequisites",
                "SuccessorActivationStartupPrerequisites",
            ),
            "ArmSuccessorClocks": (
                "= SuccessorActivationStartupPrerequisites",
                "SuccessorActivationClockPrerequisites",
            ),
            "PrepareSuccessorActivationMarker": (
                "= SuccessorActivationClockPrerequisites",
                "marker \\notin preparedSuccessorActivationMarkers",
            ),
            "OpenSuccessorIngress": (
                "= SuccessorActivationClockPrerequisites",
                "marker \\in preparedSuccessorActivationMarkers",
                "SuccessorActivationRequiredPrerequisites",
            ),
        }
        for symbol, tokens in phase_contracts.items():
            require_chain_operator(
                symbol,
                required=(
                    "SuccessorActivationCredentialReady(",
                    *tokens,
                ),
            )

        require_chain_operator(
            "LatchAppliedSuccessorStartupFailure",
            required=(
                'successorActivationStatus[parentContext][node] = "Running"',
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                "owner \\notin successorActivationFailures",
                "ExactDurableParentApplication(parentContext, node, application)",
                "successorActivationFailures \\cup {owner}",
                "successorActivationFailureHistory \\cup {owner}",
                "UNCHANGED <<successorActivationStatus, successorPredecessorStatusOwnership,",
            ),
            forbidden=(
                '![parentContext][node] = "Queued"',
                '![parentContext][node] = "Absent"',
                "owner \\notin successorActivationFailureHistory",
                "joinedByContext'",
            ),
        )
        require_chain_operator(
            "LatchRecoveredSuccessorStartupFailure",
            required=(
                'successorActivationStatus[parentContext][node] = "Running"',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                '"Recovered", parentContext, node, successorContext',
                "ExactCompleteTipRecoveryAuthority(",
                "successorActivationFailures \\cup {owner}",
                "successorActivationFailureHistory \\cup {owner}",
                "UNCHANGED <<successorActivationStatus, successorPredecessorStatusOwnership,",
            ),
            forbidden=(
                "owner \\notin successorActivationFailureHistory",
                "joinedByContext'",
            ),
        )
        require_chain_operator(
            "RehydrateCleanCompleteTipSuccessorStartup",
            required=(
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                "owner \\notin successorActivationFailures",
                "ExactDurableParentApplication(parentContext, node, application)",
                '![parentContext][node] = "Queued"',
                '![parentContext][node] = "Absent"',
                "CompleteTipRecoveryAuthorityRecord(",
                "\\cup {authority}",
            ),
            forbidden=("owner \\in successorActivationFailureHistory",),
        )
        require_chain_operator(
            "RehydrateFailedSuccessorStartup",
            required=(
                'successorActivationStatus[parentContext][node] = "Running"',
                'successorPredecessorStatusOwnership[parentContext][node] \\in {"Published", "Absent"}',
                "owner \\in successorActivationFailures",
                '![parentContext][node] = "Queued"',
                '![parentContext][node] = "Absent"',
                "successorActivationFailures \\ {owner}",
                "CompleteTipRecoveryAuthorityRecord(",
                "\\cup {authority}",
            ),
        )
        require_chain_operator(
            "AuthenticateRecoveredSuccessorActivation",
            required=(
                '"Recovered", parentContext, node, successorContext',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                "owner \\notin successorActivationFailures",
                "ExactDurableParentApplication(parentContext, node, application)",
                "CompleteTipRecoveryAuthorityRecord(",
                "authority \\in successorRecoveryAuthorities",
                "successorActivationTokens \\cup {token}",
            ),
            forbidden=('"Applied", parentContext, node, successorContext',),
        )
        require_chain_operator(
            "ExactCompleteTipRecoveryAuthority",
            required=(
                "ExactDurableParentApplication(parentContext, node, application)",
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "CompleteTipRecoveryAuthorityRecord(",
                "\\in successorRecoveryAuthorities",
            ),
        )
        require_chain_operator(
            "CompleteTipRecoveryAuthoritySet",
            required=(
                'kind: {"CompleteTip"}',
                "application: Chain!DecisionEvidenceSet",
            ),
            forbidden=('"SnapshotBootstrap"',),
        )
        require_chain_operator(
            "SnapshotBootstrapRecoveryAuthoritySet",
            required=(
                'kind: {"SnapshotBootstrap"}',
                "parentContext: AdmissibleContextRecords",
                "successorContext: AdmissibleContextRecords",
            ),
            forbidden=("application: Chain!DecisionEvidenceSet",),
        )
        require_chain_operator(
            "SuccessorRecoveryAuthoritySet",
            exact=(
                "CompleteTipRecoveryAuthoritySet \\cup "
                "SnapshotBootstrapRecoveryAuthoritySet"
            ),
        )
        require_chain_operator(
            "CompleteTipRecoveryAuthorityRecord",
            required=(
                'kind |-> "CompleteTip"',
                "application |-> application",
            ),
            forbidden=('kind |-> "SnapshotBootstrap"',),
        )
        require_chain_operator(
            "SnapshotBootstrapRecoveryAuthorityRecord",
            required=(
                'kind |-> "SnapshotBootstrap"',
                "successorContext |-> successorContext",
            ),
            forbidden=("application |-> application",),
        )
        require_chain_operator(
            "ExactSnapshotBootstrapRecoveryAuthority",
            required=(
                "successorContext = CanonicalIndexedContext(parentContext.height + 1)",
                "SnapshotBootstrapRecoveryAuthorityRecord(",
                "\\in successorRecoveryAuthorities",
            ),
            forbidden=(
                "ExactDurableParentApplication(",
                "CompleteTipRecoveryAuthorityRecord(",
            ),
        )
        require_chain_theorem(
            "SnapshotBootstrapAuthorityIsDistinctFromCompleteTipAuthority",
            (
                "\\A parentContext \\in AdmissibleContextRecords, "
                "node \\in ValidatorIds, "
                "successorContext \\in AdmissibleContextRecords, "
                "application \\in Chain!DecisionEvidenceSet: "
                "SnapshotBootstrapRecoveryAuthorityRecord( "
                "parentContext, node, successorContext) "
                "# CompleteTipRecoveryAuthorityRecord( "
                "parentContext, node, successorContext, application)"
            ),
        )
        require_chain_operator(
            "EventualFailureFreeSuccessorStartupSuffix",
            exact=(
                "\\A parentContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "<>[](SuccessorActivationOwner(parentContext, node) "
                "\\notin successorActivationFailures)"
            ),
        )
        require_chain_operator(
            "IndexedChainSpec",
            exact=(
                "/\\ IndexedChainInit "
                "/\\ [][IndexedChainNext]_IndexedChainVars "
                "/\\ IndexedFairness "
                "/\\ EventualFailureFreeSuccessorStartupSuffix"
            ),
        )
        require_chain_operator(
            "ActivateAppliedSuccessorHeight",
            required=(
                'ExactSuccessorActivationToken( "Applied", parentContext, node, successorContext)',
                'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
                'successorActivationStatus[parentContext][node] = "Running"',
                "= SuccessorActivationRequiredPrerequisites",
                "marker \\in preparedSuccessorActivationMarkers",
                '![parentContext][node] = "Complete"',
                '![parentContext][node] = "Absent"',
                "successorActivationCompletions \\cup {token}",
                "joinedByContext' =",
            ),
        )
        require_chain_operator(
            "ActivateRecoveredSuccessorHeight",
            required=(
                'ExactSuccessorActivationToken( "Recovered", parentContext, node, successorContext)',
                'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
                "ExactCompleteTipRecoveryAuthority(",
                "= SuccessorActivationRequiredPrerequisites",
                "marker \\in preparedSuccessorActivationMarkers",
                "UNCHANGED successorActivationStatus",
                "successorActivationCompletions \\cup {token}",
                "joinedByContext' =",
            ),
            forbidden=(
                '"Applied", parentContext, node, successorContext',
                '![parentContext][node] = "Complete"',
            ),
        )
        join_writes = len(re.findall(r"joinedByContext'\s*=", raw_source))
        if join_writes != 2:
            errors.append(
                f"{refinement_path}: exactly the Applied and Recovered "
                f"publication actions may write joinedByContext; found {join_writes} writes"
            )

        require_chain_operator(
            "IndexedHistoricalRecoveryTargetReady",
            exact=(
                "/\\ node \\in Responsive "
                "/\\ node \\in IndexedCore(initialContext, 6) "
                "/\\ node \\in joinedByContext[initialContext] "
                "/\\ ExactNodeLocationAt(initialContext, node) "
                "/\\ ~IndexedAsync(initialContext)!NodeHasDecision(node) "
                "/\\ ~IndexedProjectedNodeHasApplication(initialContext, node) "
                "/\\ ~IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)"
            ),
        )
        require_chain_operator(
            "IndexedHistoricalRecoverySourceReady",
            required=(
                "initialContext \\in JoinedContexts",
                "source \\in IndexedCurrentDecisions(initialContext)",
                "source \\in IndexedCurrentApplications(initialContext)",
                "source \\in durableDecisionEvidence",
                "source \\in durableApplicationEvidence",
                "source.node = server",
                "initialContext.height < MaxHeight",
                "Chain!CanonicalCommitForSlot(",
                "initialContext.height = MaxHeight",
                "Chain!ReceiptOutsideChainHorizon(source)",
                "server \\in IndexedAsync(initialContext)! AsyncCurrentResponsiveVoters",
                "server \\in IndexedCore(initialContext, 6)",
                "server \\in joinedByContext[initialContext]",
                "BodyHeldBy(IndexedCore(initialContext, 9), server,",
            ),
            forbidden=("VotingRoster", "server \\in source.qc.signers"),
        )
        require_chain_operator(
            "IndexedHistoricalRecoveryReady",
            exact=(
                "/\\ node \\in Responsive "
                "/\\ node \\in IndexedCore(initialContext, 6) "
                "/\\ node \\in joinedByContext[initialContext] "
                "/\\ ExactNodeLocationAt(initialContext, node) "
                "/\\ ~IndexedAsync(initialContext)!NodeHasDecision(node) "
                "/\\ ~IndexedProjectedNodeHasApplication(initialContext, node) "
                "/\\ \\E server \\in ValidatorIds, "
                "source \\in Chain!DecisionEvidenceSet: "
                "IndexedHistoricalRecoverySourceReady( "
                "initialContext, server, source)"
            ),
        )
        require_chain_operator(
            "IndexedOpenHistoricalRecovery",
            exact=(
                "/\\ IndexedHistoricalRecoveryTargetReady(initialContext, node) "
                "/\\ IndexedHistoricalRecoverySourceReady( "
                "initialContext, server, source) "
                "/\\ IndexedAsync(initialContext)!OpenHistoricalRecovery(node)"
            ),
        )
        require_chain_operator(
            "IndexedJoinedRunnerStep",
            required=(
                "\\E node \\in Responsive: IndexedAsync(initialContext)!RunHistoricalRecoveryNode(node)",
                "IndexedNodeCurrentAt(initialContext, node)",
                "IndexedAsync(initialContext)!RunNode(node)",
                "node \\in joinedByContext[initialContext]",
                "IndexedAsync(initialContext)!RunHistoricalServer(node)",
            ),
        )
        require_chain_operator(
            "IndexedJoinedNonRunnerStep",
            required=(
                "\\E node \\in Responsive: IndexedAsync(initialContext)! DirectHistoricalCommitCertificateDiscoveryStep(node)",
                "\\E node \\in Responsive: IndexedAsync(initialContext)! ServiceHistoricalRecoveryIoWorker(node)",
                "\\E node \\in Responsive: IndexedAsync(initialContext)! EnqueueHistoricalRecoveryIoLocalControl(node)",
                "IndexedOpenHistoricalRecovery( initialContext, node, server, source)",
                "\\E node \\in IndexedAsync(initialContext)!"
                "AsyncCurrentResponsiveVoters: /\\ "
                "IndexedNodeCurrentAt(initialContext, node) /\\ "
                "IndexedAsync(initialContext)! "
                "ResolveCandidateProducerContinuation(node)",
                "UNCHANGED IndexedScheduler(initialContext, 33)",
            ),
        )
        require_chain_operator(
            "IndexedProductActionAt",
            exact=(
                "/\\ IndexedJoinedAsyncNext(initialContext) "
                "/\\ \\A otherContext \\in AdmissibleContextRecords "
                "\\ {initialContext}: UNCHANGED IndexedAsyncStateAt(otherContext) "
                "/\\ IndexedAsyncStateShape' "
                "/\\ JoinedByContextShape' "
                "/\\ SuccessorActivationShape' "
                "/\\ IndexedReceiptClassification(initialContext)"
            ),
        )
        require_chain_operator(
            "IndexedChainNext",
            exact=(
                "/\\ IndexedAsyncStateShape "
                "/\\ JoinedByContextShape "
                "/\\ SuccessorActivationShape "
                "/\\ \\/ \\E initialContext \\in JoinedContexts: "
                "IndexedProductActionAt(initialContext) "
                "\\/ \\E parentContext \\in AdmissibleContextRecords, "
                "node \\in ValidatorIds: "
                "IndexedSuccessorActivationProgressStep(parentContext, node)"
            ),
        )
        historical_product_steps = {
            "IndexedOpenHistoricalRecoveryStep": (
                "/\\ IndexedChainNext "
                "/\\ \\E server \\in ValidatorIds, "
                "source \\in Chain!DecisionEvidenceSet: "
                "IndexedOpenHistoricalRecovery( "
                "initialContext, node, server, source)"
            ),
            "IndexedRunHistoricalRecoveryStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstRunHistoricalRecoveryNode(node)"
            ),
            "IndexedHistoricalCommitCertificateDiscoveryStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstHistoricalCommitCertificateDiscovery(node)"
            ),
            "IndexedHistoricalRecoveryIoWorkerStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstServiceHistoricalRecoveryIoWorker(node)"
            ),
            "IndexedAdmitHistoricalRecoveryPacketStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstAdmitHistoricalRecoveryPacket(recipient, source)"
            ),
            "IndexedResolveLocalProducerContinuationStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedNodeCurrentAt(initialContext, node) "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstResolveLocalCandidateProducerContinuation(node)"
            ),
            "IndexedServiceConditionalProducerContinuationStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedNodeCurrentAt(initialContext, node) "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstServiceConditionalTransportProducerContinuation(node)"
            ),
            "IndexedServiceVolatileProducerContinuationStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedNodeCurrentAt(initialContext, node) "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstServiceVolatileBodyProducerContinuation(node)"
            ),
            "IndexedRetireLeaderWireLifecycleStep": (
                "/\\ IndexedChainNext "
                "/\\ IndexedAsync(initialContext)! "
                "PostGstRetireLeaderWireLifecycleSlot(slot)"
            ),
        }
        for symbol, exact_body in historical_product_steps.items():
            require_chain_operator(symbol, exact=exact_body)
        require_chain_operator(
            "IndexedHistoricalRecoveryTargetCoherence",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords, "
                "node \\in ValidatorIds: "
                "IndexedAsync(initialContext)!HistoricalRecoveryTarget(node) "
                "=> /\\ node \\in Responsive "
                "/\\ node \\in joinedByContext[initialContext] "
                "/\\ ExactNodeLocationAt(initialContext, node) "
                "/\\ ~IndexedAsync(initialContext)!NodeHasApplication(node)"
            ),
        )
        require_chain_operator(
            "HistoricalRecoveryOutstanding",
            exact=(
                "/\\ node \\in Responsive "
                "/\\ node \\in joinedByContext[initialContext] "
                "/\\ ExactNodeLocationAt(initialContext, node) "
                "/\\ ~IndexedAsync(initialContext)!NodeHasApplication(node)"
            ),
        )
        require_chain_operator(
            "HistoricalRecoveryProgressEligible",
            exact=(
                "/\\ HistoricalRecoveryOutstanding(initialContext, node) "
                "/\\ \\/ IndexedHistoricalRecoveryReady(initialContext, node) "
                "\\/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node) "
                "\\/ IndexedAsync(initialContext)!NodeHasDecision(node)"
            ),
        )
        require_chain_operator(
            "HistoricalRecoveryComplete",
            exact=(
                "IF initialContext.height = MaxHeight "
                "THEN IndexedAsync(initialContext)!NodeHasApplication(node) "
                "ELSE nodeHeight[node] > initialContext.height"
            ),
        )
        require_chain_operator(
            "IndexedExactHistoricalRecoveryProgress",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "HistoricalRecoveryOutstanding(initialContext, node) "
                "~> HistoricalRecoveryComplete(initialContext, node)"
            ),
        )
        require_chain_operator(
            "IndexedAllResponsiveExactApplicationsAt",
            exact=(
                "\\A node \\in Responsive: "
                "IndexedAsync(initialContext)!NodeHasApplication(node)"
            ),
        )
        require_chain_operator(
            "IndexedContextCompleted",
            exact=(
                "IF initialContext.height = MaxHeight "
                "THEN IndexedAllResponsiveExactApplicationsAt(initialContext) "
                "ELSE \\A node \\in Responsive: "
                "nodeHeight[node] > initialContext.height"
            ),
        )
        require_chain_operator(
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant",
            required=(
                "SuccessorActivationShape",
                "SuccessorHeightActivated(parentContext, node)",
                "node \\in joinedByContext[ CanonicalIndexedContext( parentContext.height + 1)]",
            ),
            forbidden=(
                "terminalContext",
                "ProductionTerminal",
                'successorActivationStatus[terminalContext][node] = "Idle"',
            ),
        )
        production_trace_constants = (
            "ProductionAppliedSuccessorTraceRefinesIndexedActivation",
            "ProductionRecoveredSuccessorTraceRefinesIndexedActivation",
            "ProductionStartupFailureAndRestartRefinesIndexedLifecycle",
            "ProductionHistoricalCertificateTraceRefinesIndexedAsync",
            "ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync",
            "ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal",
        )
        production_constant_block = re.search(
            rf"(?ms)^CONSTANTS\s+({re.escape(production_trace_constants[0])}"
            r".*?)(?=^\S)",
            strip_tla_comments(raw_source, preserve_string_contents=True),
        )
        if production_constant_block is None:
            errors.append(
                f"{refinement_path}: missing explicit production successor/exact-"
                "recovery trace constants"
            )
        else:
            declared_trace_constants = tuple(
                re.findall(
                    r"[A-Za-z_][A-Za-z0-9_]*",
                    production_constant_block.group(1),
                )
            )
            if declared_trace_constants != production_trace_constants:
                errors.append(
                    f"{refinement_path}: production successor/exact-recovery "
                    "trace constants must equal the exact ordered six-claim "
                    f"inventory {production_trace_constants!r}; found "
                    f"{declared_trace_constants!r}"
                )
        require_chain_operator(
            "ProductionSuccessorAndExactRecoveryTraceRefinement",
            exact=(
                "/\\ ProductionAppliedSuccessorTraceRefinesIndexedActivation = TRUE "
                "/\\ ProductionRecoveredSuccessorTraceRefinesIndexedActivation = TRUE "
                "/\\ ProductionStartupFailureAndRestartRefinesIndexedLifecycle = TRUE "
                "/\\ ProductionHistoricalCertificateTraceRefinesIndexedAsync = TRUE "
                "/\\ ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync = TRUE "
                "/\\ ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal = TRUE"
            ),
        )
        for retired_terminal_claim in (
            "ProductionTerminalApplicationExcludesActivation",
            "ProductionTerminalSuccessorKernel",
            "ProductionMaxHeightTerminalKernel",
        ):
            if retired_terminal_claim in strip_tla_comments(raw_source):
                errors.append(
                    f"{refinement_path}: production terminal claim/kernel "
                    f"{retired_terminal_claim} is prohibited; MaxHeight is only "
                    "a finite-horizon projection"
                )
        exact_production_obligation = (
            "/\\ ProductionSuccessorAndExactRecoveryTraceRefinement "
            "/\\ (IndexedChainSpec => []"
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)"
        )
        production_obligation = _top_level_operator_body(
            raw_source,
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
            preserve_string_contents=True,
        )
        swapped_production_obligation = _top_level_theorem_body(
            raw_source,
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
            preserve_string_contents=True,
        )
        if production_obligation is None or swapped_production_obligation is not None:
            errors.append(
                f"{refinement_path}: canonical exact historical-recovery "
                "production refinement obligation must be one operator, not a "
                "proofless theorem"
            )
        else:
            obligation_body, obligation_line = production_obligation
            obligation_normalized = " ".join(obligation_body.split())
            if obligation_normalized != exact_production_obligation:
                errors.append(
                    f"{refinement_path}:{obligation_line}: canonical exact "
                    "historical-recovery production refinement obligation must "
                    f"state only {exact_production_obligation!r}; found "
                    f"{obligation_normalized!r}"
                )

        production_bridge = _top_level_theorem_body(
            raw_source,
            "SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement",
            preserve_string_contents=True,
        )
        exact_bridge_statement = (
            "ProductionSuccessorAndExactRecoveryTraceRefinement => "
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
        )
        exact_bridge_proof = (
            "BY IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant "
            "DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
        )
        if production_bridge is None:
            errors.append(
                f"{refinement_path}: missing exact historical-recovery cross-tool "
                "bridge theorem"
            )
        else:
            bridge_body, bridge_line = production_bridge
            bridge_parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
                bridge_body,
                maxsplit=1,
            )
            bridge_statement = " ".join(bridge_parts[0].split())
            bridge_proof = (
                " ".join(bridge_parts[1].split())
                if len(bridge_parts) == 2
                else ""
            )
            if bridge_statement != exact_bridge_statement:
                errors.append(
                    f"{refinement_path}:{bridge_line}: exact historical-recovery "
                    "cross-tool bridge must state only "
                    f"{exact_bridge_statement!r}; found {bridge_statement!r}"
                )
            if bridge_proof != exact_bridge_proof:
                errors.append(
                    f"{refinement_path}:{bridge_line}: exact historical-recovery "
                    "cross-tool bridge must retain reviewed non-tautological proof "
                    f"{exact_bridge_proof!r}; found {bridge_proof!r}"
                )
        if indexed_fairness is not None:
            fairness_normalized = normalize_chain_contract(indexed_fairness[0])
            exact_historical_fairness = (
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedOpenHistoricalRecoveryStep(initialContext, node))",
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedRunHistoricalRecoveryStep(initialContext, node))",
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedHistoricalCommitCertificateDiscoveryStep( "
                "initialContext, node))",
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedHistoricalRecoveryIoWorkerStep( initialContext, node))",
                "\\A recipient \\in ValidatorIds, "
                "source \\in IndexedAsync(initialContext)!AsyncIngressSources: "
                "WF_IndexedChainVars( IndexedAdmitHistoricalRecoveryPacketStep( "
                "initialContext, recipient, source))",
            )
            exact_responsive_service_fairness = (
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedHistoricalServerStep(initialContext, node))",
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedIoWorkerStep(initialContext, node))",
            )
            exact_ordinary_packet_fairness = (
                "\\A recipient \\in Responsive, "
                "source \\in IndexedAsync(initialContext)!AsyncIngressSources: "
                "WF_IndexedChainVars( "
                "IndexedAdmitPacketStep(initialContext, recipient, source))"
            )
            exact_producer_continuation_fairness = (
                "\\A node \\in IndexedAsync(initialContext)!"
                "AsyncVotersAt(initialContext): /\\ "
                "WF_IndexedChainVars( "
                "IndexedResolveLocalProducerContinuationStep( "
                "initialContext, node)) /\\ "
                "WF_IndexedChainVars( "
                "IndexedServiceConditionalProducerContinuationStep( "
                "initialContext, node)) /\\ "
                "WF_IndexedChainVars( "
                "IndexedServiceVolatileProducerContinuationStep( "
                "initialContext, node))"
            )
            exact_leader_wire_retire_fairness = (
                "\\A slot \\in IndexedAsync(initialContext)!"
                "AsyncLeaderWireLifecycleSlotSet: WF_IndexedChainVars( "
                "IndexedRetireLeaderWireLifecycleStep(initialContext, slot))"
            )
            activation_fairness = (
                "\\A node \\in Responsive: WF_IndexedChainVars( "
                "IndexedSuccessorActivationProgressStep( initialContext, node))"
            )
            for exact_fairness in exact_historical_fairness:
                normalized_fairness = normalize_chain_contract(exact_fairness)
                if fairness_normalized.count(normalized_fairness) != 1:
                    errors.append(
                        f"{refinement_path}:{indexed_fairness[1]}: "
                        "IndexedFairness must contain exactly one all-required-"
                        "node exact historical-recovery product clause "
                        f"{normalized_fairness!r}"
                    )
            for exact_fairness in exact_responsive_service_fairness:
                normalized_fairness = normalize_chain_contract(exact_fairness)
                if fairness_normalized.count(normalized_fairness) != 1:
                    errors.append(
                        f"{refinement_path}:{indexed_fairness[1]}: "
                        "IndexedFairness must contain exactly one Responsive "
                        "joined archive-service product clause "
                        f"{normalized_fairness!r}"
                    )
            normalized_ordinary_packet_fairness = normalize_chain_contract(
                exact_ordinary_packet_fairness
            )
            if (
                fairness_normalized.count(normalized_ordinary_packet_fairness)
                != 1
            ):
                errors.append(
                    f"{refinement_path}:{indexed_fairness[1]}: "
                    "IndexedFairness must contain exactly one ordinary packet "
                    "clause over Responsive x AsyncIngressSources"
                )
            for label, exact_fairness in (
                (
                    "three current-voter producer continuations",
                    exact_producer_continuation_fairness,
                ),
                (
                    "bounded leader-wire retirement",
                    exact_leader_wire_retire_fairness,
                ),
            ):
                normalized_fairness = normalize_chain_contract(exact_fairness)
                if fairness_normalized.count(normalized_fairness) != 1:
                    errors.append(
                        f"{refinement_path}:{indexed_fairness[1]}: "
                        "IndexedFairness must contain exactly one "
                        f"{label} clause"
                    )
            normalized_activation_fairness = normalize_chain_contract(
                activation_fairness
            )
            if fairness_normalized.count(normalized_activation_fairness) != 1:
                errors.append(
                    f"{refinement_path}:{indexed_fairness[1]}: IndexedFairness "
                    "must contain exactly one responsive-validator fair "
                    "successor-activation pipeline"
                )
            fair_action_names = re.findall(
                r"WF_IndexedChainVars\s*\(\s*([A-Za-z][A-Za-z0-9_]*)",
                indexed_fairness[0],
            )
            canonical_fair_actions = (
                "IndexedSetGstStep",
                "IndexedTickStep",
                "IndexedRunNodeStep",
                "IndexedOpenHistoricalRecoveryStep",
                "IndexedRunHistoricalRecoveryStep",
                "IndexedCommitCertificateDiscoveryStep",
                "IndexedHistoricalCommitCertificateDiscoveryStep",
                "IndexedHistoricalServerStep",
                "IndexedIoWorkerStep",
                "IndexedHistoricalRecoveryIoWorkerStep",
                "IndexedResolveLocalProducerContinuationStep",
                "IndexedServiceConditionalProducerContinuationStep",
                "IndexedServiceVolatileProducerContinuationStep",
                "IndexedRetireLeaderWireLifecycleStep",
                "IndexedAdmitPacketStep",
                "IndexedAdmitHistoricalRecoveryPacketStep",
                "IndexedSuccessorActivationProgressStep",
            )
            fair_action_counts = {
                action: fair_action_names.count(action)
                for action in canonical_fair_actions
                if fair_action_names.count(action) != 1
            }
            unexpected_fair_actions = sorted(
                set(fair_action_names).difference(canonical_fair_actions)
            )
            if (
                fair_action_counts
                or unexpected_fair_actions
                or len(fair_action_names) != len(canonical_fair_actions)
            ):
                errors.append(
                    f"{refinement_path}:{indexed_fairness[1]}: "
                    "IndexedFairness must name exactly the 17 canonical indexed "
                    "product fair actions; "
                    f"counts={fair_action_counts!r}, "
                    f"unexpected={unexpected_fair_actions!r}, "
                    f"total={len(fair_action_names)}"
                )

    liveness_path = formal_dir / "SumeragiV2ChainLivenessProofs.tla"
    if not liveness_path.is_file():
        errors.append(
            f"{liveness_path}: missing non-circular chain temporal composition"
        )
    else:
        liveness_raw = liveness_path.read_text(encoding="utf-8")
        liveness_source = strip_tla_comments(
            liveness_raw, preserve_string_contents=True
        )
        extends = re.search(r"(?m)^EXTENDS\s+([^\n]+)$", liveness_source)
        exact_extends = (
            "SumeragiV2HistoricalRecoveryTemporalClosureProofs, TLAPS"
        )
        if extends is None or " ".join(extends.group(1).split()) != exact_extends:
            errors.append(
                f"{liveness_path}: chain temporal composition must extend "
                f"exactly {exact_extends!r} so historical residuals and "
                "successor progress are parent theorems rather than "
                "impossible child dependencies"
            )

        def require_liveness_operator(symbol: str, exact: str) -> None:
            extracted = _top_level_operator_body(
                liveness_raw, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{liveness_path}: missing chain temporal operator {symbol}"
                )
                return
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact:
                errors.append(
                    f"{liveness_path}:{line}: {symbol} must equal only "
                    f"{exact!r}; found {normalized!r}"
                )

        require_liveness_operator(
            "HistoricalRecoveryOpenOutcome",
            exact=(
                "\\/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node) "
                "\\/ IndexedAsync(initialContext)!NodeHasDecision(node) "
                "\\/ IndexedAsync(initialContext)!NodeHasApplication(node)"
            ),
        )
        require_liveness_operator(
            "IndexedHistoricalRecoveryTargetDecisionProgress",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "IndexedAsync(initialContext)!HistoricalRecoveryTarget(node) "
                "~> IndexedAsync(initialContext)!NodeHasDecision(node)"
            ),
        )
        require_liveness_operator(
            "IndexedResponsiveDecisionApplicationProgress",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "IndexedAsync(initialContext)!NodeHasDecision(node) "
                "~> IndexedAsync(initialContext)!NodeHasApplication(node)"
            ),
        )
        require_liveness_operator(
            "IndexedHistoricalRecoveryAsyncTemporalPrerequisites",
            exact=(
                "/\\ IndexedHistoricalRecoveryTargetDecisionProgress "
                "/\\ IndexedResponsiveDecisionApplicationProgress"
            ),
        )
        require_liveness_operator(
            "IndexedHistoricalRecoveryEligibilityProgress",
            exact=(
                "\\A initialContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: "
                "HistoricalRecoveryOutstanding(initialContext, node) "
                "~> HistoricalRecoveryProgressEligible(initialContext, node)"
            ),
        )
        require_liveness_operator(
            "IndexedHistoricalRecoveryTemporalPrerequisites",
            exact=(
                "/\\ IndexedHistoricalRecoveryEligibilityProgress "
                "/\\ IndexedHistoricalRecoveryAsyncTemporalPrerequisites"
            ),
        )

        def require_liveness_theorem(
            symbol: str,
            exact_statement: str,
            required_proof_tokens: tuple[str, ...] = (),
        ) -> None:
            extracted = _top_level_theorem_body(
                liveness_raw, symbol, preserve_string_contents=True
            )
            if extracted is None:
                errors.append(
                    f"{liveness_path}: missing chain temporal theorem {symbol}"
                )
                return
            body, line = extracted
            parts = re.split(
                r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
            )
            statement = " ".join(parts[0].split())
            if statement != exact_statement:
                errors.append(
                    f"{liveness_path}:{line}: {symbol} must state only "
                    f"{exact_statement!r}; found {statement!r}"
                )
            proof = parts[1] if len(parts) == 2 else ""
            missing = tuple(
                token
                for token in required_proof_tokens
                if not _tla_dependency_present(proof, token)
            )
            if missing:
                errors.append(
                    f"{liveness_path}:{line}: {symbol} proof must retain exact "
                    f"temporal dependencies {missing!r}"
                )

        require_liveness_theorem(
            "IndexedChainSpecEventuallyOpensReadyHistoricalRecovery",
            exact_statement=(
                "\\A initialContext \\in AdmissibleContextRecords, "
                "node \\in Responsive: IndexedChainSpec => "
                "(IndexedHistoricalRecoveryReady(initialContext, node) "
                "~> HistoricalRecoveryOpenOutcome(initialContext, node))"
            ),
            required_proof_tokens=(
                "WF_IndexedChainVars(",
                "IndexedOpenHistoricalRecoveryStep(initialContext, node)",
                "IndexedHistoricalRecoveryReadyPersistsOrOpens",
                "IndexedHistoricalRecoveryReadyEnablesExactOpen",
                "IndexedExactOpenRecordsHistoricalRecoveryTarget",
            ),
        )
        require_liveness_theorem(
            "IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites",
            exact_statement=(
                "/\\ IndexedChainSpec "
                "/\\ IndexedHistoricalRecoveryTemporalPrerequisites "
                "=> IndexedExactHistoricalRecoveryProgress"
            ),
            required_proof_tokens=(
                "IndexedChainSpecEventuallyOpensReadyHistoricalRecovery",
                "IndexedHistoricalRecoveryEligibilityProgress",
                "IndexedHistoricalRecoveryTargetDecisionProgress",
                "IndexedResponsiveDecisionApplicationProgress",
                "HistoricalRecoveryProgressEligible",
                "HistoricalRecoveryComplete",
            ),
        )
        require_liveness_theorem(
            "IndexedSuccessorActivationProgressFromStarvationProof",
            exact_statement=(
                "IndexedChainSpec => IndexedSuccessorActivationProgress"
            ),
            required_proof_tokens=(
                "SuccessorActivationStarvationFreedomObligation",
                "SuccessorActivationStarvationMatchesChainProgress",
            ),
        )
        require_liveness_theorem(
            "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress",
            exact_statement=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "/\\ IndexedExactHistoricalRecoveryProgress "
                "/\\ IndexedSuccessorActivationProgress "
                "/\\ VerificationOneHeightCompletion "
                "=> IndexedExactHeightLivenessProperty"
            ),
            required_proof_tokens=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "HeightLivenessFromOneHeightAndExactRecoveryProgress",
                "IndexedProjectedCompletionReachesExactCompletion",
            ),
        )
        require_liveness_theorem(
            "IndexedExactHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs",
            exact_statement=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "/\\ IndexedHistoricalRecoveryTemporalPrerequisites "
                "=> IndexedExactHeightLivenessProperty"
            ),
            required_proof_tokens=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites",
                "IndexedSuccessorActivationProgressFromStarvationProof",
                "VerificationOneHeightCompletionObligation",
                "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress",
            ),
        )
        require_liveness_theorem(
            "IndexedHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs",
            exact_statement=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "/\\ IndexedHistoricalRecoveryTemporalPrerequisites "
                "=> IndexedHeightLivenessProperty"
            ),
            required_proof_tokens=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites",
                "IndexedSuccessorActivationProgressFromStarvationProof",
                "VerificationOneHeightCompletionObligation",
                "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress",
            ),
        )
        require_liveness_theorem(
            "IndexedHeightLivenessFromHistoricalReleaseResidualsAndSuccessorProofs",
            exact_statement=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
                "=> IndexedHeightLivenessProperty"
            ),
            required_proof_tokens=(
                "IndexedLiveChainSpecProjectsIndexedChainSpec",
                "IndexedHistoricalReleaseResidualsDischargeExactProgress",
                "IndexedSuccessorActivationProgressFromStarvationProof",
                "VerificationOneHeightCompletionObligation",
                "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress",
                "IndexedExactContextCompletionImpliesProjectedCompletion",
            ),
        )
        require_liveness_theorem(
            "IndexedHeightLivenessFromFixedDeadlineDisseminationAndExposureProofs",
            exact_statement=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "/\\ IndexedLocalAdequateLeaderFixedDeadlineAnd"
                "ResponsiveDisseminationProperty "
                "/\\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty "
                "=> IndexedHeightLivenessProperty"
            ),
            required_proof_tokens=(
                "IndexedAdequateLeaderFixedDeadlineDisseminationAnd"
                "ExposureSupplyLocalConvergence",
                "IndexedHeightLivenessFromHistoricalReleaseResidualsAndSuccessorProofs",
            ),
        )
        require_liveness_theorem(
            "HeightLivenessObligation",
            exact_statement=(
                "/\\ IndexedLiveChainSpec "
                "/\\ IndexedGstEventuallyCondition "
                "=> IndexedHeightLivenessProperty"
            ),
            required_proof_tokens=(
                "IndexedLiveChainSpecProvidesLocalAdequateLeaderFixedDeadlineAnd"
                "ResponsiveDissemination",
                "IndexedLiveChainSpecProvidesLocalAdequateLeaderFreshSelfCorridorExposure",
                "IndexedHeightLivenessFromFixedDeadlineDisseminationAndExposureProofs",
            ),
        )

        if re.search(r"(?m)^CONSTANTS?\b", liveness_source):
            errors.append(
                f"{liveness_path}: chain temporal composition may not replace "
                "the two Async progress properties with unconstrained constants"
            )
    errors.extend(_successor_activation_rank_source_fidelity_errors(formal_dir))
    errors.extend(_successor_production_source_fidelity_errors(ROOT_DIR))
    return errors

def _retired_path_present(path: Path) -> bool:
    """Treat an empty, untracked legacy directory as absent."""

    if path.is_dir() and not path.is_symlink():
        return any(path.iterdir())
    return path.exists()


def _nightly_chaos_cold_cache_errors(repo_root: Path) -> list[str]:
    """Pin the online prefetch/offline chaos boundary for a cold Cargo cache."""

    harness_path = repo_root / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    lock_path = repo_root / "scripts" / "formal" / "sumeragi_v2_harness.lock"
    launcher_path = repo_root / "scripts" / "run_sumeragi_v2_100k_chaos.sh"
    workflow_path = repo_root / ".github" / "workflows" / "nightly_sumeragi_formal.yml"
    required_paths = (harness_path, lock_path, launcher_path, workflow_path)
    errors = [
        f"{path}: missing cold-cache chaos contract input"
        for path in required_paths
        if not path.is_file() or path.is_symlink()
    ]
    if errors:
        return errors

    harness = harness_path.read_text(encoding="utf-8")
    lock_declaration = (
        'readonly HARNESS_LOCK="${REPO_ROOT}/scripts/formal/'
        'sumeragi_v2_harness.lock"'
    )
    if harness.count(lock_declaration) != 1:
        errors.append(
            f"{harness_path}: harness must name the pinned standalone lock "
            "exactly once"
        )
    digest_matches = re.findall(
        r'(?m)^readonly HARNESS_LOCK_SHA256="([0-9a-f]{64})"$', harness
    )
    if len(digest_matches) != 1:
        errors.append(
            f"{harness_path}: harness must pin exactly one literal SHA-256 "
            "for the standalone lock"
        )
    else:
        actual_digest = hashlib.sha256(lock_path.read_bytes()).hexdigest()
        if digest_matches[0] != actual_digest:
            errors.append(
                f"{harness_path}: pinned standalone lock digest disagrees "
                f"with {lock_path}"
            )

    normalized_harness = " ".join(harness.split())
    exact_network_mode = (
        'if [[ "$1" == "--fetch" ]]; then export CARGO_NET_OFFLINE=false '
        "else export CARGO_NET_OFFLINE=true fi"
    )
    if exact_network_mode not in normalized_harness:
        errors.append(
            f"{harness_path}: only --fetch may run online and every test mode "
            "must force CARGO_NET_OFFLINE=true"
        )
    lock_validation_tokens = (
        '[[ ! -f "$HARNESS_LOCK" || -L "$HARNESS_LOCK"',
        '"$(hash_file "$HARNESS_LOCK")" != "$HARNESS_LOCK_SHA256"',
    )
    missing_lock_validation = [
        token for token in lock_validation_tokens if token not in normalized_harness
    ]
    if missing_lock_validation:
        errors.append(
            f"{harness_path}: standalone lock validation is incomplete; "
            f"missing {missing_lock_validation}"
        )

    wait_definition = "wait_for_external_cargo() {"
    exact_process_snapshot = "    ps -axo pid,etime,command"
    run_cargo_definition = (
        'run_cargo() {\n'
        "  wait_for_external_cargo\n"
        '  command cargo "$@"\n'
        "}"
    )
    if harness.count(wait_definition) != 1:
        errors.append(
            f"{harness_path}: harness must define exactly one Cargo/rustc "
            "quiescence wait"
        )
    if harness.count(exact_process_snapshot) != 1:
        errors.append(
            f"{harness_path}: harness must execute the exact "
            "`ps -axo pid,etime,command` snapshot"
        )
    if harness.count(run_cargo_definition) != 1:
        errors.append(
            f"{harness_path}: every harness Cargo command must use the exact "
            "wait_for_external_cargo/run_cargo wrapper"
        )
    direct_cargo_lines = [
        line
        for line in harness.splitlines()
        if re.match(r"^\s*(?:command\s+)?cargo(?:\s|$)", line)
    ]
    if direct_cargo_lines != ['  command cargo "$@"']:
        errors.append(
            f"{harness_path}: direct Cargo execution bypasses run_cargo; "
            f"found {direct_cargo_lines}"
        )
    fixed_modes = set(re.findall(r"(?m)^  (--[a-z0-9-]+)\)$", harness))
    expected_fixed_modes = {
        "--fetch",
        "--unit",
        "--fast-network",
        "--chaos-100k",
        "--model-replay",
        "--verus",
        "--clippy",
    }
    if fixed_modes != expected_fixed_modes:
        errors.append(
            f"{harness_path}: formal harness fixed-mode inventory is not exact; "
            f"expected {sorted(expected_fixed_modes)}, found {sorted(fixed_modes)}"
        )
    arbitrary_dispatch_tokens = ('"${@:2}"', "bash -c", "sh -c", "env cargo")
    retained_dispatch_tokens = [
        token for token in arbitrary_dispatch_tokens if token in harness
    ]
    if retained_dispatch_tokens:
        errors.append(
            f"{harness_path}: formal harness retains arbitrary child-command "
            f"dispatch tokens {retained_dispatch_tokens}"
        )
    if harness.count('"$@"') != 1:
        errors.append(
            f"{harness_path}: the argument vector may be forwarded only by the "
            "guarded run_cargo wrapper"
        )
    expected_verus_branch = """\
  --verus)
    if (($# != 1)); then
      echo "--verus accepts no additional arguments" >&2
      exit 2
    fi
    run_cargo verus verify --locked --offline -p iroha_sumeragi_core --features verus \\
      --fwd-verus-args-to roots -- \\
      --rlimit 60 \\
      --expand-errors \\
      --no-cheating
    ;;"""
    expected_clippy_branch = """\
  --clippy)
    if (($# != 1)); then
      echo "--clippy accepts no additional arguments" >&2
      exit 2
    fi
    run_cargo clippy --locked --offline -p iroha_sumeragi_core --lib -- -D warnings
    ;;"""
    if (
        harness.count(expected_verus_branch) != 1
        or harness.count(expected_clippy_branch) != 1
        or harness.count(
            'echo "positional harness commands are unsupported; '
            'select one fixed mode" >&2'
        )
        != 1
    ):
        errors.append(
            f"{harness_path}: formal harness must fail closed outside its exact "
            "reviewed Verus and Clippy command branches"
        )

    lock_copy = harness.find('cp -- "$HARNESS_LOCK" Cargo.lock')
    case_start = harness.find('case "$1" in')
    fetch_start = harness.find("  --fetch)", case_start)
    unit_start = harness.find("  --unit)", fetch_start)
    if not (0 <= lock_copy < case_start < fetch_start < unit_start):
        errors.append(
            f"{harness_path}: the verified standalone lock must be copied "
            "before dispatching --fetch or any offline test mode"
        )
        fetch_branch = ""
    else:
        fetch_branch = harness[fetch_start:unit_start]
    fetch_commands = re.findall(r"(?m)^\s*run_cargo fetch[^\n]*$", fetch_branch)
    if fetch_commands != ["    run_cargo fetch --locked"]:
        errors.append(
            f"{harness_path}: --fetch must perform exactly one online "
            f"guarded `run_cargo fetch --locked`; found {fetch_commands}"
        )

    chaos_start = harness.find("  --chaos-100k)", unit_start)
    unit_branch = (
        ""
        if unit_start < 0 or chaos_start < 0
        else harness[unit_start:chaos_start]
    )
    required_unit_inventory_tokens = (
        "    if ((${#listed_unit_tests[@]} != 137)); then",
        '      echo "expected exactly 137 Sumeragi v2 reducer unit tests" >&2',
        "    if ((${#listed_ignored_unit_tests[@]} != 0)); then",
        '      echo "reducer unit gate requires all 137 tests to be runnable" >&2',
    )
    missing_unit_inventory_tokens = [
        token
        for token in required_unit_inventory_tokens
        if unit_branch.count(token) != 1
    ]
    if missing_unit_inventory_tokens:
        errors.append(
            f"{harness_path}: --unit must seal exactly 137 runnable "
            "source-shared tests; missing or repeated "
            f"{missing_unit_inventory_tokens}"
        )
    replay_start = harness.find("  --model-replay)", chaos_start)
    chaos_branch = (
        ""
        if chaos_start < 0 or replay_start < 0
        else harness[chaos_start:replay_start]
    )
    chaos_cargo_commands = re.findall(r"(?m)^\s*run_cargo test\b", chaos_branch)
    offline_chaos_commands = re.findall(
        r"(?m)^\s*run_cargo test --locked --offline "
        r"-p iroha_sumeragi_core\s*\\?$",
        chaos_branch,
    )
    if len(chaos_cargo_commands) != 2 or len(offline_chaos_commands) != 2:
        errors.append(
            f"{harness_path}: --chaos-100k inventory and execution must both "
            "remain --locked --offline"
        )

    launcher = launcher_path.read_text(encoding="utf-8")
    chaos_invocation = (
        "bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k"
    )
    if launcher.count(chaos_invocation) != 1:
        errors.append(
            f"{launcher_path}: source-attested chaos launcher must invoke "
            "the offline harness gate exactly once"
        )

    workflow = workflow_path.read_text(encoding="utf-8")
    job_match = re.search(
        r"(?ms)^  sumeragi-v2-chaos-100k:\n(?P<body>.*?)"
        r"(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        workflow,
    )
    if job_match is None:
        errors.append(
            f"{workflow_path}: missing independent sumeragi-v2-chaos-100k job"
        )
    else:
        job = job_match.group("body")
        cache_marker = (
            "- uses: Swatinem/rust-cache@"
            "e18b497796c12c097a38f9edb9d0641fb99eee32"
        )
        fetch_marker = (
            "run: bash scripts/formal/run_sumeragi_v2_harness.sh --fetch"
        )
        gate_marker = "run: bash scripts/run_sumeragi_v2_100k_chaos.sh"
        counts = {
            "cache": job.count(cache_marker),
            "fetch": job.count(fetch_marker),
            "source_attested_gate": job.count(gate_marker),
        }
        if counts != {"cache": 1, "fetch": 1, "source_attested_gate": 1}:
            errors.append(
                f"{workflow_path}: nightly chaos job must contain exactly one "
                f"cache, pinned prefetch, and source-attested gate; counts={counts}"
            )
        elif not (
            job.index(cache_marker)
            < job.index(fetch_marker)
            < job.index(gate_marker)
        ):
            errors.append(
                f"{workflow_path}: nightly --fetch must run after cache restore "
                "and before the source-attested chaos gate"
            )
    return errors

def _production_liveness_release_inventory_guard_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the independent shell guard to the reviewed release inventory."""

    errors: list[str] = []
    guard_path = (
        repo_root / "ci" / "check_sumeragi_v2_multilane_release_inventory.sh"
    )
    if not guard_path.is_file() or guard_path.is_symlink():
        return [
            f"{guard_path}: independent multilane release inventory guard "
            "must be a regular file"
        ]
    guard_source = guard_path.read_text(encoding="utf-8")
    observed_guard_sha256 = hashlib.sha256(guard_path.read_bytes()).hexdigest()
    if observed_guard_sha256 != _PRODUCTION_LIVENESS_INVENTORY_GUARD_SHA256:
        errors.append(
            f"{guard_path}: independent inventory guard source SHA-256 must equal "
            f"{_PRODUCTION_LIVENESS_INVENTORY_GUARD_SHA256}; found "
            f"{observed_guard_sha256}"
        )

    expected_count_line = (
        "readonly canonical_production_test_count="
        f"{_PRODUCTION_LIVENESS_RELEASE_COUNT}"
    )
    if guard_source.splitlines().count(expected_count_line) != 1:
        errors.append(
            f"{guard_path}: independent multilane release inventory guard must "
            f"seal exactly {_PRODUCTION_LIVENESS_RELEASE_COUNT} production tests"
        )

    guarded_modules = (
        "kura::tests",
        "sumeragi::authoritative_runtime_gate_tests",
        "sumeragi::serviced_candidate_store::tests",
        "sumeragi::v2_effects::tests",
        "sumeragi::v2::tests",
        "sumeragi::v2_runtime::tests",
        "merge_sidecar::tests",
        "sumeragi::v2_lane_work::tests",
        "sumeragi::v2_lifecycle_recovery::tests",
        "sumeragi::v2_runner::tests",
        "sumeragi::v2_worker::tests",
        "network::tests",
        "network::inbound_source_memory_bound_tests",
        "network::handle_update_tests",
        "network_relay_tests",
    )
    module_contract_counts = {
        module: count
        for _leg_id, module, count in _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
    }
    expected_changed_module_counts = {
        module: module_contract_counts.get(module) for module in guarded_modules
    }
    changed_module_blocks = re.findall(
        r"expected_changed_module_counts = (\{.*?\})\nif any\(",
        guard_source,
        flags=re.DOTALL,
    )
    observed_changed_module_counts: Any = None
    if len(changed_module_blocks) == 1:
        try:
            observed_changed_module_counts = ast.literal_eval(
                changed_module_blocks[0]
            )
        except (SyntaxError, ValueError):
            observed_changed_module_counts = None
    if (
        any(count is None for count in expected_changed_module_counts.values())
        or observed_changed_module_counts != expected_changed_module_counts
    ):
        errors.append(
            f"{guard_path}: independent guard changed-module counts must equal "
            "the exact reviewed release inventory"
        )

    expected_inventory_digest_literal = (
        f'    "{_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256[:32]}"\n'
        f'    "{_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256[32:]}"'
    )
    if guard_source.count(expected_inventory_digest_literal) != 1:
        errors.append(
            f"{guard_path}: independent guard canonical production TSV "
            f"SHA-256 must equal {_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256}"
        )

    guard_bindings = (
        (
            "require_exact_token \\\n"
            '  "$release_runner" \\\n'
            "  \"readonly expected_production_liveness_test_count="
            '${canonical_production_test_count}"',
            "release-runner production count",
        ),
        (
            "require_exact_token \\\n"
            '  "$release_receipt_writer" \\\n'
            '  "_PRODUCTION_TEST_COUNT = '
            '${canonical_production_test_count}"',
            "receipt-writer production count",
        ),
    )
    for binding, description in guard_bindings:
        if guard_source.count(binding) != 1:
            errors.append(
                f"{guard_path}: independent guard must bind the {description} "
                "exactly once"
            )

    release_path = repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    if not release_path.is_file() or release_path.is_symlink():
        errors.append(
            f"{release_path}: release runner invoking the independent inventory "
            "guard must be a regular file"
        )
    else:
        release_source = release_path.read_text(encoding="utf-8")
        invocation = "bash ci/check_sumeragi_v2_multilane_release_inventory.sh"
        if release_source.splitlines().count(invocation) != 1:
            errors.append(
                f"{release_path}: release runner must invoke the independent "
                "multilane inventory guard exactly once"
            )
    return errors


def _sumeragi_v2_package_layout_guard_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the package-local reducer guard and its verifier invocation."""

    errors: list[str] = []
    guard_path = repo_root / "scripts" / "check_sumeragi_v2_package_layout.sh"
    if not guard_path.is_file() or guard_path.is_symlink():
        return [
            f"{guard_path}: Sumeragi v2 package-layout guard must be a regular file"
        ]
    guard_source = guard_path.read_text(encoding="utf-8")
    observed_guard_sha256 = hashlib.sha256(guard_path.read_bytes()).hexdigest()
    if observed_guard_sha256 != _SUMERAGI_V2_PACKAGE_LAYOUT_GUARD_SHA256:
        errors.append(
            f"{guard_path}: package-layout guard source SHA-256 must equal "
            f"{_SUMERAGI_V2_PACKAGE_LAYOUT_GUARD_SHA256}; found "
            f"{observed_guard_sha256}"
        )

    required_fragments = (
        (
            "set -euo pipefail",
            "strict shell failure propagation",
        ),
        (
            'REFINEMENT_TESTS="$CORE_DIR/refinement_cases.rs"',
            "package-local refinement test identity",
        ),
        (
            'REFINEMENT_PIPELINE_TESTS="$CORE_DIR/refinement_cases/'
            'terminal_body_pipeline.rs"',
            "package-local terminal body-pipeline refinement test identity",
        ),
        (
            "if [[ \"$path_attribute_count\" != 1 || "
            "\"$reviewed_outer_path_count\" != 1 \\\n",
            "single reviewed path-attribute cardinality",
        ),
        (
            "'^#\\[cfg\\(test\\)\\]\\n#\\[path = "
            "\"refinement_cases.rs\"\\]\\nmod tests;$'",
            "adjacent test-only refinement split",
        ),
        (
            "reviewed_pipeline_include_count=\"$(\n"
            "  rg -F -c 'include!(\"refinement_cases/terminal_body_pipeline.rs\");'",
            "identity-preserving terminal body-pipeline refinement include",
        ),
        (
            "'include(_str|_bytes)?![[:space:]]*\\([[:space:]]*\"\\.\\.'",
            "parent-relative include rejection",
        ),
    )
    for fragment, description in required_fragments:
        if guard_source.count(fragment) != 1:
            errors.append(
                f"{guard_path}: package-layout guard must retain the exact "
                f"{description} contract once"
            )

    verify_path = repo_root / "scripts" / "verify_sumeragi_v2.sh"
    if not verify_path.is_file() or verify_path.is_symlink():
        errors.append(
            f"{verify_path}: Sumeragi v2 verifier invoking the package-layout "
            "guard must be a regular file"
        )
    else:
        verify_source = verify_path.read_text(encoding="utf-8")
        observed_verify_sha256 = hashlib.sha256(verify_path.read_bytes()).hexdigest()
        if observed_verify_sha256 != _SUMERAGI_V2_PACKAGE_LAYOUT_VERIFIER_SHA256:
            errors.append(
                f"{verify_path}: Sumeragi v2 verifier source SHA-256 must equal "
                f"{_SUMERAGI_V2_PACKAGE_LAYOUT_VERIFIER_SHA256}; found "
                f"{observed_verify_sha256}"
            )
        invocation = 'bash "$REPO_ROOT/scripts/check_sumeragi_v2_package_layout.sh"'
        if verify_source.splitlines().count(invocation) != 1:
            errors.append(
                f"{verify_path}: Sumeragi v2 verifier must invoke the "
                "package-layout guard exactly once"
            )
    return errors
