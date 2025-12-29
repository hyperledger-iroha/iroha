"""Tests for scripts/check_android_fixtures.py."""

from __future__ import annotations

import base64
import importlib.util
import json
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "check_android_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_android_fixtures", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
SPEC.loader.exec_module(MODULE)


def _write_payloads(path: Path, entries: list[dict]) -> Path:
    path.write_text(json.dumps(entries, indent=2), encoding="utf-8")
    return path


def _write_manifest(path: Path, fixtures: list[dict]) -> Path:
    path.write_text(json.dumps({"fixtures": fixtures}, indent=2), encoding="utf-8")
    return path


def _fixture_entry(name: str, encoded_file: str, payload: bytes, signed: bytes) -> dict:
    payload_b64 = base64.b64encode(payload).decode()
    signed_b64 = base64.b64encode(signed).decode()
    return {
        "name": name,
        "encoded_file": encoded_file,
        "payload_base64": payload_b64,
        "payload_hash": MODULE.iroha_hash(payload),  # type: ignore[attr-defined]
        "encoded_len": len(payload),
        "signed_base64": signed_b64,
        "signed_hash": MODULE.iroha_hash(signed),  # type: ignore[attr-defined]
        "signed_len": len(signed),
    }


def test_summary_includes_artifact_metadata(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"alpha-payload"
    signed_bytes = b"alpha-signed"
    (resources / "alpha.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [{"name": "alpha", "encoded": base64.b64encode(payload_bytes).decode()}],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [_fixture_entry("alpha", "alpha.norito", payload_bytes, signed_bytes)],
    )
    summary_path = tmp_path / "summary.json"

    exit_code = MODULE.main(
        [
          "--resources",
          str(resources),
          "--fixtures",
          str(payloads_path),
          "--manifest",
          str(manifest_path),
          "--json-out",
          str(summary_path),
          "--quiet",
        ]
    )
    assert exit_code == 0

    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["result"]["status"] == "ok"
    artifacts = summary["artifacts"]
    assert artifacts["manifest"]["fixture_count"] == 1
    assert artifacts["manifest"]["sha256"] == MODULE.sha256_file(manifest_path)  # type: ignore[attr-defined]
    assert artifacts["payloads"]["entry_count"] == 1
    assert artifacts["payloads"]["sha256"] == MODULE.sha256_file(payloads_path)  # type: ignore[attr-defined]
    assert artifacts["encoded"]["file_count"] == 1
    assert artifacts["encoded"]["aggregate_sha256"] == MODULE.hash_encoded_directory(resources)  # type: ignore[attr-defined]


def test_errors_propagate_into_summary(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"bravo"
    signed_bytes = b"bravo-signed"
    (resources / "bravo.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [{"name": "bravo", "encoded": base64.b64encode(payload_bytes).decode()}],
    )
    bad_manifest = _fixture_entry("bravo", "bravo.norito", payload_bytes, signed_bytes)
    bad_manifest["payload_hash"] = "deadbeef"
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [bad_manifest],
    )
    summary_path = tmp_path / "summary.json"

    exit_code = MODULE.main(
        [
          "--resources",
          str(resources),
          "--fixtures",
          str(payloads_path),
          "--manifest",
          str(manifest_path),
          "--json-out",
          str(summary_path),
          "--quiet",
        ]
    )
    assert exit_code == 1
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["result"]["status"] == "error"
    assert summary["result"]["error_count"] >= 1
    assert summary["artifacts"]["manifest"]["fixture_count"] == 1
