"""Tests for scripts/ci/check_soracloud_observability_evidence.py."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "ci"
    / "check_soracloud_observability_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_soracloud_observability_evidence", MODULE_PATH
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
SPEC.loader.exec_module(MODULE)


def evidence_payload() -> dict:
    def records(names: tuple[str, ...]) -> dict:
        return {
            name: {"present": True, "source": f"source:{name}"}
            for name in names
        }

    return {
        "generated_at": "2026-04-25T00:00:00Z",
        "deployment": {
            "name": "soracloud-production",
            "environment": "production",
            "operator": "ops",
        },
        "metrics": records(MODULE.REQUIRED_METRICS),
        "status_fields": records(MODULE.REQUIRED_STATUS_FIELDS),
        "alerts": {
            name: {
                "enabled": True,
                "severity": "critical",
                "runbook": "docs/source/soracloud/vue3_spa_api_runbook.md",
            }
            for name in MODULE.REQUIRED_ALERTS
        },
        "dashboards": [
            {
                "name": "Soracloud production posture",
                "url": "https://observability.example.invalid/d/soracloud",
            }
        ],
    }


def test_valid_evidence_passes(tmp_path: Path) -> None:
    evidence_path = tmp_path / "evidence.json"
    evidence_path.write_text(json.dumps(evidence_payload()), encoding="utf-8")

    assert MODULE.main(["--evidence", str(evidence_path)]) == 0


def test_missing_required_metric_fails(tmp_path: Path) -> None:
    payload = evidence_payload()
    del payload["metrics"]["hf_fallback_use"]
    evidence_path = tmp_path / "evidence.json"
    evidence_path.write_text(json.dumps(payload), encoding="utf-8")

    assert MODULE.main(["--evidence", str(evidence_path)]) == 1


def test_disabled_alert_fails(tmp_path: Path) -> None:
    payload = evidence_payload()
    payload["alerts"]["model_host_stale_heartbeats"]["enabled"] = False
    evidence_path = tmp_path / "evidence.json"
    evidence_path.write_text(json.dumps(payload), encoding="utf-8")

    assert MODULE.main(["--evidence", str(evidence_path)]) == 1
