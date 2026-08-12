#!/usr/bin/env python3
"""Prepare, finalize, and verify a signed SoraFS topology envelope.

This tool never accepts a private key.  ``prepare`` emits the exact
domain-separated bytes for an independently administered external software
Ed25519 signer.  ``finalize`` replays the reviewed topology and signer tuple
before accepting the detached raw signature.  ``verify`` independently
replays the same trust tuple and emits only the authenticated public binding.
All output files are created exclusively, without following links.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import secrets
import stat
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sccp_release_common import (  # noqa: E402
    SccpReleaseError,
    canonical_json_file_bytes,
    parse_json_bytes,
    require_canonical_json_file,
    verify_ed25519,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
)
from sorafs_evidence_json import read_evidence_bytes  # noqa: E402
from sorafs_l1_lane_evidence_inventory import InventoryError  # noqa: E402
from sorafs_evidence_sensitivity import (  # noqa: E402
    COMMON_SENSITIVE_KEY_NORMALIZED,
    HIGH_RISK_SENSITIVE_KEY_FRAGMENTS,
    normalize_sensitive_key,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
from sorafs_software_signer_evidence import (  # noqa: E402
    parse_foundational_signer_public_key,
    validate_foundational_software_signer,
)
from sorafs_topology_qualification import (  # noqa: E402
    AUTHENTICATED_TOPOLOGY_BINDING_FIELDS,
    DEFAULT_MAX_QUALIFICATION_REVIEW_AGE_SECS,
    MAX_BOUNDED_INTEGER,
    MAX_QUALIFICATION_ENVELOPE_BYTES,
    SIGNED_QUALIFICATION_ENVELOPE_FIELDS,
    SIGNED_QUALIFICATION_ENVELOPE_SCHEMA,
    TOPOLOGY_BINDING_FIELDS,
    TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN,
    add_topology_qualification_argument,
    load_signed_topology_qualification_binding,
    load_topology_qualification_binding,
    topology_qualification_envelope_signing_bytes,
)


PREPARED_ENVELOPE_FIELDS = SIGNED_QUALIFICATION_ENVELOPE_FIELDS - {
    "signature_hex"
}
RAW_ED25519_SIGNATURE_BYTES = 64
SECRET_ARGUMENT_PREFIXES = (
    "--private",
    "--seed",
    "--signing-key",
    "--secret",
)
SECRET_ARGUMENT_NAMES = frozenset(
    {"mnemonic", "password", "privatekey", "seed", "signingkey", "token"}
)
SINGLETON_OPTIONS = frozenset(
    {
        "--deployment-id",
        "--environment",
        "--envelope-out",
        "--max-topology-qualification-review-age-secs",
        "--now-unix",
        "--prepared",
        "--prepared-out",
        "--reviewed-at-unix",
        "--signature-file",
        "--signing-payload-out",
        "--topology-qualification-envelope",
        "--topology-qualification-signer-administrator-id",
        "--topology-qualification-signer-key-revision",
        "--topology-qualification-signer-policy-digest-hex",
        "--topology-qualification-signer-policy-revision",
        "--topology-qualification-signer-service-id",
        "--topology-qualification-summary",
        "--topology-qualification-verification-public-key-hex",
        "--verification-out",
    }
)
OPEN_DIR_FD_SUPPORTED = os.open in os.supports_dir_fd
STAT_DIR_FD_SUPPORTED = os.stat in os.supports_dir_fd
STAT_NOFOLLOW_SUPPORTED = os.stat in os.supports_follow_symlinks
UNLINK_DIR_FD_SUPPORTED = os.unlink in os.supports_dir_fd
LINK_DIR_FD_SUPPORTED = os.link in os.supports_dir_fd
LINK_NOFOLLOW_SUPPORTED = os.link in os.supports_follow_symlinks

assert len(PREPARED_ENVELOPE_FIELDS) == 20
assert len(SIGNED_QUALIFICATION_ENVELOPE_FIELDS) == 21


class TopologyEnvelopeError(ValueError):
    """A public-safe topology-envelope validation failure."""


class TopologyEnvelopePreflightError(ValueError):
    """An unsafe CLI or output-path request."""


class _SanitizedArgumentParser(EvidenceArgumentParser):
    """Reject malformed arguments without rendering attacker-controlled text."""

    def error(self, _message: str) -> None:
        raise TopologyEnvelopePreflightError(
            "invalid topology envelope arguments"
        )


@dataclass
class _StagedOutput:
    path: Path
    parent_fd: int
    parent_identity: tuple[int, int]
    leaf: str
    temporary_leaf: str
    descriptor: int
    identity: tuple[int, int]
    expected_size: int
    expected_sha256: str
    label: str
    temporary_present: bool = True
    published: bool = False


def prepared_topology_qualification_signing_bytes(
    prepared: Mapping[str, Any],
) -> bytes:
    """Return final-envelope signing bytes for one exact prepared object."""

    if not isinstance(prepared, Mapping) or set(prepared) != PREPARED_ENVELOPE_FIELDS:
        raise TopologyEnvelopeError(
            "prepared topology envelope fields must match the schema-closed contract"
        )
    if prepared.get("schema") != SIGNED_QUALIFICATION_ENVELOPE_SCHEMA:
        raise TopologyEnvelopeError(
            "prepared topology envelope schema must match the signed contract"
        )
    candidate = dict(prepared)
    candidate["signature_hex"] = "00" * RAW_ED25519_SIGNATURE_BYTES
    payload = topology_qualification_envelope_signing_bytes(candidate)
    if not payload.startswith(TOPOLOGY_QUALIFICATION_SIGNATURE_DOMAIN):
        raise TopologyEnvelopeError(
            "prepared topology envelope signing domain must match the contract"
        )
    return payload


def _bounded_clock(value: Any, *, label: str, allow_zero: bool = False) -> int:
    lower = 0 if allow_zero else 1
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < lower
        or value > MAX_BOUNDED_INTEGER
    ):
        interval = "0..2^63-1" if allow_zero else "1..2^63-1"
        raise TopologyEnvelopeError(f"{label} must be in {interval}")
    return value


def _trusted_signer_inputs(
    args: argparse.Namespace,
) -> tuple[bytes, dict[str, Any]]:
    errors: list[str] = []
    public_key = parse_foundational_signer_public_key(
        args.topology_qualification_verification_public_key_hex,
        errors,
        path="topology verification public key",
    )
    signer_errors: list[str] = []
    signer = validate_foundational_software_signer(
        {
            "backend": "software",
            "service_id": args.topology_qualification_signer_service_id,
            "administrator_id": (
                args.topology_qualification_signer_administrator_id
            ),
            "key_revision": args.topology_qualification_signer_key_revision,
            "policy_revision": args.topology_qualification_signer_policy_revision,
            "policy_digest_sha256": (
                args.topology_qualification_signer_policy_digest_hex
            ),
        },
        signer_errors,
    )
    errors.extend(f"topology {error}" for error in signer_errors)
    try:
        _bounded_clock(args.now_unix, label="--now-unix")
        _bounded_clock(
            args.max_topology_qualification_review_age_secs,
            label="--max-topology-qualification-review-age-secs",
            allow_zero=True,
        )
    except TopologyEnvelopeError as error:
        errors.append(str(error))
    if errors:
        raise TopologyEnvelopeError(errors[0])
    assert public_key is not None
    return public_key, signer


def build_prepared_envelope(args: argparse.Namespace) -> dict[str, Any]:
    """Replay the reviewed summary and construct one exact unsigned envelope."""

    public_key, signer = _trusted_signer_inputs(args)
    reviewed_at_unix = _bounded_clock(
        args.reviewed_at_unix,
        label="--reviewed-at-unix",
    )
    now_unix = _bounded_clock(args.now_unix, label="--now-unix")
    max_age = _bounded_clock(
        args.max_topology_qualification_review_age_secs,
        label="--max-topology-qualification-review-age-secs",
        allow_zero=True,
    )
    if reviewed_at_unix > now_unix:
        raise TopologyEnvelopeError(
            "--reviewed-at-unix must not be later than --now-unix"
        )
    if now_unix - reviewed_at_unix > max_age:
        raise TopologyEnvelopeError(
            "topology review exceeds --max-topology-qualification-review-age-secs"
        )

    binding, errors = load_topology_qualification_binding(
        args.topology_qualification_summary,
        expected_deployment_id=args.deployment_id,
        expected_environment=args.environment,
    )
    if errors or binding is None:
        raise TopologyEnvelopeError(
            errors[0] if errors else "topology qualification summary is invalid"
        )
    if set(binding) != TOPOLOGY_BINDING_FIELDS:
        raise TopologyEnvelopeError(
            "topology qualification binding fields must match the schema-closed contract"
        )

    prepared = {
        "schema": SIGNED_QUALIFICATION_ENVELOPE_SCHEMA,
        **binding,
        "signer_authentication_kind": "external-ed25519",
        "signer_backend": signer["signer_backend"],
        "signer_service_id": signer["signer_service_id"],
        "signer_administrator_id": signer["signer_administrator_id"],
        "signer_key_revision": signer["signer_key_revision"],
        "signer_policy_revision": signer["signer_policy_revision"],
        "signer_policy_digest_sha256": signer[
            "signer_policy_digest_sha256"
        ],
        "signer_public_key_fingerprint_sha256": hashlib.sha256(public_key).hexdigest(),
        "reviewed_at_unix": reviewed_at_unix,
        "signature_algorithm": "ed25519",
    }
    prepared_topology_qualification_signing_bytes(prepared)
    return prepared


def _load_prepared(path: Path) -> dict[str, Any]:
    try:
        raw = read_evidence_bytes(
            path,
            MAX_QUALIFICATION_ENVELOPE_BYTES,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise TopologyEnvelopeError(str(error)) from error
    value = parse_json_bytes(
        raw,
        label="prepared topology envelope",
        maximum=MAX_QUALIFICATION_ENVELOPE_BYTES,
    )
    require_canonical_json_file(raw, value, label="prepared topology envelope")
    if not isinstance(value, dict):
        raise TopologyEnvelopeError("prepared topology envelope must be an object")
    prepared_topology_qualification_signing_bytes(value)
    return value


def _load_detached_signature(path: Path) -> bytes:
    try:
        signature = read_evidence_bytes(
            path,
            RAW_ED25519_SIGNATURE_BYTES,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise TopologyEnvelopeError(str(error)) from error
    if len(signature) != RAW_ED25519_SIGNATURE_BYTES or not any(signature):
        raise TopologyEnvelopeError(
            "detached topology signature must be exactly 64 non-zero raw bytes"
        )
    return signature


def finalize_envelope(args: argparse.Namespace) -> dict[str, Any]:
    """Replay prepared inputs and authenticate one detached signature."""

    expected = build_prepared_envelope(args)
    prepared = _load_prepared(args.prepared)
    if canonical_json_file_bytes(prepared) != canonical_json_file_bytes(expected):
        raise TopologyEnvelopeError(
            "prepared topology envelope must exactly match the replayed review"
        )
    signature = _load_detached_signature(args.signature_file)
    public_key, _signer = _trusted_signer_inputs(args)
    if not verify_ed25519(
        public_key,
        signature,
        prepared_topology_qualification_signing_bytes(prepared),
    ):
        raise TopologyEnvelopeError(
            "detached Ed25519 topology signature is invalid"
        )
    envelope = dict(prepared)
    envelope["signature_hex"] = signature.hex()
    if set(envelope) != SIGNED_QUALIFICATION_ENVELOPE_FIELDS:
        raise TopologyEnvelopeError(
            "signed topology envelope fields must match the schema-closed contract"
        )
    return envelope


def verify_envelope(args: argparse.Namespace) -> dict[str, Any]:
    """Verify the finalized envelope and return its payload-free binding."""

    public_key, _signer = _trusted_signer_inputs(args)
    binding, errors = load_signed_topology_qualification_binding(
        args.topology_qualification_summary,
        args.topology_qualification_envelope,
        trusted_public_key=public_key,
        trusted_signer_service_id=args.topology_qualification_signer_service_id,
        trusted_signer_administrator_id=(
            args.topology_qualification_signer_administrator_id
        ),
        trusted_key_revision=args.topology_qualification_signer_key_revision,
        trusted_policy_revision=args.topology_qualification_signer_policy_revision,
        trusted_policy_digest_hex=(
            args.topology_qualification_signer_policy_digest_hex
        ),
        now_unix=args.now_unix,
        max_review_age_secs=args.max_topology_qualification_review_age_secs,
        expected_deployment_id=args.deployment_id,
        expected_environment=args.environment,
    )
    if errors or binding is None:
        raise TopologyEnvelopeError(
            errors[0] if errors else "signed topology envelope is invalid"
        )
    if set(binding) != AUTHENTICATED_TOPOLOGY_BINDING_FIELDS:
        raise TopologyEnvelopeError(
            "authenticated topology binding fields must match the schema-closed contract"
        )
    return binding


def _add_common_arguments(parser: argparse.ArgumentParser) -> None:
    add_topology_qualification_argument(parser)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--now-unix", required=True, type=positive_int_arg)
    parser.add_argument(
        "--max-topology-qualification-review-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_QUALIFICATION_REVIEW_AGE_SECS,
    )
    parser.add_argument(
        "--topology-qualification-verification-public-key-hex",
        required=True,
    )
    parser.add_argument(
        "--topology-qualification-signer-service-id",
        required=True,
    )
    parser.add_argument(
        "--topology-qualification-signer-administrator-id",
        required=True,
    )
    parser.add_argument(
        "--topology-qualification-signer-key-revision",
        required=True,
        type=positive_int_arg,
    )
    parser.add_argument(
        "--topology-qualification-signer-policy-revision",
        required=True,
        type=positive_int_arg,
    )
    parser.add_argument(
        "--topology-qualification-signer-policy-digest-hex",
        required=True,
    )


def _parser() -> _SanitizedArgumentParser:
    parser = _SanitizedArgumentParser(description=__doc__, allow_abbrev=False)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser(
        "prepare",
        help="Validate the review and emit external signing bytes.",
        allow_abbrev=False,
    )
    _add_common_arguments(prepare)
    prepare.add_argument("--reviewed-at-unix", required=True, type=positive_int_arg)
    prepare.add_argument("--prepared-out", required=True, type=Path)
    prepare.add_argument("--signing-payload-out", required=True, type=Path)
    finalize = commands.add_parser(
        "finalize",
        help="Replay the review and attach a detached signature.",
        allow_abbrev=False,
    )
    _add_common_arguments(finalize)
    finalize.add_argument("--reviewed-at-unix", required=True, type=positive_int_arg)
    finalize.add_argument("--prepared", required=True, type=Path)
    finalize.add_argument("--signature-file", required=True, type=Path)
    finalize.add_argument("--envelope-out", required=True, type=Path)
    verify = commands.add_parser(
        "verify",
        help="Replay trust and verify one finalized envelope.",
        allow_abbrev=False,
    )
    _add_common_arguments(verify)
    verify.add_argument(
        "--topology-qualification-envelope",
        required=True,
        type=Path,
    )
    verify.add_argument("--verification-out", type=Path)
    return parser


def _contains_secret_option(arguments: Sequence[str]) -> bool:
    for argument in arguments:
        if not isinstance(argument, str):
            continue
        folded = argument.casefold()
        if folded.startswith(SECRET_ARGUMENT_PREFIXES):
            return True
        if not folded.startswith("--"):
            continue
        normalized = normalize_sensitive_key(folded.split("=", 1)[0][2:])
        if normalized in SECRET_ARGUMENT_NAMES:
            return True
        if normalized in COMMON_SENSITIVE_KEY_NORMALIZED or any(
            fragment in normalized for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
        ):
            return True
    return False


def _require_unique_scalar_options(arguments: Sequence[str]) -> None:
    counts = {option: 0 for option in SINGLETON_OPTIONS}
    for argument in arguments:
        if not isinstance(argument, str):
            continue
        option = argument.split("=", 1)[0]
        if option in counts:
            counts[option] += 1
    if any(count > 1 for count in counts.values()):
        raise TopologyEnvelopePreflightError(
            "topology envelope scalar options must not be repeated"
        )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse expanded response-file arguments without accepting secrets."""

    raw_args = list(sys.argv[1:] if argv is None else argv)
    if _contains_secret_option(raw_args):
        raise TopologyEnvelopePreflightError(
            "secret signing inputs are not accepted"
        )
    parser = _parser()
    expanded_args = expand_response_args(raw_args, parser)
    if _contains_secret_option(expanded_args):
        raise TopologyEnvelopePreflightError(
            "secret signing inputs are not accepted"
        )
    _require_unique_scalar_options(expanded_args)
    return parser.parse_args(expanded_args)


def _open_anchored_output_parent(path: Path, *, label: str) -> tuple[int, str]:
    """Open every output ancestor without following links."""

    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory = getattr(os, "O_DIRECTORY", 0)
    if (
        not nofollow
        or not directory
        or not OPEN_DIR_FD_SUPPORTED
        or not STAT_DIR_FD_SUPPORTED
        or not STAT_NOFOLLOW_SUPPORTED
        or not UNLINK_DIR_FD_SUPPORTED
        or not LINK_DIR_FD_SUPPORTED
        or not LINK_NOFOLLOW_SUPPORTED
    ):
        raise TopologyEnvelopePreflightError(
            f"{label} cannot guarantee anchored no-follow publication"
        )
    parts = path.parts
    if path.is_absolute():
        if not parts or parts[0] != os.sep:
            raise TopologyEnvelopePreflightError(
                f"{label} path root is not canonical"
            )
        anchor = os.sep
        relative_parts = parts[1:]
    else:
        anchor = "."
        relative_parts = parts
    if not relative_parts or any(
        part in {"", ".", ".."} for part in relative_parts
    ):
        raise TopologyEnvelopePreflightError(
            f"{label} path must name a direct file"
        )
    flags = os.O_RDONLY | directory | nofollow | getattr(os, "O_CLOEXEC", 0)
    current = -1
    try:
        current = os.open(anchor, flags)
        for part in relative_parts[:-1]:
            child = os.open(part, flags, dir_fd=current)
            metadata = os.fstat(child)
            if not stat.S_ISDIR(metadata.st_mode):
                os.close(child)
                raise TopologyEnvelopePreflightError(
                    f"{label} parent chain must contain direct directories"
                )
            os.close(current)
            current = child
        return current, relative_parts[-1]
    except TopologyEnvelopePreflightError:
        if current >= 0:
            os.close(current)
        raise
    except (OSError, RuntimeError) as error:
        if current >= 0:
            os.close(current)
        raise TopologyEnvelopePreflightError(
            f"{label} parent chain is not accessible"
        ) from error


def _require_leaf_absent(parent_fd: int, leaf: str, *, label: str) -> None:
    try:
        os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return
    except (OSError, RuntimeError) as error:
        raise TopologyEnvelopePreflightError(
            f"{label} cannot be inspected safely"
        ) from error
    raise TopologyEnvelopePreflightError(f"{label} must not already exist")


def _require_private_output_parent(parent_fd: int, *, label: str) -> None:
    """Require an owner-controlled directory for conditional rollback safety."""

    try:
        metadata = os.fstat(parent_fd)
        owner = os.geteuid()
    except (AttributeError, OSError, RuntimeError) as error:
        raise TopologyEnvelopePreflightError(
            f"{label} parent ownership cannot be verified"
        ) from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != owner
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise TopologyEnvelopePreflightError(
            f"{label} parent must be owned by the invoking user and mode 0700"
        )


def _unlink_owned(
    parent_fd: int,
    leaf: str,
    identity: tuple[int, int],
) -> bool:
    """Remove one path only when it is still the exact inode we created."""

    try:
        metadata = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return True
    except (OSError, RuntimeError):
        return False
    if (metadata.st_dev, metadata.st_ino) != identity:
        return False
    try:
        os.unlink(leaf, dir_fd=parent_fd)
    except FileNotFoundError:
        return True
    except (OSError, RuntimeError):
        return False
    return True


def _output_path_identity_matches(record: _StagedOutput) -> bool:
    """Reopen a requested output path and compare its parent and leaf identity."""

    parent_fd = -1
    try:
        parent_fd, leaf = _open_anchored_output_parent(
            record.path,
            label=record.label,
        )
        parent = os.fstat(parent_fd)
        leaf_stat = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        return (
            parent.st_dev,
            parent.st_ino,
            leaf_stat.st_dev,
            leaf_stat.st_ino,
        ) == (
            *record.parent_identity,
            *record.identity,
        )
    except (OSError, RuntimeError, TopologyEnvelopePreflightError):
        return False
    finally:
        if parent_fd >= 0:
            os.close(parent_fd)


def _staged_bytes_match(record: _StagedOutput) -> bool:
    """Rehash the still-open staged inode and confirm exact bytes and metadata."""

    try:
        before = os.fstat(record.descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or (before.st_dev, before.st_ino) != record.identity
            or before.st_size != record.expected_size
            or stat.S_IMODE(before.st_mode) != 0o600
        ):
            return False
        os.lseek(record.descriptor, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        remaining = record.expected_size
        while remaining:
            chunk = os.read(record.descriptor, min(1024 * 1024, remaining))
            if not chunk:
                return False
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(record.descriptor, 1):
            return False
        after = os.fstat(record.descriptor)
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        return (
            digest.hexdigest() == record.expected_sha256
            and after.st_nlink == 1
            and stat.S_IMODE(after.st_mode) == 0o600
            and all(
                getattr(before, field) == getattr(after, field)
                for field in stable_fields
            )
        )
    except (OSError, RuntimeError):
        return False


def _publish_new_outputs(*outputs: tuple[Path, bytes, str]) -> None:
    """Stage, fsync, and exclusively publish one rollback-safe output set."""

    identities: set[str] = set()
    for path, _payload, _label in outputs:
        identity = os.path.abspath(os.fspath(path))
        if identity in identities:
            raise TopologyEnvelopePreflightError(
                "topology output paths must be distinct"
            )
        identities.add(identity)

    staged: list[_StagedOutput] = []
    parent_fds: list[int] = []
    success = False
    active_label = "topology output"
    try:
        for path, payload, label in outputs:
            active_label = label
            if not isinstance(payload, bytes) or not payload:
                raise TopologyEnvelopePreflightError(
                    f"{label} must contain non-empty bytes"
                )
            parent_fd, leaf = _open_anchored_output_parent(path, label=label)
            parent_fds.append(parent_fd)
            _require_private_output_parent(parent_fd, label=label)
            _require_leaf_absent(parent_fd, leaf, label=label)
            temporary_leaf = f".sorafs-topology-{secrets.token_hex(16)}.tmp"
            flags = (
                os.O_RDWR
                | os.O_CREAT
                | os.O_EXCL
                | os.O_NOFOLLOW
                | getattr(os, "O_CLOEXEC", 0)
            )
            descriptor = os.open(
                temporary_leaf,
                flags,
                0o600,
                dir_fd=parent_fd,
            )
            metadata = os.fstat(descriptor)
            parent_metadata = os.fstat(parent_fd)
            record = _StagedOutput(
                path=path,
                parent_fd=parent_fd,
                parent_identity=(parent_metadata.st_dev, parent_metadata.st_ino),
                leaf=leaf,
                temporary_leaf=temporary_leaf,
                descriptor=descriptor,
                identity=(metadata.st_dev, metadata.st_ino),
                expected_size=len(payload),
                expected_sha256=hashlib.sha256(payload).hexdigest(),
                label=label,
            )
            staged.append(record)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise TopologyEnvelopePreflightError(
                    f"{label} staging path is not a direct regular file"
                )
            os.fchmod(descriptor, 0o600)
            view = memoryview(payload)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    raise OSError("short output write")
                view = view[written:]
            os.fsync(descriptor)
            if not _staged_bytes_match(record):
                raise TopologyEnvelopePreflightError(
                    f"{label} changed while it was staged"
                )

        for record in staged:
            _require_leaf_absent(
                record.parent_fd,
                record.leaf,
                label=record.label,
            )
        for record in staged:
            active_label = record.label
            temporary = os.stat(
                record.temporary_leaf,
                dir_fd=record.parent_fd,
                follow_symlinks=False,
            )
            if (
                not stat.S_ISREG(temporary.st_mode)
                or temporary.st_nlink != 1
                or (temporary.st_dev, temporary.st_ino) != record.identity
            ):
                raise TopologyEnvelopePreflightError(
                    f"{record.label} changed before publication"
                )
            if not _staged_bytes_match(record):
                raise TopologyEnvelopePreflightError(
                    f"{record.label} bytes changed before publication"
                )
            os.link(
                record.temporary_leaf,
                record.leaf,
                src_dir_fd=record.parent_fd,
                dst_dir_fd=record.parent_fd,
                follow_symlinks=False,
            )
            record.published = True
            published = os.stat(
                record.leaf,
                dir_fd=record.parent_fd,
                follow_symlinks=False,
            )
            if (published.st_dev, published.st_ino) != record.identity:
                raise TopologyEnvelopePreflightError(
                    f"{record.label} changed during publication"
                )
            record.temporary_present = not _unlink_owned(
                record.parent_fd,
                record.temporary_leaf,
                record.identity,
            )
            final = os.stat(
                record.leaf,
                dir_fd=record.parent_fd,
                follow_symlinks=False,
            )
            if (
                not stat.S_ISREG(final.st_mode)
                or final.st_nlink != 1
                or (final.st_dev, final.st_ino) != record.identity
                or final.st_size != record.expected_size
                or stat.S_IMODE(final.st_mode) != 0o600
                or not _staged_bytes_match(record)
                or not _output_path_identity_matches(record)
            ):
                raise TopologyEnvelopePreflightError(
                    f"{record.label} changed after publication"
                )
            os.fsync(record.parent_fd)
        for record in staged:
            final = os.stat(
                record.leaf,
                dir_fd=record.parent_fd,
                follow_symlinks=False,
            )
            if (
                not stat.S_ISREG(final.st_mode)
                or final.st_nlink != 1
                or (final.st_dev, final.st_ino) != record.identity
                or final.st_size != record.expected_size
                or stat.S_IMODE(final.st_mode) != 0o600
                or not _staged_bytes_match(record)
                or not _output_path_identity_matches(record)
            ):
                raise TopologyEnvelopePreflightError(
                    f"{record.label} changed before the output set committed"
                )
        success = True
    except TopologyEnvelopePreflightError:
        raise
    except (OSError, RuntimeError) as error:
        raise TopologyEnvelopePreflightError(
            f"{active_label} could not be published safely"
        ) from error
    finally:
        if not success:
            for record in reversed(staged):
                _unlink_owned(
                    record.parent_fd,
                    record.leaf,
                    record.identity,
                )
                if record.temporary_present:
                    _unlink_owned(
                        record.parent_fd,
                        record.temporary_leaf,
                        record.identity,
                    )
                try:
                    os.fsync(record.parent_fd)
                except (OSError, RuntimeError):
                    pass
        for record in reversed(staged):
            try:
                os.close(record.descriptor)
            except OSError:
                pass
        for parent_fd in reversed(parent_fds):
            os.close(parent_fd)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the public no-private-key topology-envelope workflow."""

    try:
        args = parse_args(argv)
    except TopologyEnvelopePreflightError as error:
        emit_checker_error_lines((str(error),))
        return 2
    except (SystemExit, ValueError) as error:
        if isinstance(error, SystemExit):
            return error.code if isinstance(error.code, int) else 2
        emit_checker_exception(error)
        return 2

    try:
        if args.command == "prepare":
            prepared = build_prepared_envelope(args)
            signing_payload = prepared_topology_qualification_signing_bytes(prepared)
            rendered = canonical_json_file_bytes(prepared)
            _publish_new_outputs(
                (
                    args.prepared_out,
                    rendered,
                    "prepared topology output",
                ),
                (
                    args.signing_payload_out,
                    signing_payload,
                    "topology signing-payload output",
                ),
            )
            emit_checker_notice(
                "Prepared the topology review for external software Ed25519 signing."
            )
            return 0
        if args.command == "finalize":
            envelope = finalize_envelope(args)
            _publish_new_outputs(
                (
                    args.envelope_out,
                    canonical_json_file_bytes(envelope),
                    "signed topology output",
                )
            )
            emit_checker_notice(
                "Finalized the independently signed topology qualification envelope."
            )
            return 0

        binding = verify_envelope(args)
        rendered = canonical_json_file_bytes(binding)
        if args.verification_out is None:
            sys.stdout.buffer.write(rendered)
        else:
            _publish_new_outputs(
                (
                    args.verification_out,
                    rendered,
                    "topology verification output",
                )
            )
        return 0
    except TopologyEnvelopePreflightError as error:
        emit_checker_error_lines((str(error),))
        return 2
    except (InventoryError, SccpReleaseError, TopologyEnvelopeError, ValueError) as error:
        emit_checker_exception(error)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
