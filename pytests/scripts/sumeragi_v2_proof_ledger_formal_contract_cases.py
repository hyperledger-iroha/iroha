# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def test_ledger_is_canonical_json() -> None:
    module = load_checker()
    source = module.LEDGER_PATH.read_text(encoding="utf-8")
    parsed = json.loads(source)

    assert source == json.dumps(parsed, indent=2, ensure_ascii=False) + "\n"


def copy_audited_rank_leaf_contract_fixture(tmp_path: Path, module) -> Path:
    """Install the reviewed Stage-4/5 contracts around the current proof source."""

    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    vocabulary_source = vocabulary.read_text(encoding="utf-8")
    property_block = r'''
ProtectedStage4RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<4, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<4, position>>))

ProtectedStage5RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<5, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<5, position>>))
'''
    if "ProtectedStage4RankProgressProperty" not in vocabulary_source:
        vocabulary_source = vocabulary_source.replace(
            "=============================================================================\n",
            property_block + "\n=============================================================================\n",
            1,
        )
        vocabulary.write_text(vocabulary_source, encoding="utf-8")

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof_source = proof.read_text(encoding="utf-8")
    wrapper_block = r'''
THEOREM ProtectedStage4RankProgressFromFairScheduler ==
  \A initialContext:
    ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage4RankDescent
   DEF ProtectedStage4RankProgressProperty

THEOREM ProtectedStage5RankProgressFromFairFifo ==
  \A initialContext:
    ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage5RankDescent
   DEF ProtectedStage5RankProgressProperty
'''
    if "ProtectedStage4RankProgressFromFairScheduler" not in proof_source:
        proof_source = proof_source.replace(
            "=============================================================================\n",
            wrapper_block + "\n=============================================================================\n",
            1,
        )
        proof.write_text(proof_source, encoding="utf-8")
    return formal_dir


def audited_rank_leaf_contract_errors(module, formal_dir: Path) -> list[str]:
    """Run both source and ledger-target guards for the audited rank leaves."""

    proof_source = (
        formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    ).read_text(encoding="utf-8")
    errors = module._async_proof_architecture_errors(formal_dir)
    errors.extend(
        module._proof_obligation_architecture_errors(
            module.load_ledger()["obligations"],
            {"SumeragiV2AsyncLivenessProofs": proof_source},
        )
    )
    return errors


def test_audited_rank_leaf_synthetic_contract_is_green(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)

    assert audited_rank_leaf_contract_errors(module, formal_dir) == []


@pytest.mark.parametrize(
    ("filename", "kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => <>AsyncProgressOwnershipInvariant",
            "AsyncSpecAlwaysProgressOwnershipInvariant must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncBracketNextPreservesProgressOwnership",
            "AsyncBracketNextPreservesStrongTypeInvariant",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage4RankProgressProperty",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "ProtectedStage4RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage4RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage4RankProgressFromFairScheduler must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "BY FairProtectedStage4RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage5RankProgressProperty",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "ProtectedStage5RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage5RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage5RankProgressFromFairFifo must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "BY FairProtectedStage5RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
    ),
)
def test_audited_rank_leaf_source_mutations_fail_closed(
    tmp_path: Path,
    filename: str,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)
    path = formal_dir / filename
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = audited_rank_leaf_contract_errors(module, formal_dir)
    assert any(
        expected_error in error and symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "ProtectedServeStage5CarrierFacts",
            "ServeOccurrenceIndexCharacterization",
        ),
        (
            "ProtectedServeStage5EnablesFairWorker",
            "QueuedIoEnablesPostGstService",
        ),
        (
            "ProtectedServeStage5WorkerStrictlyProgresses",
            "TailRemovesUniqueServeOccurrence",
        ),
        (
            "ProtectedServeStage5UnlessProgress",
            "AsyncBracketNextPreservesStrongTypeInvariant",
        ),
        (
            "FairProtectedServeStage5RankDescent",
            "ProtectedServeStage5EnablesFairWorker",
        ),
        (
            "ProtectedServeRankProgressFromFairFifo",
            "FairProtectedServeStage5RankDescent",
        ),
    ),
)
def test_protected_serve_fifo_proof_dependency_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    token: str,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof.write_text(
        delete_tla_theorem_token(
            proof.read_text(encoding="utf-8"),
            symbol,
            token,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error
        and "omits explicit transition/fairness inventory" in error
        and token in error
        for error in errors
    ), errors


def test_serve_occurrence_rank_and_starvation_conjunct_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ServeJobRank(node, job) == <<5, ServeJobIndex(node, job)>>",
            "ServeJobRank(node, job) == <<5, CandidateIoIndex("
            "job.candidate, asyncIoQueues[node])>>",
            1,
        ).replace(
            "     \\/ ProtectedServeRankDecreaseStep\n",
            "",
            1,
        ).replace(
            "  /\\ ProtectedServeStarvationProperty(specification)\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ServeJobRank must equal only" in error for error in errors)
    assert any("PostGstProductiveStep must equal only" in error for error in errors)
    assert any("StarvationFreedomProperty must equal only" in error for error in errors)


def test_exact_removal_and_protected_slot_geometry_theorems_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    removal = source.index("THEOREM OneRemovalIncreasesSourceProtectionByAtMostOne")
    universe = source.index("THEOREM ProtectedProgressSlotUniverseSize")
    mutated = (
        source[:removal]
        + source[removal:universe].replace(
            "LET after == SequenceWithoutIndex(before, selected)",
            "LET after == Tail(before)",
            1,
        )
        + source[universe:].replace(
            "Cardinality(ProtectedProgressSlotUniverse) = 2 * N + 3",
            "Cardinality(ProtectedProgressSlotUniverse) = N + 3",
            1,
        )
    )
    proofs.write_text(mutated, encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "OneRemovalIncreasesSourceProtectionByAtMostOne must state only" in error
        for error in errors
    )
    assert any(
        "ProtectedProgressSlotUniverseSize must state only" in error
        for error in errors
    )


def test_normal_proposal_prepare_protection_contract_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "     \\/ NormalProposalPrepareCandidate(candidate)\n", "", 1
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_kind_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"Proposal", "PrepareVote", "CommitVote"}',
            '{"Proposal", "PrepareVote"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkKinds must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_requires_canonical_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ candidate \\in AsyncCandidateSet\n",
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ AsyncCandidateTyped(candidate)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_delivery_class_is_frozen_at_admission(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    frozen_network = (
        "    /\\ candidate = FrozenNormalDeliveryCandidate(\n"
        "                     item, consumerContext, consumerView,\n"
        "                     consumerGeneration)\n"
    )
    assert frozen_network in source
    vocabulary.write_text(
        source.replace(
            frozen_network,
            "    /\\ candidate = NormalDeliveryCandidate(item)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkCandidate must equal only" in error
        for error in errors
    )

    frozen_identity = (
        "       consumerContext, consumerView, consumerGeneration, item,\n"
    )
    assert frozen_identity in source
    vocabulary.write_text(
        source.replace(
            frozen_identity,
            "       context, nodeView[item.envelope.recipient],\n"
            "       generation[item.envelope.recipient], item,\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenNormalDeliveryCandidate must equal only" in error
        for error in errors
    )


def test_normal_install_successor_is_required_and_frozen(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    install_successor_branch = (
        "     \\/ \\E command \\in AsyncCandidateSet,\n"
        "            installedContext \\in ContextRecords,\n"
        "            priorGeneration \\in Generations,\n"
        "            subject \\in SubjectOrNone:\n"
        "          /\\ command.kind = \"PersistInstallTC\"\n"
        "          /\\ command.view + 1 \\in Views\n"
        "          /\\ candidate = FrozenInstallProposalSuccessor(\n"
        "                           command, installedContext,\n"
        "                           priorGeneration, subject)\n"
    )
    assert install_successor_branch in source
    vocabulary.write_text(
        source.replace(install_successor_branch, "", 1),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNoItemCandidate must equal only" in error
        for error in errors
    )

    frozen_generation = "NextCandidateGeneration(priorGeneration)"
    assert frozen_generation in source
    vocabulary.write_text(
        source.replace(
            frozen_generation,
            "generation[command.node]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenInstallProposalSuccessor must equal only" in error
        for error in errors
    )


def test_begin_prepare_parent_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"DeliverProposal", "ValidateBody"}',
            '{"DeliverProposal"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalBeginPrepareParentKinds must equal only" in error
        for error in errors
    )


def test_normal_candidate_step_stability_theorem_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    proofs.write_text(
        source.replace(
            "    /\\ AsyncNext\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            "    /\\ PostGstSchedulerActionEnabled\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "AsyncNextPreservesNormalProposalPrepareCandidate must state only"
        in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_enablement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    property_offset = source.index("DeadlockFreedomProperty(specification) ==")
    enabled_offset = source.index(
        "PostGstProductiveActionEnabled", property_offset
    )
    vocabulary.write_text(
        source[:enabled_offset]
        + source[enabled_offset:].replace(
            "PostGstProductiveActionEnabled",
            "PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "DeadlockFreedomProperty must equal only" in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_productive_alias(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "PostGstProductiveActionEnabled == ENABLED PostGstProductiveStep",
            "PostGstProductiveActionEnabled == PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "PostGstProductiveActionEnabled must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_dual_progress_ingress_geometry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '   "TimeoutCertificate", "CertifiedRequest", "CommitCertificateRequest",\n',
            '   "TimeoutCertificate", "CommitCertificateRequest",\n',
            1,
        ).replace(
            "    + Cardinality(\n"
            "        IngressTransportCompletionProtectedSourcesFor(lanes, recipient))\n",
            "",
            1,
        ).replace(
            'IngressTransportCompletionKinds == {"Chunk", "CertifiedResponse"}',
            'IngressTransportCompletionKinds == {"Chunk"}',
            1,
        ).replace(
            "  \\/ ~IngressLaneHasTransportCompletionIn(\n"
            "       asyncIngressLanes, item.envelope.recipient, item.source)\n",
            "  \\/ TRUE\n",
            1,
        ).replace(
            '                    "TimeoutCertificate", "Chunk", "CertifiedResponse",\n'
            '                    "CommitCertificateResponse",\n',
            '                    "TimeoutCertificate", "Chunk", "CertifiedRequest",\n'
            '                    "CertifiedResponse", "CommitCertificateRequest",\n'
            '                    "CommitCertificateResponse",\n',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "IngressTransportCompletionKinds must equal only" in error
        for error in errors
    )
    assert any("IngressProgressKinds must equal only" in error for error in errors)
    assert any(
        "IngressProtectedSlotCountFor must equal only" in error for error in errors
    )
    assert any(
        "AsyncTransportCompletionOwnerGateAllows must equal only" in error
        for error in errors
    )
    assert any("DeliveryClass must equal only" in error for error in errors)


def test_async_source_fidelity_pins_untrusted_transport_completion_exclusion(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in AsyncIngressSources,\n",
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in ValidatorIds,\n",
            1,
        )
        .replace(
            '  /\\ (item.kind \\notin {"Noise", "Chunk", "CertifiedResponse"}\n'
            "        => item.source \\in ValidatorIds)",
            '  /\\ (item.kind # "Noise" => item.source \\in ValidatorIds)',
            1,
        )
        .replace(
            "  IN /\\ kind \\in IngressTransportCompletionKinds\n",
            '  IN /\\ kind = "Chunk"\n',
            1,
        )
        .replace("     /\\ nonce = 0\n", "", 1)
        .replace(
            "       InjectUntrustedTransportCompletion(kind, recipient, nonce)\n",
            "       TRUE\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncNetworkItems omits required production behavior" in error
        for error in errors
    )
    assert any(
        "AsyncItemTyped omits required production behavior" in error
        for error in errors
    )
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )
    assert any(
        "AsyncFaultStep omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        source.replace("     /\\ nonce = 0\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )


def test_async_source_fidelity_pins_timeout_signer_partition_without_displacement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            "AsyncDeferredProgressCapacity >= 2 * N + 3",
            "AsyncDeferredProgressCapacity >= N + 3",
            1,
        ).replace(
            '    [] command.kind = "DeliverTimeout" ->\n'
            '         command.item.kind = "TimeoutVote"\n',
            "",
            1,
        ).replace(
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN queue\n",
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN SequenceWithoutIndex(queue, 1)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncConfiguration omits required production behavior" in error
        for error in errors
    )
    assert any("ProtectedProgressCommand must equal only" in error for error in errors)
    assert any("DeferredProgressAfter must equal only" in error for error in errors)


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "       /\\ candidate.kind \\in AsyncTimeoutLifecycleKinds\n",
            "       /\\ candidate.causalOrigin.phase "
            "\\in AsyncTimeoutLifecycleKinds\n",
        ),
        (
            "       QueuedCandidates \\cup DeferredCandidates\n"
            "         \\cup CausalCandidates \\cup TrackedWorkCandidates:\n",
            "       QueuedCandidates \\cup DeferredCandidates\n"
            "         \\cup CausalCandidates:\n",
        ),
    ),
)
def test_async_source_fidelity_pins_current_timeout_lifecycle_stage_classifier(
    tmp_path: Path, old: str, new: str
) -> None:
    """Retained timeout origins must not turn proposal successors into clocks."""

    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    assert old in source
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(old, new, 1),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncOlderOrEqualTimeoutLifecycleOwned must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_live_serve_occurrence_identity(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            'AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))',
            'AsyncIoJob("Serve", candidate, 0)',
            1,
        ).replace(
            "    /\\ AsyncIoServeNonceOwnership(asyncIoQueues[node])\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncIoCertifiedServeJob must equal only" in error for error in errors)
    assert any(
        "AsyncIoQueueContentTypeInvariant must equal only" in error
        for error in errors
    )


def copy_timeout_vote_window_fixture(tmp_path: Path, module) -> Path:
    """Copy the bounded TimeoutVote production and regression sources."""

    relatives = (
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/types.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/tests.rs"),
    )
    for relative in relatives:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(module.ROOT_DIR / relative, destination)
    return tmp_path / relatives[0]


def test_async_source_fidelity_pins_timeout_vote_semantic_capacity_bypass(
    tmp_path: Path,
) -> None:
    """The bounded TimeoutVote production and regression sources are sealed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert errors == []


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "admit_authenticated_payload",
            (
                "if !reducer::timeout_vote_view_is_admissible("
                "current_view, vote.round.view)"
            ),
            "if false",
            "current/adjacent view window",
        ),
        (
            "admit_authenticated_payload",
            (
                "locked_commit_progress || matches!(key, "
                "IngressSemanticKey::TimeoutVote { .. })"
            ),
            "locked_commit_progress",
            "bypass only ordinary semantic capacity",
        ),
        (
            "prune_ingress_records",
            (
                "round.height == current_height\n"
                "                        "
                "&& reducer::timeout_vote_view_is_admissible("
                "current_view, round.view)"
            ),
            "round.height == current_height",
            "retained only at the current height and current/adjacent view",
        ),
        (
            "prune_ingress_records",
            (
                "matches_current_lock(*key, record.fingerprint) "
                "|| matches_retained_timeout(*key)"
            ),
            "matches_current_lock(*key, record.fingerprint)",
            "preserve either the exact lock or retained TimeoutVote",
        ),
    ),
)
def test_timeout_vote_semantic_capacity_rejects_real_source_mutations(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Bounded admission and both protected prune arms fail closed."""

    module = load_checker()
    rust_path = copy_timeout_vote_window_fixture(tmp_path, module)
    mutate_rust_item_source_in_context(
        module,
        rust_path,
        item_name,
        (("impl", "SumeragiV2Adapter"),),
        old,
        new,
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


def test_timeout_vote_semantic_capacity_rejects_two_roster_sets(
    tmp_path: Path,
) -> None:
    """The semantic table reserves lock plus both bounded timeout rounds."""

    module = load_checker()
    rust_path = copy_timeout_vote_window_fixture(tmp_path, module)
    mutate_rust_item_source(
        module,
        rust_path,
        "semantic_ingress_capacity",
        "roster_len.saturating_mul(3)",
        "roster_len.saturating_mul(2)",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        "three roster-bounded protected sets" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 1",
            "FUTURE_TIMEOUT_VOTE_LOOKAHEAD: u64 = 2",
            "lookahead must remain exactly one view",
        ),
        (
            "current_view.saturating_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)",
            "current_view.wrapping_add(FUTURE_TIMEOUT_VOTE_LOOKAHEAD)",
            "lower bound and saturating one-view upper bound",
        ),
    ),
)
def test_timeout_vote_view_window_rejects_predicate_mutations(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """The one-round helper cannot widen, wrap, or lose its exact bound."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    types_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/types.rs"
    mutate_source_once(types_path, old, new)

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "on_timeout_vote",
            (
                "if !timeout_vote_view_is_admissible("
                "self.durable.current_view(), vote.round().view())"
            ),
            "if false",
            "admission must use the bounded current/adjacent predicate",
        ),
        (
            "on_persisted",
            "self.timeout_votes.retain(|round, _| {",
            "self.timeout_votes.clear();\n                if false {",
            "retain exactly the current/adjacent vote and formed-certificate pools",
        ),
        (
            "on_persisted",
            "self.formed_timeouts.retain(|round| {",
            "self.formed_timeouts.clear();\n                if false {",
            "retain exactly the current/adjacent vote and formed-certificate pools",
        ),
    ),
)
def test_timeout_vote_view_window_rejects_reducer_mutations(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Reducer admission and both install-retention pools stay bounded."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    reducer_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/reducer.rs"
    mutate_rust_item_source_in_context(
        module,
        reducer_path,
        item_name,
        (("impl", "Reducer"),),
        old,
        new,
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "adjacent_future_timeout_votes_form_a_catch_up_certificate",
        "timeout_install_preserves_adjacent_shares_for_the_new_current_view",
        "timeout_votes_beyond_adjacent_lookahead_are_ignored",
    ),
)
def test_timeout_vote_view_window_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Catch-up, install preservation, and far-future rejection stay sealed."""

    module = load_checker()
    copy_timeout_vote_window_fixture(tmp_path, module)
    tests_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_core/tests.rs"
    mutate_rust_item_source(
        module,
        tests_path,
        test_name,
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "capacity_bypass_records_follow_current_lock_and_timeout_view",
        "adjacent_future_timeout_vote_remains_retryable_until_current_view_advances",
        "full_normal_deferred_lane_cannot_drop_absolute_timeout",
        "busy_deferred_source_identity_coalesces_across_consumer_view_change",
    ),
)
def test_timeout_vote_semantic_capacity_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Capacity, adjacent, full-lane, and cross-view regressions stay exact."""

    module = load_checker()
    rust_path = copy_timeout_vote_window_fixture(tmp_path, module)
    mutate_rust_item_source(
        module,
        rust_path,
        test_name,
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._timeout_vote_semantic_capacity_source_fidelity_errors(
        tmp_path
    )

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


MERGE_RUNTIME_PROJECTED_FIELDS = (
    "merge_sidecar_inbound_session_capacity",
    "merge_sidecar_inbound_sessions_per_peer",
    "merge_sidecar_inbound_assembly_bytes",
    "merge_sidecar_inbound_assembly_bytes_per_peer",
    "merge_sidecar_deferred_block_capacity",
    "merge_sidecar_future_block_distance",
    "merge_sidecar_request_timeout_ms",
    "merge_sidecar_outbound_sessions_per_source",
    "merge_sidecar_outbound_bytes_per_source",
    "merge_sidecar_server_request_gates_per_source",
    "pending_certified_merge_entry_capacity",
    "pending_queue_plan_admission_capacity",
    "pending_control_sidecar_bytes",
    "merge_signing_guard_record_capacity",
    "merge_signing_guard_record_bytes",
    "merge_signing_guard_total_bytes",
)


def test_merge_runtime_config_v6_inventory_is_static_and_current() -> None:
    module = load_checker()
    checker_source = SCRIPT.read_text(encoding="utf-8")

    assert tuple(
        projected_field
        for projected_field, *_rest in module.MERGE_RUNTIME_CONFIG_FIELDS
    ) == MERGE_RUNTIME_PROJECTED_FIELDS
    assert len(module.MERGE_RUNTIME_CONFIG_FIELDS) == 16
    assert (
        checker_source.count(
            '"pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;"'
        )
        == 2
    )
    assert (
        '"pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 3;"'
        not in checker_source
    )


def test_merge_runtime_config_v6_source_binding_accepts_repository() -> None:
    module = load_checker()

    assert module._merge_runtime_config_production_source_fidelity_errors() == []


@pytest.mark.parametrize(
    ("relative", "injected"),
    (
        (
            Path("crates/iroha_config/src/parameters/actual.rs"),
            "\nfn retired_ttl_config_mutant() {\n"
            "    let merge_sidecar_server_request_gate_ttl_ms = 1_u64;\n"
            "    drop(merge_sidecar_server_request_gate_ttl_ms);\n"
            "}\n",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
            "\nfn retired_ttl_runner_mutant() {\n"
            "    let merge_sidecar_server_request_gate_ttl = "
            "core::time::Duration::from_secs(1);\n"
            "    drop(merge_sidecar_server_request_gate_ttl);\n"
            "}\n",
        ),
        (
            Path("crates/iroha_core/src/merge_sidecar.rs"),
            "\nconst SERVER_REQUEST_GATE_TTL: u64 = 1;\n",
        ),
    ),
)
def test_merge_runtime_config_v6_rejects_reintroduced_wall_clock_gate_ttl(
    tmp_path: Path,
    relative: Path,
    injected: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    path = repo_root / relative
    canonical_source = path.read_text(encoding="utf-8")
    module = load_checker()
    assert (
        module._retired_sidecar_gate_ttl_source_errors(
            path,
            canonical_source,
            str(relative),
        )
        == []
    )
    path.write_text(
        canonical_source + injected,
        encoding="utf-8",
    )

    errors = module._retired_sidecar_gate_ttl_source_errors(
        path,
        path.read_text(encoding="utf-8"),
        str(relative),
    )

    assert any(
        "retired wall-clock sidecar gate TTL must remain absent from production"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize("field", MERGE_RUNTIME_PROJECTED_FIELDS)
def test_merge_runtime_config_v6_rejects_each_projection_field_substitution(
    tmp_path: Path,
    field: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    actual_path = repo_root / "crates/iroha_config/src/parameters/actual.rs"
    source = actual_path.read_text(encoding="utf-8")
    projection_start = source.index("limits: SumeragiV2Limits {")
    projection_end = source.index(
        "native_amx_signing_guard_record_capacity,", projection_start
    )
    needle = f"                {field},"
    position = source.index(needle, projection_start, projection_end)
    replacement = f"                {field}: 0,"
    actual_path.write_text(
        source[:position] + replacement + source[position + len(needle) :],
        encoding="utf-8",
    )

    errors = merge_runtime_config_errors(repo_root)

    assert any(
        "shared fingerprint projection carries all 16 config-v6 merge fields"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "region", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION:",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;",
            "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 5;",
            "merge-runtime shared-config format version 6",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY:",
            "V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "V2_RETIRED_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "config-v6 default V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES:",
            "V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES",
            "V2_RETIRED_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES",
            "merge-signing metadata headroom has one named config source",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "pub struct SumeragiV2RuntimeLimits {",
            "defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
            "defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER",
            "user config field merge_sidecar_inbound_session_capacity",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "limits: actual::SumeragiV2RuntimeLimits {",
            ".merge_sidecar_inbound_session_capacity,",
            ".merge_sidecar_inbound_sessions_per_peer,",
            "user parsing maps all 16 config-v6 merge fields without substitution",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "let merge_sidecar_inbound_session_capacity = canonical_bounded_size(",
            "merge_sidecar_inbound_sessions_per_peer,\n"
            "            merge_sidecar_inbound_session_capacity,",
            "merge_sidecar_inbound_sessions_per_peer,\n"
            "            merge_sidecar_inbound_sessions_per_peer,",
            "config validation preserves decided and ordinary inbound session corridors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "let merge_sidecar_limits = MergeSidecarLimits::new(",
            "non_zero(config.limits.merge_sidecar_inbound_sessions_per_peer)?",
            "non_zero(config.limits.merge_sidecar_inbound_session_capacity)?",
            "runner constructs live sidecar and signing limits from all projected merge fields",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "None => MergeSidecarTransport::open_durable_with_server_stream_capacity(",
            "limits.merge_sidecar_limits,",
            "MergeSidecarLimits::defaults(),",
            "adapter must derive the canonical responder roster and restore or open only its exact durable source, stream, and roster geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "let mut adapter = Self {",
            "merge_sidecars,\n"
            "            exact_output_handoff_owner,\n"
            "            authenticated_merge_qcs:",
            "merge_sidecars,\n"
            "            authenticated_merge_qcs:",
            "adapter hands the exact rehydrated sidecar transport into the live production field",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "Self::derive_server_request_capacities(\n"
            "                reply_source_capacity,\n"
            "                limits,\n"
            "                server_stream_capacity,\n"
            "            )?",
            "Self::derive_server_request_capacities(\n"
            "                reply_source_capacity,\n"
            "                limits,\n"
            "                MAX_CERTIFIED_MERGE_SEMANTIC_PEERS,\n"
            "            )?",
            "live sidecar transport derives checked source-partition capacities",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "let merge_signing_guard = MergeSigningGuard::open_with_committed_frontier(",
            "limits.merge_signing_guard_limits,",
            "MergeSigningGuardLimits::defaults(),",
            "adapter opens the durable merge-signing journal with fingerprinted limits",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn defer_block_with_priority(",
            "self.limits.future_block_distance",
            "u64::MAX",
            "live sidecar carrier admission consumes configured future distance",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if bytes.len() > self.limits.max_record_bytes",
            "total > self.limits.max_total_bytes",
            "total > usize::MAX",
            "merge-signing authorization consumes configured aggregate bytes",
        ),
        (
            "crates/irohad/src/main.rs",
            "Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(",
            "&config.sumeragi.limits,",
            "&iroha_config::parameters::actual::SumeragiV2RuntimeLimits::default(),",
            "daemon passes fingerprinted pending-control limits into production Kura",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "let pending_control_sidecar_limits = PendingControlSidecarLimits::from_config(",
            "sumeragi_limits,",
            "&SumeragiV2RuntimeLimits::default(),",
            "Kura validates pending-control limits before opening its store",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "pub(crate) fn persist_pending_certified_merge_entry(",
            "paths.len() == self.pending_control_sidecar_limits.certified_merge_entries",
            "paths.len() == usize::MAX",
            "Kura merge admission consumes the configured pending-entry count",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "pub fn persist_pending_queue_plan_admission_certificate(",
            "paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions",
            "paths.len() == usize::MAX",
            "Kura QueuePlan admission consumes the configured certificate count",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "fn validate_pending_merge_entries_on_startup(",
            ".combined_bytes_within_limit(merge_bytes, admission_bytes)",
            ".merge_bytes_within_limit(merge_bytes)",
            "Kura startup consumes the configured shared pending byte limit",
        ),
    ),
)
def test_merge_runtime_config_v6_rejects_disconnected_production_seams(
    tmp_path: Path,
    relative: str,
    region: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    repo_root = copy_merge_runtime_config_fixture(tmp_path)
    path = repo_root / relative
    source = path.read_text(encoding="utf-8")
    region_start = source.index(region)
    mutation = source.index(old, region_start)
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = merge_runtime_config_errors(repo_root)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "check_queue_limit",
            ".checked_add(frame_len)",
            ".saturating_add(frame_len)",
            "checked byte/frame queue admission and overflow rejection",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "encrypted_frame_geometry",
            "u32::try_from(encrypted_size).map_err(|_| Error::FrameTooLarge)?",
            "encrypted_size as u32",
            "checked encrypted sender geometry encrypted_frame_geometry",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
            "crate::peer::data_message_wire_len_from_payload_len::<RelayMessage<T>>(relay_len)",
            "relay_len",
            "checked P2P transport geometry "
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "enqueue_encrypted",
            "if encrypted_size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if encrypted_size > self.max_frame_bytes {",
            "checked runtime-clamped encrypted geometry before cap/queue admission",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "account_enqueued",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .checked_add(frame_len)",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .saturating_add(frame_len)",
            "checked admitted queue-byte accounting",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_plaintext_cap",
            ".min(MAX_ENCRYPTED_FRAME_BYTES)",
            ".min(usize::MAX)",
            "checked P2P transport geometry frame_plaintext_cap",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_queue_charge",
            ".checked_add(P2P_FRAME_LENGTH_PREFIX_BYTES)",
            ".checked_add(0)",
            "checked P2P transport geometry frame_queue_charge",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_short_p2p_frame_math(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "checked_encoded_frame_len",
            "let encoded_len = ncore::encoded_frame_len(message)?;",
            "let encoded_len = 0;",
            "exact Norito counting preflight before P2P allocation",
        ),
        (
            "try_send",
            "if encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if false && encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "QUIC counting preflight and post-encryption runtime-cap check",
        ),
        (
            "reserve_for_frame",
            "if size > self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if size > self.max_frame_bytes {",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "reserve_for_frame",
            ".ok_or(Error::FrameTooLarge)?\n                .min(needed);",
            ".ok_or(Error::FrameTooLarge)?\n                .min(usize::MAX);",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "prepare_message",
            "let encoded_len = "
            "checked_encoded_frame_len::<T, E>(msg, self.max_frame_bytes)?;",
            "let encoded_len = 0;",
            "counting sender preflight before material encoding",
        ),
        (
            "prepare_encoded_buffer",
            "let max_plaintext = frame_plaintext_cap_for::<E>(self.max_frame_bytes);",
            "let max_plaintext = usize::MAX;",
            "generic AEAD cap before sender batching",
        ),
        (
            "enqueue_encrypted",
            "if self.encrypted.len() != encrypted_size {",
            "if false && self.encrypted.len() != encrypted_size {",
            "post-encryption sender geometry agreement",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_runtime_frame_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "merge",
            "other.bytes = 0;",
            "let _released_on_drop = other.bytes;",
            "already-accounted source leases coalesce without release and reacquisition",
        ),
        (
            "credit_owner",
            "if required.len() > self.max_sources {",
            "if false && required.len() > self.max_sources {",
            "shared authenticated-source registry preserves identity, protected sources, and capacity",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_source_owner_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "try_reserve_for_source",
            "(Some(retained), Some(candidate)) => !retained.matches(candidate),",
            "(Some(_), Some(_)) => false,",
            "queued progress tickets must retain the exact weak delivery authority rather than reusing ordinal-equivalent tenure",
        ),
        (
            "try_reserve_for_source",
            "if source_retained.is_some_and(|retained| retained.items >= 1) {",
            "if source_retained.is_some_and(|retained| retained.items >= 2) {",
            "distinct broadcast or direct requests remain FIFO-ranked behind a target owner",
        ),
        (
            "submit_progress_message_to_source",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned\n"
            "            | ProgressLeaseAttempt::CancelledMembership => return Ok(None),",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned\n"
            "            | ProgressLeaseAttempt::CancelledMembership => "
            "return Ok(Some(NetworkActorAdmittedTicketIdentity::forged())),",
            (
                "same-request and cancelled admission return no new ticket identity, "
                "while invalid ownership cannot substitute for the original request"
            ),
        ),
        (
            "broadcast_recoverable",
            "&& Arc::ptr_eq(&ticket.topology, &self.reliable_broadcast_topology)",
            "&& true",
            "broadcast retry tickets bind digest, actor budget, and topology publication",
        ),
        (
            "broadcast_recoverable",
            "if !target.membership.is_active() {",
            "if false && !target.membership.is_active() {",
            "broadcast fanout admits each active topology authority through an isolated target source",
        ),
        (
            "progress_ticket_request_digest",
            "let metadata = [0_u8, priority_tag(post.priority)];",
            "let metadata = [1_u8, priority_tag(post.priority)];",
            "canonical progress digest keeps Post and Broadcast request identities disjoint",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_local_actor_split_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source(module, network_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


def test_transport_geometry_rejects_ordinal_equivalent_weak_authority_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source_in_context(
        module,
        network_path,
        "matches",
        (("impl", "WeakProgressDeliveryAuthority"),),
        "Arc::ptr_eq(&retained, &candidate.tenure)",
        "retained.connection_ordinal == candidate.tenure.connection_ordinal",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "weak progress authority matching must preserve exact Arc ownership"
        in error
        for error in errors
    ), errors
