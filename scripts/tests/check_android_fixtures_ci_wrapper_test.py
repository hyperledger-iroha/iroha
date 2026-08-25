"""End-to-end check for ci/check_android_fixtures.sh."""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
import os
import shutil
import struct
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

TEST_NETWORK_ID = (
    "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"
)


def _field(value: bytes) -> bytes:
    return MODULE.compact_length(len(value)) + value


def _signed_transaction(payload: bytes, signature: bytes) -> bytes:
    return _field(signature) + _field(payload) + _field(b"\x00")


def _transaction_payload(suffix: bytes = b"") -> bytes:
    identity = bytes.fromhex(TEST_NETWORK_ID)
    domain = (0).to_bytes(4, "little") + _field(identity)
    return _field(domain) + suffix


def _crc64_xz(data: bytes) -> int:
    crc = 0xFFFF_FFFF_FFFF_FFFF
    for value in data:
        crc ^= value
        for _ in range(8):
            crc = (
                (crc >> 1) ^ 0xC96C_5795_D787_0F42 if crc & 1 else crc >> 1
            )
    return crc ^ 0xFFFF_FFFF_FFFF_FFFF


def _canonical_frame(payload: bytes, schema: bytes) -> bytes:
    return b"".join(
        (
            b"NRT0\x00\x00",
            schema,
            b"\x00",
            struct.pack("<Q", len(payload)),
            struct.pack("<Q", _crc64_xz(payload)),
            b"\x02",
            payload,
        )
    )


def _write_fixture_set(base: Path) -> tuple[Path, Path, Path]:
    resources = base / "resources"
    resources.mkdir(parents=True, exist_ok=True)

    payload_bare = _transaction_payload(b"alpha-fixture")
    signed_bare = _signed_transaction(payload_bare, b"alpha-signed")
    payload_bytes = _canonical_frame(payload_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    signed_bytes = _canonical_frame(signed_bare, MODULE.SIGNED_TRANSACTION_SCHEMA)
    (resources / "alpha.norito").write_bytes(payload_bytes)

    payloads_path = base / "transaction_payloads.json"
    creation_time_ms = 1_735_000_000_111
    network_id = TEST_NETWORK_ID
    authority = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    time_to_live_ms = 5000
    nonce = 42
    payloads_path.write_text(
        json.dumps(
            [
                {
                    "name": "alpha",
                    "payload_base64": base64.b64encode(payload_bytes).decode(),
                    "payload_hash": MODULE.iroha_hash(payload_bytes),
                    "signed_base64": base64.b64encode(signed_bytes).decode(),
                    "signed_hash": MODULE.signed_transaction_entrypoint_hash(
                        signed_bare
                    ),
                    "creation_time_ms": creation_time_ms,
                    "network_id": network_id,
                    "authority": authority,
                    "time_to_live_ms": time_to_live_ms,
                    "nonce": nonce,
                    "payload": {
                        "authority": authority,
                        "network_id": network_id,
                        "creation_time_ms": creation_time_ms,
                        "admission_intent": {
                            "intent": "ordinary",
                            "value": None,
                        },
                        "executable": {"Instructions": []},
                        "fee_payment": {
                            "payer": "authority",
                            "value": {"charge_limits": [], "gas_limit": None},
                        },
                        "metadata": {},
                        "nonce": nonce,
                        "time_to_live_ms": time_to_live_ms,
                    },
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
                        "signed_hash": MODULE.signed_transaction_entrypoint_hash(signed_bare),  # type: ignore[attr-defined]
                        "signed_len": len(signed_bytes),
                        "creation_time_ms": creation_time_ms,
                        "network_id": network_id,
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
