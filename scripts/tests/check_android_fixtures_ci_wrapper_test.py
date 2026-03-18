"""End-to-end check for ci/check_android_fixtures.sh."""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
import os
import shutil
import subprocess
import uuid
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts/check_android_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_android_fixtures", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _write_fixture_set(base: Path) -> tuple[Path, Path, Path]:
    resources = base / "resources"
    resources.mkdir(parents=True, exist_ok=True)

    payload_bytes = b"alpha-fixture"
    signed_bytes = b"alpha-signed"
    (resources / "alpha.norito").write_bytes(payload_bytes)

    payloads_path = base / "transaction_payloads.json"
    creation_time_ms = 1_735_000_000_111
    chain = "00000002"
    authority = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
    time_to_live_ms = 5000
    nonce = 42
    payloads_path.write_text(
        json.dumps(
            [
                {
                    "name": "alpha",
                    "encoded": base64.b64encode(payload_bytes).decode(),
                    "creation_time_ms": creation_time_ms,
                    "chain": chain,
                    "authority": authority,
                    "time_to_live_ms": time_to_live_ms,
                    "nonce": nonce,
                }
            ],
            indent=2,
        ),
        encoding="utf-8",
    )

    manifest_path = base / "transaction_fixtures.manifest.json"
    manifest_path.write_text(
        json.dumps(
            {
                "fixtures": [
                    {
                        "name": "alpha",
                        "encoded_file": "alpha.norito",
                        "payload_base64": base64.b64encode(payload_bytes).decode(),
                        "payload_hash": MODULE.iroha_hash(payload_bytes),  # type: ignore[attr-defined]
                        "encoded_len": len(payload_bytes),
                        "signed_base64": base64.b64encode(signed_bytes).decode(),
                        "signed_hash": MODULE.iroha_hash(signed_bytes),  # type: ignore[attr-defined]
                        "signed_len": len(signed_bytes),
                        "creation_time_ms": creation_time_ms,
                        "chain": chain,
                        "authority": authority,
                        "time_to_live_ms": time_to_live_ms,
                        "nonce": nonce,
                    }
                ]
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    return resources, payloads_path, manifest_path


def test_ci_wrapper_emits_summary_with_custom_destination() -> None:
    scratch = REPO_ROOT / "target-codex" / f"android-parity-ci-{uuid.uuid4().hex}"
    scratch.mkdir(parents=True, exist_ok=True)
    resources, payloads_path, manifest_path = _write_fixture_set(scratch)
    summary_path = scratch / "summary.json"

    env = os.environ.copy()
    env.update(
        {
            "ANDROID_FIXTURE_RESOURCES": str(resources),
            "ANDROID_FIXTURE_PAYLOADS": str(payloads_path),
            "ANDROID_FIXTURE_MANIFEST": str(manifest_path),
            "ANDROID_PARITY_SUMMARY": str(summary_path),
        }
    )

    try:
        result = subprocess.run(
            ["bash", "ci/check_android_fixtures.sh"],
            cwd=REPO_ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 0, f"{result.stdout}\n{result.stderr}"
        assert summary_path.exists(), "parity summary was not written"

        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        assert summary["result"]["status"] == "ok"
        artifacts = summary["artifacts"]
        assert artifacts["payloads"]["path"] == str(payloads_path.resolve())
        assert artifacts["encoded"]["file_count"] == 1
        assert artifacts["manifest"]["fixture_count"] == 1
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
