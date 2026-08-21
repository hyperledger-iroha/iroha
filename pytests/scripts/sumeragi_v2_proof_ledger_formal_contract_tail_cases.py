# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


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
