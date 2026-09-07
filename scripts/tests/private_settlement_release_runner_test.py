"""Tests for the fail-closed AtomicPrivateSettlementV1 release runner."""

from __future__ import annotations

import copy
from contextlib import ExitStack, contextmanager
import hashlib
import importlib.util
import io
import json
import os
import stat
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_release_runner.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_release_runner", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

COMMIT = "a" * 40
HARDWARE = "b" * 64
HARDWARE_PROFILE = "9" * 64
CONFIGURATION = "c" * 64
EXECUTABLE = "d" * 64
INVOCATION_NONCE = "8" * 64


def _iroha_hash_literal(body: str) -> str:
    uppercase = body.upper()
    crc = 0xFFFF
    for byte in f"hash:{uppercase}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return f"hash:{uppercase}#{crc:04X}"


def plan() -> dict[str, Any]:
    """Return the response-validation subset of a frozen plan."""

    return {
        "commit": COMMIT,
        "hardware": {
            "sha256": HARDWARE,
            "profile_sha256": HARDWARE_PROFILE,
        },
    }


def process_inventory(participants: int) -> list[dict[str, Any]]:
    """Build the exact real-process topology acknowledgement."""

    rows: list[tuple[str, int | None, int | None]] = [("coordinator", None, None)]
    rows.extend(
        ("global_validator", None, validator)
        for validator in range(MODULE.GLOBAL_VALIDATORS)
    )
    rows.extend(
        ("dataspace_validator", dataspace, validator)
        for dataspace in range(participants)
        for validator in range(MODULE.VALIDATORS_PER_DATASPACE)
    )
    return [
        {
            "role": role,
            "dataspace_ordinal": dataspace,
            "validator_ordinal": validator,
            "pid": index + 100,
            "executable_sha256": EXECUTABLE,
            "revision": COMMIT,
            "health_observed": True,
        }
        for index, (role, dataspace, validator) in enumerate(rows)
    ]


def _ethernet_ipv4_tcp(source_port: int, destination_port: int) -> bytes:
    ethernet = bytes.fromhex("00112233445566778899aabb0800")
    ipv4 = bytearray(20)
    ipv4[0] = 0x45
    ipv4[2:4] = (40).to_bytes(2, "big")
    ipv4[8] = 64
    ipv4[9] = 6
    ipv4[12:16] = bytes((127, 0, 0, 1))
    ipv4[16:20] = bytes((127, 0, 0, 1))
    tcp = bytearray(20)
    tcp[:2] = source_port.to_bytes(2, "big")
    tcp[2:4] = destination_port.to_bytes(2, "big")
    tcp[12] = 5 << 4
    return ethernet + bytes(ipv4) + bytes(tcp)


def _write_pcapng(path: Path, packets: list[bytes]) -> None:
    body = bytearray(MODULE.capture_split.pcapng.PCAPNG_SECTION_HEADER)
    body.extend(MODULE.capture_split.pcapng._interface_description(1, 65_535, 6))
    for index, packet in enumerate(packets, 1):
        body.extend(
            MODULE.capture_split.pcapng._enhanced_packet(
                index * 1_000_000, len(packet), len(packet), packet
            )
        )
    path.write_bytes(bytes(body))


def _write_pcap(path: Path, packets: list[bytes]) -> None:
    body = bytearray(struct.pack("<IHHIIII", 0xA1B2C3D4, 2, 4, 0, 0, 65_535, 1))
    for index, packet in enumerate(packets, 1):
        body.extend(struct.pack("<IIII", index, 0, len(packet), len(packet)))
        body.extend(packet)
    path.write_bytes(bytes(body))
    path.chmod(0o600)


def _source_binding(sources: list[bytes]) -> dict[str, Any]:
    digest = hashlib.sha256()
    digest.update(b"iroha:aps-leakage-source-binding:v1\0")
    digest.update(struct.pack("<Q", len(sources)))
    for source in sources:
        digest.update(struct.pack("<Q", len(source)))
        digest.update(source)
    return {
        "source_sha256": digest.hexdigest(),
        "source_bytes": sum(map(len, sources)),
        "source_count": len(sources),
    }


def _atomicity_evidence(peer_index: int, participants: int) -> bytes:
    count_names = (
        "governance",
        "pools",
        "roots",
        "nullifiers",
        "commitments",
        "encrypted_outputs",
        "replay_markers",
        "receipts",
        "abort_markers",
        "staged_pool_heads",
        "staged_nullifiers",
        "staged_output_commitments",
        "replicated_staged_locks",
        "staged_locks",
    )
    baseline = {name: 0 for name in count_names}
    prepared = dict(baseline)
    prepared["replicated_staged_locks"] = 1 + participants * 9
    if peer_index >= MODULE.GLOBAL_VALIDATORS:
        prepared.update(
            {
                "staged_pool_heads": 1,
                "staged_nullifiers": 2,
                "staged_output_commitments": 3,
                "staged_locks": 6,
            }
        )
    final = dict(baseline)
    final.update(
        {
            "roots": participants,
            "nullifiers": participants * 2,
            "commitments": participants * 3,
            "encrypted_outputs": participants * 3,
            "replay_markers": 1,
            "receipts": 1,
        }
    )
    observations = []
    empty_replicated_staged = _iroha_hash_literal("4" * 64)
    prepared_replicated_staged = _iroha_hash_literal("5" * 64)
    empty_staged = _iroha_hash_literal("3" * 64)
    prepared_staged = (
        _iroha_hash_literal(
            f"{7 + (peer_index - MODULE.GLOBAL_VALIDATORS) // MODULE.VALIDATORS_PER_DATASPACE:02X}"
            * 32
        )
        if peer_index >= MODULE.GLOBAL_VALIDATORS
        else empty_staged
    )
    for index, (counts, ledger, replicated_staged, staged) in enumerate(
        (
            (
                baseline,
                _iroha_hash_literal("1" * 64),
                empty_replicated_staged,
                empty_staged,
            ),
            (
                prepared,
                _iroha_hash_literal("1" * 64),
                prepared_replicated_staged,
                prepared_staged,
            ),
            (
                final,
                _iroha_hash_literal("2" * 64),
                empty_replicated_staged,
                empty_staged,
            ),
        )
    ):
        response = json.dumps(
            {
                "format_version": 1,
                "height": index + 1,
                "commitment": _iroha_hash_literal(
                    f"{peer_index + index + 4:02X}" * 32
                ),
                "ledger_commitment": ledger,
                "replicated_staged_lock_commitment": replicated_staged,
                "staged_lock_commitment": staged,
                "counts": counts,
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
        observations.append(
            {
                "peer_index": peer_index,
                "response_sha256": hashlib.sha256(response).hexdigest(),
                "response_hex": response.hex(),
                "height": index + 1,
                "commitment": _iroha_hash_literal(
                    f"{peer_index + index + 4:02X}" * 32
                ),
                "ledger_commitment": ledger,
                "replicated_staged_lock_commitment": replicated_staged,
                "staged_lock_commitment": staged,
                "counts": counts,
            }
        )
    return json.dumps(
        {
            "version": 1,
            "peer_index": peer_index,
            "registered": observations[1],
            "observations": observations,
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode()


def _write_restricted_archive(
    path: Path, rows: list[tuple[str, str, bytes]]
) -> dict[str, dict[str, Any]]:
    rows.sort(key=lambda row: (row[0], row[1]))
    body = bytearray(MODULE.LEAKAGE_RESTRICTED_SOURCE_DOMAIN_V1)
    body.extend(struct.pack("<I", len(rows)))
    grouped: dict[str, list[bytes]] = {}
    for ordinal, (surface, relative, source) in enumerate(rows):
        assert source
        grouped.setdefault(surface, []).append(source)
        encoded_surface = surface.encode("ascii")
        encoded_relative = relative.encode("utf-8")
        body.extend(struct.pack("<IH", ordinal, len(encoded_surface)))
        body.extend(encoded_surface)
        body.extend(struct.pack("<I", len(encoded_relative)))
        body.extend(encoded_relative)
        body.extend(struct.pack("<Q", len(source)))
        body.extend(source)
    path.write_bytes(bytes(body))
    path.chmod(0o600)
    return {surface: _source_binding(sources) for surface, sources in grouped.items()}


def leakage_payload(job: dict[str, Any], evidence: Path) -> dict[str, Any]:
    """Write one exact source-replayable leakage fixture."""

    peer_count = 16
    variant_marker = b"L" if job.get("variant", "left") == "left" else b"R"
    torii_ports = list(range(20_000, 20_000 + peer_count))
    participant_visibilities = MODULE.canonical_participant_visibilities(
        MODULE.PRIMARY_PARTICIPANTS
    )
    public_peer_count = (
        1
        + participant_visibilities.count(MODULE.PUBLIC_PARTICIPANT_VISIBILITY)
    ) * MODULE.VALIDATORS_PER_DATASPACE
    restricted_peer_count = participant_visibilities.count(
        MODULE.RESTRICTED_PARTICIPANT_VISIBILITY
    ) * MODULE.VALIDATORS_PER_DATASPACE
    public_ports = list(range(30_000, 30_000 + public_peer_count))
    restricted_ports = list(range(40_000, 40_000 + restricted_peer_count))
    request_packet = _ethernet_ipv4_tcp(50_000, torii_ports[0])
    response_packet = _ethernet_ipv4_tcp(torii_ports[0], 50_000)
    public_packet = _ethernet_ipv4_tcp(public_ports[0], 50_001)
    restricted_packet = _ethernet_ipv4_tcp(50_002, restricted_ports[0])
    all_packets = [
        request_packet,
        response_packet,
        public_packet,
        restricted_packet,
    ]
    raw_path = evidence / MODULE.SURFACE_FILES["restricted_packet_source"]
    _write_pcap(raw_path, all_packets)
    raw_binding = MODULE.file_binding(raw_path)
    ports = {
        "version": 1,
        "torii_ports": torii_ports,
        "public_p2p_ports": public_ports,
        "restricted_p2p_ports": restricted_ports,
    }
    groups = MODULE.capture_split.validate_port_manifest(ports)
    MODULE.capture_split.split_capture(raw_path, evidence, groups)
    block = b"opaque-canonical-block-" + variant_marker
    (evidence / MODULE.SURFACE_FILES["block_wire_capture"]).write_bytes(
        MODULE.LEAKAGE_BLOCK_WIRE_MAGIC_V1
        + struct.pack("<I", 1)
        + struct.pack("<Q", len(block))
        + block
    )
    digest = "5" * 64
    event_sources = [b"opaque-event-source"]
    event_records = [
        {
            "peer_index": 0,
            "source_sha256": hashlib.sha256(event_sources[0]).hexdigest(),
            "source_bytes": len(event_sources[0]),
        }
    ]
    query_sources = [f"opaque-query-{index:03}".encode() for index in range(peer_count)]
    query_records = [
        {
            "peer_index": index,
            "source_sha256": hashlib.sha256(query_sources[index]).hexdigest(),
            "source_bytes": len(query_sources[index]),
        }
        for index in range(peer_count)
    ]
    operator_sources = []
    log_records = []
    telemetry_sources = []
    telemetry_records = []
    for index in range(peer_count):
        stdout = f"validator-{index:03}-stdout".encode()
        stderr = b""
        operator_sources.append(
            struct.pack("<Q", len(stdout))
            + stdout
            + struct.pack("<Q", len(stderr))
            + stderr
        )
        log_records.append(
            {
                "peer_index": index,
                "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
                "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
                "stdout_bytes": len(stdout),
                "stderr_bytes": len(stderr),
            }
        )
        status = f"status-{index:03}".encode()
        metrics = f"metrics-{index:03}".encode()
        source = (
            struct.pack("<Q", len(status))
            + status
            + struct.pack("<Q", len(metrics))
            + metrics
        )
        telemetry_sources.append(source)
        telemetry_records.append(
            {
                "peer_index": index,
                "status_sha256": hashlib.sha256(status).hexdigest(),
                "status_bytes": len(status),
                "metrics_sha256": hashlib.sha256(metrics).hexdigest(),
                "metrics_bytes": len(metrics),
                "source_sha256": hashlib.sha256(source).hexdigest(),
                "source_bytes": len(source),
            }
        )
    for surface, records in (
        ("event_capture", event_records),
        ("query_capture", query_records),
        ("operator_log", log_records),
        ("telemetry_capture", telemetry_records),
    ):
        (evidence / MODULE.SURFACE_FILES[surface]).write_text(
            json.dumps({"version": 1, "records": records}, sort_keys=True),
            encoding="utf-8",
        )
    derivative_sources: dict[str, bytes] = {}
    for surface, kind in (
        ("kura_artifact", "kura"),
        ("merge_artifact", "merge"),
        ("snapshot_artifact", "snapshot"),
    ):
        source = f"opaque-source-{surface}-".encode() + variant_marker
        derivative_sources[surface] = source
        (evidence / MODULE.SURFACE_FILES[surface]).write_bytes(
            MODULE.LEAKAGE_ARTIFACT_FRAME_DOMAIN_V1
            + struct.pack("<H", len(kind))
            + kind.encode()
            + struct.pack("<I", 1)
            + struct.pack("<I", 0)
            + hashlib.sha256(f"path-{surface}".encode()).digest()
            + struct.pack("<Q", len(source))
            + hashlib.sha256(source).digest()
        )
    restricted_rows: list[tuple[str, str, bytes]] = [
        ("block_wire_capture", "carrier-block-wire.bin", (evidence / MODULE.SURFACE_FILES["block_wire_capture"]).read_bytes()),
        ("event_capture", "event-000.norito", event_sources[0]),
        (
            "coordinator_log",
            "coordinator-000/stdout-stderr.log",
            b"coordinator-log",
        ),
        ("confidential_da", "sidecar-000.bin", b"opaque-confidential-sidecar"),
    ]
    restricted_rows.extend(
        ("query_capture", f"peer-{index:03}.norito", source)
        for index, source in enumerate(query_sources)
    )
    restricted_rows.extend(
        ("operator_log", f"validator-{index:03}.stdout-stderr", source)
        for index, source in enumerate(operator_sources)
    )
    restricted_rows.extend(
        ("telemetry_capture", f"peer-{index:03}.status-metrics", source)
        for index, source in enumerate(telemetry_sources)
    )
    for surface, source in derivative_sources.items():
        restricted_rows.append((surface, f"path-{surface}", source))
    restricted_rows.extend(
        ("atomicity_observation", f"peer-{index:03}.json", _atomicity_evidence(index, 3))
        for index in range(peer_count)
    )
    restricted_path = evidence / MODULE.SURFACE_FILES["restricted_audit_source"]
    source_groups = _write_restricted_archive(restricted_path, restricted_rows)
    artifacts = []
    for surface in sorted(MODULE.SURFACE_FILES):
        path = evidence / MODULE.SURFACE_FILES[surface]
        packet = path.suffix == ".pcapng"
        if packet:
            source_claim = {
                "source_sha256": raw_binding["sha256"],
                "source_bytes": raw_binding["bytes"],
                "source_count": 1,
            }
        elif surface == "restricted_packet_source":
            source_claim = {
                "source_sha256": raw_binding["sha256"],
                "source_bytes": raw_binding["bytes"],
                "source_count": 1,
            }
        elif surface == "restricted_audit_source":
            source_claim = MODULE._single_file_source_binding(path)
        else:
            source_claim = source_groups[surface]
        artifacts.append(
            {
                "surface": surface,
                "relative_name": MODULE.SURFACE_FILES[surface],
                **MODULE.file_binding(path),
                **source_claim,
            }
        )
    packet_counts = {
        "sanitized_packets": 4,
        "torii_packets": 2,
        "public_p2p_packets": 1,
        "restricted_p2p_packets": 1,
        "torii_request_packets": 1,
        "torii_response_packets": 1,
    }
    port_binding = MODULE.capture_split.canonical_port_manifest_binding(
        groups
    )
    tcpdump_stderr = (
        b"tcpdump: listening on lo0\n"
        b"4 packets captured\n"
        b"4 packets received by filter\n"
        b"0 packets dropped by kernel\n"
    )
    return {
        "variant": job.get("variant", "left"),
        "canaries_injected": job["canary_names"],
        "canary_commitments": job["canary_commitments"],
        "only_secret_fields_changed": True,
        "capture_complete": True,
        "finalized_receipt_observed": True,
        "successful_leg_applications": 3,
        "each_leg_applied_exactly_once": True,
        "continuous_atomicity_checks": peer_count * 3,
        "partial_visible_observations": 0,
        "partial_spendable_observations": 0,
        "capture_provenance": {
            "raw_pcap": raw_binding,
            "port_manifest": port_binding,
            "ports": ports,
            "packet_counts": packet_counts,
            "tcpdump": {
                "stderr_base64": MODULE.base64.b64encode(tcpdump_stderr).decode("ascii"),
                "stderr_sha256": hashlib.sha256(tcpdump_stderr).hexdigest(),
                "stderr_bytes": len(tcpdump_stderr),
                "statistics": {
                    "captured_packets": 4,
                    "received_by_filter_packets": 4,
                    "drop_counters": {"kernel": 0},
                },
            },
        },
        "artifacts": artifacts,
        "traffic_counts": {
            "torii_request_packets": 1,
            "torii_response_packets": 1,
            "public_p2p_packets": 1,
            "restricted_p2p_packets": 1,
            "block_messages": 1,
            "query_responses": peer_count,
            "event_records": 1,
            "log_records": peer_count,
            "telemetry_records": peer_count,
        },
    }


def fault_job(participants: int = 3) -> dict[str, Any]:
    return {
        "request_id": "e" * 64,
        "invocation_nonce": INVOCATION_NONCE,
        "kind": "fault",
        "participants": participants,
        "seed": 7,
        "run": 2,
        "configuration_sha256": CONFIGURATION,
    }


def fault_payload(participants: int = 3) -> dict[str, Any]:
    """Return every required controller and persistence acknowledgement."""

    return {
        "committee_validator_restarts": list(range(participants)),
        "maximum_simultaneously_unavailable_per_committee": 1,
        "quorum_progress_with_one_unavailable": True,
        "coordinator_restarted": True,
        "global_node_restarted": True,
        "prepare_qc_normalization": {
            "first_signer_subset": [0, 1, 2],
            "second_signer_subset": [0, 1, 3],
            "certified_body_sha256": "1" * 64,
            "first_qc_sha256": "2" * 64,
            "second_qc_sha256": "3" * 64,
            "first_normalized_barrier_sha256": "4" * 64,
            "second_normalized_barrier_sha256": "4" * 64,
            "equivalent_subsets_accepted": True,
            "changed_body_rejected": True,
            "authority_index_binding_verified": True,
            "signed_body_binding_verified": True,
        },
        "loss_trials": [
            {
                "phase": phase,
                "loss_percent": percentage,
                "control_acknowledged": True,
                "healed": True,
                "converged": True,
                "partial_visibility_observed": False,
            }
            for phase in MODULE.fault_report.REQUIRED_LOSS_PHASES
            for percentage in MODULE.fault_report.REQUIRED_LOSS_PERCENTAGES
        ],
        "phase_cut_partitions": [
            {
                "cut": cut,
                "control_acknowledged": True,
                "delayed_delivery": True,
                "healed": True,
                "converged": True,
                "partial_visibility_observed": False,
            }
            for cut in MODULE.fault_report.REQUIRED_PHASE_CUTS
        ],
        "crash_recoveries": [
            {
                "boundary": boundary,
                "process_restarted": True,
                "durable_state_reconciled": True,
                "converged": True,
                "partial_visibility_observed": False,
            }
            for boundary in MODULE.fault_report.REQUIRED_CRASH_BOUNDARIES
        ],
        "atomicity": {
            "continuous_checks": 100,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
            "aborted_private_state_changes": 0,
            "successful_leg_applications": participants,
            "each_leg_applied_exactly_once": True,
            "invalid_leg_state_byte_identical": True,
            "replay_rejected": True,
        },
        "all_nodes_converged": True,
    }


def _canonical_occurrence(
    control_type: str,
    peer_index: int | None,
    command: dict[str, Any],
    acknowledgement: dict[str, Any] | None = None,
    *,
    restart: bool = False,
    before_pid: int = 500,
    after_pid: int = 501,
) -> dict[str, Any]:
    command_bytes = MODULE.canonical_bytes(command)
    command_sha = MODULE.hashlib.sha256(command_bytes).hexdigest()
    if acknowledgement is None:
        acknowledgement = dict(command)
    elif acknowledgement.pop("_bind_command", False):
        acknowledgement["command_sha256"] = command_sha
    acknowledgement_bytes = MODULE.canonical_bytes(acknowledgement)
    return {
        "control_type": control_type,
        "peer_index": peer_index,
        "command_sha256": command_sha,
        "command_hex": command_bytes.hex(),
        "acknowledgement_sha256": MODULE.hashlib.sha256(
            acknowledgement_bytes
        ).hexdigest(),
        "acknowledgement_hex": acknowledgement_bytes.hex(),
        "before_pid": before_pid if restart else None,
        "after_pid": after_pid if restart else None,
    }


def _restart_occurrence(
    control_type: str,
    peer_index: int,
    revision: int,
    operation: str,
    *,
    before_pid: int,
    after_pid: int,
) -> dict[str, Any]:
    acknowledgement_operation = (
        "validator_restarted_after_quorum_progress"
        if operation == "stop_validator_for_quorum_progress"
        else operation
    )
    return _canonical_occurrence(
        control_type,
        peer_index,
        {
            "format_version": 1,
            "revision": revision,
            "operation": operation,
            "peer_index": peer_index,
            "before_pid": before_pid,
        },
        {
            "format_version": 1,
            "revision": revision,
            "operation": acknowledgement_operation,
            "peer_index": peer_index,
            "before_pid": before_pid,
            "after_pid": after_pid,
            "health_observed": True,
            "_bind_command": True,
        },
        restart=True,
        before_pid=before_pid,
        after_pid=after_pid,
    )


def _coordinator_restart_occurrence(
    participants: int,
    revision: int,
    bundle_id: str,
    *,
    before_pid: int,
    after_pid: int,
) -> dict[str, Any]:
    return _canonical_occurrence(
        "coordinator_restart",
        None,
        {
            "format_version": 1,
            "revision": revision,
            "operation": "recover_prepare_commit",
            "committee_endpoints": [
                [f"http://127.0.0.1:{10_000 + dataspace * 4 + peer}" for peer in range(4)]
                for dataspace in range(participants)
            ],
            "manifest": {"bundle_id": _iroha_hash_literal(bundle_id)},
            "authority_catalog": [{} for _ in range(participants)],
            "deltas": [{} for _ in range(participants)],
            "barrier": None,
        },
        {
            "format_version": 1,
            "revision": revision,
            "pid": after_pid,
            "operation": "recover_prepare_commit",
            "barrier": {},
            "commit_certificates": [{} for _ in range(participants)],
            "_bind_command": True,
        },
        restart=True,
        before_pid=before_pid,
        after_pid=after_pid,
    )


def _route_occurrence(
    phase: str,
    action: str,
    revision: int,
    *,
    bundle_id: str,
    seed: int = 7,
    drop_first: int,
    match_limit: int,
    matched: int,
    passed: int,
    dropped: int,
    held: int,
    released: int,
    predecessor: str | None = None,
) -> dict[str, Any]:
    command = {
        "action": action,
        "bundle_id": bundle_id,
        "drop_first": drop_first,
        "format_version": 1,
        "match_limit": match_limit,
        "phase": phase,
        "revision": revision,
        "seed": seed,
    }
    command_bytes = MODULE.canonical_bytes(command)
    command_sha = MODULE.hashlib.sha256(command_bytes).hexdigest()
    acknowledgement = {
        "action": action,
        "bundle_id": bundle_id,
        "command_sha256": command_sha,
        "dropped": dropped,
        "format_version": 1,
        "held": held,
        "matched": matched,
        "passed": passed,
        "phase": phase,
        "predecessor_command_sha256": predecessor,
        "released": released,
        "request_digests": [f"{revision * 1000 + index + 1:064x}" for index in range(matched)],
        "revision": revision,
        "seed": seed,
    }
    acknowledgement_bytes = MODULE.canonical_bytes(acknowledgement)
    return {
        "control_type": phase,
        "peer_index": MODULE.VALIDATORS_PER_DATASPACE,
        "command_sha256": command_sha,
        "command_hex": command_bytes.hex(),
        "acknowledgement_sha256": MODULE.hashlib.sha256(
            acknowledgement_bytes
        ).hexdigest(),
        "acknowledgement_hex": acknowledgement_bytes.hex(),
        "before_pid": None,
        "after_pid": None,
    }


def _consensus_carrier_occurrence(
    peer_index: int, action: str, revision: int
) -> dict[str, Any]:
    drain = action == "heal"
    rules = (
        []
        if drain
        else [
            {
                "action": "hold",
                "height": 10,
                "kind": "proposal",
                "view": 0,
            }
        ]
    )
    command = {
        "drain": drain,
        "queue_capacity": 512 if drain else 256,
        "release": [],
        "revision": revision,
        "rules": rules,
        "version": 5,
    }
    command_bytes = MODULE.canonical_bytes(command)
    command_sha = MODULE.hashlib.sha256(command_bytes).hexdigest()
    sequence = peer_index + 1
    acknowledgement = {
        "command_digest": _iroha_hash_literal(command_sha),
        "delivered": [sequence] if drain else [],
        "dropped": 0,
        "drain_fence": revision if drain else None,
        "draining": False,
        "fatal": False,
        "held": [] if drain else [{"sequence": sequence}],
        "held_bytes": 0 if drain else 128,
        "in_flight": None,
        "in_flight_bytes": 0,
        "last_error": None,
        "overflowed": 0,
        "queue_capacity": command["queue_capacity"],
        "rejected_commands": 0,
        "release_pending": [],
        "retired": [],
        "revision": revision,
        "rules": rules,
        "version": 5,
    }
    acknowledgement_bytes = MODULE.canonical_bytes(acknowledgement)
    return {
        "control_type": "consensus_carrier",
        "peer_index": peer_index,
        "command_sha256": command_sha,
        "command_hex": command_bytes.hex(),
        "acknowledgement_sha256": MODULE.hashlib.sha256(
            acknowledgement_bytes
        ).hexdigest(),
        "acknowledgement_hex": acknowledgement_bytes.hex(),
        "before_pid": None,
        "after_pid": None,
    }


def _state_counts(
    participants: int,
    *,
    finalized: bool,
    replicated_staged: bool = False,
    local_staged: bool = False,
) -> dict[str, int]:
    counts = {field: 0 for field in MODULE.FAULT_STATE_COUNT_FIELDS}
    counts.update({"governance": participants, "pools": participants})
    if finalized:
        counts.update(
            {
                "roots": participants,
                "nullifiers": participants * 2,
                "commitments": participants * 3,
                "encrypted_outputs": participants * 3,
                "replay_markers": 1,
                "receipts": 1,
            }
        )
    if replicated_staged:
        counts["replicated_staged_locks"] = 1 + participants * 9
    if local_staged:
        counts.update(
            {
                "staged_pool_heads": 1,
                "staged_nullifiers": 2,
                "staged_output_commitments": 3,
                "staged_locks": 6,
            }
        )
    return counts


def _state_observation(
    participants: int,
    peer_index: int,
    *,
    label: str,
    finalized: bool,
    full_locks: bool,
) -> dict[str, Any]:
    replicated_staged = label == "nonfinalized" and full_locks
    local_staged = replicated_staged and peer_index >= MODULE.GLOBAL_VALIDATORS
    ledger = _iroha_hash_literal(("4" if finalized else "1") * 64)
    replicated_staged_commitment = _iroha_hash_literal(
        ("5" if replicated_staged else "6") * 64
    )
    staged_commitment = _iroha_hash_literal(
        ("3" if local_staged else "2") * 64
    )
    response = {
        "format_version": 1,
        "height": 10 + (1 if finalized else 0),
        "commitment": _iroha_hash_literal(f"{(peer_index % 9) + 1:064X}"),
        "ledger_commitment": ledger,
        "replicated_staged_lock_commitment": replicated_staged_commitment,
        "staged_lock_commitment": staged_commitment,
        "counts": _state_counts(
            participants,
            finalized=finalized,
            replicated_staged=replicated_staged,
            local_staged=local_staged,
        ),
    }
    response_bytes = MODULE.canonical_bytes(response)
    return {
        "peer_index": peer_index,
        "response_sha256": MODULE.hashlib.sha256(response_bytes).hexdigest(),
        "response_hex": response_bytes.hex(),
        **{
            field: response[field]
            for field in (
                "height",
                "commitment",
                "ledger_commitment",
                "replicated_staged_lock_commitment",
                "staged_lock_commitment",
                "counts",
            )
        },
    }


def _rewrite_state_observation(
    observation: dict[str, Any], **changes: Any
) -> None:
    """Rewrite a bound state response and its public projection in place."""

    response = json.loads(bytes.fromhex(observation["response_hex"]).decode())
    response.update(copy.deepcopy(changes))
    response_bytes = MODULE.canonical_bytes(response)
    observation.update(
        {
            "response_sha256": MODULE.hashlib.sha256(response_bytes).hexdigest(),
            "response_hex": response_bytes.hex(),
            **{
                field: copy.deepcopy(response[field])
                for field in (
                    "height",
                    "commitment",
                    "ledger_commitment",
                    "replicated_staged_lock_commitment",
                    "staged_lock_commitment",
                    "counts",
                )
            },
        }
    )


def _refresh_fault_observation_summary(
    summary: dict[str, Any], bundle_id: str
) -> None:
    """Recompute the real V1 attempt and response chains for a test summary."""

    peer_index = summary["peer_index"]
    response_chain = MODULE.hashlib.sha256()
    response_chain.update(MODULE.FAULT_CONTINUOUS_OBSERVATION_DOMAIN_V1)
    response_chain.update(bytes.fromhex(bundle_id))
    response_chain.update(MODULE.struct.pack("<Q", peer_index))
    first_response = None
    last_response = None
    total_successes = 0
    total_failures = 0
    total_baseline = 0
    total_finalized = 0
    for phase_index, phase in enumerate(summary["phase_coverage"]):
        attempt_chain = MODULE.hashlib.sha256()
        attempt_chain.update(MODULE.FAULT_CONTINUOUS_OBSERVATION_PHASE_DOMAIN_V1)
        attempt_chain.update(bytes.fromhex(bundle_id))
        attempt_chain.update(MODULE.struct.pack("<Q", peer_index))
        attempt_chain.update(MODULE.struct.pack("<Q", phase_index))
        phase_name = phase["phase"].encode("ascii")
        attempt_chain.update(MODULE.struct.pack("<Q", len(phase_name)))
        attempt_chain.update(phase_name)
        attempt_chain.update(bytes((int(phase["expected_unavailable"]),)))
        attempt_chain.update(bytes((int(phase["finalization_allowed"]),)))
        phase_successes = 0
        phase_failures = 0
        phase_baseline = 0
        phase_finalized = 0
        for attempt in phase["attempts"]:
            for _ in range(attempt["repetitions"]):
                attempt_class = attempt["class"]
                if attempt_class == "expected_unavailable":
                    phase_failures += 1
                    attempt_chain.update(b"\x00")
                    attempt_chain.update(
                        MODULE.FAULT_CONTINUOUS_EXPECTED_UNAVAILABLE_CLASS_V1.encode(
                            "ascii"
                        )
                    )
                    continue
                response_digest = MODULE.hashlib.sha256(
                    bytes.fromhex(attempt["evidence"])
                ).digest()
                phase_successes += 1
                if attempt_class == "baseline":
                    phase_baseline += 1
                    attempt_chain.update(b"\x01")
                elif attempt_class == "finalized":
                    phase_finalized += 1
                    attempt_chain.update(b"\x02")
                else:
                    raise AssertionError(f"unknown fixture attempt class {attempt_class}")
                attempt_chain.update(response_digest)
                response_chain.update(response_digest)
                response_hex = response_digest.hex()
                first_response = first_response or response_hex
                last_response = response_hex
        checkpoint = phase["checkpoint_attempt"]
        bindings = phase["checkpoint_control_bindings"]
        attempt_chain.update(b"checkpoint\0")
        attempt_chain.update(MODULE.struct.pack("<Q", checkpoint))
        attempt_chain.update(b"checkpoint-controls\0")
        attempt_chain.update(MODULE.struct.pack("<Q", len(bindings)))
        for binding in bindings:
            encoded = binding.encode("ascii")
            attempt_chain.update(MODULE.struct.pack("<Q", len(encoded)))
            attempt_chain.update(encoded)
        phase.update(
            {
                "successful_observations": phase_successes,
                "poll_failures": phase_failures,
                "baseline_observations": phase_baseline,
                "finalized_observations": phase_finalized,
                "attempt_chain_sha256": attempt_chain.hexdigest(),
            }
        )
        total_successes += phase_successes
        total_failures += phase_failures
        total_baseline += phase_baseline
        total_finalized += phase_finalized
    assert first_response is not None and last_response is not None
    summary.update(
        {
            "check_count": total_successes,
            "poll_failure_count": total_failures,
            "first_response_sha256": first_response,
            "last_response_sha256": last_response,
            "response_chain_sha256": response_chain.hexdigest(),
            "baseline_observations": total_baseline,
            "finalized_observations": total_finalized,
        }
    )


def _refresh_fault_observation_row(row: dict[str, Any]) -> None:
    """Recompute all continuous summary bindings in a test observation row."""

    for summary in row["continuous_observations"]:
        _refresh_fault_observation_summary(summary, row["bundle_id"])
    row["continuous_checks"] = sum(
        summary["check_count"] for summary in row["continuous_observations"]
    )


def write_fault_evidence(
    evidence_dir: Path,
    payload: dict[str, Any],
    *,
    participants: int = 3,
    seed: int = 7,
    run: int = 2,
) -> None:
    controls: list[dict[str, Any]] = []
    observations: list[dict[str, Any]] = []
    revision = 1
    peer_count = (participants + 1) * MODULE.VALIDATORS_PER_DATASPACE
    collections = ("loss_trials", "phase_cut_partitions", "crash_recoveries")
    crash_phases = {
        "sidecar_fsync": "after_private_settlement_sidecar_fsync",
        "staged_delta_fsync": "after_private_settlement_staged_delta_fsync",
        "prepare_qc": "after_private_settlement_prepare_qc_fsync",
        "prepare_registration_kura_append": "after_private_settlement_kura_append",
        "prepare_registration_wsv_application": "after_private_settlement_wsv_application",
        "commit_qc": "after_private_settlement_commit_qc_fsync",
        "finalization_kura_append": "after_private_settlement_kura_append",
        "finalization_wsv_application": "after_private_settlement_wsv_application",
        "receipt_publication": "after_private_settlement_receipt_publication",
    }
    total_checks = 0
    for collection in collections:
        for index, trial in enumerate(payload[collection]):
            record_id = f"n{participants}:s{seed}:r{run}:{collection}:{index}"
            bundle_id = MODULE.hashlib.sha256(record_id.encode()).hexdigest()
            trial_controls: list[dict[str, Any]] = []
            expected_after_state = (
                "reverted" if collection == "crash_recoveries" else "finalized"
            )
            if collection == "loss_trials":
                dropped = trial["loss_percent"] // 5
                trial_controls.append(
                    _route_occurrence(
                        trial["phase"],
                        "loss",
                        revision,
                        bundle_id=bundle_id,
                        seed=seed,
                        drop_first=dropped,
                        match_limit=20,
                        matched=20,
                        passed=20 - dropped,
                        dropped=dropped,
                        held=0,
                        released=0,
                    )
                )
                revision += 1
                trial_controls.append(
                    _route_occurrence(
                        trial["phase"],
                        "pass",
                        revision,
                        bundle_id=bundle_id,
                        seed=seed,
                        drop_first=0,
                        match_limit=0,
                        matched=1,
                        passed=1,
                        dropped=0,
                        held=0,
                        released=0,
                    )
                )
                revision += 1
            elif collection == "phase_cut_partitions" and trial["cut"] != "carrier_before_global_finality":
                phase = {
                    "da_before_availability_qc": "restricted_da",
                    "prepare_before_complete_barrier": "prepare",
                    "commit_before_complete_barrier": "commit",
                }[trial["cut"]]
                hold = _route_occurrence(
                    phase,
                    "hold",
                    revision,
                    bundle_id=bundle_id,
                    seed=seed,
                    drop_first=0,
                    match_limit=1,
                    matched=1,
                    passed=0,
                    dropped=0,
                    held=1,
                    released=0,
                )
                revision += 1
                trial_controls.append(hold)
                trial_controls.append(
                    _route_occurrence(
                        phase,
                        "pass",
                        revision,
                        bundle_id=bundle_id,
                        seed=seed,
                        drop_first=0,
                        match_limit=0,
                        matched=1,
                        passed=0,
                        dropped=0,
                        held=1,
                        released=1,
                        predecessor=hold["command_sha256"],
                    )
                )
                revision += 1
            elif collection == "phase_cut_partitions":
                for dataspace_ordinal in range(participants):
                    before_pid = 600 + dataspace_ordinal * 2
                    trial_controls.append(
                        _restart_occurrence(
                            "validator_restart",
                            4 + dataspace_ordinal * 4,
                            revision,
                            "stop_validator_for_quorum_progress",
                            before_pid=before_pid,
                            after_pid=before_pid + 1,
                        )
                    )
                    revision += 1
                trial_controls.append(
                    _restart_occurrence(
                        "global_restart",
                        0,
                        revision,
                        "restart_validator",
                        before_pid=700,
                        after_pid=701,
                    )
                )
                revision += 1
                trial_controls.append(
                    _coordinator_restart_occurrence(
                        participants,
                        revision,
                        bundle_id,
                        before_pid=702,
                        after_pid=703,
                    )
                )
                revision += 1
                for carrier_action in ("hold", "heal"):
                    for peer_index in range(MODULE.GLOBAL_VALIDATORS):
                        trial_controls.append(
                            _consensus_carrier_occurrence(
                                peer_index, carrier_action, revision
                            )
                        )
                        revision += 1
            else:
                boundary = trial["boundary"]
                phase = crash_phases[boundary]
                global_boundary = boundary in {
                    "prepare_registration_kura_append",
                    "prepare_registration_wsv_application",
                    "finalization_kura_append",
                    "finalization_wsv_application",
                }
                target_peer = 0 if global_boundary else 4
                restart_type = (
                    "global_restart" if global_boundary else "validator_restart"
                )
                cut = {
                    "version": 1,
                    "revision": revision,
                    "phase": phase,
                    "source_id": bundle_id,
                }
                trial_controls.append(
                    _canonical_occurrence("persistence_cut", target_peer, cut)
                )
                revision += 1
                trial_controls.append(
                    _restart_occurrence(
                        restart_type,
                        target_peer,
                        revision,
                        "recover_crashed_validator",
                        before_pid=800 + index * 2,
                        after_pid=801 + index * 2,
                    )
                )
                revision += 1
                if boundary in {
                    "prepare_registration_kura_append",
                    "prepare_registration_wsv_application",
                    "finalization_kura_append",
                    "finalization_wsv_application",
                    "receipt_publication",
                }:
                    expected_after_state = "finalized"
            control_row = {
                "record": record_id,
                "bundle_id": bundle_id,
                "participants": participants,
                "seed": seed,
                "run": run,
                "collection": collection,
                "trial_index": index,
                "controls": trial_controls,
            }
            controls.append(control_row)
            phase_contract = MODULE._fault_observation_phase_contract(
                control_row,
                collection=collection,
                label=f"fixture.{collection}[{index}]",
            )
            snapshots = []
            full_lock_boundary = collection != "crash_recoveries" or trial[
                "boundary"
            ] not in {
                "sidecar_fsync",
                "staged_delta_fsync",
                "prepare_qc",
            }
            for label in ("before", "nonfinalized", "after"):
                finalized = label == "after" and expected_after_state == "finalized"
                snapshots.append(
                    {
                        "label": label,
                        "validators": [
                            _state_observation(
                                participants,
                                peer_index,
                                label=label,
                                finalized=finalized,
                                full_locks=label == "nonfinalized"
                                and full_lock_boundary,
                            )
                            for peer_index in range(peer_count)
                        ],
                    }
                )
            continuous_observations = []
            for peer_index in range(peer_count):
                baseline_response = snapshots[0]["validators"][peer_index][
                    "response_hex"
                ]
                final_response = snapshots[2]["validators"][peer_index][
                    "response_hex"
                ]
                phase_coverage = []
                for (
                    phase_name,
                    expected_unavailable_peers,
                    finalization_allowed,
                    checkpoint_bindings,
                ) in phase_contract:
                    expected_unavailable = peer_index in expected_unavailable_peers
                    if expected_unavailable:
                        attempts = [
                            {
                                "class": "expected_unavailable",
                                "evidence": MODULE.FAULT_CONTINUOUS_EXPECTED_UNAVAILABLE_CLASS_V1,
                                "repetitions": 1,
                            }
                        ]
                    else:
                        terminal_finalized = (
                            phase_name == "terminal"
                            and expected_after_state == "finalized"
                        )
                        attempts = [
                            {
                                "class": (
                                    "finalized" if terminal_finalized else "baseline"
                                ),
                                "evidence": (
                                    final_response
                                    if terminal_finalized
                                    else baseline_response
                                ),
                                "repetitions": 2 if phase_name == "preflight" else 1,
                            }
                        ]
                    phase_coverage.append(
                        {
                            "phase": phase_name,
                            "expected_unavailable": expected_unavailable,
                            "finalization_allowed": finalization_allowed,
                            "successful_observations": 0,
                            "poll_failures": 0,
                            "baseline_observations": 0,
                            "finalized_observations": 0,
                            "checkpoint_attempt": 0,
                            "checkpoint_control_bindings": list(
                                checkpoint_bindings
                            ),
                            "attempt_chain_sha256": "",
                            "attempts": attempts,
                        }
                    )
                summary = {
                    "peer_index": peer_index,
                    "phase_coverage": phase_coverage,
                }
                _refresh_fault_observation_summary(summary, bundle_id)
                continuous_observations.append(summary)
            continuous_checks = sum(
                summary["check_count"] for summary in continuous_observations
            )
            observations.append(
                {
                    "record": record_id,
                    "bundle_id": bundle_id,
                    "participants": participants,
                    "seed": seed,
                    "run": run,
                    "collection": collection,
                    "trial_index": index,
                    "expected_after_state": expected_after_state,
                    "continuous_checks": continuous_checks,
                    "continuous_observations": continuous_observations,
                    "partial_visibility_observed": False,
                    "partial_spendable_observations": 0,
                    "snapshots": snapshots,
                }
            )
            total_checks += continuous_checks

    control_path = evidence_dir / MODULE.FAULT_CONTROL_EVIDENCE_FILE
    observation_path = evidence_dir / MODULE.FAULT_OBSERVATION_EVIDENCE_FILE
    control_path.write_bytes(b"".join(MODULE.canonical_bytes(row) + b"\n" for row in controls))
    observation_path.write_bytes(
        b"".join(MODULE.canonical_bytes(row) + b"\n" for row in observations)
    )
    control_sha = MODULE.hashlib.sha256(control_path.read_bytes()).hexdigest()
    observation_sha = MODULE.hashlib.sha256(observation_path.read_bytes()).hexdigest()
    payload["atomicity"]["continuous_checks"] = total_checks
    for collection in collections:
        for index, trial in enumerate(payload[collection]):
            record_id = f"n{participants}:s{seed}:r{run}:{collection}:{index}"
            trial.update(
                {
                    "control_transcript_sha256": control_sha,
                    "control_transcript_record": record_id,
                    "observation_capture_sha256": observation_sha,
                    "observation_capture_record": record_id,
                }
            )


def response(job: dict[str, Any], payload: dict[str, Any]) -> dict[str, Any]:
    """Wrap a job-specific payload in the exact process-harness envelope."""

    return {
        "version": MODULE.VERSION,
        "protocol": MODULE.PROTOCOL,
        "request_id": job["request_id"],
        "invocation_nonce": job["invocation_nonce"],
        "kind": job["kind"],
        "commit": COMMIT,
        "hardware_sha256": HARDWARE,
        "hardware_profile_sha256": HARDWARE_PROFILE,
        "configuration_sha256": job["configuration_sha256"],
        "participants": job["participants"],
        "passed": True,
        "mandatory_signed_rs16_da_rbc": True,
        "signed_rs16_da_observations": (
            MODULE.minimum_signed_rs16_da_observations(job["participants"])
        ),
        "authenticated_message_control": True,
        "process_inventory": process_inventory(job["participants"]),
        "payload": payload,
    }


class CanonicalHashLiteralTests(unittest.TestCase):
    """Check canonical hash checksums against independent bitwise fixtures."""

    def test_fixed_checksum_vectors_preserve_literal_bytes(self) -> None:
        vectors = (
            ("0" * 64, "D52F"),
            ("F" * 64, "C6C0"),
            ("0123456789ABCDEF" * 4, "EFDA"),
            ("4" * 64, "B84D"),
        )
        for body, checksum in vectors:
            with self.subTest(body=body):
                literal = f"hash:{body}#{checksum}"
                self.assertEqual(_iroha_hash_literal(body), literal)
                self.assertEqual(
                    MODULE.canonical_iroha_hash_body(literal, "hash_test"),
                    body.lower(),
                )

    def test_standard_library_crc_matches_independent_bit_loop(self) -> None:
        # Keep _iroha_hash_literal's independent polynomial bit loop: using
        # the production helper to construct these literals would hide drift.
        for offset in range(32):
            for value in range(256):
                body_bytes = bytearray(32)
                body_bytes[offset] = value
                body = body_bytes.hex()
                with self.subTest(offset=offset, value=value):
                    self.assertEqual(
                        MODULE.canonical_iroha_hash_body(
                            _iroha_hash_literal(body), "hash_test"
                        ),
                        body,
                    )
        for index in range(256):
            body = hashlib.sha256(index.to_bytes(2, "big")).hexdigest()
            with self.subTest(dense_body=index):
                self.assertEqual(
                    MODULE.canonical_iroha_hash_body(
                        _iroha_hash_literal(body), "hash_test"
                    ),
                    body,
                )

    def test_malformed_syntax_and_checksum_corruption_fail_closed(self) -> None:
        body = "0123456789ABCDEF" * 4
        literal = f"hash:{body}#EFDA"
        malformed = (
            None,
            True,
            1,
            [],
            {},
            literal.encode("ascii"),
            literal.lower(),
            literal[:-4] + "efda",
            literal.replace("hash:", "Hash:"),
            literal.replace("hash:", "hash"),
            literal.replace("#", ":"),
            "hash:#EFDA",
            f"hash:{body[:-1]}#EFDA",
            f"hash:{body}0#EFDA",
            f"hash:{body[:-1]}G#EFDA",
            f"hash:{body[:-1]}Ｆ#EFDA",
            literal[:-1],
            literal + "0",
            literal[:-1] + "G",
            " " + literal,
            literal + " ",
            literal + "\n",
            literal + "\0",
        )
        for value in malformed:
            with self.subTest(value=value):
                with self.assertRaisesRegex(
                    MODULE.RunnerError,
                    "hash_test is not a canonical Iroha hash literal",
                ):
                    MODULE.canonical_iroha_hash_body(value, "hash_test")
        for bit in range(16):
            corrupted = literal[:-4] + f"{0xEFDA ^ (1 << bit):04X}"
            with self.subTest(checksum_bit=bit), self.assertRaisesRegex(
                MODULE.RunnerError, "hash_test has an invalid Iroha hash checksum"
            ):
                MODULE.canonical_iroha_hash_body(corrupted, "hash_test")
        for offset in range(64):
            replacement = "1" if body[offset] == "0" else "0"
            corrupted = f"hash:{body[:offset]}{replacement}{body[offset + 1:]}#EFDA"
            with self.subTest(body_offset=offset), self.assertRaisesRegex(
                MODULE.RunnerError, "hash_test has an invalid Iroha hash checksum"
            ):
                MODULE.canonical_iroha_hash_body(corrupted, "hash_test")


class PrivateSettlementReleaseRunnerTests(unittest.TestCase):
    """Exercise deterministic planning and fail-closed response materialization."""

    def test_smoke_prerequisite_binds_gate_and_execution_source(self) -> None:
        import private_settlement_smoke_campaign as smoke

        campaign = {
            "source": {"commit": COMMIT, "tree": "1" * 40, "source_sha256": "2" * 64},
            "artifacts": {"validator": {"sha256": "3" * 64}, "integration": {"sha256": "4" * 64}},
        }
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            MODULE.write_json(root / "campaign.json", campaign)
            with mock.patch.object(smoke, "validate_campaign", return_value=campaign) as validate, mock.patch.object(
                smoke, "source_seal", return_value=campaign["source"]
            ) as seal:
                receipt = MODULE.validate_smoke_prerequisite(root, source_root=root, commit=COMMIT)
            validate.assert_called_once_with(root, expected_commit=COMMIT)
            seal.assert_called_once_with(root, COMMIT)
            self.assertEqual(receipt["campaign_sha256"], MODULE.file_binding(root / "campaign.json")["sha256"])
            self.assertEqual(receipt["runs"], 10)
            self.assertNotIn(str(root), MODULE.canonical_bytes(receipt).decode())

            with mock.patch.object(smoke, "validate_campaign", return_value=campaign), mock.patch.object(
                smoke, "source_seal", return_value={**campaign["source"], "source_sha256": "5" * 64}
            ), self.assertRaisesRegex(MODULE.RunnerError, "execution source differs"):
                MODULE.validate_smoke_prerequisite(root, source_root=root, commit=COMMIT)

            def substitute_after_validation(*_args: Any) -> dict[str, Any]:
                MODULE.write_json(root / "campaign.json", {**campaign, "substituted": True})
                return campaign["source"]

            with mock.patch.object(smoke, "validate_campaign", return_value=campaign), mock.patch.object(
                smoke, "source_seal", side_effect=substitute_after_validation
            ), self.assertRaisesRegex(MODULE.RunnerError, "changed after validation"):
                MODULE.validate_smoke_prerequisite(root, source_root=root, commit=COMMIT)

    def test_release_execution_rejects_missing_smoke_before_starting_work(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source, output = root / "source", root / "new-parent" / "output"
            source.mkdir()
            frozen_plan = {"commit": COMMIT, "jobs": [], "harness": {"sha256": EXECUTABLE}}
            with mock.patch.object(MODULE, "load_plan", return_value=(frozen_plan, root)), mock.patch.object(
                MODULE, "validate_campaign_timeout"
            ), mock.patch.object(MODULE, "file_binding", return_value={}), mock.patch.object(
                MODULE, "verify_source_checkout"
            ), mock.patch.object(MODULE, "verify_harness", return_value=frozen_plan["harness"]), mock.patch.object(
                MODULE, "invoke_harness"
            ) as invoke, self.assertRaisesRegex(MODULE.RunnerError, "smoke prerequisite failed"):
                MODULE.execute_plan(
                    root / "plan.json", output, source_root=source,
                    harness=root / "harness", smoke_campaign=root / "missing-smoke",
                    timeout_seconds=7_200,
                )
            invoke.assert_not_called()
            self.assertFalse(output.parent.exists())

    def test_release_execution_cli_requires_explicit_smoke_evidence(self) -> None:
        arguments = ["execute", "--plan", "/plan", "--output-dir", "/output",
                     "--source-root", "/source", "--harness", "/harness"]
        with mock.patch("sys.stderr", new_callable=io.StringIO) as error, self.assertRaises(SystemExit) as result:
            MODULE.parse_args(arguments)
        self.assertEqual(result.exception.code, 2)
        self.assertIn("--smoke-campaign", error.getvalue())
        self.assertEqual(
            MODULE.parse_args(arguments + ["--smoke-campaign", "/smoke"]).smoke_campaign, Path("/smoke")
        )

    def test_job_matrix_is_complete_canonical_and_deterministic(self) -> None:
        canaries = MODULE.build_canary_manifest(COMMIT)
        configurations = {
            participants: f"{participants:064x}"
            for participants in MODULE.PARTICIPANTS
        }
        first = MODULE.build_jobs(
            configurations,
            tuple(range(10)),
            MODULE.MIN_WARMUPS,
            MODULE.MIN_MEASURED,
            canaries,
        )
        second = MODULE.build_jobs(
            configurations,
            tuple(range(10)),
            MODULE.MIN_WARMUPS,
            MODULE.MIN_MEASURED,
            canaries,
        )
        expected = (
            len(MODULE.PARTICIPANTS) * 10
            + len(MODULE.PROFILES)
            * len(MODULE.PARTICIPANTS)
            * (MODULE.MIN_WARMUPS + MODULE.MIN_MEASURED)
            + 2
        )
        self.assertEqual(first, second)
        self.assertEqual(len(first), expected)
        self.assertEqual(len({job["request_id"] for job in first}), expected)
        self.assertEqual(first[0]["kind"], "fault")
        self.assertEqual(first[-2]["variant"], "left")
        self.assertEqual(first[-1]["variant"], "right")

    def test_participant_visibility_profiles_are_canonical_and_deterministic(self) -> None:
        for participants in MODULE.PARTICIPANTS:
            expected = ["public"] + ["restricted"] * (participants - 1)
            self.assertEqual(
                MODULE.canonical_participant_visibilities(participants), expected
            )
            configuration = MODULE.build_configuration(
                participants,
                seeds=tuple(range(MODULE.MIN_FAULT_SEEDS)),
                warmups=MODULE.MIN_WARMUPS,
                measured=MODULE.MIN_MEASURED,
            )
            self.assertEqual(configuration["participant_visibilities"], expected)
        self.assertEqual(
            MODULE.canonical_participant_visibilities(3),
            ["public", "restricted", "restricted"],
        )
        primary = MODULE.canonical_participant_visibilities(3)
        self.assertEqual(
            (1 + primary.count(MODULE.PUBLIC_PARTICIPANT_VISIBILITY))
            * MODULE.VALIDATORS_PER_DATASPACE,
            8,
        )
        self.assertEqual(
            primary.count(MODULE.RESTRICTED_PARTICIPANT_VISIBILITY)
            * MODULE.VALIDATORS_PER_DATASPACE,
            8,
        )
        with self.assertRaisesRegex(MODULE.RunnerError, "visibility policy"):
            MODULE.canonical_participant_visibilities(255)

    def test_canary_sets_cover_both_secret_only_variants(self) -> None:
        manifest = MODULE.build_canary_manifest(COMMIT)
        names = [entry["name"] for entry in manifest["canaries"]]
        self.assertEqual(names, sorted(names))
        self.assertTrue(
            set(MODULE.release_evidence.REQUIRED_LEAKAGE_CANARY_NAMES).issubset(names)
        )
        left = MODULE.canaries_for_variant(manifest, "left")
        right = MODULE.canaries_for_variant(manifest, "right")
        self.assertEqual(len(left), 6)
        self.assertEqual(len(right), 6)
        self.assertTrue(
            all(
                MODULE.object_digest(a) != MODULE.object_digest(b)
                for a, b in zip(left, right)
            )
        )
        left_by_name = {entry["name"]: entry["value"] for entry in left}
        right_by_name = {
            entry["name"].removesuffix("_variant_b"): entry["value"]
            for entry in right
        }
        self.assertEqual(left_by_name["account_id"], MODULE.LEAKAGE_ACCOUNT_LEFT_I105)
        self.assertEqual(right_by_name["account_id"], MODULE.LEAKAGE_ACCOUNT_RIGHT_I105)
        self.assertEqual(left_by_name["asset_id"], MODULE.LEAKAGE_ASSET_LEFT)
        self.assertEqual(right_by_name["asset_id"], MODULE.LEAKAGE_ASSET_RIGHT)
        self.assertLess(left_by_name["amount"] + 12, 1 << 120)
        self.assertLess(right_by_name["amount"] + 12, 1 << 120)

    def test_process_inventory_must_name_every_real_validator_and_coordinator(
        self,
    ) -> None:
        MODULE.validate_process_inventory(
            process_inventory(3), participants=3, commit=COMMIT, label="fixture"
        )
        missing = process_inventory(3)[:-1]
        with self.assertRaisesRegex(MODULE.RunnerError, "process topology mismatch"):
            MODULE.validate_process_inventory(
                missing, participants=3, commit=COMMIT, label="fixture"
            )
        duplicate_pid = process_inventory(3)
        duplicate_pid[-1]["pid"] = duplicate_pid[0]["pid"]
        with self.assertRaisesRegex(MODULE.RunnerError, "reuses PID"):
            MODULE.validate_process_inventory(
                duplicate_pid, participants=3, commit=COMMIT, label="fixture"
            )
        reordered = process_inventory(3)
        reordered[1], reordered[2] = reordered[2], reordered[1]
        with self.assertRaisesRegex(MODULE.RunnerError, "reordered"):
            MODULE.validate_process_inventory(
                reordered, participants=3, commit=COMMIT, label="fixture"
            )

    def test_common_response_requires_freshness_and_validator_scaled_da(self) -> None:
        job = fault_job()
        valid = response(job, fault_payload())
        MODULE.validate_common_response(valid, plan=plan(), job=job)
        stale = copy.deepcopy(valid)
        stale["invocation_nonce"] = "7" * 64
        with self.assertRaisesRegex(MODULE.RunnerError, "frozen request"):
            MODULE.validate_common_response(stale, plan=plan(), job=job)
        insufficient_da = copy.deepcopy(valid)
        insufficient_da["signed_rs16_da_observations"] -= 1
        with self.assertRaisesRegex(MODULE.RunnerError, "cover every validator"):
            MODULE.validate_common_response(
                insufficient_da, plan=plan(), job=job
            )

    def test_fault_response_materializes_reporter_valid_bound_evidence(self) -> None:
        job = fault_job()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            publication = root / "publication"
            evidence.mkdir()
            publication.mkdir()
            payload = fault_payload()
            write_fault_evidence(evidence, payload)
            result = response(job, payload)
            raw, artifacts = MODULE.materialize_fault_response(
                result,
                plan=plan(),
                job=job,
                evidence_dir=evidence,
                publication_root=publication,
            )
            parsed = MODULE.fault_report.parse_run(raw, "fixture")
            self.assertEqual(parsed[:3], (3, 7, 2))
            self.assertEqual(
                {artifact["kind"] for artifact in artifacts},
                {"operator_log", "sanitized_capture"},
            )
            for artifact in artifacts:
                self.assertTrue((publication / artifact["path"]).is_file())

    def test_fault_continuous_observer_summaries_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            payload = fault_payload()
            write_fault_evidence(evidence, payload)
            rows, _binding = MODULE.read_bound_jsonl_file(
                evidence / MODULE.FAULT_OBSERVATION_EVIDENCE_FILE,
                "fault observations",
            )
            control_rows, _control_binding = MODULE.read_bound_jsonl_file(
                evidence / MODULE.FAULT_CONTROL_EVIDENCE_FILE,
                "fault controls",
            )
            control_by_record = MODULE.validate_fault_control_records(
                control_rows, participants=3, seed=7, run=2
            )

            def validate_observations(candidate: list[dict[str, Any]]) -> None:
                MODULE.validate_fault_observation_records(
                    candidate,
                    participants=3,
                    seed=7,
                    run=2,
                    control_by_record=control_by_record,
                )

            too_short = copy.deepcopy(rows)
            too_short[0]["continuous_observations"][0]["check_count"] = 2
            with self.assertRaisesRegex(MODULE.RunnerError, "lacks a live polling"):
                validate_observations(too_short)
            missing_phase = copy.deepcopy(rows)
            missing_phase[0]["continuous_observations"][0]["phase_coverage"].pop()
            with self.assertRaisesRegex(MODULE.RunnerError, "incomplete phase coverage"):
                validate_observations(missing_phase)
            carrier_row = next(
                row
                for row in rows
                if row["collection"] == "phase_cut_partitions"
                and row["trial_index"] == 3
            )
            wrong_exemption = copy.deepcopy(rows)
            wrong_exemption_row = next(
                row
                for row in wrong_exemption
                if row["record"] == carrier_row["record"]
            )
            committee_phase = next(
                phase
                for phase in wrong_exemption_row["continuous_observations"][1][
                    "phase_coverage"
                ]
                if phase["phase"] == "committee_unavailable"
            )
            committee_phase["expected_unavailable"] = True
            with self.assertRaisesRegex(
                MODULE.RunnerError, "contradicts authenticated controls"
            ):
                validate_observations(wrong_exemption)
            missing_outage = copy.deepcopy(rows)
            missing_outage_row = next(
                row
                for row in missing_outage
                if row["record"] == carrier_row["record"]
            )
            expected_peer = missing_outage_row["continuous_observations"][4]
            expected_phase = next(
                phase
                for phase in expected_peer["phase_coverage"]
                if phase["phase"] == "committee_unavailable"
            )
            expected_phase["attempts"] = [
                {
                    "class": "baseline",
                    "evidence": missing_outage_row["snapshots"][0]["validators"][4][
                        "response_hex"
                    ],
                    "repetitions": 1,
                }
            ]
            _refresh_fault_observation_row(missing_outage_row)
            with self.assertRaisesRegex(MODULE.RunnerError, "did not observe.*outage"):
                validate_observations(missing_outage)
            unexpected_failure = copy.deepcopy(rows)
            unexpected_failure_row = next(
                row
                for row in unexpected_failure
                if row["record"] == carrier_row["record"]
            )
            available_peer = unexpected_failure_row["continuous_observations"][1]
            available_phase = next(
                phase
                for phase in available_peer["phase_coverage"]
                if phase["phase"] == "committee_unavailable"
            )
            available_phase["attempts"] = [
                {
                    "class": "expected_unavailable",
                    "evidence": MODULE.FAULT_CONTINUOUS_EXPECTED_UNAVAILABLE_CLASS_V1,
                    "repetitions": 1,
                }
            ]
            _refresh_fault_observation_row(unexpected_failure_row)
            with self.assertRaisesRegex(
                MODULE.RunnerError, "unallowlisted poll failure"
            ):
                validate_observations(unexpected_failure)
            raw_poll_error = copy.deepcopy(rows)
            raw_poll_error[0]["continuous_observations"][0]["phase_coverage"][0][
                "error"
            ] = "private endpoint error"
            with self.assertRaisesRegex(MODULE.RunnerError, "unknown=\\['error'\\]"):
                validate_observations(raw_poll_error)
            forged_chain = copy.deepcopy(rows)
            forged_chain[0]["continuous_observations"][0]["phase_coverage"][0][
                "attempt_chain_sha256"
            ] = "f" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "attempt stream does not bind"):
                validate_observations(forged_chain)
            inflated_stream = copy.deepcopy(rows)
            inflated_stream[0]["continuous_observations"][0]["phase_coverage"][0][
                "attempts"
            ][0]["repetitions"] += 1
            with self.assertRaisesRegex(MODULE.RunnerError, "attempt stream does not bind"):
                validate_observations(inflated_stream)
            deleted_stream = copy.deepcopy(rows)
            deleted_stream[0]["continuous_observations"][0]["phase_coverage"][0][
                "attempts"
            ] = []
            with self.assertRaisesRegex(MODULE.RunnerError, "no ordered phase attempts"):
                validate_observations(deleted_stream)
            adjacent_rle = copy.deepcopy(rows)
            adjacent_phase = adjacent_rle[0]["continuous_observations"][0][
                "phase_coverage"
            ][0]
            adjacent_phase["attempts"] = [
                {
                    **copy.deepcopy(adjacent_phase["attempts"][0]),
                    "repetitions": 1,
                },
                {
                    **copy.deepcopy(adjacent_phase["attempts"][0]),
                    "repetitions": 1,
                },
            ]
            _refresh_fault_observation_row(adjacent_rle[0])
            with self.assertRaisesRegex(MODULE.RunnerError, "non-canonical attempt stream"):
                validate_observations(adjacent_rle)
            reordered_stream = copy.deepcopy(rows)
            reordered_phase = reordered_stream[0]["continuous_observations"][0][
                "phase_coverage"
            ][0]
            original_run = copy.deepcopy(reordered_phase["attempts"][0])
            nonfinalized_response = reordered_stream[0]["snapshots"][1]["validators"][
                0
            ]["response_hex"]
            reordered_phase["attempts"] = [
                {**original_run, "repetitions": 1},
                {
                    "class": "baseline",
                    "evidence": nonfinalized_response,
                    "repetitions": 1,
                },
            ]
            _refresh_fault_observation_row(reordered_stream[0])
            reordered_phase["attempts"].reverse()
            with self.assertRaisesRegex(MODULE.RunnerError, "attempt stream does not bind"):
                validate_observations(reordered_stream)
            transplanted_peer = copy.deepcopy(rows)
            transplanted_peer[0]["continuous_observations"][1]["phase_coverage"][0] = (
                copy.deepcopy(
                    transplanted_peer[0]["continuous_observations"][0][
                        "phase_coverage"
                    ][0]
                )
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "attempt stream does not bind"):
                validate_observations(transplanted_peer)
            transplanted_bundle = copy.deepcopy(rows)
            transplanted_bundle[1]["continuous_observations"][0]["phase_coverage"][0] = (
                copy.deepcopy(
                    transplanted_bundle[0]["continuous_observations"][0][
                        "phase_coverage"
                    ][0]
                )
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "attempt stream does not bind"):
                validate_observations(transplanted_bundle)
            wrong_checkpoint_binding = copy.deepcopy(rows)
            wrong_checkpoint_binding[0]["continuous_observations"][0][
                "phase_coverage"
            ][1]["checkpoint_control_bindings"] = [
                f"acknowledgement:{'f' * 64}"
            ]
            with self.assertRaisesRegex(
                MODULE.RunnerError, "contradicts authenticated controls"
            ):
                validate_observations(wrong_checkpoint_binding)
            moved_checkpoint = copy.deepcopy(rows)
            moved_phase = moved_checkpoint[0]["continuous_observations"][0][
                "phase_coverage"
            ][1]
            moved_phase["checkpoint_attempt"] = 1
            _refresh_fault_observation_row(moved_checkpoint[0])
            with self.assertRaisesRegex(MODULE.RunnerError, "no post-checkpoint attempt"):
                validate_observations(moved_checkpoint)
            private_failure = copy.deepcopy(rows)
            private_failure_row = next(
                row
                for row in private_failure
                if row["record"] == carrier_row["record"]
            )
            private_failure_phase = next(
                phase
                for phase in private_failure_row["continuous_observations"][4][
                    "phase_coverage"
                ]
                if phase["phase"] == "committee_unavailable"
            )
            private_failure_phase["attempts"][0]["evidence"] = (
                "private connection detail"
            )
            _refresh_fault_observation_row(private_failure_row)
            with self.assertRaisesRegex(MODULE.RunnerError, "unallowlisted poll failure"):
                validate_observations(private_failure)
            malformed_success = copy.deepcopy(rows)
            malformed_success[0]["continuous_observations"][0]["phase_coverage"][0][
                "attempts"
            ][0]["evidence"] = MODULE.canonical_bytes({}).hex()
            _refresh_fault_observation_row(malformed_success[0])
            with self.assertRaisesRegex(MODULE.RunnerError, "missing="):
                validate_observations(malformed_success)
            premature_finalization = copy.deepcopy(rows)
            premature_phase = premature_finalization[0]["continuous_observations"][0][
                "phase_coverage"
            ][1]
            premature_phase["attempts"] = [
                {
                    "class": "finalized",
                    "evidence": premature_finalization[0]["snapshots"][2][
                        "validators"
                    ][0]["response_hex"],
                    "repetitions": 1,
                }
            ]
            _refresh_fault_observation_row(premature_finalization[0])
            with self.assertRaisesRegex(MODULE.RunnerError, "finalized in a disallowed phase"):
                validate_observations(premature_finalization)
            finalized_then_baseline = copy.deepcopy(rows)
            rollback_phase = finalized_then_baseline[0]["continuous_observations"][0][
                "phase_coverage"
            ][2]
            rollback_phase["attempts"] = [
                {
                    "class": "finalized",
                    "evidence": finalized_then_baseline[0]["snapshots"][2][
                        "validators"
                    ][0]["response_hex"],
                    "repetitions": 1,
                },
                {
                    "class": "baseline",
                    "evidence": finalized_then_baseline[0]["snapshots"][0][
                        "validators"
                    ][0]["response_hex"],
                    "repetitions": 1,
                },
            ]
            _refresh_fault_observation_row(finalized_then_baseline[0])
            with self.assertRaisesRegex(MODULE.RunnerError, "finalized state rolled back"):
                validate_observations(finalized_then_baseline)
            unanchored = copy.deepcopy(rows)
            unanchored[0]["continuous_observations"][0][
                "first_response_sha256"
            ] = "f" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "exemplar endpoints"):
                validate_observations(unanchored)
            unclassified = copy.deepcopy(rows)
            unclassified[0]["continuous_observations"][0][
                "baseline_observations"
            ] -= 1
            with self.assertRaisesRegex(MODULE.RunnerError, "unclassified observation"):
                validate_observations(unclassified)
            swapped_aggregates = copy.deepcopy(rows)
            swapped_summary = swapped_aggregates[0]["continuous_observations"][0]
            swapped_summary["baseline_observations"] -= 1
            swapped_summary["finalized_observations"] += 1
            with self.assertRaisesRegex(MODULE.RunnerError, "phase totals"):
                validate_observations(swapped_aggregates)
            reused_bundle = copy.deepcopy(rows)
            reused_bundle[1]["bundle_id"] = reused_bundle[0]["bundle_id"]
            with self.assertRaisesRegex(MODULE.RunnerError, "reuses an APS bundle"):
                validate_observations(reused_bundle)
            missing_registration_recovery = copy.deepcopy(rows)
            registration_row = next(
                row
                for row in missing_registration_recovery
                if row["collection"] == "crash_recoveries"
                and row["trial_index"] == 3
            )
            registration_row["snapshots"][1]["validators"] = copy.deepcopy(
                registration_row["snapshots"][0]["validators"]
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "full replicated Prepare lock"
            ):
                validate_observations(missing_registration_recovery)
            mixed_lock_plane = copy.deepcopy(rows)
            mixed_row = next(
                row
                for row in mixed_lock_plane
                if row["collection"] == "loss_trials"
            )
            mixed_row["snapshots"][1]["validators"][4] = copy.deepcopy(
                mixed_row["snapshots"][0]["validators"][4]
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "full replicated Prepare lock"
            ):
                validate_observations(mixed_lock_plane)
            divergent_replicated_lock = copy.deepcopy(rows)
            divergent_replicated_row = next(
                row
                for row in divergent_replicated_lock
                if row["collection"] == "loss_trials"
            )
            _rewrite_state_observation(
                divergent_replicated_row["snapshots"][1]["validators"][4],
                replicated_staged_lock_commitment=_iroha_hash_literal("7" * 64),
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "full replicated Prepare lock"
            ):
                validate_observations(divergent_replicated_lock)
            incomplete_local_lock = copy.deepcopy(rows)
            incomplete_local_row = next(
                row
                for row in incomplete_local_lock
                if row["collection"] == "loss_trials"
            )
            incomplete_counts = copy.deepcopy(
                incomplete_local_row["snapshots"][1]["validators"][4]["counts"]
            )
            incomplete_counts.update(
                {
                    "staged_pool_heads": 0,
                    "staged_nullifiers": 0,
                    "staged_output_commitments": 0,
                    "staged_locks": 0,
                }
            )
            _rewrite_state_observation(
                incomplete_local_row["snapshots"][1]["validators"][4],
                staged_lock_commitment=_iroha_hash_literal("2" * 64),
                counts=incomplete_counts,
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "complete local leg lock"
            ):
                validate_observations(incomplete_local_lock)
            divergent_local_lock = copy.deepcopy(rows)
            divergent_local_row = next(
                row
                for row in divergent_local_lock
                if row["collection"] == "loss_trials"
            )
            _rewrite_state_observation(
                divergent_local_row["snapshots"][1]["validators"][5],
                staged_lock_commitment=_iroha_hash_literal("8" * 64),
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "divergent committee-local locks"
            ):
                validate_observations(divergent_local_lock)
            bad_hash_checksum = copy.deepcopy(rows)
            bad_hash_row = next(
                row
                for row in bad_hash_checksum
                if row["collection"] == "loss_trials"
            )
            invalid_hash = bad_hash_row["snapshots"][0]["validators"][0][
                "commitment"
            ]
            invalid_hash = invalid_hash[:-4] + (
                "0000" if invalid_hash[-4:] != "0000" else "FFFF"
            )
            _rewrite_state_observation(
                bad_hash_row["snapshots"][0]["validators"][0],
                commitment=invalid_hash,
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "checksum"):
                validate_observations(bad_hash_checksum)
            substituted_control_bundle = copy.deepcopy(control_rows)
            substituted_control_bundle[0]["bundle_id"] = "e" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "binds another APS bundle"):
                MODULE.validate_fault_control_records(
                    substituted_control_bundle, participants=3, seed=7, run=2
                )
            substituted_restart = copy.deepcopy(control_rows)
            crash_restart = next(
                control
                for row in substituted_restart
                if row["collection"] == "crash_recoveries"
                for control in row["controls"]
                if control["control_type"] == "validator_restart"
            )
            crash_restart["before_pid"] += 10
            with self.assertRaisesRegex(MODULE.RunnerError, "restart acknowledgement"):
                MODULE.validate_fault_control_records(
                    substituted_restart, participants=3, seed=7, run=2
                )

    def test_fault_restart_topology_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            payload = fault_payload()
            write_fault_evidence(evidence, payload)
            control_rows, _binding = MODULE.read_bound_jsonl_file(
                evidence / MODULE.FAULT_CONTROL_EVIDENCE_FILE,
                "fault controls",
            )
            by_record = {row["record"]: row for row in control_rows}

            carrier = copy.deepcopy(
                by_record["n3:s7:r2:phase_cut_partitions:3"]
            )
            carrier["controls"] = [
                control
                for control in carrier["controls"]
                if not (
                    control["control_type"] == "validator_restart"
                    and control["peer_index"] == 8
                )
            ]
            with self.assertRaisesRegex(MODULE.RunnerError, "restart topology"):
                MODULE.validate_fault_trial_control_semantics(
                    carrier,
                    collection="phase_cut_partitions",
                    trial=payload["phase_cut_partitions"][3],
                    label="carrier",
                )

            extra_route_control = copy.deepcopy(
                by_record["n3:s7:r2:phase_cut_partitions:3"]
            )
            extra_route_control["controls"].append(
                _route_occurrence(
                    "prepare",
                    "hold",
                    99_999,
                    bundle_id=extra_route_control["bundle_id"],
                    seed=7,
                    drop_first=0,
                    match_limit=1,
                    matched=1,
                    passed=0,
                    dropped=0,
                    held=1,
                    released=0,
                )
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "control allowlist"):
                MODULE._fault_observation_phase_contract(
                    extra_route_control,
                    collection="phase_cut_partitions",
                    label="carrier",
                )

            duplicate_transition = copy.deepcopy(
                by_record["n3:s7:r2:phase_cut_partitions:3"]
            )
            participant_restarts = [
                control
                for control in duplicate_transition["controls"]
                if control["control_type"] == "validator_restart"
            ]
            participant_restarts[1]["before_pid"] = participant_restarts[0][
                "before_pid"
            ]
            participant_restarts[1]["after_pid"] = participant_restarts[0][
                "after_pid"
            ]
            with self.assertRaisesRegex(MODULE.RunnerError, "restart topology"):
                MODULE.validate_fault_trial_control_semantics(
                    duplicate_transition,
                    collection="phase_cut_partitions",
                    trial=payload["phase_cut_partitions"][3],
                    label="carrier",
                )

            empty_hold = copy.deepcopy(
                by_record["n3:s7:r2:phase_cut_partitions:3"]
            )
            hold_control = next(
                control
                for control in empty_hold["controls"]
                if control["control_type"] == "consensus_carrier"
            )
            hold_ack = MODULE.strict_json_loads(
                bytes.fromhex(hold_control["acknowledgement_hex"]).decode(),
                "hold acknowledgement",
            )
            hold_ack["held"] = []
            hold_ack["held_bytes"] = 0
            hold_ack_bytes = MODULE.canonical_bytes(hold_ack)
            hold_control["acknowledgement_hex"] = hold_ack_bytes.hex()
            hold_control["acknowledgement_sha256"] = MODULE.hashlib.sha256(
                hold_ack_bytes
            ).hexdigest()
            with self.assertRaisesRegex(MODULE.RunnerError, "active carrier Hold"):
                MODULE.validate_fault_trial_control_semantics(
                    empty_hold,
                    collection="phase_cut_partitions",
                    trial=payload["phase_cut_partitions"][3],
                    label="carrier",
                )

            crash = copy.deepcopy(by_record["n3:s7:r2:crash_recoveries:0"])
            restart = next(
                control
                for control in crash["controls"]
                if control["control_type"] == "validator_restart"
            )
            restart["peer_index"] = 5
            with self.assertRaisesRegex(MODULE.RunnerError, "persistence cut"):
                MODULE.validate_fault_trial_control_semantics(
                    crash,
                    collection="crash_recoveries",
                    trial=payload["crash_recoveries"][0],
                    label="crash",
                )

            wrong_receipt_target = copy.deepcopy(
                by_record["n3:s7:r2:crash_recoveries:8"]
            )
            for control in wrong_receipt_target["controls"]:
                control["peer_index"] = 0
                if control["control_type"] == "validator_restart":
                    control["control_type"] = "global_restart"
            with self.assertRaisesRegex(MODULE.RunnerError, "persistence cut"):
                MODULE.validate_fault_trial_control_semantics(
                    wrong_receipt_target,
                    collection="crash_recoveries",
                    trial=payload["crash_recoveries"][8],
                    label="receipt",
                )

    def test_fault_evidence_cache_binds_capture_to_each_transcript(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            identity = {
                "record": "n3:s7:r2:loss_trials:0",
                "participants": 3,
                "seed": 7,
                "run": 2,
                "collection": "loss_trials",
                "trial_index": 0,
                "bundle_id": "a" * 64,
            }

            def write_jsonl(name: str, rows: list[dict[str, Any]]) -> tuple[Path, bytes]:
                path = root / name
                payload = b"".join(
                    MODULE.canonical_bytes(row) + b"\n" for row in rows
                )
                path.write_bytes(payload)
                return path, payload

            transcript_one_path, transcript_one = write_jsonl(
                "control-one.jsonl", [{**identity, "variant": "one"}]
            )
            transcript_two_path, transcript_two = write_jsonl(
                "control-two.jsonl", [{**identity, "variant": "two"}]
            )
            capture_entry = {
                **identity,
                "partial_visibility_observed": False,
                "partial_spendable_observations": 0,
            }
            capture_path, capture = write_jsonl("capture.jsonl", [capture_entry])
            transcript_one_sha = hashlib.sha256(transcript_one).hexdigest()
            transcript_two_sha = hashlib.sha256(transcript_two).hexdigest()
            capture_sha = hashlib.sha256(capture).hexdigest()

            def raw_record(transcript_sha: str) -> dict[str, Any]:
                return {
                    "participants": 3,
                    "seed": 7,
                    "run": 2,
                    "loss_trials": [
                        {
                            "control_transcript_sha256": transcript_sha,
                            "control_transcript_record": identity["record"],
                            "observation_capture_sha256": capture_sha,
                            "observation_capture_record": identity["record"],
                            "partial_visibility_observed": False,
                        }
                    ],
                    "phase_cut_partitions": [],
                    "crash_recoveries": [],
                    "atomicity": {"partial_spendable_observations": 0},
                }

            raw_one_path, _raw_one = write_jsonl(
                "raw-one.jsonl", [raw_record(transcript_one_sha)]
            )
            raw_two_path, _raw_two = write_jsonl(
                "raw-two.jsonl", [raw_record(transcript_two_sha)]
            )
            artifact = MODULE.release_evidence.Artifact
            artifacts = [
                artifact(
                    kind="operator_log",
                    path=MODULE.PurePosixPath(transcript_one_path.name),
                    sha256=transcript_one_sha,
                    bytes=len(transcript_one),
                ),
                artifact(
                    kind="operator_log",
                    path=MODULE.PurePosixPath(transcript_two_path.name),
                    sha256=transcript_two_sha,
                    bytes=len(transcript_two),
                ),
                artifact(
                    kind="sanitized_capture",
                    path=MODULE.PurePosixPath(capture_path.name),
                    sha256=capture_sha,
                    bytes=len(capture),
                ),
            ]

            class FakeValidator:
                def __init__(self) -> None:
                    self.capture_transcripts: list[str] = []

                def validate_fault_control_records(
                    self, records: list[dict[str, Any]], **_kwargs: Any
                ) -> dict[str, dict[str, Any]]:
                    return {record["record"]: record for record in records}

                def validate_fault_observation_records(
                    self,
                    _records: list[dict[str, Any]],
                    *,
                    control_by_record: dict[str, dict[str, Any]],
                    **_kwargs: Any,
                ) -> None:
                    self.capture_transcripts.append(
                        next(iter(control_by_record.values()))["variant"]
                    )

                def validate_fault_trial_control_semantics(
                    self, *_args: Any, **_kwargs: Any
                ) -> None:
                    return None

            validator = FakeValidator()
            with mock.patch.object(
                MODULE.release_evidence,
                "_load_fault_evidence_validator",
                return_value=validator,
            ):
                MODULE.release_evidence._validate_fault_trial_evidence_bindings(
                    [raw_one_path, raw_two_path], artifacts, root
                )
            self.assertEqual(validator.capture_transcripts, ["one", "two"])

            mismatched_capture = copy.deepcopy(capture_entry)
            mismatched_capture["bundle_id"] = "b" * 64
            _mismatch_path, mismatch_payload = write_jsonl(
                "capture-mismatch.jsonl", [mismatched_capture]
            )
            mismatch_sha = hashlib.sha256(mismatch_payload).hexdigest()
            mismatch_artifacts = [
                artifacts[0],
                artifact(
                    kind="sanitized_capture",
                    path=MODULE.PurePosixPath("capture-mismatch.jsonl"),
                    sha256=mismatch_sha,
                    bytes=len(mismatch_payload),
                ),
            ]
            mismatch_raw_path, _mismatch_raw = write_jsonl(
                "raw-mismatch.jsonl",
                [
                    {
                        **raw_record(transcript_one_sha),
                        "loss_trials": [
                            {
                                **raw_record(transcript_one_sha)["loss_trials"][0],
                                "observation_capture_sha256": mismatch_sha,
                            }
                        ],
                    }
                ],
            )
            with mock.patch.object(
                MODULE.release_evidence,
                "_load_fault_evidence_validator",
                return_value=FakeValidator(),
            ), self.assertRaisesRegex(
                MODULE.release_evidence.EvidenceError, "bundle_id"
            ):
                MODULE.release_evidence._validate_fault_trial_evidence_bindings(
                    [mismatch_raw_path], mismatch_artifacts, root
                )

    def test_any_missing_control_acknowledgement_fails_closed(self) -> None:
        job = fault_job()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            publication = root / "publication"
            evidence.mkdir()
            publication.mkdir()
            payload = fault_payload()
            write_fault_evidence(evidence, payload)
            result = response(job, payload)
            result["payload"]["loss_trials"][0]["control_acknowledged"] = False
            with self.assertRaisesRegex(MODULE.RunnerError, "fault harness result"):
                MODULE.materialize_fault_response(
                    result,
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                    publication_root=publication,
                )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "evidence"
            publication = root / "publication"
            evidence.mkdir()
            publication.mkdir()
            payload = fault_payload()
            write_fault_evidence(evidence, payload)
            result = response(job, payload)
            result["payload"]["prepare_qc_normalization"][
                "second_normalized_barrier_sha256"
            ] = "5" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "quorum-equivalent"):
                MODULE.materialize_fault_response(
                    result,
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                    publication_root=publication,
                )

    def test_benchmark_requires_positive_real_measurements_and_atomic_finality(
        self,
    ) -> None:
        job = {
            "request_id": "f" * 64,
            "invocation_nonce": INVOCATION_NONCE,
            "kind": "benchmark",
            "profile": "private",
            "participants": 3,
            "seed": 1,
            "run": 0,
            "warmup": False,
            "configuration_sha256": CONFIGURATION,
        }
        payload = {
            "stages_ms": {
                stage: float(index + 1)
                for index, stage in enumerate(
                    MODULE.benchmark_report.REQUIRED_PRIVATE_STAGES
                )
            },
            **{
                field: 10.0
                for field in MODULE.benchmark_report.RESOURCE_FIELDS
            },
            "finalized_receipt_observed": True,
            "successful_leg_applications": 3,
            "each_leg_applied_exactly_once": True,
            "partial_visible_observations": 0,
            "partial_spendable_observations": 0,
        }
        raw = MODULE.materialize_benchmark_response(
            response(job, payload), plan=plan(), job=job
        )
        self.assertEqual(raw["profile"], "private")
        broken = copy.deepcopy(payload)
        broken["network_bytes"] = 0
        with self.assertRaisesRegex(
            MODULE.RunnerError, "network_bytes must be positive"
        ):
            MODULE.materialize_benchmark_response(
                response(job, broken), plan=plan(), job=job
            )

    def test_leakage_response_must_bind_every_capture_file_and_count(self) -> None:
        canaries = MODULE.build_canary_manifest(COMMIT)
        selected = MODULE.canaries_for_variant(canaries, "left")
        job = {
            "request_id": "1" * 64,
            "invocation_nonce": INVOCATION_NONCE,
            "kind": "leakage",
            "participants": 3,
            "seed": 0,
            "run": 0,
            "variant": "left",
            "canary_names": [entry["name"] for entry in selected],
            "canary_commitments": {
                entry["name"]: MODULE.object_digest(entry) for entry in selected
            },
            "configuration_sha256": CONFIGURATION,
        }
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            payload = leakage_payload(job, evidence)
            counts, surfaces = MODULE.validate_leakage_response(
                response(job, payload),
                plan=plan(),
                job=job,
                evidence_dir=evidence,
            )
            self.assertEqual(len(surfaces), len(MODULE.SURFACE_FILES))
            self.assertTrue(all(value >= 1 for value in counts.values()))
            self.assertTrue(all(binding["bytes"] > 0 for _, _, binding in surfaces))
            fabricated_count = copy.deepcopy(payload)
            fabricated_count["traffic_counts"]["query_responses"] = 1
            with self.assertRaisesRegex(MODULE.RunnerError, "not source-backed"):
                MODULE.validate_leakage_response(
                    response(job, fabricated_count),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            fabricated_split = copy.deepcopy(payload)
            fabricated_split["capture_provenance"]["packet_counts"][
                "torii_packets"
            ] = 3
            with self.assertRaisesRegex(MODULE.RunnerError, "final packet files"):
                MODULE.validate_leakage_response(
                    response(job, fabricated_split),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            substituted_raw = copy.deepcopy(payload)
            packet_row = next(
                row
                for row in substituted_raw["artifacts"]
                if row["relative_name"].endswith(".pcapng")
            )
            packet_row["source_sha256"] = "4" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "raw pcap"):
                MODULE.validate_leakage_response(
                    response(job, substituted_raw),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            false_port_binding = copy.deepcopy(payload)
            false_port_binding["capture_provenance"]["port_manifest"]["sha256"] = "4" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "not derived"):
                MODULE.validate_leakage_response(
                    response(job, false_port_binding),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            false_tcpdump = copy.deepcopy(payload)
            false_tcpdump["capture_provenance"]["tcpdump"]["statistics"][
                "received_by_filter_packets"
            ] += 1
            with self.assertRaisesRegex(MODULE.RunnerError, "retained stderr"):
                MODULE.validate_leakage_response(
                    response(job, false_tcpdump),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            raw_path = evidence / MODULE.SURFACE_FILES["restricted_packet_source"]
            original_raw = raw_path.read_bytes()
            changed_raw = bytearray(original_raw)
            changed_raw[-1] ^= 1
            raw_path.write_bytes(bytes(changed_raw))
            raw_rebound = copy.deepcopy(payload)
            changed_binding = MODULE.file_binding(raw_path)
            raw_rebound["capture_provenance"]["raw_pcap"] = changed_binding
            for row in raw_rebound["artifacts"]:
                if row["surface"] == "restricted_packet_source":
                    row.update(changed_binding)
                    row["source_sha256"] = changed_binding["sha256"]
                    row["source_bytes"] = changed_binding["bytes"]
                elif row["relative_name"].endswith(".pcapng"):
                    row["source_sha256"] = changed_binding["sha256"]
                    row["source_bytes"] = changed_binding["bytes"]
            with self.assertRaisesRegex(MODULE.RunnerError, "exact derivatives"):
                MODULE.validate_leakage_response(
                    response(job, raw_rebound),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            raw_path.write_bytes(original_raw)
            split_path = evidence / MODULE.SURFACE_FILES["torii_capture"]
            original_split = split_path.read_bytes()
            changed_split = bytearray(original_split)
            changed_split[-8] ^= 1
            split_path.write_bytes(bytes(changed_split))
            split_rebound = copy.deepcopy(payload)
            split_row = next(
                row
                for row in split_rebound["artifacts"]
                if row["surface"] == "torii_capture"
            )
            split_row.update(MODULE.file_binding(split_path))
            with self.assertRaisesRegex(
                MODULE.RunnerError, "exact derivatives|cannot be replayed"
            ):
                MODULE.validate_leakage_response(
                    response(job, split_rebound),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            split_path.write_bytes(original_split)
            block_path = evidence / MODULE.SURFACE_FILES["block_wire_capture"]
            original_block = block_path.read_bytes()
            changed_block = bytearray(original_block)
            changed_block[-1] ^= 1
            block_path.write_bytes(bytes(changed_block))
            block_rebound = copy.deepcopy(payload)
            block_row = next(
                row
                for row in block_rebound["artifacts"]
                if row["surface"] == "block_wire_capture"
            )
            block_row.update(MODULE.file_binding(block_path))
            with self.assertRaisesRegex(MODULE.RunnerError, "retained raw source"):
                MODULE.validate_leakage_response(
                    response(job, block_rebound),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            block_path.write_bytes(original_block)
            broken = copy.deepcopy(payload)
            broken["artifacts"] = broken["artifacts"][:-1]
            with self.assertRaisesRegex(MODULE.RunnerError, "every required surface"):
                MODULE.validate_leakage_response(
                    response(job, broken),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            reordered = copy.deepcopy(payload)
            reordered["artifacts"][0], reordered["artifacts"][1] = (
                reordered["artifacts"][1],
                reordered["artifacts"][0],
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "canonically ordered"):
                MODULE.validate_leakage_response(
                    response(job, reordered),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            empty_counts = copy.deepcopy(payload)
            first_channel = MODULE.leakage_audit.REQUIRED_COUNT_CHANNELS[0]
            empty_counts["traffic_counts"][first_channel] = 0
            with self.assertRaisesRegex(MODULE.RunnerError, "must be in 1"):
                MODULE.validate_leakage_response(
                    response(job, empty_counts),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            empty_capture = copy.deepcopy(payload)
            empty_row = empty_capture["artifacts"][-1]
            empty_path = evidence / empty_row["relative_name"]
            original_bytes = empty_path.read_bytes()
            empty_path.write_bytes(b"")
            empty_row.update(MODULE.file_binding(empty_path))
            with self.assertRaisesRegex(MODULE.RunnerError, "must not be empty"):
                MODULE.validate_leakage_response(
                    response(job, empty_capture),
                    plan=plan(),
                    job=job,
                    evidence_dir=evidence,
                )
            empty_path.write_bytes(original_bytes)
            surface, source, expected_binding = surfaces[0]
            source.write_bytes(source.read_bytes() + b"mutation")
            with self.assertRaisesRegex(MODULE.RunnerError, "changed before copy"):
                MODULE.copy_bound_file(
                    source,
                    evidence / f"copy-{surface}",
                    expected=expected_binding,
                )

    def test_atomicity_replay_rejects_invalid_heights_and_terminal_staged_locks(
        self,
    ) -> None:
        def rewrite_projection(observation: dict[str, Any]) -> None:
            raw = json.loads(bytes.fromhex(observation["response_hex"]).decode())
            for field in (
                "height",
                "commitment",
                "ledger_commitment",
                "replicated_staged_lock_commitment",
                "staged_lock_commitment",
                "counts",
            ):
                raw[field] = observation[field]
            encoded = json.dumps(
                raw, sort_keys=True, separators=(",", ":")
            ).encode()
            observation["response_hex"] = encoded.hex()
            observation["response_sha256"] = hashlib.sha256(encoded).hexdigest()

        def write_atomicity_archive(
            archive: Path, replacements: dict[int, bytes]
        ) -> list[dict[str, Any]]:
            sources = [
                replacements.get(index, _atomicity_evidence(index, 3))
                for index in range(16)
            ]
            _write_restricted_archive(
                archive,
                [
                    (
                        "atomicity_observation",
                        f"peer-{index:03}.json",
                        source,
                    )
                    for index, source in enumerate(sources)
                ],
            )
            return MODULE._validate_restricted_leakage_source_archive(archive)[
                "atomicity_observation"
            ]["rows"]

        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / "restricted.bin"
            valid_source = _atomicity_evidence(0, 3)
            rows = write_atomicity_archive(archive, {0: valid_source})
            self.assertEqual(
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16),
                48,
            )

            bad_checksum = json.loads(valid_source)
            first_observation = bad_checksum["observations"][0]
            literal = first_observation["commitment"]
            first_observation["commitment"] = literal[:-4] + (
                "0000" if literal[-4:] != "0000" else "FFFF"
            )
            rewrite_projection(first_observation)
            rows = write_atomicity_archive(
                archive,
                {
                    0: json.dumps(
                        bad_checksum, sort_keys=True, separators=(",", ":")
                    ).encode()
                },
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "checksum"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            noncanonical_response = json.loads(valid_source)
            first_observation = noncanonical_response["observations"][0]
            decoded_response = json.loads(
                bytes.fromhex(first_observation["response_hex"]).decode()
            )
            noncanonical_bytes = json.dumps(decoded_response, indent=1).encode()
            first_observation["response_hex"] = noncanonical_bytes.hex()
            first_observation["response_sha256"] = hashlib.sha256(
                noncanonical_bytes
            ).hexdigest()
            rows = write_atomicity_archive(
                archive,
                {
                    0: json.dumps(
                        noncanonical_response,
                        sort_keys=True,
                        separators=(",", ":"),
                    ).encode()
                },
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "canonical compact JSON"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            missing_registered = json.loads(valid_source)
            missing_registered["registered"] = copy.deepcopy(
                missing_registered["observations"][0]
            )
            rows = write_atomicity_archive(
                archive,
                {
                    0: json.dumps(
                        missing_registered, sort_keys=True, separators=(",", ":")
                    ).encode()
                },
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "complete registered replicated Prepare lock"
            ):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            missing_local = json.loads(_atomicity_evidence(4, 3))
            registered = missing_local["registered"]
            registered["counts"].update(
                {
                    "staged_pool_heads": 0,
                    "staged_nullifiers": 0,
                    "staged_output_commitments": 0,
                    "staged_locks": 0,
                }
            )
            registered["staged_lock_commitment"] = _iroha_hash_literal("3" * 64)
            rewrite_projection(registered)
            rows = write_atomicity_archive(
                archive,
                {
                    4: json.dumps(
                        missing_local, sort_keys=True, separators=(",", ":")
                    ).encode()
                },
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "registered committee-local leg lock"
            ):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            divergent_local = json.loads(_atomicity_evidence(5, 3))
            divergent_local["registered"]["staged_lock_commitment"] = (
                _iroha_hash_literal("F" * 64)
            )
            rewrite_projection(divergent_local["registered"])
            rows = write_atomicity_archive(
                archive,
                {
                    5: json.dumps(
                        divergent_local, sort_keys=True, separators=(",", ":")
                    ).encode()
                },
            )
            with self.assertRaisesRegex(
                MODULE.RunnerError, "divergent registered committee-local locks"
            ):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            negative_height = json.loads(valid_source)
            negative_height["observations"][0]["height"] = -1
            rewrite_projection(negative_height["observations"][0])
            rows = write_atomicity_archive(
                archive,
                {
                    0: json.dumps(
                        negative_height, sort_keys=True, separators=(",", ":")
                    ).encode()
                },
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "height"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            terminal_staged = json.loads(valid_source)
            final = terminal_staged["observations"][-1]
            final["counts"].update(
                {
                    "staged_pool_heads": 1,
                    "staged_nullifiers": 2,
                    "staged_output_commitments": 3,
                    "staged_locks": 6,
                }
            )
            final["staged_lock_commitment"] = _iroha_hash_literal("4" * 64)
            rewrite_projection(final)
            rows = write_atomicity_archive(
                archive,
                {
                    0: json.dumps(
                        terminal_staged, sort_keys=True, separators=(",", ":")
                    ).encode()
                },
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "retained staged locks"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

            terminal_replicated = json.loads(valid_source)
            final = terminal_replicated["observations"][-1]
            final["counts"]["replicated_staged_locks"] = 1 + 3 * 9
            final["replicated_staged_lock_commitment"] = _iroha_hash_literal(
                "6" * 64
            )
            rewrite_projection(final)
            rows = write_atomicity_archive(
                archive,
                {
                    0: json.dumps(
                        terminal_replicated,
                        sort_keys=True,
                        separators=(",", ":"),
                    ).encode()
                },
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "retained staged locks"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 16)

    def test_differential_manifest_is_accepted_by_release_validator(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifacts: dict[Any, Any] = {}
            for variant in ("left", "right"):
                for index, surface in enumerate(sorted(MODULE.SURFACE_FILES)):
                    path = root / "leakage" / variant / MODULE.SURFACE_FILES[surface]
                    path.parent.mkdir(parents=True, exist_ok=True)
                    if surface == "restricted_audit_source":
                        _write_restricted_archive(
                            path,
                            [("query_capture", "peer-000.norito", b"opaque-source")],
                        )
                    elif path.suffix == ".pcapng":
                        _write_pcapng(path, [_ethernet_ipv4_tcp(20_000, 20_001)])
                    elif path.suffix == ".json":
                        path.write_text(
                            json.dumps({"opaque": f"capture-{index:02d}"}) + "\n",
                            encoding="utf-8",
                        )
                    else:
                        content = bytearray(
                            f"opaque-capture-{index:02d}\n".encode()
                        )
                        if (
                            variant == "right"
                            and surface
                            in MODULE.REQUIRED_DIFFERENTIAL_STATE_CHANGES
                        ):
                            content[0] ^= 1
                        path.write_bytes(content)
                    binding = MODULE.file_binding(path, relative_to=root)
                    relative = MODULE.PurePosixPath(binding["path"])
                    artifacts[relative] = MODULE.release_evidence.Artifact(
                        kind=surface,
                        path=relative,
                        sha256=binding["sha256"],
                        bytes=binding["bytes"],
                    )
            manifest_path = root / "differential-pairs-v1.json"
            MODULE.write_json(
                manifest_path,
                MODULE.differential_pair_manifest(root, COMMIT),
            )
            bindings = MODULE.release_evidence._validate_differential_pair_manifest(
                manifest_path,
                commit=COMMIT,
                root=root,
                artifacts_by_path=artifacts,
            )
            self.assertEqual(len(bindings), len(MODULE.SURFACE_FILES) * 2)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(
                [pair["surface"] for pair in manifest["pairs"]],
                sorted(MODULE.SURFACE_FILES),
            )
            canary_path = root / "canary-manifest-v1.json"
            MODULE.write_json(canary_path, MODULE.build_canary_manifest(COMMIT))
            count_paths = []
            for variant in ("left", "right"):
                count_path = root / f"traffic-counts-{variant}.json"
                MODULE.write_json(
                    count_path,
                    {
                        "version": 1,
                        "channels": {
                            channel: 1
                            for channel in MODULE.leakage_audit.REQUIRED_COUNT_CHANNELS
                        },
                    },
                )
                count_paths.append(count_path)
            audit = MODULE.leakage_audit.run_audit(
                canary_path,
                [
                    *(root / "leakage" / "left").iterdir(),
                    *(root / "leakage" / "right").iterdir(),
                    *count_paths,
                ],
                differential_left=root / "leakage" / "left",
                differential_right=root / "leakage" / "right",
                traffic_counts_left=count_paths[0],
                traffic_counts_right=count_paths[1],
            )
            self.assertTrue(audit["passed"])
            changed_surface = "block_wire_capture"
            left_state = (
                root
                / "leakage"
                / "left"
                / MODULE.SURFACE_FILES[changed_surface]
            )
            right_state = (
                root
                / "leakage"
                / "right"
                / MODULE.SURFACE_FILES[changed_surface]
            )
            right_bytes = right_state.read_bytes()
            right_state.write_bytes(left_state.read_bytes())
            with self.assertRaisesRegex(MODULE.RunnerError, "did not change"):
                MODULE.differential_pair_manifest(root, COMMIT)
            right_state.write_bytes(right_bytes)

    def test_bound_plan_paths_reject_symlinked_parent_components(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workspace = Path(temporary)
            root = workspace / "plan"
            outside = workspace / "outside"
            root.mkdir()
            outside.mkdir()
            (outside / "config.json").write_text("{}\n", encoding="utf-8")
            (root / "linked").symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(MODULE.RunnerError, "symbolic link"):
                MODULE.regular_file_under(
                    root,
                    MODULE.PurePosixPath("linked/config.json"),
                    "fixture",
                )

    def test_strict_json_rejects_duplicate_keys_and_nonfinite_values(self) -> None:
        with self.assertRaisesRegex(MODULE.RunnerError, "duplicate key"):
            MODULE.strict_json_loads('{"passed":true,"passed":false}', "fixture")
        with self.assertRaisesRegex(MODULE.RunnerError, "non-JSON constant"):
            MODULE.strict_json_loads('{"latency":NaN}', "fixture")

    def test_request_revalidates_canary_contents_and_commitments(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            canary_path = root / "canary-manifest-v1.json"
            canaries = MODULE.build_canary_manifest(COMMIT)
            MODULE.write_json(canary_path, canaries)
            configuration_path = root / "configurations" / "n3.json"
            MODULE.write_json(
                configuration_path,
                MODULE.build_configuration(
                    3,
                    seeds=tuple(range(MODULE.MIN_FAULT_SEEDS)),
                    warmups=MODULE.MIN_WARMUPS,
                    measured=MODULE.MIN_MEASURED,
                ),
            )
            configuration_binding = MODULE.file_binding(configuration_path)
            manifest_path = root / "configuration-manifest-v1.json"
            MODULE.write_json(
                manifest_path,
                {
                    "configurations": [
                        {
                            "participants": 3,
                            "path": "configurations/n3.json",
                            **configuration_binding,
                        }
                    ]
                },
            )
            selected = MODULE.canaries_for_variant(canaries, "left")
            job = {
                "request_id": "1" * 64,
                "invocation_nonce": INVOCATION_NONCE,
                "kind": "leakage",
                "participants": 3,
                "seed": 0,
                "run": 0,
                "variant": "left",
                "canary_names": [entry["name"] for entry in selected],
                "canary_commitments": {
                    entry["name"]: MODULE.object_digest(entry)
                    for entry in selected
                },
                "configuration_sha256": configuration_binding["sha256"],
            }
            frozen_plan = {
                **plan(),
                "configuration_manifest": {
                    "path": manifest_path.name,
                    **MODULE.file_binding(manifest_path),
                },
                "canary_manifest": {
                    "path": canary_path.name,
                    **MODULE.file_binding(canary_path),
                },
            }
            request = MODULE.build_request(frozen_plan, root, job)
            self.assertEqual(request["payload"]["canaries"], selected)
            self.assertEqual(
                request["participant_visibilities"],
                ["public", "restricted", "restricted"],
            )
            canaries["canaries"][0]["value"] = "changed-secret"
            MODULE.write_json(canary_path, canaries)
            frozen_plan["canary_manifest"].update(
                MODULE.file_binding(canary_path)
            )
            with self.assertRaisesRegex(MODULE.RunnerError, "frozen job binding"):
                MODULE.build_request(frozen_plan, root, job)

    def test_success_without_response_file_is_not_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            harness = Path(temporary) / "empty-harness.sh"
            harness.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            os.chmod(harness, 0o700)
            process = mock.Mock(pid=987654, wait=mock.Mock(return_value=0),
                                poll=mock.Mock(return_value=0))
            with mock.patch.object(MODULE.subprocess, "Popen", return_value=process), mock.patch.object(
                MODULE, "_process_group_exists", return_value=False
            ), self.assertRaisesRegex(MODULE.RunnerError, "without a regular response"):
                MODULE.invoke_harness(
                    harness.resolve(),
                    {"kind": "fault"},
                    attempt_dir=Path(temporary).resolve() / "retained-attempt",
                    timeout_seconds=5,
                )
            attempt = Path(temporary).resolve() / "retained-attempt"
            self.assertTrue((attempt / "request.json").is_file())
            self.assertTrue((attempt / "stdout.log").is_file())
            self.assertTrue((attempt / "stderr.log").is_file())
            self.assertFalse(MODULE.read_json_file(attempt / "response-outcome.json", "test outcome")["passed"])

    def test_publication_fragment_replays_applicable_final_validators(self) -> None:
        from scripts.tests.private_settlement_release_evidence_test import (
            PrivateSettlementReleaseEvidenceTests,
        )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = PrivateSettlementReleaseEvidenceTests().make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            MODULE.validate_publication_fragment(
                root,
                manifest["artifacts"],
                commit=manifest["commit"],
            )
            benchmark_artifact = next(
                artifact
                for artifact in manifest["artifacts"]
                if artifact["kind"] == "benchmark_report"
            )
            baseline = json.loads(
                (root / benchmark_artifact["path"]).read_text(encoding="utf-8")
            )
            MODULE.validate_benchmark_baseline(baseline, "fixture baseline")
            baseline["passed"] = False
            with self.assertRaises(MODULE.RunnerError):
                MODULE.validate_benchmark_baseline(baseline, "fixture baseline")

    def test_plan_seed_and_sample_minima_cannot_be_weakened(self) -> None:
        with self.assertRaises(MODULE.RunnerError):
            MODULE.verify_seed_policy(tuple(range(9)))
        with self.assertRaises(MODULE.RunnerError):
            MODULE.verify_seed_policy((0, 1, 2, 3, 4, 5, 6, 7, 8, 8))
        with self.assertRaisesRegex(MODULE.RunnerError, "unsigned 64-bit"):
            MODULE.verify_seed_policy(
                (*range(9), MODULE.MAX_SEED + 1)
            )
        with self.assertRaisesRegex(MODULE.RunnerError, "at most"):
            MODULE.verify_seed_policy(tuple(range(MODULE.MAX_FAULT_SEEDS + 1)))

    def test_fault_timeout_covers_activation_and_nonfinalized_expiry_floor(self) -> None:
        expected_floor = (
            MODULE.PRIVACY_PROFILE_ACTIVATION_DELAY_BLOCKS
            + MODULE.FAULT_NONFINALIZED_EXPIRY_TRIALS
            * (MODULE.FAULT_BUNDLE_EXPIRY_BLOCKS + 1)
        ) * MODULE.REAL_PROCESS_BLOCK_CADENCE_SECONDS
        self.assertEqual(MODULE.FAULT_HARNESS_PROTOCOL_FLOOR_SECONDS, expected_floor)
        self.assertGreater(MODULE.DEFAULT_HARNESS_TIMEOUT_SECONDS, expected_floor)

        with self.assertRaisesRegex(MODULE.RunnerError, "protocol floor"):
            MODULE.validate_campaign_timeout(
                [{"kind": "fault"}], expected_floor - 1
            )
        MODULE.validate_campaign_timeout([{"kind": "fault"}], expected_floor)
        MODULE.validate_campaign_timeout([{"kind": "benchmark"}], 1)
        with self.assertRaisesRegex(MODULE.RunnerError, "warmups"):
            MODULE.build_configuration(
                3,
                seeds=tuple(range(10)),
                warmups=MODULE.MAX_WARMUPS + 1,
                measured=MODULE.MIN_MEASURED,
            )
        configuration = MODULE.build_configuration(
            3,
            seeds=tuple(range(10)),
            warmups=5,
            measured=30,
        )
        self.assertTrue(
            configuration["consensus"]["mandatory_signed_rs16_da_rbc"]
        )
        self.assertFalse(configuration["consensus"]["legacy_rbc_bypass_permitted"])
        self.assertTrue(
            configuration["fault_matrix"]["prepare_qc_normalization"][
                "accept_equivalent_subsets_only_for_identical_body"
            ]
        )
        self.assertEqual(
            configuration["topology"]["total_validator_processes"], 16
        )
        self.assertEqual(
            configuration["execution"]["rayon_worker_threads"],
            MODULE.RAYON_WORKER_THREADS,
        )
        self.assertEqual(
            configuration["execution"]["validator_worker_threads"],
            MODULE.VALIDATOR_WORKER_THREADS,
        )
        self.assertEqual(
            configuration["execution"]["cargo_build_jobs"],
            MODULE.CARGO_BUILD_JOBS,
        )
        self.assertEqual(
            configuration["execution"]["cargo_release_codegen_units"],
            MODULE.CARGO_RELEASE_CODEGEN_UNITS,
        )
        self.assertIs(
            configuration["execution"]["cargo_incremental"],
            MODULE.CARGO_INCREMENTAL,
        )
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)
            with self.assertRaisesRegex(MODULE.RunnerError, "outside"):
                MODULE.require_external_output(source / "evidence", source)


class PrivateSettlementFailureRetentionTests(unittest.TestCase):
    """Exercise disk retention with synthetic responses and no real processes."""

    @staticmethod
    def fake_process(*, response_bytes: bytes | None = b"{}", exit_code: int = 0):
        """Return a Popen stand-in that writes real raw files but starts no process."""

        def start(command, **options):
            request_path = Path(command[command.index("--aps-request") + 1])
            response_path = Path(command[command.index("--aps-response") + 1])
            evidence = Path(command[command.index("--aps-evidence-dir") + 1])
            options["stdout"].write(b"retained synthetic stdout\n")
            options["stderr"].write(b"retained synthetic stderr\n")
            if response_bytes is not None:
                response_path.write_bytes(response_bytes)
                response_path.chmod(0o600)
            request = json.loads(request_path.read_text())
            if request.get("kind") != "benchmark":
                (evidence / "raw.bin").write_bytes(b"retained raw capture\x00")
            return mock.Mock(pid=987654, wait=mock.Mock(return_value=exit_code),
                             poll=mock.Mock(return_value=exit_code))

        return start

    @contextmanager
    def execution_fixture(self, root: Path):
        """Stub network semantics while exercising all persistent execution stages.

        This reduced synthetic matrix tests bookkeeping only. Production plan,
        process, report and smoke validators remain mandatory and unchanged.
        """

        source = root / "source"
        source.mkdir()
        harness = root / "synthetic-harness"
        harness.write_text("synthetic executable bytes\n")
        harness.chmod(0o700)
        frozen = {
            "commit": COMMIT,
            "harness": MODULE.file_binding(harness),
            "jobs": [
                {"request_id": f"{index:064x}", "kind": kind, **extra}
                for index, (kind, extra) in enumerate((
                    ("fault", {}), ("benchmark", {}),
                    ("leakage", {"variant": "left"}),
                    ("leakage", {"variant": "right"}),
                ), 1)
            ],
            "requirements": {"seeds": [0], "warmups": 0, "measured": 1,
                             "bootstrap_iterations": 100},
            "benchmark_baseline": None,
        }
        for key, value in (("hardware", {}), ("canary_manifest", {}),
                           ("configuration_manifest", {"configurations": []})):
            path = root / f"{key}.json"
            MODULE.write_json(path, value)
            frozen[key] = {"path": path.name, **MODULE.file_binding(path)}
        plan_path = root / "plan.json"
        MODULE.write_json(plan_path, frozen)
        output = root / "retained-campaign"
        with ExitStack() as stack:
            def patch(target, name, **kwargs):
                return stack.enter_context(mock.patch.object(target, name, **kwargs))

            patch(MODULE, "PARTICIPANTS", new=(3,))
            patch(MODULE, "PROFILES", new=("private",))
            patch(MODULE, "load_plan", return_value=(frozen, root))
            patch(MODULE, "verify_source_checkout")
            smoke = patch(MODULE, "validate_smoke_prerequisite", return_value={"passed": True, "runs": 10})
            patch(MODULE, "build_request", side_effect=lambda _plan, _root, job: dict(job))
            process = patch(MODULE.subprocess, "Popen", side_effect=self.fake_process())
            patch(MODULE, "_process_group_exists", return_value=False)
            patch(MODULE, "materialize_fault_response", return_value=({"synthetic": "fault"}, []))
            benchmark = patch(MODULE, "materialize_benchmark_response", return_value={"synthetic": "benchmark"})
            patch(MODULE, "validate_leakage_response", return_value=({"messages": 1}, []))
            patch(MODULE.fault_report, "load_runs", return_value=[])
            patch(MODULE.fault_report, "input_bindings", return_value=[])
            patch(MODULE.fault_report, "build_report", return_value={"passed": True})
            patch(MODULE.benchmark_report, "load_jsonl", return_value=[])
            patch(MODULE.benchmark_report, "build_report", return_value={"passed": True})
            for name in ("write_fault_csv", "write_benchmark_csv"):
                patch(MODULE, name, side_effect=lambda path, _rows: path.write_text("synthetic\n"))
            patch(MODULE, "differential_pair_manifest", return_value={})
            leakage = patch(MODULE.leakage_audit, "run_audit", return_value={"passed": True})
            patch(MODULE, "validate_publication_fragment")
            yield {
                "plan": frozen, "output": output, "smoke": smoke,
                "process": process, "benchmark": benchmark, "leakage": leakage,
                "execute": lambda: MODULE.execute_plan(
                    plan_path, output, source_root=source, harness=harness,
                    smoke_campaign=root / "synthetic-smoke", timeout_seconds=7_200,
                ),
            }

    def assert_no_success(self, output: Path) -> dict[str, Any]:
        self.assertFalse((output / "release-artifact-fragment-v1.json").exists())
        failure = json.loads((output / "failure.json").read_text())
        self.assertIs(failure["passed"], False)
        self.assertEqual(failure["planned_jobs"], 4)
        return failure

    def test_private_records_and_attempt_directories_are_fresh_and_owner_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            attempt = root / "attempt"
            previous = os.umask(0)
            try:
                MODULE.fresh_private_directory(attempt)
                MODULE.private_record(attempt / "outcome.json", {"passed": False})
            finally:
                os.umask(previous)
            self.assertEqual(stat.S_IMODE(attempt.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE((attempt / "outcome.json").stat().st_mode), 0o600)
            with self.assertRaises(FileExistsError):
                MODULE.fresh_private_directory(attempt)
            with self.assertRaises(FileExistsError):
                MODULE.private_record(attempt / "outcome.json", {"passed": True})
            self.assertFalse(json.loads((attempt / "outcome.json").read_text())["passed"])
            with self.assertRaisesRegex(MODULE.RunnerError, "absolute and canonical"):
                MODULE.fresh_private_directory(Path("relative-attempt"))

    def test_retained_inventory_does_not_follow_harness_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            evidence = root / "evidence"
            evidence.mkdir()
            external = root / "outside.txt"
            external.write_text("not an evidence file")
            (evidence / "link").symlink_to(external)
            (evidence / "raw.bin").write_bytes(b"capture")
            inventory = MODULE.retained_file_inventory(evidence)
            self.assertEqual(inventory[0], {"path": "link", "kind": "symlink", "target": str(external)})
            self.assertEqual(inventory[1]["sha256"], hashlib.sha256(b"capture").hexdigest())

    def test_failed_harness_responses_preserve_all_raw_evidence(self) -> None:
        cases = ((7, b'{"partial":true}', "exited 7"),
                 (0, None, "without a regular response"),
                 (0, b'{"truncated":', "not valid JSON"))
        for exit_code, raw_response, reason in cases:
            with self.subTest(exit_code=exit_code, response=raw_response), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                attempt = root / "attempt"
                with mock.patch.object(MODULE.subprocess, "Popen", side_effect=self.fake_process(
                    response_bytes=raw_response, exit_code=exit_code
                )), mock.patch.object(MODULE, "_process_group_exists", return_value=False), mock.patch.object(
                    MODULE, "_terminate_owned_process_group"
                ) as terminate, self.assertRaisesRegex(MODULE.RunnerError, reason):
                    MODULE.invoke_harness(root / "synthetic-harness", {"kind": "fault", "request_id": "1" * 64},
                                          attempt_dir=attempt, timeout_seconds=1)
                terminate.assert_not_called()
                self.assertEqual((attempt / "stdout.log").read_bytes(), b"retained synthetic stdout\n")
                self.assertEqual((attempt / "stderr.log").read_bytes(), b"retained synthetic stderr\n")
                self.assertEqual((attempt / "evidence" / "raw.bin").read_bytes(), b"retained raw capture\x00")
                if raw_response is None:
                    self.assertFalse((attempt / "response.json").exists())
                else:
                    self.assertEqual((attempt / "response.json").read_bytes(), raw_response)
                self.assertEqual(json.loads((attempt / "process-outcome.json").read_text())["exit_code"], exit_code)
                outcome = json.loads((attempt / "response-outcome.json").read_text())
                self.assertFalse(outcome["passed"])
                self.assertIn("request.json", {entry["path"] for entry in outcome["retained_files"]})

    def test_semantic_failure_retains_earlier_success_and_complete_denominator(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.execution_fixture(Path(temporary).resolve()) as fixture:
            fixture["benchmark"].side_effect = MODULE.RunnerError("synthetic semantic failure")
            with self.assertRaisesRegex(MODULE.RunnerError, "synthetic semantic failure"):
                fixture["execute"]()
            failure = self.assert_no_success(fixture["output"])
            jobs = fixture["plan"]["jobs"]
            self.assertEqual([job["request_id"] for job in failure["completed_jobs"]], [jobs[0]["request_id"]])
            self.assertEqual(failure["failed_job"]["request_id"], jobs[1]["request_id"])
            self.assertEqual(failure["not_started_request_ids"], [job["request_id"] for job in jobs[2:]])
            self.assertEqual(json.loads((fixture["output"] / "frozen-plan.json").read_text()), fixture["plan"])
            attempts = sorted((fixture["output"] / "attempts").iterdir())
            self.assertEqual(len(attempts), 2)
            for index, attempt in enumerate(attempts):
                self.assertEqual((attempt / "response.json").read_bytes(), b"{}")
                self.assertTrue((attempt / "stdout.log").is_file())
                self.assertEqual(json.loads((attempt / "validation-outcome.json").read_text())["passed"], index == 0)
            fixture["smoke"].assert_called_once()
            self.assertEqual(fixture["process"].call_count, 2)
            with self.assertRaisesRegex(MODULE.RunnerError, "already exists"):
                fixture["execute"]()

    def test_report_failure_retains_failing_report_and_all_completed_jobs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.execution_fixture(Path(temporary).resolve()) as fixture:
            report = {"passed": False, "reason": "synthetic canary mismatch"}
            fixture["leakage"].return_value = report
            with self.assertRaisesRegex(MODULE.RunnerError, "leakage audit found"):
                fixture["execute"]()
            failure = self.assert_no_success(fixture["output"])
            self.assertEqual(failure["stage"], "reports")
            self.assertEqual(len(failure["completed_jobs"]), 4)
            self.assertIsNone(failure["failed_job"])
            self.assertEqual(failure["not_started_request_ids"], [])
            report_path = fixture["output"] / "publication" / "reports" / "leakage-report-v1.json"
            self.assertEqual(json.loads(report_path.read_text()), report)
            self.assertEqual(len(list((fixture["output"] / "attempts").glob("*/response.json"))), 4)

    def test_final_smoke_revalidation_failure_never_publishes_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.execution_fixture(Path(temporary).resolve()) as fixture:
            fixture["smoke"].side_effect = [{"passed": True, "runs": 10},
                                           MODULE.RunnerError("smoke evidence changed")]
            with self.assertRaisesRegex(MODULE.RunnerError, "smoke evidence changed"):
                fixture["execute"]()
            failure = self.assert_no_success(fixture["output"])
            self.assertEqual(failure["stage"], "final_revalidation")
            self.assertEqual(len(failure["completed_jobs"]), 4)
            self.assertEqual(fixture["smoke"].call_count, 2)
            self.assertFalse((fixture["output"] / "campaign-validation.json").exists())
            self.assertFalse(list(fixture["output"].glob("*.pending.json")))

    def test_fragment_is_published_only_after_pending_file_and_directory_sync(self) -> None:
        for fail_at in (1, 2, 3):
            with self.subTest(fsync_call=fail_at), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                final = root / "fragment.json"
                real_fsync = os.fsync
                calls = 0

                def sync(descriptor):
                    nonlocal calls
                    calls += 1
                    if calls == fail_at:
                        raise OSError("synthetic fsync failure")
                    real_fsync(descriptor)

                with mock.patch.object(MODULE.os, "fsync", side_effect=sync), self.assertRaisesRegex(OSError, "fsync failure"):
                    MODULE.publish_campaign_fragment(final, {"passed": True})
                self.assertFalse(final.exists())
                self.assertEqual(json.loads((root / "fragment.pending.json").read_text()), {"passed": True})

    def test_fragment_write_failure_retains_partial_pending_and_campaign_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.execution_fixture(Path(temporary).resolve()) as fixture:
            record = MODULE.private_record

            def fail_pending(path, value):
                if path.name == "release-artifact-fragment-v1.pending.json":
                    path.write_bytes(b'{"partial":')
                    raise OSError("synthetic fragment write failure")
                record(path, value)

            with mock.patch.object(MODULE, "private_record", side_effect=fail_pending), self.assertRaisesRegex(
                OSError, "fragment write failure"
            ):
                fixture["execute"]()
            failure = self.assert_no_success(fixture["output"])
            self.assertEqual(failure["stage"], "publication")
            self.assertEqual(len(failure["completed_jobs"]), 4)
            pending = fixture["output"] / "release-artifact-fragment-v1.pending.json"
            self.assertEqual(pending.read_bytes(), b'{"partial":')
            self.assertTrue((fixture["output"] / "campaign-validation.json").is_file())

    def test_fragment_directory_sync_failure_retains_pending_and_campaign_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.execution_fixture(Path(temporary).resolve()) as fixture:
            final = fixture["output"] / "release-artifact-fragment-v1.json"
            real_fsync = os.fsync
            failed = False

            def fail_publication_sync(descriptor):
                nonlocal failed
                if final.exists() and not failed:
                    failed = True
                    raise OSError("synthetic publication directory sync failure")
                real_fsync(descriptor)

            with mock.patch.object(MODULE.os, "fsync", side_effect=fail_publication_sync), self.assertRaisesRegex(
                OSError, "publication directory sync failure"
            ):
                fixture["execute"]()
            failure = self.assert_no_success(fixture["output"])
            self.assertEqual(failure["stage"], "publication")
            pending = fixture["output"] / "release-artifact-fragment-v1.pending.json"
            self.assertTrue(json.loads(pending.read_text())["real_process_campaign_complete"])
            self.assertEqual(len(failure["completed_jobs"]), 4)

    def test_successful_campaign_retains_attempts_and_publishes_atomically(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.execution_fixture(Path(temporary).resolve()) as fixture:
            final = fixture["execute"]()
            self.assertEqual(final.name, "release-artifact-fragment-v1.json")
            self.assertTrue(json.loads(final.read_text())["real_process_campaign_complete"])
            self.assertFalse((fixture["output"] / "failure.json").exists())
            self.assertEqual(len(list((fixture["output"] / "attempts").glob("*/validation-outcome.json"))), 4)
            self.assertEqual(final.read_bytes(), (fixture["output"] / "release-artifact-fragment-v1.pending.json").read_bytes())
            self.assertEqual(stat.S_IMODE(final.stat().st_mode), 0o600)
            self.assertEqual(fixture["smoke"].call_count, 2)


if __name__ == "__main__":
    unittest.main()
