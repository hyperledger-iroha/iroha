"""Tests for scripts/check_android_fixtures.py."""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
from typing import Optional
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "check_android_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_android_fixtures", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _write_payloads(path: Path, entries: list[dict]) -> Path:
    path.write_text(json.dumps(entries, indent=2), encoding="utf-8")
    return path


def _write_manifest(path: Path, fixtures: list[dict]) -> Path:
    path.write_text(json.dumps({"fixtures": fixtures}, indent=2), encoding="utf-8")
    return path


def _fixture_entry(
    name: str,
    encoded_file: str,
    payload: bytes,
    signed: bytes,
    creation_time_ms: int,
    chain: str,
    authority: str,
    time_to_live_ms: Optional[int],
    nonce: Optional[int],
) -> dict:
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
        "creation_time_ms": creation_time_ms,
        "chain": chain,
        "authority": authority,
        "time_to_live_ms": time_to_live_ms,
        "nonce": nonce,
    }


def test_summary_includes_artifact_metadata(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"alpha-payload"
    signed_bytes = b"alpha-signed"
    (resources / "alpha.norito").write_bytes(payload_bytes)

    creation_time_ms = 1_735_000_000_123
    chain = "00000002"
    authority = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
    time_to_live_ms = 5000
    nonce = 42
    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
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
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "alpha",
                "alpha.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms,
                chain=chain,
                authority=authority,
                time_to_live_ms=time_to_live_ms,
                nonce=nonce,
            )
        ],
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

    creation_time_ms = 1_735_000_000_222
    chain = "00000003"
    authority = "6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"
    time_to_live_ms = None
    nonce = None
    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "bravo",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": creation_time_ms,
                "chain": chain,
                "authority": authority,
                "time_to_live_ms": time_to_live_ms,
                "nonce": nonce,
            }
        ],
    )
    bad_manifest = _fixture_entry(
        "bravo",
        "bravo.norito",
        payload_bytes,
        signed_bytes,
        creation_time_ms,
        chain=chain,
        authority=authority,
        time_to_live_ms=time_to_live_ms,
        nonce=nonce,
    )
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


def test_creation_time_mismatch_triggers_error(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"charlie-payload"
    signed_bytes = b"charlie-signed"
    (resources / "charlie.norito").write_bytes(payload_bytes)

    chain = "00000004"
    authority = "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp"
    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "charlie",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_333,
                "chain": chain,
                "authority": authority,
                "time_to_live_ms": None,
                "nonce": None,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "charlie",
                "charlie.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_999,
                chain=chain,
                authority=authority,
                time_to_live_ms=None,
                nonce=None,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_chain_mismatch_triggers_error(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"delta-payload"
    signed_bytes = b"delta-signed"
    (resources / "delta.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "delta",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_444,
                "chain": "00000004",
                "authority": "6cmzPVPX8DcdUnE1nGLZBU1opw24wjxczQNqhCCYvMzKfJR2rGs9tan",
                "time_to_live_ms": None,
                "nonce": None,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "delta",
                "delta.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_444,
                chain="00000005",
                authority="6cmzPVPX8DcdUnE1nGLZBU1opw24wjxczQNqhCCYvMzKfJR2rGs9tan",
                time_to_live_ms=None,
                nonce=None,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_authority_mismatch_triggers_error(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"golf-payload"
    signed_bytes = b"golf-signed"
    (resources / "golf.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "golf",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_777,
                "chain": "00000008",
                "authority": "6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL",
                "time_to_live_ms": None,
                "nonce": None,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "golf",
                "golf.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_777,
                chain="00000008",
                authority="6cmzPVPX4QdPT36dHgSFoznxS3MV99eV8CzeuZFTeqqsBgXDUYfft81",
                time_to_live_ms=None,
                nonce=None,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_time_to_live_mismatch_triggers_error(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"hotel-payload"
    signed_bytes = b"hotel-signed"
    (resources / "hotel.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "hotel",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_888,
                "chain": "00000009",
                "authority": "6cmzPVPX4QdPT36dHgSFoznxS3MV99eV8CzeuZFTeqqsBgXDUYfft81",
                "time_to_live_ms": 5000,
                "nonce": 7,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "hotel",
                "hotel.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_888,
                chain="00000009",
                authority="6cmzPVPX4QdPT36dHgSFoznxS3MV99eV8CzeuZFTeqqsBgXDUYfft81",
                time_to_live_ms=6000,
                nonce=7,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_nonce_mismatch_triggers_error(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"india-payload"
    signed_bytes = b"india-signed"
    (resources / "india.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "india",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_999,
                "chain": "00000010",
                "authority": "6cmzPVPX4Vnjpp7MFrUdgoZ9scoVXwFPcp4U6r6yELFetMDx2taw8et",
                "time_to_live_ms": None,
                "nonce": 9,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "india",
                "india.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_999,
                chain="00000010",
                authority="6cmzPVPX4Vnjpp7MFrUdgoZ9scoVXwFPcp4U6r6yELFetMDx2taw8et",
                time_to_live_ms=None,
                nonce=11,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_missing_nonce_field_fails(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"echo-payload"
    signed_bytes = b"echo-signed"
    (resources / "echo.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "echo",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_555,
                "chain": "00000006",
                "authority": "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
                "time_to_live_ms": None,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "echo",
                "echo.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_555,
                chain="00000006",
                authority="6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
                time_to_live_ms=None,
                nonce=None,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1


def test_missing_time_to_live_field_fails(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"foxtrot-payload"
    signed_bytes = b"foxtrot-signed"
    (resources / "foxtrot.norito").write_bytes(payload_bytes)

    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "foxtrot",
                "encoded": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_666,
                "chain": "00000007",
                "authority": "6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
                "nonce": None,
            }
        ],
    )
    manifest_path = _write_manifest(
        tmp_path / "transaction_fixtures.manifest.json",
        [
            _fixture_entry(
                "foxtrot",
                "foxtrot.norito",
                payload_bytes,
                signed_bytes,
                creation_time_ms=1_735_000_000_666,
                chain="00000007",
                authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
                time_to_live_ms=None,
                nonce=None,
            )
        ],
    )

    exit_code = MODULE.main(
        [
            "--resources",
            str(resources),
            "--fixtures",
            str(payloads_path),
            "--manifest",
            str(manifest_path),
            "--quiet",
        ]
    )
    assert exit_code == 1
