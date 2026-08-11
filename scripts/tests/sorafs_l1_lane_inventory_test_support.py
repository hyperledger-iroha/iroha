"""Test-only helpers for independently signed exact-17 lane inventories."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

import sorafs_l1_lane_evidence_inventory as inventory
from sorafs_resilience_test_support import public_key_from_seed, sign
from sorafs_l1_lane_inventory_integration import VerifiedLaneInventory


SIGNING_SEED = bytes.fromhex("5d" * 32)
PUBLIC_KEY = public_key_from_seed(SIGNING_SEED)
SERVICE_ID = "sorafs-l1-inventory-signer-service"
ADMINISTRATOR_ID = "sorafs-l1-inventory-administrator"
KEY_REVISION = 13
POLICY_REVISION = 17
POLICY_DIGEST_SHA256 = hashlib.sha256(
    b"test reviewed SoraFS L1 inventory policy"
).hexdigest()


def normalize_generated_at_unix(value: Any, generated_at_unix: int) -> None:
    """Set every nested fixture generation timestamp to one fresh instant."""

    if isinstance(value, dict):
        if "generated_at_unix" in value:
            value["generated_at_unix"] = generated_at_unix
        for child in value.values():
            normalize_generated_at_unix(child, generated_at_unix)
    elif isinstance(value, list):
        for child in value:
            normalize_generated_at_unix(child, generated_at_unix)


def _trust(
    topology: Mapping[str, object],
    *,
    deployment_id: str,
    environment: str,
    now_unix: int,
) -> dict[str, object]:
    return {
        "deployment_id": deployment_id,
        "environment": environment,
        "evaluation_now": now_unix,
        "verification_public_key_hex": PUBLIC_KEY.hex(),
        "service_id": SERVICE_ID,
        "administrator_id": ADMINISTRATOR_ID,
        "key_revision": KEY_REVISION,
        "policy_revision": POLICY_REVISION,
        "policy_digest_sha256": POLICY_DIGEST_SHA256,
        "expected_topology_qualification_summary_sha256": topology[
            "qualification_summary_sha256"
        ],
        "expected_topology_manifest_sha256": topology["manifest_sha256"],
        "expected_topology_canonical_manifest_sha256": topology[
            "canonical_manifest_sha256"
        ],
        "expected_validator_ids_sha256": topology["validator_ids_sha256"],
    }


def write_signed_inventory(
    path: Path,
    lane_paths: Sequence[tuple[str, Path]],
    topology: Mapping[str, object],
    *,
    deployment_id: str,
    environment: str,
    now_unix: int,
) -> Path:
    """Write a genuine signed inventory over the supplied exact lane bytes."""

    specs = tuple(lane_paths)
    trust = _trust(
        topology,
        deployment_id=deployment_id,
        environment=environment,
        now_unix=now_unix,
    )
    unsigned = inventory.build_unsigned_inventory(
        specs,
        generated_at_unix=now_unix,
        **trust,
    )
    signature = sign(SIGNING_SEED, inventory.signing_bytes(unsigned)).hex()
    signed = inventory.finalize_inventory(unsigned, signature, specs, **trust)
    path.write_bytes(inventory.canonical_file_bytes(signed))
    return path


def inventory_cli_args(path: Path) -> list[str]:
    """Return the public trust tuple accepted by integrated release tools."""

    return [
        "--l1-lane-evidence-inventory",
        str(path),
        "--l1-lane-evidence-inventory-verification-public-key-hex",
        PUBLIC_KEY.hex(),
        "--l1-lane-evidence-inventory-signer-service-id",
        SERVICE_ID,
        "--l1-lane-evidence-inventory-signer-administrator-id",
        ADMINISTRATOR_ID,
        "--l1-lane-evidence-inventory-signer-key-revision",
        str(KEY_REVISION),
        "--l1-lane-evidence-inventory-signer-policy-revision",
        str(POLICY_REVISION),
        "--l1-lane-evidence-inventory-signer-policy-digest-sha256",
        POLICY_DIGEST_SHA256,
    ]


def verified_inventory(
    path: Path,
    lane_paths: Sequence[tuple[str, Path]],
    topology: Mapping[str, object],
    *,
    deployment_id: str,
    environment: str,
    now_unix: int,
) -> VerifiedLaneInventory:
    """Replay a test inventory and return the production integration value."""

    signed, _raw = inventory.load_canonical_inventory_file(path)
    trust = _trust(
        topology,
        deployment_id=deployment_id,
        environment=environment,
        now_unix=now_unix,
    )
    verification = inventory.verify_inventory(signed, lane_paths, **trust)
    return VerifiedLaneInventory(
        verification,
        {row["lane"]: row["summary_sha256"] for row in signed["summaries"]},
    )


def write_payload_inventory(
    root: Path,
    payloads: Mapping[str, Mapping[str, Any]],
    topology: Mapping[str, object],
    *,
    deployment_id: str,
    environment: str,
    now_unix: int,
) -> tuple[Path, tuple[tuple[str, Path], ...]]:
    """Canonicalize 17 supplied payloads and sign their exact copied bytes."""

    summary_root = root / ".l1-lane-inventory-summaries"
    summary_root.mkdir(exist_ok=True)
    lane_paths = []
    for lane, _schema in inventory.LANES:
        path = summary_root / f"{lane}.summary"
        payload = dict(payloads[lane])
        payload["topology_qualification"] = dict(topology)
        path.write_bytes(inventory.canonical_file_bytes(payload))
        lane_paths.append((lane, path))
    inventory_path = root / "l1-lane-evidence.inventory"
    write_signed_inventory(
        inventory_path,
        lane_paths,
        topology,
        deployment_id=deployment_id,
        environment=environment,
        now_unix=now_unix,
    )
    return inventory_path, tuple(lane_paths)


def write_checker_inventory(
    root: Path,
    *,
    write_topology: Callable[..., Path],
    load_topology: Callable[..., tuple[dict[str, Any] | None, list[str]]],
    payload_factory: Callable[..., dict[str, Any]],
    deployment_id: str,
    environment: str,
    generated_at_unix: int,
) -> tuple[Path, tuple[tuple[str, Path], ...], dict[str, Any]]:
    """Build a checker fixture from existing lanes or a payload factory."""

    topology_path = write_topology(root, deployment_id, environment)
    topology, errors = load_topology(
        topology_path,
        expected_deployment_id=deployment_id,
        expected_environment=environment,
    )
    if errors or topology is None:
        raise AssertionError("test topology fixture did not validate")
    inventory_path = root / "l1-lane-evidence.inventory"
    summary_root = root / ".l1-lane-inventory-summaries"
    cached_lanes = tuple(
        (lane, summary_root / f"{lane}.summary") for lane, _schema in inventory.LANES
    )
    if inventory_path.exists() and all(path.exists() for _lane, path in cached_lanes):
        return inventory_path, cached_lanes, topology
    payloads = {}
    for lane, _schema in inventory.LANES:
        existing = root / f"{lane}.json"
        payloads[lane] = (
            json.loads(existing.read_text(encoding="utf-8"))
            if existing.exists()
            else payload_factory(
                lane,
                generated_at_unix=generated_at_unix,
                deployment_id=deployment_id,
                environment=environment,
            )
        )
    try:
        path, lanes = write_payload_inventory(
            root, payloads, topology, deployment_id=deployment_id,
            environment=environment, now_unix=generated_at_unix + 1,
        )
    except inventory.InventoryError:
        payloads = {
            lane: payload_factory(
                lane, generated_at_unix=generated_at_unix,
                deployment_id=deployment_id, environment=environment,
            )
            for lane, _schema in inventory.LANES
        }
        path, lanes = write_payload_inventory(
            root, payloads, topology, deployment_id=deployment_id,
            environment=environment, now_unix=generated_at_unix + 1,
        )
    return path, lanes, topology
