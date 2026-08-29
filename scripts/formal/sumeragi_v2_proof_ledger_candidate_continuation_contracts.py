# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _async_candidate_producer_continuation_contract_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the finite producer handoff, minimum action, and rank contract."""
    path = formal_dir / "SumeragiV2AsyncCandidateProducerContinuationProofs.tla"
    if not path.is_file() or path.is_symlink():
        return [
            f"{path}: producer-continuation contract must be a regular file"
        ]
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return [f"{path}: cannot read producer-continuation contract: {error}"]
    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    if not network_path.is_file() or network_path.is_symlink():
        return [
            f"{network_path}: producer-continuation implementation must be "
            "a regular file"
        ]
    try:
        network_source = network_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return [
            f"{network_path}: cannot read producer-continuation "
            f"implementation: {error}"
        ]
    errors: list[str] = []
    def require_continuation_physical_cut_operator(
        symbol: str,
        required: tuple[str, ...],
        required_fragments: tuple[str, ...] = (),
    ) -> None:
        extracted = _top_level_operator_body(
            network_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{network_path}: missing producer-continuation physical-cut "
                f"operator {symbol}"
            )
            return
        body, line = extracted
        tokens = set(tla_code_tokens(body))
        normalized = " ".join(body.split())
        missing = tuple(token for token in required if token not in tokens)
        missing_fragments = tuple(
            fragment
            for fragment in required_fragments
            if fragment not in normalized
        )
        if missing or missing_fragments:
            errors.append(
                f"{network_path}:{line}: {symbol} must retain the immutable "
                "producer-continuation physical cut and true pre-cut ingress "
                f"semantics; missing={missing!r}, "
                f"missing_fragments={missing_fragments!r}"
            )

    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRecord",
        ("physicalCut",),
        ("physicalCut |-> physicalCut",),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRecordSet",
        ("physicalCut", "Nat"),
        ("physicalCut \\in Nat",),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRecordAfterStep",
        ("record",),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationStateAfterDeparture",
        (
            "AsyncCandidateProducerContinuationRecord",
            (
                "AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn"
            ),
            "AsyncCandidateProducerContinuationPhysicalCutIn",
        ),
        (
            "AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn( "
            "state, candidate)",
            "AsyncCandidateProducerContinuationPhysicalCutIn( state, candidate)",
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRunnerMayPrecedeIngress",
        (
            "AsyncIngressSchedulerBarrierActive",
            "AsyncEarliestIngressPhysicalOrdinal",
            "physicalCut",
            "ordinal",
            "AsyncEarliestIngressSchedulerOrdinal",
        ),
        (
            "record.physicalCut <= AsyncEarliestIngressPhysicalOrdinal(node)",
            "AsyncEarliestIngressPhysicalOrdinal(node) < record.physicalCut",
            "record.ordinal <= AsyncEarliestIngressSchedulerOrdinal(node)",
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncTimeoutLifecyclePhysicalCut",
        ("asyncControlServiceState", "timeoutLifecyclePhysicalCut", "node"),
        ("asyncControlServiceState.timeoutLifecyclePhysicalCut[node]",),
    )
    require_continuation_physical_cut_operator(
        "AsyncTimeoutLifecyclePhysicalCutForStep",
        (
            "timeoutLifecycleOrdinal",
            "timeoutLifecyclePhysicalCut",
            "AsyncTimeoutLifecycleUsesRecordedOriginOrdinal",
            "physicalCut",
            "AsyncNextIngressPhysicalOrdinal",
        ),
        ("ELSE AsyncNextIngressPhysicalOrdinal(node)'",),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateLifecycleStateAfterTimeoutOwnership",
        (
            "timeoutLifecycleOrdinal",
            "timeoutLifecycleOrigin",
            "timeoutLifecyclePhysicalCut",
            "AsyncTimeoutLifecycleResetThisStep",
            "AsyncTimeoutLifecycleTransfersThisStep",
            "AsyncTimeoutLifecyclePhysicalCutForStep",
        ),
        ("!.timeoutLifecyclePhysicalCut",),
    )
    require_continuation_physical_cut_operator(
        "AsyncRetransmitLifecyclePhysicalCut",
        (
            "asyncControlServiceState",
            "retransmitLifecyclePhysicalCut",
            "node",
        ),
        ("asyncControlServiceState.retransmitLifecyclePhysicalCut[node]",),
    )
    require_continuation_physical_cut_operator(
        "AsyncRetransmitLifecyclePhysicalCutForStep",
        (
            "state",
            "retransmitLifecycleOrdinal",
            "retransmitLifecyclePhysicalCut",
            "AsyncNextIngressPhysicalOrdinal",
        ),
        ("ELSE AsyncNextIngressPhysicalOrdinal(node)'",),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
        (
            "retransmitLifecycleOrdinal",
            "retransmitLifecyclePhysicalCut",
            "AsyncRetransmitLifecycleResetThisStep",
            "AsyncRetransmitLifecycleEpisodeCompletesThisStep",
            "AsyncRetransmitLifecycleConsumesFreshOrdinal",
            "AsyncRetransmitLifecyclePhysicalCutForStep",
        ),
        ("!.retransmitLifecyclePhysicalCut",),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationMayPrecedeOwnedRetransmit",
        (
            "AsyncRetransmitLifecycleOwned",
            "sourcePhysicalOrdinal",
            "AsyncRetransmitLifecyclePhysicalCut",
            "ordinal",
            "AsyncRetransmitLifecycleOrdinal",
        ),
        (
            "record.sourcePhysicalOrdinal < "
            "AsyncRetransmitLifecyclePhysicalCut(node)",
            "record.ordinal < AsyncRetransmitLifecycleOrdinal(node)",
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationMayOwnRuntimeTurn",
        (
            "AsyncCandidateProducerContinuationScheduledPredecessorsFor",
            "AsyncCandidateProducerContinuationMayPrecedeOwnedTimeout",
            "AsyncCandidateProducerContinuationMayPrecedeOwnedRetransmit",
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationPhysicallyBehindOwnedTimeout",
        (
            "AsyncTimeoutLifecycleOrdinal",
            "sourcePhysicalOrdinal",
            "AsyncTimeoutLifecyclePhysicalCut",
        ),
        (
            "record.sourcePhysicalOrdinal >= "
            "AsyncTimeoutLifecyclePhysicalCut(node)",
        ),
    )
    require_continuation_physical_cut_operator(
        (
            "AsyncCandidateProducerContinuationRuntimePhysicallyEligible"
            "RecordsForNode"
        ),
        (
            (
                "AsyncCandidateProducerContinuationPhysicallyEligible"
                "ResolutionRecordsForNode"
            ),
            "AsyncCandidateProducerContinuationPhysicallyBehindOwnedTimeout",
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRuntimeSelectedResolutionRecord",
        (
            (
                "AsyncCandidateProducerContinuationRuntimePhysicallyEligible"
                "RecordsForNode"
            ),
            (
                "AsyncCandidateProducerContinuationRuntimeResolution"
                "PredecessorsFor"
            ),
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord",
        (
            (
                "AsyncCandidateProducerContinuationRuntimeSelected"
                "ResolutionRecord"
            ),
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode",
        (
            (
                "AsyncCandidateProducerContinuationRuntimePhysicallyEligible"
                "RecordsForNode"
            ),
            "AsyncCandidateProducerContinuationRunnerMayPrecedeIngress",
        ),
    )
    require_continuation_physical_cut_operator(
        "AsyncCandidateProducerContinuationRunnerResolutionRequired",
        (
            "AsyncCandidateProducerContinuationIngressResolutionRequired",
            (
                "AsyncCandidateProducerContinuationRunnerSelected"
                "ResolutionRecord"
            ),
            (
                "AsyncCandidateProducerContinuationRunnableResolution"
                "RecordsForNode"
            ),
        ),
    )
    require_continuation_physical_cut_operator(
        (
            "AsyncCandidateProducerContinuationEnqueueConsumesSelected"
            "ReplayReservation"
        ),
        (
            (
                "AsyncCandidateProducerContinuationRuntimeSelected"
                "ResolutionRecord"
            ),
            (
                "AsyncCandidateProducerContinuationRuntimeResolution"
                "Required"
            ),
            "asyncRunnerBudget",
        ),
    )

    for symbol, required_statement_tokens, required_proof_tokens in (
        (
            "AsyncCandidateProducerContinuationStepPreservesPhysicalCut",
            (
                "AsyncCandidateProducerContinuationRecordAfterStep",
                "physicalCut",
            ),
            ("AsyncCandidateProducerContinuationRecordAfterStep",),
        ),
        (
            "AsyncCandidateProducerContinuationReservationFreezesPhysicalCut",
            (
                "AsyncCandidateProducerContinuationStateAfterDeparture",
                (
                    "AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn"
                ),
                "AsyncCandidateProducerContinuationPhysicalCutIn",
                "physicalCut",
            ),
            (
                "AsyncCandidateProducerContinuationStateAfterDeparture",
                "AsyncCandidateProducerContinuationRecord",
            ),
        ),
        (
            "AsyncCandidateProducerContinuationPostCutIngressCannotBlockRunnerTurn",
            (
                "physicalCut",
                "AsyncEarliestIngressPhysicalOrdinal",
                (
                    "AsyncCandidateProducerContinuationRunnerResolutionRecords"
                    "ForNode"
                ),
            ),
            ("AsyncCandidateProducerContinuationRunnerMayPrecedeIngress",),
        ),
        (
            "AsyncCandidateProducerContinuationOnlyPreCutIngressCanBlockRunnerTurn",
            (
                "physicalCut",
                "AsyncEarliestIngressPhysicalOrdinal",
                "AsyncEarliestIngressSchedulerOrdinal",
            ),
            ("AsyncCandidateProducerContinuationRunnerMayPrecedeIngress",),
        ),
        (
            "AsyncCandidateProducerContinuationPostTimeoutCutCannotOwnRunnerTurn",
            (
                "AsyncTimeoutLifecyclePhysicalCut",
                "sourcePhysicalOrdinal",
                (
                    "AsyncCandidateProducerContinuationRunnableResolution"
                    "RecordsForNode"
                ),
            ),
            (
                "AsyncCandidateProducerContinuationMayPrecedeOwnedTimeout",
            ),
        ),
        (
            "AsyncCandidateProducerContinuationPostRetransmitCutCannotOwnRunnerTurn",
            (
                "AsyncRetransmitLifecyclePhysicalCut",
                "sourcePhysicalOrdinal",
                (
                    "AsyncCandidateProducerContinuationRunnableResolution"
                    "RecordsForNode"
                ),
            ),
            (
                "AsyncCandidateProducerContinuationMayPrecedeOwnedRetransmit",
            ),
        ),
        (
            (
                "AsyncCandidateProducerContinuationRuntimeSelectionIs"
                "LogicalMinimum"
            ),
            (
                (
                    "AsyncCandidateProducerContinuationRuntimeSelected"
                    "ResolutionRecord"
                ),
                (
                    "AsyncCandidateProducerContinuationRuntimePhysically"
                    "EligibleRecordsForNode"
                ),
                (
                    "AsyncCandidateProducerContinuationRuntimeResolution"
                    "PredecessorsFor"
                ),
            ),
            (
                "AsyncCandidateProducerContinuationLogicalOccurrenceRank",
                (
                    "AsyncCandidateProducerContinuationLogicalPredecessor"
                    "StrictlyLowersOccurrenceRank"
                ),
            ),
        ),
        (
            (
                "AsyncCandidateProducerContinuationRunnerSelectionIsTwoStage"
                "LogicalMinimum"
            ),
            (
                (
                    "AsyncCandidateProducerContinuationRuntimePhysically"
                    "EligibleRecordsForNode"
                ),
                (
                    "AsyncCandidateProducerContinuationRuntimeResolution"
                    "PredecessorsFor"
                ),
                (
                    "AsyncCandidateProducerContinuationRunnableResolution"
                    "RecordsForNode"
                ),
            ),
            (
                "AsyncCandidateProducerContinuationRuntimeSelectionIsLogicalMinimum",
            ),
        ),
        (
            "AsyncTimeoutLifecycleFreezeBoundaryMintsAfterPriorAdmissions",
            (
                "AsyncTimeoutLifecyclePhysicalCut",
                "AsyncNextIngressPhysicalOrdinal",
            ),
            (
                "AsyncCandidateLifecycleStateAfterTimeoutOwnership",
                "AsyncTimeoutLifecyclePhysicalCutForStep",
            ),
        ),
        (
            "AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint",
            (
                "AsyncTimeoutLifecyclePhysicalCut",
                "AsyncTimeoutLifecycleOrdinal",
            ),
            ("AsyncCandidateLifecycleStateAfterTimeoutOwnership",),
        ),
        (
            "AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint",
            (
                "AsyncTimeoutLifecyclePhysicalCut",
                "AsyncTimeoutLifecycleTransfersThisStep",
            ),
            ("AsyncCandidateLifecycleStateAfterTimeoutOwnership",),
        ),
        (
            "AsyncRetransmitFreshLiveEpisodeFreezesIngressPhysicalCut",
            (
                "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
                "AsyncRetransmitLifecycleConsumesFreshOrdinal",
                "retransmitLifecyclePhysicalCut",
                "AsyncNextIngressPhysicalOrdinal",
            ),
            (
                "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
                "AsyncRetransmitLifecyclePhysicalCutForStep",
            ),
        ),
        (
            "AsyncRetransmitLiveEpisodeRetainsIngressPhysicalCut",
            (
                "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
                "retransmitLifecycleOrdinal",
                "retransmitLifecyclePhysicalCut",
            ),
            ("AsyncCandidateLifecycleStateAfterServeIngressAdmission",),
        ),
        (
            "AsyncRetransmitLifecycleFreezeBoundaryMintsAfterPriorAdmissions",
            (
                "AsyncRetransmitLifecyclePhysicalCut",
                "AsyncNextIngressPhysicalOrdinal",
                "AsyncRetransmitLifecycleOrdinal",
                "AsyncNextCandidateLifecycleOrdinal",
            ),
            (
                "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
                "AsyncRetransmitLifecyclePhysicalCutForStep",
            ),
        ),
        (
            "AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint",
            (
                "AsyncRetransmitLifecycleOrdinal",
                "AsyncRetransmitLifecyclePhysicalCut",
            ),
            ("AsyncCandidateLifecycleStateAfterServeIngressAdmission",),
        ),
        (
            "AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint",
            (
                "AsyncRetransmitLifecycleOrdinal",
                "AsyncRetransmitLifecyclePhysicalCut",
                "AsyncRetransmitLifecycleResetThisStep",
                "AsyncRetransmitLifecycleEpisodeCompletesThisStep",
            ),
            ("AsyncCandidateLifecycleStateAfterServeIngressAdmission",),
        ),
    ):
        extracted = _top_level_theorem_body(
            network_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{network_path}: missing producer-continuation physical-cut "
                f"theorem {symbol}"
            )
            continue
        body, line = extracted
        statement, *proof_parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
            maxsplit=1,
        )
        proof = "" if not proof_parts else proof_parts[0]
        statement_tokens = set(tla_code_tokens(statement))
        missing_statement = tuple(
            token
            for token in required_statement_tokens
            if token not in statement_tokens
        )
        missing_proof = tuple(
            token
            for token in required_proof_tokens
            if not _tla_dependency_present(proof, token)
        )
        if missing_statement or missing_proof:
            errors.append(
                f"{network_path}:{line}: {symbol} must prove stored-cut "
                "preservation and distinguish post-cut ingress from true "
                f"pre-cut ingress; missing_statement={missing_statement!r}, "
                f"missing_proof={missing_proof!r}"
            )
    exact_proof_operators = {
        "AsyncCandidateProducerContinuationExactOwner": (
            '\\E record \\in AsyncCandidateProducerContinuations: /\\ '
            'record.status \\in {"Reserved", "Materialized"} /\\ '
            'record.node \\in {target, leader} /\\ record.context = '
            'leaderContext /\\ record.height = leaderContext.height /\\ '
            'record.view = leaderView /\\ record.identity.leader = leader '
            '/\\ record.subject = subject /\\ record.phase \\in '
            'AsyncCandidateServiceTrackedKinds /\\ (record.phase \\in '
            '{"BeginDecision", "PersistDecision"} => record.node = target) '
            '/\\ record.identity = AsyncCandidateServiceIdentity(record.candidate) /\\ '
            'record.causalOrigin = record.candidate.causalOrigin'
        ),
        "AsyncCandidateProducerContinuationFrozenRecords": (
            "{record \\in "
            "AsyncCandidateProducerContinuationResolutionRecordsForNode(node): "
            "/\\ record.ordinal <= targetOrdinal /\\ record.causalOrigin "
            "\\in "
            "AsyncCandidateProducerContinuationFrozenPredecessorOrigins( "
            "node, targetOrdinal)}"
        ),
        "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates": (
            "{candidate \\in AsyncCandidateSet: \\E record \\in "
            "AsyncCandidateProducerContinuationFrozenRecords( node, "
            'targetOrdinal): /\\ record.status = "Reserved" /\\ '
            'record.sourceClass = "Local" /\\ '
            "~AsyncCandidateProducerContinuationConcreteSuccessorOwned(record) "
            "/\\ ~AsyncCandidateProducerContinuationHandoffRetired(record) "
            "/\\ candidate = record.candidate}"
        ),
        "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates": (
            "{AsyncLeaderWireRuntimeCandidate(record.item): record \\in "
            "{owned \\in asyncLeaderWireLifecycles: /\\ owned.recipient = node "
            "/\\ owned.schedulerOrdinal < targetOrdinal /\\ "
            "AsyncLeaderWireLifecycleActive(owned) /\\ "
            "owned.physicalAdmissionOrdinal < physicalCut}}"
        ),
        "AsyncCandidateProducerContinuationFrozenCandidateOwners": (
            "AsyncCandidateProducerContinuationFrozenCausalCandidates( "
            "node, targetOrdinal) \\cup "
            "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates( "
            "node, targetOrdinal) \\cup "
            "AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates( "
            "node, targetOrdinal) \\cup "
            "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates( "
            "node, targetOrdinal, "
            "AsyncCandidateProducerContinuationTargetPhysicalCut( node, "
            "targetOrdinal))"
        ),
        "AsyncCandidateProducerContinuationFrozenCandidateTokens": (
            'UNION {{<<"Candidate", candidate, token>>: token \\in '
            "1..AsyncCandidateProducerContinuationCausalWeight(candidate.kind)}: "
            "candidate \\in AsyncCandidateProducerContinuationFrozenCandidateOwners( "
            "node, targetOrdinal)}"
        ),
        "AsyncCandidateProducerContinuationFrozenStatusTokens": (
            'UNION {{<<"Continuation", record.identity, token>>: token \\in '
            "1..AsyncCandidateProducerContinuationStatusRank(record.status)}: "
            "record \\in AsyncCandidateProducerContinuationFrozenRecords( node, "
            "targetOrdinal)}"
        ),
        "AsyncCandidateProducerContinuationFrozenPrefixDescentProperty": (
            "specification => \\A node \\in AsyncVotersAt(initialContext), "
            "identity \\in AsyncCandidateServiceIdentities, targetOrdinal \\in Nat "
            "\\ {0}, targetStage \\in "
            "AsyncCandidateServiceStageClasses, status \\in "
            '{"Reserved", "Materialized"}, budget \\in '
            "AsyncCandidateProducerContinuationFrozenPrefixRankCarrier: "
            "AsyncCandidateProducerContinuationFrozenPrefixAtBudget( node, "
            "identity, targetOrdinal, targetStage, status, budget) ~> "
            "AsyncCandidateProducerContinuationPrefixDescentGoal( node, "
            "identity, targetOrdinal, targetStage, status, budget)"
        ),
        "AsyncCandidateProducerContinuationFrozenPrefixClosureProperty": (
            "specification => \\A node \\in AsyncVotersAt(initialContext), "
            "identity \\in AsyncCandidateServiceIdentities, targetOrdinal \\in Nat "
            "\\ {0}, targetStage \\in "
            "AsyncCandidateServiceStageClasses, status \\in "
            '{"Reserved", "Materialized"}, budget \\in '
            "AsyncCandidateProducerContinuationFrozenPrefixRankCarrier: "
            "AsyncCandidateProducerContinuationFrozenPrefixAtBudget( node, "
            "identity, targetOrdinal, targetStage, status, budget) ~> "
            "AsyncCandidateProducerContinuationTargetStatusExit( identity, "
            "status)"
        ),
        "AsyncCandidateProducerContinuationDormantReservationClosureProperty": (
            "specification => \\A node \\in AsyncVotersAt(initialContext), "
            "record \\in AsyncCandidateProducerContinuationRecordSet: /\\ "
            "gst /\\ record = "
            "AsyncCandidateProducerContinuationSelectedResolutionRecord(node) "
            '/\\ record.status = "Reserved" /\\ record \\in '
            "AsyncCandidateProducerContinuationResolutionRecordsForNode(node) "
            "~> AsyncCandidateProducerContinuationDormantReservationGoal(record)"
        ),
    }
    exact_network_operators = {
        "AsyncCandidateProducerContinuationCapacity": (
            "AsyncCandidateServiceRecordCapacity"
        ),
        "AsyncCandidateProducerContinuationStatuses": (
            '{"Reserved", "Materialized", "Terminal"}'
        ),
        "AsyncCandidateProducerContinuationSourceClasses": (
            '{"Local", "ConditionalTransport", "VolatileBody"}'
        ),
        "AsyncCandidateProducerContinuationConditionalResponsiveTransportKinds": (
            '{"DeliverProposal", "DeliverVote", "DeliverQC", '
            '"DeliverTimeout", "DeliverTC"}'
        ),
        "AsyncCandidateProducerContinuationVolatileBodyReconstructionKinds": (
            '{"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"}'
        ),
        "AsyncCandidateProducerContinuationLocallyReconstructibleKinds": (
            '{"AssembleBody", "TimeoutElapsed", "StoreBody", '
            '"ValidateBody", "Apply"}'
        ),
        "AsyncCandidateProducerContinuationExternalResidualKinds": (
            "AsyncCandidateProducerContinuationConditionalResponsiveTransportKinds "
            "\\cup "
            "AsyncCandidateProducerContinuationVolatileBodyReconstructionKinds"
        ),
        "AsyncCandidateProducerContinuationSourceClass": (
            "CASE candidate.kind \\in "
            "AsyncCandidateProducerContinuationLocallyReconstructibleKinds -> "
            '"Local" [] candidate.kind \\in '
            "AsyncCandidateProducerContinuationConditionalResponsiveTransportKinds "
            '-> "ConditionalTransport" [] OTHER -> "VolatileBody"'
        ),
        "AsyncCandidateProducerContinuationItemCarrierIn": (
            "\\/ item \\in retainedControl \\/ \\E packet \\in transport: "
            "packet.item = item \\/ \\E recipient \\in ValidatorIds, source "
            "\\in AsyncIngressSources: item \\in "
            "SequenceSet(ingressLanes[recipient][source])"
        ),
        "AsyncCandidateProducerContinuationDeclaredHandoffOwned": (
            "\\E successor \\in record.handoffCandidates: \\/ "
            "CandidateScheduled(successor) \\/ "
            "AsyncCandidateServiceCoalesced(successor) \\/ "
            "AsyncCandidateProducerContinuationActiveForIdentity( "
            "AsyncCandidateServiceIdentity(successor)) \\/ "
            "successor.causalOrigin \\in "
            "AsyncCandidateLifecycleDurableReplayOriginsForNode( "
            "successor.node)"
        ),
        "AsyncCandidateProducerContinuationDeclaredHandoffRetired": (
            "\\/ record.handoffCandidates = {} \\/ \\A successor \\in "
            "record.handoffCandidates: \\/ "
            "AsyncCandidateInternalBodyAvailableStageRetired(successor) \\/ "
            "AsyncCandidateProducerContinuationTerminalForIdentity( "
            "AsyncCandidateServiceIdentity(successor))"
        ),
        "AsyncCandidateConditionalTransportCarrier": (
            '/\\ record.sourceClass = "ConditionalTransport" /\\ '
            "record.candidate.item # NoAsyncItem /\\ "
            "AsyncCandidateProducerContinuationItemCarrier( "
            "record.candidate.item)"
        ),
        "AsyncCandidateConditionalTransportRetired": (
            '/\\ record.sourceClass = "ConditionalTransport" /\\ \\/ '
            "AsyncCandidateServiceCoalesced(record.candidate) \\/ "
            "AsyncControlServiceIdentityServicedOrAdvancedIn( "
            "asyncControlServiceState, record.candidate.item) \\/ /\\ "
            "record.handoffCandidates = {} /\\ "
            "~AsyncCandidateConditionalTransportCarrier(record) /\\ "
            "AsyncControlServiceSlotOwnedIn( asyncControlServiceState, "
            "record.candidate.item) /\\ "
            "AsyncControlServiceIdentityMatches( record.candidate.item, "
            "AsyncControlServiceRecordForItemIn( asyncControlServiceState, "
            "record.candidate.item))"
        ),
        "AsyncCandidateVolatileBodyExactRequestCarrierIn": (
            '\\E request \\in activeRequests: /\\ request.kind = '
            '"CertifiedRequest" /\\ request.source = candidate.node /\\ '
            "request.envelope.height = candidate.height /\\ "
            "request.envelope.view = candidate.view /\\ "
            "request.envelope.subject = candidate.subject /\\ "
            "candidate.evidence \\in QcRecordSet /\\ "
            "request.envelope.certificate = candidate.evidence"
        ),
        "AsyncCandidateVolatileBodyCarrier": (
            '/\\ record.sourceClass = "VolatileBody" /\\ \\/ '
            "AsyncCandidateProducerContinuationDeclaredHandoffOwned(record) "
            "\\/ AsyncCandidateInternalBodyAvailableStageRetired("
            "record.candidate) \\/ "
            "AsyncCandidateVolatileBodyExactRequestCarrierIn( "
            "asyncActiveRequests, record.candidate) \\/ /\\ "
            "record.candidate.evidence \\in AsyncNetworkItems /\\ "
            "AsyncCandidateProducerContinuationItemCarrier( "
            "record.candidate.evidence)"
        ),
        "AsyncCandidateVolatileBodyRetired": (
            '/\\ record.sourceClass = "VolatileBody" /\\ \\/ '
            "AsyncCandidateServiceCoalesced(record.candidate) \\/ /\\ "
            "record.handoffCandidates = {} /\\ "
            "AsyncCandidateInternalBodyAvailableStageRetired( "
            "record.candidate) \\/ /\\ record.handoffCandidates # {} /\\ "
            "AsyncCandidateProducerContinuationDeclaredHandoffRetired(record)"
        ),
        "AsyncCandidateConditionalTransportCarrierAfter": (
            '/\\ record.sourceClass = "ConditionalTransport" /\\ '
            "record.candidate.item # NoAsyncItem /\\ "
            "AsyncCandidateProducerContinuationItemCarrierIn( "
            "asyncRetainedControl', asyncTransport', asyncIngressLanes', "
            "record.candidate.item)"
        ),
        "AsyncCandidateProducerContinuationDeclaredHandoffOwnedAfterIn": (
            "\\E successor \\in record.handoffCandidates: \\/ "
            "CandidateScheduledAfter(successor) \\/ \\E serviced \\in "
            "state.candidateServiceMarkers \\cup "
            "state.candidateTerminalTombstones: serviced.identity = "
            "AsyncCandidateServiceIdentity(successor) \\/ "
            "AsyncCandidateProducerContinuationActiveForIdentityIn( state, "
            "AsyncCandidateServiceIdentity(successor)) \\/ "
            "successor.causalOrigin \\in "
            "AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter( "
            "successor.node)"
        ),
        "AsyncCandidateProducerContinuationDeclaredHandoffRetiredAfterIn": (
            "\\/ record.handoffCandidates = {} \\/ \\A successor \\in "
            "record.handoffCandidates: \\/ "
            "AsyncCandidateInternalBodyAvailableStageRetiredAfter(successor) "
            "\\/ AsyncCandidateProducerContinuationTerminalForIdentityIn( "
            "state, AsyncCandidateServiceIdentity(successor))"
        ),
        "AsyncCandidateConditionalTransportRetiredAfterIn": (
            '/\\ record.sourceClass = "ConditionalTransport" /\\ \\/ '
            "AsyncCandidateProducerContinuationServiceCoalescedIn( state, "
            "record.candidate) \\/ "
            "AsyncControlServiceIdentityServicedOrAdvancedIn( state, "
            "record.candidate.item) \\/ /\\ record.handoffCandidates = {} "
            "/\\ ~AsyncCandidateConditionalTransportCarrierAfter(record) "
            "/\\ AsyncControlServiceSlotOwnedIn( state, "
            "record.candidate.item) /\\ "
            "AsyncControlServiceIdentityMatches( record.candidate.item, "
            "AsyncControlServiceRecordForItemIn( state, "
            "record.candidate.item))"
        ),
        "AsyncCandidateVolatileBodyCarrierAfterIn": (
            '/\\ record.sourceClass = "VolatileBody" /\\ \\/ '
            "AsyncCandidateProducerContinuationDeclaredHandoffOwnedAfterIn( "
            "state, record) \\/ "
            "AsyncCandidateInternalBodyAvailableStageRetiredAfter( "
            "record.candidate) \\/ "
            "AsyncCandidateVolatileBodyExactRequestCarrierIn( "
            "asyncActiveRequests', record.candidate) \\/ /\\ "
            "record.candidate.evidence \\in AsyncNetworkItems /\\ "
            "AsyncCandidateProducerContinuationItemCarrierIn( "
            "asyncRetainedControl', asyncTransport', asyncIngressLanes', "
            "record.candidate.evidence)"
        ),
        "AsyncCandidateVolatileBodyRetiredAfterIn": (
            '/\\ record.sourceClass = "VolatileBody" /\\ \\/ '
            "AsyncCandidateProducerContinuationServiceCoalescedIn( state, "
            "record.candidate) \\/ /\\ record.handoffCandidates = {} /\\ "
            "AsyncCandidateInternalBodyAvailableStageRetiredAfter( "
            "record.candidate) \\/ /\\ record.handoffCandidates # {} /\\ "
            "AsyncCandidateProducerContinuationDeclaredHandoffRetiredAfterIn( "
            "state, record)"
        ),
        "AsyncCandidateProducerContinuationStatusRank": (
            'CASE status = "Reserved" -> 2 [] status = "Materialized" -> 1 '
            "[] OTHER -> 0"
        ),
        "AsyncCandidateProducerContinuationRecord": (
            "[identity |-> AsyncCandidateServiceIdentity(candidate), "
            "candidate |-> candidate, handoffCandidates |-> "
            "handoffCandidates, address |-> [node |-> candidate.node, "
            "slot |-> lifecycleSlot, stage |-> "
            "AsyncCandidateServiceStageForKind(candidate.kind)], node |-> "
            "candidate.node, context |-> candidate.consumerContext, height "
            "|-> candidate.height, view |-> candidate.view, subject |-> "
            "candidate.subject, phase |-> candidate.kind, sourceClass |-> "
            "AsyncCandidateProducerContinuationSourceClass(candidate), "
            "causalOrigin |-> candidate.causalOrigin, ordinal |-> ordinal, "
            "sourcePhysicalOrdinal |-> sourcePhysicalOrdinal, physicalCut "
            "|-> physicalCut, status |-> status]"
        ),
        "AsyncCandidateProducerContinuationActiveForIdentityIn": (
            "\\E record \\in "
            "AsyncCandidateProducerContinuationRecordsForIdentityIn( state, "
            'identity): record.status \\in {"Reserved", "Materialized"}'
        ),
        "AsyncCandidateProducerContinuationTerminalForIdentityIn": (
            "\\E record \\in "
            "AsyncCandidateProducerContinuationRecordsForIdentityIn( state, "
            'identity): record.status = "Terminal"'
        ),
        "AsyncCandidateProducerContinuationPartitionInvariantIn": (
            "/\\ \\A left, right \\in state.producerContinuations: /\\ "
            "(left.identity = right.identity => left = right) /\\ "
            "(left.address = right.address => left = right) /\\ "
            "((left.node = right.node /\\ left.ordinal = right.ordinal /\\ "
            "left.phase = right.phase) => left = right) /\\ ((left.node = "
            "right.node /\\ left.ordinal = right.ordinal /\\ left.status "
            '\\in {"Reserved", "Materialized"} /\\ right.status \\in '
            '{"Reserved", "Materialized"}) => left = right) /\\ \\A record '
            "\\in state.producerContinuations: /\\ record.address \\in "
            "AsyncCandidateServiceStageOwnerAddresses /\\ "
            "record.address.node = record.node /\\ record.address.stage = "
            "AsyncCandidateServiceStageForKind(record.phase) /\\ \\A "
            "successor \\in record.handoffCandidates: /\\ successor.node = "
            "record.node /\\ successor.causalOrigin = record.causalOrigin"
        ),
        "AsyncCandidateProducerContinuationScheduledExclusionInvariant": (
            "\\A candidate \\in QueuedCandidates \\cup DeferredCandidates "
            "\\cup CausalCandidates \\cup TrackedWorkCandidates: "
            "\\/ ~AsyncCandidateProducerContinuationBlocks(candidate) \\/ "
            "AsyncCandidateProducerContinuationExactReplayCarrier(candidate)"
        ),
        "AsyncCandidateProducerContinuationExactReplayCarrier": (
            "\\E record \\in AsyncCandidateProducerContinuations: /\\ "
            'record.status \\in {"Reserved", "Materialized"} /\\ '
            'record.sourceClass = "Local" /\\ record.candidate = candidate '
            "/\\ record.ordinal = AsyncCandidateLifecycleOrdinal(candidate)"
        ),
        "AsyncCandidateProducerContinuationLocalReplayCarrier": (
            '/\\ record.sourceClass = "Local" /\\ '
            "CandidateScheduled(record.candidate)"
        ),
        "AsyncCandidateProducerContinuationLocalReplayCarrierAfter": (
            '/\\ record.sourceClass = "Local" /\\ '
            "CandidateScheduledAfter(record.candidate)"
        ),
        "AsyncCandidateProducerContinuationExactLocalReplayStep": (
            "LET record == "
            "AsyncCandidateProducerContinuationSelectedReplayRecord(node) IN "
            '/\\ record.status = "Reserved" /\\ '
            'record.sourceClass = "Local" /\\ '
            "~AsyncCandidateProducerContinuationRunnerResolutionReady(node) /\\ "
            'asyncRunnerPhase[node] = "Local" /\\ '
            "asyncRunnerBudget[node] > 0 /\\ "
            "CanEnqueueClass(node, record.candidate.class) /\\ "
            "AsyncCandidateProducerContinuationExactReplayIdentity( node, "
            "AsyncCandidateProducerContinuationSelectedLocalCandidate(node)) "
            "/\\ AsyncCandidateLifecycleOrdinal(record.candidate) = "
            "record.ordinal /\\ "
            "AsyncCandidateLifecycleSourcePhysicalOrdinal(record.candidate) "
            "= record.sourcePhysicalOrdinal /\\ "
            "AsyncCandidateLifecyclePhysicalCut(record.candidate) = "
            "record.physicalCut /\\ EnqueueCandidate(record.candidate) /\\ "
            "UNCHANGED vars /\\ UNCHANGED asyncCausalQueues /\\ UNCHANGED "
            "AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService /\\ "
            "asyncRunnerPhase' = asyncRunnerPhase /\\ asyncRunnerBudget' = "
            "[asyncRunnerBudget EXCEPT ![node] = @ - 1]"
        ),
        "AsyncCandidateProducerContinuationHandoffCandidatesThisStep": (
            "IF candidate \\in AsyncCandidateServicesThisStep THEN "
            "SequenceSet(CommandSuccessors(candidate)) ELSE {}"
        ),
        "AsyncCandidateProducerContinuationInitialStatusAfter": '"Reserved"',
        "AsyncCandidateProducerContinuationSourceAfter": (
            "/\\ AsyncCandidateProducerContinuationDeparture(candidate) /\\ "
            "~AsyncCandidateProducerContinuationGoalAfter(candidate) /\\ "
            "candidate.kind \\in AsyncCandidateServiceTrackedKinds"
        ),
        "AsyncCandidateProducerContinuationAddressCanAdvanceIn": (
            "LET address == "
            "AsyncCandidateProducerContinuationAddressForIn(state, "
            "candidate) ordinal == "
            "AsyncCandidateProducerContinuationOrdinalForIn(state, "
            "candidate) IN \\A record \\in "
            "AsyncCandidateProducerContinuationRecordsForAddressIn( state, "
            'address): /\\ record.status = "Terminal" /\\ record.context = '
            "candidate.consumerContext /\\ record.height = candidate.height "
            "/\\ record.view < candidate.view /\\ record.ordinal < ordinal"
        ),
        "AsyncCandidateProducerContinuationHandoffOwned": (
            '\\/ /\\ record.sourceClass = "Local" /\\ '
            "\\/ "
            "AsyncCandidateProducerContinuationDeclaredHandoffOwned(record) "
            "\\/ "
            "AsyncCandidateProducerContinuationLocalReplayCarrier(record) "
            '\\/ /\\ record.sourceClass = "ConditionalTransport" /\\ \\/ '
            "AsyncCandidateProducerContinuationDeclaredHandoffOwned(record) "
            "\\/ AsyncCandidateConditionalTransportCarrier(record) \\/ "
            "AsyncCandidateVolatileBodyCarrier(record)"
        ),
        "AsyncCandidateProducerContinuationHandoffRetired": (
            '\\/ /\\ record.sourceClass = "Local" /\\ '
            "AsyncCandidateProducerContinuationDeclaredHandoffRetired(record) "
            "\\/ AsyncCandidateConditionalTransportRetired(record) \\/ "
            "AsyncCandidateVolatileBodyRetired(record)"
        ),
        "AsyncCandidateProducerContinuationConcreteSuccessorOwned": (
            "AsyncCandidateProducerContinuationHandoffOwned(record)"
        ),
        "AsyncCandidateProducerContinuationResolutionPredecessorsFor": (
            "{other \\in "
            "AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode( "
            "node): AsyncCandidateProducerContinuationLogicalPrecedes(other, "
            "record)}"
        ),
        "AsyncCandidateProducerContinuationSelectedResolutionRecord": (
            "CHOOSE record \\in "
            "AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode( "
            "node): "
            "AsyncCandidateProducerContinuationResolutionPredecessorsFor( "
            "node, record) = {}"
        ),
        "AsyncCandidateProducerContinuationResolutionRequired": (
            "AsyncCandidateProducerContinuationResolutionRecordsForNode(node) "
            "# {}"
        ),
        "AsyncCandidateProducerContinuationSelectedSourceClass": (
            "/\\ AsyncCandidateProducerContinuationResolutionRequired(node) "
            "/\\ sourceClass \\in "
            "AsyncCandidateProducerContinuationSourceClasses /\\ "
            "(AsyncCandidateProducerContinuationSelectedResolutionRecord(node)) "
            ".sourceClass = sourceClass"
        ),
        "AsyncCandidateProducerContinuationResolutionReady": (
            "LET record == "
            "AsyncCandidateProducerContinuationSelectedResolutionRecord(node) "
            "IN /\\ "
            "AsyncCandidateProducerContinuationResolutionRequired(node) /\\ "
            '\\/ record.status = "Materialized" \\/ '
            "AsyncCandidateProducerContinuationConcreteSuccessorOwned(record) "
            "\\/ "
            "AsyncCandidateProducerContinuationHandoffRetired(record)"
        ),
        "AsyncCandidateProducerContinuationRunnerMayPrecedeIngress": (
            "\\/ ~AsyncIngressSchedulerBarrierActive(node) "
            "\\/ record.physicalCut <= "
            "AsyncEarliestIngressPhysicalOrdinal(node) \\/ /\\ "
            "AsyncEarliestIngressPhysicalOrdinal(node) < record.physicalCut "
            "/\\ record.ordinal <= AsyncEarliestIngressSchedulerOrdinal(node)"
        ),
        "AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode": (
            "{record \\in "
            "AsyncCandidateProducerContinuationRuntimePhysicallyEligibleRecordsForNode( "
            "node): "
            "AsyncCandidateProducerContinuationRunnerMayPrecedeIngress("
            "node, record)}"
        ),
        "AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord": (
            "AsyncCandidateProducerContinuationRuntimeSelectedResolutionRecord("
            "node)"
        ),
        "AsyncCandidateProducerContinuationRunnerResolutionRequired": (
            "/\\ AsyncCandidateProducerContinuationIngressResolutionRequired("
            "node) "
            "/\\ AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord("
            "node) \\in "
            "AsyncCandidateProducerContinuationRunnableResolutionRecordsForNode( "
            "node)"
        ),
        "AsyncCandidateProducerContinuationRunnerResolutionReady": (
            "/\\ "
            "AsyncCandidateProducerContinuationRunnerResolutionRequired(node) "
            "/\\ AsyncCandidateProducerContinuationRuntimeResolutionReady("
            "node)"
        ),
        "ResolveCandidateProducerContinuation": (
            "/\\ node \\in AsyncCurrentResponsiveVoters /\\ "
            "AsyncCandidateProducerContinuationResolutionReady(node) /\\ "
            "UNCHANGED vars /\\ UNCHANGED asyncCausalQueues /\\ UNCHANGED "
            "AsyncSchedulerExceptCausalAndControlService"
        ),
        "ResolveLocalCandidateProducerContinuation": (
            "/\\ AsyncCandidateProducerContinuationSelectedSourceClass(node, "
            '"Local") /\\ ResolveCandidateProducerContinuation(node)'
        ),
        "ServiceConditionalTransportProducerContinuation": (
            "/\\ AsyncCandidateProducerContinuationSelectedSourceClass( node, "
            '"ConditionalTransport") /\\ '
            "ResolveCandidateProducerContinuation(node)"
        ),
        "ServiceVolatileBodyProducerContinuation": (
            "/\\ AsyncCandidateProducerContinuationSelectedSourceClass( node, "
            '"VolatileBody") /\\ ResolveCandidateProducerContinuation(node)'
        ),
        "PostGstResolveLocalCandidateProducerContinuation": (
            "/\\ gst /\\ ResolveLocalCandidateProducerContinuation(node) /\\ "
            "AsyncNonRunnerOuterFrame"
        ),
        "PostGstServiceConditionalTransportProducerContinuation": (
            "/\\ gst /\\ "
            "ServiceConditionalTransportProducerContinuation(node) /\\ "
            "AsyncNonRunnerOuterFrame"
        ),
        "PostGstServiceVolatileBodyProducerContinuation": (
            "/\\ gst /\\ ServiceVolatileBodyProducerContinuation(node) /\\ "
            "AsyncNonRunnerOuterFrame"
        ),
        "AsyncCandidateProducerContinuationSelectedForResolution": (
            "/\\ ResolveCandidateProducerContinuation(record.node) /\\ "
            "AsyncCandidateProducerContinuationSelectedResolutionRecord( "
            "record.node) = record"
        ),
        "AsyncCandidateProducerContinuationSelectedForRunnerResolution": (
            "/\\ ResolveRunNodeCandidateProducerContinuation(record.node) "
            "/\\ AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord( "
            "record.node) = record"
        ),
        "AsyncCandidateProducerContinuationSelectedForRunnerReplay": (
            "/\\ ReplayRunNodeCandidateProducerContinuation(record.node) "
            "/\\ AsyncCandidateProducerContinuationExactRuntimeReplayStep( "
            "record.node) /\\ "
            "AsyncCandidateProducerContinuationSelectedReplayRecord( "
            "record.node) = record"
        ),
        "AsyncCandidateProducerContinuationSelectedForAcknowledgement": (
            "\\/ AsyncCandidateProducerContinuationSelectedForResolution(record) "
            "\\/ "
            "AsyncCandidateProducerContinuationSelectedForRunnerResolution(record) "
            "\\/ "
            "AsyncCandidateProducerContinuationSelectedForRunnerReplay(record)"
        ),
        "AsyncCandidateProducerContinuationHandoffOwnedAfterIn": (
            '\\/ /\\ record.sourceClass = "Local" /\\ '
            "\\/ "
            "AsyncCandidateProducerContinuationDeclaredHandoffOwnedAfterIn( "
            "state, record) \\/ "
            "AsyncCandidateProducerContinuationLocalReplayCarrierAfter(record) "
            '\\/ /\\ record.sourceClass = '
            '"ConditionalTransport" /\\ \\/ '
            "AsyncCandidateProducerContinuationDeclaredHandoffOwnedAfterIn( "
            "state, record) \\/ "
            "AsyncCandidateConditionalTransportCarrierAfter(record) \\/ "
            "AsyncCandidateVolatileBodyCarrierAfterIn(state, record)"
        ),
        "AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn": (
            "AsyncCandidateProducerContinuationHandoffOwnedAfterIn(state, "
            "record)"
        ),
        "AsyncCandidateProducerContinuationHandoffRetiredAfterIn": (
            '\\/ /\\ record.sourceClass = "Local" /\\ '
            "AsyncCandidateProducerContinuationDeclaredHandoffRetiredAfterIn( "
            "state, record) \\/ "
            "AsyncCandidateConditionalTransportRetiredAfterIn(state, record) "
            "\\/ AsyncCandidateVolatileBodyRetiredAfterIn(state, record)"
        ),
        "AsyncCandidateProducerContinuationRecordAfterStep": (
            'IF record.status = "Terminal" THEN record ELSE IF \\/ '
            "AsyncCandidateProducerContinuationTerminalAfter(record) \\/ "
            "/\\ AsyncCandidateProducerContinuationSelectedForAcknowledgement( "
            'record) /\\ \\/ record.status = "Materialized" \\/ /\\ '
            'record.status = "Reserved" /\\ '
            "AsyncCandidateProducerContinuationHandoffRetiredAfterIn( state, "
            'record) THEN [record EXCEPT !.status = "Terminal"] ELSE IF /\\ '
            "AsyncCandidateProducerContinuationSelectedForAcknowledgement( "
            'record) /\\ record.status = "Reserved" /\\ '
            "AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn( "
            "state, "
            'record) THEN [record EXCEPT !.status = "Materialized"] ELSE record'
        ),
    }
    for operator_source, operator_path, exact_operators in (
        (source, path, exact_proof_operators),
        (network_source, network_path, exact_network_operators),
    ):
        for symbol, expected in exact_operators.items():
            extracted = _top_level_operator_body(
                operator_source,
                symbol,
                preserve_string_contents=True,
            )
            if extracted is None:
                errors.append(
                    f"{operator_path}: missing reviewed "
                    f"producer-continuation operator {symbol}"
                )
                continue
            body, line = extracted
            observed = " ".join(body.split())
            if observed != expected:
                errors.append(
                    f"{operator_path}:{line}: {symbol} must retain the exact "
                    "finite producer-continuation contract; "
                    f"expected {expected!r}; found {observed!r}"
                )
    proof_code = strip_tla_comments(source, preserve_string_contents=True)
    for symbol in (
        "AsyncCandidateProducerContinuationFrozenPrefixDescentProperty",
        "AsyncCandidateProducerContinuationFrozenPrefixClosureProperty",
        "AsyncCandidateProducerContinuationDormantReservationClosureProperty",
    ):
        declaration = re.search(
            rf"(?m)^[ \t]*{re.escape(symbol)}[ \t]*"
            r"\([ \t\r\n]*specification[ \t\r\n]*,"
            r"[ \t\r\n]*initialContext[ \t\r\n]*\)[ \t]*==",
            proof_code,
        )
        if declaration is None:
            errors.append(
                f"{path}: {symbol} must retain exactly the reviewed "
                "two-argument (specification, initialContext) contract"
            )
    exact_proof_theorem_statements = {
        "CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight": (
            "\\A command \\in AsyncCandidateSet: "
            "AsyncCandidateProducerContinuationSuccessorBatchWeight(command) "
            '+ AsyncCandidateProducerContinuationStatusRank("Reserved") < '
            "AsyncCandidateProducerContinuationCausalWeight(command.kind)"
        ),
        "CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound": (
            "\\A node \\in ValidatorIds, targetOrdinal \\in Nat: /\\ "
            "AsyncStrongTypeInvariant /\\ "
            "AsyncCandidateServiceLifecycleInvariant => /\\ IsFiniteSet( "
            "AsyncCandidateProducerContinuationFrozenCandidateOwners( node, "
            "targetOrdinal)) /\\ Cardinality( "
            "AsyncCandidateProducerContinuationFrozenCandidateOwners( node, "
            "targetOrdinal)) <= AsyncCandidateProducerEpisodeCapacity + "
            "AsyncCandidateProducerContinuationCapacity + "
            "AsyncOrdinaryIngressCarrierEvidenceCapacity"
        ),
        "CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge": (
            "\\A node \\in ValidatorIds, targetOrdinal \\in Nat: /\\ gst "
            "/\\ AsyncStrongTypeInvariant /\\ "
            "AsyncProgressOwnershipInvariant /\\ "
            "AsyncCandidateServiceLifecycleInvariant /\\ "
            "(AsyncCandidateProducerContinuationTargetPhysicalCut( node, "
            "targetOrdinal))' = "
            "AsyncCandidateProducerContinuationTargetPhysicalCut( node, "
            "targetOrdinal) /\\ AsyncNext => "
            "((AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates( "
            "node, targetOrdinal))' \\ "
            "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates( "
            "node, targetOrdinal)) \\subseteq "
            "AsyncCandidateProducerContinuationFrozenCausalCandidates( node, "
            "targetOrdinal)"
        ),
        "CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge": (
            "\\A node \\in ValidatorIds, targetOrdinal \\in Nat: /\\ "
            "AsyncStrongTypeInvariant /\\ AsyncProgressOwnershipInvariant "
            "/\\ AsyncCandidateServiceLifecycleInvariant /\\ AsyncNext /\\ "
            "AsyncControlServiceSlotTransition /\\ "
            "AsyncCandidateProducerContinuationExactLocalReplayStep(node) /\\ "
            "(AsyncCandidateProducerContinuationTargetPhysicalCut( node, "
            "targetOrdinal))' = "
            "AsyncCandidateProducerContinuationTargetPhysicalCut( node, "
            "targetOrdinal) => "
            "(AsyncCandidateProducerContinuationFrozenCandidateOwners( node, "
            "targetOrdinal))' = "
            "AsyncCandidateProducerContinuationFrozenCandidateOwners( node, "
            "targetOrdinal)"
        ),
        "CandidateProducerContinuationFrozenPrefixStepCannotReplenish": (
            "\\A node \\in ValidatorIds, identity \\in AsyncCandidateServiceIdentities, "
            "targetOrdinal \\in Nat "
            "\\ {0}, targetStage \\in "
            "AsyncCandidateServiceStageClasses, status \\in "
            '{"Reserved", "Materialized"}, budget \\in '
            "AsyncCandidateProducerContinuationFrozenPrefixRankCarrier: /\\ "
            "AsyncStrongTypeInvariant /\\ AsyncProgressOwnershipInvariant "
            "/\\ AsyncCandidateServiceLifecycleInvariant /\\ "
            "AsyncCandidateProducerContinuationFrozenPrefixAtBudget( node, "
            "identity, targetOrdinal, targetStage, status, budget) /\\ "
            "[AsyncNext]_AsyncAllVars => \\/ "
            "AsyncCandidateProducerContinuationFrozenPrefixAtBudget( node, "
            "identity, targetOrdinal, targetStage, status, budget)' \\/ "
            "(AsyncCandidateProducerContinuationPrefixDescentGoal( node, "
            "identity, targetOrdinal, targetStage, status, budget))'"
        ),
    }
    for symbol, expected in exact_proof_theorem_statements.items():
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{path}: missing reviewed producer-continuation theorem "
                f"{symbol}"
            )
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
                f"{path}:{line}: {symbol} must retain the exact reviewed "
                "producer-continuation rank statement; "
                f"expected {expected!r}; found {observed!r}"
            )
    exact_leader_wire_producer_statement_sha256 = {
        "AsyncFrozenServeSourceCannotResurrectAtGst": (
            "9c7153b36e085242cbb711e2f4ae749aead7d6e93c55bef96ab28bb2b3ada029"
        ),
        "CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier": (
            "8c51cc12d6520bfb08211907a9576cc5ef9d0713bb8fafa5098bbd61b08b05d9"
        ),
        "CandidateProducerContinuationActionInertDormantHasZeroFrozenStage": (
            "1b256ad8b6e371ddf207e72d8aaaa75f89f867cadb36b2704c49e03e914e9f71"
        ),
        "CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix": (
            "c6d4e93d4b7f3a7215d2ff2a4f246286cd6a3a26187eb7f182db98cfdb6b06c8"
        ),
        "CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame": (
            "82204af3840d76e420e671fad567b20995bd649e7c5c53e6a7d96a358c0e0fc1"
        ),
        "CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage": (
            "4b3f6929001c71087c7dd18356338478b08d5e8453ef10839cca9974412116cb"
        ),
        "AsyncFrozenLeaderWireIngressRankOrderingIsWellFounded": (
            "f42decd8d5a9b10dae1f22827c5c843f6dcefbdafdfba0f22fc769e80ddc8a4c"
        ),
        "AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded": (
            "70aa067368776f24476ddb5f2996fec56facda720290835d19b585c9b97c0843"
        ),
        "AsyncFrozenLeaderWireBarrierRankOrderingIsWellFounded": (
            "d715302c130ef21e1e420bedc8c1ba59ab688a132c9809a293aa5bedb6eb2411"
        ),
        "AsyncFrozenLeaderWireBarrierRankIsFinite": (
            "0ad5e1e602a4a49f5c4831be8871953ff94a03fd99a6b5e81f7bc888e41d3089"
        ),
        "AsyncCertifiedResponsePhysicalBarrierRankIsFinite": (
            "168d7a958824266232560350961095a2c622914a9ace3cd710a8d51b17b5ba36"
        ),
        "CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell": (
            "e1f1dd69579e3bbd611ab72d59e10646d447af05222e3aa5373958448079730f"
        ),
        "CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst": (
            "bd9dfed86b8e68fc564a7635a7a0415989debe3dde4d28058adda53f6601eb3b"
        ),
        "CandidateProducerContinuationFrozenSourcePrefixStepCannotReplenish": (
            "7402b0b99d66537dd7bcd34c4165cb90b04e871b7530f13b25e27a4117bb8f51"
        ),
        "CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends": (
            "93613b25fe6f3385483165e67268a3b54a679d8e8f6829c17cf7a970e8c7b3bd"
        ),
    }
    for symbol, expected_sha256 in (
        exact_leader_wire_producer_statement_sha256.items()
    ):
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{path}: missing reviewed leader-wire producer theorem {symbol}"
            )
            continue
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
            maxsplit=1,
        )[0]
        observed = " ".join(statement.split())
        observed_sha256 = hashlib.sha256(observed.encode("utf-8")).hexdigest()
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}:{line}: {symbol} must retain the exact leader-wire "
                "producer statement SHA-256 "
                f"{expected_sha256}; found {observed_sha256}"
            )

    source_class_split = _top_level_theorem_body(
        network_source,
        "CandidateProducerContinuationResolutionSplitsReviewedSourceClass",
        preserve_string_contents=True,
    )
    expected_source_class_split = (
        "\\A node \\in ValidatorIds: /\\ "
        "AsyncControlServiceStateTypeInvariant /\\ "
        "ResolveCandidateProducerContinuation(node) => \\/ "
        "ResolveLocalCandidateProducerContinuation(node) \\/ "
        "ServiceConditionalTransportProducerContinuation(node) \\/ "
        "ServiceVolatileBodyProducerContinuation(node)"
    )
    if source_class_split is None:
        errors.append(
            f"{network_path}: missing reviewed producer-continuation "
            "source-class split theorem"
        )
    else:
        body, line = source_class_split
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
            maxsplit=1,
        )[0]
        observed_statement = " ".join(statement.split())
        if observed_statement != expected_source_class_split:
            errors.append(
                f"{network_path}:{line}: "
                "CandidateProducerContinuationResolutionSplitsReviewedSourceClass "
                "must retain the exact Local/ConditionalTransport/VolatileBody "
                f"action partition; expected {expected_source_class_split!r}; "
                f"found {observed_statement!r}"
            )

    dormant_goal = _top_level_operator_body(
        source,
        "AsyncCandidateProducerContinuationDormantReservationGoal",
        preserve_string_contents=True,
    )
    if dormant_goal is None:
        errors.append(
            f"{path}: missing reviewed dormant producer-continuation goal"
        )
    else:
        body, line = dormant_goal
        if not _tla_dependency_present(
            body,
            "AsyncCandidateProducerContinuationHandoffRetired",
        ):
            errors.append(
                f"{path}:{line}: dormant producer-continuation goal must "
                "retain deterministic retired-handoff resolution"
            )

    dormant_property = _top_level_operator_body(
        source,
        "AsyncCandidateProducerContinuationDormantReservationClosureProperty",
        preserve_string_contents=True,
    )
    if dormant_property is None:
        errors.append(
            f"{path}: missing reviewed dormant producer-continuation closure"
        )
    else:
        body, line = dormant_property
        if not _tla_dependency_present(body, "gst"):
            errors.append(
                f"{path}:{line}: dormant producer-continuation closure must "
                "remain explicitly post-GST; pre-GST restart/replay belongs "
                "to the reset/replay kernel"
            )

    reviewed_theorems = (
        "AsyncCandidateProducerContinuationConstructorIsTyped",
        "AsyncCandidateProducerContinuationStatusRankIsNatural",
        "AsyncCandidateProducerContinuationHandoffRetainsExactLifecycle",
        "AsyncCandidateIgnoredDepartureDeclaresNoReplayHandoff",
        "AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal",
        "AsyncCandidateProducerContinuationDepartureSplitsSourceResidualOrGoal",
        "AsyncCandidateProducerContinuationLocalSourceExcludesTransportResidual",
        "AsyncCandidateProducerTransportResidualIsContinuationSource",
        "AsyncCandidateProducerTransportResidualSplitsPhysicalClass",
        "AsyncCandidateLifecycleDeparturesThisStepIsSingleton",
        "AsyncCandidateIgnoredExactProtocolDepartureIsContinuationSourceOrGoal",
        "AsyncCandidateSuccessfulExactProtocolServiceIsContinuationSourceOrGoal",
        "AsyncCandidateProducerContinuationStateInstallsExactSourceRecord",
        "AsyncCandidateProducerDepartureCreatesContinuationOrGoal",
        "AsyncCandidateProducerSourceTransitionInstallsExactContinuation",
        "AsyncCandidateProducerContinuationTerminalRecordIsFixed",
        "AsyncCandidateProducerContinuationStatusIsMonotone",
        "AsyncCandidateProducerContinuationResolvedReservedRankStrictlyDrops",
        "AsyncCandidateProducerContinuationMaterializedIsOneStep",
        "AsyncCandidateProducerContinuationUnselectedActiveRecordIsFixed",
        "ResolveCandidateProducerContinuationNeverReplaysDrainedParent",
        "CandidateProducerContinuationBlocksRunnerUntilHandoffResolution",
        "CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner",
        "ExternalCandidateProducerContinuationSelectionIsReady",
        "AsyncCandidateProducerContinuationGstExcludesResetReplay",
        "ConditionalTransportContinuationReadyEnablesFairService",
        "VolatileBodyContinuationReadyEnablesFairService",
        "LocalContinuationReadyEnablesFairResolution",
        "ExternalContinuationFairServiceStrictlyDropsStatusRank",
        "LocalContinuationFairResolutionStrictlyDropsStatusRank",
        "ExternalContinuationPersistsOrDescendsOrReplayExits",
        "LocalContinuationPersistsOrDescendsOrReplayExits",
        "CandidateProducerContinuationSuccessorBatchConsumesFrozenWeight",
        "CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight",
        "CandidateProducerContinuationPostCutCausalRootCannotEnterFrozenPrefix",
        "CandidateProducerContinuationCausalSuccessorRetainsFrozenPhysicalClass",
        "CandidateProducerContinuationPostCutServeCannotEnterFrozenPrefix",
        "CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded",
        "AsyncFrozenServeSourceCannotResurrectAtGst",
        "AsyncProtectedCandidateTargetPhysicalCutMatchesLifecycle",
        "AsyncProtectedCandidateTargetPhysicalCutPersists",
        "AsyncProtectedCandidateSelectedServeOwnerGeometryIsComplete",
        "AsyncProtectedCandidateSelectedOwnerIsConcreteAndEnabled",
        "CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier",
        "CandidateProducerContinuationActionInertDormantHasZeroFrozenStage",
        "CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix",
        "CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix",
        "CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame",
        "CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage",
        "CandidateProducerContinuationPreCutOrdinaryIngressConsumesBarrierStage",
        "AsyncFrozenLeaderWireIngressRankOrderingIsWellFounded",
        "AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded",
        "AsyncFrozenLeaderWireBarrierRankOrderingIsWellFounded",
        "AsyncFrozenLeaderWireBarrierRankIsFinite",
        "AsyncProtectedCandidateIngressEpisodeRankOrderingIsWellFounded",
        "AsyncProtectedCandidateIngressEpisodeRankIsFinite",
        "AsyncCertifiedResponsePhysicalBarrierRankIsFinite",
        "CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound",
        "AsyncCandidateProducerContinuationReclamationPreservesIdentity",
        "AsyncCandidateProducerContinuationPreservedOrTerminal",
        "CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge",
        "CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell",
        "CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst",
        "CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst",
        "CandidateProducerContinuationFrozenServeCutCannotReplenish",
        "CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge",
        "AsyncProtectedCandidateFrozenPrefixStepIsDescentOrFrame",
        "AsyncProtectedCandidateIngressEpisodeStepIsDescentOrFrame",
        "CandidateProducerContinuationTargetPhysicalCutIsStableUntilStatusExit",
        "HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay",
        "HistoricalCandidateProducerContinuationNonreadyTurnUsesLocalReplay",
        "HistoricalCandidateProducerContinuationLocalReplayTurnApproachesReady",
        "HistoricalCandidateProducerContinuationReadyTurnConsumesExactStage",
        "HistoricalCandidateProducerContinuationReadyTurnExitsSelectedStatus",
        "CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive",
        "CandidateProducerContinuationFrozenOriginsCannotReplenish",
        "CandidateProducerContinuationFrozenPrefixStepCannotReplenish",
        "CandidateProducerContinuationFrozenSourcePrefixStepCannotReplenish",
        "CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends",
        "CandidateProducerContinuationFairResolutionStrictlyDescendsFrozenPrefix",
        "CandidateProducerContinuationDormantGoalIsReadyOrExited",
        "AsyncCandidateProducerContinuationSameHeightRestartPreserved",
        "AsyncCandidateProducerContinuationResetPreservesActiveReservation",
        "AsyncCandidateProducerContinuationResetReopensOnlyUnstableTerminal",
        "AsyncCandidateProducerContinuationResetCannotResurrectDifferentOwner",
        "AsyncCandidateProducerContinuationReplacementRetiresOnlyTerminal",
        "AsyncCandidateProducerContinuationExactRetryCoalesces",
        "AsyncCandidateProducerContinuationHighWatermarkBlocksOldStage",
        "AsyncCandidateProducerContinuationRolloverOnlyStartsEmpty",
        "LocalCandidateProducerContinuationResolutionUsesReviewedFairAction",
        "ConditionalTransportProducerContinuationServiceUsesReviewedFairAction",
        "VolatileBodyProducerContinuationServiceUsesReviewedFairAction",
    )
    observed_theorems = tuple(
        re.findall(
            r"(?m)^[ \t]*(?:LOCAL[ \t]+)?"
            r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"([A-Za-z_][A-Za-z0-9_]*)\b",
            proof_code,
        )
    )
    if observed_theorems != reviewed_theorems:
        errors.append(
            f"{path}: producer-continuation proof must declare exactly the "
            f"reviewed theorem inventory {reviewed_theorems!r}; found "
            f"{observed_theorems!r}"
        )

    required_theorem_dependencies = {
        "AsyncCandidateProducerContinuationResolvedReservedRankStrictlyDrops": (
            "AsyncCandidateProducerContinuationSelectedForResolution",
            "AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn",
            "AsyncCandidateProducerContinuationHandoffRetiredAfterIn",
            "AsyncCandidateProducerContinuationStatusRank",
        ),
        "AsyncCandidateProducerContinuationMaterializedIsOneStep": (
            "AsyncCandidateProducerContinuationSelectedForResolution",
            "AsyncCandidateProducerContinuationRecordAfterStep",
        ),
        "AsyncCandidateProducerContinuationUnselectedActiveRecordIsFixed": (
            "AsyncCandidateProducerContinuationTerminalAfter",
            "AsyncCandidateProducerContinuationSelectedForResolution",
            "AsyncCandidateProducerContinuationSelectedForAcknowledgement",
            "AsyncCandidateProducerContinuationRecordAfterStep",
        ),
        "CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound": (
            "AsyncCandidateProducerContinuationFrozenCandidateOwners",
            "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates",
            "AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates",
            "AsyncCandidateProducerEpisodeCapacity",
            "AsyncCandidateProducerContinuationCapacity",
            "AsyncOrdinaryIngressCarrierEvidenceCapacity",
        ),
        "AsyncFrozenServeSourceCannotResurrectAtGst": (
            "AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal",
            "AsyncIngressPhysicalHighWatermarkIsMonotone",
            "AsyncSharedSchedulerHighWatermarkIsMonotone",
            "AsyncServeQueuedIdentityDepartureInstallsTombstone",
            "AsyncServeTombstonedIdentityCannotRequeueAtGst",
        ),
        "CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier": (
            "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
            "AsyncFrozenLeaderWireBarrierRecords",
            "AsyncLeaderWireLifecycleActive",
        ),
        "CandidateProducerContinuationActionInertDormantHasZeroFrozenStage": (
            "AsyncFrozenLeaderWireBarrierRemainingStage",
            "AsyncFrozenLeaderWireBarrierRecords",
            "AsyncLeaderWireActionInertDormant",
        ),
        "CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix": (
            "AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal",
            "AsyncIngressPhysicalHighWatermarkIsMonotone",
            "AsyncFrozenLeaderWireBarrierRecords",
        ),
        "CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame": (
            "DropPolicyRejectedHiddenPacket",
            "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
            "AsyncFrozenLeaderWireBarrierStageTokens",
            "AsyncFrozenLeaderWireBarrierRecords",
        ),
        "CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage": (
            "LeaderWireIngressDrainNeverInventsRuntimeOwner",
            "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
            "AsyncFrozenLeaderWireBarrierStageBudget",
            "AsyncLeaderWireLifecyclesAfterIngressDrain",
        ),
        "AsyncFrozenLeaderWireIngressRankOrderingIsWellFounded": (
            "NatLessThanWellFounded",
            "WFLexPairOrdering",
            "AsyncFrozenLeaderWireIngressRankOrdering",
            "AsyncFrozenLeaderWireIngressRankCarrier",
        ),
        "AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded": (
            "AsyncFrozenLeaderWireIngressRankOrderingIsWellFounded",
            "AsyncFrozenLeaderWireIngressDependencyRankOrdering",
            "AsyncFrozenLeaderWireIngressDependencyRankCarrier",
            "AsyncFrozenLeaderWirePhysicalRankOrdering",
        ),
        "AsyncFrozenLeaderWireBarrierRankOrderingIsWellFounded": (
            "CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded",
            "AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded",
            "AsyncFrozenLeaderWireBarrierRankOrdering",
            "AsyncFrozenLeaderWireBarrierRankCarrier",
        ),
        "AsyncFrozenLeaderWireBarrierRankIsFinite": (
            "AsyncLeaderWirePotentialPredecessorUniverseIsFinite",
            "CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier",
            "AsyncFrozenLeaderWireBarrierRank",
            "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
        ),
        "AsyncCertifiedResponsePhysicalBarrierRankIsFinite": (
            "AsyncCertifiedResponsePhysicalBarrierRank",
            "AsyncCertifiedResponseFrozenLeaderWireRecords",
            "AsyncCertifiedResponseFrozenLeaderWireStageTokens",
            "AsyncFrozenLeaderWireIngressDependencyRankCarrier",
        ),
        "CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell": (
            "AsyncLeaderWireLifecycleSharedOrdinalInvariant",
            "AsyncCandidateProducerContinuationLifecycleCoverageInvariant",
            "AsyncLeaderWireContinuationSharedOrdinalNoCollisionInvariant",
            "CandidateAdmissionCoalesced",
        ),
        "CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst": (
            "AsyncSharedSchedulerHighWatermarkIsMonotone",
            "AsyncIngressPhysicalHighWatermarkIsMonotone",
            "CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier",
            "AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier",
            "AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal",
            "RuntimeLeaderWireCannotRetireMerelyFromIngressPop",
            "RetireLeaderWireLifecycleRetainsTerminalTombstone",
        ),
        "CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge": (
            "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates",
            "AsyncCandidateProducerContinuationFrozenCausalCandidates",
            "AsyncCandidateProducerContinuationTargetPhysicalCut",
            "AsyncCandidateProducerContinuationGstExcludesResetReplay",
            "AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck",
            "AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor",
            "AsyncCandidateProducerSemanticHandoffRetirementRequiresAck",
        ),
        "CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge": (
            "AsyncCandidateProducerContinuationFrozenCandidateOwners",
            "AsyncCandidateProducerContinuationExactLocalReplayStep",
            "AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation",
            "AsyncCandidateProducerContinuationExactLocalReplayPublishesStoredCarrier",
        ),
        "CandidateProducerContinuationFrozenPrefixStepCannotReplenish": (
            "CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight",
            "CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge",
            "CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell",
            "CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst",
            "CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst",
            "CandidateProducerContinuationFrozenServeCutCannotReplenish",
            "CandidateProducerContinuationTargetPhysicalCutIsStableUntilStatusExit",
            "CandidateProducerContinuationActionInertDormantHasZeroFrozenStage",
            "CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix",
            "CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame",
            "CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage",
            "CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge",
            "AsyncCandidateProducerContinuationFrozenCandidateOwners",
            "AsyncCandidateProducerContinuationFrozenLeaderWireCandidates",
            "AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates",
        ),
        "CandidateProducerContinuationFrozenSourcePrefixStepCannotReplenish": (
            "CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight",
            "CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge",
            "CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge",
            "MatchingClaimedCertifiedResponseIsAuthorized",
            "FirstDrainableIngressIndexIsDrainable",
            "FirstDrainableIngressLaneIndexIsDrainable",
            "DrainFairIngressSelectedClaimPopShape",
            "AsyncFrozenServeSourceCannotResurrectAtGst",
            "AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain",
            "AsyncServeQueuedIdentityDepartureInstallsTombstone",
            "AsyncServeTombstonedIdentityCannotRequeueAtGst",
        ),
        "CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends": (
            "ExternalContinuationFairServiceStrictlyDropsStatusRank",
            "LocalContinuationFairResolutionStrictlyDropsStatusRank",
            "CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner",
            "AsyncCandidateProducerContinuationFrozenSourcePrefixRank",
            "AsyncCandidateProducerContinuationFrozenPrefixRankOrdering",
        ),
        "ResolveCandidateProducerContinuationNeverReplaysDrainedParent": (
            "ResolveCandidateProducerContinuation",
            "asyncCausalQueues",
            "vars",
        ),
        "CandidateProducerContinuationBlocksRunnerUntilHandoffResolution": (
            "RunNodeWork",
            "AsyncCandidateProducerContinuationRunnerResolutionRequired",
        ),
        "CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner": (
            "AsyncCandidateProducerContinuationResolutionRecordsForNode",
            "AsyncCandidateProducerContinuationResolutionPredecessorsFor",
            "AsyncCandidateProducerContinuationSelectedResolutionRecord",
        ),
        "AsyncCandidateProducerContinuationSameHeightRestartPreserved": (
            "AsyncControlServiceStateAfterReset",
            "producerContinuations",
        ),
        "AsyncCandidateProducerContinuationReplacementRetiresOnlyTerminal": (
            "AsyncCandidateProducerContinuationReservationAvailableIn",
            "AsyncCandidateProducerContinuationStateAfterDeparture",
            "record.status",
            "record.ordinal",
        ),
        "AsyncCandidateProducerContinuationExactRetryCoalesces": (
            "AsyncCandidateProducerContinuationRecorded",
            "CandidateAdmissionCoalesced",
        ),
        "LocalCandidateProducerContinuationResolutionUsesReviewedFairAction": (
            "PostGstResolveLocalCandidateProducerContinuation",
            "AsyncFairActionAt",
        ),
        "ConditionalTransportProducerContinuationServiceUsesReviewedFairAction": (
            "PostGstServiceConditionalTransportProducerContinuation",
            "AsyncFairActionAt",
        ),
        "VolatileBodyProducerContinuationServiceUsesReviewedFairAction": (
            "PostGstServiceVolatileBodyProducerContinuation",
            "AsyncFairActionAt",
        ),
    }
    for theorem, dependencies in required_theorem_dependencies.items():
        extracted = _top_level_theorem_body(
            source,
            theorem,
            preserve_string_contents=True,
        )
        if extracted is None:
            continue
        body, line = extracted
        missing = [
            dependency
            for dependency in dependencies
            if not _tla_dependency_present(body, dependency)
        ]
        if missing:
            errors.append(
                f"{path}:{line}: {theorem} must retain reviewed "
                f"producer-continuation dependencies {missing!r}"
            )

    required_network_fragments = {
        "AsyncCandidateServiceLifecycleInvariant": (
            "AsyncCandidateProducerContinuationScheduledExclusionInvariant",
        ),
        "RunNodeWork": (
            "IF AsyncCandidateProducerContinuationOwnsRunNodeTurn(node) "
            "THEN IF "
            "AsyncCandidateProducerContinuationRunnerResolutionReady(node) THEN "
            "ResolveRunNodeCandidateProducerContinuation(node) ELSE "
            "ReplayRunNodeCandidateProducerContinuation(node)",
        ),
        "AsyncControlServiceStateAfterReset": (
            "producerContinuations |-> "
            "AsyncCandidateProducerContinuationsAfterReset(state, resetNodes)",
        ),
        "AsyncTransportInit": (
            "producerContinuations |-> {}",
        ),
        "AsyncControlServiceSlotTransition": (
            "AsyncCandidateProducerContinuationStateAfterDeparture",
            "AsyncCandidateProducerContinuationPartitionInvariantIn",
            "AsyncCandidateProducerContinuationCapacity",
        ),
    }
    for symbol, required in required_network_fragments.items():
        extracted = _top_level_operator_body(
            network_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{network_path}: missing producer-continuation host "
                f"operator {symbol}"
            )
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        missing_or_repeated = [
            fragment for fragment in required if normalized.count(fragment) != 1
        ]
        if missing_or_repeated:
            errors.append(
                f"{network_path}:{line}: {symbol} must retain each reviewed "
                "producer-continuation dependency exactly once; "
                f"missing_or_repeated={missing_or_repeated!r}"
            )

    preservation = _top_level_theorem_body(
        network_source,
        "AsyncNextPreservesCandidateProducerContinuationScheduledExclusion",
        preserve_string_contents=True,
    )
    if preservation is None:
        errors.append(
            f"{network_path}: missing producer-continuation scheduled "
            "exclusion preservation theorem"
        )
    else:
        body, line = preservation
        required = (
            "AsyncStrongTypeInvariant",
            "AsyncProgressOwnershipInvariant",
            "AsyncCandidateServiceLifecycleInvariant",
            "AsyncNext",
            "AsyncCandidateProducerContinuationScheduledExclusionInvariant'",
            "AsyncCandidateProducerContinuationBlocks",
            "CandidateAdmissionCoalesced",
            "AsyncControlServiceSlotTransition",
        )
        missing = [
            dependency
            for dependency in required
            if not _tla_dependency_present(body, dependency)
        ]
        if missing:
            errors.append(
                f"{network_path}:{line}: producer-continuation scheduled "
                f"exclusion preservation must retain {missing!r}"
            )
    return errors


def _producer_continuation_physical_cut_mutation_contract_errors(
    repo_root: Path,
) -> list[str]:
    """Pin positive/failing TLC pairs for each repaired physical-cut lasso."""
    errors: list[str] = []
    formal_dir = repo_root / "formal" / "sumeragi_v2"
    expected_formal = set(
        PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_FORMAL_ARTIFACTS
    )
    expected_digest_paths = expected_formal | {
        PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER
    }
    if (
        len(PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_FORMAL_ARTIFACTS)
        != 11
        or len(expected_formal) != 11
        or set(PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_SHA256)
        != expected_digest_paths
    ):
        errors.append(
            "producer-continuation physical-cut mutation inventory must "
            "contain exactly eleven formal artifacts plus one runner"
        )
    for relative, expected_sha256 in (
        PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_SHA256.items()
    ):
        path = (
            formal_dir / relative
            if relative in expected_formal
            else repo_root / relative
        )
        if not path.is_file() or path.is_symlink():
            continue
        observed_sha256 = _sha256_file(path)
        if observed_sha256 != expected_sha256:
            errors.append(
                f"{path}: physical-cut mutation source SHA-256 must equal "
                f"{expected_sha256}; found {observed_sha256}"
            )
    model_path = (
        formal_dir / "SumeragiV2ProducerContinuationPhysicalCutMutation.tla"
    )
    if not model_path.is_file() or model_path.is_symlink():
        return [
            f"{model_path}: producer-continuation physical-cut mutation "
            "model must be a regular file"
        ]
    try:
        model_source = model_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return [f"{model_path}: cannot read physical-cut mutation model: {error}"]

    required_model_operators = {
        "CurrentIngressFixedSpec": (
            "CurrentIngressInit",
            "CurrentIngressFixedRunner",
            "WF_mutationVars",
        ),
        "CurrentIngressChurnBugSpec": (
            "CurrentIngressInit",
            "CurrentIngressChurnBugRunner",
            "WF_mutationVars",
        ),
        "ContinuationPhysicalCutFixedSpec": (
            "ContinuationInit",
            "ContinuationPhysicalCutFixedRunner",
            "WF_mutationVars",
        ),
        "ContinuationLogicalOnlyBugSpec": (
            "ContinuationInit",
            "ContinuationLogicalOnlyBugRunner",
            "WF_mutationVars",
        ),
        "TimeoutCutSelectionInit": (
            "phase",
            "replaySourcePhysicalOrdinal",
            "targetHasPhysicalSource",
        ),
        "TimeoutCutFilteredSelectsPreCutTarget": (
            "SourcePhysicalOrdinal",
            "LogicalOrdinal",
            "targetDone",
            "lastSelected",
        ),
        "TimeoutCutLogicalMinimumSelectsPostCutReplay": (
            "SourcePhysicalOrdinal",
            "LogicalOnlyPrecedes",
            "replayEpoch",
            "lastSelected",
        ),
        "TimeoutCutFilteredFixedSpec": (
            "TimeoutCutSelectionInit",
            "TimeoutCutFilteredFixedRunner",
            "WF_mutationVars",
        ),
        "TimeoutCutLogicalMinimumBugSpec": (
            "TimeoutCutSelectionInit",
            "TimeoutCutLogicalMinimumBugRunner",
            "WF_mutationVars",
        ),
        "CausalSuccessorRetainsPostCutPhysicalRoot": (
            "replayStage",
            "replaySourcePhysicalOrdinal",
        ),
        "TimeoutCutFilterNeverSelectsPostCutReplay": (
            "phase",
            "lastSelected",
        ),
        "EventuallyExactTargetCompletes": ("targetDone",),
    }
    for symbol, required in required_model_operators.items():
        extracted = _top_level_operator_body(
            model_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{model_path}: missing physical-cut mutation operator {symbol}"
            )
            continue
        body, line = extracted
        tokens = set(tla_code_tokens(body))
        missing = tuple(token for token in required if token not in tokens)
        if missing:
            errors.append(
                f"{model_path}:{line}: {symbol} must retain the exact "
                "physical-cut mutation contract; "
                f"missing={missing!r}"
            )

    required_model_fragments = {
        "TimeoutCutSelectionInit": (
            'phase = "TimeoutSelection"',
            'lastSelected = "None"',
        ),
        "TimeoutCutFilteredSelectsPreCutTarget": (
            'SourcePhysicalOrdinal("Target") < 2',
            'SourcePhysicalOrdinal("Replay") >= 2',
            'lastSelected\' = "Target"',
        ),
        "TimeoutCutLogicalMinimumSelectsPostCutReplay": (
            'SourcePhysicalOrdinal("Replay") >= 2',
            'LogicalOnlyPrecedes("Replay", "Target")',
            'lastSelected\' = "Replay"',
        ),
        "TimeoutCutFilterNeverSelectsPostCutReplay": (
            'phase # "TimeoutSelection"',
            'lastSelected # "Replay"',
        ),
    }
    for symbol, required in required_model_fragments.items():
        extracted = _top_level_operator_body(
            model_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        missing = tuple(fragment for fragment in required if fragment not in normalized)
        if missing:
            errors.append(
                f"{model_path}:{line}: {symbol} must retain exact timeout-cut "
                f"selection identities; missing={missing!r}"
            )

    periodic_model_path = (
        formal_dir / "SumeragiV2AdequateLeaderPeriodicPrefixMutation.tla"
    )
    if not periodic_model_path.is_file() or periodic_model_path.is_symlink():
        errors.append(
            f"{periodic_model_path}: adequate-leader periodic-prefix "
            "mutation model must be a regular file"
        )
    else:
        try:
            periodic_model_source = periodic_model_path.read_text(
                encoding="utf-8"
            )
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{periodic_model_path}: cannot read periodic-prefix "
                f"mutation model: {error}"
            )
        else:
            periodic_operators = {
                "PeriodicPredecessorOrdinals": (
                    "retransmitOrdinal",
                    "TimeoutOrdinal",
                ),
                "FixedInit": (
                    "candidateAhead",
                    "frozenSnapshot",
                    "nextOrdinal",
                ),
                "ServiceFrozenPeriodicIdentity": (
                    "PeriodicRuntimeReady",
                    "frozenSnapshot",
                    "retiredOrdinals",
                ),
                "AcquireFreshPeriodicAtSharedHighWatermark": (
                    "nextOrdinal",
                    "retransmitOrdinal",
                    "FrozenSnapshotRetired",
                ),
                "StartFiniteOwnerEpisodeWithHiddenPeriodicPrefix": (
                    "frozenSnapshot",
                    "candidateAhead",
                    "phase",
                ),
                "ReplaceRetiredPeriodicAtSameOrdinal": (
                    "retiredOrdinals",
                    "replacementEpoch",
                    "retransmitOrdinal",
                ),
                "FixedSpec": ("FixedInit", "FixedNext", "WF_mutationVars"),
                "HiddenPrefixBugSpec": (
                    "HiddenPrefixBugInit",
                    "HiddenPrefixBugNext",
                    "WF_mutationVars",
                ),
                "ReplenishmentBugSpec": (
                    "ReplenishmentBugInit",
                    "ReplenishmentBugNext",
                    "WF_mutationVars",
                ),
                "FrozenPeriodicSnapshotCannotReplenish": (
                    "PeriodicPredecessorOrdinals",
                    "frozenSnapshot",
                ),
                "FiniteOwnerEpisodeStartsAfterPeriodicPrefixDrains": (
                    "FrozenSnapshotRetired",
                    "PeriodicPredecessorOrdinals",
                ),
                "TargetEventuallyDone": ("targetDone",),
            }
            for symbol, required in periodic_operators.items():
                extracted = _top_level_operator_body(
                    periodic_model_source,
                    symbol,
                    preserve_string_contents=True,
                )
                if extracted is None:
                    errors.append(
                        f"{periodic_model_path}: missing periodic-prefix "
                        f"mutation operator {symbol}"
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
                        f"{periodic_model_path}:{line}: {symbol} must retain "
                        "the exact periodic-prefix mutation contract; "
                        f"missing={missing!r}"
                    )

    config_contracts = {
        "current_ingress_physical_cut_fixed.cfg": (
            "SPECIFICATION CurrentIngressFixedSpec",
            "INVARIANT CurrentIngressTurnSelectsExactCarrier",
            "PROPERTY EventuallyExactTargetCompletes",
        ),
        "current_ingress_replenishment_churn_bug.cfg": (
            "SPECIFICATION CurrentIngressChurnBugSpec",
            "PROPERTY EventuallyExactTargetCompletes",
        ),
        "producer_continuation_physical_cut_fixed.cfg": (
            "SPECIFICATION ContinuationPhysicalCutFixedSpec",
            "INVARIANT CausalSuccessorRetainsPostCutPhysicalRoot",
            "PROPERTY EventuallyExactTargetCompletes",
        ),
        "producer_continuation_logical_only_replay_bug.cfg": (
            "SPECIFICATION ContinuationLogicalOnlyBugSpec",
            "INVARIANT CausalSuccessorRetainsPostCutPhysicalRoot",
        ),
        "producer_continuation_timeout_cut_fixed.cfg": (
            "SPECIFICATION TimeoutCutFilteredFixedSpec",
            "INVARIANT TimeoutCutFilterNeverSelectsPostCutReplay",
            "PROPERTY EventuallyExactTargetCompletes",
        ),
        "producer_continuation_timeout_cut_logical_minimum_bug.cfg": (
            "SPECIFICATION TimeoutCutLogicalMinimumBugSpec",
            "PROPERTY EventuallyExactTargetCompletes",
        ),
        "adequate_leader_periodic_prefix_fixed.cfg": (
            "SPECIFICATION FixedSpec",
            "INVARIANT FrozenPeriodicSnapshotCannotReplenish",
            "INVARIANT RetiredPeriodicIdentityCannotResurrect",
            "INVARIANT FiniteOwnerEpisodeStartsAfterPeriodicPrefixDrains",
            "PROPERTY TargetEventuallyDone",
        ),
        "adequate_leader_periodic_hidden_prefix_bug.cfg": (
            "SPECIFICATION HiddenPrefixBugSpec",
            "INVARIANT FiniteOwnerEpisodeStartsAfterPeriodicPrefixDrains",
        ),
        "adequate_leader_periodic_replenishment_bug.cfg": (
            "SPECIFICATION ReplenishmentBugSpec",
            "PROPERTY TargetEventuallyDone",
        ),
    }
    for filename, required in config_contracts.items():
        config_path = formal_dir / filename
        if not config_path.is_file() or config_path.is_symlink():
            errors.append(
                f"{config_path}: physical-cut mutation config must be a "
                "regular file"
            )
            continue
        try:
            config_source = config_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{config_path}: cannot read mutation config: {error}")
            continue
        missing_or_repeated = [
            fragment
            for fragment in required
            if config_source.count(fragment) != 1
        ]
        if missing_or_repeated:
            errors.append(
                f"{config_path}: physical-cut mutation config must retain "
                "each reviewed obligation exactly once; "
                f"missing_or_repeated={missing_or_repeated!r}"
            )

    runner_path = (
        repo_root / PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER
    )
    if not runner_path.is_file() or runner_path.is_symlink():
        errors.append(
            f"{runner_path}: physical-cut mutation runner must be a regular file"
        )
        return errors
    try:
        runner_source = runner_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        errors.append(f"{runner_path}: cannot read mutation runner: {error}")
        return errors
    required_runner_fragments = (
        'readonly TLA2TOOLS_VERSION="1.7.4"',
        'readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"',
        "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88",
        'readonly EXPECTED_JAVA_VERSION=\'openjdk version "21.0.12"\'',
        'readonly MODEL="SumeragiV2ProducerContinuationPhysicalCutMutation.tla"',
        'readonly CURRENT_FIXED_CONFIG="current_ingress_physical_cut_fixed.cfg"',
        'readonly CURRENT_CHURN_BUG_CONFIG="current_ingress_replenishment_churn_bug.cfg"',
        'readonly CONTINUATION_FIXED_CONFIG="producer_continuation_physical_cut_fixed.cfg"',
        'readonly CONTINUATION_LOGICAL_BUG_CONFIG="producer_continuation_logical_only_replay_bug.cfg"',
        'readonly TIMEOUT_FIXED_CONFIG="producer_continuation_timeout_cut_fixed.cfg"',
        'readonly TIMEOUT_LOGICAL_BUG_CONFIG="producer_continuation_timeout_cut_logical_minimum_bug.cfg"',
        'readonly PERIODIC_MODEL="SumeragiV2AdequateLeaderPeriodicPrefixMutation.tla"',
        'readonly PERIODIC_FIXED_CONFIG="adequate_leader_periodic_prefix_fixed.cfg"',
        'readonly PERIODIC_HIDDEN_BUG_CONFIG="adequate_leader_periodic_hidden_prefix_bug.cfg"',
        'readonly PERIODIC_REPLENISHMENT_BUG_CONFIG="adequate_leader_periodic_replenishment_bug.cfg"',
        'current_fixed_log="$(run_tlc current-ingress-fixed "$CURRENT_FIXED_CONFIG" 0)"',
        'continuation_fixed_log="$(run_tlc continuation-cut-fixed "$CONTINUATION_FIXED_CONFIG" 0)"',
        'timeout_fixed_log="$(run_tlc timeout-cut-fixed "$TIMEOUT_FIXED_CONFIG" 0)"',
        'current_bug_log="$(run_tlc current-ingress-churn "$CURRENT_CHURN_BUG_CONFIG" 13)"',
        'timeout_bug_log="$(run_tlc timeout-logical-minimum "$TIMEOUT_LOGICAL_BUG_CONFIG" 13)"',
        'run_tlc adequate-periodic-fixed "$PERIODIC_FIXED_CONFIG" 0 "$PERIODIC_MODEL"',
        '"$PERIODIC_HIDDEN_BUG_CONFIG" 12 "$PERIODIC_MODEL"',
        '"$PERIODIC_REPLENISHMENT_BUG_CONFIG" 13 "$PERIODIC_MODEL"',
        "run_tlc continuation-logical-only \"$CONTINUATION_LOGICAL_BUG_CONFIG\" 12",
        "Error: Temporal properties were violated.",
        "Error: Invariant CausalSuccessorRetainsPostCutPhysicalRoot is violated.",
        "Error: Invariant FiniteOwnerEpisodeStartsAfterPeriodicPrefixDrains is violated.",
        "Back to state",
        "sumeragi_v2_tlc_assert_fixed_success",
        "sumeragi_v2_tlc_assert_nonzero_state_space",
    )
    normalized_runner = " ".join(runner_source.split())
    missing_or_repeated = [
        fragment
        for fragment in required_runner_fragments
        if normalized_runner.count(fragment) != 1
    ]
    if missing_or_repeated:
        errors.append(
            f"{runner_path}: physical-cut mutation runner must retain each "
            "pinned tool, scenario, status, and diagnostic exactly once; "
            f"missing_or_repeated={missing_or_repeated!r}"
        )
    diagnostic_contract = "SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN"
    if normalized_runner.count(diagnostic_contract) != 4:
        errors.append(
            f"{runner_path}: physical-cut mutation runner must apply the "
            "shared primary-diagnostic contract exactly four times"
        )
    return errors
