#!/usr/bin/env python3
"""Collect four peer-local Taira lifecycle windows into release evidence.

This helper is an offline, two-phase collector. ``prepare`` validates the four
owner-produced raw windows, proves each local append-only chain, globally
resequences the rows, retains the exact four raw artifacts, and writes an exact
lifecycle verification request.
``finalize`` directly invokes one immutable, digest-pinned native verifier for
that request and all five journals, captures its bounded canonical receipt, and
then publishes the lifecycle evidence consumed by
``check_taira_public_v2_24h_soak_evidence.py``.

The collector is not an observation authority, replay broker, workload runner,
or native verifier. A protected controller must supply the exact deploy handoff,
capture the four windows, pin the independently built native verifier, and keep
the output directory owner-private. No environment variables are used.
"""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
import hashlib
import json
import os
from pathlib import Path
import selectors
import stat
import subprocess
import sys
import time
from typing import NoReturn

try:
    from scripts import check_taira_public_v2_24h_soak_evidence as public_verifier
    from scripts import taira_peer_supervisor as peer_supervisor
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import check_taira_public_v2_24h_soak_evidence as public_verifier
    import taira_peer_supervisor as peer_supervisor


PREPARED_SCHEMA = "iroha.taira.public-v2-24h-lifecycle-prepared.v1"
REQUEST_SCHEMA = "iroha.taira.public-v2-24h-lifecycle-native-request.v1"
PREPARED_FILENAME = "lifecycle-prepared.json"
JOURNAL_FILENAME = "lifecycle-journal.jsonl"
REQUEST_FILENAME = "lifecycle-native-verifier-request.json"
RECEIPT_FILENAME = "lifecycle-native-verifier-receipt.json"
FINAL_FILENAME = "lifecycle-evidence.json"
RAW_FILENAMES = tuple(
    f"lifecycle-raw-{validator}.jsonl" for validator in public_verifier.VALIDATORS
)
PREPARED_PHASE_FILES = {
    PREPARED_FILENAME,
    JOURNAL_FILENAME,
    REQUEST_FILENAME,
    *RAW_FILENAMES,
}
PREPARE_PARTIAL_FILENAME = ".lifecycle-prepare.partial"
FINALIZE_PARTIAL_FILENAME = ".lifecycle-finalize.partial"
GLOBAL_CHAIN_DOMAIN = b"iroha.taira.public-v2-24h.lifecycle-global-chain.v1\0"
PREPARED_IDENTITY_DOMAIN = b"iroha.taira.public-v2-24h.lifecycle-prepared.v1\0"
MAX_RAW_WINDOW_BYTES = peer_supervisor.LIFECYCLE_JOURNAL_MAX_BYTES
MAX_DEPLOY_HANDOFF_BYTES = public_verifier.MAX_HANDOFF_BYTES
MAX_PREPARED_BYTES = 4 * 1024 * 1024
MAX_REQUEST_BYTES = 1024 * 1024
MAX_RECEIPT_BYTES = public_verifier.MAX_LIFECYCLE_JOURNAL_RECEIPT_BYTES
MAX_GLOBAL_JOURNAL_BYTES = public_verifier.MAX_LIFECYCLE_JOURNAL_BYTES
MAX_NATIVE_VERIFIER_BYTES = 256 * 1024 * 1024
NATIVE_VERIFIER_TIMEOUT_SECONDS = 120
# TODO: install this collector only behind the protected long-lived public-soak
# controller, and add the independent native verifier that answers REQUEST_SCHEMA.
STABLE_FIELDS = (
    "st_dev",
    "st_ino",
    "st_mode",
    "st_uid",
    "st_gid",
    "st_nlink",
    "st_size",
    "st_mtime_ns",
    "st_ctime_ns",
)
PREPARED_FIELDS = {
    "schema",
    "schema_version",
    "deploy_handoff_sha256",
    "deployment",
    "raw_windows",
    "baseline",
    "terminal",
    "journal_inventory",
    "lifecycle_window_sha256",
    "prepared_identity_sha256",
}
DEPLOYMENT_FIELDS = {
    "deployment_completed_at_unix_ms",
    "restart_generation",
    "config_set_sha256",
    "topology_sha256",
    "signed_genesis_sha256",
    "supervisor_sha256",
    "genesis_block_hash",
    "receipt_signers",
}
RAW_IDENTITY_FIELDS = public_verifier.LIFECYCLE_RAW_WINDOW_FIELDS
REQUEST_FIELDS = {
    "schema",
    "schema_version",
    "protocol",
    "prepared_identity_sha256",
    "journal_artifact_sha256",
    "journal_artifact_size_bytes",
    "journal_records_sha256",
    "journal_record_count",
    "lifecycle_window_sha256",
}
INVENTORY_FIELDS = public_verifier.INVENTORY_REFERENCE_FIELDS


class LifecycleCollectionError(RuntimeError):
    """The four-peer lifecycle collection is unsafe or inconsistent."""


def _fail(message: str) -> NoReturn:
    raise LifecycleCollectionError(message)


def _canonical_json(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise LifecycleCollectionError(
            f"value is not canonically encodable: {error}"
        ) from error


def _reject_constant(value: str) -> NoReturn:
    _fail(f"non-finite JSON number is forbidden: {value}")


def _pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"duplicate JSON field is forbidden: {key}")
        result[key] = value
    return result


def _decode_json(payload: bytes, label: str, *, canonical: bool = True) -> dict[str, object]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise LifecycleCollectionError(f"{label} is not strict JSON") from error
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    if canonical and _canonical_json(value) != payload:
        _fail(f"{label} is not canonical JSON")
    return value


def _exact(value: object, fields: set[str], label: str) -> Mapping[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        _fail(f"{label} fields are not exact")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        _fail(f"{label} must be an exact integer >= {minimum}")
    return value


def _sha256(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or public_verifier.SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _identity_text(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or public_verifier.IDENTITY_RE.fullmatch(value) is None
    ):
        _fail(f"{label} is not canonical")
    return value


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return tuple(getattr(info, field) for field in STABLE_FIELDS)


def _private_directory(path: Path, label: str) -> os.stat_result:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as error:
        raise LifecycleCollectionError(f"cannot inspect {label}: {path}") from error
    if (
        resolved != path
        or stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.geteuid()
        or stat.S_IMODE(info.st_mode) != 0o700
    ):
        _fail(f"{label} is not one owner-private canonical directory")
    return info


def _read_stable(path: Path, maximum_bytes: int, label: str) -> bytes:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise LifecycleCollectionError(f"cannot inspect {label}: {path}") from error
    if (
        resolved != path
        or stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(before.st_mode) & 0o022
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum_bytes
    ):
        _fail(f"{label} is not one bounded owner-controlled regular file")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            _fail(f"{label} changed while opening")
        body = bytearray()
        while len(body) <= maximum_bytes:
            chunk = os.read(descriptor, min(1024 * 1024, maximum_bytes + 1 - len(body)))
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        named = path.lstat()
    except OSError as error:
        raise LifecycleCollectionError(f"{label} vanished while reading") from error
    if (
        len(body) > maximum_bytes
        or _identity(opened) != _identity(after)
        or _identity(after) != _identity(named)
    ):
        _fail(f"{label} changed while reading")
    return bytes(body)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _publish_new(path: Path, body: bytes, label: str, maximum_bytes: int) -> None:
    if not body or len(body) > maximum_bytes:
        _fail(f"{label} has an invalid publication size")
    parent = _private_directory(path.parent, f"{label} directory")
    if os.path.lexists(path):
        _fail(f"{label} already exists")
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    descriptor = os.open(
        temporary,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    published = False
    try:
        os.fchmod(descriptor, 0o600)
        offset = 0
        while offset < len(body):
            written = os.write(descriptor, body[offset:])
            if written <= 0:
                raise OSError(f"short {label} write")
            offset += written
        os.fsync(descriptor)
        staged = os.fstat(descriptor)
        current_parent = _private_directory(path.parent, f"{label} directory")
        if (
            staged.st_uid != os.geteuid()
            or stat.S_IMODE(staged.st_mode) != 0o600
            or staged.st_nlink != 1
            or staged.st_size != len(body)
            or (current_parent.st_dev, current_parent.st_ino)
            != (parent.st_dev, parent.st_ino)
        ):
            _fail(f"{label} staging identity is unsafe")
        os.link(temporary, path, follow_symlinks=False)
        path_info = path.lstat()
        if (path_info.st_dev, path_info.st_ino) != (staged.st_dev, staged.st_ino):
            _fail(f"{label} publication inode changed")
        temporary.unlink()
        published = True
        _fsync_directory(path.parent)
    finally:
        os.close(descriptor)
        if not published:
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass
    captured = _read_stable(path, maximum_bytes, label)
    if captured != body:
        _fail(f"{label} publication bytes changed")


def _begin_transaction(output: Path, marker_name: str, allowed_names: set[str]) -> Path:
    _private_directory(output, "lifecycle output directory")
    names = {entry.name for entry in output.iterdir()}
    if names != allowed_names:
        _fail("lifecycle output directory does not have the exact prior phase")
    marker = output / marker_name
    descriptor = os.open(
        marker,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        os.fchmod(descriptor, 0o600)
        os.write(descriptor, b"incomplete\n")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _fsync_directory(output)
    return marker


def _finish_transaction(marker: Path) -> None:
    marker.unlink()
    _fsync_directory(marker.parent)


def _capture_pinned_native_verifier(
    path: Path, expected_sha256: str
) -> tuple[bytes, tuple[int, ...], int]:
    """Capture one immutable native verifier and its exact filesystem identity."""

    expected = _sha256(expected_sha256, "expected native verifier binary")
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail("native verifier path must be absolute and normalized")
    try:
        parent = path.parent.lstat()
        resolved_parent = path.parent.resolve(strict=True)
    except OSError as error:
        raise LifecycleCollectionError("cannot inspect native verifier parent") from error
    if (
        resolved_parent != path.parent
        or stat.S_ISLNK(parent.st_mode)
        or not stat.S_ISDIR(parent.st_mode)
        or parent.st_uid not in {0, os.geteuid()}
        or stat.S_IMODE(parent.st_mode) & 0o022
    ):
        _fail("native verifier parent is not owner-controlled and non-writable")
    payload = _read_stable(path, MAX_NATIVE_VERIFIER_BYTES, "native verifier executable")
    info = path.lstat()
    if (
        stat.S_IMODE(info.st_mode) != 0o555
        or info.st_mode & (stat.S_ISUID | stat.S_ISGID)
        or hashlib.sha256(payload).hexdigest() != expected
    ):
        _fail("native verifier differs from its immutable executable pin")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    opened = os.fstat(descriptor)
    if _identity(opened) != _identity(info):
        os.close(descriptor)
        _fail("native verifier changed while binding its executable descriptor")
    return payload, _identity(opened), descriptor


def _stop_child(process: subprocess.Popen[bytes]) -> None:
    try:
        process.terminate()
        process.wait(timeout=2)
    except (OSError, subprocess.TimeoutExpired):
        try:
            process.kill()
            process.wait(timeout=2)
        except (OSError, subprocess.TimeoutExpired):
            pass


def _invoke_native_verifier(
    executable: Path,
    output: Path,
    expected_binary_sha256: str,
    expected_source_sha256: str,
) -> bytes:
    """Invoke one pinned verifier with fixed inputs and bounded output."""

    source_sha256 = _sha256(
        expected_source_sha256, "expected native verifier source"
    )
    inputs = {
        PREPARED_FILENAME: _read_stable(
            output / PREPARED_FILENAME, MAX_PREPARED_BYTES,
            "prepared lifecycle document",
        ),
        JOURNAL_FILENAME: _read_stable(
            output / JOURNAL_FILENAME, MAX_GLOBAL_JOURNAL_BYTES,
            "global lifecycle journal",
        ),
        REQUEST_FILENAME: _read_stable(
            output / REQUEST_FILENAME, MAX_REQUEST_BYTES,
            "native lifecycle request",
        ),
        **{
            name: _read_stable(
                output / name,
                MAX_RAW_WINDOW_BYTES,
                f"retained raw lifecycle window {name}",
            )
            for name in RAW_FILENAMES
        },
    }
    binary_payload, binary_identity, binary_descriptor = _capture_pinned_native_verifier(
        executable, expected_binary_sha256
    )
    argv = [
        str(executable),
        "--prepared", str(output / PREPARED_FILENAME),
        "--journal", str(output / JOURNAL_FILENAME),
        "--request", str(output / REQUEST_FILENAME),
    ]
    for name in RAW_FILENAMES:
        argv.extend(("--raw-window", str(output / name)))
    argv.extend(
        (
            "--expected-verifier-binary-sha256",
            expected_binary_sha256,
            "--expected-verifier-source-sha256",
            source_sha256,
        )
    )
    if _identity(executable.lstat()) != binary_identity:
        os.close(binary_descriptor)
        _fail("native verifier changed before invocation")
    try:
        process = subprocess.Popen(
            argv,
            cwd=output,
            env={
                "HOME": str(output),
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": os.defpath,
                "TMPDIR": str(output),
            },
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            close_fds=True,
        )
    except OSError as error:
        raise LifecycleCollectionError("cannot execute pinned native verifier") from error
    finally:
        os.close(binary_descriptor)
    if _identity(executable.lstat()) != binary_identity:
        _stop_child(process)
        _fail("native verifier changed while starting")
    assert process.stdout is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + NATIVE_VERIFIER_TIMEOUT_SECONDS
    receipt = bytearray()
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                _stop_child(process)
                _fail("native verifier timed out")
            events = selector.select(remaining)
            if not events:
                _stop_child(process)
                _fail("native verifier timed out")
            for key, _mask in events:
                chunk = os.read(key.fd, 64 * 1024)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                receipt.extend(chunk)
                if len(receipt) > MAX_RECEIPT_BYTES:
                    _stop_child(process)
                    _fail("native verifier receipt exceeds its fixed bound")
        remaining = deadline - time.monotonic()
        try:
            status = process.wait(timeout=max(remaining, 0.001))
        except subprocess.TimeoutExpired:
            _stop_child(process)
            _fail("native verifier timed out")
    finally:
        selector.close()
        process.stdout.close()
    if status != 0:
        _fail(f"native verifier failed with status {status}")
    if not receipt:
        _fail("native verifier returned an empty receipt")
    current_binary = _read_stable(
        executable, MAX_NATIVE_VERIFIER_BYTES,
        "native verifier executable after invocation",
    )
    if current_binary != binary_payload or _identity(executable.lstat()) != binary_identity:
        _fail("native verifier changed during invocation")
    for name, captured in inputs.items():
        current = _read_stable(
            output / name,
            {
                PREPARED_FILENAME: MAX_PREPARED_BYTES,
                JOURNAL_FILENAME: MAX_GLOBAL_JOURNAL_BYTES,
                REQUEST_FILENAME: MAX_REQUEST_BYTES,
                **{name: MAX_RAW_WINDOW_BYTES for name in RAW_FILENAMES},
            }[name],
            f"{name} after native verification",
        )
        if current != captured:
            _fail("native verifier input changed during invocation")
    if {entry.name for entry in output.iterdir()} != set(inputs):
        _fail("native verifier changed the collector output inventory")
    return bytes(receipt)


def _checkpoint(value: object, validator_id: str, node_id: str, label: str) -> Mapping[str, object]:
    checkpoint = _exact(value, peer_supervisor.LIFECYCLE_CHECKPOINT_FIELDS, label)
    _integer(checkpoint["captured_at_unix_ms"], f"{label} capture time", minimum=1)
    _integer(checkpoint["journal_sequence"], f"{label} sequence", minimum=1)
    _sha256(checkpoint["journal_chain_sha256"], f"{label} chain")
    validators = checkpoint["validators"]
    if not isinstance(validators, list) or len(validators) != 1:
        _fail(f"{label} must contain one peer")
    row = _exact(
        validators[0], peer_supervisor.LIFECYCLE_VALIDATOR_FIELDS, f"{label} peer"
    )
    if row["validator_id"] != validator_id or row["node_id"] != node_id:
        _fail(f"{label} peer identity differs from the raw window")
    for field in ("restart_count", "unexpected_exit_total"):
        _integer(row[field], f"{label} {field}")
    for field in ("supervisor_generation", "process_generation"):
        _integer(row[field], f"{label} {field}", minimum=1)
    return checkpoint


def _local_next_chain(prior: str, record: Mapping[str, object]) -> str:
    return hashlib.sha256(
        peer_supervisor.LIFECYCLE_CHAIN_DOMAIN
        + bytes.fromhex(prior)
        + _canonical_json(record)
    ).hexdigest()


def _parse_raw_window(
    path: Path,
    expected_validator: str,
    expected_node_id: str,
    expected_binding_sha256: str,
) -> tuple[
    Mapping[str, object],
    list[dict[str, object]],
    dict[str, object],
    bytes,
]:
    payload = _read_stable(path, MAX_RAW_WINDOW_BYTES, f"raw window {expected_validator}")
    lines = payload.splitlines(keepends=True)
    if len(lines) < 3 or not payload.endswith(b"\n"):
        _fail(f"raw window {expected_validator} cannot cover both lifecycle edges")
    header = _exact(
        _decode_json(lines[0], f"raw window header {expected_validator}"),
        peer_supervisor.LIFECYCLE_RAW_WINDOW_FIELDS,
        f"raw window header {expected_validator}",
    )
    if (
        header["schema"] != peer_supervisor.LIFECYCLE_RAW_WINDOW_SCHEMA
        or type(header["schema_version"]) is not int
        or header["schema_version"] != 1
        or header["validator_id"] != expected_validator
        or header["node_id"] != expected_node_id
    ):
        _fail(f"raw window {expected_validator} identity is wrong")
    binding_sha256 = _sha256(
        header["binding_sha256"], f"raw window {expected_validator} binding"
    )
    if binding_sha256 != expected_binding_sha256:
        _fail(f"raw window {expected_validator} binding differs from deployment")
    baseline = _checkpoint(
        header["baseline"], expected_validator, expected_node_id,
        f"raw window {expected_validator} baseline",
    )
    terminal = _checkpoint(
        header["terminal"], expected_validator, expected_node_id,
        f"raw window {expected_validator} terminal",
    )
    baseline_sequence = int(baseline["journal_sequence"])
    terminal_sequence = int(terminal["journal_sequence"])
    count = _integer(
        header["record_count"], f"raw window {expected_validator} count", minimum=2
    )
    if count != terminal_sequence - baseline_sequence or len(lines) != count + 1:
        _fail(f"raw window {expected_validator} sequence interval is not exact")
    record_bytes = b"".join(lines[1:])
    records_sha256 = hashlib.sha256(
        b"iroha.taira.peer-supervisor-raw-window-records.v1\0" + record_bytes
    ).hexdigest()
    if _sha256(header["records_sha256"], "raw record-set digest") != records_sha256:
        _fail(f"raw window {expected_validator} record digest is wrong")
    records: list[dict[str, object]] = []
    chain = str(baseline["journal_chain_sha256"])
    prior_observed = int(baseline["captured_at_unix_ms"])
    baseline_row = baseline["validators"][0]  # type: ignore[index]
    terminal_row = terminal["validators"][0]  # type: ignore[index]
    for index, line in enumerate(lines[1:]):
        row = dict(
            _exact(
                _decode_json(line, f"raw row {expected_validator}:{index}"),
                peer_supervisor.LIFECYCLE_RECORD_FIELDS,
                f"raw row {expected_validator}:{index}",
            )
        )
        if (
            _integer(row["index"], "raw row index") != index
            or _integer(row["journal_sequence"], "raw row sequence")
            != baseline_sequence + index + 1
            or row["validator_id"] != expected_validator
            or row["node_id"] != expected_node_id
            or row["event"] != "healthy"
        ):
            _fail(f"raw window {expected_validator} contains a non-healthy or spliced row")
        observed = _integer(row["observed_at_unix_ms"], "raw row time", minimum=1)
        if not (
            int(baseline["captured_at_unix_ms"])
            <= observed
            <= int(terminal["captured_at_unix_ms"])
        ):
            _fail(f"raw window {expected_validator} observation escapes its window")
        if observed < prior_observed:
            _fail(f"raw window {expected_validator} observation timestamps regress")
        prior_observed = observed
        for field in (
            "restart_count",
            "supervisor_generation",
            "process_generation",
            "unexpected_exit_total",
        ):
            minimum = 1 if field.endswith("generation") else 0
            _integer(row[field], f"raw row {field}", minimum=minimum)
            if row[field] != baseline_row[field] or row[field] != terminal_row[field]:
                _fail(f"raw window {expected_validator} proves lifecycle drift")
        original = dict(row)
        original["index"] = int(row["journal_sequence"]) - 1
        chain = _local_next_chain(chain, original)
        records.append(row)
    if chain != terminal["journal_chain_sha256"]:
        _fail(f"raw window {expected_validator} does not reach its terminal chain")
    identity = {
        "artifact_sha256": hashlib.sha256(payload).hexdigest(),
        "artifact_size_bytes": len(payload),
        "baseline_sequence": baseline_sequence,
        "binding_sha256": binding_sha256,
        "node_id": expected_node_id,
        "record_count": count,
        "records_sha256": records_sha256,
        "terminal_sequence": terminal_sequence,
        "validator_id": expected_validator,
    }
    if set(identity) != RAW_IDENTITY_FIELDS:
        _fail("raw window identity fields are not exact")
    return header, records, identity, payload


def _decode_deploy_handoff(payload: bytes) -> tuple[str, dict[str, object]]:
    document = _exact(
        _decode_json(payload, "deploy handoff"),
        public_verifier.HANDOFF_DOCUMENT_FIELDS,
        "deploy handoff",
    )
    if (
        document["schema"] != public_verifier.HANDOFF_SCHEMA
        or type(document["schema_version"]) is not int
        or document["schema_version"] != 1
        or document["kind"] != "deploy"
    ):
        _fail("deploy handoff schema or kind is wrong")
    identity = _exact(
        document["identity"], public_verifier.DEPLOY_IDENTITY_FIELDS,
        "deploy handoff identity",
    )
    signers = _exact(
        identity["receipt_signers"], set(public_verifier.VALIDATORS),
        "deploy receipt signers",
    )
    signer_projection: dict[str, dict[str, str]] = {}
    seen_nodes: set[str] = set()
    for validator in public_verifier.VALIDATORS:
        signer = _exact(
            signers[validator], public_verifier.RECEIPT_SIGNER_FIELDS,
            f"deploy receipt signer {validator}",
        )
        node_id = _identity_text(signer["node_id"], f"deploy node ID {validator}")
        try:
            public_key = public_verifier._public_key(
                signer["public_key"], f"deploy receipt key {validator}"
            )
            derived_node_id = public_verifier._receipt_node_id(public_key)
        except public_verifier.EvidenceError as error:
            raise LifecycleCollectionError(str(error)) from error
        if node_id != derived_node_id:
            _fail(f"deploy node ID {validator} is not derived from its receipt key")
        if node_id in seen_nodes:
            _fail("deploy receipt signer node IDs are aliased")
        seen_nodes.add(node_id)
        binary_stat = signer["binary_stat_seal"]
        if not isinstance(binary_stat, list) or len(binary_stat) != 5:
            _fail(f"deploy receipt signer {validator} binary stat seal is not exact")
        normalized_stat = [
            _integer(value, f"deploy receipt signer {validator} stat {index}")
            for index, value in enumerate(binary_stat)
        ]
        config_sha256 = _sha256(
            signer["config_sha256"], f"deploy receipt signer {validator} config"
        )
        runtime_binding = _sha256(
            signer["runtime_binding_sha256"],
            f"deploy runtime binding {validator}",
        )
        expected_runtime_binding = public_verifier._runtime_binding_sha256(
            _sha256(identity["validator_binary_sha256"], "validator binary"),
            normalized_stat,
            config_sha256,
            _sha256(identity["restart_generation"], "restart generation"),
        )
        if runtime_binding != expected_runtime_binding:
            _fail(f"deploy runtime binding {validator} is not derived")
        lifecycle_binding = _sha256(
            signer["lifecycle_binding_sha256"],
            f"deploy lifecycle binding {validator}",
        )
        if lifecycle_binding != public_verifier._lifecycle_binding_sha256(
            runtime_binding,
            str(identity["restart_generation"]),
            validator,
            node_id,
        ):
            _fail(f"deploy lifecycle binding {validator} is not derived")
        signer_projection[validator] = {
            "lifecycle_binding_sha256": lifecycle_binding,
            "node_id": node_id,
        }
    deployment = {
        "config_set_sha256": _sha256(identity["config_set_sha256"], "config set"),
        "deployment_completed_at_unix_ms": _integer(
            identity["deployment_completed_at_unix_ms"],
            "deployment completion",
            minimum=1,
        ),
        "genesis_block_hash": identity["genesis_block_hash"],
        "receipt_signers": signer_projection,
        "restart_generation": _sha256(identity["restart_generation"], "restart generation"),
        "signed_genesis_sha256": _sha256(
            identity["signed_genesis_sha256"], "signed genesis"
        ),
        "supervisor_sha256": _sha256(identity["supervisor_sha256"], "supervisor"),
        "topology_sha256": _sha256(identity["topology_sha256"], "topology"),
    }
    if set(deployment) != DEPLOYMENT_FIELDS:
        _fail("deployment projection fields are not exact")
    return hashlib.sha256(payload).hexdigest(), deployment


def _global_chain_root(deploy_digest: str, baseline: Mapping[str, object]) -> str:
    return hashlib.sha256(
        GLOBAL_CHAIN_DOMAIN + bytes.fromhex(deploy_digest) + _canonical_json(baseline)
    ).hexdigest()


def _global_next_chain(prior: str, row: Mapping[str, object]) -> str:
    return hashlib.sha256(
        GLOBAL_CHAIN_DOMAIN + bytes.fromhex(prior) + _canonical_json(row)
    ).hexdigest()


def _lifecycle_window_digest(window: Mapping[str, object]) -> str:
    return hashlib.sha256(
        b"iroha.taira.public-v2-24h.lifecycle-window.v1\0"
        + _canonical_json(window)
    ).hexdigest()


def _prepare(
    deploy_path: Path, raw_paths: Sequence[Path], output: Path
) -> dict[str, object]:
    if len(raw_paths) != len(public_verifier.VALIDATORS):
        _fail("prepare requires exactly four raw lifecycle windows")
    if len(set(raw_paths)) != len(raw_paths) or deploy_path in set(raw_paths):
        _fail("lifecycle input paths are aliased")
    deploy_payload = _read_stable(
        deploy_path, MAX_DEPLOY_HANDOFF_BYTES, "deploy handoff"
    )
    deploy_digest, deployment = _decode_deploy_handoff(deploy_payload)
    parsed: list[
        tuple[
            Mapping[str, object],
            list[dict[str, object]],
            dict[str, object],
            bytes,
        ]
    ] = []
    for validator, path in zip(public_verifier.VALIDATORS, raw_paths, strict=True):
        expected_signer = deployment["receipt_signers"][validator]  # type: ignore[index]
        parsed.append(
            _parse_raw_window(
                path,
                validator,
                expected_signer["node_id"],
                expected_signer["lifecycle_binding_sha256"],
            )
        )
    baseline_validators = [
        item[0]["baseline"]["validators"][0] for item in parsed  # type: ignore[index]
    ]
    terminal_validators = [
        item[0]["terminal"]["validators"][0] for item in parsed  # type: ignore[index]
    ]
    baseline_time = max(
        int(item[0]["baseline"]["captured_at_unix_ms"])  # type: ignore[index]
        for item in parsed
    )
    terminal_time = min(
        int(item[0]["terminal"]["captured_at_unix_ms"])  # type: ignore[index]
        for item in parsed
    )
    if baseline_time < int(deployment["deployment_completed_at_unix_ms"]):
        _fail("lifecycle baseline predates deployment completion")
    if terminal_time <= baseline_time:
        _fail("four peer-local lifecycle windows do not overlap")
    baseline = {
        "captured_at_unix_ms": baseline_time,
        "journal_sequence": 0,
        "journal_chain_sha256": "",
        "validators": baseline_validators,
    }
    chain = _global_chain_root(deploy_digest, baseline)
    baseline["journal_chain_sha256"] = chain
    sortable: list[tuple[int, int, int, dict[str, object]]] = []
    for validator_index, (
        _header,
        records,
        _identity_value,
        _payload,
    ) in enumerate(parsed):
        common_records = [
            row
            for row in records
            if baseline_time <= int(row["observed_at_unix_ms"]) <= terminal_time
        ]
        if len(common_records) < 2:
            _fail(
                "common lifecycle interval does not cover every validator twice"
            )
        for row in common_records:
            sortable.append(
                (
                    int(row["observed_at_unix_ms"]),
                    validator_index,
                    int(row["journal_sequence"]),
                    row,
                )
            )
    sortable.sort(key=lambda item: item[:3])
    rows: list[dict[str, object]] = []
    for index, (_observed, _validator_index, _local_sequence, original) in enumerate(sortable):
        row = dict(original)
        row["index"] = index
        row["journal_sequence"] = index + 1
        chain = _global_next_chain(chain, row)
        rows.append(row)
    if len(rows) < len(public_verifier.VALIDATORS) * 2:
        _fail("global lifecycle journal does not cover every validator twice")
    terminal = {
        "captured_at_unix_ms": terminal_time,
        "journal_sequence": len(rows),
        "journal_chain_sha256": chain,
        "validators": terminal_validators,
    }
    journal_header = {
        "record_count": len(rows),
        "schema": public_verifier.LIFECYCLE_JOURNAL_SCHEMA,
        "schema_version": 1,
    }
    journal_record_bytes = b"".join(_canonical_json(row) for row in rows)
    journal_body = _canonical_json(journal_header) + journal_record_bytes
    journal_sha256 = hashlib.sha256(journal_body).hexdigest()
    journal_records_sha256 = hashlib.sha256(
        b"iroha.taira.public-v2-24h.lifecycle-journal-records.v1\0"
        + journal_record_bytes
    ).hexdigest()
    journal_reference = {
        "kind": "lifecycle-journal",
        "record_count": len(rows),
        "records_sha256": journal_records_sha256,
        "schema": public_verifier.LIFECYCLE_JOURNAL_SCHEMA,
        "sha256": journal_sha256,
        "size_bytes": len(journal_body),
    }
    raw_identities = [item[2] for item in parsed]
    window = {
        "baseline": baseline,
        "config_set_sha256": deployment["config_set_sha256"],
        "deployment_completed_at_unix_ms": deployment[
            "deployment_completed_at_unix_ms"
        ],
        "genesis_block_hash": deployment["genesis_block_hash"],
        "journal_inventory": journal_reference,
        "raw_windows": raw_identities,
        "restart_events": 0,
        "restart_generation": deployment["restart_generation"],
        "schema": public_verifier.LIFECYCLE_SCHEMA,
        "schema_version": 1,
        "signed_genesis_sha256": deployment["signed_genesis_sha256"],
        "supervisor_sha256": deployment["supervisor_sha256"],
        "terminal": terminal,
        "topology_sha256": deployment["topology_sha256"],
        "unexpected_exit_events": 0,
    }
    if set(window) != public_verifier.LIFECYCLE_FIELDS - {
        "native_journal_verifier_receipt"
    }:
        _fail("prepared lifecycle window fields are not exact")
    window_sha256 = _lifecycle_window_digest(window)
    prepared_without_identity = {
        "baseline": baseline,
        "deploy_handoff_sha256": deploy_digest,
        "deployment": deployment,
        "journal_inventory": journal_reference,
        "lifecycle_window_sha256": window_sha256,
        "raw_windows": raw_identities,
        "schema": PREPARED_SCHEMA,
        "schema_version": 1,
        "terminal": terminal,
    }
    prepared_identity = hashlib.sha256(
        PREPARED_IDENTITY_DOMAIN + _canonical_json(prepared_without_identity)
    ).hexdigest()
    prepared = dict(prepared_without_identity)
    prepared["prepared_identity_sha256"] = prepared_identity
    if set(prepared) != PREPARED_FIELDS:
        _fail("prepared lifecycle document fields are not exact")
    request = {
        "journal_artifact_sha256": journal_sha256,
        "journal_artifact_size_bytes": len(journal_body),
        "journal_record_count": len(rows),
        "journal_records_sha256": journal_records_sha256,
        "lifecycle_window_sha256": window_sha256,
        "prepared_identity_sha256": prepared_identity,
        "protocol": public_verifier.NATIVE_JOURNAL_VERIFIER_PROTOCOL,
        "schema": REQUEST_SCHEMA,
        "schema_version": 1,
    }
    if set(request) != REQUEST_FIELDS:
        _fail("native lifecycle request fields are not exact")
    marker = _begin_transaction(output, PREPARE_PARTIAL_FILENAME, set())
    try:
        for name, item in zip(RAW_FILENAMES, parsed, strict=True):
            _publish_new(
                output / name,
                item[3],
                f"retained raw lifecycle window {name}",
                MAX_RAW_WINDOW_BYTES,
            )
        _publish_new(
            output / JOURNAL_FILENAME,
            journal_body,
            "global lifecycle journal",
            MAX_GLOBAL_JOURNAL_BYTES,
        )
        _publish_new(
            output / PREPARED_FILENAME,
            _canonical_json(prepared),
            "prepared lifecycle document",
            MAX_PREPARED_BYTES,
        )
        _publish_new(
            output / REQUEST_FILENAME,
            _canonical_json(request),
            "lifecycle native-verifier request",
            MAX_REQUEST_BYTES,
        )
    except BaseException:
        raise
    else:
        _finish_transaction(marker)
    return request


def _validate_prepared(
    prepared: Mapping[str, object], journal_payload: bytes
) -> tuple[dict[str, object], str]:
    _exact(prepared, PREPARED_FIELDS, "prepared lifecycle document")
    if (
        prepared["schema"] != PREPARED_SCHEMA
        or type(prepared["schema_version"]) is not int
        or prepared["schema_version"] != 1
    ):
        _fail("prepared lifecycle document schema is wrong")
    without_identity = {
        field: prepared[field]
        for field in PREPARED_FIELDS - {"prepared_identity_sha256"}
    }
    expected_identity = hashlib.sha256(
        PREPARED_IDENTITY_DOMAIN + _canonical_json(without_identity)
    ).hexdigest()
    if _sha256(prepared["prepared_identity_sha256"], "prepared identity") != expected_identity:
        _fail("prepared lifecycle identity digest is wrong")
    inventory = _exact(
        prepared["journal_inventory"], INVENTORY_FIELDS, "journal inventory"
    )
    if (
        inventory["kind"] != "lifecycle-journal"
        or inventory["schema"] != public_verifier.LIFECYCLE_JOURNAL_SCHEMA
        or _sha256(inventory["sha256"], "journal digest")
        != hashlib.sha256(journal_payload).hexdigest()
        or _integer(inventory["size_bytes"], "journal size", minimum=1)
        != len(journal_payload)
    ):
        _fail("prepared journal reference differs from its artifact")
    window = {
        "baseline": prepared["baseline"],
        "config_set_sha256": prepared["deployment"]["config_set_sha256"],  # type: ignore[index]
        "deployment_completed_at_unix_ms": prepared["deployment"][  # type: ignore[index]
            "deployment_completed_at_unix_ms"
        ],
        "genesis_block_hash": prepared["deployment"]["genesis_block_hash"],  # type: ignore[index]
        "journal_inventory": prepared["journal_inventory"],
        "raw_windows": prepared["raw_windows"],
        "restart_events": 0,
        "restart_generation": prepared["deployment"]["restart_generation"],  # type: ignore[index]
        "schema": public_verifier.LIFECYCLE_SCHEMA,
        "schema_version": 1,
        "signed_genesis_sha256": prepared["deployment"][  # type: ignore[index]
            "signed_genesis_sha256"
        ],
        "supervisor_sha256": prepared["deployment"]["supervisor_sha256"],  # type: ignore[index]
        "terminal": prepared["terminal"],
        "topology_sha256": prepared["deployment"]["topology_sha256"],  # type: ignore[index]
        "unexpected_exit_events": 0,
    }
    window_sha256 = _lifecycle_window_digest(window)
    if _sha256(prepared["lifecycle_window_sha256"], "lifecycle window") != window_sha256:
        _fail("prepared lifecycle window digest is wrong")
    return window, expected_identity


def _validate_retained_raw_windows(
    prepared: Mapping[str, object], output: Path
) -> None:
    """Re-parse and bind all retained peer-local journals before invocation."""

    deployment = _exact(
        prepared["deployment"], DEPLOYMENT_FIELDS, "prepared deployment projection"
    )
    signers = _exact(
        deployment["receipt_signers"],
        set(public_verifier.VALIDATORS),
        "prepared deploy receipt signers",
    )
    raw_windows = prepared["raw_windows"]
    if not isinstance(raw_windows, list) or len(raw_windows) != len(RAW_FILENAMES):
        _fail("prepared raw lifecycle window inventory is not exact")
    for validator, name, expected_value in zip(
        public_verifier.VALIDATORS,
        RAW_FILENAMES,
        raw_windows,
        strict=True,
    ):
        signer = _exact(
            signers[validator],
            {"lifecycle_binding_sha256", "node_id"},
            f"prepared deploy receipt signer {validator}",
        )
        expected = _exact(
            expected_value,
            RAW_IDENTITY_FIELDS,
            f"prepared raw lifecycle identity {validator}",
        )
        _header, _records, actual, _payload = _parse_raw_window(
            output / name,
            validator,
            str(signer["node_id"]),
            str(signer["lifecycle_binding_sha256"]),
        )
        if actual != expected:
            _fail(f"retained raw lifecycle window {validator} changed after prepare")


def _finalize(
    output: Path,
    native_verifier_path: Path,
    expected_binary_sha256: str,
    expected_source_sha256: str,
) -> dict[str, object]:
    allowed = set(PREPARED_PHASE_FILES)
    _private_directory(output, "lifecycle output directory")
    if {entry.name for entry in output.iterdir()} != allowed:
        _fail("lifecycle output directory does not have the exact prepared phase")
    prepared_payload = _read_stable(
        output / PREPARED_FILENAME, MAX_PREPARED_BYTES, "prepared lifecycle document"
    )
    journal_payload = _read_stable(
        output / JOURNAL_FILENAME, MAX_GLOBAL_JOURNAL_BYTES, "global lifecycle journal"
    )
    request_payload = _read_stable(
        output / REQUEST_FILENAME, MAX_REQUEST_BYTES, "native lifecycle request"
    )
    prepared = _exact(
        _decode_json(prepared_payload, "prepared lifecycle document"),
        PREPARED_FIELDS,
        "prepared lifecycle document",
    )
    window, prepared_identity = _validate_prepared(prepared, journal_payload)
    _validate_retained_raw_windows(prepared, output)
    request = _exact(
        _decode_json(request_payload, "native lifecycle request"),
        REQUEST_FIELDS,
        "native lifecycle request",
    )
    if (
        request["schema"] != REQUEST_SCHEMA
        or type(request["schema_version"]) is not int
        or request["schema_version"] != 1
        or request["protocol"] != public_verifier.NATIVE_JOURNAL_VERIFIER_PROTOCOL
        or request["prepared_identity_sha256"] != prepared_identity
        or request["journal_artifact_sha256"]
        != hashlib.sha256(journal_payload).hexdigest()
        or request["journal_artifact_size_bytes"] != len(journal_payload)
        or request["lifecycle_window_sha256"]
        != prepared["lifecycle_window_sha256"]
    ):
        _fail("native lifecycle request differs from prepared evidence")
    receipt_payload = _invoke_native_verifier(
        native_verifier_path,
        output,
        expected_binary_sha256,
        expected_source_sha256,
    )
    receipt = _exact(
        _decode_json(receipt_payload, "native lifecycle receipt"),
        public_verifier.LIFECYCLE_JOURNAL_RECEIPT_FIELDS,
        "native lifecycle receipt",
    )
    if (
        receipt["schema"] != public_verifier.LIFECYCLE_JOURNAL_RECEIPT_SCHEMA
        or type(receipt["schema_version"]) is not int
        or receipt["schema_version"] != 1
        or receipt["protocol"] != public_verifier.NATIVE_JOURNAL_VERIFIER_PROTOCOL
        or _sha256(receipt["verifier_binary_sha256"], "native verifier binary")
        != _sha256(expected_binary_sha256, "expected native verifier binary")
        or _sha256(receipt["verifier_source_sha256"], "native verifier source")
        != _sha256(expected_source_sha256, "expected native verifier source")
        or receipt["journal_artifact_sha256"] != request["journal_artifact_sha256"]
        or receipt["journal_artifact_size_bytes"]
        != request["journal_artifact_size_bytes"]
        or receipt["journal_records_sha256"] != request["journal_records_sha256"]
        or receipt["journal_record_count"] != request["journal_record_count"]
        or receipt["lifecycle_window_sha256"] != request["lifecycle_window_sha256"]
        or receipt["verification_result"] != "verified"
    ):
        _fail("native lifecycle receipt does not exactly satisfy the request")
    evidence = dict(window)
    evidence["native_journal_verifier_receipt"] = {
        "sha256": hashlib.sha256(receipt_payload).hexdigest(),
        "size_bytes": len(receipt_payload),
    }
    if set(evidence) != public_verifier.LIFECYCLE_FIELDS:
        _fail("final lifecycle evidence fields are not exact")
    marker = _begin_transaction(output, FINALIZE_PARTIAL_FILENAME, allowed)
    try:
        _publish_new(
            output / RECEIPT_FILENAME,
            receipt_payload,
            "native lifecycle receipt",
            MAX_RECEIPT_BYTES,
        )
        _publish_new(
            output / FINAL_FILENAME,
            _canonical_json(evidence),
            "final lifecycle evidence",
            public_verifier.MAX_LIFECYCLE_BYTES,
        )
    except BaseException:
        raise
    else:
        _finish_transaction(marker)
    return evidence


def build_parser() -> argparse.ArgumentParser:
    """Build the exact two-phase collector command line."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    subparsers = parser.add_subparsers(dest="command", required=True)
    prepare = subparsers.add_parser("prepare", allow_abbrev=False)
    prepare.add_argument("--deploy-handoff", type=Path, required=True)
    prepare.add_argument(
        "--raw-window",
        type=Path,
        action="append",
        required=True,
        help="Repeat exactly four times in taira-validator-1..4 order.",
    )
    prepare.add_argument("--output-directory", type=Path, required=True)
    finalize = subparsers.add_parser("finalize", allow_abbrev=False)
    finalize.add_argument("--output-directory", type=Path, required=True)
    finalize.add_argument("--native-verifier", type=Path, required=True)
    finalize.add_argument("--native-verifier-binary-sha256", required=True)
    finalize.add_argument("--native-verifier-source-sha256", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Run one offline lifecycle collection phase."""

    args = build_parser().parse_args(argv)
    try:
        if args.command == "prepare":
            request = _prepare(
                args.deploy_handoff,
                args.raw_window,
                args.output_directory,
            )
            print(
                "Taira public lifecycle request prepared: "
                f"sha256={hashlib.sha256(_canonical_json(request)).hexdigest()}"
            )
        else:
            evidence = _finalize(
                args.output_directory,
                args.native_verifier,
                args.native_verifier_binary_sha256,
                args.native_verifier_source_sha256,
            )
            print(
                "Taira public lifecycle evidence finalized: "
                f"sha256={hashlib.sha256(_canonical_json(evidence)).hexdigest()}"
            )
    except (LifecycleCollectionError, OSError, ValueError) as error:
        print(f"Taira public lifecycle collection failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
