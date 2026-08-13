#!/usr/bin/env python3
"""Validate payload-free promotion receipt-verifier output."""

from __future__ import annotations

import json
import os
import re
import selectors
import signal
import stat
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any


PROMOTION_SIGNER_ROLE = "promotion"
PROMOTION_SIGNER_DOMAIN = (
    "sorafs.production-readiness.foundational-prerequisites.v1"
)
RECEIPT_VALIDATION_SCHEMA = (
    "sorafs.external_software_signer.signature_receipt_validation.v1"
)
RECEIPT_VALIDATION_FIELDS = frozenset(
    {
        "schema",
        "status",
        "operation_id_hex",
        "payload_digest_blake3_hex",
        "payload_length",
        "signature_digest_blake3_hex",
        "binding_digest_blake3_hex",
        "backend",
        "service_id",
        "administrator_id",
        "role",
        "domain",
        "signature_algorithm",
        "key_revision",
        "policy_revision",
        "policy_digest_sha256",
        "public_key_digest_blake3_hex",
        "commit_sequence",
        "commit_audit_head_blake3_hex",
        "audit_sequence",
        "audit_head_blake3_hex",
        "replayed",
        "revoked",
        "payload_signature_valid",
        "provenance_attestation_valid",
        "response_attestation_valid",
    }
)
LOWER_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
MAX_RECEIPT_VALIDATION_BYTES = 16 * 1024
MAX_RECEIPT_VERIFIER_DIAGNOSTIC_BYTES = 16 * 1024
RECEIPT_VERIFIER_TIMEOUT_SECS = 30
RECEIPT_VERIFIER_CLEANUP_TIMEOUT_SECS = 1


def _write_private_verifier_input(path: Path, payload: bytes, mode: int) -> None:
    """Write one exact verifier input below a fresh private directory."""

    descriptor = os.open(
        path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_BINARY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        mode,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError("short verifier input write")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _read_private_verifier_output(path: Path) -> bytes:
    """Read one bounded regular single-link output without following a link."""

    descriptor = os.open(
        path,
        os.O_RDONLY
        | getattr(os, "O_BINARY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0),
    )
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_size > MAX_RECEIPT_VALIDATION_BYTES
            or stat.S_IMODE(before.st_mode) & 0o077
        ):
            raise OSError("unsafe verifier output")
        chunks: list[bytes] = []
        size = 0
        while True:
            chunk = os.read(
                descriptor,
                min(4096, MAX_RECEIPT_VALIDATION_BYTES + 1 - size),
            )
            if not chunk:
                break
            chunks.append(chunk)
            size += len(chunk)
            if size > MAX_RECEIPT_VALIDATION_BYTES:
                raise OSError("oversized verifier output")
        after = os.fstat(descriptor)
        if after.st_nlink != 1 or any(
            getattr(before, field) != getattr(after, field)
            for field in ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
        ):
            raise OSError("unstable verifier output")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _verifier_process_group_exists(process: subprocess.Popen[bytes]) -> bool:
    """Return whether the verifier's isolated POSIX process group is live."""

    if os.name != "posix":
        return process.poll() is None
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except OSError:
        return True
    return True


def _kill_and_reap_verifier(process: subprocess.Popen[bytes]) -> bool:
    """Kill the isolated verifier group and reap its direct child, boundedly."""

    deadline = time.monotonic() + RECEIPT_VERIFIER_CLEANUP_TIMEOUT_SECS

    def kill_group() -> bool:
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            elif process.poll() is None:  # pragma: no cover - POSIX release host
                process.kill()
        except ProcessLookupError:
            return True
        except OSError:
            return False
        return True

    try:
        signal_ok = kill_group()
        while process.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return False
            try:
                process.wait(timeout=min(remaining, 0.05))
            except subprocess.TimeoutExpired:
                signal_ok = kill_group() and signal_ok

        if os.name != "posix":  # pragma: no cover - POSIX release host
            return signal_ok
        while _verifier_process_group_exists(process):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return False
            signal_ok = kill_group() and signal_ok
            time.sleep(min(0.01, remaining))
        return signal_ok
    except (OSError, ValueError, subprocess.SubprocessError):
        return False


def _close_verifier_pipe(
    selector: selectors.BaseSelector,
    pipe: Any,
) -> bool:
    """Unregister and close one verifier diagnostic pipe."""

    closed = True
    try:
        selector.unregister(pipe)
    except KeyError:
        pass
    except (OSError, ValueError):
        closed = False
    try:
        pipe.close()
    except OSError:
        closed = False
    return closed


def _run_bounded_verifier(command: list[str], root: Path) -> str | None:
    """Run one verifier with bounded diagnostics, time, and descendants."""

    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    failure: str | None = None
    cleanup_required = True
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=root,
            env={"LANG": "C", "LC_ALL": "C", "PATH": os.defpath},
            bufsize=0,
            start_new_session=os.name == "posix",
            umask=0o077 if os.name == "posix" else -1,
        )
        assert process.stdout is not None
        assert process.stderr is not None
        for pipe in (process.stdout, process.stderr):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ)

        deadline = time.monotonic() + RECEIPT_VERIFIER_TIMEOUT_SECS
        diagnostic_bytes = 0
        while selector.get_map() and failure is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                failure = "external software signer receipt verifier could not run"
                break
            for key, _events in selector.select(min(remaining, 0.05)):
                allowance = (
                    MAX_RECEIPT_VERIFIER_DIAGNOSTIC_BYTES - diagnostic_bytes
                )
                if allowance <= 0:
                    failure = "external software signer receipt verification failed"
                    break
                try:
                    chunk = os.read(key.fd, min(4096, allowance))
                except BlockingIOError:
                    continue
                if not chunk:
                    if not _close_verifier_pipe(selector, key.fileobj):
                        failure = (
                            "external software signer receipt verifier could not run"
                        )
                        break
                    continue
                diagnostic_bytes += len(chunk)
                # The verifier contract forbids all diagnostics. Read no more
                # than the shared cap and never retain or report
                # verifier-controlled content after this invocation.
                failure = "external software signer receipt verification failed"
                break

        if failure is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                failure = "external software signer receipt verifier could not run"
            else:
                try:
                    returncode = process.wait(timeout=remaining)
                except subprocess.TimeoutExpired:
                    failure = "external software signer receipt verifier could not run"
                else:
                    if returncode != 0:
                        failure = (
                            "external software signer receipt verification failed"
                        )
                    elif _verifier_process_group_exists(process):
                        # A successful verifier must not leave descendants
                        # behind, even when they closed the inherited pipes.
                        failure = (
                            "external software signer receipt verifier could not run"
                        )
                    else:
                        cleanup_required = False
    except (OSError, ValueError, subprocess.SubprocessError):
        failure = "external software signer receipt verifier could not run"
    finally:
        cleanup_ok = True
        if process is not None and cleanup_required:
            cleanup_ok = _kill_and_reap_verifier(process)
        for pipe in (
            None if process is None else process.stdout,
            None if process is None else process.stderr,
        ):
            if pipe is not None and not pipe.closed:
                cleanup_ok = _close_verifier_pipe(selector, pipe) and cleanup_ok
        try:
            selector.close()
        except OSError:
            cleanup_ok = False
        if not cleanup_ok:
            failure = "external software signer receipt verifier could not run"
    return failure


def run_offline_receipt_verifier(
    *,
    verifier: bytes,
    binding: bytes,
    payload: bytes,
    signature: bytes,
    receipt: bytes,
    operation_id_hex: str,
) -> tuple[bytes | None, list[str]]:
    """Replay one pinned offline verifier over private exact-byte copies."""

    errors: list[str] = []
    try:
        with tempfile.TemporaryDirectory(
            prefix="sorafs-promotion-receipt-"
        ) as temporary_directory:
            root = Path(temporary_directory)
            verifier_path = root / "receipt-verifier"
            binding_path = root / "promotion.binding.norito"
            payload_path = root / "foundational.signing-payload.bin"
            signature_path = root / "signature.raw"
            receipt_path = root / "signature-receipt.json"
            validation_path = root / "receipt-validation.json"
            for path, content, mode in (
                (verifier_path, verifier, 0o500),
                (binding_path, binding, 0o400),
                (payload_path, payload, 0o400),
                (signature_path, signature, 0o400),
                (receipt_path, receipt, 0o400),
            ):
                _write_private_verifier_input(path, content, mode)
            failure = _run_bounded_verifier(
                [
                    str(verifier_path),
                    "verify-receipt",
                    "--binding",
                    str(binding_path),
                    "--payload",
                    str(payload_path),
                    "--signature",
                    str(signature_path),
                    "--receipt",
                    str(receipt_path),
                    "--expected-operation-id",
                    operation_id_hex,
                    "--validation-out",
                    str(validation_path),
                ],
                root,
            )
            if failure is not None:
                errors.append(failure)
                return None, errors
            return _read_private_verifier_output(validation_path), errors
    except (OSError, subprocess.SubprocessError):
        errors.append("external software signer receipt verifier could not run")
        return None, errors


def canonical_json_bytes(value: Any) -> bytes:
    """Return the exact compact JSON representation required from the verifier."""

    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")


def parse_canonical_validation(
    raw: bytes,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Decode a schema-closed, duplicate-free canonical validation artifact."""

    errors: list[str] = []

    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise ValueError("duplicate member")
            value[key] = item
        return value

    try:
        text = raw.decode("ascii")
        value = json.loads(
            text,
            object_pairs_hook=reject_duplicates,
            parse_constant=lambda _value: (_ for _ in ()).throw(
                ValueError("non-finite number")
            ),
        )
    except (RecursionError, UnicodeDecodeError, ValueError, json.JSONDecodeError):
        return None, ["software signer receipt validation must be strict ASCII JSON"]
    if not isinstance(value, dict):
        return None, ["software signer receipt validation must be a JSON object"]
    try:
        canonical = canonical_json_bytes(value)
    except (TypeError, ValueError):
        return None, ["software signer receipt validation contains invalid JSON values"]
    if raw != canonical:
        errors.append("software signer receipt validation must use canonical JSON")
    if set(value) != RECEIPT_VALIDATION_FIELDS:
        errors.append("software signer receipt validation fields do not match the contract")
    return value, errors


def _canonical_nonzero_digest(value: Any) -> bool:
    return (
        isinstance(value, str)
        and LOWER_DIGEST_RE.fullmatch(value) is not None
        and any(bytes.fromhex(value))
    )


def _positive_integer(value: Any) -> bool:
    return (
        isinstance(value, int)
        and not isinstance(value, bool)
        and 0 < value <= (1 << 63) - 1
    )


def validate_receipt_validation(
    value: dict[str, Any],
    *,
    operation_id_hex: str,
    payload_length: int,
    service_id: str,
    administrator_id: str,
    key_revision: int,
    policy_revision: int,
    policy_digest_sha256: str,
) -> list[str]:
    """Require the verifier result to match every reviewed promotion binding."""

    errors: list[str] = []
    exact = {
        "schema": RECEIPT_VALIDATION_SCHEMA,
        "status": "valid",
        "operation_id_hex": operation_id_hex,
        "payload_length": payload_length,
        "backend": "software",
        "service_id": service_id,
        "administrator_id": administrator_id,
        "role": PROMOTION_SIGNER_ROLE,
        "domain": PROMOTION_SIGNER_DOMAIN,
        "signature_algorithm": "ed25519",
        "key_revision": key_revision,
        "policy_revision": policy_revision,
        "policy_digest_sha256": policy_digest_sha256,
        "revoked": False,
        "payload_signature_valid": True,
        "provenance_attestation_valid": True,
        "response_attestation_valid": True,
    }
    for field, expected in exact.items():
        if value.get(field) != expected:
            errors.append(
                f"software signer receipt validation {field} does not match the reviewed promotion binding"
            )
    for field in (
        "payload_digest_blake3_hex",
        "signature_digest_blake3_hex",
        "binding_digest_blake3_hex",
        "public_key_digest_blake3_hex",
        "commit_audit_head_blake3_hex",
        "audit_head_blake3_hex",
    ):
        if not _canonical_nonzero_digest(value.get(field)):
            errors.append(
                f"software signer receipt validation {field} must be a non-zero lowercase digest"
            )
    for field in ("commit_sequence", "audit_sequence"):
        if not _positive_integer(value.get(field)):
            errors.append(
                f"software signer receipt validation {field} must be a positive bounded integer"
            )
    if not isinstance(value.get("replayed"), bool):
        errors.append("software signer receipt validation replayed must be boolean")
    if value.get("commit_sequence") != value.get("audit_sequence"):
        errors.append(
            "software signer receipt validation commit and audit sequences must match"
        )
    if value.get("commit_audit_head_blake3_hex") != value.get(
        "audit_head_blake3_hex"
    ):
        errors.append(
            "software signer receipt validation commit and audit heads must match"
        )
    return errors


__all__ = [
    "PROMOTION_SIGNER_DOMAIN",
    "PROMOTION_SIGNER_ROLE",
    "RECEIPT_VALIDATION_FIELDS",
    "RECEIPT_VALIDATION_SCHEMA",
    "canonical_json_bytes",
    "parse_canonical_validation",
    "run_offline_receipt_verifier",
    "validate_receipt_validation",
]
