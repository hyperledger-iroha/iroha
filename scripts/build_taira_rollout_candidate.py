#!/usr/bin/env python3
"""Build and authenticate the first-release dual-target Taira candidate.

The builder accepts one already signed Linux/aarch64 native-privacy rollout
archive and one fresh macOS/arm64 four-peer receipt.  It authenticates both,
creates the sole canonical admission archive layout, signs that exact archive,
and runs the ordinary admission verifier before returning any output.

The companion ``pack-deploy-payload`` command creates a deterministic transport
archive for the exact reset bundle, validator, and supervisor bound by the
macOS receipt.  The deploy payload is published beside (not inside) the
admission archive so the admission layout remains closed and small enough to
inspect independently.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import re
import stat
import sys
import tarfile
import tempfile
import time
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import NoReturn

try:
    from . import deploy_taira_v21_reset as deploy
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        canonical_relative_path,
        create_fresh_directory,
        exclusive_output_fd,
        exclusive_write_bytes,
        format_source_date_epoch,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_path,
        validate_release_manifest,
    )
    from .release_manifest_signing import (
        ReleaseManifestSignatureError,
        sign_release_manifest,
        verify_release_manifest,
    )
except ImportError:
    import deploy_taira_v21_reset as deploy
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        canonical_relative_path,
        create_fresh_directory,
        exclusive_output_fd,
        exclusive_write_bytes,
        format_source_date_epoch,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_path,
        validate_release_manifest,
    )
    from release_manifest_signing import (
        ReleaseManifestSignatureError,
        sign_release_manifest,
        verify_release_manifest,
    )


DEPLOY_PAYLOAD_SCHEMA = "iroha.taira.macos_arm64_deploy_payload"
DEPLOY_PAYLOAD_SCHEMA_VERSION = 1
DEPLOY_PAYLOAD_MANIFEST = "deploy-payload-v1.json"
MAX_DEPLOY_MEMBERS = 100_000
MAX_DEPLOY_LOGICAL_BYTES = 96 * 1024 * 1024 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")


class TairaCandidateBuildError(RuntimeError):
    """The proposed candidate is incomplete, mutable, or unauthenticated."""


def _sealed_controller_manifest_path() -> Path:
    return Path(__file__).resolve().parent.parent / "authority-controller-v1.json"


@dataclass(frozen=True)
class PayloadMember:
    source_root: Path
    source_relative: str
    archive_relative: str
    info: StableFile
    mode: int


def _fail(message: str) -> NoReturn:
    raise TairaCandidateBuildError(message)


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def _commit(value: object, label: str) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        _fail(f"{label} must be one full lowercase 40-hex commit")
    return value


def _canonical_path(path: Path, label: str, *, directory: bool = False) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as exc:
        raise TairaCandidateBuildError(f"cannot inspect {label}: {exc}") from exc
    if resolved != path or stat.S_ISLNK(metadata.st_mode):
        _fail(f"{label} must be one canonical non-symlink path")
    expected = stat.S_ISDIR if directory else stat.S_ISREG
    if not expected(metadata.st_mode):
        _fail(f"{label} has the wrong file type")
    return path


def _load_canonical_object(path: Path, label: str) -> dict[str, object]:
    try:
        _, payload = stable_read_path(path, max_size=admission.MAX_JSON_BYTES)
        value = admission._canonical_object(payload, label)
    except (ReleaseArtifactError, admission.TairaRolloutAdmissionError) as exc:
        raise TairaCandidateBuildError(str(exc)) from exc
    return value


def _write_tar(
    output: Path,
    *,
    prefix: str,
    members: Iterable[PayloadMember],
    directories: Iterable[str] = (),
    extra_payloads: Mapping[str, bytes],
    source_date_epoch: int,
) -> StableFile:
    try:
        format_source_date_epoch(source_date_epoch)
    except ReleaseArtifactError as exc:
        raise TairaCandidateBuildError(str(exc)) from exc
    member_list = sorted(members, key=lambda row: row.archive_relative)
    directory_list = sorted(directories)
    directory_set = set(directory_list)
    extras = sorted(extra_payloads.items())
    unsorted_names = (
        [row.archive_relative for row in member_list]
        + directory_list
        + [name for name, _ in extras]
    )
    if len(unsorted_names) != len(set(unsorted_names)):
        _fail("archive member inventory must be unique")
    try:
        canonical_prefix = canonical_relative_path(prefix)
        for name in unsorted_names:
            if canonical_relative_path(name) != name:
                _fail(f"archive member path is not canonical: {name!r}")
    except ReleaseArtifactError as exc:
        raise TairaCandidateBuildError(str(exc)) from exc
    names = sorted(unsorted_names)
    if len(names) > MAX_DEPLOY_MEMBERS:
        _fail("archive member inventory exceeds its first-release bound")
    logical_bytes = sum(row.info.size for row in member_list) + sum(
        len(payload) for _, payload in extras
    )
    if logical_bytes > MAX_DEPLOY_LOGICAL_BYTES:
        _fail("archive logical bytes exceed the first-release bound")

    output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        with exclusive_output_fd(output, mode=0o600) as descriptor:
            with (
                os.fdopen(os.dup(descriptor), "wb") as raw,
                gzip.GzipFile(
                    filename="", mode="wb", fileobj=raw, mtime=source_date_epoch
                ) as compressed,
                tarfile.open(
                    fileobj=compressed,
                    mode="w",
                    format=tarfile.GNU_FORMAT,
                ) as archive,
            ):
                file_rows = {row.archive_relative: row for row in member_list}
                extra_rows = dict(extras)
                for relative in names:
                    pure = PurePosixPath(relative)
                    if pure.is_absolute() or ".." in pure.parts or not pure.parts:
                        _fail(f"unsafe archive member path: {relative!r}")
                    arcname = f"{canonical_prefix}/{relative}"
                    if relative in directory_set:
                        info = tarfile.TarInfo(arcname)
                        info.type = tarfile.DIRTYPE
                        info.size = 0
                        info.mode = 0o700
                        info.uid = 0
                        info.gid = 0
                        info.uname = ""
                        info.gname = ""
                        info.mtime = source_date_epoch
                        archive.addfile(info)
                        continue
                    if relative in extra_rows:
                        payload = extra_rows[relative]
                        info = tarfile.TarInfo(arcname)
                        info.size = len(payload)
                        info.mode = 0o600
                        info.uid = 0
                        info.gid = 0
                        info.uname = ""
                        info.gname = ""
                        info.mtime = source_date_epoch
                        import io

                        archive.addfile(info, io.BytesIO(payload))
                        continue
                    row = file_rows[relative]
                    info = tarfile.TarInfo(arcname)
                    info.size = row.info.size
                    info.mode = row.mode
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.mtime = source_date_epoch
                    with (
                        stable_open_relative(
                            row.source_root,
                            row.source_relative,
                            expected=row.info,
                        ) as source_descriptor,
                        os.fdopen(os.dup(source_descriptor), "rb") as source,
                    ):
                        archive.addfile(info, source)
            os.fsync(descriptor)
    except BaseException:
        output.unlink(missing_ok=True)
        raise
    return stable_hash_path(output)


def _authority_members(authority_dir: Path) -> list[PayloadMember]:
    try:
        inventory = scan_inventory_paths(authority_dir)
    except ReleaseArtifactError as exc:
        raise TairaCandidateBuildError(
            f"cannot scan Linux authority directory: {exc}"
        ) from exc
    expected = sorted(admission.LINUX_AUTHORITY_FILES)
    if inventory != expected:
        _fail(
            "Linux authority inventory is not exact: "
            f"expected={expected}, actual={inventory}"
        )
    return [
        PayloadMember(
            source_root=authority_dir,
            source_relative=relative,
            archive_relative=f"{admission.LINUX_AUTHORITY_DIRECTORY}/{relative}",
            info=stable_hash_relative(authority_dir, relative),
            mode=0o600,
        )
        for relative in expected
    ]


def _final_manifest(
    *,
    archive: Path,
    archive_info: StableFile,
    source: admission.SourceIdentity,
    source_date_epoch: int,
) -> bytes:
    return canonical_json_bytes(
        validate_release_manifest(
            {
                "arch": "arm64",
                "artifacts": [
                    {
                        "format": "tar.gz",
                        "kind": "reference-validator",
                        "path": archive.name,
                        "profile": "iroha3",
                        "sha256": archive_info.sha256,
                        "size": archive_info.size,
                        "target": "taira-rollout-admission-v1",
                    }
                ],
                "built_at": format_source_date_epoch(source_date_epoch),
                "commit": source.commit,
                "os": "macos",
                "schema": "iroha.release_manifest",
                "schema_version": 1,
                "source_date_epoch": source_date_epoch,
                "version": (
                    f"taira-admission-{source.workspace_source_manifest_sha256[:16]}"
                ),
            }
        )
    )


def assemble_candidate(args: argparse.Namespace) -> dict[str, object]:
    source = admission.SourceIdentity(
        _commit(args.source_commit, "source commit"),
        _commit(
            args.dpn_validator_release_commit,
            "DPN validator release commit",
        ),
        _sha256(args.cargo_lock_sha256, "Cargo.lock digest"),
        _sha256(args.workspace_source_manifest_sha256, "workspace source digest"),
    )
    signer_fingerprint = _sha256(
        args.trusted_signing_fingerprint, "trusted signing fingerprint"
    )
    verifier_sha = _sha256(
        args.trusted_release_manifest_verifier_sha256,
        "trusted release-manifest verifier digest",
    )
    receipt_id = _sha256(args.expected_receipt_id, "expected receipt ID")
    controller_digest = _sha256(
        args.controller_digest, "sealed macOS controller digest"
    )
    controller_manifest = _canonical_path(
        args.controller_manifest, "sealed macOS controller manifest"
    )
    expected_controller_manifest = _sealed_controller_manifest_path()
    if controller_manifest != expected_controller_manifest:
        _fail(
            "controller manifest is not the sibling of the sealed candidate controller"
        )
    controller_info, controller_payload = stable_read_path(
        controller_manifest, max_size=admission.MAX_JSON_BYTES
    )
    admission._validate_controller_manifest(
        controller_payload,
        expected_digest=controller_digest,
        expected_source_commit=source.commit,
    )
    linux_archive = _canonical_path(args.linux_archive, "Linux rollout archive")
    if not linux_archive.name.endswith(".tar.gz"):
        _fail("Linux rollout archive must use .tar.gz")
    linux_authority = _canonical_path(
        args.linux_authority_dir, "Linux authority directory", directory=True
    )
    receipt_path = _canonical_path(args.macos_receipt, "macOS receipt")
    verifier = _canonical_path(
        args.release_manifest_verifier, "native release-manifest verifier"
    )
    output = args.output_directory
    if not output.is_absolute():
        _fail("output directory must be absolute")
    try:
        create_fresh_directory(output, mode=0o700)
    except ReleaseArtifactError as exc:
        raise TairaCandidateBuildError(str(exc)) from exc

    try:
        now_unix = int(time.time()) if args.now_unix is None else args.now_unix
        receipt_object = _load_canonical_object(receipt_path, "macOS receipt")
        _, receipt_payload = stable_read_path(
            receipt_path, max_size=admission.MAX_JSON_BYTES
        )
        admission._validate_macos_receipt(
            receipt_payload,
            expected_source=source,
            expected_receipt_id=receipt_id,
            consumed_receipt_ids=set(),
            now_unix=now_unix,
        )

        authority_rows = _authority_members(linux_authority)
        linux_info = stable_hash_path(
            linux_archive,
            max_size=admission.taira_release_authority.MAX_ARCHIVE_LOGICAL_BYTES,
        )
        with tempfile.TemporaryDirectory(
            prefix="taira-candidate-linux-", dir=output
        ) as raw_verify:
            verify_root = Path(raw_verify).resolve(strict=True)
            staged_authority = verify_root / admission.LINUX_AUTHORITY_DIRECTORY
            for row in authority_rows:
                destination = verify_root / row.archive_relative
                destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                with (
                    stable_open_relative(
                        row.source_root, row.source_relative, expected=row.info
                    ) as source_descriptor,
                    exclusive_output_fd(destination, mode=0o600) as target,
                ):
                    while chunk := os.read(source_descriptor, 1024 * 1024):
                        view = memoryview(chunk)
                        while view:
                            written = os.write(target, view)
                            if written <= 0:
                                _fail("short write staging Linux authority")
                            view = view[written:]
            staged_linux = verify_root / "linux" / linux_archive.name
            with (
                stable_open_relative(
                    linux_archive.parent, linux_archive.name, expected=linux_info
                ) as source_descriptor,
                exclusive_output_fd(staged_linux, mode=0o600) as target,
            ):
                while chunk := os.read(source_descriptor, 1024 * 1024):
                    view = memoryview(chunk)
                    while view:
                        written = os.write(target, view)
                        if written <= 0:
                            _fail("short write staging Linux archive")
                        view = view[written:]
            nested_manifest_sha = stable_hash_relative(
                staged_authority, "release_manifest.json"
            ).sha256
            nested = admission._verify_closed_linux_authority(
                verify_root,
                expected_source=source,
                expected_manifest_sha256=nested_manifest_sha,
                expected_native_verifier_sha256=verifier_sha,
                trusted_signing_fingerprint=signer_fingerprint,
                release_manifest_verifier_path=verifier,
                trusted_release_manifest_verifier_sha256=verifier_sha,
                linux_archive_path=staged_linux,
            )
            admission._verify_existing_linux_authority(
                verify_root,
                linux_archive_path=staged_linux,
                expected_source=source,
                trusted_signing_fingerprint=signer_fingerprint,
                native_verifier_sha256=verifier_sha,
            )

        linux_relative = f"linux/{linux_archive.name}"
        receipt_relative = admission.MACOS_RECEIPT_PATH
        members = [
            *authority_rows,
            PayloadMember(
                linux_archive.parent,
                linux_archive.name,
                linux_relative,
                linux_info,
                0o600,
            ),
            PayloadMember(
                controller_manifest.parent,
                controller_manifest.name,
                admission.CONTROLLER_MANIFEST_PATH,
                controller_info,
                0o600,
            ),
            PayloadMember(
                receipt_path.parent,
                receipt_path.name,
                receipt_relative,
                stable_hash_path(receipt_path),
                0o600,
            ),
        ]
        inventory = [
            {
                "path": row.archive_relative,
                "sha256": row.info.sha256,
                "size": row.info.size,
            }
            for row in sorted(members, key=lambda item: item.archive_relative)
        ]
        admission_manifest = canonical_json_bytes(
            {
                "controller": {
                    "digest": controller_digest,
                    "manifest_path": admission.CONTROLLER_MANIFEST_PATH,
                    "platform": "macos",
                    "source_commit": source.commit,
                },
                "inventory": inventory,
                "linux_arm64": {
                    "arch": "aarch64",
                    "archive_path": linux_relative,
                    "authority_directory": admission.LINUX_AUTHORITY_DIRECTORY,
                    "authority_manifest_sha256": nested["manifest_sha256"],
                    "authority_native_verifier_sha256": verifier_sha,
                    "os": "linux",
                },
                "macos_arm64": {
                    "arch": "arm64",
                    "os": "macos",
                    "receipt_id": receipt_id,
                    "receipt_path": receipt_relative,
                },
                "schema": admission.ADMISSION_SCHEMA,
                "schema_version": admission.ADMISSION_SCHEMA_VERSION,
                "source": source.as_dict(),
                "trust": {
                    "release_manifest_verifier_sha256": verifier_sha,
                    "signer_fingerprint_sha256": signer_fingerprint,
                },
            }
        )
        archive_name = (
            f"taira-admission-{source.workspace_source_manifest_sha256[:16]}-"
            "macos-arm64.tar.gz"
        )
        archive_path = output / archive_name
        archive_info = _write_tar(
            archive_path,
            prefix=archive_name.removesuffix(".tar.gz"),
            members=members,
            extra_payloads={admission.ADMISSION_MANIFEST_PATH: admission_manifest},
            source_date_epoch=args.source_date_epoch,
        )

        authority_dir = output / f"{archive_name.removesuffix('.tar.gz')}.authority"
        create_fresh_directory(authority_dir, mode=0o700)
        manifest = authority_dir / "release_manifest.json"
        signature = authority_dir / "release_manifest.json.sig"
        public_key = authority_dir / "release_manifest.json.pub"
        exclusive_write_bytes(
            manifest,
            _final_manifest(
                archive=archive_path,
                archive_info=archive_info,
                source=source,
                source_date_epoch=args.source_date_epoch,
            ),
            mode=0o600,
        )
        signing = sign_release_manifest(
            manifest,
            args.external_signer,
            args.signing_public_key,
            signer_fingerprint,
            signature,
            public_key,
            verifier,
            verifier_sha,
        )
        verify_release_manifest(
            manifest,
            signature,
            public_key,
            signer_fingerprint,
            verifier,
            verifier_sha,
        )

        replay = output / ".empty-replay-ledger.json"
        exclusive_write_bytes(
            replay, admission.canonical_replay_ledger_bytes([]), mode=0o600
        )
        try:
            verified = admission.verify_admission(
                archive_path=archive_path,
                authority_dir=authority_dir,
                expected_source=source,
                expected_receipt_id=receipt_id,
                replay_ledger_path=replay,
                trusted_signing_fingerprint=signer_fingerprint,
                release_manifest_verifier_path=verifier,
                trusted_release_manifest_verifier_sha256=verifier_sha,
                now_unix=now_unix,
            )
        finally:
            replay.unlink(missing_ok=True)
        return {
            "archive": str(archive_path),
            "archive_sha256": archive_info.sha256,
            "authority_dir": str(authority_dir),
            "authority_manifest_sha256": signing["manifest_sha256"],
            "receipt_id": receipt_object["receipt_id"],
            "controller_digest": controller_digest,
            "source": source.as_dict(),
            "verified": verified["verified"],
        }
    except Exception as exc:
        # The output directory is fresh and exclusively owned by this command.
        # Leave it in place for forensic inspection and make that explicit to
        # an operator without changing the original exception type.
        exc.add_note(f"partial candidate retained for inspection: {output}")
        raise


def _scan_deploy_tree(root: Path) -> tuple[list[PayloadMember], list[str]]:
    rows: list[PayloadMember] = []
    directories: list[str] = []
    count = 0
    logical_bytes = 0
    for current, directory_names, file_names in os.walk(root, followlinks=False):
        directory_names.sort()
        file_names.sort()
        current_path = Path(current)
        for name in directory_names:
            path = current_path / name
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
                _fail(f"deploy bundle contains an unsafe directory: {path}")
            count += 1
            if count > MAX_DEPLOY_MEMBERS:
                _fail("deploy bundle exceeds its member-count bound")
            directories.append(f"bundle/{path.relative_to(root).as_posix()}")
        for name in file_names:
            count += 1
            if count > MAX_DEPLOY_MEMBERS:
                _fail("deploy bundle exceeds its member-count bound")
            path = current_path / name
            info = path.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISREG(info.st_mode)
                or info.st_nlink != 1
                or info.st_size <= 0
            ):
                _fail(f"deploy bundle contains an unsafe regular file: {path}")
            relative = path.relative_to(root).as_posix()
            stable = stable_hash_relative(root, relative)
            logical_bytes += stable.size
            if logical_bytes > MAX_DEPLOY_LOGICAL_BYTES:
                _fail("deploy bundle exceeds its logical-size bound")
            rows.append(
                PayloadMember(
                    root,
                    relative,
                    f"bundle/{relative}",
                    stable,
                    0o600,
                )
            )
    return rows, directories


def pack_deploy_payload(args: argparse.Namespace) -> dict[str, object]:
    source = admission.SourceIdentity(
        _commit(args.source_commit, "source commit"),
        _commit(
            args.dpn_validator_release_commit,
            "DPN validator release commit",
        ),
        _sha256(args.cargo_lock_sha256, "Cargo.lock digest"),
        _sha256(args.workspace_source_manifest_sha256, "workspace source digest"),
    )
    bundle = _canonical_path(args.reset_bundle, "reset bundle", directory=True)
    binary = _canonical_path(args.validator_binary, "validator binary")
    supervisor = _canonical_path(args.supervisor, "validator supervisor")
    receipt_path = _canonical_path(args.macos_receipt, "macOS receipt")
    receipt = _load_canonical_object(receipt_path, "macOS receipt")
    receipt_id = _sha256(receipt.get("receipt_id"), "receipt ID")
    _, receipt_payload = stable_read_path(
        receipt_path, max_size=admission.MAX_JSON_BYTES
    )
    admission._validate_macos_receipt(
        receipt_payload,
        expected_source=source,
        expected_receipt_id=receipt_id,
        consumed_receipt_ids=set(),
        now_unix=int(time.time()) if args.now_unix is None else args.now_unix,
    )

    reset_manifest = stable_hash_relative(bundle, "reset-manifest.json")
    binary_info = stable_hash_path(binary)
    supervisor_info = stable_hash_path(supervisor)
    if reset_manifest.sha256 != receipt["reset_manifest_sha256"]:
        _fail("reset manifest bytes differ from the macOS receipt")
    if binary_info.sha256 != receipt["validator_binary_sha256"]:
        _fail("validator bytes differ from the macOS receipt")
    if supervisor_info.sha256 != receipt["supervisor_sha256"]:
        _fail("supervisor bytes differ from the macOS receipt")

    try:
        bundle_plan = deploy.validate_bundle(
            bundle,
            expected_reset_manifest_sha256=reset_manifest.sha256,
            expected_binary_sha256=binary_info.sha256,
            expected_source_commit=source.commit,
            expected_dpn_validator_release_commit=(
                source.dpn_validator_release_commit
            ),
            minimum_free_bytes=0,
            maximum_fsync_latency_ms=10_000,
        )
        deploy.require_bundle_runtime_unchanged(bundle_plan)
    except deploy.DeploymentError as exc:
        raise TairaCandidateBuildError(
            f"reset bundle failed production revalidation: {exc}"
        ) from exc
    if len(bundle_plan.peers) != admission.PEER_COUNT:
        _fail("reset bundle must contain exactly four validators")

    expected_configs = receipt["validator_config_sha256"]
    assert isinstance(expected_configs, dict)
    actual_configs = {peer.slug: peer.config_sha256 for peer in bundle_plan.peers}
    if actual_configs != expected_configs:
        _fail("validator config bytes differ from the macOS receipt")

    members, directories = _scan_deploy_tree(bundle)
    directories.extend(("bin", "supervisor"))
    members.extend(
        [
            PayloadMember(binary.parent, binary.name, "bin/irohad", binary_info, 0o500),
            PayloadMember(
                supervisor.parent,
                supervisor.name,
                "supervisor/taira_peer_supervisor.py",
                supervisor_info,
                0o500,
            ),
        ]
    )
    inventory = sorted(
        [
            {
                "kind": "file",
                "mode": format(row.mode, "04o"),
                "path": row.archive_relative,
                "sha256": row.info.sha256,
                "size": row.info.size,
            }
            for row in members
        ]
        + [
            {
                "kind": "directory",
                "mode": "0700",
                "path": relative,
            }
            for relative in directories
        ],
        key=lambda row: str(row["path"]),
    )
    manifest = canonical_json_bytes(
        {
            "components": {
                "reset_manifest_sha256": reset_manifest.sha256,
                "supervisor_sha256": supervisor_info.sha256,
                "validator_binary_sha256": binary_info.sha256,
                "validator_config_sha256": expected_configs,
            },
            "inventory": inventory,
            "receipt_id": receipt_id,
            "schema": DEPLOY_PAYLOAD_SCHEMA,
            "schema_version": DEPLOY_PAYLOAD_SCHEMA_VERSION,
            "source": source.as_dict(),
        }
    )
    output = args.output
    if not output.is_absolute() or not output.name.endswith(".tar.gz"):
        _fail("deploy payload output must be an absolute .tar.gz path")
    prefix = output.name.removesuffix(".tar.gz")
    info = _write_tar(
        output,
        prefix=prefix,
        members=members,
        directories=directories,
        extra_payloads={DEPLOY_PAYLOAD_MANIFEST: manifest},
        source_date_epoch=args.source_date_epoch,
    )
    return {
        "archive": str(output),
        "archive_sha256": info.sha256,
        "archive_size": info.size,
        "receipt_id": receipt_id,
        "source": source.as_dict(),
    }


def _source_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--source-date-epoch", type=int, required=True)
    parser.add_argument("--now-unix", type=int)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    subparsers = parser.add_subparsers(dest="command", required=True)
    assemble = subparsers.add_parser("assemble", allow_abbrev=False)
    _source_arguments(assemble)
    assemble.add_argument("--linux-archive", type=Path, required=True)
    assemble.add_argument("--linux-authority-dir", type=Path, required=True)
    assemble.add_argument("--macos-receipt", type=Path, required=True)
    assemble.add_argument("--expected-receipt-id", required=True)
    assemble.add_argument("--controller-manifest", type=Path, required=True)
    assemble.add_argument("--controller-digest", required=True)
    assemble.add_argument("--trusted-signing-fingerprint", required=True)
    assemble.add_argument("--release-manifest-verifier", type=Path, required=True)
    assemble.add_argument("--trusted-release-manifest-verifier-sha256", required=True)
    assemble.add_argument("--external-signer", type=Path, required=True)
    assemble.add_argument("--signing-public-key", type=Path, required=True)
    assemble.add_argument("--output-directory", type=Path, required=True)

    pack = subparsers.add_parser("pack-deploy-payload", allow_abbrev=False)
    _source_arguments(pack)
    pack.add_argument("--reset-bundle", type=Path, required=True)
    pack.add_argument("--validator-binary", type=Path, required=True)
    pack.add_argument("--supervisor", type=Path, required=True)
    pack.add_argument("--macos-receipt", type=Path, required=True)
    pack.add_argument("--output", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        result = (
            assemble_candidate(args)
            if args.command == "assemble"
            else pack_deploy_payload(args)
        )
    except (
        OSError,
        ReleaseArtifactError,
        ReleaseManifestSignatureError,
        TairaCandidateBuildError,
        admission.TairaRolloutAdmissionError,
        deploy.DeploymentError,
        tarfile.TarError,
    ) as exc:
        print(f"Taira candidate build refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
