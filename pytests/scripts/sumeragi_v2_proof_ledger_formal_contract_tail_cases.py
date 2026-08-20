# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


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
