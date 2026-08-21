# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

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
