#!/usr/bin/env python3
"""Authenticate and sign one already-built unsigned Taira rollout archive.

This is the authority half of the two-phase Taira release contract.  It never
invokes Cargo, Git, the validator, the privacy evidence runner, Kagami, or any
other executable produced by the source build.  The only child executables are
the externally provisioned signer and the independently pinned native release
manifest verifier, both invoked through ``release_manifest_signing``'s stable
snapshot and minimal-environment boundary.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import platform
import re
import sys
from pathlib import Path
from types import SimpleNamespace

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from generate_release_manifest import build_release_manifest
from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_json_bytes,
    create_fresh_directory,
    exclusive_write_bytes,
    scan_inventory_paths,
    stable_read_relative,
    stable_hash_relative,
    stable_read_path,
)
from release_manifest_signing import (
    ReleaseManifestSignatureError,
    sign_release_manifest,
    verify_release_manifest,
)
from seal_taira_release_controllers import (
    MANIFEST_NAME as CONTROLLER_MANIFEST_NAME,
)
from seal_taira_release_controllers import (
    verify as verify_controller_closure,
)
from taira_release_authority import (
    AUTHORITY_ENVELOPE_SUFFIX,
    DURABLE_RECEIPT_SUFFIX,
    AuthorizedAuthority,
    TairaReleaseAuthorityError,
    build_authority,
    require_independent_native_evidence_authority_provisioned,
)

COMMIT_RE = re.compile(r"[0-9a-f]{40}")
SHA256_RE = re.compile(r"[0-9a-f]{64}")
AUTHORITY_PAYLOAD = "taira-exact12-release-authority-v1.json"
AUTHORITY_ENVELOPE = AUTHORITY_PAYLOAD + AUTHORITY_ENVELOPE_SUFFIX
DURABLE_RECEIPT = AUTHORITY_PAYLOAD + DURABLE_RECEIPT_SUFFIX
ARTIFACT_SPECS = (
    f"iroha3:taira-exact12:release-evidence:json:{AUTHORITY_PAYLOAD}",
    f"iroha3:taira-authority:release-evidence:json:{AUTHORITY_ENVELOPE}",
    f"iroha3:taira-authority:release-evidence:json:{DURABLE_RECEIPT}",
    "iroha3:taira-authority:release-evidence:binary:release_artifact_contract.py",
    "iroha3:taira-authority:reference-validator:binary:sorafs-validate",
    "iroha3:taira-authority:release-evidence:binary:taira_release_authority.py",
    f"iroha3:taira-authority:release-evidence:json:{CONTROLLER_MANIFEST_NAME}",
)
ARTIFACT_FILES = (
    AUTHORITY_PAYLOAD,
    AUTHORITY_ENVELOPE,
    DURABLE_RECEIPT,
    "release_artifact_contract.py",
    "sorafs-validate",
    "taira_release_authority.py",
    CONTROLLER_MANIFEST_NAME,
)
PUBLIC_PRIVACY_INPUTS = {
    "bootle_lantern_broker_public.json": (
        "provenance/privacy-bootstrap/bootle_lantern_broker_public.json",
        4 * 1024 * 1024,
    ),
    "config.toml": ("provenance/privacy-bootstrap/config.toml", 8 * 1024 * 1024),
    "genesis.json": ("provenance/privacy-bootstrap/genesis.json", 16 * 1024 * 1024),
    "nevo-reset.review.json": (
        "provenance/privacy-bootstrap/nevo-reset.review.json",
        4 * 1024 * 1024,
    ),
    "privacy_bootstrap_plan.json": (
        "provenance/privacy-bootstrap/privacy_bootstrap_plan.json",
        8 * 1024 * 1024,
    ),
}


class FinalizationError(RuntimeError):
    """The unsigned archive cannot cross the release authority boundary."""


def _require_independent_native_evidence_authority() -> None:
    """Translate the shared provisioning barrier into finalizer's error."""

    try:
        require_independent_native_evidence_authority_provisioned()
    except TairaReleaseAuthorityError as exc:
        raise FinalizationError(str(exc)) from exc


def _canonical_absolute(path: Path, label: str, *, must_exist: bool = True) -> Path:
    if not path.is_absolute():
        raise FinalizationError(f"{label} must be an absolute path")
    absolute = Path(os.path.abspath(path))
    if path != absolute:
        raise FinalizationError(
            f"{label} must use its canonical physical path without symlink aliases"
        )
    comparison = absolute if must_exist else absolute.parent
    try:
        canonical = comparison.resolve(strict=True)
    except OSError as exc:
        raise FinalizationError(f"cannot resolve {label}: {exc}") from exc
    expected = comparison
    if canonical != expected:
        raise FinalizationError(
            f"{label} must use its canonical physical path without symlink aliases"
        )
    if must_exist and not absolute.exists():
        raise FinalizationError(f"{label} does not exist")
    return absolute


def _external_authority_file(path: Path, label: str, checkout_root: Path) -> Path:
    absolute = _canonical_absolute(path, label)
    if absolute == checkout_root or checkout_root in absolute.parents:
        raise FinalizationError(f"{label} must be provisioned outside the checkout")
    return absolute


def _copy_stable(source: Path, destination: Path, *, mode: int, maximum: int) -> bytes:
    before, payload = stable_read_path(source, max_size=maximum)
    if not payload:
        raise FinalizationError(f"authority input is empty: {source}")
    exclusive_write_bytes(destination, payload, mode=mode)
    after, replay = stable_read_path(source, max_size=maximum)
    if after != before or replay != payload:
        raise FinalizationError(f"authority input changed while copied: {source}")
    return payload


def _write_checksums(artifacts: Path) -> None:
    if scan_inventory_paths(artifacts) != sorted(ARTIFACT_FILES):
        raise FinalizationError("authority artifact inventory is not exactly closed")
    captured = {
        name: stable_hash_relative(artifacts, name) for name in sorted(ARTIFACT_FILES)
    }
    if scan_inventory_paths(artifacts) != sorted(ARTIFACT_FILES):
        raise FinalizationError("authority artifact inventory changed while hashed")
    for name, before in captured.items():
        if stable_hash_relative(artifacts, name) != before:
            raise FinalizationError(f"authority artifact changed while hashed: {name}")
    payload = "".join(
        f"{captured[name].sha256}  {name}\n" for name in sorted(captured)
    ).encode("ascii")
    exclusive_write_bytes(artifacts / "SHA256SUMS", payload)


def _authority_args(args: argparse.Namespace, evidence_root: Path, archive: Path):
    return SimpleNamespace(
        command="create",
        evidence_root=str(evidence_root),
        commit=args.commit,
        dpn_validator_release_commit=args.dpn_validator_release_commit,
        signing_fingerprint=args.trusted_signing_fingerprint,
        native_verifier_sha256=args.trusted_release_manifest_verifier_sha256,
        archive=str(archive),
        image_manifest_digest=None,
        image_id=None,
        image_tag=[],
    )


def _verify_public_privacy_inputs(
    public_input_root: Path,
    evidence_root: Path,
) -> dict[str, str]:
    expected_names = sorted(PUBLIC_PRIVACY_INPUTS)
    if scan_inventory_paths(public_input_root) != expected_names:
        raise FinalizationError(
            "trusted public privacy input inventory is not exactly five files"
        )
    digests: dict[str, str] = {}
    captures: dict[str, tuple[object, bytes, object, bytes]] = {}
    for name in expected_names:
        bundled_relative, maximum = PUBLIC_PRIVACY_INPUTS[name]
        trusted_info, trusted_payload = stable_read_relative(
            public_input_root,
            name,
            max_size=maximum,
            return_payload=True,
        )
        bundled_info, bundled_payload = stable_read_relative(
            evidence_root,
            bundled_relative,
            max_size=maximum,
            return_payload=True,
        )
        if (
            trusted_payload != bundled_payload
            or trusted_info.sha256 != bundled_info.sha256
        ):
            raise FinalizationError(
                f"unsigned archive substituted the trusted public privacy input: {name}"
            )
        digests[name] = trusted_info.sha256
        assert trusted_payload is not None
        assert bundled_payload is not None
        captures[name] = (
            trusted_info,
            trusted_payload,
            bundled_info,
            bundled_payload,
        )
    if scan_inventory_paths(public_input_root) != expected_names:
        raise FinalizationError(
            "trusted public privacy input inventory changed during finalization"
        )
    # Replay both sides after every comparison.  Checking each pair only once
    # leaves an earlier path replaceable while a later file is inspected.
    for name in expected_names:
        bundled_relative, maximum = PUBLIC_PRIVACY_INPUTS[name]
        trusted_info, trusted_payload = stable_read_relative(
            public_input_root,
            name,
            max_size=maximum,
            return_payload=True,
        )
        bundled_info, bundled_payload = stable_read_relative(
            evidence_root,
            bundled_relative,
            max_size=maximum,
            return_payload=True,
        )
        if (
            captures[name]
            != (trusted_info, trusted_payload, bundled_info, bundled_payload)
            or trusted_payload != bundled_payload
        ):
            raise FinalizationError(
                f"public privacy input changed during exact byte comparison: {name}"
            )
    return digests


def _manifest_args(
    args: argparse.Namespace,
    artifacts: Path,
    manifest: Path,
    workspace_source_manifest_sha256: str,
):
    return SimpleNamespace(
        artifacts_dir=str(artifacts),
        version=f"taira-{workspace_source_manifest_sha256[:16]}",
        commit=args.commit,
        source_date_epoch=args.source_date_epoch,
        os_tag="linux",
        arch="aarch64",
        artifact=list(ARTIFACT_SPECS),
        output=str(manifest),
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-root", required=True)
    parser.add_argument("--archive", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--source-date-epoch", required=True)
    parser.add_argument("--checkout-root", required=True)
    parser.add_argument("--public-privacy-input-dir", required=True)
    parser.add_argument("--controller-manifest", required=True)
    parser.add_argument("--controller-digest", required=True)
    parser.add_argument("--external-signer", required=True)
    parser.add_argument("--signing-public-key", required=True)
    parser.add_argument("--trusted-signing-fingerprint", required=True)
    parser.add_argument("--release-manifest-verifier", required=True)
    parser.add_argument(
        "--trusted-release-manifest-verifier-sha256",
        required=True,
    )
    return parser.parse_args(argv)


def finalize(args: argparse.Namespace) -> dict[str, object]:
    # This must be the first operation: do not inspect paths, controller state,
    # signer material, public inputs, or output state for untrusted native bytes.
    _require_independent_native_evidence_authority()

    if platform.system() != "Linux" or platform.machine() != "aarch64":
        raise FinalizationError(
            "Taira Linux authority finalization requires native Linux aarch64"
        )
    if COMMIT_RE.fullmatch(args.commit) is None:
        raise FinalizationError("release commit must be exactly 40 lowercase hex")
    if COMMIT_RE.fullmatch(args.dpn_validator_release_commit) is None:
        raise FinalizationError(
            "DPN validator release commit must be exactly 40 lowercase hex"
        )
    for label, value in (
        ("trusted signing fingerprint", args.trusted_signing_fingerprint),
        (
            "trusted release-manifest verifier SHA-256",
            args.trusted_release_manifest_verifier_sha256,
        ),
        ("controller closure digest", args.controller_digest),
    ):
        if SHA256_RE.fullmatch(value) is None:
            raise FinalizationError(f"{label} must be exactly 64 lowercase hex")

    evidence_root = _canonical_absolute(Path(args.evidence_root), "evidence root")
    archive = _canonical_absolute(Path(args.archive), "unsigned rollout archive")
    output = _canonical_absolute(
        Path(args.output_dir), "authority output", must_exist=False
    )
    checkout_root = _canonical_absolute(Path(args.checkout_root), "checkout root")
    public_input_root = _canonical_absolute(
        Path(args.public_privacy_input_dir), "trusted public privacy input directory"
    )
    if not checkout_root.is_dir() or checkout_root.is_symlink():
        raise FinalizationError("checkout root must be a non-symlink directory")
    controller_manifest = _canonical_absolute(
        Path(args.controller_manifest), "controller manifest"
    )
    controller_root = SCRIPT_DIR.parent
    if controller_manifest != controller_root / CONTROLLER_MANIFEST_NAME:
        raise FinalizationError(
            "controller manifest must be the sibling manifest of the sealed finalizer"
        )
    try:
        verify_controller_closure(
            controller_root,
            args.controller_digest,
            "linux",
            args.commit,
        )
    except Exception as exc:
        raise FinalizationError(
            f"sealed controller closure verification failed: {exc}"
        ) from exc
    signer = _external_authority_file(
        Path(args.external_signer), "external signer", checkout_root
    )
    public_key = _external_authority_file(
        Path(args.signing_public_key), "signing public key", checkout_root
    )
    native_verifier = _external_authority_file(
        Path(args.release_manifest_verifier),
        "native release-manifest verifier",
        checkout_root,
    )
    if not evidence_root.is_dir() or evidence_root.is_symlink():
        raise FinalizationError("evidence root must be a non-symlink directory")
    if not public_input_root.is_dir() or public_input_root.is_symlink():
        raise FinalizationError(
            "trusted public privacy input directory must be a non-symlink directory"
        )
    expected_archive = evidence_root.parent / f"{evidence_root.name}.tar.gz"
    expected_output = evidence_root.parent / f"{evidence_root.name}.authority"
    if archive != expected_archive:
        raise FinalizationError("archive name must exactly match its evidence root")
    if output != expected_output:
        raise FinalizationError("authority output must exactly match its evidence root")

    public_input_digests = _verify_public_privacy_inputs(
        public_input_root,
        evidence_root,
    )

    authorized: AuthorizedAuthority = build_authority(
        _authority_args(args, evidence_root, archive)
    )
    authority = authorized.subject
    authority_envelope_payload = authorized.authority_envelope
    durable_receipt_payload = authorized.durable_receipt
    if authority.get("dpn_validator_release_commit") != args.dpn_validator_release_commit:
        raise FinalizationError("authority returned the wrong DPN validator release commit")
    workspace_digest = authority["workspace_source_manifest_sha256"]
    if (
        not isinstance(workspace_digest, str)
        or SHA256_RE.fullmatch(workspace_digest) is None
    ):
        raise FinalizationError("authority returned an invalid workspace source digest")

    create_fresh_directory(output, mode=0o700)
    artifacts = create_fresh_directory(output / "artifacts", mode=0o755)
    bundled_scripts = evidence_root / "scripts"
    for name, mode in (
        ("release_artifact_contract.py", 0o644),
        ("taira_release_authority.py", 0o755),
    ):
        source_payload = _copy_stable(
            SCRIPT_DIR / name,
            artifacts / name,
            mode=mode,
            maximum=4 * 1024 * 1024,
        )
        _, bundled_payload = stable_read_path(
            bundled_scripts / name,
            max_size=4 * 1024 * 1024,
        )
        if bundled_payload != source_payload:
            raise FinalizationError(
                f"unsigned archive does not carry the exact finalizer helper: {name}"
            )
    verifier_payload = _copy_stable(
        native_verifier,
        artifacts / "sorafs-validate",
        mode=0o755,
        maximum=256 * 1024 * 1024,
    )
    if hashlib.sha256(verifier_payload).hexdigest() != (
        args.trusted_release_manifest_verifier_sha256
    ):
        raise FinalizationError(
            "copied native verifier differs from its trusted digest"
        )
    _copy_stable(
        controller_manifest,
        artifacts / CONTROLLER_MANIFEST_NAME,
        mode=0o644,
        maximum=4 * 1024 * 1024,
    )

    authority_payload = canonical_json_bytes(authority)
    exclusive_write_bytes(artifacts / AUTHORITY_PAYLOAD, authority_payload)
    exclusive_write_bytes(artifacts / AUTHORITY_ENVELOPE, authority_envelope_payload)
    exclusive_write_bytes(artifacts / DURABLE_RECEIPT, durable_receipt_payload)
    _write_checksums(artifacts)

    manifest = output / "release_manifest.json"
    manifest_args = _manifest_args(args, artifacts, manifest, workspace_digest)
    manifest_payload = canonical_json_bytes(build_release_manifest(manifest_args))
    exclusive_write_bytes(manifest, manifest_payload)

    signature = output / "release_manifest.json.sig"
    installed_public_key = output / "release_manifest.json.pub"
    sign_release_manifest(
        manifest,
        signer,
        public_key,
        args.trusted_signing_fingerprint,
        signature,
        installed_public_key,
        native_verifier,
        args.trusted_release_manifest_verifier_sha256,
    )
    verify_release_manifest(
        manifest,
        signature,
        installed_public_key,
        args.trusted_signing_fingerprint,
        artifacts / "sorafs-validate",
        args.trusted_release_manifest_verifier_sha256,
    )

    replay_manifest = canonical_json_bytes(build_release_manifest(manifest_args))
    if replay_manifest != manifest_payload:
        raise FinalizationError("release manifest replay differs after signing")
    replay: AuthorizedAuthority = build_authority(
        _authority_args(args, evidence_root, archive)
    )
    replay_authority = canonical_json_bytes(replay.subject)
    if replay_authority != authority_payload:
        raise FinalizationError("release authority subject changed after signing")
    if (
        replay.authority_envelope != authority_envelope_payload
        or replay.durable_receipt != durable_receipt_payload
    ):
        raise FinalizationError(
            "release authority authenticated sidecars changed after signing"
        )
    final_inventory = scan_inventory_paths(output)
    expected_inventory = sorted(
        [
            "release_manifest.json",
            "release_manifest.json.pub",
            "release_manifest.json.sig",
            "artifacts/SHA256SUMS",
            *[f"artifacts/{name}" for name in ARTIFACT_FILES],
        ]
    )
    if final_inventory != expected_inventory:
        raise FinalizationError(
            "signed authority output inventory is not exactly closed"
        )
    if _verify_public_privacy_inputs(public_input_root, evidence_root) != (
        public_input_digests
    ):
        raise FinalizationError(
            "trusted public privacy input digest set changed after signing"
        )
    return {
        "archive": str(archive),
        "authority_dir": str(output),
        "commit": args.commit,
        "dpn_validator_release_commit": args.dpn_validator_release_commit,
        "controller_digest": args.controller_digest,
        "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
        "public_privacy_input_sha256": public_input_digests,
        "workspace_source_manifest_sha256": workspace_digest,
    }


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        result = finalize(args)
    except (
        FinalizationError,
        OSError,
        ReleaseArtifactError,
        ReleaseManifestSignatureError,
        TairaReleaseAuthorityError,
    ) as exc:
        print(f"Taira authority finalization error: {exc}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(canonical_json_bytes(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
