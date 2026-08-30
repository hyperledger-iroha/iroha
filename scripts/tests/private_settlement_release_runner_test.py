"""Tests for the fail-closed AtomicPrivateSettlementV1 release runner."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

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
        "staged_locks",
    )
    baseline = {name: 0 for name in count_names}
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
    empty_staged = _iroha_hash_literal("3" * 64)
    for index, (counts, ledger) in enumerate(
        (
            (baseline, _iroha_hash_literal("1" * 64)),
            (baseline, _iroha_hash_literal("1" * 64)),
            (final, _iroha_hash_literal("2" * 64)),
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
                "staged_lock_commitment": empty_staged,
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
                "staged_lock_commitment": empty_staged,
                "counts": counts,
            }
        )
    return json.dumps(
        {"version": 1, "peer_index": peer_index, "observations": observations},
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
    public_ports = list(range(30_000, 30_004))
    restricted_ports = list(range(40_000, 40_012))
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
        "peer_index": 0,
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


def _state_counts(participants: int, *, finalized: bool, staged: bool = False) -> dict[str, int]:
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
    if staged:
        counts.update(
            {
                "staged_pool_heads": participants,
                "staged_nullifiers": participants * 2,
                "staged_output_commitments": participants * 3,
                "staged_locks": participants,
            }
        )
    return counts


def _state_observation(
    participants: int,
    peer_index: int,
    *,
    label: str,
    finalized: bool,
) -> dict[str, Any]:
    nonfinalized = label == "nonfinalized"
    ledger = ("4" if finalized else "1") * 64
    staged = ("3" if nonfinalized else "2") * 64
    response = {
        "format_version": 1,
        "height": 10 + (1 if finalized else 0),
        "commitment": f"{(peer_index % 9) + 1:064x}",
        "ledger_commitment": ledger,
        "staged_lock_commitment": staged,
        "counts": _state_counts(
            participants, finalized=finalized, staged=nonfinalized
        ),
    }
    response_bytes = MODULE.canonical_bytes(response)
    return {
        "peer_index": peer_index,
        "response_sha256": MODULE.hashlib.sha256(response_bytes).hexdigest(),
        "response_hex": response_bytes.hex(),
        **{field: response[field] for field in ("height", "commitment", "ledger_commitment", "staged_lock_commitment", "counts")},
    }


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
        "commit_qc": "after_private_settlement_commit_qc_fsync",
        "kura_append": "after_private_settlement_kura_append",
        "wsv_application": "after_private_settlement_wsv_application",
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
                for carrier_action in ("hold", "heal"):
                    for peer_index in range(MODULE.GLOBAL_VALIDATORS):
                        trial_controls.append(
                            _consensus_carrier_occurrence(
                                peer_index, carrier_action, revision
                            )
                        )
                        revision += 1
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
            else:
                phase = crash_phases[trial["boundary"]]
                target_peer = 0 if index in {4, 5} else 4
                restart_type = (
                    "global_restart" if index in {4, 5} else "validator_restart"
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
                if index >= 4:
                    expected_after_state = "finalized"
            controls.append(
                {
                    "record": record_id,
                    "bundle_id": bundle_id,
                    "participants": participants,
                    "seed": seed,
                    "run": run,
                    "collection": collection,
                    "trial_index": index,
                    "controls": trial_controls,
                }
            )
            snapshots = []
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
                            )
                            for peer_index in range(peer_count)
                        ],
                    }
                )
            continuous_observations = []
            for peer_index in range(peer_count):
                first_response = snapshots[0]["validators"][peer_index]["response_sha256"]
                middle_response = snapshots[1]["validators"][peer_index]["response_sha256"]
                last_response = snapshots[2]["validators"][peer_index]["response_sha256"]
                continuous_observations.append(
                    {
                        "peer_index": peer_index,
                        "check_count": 3,
                        "first_response_sha256": first_response,
                        "last_response_sha256": last_response,
                        "response_chain_sha256": MODULE.hashlib.sha256(
                            bytes.fromhex(first_response)
                            + bytes.fromhex(middle_response)
                            + bytes.fromhex(last_response)
                        ).hexdigest(),
                        "baseline_observations": (
                            2 if expected_after_state == "finalized" else 3
                        ),
                        "finalized_observations": (
                            1 if expected_after_state == "finalized" else 0
                        ),
                    }
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
                    "continuous_checks": peer_count * 3,
                    "continuous_observations": continuous_observations,
                    "partial_visibility_observed": False,
                    "partial_spendable_observations": 0,
                    "snapshots": snapshots,
                }
            )
            total_checks += peer_count * 3

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


class PrivateSettlementReleaseRunnerTests(unittest.TestCase):
    """Exercise deterministic planning and fail-closed response materialization."""

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
            too_short = copy.deepcopy(rows)
            too_short[0]["continuous_observations"][0]["check_count"] = 2
            with self.assertRaisesRegex(MODULE.RunnerError, "lacks a live polling"):
                MODULE.validate_fault_observation_records(
                    too_short, participants=3, seed=7, run=2
                )
            unanchored = copy.deepcopy(rows)
            unanchored[0]["continuous_observations"][0][
                "first_response_sha256"
            ] = "f" * 64
            with self.assertRaisesRegex(MODULE.RunnerError, "exemplar endpoints"):
                MODULE.validate_fault_observation_records(
                    unanchored, participants=3, seed=7, run=2
                )
            unclassified = copy.deepcopy(rows)
            unclassified[0]["continuous_observations"][0][
                "baseline_observations"
            ] -= 1
            with self.assertRaisesRegex(MODULE.RunnerError, "unclassified observation"):
                MODULE.validate_fault_observation_records(
                    unclassified, participants=3, seed=7, run=2
                )
            reused_bundle = copy.deepcopy(rows)
            reused_bundle[1]["bundle_id"] = reused_bundle[0]["bundle_id"]
            with self.assertRaisesRegex(MODULE.RunnerError, "reuses an APS bundle"):
                MODULE.validate_fault_observation_records(
                    reused_bundle, participants=3, seed=7, run=2
                )
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
                by_record["n3:s7:r2:crash_recoveries:6"]
            )
            for control in wrong_receipt_target["controls"]:
                control["peer_index"] = 0
                if control["control_type"] == "validator_restart":
                    control["control_type"] = "global_restart"
            with self.assertRaisesRegex(MODULE.RunnerError, "persistence cut"):
                MODULE.validate_fault_trial_control_semantics(
                    wrong_receipt_target,
                    collection="crash_recoveries",
                    trial=payload["crash_recoveries"][6],
                    label="receipt",
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
            raw["height"] = observation["height"]
            raw["staged_lock_commitment"] = observation["staged_lock_commitment"]
            raw["counts"] = observation["counts"]
            encoded = json.dumps(
                raw, sort_keys=True, separators=(",", ":")
            ).encode()
            observation["response_hex"] = encoded.hex()
            observation["response_sha256"] = hashlib.sha256(encoded).hexdigest()

        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / "restricted.bin"
            valid_source = _atomicity_evidence(0, 3)
            _write_restricted_archive(
                archive, [("atomicity_observation", "peer-000.json", valid_source)]
            )
            rows = MODULE._validate_restricted_leakage_source_archive(archive)[
                "atomicity_observation"
            ]["rows"]
            self.assertEqual(
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 1),
                3,
            )

            negative_height = json.loads(valid_source)
            negative_height["observations"][0]["height"] = -1
            rewrite_projection(negative_height["observations"][0])
            _write_restricted_archive(
                archive,
                [
                    (
                        "atomicity_observation",
                        "peer-000.json",
                        json.dumps(
                            negative_height, sort_keys=True, separators=(",", ":")
                        ).encode(),
                    )
                ],
            )
            rows = MODULE._validate_restricted_leakage_source_archive(archive)[
                "atomicity_observation"
            ]["rows"]
            with self.assertRaisesRegex(MODULE.RunnerError, "height"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 1)

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
            _write_restricted_archive(
                archive,
                [
                    (
                        "atomicity_observation",
                        "peer-000.json",
                        json.dumps(
                            terminal_staged, sort_keys=True, separators=(",", ":")
                        ).encode(),
                    )
                ],
            )
            rows = MODULE._validate_restricted_leakage_source_archive(archive)[
                "atomicity_observation"
            ]["rows"]
            with self.assertRaisesRegex(MODULE.RunnerError, "retained staged locks"):
                MODULE._validate_leakage_atomicity_observations(archive, rows, 3, 1)

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
            with self.assertRaisesRegex(
                MODULE.RunnerError, "without a regular response"
            ):
                MODULE.invoke_harness(
                    harness,
                    {"kind": "fault"},
                    timeout_seconds=5,
                )

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
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)
            with self.assertRaisesRegex(MODULE.RunnerError, "outside"):
                MODULE.require_external_output(source / "evidence", source)


if __name__ == "__main__":
    unittest.main()
