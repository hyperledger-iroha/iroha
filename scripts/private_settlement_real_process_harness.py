#!/usr/bin/env python3
"""Run one genuine AtomicPrivateSettlementV1 release experiment.

This executable implements the exact three-path contract consumed by
``private_settlement_release_runner.py``.  Private settlement and its
transparent Native AMX control and authenticated fault campaign are backed by
ignored Rust real-process tests. Leakage runs additionally bind one raw
loopback capture owned by this process to the Rust-published validator-port
manifest, then replay the canonical split files before publishing evidence.

The Python boundary authenticates the request, source revision, configuration,
Rust result freshness, and final response before publishing the response with
an atomic no-overwrite link.  Missing measurements are errors; this harness
never substitutes defaults or synthetic observations.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import os
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Mapping, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = SCRIPT_DIR.parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import private_settlement_release_runner as runner
import private_settlement_capture_split as capture_split

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
LEAKAGE_REQUEST_PAYLOAD_FIELDS = {
    "variant",
    "canaries",
    "canary_commitments",
    "only_secret_fields_change",
    "capture_surfaces",
    "traffic_count_channels",
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
LEAKAGE_RUST_RESULT_FIELDS = {
    "variant",
    "canaries_injected",
    "canary_commitments",
    "only_secret_fields_changed",
    "nonpacket_capture_complete",
    "finalized_receipt_observed",
    "successful_leg_applications",
    "each_leg_applied_exactly_once",
    "continuous_atomicity_checks",
    "partial_visible_observations",
    "partial_spendable_observations",
    "nonpacket_artifacts",
    "nonpacket_record_counts",
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
LEAKAGE_TEST_NAME = (
    "nexus::atomic_private_settlement_localnet::"
    "atomic_private_settlement_real_process_leakage_harness"
)
TARGET_DIR = REPOSITORY_ROOT / "target" / "aps-private-settlement-real-process"
VALIDATOR_EXECUTABLE = TARGET_DIR / "release" / "iroha3d"
TCPDUMP = Path("/usr/sbin/tcpdump")
TCPDUMP_START_TIMEOUT_SECONDS = 10.0
TCPDUMP_STOP_TIMEOUT_SECONDS = 20.0
SPLIT_CAPTURE_SURFACES = {
    "public_p2p_capture": "public_p2p",
    "restricted_p2p_capture": "restricted_p2p",
    "sanitized_capture": "sanitized",
    "torii_capture": "torii",
}
PYTHON_CAPTURE_SURFACES = frozenset(
    {*SPLIT_CAPTURE_SURFACES, "restricted_packet_source"}
)


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


def _copy_stable_owner_only_file(
    source: Path, destination: Path, label: str, limit: int
) -> dict[str, Any]:
    """Retain one exact bounded source without following links or overwriting."""

    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        source_flags |= os.O_NOFOLLOW
    try:
        source_descriptor = os.open(source, source_flags)
    except OSError as error:
        raise HarnessError(f"{label} must be a readable regular non-symlink file") from error
    destination_descriptor: int | None = None
    created = False
    digest = hashlib.sha256()
    copied = 0
    try:
        before = os.fstat(source_descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_size < 1
            or before.st_size > limit
        ):
            raise HarnessError(f"{label} is empty, non-regular, or exceeds its bound")
        if destination.parent.is_symlink() or not destination.parent.is_dir():
            raise HarnessError(f"{label} destination parent is unsafe")
        destination_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            destination_flags |= os.O_NOFOLLOW
        destination_descriptor = os.open(destination, destination_flags, 0o600)
        created = True
        os.fchmod(destination_descriptor, 0o600)
        with os.fdopen(source_descriptor, "rb", closefd=False) as source_stream:
            with os.fdopen(destination_descriptor, "wb", closefd=False) as destination_stream:
                while chunk := source_stream.read(1024 * 1024):
                    copied += len(chunk)
                    if copied > limit:
                        raise HarnessError(f"{label} exceeds its bounded size")
                    digest.update(chunk)
                    destination_stream.write(chunk)
                destination_stream.flush()
                os.fsync(destination_stream.fileno())
        after = os.fstat(source_descriptor)
        retained = os.fstat(destination_descriptor)
        stable = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
        if copied != before.st_size or any(
            getattr(before, field) != getattr(after, field) for field in stable
        ):
            raise HarnessError(f"{label} changed while it was retained")
        if (
            retained.st_size != copied
            or not stat.S_ISREG(retained.st_mode)
            or stat.S_IMODE(retained.st_mode) != 0o600
        ):
            raise HarnessError(f"{label} retained copy is incomplete or not owner-only")
        parent_descriptor = os.open(destination.parent, os.O_RDONLY)
        try:
            os.fsync(parent_descriptor)
        finally:
            os.close(parent_descriptor)
    except BaseException:
        if destination_descriptor is not None:
            os.close(destination_descriptor)
            destination_descriptor = None
        os.close(source_descriptor)
        source_descriptor = -1
        if created:
            try:
                destination.unlink()
            except FileNotFoundError:
                pass
        raise
    finally:
        if destination_descriptor is not None:
            os.close(destination_descriptor)
        if source_descriptor >= 0:
            os.close(source_descriptor)
    binding = runner.file_binding(destination)
    expected = {"sha256": digest.hexdigest(), "bytes": copied}
    if binding != expected:
        raise HarnessError(f"{label} retained copy differs from its stable source")
    return binding


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
    if stat.S_IMODE(evidence.stat().st_mode) != 0o700:
        raise HarnessError("evidence directory must be owner-only mode 0700")
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
    if request["kind"] not in {"benchmark", "fault", "leakage"}:
        raise HarnessError(
            "real process harness supports benchmark, fault, and leakage requests only"
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
            (
                BENCHMARK_REQUEST_PAYLOAD_FIELDS
                if request["kind"] == "benchmark"
                else FAULT_REQUEST_PAYLOAD_FIELDS
                if request["kind"] == "fault"
                else LEAKAGE_REQUEST_PAYLOAD_FIELDS
            ),
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
    elif request["kind"] == "fault":
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
    else:
        if (
            participants != runner.PRIMARY_PARTICIPANTS
            or request["run"] != 0
            or payload["variant"] not in {"left", "right"}
            or payload["only_secret_fields_change"] is not True
        ):
            raise HarnessError("leakage request is outside the primary differential profile")
        expected_canaries = runner.canaries_for_variant(
            runner.build_canary_manifest(request["commit"]), payload["variant"]
        )
        expected_commitments = {
            entry["name"]: runner.object_digest(entry) for entry in expected_canaries
        }
        expected_surfaces = [
            {"surface": surface, "relative_name": runner.SURFACE_FILES[surface]}
            for surface in sorted(runner.SURFACE_FILES)
        ]
        if (
            payload["canaries"] != expected_canaries
            or payload["canary_commitments"] != expected_commitments
            or payload["capture_surfaces"] != expected_surfaces
            or payload["traffic_count_channels"]
            != list(runner.leakage_audit.REQUIRED_COUNT_CHANNELS)
        ):
            raise HarnessError("leakage request differs from its commit-bound canary profile")
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
        if request["kind"] == "fault"
        else 1
    )
    if request["run"] >= run_limit:
        raise HarnessError("release run index is outside the configured matrix")
    if request["seed"] != seeds[request["run"] % len(seeds)]:
        raise HarnessError("release seed does not match the canonical run schedule")
    job_body = {
        "kind": request["kind"],
        **(
            {"profile": profile, "warmup": payload["warmup"]}
            if request["kind"] == "benchmark"
            else {
                "variant": payload["variant"],
                "canary_names": [entry["name"] for entry in payload["canaries"]],
                "canary_commitments": payload["canary_commitments"],
            }
            if request["kind"] == "leakage"
            else {}
        ),
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


def _loopback_interface() -> str:
    """Resolve one exact live loopback interface without accepting a wildcard."""

    try:
        names = {name for _index, name in socket.if_nameindex()}
    except OSError as error:
        raise HarnessError("cannot enumerate loopback interfaces") from error
    for candidate in ("lo0", "lo"):
        if candidate in names:
            return candidate
    raise HarnessError("neither lo0 nor lo is available for the leakage capture")


def _start_tcpdump(
    raw_pcap: Path, stderr_path: Path
) -> tuple[subprocess.Popen[bytes], Any, Path]:
    """Start the sole harness-owned loopback capture and prove it is recording."""

    if TCPDUMP.is_symlink() or not TCPDUMP.is_file() or not os.access(TCPDUMP, os.X_OK):
        raise HarnessError("/usr/sbin/tcpdump is unavailable as an executable regular file")
    if raw_pcap.exists() or stderr_path.exists():
        raise HarnessError("leakage capture temporary paths must start absent")
    stderr_stream = stderr_path.open("xb")
    try:
        process = subprocess.Popen(
            [
                str(TCPDUMP),
                "-i",
                _loopback_interface(),
                "-n",
                "-s",
                "0",
                "-U",
                "-w",
                str(raw_pcap),
            ],
            cwd=REPOSITORY_ROOT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=stderr_stream,
        )
    except OSError as error:
        stderr_stream.close()
        raise HarnessError("cannot start /usr/sbin/tcpdump") from error
    deadline = time.monotonic() + TCPDUMP_START_TIMEOUT_SECONDS
    while time.monotonic() <= deadline:
        returncode = process.poll()
        if returncode is not None:
            stderr_stream.flush()
            stderr_stream.close()
            detail = stderr_path.read_text(encoding="utf-8", errors="replace")[-2_000:]
            raise HarnessError(
                f"tcpdump exited before network startup ({returncode}): {detail}"
            )
        try:
            if raw_pcap.stat().st_size >= 24:
                return process, stderr_stream, stderr_path
        except FileNotFoundError:
            pass
        time.sleep(0.05)
    process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=TCPDUMP_STOP_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
    stderr_stream.close()
    raise HarnessError("tcpdump did not publish a capture header before network startup")


def _parse_tcpdump_statistics(raw: bytes) -> dict[str, Any]:
    """Parse tcpdump's final summary without inventing an absent drop counter."""
    try:
        return capture_split.parse_tcpdump_statistics(raw)
    except capture_split.CaptureSplitError as error:
        raise HarnessError(str(error)) from error


def _stop_tcpdump(
    process: subprocess.Popen[bytes], stderr_stream: Any, stderr_path: Path
) -> dict[str, Any]:
    """Stop the exact capture child and authenticate its final packet statistics."""

    if process.poll() is not None:
        stderr_stream.flush()
        stderr_stream.close()
        raise HarnessError(
            f"tcpdump exited before the harness stopped it ({process.returncode})"
        )
    process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=TCPDUMP_STOP_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        raise HarnessError("tcpdump did not stop cleanly after SIGINT") from error
    stderr_stream.flush()
    os.fsync(stderr_stream.fileno())
    stderr_stream.close()
    if process.returncode != 0:
        raise HarnessError(f"tcpdump exited unsuccessfully ({process.returncode})")
    raw_statistics = _regular_file_bytes(
        stderr_path, "tcpdump stderr", runner.MAX_HARNESS_RESPONSE_BYTES
    )
    return _parse_tcpdump_statistics(raw_statistics)


def _encoded_request_canaries(entries: Any) -> list[Any]:
    """Expand the exact request canaries using the publication scanner's encodings."""

    if not isinstance(entries, list) or not entries:
        raise HarnessError("leakage request does not contain canaries to scan")
    expanded: list[Any] = []
    names: set[str] = set()
    audit = runner.leakage_audit
    try:
        for index, entry in enumerate(entries):
            if not isinstance(entry, dict) or set(entry) != {"name", "kind", "value"}:
                raise HarnessError(f"leakage canary[{index}] has an invalid shape")
            name = entry["name"]
            kind = entry["kind"]
            value = entry["value"]
            if not isinstance(name, str) or not name or name in names:
                raise HarnessError(f"leakage canary[{index}] has an invalid name")
            names.add(name)
            if kind == "text" and isinstance(value, str) and value:
                expanded.extend(audit._text_variants(name, value))
            elif (
                kind == "integer"
                and isinstance(value, int)
                and not isinstance(value, bool)
            ):
                expanded.extend(audit._integer_variants(name, value))
            elif kind == "binary_base64" and isinstance(value, str):
                decoded = base64.b64decode(value, validate=True)
                expanded.extend(audit._binary_variants(name, decoded))
            else:
                raise HarnessError(f"leakage canary[{index}] has an invalid typed value")
    except (binascii.Error, ValueError, audit.AuditInputError) as error:
        raise HarnessError(f"leakage request canary encoding is invalid: {error}") from error
    return expanded


def _scan_raw_capture(
    raw_pcap: Path, request_canaries: Any
) -> dict[str, Any]:
    """Scan and bind the complete stable raw capture before any port filtering."""

    canaries = _encoded_request_canaries(request_canaries)
    try:
        binding, hits = runner.leakage_audit._inspect_file(
            raw_pcap,
            canaries,
            chunk_bytes=runner.leakage_audit.DEFAULT_CHUNK_BYTES,
            maximum_bytes=runner.leakage_audit.DEFAULT_MAX_FILE_BYTES,
        )
    except runner.leakage_audit.AuditInputError as error:
        raise HarnessError(f"cannot scan the complete raw packet capture: {error}") from error
    if hits:
        leaked_names = sorted({hit["canary"] for hit in hits})
        raise HarnessError(
            "the complete raw packet capture contains planted canaries: "
            + ", ".join(leaked_names)
        )
    return binding


def _exact_integer(value: Any, expected: int) -> bool:
    """Return whether a JSON value is exactly the requested integer, excluding bool."""

    return isinstance(value, int) and not isinstance(value, bool) and value == expected


def _materialize_leakage_result(
    result: dict[str, Any],
    *,
    request: Mapping[str, Any],
    evidence_dir: Path,
    raw_pcap: Path,
    port_manifest: Path,
    tcpdump_stderr: Path,
    tcpdump_statistics: Mapping[str, Any],
) -> dict[str, Any]:
    """Bind real Rust non-packet sources and replay the outer packet capture."""

    try:
        payload = runner.exact_fields(
            result["payload"], LEAKAGE_RUST_RESULT_FIELDS, "Rust leakage payload"
        )
    except (KeyError, runner.RunnerError) as error:
        raise HarnessError(f"Rust leakage payload is invalid: {error}") from error
    if (
        payload["variant"] != request["payload"]["variant"]
        or payload["canaries_injected"]
        != [entry["name"] for entry in request["payload"]["canaries"]]
        or payload["canary_commitments"]
        != request["payload"]["canary_commitments"]
        or payload["only_secret_fields_changed"] is not True
        or payload["nonpacket_capture_complete"] is not True
        or payload["finalized_receipt_observed"] is not True
        or payload["successful_leg_applications"] != request["participants"]
        or payload["each_leg_applied_exactly_once"] is not True
        or not _exact_integer(payload["partial_visible_observations"], 0)
        or not _exact_integer(payload["partial_spendable_observations"], 0)
        or isinstance(payload["continuous_atomicity_checks"], bool)
        or not isinstance(payload["continuous_atomicity_checks"], int)
        or payload["continuous_atomicity_checks"] < (request["participants"] + 1) * 12
    ):
        raise HarnessError("Rust leakage result did not prove the requested private run")

    nonpacket_surfaces = {
        surface: name
        for surface, name in runner.SURFACE_FILES.items()
        if surface not in PYTHON_CAPTURE_SURFACES
    }
    artifacts = payload["nonpacket_artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != len(nonpacket_surfaces):
        raise HarnessError("Rust leakage result omitted a non-packet surface")
    final_artifacts: list[dict[str, Any]] = []
    for index, item in enumerate(artifacts):
        try:
            row = runner.exact_fields(
                item,
                {
                    "surface",
                    "relative_name",
                    "sha256",
                    "bytes",
                    "source_sha256",
                    "source_bytes",
                    "source_count",
                },
                f"Rust leakage artifact[{index}]",
            )
        except runner.RunnerError as error:
            raise HarnessError(str(error)) from error
        expected_surface = sorted(nonpacket_surfaces)[index]
        if (
            row["surface"] != expected_surface
            or row["relative_name"] != nonpacket_surfaces[expected_surface]
            or not isinstance(row["source_count"], int)
            or isinstance(row["source_count"], bool)
            or row["source_count"] < 1
            or not isinstance(row["source_bytes"], int)
            or isinstance(row["source_bytes"], bool)
            or row["source_bytes"] < 1
        ):
            raise HarnessError("Rust leakage artifact source provenance is incomplete")
        _require_sha(
            row["source_sha256"],
            f"Rust leakage artifact[{index}].source_sha256",
        )
        path = evidence_dir / row["relative_name"]
        binding = runner.file_binding(path)
        if binding != {"sha256": row["sha256"], "bytes": row["bytes"]}:
            raise HarnessError("Rust leakage artifact binding differs from its source file")
        final_artifacts.append(dict(row))

    expected_nonpacket_counts = {
        "block_messages",
        "query_responses",
        "event_records",
        "log_records",
        "telemetry_records",
    }
    nonpacket_counts = payload["nonpacket_record_counts"]
    if not isinstance(nonpacket_counts, dict) or set(nonpacket_counts) != expected_nonpacket_counts:
        raise HarnessError("Rust leakage non-packet count inventory is incomplete")
    if any(
        isinstance(value, bool) or not isinstance(value, int) or value < 1
        for value in nonpacket_counts.values()
    ):
        raise HarnessError("Rust leakage non-packet count inventory contains an empty channel")
    try:
        derived_nonpacket = runner.derive_leakage_nonpacket_counts(evidence_dir)
    except runner.RunnerError as error:
        raise HarnessError(f"cannot replay Rust non-packet evidence: {error}") from error
    if derived_nonpacket != nonpacket_counts:
        raise HarnessError("Rust leakage non-packet counts are not source-backed")

    try:
        port_document, groups, manifest_binding = (
            capture_split.load_bound_port_manifest(port_manifest)
        )
    except (
        capture_split.CaptureSplitError,
        capture_split.pcapng.CaptureFormatError,
        OSError,
    ) as error:
        raise HarnessError(f"invalid Rust leakage port manifest: {error}") from error
    expected_peers = (request["participants"] + 1) * 4
    if (
        len(groups["torii"]) != expected_peers
        or len(groups["public_p2p"]) != 4
        or len(groups["restricted_p2p"]) != request["participants"] * 4
    ):
        raise HarnessError("Rust leakage port manifest does not cover the exact topology")
    captured_packets = tcpdump_statistics.get("captured_packets")
    received_packets = tcpdump_statistics.get("received_by_filter_packets")
    drop_counters = tcpdump_statistics.get("drop_counters")
    if (
        isinstance(captured_packets, bool)
        or not isinstance(captured_packets, int)
        or captured_packets < 1
        or isinstance(received_packets, bool)
        or not isinstance(received_packets, int)
        or received_packets < captured_packets
        or not isinstance(drop_counters, dict)
        or not drop_counters
        or not set(drop_counters).issubset({"kernel", "interface"})
        or any(
            not isinstance(name, str)
            or isinstance(count, bool)
            or not isinstance(count, int)
            or count != 0
            for name, count in drop_counters.items()
        )
    ):
        raise HarnessError("tcpdump statistics do not prove a complete packet capture")
    raw_binding = _scan_raw_capture(raw_pcap, request["payload"]["canaries"])
    tcpdump_stderr_bytes = _regular_file_bytes(
        tcpdump_stderr, "tcpdump stderr", runner.MAX_HARNESS_RESPONSE_BYTES
    )
    try:
        replayed_tcpdump_statistics = capture_split.parse_tcpdump_statistics(
            tcpdump_stderr_bytes
        )
    except capture_split.CaptureSplitError as error:
        raise HarnessError(f"cannot replay tcpdump statistics: {error}") from error
    if replayed_tcpdump_statistics != tcpdump_statistics:
        raise HarnessError("tcpdump statistics changed between stop and evidence replay")
    try:
        split_packet_counts = capture_split.packet_count_claims(
            capture_split.split_capture(
                raw_pcap,
                evidence_dir,
                groups,
                expected_source_packets=captured_packets,
            )
        )
        packet_bindings_before = {
            surface: runner.file_binding(evidence_dir / relative_name)
            for surface, relative_name in capture_split.OUTPUT_NAMES.items()
        }
        replayed_packet_counts = capture_split.derive_split_packet_counts(
            evidence_dir, groups
        )
        packet_bindings_after = {
            surface: runner.file_binding(evidence_dir / relative_name)
            for surface, relative_name in capture_split.OUTPUT_NAMES.items()
        }
        raw_binding_after = runner.file_binding(raw_pcap)
        post_port_document, post_groups, manifest_binding_after = (
            capture_split.load_bound_port_manifest(port_manifest)
        )
    except (
        capture_split.CaptureSplitError,
        capture_split.pcapng.CaptureFormatError,
        runner.RunnerError,
        OSError,
    ) as error:
        raise HarnessError(f"leakage packet capture cannot be authenticated: {error}") from error
    if split_packet_counts != replayed_packet_counts:
        raise HarnessError("leakage packet counts changed during independent replay")
    if packet_bindings_before != packet_bindings_after:
        raise HarnessError("leakage split packet files changed during replay")
    if raw_binding_after != raw_binding:
        raise HarnessError("raw packet capture changed after its complete canary scan")
    if (
        post_port_document != port_document
        or post_groups != groups
        or manifest_binding_after != manifest_binding
        or capture_split.canonical_port_manifest_binding(
            capture_split.validate_port_manifest(port_document)
        )
        != manifest_binding
    ):
        raise HarnessError("capture port manifest changed or lost its document binding")
    packet_bindings_by_name = {
        capture_split.OUTPUT_NAMES[channel]: binding
        for channel, binding in packet_bindings_after.items()
    }
    for surface in sorted(SPLIT_CAPTURE_SURFACES):
        path = evidence_dir / runner.SURFACE_FILES[surface]
        final_artifacts.append(
            {
                "surface": surface,
                "relative_name": runner.SURFACE_FILES[surface],
                **packet_bindings_by_name[path.name],
                "source_sha256": raw_binding["sha256"],
                "source_bytes": raw_binding["bytes"],
                "source_count": 1,
            }
        )
    retained_raw = evidence_dir / runner.SURFACE_FILES["restricted_packet_source"]
    retained_raw_binding = _copy_stable_owner_only_file(
        raw_pcap,
        retained_raw,
        "complete raw packet capture",
        runner.leakage_audit.DEFAULT_MAX_FILE_BYTES,
    )
    if retained_raw_binding != raw_binding:
        raise HarnessError("retained raw packet capture differs from its scanned source")
    final_artifacts.append(
        {
            "surface": "restricted_packet_source",
            "relative_name": runner.SURFACE_FILES["restricted_packet_source"],
            **retained_raw_binding,
            "source_sha256": retained_raw_binding["sha256"],
            "source_bytes": retained_raw_binding["bytes"],
            "source_count": 1,
        }
    )
    final_artifacts.sort(key=lambda row: row["surface"])
    traffic_counts = {
        "torii_request_packets": split_packet_counts["torii_request_packets"],
        "torii_response_packets": split_packet_counts["torii_response_packets"],
        "public_p2p_packets": split_packet_counts["public_p2p_packets"],
        "restricted_p2p_packets": split_packet_counts["restricted_p2p_packets"],
        **nonpacket_counts,
    }
    if any(value < 1 for value in traffic_counts.values()):
        raise HarnessError("one or more source-backed leakage channels are empty")
    result["payload"] = {
        "variant": payload["variant"],
        "canaries_injected": payload["canaries_injected"],
        "canary_commitments": payload["canary_commitments"],
        "only_secret_fields_changed": True,
        "capture_complete": True,
        "finalized_receipt_observed": True,
        "successful_leg_applications": payload["successful_leg_applications"],
        "each_leg_applied_exactly_once": True,
        "continuous_atomicity_checks": payload["continuous_atomicity_checks"],
        "partial_visible_observations": 0,
        "partial_spendable_observations": 0,
        "capture_provenance": {
            "raw_pcap": raw_binding,
            "port_manifest": manifest_binding,
            "ports": port_document,
            "packet_counts": split_packet_counts,
            "tcpdump": {
                "stderr_base64": base64.b64encode(tcpdump_stderr_bytes).decode("ascii"),
                "stderr_sha256": hashlib.sha256(tcpdump_stderr_bytes).hexdigest(),
                "stderr_bytes": len(tcpdump_stderr_bytes),
                "statistics": dict(tcpdump_statistics),
            },
        },
        "artifacts": final_artifacts,
        "traffic_counts": traffic_counts,
    }
    return result


def run_rust_harness(
    request_path: Path,
    raw_request: bytes,
    request: Mapping[str, Any],
    evidence_dir: Path,
) -> dict[str, Any]:
    """Build the feature-isolated daemon and run the exact ignored Rust test."""

    request_sha = hashlib.sha256(raw_request).hexdigest()
    with tempfile.TemporaryDirectory(prefix="aps-real-process-") as temporary:
        temporary_root = Path(temporary)
        rust_result = temporary_root / "rust-result.json"
        raw_pcap = temporary_root / "leakage-loopback.pcap"
        port_manifest = temporary_root / "leakage-ports.json"
        tcpdump_stderr = temporary_root / "tcpdump.stderr"
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
        test_name = (
            BENCHMARK_TEST_NAME
            if request["kind"] == "benchmark"
            else FAULT_TEST_NAME
            if request["kind"] == "fault"
            else LEAKAGE_TEST_NAME
        )
        capture: tuple[subprocess.Popen[bytes], Any, Path] | None = None
        tcpdump_statistics: dict[str, Any] | None = None
        if request["kind"] == "leakage":
            environment["APS_REAL_PROCESS_PORT_MANIFEST"] = str(port_manifest)
            capture = _start_tcpdump(raw_pcap, tcpdump_stderr)
        try:
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
                    test_name,
                    "--",
                    "--ignored",
                    "--exact",
                    "--nocapture",
                    "--test-threads=1",
                ],
                environment=environment,
            )
        finally:
            if capture is not None:
                tcpdump_statistics = _stop_tcpdump(*capture)
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
        if request["kind"] == "leakage":
            if not isinstance(result, dict):
                raise HarnessError("Rust leakage result is not an object")
            if tcpdump_statistics is None:
                raise HarnessError("leakage capture lacks authenticated tcpdump statistics")
            result = _materialize_leakage_result(
                result,
                request=request,
                evidence_dir=evidence_dir,
                raw_pcap=raw_pcap,
                port_manifest=port_manifest,
                tcpdump_stderr=tcpdump_stderr,
                tcpdump_statistics=tcpdump_statistics,
            )
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
            if request["kind"] == "fault"
            else runner.exact_fields(
                result["payload"], runner.LEAKAGE_PAYLOAD_FIELDS, "Rust result.payload"
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
            else {
                "variant": request["payload"]["variant"],
                "canary_names": [
                    entry["name"] for entry in request["payload"]["canaries"]
                ],
                "canary_commitments": request["payload"]["canary_commitments"],
            }
            if request["kind"] == "leakage"
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
        elif request["kind"] == "fault":
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
        else:
            if evidence_dir is None:
                raise HarnessError("leakage result validation requires its evidence directory")
            runner.validate_leakage_response(
                envelope,
                plan=plan,
                job=job,
                evidence_dir=evidence_dir,
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
        if request["kind"] == "leakage" and {
            entry.name for entry in evidence_dir.iterdir()
        } != set(runner.SURFACE_FILES.values()):
            raise HarnessError(
                f"leakage campaign did not emit its exact {len(runner.SURFACE_FILES)}-file inventory"
            )
        publish_response(response_path, build_response(request, result))
        return 0
    except HarnessError as error:
        print(f"private-settlement real-process harness: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
