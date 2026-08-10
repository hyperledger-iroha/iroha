#!/usr/bin/env python3
"""Create or verify the portable signed-authority payload for a Taira release.

This command does not sign anything.  It creates the canonical JSON payload
which is placed in the closed aggregate release inventory and then signed with
``release_manifest_signing.py``.  Verification rebuilds the payload from the
actual release subject and native exact-12 evidence and requires byte-for-byte
equality, so a valid signature cannot authorize a relocated or substituted
artifact by path alone.

The current implementation can close byte identities, but it cannot establish
that the source-built native runner actually produced semantically valid
Exact12 results.  Every production entry point is therefore provisioned closed
until a separately authenticated native-evidence authority is installed.  The
lower structural builder is retained only for hostile and post-provisioning
tests; its self-consistent hashes are not release authority.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import sys
import tarfile
from pathlib import Path
from pathlib import PurePosixPath

try:
    from .release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        canonical_relative_path,
        exclusive_write_bytes,
        load_json_object,
        stable_hash_path,
        stable_hash_relative,
    )
except ImportError:
    from release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        canonical_relative_path,
        exclusive_write_bytes,
        load_json_object,
        stable_hash_path,
        stable_hash_relative,
    )


SCHEMA = "iroha.taira.exact12_release_authority"
SCHEMA_VERSION = 1
PROTOCOL_COUNT = 12
STAGE_COUNT = 48
REGISTRY_SHA256 = "734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51"
MAX_AUTHORITY_BYTES = 1024 * 1024
MAX_ARCHIVE_MEMBERS = 100_000
MAX_ARCHIVE_LOGICAL_BYTES = 16 * 1024 * 1024 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
IMAGE_DIGEST_RE = re.compile(r"sha256:[0-9a-f]{64}")
IMAGE_TAG_RE = re.compile(r"[a-z0-9][a-z0-9./_-]{0,190}:[a-z0-9][a-z0-9._-]{0,127}")

PROTOCOLS = (
    ("zk-ace-pq-authorization-v0", "ZkAcePqAuthorizationV0"),
    ("anonymous-pgc-k-out-of-n-v1", "AnonymousPgcKOutOfNV1"),
    ("verange-transparent-range-v1", "VeRangeTransparentRangeV1"),
    ("iroha-zk-ams-v1", "IrohaZkAmsV1"),
    ("vega-existing-credential-zk-v0", "VegaExistingCredentialZkV0"),
    ("iroha-zk-x509-stark-p256-v0", "IrohaZkX509StarkP256V0"),
    ("iroha-jindo-polynomial-commitment-v0", "IrohaJindoPolynomialCommitmentV0"),
    ("iroha-bootle-lantern-anoncred-v1", "IrohaBootleLanternAnoncredV1"),
    ("orchard-halo2-actions-v1", "OrchardHalo2ActionsV1"),
    ("monero-fcmp-plus-plus-v1", "MoneroFcmpPlusPlusV1"),
    ("iroha-ivm-private-note-stark-v1", "IrohaIvmPrivateNoteStarkV1"),
    ("pq-masp-stark-v0", "PqMaspStarkV0"),
)
RETIRED_LABELS = (
    "zkat-policy-private-auth-v1",
    "zk-ams-recursive-admission-v0",
    "silent-threshold-anoncred-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
)

EVIDENCE_PATHS = {
    "cargo_lock": "provenance/Cargo.lock",
    "dpn_validator_build_provenance": (
        "provenance/dpn-validator-build.provenance.json"
    ),
    "command_manifest_json": (
        "provenance/privacy-native/command-manifest-v1.json"
    ),
    "command_manifest_norito": (
        "provenance/privacy-native/command-manifest-v1.norito"
    ),
    "exact12_matrix": "provenance/privacy-native/exact12-v1.tsv",
    "expectations_json": "provenance/privacy-native/expectations-v1.json",
    "expectations_norito": "provenance/privacy-native/expectations-v1.norito",
    "x509_resource_json": ("provenance/privacy-native/zk-x509-resource-v1.json"),
    "x509_resource_norito": ("provenance/privacy-native/zk-x509-resource-v1.norito"),
    "receipt_json": "provenance/privacy-native/receipt-v1.json",
    "receipt_norito": "provenance/privacy-native/receipt-v1.norito",
    "runner_binary": "bin/taira_privacy_release_runner",
    "stage_artifacts_json": (
        "provenance/privacy-native/stage-artifacts-v1.json"
    ),
    "stage_artifacts_norito": (
        "provenance/privacy-native/stage-artifacts-v1.norito"
    ),
    "validator_binary": "bin/iroha3d",
    "workspace_source_manifest": (
        "provenance/privacy-native/workspace-source-manifest.sha256"
    ),
}

INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA = (
    "iroha.taira.independent-native-evidence-authority.v1"
)
INDEPENDENT_NATIVE_EVIDENCE_REPLAY_NAMESPACE = (
    "iroha.taira.independent-native-evidence-authority-replay.v1"
)
INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_PROVISIONING_ERROR = (
    f"{INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_SCHEMA} is not provisioned: "
    "release authority requires a canonical authority-origin envelope signed "
    "under a separately pinned trust root that is inaccessible to the "
    "source-built runner, build lane, runtime, candidate signer, and release "
    "signer; the envelope must bind the exact runner and validator bytes, the "
    "JSON and Norito command manifest, receipt, stage-artifact, expectation, "
    "and ZK-X509 resource bytes and digests, the closed Exact12 operation and "
    "outcome table, source and DPN commits, Cargo.lock and workspace-source "
    "digests, the native Linux host and installation identity, the installed "
    "controller digest, and a fresh run nonce, issued time, expiry, and replay "
    f"identity in {INDEPENDENT_NATIVE_EVIDENCE_REPLAY_NAMESPACE}; that "
    "authority must independently verify JSON/Norito correspondence and "
    "native result semantics and must reject runner self-hashes, recomputed "
    "archive hashes, caller markers, reused signing keys, stale runs, and "
    "legacy unsigned evidence"
)


class TairaReleaseAuthorityError(RuntimeError):
    """The release cannot be authorized by the exact first-release contract."""


def _fail(message: str) -> None:
    raise TairaReleaseAuthorityError(message)


def require_independent_native_evidence_authority_provisioned() -> None:
    """Refuse release trust until an independent semantic authority exists.

    This has deliberately no arguments, environment switch, marker file, or
    key-based escape hatch.  Provisioning requires a new authenticated broker
    and verifier path, not reuse of the release or candidate signer.
    """

    raise TairaReleaseAuthorityError(
        INDEPENDENT_NATIVE_EVIDENCE_AUTHORITY_PROVISIONING_ERROR
    )


def _sha256(value: str, label: str) -> str:
    if SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be exactly 64 lowercase hexadecimal characters")
    return value


def _commit(value: str) -> str:
    if COMMIT_RE.fullmatch(value) is None:
        _fail("release commit must be exactly 40 lowercase hexadecimal characters")
    return value


def _read_source_digest(root: Path) -> tuple[StableFile, str]:
    relative = EVIDENCE_PATHS["workspace_source_manifest"]
    info, payload = _stable_read(root, relative, maximum=65)
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as exc:
        raise TairaReleaseAuthorityError(
            "workspace source manifest digest file must be ASCII"
        ) from exc
    if not text.endswith("\n") or text.count("\n") != 1:
        _fail("workspace source manifest digest file must contain one final newline")
    return info, _sha256(text[:-1], "workspace source manifest")


def _stable_read(
    root: Path,
    relative: str,
    *,
    maximum: int,
) -> tuple[StableFile, bytes]:
    """Read a stable evidence file and confirm the bytes match its stable hash."""

    info = stable_hash_relative(root, relative, max_size=maximum)
    absolute = root / relative
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(absolute, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            (opened.st_dev, opened.st_ino) != (info.device, info.inode)
            or opened.st_size != info.size
            or opened.st_nlink != 1
        ):
            _fail(f"release evidence changed before reading {relative!r}")
        payload = bytearray()
        while len(payload) <= maximum:
            chunk = os.read(descriptor, min(1024 * 1024, maximum + 1 - len(payload)))
            if not chunk:
                break
            payload.extend(chunk)
        closed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if len(payload) > maximum:
        _fail(f"release evidence {relative!r} exceeds its byte bound")
    if (
        (closed.st_dev, closed.st_ino) != (info.device, info.inode)
        or closed.st_size != info.size
        or closed.st_mtime_ns != info.mtime_ns
        or closed.st_ctime_ns != info.ctime_ns
        or closed.st_nlink != 1
    ):
        _fail(f"release evidence changed while reading {relative!r}")
    if hashlib.sha256(payload).hexdigest() != info.sha256:
        _fail(f"release evidence digest changed while reading {relative!r}")
    return info, bytes(payload)


def _validate_exact12_matrix(payload: bytes) -> dict[str, object]:
    if not payload or payload[-1:] != b"\n" or b"\r" in payload or b"\0" in payload:
        _fail("exact12 matrix must use canonical LF-delimited UTF-8")
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise TairaReleaseAuthorityError("exact12 matrix must be UTF-8") from exc

    version_rows: list[list[str]] = []
    registry_rows: list[list[str]] = []
    protocol_rows: list[list[str]] = []
    envelope_rows: list[list[str]] = []
    retired_rows: list[list[str]] = []
    for line in text[:-1].split("\n"):
        if line.startswith("#"):
            continue
        if not line:
            _fail("exact12 matrix contains an empty non-comment row")
        fields = line.split("\t")
        kind = fields[0]
        if kind == "matrix-version":
            version_rows.append(fields)
        elif kind == "registry-sha256":
            registry_rows.append(fields)
        elif kind == "protocol":
            protocol_rows.append(fields)
        elif kind == "typed-envelope":
            envelope_rows.append(fields)
        elif kind == "retired":
            retired_rows.append(fields)
        else:
            _fail(f"exact12 matrix contains unknown row kind {kind!r}")

    if version_rows != [["matrix-version", "1"]]:
        _fail("exact12 matrix must contain exactly one canonical v1 row")
    if registry_rows != [["registry-sha256", REGISTRY_SHA256]]:
        _fail("exact12 matrix carries the wrong first-release registry digest")
    if len(protocol_rows) != PROTOCOL_COUNT:
        _fail("exact12 matrix must contain exactly 12 protocol rows")
    if len(envelope_rows) != PROTOCOL_COUNT:
        _fail("exact12 matrix must contain exactly 12 typed-envelope rows")

    expected_protocol_rows = [
        ["protocol", str(index), label, variant, variant]
        for index, (label, variant) in enumerate(PROTOCOLS)
    ]
    if protocol_rows != expected_protocol_rows:
        _fail("exact12 protocol rows are reordered, missing, aliased, or non-canonical")

    labels = [label for label, _ in PROTOCOLS]
    computed_registry = hashlib.sha256(
        "".join(f"{label}\n" for label in labels).encode("utf-8")
    ).hexdigest()
    if computed_registry != REGISTRY_SHA256:
        _fail("compiled exact12 authority registry constant is inconsistent")

    for index, row in enumerate(envelope_rows):
        if len(row) != 6:
            _fail(f"typed-envelope row {index} has the wrong arity")
        label, variant = PROTOCOLS[index]
        if row[1:4] != [label, variant, variant]:
            _fail(f"typed-envelope row {index} does not match its protocol")
        _sha256(row[4], f"typed-envelope statement digest {index}")
        _sha256(row[5], f"typed-envelope envelope digest {index}")
    if len({row[4] for row in envelope_rows}) != PROTOCOL_COUNT:
        _fail("typed-envelope statement digests must be unique")
    if len({row[5] for row in envelope_rows}) != PROTOCOL_COUNT:
        _fail("typed-envelope envelope digests must be unique")

    if retired_rows != [["retired", label] for label in RETIRED_LABELS]:
        _fail("exact12 retired-label rows are missing, reordered, or aliased")
    if set(labels) & set(RETIRED_LABELS):
        _fail("an active exact12 protocol is also marked retired")

    return {
        "protocol_count": PROTOCOL_COUNT,
        "protocol_labels": labels,
        "registry_sha256": REGISTRY_SHA256,
        "retired_labels": list(RETIRED_LABELS),
        "stage_count": STAGE_COUNT,
        "typed_envelope_count": PROTOCOL_COUNT,
    }


def _validate_build_provenance(
    payload: bytes,
    *,
    expected_commit: str,
    expected_dpn_commit: str,
    expected_cargo_lock_sha256: str,
    expected_workspace_source_manifest_sha256: str,
) -> None:
    try:
        value = load_json_object(payload, "DPN validator build provenance")
    except ReleaseArtifactError as exc:
        raise TairaReleaseAuthorityError(str(exc)) from exc
    if canonical_json_bytes(value) != payload:
        _fail("DPN validator build provenance is not canonical deterministic JSON")
    expected_fields = {
        "dpn_validator_release_commit",
        "iroha_git_head",
        "iroha_source_attested",
        "iroha_source_bundle_provenance_sha256",
        "iroha_source_tree_sha256",
        "iroha_tracked_patch_sha256",
        "iroha_worktree_clean",
        "schema_version",
        "validator_lock_sha256",
        "workspace_source_manifest_sha256",
    }
    if set(value) != expected_fields:
        _fail("DPN validator build provenance fields are not exact")
    if value["schema_version"] != 1 or value["iroha_source_attested"] is not True:
        _fail("DPN validator build provenance is not one attested v1 release source")
    if value["iroha_git_head"] != expected_commit:
        _fail("DPN validator build provenance Iroha commit differs")
    if value["dpn_validator_release_commit"] != expected_dpn_commit:
        _fail("DPN validator build provenance release commit differs")
    if value["validator_lock_sha256"] != expected_cargo_lock_sha256:
        _fail("DPN validator build provenance Cargo.lock digest differs")
    if (
        value["workspace_source_manifest_sha256"]
        != expected_workspace_source_manifest_sha256
    ):
        _fail("DPN validator build provenance workspace source digest differs")
    for field in (
        "iroha_source_bundle_provenance_sha256",
        "iroha_source_tree_sha256",
        "iroha_tracked_patch_sha256",
    ):
        if not isinstance(value[field], str) or SHA256_RE.fullmatch(value[field]) is None:
            _fail(f"DPN validator build provenance {field} is invalid")
    if not isinstance(value["iroha_worktree_clean"], bool):
        _fail("DPN validator build provenance worktree cleanliness is invalid")


def _evidence(
    root: Path,
    *,
    expected_commit: str,
    expected_dpn_commit: str,
) -> tuple[list[dict[str, object]], dict[str, object], str]:
    root = Path(os.path.abspath(root))
    source_info, source_digest = _read_source_digest(root)
    matrix_info, matrix_payload = _stable_read(
        root,
        EVIDENCE_PATHS["exact12_matrix"],
        maximum=1024 * 1024,
    )
    exact12 = _validate_exact12_matrix(matrix_payload)

    provenance_info, provenance_payload = _stable_read(
        root,
        EVIDENCE_PATHS["dpn_validator_build_provenance"],
        maximum=1024 * 1024,
    )
    cargo_info = stable_hash_relative(root, EVIDENCE_PATHS["cargo_lock"])
    _validate_build_provenance(
        provenance_payload,
        expected_commit=expected_commit,
        expected_dpn_commit=expected_dpn_commit,
        expected_cargo_lock_sha256=cargo_info.sha256,
        expected_workspace_source_manifest_sha256=source_digest,
    )
    captured = {
        "cargo_lock": cargo_info,
        "dpn_validator_build_provenance": provenance_info,
        "exact12_matrix": matrix_info,
        "workspace_source_manifest": source_info,
    }
    artifacts = []
    for name, relative in sorted(EVIDENCE_PATHS.items()):
        canonical_relative_path(relative)
        info = captured.get(name)
        if info is None:
            info = stable_hash_relative(root, relative)
            captured[name] = info
        if info.size <= 0:
            _fail(f"release evidence {relative!r} must not be empty")
        artifacts.append(
            {
                "name": name,
                "path": relative,
                "sha256": info.sha256,
                "size": info.size,
            }
        )
    for name, relative in sorted(EVIDENCE_PATHS.items()):
        if stable_hash_relative(root, relative) != captured[name]:
            _fail(f"release evidence changed while authority was assembled: {relative!r}")
    return artifacts, exact12, source_digest


def _archive_subject(path: Path) -> tuple[dict[str, object], StableFile]:
    absolute = Path(os.path.abspath(path))
    if absolute.name != canonical_relative_path(absolute.name):
        _fail("release archive basename is not portable")
    if not absolute.name.endswith(".tar.gz"):
        _fail("Taira rollout archive must use the .tar.gz format")
    info = stable_hash_path(absolute)
    if info.size <= 0:
        _fail("Taira rollout archive must not be empty")
    return (
        {
            "kind": "taira-rollout-tar-gzip-v1",
            "name": absolute.name,
            "sha256": info.sha256,
            "size": info.size,
        },
        info,
    )


def _verify_archive_evidence(
    archive_path: Path,
    artifacts: list[dict[str, object]],
    expected_info: StableFile,
) -> None:
    """Require the signed archive bytes to contain the exact evidence hashes."""

    archive_name = archive_path.name
    prefix = archive_name.removesuffix(".tar.gz")
    expected = {
        f"{prefix}/{row['path']}": row
        for row in artifacts
    }
    seen: set[str] = set()
    verified_evidence: set[str] = set()
    logical_bytes = 0
    member_count = 0
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(archive_path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            (opened.st_dev, opened.st_ino)
            != (expected_info.device, expected_info.inode)
            or opened.st_size != expected_info.size
            or opened.st_mtime_ns != expected_info.mtime_ns
            or opened.st_ctime_ns != expected_info.ctime_ns
            or opened.st_nlink != 1
        ):
            _fail("Taira rollout archive changed before inspection")
        with os.fdopen(os.dup(descriptor), "rb") as stream:
            with tarfile.open(fileobj=stream, mode="r:gz") as archive:
                for member in archive:
                    member_count += 1
                    if member_count > MAX_ARCHIVE_MEMBERS:
                        _fail("Taira rollout archive exceeds its member-count bound")
                    name = (
                        member.name.removesuffix("/")
                        if member.isdir()
                        else member.name
                    )
                    try:
                        canonical_relative_path(name)
                    except ReleaseArtifactError as exc:
                        raise TairaReleaseAuthorityError(
                            "Taira rollout archive has an unsafe member: "
                            f"{member.name!r}"
                        ) from exc
                    parts = PurePosixPath(name).parts
                    if not parts or parts[0] != prefix:
                        _fail(
                            "Taira rollout archive contains a member outside "
                            "its exact prefix"
                        )
                    if name in seen:
                        _fail(f"Taira rollout archive repeats member {name!r}")
                    seen.add(name)
                    if member.isdir():
                        if name in expected:
                            _fail(
                                "Taira rollout archive evidence must be a regular "
                                f"file: {name!r}"
                            )
                        continue
                    if not member.isfile() or member.issparse():
                        _fail(
                            "Taira rollout archive must not contain links, sparse "
                            "files, devices, FIFOs, or sockets"
                        )
                    logical_bytes += member.size
                    if logical_bytes > MAX_ARCHIVE_LOGICAL_BYTES:
                        _fail("Taira rollout archive exceeds its logical-size bound")
                    row = expected.get(name)
                    if row is None:
                        continue
                    if member.size != row["size"]:
                        _fail(
                            f"Taira rollout archive evidence size differs for {name!r}"
                        )
                    extracted = archive.extractfile(member)
                    if extracted is None:
                        _fail(
                            f"cannot read Taira rollout archive evidence {name!r}"
                        )
                    digest = hashlib.sha256()
                    remaining = member.size
                    while remaining:
                        chunk = extracted.read(min(1024 * 1024, remaining))
                        if not chunk:
                            _fail(
                                "Taira rollout archive evidence "
                                f"{name!r} is truncated"
                            )
                        digest.update(chunk)
                        remaining -= len(chunk)
                    if extracted.read(1):
                        _fail(
                            f"Taira rollout archive evidence {name!r} exceeds its header"
                        )
                    if digest.hexdigest() != row["sha256"]:
                        _fail(
                            f"Taira rollout archive evidence digest differs for {name!r}"
                        )
                    verified_evidence.add(name)
        closed = os.fstat(descriptor)
        if (
            closed.st_dev != opened.st_dev
            or closed.st_ino != opened.st_ino
            or closed.st_size != opened.st_size
            or closed.st_mtime_ns != opened.st_mtime_ns
            or closed.st_ctime_ns != opened.st_ctime_ns
            or closed.st_nlink != 1
        ):
            _fail("Taira rollout archive changed while it was inspected")
    finally:
        os.close(descriptor)

    missing = sorted(set(expected) - verified_evidence)
    if missing:
        _fail(f"Taira rollout archive omits exact12 evidence: {missing}")
    if stable_hash_path(archive_path) != expected_info:
        _fail("Taira rollout archive path changed during inspection")


def _image_subject(
    manifest_digest: str,
    image_id: str,
    tags: list[str],
    source_digest: str,
) -> dict[str, object]:
    if IMAGE_DIGEST_RE.fullmatch(manifest_digest) is None:
        _fail("OCI manifest digest must be one lowercase sha256 digest")
    if IMAGE_DIGEST_RE.fullmatch(image_id) is None:
        _fail("OCI image ID must be one lowercase sha256 digest")
    if not tags:
        _fail("Taira validator image authority requires published tags")
    if tags != sorted(set(tags)):
        _fail("Taira validator image tags must be unique and canonically sorted")
    for tag in tags:
        if IMAGE_TAG_RE.fullmatch(tag) is None:
            _fail(f"Taira validator image tag is not canonical: {tag!r}")

    immutable_suffix = f":taira-source-{source_digest}"
    immutable = [
        f"hyperledger/iroha{immutable_suffix}",
        f"docker.soramitsu.co.jp/iroha3/iroha{immutable_suffix}",
    ]
    matched = []
    for prefix in immutable:
        matches = [
            tag
            for tag in tags
            if tag == prefix or tag.startswith(f"{prefix}-")
        ]
        if len(matches) != 1:
            _fail(
                "Taira image authority must contain exactly one source-bound "
                f"immutable tag under {prefix!r}"
            )
        matched.extend(matches)
    allowed_latest = {
        "hyperledger/iroha:taira-latest",
        "docker.soramitsu.co.jp/iroha3/iroha:taira-latest",
    }
    extras = set(tags) - set(matched)
    if extras not in (set(), allowed_latest):
        _fail("Taira image authority contains unsupported or partial rolling tags")
    return {
        "image_id": image_id,
        "kind": "taira-validator-oci-image-v1",
        "manifest_digest": manifest_digest,
        "name": "taira-validator",
        "tags": tags,
    }


def _build_untrusted_authority_structure(args: argparse.Namespace) -> dict[str, object]:
    """Build the byte-closure structure without claiming semantic authority."""

    commit = _commit(args.commit)
    dpn_commit = _commit(args.dpn_validator_release_commit)
    artifacts, exact12, source_digest = _evidence(
        Path(args.evidence_root),
        expected_commit=commit,
        expected_dpn_commit=dpn_commit,
    )
    if args.archive is not None:
        archive_path = Path(args.archive)
        subject, archive_info = _archive_subject(archive_path)
        _verify_archive_evidence(archive_path, artifacts, archive_info)
    else:
        subject = _image_subject(
            args.image_manifest_digest,
            args.image_id,
            args.image_tag,
            source_digest,
        )
    return {
        "commit": commit,
        "dpn_validator_release_commit": dpn_commit,
        "exact12": exact12,
        "native_release_evidence": artifacts,
        "native_verifier_protocol": "sorafs-validate-release-manifest-v1",
        "native_verifier_sha256": _sha256(
            args.native_verifier_sha256,
            "native release-manifest verifier SHA-256",
        ),
        "release_profile": "release",
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "signing_authority_fingerprint_sha256": _sha256(
            args.signing_fingerprint,
            "release signing authority fingerprint",
        ),
        "subject": subject,
        "workspace_source_manifest_sha256": source_digest,
    }


def build_authority(args: argparse.Namespace) -> dict[str, object]:
    """Production authority entry point, closed before evidence/path access."""

    require_independent_native_evidence_authority_provisioned()
    return _build_untrusted_authority_structure(args)


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("create", "verify"):
        subparser = subparsers.add_parser(command)
        subparser.add_argument("--evidence-root", required=True)
        subparser.add_argument("--commit", required=True)
        subparser.add_argument("--dpn-validator-release-commit", required=True)
        subparser.add_argument("--signing-fingerprint", required=True)
        subparser.add_argument("--native-verifier-sha256", required=True)
        subject = subparser.add_mutually_exclusive_group(required=True)
        subject.add_argument("--archive")
        subject.add_argument("--image-manifest-digest")
        subparser.add_argument("--image-id")
        subparser.add_argument("--image-tag", action="append", default=[])
        if command == "create":
            subparser.add_argument("--output", required=True)
        else:
            subparser.add_argument("--authority", required=True)
    args = parser.parse_args(argv)
    image_mode = args.image_manifest_digest is not None
    if image_mode != (args.image_id is not None):
        parser.error("--image-manifest-digest and --image-id must be supplied together")
    if not image_mode and args.image_tag:
        parser.error("--image-tag is only valid for an image subject")
    return args


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        expected = build_authority(args)
        expected_bytes = canonical_json_bytes(expected)
        if args.command == "create":
            exclusive_write_bytes(Path(args.output), expected_bytes, mode=0o644)
        else:
            authority = Path(args.authority)
            _, payload = _stable_read(
                Path(os.path.abspath(authority)).parent,
                Path(os.path.abspath(authority)).name,
                maximum=MAX_AUTHORITY_BYTES,
            )
            parsed = load_json_object(payload, "Taira release authority")
            if canonical_json_bytes(parsed) != payload:
                _fail("Taira release authority is not canonical deterministic JSON")
            if parsed != expected:
                _fail("Taira release authority does not match the exact release subject")
    except (
        OSError,
        ReleaseArtifactError,
        TairaReleaseAuthorityError,
        tarfile.TarError,
    ) as exc:
        print(f"Taira release authority error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
