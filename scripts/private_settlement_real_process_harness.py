#!/usr/bin/env python3
"""Run one genuine AtomicPrivateSettlementV1 release experiment.

This executable implements the exact three-path contract consumed by
``private_settlement_release_runner.py``.  Private settlement and its
transparent Native AMX control and authenticated fault campaign are backed by
ignored Rust real-process tests. Leakage capture remains fail-closed until its
real-process implementation exists.

The Python boundary authenticates the request, source revision, configuration,
Rust result freshness, and final response before publishing the response with
an atomic no-overwrite link.  Missing measurements are errors; this harness
never substitutes defaults or synthetic observations.
"""

from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Mapping, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = SCRIPT_DIR.parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import private_settlement_release_runner as runner

REQUEST_FIELDS = {
    "version",
    "protocol",
    "request_id",
    "invocation_nonce",
    "kind",
    "commit",
    "hardware_sha256",
    "hardware_profile_sha256",
    "configuration_sha256",
    "participants",
    "validators_per_dataspace",
    "global_validators",
    "quorum",
    "mandatory_signed_rs16_da_rbc",
    "minimum_signed_rs16_da_observations",
    "authenticated_message_control",
    "seed",
    "run",
    "configuration",
    "payload",
}
BENCHMARK_REQUEST_PAYLOAD_FIELDS = {"profile", "warmup", "stages", "resources"}
FAULT_REQUEST_PAYLOAD_FIELDS = {
    "loss_phases",
    "loss_percentages",
    "phase_cuts",
    "crash_boundaries",
    "committee_validator_restarts",
    "restart_coordinator",
    "restart_global_node",
    "maximum_simultaneously_unavailable_per_committee",
    "continuous_atomicity_checks",
    "prepare_qc_normalization",
}
RUST_RESULT_FIELDS = {
    "version",
    "protocol",
    "request_id",
    "invocation_nonce",
    "request_sha256",
    "commit",
    "participants",
    "mandatory_signed_rs16_da_rbc",
    "signed_rs16_da_observations",
    "authenticated_message_control",
    "process_inventory",
    "payload",
}
BENCHMARK_RESULT_FIELDS = {
    "stages_ms",
    *runner.benchmark_report.RESOURCE_FIELDS,
    *runner.BENCHMARK_CORRECTNESS_FIELDS,
}
MAX_REQUEST_BYTES = 16 * 1024 * 1024
BENCHMARK_TEST_NAME = (
    "nexus::atomic_private_settlement_localnet::"
    "atomic_private_settlement_real_process_benchmark_harness"
)
FAULT_TEST_NAME = (
    "nexus::atomic_private_settlement_localnet::"
    "atomic_private_settlement_real_process_fault_harness"
)
TARGET_DIR = REPOSITORY_ROOT / "target" / "aps-private-settlement-real-process"
VALIDATOR_EXECUTABLE = TARGET_DIR / "release" / "iroha3d"


class HarnessError(ValueError):
    """Raised when the real-process harness cannot prove a requested result."""


def benchmark_stages(profile: str) -> tuple[str, ...]:
    """Return the exact stage inventory for one implemented benchmark profile."""

    if profile == "private":
        return tuple(runner.benchmark_report.REQUIRED_PRIVATE_STAGES)
    if profile == "transparent_control":
        return ("global_finality", "end_to_end")
    raise HarnessError("unsupported real-process benchmark profile")


def _strict_json_loads(raw: str, label: str) -> Any:
    """Decode strict JSON with the runner's duplicate/non-finite checks."""

    try:
        return runner.strict_json_loads(raw, label)
    except runner.RunnerError as error:
        raise HarnessError(str(error)) from error


def _regular_file_bytes(path: Path, label: str, limit: int) -> bytes:
    """Read a stable bounded regular file without following its final symlink."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise HarnessError(f"{label} must be a readable regular non-symlink file") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise HarnessError(f"{label} must be a regular file")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            raw = stream.read(limit + 1)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if len(raw) > limit:
        raise HarnessError(f"{label} exceeds the bounded file size")
    if len(raw) != before.st_size or any(
        getattr(before, field) != getattr(after, field) for field in stable
    ):
        raise HarnessError(f"{label} changed while it was read")
    return raw


def _sha256_file(path: Path, label: str) -> str:
    """Hash a stable regular file."""

    digest = hashlib.sha256()
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise HarnessError(f"{label} is not a readable non-symlink file") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise HarnessError(f"{label} is not a regular file")
        with os.fdopen(descriptor, "rb", closefd=False) as stream:
            while chunk := stream.read(1024 * 1024):
                digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(getattr(before, field) != getattr(after, field) for field in stable):
        raise HarnessError(f"{label} changed while it was hashed")
    return digest.hexdigest()


def parse_exact_arguments(argv: Sequence[str]) -> tuple[Path, Path, Path]:
    """Parse the runner's exact ordered three-argument invocation contract."""

    if len(argv) != 6 or tuple(argv[::2]) != (
        "--aps-request",
        "--aps-response",
        "--aps-evidence-dir",
    ):
        raise HarnessError(
            "expected exactly --aps-request PATH --aps-response PATH "
            "--aps-evidence-dir PATH"
        )
    request, response, evidence = (Path(argv[index]) for index in (1, 3, 5))
    if len({os.path.abspath(path) for path in (request, response, evidence)}) != 3:
        raise HarnessError("request, response, and evidence paths must be distinct")
    return request, response, evidence


def canonicalize_paths(
    request: Path, response: Path, evidence: Path
) -> tuple[Path, Path, Path]:
    """Resolve existing boundaries and pin the absent response to a real parent."""

    if any(path.name in ("", ".", "..") for path in (request, response, evidence)):
        raise HarnessError("harness paths must name exact final components")
    if request.is_symlink() or response.is_symlink() or evidence.is_symlink():
        raise HarnessError("harness final path components must not be symbolic links")
    try:
        canonical_request = request.resolve(strict=True)
        canonical_evidence = evidence.resolve(strict=True)
        canonical_response_parent = response.parent.resolve(strict=True)
    except OSError as error:
        raise HarnessError("harness path boundary cannot be resolved") from error
    canonical_response = canonical_response_parent / response.name
    if len({canonical_request, canonical_response, canonical_evidence}) != 3:
        raise HarnessError("request, response, and evidence paths must be distinct")
    return canonical_request, canonical_response, canonical_evidence


def validate_paths(request: Path, response: Path, evidence: Path) -> None:
    """Reject symlink, reused-response, and non-empty evidence boundaries."""

    if request.is_symlink() or not request.is_file():
        raise HarnessError("request must be a regular non-symlink file")
    if response.is_symlink() or response.exists():
        raise HarnessError("response path must not already exist")
    if evidence.is_symlink() or not evidence.is_dir():
        raise HarnessError("evidence path must be a regular directory")
    for parent, label in ((response.parent, "response"), (evidence.parent, "evidence")):
        if parent.is_symlink() or not parent.is_dir():
            raise HarnessError(f"{label} parent must be a regular directory")
    try:
        entries = list(evidence.iterdir())
    except OSError as error:
        raise HarnessError("cannot enumerate the evidence directory") from error
    if entries:
        raise HarnessError("release evidence directory must start empty")


def _require_sha(value: Any, label: str, *, git: bool = False) -> str:
    pattern = runner.GIT_OBJECT if git else runner.SHA256
    if not isinstance(value, str) or pattern.fullmatch(value) is None:
        raise HarnessError(f"{label} is not a canonical lowercase digest")
    if not git and value == "0" * 64:
        raise HarnessError(f"{label} must be non-zero")
    return value


def _configuration_file_bytes(value: Mapping[str, Any]) -> bytes:
    """Render the exact pretty JSON bytes used by release-runner plan files."""

    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def validate_request(value: Any) -> dict[str, Any]:
    """Validate and canonically bind one supported release request."""

    try:
        request = runner.exact_fields(value, REQUEST_FIELDS, "harness request")
    except runner.RunnerError as error:
        raise HarnessError(str(error)) from error
    if request["version"] != runner.VERSION or request["protocol"] != runner.PROTOCOL:
        raise HarnessError("request protocol header is invalid")
    # TODO: Implement the leakage branch only after every required public and
    # restricted capture surface is collected from the real process topology.
    if request["kind"] == "leakage":
        raise HarnessError(
            "real process harness does not yet support leakage requests"
        )
    if request["kind"] not in {"benchmark", "fault"}:
        raise HarnessError(
            "real process harness currently supports benchmark and fault requests only"
        )
    participants = request["participants"]
    if participants not in runner.PARTICIPANTS:
        raise HarnessError("unsupported release participant count")
    if (
        request["validators_per_dataspace"] != runner.VALIDATORS_PER_DATASPACE
        or request["global_validators"] != runner.GLOBAL_VALIDATORS
        or request["quorum"] != runner.QUORUM
        or request["mandatory_signed_rs16_da_rbc"] is not True
        or request["authenticated_message_control"] is not True
        or request["minimum_signed_rs16_da_observations"]
        != runner.minimum_signed_rs16_da_observations(participants)
    ):
        raise HarnessError("request weakens the mandatory release topology")
    _require_sha(request["request_id"], "request_id")
    _require_sha(request["invocation_nonce"], "invocation_nonce")
    _require_sha(request["commit"], "commit", git=True)
    _require_sha(request["hardware_sha256"], "hardware_sha256")
    _require_sha(request["hardware_profile_sha256"], "hardware_profile_sha256")
    configuration_sha = _require_sha(
        request["configuration_sha256"], "configuration_sha256"
    )
    if (
        isinstance(request["seed"], bool)
        or not isinstance(request["seed"], int)
        or not 0 <= request["seed"] <= runner.MAX_SEED
    ):
        raise HarnessError("seed must be an unsigned 64-bit integer")
    if (
        isinstance(request["run"], bool)
        or not isinstance(request["run"], int)
        or request["run"] < 0
    ):
        raise HarnessError("run must be a non-negative integer")
    try:
        payload = runner.exact_fields(
            request["payload"],
            BENCHMARK_REQUEST_PAYLOAD_FIELDS
            if request["kind"] == "benchmark"
            else FAULT_REQUEST_PAYLOAD_FIELDS,
            "harness request.payload",
        )
    except runner.RunnerError as error:
        raise HarnessError(str(error)) from error
    if request["kind"] == "benchmark":
        profile = payload["profile"]
        if profile not in runner.PROFILES:
            raise HarnessError("unsupported real-process benchmark profile")
        if not isinstance(payload["warmup"], bool):
            raise HarnessError("benchmark warmup must be a boolean")
        if payload["stages"] != list(benchmark_stages(profile)):
            raise HarnessError("benchmark stages differ from the canonical profile")
        if payload["resources"] != list(runner.benchmark_report.RESOURCE_FIELDS):
            raise HarnessError("benchmark resource fields differ from the canonical profile")
    else:
        expected_fault = {
            "loss_phases": list(runner.fault_report.REQUIRED_LOSS_PHASES),
            "loss_percentages": list(runner.fault_report.REQUIRED_LOSS_PERCENTAGES),
            "phase_cuts": list(runner.fault_report.REQUIRED_PHASE_CUTS),
            "crash_boundaries": list(runner.fault_report.REQUIRED_CRASH_BOUNDARIES),
            "committee_validator_restarts": list(range(participants)),
            "restart_coordinator": True,
            "restart_global_node": True,
            "maximum_simultaneously_unavailable_per_committee": 1,
            "continuous_atomicity_checks": True,
            "prepare_qc_normalization": {
                "first_signer_subset": [0, 1, 2],
                "second_signer_subset": [0, 1, 3],
                "accept_equivalent_subsets_only_for_identical_body": True,
                "bind_authority_indices": True,
                "bind_every_signed_body": True,
                "reject_changed_certified_body": True,
            },
        }
        if payload != expected_fault:
            raise HarnessError("fault request differs from the canonical release matrix")
    configuration = request["configuration"]
    if not isinstance(configuration, dict):
        raise HarnessError("request configuration must be an object")
    try:
        seeds = configuration["fault_matrix"]["seeds"]
        benchmark = configuration["benchmark"]
        expected_configuration = runner.build_configuration(
            participants,
            seeds=seeds,
            warmups=benchmark["warmups_per_profile"],
            measured=benchmark["measured_bundles_per_profile"],
        )
    except (KeyError, TypeError, runner.RunnerError) as error:
        raise HarnessError(f"embedded configuration is invalid: {error}") from error
    if configuration != expected_configuration:
        raise HarnessError("embedded configuration is not the canonical release profile")
    actual_configuration_sha = hashlib.sha256(
        _configuration_file_bytes(configuration)
    ).hexdigest()
    if actual_configuration_sha != configuration_sha:
        raise HarnessError("configuration digest does not bind the embedded configuration")
    run_limit = (
        (
            benchmark["warmups_per_profile"]
            if payload["warmup"]
            else benchmark["measured_bundles_per_profile"]
        )
        if request["kind"] == "benchmark"
        else len(seeds)
    )
    if request["run"] >= run_limit:
        raise HarnessError("release run index is outside the configured matrix")
    if request["seed"] != seeds[request["run"] % len(seeds)]:
        raise HarnessError("release seed does not match the canonical run schedule")
    job_body = {
        "kind": request["kind"],
        **({"profile": profile, "warmup": payload["warmup"]} if request["kind"] == "benchmark" else {}),
        "participants": participants,
        "seed": request["seed"],
        "run": request["run"],
        "configuration_sha256": configuration_sha,
    }
    if request["request_id"] != runner.object_digest(job_body):
        raise HarnessError("request_id does not bind the canonical release job")
    return request


def bind_source_revision(request: Mapping[str, Any]) -> None:
    """Require the current checkout to be the exact clean requested revision."""

    try:
        head = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=REPOSITORY_ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise HarnessError("cannot resolve the current source revision") from error
    if head != request["commit"]:
        raise HarnessError("requested commit differs from the current source revision")
    try:
        status = subprocess.run(
            ["git", "status", "--porcelain=v1", "--untracked-files=all"],
            cwd=REPOSITORY_ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise HarnessError("cannot authenticate source cleanliness") from error
    if status:
        raise HarnessError("real-process release harness requires a clean checkout")


def _run(command: Sequence[str], *, environment: Mapping[str, str]) -> None:
    """Run one required build/test command without accepting partial success."""

    try:
        subprocess.run(
            list(command),
            cwd=REPOSITORY_ROOT,
            env=dict(environment),
            check=True,
        )
    except OSError as error:
        raise HarnessError(f"cannot execute {command[0]}") from error
    except subprocess.CalledProcessError as error:
        raise HarnessError(
            f"real-process command failed with exit code {error.returncode}"
        ) from error


def run_rust_harness(
    request_path: Path,
    raw_request: bytes,
    request: Mapping[str, Any],
    evidence_dir: Path,
) -> dict[str, Any]:
    """Build the feature-isolated daemon and run the exact ignored Rust test."""

    request_sha = hashlib.sha256(raw_request).hexdigest()
    with tempfile.TemporaryDirectory(prefix="aps-real-process-") as temporary:
        rust_result = Path(temporary) / "rust-result.json"
        environment = os.environ.copy()
        for name in tuple(environment):
            if name.startswith("IROHA_TEST_") or name.startswith("APS_REAL_PROCESS_"):
                environment.pop(name, None)
        environment.update(
            {
                "IROHA_TEST_SKIP_BUILD": "1",
                "IROHA_TEST_BUILD_PROFILE": "release",
                "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL": str(VALIDATOR_EXECUTABLE),
                "APS_REAL_PROCESS_REQUEST": str(request_path),
                "APS_REAL_PROCESS_RESULT": str(rust_result),
                "APS_REAL_PROCESS_REQUEST_SHA256": request_sha,
                "APS_REAL_PROCESS_EVIDENCE_DIR": str(evidence_dir),
            }
        )
        _run(
            [
                "cargo",
                "build",
                "--locked",
                "--offline",
                "--release",
                "-p",
                "irohad",
                "--bin",
                "iroha3d",
                "--features",
                "test-network-message-control",
                "--target-dir",
                str(TARGET_DIR),
            ],
            environment=environment,
        )
        validator_sha = _sha256_file(VALIDATOR_EXECUTABLE, "validator executable")
        environment["APS_REAL_PROCESS_VALIDATOR_SHA256"] = validator_sha
        _run(
            [
                "cargo",
                "test",
                "--locked",
                "--offline",
                "--release",
                "-p",
                "integration_tests",
                "--test",
                "nexus_and_streaming",
                "--features",
                "atomic-private-settlement-release",
                "--target-dir",
                str(TARGET_DIR),
                BENCHMARK_TEST_NAME
                if request["kind"] == "benchmark"
                else FAULT_TEST_NAME,
                "--",
                "--ignored",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ],
            environment=environment,
        )
        if not rust_result.exists() or rust_result.is_symlink():
            raise HarnessError("Rust benchmark did not publish a result")
        raw_result = _regular_file_bytes(
            rust_result, "Rust benchmark result", runner.MAX_HARNESS_RESPONSE_BYTES
        )
        try:
            decoded = raw_result.decode("utf-8")
        except UnicodeDecodeError as error:
            raise HarnessError("Rust benchmark result is not UTF-8") from error
        result = _strict_json_loads(decoded, "Rust benchmark result")
        if hashlib.sha256(raw_request).hexdigest() != request_sha:
            raise HarnessError("request changed while the Rust benchmark ran")
        return validate_rust_result(
            result,
            request=request,
            request_sha=request_sha,
            evidence_dir=evidence_dir,
        )


def validate_rust_result(
    value: Any,
    *,
    request: Mapping[str, Any],
    request_sha: str,
    evidence_dir: Path | None = None,
) -> dict[str, Any]:
    """Validate an exact, fresh Rust measurement with no response reuse."""

    try:
        result = runner.exact_fields(value, RUST_RESULT_FIELDS, "Rust result")
    except runner.RunnerError as error:
        raise HarnessError(str(error)) from error
    if (
        result["version"] != runner.VERSION
        or result["protocol"] != runner.PROTOCOL
        or result["request_id"] != request["request_id"]
        or result["invocation_nonce"] != request["invocation_nonce"]
        or result["request_sha256"] != request_sha
        or result["commit"] != request["commit"]
        or result["participants"] != request["participants"]
        or result["mandatory_signed_rs16_da_rbc"] is not True
        or result["authenticated_message_control"] is not True
    ):
        raise HarnessError("Rust result does not bind the exact invocation")
    observations = result["signed_rs16_da_observations"]
    if (
        isinstance(observations, bool)
        or not isinstance(observations, int)
        or observations < request["minimum_signed_rs16_da_observations"]
    ):
        raise HarnessError("Rust result lacks per-validator signed RS16 observations")
    try:
        runner.validate_process_inventory(
            result["process_inventory"],
            participants=request["participants"],
            commit=request["commit"],
            label="Rust result.process_inventory",
        )
        payload = (
            runner.exact_fields(
                result["payload"], BENCHMARK_RESULT_FIELDS, "Rust result.payload"
            )
            if request["kind"] == "benchmark"
            else runner.exact_fields(
                result["payload"], runner.FAULT_PAYLOAD_FIELDS, "Rust result.payload"
            )
        )
    except runner.RunnerError as error:
        raise HarnessError(str(error)) from error
    # Reuse the publication validator against a minimal bound plan/job so the
    # harness cannot accidentally diverge from the release runner's semantics.
    envelope = build_response(request, result)
    plan = {
        "commit": request["commit"],
        "hardware": {
            "sha256": request["hardware_sha256"],
            "profile_sha256": request["hardware_profile_sha256"],
        },
    }
    job = {
        "request_id": request["request_id"],
        "invocation_nonce": request["invocation_nonce"],
        "kind": request["kind"],
        **(
            {
                "profile": request["payload"]["profile"],
                "warmup": request["payload"]["warmup"],
            }
            if request["kind"] == "benchmark"
            else {}
        ),
        "participants": request["participants"],
        "seed": request["seed"],
        "run": request["run"],
        "configuration_sha256": request["configuration_sha256"],
    }
    try:
        if request["kind"] == "benchmark":
            runner.materialize_benchmark_response(envelope, plan=plan, job=job)
        else:
            if evidence_dir is None:
                raise HarnessError("fault result validation requires its evidence directory")
            with tempfile.TemporaryDirectory(prefix="aps-fault-validation-") as publication:
                runner.materialize_fault_response(
                    envelope,
                    plan=plan,
                    job=job,
                    evidence_dir=evidence_dir,
                    publication_root=Path(publication),
                )
    except runner.RunnerError as error:
        raise HarnessError(f"Rust release measurement is invalid: {error}") from error
    result["payload"] = payload
    return result


def build_response(
    request: Mapping[str, Any], rust_result: Mapping[str, Any]
) -> dict[str, Any]:
    """Build the exact response envelope accepted by the release runner."""

    return {
        "version": runner.VERSION,
        "protocol": runner.PROTOCOL,
        "request_id": request["request_id"],
        "invocation_nonce": request["invocation_nonce"],
        "kind": request["kind"],
        "commit": request["commit"],
        "hardware_sha256": request["hardware_sha256"],
        "hardware_profile_sha256": request["hardware_profile_sha256"],
        "configuration_sha256": request["configuration_sha256"],
        "participants": request["participants"],
        "passed": True,
        "mandatory_signed_rs16_da_rbc": rust_result[
            "mandatory_signed_rs16_da_rbc"
        ],
        "signed_rs16_da_observations": rust_result[
            "signed_rs16_da_observations"
        ],
        "authenticated_message_control": rust_result[
            "authenticated_message_control"
        ],
        "process_inventory": rust_result["process_inventory"],
        "payload": rust_result["payload"],
    }


def publish_response(path: Path, response: Mapping[str, Any]) -> None:
    """Atomically publish a response without replacing an existing path."""

    encoded = (
        json.dumps(response, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    parent = path.parent
    if parent.is_symlink() or not parent.is_dir():
        raise HarnessError("response parent changed before publication")
    directory_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    directory_flags |= getattr(os, "O_DIRECTORY", 0)
    directory_flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        directory = os.open(parent, directory_flags)
    except OSError as error:
        raise HarnessError("cannot pin response parent for publication") from error
    temporary_name = f".aps-response-{os.getpid()}.tmp"
    temporary_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    temporary_flags |= getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    temporary_created = False
    try:
        try:
            descriptor = os.open(temporary_name, temporary_flags, 0o600, dir_fd=directory)
            temporary_created = True
        except OSError as error:
            raise HarnessError("cannot create temporary response in pinned parent") from error
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        try:
            os.link(
                temporary_name,
                path.name,
                src_dir_fd=directory,
                dst_dir_fd=directory,
                follow_symlinks=False,
            )
        except (OSError, TypeError) as error:
            raise HarnessError("response path appeared before atomic publication") from error
        os.fsync(directory)
    finally:
        if temporary_created:
            try:
                os.unlink(temporary_name, dir_fd=directory)
            except FileNotFoundError:
                pass
        os.close(directory)


def main(argv: Sequence[str] | None = None) -> int:
    """Execute one fully bound real-process release request."""

    arguments = list(sys.argv[1:] if argv is None else argv)
    try:
        request_path, response_path, evidence_dir = canonicalize_paths(
            *parse_exact_arguments(arguments)
        )
        validate_paths(request_path, response_path, evidence_dir)
        raw_request = _regular_file_bytes(
            request_path, "harness request", MAX_REQUEST_BYTES
        )
        try:
            request_text = raw_request.decode("utf-8")
        except UnicodeDecodeError as error:
            raise HarnessError("harness request is not UTF-8") from error
        request = validate_request(_strict_json_loads(request_text, "harness request"))
        bind_source_revision(request)
        result = run_rust_harness(request_path, raw_request, request, evidence_dir)
        if _regular_file_bytes(request_path, "harness request", MAX_REQUEST_BYTES) != raw_request:
            raise HarnessError("request changed during real-process execution")
        if request["kind"] == "benchmark" and list(evidence_dir.iterdir()):
            raise HarnessError("benchmark emitted undeclared evidence files")
        if request["kind"] == "fault" and {
            entry.name for entry in evidence_dir.iterdir()
        } != {
            runner.FAULT_CONTROL_EVIDENCE_FILE,
            runner.FAULT_OBSERVATION_EVIDENCE_FILE,
        }:
            raise HarnessError("fault campaign did not emit its exact evidence inventory")
        publish_response(response_path, build_response(request, result))
        return 0
    except HarnessError as error:
        print(f"private-settlement real-process harness: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
