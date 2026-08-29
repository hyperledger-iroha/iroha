# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


def test_async_release_requires_checked_type_closure_and_step_refinement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    valid = r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext
THEOREM AsyncTypeInvariantObligation ==
  \A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant
BY PTL
THEOREM AsyncNextPreservesNormalProposalPrepareCandidate ==
  \A candidate:
    /\ NormalProposalPrepareCandidate(candidate)
    /\ AsyncNext
    => NormalProposalPrepareCandidate(candidate)'
BY PTL
=============================================================================
"""
    path.write_text(valid, encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    path.write_text(
        valid.replace(
            "\\A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant",
            "AsyncTypeInvariant => []AsyncTypeInvariant",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "AsyncTypeInvariantObligation must state only" in error for error in errors
    )


def test_production_trace_certificate_rejects_every_nested_field_hash_and_source_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )

    def leaf_paths(value, prefix=()):
        if isinstance(value, dict):
            for key in sorted(value):
                yield from leaf_paths(value[key], (*prefix, key))
        elif isinstance(value, list):
            for index, item in enumerate(value):
                yield from leaf_paths(item, (*prefix, index))
        else:
            yield prefix

    for path in leaf_paths(expected):
        observed = copy.deepcopy(expected)
        owner = observed
        for component in path[:-1]:
            owner = owner[component]
        original = owner[path[-1]]
        if isinstance(original, bool):
            replacement = not original
        elif isinstance(original, int):
            replacement = original + 1
        else:
            replacement = f"{original}-drift"
        owner[path[-1]] = replacement
        errors = module._production_trace_extraction_evidence_errors(
            {},
            observed,
            tlaps_evidence={},
            verus_evidence={},
            cross_tool_evidence={},
            artifacts=paths,
        )
        assert errors and "canonical current theorem certificate" in errors[0], path


def test_production_trace_certificate_rejects_missing_proof_linkage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_checker()
    expected = _synthetic_production_trace_certificate(module)
    artifact = tmp_path / "artifact"
    artifact.write_bytes(b"evidence\n")
    paths = _synthetic_trace_artifact_paths(module, artifact)
    monkeypatch.setattr(
        module,
        "build_production_trace_extraction_evidence",
        lambda *args, **kwargs: expected,
    )
    observed = copy.deepcopy(expected)
    del observed["proof_linkage"]["component_evidence"]

    errors = module._production_trace_extraction_evidence_errors(
        {},
        observed,
        tlaps_evidence={},
        verus_evidence={},
        cross_tool_evidence={},
        artifacts=paths,
    )

    assert errors == [
        "production trace-extraction evidence does not match the canonical "
        "current theorem certificate at $.proof_linkage"
    ]


@pytest.mark.parametrize(
    ("replacement", "expected_counts"),
    (
        ("let linked_before = match removed_statat(", "(0, 0, 0)"),
        (
            """if false {
                let linked_before = match rustix::fs::statat(
                    &self.directory.directory,
                    &self.entry_name,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                ) {
                    Ok(stat) => stat,
                    Err(_) => unreachable!(),
                };
                let _ = linked_before;
            }
            let linked_before = match rustix::fs::statat(""",
            "(0, 0, 2)",
        ),
    ),
)
def test_serviced_candidate_read_discriminator_fails_closed_without_crashing(
    tmp_path: Path,
    replacement: str,
    expected_counts: str,
) -> None:
    """Missing or duplicate bounded-read ownership returns a diagnostic."""

    module = load_checker()
    copy_serviced_candidate_production_fixture(tmp_path)
    assert module._serviced_candidate_production_source_fidelity_errors(tmp_path) == []
    safety_path = tmp_path / "crates/iroha_core/src/sumeragi/safety_wal.rs"
    mutate_source_once(
        safety_path,
        "let linked_before = match rustix::fs::statat(",
        replacement,
    )

    errors = module._serviced_candidate_production_source_fidelity_errors(tmp_path)
    assert any(
        "require exactly one parsed bounded adjacent read" in error
        and f"discriminator_counts={expected_counts}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("kind", "symbol", "old", "new"),
    (
        (
            "operator",
            "AsyncIngressSchedulerBarrierActive",
            "  \\/ AsyncOrdinaryIngressProtectedRecordsAt(node) # {}",
            "  \\/ FALSE",
        ),
        (
            "operator",
            "AsyncEarliestIngressSchedulerOrdinal",
            "       ELSE AsyncOrdinaryIngressEarliestPhysicalRecord(\n"
            "              node).schedulerOrdinal",
            "       ELSE AsyncLeaderWireEarliestPhysicalIngressRecord(\n"
            "              node).schedulerOrdinal",
        ),
        (
            "operator",
            "AsyncOlderRuntimeLifecyclePrecedesIngressScheduler",
            "  /\\ AsyncSelectedRuntimeSourcePhysicalOrdinal(node)\n"
            "       < AsyncEarliestIngressPhysicalOrdinal(node)",
            "  /\\ TRUE",
        ),
        (
            "operator",
            "AsyncOlderLocalLifecyclePrecedesServeIngress",
            "  /\\ LocalSourceLifecyclePhysicalOrdinal(\n"
            "       node, SelectedLocalSource(node))\n"
            "       < AsyncEarliestIngressPhysicalOrdinal(node)",
            "  /\\ TRUE",
        ),
        (
            "operator",
            "AsyncCandidateLifecycleStateAfterServeIngressAdmission",
            "     !.retransmitLifecycleOrdinal =",
            "     !.timeoutLifecycleOrdinal =",
        ),
        (
            "operator",
            "AsyncSharedSchedulerOrdinalInjectionInvariant",
            "  /\\ \\A admission \\in asyncServeIngressAdmissions:\n"
            "       AsyncRetransmitLifecycleOwned(admission.node)\n"
            "         => admission.schedulerOrdinal #\n"
            "              AsyncRetransmitLifecycleOrdinal(admission.node)\n",
            "",
        ),
        (
            "theorem",
            "SerializedLocalPrecedesServeIngressExactFrame",
            "         /\\ LocalSourceLifecyclePhysicalOrdinal(\n"
            "              node, SelectedLocalSource(node))\n"
            "              < AsyncEarliestIngressPhysicalOrdinal(node)",
            "         /\\ TRUE",
        ),
        (
            "theorem",
            "AsyncLaterServeTicketInterleavesOlderRuntimeEpisode",
            "    /\\ AsyncSelectedRuntimeSourcePhysicalOrdinal(node)\n"
            "         < AsyncEarliestIngressPhysicalOrdinal(node)",
            "    /\\ TRUE",
        ),
        (
            "theorem",
            "AsyncLaterServeTicketInterleavesOlderLocalEpisode",
            "    /\\ LocalSourceLifecyclePhysicalOrdinal(\n"
            "         node, SelectedLocalSource(node))\n"
            "         < AsyncEarliestIngressPhysicalOrdinal(node)",
            "    /\\ TRUE",
        ),
    ),
)
def test_serve_scheduler_ordinal_release_contract_rejects_current_weakening(
    tmp_path: Path,
    kind: str,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_serve_scheduler_ordinal_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    mutate = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutate(source, symbol, old, new), encoding="utf-8")
    module.SERVE_SCHEDULER_ORDINAL_RELEASE_SOURCE_SHA256[path.name] = (
        hashlib.sha256(path.read_bytes()).hexdigest()
    )

    errors = module._serve_scheduler_ordinal_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    prefix = "theorem " if kind == "theorem" else ""
    assert any(
        f"{prefix}{symbol} must equal only" in error for error in errors
    ), errors


def _assert_commit_import_release_or_stale_artifact(
    tmp_path: Path, artifact_name: str
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_commit_import_provenance_mutation_fixture(
        tmp_path, module
    )
    path = repo_root / artifact_name if "/" in artifact_name else formal_dir / artifact_name
    release_mutations = {
        "SumeragiV2AsyncNetwork.tla": (
            "DirectCommitQcCandidateHasExactImportLineage",
            "    /\\ item.envelope.qc.context = context\n",
            "    /\\ TRUE\n",
        ),
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs.tla": (
            "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry",
            "  IndexedChainSpec\n"
            "    => IndexedHistoricalCertificateLocalImportCandidateEntryProperty\n",
            "  IndexedChainSpec\n    => TRUE\n",
        ),
    }
    release_mutation = release_mutations.get(artifact_name)
    if release_mutation is None:
        path.write_text(
            path.read_text(encoding="utf-8") + "\n\\* stale import provenance\n",
            encoding="utf-8",
        )
    else:
        symbol, old, new = release_mutation
        source = path.read_text(encoding="utf-8")
        path.write_text(
            mutate_tla_theorem(source, symbol, old, new), encoding="utf-8"
        )
        module.COMMIT_IMPORT_PROVENANCE_RELEASE_SOURCE_SHA256[path.name] = (
            hashlib.sha256(path.read_bytes()).hexdigest()
        )

    errors = module._commit_import_provenance_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    if release_mutation is None:
        assert any(
            str(path) in error
            and (
                "must match exact reviewed SHA-256" in error
                or "must match frozen SHA-256" in error
            )
            for error in errors
        ), errors
    else:
        assert any(
            f"Commit-import release theorem {symbol} must state only" in error
            for error in errors
        ), errors
