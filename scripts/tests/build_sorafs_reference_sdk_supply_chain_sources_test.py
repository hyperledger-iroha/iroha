"""Tests for the workflow-owned SF-11 supply-chain source assembler."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Any

import pytest


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "build_sorafs_reference_sdk_supply_chain_sources.py"
)
SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_reference_sdk_supply_chain_sources",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import sccp_release_common as RELEASE_CRYPTO  # noqa: E402
import sorafs_reference_sdk_supply_chain as SOURCES  # noqa: E402


NOW_UNIX = 1_800_700_000
GENERATED_AT_UNIX = NOW_UNIX - 120
DEPLOYMENT_ID = "reference-sdk-release-20260701"
ENVIRONMENT = "production"
VERSION = "1.0.0"
CERTIFICATE_IDENTITY = (
    "https://github.com/hyperledger/iroha/"
    ".github/workflows/sorafs-cli-release.yml@refs/tags/sorafs-cli-v1.0.0"
)
OIDC_ISSUER = "https://token.actions.githubusercontent.com"
SIGNING_SEED = hashlib.sha256(b"sf11-source-assembler-test-key").digest()


def public_key_from_seed(seed: bytes) -> bytes:
    """Derive the temporary Ed25519 public key used by this test module."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return RELEASE_CRYPTO._ed_encode(  # noqa: SLF001 - test-only signer
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            scalar,
        )
    )


def sign(seed: bytes, message: bytes) -> bytes:
    """Sign with an in-memory seed that never enters a fixture or source file."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    prefix = digest[32:]
    public_key = public_key_from_seed(seed)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little")
    nonce %= RELEASE_CRYPTO._ED_L  # noqa: SLF001
    encoded_r = RELEASE_CRYPTO._ed_encode(  # noqa: SLF001
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            nonce,
        )
    )
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(),
        "little",
    ) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    scalar_bytes = (
        (nonce + challenge * scalar) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    ).to_bytes(32, "little")
    return encoded_r + scalar_bytes


PUBLIC_KEY = public_key_from_seed(SIGNING_SEED)
PUBLIC_KEY_HEX = PUBLIC_KEY.hex()
PUBLIC_KEY_FINGERPRINT = hashlib.sha256(PUBLIC_KEY).hexdigest()


def write_json(path: Path, payload: Any) -> Path:
    """Write canonical JSON fixture bytes."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            payload,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    return path


def sha256(path: Path) -> str:
    """Return one fixture file digest."""

    return hashlib.sha256(path.read_bytes()).hexdigest()


def common(schema: str) -> dict[str, Any]:
    """Return common source or receipt fields."""

    return {
        "schema": schema,
        "generated_at_unix": GENERATED_AT_UNIX,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
    }


def spdx(name: str) -> dict[str, Any]:
    """Return one minimal valid SPDX document."""

    return {
        "spdxVersion": "SPDX-2.3",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": name,
        "creationInfo": {"creators": ["Tool: syft-1.44.0"]},
        "packages": [{"name": name, "SPDXID": "SPDXRef-Package"}],
    }


def sarif() -> dict[str, Any]:
    """Return one valid vulnerability-free SARIF report."""

    return {
        "version": "2.1.0",
        "runs": [
            {
                "tool": {"driver": {"name": "grype", "rules": []}},
                "results": [],
            }
        ],
    }


def build_fixture(root: Path, receipts_root: Path) -> None:
    """Create the exact signed-job and external-receipt directory contracts."""

    release_manifest = write_json(
        root / "release-authentication/release_manifest.json",
        {"schema": "sorafs.cli.release-manifest.v1", "version": VERSION},
    )
    manifest_digest = sha256(release_manifest)
    source_sbom_payload = spdx("source-release")
    source_report_payload = sarif()
    aggregate_attestation_payload = {
        "bundle": "github-aggregate-multi-subject-attestation",
        "subjects": [
            {"target": target} for target in SOURCES.REQUIRED_RELEASE_TARGETS
        ],
    }
    for target in SOURCES.REQUIRED_RELEASE_TARGETS:
        candidate = root / "release-candidates" / f"sorafs-cli-{VERSION}-{target}"
        archive_name = f"sorafs-cli-{VERSION}-{target}.tar.gz"
        archive = candidate / "platform-archive" / archive_name
        archive.parent.mkdir(parents=True, exist_ok=True)
        archive.write_bytes(f"release archive for {target}\n".encode())
        archive_digest = sha256(archive)
        summary = {
            "schema": MODULE.RELEASE_CANDIDATE_SCHEMA,
            "status": "verified",
            "version": VERSION,
            "target": target,
            "archive": archive_name,
            "archive_sha256": archive_digest,
            "manifest": f"sorafs-cli-{VERSION}-{target}.manifest.json",
            "manifest_sha256": hashlib.sha256(target.encode()).hexdigest(),
            "payload_file_count": 21,
            "clean_smoke_binary_count": 3,
        }
        write_json(candidate / "platform-archive/candidate-package-first.json", summary)
        write_json(candidate / "platform-archive/candidate-package-replay.json", summary)
        write_json(candidate / "sorafs-release.spdx.json", source_sbom_payload)
        write_json(
            candidate / "sorafs-release-vulnerabilities.sarif",
            source_report_payload,
        )
        write_json(candidate / f"sorafs-cli-{target}.spdx.json", spdx(target))
        write_json(
            candidate / f"sorafs-cli-{target}-vulnerabilities.sarif",
            sarif(),
        )
        checksum_lines = [
            f"{sha256(path)}  {path.relative_to(candidate).as_posix()}"
            for path in sorted(candidate.rglob("*"))
            if path.is_file()
        ]
        sha256sums = candidate / "SHA256SUMS"
        sha256sums.write_text("\n".join(checksum_lines) + "\n", encoding="utf-8")
        attestation = write_json(
            root / "github-attestations" / f"{target}.json",
            aggregate_attestation_payload,
        )
        cosign = write_json(
            archive.with_name(archive.name + ".sigstore.json"),
            {"bundle": "cosign", "target": target},
        )
        sha256sums_cosign = write_json(
            candidate / "SHA256SUMS.sigstore.json",
            {"bundle": "sha256sums-cosign", "target": target},
        )

        release_receipt = common(SOURCES.RELEASE_REHEARSAL_RECEIPT_SCHEMA)
        release_receipt.update(
            {
                "release_manifest_digest_hex": manifest_digest,
                "target": target,
                "subject_sha256": archive_digest,
                "verification_key_fingerprint_hex": PUBLIC_KEY_FINGERPRINT,
                "operations": {
                    "binary_smoke": "passed",
                    "deterministic_archive_replay": "passed",
                    "installation": "passed",
                    "rollback": "passed",
                    "yank": "passed",
                },
                "signature_algorithm": "ed25519",
                "signature_hex": "00" * 64,
            }
        )
        release_receipt["signature_hex"] = sign(
            SIGNING_SEED,
            SOURCES.release_rehearsal_receipt_signing_bytes(release_receipt),
        ).hex()
        write_json(
            receipts_root / MODULE.RELEASE_RECEIPT_SUBDIRECTORY / f"{target}.json",
            release_receipt,
        )

        provenance_receipt = common(
            SOURCES.PROVENANCE_VERIFICATION_RECEIPT_SCHEMA
        )
        provenance_receipt.update(
            {
                "release_manifest_digest_hex": manifest_digest,
                "target": target,
                "certificate_identity": CERTIFICATE_IDENTITY,
                "oidc_issuer": OIDC_ISSUER,
                "verification_key_fingerprint_hex": PUBLIC_KEY_FINGERPRINT,
                "subject_sha256": archive_digest,
                "attestation_bundle_sha256": sha256(attestation),
                "cosign_bundle_sha256": sha256(cosign),
                "sha256sums_sha256": sha256(sha256sums),
                "sha256sums_cosign_bundle_sha256": sha256(
                    sha256sums_cosign
                ),
                "oidc_identity_status": "verified",
                "cosign_provenance_status": "verified",
                "signature_algorithm": "ed25519",
                "signature_hex": "00" * 64,
            }
        )
        signing_bytes = SOURCES.provenance_receipt_signing_bytes(
            provenance_receipt
        )
        provenance_receipt["signature_hex"] = sign(
            SIGNING_SEED,
            signing_bytes,
        ).hex()
        write_json(
            receipts_root
            / MODULE.PROVENANCE_RECEIPT_SUBDIRECTORY
            / f"{target}.json",
            provenance_receipt,
        )


def build(root: Path, receipts_root: Path) -> dict[str, Any]:
    """Invoke the source assembler with the reviewed fixture context."""

    return MODULE.build_sources(
        source_root=root,
        external_receipts_root=receipts_root,
        version=VERSION,
        deployment_id=DEPLOYMENT_ID,
        environment=ENVIRONMENT,
        generated_at_unix=GENERATED_AT_UNIX,
        now_unix=NOW_UNIX,
        provenance_certificate_identity=CERTIFICATE_IDENTITY,
        provenance_oidc_issuer=OIDC_ISSUER,
        provenance_verification_public_key_hex=PUBLIC_KEY_HEX,
    )


def reopen_sources(
    root: Path,
) -> tuple[SOURCES.SupplyChainSourceResult | None, list[str]]:
    """Reopen a retained source tree through the production validator."""

    manifest_digest = sha256(
        root / "release-authentication/release_manifest.json"
    )

    def authenticate(
        claimed_fingerprint: str,
        message: bytes,
        signature: bytes,
    ) -> bool:
        return MODULE.secrets.compare_digest(
            claimed_fingerprint,
            PUBLIC_KEY_FINGERPRINT,
        ) and RELEASE_CRYPTO.verify_ed25519(PUBLIC_KEY, signature, message)

    return SOURCES.validate_supply_chain_sources(
        root,
        expected_deployment_id=DEPLOYMENT_ID,
        expected_environment=ENVIRONMENT,
        expected_release_manifest_digest_hex=manifest_digest,
        expected_certificate_identity=CERTIFICATE_IDENTITY,
        expected_verification_key_fingerprint_hex=PUBLIC_KEY_FINGERPRINT,
        verification_receipt_authenticator=authenticate,
        now_unix=NOW_UNIX,
        expected_oidc_issuer=OIDC_ISSUER,
    )


def test_builds_and_reopens_exact_four_source_indexes(tmp_path: Path) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)

    summary = build(root, receipts_root)

    assert summary["schema"] == MODULE.SUMMARY_SCHEMA
    assert summary["status"] == "validated"
    assert summary["generated_at_unix"] == GENERATED_AT_UNIX
    assert summary["release_manifest_digest_hex"] == sha256(
        root / "release-authentication/release_manifest.json"
    )
    assert summary["provenance_verification_key_fingerprint_hex"] == (
        PUBLIC_KEY_FINGERPRINT
    )
    assert [artifact["kind"] for artifact in summary["source_artifacts"]] == list(
        SOURCES.SOURCE_ARTIFACT_KINDS
    )
    assert {
        path.name for path in root.glob("*.json")
    } == set(SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values())
    provenance = json.loads((root / "provenance-bundle.json").read_text())
    assert provenance["certificate_identity"] == CERTIFICATE_IDENTITY
    assert provenance["oidc_issuer"] == OIDC_ISSUER
    assert provenance["verification_key_fingerprint_hex"] == (
        PUBLIC_KEY_FINGERPRINT
    )
    assert [row["target"] for row in provenance["targets"]] == list(
        SOURCES.REQUIRED_RELEASE_TARGETS
    )
    assert all(
        set(row)
        == {
            "target",
            "attestation_bundle",
            "cosign_bundle",
            "sha256sums",
            "sha256sums_cosign_bundle",
            "verification_receipt",
        }
        for row in provenance["targets"]
    )
    assert len(
        {
            row["attestation_bundle"]["sha256"]
            for row in provenance["targets"]
        }
    ) == 1


def test_missing_external_receipt_fails_before_writing_indexes(
    tmp_path: Path,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    (
        receipts_root
        / MODULE.RELEASE_RECEIPT_SUBDIRECTORY
        / f"{SOURCES.REQUIRED_RELEASE_TARGETS[0]}.json"
    ).unlink()

    with pytest.raises(
        MODULE.SourceBuildError,
        match="must contain exactly five canonical target files",
    ):
        build(root, receipts_root)

    assert not any(
        (root / path).exists()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert not (root / MODULE.EVIDENCE_SUBDIRECTORY).exists()


def test_bad_external_provenance_signature_leaves_failed_workspace_untouched(
    tmp_path: Path,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    receipt = (
        receipts_root
        / MODULE.PROVENANCE_RECEIPT_SUBDIRECTORY
        / f"{SOURCES.REQUIRED_RELEASE_TARGETS[0]}.json"
    )
    payload = json.loads(receipt.read_text())
    payload["signature_hex"] = "01" * 64
    write_json(receipt, payload)

    with pytest.raises(
        MODULE.SourceBuildError,
        match="failed canonical source validation",
    ):
        build(root, receipts_root)

    assert all(
        (root / path).is_file()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert (root / MODULE.EVIDENCE_SUBDIRECTORY).is_dir()


def test_tampered_release_receipt_leaves_failed_workspace_untouched(
    tmp_path: Path,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    receipt = (
        receipts_root
        / MODULE.RELEASE_RECEIPT_SUBDIRECTORY
        / f"{SOURCES.REQUIRED_RELEASE_TARGETS[0]}.json"
    )
    payload = json.loads(receipt.read_text())
    payload["operations"]["rollback"] = "failed"
    write_json(receipt, payload)

    with pytest.raises(
        MODULE.SourceBuildError,
        match="failed canonical source validation",
    ):
        build(root, receipts_root)

    assert all(
        (root / path).is_file()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert (root / MODULE.EVIDENCE_SUBDIRECTORY).is_dir()


def test_replay_summary_drift_fails_closed(tmp_path: Path) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    target = SOURCES.REQUIRED_RELEASE_TARGETS[0]
    replay = (
        root
        / "release-candidates"
        / f"sorafs-cli-{VERSION}-{target}"
        / "platform-archive"
        / "candidate-package-replay.json"
    )
    payload = json.loads(replay.read_text())
    payload["clean_smoke_binary_count"] = 2
    write_json(replay, payload)

    with pytest.raises(
        MODULE.SourceBuildError,
        match="replay summary must be byte-identical",
    ):
        build(root, receipts_root)

    assert not any(
        (root / path).exists()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )


def test_source_parent_swap_between_stat_and_open_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    watched = root / "release-candidates"
    displaced = root / "release-candidates-original"
    real_open = MODULE.os.open
    swapped = False

    def swapping_open(
        path: Any,
        flags: int,
        mode: int = 0o777,
        *,
        dir_fd: int | None = None,
    ) -> int:
        nonlocal swapped
        if not swapped and path == "release-candidates" and dir_fd is not None:
            watched.rename(displaced)
            watched.mkdir()
            swapped = True
        return real_open(path, flags, mode, dir_fd=dir_fd)

    monkeypatch.setattr(MODULE.os, "open", swapping_open)
    try:
        with pytest.raises(
            MODULE.SourceBuildError,
            match="parent directory changed while it was opened",
        ):
            build(root, receipts_root)
    finally:
        if swapped:
            watched.rmdir()
            displaced.rename(watched)

    assert swapped
    assert not any(
        (root / path).exists()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert not (root / MODULE.EVIDENCE_SUBDIRECTORY).exists()


def test_external_receipt_parent_swap_between_stat_and_open_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    watched = receipts_root / MODULE.RELEASE_RECEIPT_SUBDIRECTORY
    displaced = receipts_root / "release-rehearsal-original"
    real_open = MODULE.os.open
    swapped = False

    def swapping_open(
        path: Any,
        flags: int,
        mode: int = 0o777,
        *,
        dir_fd: int | None = None,
    ) -> int:
        nonlocal swapped
        if (
            not swapped
            and path == MODULE.RELEASE_RECEIPT_SUBDIRECTORY
            and dir_fd is not None
        ):
            watched.rename(displaced)
            watched.mkdir()
            swapped = True
        return real_open(path, flags, mode, dir_fd=dir_fd)

    monkeypatch.setattr(MODULE.os, "open", swapping_open)
    try:
        with pytest.raises(
            MODULE.SourceBuildError,
            match="external release-rehearsal receipt directory changed "
            "while it was opened",
        ):
            build(root, receipts_root)
    finally:
        if swapped:
            watched.rmdir()
            displaced.rename(watched)

    assert swapped
    assert not any(
        (root / path).exists()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert not (root / MODULE.EVIDENCE_SUBDIRECTORY).exists()


def test_source_hard_link_is_rejected_before_publication(tmp_path: Path) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    target = SOURCES.REQUIRED_RELEASE_TARGETS[0]
    source_sbom = (
        root
        / "release-candidates"
        / f"sorafs-cli-{VERSION}-{target}"
        / "sorafs-release.spdx.json"
    )
    MODULE.os.link(source_sbom, root / "unexpected-source-sbom-hardlink.json")

    with pytest.raises(
        MODULE.SourceBuildError,
        match="must have exactly one hard link",
    ):
        build(root, receipts_root)

    assert not any(
        (root / path).exists()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert not (root / MODULE.EVIDENCE_SUBDIRECTORY).exists()


def test_failure_never_deletes_substituted_or_original_outputs(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    escaped_evidence = root / "escaped-evidence"
    replacement_evidence = root / MODULE.EVIDENCE_SUBDIRECTORY
    sentinel = replacement_evidence / "sentinel.txt"
    destructive_calls: list[str] = []

    def forbidden_delete(*_args: Any, **_kwargs: Any) -> None:
        destructive_calls.append("delete")
        raise AssertionError("source assembly must never delete failure outputs")

    monkeypatch.setattr(MODULE.os, "unlink", forbidden_delete)
    monkeypatch.setattr(MODULE.os, "rmdir", forbidden_delete)

    def substitute_before_validation(
        _source_root: Path,
        **_kwargs: Any,
    ) -> tuple[None, list[str]]:
        replacement_evidence.rename(escaped_evidence)
        replacement_evidence.mkdir()
        sentinel.write_text("failure path must not delete\n", encoding="utf-8")
        return None, ["forced validation failure"]

    monkeypatch.setattr(
        MODULE,
        "validate_supply_chain_sources",
        substitute_before_validation,
    )
    with pytest.raises(
        MODULE.SourceBuildError,
        match="failed canonical source validation",
    ):
        build(root, receipts_root)

    assert destructive_calls == []
    assert sentinel.read_text(encoding="utf-8") == "failure path must not delete\n"
    assert escaped_evidence.is_dir()
    assert {
        path.name for path in escaped_evidence.iterdir()
    } == {
        MODULE.RELEASE_RECEIPT_SUBDIRECTORY,
        MODULE.PROVENANCE_RECEIPT_SUBDIRECTORY,
    }
    assert all(
        (root / path).is_file()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )


def test_post_validator_source_index_replacement_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    original_validator = MODULE.validate_supply_chain_sources
    displaced = root / "release-rehearsal.validated"
    replacement = b'{"attacker_substitution":true}\n'

    def replace_after_validation(
        source_root: Path,
        **kwargs: Any,
    ) -> tuple[SOURCES.SupplyChainSourceResult | None, list[str]]:
        result, errors = original_validator(source_root, **kwargs)
        source_index = root / "release-rehearsal.json"
        source_index.rename(displaced)
        source_index.write_bytes(replacement)
        return result, errors

    monkeypatch.setattr(
        MODULE,
        "validate_supply_chain_sources",
        replace_after_validation,
    )
    with pytest.raises(
        MODULE.SourceBuildError,
        match="changed after it was created",
    ):
        build(root, receipts_root)

    assert (root / "release-rehearsal.json").read_bytes() == replacement
    assert displaced.is_file()
    assert all(
        (root / path).is_file()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
        if path != "release-rehearsal.json"
    )
    assert (root / MODULE.EVIDENCE_SUBDIRECTORY).is_dir()


def test_post_validator_in_place_source_index_mutation_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    original_validator = MODULE.validate_supply_chain_sources
    mutated = False

    def mutate_after_validation(
        source_root: Path,
        **kwargs: Any,
    ) -> tuple[SOURCES.SupplyChainSourceResult | None, list[str]]:
        nonlocal mutated
        result, errors = original_validator(source_root, **kwargs)
        source_index = root / "release-rehearsal.json"
        with source_index.open("r+b", buffering=0) as handle:
            first = handle.read(1)
            assert first == b"{"
            handle.seek(0)
            handle.write(b"[")
        mutated = True
        return result, errors

    monkeypatch.setattr(
        MODULE,
        "validate_supply_chain_sources",
        mutate_after_validation,
    )
    with pytest.raises(MODULE.SourceBuildError):
        build(root, receipts_root)

    assert mutated
    source_index = root / "release-rehearsal.json"
    assert source_index.read_bytes().startswith(b"[")
    assert all(
        (root / path).is_file()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert (root / MODULE.EVIDENCE_SUBDIRECTORY).is_dir()


def test_post_validator_hard_link_of_source_index_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    original_validator = MODULE.validate_supply_chain_sources
    added_link = root / "release-rehearsal.post-validation-hardlink"

    def link_after_validation(
        source_root: Path,
        **kwargs: Any,
    ) -> tuple[SOURCES.SupplyChainSourceResult | None, list[str]]:
        result, errors = original_validator(source_root, **kwargs)
        MODULE.os.link(root / "release-rehearsal.json", added_link)
        return result, errors

    monkeypatch.setattr(
        MODULE,
        "validate_supply_chain_sources",
        link_after_validation,
    )
    with pytest.raises(
        MODULE.SourceBuildError,
        match="must have exactly one hard link",
    ):
        build(root, receipts_root)

    assert added_link.is_file()
    assert (root / "release-rehearsal.json").is_file()
    assert all(
        (root / path).is_file()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )


def test_partial_publication_cannot_validate_and_rerun_refuses_names(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    real_write_exclusive = MODULE._write_exclusive

    def interrupt_after_second_source_index(
        directory: Any,
        name: str,
        payload: bytes,
        *,
        label: str,
    ) -> Any:
        created = real_write_exclusive(
            directory,
            name,
            payload,
            label=label,
        )
        if label == "sbom_index canonical source index":
            raise MODULE.SourceBuildError("simulated partial publication")
        return created

    monkeypatch.setattr(
        MODULE,
        "_write_exclusive",
        interrupt_after_second_source_index,
    )
    with pytest.raises(
        MODULE.SourceBuildError,
        match="simulated partial publication",
    ):
        build(root, receipts_root)

    partial_paths = (
        root / "release-rehearsal.json",
        root / "sbom-index.json",
    )
    assert all(path.is_file() for path in partial_paths)
    assert not (root / "vulnerability-report.json").exists()
    assert not (root / "provenance-bundle.json").exists()
    assert (root / MODULE.EVIDENCE_SUBDIRECTORY).is_dir()
    partial_digests = {path.name: sha256(path) for path in partial_paths}

    replay, replay_errors = reopen_sources(root)
    assert replay is None
    assert replay_errors

    with pytest.raises(
        MODULE.SourceBuildError,
        match="must not already exist",
    ):
        build(root, receipts_root)
    assert {path.name: sha256(path) for path in partial_paths} == partial_digests
    assert (root / MODULE.EVIDENCE_SUBDIRECTORY).is_dir()


def test_default_temporary_directory_ancestor_alias_is_portable() -> None:
    with tempfile.TemporaryDirectory() as temporary:
        base = Path(temporary)
        root = base / "signed-release"
        receipts_root = base / "external-receipts"
        build_fixture(root, receipts_root)

        summary = build(root, receipts_root)

        assert summary["status"] == "validated"


def test_success_fsyncs_generated_directories_bottom_up(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    build_fixture(root, receipts_root)
    real_fsync = MODULE.os.fsync
    real_fstat = MODULE.os.fstat
    synchronized_directories: set[tuple[int, int]] = set()
    synchronized_sequence: list[tuple[int, int]] = []

    def recording_fsync(descriptor: int) -> None:
        metadata = real_fstat(descriptor)
        if MODULE.stat.S_ISDIR(metadata.st_mode):
            identity = (metadata.st_dev, metadata.st_ino)
            synchronized_directories.add(identity)
            synchronized_sequence.append(identity)
        real_fsync(descriptor)

    monkeypatch.setattr(MODULE.os, "fsync", recording_fsync)
    summary = build(root, receipts_root)

    assert summary["status"] == "validated"
    generated_directories = (
        root,
        root / MODULE.EVIDENCE_SUBDIRECTORY,
        root
        / MODULE.EVIDENCE_SUBDIRECTORY
        / MODULE.RELEASE_RECEIPT_SUBDIRECTORY,
        root
        / MODULE.EVIDENCE_SUBDIRECTORY
        / MODULE.PROVENANCE_RECEIPT_SUBDIRECTORY,
    )
    generated_identities = tuple(
        (path.stat().st_dev, path.stat().st_ino) for path in generated_directories
    )
    assert {
        (path.stat().st_dev, path.stat().st_ino)
        for path in generated_directories
    } <= synchronized_directories
    assert tuple(synchronized_sequence[-4:]) == (
        generated_identities[2],
        generated_identities[3],
        generated_identities[1],
        generated_identities[0],
    )


def test_root_open_closes_descriptor_when_handoff_is_interrupted(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "root"
    root.mkdir()
    real_open = MODULE.os.open
    real_fstat = MODULE.os.fstat
    opened: list[int] = []

    def recording_open(*args: Any, **kwargs: Any) -> int:
        descriptor = real_open(*args, **kwargs)
        opened.append(descriptor)
        return descriptor

    def interrupting_fstat(descriptor: int) -> Any:
        if descriptor in opened:
            raise KeyboardInterrupt
        return real_fstat(descriptor)

    monkeypatch.setattr(MODULE.os, "open", recording_open)
    monkeypatch.setattr(MODULE.os, "fstat", interrupting_fstat)
    with pytest.raises(KeyboardInterrupt):
        MODULE._open_root_directory(root, label="interruptible root")

    assert len(opened) == 1
    with pytest.raises(OSError):
        real_fstat(opened[0])


def test_failed_child_directory_fsync_closes_descriptor_without_deleting_name(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    parent_path = tmp_path / "parent"
    parent_path.mkdir()
    parent = MODULE._open_root_directory(parent_path, label="parent")
    real_open = MODULE.os.open
    real_fstat = MODULE.os.fstat
    opened_child: list[int] = []

    def recording_open(*args: Any, **kwargs: Any) -> int:
        descriptor = real_open(*args, **kwargs)
        if args and args[0] == "child":
            opened_child.append(descriptor)
        return descriptor

    monkeypatch.setattr(MODULE.os, "open", recording_open)
    real_fsync = MODULE.os.fsync

    def fail_child_only(descriptor: int) -> None:
        if opened_child and descriptor == opened_child[-1]:
            raise OSError("simulated directory fsync failure")
        real_fsync(descriptor)

    monkeypatch.setattr(MODULE.os, "fsync", fail_child_only)
    try:
        with pytest.raises(
            MODULE.SourceBuildError,
            match="could not be synchronized durably",
        ):
            MODULE._create_child_directory(parent, "child", label="child")
    finally:
        MODULE._close_directory(parent)

    assert len(opened_child) == 1
    with pytest.raises(OSError):
        real_fstat(opened_child[0])
    assert (parent_path / "child").is_dir()


def test_symlinked_root_ancestor_is_allowed_but_symlink_leaf_is_rejected(
    tmp_path: Path,
) -> None:
    canonical_parent = tmp_path / "canonical-parent"
    canonical_parent.mkdir()
    alias_parent = tmp_path / "ancestor-alias"
    alias_parent.symlink_to(canonical_parent, target_is_directory=True)
    root = alias_parent / "signed-release"
    receipts_root = alias_parent / "external-receipts"
    build_fixture(root, receipts_root)

    summary = build(root, receipts_root)

    assert summary["status"] == "validated"

    leaf_alias = tmp_path / "source-leaf-alias"
    leaf_alias.symlink_to(root.resolve(), target_is_directory=True)
    with pytest.raises(MODULE.SourceBuildError, match="must not be a symlink"):
        MODULE.build_sources(
            source_root=leaf_alias,
            external_receipts_root=receipts_root,
            version=VERSION,
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
            generated_at_unix=GENERATED_AT_UNIX,
            now_unix=NOW_UNIX,
            provenance_certificate_identity=CERTIFICATE_IDENTITY,
            provenance_oidc_issuer=OIDC_ISSUER,
            provenance_verification_public_key_hex=PUBLIC_KEY_HEX,
        )


def test_retargeted_root_ancestor_alias_fails_final_binding_check(
    tmp_path: Path,
) -> None:
    original_parent = tmp_path / "original-parent"
    substitute_parent = tmp_path / "substitute-parent"
    original_parent.mkdir()
    substitute_parent.mkdir()
    original_root = original_parent / "root"
    substitute_root = substitute_parent / "root"
    original_root.mkdir()
    substitute_root.mkdir()
    alias_parent = tmp_path / "ancestor-alias"
    alias_parent.symlink_to(original_parent, target_is_directory=True)
    lexical_root = alias_parent / "root"
    handle = MODULE._open_root_directory(lexical_root, label="aliased root")

    try:
        alias_parent.unlink()
        alias_parent.symlink_to(substitute_parent, target_is_directory=True)

        with pytest.raises(
            MODULE.SourceBuildError,
            match="path changed during source assembly",
        ):
            MODULE._verify_root_binding(handle)
    finally:
        MODULE._close_directory(handle)


def test_root_leaf_swap_to_symlink_during_resolution_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "root"
    root.mkdir()
    displaced = tmp_path / "root-displaced"
    real_lstat = MODULE.Path.lstat
    swapped = False

    def swapping_lstat(path: Path) -> Any:
        nonlocal swapped
        if not swapped and path == root:
            root.rename(displaced)
            root.symlink_to(displaced, target_is_directory=True)
            swapped = True
        return real_lstat(path)

    monkeypatch.setattr(MODULE.Path, "lstat", swapping_lstat)
    try:
        with pytest.raises(MODULE.SourceBuildError, match="must not be a symlink"):
            MODULE._open_root_directory(root, label="raced root")
    finally:
        if swapped:
            root.unlink()
            displaced.rename(root)

    assert swapped


def test_retained_source_tree_replays_without_external_receipts(
    tmp_path: Path,
) -> None:
    root = tmp_path / "signed-release"
    receipts_root = tmp_path / "external-receipts"
    retained_root = tmp_path / "retained-source-tree"
    build_fixture(root, receipts_root)
    build(root, receipts_root)

    original, original_errors = reopen_sources(root)
    assert original_errors == []
    assert original is not None
    shutil.copytree(root, retained_root)
    shutil.rmtree(receipts_root)
    shutil.rmtree(root)

    replay, replay_errors = reopen_sources(retained_root)
    assert replay_errors == []
    assert replay is not None
    assert replay.to_dict() == original.to_dict()

    missing_receipt = (
        retained_root
        / MODULE.EVIDENCE_SUBDIRECTORY
        / MODULE.RELEASE_RECEIPT_SUBDIRECTORY
        / f"{SOURCES.REQUIRED_RELEASE_TARGETS[0]}.json"
    )
    missing_receipt.unlink()
    failed_replay, failed_errors = reopen_sources(retained_root)
    assert failed_replay is None
    assert failed_errors
