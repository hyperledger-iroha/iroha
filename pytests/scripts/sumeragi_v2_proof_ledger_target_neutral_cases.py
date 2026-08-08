# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


def _exact_target_neutral_source(module):
    name = "SumeragiV2ExactDecisionStageServiceClosureProofs"
    return name, (module.FORMAL_DIR / f"{name}.tla").read_text(encoding="utf-8")


def test_exact_target_neutral_physical_cut_inventory_is_fully_sealed() -> None:
    """Pin every physical-cut repair helper and theorem in the reviewed aggregate."""

    module = load_checker()
    target_module, source = _exact_target_neutral_source(module)
    stripped = module.strip_tla_comments(source)
    operators = set(
        re.findall(
            r"(?m)^(ExactDecisionTargetNeutral[A-Za-z0-9_]*)\s*(?:\(|==)",
            stripped,
        )
    )
    theorems = set(
        re.findall(
            r"(?m)^[ \t]*(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"(ExactDecisionTargetNeutral[A-Za-z0-9_]*)\s*(?:\(|==)",
            stripped,
        )
    )
    physical_cut_operators = {
        "ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut",
        "ExactDecisionTargetNeutralCausalCandidatesForSnapshot",
        "ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot",
        "ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot",
        "ExactDecisionTargetNeutralExactCandidateOccurrenceTokensForSnapshot",
        "ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot",
        "ExactDecisionTargetNeutralFrozenContinuationStatusTokensForSnapshot",
        "ExactDecisionTargetNeutralFrozenDormantLocalReplayCandidatesForSnapshot",
        "ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot",
        "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot",
        "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot",
        "ExactDecisionTargetNeutralServeReachDebtForSnapshot",
        "ExactDecisionTargetNeutralServeWorkBudgetForSnapshot",
        "ExactDecisionTargetNeutralServeWorkTokensForSnapshot",
    }
    physical_cut_theorems = {
        "ExactDecisionTargetNeutralActiveSnapshotConcreteRankIsInCarrier",
        "ExactDecisionTargetNeutralDormantLocalReplayReplacementConsumesFrozenCausalCharge",
        "ExactDecisionTargetNeutralExactLocalReplayReplacesFrozenCharge",
        "ExactDecisionTargetNeutralFrozenOrdinaryIngressCandidatesCannotReplenish",
        "ExactDecisionTargetNeutralPostCutCausalRootCannotEnterFrozenPrefix",
        "ExactDecisionTargetNeutralPostCutContinuationCannotEnterFrozenPrefix",
        "ExactDecisionTargetNeutralPostCutOrdinaryAdmissionCannotEnterFrozenPrefix",
        "ExactDecisionTargetNeutralPostCutServeCannotEnterFrozenPrefix",
        "ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock",
        "ExactDecisionTargetNeutralSnapshotProducerEpisodeDoesNotReplenish",
        "ExactDecisionTargetNeutralSnapshotProducerEpisodeStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralSnapshotRemainsActiveAtFixedClock",
    }

    assert len(operators) == module.EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_COUNT == 130
    assert len(theorems) == module.EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_COUNT == 66
    assert physical_cut_operators <= operators
    assert physical_cut_theorems <= theorems
    ledger = module.load_ledger()
    assert module._proof_obligation_architecture_errors(
        ledger["obligations"], {target_module: source}
    ) == []


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "ExactDecisionTargetNeutralCandidateRootPrecedesPhysicalCut",
            "< snapshot.physicalCuts[candidate.node]",
            "<= snapshot.physicalCuts[candidate.node]",
        ),
        (
            "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot",
            "record.sourcePhysicalOrdinal < snapshot.physicalCuts[node]",
            "record.sourcePhysicalOrdinal <= snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralServeIngressIdentitiesForSnapshot",
            "< snapshot.physicalCuts[node]",
            "<= snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralFrozenOrdinaryIngressRecordsForSnapshot",
            "carrier.physicalOrdinal < snapshot.physicalCuts[node]",
            "carrier.physicalOrdinal <= snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralFrozenContinuationRecordsForSnapshot",
            "record.sourcePhysicalOrdinal < snapshot.physicalCuts[node]",
            "record.sourcePhysicalOrdinal <= snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot",
            "ExactDecisionTargetNeutralChargeableOrdinaryIngressCandidatesForSnapshot",
            "ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot",
        ),
    ),
)
def test_exact_target_neutral_physical_cut_operator_seams_reject_weakening(
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module, source = _exact_target_neutral_source(module)
    mutated = mutate_tla_operator(source, symbol, old, new)

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"], {target_module: mutated}
    )

    assert any(
        "target-neutral operator inventory/body contract must stay fully token-sealed"
        in error
        or symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "ExactDecisionTargetNeutralPostCutOrdinaryAdmissionCannotEnterFrozenPrefix",
            "carrier.physicalOrdinal >= snapshot.physicalCuts[node]",
            "carrier.physicalOrdinal > snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralPostCutCausalRootCannotEnterFrozenPrefix",
            "lifecycle.sourcePhysicalOrdinal\n            >= snapshot.physicalCuts[node]",
            "lifecycle.sourcePhysicalOrdinal\n            > snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralPostCutContinuationCannotEnterFrozenPrefix",
            "record.sourcePhysicalOrdinal >= snapshot.physicalCuts[node]",
            "record.sourcePhysicalOrdinal > snapshot.physicalCuts[node]",
        ),
        (
            "ExactDecisionTargetNeutralPostCutServeCannotEnterFrozenPrefix",
            "AsyncServeIngressAdmissionOrdinal(node, identity)\n         >=",
            "AsyncServeIngressAdmissionOrdinal(node, identity)\n         >",
        ),
        (
            "ExactDecisionTargetNeutralDormantLocalReplayReplacementConsumesFrozenCausalCharge",
            "ExactDecisionTargetNeutralCausalCandidatesForSnapshot(\n               snapshot, node)",
            "AsyncCandidateSet",
        ),
        (
            "ExactDecisionTargetNeutralExactLocalReplayReplacesFrozenCharge",
            "= ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot(\n               snapshot, node)",
            "\\subseteq ExactDecisionTargetNeutralProoflessCandidateOwnersForSnapshot(\n               snapshot, node)",
        ),
    ),
)
def test_exact_target_neutral_physical_cut_statements_reject_weakening(
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module, source = _exact_target_neutral_source(module)
    mutated = mutate_tla_theorem(source, symbol, old, new)

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"], {target_module: mutated}
    )

    assert any(
        "target-neutral theorem inventory/statement contract must stay fully token-sealed"
        in error
        or symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock",
            "AsyncOrdinaryIngressTicketExcludesLaterLocalWork",
        ),
        (
            "ExactDecisionTargetNeutralSnapshotPredecessorsDoNotReplenishAtFixedClock",
            "AsyncCandidateProducerContinuationFrozenOwnerPrecedesPostCutReplay",
        ),
        (
            "ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame",
            "ExactDecisionTargetNeutralExactCandidateOccurrenceBudgetForSnapshot",
        ),
        (
            "ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame",
            "ExactDecisionTargetNeutralServeReachDebtForSnapshot",
        ),
        (
            "ExactDecisionTargetNeutralFrozenOrdinaryIngressCandidatesCannotReplenish",
            "LaterAcceptedOrdinaryCarrierCannotOvertakeFrozenCarrier",
        ),
        (
            "ExactDecisionTargetNeutralDormantLocalReplayReplacementConsumesFrozenCausalCharge",
            "ExactDecisionTargetNeutralPostCutContinuationCannotEnterFrozenPrefix",
        ),
        (
            "ExactDecisionTargetNeutralExactLocalReplayReplacesFrozenCharge",
            "AsyncCandidateCausalSuccessorInheritsContinuationPhysicalOwnership",
        ),
        (
            "ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame",
            "ExactDecisionTargetNeutralFrozenOrdinaryIngressCandidatesCannotReplenish",
        ),
        (
            "ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame",
            "ExactDecisionTargetNeutralDormantLocalReplayReplacementConsumesFrozenCausalCharge",
        ),
        (
            "ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame",
            "ExactDecisionTargetNeutralExactLocalReplayReplacesFrozenCharge",
        ),
    ),
)
def test_exact_target_neutral_physical_cut_proof_seams_reject_deletion(
    symbol: str,
    token: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module, source = _exact_target_neutral_source(module)
    mutated = delete_tla_theorem_token(source, symbol, token)

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"], {target_module: mutated}
    )

    assert any(
        f"{symbol} must retain reviewed proof dependencies" in error
        and token in error
        for error in errors
    ), errors
