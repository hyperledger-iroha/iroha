"""Tests for the signed, payload-free SoraFS L1 lane inventory."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import sys
from pathlib import Path

import pytest


SCRIPTS = Path(__file__).resolve().parents[1]
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import sorafs_l1_lane_evidence_inventory as MODULE  # noqa: E402
from sorafs_resilience_test_support import (  # noqa: E402
    public_key_from_seed,
    sign,
)


NOW = 1_800_000_000
DEPLOYMENT_ID = "sorafs-taira-qualification-2026-08"
ENVIRONMENT = "production"
SEED = bytes.fromhex("4d" * 32)
PUBLIC_KEY = public_key_from_seed(SEED)
SERVICE_ID = "sorafs-l1-lane-inventory-signer-a"
ADMINISTRATOR_ID = "sorafs-l1-lane-inventory-admin-b"
POLICY_DIGEST = hashlib.sha256(b"l1 lane inventory policy").hexdigest()
TOPOLOGY = {
    "qualification_summary_sha256": hashlib.sha256(b"topology summary").hexdigest(),
    "manifest_sha256": hashlib.sha256(b"topology manifest").hexdigest(),
    "canonical_manifest_sha256": hashlib.sha256(
        b"canonical topology manifest"
    ).hexdigest(),
    "deployment_id": DEPLOYMENT_ID,
    "environment": ENVIRONMENT,
    "network": MODULE.TAIRA_NETWORK,
    "chain_id": MODULE.TAIRA_CHAIN_ID,
    "chain_discriminant": MODULE.TAIRA_CHAIN_DISCRIMINANT,
    "validator_ids_sha256": hashlib.sha256(b"four taira validators").hexdigest(),
}


def summary_payload(lane: str, schema: str, *, generated: int = NOW - 60) -> dict:
    """Build one minimal payload-free, topology-bound ready lane summary."""

    return {
        "schema": schema,
        "status": "ready",
        "topology_qualification": copy.deepcopy(TOPOLOGY),
        "recognized_artifact_count": 1,
        "recognized_artifacts": [
            {
                "kind": f"{lane}_qualification",
                "schema": f"sorafs.{lane}.qualified_artifact.v1",
                "status": "passed",
                "sha256": hashlib.sha256(f"{lane} artifact".encode()).hexdigest(),
                "fingerprint": {
                    "deployment_id": DEPLOYMENT_ID,
                    "environment": ENVIRONMENT,
                    "generated_at_unix": generated,
                    "deployment_context_reviewed": True,
                },
                "valid": True,
                "errors": [],
            }
        ],
        "errors": [],
    }


def write_summaries(root: Path, *, generated: int = NOW - 60) -> list[str]:
    """Write all summaries in the exact contract order."""

    root.mkdir(parents=True, exist_ok=True)
    values = []
    for lane, schema in MODULE.LANES:
        path = root / f"{lane}.json"
        path.write_bytes(
            MODULE.canonical_file_bytes(
                summary_payload(lane, schema, generated=generated)
            )
        )
        values.append(f"{lane}={path}")
    return values


def trust(*, now: int = NOW, public_key: bytes = PUBLIC_KEY) -> dict:
    """Return the independently supplied trust tuple."""

    return {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "evaluation_now": now,
        "verification_public_key_hex": public_key.hex(),
        "service_id": SERVICE_ID,
        "administrator_id": ADMINISTRATOR_ID,
        "key_revision": 7,
        "policy_revision": 11,
        "policy_digest_sha256": POLICY_DIGEST,
        "expected_topology_qualification_summary_sha256": TOPOLOGY[
            "qualification_summary_sha256"
        ],
        "expected_topology_manifest_sha256": TOPOLOGY["manifest_sha256"],
        "expected_topology_canonical_manifest_sha256": TOPOLOGY[
            "canonical_manifest_sha256"
        ],
        "expected_validator_ids_sha256": TOPOLOGY["validator_ids_sha256"],
    }


def prepare(root: Path, **overrides: object) -> tuple[dict, tuple[tuple[str, Path], ...]]:
    """Prepare one valid unsigned inventory."""

    specs = MODULE.parse_summary_specs(write_summaries(root))
    arguments = trust()
    arguments.update(overrides)
    value = MODULE.build_unsigned_inventory(
        specs,
        generated_at_unix=NOW,
        **arguments,
    )
    return value, specs


def finalize(root: Path) -> tuple[dict, tuple[tuple[str, Path], ...]]:
    """Finalize one valid inventory using the in-memory test signer."""

    unsigned, specs = prepare(root)
    signature = sign(SEED, MODULE.signing_bytes(unsigned)).hex()
    return MODULE.finalize_inventory(unsigned, signature, specs, **trust()), specs


def rewrite(path: Path, mutation) -> None:
    """Apply a mutation while preserving the required canonical file form."""

    value = json.loads(path.read_text(encoding="ascii"))
    mutation(value)
    path.write_bytes(MODULE.canonical_file_bytes(value))


def common_cli(specs: tuple[tuple[str, Path], ...]) -> list[str]:
    """Render exact summaries and the public trust tuple for CLI tests."""

    args: list[str] = []
    for lane, path in specs:
        args.extend(("--summary", f"{lane}={path}"))
    args.extend(
        (
            "--deployment-id",
            DEPLOYMENT_ID,
            "--environment",
            ENVIRONMENT,
            "--now-unix",
            str(NOW),
            "--verification-public-key-hex",
            PUBLIC_KEY.hex(),
            "--service-id",
            SERVICE_ID,
            "--administrator-id",
            ADMINISTRATOR_ID,
            "--key-revision",
            "7",
            "--policy-revision",
            "11",
            "--policy-digest-sha256",
            POLICY_DIGEST,
            "--expected-topology-qualification-summary-sha256",
            TOPOLOGY["qualification_summary_sha256"],
            "--expected-topology-manifest-sha256",
            TOPOLOGY["manifest_sha256"],
            "--expected-topology-canonical-manifest-sha256",
            TOPOLOGY["canonical_manifest_sha256"],
            "--expected-validator-ids-sha256",
            TOPOLOGY["validator_ids_sha256"],
        )
    )
    return args


def test_exact_17_inventory_is_deterministic_and_payload_free(tmp_path: Path) -> None:
    first, specs = prepare(tmp_path)
    second = MODULE.build_unsigned_inventory(
        specs,
        generated_at_unix=NOW,
        **trust(),
    )
    assert MODULE.canonical_file_bytes(first) == MODULE.canonical_file_bytes(second)
    assert first["summary_file_count"] == 17
    assert first["recognized_summary_count"] == 17
    assert [row["lane"] for row in first["summaries"]] == [
        lane for lane, _schema in MODULE.LANES
    ]
    rendered = MODULE.canonical_file_bytes(first)
    assert b'"path"' not in rendered
    assert b'"payload"' not in rendered
    assert b"minamoto" not in rendered.lower()

    signature = sign(SEED, MODULE.signing_bytes(first)).hex()
    inventory = MODULE.finalize_inventory(first, signature, specs, **trust())
    replay_a = MODULE.verify_inventory(inventory, specs, **trust())
    replay_b = MODULE.verify_inventory(inventory, specs, **trust())
    assert MODULE.canonical_file_bytes(replay_a) == MODULE.canonical_file_bytes(replay_b)
    assert replay_a["status"] == "ready"
    assert replay_a["signer_qualification"] == "software-key-qualified"


def test_verified_artifact_status_is_a_canonical_success(tmp_path: Path) -> None:
    values = write_summaries(tmp_path)
    path = Path(values[0].partition("=")[2])
    rewrite(
        path,
        lambda value: value["recognized_artifacts"][0].update(
            {"status": "verified"}
        ),
    )
    inventory = MODULE.build_unsigned_inventory(
        MODULE.parse_summary_specs(values),
        generated_at_unix=NOW,
        **trust(),
    )
    assert inventory["status"] == "ready"


@pytest.mark.parametrize("hostile_status", [{}, [], None, 7])
def test_artifact_status_shapes_fail_closed(
    tmp_path: Path,
    hostile_status: object,
) -> None:
    values = write_summaries(tmp_path)
    path = Path(values[0].partition("=")[2])
    rewrite(
        path,
        lambda value: value["recognized_artifacts"][0].update(
            {"status": hostile_status}
        ),
    )
    with pytest.raises(MODULE.InventoryError, match="invalid recognized artifact"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(values), generated_at_unix=NOW, **trust()
        )


@pytest.mark.parametrize(
    "mutate",
    [
        lambda values: values[:-1],
        lambda values: [*values, values[-1]],
        lambda values: [values[1], values[0], *values[2:]],
        lambda values: [values[0], values[0], *values[2:]],
        lambda values: [values[0].replace("ai_prescreen", "ai-prescreen", 1), *values[1:]],
    ],
)
def test_summary_inputs_reject_missing_extra_reorder_duplicate_and_alias(
    tmp_path: Path,
    mutate,
) -> None:
    values = write_summaries(tmp_path)
    with pytest.raises(MODULE.InventoryError):
        MODULE.parse_summary_specs(mutate(values))


def test_exact_summary_byte_tamper_fails_signed_replay(tmp_path: Path) -> None:
    inventory, specs = finalize(tmp_path)
    first_path = specs[0][1]
    rewrite(first_path, lambda value: value.update({"review_generation": 2}))
    with pytest.raises(MODULE.InventoryError, match="deterministic summary replay"):
        MODULE.verify_inventory(inventory, specs, **trust())


def test_stale_and_future_lane_evidence_fail(tmp_path: Path) -> None:
    stale_specs = MODULE.parse_summary_specs(
        write_summaries(
            tmp_path / "stale",
            generated=NOW - MODULE.MAX_SUMMARY_AGE_SECS - 1,
        )
    )
    with pytest.raises(MODULE.InventoryError, match="stale"):
        MODULE.build_unsigned_inventory(
            stale_specs,
            generated_at_unix=NOW,
            **trust(),
        )
    future_specs = MODULE.parse_summary_specs(
        write_summaries(tmp_path / "future", generated=NOW + 1)
    )
    with pytest.raises(MODULE.InventoryError, match="future"):
        MODULE.build_unsigned_inventory(
            future_specs,
            generated_at_unix=NOW,
            **trust(),
        )


def test_nonsoftware_backend_fails_even_with_a_fresh_signature(tmp_path: Path) -> None:
    inventory, specs = finalize(tmp_path)
    unsigned = copy.deepcopy(inventory)
    unsigned["signer"].pop("signature_hex")
    unsigned["signer"]["backend"] = "hsm"
    inventory["signer"]["backend"] = "hsm"
    inventory["signer"]["signature_hex"] = sign(
        SEED,
        MODULE.signing_bytes(unsigned),
    ).hex()
    with pytest.raises(MODULE.InventoryError, match="deterministic summary replay"):
        MODULE.verify_inventory(inventory, specs, **trust())


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("deployment_id", "local-taira"),
        ("environment", "staging"),
        ("service_id", "test-signer"),
        ("administrator_id", "local-admin"),
    ],
)
def test_local_or_nonproduction_trust_values_fail(
    tmp_path: Path,
    field: str,
    value: str,
) -> None:
    specs = MODULE.parse_summary_specs(write_summaries(tmp_path))
    arguments = trust()
    arguments[field] = value
    with pytest.raises(MODULE.InventoryError, match="production Taira"):
        MODULE.build_unsigned_inventory(
            specs,
            generated_at_unix=NOW,
            **arguments,
        )


def test_environment_is_hard_cut_to_prod_or_production(tmp_path: Path) -> None:
    specs = MODULE.parse_summary_specs(write_summaries(tmp_path))
    arguments = trust()
    arguments["environment"] = "qa"
    with pytest.raises(MODULE.InventoryError, match="exactly prod or production"):
        MODULE.build_unsigned_inventory(
            specs,
            generated_at_unix=NOW,
            **arguments,
        )


def test_alias_field_and_reordered_rows_fail_schema_or_replay(tmp_path: Path) -> None:
    inventory, specs = finalize(tmp_path)
    alias = copy.deepcopy(inventory)
    alias["signer"]["public_key_digest_sha256"] = alias["signer"][
        "public_key_fingerprint_sha256"
    ]
    with pytest.raises(MODULE.InventoryError, match="wrong exact schema"):
        MODULE.verify_inventory(alias, specs, **trust())
    reordered = copy.deepcopy(inventory)
    reordered["summaries"][0], reordered["summaries"][1] = (
        reordered["summaries"][1],
        reordered["summaries"][0],
    )
    with pytest.raises(MODULE.InventoryError, match="deterministic summary replay"):
        MODULE.verify_inventory(reordered, specs, **trust())


def test_trailing_summary_bytes_fail_canonical_contract(tmp_path: Path) -> None:
    values = write_summaries(tmp_path)
    path = Path(values[0].partition("=")[2])
    path.write_bytes(path.read_bytes() + b" \n")
    specs = MODULE.parse_summary_specs(values)
    with pytest.raises(MODULE.InventoryError, match="exactly one trailing LF"):
        MODULE.build_unsigned_inventory(
            specs,
            generated_at_unix=NOW,
            **trust(),
        )


def test_deployment_drift_and_minamoto_observation_fail(tmp_path: Path) -> None:
    drift_values = write_summaries(tmp_path / "drift")
    drift_path = Path(drift_values[5].partition("=")[2])
    rewrite(
        drift_path,
        lambda value: value["recognized_artifacts"][0]["fingerprint"].update(
            {"deployment_id": "sorafs-other-production"}
        ),
    )
    with pytest.raises(MODULE.InventoryError, match="deployment_id"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(drift_values),
            generated_at_unix=NOW,
            **trust(),
        )

    minamoto_values = write_summaries(tmp_path / "minamoto")
    minamoto_path = Path(minamoto_values[7].partition("=")[2])
    rewrite(
        minamoto_path,
        lambda value: value.update({"read_route_observation": "minamoto-finalized"}),
    )
    with pytest.raises(MODULE.InventoryError, match="Minamoto"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(minamoto_values),
            generated_at_unix=NOW,
            **trust(),
        )


def test_signature_mutation_and_trust_substitution_fail(tmp_path: Path) -> None:
    inventory, specs = finalize(tmp_path)
    mutated = copy.deepcopy(inventory)
    signature = bytearray.fromhex(mutated["signer"]["signature_hex"])
    signature[0] ^= 1
    mutated["signer"]["signature_hex"] = signature.hex()
    with pytest.raises(MODULE.InventoryError, match="signature is invalid"):
        MODULE.verify_inventory(mutated, specs, **trust())

    alternate_key = public_key_from_seed(bytes.fromhex("5e" * 32))
    with pytest.raises(MODULE.InventoryError, match="deterministic summary replay"):
        MODULE.verify_inventory(inventory, specs, **trust(public_key=alternate_key))


def test_topology_anchor_drift_fails(tmp_path: Path) -> None:
    values = write_summaries(tmp_path)
    path = Path(values[-1].partition("=")[2])
    rewrite(
        path,
        lambda value: value["topology_qualification"].update(
            {"manifest_sha256": hashlib.sha256(b"stale topology fork").hexdigest()}
        ),
    )
    with pytest.raises(MODULE.InventoryError, match="operator-trusted fresh topology"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(values),
            generated_at_unix=NOW,
            **trust(),
        )


def test_summary_file_must_be_direct_regular_single_link(tmp_path: Path) -> None:
    values = write_summaries(tmp_path / "source")
    original = Path(values[0].partition("=")[2])
    symlink = tmp_path / "alias.json"
    symlink.symlink_to(original)
    values[0] = f"ai_prescreen={symlink}"
    with pytest.raises(MODULE.InventoryError, match="direct regular"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(values),
            generated_at_unix=NOW,
            **trust(),
        )


def test_ancestor_symlink_swap_cannot_rebind_summary_reads(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    evidence_root = tmp_path / "evidence"
    values = write_summaries(evidence_root)
    attacker_root = tmp_path / "attacker"
    attacker_root.mkdir()
    attacker = summary_payload(*MODULE.LANES[0])
    attacker["read_route_observation"] = "minamoto-finalized"
    (attacker_root / "ai_prescreen.json").write_bytes(
        MODULE.canonical_file_bytes(attacker)
    )
    original_open = MODULE.os.open
    swapped = False

    def swapping_open(path, flags, mode=0o777, *, dir_fd=None):
        nonlocal swapped
        descriptor = original_open(path, flags, mode, dir_fd=dir_fd)
        if path == "evidence" and not swapped and flags & os.O_DIRECTORY:
            swapped = True
            evidence_root.rename(tmp_path / "anchored-original")
            evidence_root.symlink_to(attacker_root, target_is_directory=True)
        return descriptor

    monkeypatch.setattr(MODULE.os, "open", swapping_open)
    with pytest.raises(MODULE.InventoryError, match="direct directories"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(values),
            generated_at_unix=NOW,
            **trust(),
        )
    assert swapped

    values = write_summaries(tmp_path / "hardlink-source")
    original = Path(values[0].partition("=")[2])
    hardlink = tmp_path / "hardlink.json"
    os.link(original, hardlink)
    with pytest.raises(MODULE.InventoryError, match="hard-linked"):
        MODULE.build_unsigned_inventory(
            MODULE.parse_summary_specs(values),
            generated_at_unix=NOW,
            **trust(),
        )


def test_cli_prepare_finalize_verify_and_replay(tmp_path: Path, capsys) -> None:
    specs = MODULE.parse_summary_specs(write_summaries(tmp_path / "summaries"))
    prepared = tmp_path / "prepared.json"
    payload = tmp_path / "signing.payload"
    assert MODULE.main(
        [
            "prepare",
            *common_cli(specs),
            "--generated-at-unix",
            str(NOW),
            "--prepared-out",
            str(prepared),
            "--signing-payload-out",
            str(payload),
        ]
    ) == 0
    signature = sign(SEED, payload.read_bytes()).hex()
    inventory = tmp_path / "inventory.json"
    assert MODULE.main(
        [
            "finalize",
            *common_cli(specs),
            "--prepared",
            str(prepared),
            "--signature-hex",
            signature,
            "--inventory-out",
            str(inventory),
        ]
    ) == 0
    assert MODULE.main(
        ["verify", *common_cli(specs), "--inventory", str(inventory)]
    ) == 0
    first = capsys.readouterr().out
    assert MODULE.main(
        ["verify", *common_cli(specs), "--inventory", str(inventory)]
    ) == 0
    assert capsys.readouterr().out == first
    assert json.loads(first)["recognized_summary_count"] == 17


def test_cli_rejects_secret_signing_arguments_without_echo(capsys) -> None:
    secret = "/runtime/secrets/private-seed"
    assert MODULE.main(["prepare", "--private-key", secret]) == 2
    captured = capsys.readouterr()
    assert "secret signing inputs are not accepted" in captured.err
    assert secret not in captured.err
