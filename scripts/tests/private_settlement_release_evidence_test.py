"""Tests for the AtomicPrivateSettlementV1 release evidence validator."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_release_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_release_evidence", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

FIXTURE_SOURCE_LOCKFILE_PAYLOAD = b"# exact release Cargo.lock\n"
FIXTURE_SOURCE_BLOB = hashlib.sha1(
    b"blob "
    + str(len(FIXTURE_SOURCE_LOCKFILE_PAYLOAD)).encode("ascii")
    + b"\0"
    + FIXTURE_SOURCE_LOCKFILE_PAYLOAD
).hexdigest()
FIXTURE_SOURCE_ENTRY = {
    "path": "Cargo.lock",
    "mode": "100644",
    "object": FIXTURE_SOURCE_BLOB,
}
FIXTURE_SOURCE_TREE = MODULE._git_inventory_tree_oid_v1(
    {"Cargo.lock": ("100644", FIXTURE_SOURCE_BLOB)}, 40
)
FIXTURE_SOURCE_COMMIT_PAYLOAD = (
    f"tree {FIXTURE_SOURCE_TREE}\n"
    "author Release Fixture <fixture@example.invalid> 1787932800 +0000\n"
    "committer Release Fixture <fixture@example.invalid> 1787932800 +0000\n"
    "\nAtomicPrivateSettlementV1 release fixture\n"
).encode("utf-8")
RELEASE_COMMIT = hashlib.sha1(
    b"commit "
    + str(len(FIXTURE_SOURCE_COMMIT_PAYLOAD)).encode("ascii")
    + b"\0"
    + FIXTURE_SOURCE_COMMIT_PAYLOAD
).hexdigest()


def fixture_source_seal(payload: bytes) -> bytes:
    """Build the one-member deterministic source seal used by the fixture."""

    path = b"Cargo.lock"
    return b"".join(
        (
            MODULE._SOURCE_SEAL_DOMAIN,
            struct.pack(">Q", 1),
            struct.pack(">Q", len(path)),
            path,
            b"F",
            struct.pack(">I", 0o644),
            struct.pack(">Q", len(payload)),
            payload,
        )
    )


class PrivateSettlementReleaseEvidenceTests(unittest.TestCase):
    """Exercise exact qualification, audit, inventory, and digest gates."""

    def make_bundle(self, root: Path) -> Path:
        release_binary_path = Path("evidence") / "bin" / "iroha3d"
        release_binary_payload = b"reproducible iroha3d candidate\n"
        release_binary_digest = hashlib.sha256(release_binary_payload).hexdigest()
        release_target = "aarch64-apple-darwin"
        source_archive_path = Path("evidence") / "source.seal"
        source_archive_payload = fixture_source_seal(FIXTURE_SOURCE_LOCKFILE_PAYLOAD)
        source_archive_digest = hashlib.sha256(source_archive_payload).hexdigest()
        source_lockfile_path = Path("evidence") / "Cargo.lock"
        source_lockfile_payload = FIXTURE_SOURCE_LOCKFILE_PAYLOAD
        source_lockfile_digest = hashlib.sha256(source_lockfile_payload).hexdigest()
        source_commit_path = Path("evidence") / "source.commit"
        source_commit_payload = FIXTURE_SOURCE_COMMIT_PAYLOAD
        source_commit_digest = hashlib.sha256(source_commit_payload).hexdigest()
        audit_report_path = Path("evidence") / "audit_report.txt"
        audit_report_payload = b"independent cryptographic audit report\n"
        audit_report_digest = hashlib.sha256(audit_report_payload).hexdigest()
        hardware_description_path = Path("evidence") / "hardware_description.json"
        hardware_description = {
            "version": 1,
            "protocol": MODULE.PROTOCOL,
            "commit": RELEASE_COMMIT,
            "collected_at_utc": "2026-08-29T00:00:00Z",
            "host_id": "bck26-lab-host-01",
            "operating_system": "macOS 15.6",
            "kernel": "Darwin 24.6.0",
            "architecture": "arm64",
            "cpu_model": "Apple M4 Max",
            "physical_cores": 16,
            "logical_cores": 16,
            "memory_bytes": 137_438_953_472,
            "storage_model": "pinned local NVMe",
            "network_description": "isolated 10 GbE laboratory fabric",
            "clock_policy": "performance cores pinned; synchronized monotonic clocks",
            "power_profile": "AC power; high-performance mode",
            "virtualized": False,
            "passed": True,
        }
        hardware_description_payload = (
            json.dumps(hardware_description, sort_keys=True)
            + "\n"
        ).encode()
        hardware_description_digest = hashlib.sha256(
            hardware_description_payload
        ).hexdigest()
        hardware_profile_digest = MODULE._hardware_profile_sha256(
            hardware_description
        )
        configuration_paths = {
            participants: Path("evidence")
            / "configurations"
            / f"private-settlement-n{participants}.toml"
            for participants in MODULE.REQUIRED_PARTICIPANTS
        }
        configuration_payloads = {
            participants: (
                f"# AtomicPrivateSettlementV1 N={participants}\n"
                "validators_per_dataspace = 4\n"
                'quorum = "3-of-4"\n'
                "mandatory_signed_rs16_da_rbc = true\n"
            ).encode()
            for participants in MODULE.REQUIRED_PARTICIPANTS
        }
        configuration_digests = {
            participants: hashlib.sha256(payload).hexdigest()
            for participants, payload in configuration_payloads.items()
        }
        configuration_manifest_path = Path("evidence") / "configuration_manifest.json"
        configuration_manifest_payload = (
            json.dumps(
                {
                    "version": 1,
                    "protocol": MODULE.PROTOCOL,
                    "commit": RELEASE_COMMIT,
                    "configurations": [
                        {
                            "participants": participants,
                            "validators_per_dataspace": 4,
                            "quorum": "3-of-4",
                            "mandatory_signed_rs16_da_rbc": True,
                            "path": configuration_paths[participants].as_posix(),
                            "sha256": configuration_digests[participants],
                            "bytes": len(configuration_payloads[participants]),
                        }
                        for participants in MODULE.REQUIRED_PARTICIPANTS
                    ],
                    "passed": True,
                },
                sort_keys=True,
            )
            + "\n"
        ).encode()

        fault_transcript_entries: list[dict[str, Any]] = []
        fault_capture_entries: list[dict[str, Any]] = []
        fault_payloads: dict[tuple[int, int], dict[str, Any]] = {}
        from scripts.tests.private_settlement_release_runner_test import (
            MODULE as RUNNER_MODULE,
            fault_payload as runner_fault_payload,
            leakage_payload as runner_leakage_payload,
            response as runner_response,
            write_fault_evidence,
        )

        for participants in MODULE.REQUIRED_PARTICIPANTS:
            for seed in range(MODULE.REQUIRED_SEEDS_PER_PARTICIPANT):
                payload = runner_fault_payload(participants)
                with tempfile.TemporaryDirectory() as evidence_temporary:
                    evidence_dir = Path(evidence_temporary)
                    write_fault_evidence(
                        evidence_dir,
                        payload,
                        participants=participants,
                        seed=seed,
                        run=seed,
                    )
                    fault_transcript_entries.extend(
                        json.loads(line)
                        for line in (
                            evidence_dir / "fault-control.jsonl"
                        ).read_text(encoding="utf-8").splitlines()
                    )
                    fault_capture_entries.extend(
                        json.loads(line)
                        for line in (
                            evidence_dir / "fault-observations.jsonl"
                        ).read_text(encoding="utf-8").splitlines()
                    )
                fault_payloads[(participants, seed)] = payload

        fault_transcript_path = Path("evidence") / "logs" / "fault-control.jsonl"
        fault_transcript_payload = (
            "\n".join(
                json.dumps(
                    entry, sort_keys=True
                )
                for entry in fault_transcript_entries
            )
            + "\n"
        ).encode()
        fault_transcript_digest = hashlib.sha256(fault_transcript_payload).hexdigest()
        fault_capture_path = Path("evidence") / "captures" / "fault-atomicity.jsonl"
        fault_capture_payload = (
            "\n".join(
                json.dumps(
                    entry, sort_keys=True
                )
                for entry in fault_capture_entries
            )
            + "\n"
        ).encode()
        fault_capture_digest = hashlib.sha256(fault_capture_payload).hexdigest()

        def fault_record(participants: int, seed: int) -> dict[str, Any]:
            payload = json.loads(json.dumps(fault_payloads[(participants, seed)]))
            payload.pop("prepare_qc_normalization")
            for collection in (
                "loss_trials",
                "phase_cut_partitions",
                "crash_recoveries",
            ):
                for index, trial in enumerate(payload[collection]):
                    record_id = (
                        f"n{participants}:s{seed}:r{seed}:{collection}:{index}"
                    )
                    trial.update(
                        {
                            "control_transcript_sha256": fault_transcript_digest,
                            "control_transcript_record": record_id,
                            "observation_capture_sha256": fault_capture_digest,
                            "observation_capture_record": record_id,
                        }
                    )
            return {
                "version": 1,
                "protocol": MODULE.PROTOCOL,
                "commit": RELEASE_COMMIT,
                "hardware_sha256": hardware_description_digest,
                "configuration_sha256": configuration_digests[participants],
                "participants": participants,
                "seed": seed,
                "run": seed,
                "validators_per_dataspace": 4,
                "quorum": "3-of-4",
                "mandatory_signed_rs16_da_rbc": True,
                "authenticated_message_control": True,
                **payload,
            }

        fault_raw_path = Path("evidence") / "real_network_fault_raw.jsonl"
        fault_raw_payload = (
            "\n".join(
                json.dumps(fault_record(participants, seed), sort_keys=True)
                for participants in MODULE.REQUIRED_PARTICIPANTS
                for seed in range(10)
            )
            + "\n"
        ).encode()
        fault_raw_digest = hashlib.sha256(fault_raw_payload).hexdigest()
        artifacts: list[dict[str, Any]] = []
        for path, payload, kind in (
            (fault_transcript_path, fault_transcript_payload, "operator_log"),
            (fault_capture_path, fault_capture_payload, "sanitized_capture"),
        ):
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
            artifacts.append(
                {
                    "kind": kind,
                    "path": path.as_posix(),
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "bytes": len(payload),
                }
            )
        for kind in MODULE.REQUIRED_ARTIFACT_KINDS:
            path = Path("evidence") / f"{kind}.txt"
            if kind == "audit_attestation":
                path = Path("evidence") / "audit_attestation.json"
                payload = (
                    json.dumps(
                        {
                            "version": 1,
                            "protocol": MODULE.PROTOCOL,
                            "commit": RELEASE_COMMIT,
                            "independent": True,
                            "organization": "Independent Cryptography Laboratory",
                            "conclusion": "passed",
                            "scopes": list(MODULE.REQUIRED_AUDIT_SCOPES),
                            "issued_at_utc": "2026-08-29T00:00:00Z",
                            "report_identifier": "ICL-APS-V1-2026",
                            "report": {
                                "path": audit_report_path.as_posix(),
                                "sha256": audit_report_digest,
                                "bytes": len(audit_report_payload),
                            },
                            "open_critical_findings": 0,
                            "open_high_findings": 0,
                            "passed": True,
                        },
                        sort_keys=True,
                    )
                    + "\n"
                ).encode()
            elif kind == "audit_report":
                path = audit_report_path
                payload = audit_report_payload
            elif kind == "reproducible_build_report":
                path = Path("evidence") / "reproducible_build_report.json"
                build_artifact = {
                    "target": release_target,
                    "name": "iroha3d",
                    "sha256": release_binary_digest,
                    "bytes": len(release_binary_payload),
                }
                builds = []
                for builder_index in range(2):
                    transcript_path = (
                        Path("evidence")
                        / "logs"
                        / f"reproducible-build-{builder_index}.log"
                    )
                    transcript_payload = (
                        f"reproducible build {builder_index} completed\n".encode()
                    )
                    transcript_destination = root / transcript_path
                    transcript_destination.parent.mkdir(parents=True, exist_ok=True)
                    transcript_destination.write_bytes(transcript_payload)
                    transcript_digest = hashlib.sha256(transcript_payload).hexdigest()
                    artifacts.append(
                        {
                            "kind": "operator_log",
                            "path": transcript_path.as_posix(),
                            "sha256": transcript_digest,
                            "bytes": len(transcript_payload),
                        }
                    )
                    builds.append(
                        {
                            "builder_id": f"independent-builder-{builder_index}",
                            "environment_sha256": str(builder_index + 1) * 64,
                            "artifacts": [dict(build_artifact)],
                            "transcript": {
                                "path": transcript_path.as_posix(),
                                "sha256": transcript_digest,
                                "bytes": len(transcript_payload),
                            },
                        }
                    )
                payload = (
                    json.dumps(
                        {
                            "version": 1,
                            "protocol": MODULE.PROTOCOL,
                            "commit": RELEASE_COMMIT,
                            "source_date_epoch": 1787932800,
                            "targets": [release_target],
                            "archived_artifacts": [
                                {
                                    **build_artifact,
                                    "path": release_binary_path.as_posix(),
                                }
                            ],
                            "builds": builds,
                            "passed": True,
                        },
                        sort_keys=True,
                    )
                    + "\n"
                ).encode()
            elif kind in {
                "auditor_key_custody_report",
                "formal_model_report",
                "randomized_seed_report",
                "soak_report",
                "source_manifest",
            }:
                path = Path("evidence") / f"{kind}.json"
                transcript_path = Path("evidence") / "logs" / f"{kind}.log"
                transcript_payload = f"{kind} completed\n".encode()
                transcript_destination = root / transcript_path
                transcript_destination.parent.mkdir(parents=True, exist_ok=True)
                transcript_destination.write_bytes(transcript_payload)
                transcript_digest = hashlib.sha256(transcript_payload).hexdigest()
                artifacts.append(
                    {
                        "kind": "operator_log",
                        "path": transcript_path.as_posix(),
                        "sha256": transcript_digest,
                        "bytes": len(transcript_payload),
                    }
                )
                transcript = {
                    "path": transcript_path.as_posix(),
                    "sha256": transcript_digest,
                    "bytes": len(transcript_payload),
                }
                if kind == "source_manifest":
                    report = {
                        "version": 1,
                        "protocol": MODULE.PROTOCOL,
                        "commit": RELEASE_COMMIT,
                        "tree": FIXTURE_SOURCE_TREE,
                        "worktree_clean": True,
                        "tracked_file_count": 1,
                        "modified": [],
                        "untracked": [],
                        "source_archive": {
                            "path": source_archive_path.as_posix(),
                            "sha256": source_archive_digest,
                            "bytes": len(source_archive_payload),
                        },
                        "source_commit": {
                            "path": source_commit_path.as_posix(),
                            "sha256": source_commit_digest,
                            "bytes": len(source_commit_payload),
                        },
                        "source_lockfile": {
                            "path": source_lockfile_path.as_posix(),
                            "sha256": source_lockfile_digest,
                            "bytes": len(source_lockfile_payload),
                        },
                        "passed": True,
                        "transcript": transcript,
                    }
                elif kind == "auditor_key_custody_report":
                    report = {
                        "version": 1,
                        "protocol": MODULE.PROTOCOL,
                        "commit": RELEASE_COMMIT,
                        "provider": "Independent HSM Laboratory",
                        "hsm_or_kms_backed": True,
                        "signing_encryption_keys_separate": True,
                        "signing_consensus_keys_separate": True,
                        "encryption_consensus_keys_separate": True,
                        "rotation_tested": True,
                        "retired_key_retention_tested": True,
                        "capsule_rewrap_tested": False,
                        "recovery_tested": True,
                        "retention_period_days": 3650,
                        "findings": [],
                        "passed": True,
                        "transcript": transcript,
                    }
                elif kind == "formal_model_report":
                    report = {
                        "version": 1,
                        "protocol": MODULE.PROTOCOL,
                        "commit": RELEASE_COMMIT,
                        "tool": "TLC",
                        "tool_version": "2.19",
                        "tool_sha256": "b" * 64,
                        "model_sha256": "c" * 64,
                        "configurations": [
                            {
                                "name": name,
                                "expected_outcome": outcome,
                                "observed_outcome": outcome,
                                "generated_states": 1,
                                "distinct_states": 1,
                                "depth": 1,
                            }
                            for name, outcome in MODULE.REQUIRED_FORMAL_CONFIGURATIONS
                        ],
                        "passed": True,
                        "transcript": transcript,
                    }
                elif kind == "randomized_seed_report":
                    report = {
                        "version": 1,
                        "protocol": MODULE.PROTOCOL,
                        "commit": RELEASE_COMMIT,
                        "seeds": list(range(10)),
                        "runs_per_seed": 1,
                        "failures": [],
                        "passed": True,
                        "transcript": transcript,
                    }
                else:
                    report = {
                        "version": 1,
                        "protocol": MODULE.PROTOCOL,
                        "commit": RELEASE_COMMIT,
                        "duration_seconds": 7200,
                        "iterations": 100,
                        "seeds": [0, 1],
                        "validators_per_dataspace": 4,
                        "quorum": "3-of-4",
                        "mandatory_signed_rs16_da_rbc": True,
                        "max_unavailable_per_committee": 1,
                        "partial_visibility_observations": 0,
                        "partial_spendable_observations": 0,
                        "failures": [],
                        "passed": True,
                        "transcript": transcript,
                    }
                payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            elif kind in MODULE.PASS_REPORT_GATES:
                gate = MODULE.PASS_REPORT_GATES[kind]
                path = Path("evidence") / f"{kind}.json"
                transcript_path = Path("evidence") / "logs" / f"{gate}.log"
                transcript_payload = f"{gate} completed with exit code 0\n".encode()
                transcript_destination = root / transcript_path
                transcript_destination.parent.mkdir(parents=True, exist_ok=True)
                transcript_destination.write_bytes(transcript_payload)
                transcript_digest = hashlib.sha256(transcript_payload).hexdigest()
                artifacts.append(
                    {
                        "kind": "operator_log",
                        "path": transcript_path.as_posix(),
                        "sha256": transcript_digest,
                        "bytes": len(transcript_payload),
                    }
                )
                if kind == "release_inventory_report":
                    details: dict[str, object] = {
                        "tree": FIXTURE_SOURCE_TREE,
                        "entries": [dict(FIXTURE_SOURCE_ENTRY)],
                    }
                elif kind == "sdk_test_report":
                    details = {
                        "sdks": {
                            sdk: {
                                "tests": 1,
                                "failures": 0,
                                "skipped": 0,
                                "package_smoke": True,
                                "passed": True,
                            }
                            for sdk in (
                                "rust",
                                "cli",
                                "kotlin",
                                "java",
                                "swift",
                                "python",
                                "javascript",
                            )
                        }
                    }
                else:
                    details = {"checks": 1, "failures": 0, "skipped": 0}
                payload = (
                    json.dumps(
                        {
                            "version": 1,
                            "protocol": MODULE.PROTOCOL,
                            "commit": RELEASE_COMMIT,
                            "gate": gate,
                            "command": f"release-gate {gate}",
                            "exit_code": 0,
                            "passed": True,
                            "started_at_utc": "2026-08-29T00:00:00Z",
                            "duration_seconds": 1.0,
                            "details": details,
                            "transcript": {
                                "path": transcript_path.as_posix(),
                                "sha256": transcript_digest,
                                "bytes": len(transcript_payload),
                            },
                        },
                        sort_keys=True,
                    )
                    + "\n"
                ).encode()
            elif kind == "release_binary":
                path = release_binary_path
                payload = release_binary_payload
            elif kind == "source_archive":
                path = source_archive_path
                payload = source_archive_payload
            elif kind == "source_commit":
                path = source_commit_path
                payload = source_commit_payload
            elif kind == "source_lockfile":
                path = source_lockfile_path
                payload = source_lockfile_payload
            elif kind == "hardware_description":
                path = hardware_description_path
                payload = hardware_description_payload
            elif kind == "configuration":
                participants = MODULE.REQUIRED_PARTICIPANTS[0]
                path = configuration_paths[participants]
                payload = configuration_payloads[participants]
            elif kind == "configuration_manifest":
                path = configuration_manifest_path
                payload = configuration_manifest_payload
            elif kind == "differential_pair_manifest":
                path = Path("evidence") / "differential_pair_manifest.json"
                payload = b"pending differential pairs\n"
            elif kind == "sbom":
                path = Path("evidence") / "sbom.cdx.json"
                payload = (
                    json.dumps(
                        {
                            "bomFormat": "CycloneDX",
                            "specVersion": "1.5",
                            "serialNumber": "urn:uuid:00000000-0000-4000-8000-000000000001",
                            "version": 1,
                            "metadata": {
                                "component": {
                                    "type": "application",
                                    "name": "iroha",
                                    "version": "3.0.0",
                                },
                                "properties": [
                                    {
                                        "name": "iroha.git.commit",
                                        "value": RELEASE_COMMIT,
                                    }
                                ],
                            },
                            "components": [
                                {
                                    "type": "file",
                                    "name": "iroha3d",
                                    "version": "3.0.0",
                                    "hashes": [
                                        {
                                            "alg": "SHA-256",
                                            "content": release_binary_digest,
                                        }
                                    ],
                                }
                            ],
                        },
                        sort_keys=True,
                    )
                    + "\n"
                ).encode()
            elif kind == "canary_manifest":
                path = Path("evidence") / "canaries.json"
                canary_manifest = RUNNER_MODULE.build_canary_manifest(RELEASE_COMMIT)
                canary_manifest["canaries"] = [
                    entry
                    for entry in canary_manifest["canaries"]
                    if not entry["name"].endswith("_variant_b")
                ]
                payload = (
                    json.dumps(canary_manifest, sort_keys=True) + "\n"
                ).encode()
            elif kind == "traffic_count_manifest":
                path = Path("evidence") / "traffic-counts-left.json"
                payload = (
                    json.dumps(
                        {
                            "version": 1,
                            "channels": {
                                "torii_request_packets": 1,
                                "torii_response_packets": 1,
                                "public_p2p_packets": 1,
                                "restricted_p2p_packets": 1,
                                "block_messages": 1,
                                "query_responses": 16,
                                "event_records": 1,
                                "log_records": 16,
                                "telemetry_records": 16,
                            },
                        },
                        sort_keys=True,
                    )
                    + "\n"
                ).encode()
            elif kind == "benchmark_raw":
                path = Path("evidence") / "benchmark_raw.jsonl"
                rows = []
                for profile in MODULE._BENCHMARK_PROFILES:
                    for participants in MODULE.REQUIRED_PARTICIPANTS:
                        for warmup, count in ((True, 5), (False, 30)):
                            for run in range(count):
                                stages = {
                                    stage: float(run + 1)
                                    for stage in (
                                        MODULE._BENCHMARK_PRIVATE_STAGES
                                        if profile == "private"
                                        else ("global_finality", "end_to_end")
                                    )
                                }
                                rows.append(
                                    {
                                        "version": 1,
                                        "protocol": MODULE.PROTOCOL,
                                        "commit": RELEASE_COMMIT,
                                        "hardware_sha256": hardware_description_digest,
                                        "hardware_profile_sha256": hardware_profile_digest,
                                        "configuration_sha256": configuration_digests[
                                            participants
                                        ],
                                        "profile": profile,
                                        "participants": participants,
                                        "seed": run % 2,
                                        "run": run,
                                        "warmup": warmup,
                                        "stages_ms": stages,
                                        **{
                                            field: float(run + 1)
                                            for field in MODULE._BENCHMARK_RESOURCE_FIELDS
                                        },
                                    }
                                )
                payload = (
                    "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n"
                ).encode()
            elif kind == "benchmark_report":
                path = Path("evidence") / "benchmark_report.json"
                report = MODULE._regenerate_benchmark_report(
                    [root / "evidence" / "benchmark_raw.jsonl"], 100
                )
                report["regressions"] = []
                report["passed"] = True
                payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            elif kind == "real_network_fault_raw":
                path = fault_raw_path
                payload = fault_raw_payload
            elif kind == "real_network_fault_report":
                path = Path("evidence") / "real_network_fault_report.json"
                payload = (
                    json.dumps(
                        {
                            "version": 1,
                            "protocol": MODULE.PROTOCOL,
                            "commit": RELEASE_COMMIT,
                            "raw_inputs": [
                                {
                                    "sha256": fault_raw_digest,
                                    "bytes": len(fault_raw_payload),
                                }
                            ],
                            "environment": {
                                "hardware_sha256": hardware_description_digest,
                                "configuration_sha256_by_participants": {
                                    str(participants): configuration_digests[
                                        participants
                                    ]
                                    for participants in MODULE.REQUIRED_PARTICIPANTS
                                },
                            },
                            "requirements": {
                                "participants": list(MODULE.REQUIRED_PARTICIPANTS),
                                "minimum_seeds_per_participant": 10,
                                "validators_per_dataspace": 4,
                                "quorum": "3-of-4",
                                "loss_phases": ["restricted_da", "prepare", "commit"],
                                "loss_percentages": list(
                                    MODULE.REQUIRED_LOSS_PERCENTAGES
                                ),
                                "phase_cuts": [
                                    "da_before_availability_qc",
                                    "prepare_before_complete_barrier",
                                    "commit_before_complete_barrier",
                                    "carrier_before_global_finality",
                                ],
                                "crash_boundaries": list(
                                    MODULE.REQUIRED_CRASH_BOUNDARIES
                                ),
                            },
                            "matrix": {
                                str(participants): {
                                    "runs": 10,
                                    "seeds": list(range(10)),
                                }
                                for participants in MODULE.REQUIRED_PARTICIPANTS
                            },
                            "passed": True,
                        },
                        sort_keys=True,
                    )
                    + "\n"
                ).encode()
            elif kind == "leakage_report":
                path = Path("evidence") / "leakage_report.json"
                payload = b"pending leakage report\n"
            elif kind == "leakage_capture_provenance":
                path = (
                    Path("evidence")
                    / "leakage"
                    / "capture-provenance-left.json"
                )
                payload = b"pending leakage provenance\n"
            elif kind == "torii_capture":
                path = Path("evidence") / "torii_capture.json"
                payload = b'{"bundle":"opaque","value":"left"}\n'
            else:
                payload = f"authenticated {kind} evidence\n".encode()
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
            artifacts.append(
                {
                    "kind": kind,
                    "path": path.as_posix(),
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "bytes": len(payload),
                }
            )

        for participants in MODULE.REQUIRED_PARTICIPANTS[1:]:
            path = configuration_paths[participants]
            payload = configuration_payloads[participants]
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
            artifacts.append(
                {
                    "kind": "configuration",
                    "path": path.as_posix(),
                    "sha256": configuration_digests[participants],
                    "bytes": len(payload),
                }
            )

        differential_pairs = []
        differential_left_root = Path("evidence") / "differential" / "left"
        differential_right_root = Path("evidence") / "differential" / "right"
        left_directory = root / differential_left_root
        right_directory = root / differential_right_root
        left_directory.mkdir(parents=True, exist_ok=True)
        right_directory.mkdir(parents=True, exist_ok=True)
        leakage_payloads: dict[str, dict[str, Any]] = {}
        leakage_jobs: dict[str, dict[str, Any]] = {}
        for index, (variant, directory) in enumerate(
            (("left", left_directory), ("right", right_directory)), 1
        ):
            job = {
                "request_id": f"{index}" * 64,
                "invocation_nonce": "8" * 64,
                "kind": "leakage",
                "participants": 3,
                "variant": variant,
                "canary_names": [],
                "canary_commitments": {},
                "configuration_sha256": "c" * 64,
            }
            leakage_jobs[variant] = job
            leakage_payloads[variant] = runner_leakage_payload(job, directory)
        for surface in MODULE.REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS:
            relative_name = Path(MODULE.DIFFERENTIAL_SURFACE_FILES[surface])
            left_path = differential_left_root / relative_name
            right_path = differential_right_root / relative_name
            left_payload = (root / left_path).read_bytes()
            right_payload = (root / right_path).read_bytes()
            self.assertEqual(len(left_payload), len(right_payload))
            left = {
                "kind": surface,
                "path": left_path.as_posix(),
                "sha256": hashlib.sha256(left_payload).hexdigest(),
                "bytes": len(left_payload),
            }
            artifacts.append(left)
            right = {
                "kind": surface,
                "path": right_path.as_posix(),
                "sha256": hashlib.sha256(right_payload).hexdigest(),
                "bytes": len(right_payload),
            }
            artifacts.append(right)
            differential_pairs.append(
                {
                    "surface": surface,
                    "relative_name": relative_name.as_posix(),
                    "left": {
                        "path": left["path"],
                        "sha256": left["sha256"],
                        "bytes": left["bytes"],
                    },
                    "right": {
                        "path": right["path"],
                        "sha256": right["sha256"],
                        "bytes": right["bytes"],
                    },
                }
            )
        for variant in ("left", "right"):
            provenance_payload = (
                json.dumps(
                    runner_response(
                        leakage_jobs[variant], leakage_payloads[variant]
                    ),
                    sort_keys=True,
                )
                + "\n"
            ).encode()
            provenance_path = (
                Path("evidence")
                / "leakage"
                / f"capture-provenance-{variant}.json"
            )
            (root / provenance_path).parent.mkdir(parents=True, exist_ok=True)
            (root / provenance_path).write_bytes(provenance_payload)
            provenance_artifact = {
                "kind": "leakage_capture_provenance",
                "path": provenance_path.as_posix(),
                "sha256": hashlib.sha256(provenance_payload).hexdigest(),
                "bytes": len(provenance_payload),
            }
            if variant == "left":
                existing = next(
                    artifact
                    for artifact in artifacts
                    if artifact["kind"] == "leakage_capture_provenance"
                )
                existing.update(provenance_artifact)
            else:
                artifacts.append(provenance_artifact)
        differential_manifest_payload = (
            json.dumps(
                {
                    "version": 1,
                    "protocol": MODULE.PROTOCOL,
                    "commit": RELEASE_COMMIT,
                    "left_root": differential_left_root.as_posix(),
                    "right_root": differential_right_root.as_posix(),
                    "pairs": differential_pairs,
                    "passed": True,
                },
                sort_keys=True,
            )
            + "\n"
        ).encode()
        differential_manifest = next(
            artifact
            for artifact in artifacts
            if artifact["kind"] == "differential_pair_manifest"
        )
        differential_manifest_path = root / differential_manifest["path"]
        differential_manifest_path.write_bytes(differential_manifest_payload)
        differential_manifest["sha256"] = hashlib.sha256(
            differential_manifest_payload
        ).hexdigest()
        differential_manifest["bytes"] = len(differential_manifest_payload)

        left_count = next(
            artifact
            for artifact in artifacts
            if artifact["kind"] == "traffic_count_manifest"
        )
        right_count_path = Path("evidence") / "traffic-counts-right.json"
        right_count_payload = (root / left_count["path"]).read_bytes()
        (root / right_count_path).write_bytes(right_count_payload)
        artifacts.append(
            {
                "kind": "traffic_count_manifest",
                "path": right_count_path.as_posix(),
                "sha256": hashlib.sha256(right_count_payload).hexdigest(),
                "bytes": len(right_count_payload),
            }
        )

        canary = next(
            artifact for artifact in artifacts if artifact["kind"] == "canary_manifest"
        )
        scanned = sorted(
            (
                {"sha256": artifact["sha256"], "bytes": artifact["bytes"]}
                for artifact in artifacts
                if artifact["kind"] in MODULE.REQUIRED_LEAKAGE_ARTIFACT_KINDS
            ),
            key=lambda item: (item["sha256"], item["bytes"]),
        )
        count_bindings = sorted(
            (
                {"sha256": artifact["sha256"], "bytes": artifact["bytes"]}
                for artifact in artifacts
                if artifact["kind"] == "traffic_count_manifest"
            ),
            key=lambda item: (item["sha256"], item["bytes"]),
        )
        leakage_payload = (
            json.dumps(
                {
                    "version": 1,
                    "passed": True,
                    "canary_manifest": {
                        "sha256": canary["sha256"],
                        "bytes": canary["bytes"],
                    },
                    "scanned_artifacts": scanned,
                    "scanned_files": len(scanned),
                    "scanned_bytes": sum(item["bytes"] for item in scanned),
                    "canary_names": list(MODULE.REQUIRED_LEAKAGE_CANARY_NAMES),
                    "findings": [],
                    "differential": {
                        "left_only": [],
                        "right_only": [],
                        "size_mismatches": [],
                        "json_shape_mismatches": [],
                        "packet_length_mismatches": [],
                    },
                    "traffic_count_manifests": count_bindings,
                    "traffic_count_mismatches": [],
                },
                sort_keys=True,
            )
            + "\n"
        ).encode()
        leakage_path = root / "evidence" / "leakage_report.json"
        leakage_path.write_bytes(leakage_payload)
        leakage_artifact = next(
            artifact for artifact in artifacts if artifact["kind"] == "leakage_report"
        )
        leakage_artifact["sha256"] = hashlib.sha256(leakage_payload).hexdigest()
        leakage_artifact["bytes"] = len(leakage_payload)
        artifacts.sort(key=lambda artifact: artifact["path"])
        manifest = {
            "version": 1,
            "protocol": MODULE.PROTOCOL,
            "commit": RELEASE_COMMIT,
            "worktree_clean": True,
            "doi": "10.5281/zenodo.1234567",
            "qualification": {
                "real_network_participants": list(MODULE.REQUIRED_PARTICIPANTS),
                "validators_per_dataspace": 4,
                "quorum": "3-of-4",
                "mandatory_signed_rs16_da_rbc": True,
                "max_unavailable_per_committee": 1,
                "loss_percentages": list(MODULE.REQUIRED_LOSS_PERCENTAGES),
                "crash_boundaries": list(MODULE.REQUIRED_CRASH_BOUNDARIES),
                "randomized_seeds": 10,
                "soak_seconds": 7200,
                "minimum_warmups": 5,
                "minimum_measured_bundles": 30,
            },
            "independent_audit": {
                "independent": True,
                "organization": "Independent Cryptography Laboratory",
                "conclusion": "passed",
                "scopes": list(MODULE.REQUIRED_AUDIT_SCOPES),
                "report_path": "evidence/audit_report.txt",
            },
            "artifacts": artifacts,
        }
        manifest_path = root / "release-manifest-v1.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        return manifest_path

    def test_complete_exact_bundle_passes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            report = MODULE.verify_bundle(self.make_bundle(Path(temporary)))
            self.assertTrue(report["passed"])
            self.assertEqual(
                report["artifact_count"],
                len(MODULE.REQUIRED_ARTIFACT_KINDS)
                + 1
                + len(MODULE.PASS_REPORT_GATES)
                + 7
                + 4
                + 2 * len(MODULE.REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS)
                + 3,
            )
            self.assertRegex(report["bundle_binding_sha256"], r"^[0-9a-f]{64}$")

    def test_configuration_manifest_requires_exact_four_validator_da_matrix(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "configuration_manifest"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["configurations"][2]["mandatory_signed_rs16_da_rbc"] = False
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "invalid network profile"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_fault_trials_must_bind_archived_transcripts_and_captures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            raw_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_raw"
            )
            raw_path = root / raw_artifact["path"]
            rows = [json.loads(line) for line in raw_path.read_text().splitlines()]
            rows[0]["loss_trials"][0]["observation_capture_sha256"] = "f" * 64
            raw_payload = (
                "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n"
            ).encode()
            raw_path.write_bytes(raw_payload)
            raw_artifact["bytes"] = len(raw_payload)
            raw_artifact["sha256"] = hashlib.sha256(raw_payload).hexdigest()

            report_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_report"
            )
            report = MODULE._regenerate_fault_report([raw_path])
            report_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            (root / report_artifact["path"]).write_bytes(report_payload)
            report_artifact["bytes"] = len(report_payload)
            report_artifact["sha256"] = hashlib.sha256(report_payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            with self.assertRaisesRegex(
                MODULE.EvidenceError, "does not resolve to one archived capture"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_fault_trial_record_must_exist_in_its_bound_capture(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            raw_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_raw"
            )
            raw_path = root / raw_artifact["path"]
            rows = [json.loads(line) for line in raw_path.read_text().splitlines()]
            rows[0]["loss_trials"][0]["observation_capture_record"] = (
                "n2:s0:r0:loss:missing"
            )
            raw_payload = (
                "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n"
            ).encode()
            raw_path.write_bytes(raw_payload)
            raw_artifact["bytes"] = len(raw_payload)
            raw_artifact["sha256"] = hashlib.sha256(raw_payload).hexdigest()

            report_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_report"
            )
            report_payload = (
                json.dumps(MODULE._regenerate_fault_report([raw_path]), sort_keys=True)
                + "\n"
            ).encode()
            (root / report_artifact["path"]).write_bytes(report_payload)
            report_artifact["bytes"] = len(report_payload)
            report_artifact["sha256"] = hashlib.sha256(report_payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            with self.assertRaisesRegex(
                MODULE.EvidenceError, "is absent from its archived capture"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_fault_transcript_semantics_must_match_the_raw_trial(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            raw_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_raw"
            )
            raw_row = json.loads(
                (root / raw_artifact["path"])
                .read_text(encoding="utf-8")
                .splitlines()[0]
            )
            transcript_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "operator_log"
                and item["path"].endswith("fault-control.jsonl")
            )
            transcript_path = root / transcript_artifact["path"]
            transcript_rows = [
                json.loads(line)
                for line in transcript_path.read_text(encoding="utf-8").splitlines()
            ]
            transcript_rows[0]["control_acknowledged"] = False
            transcript_payload = (
                "\n".join(json.dumps(row, sort_keys=True) for row in transcript_rows)
                + "\n"
            ).encode()
            transcript_path.write_bytes(transcript_payload)
            transcript_digest = hashlib.sha256(transcript_payload).hexdigest()
            for collection in (
                "loss_trials",
                "phase_cut_partitions",
                "crash_recoveries",
            ):
                for trial in raw_row[collection]:
                    trial["control_transcript_sha256"] = transcript_digest
            raw_path = root / "single-fault-row.jsonl"
            raw_path.write_text(json.dumps(raw_row) + "\n", encoding="utf-8")
            capture_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "sanitized_capture"
                and item["path"].endswith("fault-atomicity.jsonl")
            )
            artifacts = [
                MODULE.Artifact(
                    kind="operator_log",
                    path=MODULE.PurePosixPath(transcript_artifact["path"]),
                    sha256=transcript_digest,
                    bytes=len(transcript_payload),
                ),
                MODULE.Artifact(
                    kind="sanitized_capture",
                    path=MODULE.PurePosixPath(capture_artifact["path"]),
                    sha256=capture_artifact["sha256"],
                    bytes=capture_artifact["bytes"],
                ),
            ]

            with self.assertRaisesRegex(
                MODULE.EvidenceError, "nested fault evidence is invalid"
            ):
                MODULE._validate_fault_trial_evidence_bindings(
                    [raw_path], artifacts, root
                )

    def test_raw_benchmarks_must_bind_archived_hardware_description(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "benchmark_raw"
            )
            raw_path = root / artifact["path"]
            rows = [json.loads(line) for line in raw_path.read_text().splitlines()]
            rows[0]["hardware_sha256"] = "f" * 64
            payload = (
                "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n"
            ).encode()
            raw_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "different hardware or configuration"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_benchmark_statistics_are_recomputed_from_archived_samples(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "benchmark_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["profiles"]["private"]["3"]["stages_ms"]["end_to_end"]["p50"] += (
                0.001
            )
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError,
                "statistics do not match archived raw samples",
            ):
                MODULE.verify_bundle(manifest_path)

    def test_missing_release_kind_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = self.make_bundle(Path(temporary))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["artifacts"] = [
                artifact
                for artifact in manifest["artifacts"]
                if artifact["kind"] != "sbom"
            ]
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(MODULE.EvidenceError):
                MODULE.verify_bundle(manifest_path)

    def test_digest_mismatch_and_unlisted_files_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            test_report = next(
                artifact
                for artifact in manifest["artifacts"]
                if artifact["kind"] == "test_report"
            )
            (root / test_report["path"]).write_text("tampered\n", encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "byte count mismatch"):
                MODULE.verify_bundle(manifest_path)
            second_root = root / "second"
            second_root.mkdir()
            manifest_path = self.make_bundle(second_root)
            (second_root / "unlisted.txt").write_text(
                "not bound\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(MODULE.EvidenceError, "inventory mismatch"):
                MODULE.verify_bundle(manifest_path)

    def test_placeholders_cannot_satisfy_external_gates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = self.make_bundle(Path(temporary))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["worktree_clean"] = False
            manifest["doi"] = "pending"
            manifest["independent_audit"]["conclusion"] = "pending"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(MODULE.EvidenceError):
                MODULE.verify_bundle(manifest_path)

    def test_one_line_command_gate_placeholder_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "clippy_report"
            )
            payload = b"clippy passed\n"
            (root / artifact["path"]).write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "cannot read clippy_report"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_command_gate_report_must_bind_release_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "format_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["commit"] = "b" * 40
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "commit differs"):
                MODULE.verify_bundle(manifest_path)

    def test_release_inventory_mismatch_cannot_be_declared_passing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            archive_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "source_archive"
            )
            archive_payload = fixture_source_seal(b"# substituted Cargo.lock\n")
            (root / archive_artifact["path"]).write_bytes(archive_payload)
            archive_artifact["bytes"] = len(archive_payload)
            archive_artifact["sha256"] = hashlib.sha256(archive_payload).hexdigest()

            source_manifest_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "source_manifest"
            )
            source_manifest_path = root / source_manifest_artifact["path"]
            source_manifest = json.loads(
                source_manifest_path.read_text(encoding="utf-8")
            )
            source_manifest["source_archive"] = {
                "path": archive_artifact["path"],
                "sha256": archive_artifact["sha256"],
                "bytes": archive_artifact["bytes"],
            }
            source_manifest_payload = (
                json.dumps(source_manifest, sort_keys=True) + "\n"
            ).encode()
            source_manifest_path.write_bytes(source_manifest_payload)
            source_manifest_artifact["bytes"] = len(source_manifest_payload)
            source_manifest_artifact["sha256"] = hashlib.sha256(
                source_manifest_payload
            ).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, r"incorrect=\['Cargo\.lock'\]"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_sdk_report_rejects_skipped_swift_qualification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "sdk_test_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["details"]["sdks"]["swift"]["skipped"] = 1
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "swift.*not qualified"):
                MODULE.verify_bundle(manifest_path)

    def test_randomized_report_requires_all_declared_seeds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "randomized_seed_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["seeds"] = list(range(9))
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "unique seed count"):
                MODULE.verify_bundle(manifest_path)

    def test_soak_report_requires_full_atomic_two_hour_run(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item for item in manifest["artifacts"] if item["kind"] == "soak_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["duration_seconds"] = 7199
            report["partial_visibility_observations"] = 1
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "atomic two-hour run"):
                MODULE.verify_bundle(manifest_path)

    def test_formal_report_requires_safety_negative_controls_to_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "formal_model_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["configurations"][-1]["observed_outcome"] = "pass"
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "differs from expectation"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_auditor_custody_report_requires_separate_rotatable_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "auditor_key_custody_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["signing_encryption_keys_separate"] = False
            report["retired_key_retention_tested"] = False
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "separation, rotation, and retention"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_reproducible_builds_must_match_archived_candidate_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "reproducible_build_report"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["builds"][1]["artifacts"][0]["sha256"] = "d" * 64
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "byte-identical"):
                MODULE.verify_bundle(manifest_path)

    def test_sbom_must_hash_the_archived_release_binary(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item for item in manifest["artifacts"] if item["kind"] == "sbom"
            )
            sbom_path = root / artifact["path"]
            sbom = json.loads(sbom_path.read_text(encoding="utf-8"))
            sbom["components"][0]["hashes"] = []
            payload = (json.dumps(sbom, sort_keys=True) + "\n").encode()
            sbom_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "every archived release binary"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_source_manifest_must_bind_a_clean_exact_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "source_manifest"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["worktree_clean"] = False
            report["untracked"] = ["uncommitted-sdk-file.py"]
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "clean exact Git tree"):
                MODULE.verify_bundle(manifest_path)

    def test_audit_attestation_must_match_report_and_have_no_severe_findings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "audit_attestation"
            )
            report_path = root / artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["open_high_findings"] = 1
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "independent passing audit declaration"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_incomplete_fault_summary_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            report_path = root / "evidence" / "real_network_fault_report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            del report["matrix"]["16"]
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_report"
            )
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "matrix is incomplete"):
                MODULE.verify_bundle(manifest_path)

    def test_fault_summary_is_regenerated_from_exact_archived_raw_runs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            raw_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_raw"
            )
            raw_path = root / raw_artifact["path"]
            rows = [json.loads(line) for line in raw_path.read_text().splitlines()]
            rows[0]["atomicity"]["partial_visible_observations"] = 1
            raw_payload = (
                "\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n"
            ).encode()
            raw_path.write_bytes(raw_payload)
            raw_artifact["bytes"] = len(raw_payload)
            raw_artifact["sha256"] = hashlib.sha256(raw_payload).hexdigest()

            report_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "real_network_fault_report"
            )
            report_path = root / report_artifact["path"]
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["raw_inputs"] = [
                {
                    "sha256": raw_artifact["sha256"],
                    "bytes": raw_artifact["bytes"],
                }
            ]
            report_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(report_payload)
            report_artifact["bytes"] = len(report_payload)
            report_artifact["sha256"] = hashlib.sha256(report_payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "raw evidence is invalid"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_leakage_report_requires_secret_only_shape_and_count_equivalence(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            report_path = root / "evidence" / "leakage_report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["traffic_count_mismatches"] = [
                {"channel": "torii_request_packets", "left": 1, "right": 2}
            ]
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(MODULE.EvidenceError, "traffic-count finding"):
                MODULE.verify_bundle(manifest_path)

    def test_leakage_report_must_bind_every_archived_privacy_surface(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            omitted = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "torii_capture"
            )
            report_path = root / "evidence" / "leakage_report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            binding = {"sha256": omitted["sha256"], "bytes": omitted["bytes"]}
            report["scanned_artifacts"].remove(binding)
            report["scanned_files"] = len(report["scanned_artifacts"])
            report["scanned_bytes"] = sum(
                item["bytes"] for item in report["scanned_artifacts"]
            )
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "every archived privacy surface"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_release_verifier_independently_rescans_archived_canaries(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "operator_log"
                and item["path"] == "evidence/logs/formal_model_report.log"
            )
            artifact_path = root / artifact["path"]
            old_binding = {"sha256": artifact["sha256"], "bytes": artifact["bytes"]}
            canary_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "canary_manifest"
            )
            canary_manifest = json.loads(
                (root / canary_artifact["path"]).read_text(encoding="utf-8")
            )
            account_canary = next(
                item["value"]
                for item in canary_manifest["canaries"]
                if item["name"] == "account_id"
            )
            payload = f"{account_canary}\n".encode()
            artifact_path.write_bytes(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            artifact["bytes"] = len(payload)
            new_binding = {"sha256": artifact["sha256"], "bytes": artifact["bytes"]}

            formal_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "formal_model_report"
            )
            formal_path = root / formal_artifact["path"]
            formal_report = json.loads(formal_path.read_text(encoding="utf-8"))
            formal_report["transcript"] = {
                "path": artifact["path"],
                **new_binding,
            }
            formal_payload = (json.dumps(formal_report, sort_keys=True) + "\n").encode()
            formal_path.write_bytes(formal_payload)
            formal_artifact["sha256"] = hashlib.sha256(formal_payload).hexdigest()
            formal_artifact["bytes"] = len(formal_payload)

            leakage_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            leakage_path = root / leakage_artifact["path"]
            report = json.loads(leakage_path.read_text(encoding="utf-8"))
            report["scanned_artifacts"].remove(old_binding)
            report["scanned_artifacts"].append(new_binding)
            report["scanned_artifacts"].sort(
                key=lambda item: (item["sha256"], item["bytes"])
            )
            report["scanned_bytes"] = sum(
                item["bytes"] for item in report["scanned_artifacts"]
            )
            leakage_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            leakage_path.write_bytes(leakage_payload)
            leakage_artifact["sha256"] = hashlib.sha256(leakage_payload).hexdigest()
            leakage_artifact["bytes"] = len(leakage_payload)
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "contains a planted secret canary"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_release_verifier_independently_compares_differential_shapes(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            right = next(
                item
                for item in manifest["artifacts"]
                if item["path"] == "evidence/differential/right/events.json"
            )
            old_binding = {"sha256": right["sha256"], "bytes": right["bytes"]}
            right_path = root / right["path"]
            right_payload = right_path.read_bytes().replace(
                b'"source_bytes"', b'"source_bytez"', 1
            )
            self.assertEqual(len(right_payload), right["bytes"])
            right_path.write_bytes(right_payload)
            right["sha256"] = hashlib.sha256(right_payload).hexdigest()
            right["bytes"] = len(right_payload)
            new_binding = {"sha256": right["sha256"], "bytes": right["bytes"]}

            pair_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "differential_pair_manifest"
            )
            pair_path = root / pair_artifact["path"]
            pair_manifest = json.loads(pair_path.read_text(encoding="utf-8"))
            pair = next(
                item
                for item in pair_manifest["pairs"]
                if item["surface"] == "event_capture"
            )
            pair["right"] = {"path": right["path"], **new_binding}
            pair_payload = (json.dumps(pair_manifest, sort_keys=True) + "\n").encode()
            pair_path.write_bytes(pair_payload)
            pair_artifact["sha256"] = hashlib.sha256(pair_payload).hexdigest()
            pair_artifact["bytes"] = len(pair_payload)

            leakage_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            leakage_path = root / leakage_artifact["path"]
            report = json.loads(leakage_path.read_text(encoding="utf-8"))
            report["scanned_artifacts"].remove(old_binding)
            report["scanned_artifacts"].append(new_binding)
            report["scanned_artifacts"].sort(
                key=lambda item: (item["sha256"], item["bytes"])
            )
            leakage_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            leakage_path.write_bytes(leakage_payload)
            leakage_artifact["sha256"] = hashlib.sha256(leakage_payload).hexdigest()
            leakage_artifact["bytes"] = len(leakage_payload)

            manifest["artifacts"].sort(key=lambda item: item["path"])
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "JSON public shapes differ"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_release_verifier_replays_archived_capture_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            provenance = next(
                item
                for item in manifest["artifacts"]
                if item["path"]
                == "evidence/leakage/capture-provenance-right.json"
            )
            old_binding = {
                "sha256": provenance["sha256"],
                "bytes": provenance["bytes"],
            }
            provenance_path = root / provenance["path"]
            response = json.loads(provenance_path.read_text(encoding="utf-8"))
            response["payload"]["capture_provenance"]["packet_counts"][
                "sanitized_packets"
            ] = 5
            provenance_payload = (
                json.dumps(response, sort_keys=True) + "\n"
            ).encode()
            provenance_path.write_bytes(provenance_payload)
            provenance["sha256"] = hashlib.sha256(provenance_payload).hexdigest()
            provenance["bytes"] = len(provenance_payload)
            new_binding = {
                "sha256": provenance["sha256"],
                "bytes": provenance["bytes"],
            }

            leakage_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            leakage_path = root / leakage_artifact["path"]
            report = json.loads(leakage_path.read_text(encoding="utf-8"))
            report["scanned_artifacts"].remove(old_binding)
            report["scanned_artifacts"].append(new_binding)
            report["scanned_artifacts"].sort(
                key=lambda item: (item["sha256"], item["bytes"])
            )
            report["scanned_bytes"] = sum(
                item["bytes"] for item in report["scanned_artifacts"]
            )
            leakage_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            leakage_path.write_bytes(leakage_payload)
            leakage_artifact["sha256"] = hashlib.sha256(
                leakage_payload
            ).hexdigest()
            leakage_artifact["bytes"] = len(leakage_payload)

            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "provenance replay failed"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_differential_roots_reject_unpaired_archived_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            extra_path = Path("evidence/differential/left/unpaired.txt")
            extra_payload = b"same-size differential record with no right peer\n"
            (root / extra_path).write_bytes(extra_payload)
            extra = {
                "kind": "operator_log",
                "path": extra_path.as_posix(),
                "sha256": hashlib.sha256(extra_payload).hexdigest(),
                "bytes": len(extra_payload),
            }
            manifest["artifacts"].append(extra)

            leakage_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            leakage_path = root / leakage_artifact["path"]
            report = json.loads(leakage_path.read_text(encoding="utf-8"))
            report["scanned_artifacts"].append(
                {"sha256": extra["sha256"], "bytes": extra["bytes"]}
            )
            report["scanned_artifacts"].sort(
                key=lambda item: (item["sha256"], item["bytes"])
            )
            report["scanned_files"] = len(report["scanned_artifacts"])
            report["scanned_bytes"] = sum(
                item["bytes"] for item in report["scanned_artifacts"]
            )
            leakage_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            leakage_path.write_bytes(leakage_payload)
            leakage_artifact["sha256"] = hashlib.sha256(leakage_payload).hexdigest()
            leakage_artifact["bytes"] = len(leakage_payload)

            manifest["artifacts"].sort(key=lambda item: item["path"])
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "unpaired or undeclared archive artifact"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_archived_differential_traffic_counts_must_match(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            right = next(
                item
                for item in manifest["artifacts"]
                if item["path"] == "evidence/traffic-counts-right.json"
            )
            old_binding = {"sha256": right["sha256"], "bytes": right["bytes"]}
            right_path = root / right["path"]
            counts = json.loads(right_path.read_text(encoding="utf-8"))
            counts["channels"][MODULE.REQUIRED_TRAFFIC_COUNT_CHANNELS[0]] += 1
            right_payload = (json.dumps(counts, sort_keys=True) + "\n").encode()
            right_path.write_bytes(right_payload)
            right["sha256"] = hashlib.sha256(right_payload).hexdigest()
            right["bytes"] = len(right_payload)
            new_binding = {"sha256": right["sha256"], "bytes": right["bytes"]}

            report_path = root / "evidence" / "leakage_report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["scanned_artifacts"].remove(old_binding)
            report["scanned_artifacts"].append(new_binding)
            report["scanned_artifacts"].sort(
                key=lambda item: (item["sha256"], item["bytes"])
            )
            report["traffic_count_manifests"].remove(old_binding)
            report["traffic_count_manifests"].append(new_binding)
            report["traffic_count_manifests"].sort(
                key=lambda item: (item["sha256"], item["bytes"])
            )
            report["scanned_bytes"] = sum(
                item["bytes"] for item in report["scanned_artifacts"]
            )
            report_payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(report_payload)
            report_artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "leakage_report"
            )
            report_artifact["sha256"] = hashlib.sha256(report_payload).hexdigest()
            report_artifact["bytes"] = len(report_payload)
            manifest["artifacts"].sort(key=lambda item: item["path"])
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "traffic counts do not match"
            ):
                MODULE.verify_bundle(manifest_path)

    def test_benchmark_report_must_match_retained_raw_samples(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = self.make_bundle(root)
            report_path = root / "evidence" / "benchmark_report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["profiles"]["private"]["3"]["measured_runs"] = 29
            payload = (json.dumps(report, sort_keys=True) + "\n").encode()
            report_path.write_bytes(payload)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            artifact = next(
                item
                for item in manifest["artifacts"]
                if item["kind"] == "benchmark_report"
            )
            artifact["bytes"] = len(payload)
            artifact["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(
                MODULE.EvidenceError, "does not match raw evidence"
            ):
                MODULE.verify_bundle(manifest_path)


if __name__ == "__main__":
    unittest.main()
