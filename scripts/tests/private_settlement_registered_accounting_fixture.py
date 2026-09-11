"""Synthetic registered release records for the canonical verifier tests.

This helper requires the original sibling test/runner modules and writes only
fixture JSON. It starts no process, copies no implementation, and makes no real
measurement or release qualification claim. Every protocol guard remains active.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Mapping

from scripts.tests.private_settlement_release_runner_test import (
    MODULE as RUNNER,
    response as fixture_response,
)

SEEDS = tuple(range(10))
WARMUPS = 5
MEASURED = 30
# Two plans and their 8 inputs; 102 nonbenchmark starts; 351 accepted samples;
# one typed failed benchmark with its 7 retained records.
CONTROLLED_RECORD_COUNT = 2 * 11 + 102 * 3 + 351 * 9 + 7


def raw(document: Any) -> bytes:
    """Serialize deterministic fixture data, preserving explicit numeric types."""
    return (json.dumps(document, ensure_ascii=False, sort_keys=True, allow_nan=False) + "\n").encode()


def canonical_configuration_inputs(commit: str):
    """Use the actual producer's complete configurations and ten-seed policy."""
    paths = {participants: Path("evidence/configurations") / f"private-settlement-n{participants}.json"
             for participants in RUNNER.PARTICIPANTS}
    payloads = {participants: raw(RUNNER.build_configuration(participants, seeds=SEEDS,
                                                            warmups=WARMUPS, measured=MEASURED))
                for participants in RUNNER.PARTICIPANTS}
    manifest_path = Path("evidence/configuration_manifest.json")
    manifest = {"version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL, "commit": commit, "passed": True,
        "configurations": [{"participants": participants, "validators_per_dataspace": RUNNER.VALIDATORS_PER_DATASPACE,
            "quorum": RUNNER.QUORUM, "mandatory_signed_rs16_da_rbc": True, "path": paths[participants].as_posix(),
            **RUNNER.attempt_accounting.accounting_file_binding(payloads[participants])}
            for participants in RUNNER.PARTICIPANTS]}
    return paths, payloads, manifest_path, raw(manifest)


def build_registered_accounting_fixture(
    root: Path, *, commit: str, hardware_path: Path, hardware_payload: bytes,
    configuration_manifest_path: Path, configuration_manifest_payload: bytes,
    configuration_payloads: Mapping[Path, bytes], validator_sha256: str,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], tuple[bytes, list[dict[str, Any]], list[bytes]]]:
    """Retain one failed predecessor and one complete canonical synthetic plan."""

    # macOS temporary parents may be aliases such as /var/folders. Resolve the
    # test's existing directory before invoking the strict canonical collector.
    root = root.resolve(strict=True)
    artifacts: list[dict[str, Any]] = []
    def record(path: Path, value: Any, kind: str = "benchmark_accounting_record") -> bytes:
        payload = value if type(value) is bytes else raw(value)
        destination = root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.parent.chmod(0o700)
        with destination.open("xb") as stream:
            stream.write(payload)
        destination.chmod(0o600)
        artifacts.append({"kind": kind, "path": path.as_posix(), **RUNNER.attempt_accounting.accounting_file_binding(payload)})
        return payload

    hardware = json.loads(hardware_payload)
    configuration_manifest = json.loads(configuration_manifest_payload)
    configurations = {item["participants"]: item["sha256"] for item in configuration_manifest["configurations"]}
    canaries = RUNNER.build_canary_manifest(commit)
    canary_payload = raw(canaries)
    canary_path = Path("evidence") / "registered-canaries.json"
    def reference(path: Path, payload: bytes) -> dict[str, Any]:
        return {"path": path.as_posix(), **RUNNER.attempt_accounting.accounting_file_binding(payload)}
    plan = {
        "version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL, "commit": commit,
        "worktree_clean": True, "publication_evidence": False, "execution_required": True,
        "harness": {"sha256": "f" * 64, "bytes": 200},
        "harness_contract": dict(RUNNER.HARNESS_CONTRACT), "benchmark_baseline": None,
        "benchmark_accounting": RUNNER.benchmark_deadline_policy(RUNNER.DEFAULT_HARNESS_TIMEOUT_SECONDS),
        "hardware": {**reference(hardware_path, hardware_payload),
                     "profile_sha256": RUNNER.release_evidence._hardware_profile_sha256(hardware)},
        "canary_manifest": reference(canary_path, canary_payload),
        "configuration_manifest": reference(configuration_manifest_path, configuration_manifest_payload),
        "requirements": {
            "participants": list(RUNNER.PARTICIPANTS), "primary_participants": RUNNER.PRIMARY_PARTICIPANTS,
            "validators_per_dataspace": RUNNER.VALIDATORS_PER_DATASPACE, "quorum": RUNNER.QUORUM,
            "seeds": list(SEEDS), "warmups": WARMUPS, "measured": MEASURED, "bootstrap_iterations": 100,
            "loss_phases": list(RUNNER.fault_report.REQUIRED_LOSS_PHASES),
            "loss_percentages": list(RUNNER.fault_report.REQUIRED_LOSS_PERCENTAGES),
            "phase_cuts": list(RUNNER.fault_report.REQUIRED_PHASE_CUTS),
            "crash_boundaries": list(RUNNER.fault_report.REQUIRED_CRASH_BOUNDARIES),
            "capture_surfaces": sorted(RUNNER.SURFACE_FILES),
            "traffic_count_channels": list(RUNNER.leakage_audit.REQUIRED_COUNT_CHANNELS),
        },
        "jobs": RUNNER.build_jobs(configurations, SEEDS, WARMUPS, MEASURED, canaries),
    }
    plan_raw = raw(plan)
    plan_binding = RUNNER.attempt_accounting.accounting_file_binding(plan_raw)
    campaign_ids = ("campaign-0-failed", "campaign-1-complete")
    registered_ns = 1_787_932_800_000_000_000
    scope = {"version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL,
             "scope_id": hashlib.sha256(b"canonical registered accounting fixture").hexdigest(),
             "previous_scope_sha256": None, "registered_ns": registered_ns,
             "stopping_policy": "fail_fast", "deadline_policy": plan["benchmark_accounting"],
             "campaigns": [{"campaign_id": name, "plan": plan_binding} for name in campaign_ids]}
    scope_raw = record(Path("accounting/scope.json"), scope, "benchmark_scope")
    scope_sha = hashlib.sha256(scope_raw).hexdigest()
    rows = []
    for campaign_index, campaign_id in enumerate(campaign_ids):
        campaign_path = Path("accounting/campaigns") / campaign_id
        record(campaign_path / "registered-scope.json", scope_raw)
        record(campaign_path / "frozen-plan.json", plan_raw)
        inputs = {hardware_path: hardware_payload, configuration_manifest_path: configuration_manifest_payload,
                  canary_path: canary_payload, **configuration_payloads}
        for path, payload in inputs.items():
            record(campaign_path / path, payload)
        retained_plan, _ = RUNNER.load_plan(root / campaign_path / "frozen-plan.json")
        if retained_plan != plan:
            raise AssertionError("fixture canonical plan validation changed its data")
        base = {"scope_sha256": scope_sha, "campaign_id": campaign_id, "plan_sha256": plan_binding["sha256"]}
        started_ids = []
        benchmark_index = 0
        campaign_epoch = registered_ns + 1_000_000_000 + campaign_index * 100_000_000_000_000
        for ordinal, job in enumerate(plan["jobs"], 1):
            nonce = hashlib.sha256(f"fixture:{campaign_id}:{ordinal}".encode()).hexdigest()
            execution_job = {**job, "invocation_nonce": nonce}
            attempt_id = RUNNER.attempt_accounting.registered_attempt_id(scope_sha, campaign_id, plan_binding["sha256"], job["request_id"])
            identity = {**base, "attempt_id": attempt_id, "request_id": job["request_id"], "invocation_nonce": nonce}
            attempt = campaign_path / "attempts" / f"{ordinal:05}-{job['request_id']}"
            started_ns = campaign_epoch + ordinal * 200_000_000_000
            request = RUNNER.build_request(plan, root / campaign_path, execution_job)
            request_raw = record(attempt / "request.json", request)
            request_binding = RUNNER.attempt_accounting.accounting_file_binding(request_raw)
            record(attempt / "started.json", {
                "version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL, **identity,
                "command": ["synthetic-fixture-harness"], "request": request_binding, "harness": plan["harness"],
                "timeout_seconds": RUNNER.DEFAULT_HARNESS_TIMEOUT_SECONDS, "started_ns": started_ns,
            })
            started_ids.append(job["request_id"])
            failed = campaign_index == 0 and job["kind"] == "benchmark" and benchmark_index == 1
            record(attempt / "process-outcome.json", {
                "version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL, **identity,
                "finished_ns": started_ns + 100_010_000_000, "pid": ordinal + 10_000,
                "exit_code": 2 if failed else 0, "timed_out": False, "error": None,
                "passed": not failed, "retained_files": [], "completion_kind": "exited",
                "elapsed_ms": 100_010, "owned_process_group_gone": True, "bindings_unchanged": True,
            })
            if job["kind"] != "benchmark":
                continue
            benchmark_index += 1
            terminal_header = {"version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL,
                **{key: request[key] for key in ("request_id", "invocation_nonce", "commit", "participants")},
                "request_sha256": request_binding["sha256"], "elapsed_ms": 100_000}
            if failed:
                outcome = {"kind": "failed", "stage": "benchmark_worker", "reason": "execution_error"}
                response = None
            else:
                payload = {"stages_ms": {stage: float(job["run"] + 1) for stage in (
                        RUNNER.benchmark_report.REQUIRED_PRIVATE_STAGES if job["profile"] == "private"
                        else ("global_finality", "end_to_end"))},
                    **{field: float(job["run"] + 1) for field in RUNNER.benchmark_report.RESOURCE_FIELDS},
                    "finalized_receipt_observed": True, "successful_leg_applications": job["participants"],
                    "each_leg_applied_exactly_once": True, "partial_visible_observations": 0,
                    "partial_spendable_observations": 0}
                response = fixture_response(execution_job, payload)
                response.update(commit=commit, hardware_sha256=plan["hardware"]["sha256"],
                                hardware_profile_sha256=plan["hardware"]["profile_sha256"])
                for process in response["process_inventory"]:
                    process.update(revision=commit, executable_sha256=validator_sha256)
                result = {key: value for key, value in response.items()
                          if key in RUNNER.attempt_accounting.SUCCESS_RESULT_FIELDS}
                result["request_sha256"] = request_binding["sha256"]
                outcome = {"kind": "succeeded", "result": result}
            terminal_raw = record(attempt / "evidence/benchmark-protocol/rust-result.json", {**terminal_header, "outcome": outcome})
            record(attempt / "evidence/benchmark-protocol/adapter-outcome.json", {
                **terminal_header, "elapsed_ms": 100_005,
                "phase": "terminal_validation" if failed else "measurement_validation",
                "exit_code": 101 if failed else 0, "status": "failed" if failed else "succeeded",
                "reason": "rust_failed" if failed else "rust_succeeded",
                "rust_terminal": RUNNER.attempt_accounting.accounting_file_binding(terminal_raw),
            })
            validation = {"version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL, **identity,
                          "ordinal": ordinal, "kind": "benchmark", "finished_ns": started_ns + 100_020_000_000}
            if failed:
                record(attempt / "response-outcome.json", {"version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL,
                    "passed": False, "error": "synthetic fixture worker failure", "retained_files": []})
                record(attempt / "validation-outcome.json", {**validation, "passed": False,
                    "validation_kind": "not_validated", "stage": "invocation", "error": "synthetic fixture worker failure"})
                break
            response_raw = record(attempt / "response.json", response)
            response_binding = RUNNER.attempt_accounting.accounting_file_binding(response_raw)
            record(attempt / "response-outcome.json", {"version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL,
                                                       "passed": True, "response": response_binding})
            sample = RUNNER.materialize_benchmark_response(response, plan=plan, job=execution_job)
            sample["attempt_id"] = attempt_id
            sample_raw = record(attempt / "benchmark-sample.json", sample)
            record(attempt / "validation-outcome.json", {**validation, "passed": True, "validation_kind": "accepted",
                "response": response_binding, "sample": RUNNER.attempt_accounting.accounting_file_binding(sample_raw)})
            rows.append(sample)
        record(campaign_path / "campaign-closure.json", {
            "version": RUNNER.VERSION, "protocol": RUNNER.PROTOCOL, **base,
            "closed_ns": campaign_epoch + (len(plan["jobs"]) + 1) * 200_000_000_000,
            "quiescent": True, "started_request_ids": started_ids,
            "reason": "fail_fast" if campaign_index == 0 else "completed",
        })
    accounting, _, collected = RUNNER.qualify_benchmark_scope(root / "accounting/scope.json")
    if accounting["counts"] != {"planned": 700, "attempted": 352, "succeeded": 351,
            "failed": 1, "timed_out": 0, "not_started": 348, "incomplete": 0}:
        raise AssertionError("synthetic registered fixture lost its failed predecessor")
    if sum(item["kind"] == "benchmark_accounting_record" for item in artifacts) != CONTROLLED_RECORD_COUNT:
        raise AssertionError("synthetic accounting record inventory differs")
    record(Path("reports/benchmark-accounting-v1.json"), accounting, "benchmark_accounting_report")
    return artifacts, rows, collected
