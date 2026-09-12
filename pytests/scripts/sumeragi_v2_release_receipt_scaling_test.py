"""Check the complete synthetic scaling fixture and its diagnostic failures."""
import json
from pathlib import Path

import pytest

from pytests.scripts import sumeragi_v2_release_receipt_scaling as scaling


def test_receipt_scaling_fixture_records_complete_scheduled_cohorts(tmp_path: Path) -> None:
    """Every accepted request has exact finality and every other offer is rejected."""
    evidence = scaling.make_scaling_evidence(
        tmp_path, head="1" * 40, sealed_manifest="2" * 64,
    )
    report = json.loads(evidence["scaling_report"].read_bytes())
    assert report["result"] == "pass"
    assert report["errors"] == []
    metrics = report["metrics"]
    assert metrics["four_to_one_median_throughput_ratio"] == 1.6
    assert metrics["four_to_one_p95_latency_ratio"] == 1.2
    manifest = json.loads(evidence["scaling_manifest"].read_bytes())
    assert len(manifest["runs"]) == 10
    assert manifest["workload"]["drain_seconds"] == 2.0
    assert manifest["workload"]["max_submission_lag_ms"] == 10.0
    for entry in manifest["runs"]:
        raw = json.loads((evidence["scaling_root"] / entry["raw_samples"]["path"]).read_bytes())
        trace = json.loads((evidence["scaling_root"] / raw["artifacts"]["transaction_trace"]["path"]).read_bytes())
        assert len(trace["transactions"]) == 500
        assert raw["warmup"] == {
            "offered_count": 100, "accepted_count": 100, "committed_count": 100,
        }
        assert raw["summary"]["offered_count"] == 400
        expected_acceptances = 100 if entry["variant"] == "one_lane" else 160
        assert raw["summary"]["accepted_count"] == expected_acceptances
        assert raw["summary"]["committed_count"] == expected_acceptances
        for row in trace["transactions"]:
            acknowledgment = row["acknowledgment"]
            assert acknowledgment["hash"] == row["hash"]
            if acknowledgment["status"] == "Accepted":
                assert acknowledgment["rejection"] is None
                assert row["applied"]["hash"] == row["hash"]
                assert row["applied"]["status"] == "Applied"
                assert row["applied"]["scope"] == "global"
                assert row["applied"]["resolved_from"] == "state"
            else:
                assert acknowledgment["status"] == "Rejected"
                assert acknowledgment["rejection"]
                assert row["applied"] is None


def test_receipt_scaling_failure_reports_actual_structured_validator_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Quiet mode cannot hide schema drift behind an empty assertion message."""
    original_workload = scaling.fixed_workload

    def missing_drain_workload() -> dict[str, float | int]:
        workload = original_workload()
        del workload["drain_seconds"]
        return workload

    monkeypatch.setattr(scaling, "fixed_workload", missing_drain_workload)
    with pytest.raises(AssertionError, match="missing=\\['drain_seconds'\\]") as error:
        scaling.make_scaling_evidence(tmp_path, head="1" * 40, sealed_manifest="2" * 64)
    report = json.loads((tmp_path / "scaling" / "validation_report.json").read_bytes())
    assert report["result"] == "fail"
    assert report["metrics"] is None
    assert report["errors"] == [
        "evidence manifest.workload fields differ from schema; "
        "missing=['drain_seconds'], extra=[]"
    ]
    assert report["errors"][0] in str(error.value)
    assert "stdout:" in str(error.value)
    assert "stderr:" in str(error.value)


def test_fixed_workload_is_not_shared_between_fixture_contexts() -> None:
    """A deliberate negative-fixture mutation cannot contaminate another run."""
    first = scaling.fixed_workload()
    second = scaling.fixed_workload()
    del first["drain_seconds"]
    assert second["drain_seconds"] == 2.0
    assert scaling.fixed_workload() == second
