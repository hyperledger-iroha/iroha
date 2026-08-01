"""Tests for scripts/check_android_fixtures.py."""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
from typing import Optional
from pathlib import Path

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "check_android_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_android_fixtures", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _field(value: bytes) -> bytes:
    return MODULE.compact_length(len(value)) + value


def _signed_transaction(payload: bytes, signature: bytes) -> bytes:
    return _field(signature) + _field(payload) + _field(b"\x00")


def _write_payloads(path: Path, entries: list[dict]) -> Path:
    enriched: list[dict] = []
    for original in entries:
        entry = dict(original)
        payload_base64 = entry.get("payload_base64")
        name = entry.get("name")
        if isinstance(payload_base64, str) and isinstance(name, str):
            payload_bytes = base64.b64decode(payload_base64, validate=True)
            signed_bytes = _signed_transaction(
                payload_bytes, f"{name}-signed".encode()
            )
            entry.setdefault("payload_hash", MODULE.iroha_hash(payload_bytes))
            entry.setdefault("signed_base64", base64.b64encode(signed_bytes).decode())
            entry.setdefault(
                "signed_hash",
                MODULE.signed_transaction_entrypoint_hash(signed_bytes),
            )
        entry.setdefault(
            "payload",
            {
                "authority": entry.get("authority"),
                "chain": entry.get("chain"),
                "creation_time_ms": entry.get("creation_time_ms"),
                "executable": {"Instructions": []},
                "fee_payment": {
                    "payer": "authority",
                    "value": {"charge_limits": []},
                },
                "metadata": {},
                "nonce": entry.get("nonce"),
                "time_to_live_ms": entry.get("time_to_live_ms"),
            },
        )
        enriched.append(entry)
    path.write_text(json.dumps(enriched, indent=2), encoding="utf-8")
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
    time_to_live_ms: int,
    nonce: Optional[int],
) -> dict:
    signed = _signed_transaction(payload, signed)
    payload_b64 = base64.b64encode(payload).decode()
    signed_b64 = base64.b64encode(signed).decode()
    return {
        "name": name,
        "encoded_file": encoded_file,
        "payload_base64": payload_b64,
        "payload_hash": MODULE.iroha_hash(payload),  # type: ignore[attr-defined]
        "encoded_len": len(payload),
        "signed_base64": signed_b64,
        "signed_hash": MODULE.signed_transaction_entrypoint_hash(signed),  # type: ignore[attr-defined]
        "signed_len": len(signed),
        "creation_time_ms": creation_time_ms,
        "chain": chain,
        "authority": authority,
        "time_to_live_ms": time_to_live_ms,
        "nonce": nonce,
    }


def _payload_entry(name: str = "alpha") -> dict:
    return {
        "name": name,
        "payload_base64": base64.b64encode(f"{name}-payload".encode()).decode(),
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "time_to_live_ms": 100_000,
        "nonce": None,
    }


def test_native_json_loader_rejects_duplicate_payload_and_manifest_keys(
    tmp_path: Path,
) -> None:
    payloads_path = tmp_path / "transaction_payloads.json"
    payloads_path.write_text(
        '[{"name":"first","na\\u006de":"second"}]', encoding="utf-8"
    )
    with pytest.raises(ValueError, match="duplicate JSON object key 'name'"):
        MODULE.load_payload_fixtures(payloads_path)

    manifest_path = tmp_path / "transaction_fixtures.manifest.json"
    manifest_path.write_text('{"fixtures":[],"fixtures":[]}', encoding="utf-8")
    with pytest.raises(ValueError, match="duplicate JSON object key 'fixtures'"):
        MODULE.load_manifest(manifest_path)


def test_payload_loader_requires_exact_top_level_and_payload_fields(
    tmp_path: Path,
) -> None:
    entry = _payload_entry()
    entry["unexpected"] = True
    path = _write_payloads(tmp_path / "extra-top-level.json", [entry])
    with pytest.raises(ValueError, match=r"unexpected=\['unexpected'\]"):
        MODULE.load_payload_fixtures(path)

    path = _write_payloads(
        tmp_path / "extra-payload.json", [_payload_entry("payload-extra")]
    )
    document = json.loads(path.read_text(encoding="utf-8"))
    document[0]["payload"]["unexpected"] = True
    path.write_text(json.dumps(document), encoding="utf-8")
    with pytest.raises(ValueError, match=r"unexpected=\['unexpected'\]"):
        MODULE.load_payload_fixtures(path)


def test_payload_loader_requires_one_executable_variant_and_accepts_direct_call(
    tmp_path: Path,
) -> None:
    direct_path = _write_payloads(
        tmp_path / "direct-call.json", [_payload_entry("direct_call")]
    )
    direct_document = json.loads(direct_path.read_text(encoding="utf-8"))
    direct_document[0]["payload"]["executable"] = {
        "ContractCall": {
            "contract_address": "tairac1example",
            "expected_code_hash": "hash:example",
            "entrypoint": "main",
            "arguments": [],
        }
    }
    direct_path.write_text(json.dumps(direct_document), encoding="utf-8")
    assert set(MODULE.load_payload_fixtures(direct_path)) == {"direct_call"}

    ambiguous_path = _write_payloads(
        tmp_path / "ambiguous.json", [_payload_entry("ambiguous")]
    )
    ambiguous_document = json.loads(ambiguous_path.read_text(encoding="utf-8"))
    ambiguous_document[0]["payload"]["executable"] = {
        "Instructions": [],
        "ContractCall": {},
    }
    ambiguous_path.write_text(json.dumps(ambiguous_document), encoding="utf-8")
    with pytest.raises(ValueError, match="exactly one executable variant"):
        MODULE.load_payload_fixtures(ambiguous_path)


def test_payload_loader_validates_exact_executable_variant_bodies() -> None:
    instruction = {
        "wire_name": "iroha.test",
        "payload_base64": "AQ==",
    }
    contract_call = {
        "contract_address": "tairac1example",
        "expected_code_hash": "hash:example",
        "entrypoint": "main",
        "arguments": [0, 255],
    }
    for executable in (
        {"Ivm": "AQ=="},
        {"Instructions": [instruction]},
        {"ContractCall": {**contract_call, "arguments": None}},
        {
            "Batch": [
                {"Instruction": instruction},
                {"ContractCall": contract_call},
            ]
        },
    ):
        MODULE.validate_executable(executable, "executable")

    invalid_executables = (
        ({"Ivm": 1}, r"Ivm must be a base64 string"),
        ({"Ivm": "YR=="}, r"non-canonical base64"),
        ({"Instructions": {}}, r"Instructions must be an array"),
        (
            {"Instructions": [{**instruction, "unexpected": True}]},
            r"unexpected=\['unexpected'\]",
        ),
        (
            {"Instructions": [{**instruction, "wire_name": ""}]},
            r"wire_name must be a non-empty string",
        ),
        (
            {"Instructions": [{**instruction, "payload_base64": ""}]},
            r"payload_base64 must encode non-empty bytes",
        ),
        (
            {"Instructions": [{**instruction, "payload_base64": "YR=="}]},
            r"non-canonical base64",
        ),
        (
            {"ContractCall": {**contract_call, "unexpected": True}},
            r"unexpected=\['unexpected'\]",
        ),
        (
            {
                "ContractCall": {
                    "contract_address": contract_call["contract_address"],
                    "entrypoint": contract_call["entrypoint"],
                    "arguments": None,
                }
            },
            r"missing=\['expected_code_hash'\]",
        ),
        (
            {"ContractCall": {**contract_call, "arguments": [256]}},
            r"arguments must be null or an array of bytes",
        ),
        ({"Batch": []}, r"Batch must contain at least one item"),
        ({"Batch": {}}, r"Batch must be an array"),
        (
            {
                "Batch": [
                    {
                        "Instruction": instruction,
                        "ContractCall": contract_call,
                    }
                ]
            },
            r"must contain exactly one variant",
        ),
        (
            {"Batch": [{"Instruction": {**instruction, "unexpected": True}}]},
            r"unexpected=\['unexpected'\]",
        ),
    )
    for executable, diagnostic in invalid_executables:
        with pytest.raises(ValueError, match=diagnostic):
            MODULE.validate_executable(executable, "executable")


def test_manifest_requires_exact_schema_and_canonical_encoded_file(
    tmp_path: Path,
) -> None:
    entry = _fixture_entry(
        "alpha",
        "alpha.norito",
        b"payload",
        b"signed",
        1,
        "00000002",
        "sorau-example",
        100_000,
        None,
    )

    assert MODULE.compare(tmp_path, {"fixtures": [], "unexpected": True}, {}) == [
        "manifest has invalid fields: missing=[], unexpected=['unexpected']"
    ]
    errors = MODULE.compare(
        tmp_path, {"fixtures": [{**entry, "unexpected": True}]}, {}
    )
    assert errors == [
        "manifest fixture has invalid fields: missing=[], unexpected=['unexpected']"
    ]

    renamed = {**entry, "encoded_file": "renamed.norito"}
    errors = MODULE.compare(tmp_path, {"fixtures": [renamed]}, {})
    assert any("encoded_file must be exactly 'alpha.norito'" in error for error in errors)

    traversing = {
        **entry,
        "name": "../alpha",
        "encoded_file": "../alpha.norito",
    }
    errors = MODULE.compare(tmp_path, {"fixtures": [traversing]}, {})
    assert any("must not traverse directories" in error for error in errors)


@pytest.mark.parametrize(
    "encoded",
    [
        "YQ!!",
        "Y Q==",
        "YQ=",
        "YQ===",
        "YR==",
    ],
    ids=["invalid-char", "whitespace", "missing-padding", "excess-padding", "noncanonical-bits"],
)
def test_decode_base64_rejects_noncanonical_encodings(encoded: str) -> None:
    with pytest.raises(ValueError, match="(?:invalid|non-canonical) base64"):
        MODULE.decode_base64(encoded, "adversarial fixture")


@pytest.mark.parametrize("nonce", [None, 1, 0xFFFF_FFFF])
def test_transaction_nonce_validator_accepts_nonzero_u32_range(
    nonce: Optional[int],
) -> None:
    assert MODULE.is_valid_transaction_nonce(nonce)


@pytest.mark.parametrize("ttl", [1, 100_000, 0xFFFF_FFFF_FFFF_FFFF])
def test_transaction_ttl_validator_accepts_positive_integers(ttl: int) -> None:
    assert MODULE.is_valid_transaction_ttl(ttl)


@pytest.mark.parametrize("nonce", [-1, 0, 0x1_0000_0000, True])
def test_payload_loader_rejects_nonce_outside_nonzero_u32_range(
    tmp_path: Path,
    nonce: object,
) -> None:
    entry = {
        "name": "invalid-nonce",
        "payload_base64": base64.b64encode(b"payload").decode(),
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "time_to_live_ms": 100_000,
        "nonce": nonce,
    }
    path = _write_payloads(tmp_path / "transaction_payloads.json", [entry])

    with pytest.raises(ValueError, match="invalid nonce"):
        MODULE.load_payload_fixtures(path)


@pytest.mark.parametrize(
    "ttl",
    [None, 0, -1, True, False, 1.5, "100000"],
    ids=["null", "zero", "negative", "true", "false", "float", "string"],
)
def test_payload_loader_rejects_non_positive_integer_ttl(
    tmp_path: Path, ttl: object
) -> None:
    entry = {
        "name": "invalid-ttl",
        "payload_base64": base64.b64encode(b"payload").decode(),
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "time_to_live_ms": ttl,
        "nonce": None,
    }
    path = _write_payloads(tmp_path / "transaction_payloads.json", [entry])

    with pytest.raises(ValueError, match="invalid time_to_live_ms"):
        MODULE.load_payload_fixtures(path)


def test_payload_loader_rejects_missing_ttl(tmp_path: Path) -> None:
    entry = {
        "name": "missing-ttl",
        "payload_base64": base64.b64encode(b"payload").decode(),
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "nonce": None,
    }
    path = _write_payloads(tmp_path / "transaction_payloads.json", [entry])

    with pytest.raises(ValueError, match="missing time_to_live_ms field"):
        MODULE.load_payload_fixtures(path)


def test_payload_loader_rejects_retired_encoded_alias(tmp_path: Path) -> None:
    payload_base64 = base64.b64encode(b"payload").decode()
    entry = {
        "name": "retired-alias",
        "payload_base64": payload_base64,
        "encoded": payload_base64,
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "time_to_live_ms": 100_000,
        "nonce": None,
    }
    path = _write_payloads(tmp_path / "transaction_payloads.json", [entry])

    with pytest.raises(ValueError, match="retired encoded alias"):
        MODULE.load_payload_fixtures(path)


def test_payload_loader_rejects_duplicate_fixture_names(tmp_path: Path) -> None:
    entry = {
        "name": "duplicate",
        "payload_base64": base64.b64encode(b"payload").decode(),
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "time_to_live_ms": 100_000,
        "nonce": None,
    }
    path = _write_payloads(tmp_path / "transaction_payloads.json", [entry, entry])

    with pytest.raises(ValueError, match="duplicate fixture name 'duplicate'"):
        MODULE.load_payload_fixtures(path)


def test_payload_loader_rejects_renamed_cloned_payloads(tmp_path: Path) -> None:
    first = {
        "name": "first",
        "payload_base64": base64.b64encode(b"payload").decode(),
        "chain": "00000002",
        "authority": "sorau-example",
        "creation_time_ms": 1,
        "time_to_live_ms": 100_000,
        "nonce": None,
    }
    second = {**first, "name": "renamed-clone"}
    path = _write_payloads(tmp_path / "transaction_payloads.json", [first, second])

    with pytest.raises(ValueError, match="duplicate fixture payload bytes for 'renamed-clone'"):
        MODULE.load_payload_fixtures(path)


def test_manifest_checker_rejects_duplicate_names_and_files(tmp_path: Path) -> None:
    entry = _fixture_entry(
        "duplicate",
        "duplicate.norito",
        b"payload",
        b"signed",
        1,
        "00000002",
        "sorau-example",
        100_000,
        None,
    )

    errors = MODULE.compare(tmp_path, {"fixtures": [entry, entry]}, {})

    assert "manifest contains duplicate fixture name: duplicate" in errors
    assert "manifest contains duplicate encoded_file: duplicate.norito" in errors


def test_manifest_checker_rejects_renamed_cloned_payloads(tmp_path: Path) -> None:
    first = _fixture_entry(
        "first",
        "first.norito",
        b"payload",
        b"signed",
        1,
        "00000002",
        "sorau-example",
        100_000,
        None,
    )
    clone = {**first, "name": "renamed-clone", "encoded_file": "renamed-clone.norito"}

    errors = MODULE.compare(tmp_path, {"fixtures": [first, clone]}, {})

    assert f"manifest contains duplicate payload_hash: {first['payload_hash']}" in errors
    assert "manifest contains duplicate payload bytes: renamed-clone" in errors
    assert f"manifest contains duplicate signed_hash: {first['signed_hash']}" in errors
    assert "manifest contains duplicate signed bytes: renamed-clone" in errors


@pytest.mark.parametrize("nonce", [-1, 0, 0x1_0000_0000, True])
def test_manifest_checker_rejects_nonce_outside_nonzero_u32_range(
    tmp_path: Path,
    nonce: object,
) -> None:
    entry = _fixture_entry(
        "invalid-nonce",
        "invalid-nonce.norito",
        b"payload",
        b"signed",
        1,
        "00000002",
        "sorau-example",
        100_000,
        nonce,  # type: ignore[arg-type]
    )

    errors = MODULE.compare(tmp_path, {"fixtures": [entry]}, {})

    assert any("manifest fixture has invalid nonce" in error for error in errors)


@pytest.mark.parametrize(
    "ttl",
    [None, 0, -1, True, False, 1.5, "100000"],
    ids=["null", "zero", "negative", "true", "false", "float", "string"],
)
def test_manifest_checker_rejects_non_positive_integer_ttl(
    tmp_path: Path, ttl: object
) -> None:
    entry = _fixture_entry(
        "invalid-ttl",
        "invalid-ttl.norito",
        b"payload",
        b"signed",
        1,
        "00000002",
        "sorau-example",
        100_000,
        None,
    )
    entry["time_to_live_ms"] = ttl

    errors = MODULE.compare(tmp_path, {"fixtures": [entry]}, {})

    assert any(
        "manifest fixture has invalid time_to_live_ms" in error for error in errors
    )


def test_manifest_checker_rejects_missing_ttl(tmp_path: Path) -> None:
    entry = _fixture_entry(
        "missing-ttl",
        "missing-ttl.norito",
        b"payload",
        b"signed",
        1,
        "00000002",
        "sorau-example",
        100_000,
        None,
    )
    entry.pop("time_to_live_ms")

    errors = MODULE.compare(tmp_path, {"fixtures": [entry]}, {})

    assert any("manifest fixture missing time_to_live_ms field" in error for error in errors)


def test_summary_includes_artifact_metadata(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"alpha-payload"
    signed_bytes = b"alpha-signed"
    (resources / "alpha.norito").write_bytes(payload_bytes)

    creation_time_ms = 1_735_000_000_123
    chain = "00000002"
    authority = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    time_to_live_ms = 5000
    nonce = 42
    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "alpha",
                "payload_base64": base64.b64encode(payload_bytes).decode(),
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


def test_signed_hash_uses_compact_external_entrypoint_domain() -> None:
    payload = b"x" * 128
    signed = _signed_transaction(payload, b"first-signature")
    differently_signed = _signed_transaction(payload, b"second-signature")
    expected_prefix = b"\x00\x00\x00\x00\x80\x01"
    assert MODULE.compact_length(len(payload)) == b"\x80\x01"  # type: ignore[attr-defined]
    assert MODULE.signed_transaction_entrypoint_hash(signed) == MODULE.iroha_hash(  # type: ignore[attr-defined]
        expected_prefix + payload
    )
    assert (
        MODULE.signed_transaction_entrypoint_hash(signed)
        == MODULE.signed_transaction_entrypoint_hash(differently_signed)
    )
    assert MODULE.signed_transaction_entrypoint_hash(signed) != MODULE.iroha_hash(signed)  # type: ignore[attr-defined]
    with pytest.raises(ValueError, match="trailing or legacy"):
        MODULE.signed_transaction_entrypoint_hash(signed + _field(b"\x00"))


def test_errors_propagate_into_summary(tmp_path: Path) -> None:
    resources = tmp_path / "resources"
    resources.mkdir()

    payload_bytes = b"bravo"
    signed_bytes = b"bravo-signed"
    (resources / "bravo.norito").write_bytes(payload_bytes)

    creation_time_ms = 1_735_000_000_222
    chain = "00000003"
    authority = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"
    time_to_live_ms = 100_000
    nonce = None
    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "bravo",
                "payload_base64": base64.b64encode(payload_bytes).decode(),
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
    authority = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
    payloads_path = _write_payloads(
        tmp_path / "transaction_payloads.json",
        [
            {
                "name": "charlie",
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_333,
                "chain": chain,
                "authority": authority,
                "time_to_live_ms": 100_000,
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
                time_to_live_ms=100_000,
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
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_444,
                "chain": "00000004",
                "authority": "sorauﾛ1PyXﾉspjg6gnvｴ1ﾒﾑLﾈｵBﾄEwtﾃD8Rｸﾇgｦﾎｾﾚｶ7ｴvWUJA5A",
                "time_to_live_ms": 100_000,
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
                authority="sorauﾛ1PyXﾉspjg6gnvｴ1ﾒﾑLﾈｵBﾄEwtﾃD8Rｸﾇgｦﾎｾﾚｶ7ｴvWUJA5A",
                time_to_live_ms=100_000,
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
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_777,
                "chain": "00000008",
                "authority": "sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6",
                "time_to_live_ms": 100_000,
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
                authority="sorauﾛ1NcﾐuﾛﾀKﾓhﾈgｽXｦDTﾏｴtﾔﾐ8PJPfSﾕPuﾃ884ｳﾇヰ4ﾇJKTL36",
                time_to_live_ms=100_000,
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
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_888,
                "chain": "00000009",
                "authority": "sorauﾛ1NcﾐuﾛﾀKﾓhﾈgｽXｦDTﾏｴtﾔﾐ8PJPfSﾕPuﾃ884ｳﾇヰ4ﾇJKTL36",
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
                authority="sorauﾛ1NcﾐuﾛﾀKﾓhﾈgｽXｦDTﾏｴtﾔﾐ8PJPfSﾕPuﾃ884ｳﾇヰ4ﾇJKTL36",
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
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_999,
                "chain": "00000010",
                "authority": "sorauﾛ1NfｺｷﾘcﾙｦEﾑgsKti4Zﾘ6HKｳZCﾅｸｼ16fvSｲymｶｻﾘﾎ29JNWE",
                "time_to_live_ms": 100_000,
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
                authority="sorauﾛ1NfｺｷﾘcﾙｦEﾑgsKti4Zﾘ6HKｳZCﾅｸｼ16fvSｲymｶｻﾘﾎ29JNWE",
                time_to_live_ms=100_000,
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
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_555,
                "chain": "00000006",
                "authority": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                "time_to_live_ms": 100_000,
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
                authority="sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                time_to_live_ms=100_000,
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
                "payload_base64": base64.b64encode(payload_bytes).decode(),
                "creation_time_ms": 1_735_000_000_666,
                "chain": "00000007",
                "authority": "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
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
                authority="sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
                time_to_live_ms=100_000,
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
