#!/usr/bin/env python3
"""Render one exact local-testnet Taira user LaunchAgent activation manifest.

The prepared reset manifest is the sole source of artifact and provenance
bindings.  Callers select only the generation, prepared bundle, admitted
release root, and final activation path.  This helper never signs genesis,
reads runtime private keys, invokes launchctl, or applies a reset.

By default the completed activation is installed atomically without replacing
an existing path.  ``--validate-with-controller`` first runs the admitted
controller's default read-only plan against the completed temporary file; the
final activation name is published only after that projection is coherent.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time
from typing import Callable, Mapping, Sequence


SCHEMA = "iroha.taira.user-launchagent-reset.v1"
RECEIPT_SCHEMA = "iroha.taira.user-launchagent-activation-render.v1"
DRY_RUN_SCHEMA = "iroha.taira.user-launchagent-reset-plan.v1"
RESET_SCHEMA = "taira-exact2f-reset-bundle"
LOCAL_RELEASE_SCHEMA = "iroha.taira.local_testnet_reviewed_reset.v1"
PEER_COUNT = 4
UID = 501
DOMAIN = "user/501"
LABELS = tuple(
    f"org.sora.taira.user.validator-{number}" for number in range(1, PEER_COUNT + 1)
)
GENERATION_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,95}\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")
GENESIS_PUBLIC_KEY_RE = re.compile(r"ed0120[0-9A-F]{64}\Z")
MAX_RESET_MANIFEST_BYTES = 1024 * 1024
MAX_BINARY_BYTES = 1024 * 1024 * 1024
MAX_GENESIS_BYTES = 64 * 1024 * 1024
MAX_CONTROLLER_BYTES = 4 * 1024 * 1024
MAX_SOURCE_CLOSURE_FILE_BYTES = 64 * 1024 * 1024
MAX_SOURCE_CLOSURE_BYTES = 512 * 1024 * 1024
MAX_SOURCE_CLOSURE_FILES = 128
MAX_DRY_RUN_STDOUT_BYTES = 2 * 1024 * 1024
MAX_DRY_RUN_STDERR_BYTES = 256 * 1024
DRY_RUN_TIMEOUT_SECONDS = 300.0
LOCAL_SOURCE_CLOSURE_FILES = (
    "configs/soranexus/taira/config.toml",
    "configs/soranexus/taira/genesis.json",
    "configs/soranexus/taira/privacy_bootstrap_plan.json",
    "scripts/build_privacy_v1_boi_handoff.py",
    "scripts/check_native_sdk_abi22_artifact.py",
    "scripts/compose_taira_nevo_reset_genesis.py",
    "scripts/compute_workspace_source_manifest.py",
    "scripts/deploy_taira_user_launchagent_reset.py",
    "scripts/deploy_taira_v21_reset.py",
    "scripts/deploy_taira_v21_reset_authority.py",
    "scripts/deploy_taira_v21_reset_health.py",
    "scripts/extract_authenticated_taira_privacy_release.py",
    "scripts/inspect_taira_local_reset_source_closure.py",
    "scripts/inspect_taira_local_reviewed_inputs.py",
    "scripts/iso_operator_auth.py",
    "scripts/operator_http_headers.py",
    "scripts/prepare_taira_empty_reset_bundle.py",
    "scripts/release_artifact_contract.py",
    "scripts/release_manifest_signing.py",
    "scripts/render_taira_validator_bundle.py",
    "scripts/seal_taira_release_controllers.py",
    "scripts/taira_authority_client.py",
    "scripts/taira_constants.py",
    "scripts/taira_privacy_protocol_receipt.py",
    "scripts/taira_privacy_rollout_contract.py",
    "scripts/taira_release_authority.py",
    "scripts/taira_rollout_admission.py",
)
LOCAL_REVIEWED_INPUT_FILES = (
    "config.toml",
    "genesis.json",
    "nevo-reset.review.json",
    "privacy_bootstrap_plan.json",
)

MANIFEST_KEYS = frozenset(
    {
        "schema",
        "generation",
        "uid",
        "launchctl_domain",
        "labels",
        "bundle",
        "reset_manifest_sha256",
        "binary",
        "binary_sha256",
        "genesis_native_verifier",
        "genesis_native_verifier_sha256",
        "operator_status_client",
        "operator_status_client_sha256",
        "genesis_external_signer_sha256",
        "genesis_public_key",
        "genesis_expected_hash",
        "genesis_artifact_linkage_sha256",
        "nevo_review_sha256",
        "reviewed_unsigned_genesis_sha256",
        "pre_sign_rendered_genesis_sha256",
        "native_verifier_peer_config_set_sha256",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "source_commit",
        "dpn_validator_release_commit",
        "limits",
        "local_reviewed_inputs_identity_sha256",
        "local_testnet_source_closure_sha256",
        "local_testnet_python_sha256",
    }
)
LINKAGE_KEYS = frozenset(
    {
        "schema",
        "review_sha256",
        "reviewed_unsigned_genesis_sha256",
        "validator_roster_sha256",
        "pre_sign_rendered_genesis_sha256",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "genesis_expected_hash",
        "genesis_public_key",
        "external_signer_sha256",
        "native_genesis_verifier_sha256",
        "operator_status_client_sha256",
        "native_genesis_verifier_receipt_sha256",
        "native_verifier_peer_config_set_sha256",
        "local_reviewed_inputs_identity_sha256",
    }
)
LIMITS = {
    "minimum_free_bytes": 16 * 1024 * 1024 * 1024,
    "maximum_fsync_latency_ms": 500,
    "startup_timeout_seconds": 300,
    "stability_timeout_seconds": 60,
    "poll_interval_seconds": 1,
}
DRY_RUN_KEYS = frozenset(
    {
        "schema",
        "mode",
        "activation_manifest",
        "activation_manifest_sha256",
        "confirmation_required",
        "launchctl_domain",
        "labels",
        "network_id",
        "bundle",
        "binary",
        "binary_sha256",
        "genesis_native_verifier",
        "genesis_native_verifier_sha256",
        "genesis_external_signer_sha256",
        "operator_status_client",
        "operator_status_client_sha256",
        "genesis_public_key",
        "genesis_expected_hash",
        "genesis_artifact_linkage_sha256",
        "archive",
        "candidate_logs",
        "candidate_plists",
        "predecessor",
        "mutated",
    }
)


class ActivationRenderError(RuntimeError):
    """One bounded activation-rendering invariant was not met."""


def fail(message: str) -> None:
    raise ActivationRenderError(message)


@dataclasses.dataclass(frozen=True)
class Layout:
    taira_root: Path
    reset_bundles: Path
    reset_manifests: Path
    releases: Path
    local_controllers: Path
    local_python: Path
    uid: int = UID


PRODUCTION_LAYOUT = Layout(
    taira_root=Path("/Users/administrator/apps/dpn-test/taira"),
    reset_bundles=Path("/Users/administrator/apps/dpn-test/taira/reset-bundles"),
    reset_manifests=Path("/Users/administrator/apps/dpn-test/taira/reset-manifests"),
    releases=Path("/Users/administrator/apps/dpn-test/taira/releases"),
    local_controllers=Path(
        "/Users/administrator/apps/dpn-test/taira/local-reset-controller"
    ),
    local_python=Path("/opt/homebrew/bin/python3"),
)


@dataclasses.dataclass(frozen=True)
class FileIdentity:
    path: Path
    metadata: tuple[int, int, int, int, int, int, int, int, int]
    sha256: str


def metadata_identity(
    info: os.stat_result,
) -> tuple[int, int, int, int, int, int, int, int, int]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def canonical_activation_json(value: object) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def canonical_artifact_json(value: object) -> bytes:
    return (
        json.dumps(
            value,
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def reject_duplicate_keys(pairs: Sequence[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member {key!r}")
        result[key] = value
    return result


def parse_json_object(raw: bytes, label: str) -> dict[str, object]:
    def reject_constant(value: str) -> object:
        raise ValueError(f"non-finite JSON number {value}")

    try:
        value = json.loads(
            raw,
            object_pairs_hook=reject_duplicate_keys,
            parse_constant=reject_constant,
        )
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as error:
        raise ActivationRenderError(f"{label} is not exact UTF-8 JSON: {error}") from error
    if not isinstance(value, dict):
        fail(f"{label} must be one JSON object")
    return value


def require_sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        fail(f"{label} must be one lowercase SHA-256")
    return value


def require_commit(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or int(value, 16) == 0
    ):
        fail(f"{label} must be one nonzero full lowercase Git commit")
    return value


def require_absolute(path: Path, label: str) -> Path:
    if not path.is_absolute() or ".." in path.parts:
        fail(f"{label} must be one absolute non-traversing path")
    return path


def relative_descendant(
    path: Path,
    root: Path,
    label: str,
    *,
    exact_parts: int,
) -> tuple[str, ...]:
    require_absolute(path, label)
    try:
        relative = path.relative_to(root)
    except ValueError:
        fail(f"{label} must remain under {root}")
    parts = relative.parts
    if len(parts) != exact_parts or any(part in {"", ".", ".."} for part in parts):
        fail(f"{label} path shape is not exact")
    return parts


def require_private_directory(path: Path, uid: int, label: str) -> None:
    try:
        info = path.lstat()
    except OSError as error:
        raise ActivationRenderError(f"{label} is unavailable: {path}") from error
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_uid != uid
        or stat.S_IMODE(info.st_mode) != 0o700
    ):
        fail(f"{label} must be one owner-private mode-0700 directory")


def require_no_symlink_ancestry(path: Path, root: Path, label: str) -> None:
    try:
        parts = path.relative_to(root).parts
    except ValueError:
        fail(f"{label} escaped its trusted root")
    current = root
    for index, part in enumerate((None, *parts)):
        if index:
            assert part is not None
            current = current / part
        try:
            info = current.lstat()
        except FileNotFoundError:
            return
        except OSError as error:
            raise ActivationRenderError(f"{label} ancestry is unavailable") from error
        if stat.S_ISLNK(info.st_mode):
            fail(f"{label} ancestry contains a symlink: {current}")
        if index < len(parts) and not stat.S_ISDIR(info.st_mode):
            fail(f"{label} ancestry contains a non-directory: {current}")


def read_or_hash_regular(
    path: Path,
    *,
    maximum: int,
    uid: int,
    label: str,
    exact_mode: int | None = None,
    require_executable: bool = False,
    return_body: bool = False,
) -> tuple[FileIdentity, bytes | None]:
    try:
        before = path.lstat()
    except OSError as error:
        raise ActivationRenderError(f"{label} is unavailable: {path}") from error
    mode = stat.S_IMODE(before.st_mode)
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_uid != uid
        or before.st_nlink != 1
        or before.st_size > maximum
        or before.st_size <= 0
        or mode & (stat.S_IWGRP | stat.S_IWOTH)
        or (exact_mode is not None and mode != exact_mode)
        or (require_executable and mode & 0o111 == 0)
    ):
        fail(f"{label} must be one bounded owner-controlled regular file")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ActivationRenderError(f"{label} could not be opened safely") from error
    digest = hashlib.sha256()
    chunks: list[bytes] = []
    observed_size = 0
    try:
        opened = os.fstat(descriptor)
        if metadata_identity(opened) != metadata_identity(before):
            fail(f"{label} changed while opening")
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            observed_size += len(chunk)
            if observed_size > maximum:
                fail(f"{label} exceeds its size bound")
            digest.update(chunk)
            if return_body:
                chunks.append(chunk)
            remaining -= len(chunk)
    finally:
        os.close(descriptor)
    try:
        after = path.lstat()
    except OSError as error:
        raise ActivationRenderError(f"{label} disappeared while reading") from error
    if metadata_identity(after) != metadata_identity(before):
        fail(f"{label} changed while reading")
    identity = FileIdentity(path, metadata_identity(before), digest.hexdigest())
    return identity, b"".join(chunks) if return_body else None


def revalidate(identity: FileIdentity, label: str) -> None:
    try:
        current = identity.path.lstat()
    except OSError as error:
        raise ActivationRenderError(f"{label} disappeared before publication") from error
    if metadata_identity(current) != identity.metadata:
        fail(f"{label} changed before publication")


def source_closure_rows(
    manifest: Mapping[str, object],
) -> tuple[dict[str, object], ...]:
    closure = manifest.get("local_testnet_source_closure")
    if not isinstance(closure, dict) or set(closure) != {"schema", "files", "sha256"}:
        fail("local source closure keys are not exact")
    if closure.get("schema") != "iroha.taira.local-testnet-reset-source-closure.v1":
        fail("local source closure schema is unsupported")
    rows = closure.get("files")
    if not isinstance(rows, list) or not 0 < len(rows) <= MAX_SOURCE_CLOSURE_FILES:
        fail("local source closure file count is outside its bound")
    normalized: list[dict[str, object]] = []
    total_size = 0
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or set(row) != {"path", "sha256", "size"}:
            fail(f"local source closure row {index} keys are not exact")
        relative = row.get("path")
        size = row.get("size")
        if not isinstance(relative, str):
            fail(f"local source closure row {index} path is not a string")
        relative_path = PurePosixPath(relative)
        if (
            relative_path.is_absolute()
            or str(relative_path) != relative
            or not relative_path.parts
            or any(part in {"", ".", ".."} for part in relative_path.parts)
        ):
            fail(f"local source closure row {index} path is not canonical")
        if (
            isinstance(size, bool)
            or not isinstance(size, int)
            or not 0 < size <= MAX_SOURCE_CLOSURE_FILE_BYTES
        ):
            fail(f"local source closure row {index} size is outside its bound")
        normalized.append(
            {
                "path": relative,
                "sha256": require_sha256(
                    row.get("sha256"), f"local source closure row {index} SHA-256"
                ),
                "size": size,
            }
        )
        total_size += size
    paths = [str(row["path"]) for row in normalized]
    if tuple(paths) != LOCAL_SOURCE_CLOSURE_FILES:
        fail("local source closure inventory is not the exact frozen file set")
    if total_size > MAX_SOURCE_CLOSURE_BYTES:
        fail("local source closure exceeds its aggregate byte bound")
    return tuple(normalized)


def verify_source_closure(
    *,
    root: Path,
    manifest: Mapping[str, object],
    uid: int,
) -> tuple[FileIdentity, ...]:
    """Verify the exact staged source inventory before executing its controller."""

    require_private_directory(root, uid, "staged source closure root")
    rows = source_closure_rows(manifest)
    expected_files = {str(row["path"]): row for row in rows}
    expected_directories = {""}
    for relative in expected_files:
        parent = PurePosixPath(relative).parent
        while str(parent) not in {"", "."}:
            expected_directories.add(str(parent))
            parent = parent.parent

    observed_files: set[str] = set()
    observed_directories: set[str] = {""}

    def scan(directory: Path, relative: PurePosixPath) -> None:
        try:
            entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except OSError as error:
            raise ActivationRenderError(
                f"staged source closure directory is unavailable: {directory}"
            ) from error
        for entry in entries:
            current_relative = relative / entry.name
            relative_text = str(current_relative)
            try:
                info = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise ActivationRenderError(
                    f"staged source closure entry is unavailable: {relative_text}"
                ) from error
            if entry.is_symlink():
                fail(f"staged source closure contains a symlink: {relative_text}")
            if stat.S_ISDIR(info.st_mode):
                if info.st_uid != uid or stat.S_IMODE(info.st_mode) != 0o700:
                    fail(
                        f"staged source closure directory custody changed: {relative_text}"
                    )
                observed_directories.add(relative_text)
                scan(Path(entry.path), current_relative)
            elif stat.S_ISREG(info.st_mode):
                observed_files.add(relative_text)
            else:
                fail(f"staged source closure entry type is forbidden: {relative_text}")

    scan(root, PurePosixPath())
    if observed_files != set(expected_files) or observed_directories != expected_directories:
        fail("staged source closure inventory is not exact")

    identities: list[FileIdentity] = []
    for relative, row in expected_files.items():
        identity, _ = read_or_hash_regular(
            root / relative,
            maximum=MAX_SOURCE_CLOSURE_FILE_BYTES,
            uid=uid,
            label=f"staged source closure {relative}",
            exact_mode=0o600,
        )
        if identity.metadata[6] != row["size"] or identity.sha256 != row["sha256"]:
            fail(f"staged source closure file differs: {relative}")
        identities.append(identity)
    return tuple(identities)


def require_manifest_bindings(manifest: Mapping[str, object]) -> dict[str, object]:
    if manifest.get("schema") != RESET_SCHEMA or manifest.get("peer_count") != PEER_COUNT:
        fail("prepared reset manifest is not the exact four-peer reset schema")
    privacy_release = manifest.get("privacy_bootstrap_release")
    if not isinstance(privacy_release, dict):
        fail("prepared reset manifest lacks its privacy release")
    if privacy_release.get("schema") != LOCAL_RELEASE_SCHEMA:
        fail("prepared reset manifest is not the same-host local-testnet release")
    if "release_controller" in manifest or "privacy_native_verifier_sha256" in manifest:
        fail("prepared local-testnet reset claims production authority")

    linkage = manifest.get("genesis_artifact_linkage")
    if not isinstance(linkage, dict) or set(linkage) != LINKAGE_KEYS:
        fail("prepared reset genesis artifact linkage keys are not exact")
    if linkage.get("schema") != "iroha.taira.nevo-genesis-artifact-linkage.v1":
        fail("prepared reset genesis artifact linkage schema is unsupported")
    linkage_digest = hashlib.sha256(canonical_artifact_json(linkage)).hexdigest()
    if linkage_digest != require_sha256(
        manifest.get("genesis_artifact_linkage_sha256"),
        "genesis artifact linkage SHA-256",
    ):
        fail("prepared reset genesis artifact linkage digest is inconsistent")

    local_identity = require_sha256(
        manifest.get("local_reviewed_inputs_identity_sha256"),
        "local reviewed inputs identity",
    )
    scalar_pairs = {
        "genesis_expected_hash": "genesis_expected_hash",
        "genesis_public_key": "genesis_public_key",
        "external_signer_sha256": "genesis_external_signer_sha256",
        "native_genesis_verifier_sha256": "genesis_native_verifier_sha256",
        "operator_status_client_sha256": "operator_status_client_sha256",
        "pre_sign_rendered_genesis_sha256": "pre_sign_rendered_genesis_sha256",
        "bound_genesis_manifest_sha256": "bound_genesis_manifest_sha256",
        "signed_genesis_sha256": "signed_genesis_sha256",
        "native_verifier_peer_config_set_sha256": (
            "native_verifier_peer_config_set_sha256"
        ),
        "local_reviewed_inputs_identity_sha256": (
            "local_reviewed_inputs_identity_sha256"
        ),
    }
    for linkage_key, manifest_key in scalar_pairs.items():
        if linkage.get(linkage_key) != manifest.get(manifest_key):
            fail(f"prepared reset binding is inconsistent: {manifest_key}")
    if linkage.get("local_reviewed_inputs_identity_sha256") != local_identity:
        fail("prepared reset local reviewed identity is inconsistent")
    if (
        privacy_release.get("bound_genesis_manifest_sha256")
        != linkage.get("bound_genesis_manifest_sha256")
        or privacy_release.get("signed_genesis_sha256")
        != linkage.get("signed_genesis_sha256")
    ):
        fail("privacy release and genesis linkage artifact hashes differ")
    review_record = privacy_release.get("nevo_reset_review")
    if (
        not isinstance(review_record, dict)
        or set(review_record)
        != {
            "schema",
            "sha256",
            "public_inputs_sha256",
            "unsigned_genesis_sha256",
            "public_identities",
            "credential_hash_bindings",
        }
        or review_record.get("sha256") != linkage.get("review_sha256")
        or review_record.get("unsigned_genesis_sha256")
        != linkage.get("reviewed_unsigned_genesis_sha256")
    ):
        fail("privacy release and genesis linkage NEVO hashes differ")

    for field in (
        "review_sha256",
        "reviewed_unsigned_genesis_sha256",
        "validator_roster_sha256",
        "pre_sign_rendered_genesis_sha256",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "external_signer_sha256",
        "native_genesis_verifier_sha256",
        "operator_status_client_sha256",
        "native_genesis_verifier_receipt_sha256",
        "native_verifier_peer_config_set_sha256",
        "local_reviewed_inputs_identity_sha256",
    ):
        require_sha256(linkage.get(field), f"genesis linkage {field}")
    public_key = linkage.get("genesis_public_key")
    if not isinstance(public_key, str) or GENESIS_PUBLIC_KEY_RE.fullmatch(public_key) is None:
        fail("genesis public key is not one canonical Ed25519 multihash")
    expected_hash = require_sha256(linkage.get("genesis_expected_hash"), "genesis hash")
    if int(expected_hash, 16) == 0:
        fail("genesis hash must be nonzero")

    source_closure = manifest.get("local_testnet_source_closure")
    local_python = manifest.get("local_testnet_python")
    if not isinstance(source_closure, dict) or not isinstance(local_python, dict):
        fail("prepared reset lacks its local source or Python binding")
    source_closure_sha256 = require_sha256(
        source_closure.get("sha256"), "local source closure SHA-256"
    )
    closure_payload = dict(source_closure)
    del closure_payload["sha256"]
    if (
        hashlib.sha256(canonical_artifact_json(closure_payload)).hexdigest()
        != source_closure_sha256
    ):
        fail("local source closure digest is inconsistent")
    if set(local_python) != {"path", "sha256"}:
        fail("prepared reset local Python binding keys are not exact")
    local_python_sha256 = require_sha256(
        local_python.get("sha256"), "local Python SHA-256"
    )
    reviewed_release_identity = privacy_release.get("reviewed_inputs_identity_sha256")
    if reviewed_release_identity != local_identity:
        fail("privacy release and reset manifest reviewed identities differ")

    expected_privacy_keys = {
        "schema",
        "reviewed_inputs",
        "bound_genesis_manifest_sha256",
        "signed_genesis_sha256",
        "validator_config_sha256",
        "nevo_reset_review",
        "authority_claim",
        "issuer_state",
        "post_genesis_issuer_enablement_required",
        "reviewed_inputs_identity_sha256",
        "source",
    }
    if set(privacy_release) != expected_privacy_keys:
        fail("local privacy release keys are not exact")
    if (
        privacy_release.get("authority_claim")
        != "none-user-authorized-same-host-testnet"
        or privacy_release.get("issuer_state") != "disabled-no-broker"
        or privacy_release.get("post_genesis_issuer_enablement_required") is not True
    ):
        fail("local privacy release authority or issuer claim is invalid")
    reviewed_inputs = privacy_release.get("reviewed_inputs")
    if not isinstance(reviewed_inputs, dict) or set(reviewed_inputs) != set(
        LOCAL_REVIEWED_INPUT_FILES
    ):
        fail("local privacy release reviewed input inventory is not exact")
    for name in LOCAL_REVIEWED_INPUT_FILES:
        row = reviewed_inputs.get(name)
        if not isinstance(row, dict) or set(row) != {"sha256", "size"}:
            fail(f"local privacy release reviewed input row is not exact: {name}")
        size = row.get("size")
        require_sha256(row.get("sha256"), f"local reviewed input {name} SHA-256")
        if (
            isinstance(size, bool)
            or not isinstance(size, int)
            or not 0 < size <= MAX_GENESIS_BYTES
        ):
            fail(f"local privacy release reviewed input size is invalid: {name}")
    closure_rows = {str(row["path"]): row for row in source_closure_rows(manifest)}
    for reviewed_name, closure_name in (
        ("config.toml", "configs/soranexus/taira/config.toml"),
        (
            "privacy_bootstrap_plan.json",
            "configs/soranexus/taira/privacy_bootstrap_plan.json",
        ),
    ):
        reviewed_row = reviewed_inputs[reviewed_name]
        closure_row = closure_rows[closure_name]
        if (
            reviewed_row.get("sha256") != closure_row.get("sha256")
            or reviewed_row.get("size") != closure_row.get("size")
        ):
            fail(f"local reviewed input is not source-pinned: {reviewed_name}")
    source = privacy_release.get("source")
    expected_source = {
        "commit": require_commit(manifest.get("source_commit"), "source commit"),
        "dpn_validator_release_commit": require_commit(
            manifest.get("dpn_validator_release_commit"),
            "DPN validator release commit",
        ),
        "cargo_lock_sha256": require_sha256(
            manifest.get("cargo_lock_sha256"), "Cargo.lock SHA-256"
        ),
        "workspace_source_manifest_sha256": require_sha256(
            manifest.get("workspace_source_manifest_sha256"),
            "workspace source manifest SHA-256",
        ),
    }
    if source != expected_source:
        fail("local privacy release source identity is inconsistent")
    reviewed_identity_manifest = {
        "schema": "iroha.taira.local_testnet_reviewed_inputs.v1",
        "authority_claim": "none-user-authorized-same-host-testnet",
        "source": source,
        "privacy_inputs": reviewed_inputs,
    }
    if (
        hashlib.sha256(canonical_artifact_json(reviewed_identity_manifest)).hexdigest()
        != local_identity
    ):
        fail("local reviewed input identity is inconsistent")
    configs = manifest.get("configs")
    expected_config_names = {
        f"taira-validator-{number}" for number in range(1, PEER_COUNT + 1)
    }
    if (
        not isinstance(configs, dict)
        or set(configs) != expected_config_names
        or any(
            require_sha256(value, f"validator config {name} SHA-256") != value
            for name, value in configs.items()
        )
        or privacy_release.get("validator_config_sha256") != configs
    ):
        fail("local privacy release validator config binding is inconsistent")

    return {
        "linkage": linkage,
        "local_reviewed_inputs_identity_sha256": local_identity,
        "local_testnet_source_closure_sha256": source_closure_sha256,
        "local_testnet_python_path": local_python.get("path"),
        "local_testnet_python_sha256": local_python_sha256,
        "source_commit": expected_source["commit"],
        "dpn_validator_release_commit": expected_source[
            "dpn_validator_release_commit"
        ],
    }


def build_activation(
    *,
    generation: str,
    bundle: Path,
    release_root: Path,
    manifest: Mapping[str, object],
    reset_manifest_sha256: str,
) -> dict[str, object]:
    bindings = require_manifest_bindings(manifest)
    linkage = bindings["linkage"]
    assert isinstance(linkage, dict)
    activation: dict[str, object] = {
        "schema": SCHEMA,
        "generation": generation,
        "uid": UID,
        "launchctl_domain": DOMAIN,
        "labels": list(LABELS),
        "bundle": str(bundle),
        "reset_manifest_sha256": reset_manifest_sha256,
        "binary": str(release_root / "iroha3d"),
        "binary_sha256": require_sha256(manifest.get("irohad_sha256"), "iroha3d SHA-256"),
        "genesis_native_verifier": str(release_root / "kagami"),
        "genesis_native_verifier_sha256": require_sha256(
            manifest.get("genesis_native_verifier_sha256"), "Kagami SHA-256"
        ),
        "operator_status_client": str(release_root / "taira_operator_status"),
        "operator_status_client_sha256": require_sha256(
            manifest.get("operator_status_client_sha256"),
            "operator status client SHA-256",
        ),
        "genesis_external_signer_sha256": require_sha256(
            manifest.get("genesis_external_signer_sha256"),
            "external signer SHA-256",
        ),
        "genesis_public_key": linkage["genesis_public_key"],
        "genesis_expected_hash": linkage["genesis_expected_hash"],
        "genesis_artifact_linkage_sha256": require_sha256(
            manifest.get("genesis_artifact_linkage_sha256"),
            "genesis artifact linkage SHA-256",
        ),
        "nevo_review_sha256": linkage["review_sha256"],
        "reviewed_unsigned_genesis_sha256": linkage[
            "reviewed_unsigned_genesis_sha256"
        ],
        "pre_sign_rendered_genesis_sha256": linkage[
            "pre_sign_rendered_genesis_sha256"
        ],
        "native_verifier_peer_config_set_sha256": linkage[
            "native_verifier_peer_config_set_sha256"
        ],
        "bound_genesis_manifest_sha256": linkage["bound_genesis_manifest_sha256"],
        "signed_genesis_sha256": linkage["signed_genesis_sha256"],
        "source_commit": bindings["source_commit"],
        "dpn_validator_release_commit": bindings["dpn_validator_release_commit"],
        "limits": dict(LIMITS),
        "local_reviewed_inputs_identity_sha256": bindings[
            "local_reviewed_inputs_identity_sha256"
        ],
        "local_testnet_source_closure_sha256": bindings[
            "local_testnet_source_closure_sha256"
        ],
        "local_testnet_python_sha256": bindings["local_testnet_python_sha256"],
    }
    if set(activation) != MANIFEST_KEYS or len(activation) != 29:
        raise AssertionError("activation schema is not the exact 29-field local schema")
    return activation


def _read_bounded_process_output(stream: object, maximum: int, label: str) -> bytes:
    assert hasattr(stream, "seek") and hasattr(stream, "read")
    stream.seek(0)  # type: ignore[attr-defined]
    body = stream.read(maximum + 1)  # type: ignore[attr-defined]
    if len(body) > maximum:
        fail(f"controller dry-run {label} exceeds its byte bound")
    return body


def validate_controller_dry_run(
    *,
    controller: Path,
    activation_path: Path,
    activation_sha256: str,
    activation: Mapping[str, object],
    manifest: Mapping[str, object],
    layout: Layout,
) -> dict[str, object]:
    parts = relative_descendant(
        controller,
        layout.local_controllers,
        "controller",
        exact_parts=3,
    )
    if parts[1:] != ("scripts", "deploy_taira_user_launchagent_reset.py"):
        fail("controller path is not one exact staged local reset entry point")
    require_no_symlink_ancestry(controller, layout.taira_root, "controller")
    closure_root = controller.parent.parent
    closure_identities = verify_source_closure(
        root=closure_root,
        manifest=manifest,
        uid=layout.uid,
    )
    controller_identity, _ = read_or_hash_regular(
        controller,
        maximum=MAX_CONTROLLER_BYTES,
        uid=layout.uid,
        label="controller",
        exact_mode=0o600,
    )
    python_record = manifest.get("local_testnet_python")
    assert isinstance(python_record, dict)
    if python_record.get("path") != str(layout.local_python):
        fail("prepared reset names an unexpected local Python path")
    try:
        resolved_python = layout.local_python.resolve(strict=True)
    except OSError as error:
        raise ActivationRenderError("local Python cannot be resolved") from error
    python_identity, _ = read_or_hash_regular(
        resolved_python,
        maximum=MAX_BINARY_BYTES,
        uid=layout.uid,
        label="local Python",
        require_executable=True,
    )
    if python_identity.sha256 != require_sha256(
        python_record.get("sha256"), "local Python SHA-256"
    ):
        fail("local Python differs from the prepared reset binding")

    command = [
        str(layout.local_python),
        "-I",
        "-B",
        "-S",
        str(controller),
        "--manifest",
        str(activation_path),
    ]
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            process = subprocess.Popen(
                command,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                cwd="/",
                env={"LANG": "C", "LC_ALL": "C"},
                close_fds=True,
                start_new_session=True,
            )
        except OSError as error:
            raise ActivationRenderError("controller dry-run could not start") from error
        deadline = time.monotonic() + DRY_RUN_TIMEOUT_SECONDS
        while process.poll() is None:
            if (
                time.monotonic() >= deadline
                or os.fstat(stdout.fileno()).st_size > MAX_DRY_RUN_STDOUT_BYTES
                or os.fstat(stderr.fileno()).st_size > MAX_DRY_RUN_STDERR_BYTES
            ):
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                fail("controller dry-run exceeded its runtime or output bound")
            time.sleep(0.02)
        stdout_body = _read_bounded_process_output(
            stdout, MAX_DRY_RUN_STDOUT_BYTES, "stdout"
        )
        stderr_body = _read_bounded_process_output(
            stderr, MAX_DRY_RUN_STDERR_BYTES, "stderr"
        )
    if process.returncode != 0:
        detail = stderr_body.decode("utf-8", errors="replace")[:1024].strip()
        fail(f"controller dry-run failed with exit {process.returncode}: {detail}")
    projection = parse_json_object(stdout_body, "controller dry-run projection")
    if stderr_body:
        fail("controller dry-run emitted unexpected stderr")
    if set(projection) != DRY_RUN_KEYS:
        fail("controller dry-run projection keys are not exact")
    expected_confirmation = f"RESET-TAIRA-USER-501:{activation_sha256}"
    expected = {
        "schema": DRY_RUN_SCHEMA,
        "mode": "dry-run",
        "activation_manifest": str(activation_path),
        "activation_manifest_sha256": activation_sha256,
        "confirmation_required": expected_confirmation,
        "launchctl_domain": DOMAIN,
        "labels": list(LABELS),
        "bundle": activation["bundle"],
        "binary": activation["binary"],
        "binary_sha256": activation["binary_sha256"],
        "genesis_native_verifier": activation["genesis_native_verifier"],
        "genesis_native_verifier_sha256": activation[
            "genesis_native_verifier_sha256"
        ],
        "operator_status_client": activation["operator_status_client"],
        "operator_status_client_sha256": activation[
            "operator_status_client_sha256"
        ],
        "genesis_external_signer_sha256": activation[
            "genesis_external_signer_sha256"
        ],
        "genesis_public_key": activation["genesis_public_key"],
        "genesis_expected_hash": activation["genesis_expected_hash"],
        "genesis_artifact_linkage_sha256": activation[
            "genesis_artifact_linkage_sha256"
        ],
        "mutated": False,
    }
    for key, value in expected.items():
        if projection.get(key) != value:
            fail(f"controller dry-run projection differs at {key}")
    network_id = projection.get("network_id")
    if (
        not isinstance(network_id, str)
        or re.fullmatch(r"hash:[0-9A-F]{64}#[0-9A-F]{4}", network_id) is None
    ):
        fail("controller dry-run returned a noncanonical NetworkId")
    revalidate(controller_identity, "controller")
    revalidate(python_identity, "local Python")
    for identity in closure_identities:
        revalidate(identity, f"staged source closure {identity.path}")
    verify_source_closure(
        root=closure_root,
        manifest=manifest,
        uid=layout.uid,
    )
    return {
        "validated": True,
        "network_id": network_id,
        "confirmation_required": expected_confirmation,
    }


def atomic_publish(
    *,
    output: Path,
    body: bytes,
    identities: Sequence[tuple[FileIdentity, str]],
    uid: int,
    validate: Callable[[Path], dict[str, object]] | None = None,
) -> dict[str, object] | None:
    if output.exists() or output.is_symlink():
        fail("activation output already exists; replacement is forbidden")
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.next-", dir=output.parent
    )
    temporary = Path(temporary_name)
    validation: dict[str, object] | None = None
    try:
        os.fchmod(descriptor, 0o600)
        os.fchown(descriptor, uid, os.getgid())
        with os.fdopen(descriptor, "wb", closefd=True) as stream:
            stream.write(body)
            stream.flush()
            os.fsync(stream.fileno())
        temporary_identity, temporary_body = read_or_hash_regular(
            temporary,
            maximum=64 * 1024,
            uid=uid,
            label="temporary activation",
            exact_mode=0o600,
            return_body=True,
        )
        if temporary_body != body:
            fail("temporary activation bytes changed")
        if validate is not None:
            validation = validate(temporary)
        for identity, label in identities:
            revalidate(identity, label)
        revalidate(temporary_identity, "temporary activation")
        try:
            os.link(temporary, output, follow_symlinks=False)
        except FileExistsError as error:
            raise ActivationRenderError(
                "activation output appeared concurrently; replacement is forbidden"
            ) from error
        temporary.unlink()
        directory_fd = os.open(
            output.parent,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if temporary.exists() or temporary.is_symlink():
            temporary.unlink()
    final_identity, final_body = read_or_hash_regular(
        output,
        maximum=64 * 1024,
        uid=uid,
        label="activation output",
        exact_mode=0o600,
        return_body=True,
    )
    if final_body != body:
        fail("published activation bytes changed")
    if final_identity.metadata[:2] != temporary_identity.metadata[:2]:
        fail("published activation is not the verified temporary inode")
    return validation


def render(
    args: argparse.Namespace,
    *,
    layout: Layout = PRODUCTION_LAYOUT,
) -> dict[str, object]:
    if os.getuid() != layout.uid or os.geteuid() != layout.uid:
        fail(f"activation renderer must run as uid {layout.uid}")
    generation = args.generation
    if not isinstance(generation, str) or GENERATION_RE.fullmatch(generation) is None:
        fail("generation is not canonical")
    bundle = require_absolute(args.bundle, "bundle")
    release_root = require_absolute(args.release_root, "release root")
    output = require_absolute(args.output, "activation output")
    if relative_descendant(
        bundle, layout.reset_bundles, "bundle", exact_parts=1
    ) != (generation,):
        fail("bundle child does not equal the exact generation")
    release_parts = relative_descendant(
        release_root, layout.releases, "release root", exact_parts=1
    )
    if GENERATION_RE.fullmatch(release_parts[0]) is None:
        fail("release child is not canonical")
    if relative_descendant(
        output, layout.reset_manifests, "activation output", exact_parts=1
    ) != (f"{generation}.json",):
        fail("activation output name does not equal the exact generation")

    for path, label in (
        (layout.taira_root, "Taira root"),
        (layout.reset_bundles, "reset bundle root"),
        (layout.reset_manifests, "reset manifest root"),
        (layout.releases, "release root parent"),
        (bundle, "prepared reset bundle"),
        (release_root, "candidate release root"),
    ):
        require_private_directory(path, layout.uid, label)
    for path, label in (
        (bundle, "bundle"),
        (release_root, "release root"),
        (output, "activation output"),
    ):
        require_no_symlink_ancestry(path, layout.taira_root, label)

    reset_manifest_path = bundle / "reset-manifest.json"
    reset_identity, reset_body = read_or_hash_regular(
        reset_manifest_path,
        maximum=MAX_RESET_MANIFEST_BYTES,
        uid=layout.uid,
        label="prepared reset manifest",
        exact_mode=0o600,
        return_body=True,
    )
    assert reset_body is not None
    manifest = parse_json_object(reset_body, "prepared reset manifest")
    if canonical_artifact_json(manifest) != reset_body:
        fail("prepared reset manifest is not in its canonical byte form")
    local_python_record = manifest.get("local_testnet_python")
    if (
        not isinstance(local_python_record, dict)
        or local_python_record.get("path") != str(layout.local_python)
    ):
        fail("prepared reset names an unexpected local Python path")
    try:
        resolved_python = layout.local_python.resolve(strict=True)
        running_python = Path(sys.executable).resolve(strict=True)
    except OSError as error:
        raise ActivationRenderError("local Python cannot be resolved") from error
    if running_python != resolved_python:
        fail("activation renderer is not running under the reset-bound local Python")
    local_python_identity, _ = read_or_hash_regular(
        resolved_python,
        maximum=MAX_BINARY_BYTES,
        uid=layout.uid,
        label="local Python",
        require_executable=True,
    )
    if local_python_identity.sha256 != require_sha256(
        local_python_record.get("sha256"), "local Python SHA-256"
    ):
        fail("local Python differs from the prepared reset binding")
    binary_bindings = (
        ("iroha3d", "irohad_sha256", "iroha3d"),
        ("kagami", "genesis_native_verifier_sha256", "Kagami"),
        (
            "taira_operator_status",
            "operator_status_client_sha256",
            "operator status client",
        ),
    )
    identities: list[tuple[FileIdentity, str]] = [
        (reset_identity, "prepared reset manifest"),
        (local_python_identity, "local Python"),
    ]
    for filename, manifest_key, label in binary_bindings:
        path = release_root / filename
        identity, _ = read_or_hash_regular(
            path,
            maximum=MAX_BINARY_BYTES,
            uid=layout.uid,
            label=label,
            require_executable=True,
        )
        if identity.sha256 != require_sha256(manifest.get(manifest_key), f"{label} SHA-256"):
            fail(f"{label} differs from the prepared reset binding")
        identities.append((identity, label))

    linkage = manifest.get("genesis_artifact_linkage")
    if not isinstance(linkage, dict):
        fail("prepared reset lacks its genesis artifact linkage")
    bundle_artifacts = (
        ("nevo-reset.review.json", "review_sha256", "NEVO review"),
        (
            "genesis.reviewed-unsigned.json",
            "reviewed_unsigned_genesis_sha256",
            "reviewed unsigned genesis",
        ),
        (
            "genesis.pre-sign-rendered.json",
            "pre_sign_rendered_genesis_sha256",
            "pre-sign rendered genesis",
        ),
        ("genesis.json", "bound_genesis_manifest_sha256", "bound genesis manifest"),
        ("genesis.signed.nrt", "signed_genesis_sha256", "signed genesis"),
    )
    observed_artifacts: dict[str, FileIdentity] = {}
    nevo_review_body: bytes | None = None
    for filename, linkage_key, label in bundle_artifacts:
        identity, artifact_body = read_or_hash_regular(
            bundle / filename,
            maximum=MAX_GENESIS_BYTES,
            uid=layout.uid,
            label=label,
            exact_mode=0o600,
            return_body=filename == "nevo-reset.review.json",
        )
        if identity.sha256 != require_sha256(linkage.get(linkage_key), f"{label} SHA-256"):
            fail(f"{label} differs from the prepared reset linkage")
        identities.append((identity, label))
        observed_artifacts[filename] = identity
        if filename == "nevo-reset.review.json":
            nevo_review_body = artifact_body
    base_config_identity, _ = read_or_hash_regular(
        bundle / "base-config.toml",
        maximum=MAX_GENESIS_BYTES,
        uid=layout.uid,
        label="base config",
        exact_mode=0o600,
    )
    identities.append((base_config_identity, "base config"))
    privacy_release = manifest.get("privacy_bootstrap_release")
    assert isinstance(privacy_release, dict)
    reviewed_inputs = privacy_release.get("reviewed_inputs")
    assert isinstance(reviewed_inputs, dict)
    reviewed_artifacts = {
        "config.toml": base_config_identity,
        "genesis.json": observed_artifacts["genesis.reviewed-unsigned.json"],
        "nevo-reset.review.json": observed_artifacts["nevo-reset.review.json"],
    }
    for name, identity in reviewed_artifacts.items():
        row = reviewed_inputs[name]
        assert isinstance(row, dict)
        if row.get("sha256") != identity.sha256 or row.get("size") != identity.metadata[6]:
            fail(f"local reviewed input differs from the bundle: {name}")
    assert nevo_review_body is not None
    nevo_review = parse_json_object(nevo_review_body, "NEVO reset review")
    expected_review_record = {
        "schema": nevo_review.get("schema"),
        "sha256": observed_artifacts["nevo-reset.review.json"].sha256,
        "public_inputs_sha256": nevo_review.get("public_inputs_sha256"),
        "unsigned_genesis_sha256": nevo_review.get("unsigned_genesis_sha256"),
        "public_identities": nevo_review.get("public_identities"),
        "credential_hash_bindings": nevo_review.get("credential_hash_bindings"),
    }
    if privacy_release.get("nevo_reset_review") != expected_review_record:
        fail("local privacy release NEVO review projection is inconsistent")

    activation = build_activation(
        generation=generation,
        bundle=bundle,
        release_root=release_root,
        manifest=manifest,
        reset_manifest_sha256=reset_identity.sha256,
    )
    body = canonical_activation_json(activation)
    if len(body) > 64 * 1024:
        fail("activation output exceeds its byte bound")
    activation_sha256 = hashlib.sha256(body).hexdigest()

    validator = None
    if args.validate_with_controller is not None:
        controller = require_absolute(args.validate_with_controller, "controller")

        def validator(path: Path) -> dict[str, object]:
            return validate_controller_dry_run(
                controller=controller,
                activation_path=path,
                activation_sha256=activation_sha256,
                activation=activation,
                manifest=manifest,
                layout=layout,
            )

    validation = atomic_publish(
        output=output,
        body=body,
        identities=identities,
        uid=layout.uid,
        validate=validator,
    )
    confirmation = f"RESET-TAIRA-USER-501:{activation_sha256}"
    return {
        "schema": RECEIPT_SCHEMA,
        "generation": generation,
        "activation_manifest": str(output),
        "activation_manifest_sha256": activation_sha256,
        "reset_manifest": str(reset_manifest_path),
        "reset_manifest_sha256": reset_identity.sha256,
        "confirmation_required": confirmation,
        "controller_dry_run_validated": validation is not None,
        "network_id": validation.get("network_id") if validation else None,
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    result.add_argument("--generation", required=True)
    result.add_argument("--bundle", required=True, type=Path)
    result.add_argument("--release-root", required=True, type=Path)
    result.add_argument("--output", required=True, type=Path)
    result.add_argument(
        "--validate-with-controller",
        type=Path,
        help="run this exact staged controller's default read-only plan before publish",
    )
    return result


def main(argv: Sequence[str] | None = None) -> int:
    try:
        receipt = render(parser().parse_args(argv))
    except ActivationRenderError as error:
        print(f"error: {error}", file=sys.stderr)
        return 70
    print(json.dumps(receipt, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
