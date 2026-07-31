# Live-evidence and deployment-context cases for the AI prescreen gate.

def test_shape_only_runner_evidence_without_live_probes_fails_closed(
    tmp_path: Path,
) -> None:
    payload = runner()
    for field in (
        "probe_count",
        "passed_probe_count",
        "probes",
        "runner_status",
        "screening_result",
    ):
        del payload[field]
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1

    artifact = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]
    assert artifact["valid"] is False
    assert "probes must be a non-empty array" in artifact["errors"]
    assert "runner_status must be an object" in artifact["errors"]
    assert "screening_result must be an object" in artifact["errors"]


@pytest.mark.parametrize(("kind", "factory"), (("runner", runner), ("committee", committee)))
def test_synthetic_runner_or_committee_evidence_is_never_production_evidence(
    kind: str,
    factory,
    tmp_path: Path,
) -> None:
    payload = factory()
    payload["synthetic"] = True
    write_json(tmp_path / f"{kind}.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        kind,
        "--summary-out",
        str(summary),
    ) == 1

    artifact = json.loads(summary.read_text("utf-8"))["required"][kind][
        "artifacts"
    ][0]
    assert "synthetic must be false" in artifact["errors"]


def test_runner_live_probe_inventory_rejects_missing_and_duplicate_probes(
    tmp_path: Path,
) -> None:
    missing_dir = tmp_path / "missing"
    missing_dir.mkdir()
    missing = runner()
    missing["probes"].pop()
    write_json(missing_dir / "runner.json", missing)
    missing_summary = missing_dir / "summary.json"

    assert run_gate(
        missing_dir,
        "--require-kind",
        "runner",
        "--summary-out",
        str(missing_summary),
    ) == 1
    missing_errors = json.loads(missing_summary.read_text("utf-8"))["required"][
        "runner"
    ]["artifacts"][0]["errors"]
    assert "probe_count must equal probes length" in missing_errors
    assert "probes must include name `screen`" in missing_errors

    duplicate_dir = tmp_path / "duplicate"
    duplicate_dir.mkdir()
    duplicate = runner()
    duplicate["probes"].append(dict(duplicate["probes"][0]))
    duplicate["probe_count"] = len(duplicate["probes"])
    duplicate["passed_probe_count"] = len(duplicate["probes"])
    write_json(duplicate_dir / "runner.json", duplicate)
    duplicate_summary = duplicate_dir / "summary.json"

    assert run_gate(
        duplicate_dir,
        "--require-kind",
        "runner",
        "--summary-out",
        str(duplicate_summary),
    ) == 1
    duplicate_errors = json.loads(duplicate_summary.read_text("utf-8"))[
        "required"
    ]["runner"]["artifacts"][0]["errors"]
    assert "probes must not contain duplicate values" in duplicate_errors
    assert "probes must not contain duplicate fingerprints" in duplicate_errors


def test_runner_timestamps_are_probe_completion_bound(tmp_path: Path) -> None:
    payload = runner()
    payload["checked_at_unix"] = payload["generated_at_unix"] - 1
    payload["screened_at_unix"] = payload["generated_at_unix"] + 1
    payload["screening_result"]["screened_at_unix"] = payload["screened_at_unix"]
    write_json(tmp_path / "runner.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-kind",
        "runner",
        "--summary-out",
        str(summary),
    ) == 1
    errors = json.loads(summary.read_text("utf-8"))["required"]["runner"][
        "artifacts"
    ][0]["errors"]
    assert "checked_at_unix must equal generated_at_unix" in errors
    assert "screened_at_unix must not be after checked_at_unix" in errors


def test_rollout_artifacts_must_share_one_reviewed_deployment_context(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    drifted = committee()
    drifted["deployment_id"] = "ai-prescreen-staging-b"
    write_json(tmp_path / "committee.json", drifted)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    report = json.loads(summary.read_text("utf-8"))
    assert report["deployment_context"] == {}
    assert (
        "valid_deployment_contexts must contain exactly one active binding"
        in report["errors"]
    )
