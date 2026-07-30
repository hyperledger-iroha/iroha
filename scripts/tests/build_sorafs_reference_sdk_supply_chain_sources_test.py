"""Tests for the workflow-owned SF-11 supply-chain source assembler."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
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
        attestation = write_json(
            root / "github-attestations" / f"{target}.json",
            {"bundle": "github-attestation", "target": target},
        )
        cosign = write_json(
            archive.with_name(archive.name + ".sigstore.json"),
            {"bundle": "cosign", "target": target},
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


def test_bad_external_provenance_signature_removes_generated_outputs(
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

    assert not any(
        (root / path).exists()
        for path in SOURCES.DEFAULT_SOURCE_ARTIFACT_PATHS.values()
    )
    assert not (root / MODULE.EVIDENCE_SUBDIRECTORY).exists()


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
