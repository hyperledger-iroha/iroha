"""Tests for the strict Swift fixture parity policy."""

from __future__ import annotations

import base64
import copy
import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_swift_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_swift_fixtures", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write(root: Path, relative: str, contents: str | bytes) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(contents, bytes):
        path.write_bytes(contents)
    else:
        path.write_text(contents, encoding="utf-8")


def dump(root: Path, relative: str, value: object) -> None:
    write(root, relative, json.dumps(value, indent=2, sort_keys=True) + "\n")


def signed_envelope(name: str, payload: bytes) -> bytes:
    signature = name.encode("utf-8")
    return (
        MODULE.compact_length(len(signature))
        + signature
        + MODULE.compact_length(len(payload))
        + payload
        + b"\x00"
    )


def payload_body(name: str, *, shared: bool) -> dict:
    kind, action = (
        ("Test", "Test")
        if shared
        else MODULE.SWIFT_INSTRUCTION_SEMANTICS[name]
    )
    instruction = (
        {"payload_base64": "AA==", "wire_name": "iroha.test"}
        if shared
        else {
            "arguments": {
                "action": action,
                "asset_definition_id": "asset",
                "destination": "destination",
                "quantity": "1",
            },
            "kind": kind,
        }
    )
    return {
        "authority": f"authority-{name}",
        "chain": "00000001",
        "creation_time_ms": 1,
        "executable": {"Instructions": [instruction]},
        "fee_payment": {"payer": "authority", "value": {"charge_limits": []}},
        "metadata": {},
        "nonce": 1,
        "time_to_live_ms": 1000,
    }


def manifest_entry(name: str, body: dict, payload: bytes) -> dict:
    signed = signed_envelope(name, payload)
    return {
        "authority": body["authority"],
        "chain": body["chain"],
        "creation_time_ms": body["creation_time_ms"],
        "encoded_file": f"{name}.norito",
        "encoded_len": len(payload),
        "name": name,
        "nonce": body["nonce"],
        "payload_base64": base64.b64encode(payload).decode("ascii"),
        "payload_hash": MODULE.iroha_hash(payload),
        "signed_base64": base64.b64encode(signed).decode("ascii"),
        "signed_hash": MODULE.signed_transaction_entrypoint_hash(signed),
        "signed_len": len(signed),
        "time_to_live_ms": body["time_to_live_ms"],
    }


def populate_valid_corpus(tmp_path: Path) -> tuple[Path, Path, dict, dict]:
    source = tmp_path / "source"
    target = tmp_path / "target"

    shared_name = "shared_fixture"
    shared_body = payload_body(shared_name, shared=True)
    shared_bytes = b"shared-payload"
    shared_manifest_entry = manifest_entry(shared_name, shared_body, shared_bytes)
    shared_payload_entry = {
        "name": shared_name,
        **{
            key: shared_body[key]
            for key in (
                "authority",
                "chain",
                "creation_time_ms",
                "nonce",
                "time_to_live_ms",
            )
        },
        "payload": shared_body,
        **{
            key: shared_manifest_entry[key]
            for key in (
                "payload_base64",
                "payload_hash",
                "signed_base64",
                "signed_hash",
            )
        },
    }
    shared_payloads = [shared_payload_entry]
    shared_manifest = {"fixtures": [shared_manifest_entry]}
    for root in (source, target):
        dump(root, "transaction_payloads.json", shared_payloads)
        dump(root, "transaction_fixtures.manifest.json", shared_manifest)
    write(source, shared_manifest_entry["encoded_file"], shared_bytes)

    swift_payloads = []
    swift_entries = []
    for index, name in enumerate(sorted(MODULE.EXPECTED_SWIFT_FIXTURES), start=1):
        body = payload_body(name, shared=False)
        body["chain"] = f"{index:08d}"
        body["nonce"] = index
        body["creation_time_ms"] = index
        payload = f"swift-payload-{index}".encode()
        swift_payloads.append({"name": name, "payload": body})
        complete_entry = manifest_entry(name, body, payload)
        swift_entries.append(
            {
                key: complete_entry[key]
                for key in (
                    "name",
                    "payload_base64",
                    "payload_hash",
                    "signed_base64",
                    "signed_hash",
                )
            }
        )
        write(target, f"{name}.norito", payload)
    swift_manifest = {"fixtures": swift_entries}
    dump(target, "swift_parity_payloads.json", swift_payloads)
    dump(target, "swift_parity_manifest.json", swift_manifest)
    return source, target, shared_payloads, shared_manifest


def test_compare_validates_full_shared_and_three_swift_owned_fixtures(
    tmp_path: Path,
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    write(target, "js_email_identifier_request.json", "{}")
    write(target, "offline/kagemusha_peer_transport_v2.json", "{}")

    assert MODULE.compare(source, target) == ([], [], [])


def test_compare_rejects_non_swift_and_nested_norito_orphans(
    tmp_path: Path,
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    write(target, "transfer_asset.norito", "redundant")
    write(target, "nested/swift_looks_owned.norito", "nested-orphan")

    assert MODULE.compare(source, target) == (
        [],
        [
            Path("nested/swift_looks_owned.norito"),
            Path("transfer_asset.norito"),
        ],
        [],
    )


def test_compare_reports_descriptor_missing_and_content_drift(
    tmp_path: Path,
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    (target / "transaction_payloads.json").unlink()
    missing, extra, diffs = MODULE.compare(source, target)
    assert missing == [Path("transaction_payloads.json")]
    assert extra == []
    assert diffs == []

    dump(target, "transaction_payloads.json", json.loads((source / "transaction_payloads.json").read_text()))
    (target / "transaction_fixtures.manifest.json").write_text("different\n")
    missing, extra, diffs = MODULE.compare(source, target)
    assert missing == []
    assert extra == []
    assert [(src.name, dst.name) for src, dst in diffs] == [
        ("transaction_fixtures.manifest.json", "transaction_fixtures.manifest.json")
    ]


def test_compare_requires_both_canonical_descriptors(tmp_path: Path) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    (source / "transaction_fixtures.manifest.json").unlink()

    with pytest.raises(FileNotFoundError, match="missing canonical fixture"):
        MODULE.compare(source, target)


def test_duplicate_json_keys_are_rejected_before_last_wins_decode(
    tmp_path: Path,
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = source / "transaction_payloads.json"
    text = path.read_text()
    path.write_text(
        text.replace(
            '"name": "shared_fixture"',
            '"name": "shared_fixture", "name": "shared_fixture"',
            1,
        )
    )

    with pytest.raises(ValueError, match="duplicate JSON object key 'name'"):
        MODULE.compare(source, target)


@pytest.mark.parametrize(
    "mutation, message",
    [
        (lambda entry: entry.__setitem__("encoded", "legacy"), "unexpected=\\['encoded'\\]"),
        (lambda entry: entry["payload"].pop("metadata"), "missing=\\['metadata'\\]"),
        (
            lambda entry: entry["payload"]["executable"].update({"Ivm": "AA=="}),
            "exactly one executable variant",
        ),
    ],
)
def test_closed_payload_schema_rejects_alias_missing_metadata_and_multiple_variants(
    tmp_path: Path, mutation, message: str
) -> None:
    source, target, payloads, _ = populate_valid_corpus(tmp_path)
    invalid = copy.deepcopy(payloads)
    mutation(invalid[0])
    dump(source, "transaction_payloads.json", invalid)

    with pytest.raises(ValueError, match=message):
        MODULE.compare(source, target)


def test_direct_contract_call_and_canonical_ivm_are_first_class_variants(
    tmp_path: Path,
) -> None:
    source, target, payloads, _ = populate_valid_corpus(tmp_path)
    contract_call = {
        "arguments": [0, 255],
        "contract_address": "tairac1contract",
        "entrypoint": "run",
        "expected_code_hash": "hash:value",
    }
    payloads[0]["payload"]["executable"] = {"ContractCall": contract_call}
    dump(source, "transaction_payloads.json", payloads)
    dump(target, "transaction_payloads.json", payloads)
    assert MODULE.compare(source, target) == ([], [], [])

    payloads[0]["payload"]["executable"] = {"Ivm": "AA=="}
    dump(source, "transaction_payloads.json", payloads)
    dump(target, "transaction_payloads.json", payloads)
    assert MODULE.compare(source, target) == ([], [], [])

    payloads[0]["payload"]["executable"] = {"Ivm": "YQ="}
    dump(source, "transaction_payloads.json", payloads)
    with pytest.raises(ValueError, match="invalid base64"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("ttl", [None, 0, -1, True, 1.5, "1000"])
def test_payload_requires_explicit_positive_integer_ttl(
    tmp_path: Path, ttl: object
) -> None:
    source, target, payloads, _ = populate_valid_corpus(tmp_path)
    invalid = copy.deepcopy(payloads)
    if ttl is None:
        invalid[0]["payload"].pop("time_to_live_ms")
    else:
        invalid[0]["payload"]["time_to_live_ms"] = ttl
    dump(source, "transaction_payloads.json", invalid)

    with pytest.raises(ValueError, match="time_to_live_ms"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("nonce", [0, -1, True, 1.5, 2**32])
def test_nonce_bounds_are_enforced(tmp_path: Path, nonce: object) -> None:
    source, target, payloads, _ = populate_valid_corpus(tmp_path)
    invalid = copy.deepcopy(payloads)
    invalid[0]["nonce"] = nonce
    invalid[0]["payload"]["nonce"] = nonce
    dump(source, "transaction_payloads.json", invalid)

    with pytest.raises(ValueError, match="nonce"):
        MODULE.compare(source, target)


def test_manifest_rejects_traversal_and_payload_mismatch(tmp_path: Path) -> None:
    source, target, _, manifest = populate_valid_corpus(tmp_path)
    invalid = copy.deepcopy(manifest)
    invalid["fixtures"][0]["encoded_file"] = "../shared_fixture.norito"
    dump(source, "transaction_fixtures.manifest.json", invalid)
    with pytest.raises(ValueError, match="encoded_file must be exactly"):
        MODULE.compare(source, target)

    invalid = copy.deepcopy(manifest)
    invalid["fixtures"][0]["chain"] = "00000002"
    dump(source, "transaction_fixtures.manifest.json", invalid)
    with pytest.raises(ValueError, match="manifest/payload mismatch for chain"):
        MODULE.compare(source, target)


def test_swift_owned_set_is_exactly_three_artifacts(tmp_path: Path) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    (target / "swift_burn_asset_basic.norito").unlink()

    with pytest.raises(FileNotFoundError, match="missing fixture blob"):
        MODULE.compare(source, target)


def test_swift_manifest_rejects_legacy_root_and_entry_metadata(tmp_path: Path) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    manifest_path = target / "swift_parity_manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["generated_at"] = "legacy"
    dump(target, "swift_parity_manifest.json", manifest)
    with pytest.raises(ValueError, match="unexpected=\\['generated_at'\\]"):
        MODULE.compare(source, target)

    manifest.pop("generated_at")
    manifest["fixtures"][0]["encoded_file"] = "legacy.norito"
    dump(target, "swift_parity_manifest.json", manifest)
    with pytest.raises(ValueError, match="unexpected=\\['encoded_file'\\]"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("instruction_count", [0, 2])
def test_swift_payload_requires_exactly_one_instruction(
    tmp_path: Path, instruction_count: int
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    instruction = payloads[0]["payload"]["executable"]["Instructions"][0]
    payloads[0]["payload"]["executable"]["Instructions"] = [
        copy.deepcopy(instruction) for _ in range(instruction_count)
    ]
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="exactly one instruction"):
        MODULE.compare(source, target)


@pytest.mark.parametrize(
    "fixture_name,field",
    [
        (fixture_name, field)
        for fixture_name in sorted(MODULE.EXPECTED_SWIFT_FIXTURES)
        for field in ("kind", "action")
    ],
)
def test_swift_payload_rejects_every_name_kind_action_divergence(
    tmp_path: Path, fixture_name: str, field: str
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    entry = next(entry for entry in payloads if entry["name"] == fixture_name)
    instruction = entry["payload"]["executable"]["Instructions"][0]
    if field == "kind":
        instruction["kind"] = "Wrong"
    else:
        instruction["arguments"]["action"] = "WrongAction"
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="must use kind/action"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("mutation", ["missing", "extra", "non_string"])
def test_swift_instruction_argument_schema_is_exact(
    tmp_path: Path, mutation: str
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    arguments = payloads[0]["payload"]["executable"]["Instructions"][0]["arguments"]
    if mutation == "missing":
        arguments.pop("destination")
    elif mutation == "extra":
        arguments["legacy"] = "alias"
    else:
        arguments["quantity"] = 1
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="arguments"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("payer", ["owner", "Authority", True, ""])
def test_swift_fee_payer_is_exact_authority(tmp_path: Path, payer: object) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payloads[0]["payload"]["fee_payment"]["payer"] = payer
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="payer"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("charge_limits", [{}, [{}], None, ["legacy"]])
def test_swift_charge_limits_is_exact_empty_array(
    tmp_path: Path, charge_limits: object
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payloads[0]["payload"]["fee_payment"]["value"]["charge_limits"] = charge_limits
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="charge_limits"):
        MODULE.compare(source, target)


def test_swift_metadata_accepts_arbitrary_native_json_values(tmp_path: Path) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payloads[0]["payload"]["metadata"] = {
        "bool": True,
        "list": [1, None, "value"],
        "nested": {"number": 2},
    }
    dump(target, path.name, payloads)

    assert MODULE.compare(source, target) == ([], [], [])


@pytest.mark.parametrize("metadata", [None, [], "metadata"])
def test_swift_metadata_must_remain_an_explicit_object(
    tmp_path: Path, metadata: object
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payloads[0]["payload"]["metadata"] = metadata
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="metadata"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("ttl", [None, 0, -1, True, 1.5, "1000"])
def test_swift_payload_requires_explicit_positive_integer_ttl(
    tmp_path: Path, ttl: object
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payload = payloads[0]["payload"]
    if ttl is None:
        payload.pop("time_to_live_ms")
    else:
        payload["time_to_live_ms"] = ttl
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="time_to_live_ms"):
        MODULE.compare(source, target)


@pytest.mark.parametrize("nonce", [None, 0, -1, True, 1.5, 2**32])
def test_swift_payload_requires_explicit_bounded_positive_nonce(
    tmp_path: Path, nonce: object
) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payloads[0]["payload"]["nonce"] = nonce
    dump(target, path.name, payloads)

    with pytest.raises(ValueError, match="nonce"):
        MODULE.compare(source, target)


def test_swift_payload_rejects_unknown_and_duplicate_fields(tmp_path: Path) -> None:
    source, target, _, _ = populate_valid_corpus(tmp_path)
    path = target / "swift_parity_payloads.json"
    payloads = json.loads(path.read_text())
    payloads[0]["payload"]["legacy"] = "alias"
    dump(target, path.name, payloads)
    with pytest.raises(ValueError, match="unexpected=\\['legacy'\\]"):
        MODULE.compare(source, target)

    populate_valid_corpus(tmp_path)
    text = path.read_text()
    path.write_text(
        text.replace(
            '"name": "swift_burn_asset_basic"',
            '"name": "swift_burn_asset_basic", "name": "duplicate"',
            1,
        )
    )
    with pytest.raises(ValueError, match="duplicate JSON object key 'name'"):
        MODULE.compare(source, target)
