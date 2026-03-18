from __future__ import annotations

import json
from pathlib import Path

import pytest

import scripts.ministry.export_red_team_evidence as exports


def _write_file(root: Path, name: str, content: str) -> Path:
    path = root / name
    path.write_text(content, encoding="utf-8")
    return path


def test_export_evidence_copies_inputs_and_writes_manifest(tmp_path: Path) -> None:
    source_dir = tmp_path / "sources"
    source_dir.mkdir()
    log_file = _write_file(source_dir, "logs.txt", "log")
    dash_file = _write_file(source_dir, "dash.json", '{"dashboard":true}')
    alert_file = _write_file(source_dir, "alerts.yaml", "alerts")
    manifest_file = _write_file(source_dir, "manifest.json", "{}")
    evidence_file = _write_file(source_dir, "evidence.txt", "payload")

    artifacts_root = tmp_path / "artifacts"
    result = exports.export_evidence(
        scenario_id="20261112-blindfold",
        logs=[str(log_file)],
        dashboards=[str(dash_file)],
        alerts=[str(alert_file)],
        manifests=[str(manifest_file)],
        evidence_files=[str(evidence_file)],
        artifacts_root=artifacts_root,
        metadata={"owner": "security"},
    )

    scenario_root = artifacts_root / "2026-11" / "blindfold"
    assert result.manifest_path == scenario_root / "evidence_manifest.json"
    for category in ("logs", "dashboards", "alerts", "manifests", "evidence"):
        category_dir = scenario_root / category
        assert category_dir.exists(), f"{category} directory missing"
        assert category_dir.is_dir()
    manifest = json.loads(result.manifest_path.read_text(encoding="utf-8"))
    assert manifest["scenario_id"] == "20261112-blindfold"
    assert manifest["metadata"] == {"owner": "security"}
    recorded_paths = {entry["path"] for entry in manifest["files"]}
    expected_relative = {
        f"logs/{log_file.name}",
        f"dashboards/{dash_file.name}",
        f"alerts/{alert_file.name}",
        f"manifests/{manifest_file.name}",
        f"evidence/{evidence_file.name}",
    }
    assert recorded_paths == expected_relative


def test_ingest_dashboard_spec_supports_grafana(tmp_path: Path) -> None:
    class FakeGrafana:
        def fetch_dashboard(self, uid: str) -> bytes:
            assert uid == "abc123"
            return b'{"dashboard": "payload"}'

    dest_dir = tmp_path / "dashboards"
    entry = exports._ingest_dashboard_spec(
        "grafana:abc123:my-dashboard",
        dest_dir=dest_dir,
        grafana=FakeGrafana(),
        overwrite=False,
    )
    assert entry.category == "dashboards"
    assert entry.source == "grafana:abc123:my-dashboard"
    assert entry.destination.exists()
    assert entry.destination.read_text(encoding="utf-8") == '{"dashboard": "payload"}'
    assert entry.destination.name == "my-dashboard.json"


def test_parse_metadata_rejects_invalid_entries() -> None:
    entries = ["owner=security", "tier =p1"]
    parsed = exports._parse_metadata(entries)
    assert parsed == {"owner": "security", "tier": "p1"}
    with pytest.raises(ValueError):
        exports._parse_metadata(["missing-separator"])


def test_main_invokes_export_evidence(monkeypatch, tmp_path: Path) -> None:
    captured: dict[str, object] = {}

    def fake_export(**kwargs):  # type: ignore[no-untyped-def]
        captured.update(kwargs)

    monkeypatch.setattr(exports, "export_evidence", fake_export)
    args = [
        "--scenario-id",
        "20261112-blindfold",
        "--log",
        str(tmp_path / "log.txt"),
        "--dashboard",
        str(tmp_path / "dash.json"),
        "--alert",
        str(tmp_path / "alert.json"),
        "--manifest",
        str(tmp_path / "manifest.json"),
        "--evidence",
        str(tmp_path / "evidence.bin"),
        "--metadata",
        "owner=security",
        "--summary",
        str(tmp_path / "summary.json"),
    ]
    rc = exports.main(args)
    assert rc == 0
    assert captured["scenario_id"] == "20261112-blindfold"
    assert captured["logs"] == [str(tmp_path / "log.txt")]
    assert captured["metadata"] == {"owner": "security"}


def test_main_requires_grafana_url_when_dashboard_specified() -> None:
    args = [
        "--scenario-id",
        "20261112-blindfold",
        "--dashboard",
        "grafana:abc123",
    ]
    with pytest.raises(SystemExit):
        exports.main(args)
