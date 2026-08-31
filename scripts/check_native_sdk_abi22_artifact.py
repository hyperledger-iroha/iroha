#!/usr/bin/env python3
"""Record and verify fail-closed ABI-22 native SDK artifact evidence.

This checker is intentionally host-only.  It authenticates the exact native
artifact exercised by a Node, Python, C/JNI, or C# test lane, calls that
artifact's ABI probe, verifies its required entrypoints, enforces the exact
privacy C export inventory for bridge-bearing lanes, and binds the result to
the artifact bytes plus one canonical clean-source manifest. Apple and Android
release packages continue to use ``check_mobile_sdk_artifacts.sh``, which
additionally authenticates every cross-compiled slice and its transitive source
seal.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
import re
import shutil
import stat
import subprocess
import sys
from collections import Counter
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import NoReturn

if __package__:
    from .compute_workspace_source_manifest import workspace_source_manifest
else:
    # CI deliberately invokes this checker with ``python -I -S``.  Isolated
    # mode removes the script directory from ``sys.path``, so bind the one
    # required sibling by its exact path instead of weakening isolation.
    _manifest_helper_path = Path(__file__).resolve(strict=True).with_name(
        "compute_workspace_source_manifest.py"
    )
    _manifest_helper_spec = importlib.util.spec_from_file_location(
        "_iroha_compute_workspace_source_manifest",
        _manifest_helper_path,
    )
    if _manifest_helper_spec is None or _manifest_helper_spec.loader is None:
        raise ImportError(
            f"unable to load workspace source manifest helper at {_manifest_helper_path}"
        )
    _manifest_helper = importlib.util.module_from_spec(_manifest_helper_spec)
    _manifest_helper_spec.loader.exec_module(_manifest_helper)
    workspace_source_manifest = _manifest_helper.workspace_source_manifest


SCHEMA = "iroha.native-sdk-abi22-artifact.v1"
REQUIRED_BRIDGE_ABI_VERSION = 22
MAX_MANIFEST_BYTES = 64 * 1024
MAX_SYMBOL_TOOL_OUTPUT_BYTES = 16 * 1024 * 1024
MAX_EXPORTED_SYMBOLS = 1_000_000
MAX_EVIDENCE_DIRECTORY_PATH_BYTES = 4 * 1024
MAX_EVIDENCE_DIRECTORY_COMPONENTS = 64
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
TARGET_RE = re.compile(r"[a-z0-9][a-z0-9._+-]{0,127}")
SDK_VALUES = frozenset({"c-jni", "csharp", "node", "python"})
EXACT_PRIVACY_C_EXPORT_SDKS = frozenset({"c-jni", "csharp"})

APPROVED_PRIVACY_C_EXPORTS = (
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
)
STALE_PRIVACY_ABI_MARKER_RE = re.compile(
    r"(?:abi[_-]?(?:21|23)|(?:^|_)v(?:21|23)(?:$|_))",
    re.IGNORECASE,
)
DUMPBIN_EXPORT_RE = re.compile(
    rb"^\s*\d+\s+[0-9a-f]+\s+[0-9a-f]+\s+(\S+)(?:\s+.*)?$",
    re.IGNORECASE,
)

REQUIRED_SYMBOLS: Mapping[str, tuple[str, ...]] = {
    "c-jni": (
        "connect_norito_bridge_abi_version",
        "connect_norito_free",
        "connect_norito_offline_cash_payment_request_canonicalize_v1",
        "connect_norito_offline_cash_payment_canonicalize_v1",
        "connect_norito_offline_cash_payment_canonicalize_for_session_v1",
        "connect_norito_offline_cash_acknowledgement_canonicalize_v1",
        "connect_norito_offline_cash_peer_encode_payment_request_v1",
        "connect_norito_offline_cash_peer_decode_payment_request_v1",
        "connect_norito_offline_cash_peer_encode_payment_v1",
        "connect_norito_offline_cash_peer_decode_payment_v1",
        "connect_norito_offline_cash_peer_encode_acknowledgement_v1",
        "connect_norito_offline_cash_peer_decode_acknowledgement_v1",
        "connect_norito_offline_cash_release_probe_v1",
        "connect_norito_offline_cash_artifact_begin_v1",
        "connect_norito_offline_cash_artifact_write_v1",
        "connect_norito_offline_cash_artifact_finalize_v1",
        "connect_norito_offline_cash_artifact_cancel_v1",
        "connect_norito_offline_cash_artifact_set_install_v1",
        "connect_norito_offline_cash_artifact_set_uninstall_v1",
        "connect_norito_offline_cash_verification_session_open_v1",
        "connect_norito_offline_cash_verification_session_open_bound_v1",
        "connect_norito_offline_cash_verification_session_verify_payment_v1",
        "connect_norito_offline_cash_verification_session_verify_acknowledgement_v1",
        "connect_norito_offline_cash_verification_session_state_v1",
        "connect_norito_offline_cash_verification_session_close_v1",
        "connect_norito_offline_cash_wallet_runtime_session_open_v1",
        "connect_norito_offline_cash_wallet_runtime_session_status_v1",
        "connect_norito_offline_cash_wallet_runtime_session_attempt_v1",
        "connect_norito_offline_cash_wallet_runtime_session_close_v1",
        # Deprecated ABI22 verifier-only aliases remain required for compatibility.
        "connect_norito_offline_cash_wallet_session_open_v1",
        "connect_norito_offline_cash_wallet_session_open_bound_v1",
        "connect_norito_offline_cash_wallet_session_accept_payment_v1",
        "connect_norito_offline_cash_wallet_session_accept_acknowledgement_v1",
        "connect_norito_offline_cash_wallet_session_state_v1",
        "connect_norito_offline_cash_wallet_session_close_v1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizePaymentRequestV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizePaymentV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizePaymentForSessionV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizeAcknowledgementV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerEncodePaymentRequestV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerDecodePaymentRequestV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerEncodePaymentV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerDecodePaymentV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerEncodeAcknowledgementV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerDecodeAcknowledgementV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeReleaseProbeV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionOpenV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionOpenBoundV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionVerifyPaymentV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionVerifyAcknowledgementV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionStateV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionCloseV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionOpenV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionStatusV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionAttemptV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionCloseV1",
        # Deprecated ABI22 verifier-only JNI aliases remain required for compatibility.
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionOpenV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionOpenBoundV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionAcceptPaymentV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionAcceptAcknowledgementV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionStateV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionCloseV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactBeginV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactWriteV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactFinalizeV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactCancelV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactSetInstallV1",
        "Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactSetUninstallV1",
        "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
    ),
    "csharp": (
        "connect_norito_bridge_abi_version",
        "connect_norito_free",
        "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
        "iroha_privacy_compiled_profile_catalog_v1",
        "iroha_privacy_validate_compiled_profile_catalog_v1",
        "iroha_privacy_exact12_fixture_bundle_v1",
        "iroha_privacy_validate_exact12_fixture_bundle_v1",
        "iroha_privacy_free_buffer",
    ),
    "node": (
        "connectNoritoBridgeAbiVersion",
        "inspectSorafsOrderbookSubmissionForDiscriminantV1",
        "sorafsValidateAppealFinanceCancelAssetLockJson",
        "verifySorafsOrderbookSubmissionReceiptV1",
    ),
    "python": (
        "connect_norito_bridge_abi_version",
        "inspect_sorafs_orderbook_submission_for_discriminant_v1",
        "sorafs_validate_appeal_finance_cancel_asset_lock_json",
        "verify_sorafs_orderbook_submission_receipt_v1",
    ),
}


class ArtifactContractError(RuntimeError):
    """Raised when native SDK artifact evidence is incomplete or stale."""


def fail(message: str) -> NoReturn:
    """Raise one stable checker error."""

    raise ArtifactContractError(message)


def _plain_object(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        fail(f"{label} must be a JSON object")
    return value


def _reject_duplicate_object_pairs(
    pairs: list[tuple[str, object]],
) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail(f"native artifact manifest contains duplicate key {key!r}")
        result[key] = value
    return result


def _run_git(root: Path, arguments: Sequence[str]) -> str:
    environment = os.environ.copy()
    environment["GIT_CONFIG_GLOBAL"] = os.devnull
    environment["GIT_CONFIG_NOSYSTEM"] = "1"
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    result = subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
        env=environment,
    )
    if result.returncode != 0:
        detail = result.stderr.strip()
        fail(
            "native artifact source state could not be authenticated"
            + (f": {detail}" if detail else "")
        )
    return result.stdout


def source_state(root: Path) -> tuple[str, bool]:
    """Return the exact Git commit and whole-tree cleanliness."""

    commit = _run_git(root, ("rev-parse", "--verify", "HEAD")).strip()
    if COMMIT_RE.fullmatch(commit) is None:
        fail("native artifact source commit is not canonical lowercase Git SHA-1")
    status = _run_git(
        root,
        ("status", "--porcelain=v1", "--untracked-files=all"),
    )
    return commit, not bool(status.strip())


def workspace_source_manifest_sha256(root: Path) -> str:
    """Compute the repository's canonical checkout source-manifest digest."""

    try:
        digest = workspace_source_manifest(root)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise ArtifactContractError(
            "native artifact workspace source manifest could not be authenticated"
        ) from error
    if SHA256_RE.fullmatch(digest) is None or digest == "0" * 64:
        fail("native artifact workspace source manifest SHA-256 is not canonical")
    return digest


def stage_artifact(source: Path, destination: Path) -> Path:
    """Copy one stable build output into a fresh private single-link inode.

    Cargo may hard-link its top-level dynamic-library output to the matching
    artifact under ``deps/``.  That is a valid build-cache implementation
    detail, but an artifact admitted to the release evidence boundary must
    have exactly one name.  Copy through authenticated file descriptors so
    the strict evidence checker can retain its one-hard-link invariant.
    """

    if not source.is_absolute() or not destination.is_absolute():
        fail("native artifact staging paths must be absolute")
    try:
        before = source.lstat()
    except OSError as error:
        raise ArtifactContractError(
            f"native build output is unavailable: {source}"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink < 1
        or before.st_size <= 0
    ):
        fail("native build output must be one non-empty non-symbolic regular file")

    try:
        parent = destination.parent.resolve(strict=True)
        parent_metadata = parent.lstat()
    except OSError as error:
        raise ArtifactContractError(
            f"native artifact staging directory is unavailable: {destination.parent}"
        ) from error
    if not stat.S_ISDIR(parent_metadata.st_mode) or stat.S_ISLNK(
        parent_metadata.st_mode
    ):
        fail("native artifact staging parent must resolve to a real directory")
    if destination.name in {"", ".", ".."}:
        fail("native artifact staging output must name one file")
    output = parent / destination.name

    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(
        os, "O_NOFOLLOW", 0
    )
    output_flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        source_descriptor = os.open(source, source_flags)
    except OSError as error:
        raise ArtifactContractError(
            f"native build output could not be opened: {source}"
        ) from error

    output_descriptor: int | None = None
    output_identity: tuple[int, int] | None = None
    source_digest = hashlib.sha256()
    try:
        opened = os.fstat(source_descriptor)
        before_identity = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
            before.st_nlink,
        )
        opened_identity = (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            opened.st_nlink,
        )
        if opened_identity != before_identity:
            fail("native build output changed while it was opened")
        try:
            output_descriptor = os.open(output, output_flags, 0o600)
        except OSError as error:
            raise ArtifactContractError(
                f"native artifact staging output must be fresh: {output}"
            ) from error
        created = os.fstat(output_descriptor)
        output_identity = (created.st_dev, created.st_ino)
        if (
            not stat.S_ISREG(created.st_mode)
            or stat.S_IMODE(created.st_mode) != 0o600
            or created.st_nlink != 1
            or created.st_size != 0
        ):
            fail("native artifact staging output was not created privately")

        copied = 0
        while True:
            chunk = os.read(source_descriptor, 1024 * 1024)
            if not chunk:
                break
            source_digest.update(chunk)
            offset = 0
            while offset < len(chunk):
                offset += os.write(output_descriptor, chunk[offset:])
            copied += len(chunk)
        os.fsync(output_descriptor)

        source_after = os.fstat(source_descriptor)
        source_after_identity = (
            source_after.st_dev,
            source_after.st_ino,
            source_after.st_mode,
            source_after.st_size,
            source_after.st_mtime_ns,
            source_after.st_ctime_ns,
            source_after.st_nlink,
        )
        written = os.fstat(output_descriptor)
        current_output = output.lstat()
        if source_after_identity != opened_identity:
            fail("native build output changed while it was staged")
        if copied != opened.st_size:
            fail("native build output size changed while it was staged")
        if (
            not stat.S_ISREG(written.st_mode)
            or stat.S_IMODE(written.st_mode) != 0o600
            or written.st_nlink != 1
            or written.st_size != copied
            or stat.S_ISLNK(current_output.st_mode)
            or (written.st_dev, written.st_ino)
            != (current_output.st_dev, current_output.st_ino)
        ):
            fail("staged native artifact changed while it was written")
    except BaseException:
        if output_descriptor is not None and output_identity is not None:
            try:
                current = output.lstat()
                if (current.st_dev, current.st_ino) == output_identity:
                    output.unlink()
            except OSError:
                pass
        raise
    finally:
        if output_descriptor is not None:
            os.close(output_descriptor)
        os.close(source_descriptor)

    staged_digest, staged_size = stable_artifact_identity(output)
    if staged_digest != source_digest.hexdigest() or staged_size != before.st_size:
        fail("staged native artifact bytes differ from the fresh build output")
    return output


def stable_artifact_identity(path: Path) -> tuple[str, int]:
    """Hash one regular, non-linked file while detecting replacement races."""

    try:
        before = path.lstat()
    except OSError as error:
        raise ArtifactContractError(
            f"native artifact is unavailable: {path}"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
    ):
        fail("native artifact must be one non-empty regular file with one hard link")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ArtifactContractError(f"native artifact could not be opened: {path}") from error
    digest = hashlib.sha256()
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            fail("native artifact changed while it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    identity_before = (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_nlink,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        after.st_nlink,
    )
    if identity_before != identity_after:
        fail("native artifact changed while it was hashed")
    return digest.hexdigest(), opened.st_size


def stable_bounded_file_bytes(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
) -> bytes:
    """Read one bounded regular file without following or racing replacements."""

    try:
        before = path.lstat()
    except OSError as error:
        raise ArtifactContractError(f"{label} is unavailable: {path}") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum_bytes
    ):
        fail(f"{label} must be one bounded regular file with one hard link")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ArtifactContractError(f"{label} could not be opened: {path}") from error
    chunks: list[bytes] = []
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            fail(f"{label} changed while it was opened")
        remaining = maximum_bytes + 1
        while remaining:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    def identity(metadata: os.stat_result) -> tuple[int, ...]:
        return (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mode,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
            metadata.st_nlink,
        )

    try:
        current = path.lstat()
    except OSError as error:
        raise ArtifactContractError(f"{label} changed while it was read") from error
    opened_identity = identity(opened)
    if (
        opened_identity != identity(after)
        or opened_identity != identity(current)
        or not stat.S_ISREG(opened.st_mode)
        or stat.S_ISLNK(opened.st_mode)
        or opened.st_nlink != 1
    ):
        fail(f"{label} changed while it was read")
    raw = b"".join(chunks)
    if len(raw) != opened.st_size or len(raw) > maximum_bytes:
        fail(f"{label} changed size or exceeded its byte limit while it was read")
    return raw


def _symbol_tool_commands(path: Path) -> tuple[tuple[str, tuple[str, ...], str], ...]:
    rendered = str(path)
    if sys.platform == "darwin":
        return (
            ("nm", ("-gUj", rendered), "macho-lines"),
            (
                "llvm-nm",
                ("--defined-only", "--extern-only", "-j", rendered),
                "macho-lines",
            ),
        )
    if os.name == "nt":
        return (
            (
                "llvm-nm",
                ("--defined-only", "--extern-only", "-j", rendered),
                "lines",
            ),
            ("dumpbin", ("/nologo", "/exports", rendered), "dumpbin"),
        )
    return (
        ("nm", ("-D", "--defined-only", "-j", rendered), "lines"),
        (
            "llvm-nm",
            ("--defined-only", "--extern-only", "-j", rendered),
            "lines",
        ),
    )


def _parse_symbol_tool_output(raw: bytes, output_format: str) -> tuple[str, ...]:
    if len(raw) > MAX_SYMBOL_TOOL_OUTPUT_BYTES:
        fail("native artifact exported-symbol inventory exceeds its byte limit")
    symbols: list[str] = []
    for line in raw.splitlines():
        if output_format == "dumpbin":
            match = DUMPBIN_EXPORT_RE.fullmatch(line)
            if match is None:
                continue
            encoded = match.group(1)
        else:
            encoded = line.strip()
            if not encoded:
                continue
            if output_format == "macho-lines" and encoded.startswith(b"_"):
                encoded = encoded[1:]
        try:
            symbol = encoded.decode("ascii")
        except UnicodeDecodeError as error:
            raise ArtifactContractError(
                "native artifact exported-symbol inventory is not ASCII"
            ) from error
        symbols.append(symbol)
        if len(symbols) > MAX_EXPORTED_SYMBOLS:
            fail("native artifact exported-symbol inventory exceeds its entry limit")
    return tuple(symbols)


def inspect_exported_symbols(path: Path, *, required: bool) -> tuple[str, ...] | None:
    """Read a native binary's export table with the host platform's tooling."""

    failures: list[str] = []
    for tool, arguments, output_format in _symbol_tool_commands(path):
        executable = shutil.which(tool)
        if executable is None:
            continue
        environment = os.environ.copy()
        environment["LC_ALL"] = "C"
        try:
            result = subprocess.run(
                [executable, *arguments],
                check=False,
                capture_output=True,
                timeout=30,
                env=environment,
            )
        except (OSError, subprocess.SubprocessError) as error:
            failures.append(f"{tool}: {error}")
            continue
        if result.returncode != 0:
            detail = result.stderr.decode("utf-8", errors="replace").strip()[:1024]
            failures.append(f"{tool}: {detail or f'exit {result.returncode}'}")
            continue
        return _parse_symbol_tool_output(result.stdout, output_format)
    if required:
        detail = "; ".join(failures[:2])
        fail(
            "native bridge exported-symbol inventory could not be inspected"
            + (f": {detail}" if detail else ": no supported symbol tool is available")
        )
    return None


def validate_privacy_c_exports(
    symbols: Sequence[str],
    *,
    require_exact: bool,
) -> tuple[str, ...]:
    """Reject duplicate, stale, or unexpected native privacy C exports."""

    observed: list[str] = []
    for symbol in symbols:
        if type(symbol) is not str or not symbol or "\x00" in symbol:
            fail("native artifact exported-symbol inventory is malformed")
        lowered = symbol.lower()
        if (
            "privacy" in lowered or "connect_norito_bridge" in lowered
        ) and STALE_PRIVACY_ABI_MARKER_RE.search(lowered):
            fail(f"native artifact exports stale privacy/bridge ABI marker: {symbol}")
        if symbol.startswith("iroha_privacy_"):
            observed.append(symbol)
        elif "iroha_privacy_" in symbol:
            fail(f"native artifact exports decorated privacy C symbol variant: {symbol}")

    duplicates = sorted(
        symbol for symbol, count in Counter(observed).items() if count != 1
    )
    if duplicates:
        fail(
            "native artifact exports duplicate privacy C symbols: "
            + ", ".join(duplicates)
        )
    unexpected = sorted(set(observed) - set(APPROVED_PRIVACY_C_EXPORTS))
    if unexpected:
        fail(
            "native artifact exports unexpected privacy C symbols: "
            + ", ".join(unexpected)
        )
    if require_exact:
        missing = [
            symbol for symbol in APPROVED_PRIVACY_C_EXPORTS if symbol not in observed
        ]
        if missing:
            fail(
                "native bridge artifact is missing approved privacy C symbols: "
                + ", ".join(missing)
            )
    return tuple(symbol for symbol in APPROVED_PRIVACY_C_EXPORTS if symbol in observed)


def probe_c_abi(path: Path, required_symbols: Sequence[str]) -> int:
    """Load one C ABI library and require its exact exported inventory."""

    try:
        library = ctypes.CDLL(str(path))
    except OSError as error:
        raise ArtifactContractError(f"native C ABI artifact could not be loaded: {path}") from error
    missing = [symbol for symbol in required_symbols if not hasattr(library, symbol)]
    if missing:
        fail("native C ABI artifact is missing required symbols: " + ", ".join(missing))
    probe = getattr(library, "connect_norito_bridge_abi_version")
    probe.argtypes = []
    probe.restype = ctypes.c_uint32
    return int(probe())


def _probe_subprocess(
    command: Sequence[str],
    *,
    label: str,
) -> int:
    result = subprocess.run(
        list(command),
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        detail = result.stderr.strip()[:4096]
        fail(f"{label} failed" + (f": {detail}" if detail else ""))
    raw = result.stdout.strip()
    if not raw.isascii() or not raw.isdecimal():
        fail(f"{label} returned a noncanonical ABI version")
    return int(raw)


def probe_node_abi(
    path: Path,
    required_symbols: Sequence[str],
    *,
    node: str = "node",
) -> int:
    """Load one Node addon and call its exact ABI-22 probe."""

    source = r"""
const artifact = process.argv[1];
const required = JSON.parse(process.argv[2]);
let binding;
if (/\.(?:cjs|js)$/iu.test(artifact)) {
  binding = require(artifact);
} else {
  const nativeModule = { exports: {} };
  process.dlopen(nativeModule, artifact);
  binding = nativeModule.exports;
}
const missing = required.filter((name) => typeof binding[name] !== "function");
if (missing.length !== 0) {
  process.stderr.write("missing required exports: " + missing.join(", "));
  process.exit(2);
}
const version = binding.connectNoritoBridgeAbiVersion();
if (!Number.isSafeInteger(version) || version < 0) {
  process.stderr.write("ABI probe returned a non-integer");
  process.exit(3);
}
process.stdout.write(String(version));
"""
    return _probe_subprocess(
        (node, "--eval", source, str(path), json.dumps(list(required_symbols))),
        label="native Node ABI probe",
    )


def probe_python_abi(
    path: Path,
    required_symbols: Sequence[str],
    *,
    python: str = sys.executable,
) -> int:
    """Load one Python extension directly and call its exact ABI-22 probe."""

    source = r"""
import importlib.machinery
import importlib.util
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
required = json.loads(sys.argv[2])
if path.suffix == ".py":
    name = "_iroha_native_abi22_fixture"
    loader = importlib.machinery.SourceFileLoader(name, str(path))
else:
    name = "iroha_python._crypto"
    loader = importlib.machinery.ExtensionFileLoader(name, str(path))
spec = importlib.util.spec_from_loader(name, loader)
if spec is None:
    raise SystemExit("native extension has no import specification")
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)
missing = [name for name in required if not callable(getattr(module, name, None))]
if missing:
    raise SystemExit("missing required exports: " + ", ".join(missing))
version = module.connect_norito_bridge_abi_version()
if type(version) is not int or version < 0:
    raise SystemExit("ABI probe returned a non-integer")
print(version, end="")
"""
    return _probe_subprocess(
        (python, "-I", "-c", source, str(path), json.dumps(list(required_symbols))),
        label="native Python ABI probe",
    )


def probe_artifact(
    sdk: str,
    path: Path,
    *,
    node: str = "node",
    python: str = sys.executable,
) -> int:
    """Probe the exact host artifact selected for one SDK lane."""

    required = REQUIRED_SYMBOLS[sdk]
    if sdk == "node":
        return probe_node_abi(path, required, node=node)
    if sdk == "python":
        return probe_python_abi(path, required, python=python)
    return probe_c_abi(path, required)


def _require_exact_abi(version: int) -> None:
    if type(version) is not int or version != REQUIRED_BRIDGE_ABI_VERSION:
        fail(
            "native artifact bridge ABI must be exactly "
            f"{REQUIRED_BRIDGE_ABI_VERSION}; found {version!r}"
        )


def build_manifest(
    *,
    sdk: str,
    target: str,
    artifact_path: Path,
    source_root: Path,
    probe: Callable[[str, Path], int] = probe_artifact,
    symbol_inventory: Callable[[Path], Sequence[str] | None] | None = None,
) -> dict[str, object]:
    """Authenticate one artifact and return its canonical evidence manifest."""

    if sdk not in SDK_VALUES:
        fail(f"unsupported native SDK lane: {sdk!r}")
    if TARGET_RE.fullmatch(target) is None:
        fail("native SDK target must be a bounded lowercase target token")
    commit_before, clean_before = source_state(source_root)
    if not clean_before:
        fail("native SDK artifacts must be built and tested from a clean source tree")
    source_manifest_before = workspace_source_manifest_sha256(source_root)
    digest, size = stable_artifact_identity(artifact_path)
    version = probe(sdk, artifact_path)
    _require_exact_abi(version)
    exact_privacy_exports = sdk in EXACT_PRIVACY_C_EXPORT_SDKS
    if symbol_inventory is None:
        symbols = inspect_exported_symbols(
            artifact_path,
            required=exact_privacy_exports,
        )
    else:
        symbols = symbol_inventory(artifact_path)
    privacy_exports_inspected = symbols is not None
    privacy_exports = validate_privacy_c_exports(
        () if symbols is None else symbols,
        require_exact=exact_privacy_exports,
    )
    digest_after, size_after = stable_artifact_identity(artifact_path)
    if (digest_after, size_after) != (digest, size):
        fail("native artifact changed while its ABI and exports were probed")
    source_manifest_after = workspace_source_manifest_sha256(source_root)
    commit_after, clean_after = source_state(source_root)
    if (
        commit_after != commit_before
        or not clean_after
        or source_manifest_after != source_manifest_before
    ):
        fail("native SDK source changed while artifact evidence was collected")
    return {
        "artifact_sha256": digest,
        "artifact_size": size,
        "bridge_abi_version": version,
        "privacy_c_exports": list(privacy_exports),
        "privacy_c_exports_inspected": privacy_exports_inspected,
        "required_symbols": list(REQUIRED_SYMBOLS[sdk]),
        "schema": SCHEMA,
        "sdk": sdk,
        "source_commit": commit_before,
        "workspace_source_manifest_sha256": source_manifest_before,
        "source_tree_clean": True,
        "target": target,
    }


def canonical_manifest_bytes(manifest: Mapping[str, object]) -> bytes:
    """Encode one already validated evidence manifest canonically."""

    validated = validate_manifest(dict(manifest))
    return (
        json.dumps(validated, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def validate_manifest(value: object) -> dict[str, object]:
    """Validate the exact ABI-22 artifact evidence schema."""

    manifest = _plain_object(value, "native artifact manifest")
    expected_keys = {
        "artifact_sha256",
        "artifact_size",
        "bridge_abi_version",
        "privacy_c_exports",
        "privacy_c_exports_inspected",
        "required_symbols",
        "schema",
        "sdk",
        "source_commit",
        "source_tree_clean",
        "target",
        "workspace_source_manifest_sha256",
    }
    if set(manifest) != expected_keys:
        fail("native artifact manifest field inventory is not exact")
    sdk = manifest["sdk"]
    if type(sdk) is not str or sdk not in SDK_VALUES:
        fail("native artifact manifest SDK lane is unsupported")
    target = manifest["target"]
    if type(target) is not str or TARGET_RE.fullmatch(target) is None:
        fail("native artifact manifest target is not canonical")
    digest = manifest["artifact_sha256"]
    if type(digest) is not str or SHA256_RE.fullmatch(digest) is None:
        fail("native artifact manifest SHA-256 is not canonical")
    size = manifest["artifact_size"]
    if type(size) is not int or size <= 0:
        fail("native artifact manifest size must be a positive integer")
    version = manifest["bridge_abi_version"]
    _require_exact_abi(version)
    commit = manifest["source_commit"]
    if type(commit) is not str or COMMIT_RE.fullmatch(commit) is None:
        fail("native artifact manifest source commit is not canonical")
    source_manifest = manifest["workspace_source_manifest_sha256"]
    if (
        type(source_manifest) is not str
        or SHA256_RE.fullmatch(source_manifest) is None
        or source_manifest == "0" * 64
    ):
        fail("native artifact manifest source-manifest SHA-256 is not canonical")
    if manifest["source_tree_clean"] is not True:
        fail("native artifact manifest must attest a clean source tree")
    if manifest["schema"] != SCHEMA:
        fail("native artifact manifest schema is unsupported")
    required = manifest["required_symbols"]
    if type(required) is not list or tuple(required) != REQUIRED_SYMBOLS[sdk]:
        fail("native artifact required-symbol inventory is not exact")
    privacy_exports = manifest["privacy_c_exports"]
    privacy_exports_inspected = manifest["privacy_c_exports_inspected"]
    if type(privacy_exports_inspected) is not bool or type(privacy_exports) is not list:
        fail("native artifact privacy C export evidence is malformed")
    validated_privacy_exports = validate_privacy_c_exports(
        privacy_exports,
        require_exact=sdk in EXACT_PRIVACY_C_EXPORT_SDKS,
    )
    if tuple(privacy_exports) != validated_privacy_exports:
        fail("native artifact privacy C export inventory is not canonical")
    if sdk in EXACT_PRIVACY_C_EXPORT_SDKS and not privacy_exports_inspected:
        fail("native bridge manifest must attest inspected privacy C exports")
    if not privacy_exports_inspected and privacy_exports:
        fail("uninspected native privacy C export evidence must be empty")
    return dict(manifest)


def load_manifest(path: Path) -> dict[str, object]:
    """Read one bounded, canonical evidence manifest."""

    raw = stable_bounded_file_bytes(
        path,
        label="native artifact manifest",
        maximum_bytes=MAX_MANIFEST_BYTES,
    )
    try:
        parsed = json.loads(
            raw,
            object_pairs_hook=_reject_duplicate_object_pairs,
        )
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ArtifactContractError(
            f"native artifact manifest is unreadable: {path}"
        ) from error
    validated = validate_manifest(parsed)
    if raw != canonical_manifest_bytes(validated):
        fail("native artifact manifest JSON is not canonical")
    return validated


def verify_manifest(
    manifest: Mapping[str, object],
    *,
    artifact_path: Path,
    source_root: Path,
    probe: Callable[[str, Path], int] = probe_artifact,
    symbol_inventory: Callable[[Path], Sequence[str] | None] | None = None,
) -> None:
    """Re-authenticate source, artifact bytes, exports, and exact ABI."""

    expected = validate_manifest(dict(manifest))
    commit_before, clean_before = source_state(source_root)
    if (
        not clean_before
        or commit_before != expected["source_commit"]
        or expected["source_tree_clean"] is not True
    ):
        fail("native artifact manifest does not match the current clean source revision")
    source_manifest_before = workspace_source_manifest_sha256(source_root)
    if source_manifest_before != expected["workspace_source_manifest_sha256"]:
        fail("native artifact manifest does not match the current source manifest")
    digest, size = stable_artifact_identity(artifact_path)
    if digest != expected["artifact_sha256"] or size != expected["artifact_size"]:
        fail("native artifact bytes do not match the evidence manifest")
    version = probe(str(expected["sdk"]), artifact_path)
    _require_exact_abi(version)
    if version != expected["bridge_abi_version"]:
        fail("native artifact ABI probe does not match the evidence manifest")
    sdk = str(expected["sdk"])
    must_inspect = sdk in EXACT_PRIVACY_C_EXPORT_SDKS or bool(
        expected["privacy_c_exports_inspected"]
    )
    if symbol_inventory is None:
        symbols = inspect_exported_symbols(artifact_path, required=must_inspect)
    else:
        symbols = symbol_inventory(artifact_path)
    privacy_exports = validate_privacy_c_exports(
        () if symbols is None else symbols,
        require_exact=sdk in EXACT_PRIVACY_C_EXPORT_SDKS,
    )
    if bool(expected["privacy_c_exports_inspected"]):
        if symbols is None or list(privacy_exports) != expected["privacy_c_exports"]:
            fail("native artifact privacy C exports do not match the evidence manifest")
    digest_after, size_after = stable_artifact_identity(artifact_path)
    if (digest_after, size_after) != (digest, size):
        fail("native artifact changed while its ABI and exports were verified")
    source_manifest_after = workspace_source_manifest_sha256(source_root)
    commit_after, clean_after = source_state(source_root)
    if (
        commit_after != commit_before
        or not clean_after
        or source_manifest_after != source_manifest_before
    ):
        fail("native SDK source changed while artifact evidence was verified")


def _exclusive_write(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise ArtifactContractError(
            f"native artifact manifest output must be fresh: {path}"
        ) from error
    try:
        offset = 0
        while offset < len(payload):
            offset += os.write(descriptor, payload[offset:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _require_real_directory_ancestry(path: Path, *, label: str) -> None:
    """Require an existing absolute directory path without symlink components."""

    if not path.is_absolute():
        fail(f"{label} must be absolute")
    anchor = Path(path.anchor)
    current = anchor
    for component in path.parts[len(anchor.parts) :]:
        current /= component
        try:
            metadata = current.lstat()
        except OSError as error:
            raise ArtifactContractError(
                f"{label} ancestry is unavailable: {current}"
            ) from error
        if stat.S_ISLNK(metadata.st_mode):
            fail(f"{label} ancestry must not contain symlinks: {current}")
        if not stat.S_ISDIR(metadata.st_mode):
            fail(f"{label} ancestry must contain only directories: {current}")


def _exclusive_write_at(directory: int, name: str, payload: bytes) -> None:
    """Create one private regular file relative to an authenticated directory."""

    flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(name, flags, 0o600, dir_fd=directory)
    except OSError as error:
        raise ArtifactContractError(
            f"retained native artifact manifest output must be fresh: {name}"
        ) from error
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or opened.st_nlink != 1:
            fail("retained native artifact manifest must be one regular file")
        offset = 0
        while offset < len(payload):
            offset += os.write(descriptor, payload[offset:])
        os.fsync(descriptor)
        written = os.fstat(descriptor)
        if (
            not stat.S_ISREG(written.st_mode)
            or stat.S_IMODE(written.st_mode) & 0o077 != 0
            or written.st_nlink != 1
            or written.st_size != len(payload)
        ):
            fail("retained native artifact manifest changed while it was written")
    finally:
        os.close(descriptor)


def retain_verified_manifest(
    manifest: Mapping[str, object],
    *,
    artifact_path: Path,
    evidence_directory: Path,
    source_root: Path,
    probe: Callable[[str, Path], int] = probe_artifact,
) -> Path:
    """Re-authenticate and retain a manifest in one fresh private directory.

    The output directory is deliberately fresh and external to the
    authenticated source tree so retaining evidence cannot invalidate the
    clean-tree claim.
    """

    validated = validate_manifest(dict(manifest))
    verify_manifest(
        validated,
        artifact_path=artifact_path,
        source_root=source_root,
        probe=probe,
    )
    path_text = os.fspath(evidence_directory)
    try:
        encoded_path = os.fsencode(path_text)
    except UnicodeError as error:
        raise ArtifactContractError(
            "native artifact evidence directory path is not representable"
        ) from error
    if (
        not evidence_directory.is_absolute()
        or len(encoded_path) == 0
        or len(encoded_path) > MAX_EVIDENCE_DIRECTORY_PATH_BYTES
        or len(evidence_directory.parts) > MAX_EVIDENCE_DIRECTORY_COMPONENTS
        or any(component in {".", ".."} for component in evidence_directory.parts)
        or evidence_directory == Path(evidence_directory.anchor)
    ):
        fail("native artifact evidence directory must be a bounded absolute leaf path")

    parent = evidence_directory.parent
    _require_real_directory_ancestry(
        parent,
        label="native artifact evidence directory",
    )
    try:
        evidence_directory.lstat()
    except FileNotFoundError:
        pass
    except OSError as error:
        raise ArtifactContractError(
            f"native artifact evidence directory is unavailable: {evidence_directory}"
        ) from error
    else:
        fail(
            "native artifact evidence directory must be fresh and must not be a symlink"
        )

    canonical_source = source_root.resolve(strict=True)
    canonical_parent = parent.resolve(strict=True)
    canonical_output = canonical_parent / evidence_directory.name
    if canonical_output == canonical_source or canonical_source in canonical_output.parents:
        fail("native artifact evidence directory must be outside the source tree")

    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        parent_descriptor = os.open(canonical_parent, directory_flags)
    except OSError as error:
        raise ArtifactContractError(
            "native artifact evidence directory parent could not be opened"
        ) from error
    directory_descriptor: int | None = None
    try:
        parent_identity = os.fstat(parent_descriptor)
        current_parent = canonical_parent.lstat()
        if (
            not stat.S_ISDIR(parent_identity.st_mode)
            or stat.S_ISLNK(current_parent.st_mode)
            or (parent_identity.st_dev, parent_identity.st_ino)
            != (current_parent.st_dev, current_parent.st_ino)
        ):
            fail("native artifact evidence directory parent changed before creation")
        try:
            os.mkdir(evidence_directory.name, 0o700, dir_fd=parent_descriptor)
            directory_descriptor = os.open(
                evidence_directory.name,
                directory_flags,
                dir_fd=parent_descriptor,
            )
        except OSError as error:
            raise ArtifactContractError(
                "native artifact evidence directory must be created as one fresh directory"
            ) from error
        created = os.fstat(directory_descriptor)
        current = canonical_output.lstat()
        if (
            not stat.S_ISDIR(created.st_mode)
            or stat.S_IMODE(created.st_mode) & 0o077 != 0
            or stat.S_ISLNK(current.st_mode)
            or (created.st_dev, created.st_ino) != (current.st_dev, current.st_ino)
        ):
            fail("native artifact evidence directory changed while it was created")
        output_name = f"{validated['sdk']}-native-abi22.json"
        _exclusive_write_at(
            directory_descriptor,
            output_name,
            canonical_manifest_bytes(validated),
        )
        current_after_write = canonical_output.lstat()
        if (
            stat.S_ISLNK(current_after_write.st_mode)
            or (created.st_dev, created.st_ino)
            != (current_after_write.st_dev, current_after_write.st_ino)
        ):
            fail("native artifact evidence directory changed while evidence was written")
    finally:
        if directory_descriptor is not None:
            os.close(directory_descriptor)
        os.close(parent_descriptor)

    retained_path = canonical_output / f"{validated['sdk']}-native-abi22.json"
    if load_manifest(retained_path) != validated:
        fail("retained native artifact manifest does not match verified evidence")
    return retained_path


def parse_args() -> argparse.Namespace:
    """Parse the stage/record/verify command line."""

    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("stage", "record", "verify"))
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--node", default="node")
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--sdk", choices=tuple(sorted(SDK_VALUES)))
    parser.add_argument("--target")
    parser.add_argument("--evidence-dir", type=Path)
    return parser.parse_args()


def main() -> int:
    """Run the selected fail-closed artifact operation."""

    args = parse_args()
    # Preserve the final path component so ``stable_artifact_identity`` can
    # reject a symlink rather than silently authenticating its target.
    artifact = Path(os.path.abspath(args.artifact))
    if not artifact.exists() and not artifact.is_symlink():
        fail(f"native artifact is unavailable: {artifact}")
    if args.mode == "stage":
        if args.output is None:
            fail("stage mode requires --output")
        if any(
            value is not None
            for value in (
                args.manifest,
                args.source_root,
                args.sdk,
                args.target,
                args.evidence_dir,
            )
        ):
            fail("stage mode accepts only --artifact and --output")
        staged = stage_artifact(
            artifact,
            Path(os.path.abspath(args.output)),
        )
        print(staged)
        return 0
    if args.output is not None:
        fail("record and verify modes do not accept --output")
    if args.manifest is None or args.source_root is None:
        fail("record and verify modes require --manifest and --source-root")
    source_root = args.source_root.resolve(strict=True)
    probe = lambda sdk, path: probe_artifact(
        sdk,
        path,
        node=args.node,
        python=args.python,
    )
    if args.mode == "record":
        if args.evidence_dir is not None:
            fail("record mode does not accept --evidence-dir")
        if args.sdk is None or args.target is None:
            fail("record mode requires --sdk and --target")
        manifest = build_manifest(
            sdk=args.sdk,
            target=args.target,
            artifact_path=artifact,
            source_root=source_root,
            probe=probe,
        )
        _exclusive_write(args.manifest, canonical_manifest_bytes(manifest))
    else:
        if args.sdk is not None or args.target is not None:
            fail("verify mode reads SDK and target from the manifest")
        manifest = load_manifest(Path(os.path.abspath(args.manifest)))
        if args.evidence_dir is not None:
            retain_verified_manifest(
                manifest,
                artifact_path=artifact,
                evidence_directory=args.evidence_dir,
                source_root=source_root,
                probe=probe,
            )
        else:
            verify_manifest(
                manifest,
                artifact_path=artifact,
                source_root=source_root,
                probe=probe,
            )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ArtifactContractError as error:
        print(f"native SDK ABI-22 artifact check failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
