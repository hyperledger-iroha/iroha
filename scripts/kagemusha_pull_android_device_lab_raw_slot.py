#!/usr/bin/env python3
"""Pull raw Kagemusha Android device-lab artifacts from an attached device."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import io
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
from typing import Any, Callable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402


RAW_PULL_SUMMARY_SCHEMA = "iroha.android.device_lab.kagemusha.raw_pull.v1"
DEFAULT_RUN_AS_PACKAGE = "org.hyperledger.iroha.sdk.offline.wallet.lab"
DEFAULT_DEVICE_LAB_DEVICE_ROOT = "files/kagemusha-device-lab"
DEFAULT_OUT_ROOT = Path("target/kagemusha-android-raw")
MAX_RAW_SLOT_TAR_BYTES = 128 * 1024 * 1024
MAX_RAW_SLOT_FILE_BYTES = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
MAX_RAW_SLOT_FILES = 128
RAW_SLOT_REQUIRED_PATHS: tuple[str, ...] = (
    "attestation/challenge.hex",
    "attestation/harness-result.json",
    "attestation/keymint-certificate-chain.pem",
    "attestation/result.json",
    "handoff/d2d-payment.json",
    "wallet/integrity.json",
    "telemetry/telemetry.json",
    "telemetry/status.ndjson",
    "queue/pending_queue.json",
    "logs/runtime.log",
)
HARNESS_RESULT_ALLOWED_FIELDS: frozenset[str] = frozenset(
    {
        "alias",
        "attestation_security_level",
        "keymaster_security_level",
        "strongbox_attestation",
        "challenge_hex",
        "chain_length",
    }
)
ADB_LATEST_SLOT_COMMAND_HELP = (
    "adb shell run-as <package> cat files/kagemusha-device-lab/latest-slot.txt"
)
ADB_PULL_TAR_COMMAND_HELP = (
    "adb exec-out run-as <package> tar -C files/kagemusha-device-lab "
    "-cf - <slot-id> latest-slot.txt"
)


Runner = Callable[..., subprocess.CompletedProcess]


def _json_dumps(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"


def _safe_detail(text: str, limit: int = 512) -> str:
    text = text.replace("\r", "\n").strip()
    if device_lab.SECRET_RE.search(text):
        return "<redacted-secret-output>"
    if len(text) > limit:
        return text[:limit] + "...<truncated>"
    return text


def _single_safe_slot_id(raw_slot_id: str) -> tuple[str | None, list[str]]:
    normalised, errors = device_lab.validate_slot_ids([raw_slot_id])
    if errors:
        return None, errors
    if not normalised or len(normalised) != 1:
        return None, ["slot id must be a single safe directory name"]
    return normalised[0], []


def _validate_non_secret_adb_string(value: str, label: str) -> list[str]:
    if not value.strip():
        return [f"{label} must be a non-empty string"]
    if device_lab.SECRET_RE.search(value):
        return [f"{label} must not contain secret-looking material"]
    return []


def _pem_certificate_count(chain_text: str) -> int:
    return chain_text.count("-----BEGIN CERTIFICATE-----")


def _validate_harness_result(
    *,
    harness: dict[str, Any],
    challenge_text: str | None,
    chain_text: str | None,
    errors: list[str],
) -> None:
    for field in sorted(set(harness) - HARNESS_RESULT_ALLOWED_FIELDS):
        errors.append(
            "attestation/harness-result.json contains unexpected field "
            f"{device_lab._display_path(field)}"
        )
    alias = harness.get("alias")
    if not isinstance(alias, str) or not alias:
        errors.append("attestation/harness-result.json alias must be a non-empty string")
    elif alias != alias.strip():
        errors.append("attestation/harness-result.json alias must not have surrounding whitespace")
    elif device_lab.SECRET_RE.search(alias):
        errors.append("attestation/harness-result.json alias must not contain secret-looking material")
    for key in ("attestation_security_level", "keymaster_security_level"):
        level = harness.get(key)
        if not isinstance(level, str) or level not in device_lab.STRONGBOX_LEVELS:
            errors.append(f"attestation/harness-result.json {key} must be STRONGBOX")
    if harness.get("strongbox_attestation") is not True:
        errors.append("attestation/harness-result.json strongbox_attestation must be true")
    chain_length = harness.get("chain_length")
    if not isinstance(chain_length, int) or chain_length < 2:
        errors.append("attestation/harness-result.json chain_length must be at least 2")
    elif chain_text is not None:
        certificate_count = _pem_certificate_count(chain_text)
        if certificate_count < 2:
            errors.append("attestation/keymint-certificate-chain.pem must contain at least two PEM certificates")
        elif chain_length != certificate_count:
            errors.append(
                "attestation/harness-result.json chain_length must match "
                "attestation/keymint-certificate-chain.pem certificate count"
            )
    challenge_hex = harness.get("challenge_hex")
    if not isinstance(challenge_hex, str) or not challenge_hex.strip():
        errors.append("attestation/harness-result.json challenge_hex must be a non-empty string")
        return
    normalized = "".join(challenge_hex.split()).lower()
    if challenge_hex != normalized:
        errors.append("attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace")
        return
    if len(normalized) % 2 != 0:
        errors.append("attestation/harness-result.json challenge_hex must have even length")
        return
    try:
        bytes.fromhex(normalized)
    except ValueError:
        errors.append("attestation/harness-result.json challenge_hex must be hex")
        return
    if challenge_text is not None and normalized != challenge_text.strip().lower():
        errors.append("attestation/harness-result.json challenge_hex must match attestation/challenge.hex")


def _adb_command(adb: str, serial: str | None, args: list[str]) -> list[str]:
    command = [adb]
    if serial:
        command.extend(["-s", serial])
    command.extend(args)
    return command


def _run_latest_slot_query(
    *,
    adb: str,
    serial: str | None,
    run_as_package: str,
    device_lab_root: str,
    timeout_seconds: int,
    runner: Runner,
) -> tuple[str | None, list[str]]:
    command = _adb_command(
        adb,
        serial,
        [
            "shell",
            "run-as",
            run_as_package,
            "cat",
            f"{device_lab_root}/latest-slot.txt",
        ],
    )
    try:
        result = runner(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return None, [f"failed to read latest raw slot from attached device: {exc}"]
    if result.returncode != 0:
        detail = _safe_detail(str(result.stderr))
        suffix = f": {detail}" if detail else ""
        return None, [f"failed to read latest raw slot from attached device{suffix}"]
    lines = [line.strip() for line in result.stdout.replace("\r", "\n").splitlines()]
    lines = [line for line in lines if line]
    if len(lines) != 1:
        return None, ["latest-slot.txt must contain exactly one slot id"]
    return lines[0], []


def _run_raw_slot_tar_pull(
    *,
    adb: str,
    serial: str | None,
    run_as_package: str,
    device_lab_root: str,
    slot_id: str,
    timeout_seconds: int,
    runner: Runner,
) -> tuple[bytes | None, list[str]]:
    command = _adb_command(
        adb,
        serial,
        [
            "exec-out",
            "run-as",
            run_as_package,
            "tar",
            "-C",
            device_lab_root,
            "-cf",
            "-",
            slot_id,
            "latest-slot.txt",
        ],
    )
    try:
        result = runner(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout_seconds,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return None, [f"failed to pull raw slot tar from attached device: {exc}"]
    if result.returncode != 0:
        stderr = result.stderr.decode("utf-8", errors="replace")
        detail = _safe_detail(stderr)
        suffix = f": {detail}" if detail else ""
        return None, [f"failed to pull raw slot tar from attached device{suffix}"]
    data = bytes(result.stdout)
    if not data:
        return None, ["raw slot tar stream must be non-empty"]
    if len(data) > MAX_RAW_SLOT_TAR_BYTES:
        return None, [f"raw slot tar stream must not exceed {MAX_RAW_SLOT_TAR_BYTES} bytes"]
    return data, []


def _normalise_tar_member_name(name: str, errors: list[str]) -> str | None:
    if device_lab.SECRET_RE.search(name):
        errors.append("raw slot tar member path must not contain secret-looking material")
        return None
    candidate = PurePosixPath(name)
    normalised = candidate.as_posix()
    if (
        not name.strip()
        or name.startswith("/")
        or "\\" in name
        or candidate.is_absolute()
        or normalised in {"", "."}
        or ".." in candidate.parts
    ):
        errors.append(f"raw slot tar member has unsafe path {device_lab._display_path(name)!r}")
        return None
    return normalised


def _member_allowed_for_slot(relative: str, slot_id: str) -> bool:
    return relative == "latest-slot.txt" or relative == slot_id or relative.startswith(
        f"{slot_id}/"
    )


def _write_regular_member(
    *,
    tar: tarfile.TarFile,
    member: tarfile.TarInfo,
    destination_root: Path,
    relative: str,
    errors: list[str],
) -> int:
    if member.size < 0:
        errors.append(f"raw slot tar member {relative} has invalid size")
        return 0
    if member.size > MAX_RAW_SLOT_FILE_BYTES:
        errors.append(
            f"raw slot tar member {relative} must not exceed {MAX_RAW_SLOT_FILE_BYTES} bytes"
        )
        return 0
    source = tar.extractfile(member)
    if source is None:
        errors.append(f"raw slot tar member {relative} could not be read")
        return 0
    try:
        data = source.read()
    except OSError:
        errors.append(f"raw slot tar member {relative} could not be read")
        return 0
    if len(data) != member.size:
        errors.append(f"raw slot tar member {relative} changed while being read")
        return 0
    destination = destination_root / relative
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        errors.append(f"raw slot tar member {relative} parent directory could not be created")
        return 0
    try:
        with destination.open("xb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
    except FileExistsError:
        errors.append(f"raw slot tar member {relative} is duplicated")
        return 0
    except OSError:
        errors.append(f"raw slot tar member {relative} could not be written")
        return 0
    return len(data)


def extract_raw_slot_tar(
    tar_bytes: bytes,
    destination_root: Path,
    slot_id: str,
) -> list[str]:
    """Strictly extract a raw device-lab tar stream under ``destination_root``."""

    errors: list[str] = []
    seen: set[str] = set()
    file_count = 0
    total_bytes = 0
    try:
        tar = tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:*")
    except tarfile.TarError:
        return ["raw slot tar stream could not be parsed"]
    with tar:
        for member in tar:
            relative = _normalise_tar_member_name(member.name, errors)
            if relative is None:
                continue
            if not _member_allowed_for_slot(relative, slot_id):
                errors.append(f"raw slot tar member {relative} is outside requested slot")
                continue
            if relative in seen:
                errors.append(f"raw slot tar member {relative} is duplicated")
                continue
            seen.add(relative)
            if member.isdir():
                (destination_root / relative).mkdir(parents=True, exist_ok=True)
                continue
            if member.issym() or member.islnk():
                errors.append(
                    f"raw slot tar member {relative} must not be a symlink or hardlink"
                )
                continue
            if not member.isfile():
                errors.append(f"raw slot tar member {relative} must be a regular file")
                continue
            file_count += 1
            if file_count > MAX_RAW_SLOT_FILES:
                errors.append(f"raw slot tar must not contain more than {MAX_RAW_SLOT_FILES} files")
                continue
            total_bytes += _write_regular_member(
                tar=tar,
                member=member,
                destination_root=destination_root,
                relative=relative,
                errors=errors,
            )
            if total_bytes > MAX_RAW_SLOT_TAR_BYTES:
                errors.append(
                    f"raw slot extracted bytes must not exceed {MAX_RAW_SLOT_TAR_BYTES}"
                )
    return errors


def _read_text_file(path: Path, label: str, errors: list[str], max_bytes: int = 64 * 1024) -> str | None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        errors.append(f"{label} is missing")
        return None
    except OSError:
        errors.append(f"{label} metadata could not be read")
        return None
    if stat.S_ISLNK(mode):
        errors.append(f"{label} must not be a symlink")
        return None
    if not stat.S_ISREG(mode):
        errors.append(f"{label} must be a regular file")
        return None
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return None
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
        return None
    try:
        data = path.read_bytes()
    except OSError:
        errors.append(f"{label} could not be read")
        return None
    if len(data) > max_bytes:
        errors.append(f"{label} must not exceed {max_bytes} bytes")
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        errors.append(f"{label} could not be read")
        return None


def _validate_raw_slot_files(slot_path: Path, slot_id: str, root_latest: Path) -> list[str]:
    errors: list[str] = []
    if device_lab.SECRET_RE.search(str(slot_path)):
        return ["raw slot path must not contain secret-looking material"]
    try:
        slot_mode = slot_path.lstat().st_mode
    except FileNotFoundError:
        return ["raw slot directory is missing"]
    except OSError:
        return ["raw slot directory metadata could not be read"]
    if stat.S_ISLNK(slot_mode):
        return ["raw slot directory must not be a symlink"]
    if not stat.S_ISDIR(slot_mode):
        return ["raw slot path must be a directory"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        slot_path,
        "raw slot ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors

    for path in slot_path.rglob("*"):
        relative = path.relative_to(slot_path).as_posix()
        if device_lab.SECRET_RE.search(relative):
            errors.append("raw slot artifact paths must not contain secret-looking material")
            continue
        try:
            mode = path.lstat().st_mode
        except OSError:
            errors.append(f"raw slot artifact {relative} metadata could not be read")
            continue
        if stat.S_ISLNK(mode):
            errors.append(f"raw slot artifact {relative} must not be a symlink")
            continue
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode):
            errors.append(f"raw slot artifact {relative} must be a regular file")
            continue
        try:
            link_count = path.stat().st_nlink
        except OSError:
            errors.append(f"raw slot artifact {relative} hardlink metadata could not be read")
            continue
        if link_count > 1:
            errors.append(f"raw slot artifact {relative} must not be hardlinked")
        try:
            size = path.stat().st_size
        except OSError:
            errors.append(f"raw slot artifact {relative} size could not be read")
            continue
        if size == 0:
            errors.append(f"raw slot artifact {relative} must be non-empty")
        if size > MAX_RAW_SLOT_FILE_BYTES:
            errors.append(
                f"raw slot artifact {relative} must not exceed {MAX_RAW_SLOT_FILE_BYTES} bytes"
            )

    for relative in RAW_SLOT_REQUIRED_PATHS:
        if not (slot_path / relative).exists():
            errors.append(f"raw slot artifact {relative} is missing")

    latest_text = _read_text_file(root_latest, "latest-slot.txt", errors)
    if latest_text is not None and latest_text.strip() != slot_id:
        errors.append("latest-slot.txt must match slot id")

    challenge_text = _read_text_file(
        slot_path / "attestation" / "challenge.hex",
        "attestation/challenge.hex",
        errors,
    )
    chain_text = _read_text_file(
        slot_path / "attestation" / "keymint-certificate-chain.pem",
        "attestation/keymint-certificate-chain.pem",
        errors,
    )
    harness = device_lab._load_json(
        slot_path / "attestation" / "harness-result.json",
        "attestation harness result",
        errors,
    )
    if harness is not None:
        _validate_harness_result(
            harness=dict(harness),
            challenge_text=challenge_text,
            chain_text=chain_text,
            errors=errors,
        )

    result = device_lab._load_json(slot_path / "attestation" / "result.json", "attestation result", errors)
    if result is not None:
        if result.get("slot_id") != slot_id:
            errors.append("attestation/result.json slot_id must match slot id")
        if result.get("slot") not in (None, slot_id):
            errors.append("attestation/result.json slot must match slot id")
        if result.get("status") != "ok":
            errors.append("attestation/result.json status must be ok")
        if result.get("strongbox_attestation") is not True:
            errors.append("attestation/result.json strongbox_attestation must be true")
        if result.get("physical_device_attestation") is not True:
            errors.append("attestation/result.json physical_device_attestation must be true")
        chain_path = result.get("attestation_certificate_chain_path")
        if chain_path != "attestation/keymint-certificate-chain.pem":
            errors.append(
                "attestation/result.json attestation_certificate_chain_path must be "
                "attestation/keymint-certificate-chain.pem"
            )
        chain_digest = result.get("attestation_certificate_chain_sha256")
        chain_file = slot_path / "attestation" / "keymint-certificate-chain.pem"
        if isinstance(chain_digest, str) and chain_file.exists():
            digest = hashlib.sha256(chain_file.read_bytes()).hexdigest()
            if chain_digest != digest:
                errors.append("attestation/result.json certificate-chain SHA-256 mismatch")
        challenge_digest = result.get("attestation_challenge_sha256")
        if challenge_text is not None:
            challenge_hex = challenge_text.strip().lower()
            try:
                challenge = bytes.fromhex(challenge_hex)
            except ValueError:
                errors.append("attestation/challenge.hex must be lowercase hexadecimal")
                challenge = b""
            if challenge and isinstance(challenge_digest, str):
                if hashlib.sha256(challenge).hexdigest() != challenge_digest:
                    errors.append("attestation/result.json attestation challenge SHA-256 mismatch")

    status_text = _read_text_file(
        slot_path / "telemetry" / "status.ndjson",
        "telemetry/status.ndjson",
        errors,
    )
    if status_text is not None:
        has_ok = False
        for line_no, raw_line in enumerate(status_text.splitlines(), start=1):
            if not raw_line.strip():
                continue
            try:
                status_event = json.loads(raw_line)
            except json.JSONDecodeError:
                errors.append(f"telemetry/status.ndjson line {line_no} must be JSON")
                continue
            if status_event.get("status") == "ok":
                has_ok = True
        if not has_ok:
            errors.append("telemetry/status.ndjson must contain at least one ok status")

    runtime_text = _read_text_file(
        slot_path / "logs" / "runtime.log",
        "logs/runtime.log",
        errors,
    )
    if runtime_text is not None and device_lab.KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER not in runtime_text:
        errors.append("logs/runtime.log must contain Kagemusha device-lab completion marker")

    return errors


def _validate_output_root(root: Path) -> list[str]:
    if device_lab.SECRET_RE.search(str(root)):
        return ["raw output root path must not contain secret-looking material"]
    root_exists, errors = device_lab.classify_device_lab_root_path(root)
    if errors:
        return errors
    if not root_exists:
        try:
            root.mkdir(parents=True, exist_ok=True)
        except OSError:
            return ["raw output root directory could not be created"]
        root_exists, errors = device_lab.classify_device_lab_root_path(root)
        if errors:
            return errors
        if not root_exists:
            return ["raw output root must be an existing directory"]
    return []


def _write_latest_slot(root: Path, slot_id: str) -> list[str]:
    latest_path = root / "latest-slot.txt"
    errors = device_lab.validate_summary_output_path(latest_path, "raw latest-slot output")
    if errors:
        return errors
    fd, temp_name = tempfile.mkstemp(prefix=".latest-slot.", suffix=".tmp", dir=root)
    temp_path = Path(temp_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as output:
            output.write(slot_id + "\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temp_path, latest_path)
        with latest_path.open("rb") as readback:
            if readback.read() != (slot_id + "\n").encode("utf-8"):
                return ["raw latest-slot output readback mismatch"]
        try:
            dir_fd = os.open(root, os.O_RDONLY)
        except OSError:
            return ["raw latest-slot output parent directory could not be synced"]
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)
    except OSError:
        return ["raw latest-slot output could not be written"]
    finally:
        temp_path.unlink(missing_ok=True)
    return []


def _write_summary(path: Path, payload: dict[str, Any]) -> list[str]:
    errors = device_lab.validate_summary_output_path(path, "raw pull summary output")
    if errors:
        return errors
    fd, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    temp_path = Path(temp_name)
    encoded = _json_dumps(payload).encode("utf-8")
    try:
        with os.fdopen(fd, "wb") as output:
            output.write(encoded)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temp_path, path)
        if path.read_bytes() != encoded:
            return ["raw pull summary output readback mismatch"]
    except OSError:
        return ["raw pull summary output could not be written"]
    finally:
        temp_path.unlink(missing_ok=True)
    return []


def _raw_artifact_digests(slot_path: Path) -> dict[str, str]:
    digests: dict[str, str] = {}
    for relative in RAW_SLOT_REQUIRED_PATHS:
        path = slot_path / relative
        if path.is_file():
            digests[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    return digests


def pull_raw_slot(
    args: argparse.Namespace,
    *,
    runner: Runner = subprocess.run,
) -> tuple[int, Path | None, list[str]]:
    """Pull and validate one raw Kagemusha Android device-lab slot."""

    errors: list[str] = []
    for value, label in (
        (args.adb, "adb executable"),
        (args.run_as_package, "run-as package"),
        (args.device_lab_root, "device lab root"),
    ):
        errors.extend(_validate_non_secret_adb_string(value, label))
    if args.serial:
        errors.extend(_validate_non_secret_adb_string(args.serial, "ADB serial"))
    if args.adb_timeout_seconds <= 0:
        errors.append("--adb-timeout-seconds must be positive")
    if errors:
        return 1, None, errors

    if args.slot_id:
        raw_slot_id = args.slot_id
    else:
        raw_slot_id, latest_errors = _run_latest_slot_query(
            adb=args.adb,
            serial=args.serial,
            run_as_package=args.run_as_package,
            device_lab_root=args.device_lab_root,
            timeout_seconds=args.adb_timeout_seconds,
            runner=runner,
        )
        if latest_errors:
            return 1, None, latest_errors
        assert raw_slot_id is not None
    slot_id, slot_errors = _single_safe_slot_id(raw_slot_id)
    if slot_errors or slot_id is None:
        return 1, None, slot_errors

    output_root = args.out_root
    root_errors = _validate_output_root(output_root)
    if root_errors:
        return 1, None, root_errors
    final_slot = output_root / slot_id
    try:
        final_slot_mode = final_slot.lstat().st_mode
    except FileNotFoundError:
        final_slot_mode = None
    except OSError:
        return 1, None, ["raw slot directory metadata could not be read"]
    if final_slot_mode is not None:
        return 1, None, ["slot directory already exists; refuse to overwrite raw evidence"]

    tar_bytes, tar_errors = _run_raw_slot_tar_pull(
        adb=args.adb,
        serial=args.serial,
        run_as_package=args.run_as_package,
        device_lab_root=args.device_lab_root,
        slot_id=slot_id,
        timeout_seconds=args.adb_timeout_seconds,
        runner=runner,
    )
    if tar_errors or tar_bytes is None:
        return 1, None, tar_errors

    temp_parent = Path(tempfile.mkdtemp(prefix=f".{slot_id}.", dir=output_root))
    try:
        extract_errors = extract_raw_slot_tar(tar_bytes, temp_parent, slot_id)
        if extract_errors:
            return 1, None, extract_errors
        stage_slot = temp_parent / slot_id
        validate_errors = _validate_raw_slot_files(
            stage_slot,
            slot_id,
            temp_parent / "latest-slot.txt",
        )
        if validate_errors:
            return 1, None, validate_errors
        try:
            stage_slot.rename(final_slot)
        except OSError:
            return 1, None, ["raw slot directory could not be installed"]
    finally:
        shutil.rmtree(temp_parent, ignore_errors=True)

    latest_errors = _write_latest_slot(output_root, slot_id)
    if latest_errors:
        return 1, final_slot, latest_errors
    if args.summary_out is not None:
        summary = {
            "schema": RAW_PULL_SUMMARY_SCHEMA,
            "pulled_at_utc": dt.datetime.now(dt.timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "slot_id": slot_id,
            "run_as_package": args.run_as_package,
            "adb_serial": args.serial or "",
            "device_lab_root": args.device_lab_root,
            "output_root": str(output_root),
            "slot_path": str(final_slot),
            "artifact_sha256": _raw_artifact_digests(final_slot),
        }
        summary_errors = _write_summary(args.summary_out, summary)
        if summary_errors:
            return 1, final_slot, summary_errors
    return 0, final_slot, []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Pull raw Kagemusha Android device-lab artifacts written by "
            "KagemushaDeviceLabArtifactExportTest from an attached physical device."
        )
    )
    parser.add_argument("--adb", default="adb")
    parser.add_argument("--serial")
    parser.add_argument("--run-as-package", default=DEFAULT_RUN_AS_PACKAGE)
    parser.add_argument("--device-lab-root", default=DEFAULT_DEVICE_LAB_DEVICE_ROOT)
    parser.add_argument("--slot-id")
    parser.add_argument("--out-root", type=Path, default=DEFAULT_OUT_ROOT)
    parser.add_argument("--summary-out", type=Path)
    parser.add_argument("--adb-timeout-seconds", type=int, default=120)
    parser.epilog = (
        f"Latest-slot command: {ADB_LATEST_SLOT_COMMAND_HELP}\n"
        f"Raw-tar command: {ADB_PULL_TAR_COMMAND_HELP}\n"
        "The extractor rejects symlink, hardlink, special-file, traversal, "
        "duplicate, oversized, and slot-mismatched tar members."
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    status, slot_path, errors = pull_raw_slot(args)
    if status != 0:
        for error in errors:
            print(f"[kagemusha-android-raw-pull] {error}", file=sys.stderr)
        return status
    assert slot_path is not None
    print(f"[kagemusha-android-raw-pull] wrote {slot_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
