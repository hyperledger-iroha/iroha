"""Tests for source-bound reference SDK supply-chain validation."""

from __future__ import annotations

import hashlib
import hmac
import importlib.util
import json
import os
import sys
from pathlib import Path
from typing import Any

import pytest


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "sorafs_reference_sdk_supply_chain.py"
)
SPEC = importlib.util.spec_from_file_location(
    "sorafs_reference_sdk_supply_chain",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_700_000
GENERATED_AT = NOW_UNIX - 120
DEPLOYMENT_ID = "reference-sdk-release-20260701"
ENVIRONMENT = "production"
MANIFEST_DIGEST = hashlib.sha256(b"release-manifest").hexdigest()
CERTIFICATE_IDENTITY = (
    "https://github.com/hyperledger/iroha/"
    ".github/workflows/sorafs-cli-release.yml@refs/tags/sorafs-cli-v1.0.0"
)
OIDC_ISSUER = "https://token.actions.githubusercontent.com"
VERIFICATION_KEY_FINGERPRINT = hashlib.sha256(
    b"trusted-provenance-verification-key"
).hexdigest()
MappingByTarget = dict[str, Any]
RELEASE_OPERATIONS = (
    "binary_smoke",
    "deterministic_archive_replay",
    "installation",
    "rollback",
    "yank",
)


def write_json(path: Path, payload: Any) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    return path


def read_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(payload, dict)
    return payload


def file_ref(root: Path, path: Path) -> dict[str, str]:
    return {
        "artifact_path": path.relative_to(root).as_posix(),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def receipt_test_signature(message: bytes) -> bytes:
    return hashlib.sha512(b"test-ed25519-boundary\x00" + message).digest()


def sign_release_rehearsal_receipt(payload: dict[str, Any]) -> None:
    payload["signature_hex"] = "01" * 64
    signing_bytes = MODULE.release_rehearsal_receipt_signing_bytes(payload)
    payload["signature_hex"] = receipt_test_signature(signing_bytes).hex()


def sign_provenance_receipt(payload: dict[str, Any]) -> None:
    payload["signature_hex"] = "01" * 64
    signing_bytes = MODULE.provenance_receipt_signing_bytes(payload)
    payload["signature_hex"] = receipt_test_signature(signing_bytes).hex()


def authenticate_test_receipt(
    key_fingerprint: str,
    message: bytes,
    signature: bytes,
) -> bool:
    return key_fingerprint == VERIFICATION_KEY_FINGERPRINT and hmac.compare_digest(
        signature,
        receipt_test_signature(message),
    )


def common(schema: str, *, generated_at: int = GENERATED_AT) -> dict[str, Any]:
    return {
        "schema": schema,
        "generated_at_unix": generated_at,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
        "release_manifest_digest_hex": MANIFEST_DIGEST,
    }


def spdx(name: str) -> dict[str, Any]:
    return {
        "spdxVersion": "SPDX-2.3",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": name,
        "creationInfo": {"creators": ["Tool: syft-1.44.0"]},
        "packages": [{"name": f"{name}-package", "SPDXID": "SPDXRef-Package"}],
    }


def sarif(severities: tuple[str, ...] = ()) -> dict[str, Any]:
    rules = [
        {
            "id": f"CVE-TEST-{index}",
            "properties": {"security-severity": severity},
        }
        for index, severity in enumerate(severities)
    ]
    results = [
        {
            "ruleId": f"CVE-TEST-{index}",
            "message": {"text": "payload-free test finding"},
        }
        for index, _severity in enumerate(severities)
    ]
    return {
        "version": "2.1.0",
        "runs": [
            {
                "tool": {"driver": {"name": "grype", "rules": rules}},
                "results": results,
            }
        ],
    }


def write_source_bundle(
    root: Path,
    *,
    source_severities: tuple[str, ...] = (),
    platform_severities: MappingByTarget | None = None,
    release_statuses: MappingByTarget | None = None,
    provenance_statuses: MappingByTarget | None = None,
) -> dict[str, Path]:
    root.mkdir(parents=True, exist_ok=True)
    platform_severities = platform_severities or {}
    release_statuses = release_statuses or {}
    provenance_statuses = provenance_statuses or {}

    source_sbom = write_json(root / "raw/source.spdx.json", spdx("source"))
    source_report = write_json(
        root / "raw/source-vulnerabilities.sarif",
        sarif(source_severities),
    )

    release_rows: list[dict[str, Any]] = []
    sbom_rows: list[dict[str, Any]] = []
    vulnerability_rows: list[dict[str, Any]] = []
    provenance_rows: list[dict[str, Any]] = []
    for target in MODULE.REQUIRED_RELEASE_TARGETS:
        release_artifact = root / f"raw/{target}.release"
        release_artifact.parent.mkdir(parents=True, exist_ok=True)
        release_artifact.write_bytes(f"release artifact for {target}\n".encode())
        release_artifact_digest = file_ref(root, release_artifact)["sha256"]
        operations = {operation: "passed" for operation in RELEASE_OPERATIONS}
        operations.update(release_statuses.get(target, {}))
        release_receipt_payload = common(
            MODULE.RELEASE_REHEARSAL_RECEIPT_SCHEMA
        )
        release_receipt_payload.update(
            {
                "target": target,
                "subject_sha256": release_artifact_digest,
                "operations": operations,
                "verification_key_fingerprint_hex": (
                    VERIFICATION_KEY_FINGERPRINT
                ),
                "signature_algorithm": "ed25519",
                "signature_hex": "01" * 64,
            }
        )
        sign_release_rehearsal_receipt(release_receipt_payload)
        release_receipt = write_json(
            root / f"receipts/{target}.release.json",
            release_receipt_payload,
        )
        release_rows.append(
            {
                "target": target,
                "release_artifact": file_ref(root, release_artifact),
                "receipt": file_ref(root, release_receipt),
            }
        )

        platform_sbom = write_json(
            root / f"raw/{target}.spdx.json",
            spdx(target),
        )
        sbom_rows.append(
            {
                "target": target,
                "platform_sbom": file_ref(root, platform_sbom),
            }
        )

        platform_report = write_json(
            root / f"raw/{target}-vulnerabilities.sarif",
            sarif(tuple(platform_severities.get(target, ()))),
        )
        vulnerability_rows.append(
            {
                "target": target,
                "platform_report": file_ref(root, platform_report),
            }
        )

        attestation = write_json(
            root / f"raw/{target}.attestation.json",
            {"bundle": "github-attestation", "target": target},
        )
        cosign = write_json(
            root / f"raw/{target}.sigstore.json",
            {"bundle": "cosign", "target": target},
        )
        sha256sums = root / f"raw/{target}/SHA256SUMS"
        sha256sums.parent.mkdir(parents=True, exist_ok=True)
        sha256sums.write_text(
            f"{release_artifact_digest}  {target}.release\n",
            encoding="utf-8",
        )
        sha256sums_cosign = write_json(
            root / f"raw/{target}/SHA256SUMS.sigstore.json",
            {"bundle": "sha256sums-cosign", "target": target},
        )
        statuses = {
            "oidc_identity_status": "verified",
            "cosign_provenance_status": "verified",
        }
        statuses.update(provenance_statuses.get(target, {}))
        provenance_receipt_payload = common(
            MODULE.PROVENANCE_VERIFICATION_RECEIPT_SCHEMA
        )
        provenance_receipt_payload.update(
            {
                "target": target,
                "certificate_identity": CERTIFICATE_IDENTITY,
                "oidc_issuer": OIDC_ISSUER,
                "verification_key_fingerprint_hex": (
                    VERIFICATION_KEY_FINGERPRINT
                ),
                "subject_sha256": release_artifact_digest,
                "attestation_bundle_sha256": file_ref(root, attestation)["sha256"],
                "cosign_bundle_sha256": file_ref(root, cosign)["sha256"],
                "sha256sums_sha256": file_ref(root, sha256sums)["sha256"],
                "sha256sums_cosign_bundle_sha256": file_ref(
                    root,
                    sha256sums_cosign,
                )["sha256"],
                "signature_algorithm": "ed25519",
                "signature_hex": "01" * 64,
                **statuses,
            }
        )
        sign_provenance_receipt(provenance_receipt_payload)
        provenance_receipt = write_json(
            root / f"receipts/{target}.provenance.json",
            provenance_receipt_payload,
        )
        provenance_rows.append(
            {
                "target": target,
                "attestation_bundle": file_ref(root, attestation),
                "cosign_bundle": file_ref(root, cosign),
                "sha256sums": file_ref(root, sha256sums),
                "sha256sums_cosign_bundle": file_ref(
                    root,
                    sha256sums_cosign,
                ),
                "verification_receipt": file_ref(root, provenance_receipt),
            }
        )

    release_rehearsal = common(MODULE.RELEASE_REHEARSAL_SCHEMA)
    release_rehearsal["targets"] = release_rows
    sbom_index = common(MODULE.SBOM_INDEX_SCHEMA)
    sbom_index.update(
        {
            "source_sbom": file_ref(root, source_sbom),
            "targets": sbom_rows,
        }
    )
    vulnerability_report = common(MODULE.VULNERABILITY_REPORT_SCHEMA)
    vulnerability_report.update(
        {
            "source_report": file_ref(root, source_report),
            "targets": vulnerability_rows,
        }
    )
    provenance_bundle = common(MODULE.PROVENANCE_BUNDLE_SCHEMA)
    provenance_bundle.update(
        {
            "certificate_identity": CERTIFICATE_IDENTITY,
            "oidc_issuer": OIDC_ISSUER,
            "verification_key_fingerprint_hex": VERIFICATION_KEY_FINGERPRINT,
            "targets": provenance_rows,
        }
    )

    return {
        "release_rehearsal": write_json(
            root / "release-rehearsal.json",
            release_rehearsal,
        ),
        "sbom_index": write_json(root / "sbom-index.json", sbom_index),
        "vulnerability_report": write_json(
            root / "vulnerability-report.json",
            vulnerability_report,
        ),
        "provenance_bundle": write_json(
            root / "provenance-bundle.json",
            provenance_bundle,
        ),
        "source_sbom": source_sbom,
        "source_report": source_report,
    }

def validate(root: Path, **overrides: Any):
    arguments: dict[str, Any] = {
        "expected_deployment_id": DEPLOYMENT_ID,
        "expected_environment": ENVIRONMENT,
        "expected_release_manifest_digest_hex": MANIFEST_DIGEST,
        "expected_certificate_identity": CERTIFICATE_IDENTITY,
        "expected_verification_key_fingerprint_hex": (
            VERIFICATION_KEY_FINGERPRINT
        ),
        "verification_receipt_authenticator": authenticate_test_receipt,
        "expected_oidc_issuer": OIDC_ISSUER,
        "now_unix": NOW_UNIX,
    }
    arguments.update(overrides)
    return MODULE.validate_supply_chain_sources(root, **arguments)


def update_reference(
    root: Path,
    index_path: Path,
    container: str,
    referenced_path: Path,
    *,
    row: int | None = None,
    field: str | None = None,
) -> None:
    payload = read_json(index_path)
    reference = file_ref(root, referenced_path)
    if row is None:
        payload[container] = reference
    else:
        assert field is not None
        payload[container][row][field] = reference
    write_json(index_path, payload)


def test_valid_bundle_derives_deterministic_canary_fields(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)

    result, errors = validate(tmp_path)

    assert errors == []
    assert result is not None
    assert result.generated_at_unix == GENERATED_AT
    assert [binding.kind for binding in result.source_artifacts] == list(
        MODULE.SOURCE_ARTIFACT_KINDS
    )
    assert [binding.artifact_path for binding in result.source_artifacts] == [
        "release-rehearsal.json",
        "sbom-index.json",
        "vulnerability-report.json",
        "provenance-bundle.json",
    ]
    assert result.sbom_index_digest_hex == hashlib.sha256(
        paths["sbom_index"].read_bytes()
    ).hexdigest()
    assert [row.target for row in result.target_results] == list(
        MODULE.REQUIRED_RELEASE_TARGETS
    )
    assert all(row.binary_smoke_passed for row in result.target_results)
    assert all(row.deterministic_archive_replay_passed for row in result.target_results)
    assert all(row.installation_verified for row in result.target_results)
    assert all(row.rollback_verified for row in result.target_results)
    assert all(row.yank_verified for row in result.target_results)
    assert all(row.sbom_generated for row in result.target_results)
    assert all(row.critical_vulnerability_count == 0 for row in result.target_results)
    assert all(row.high_vulnerability_count == 0 for row in result.target_results)
    assert all(row.oidc_identity_verified for row in result.target_results)
    assert all(row.cosign_provenance_verified for row in result.target_results)
    first = json.dumps(result.to_dict(), sort_keys=True)
    second = json.dumps(result.to_dict(), sort_keys=True)
    assert first == second
    assert result.canary_fields()["source_artifacts"] == result.to_dict()[
        "source_artifacts"
    ]


def test_result_timestamp_is_oldest_validated_constituent(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    oldest = GENERATED_AT - 300
    payload = read_json(paths["sbom_index"])
    payload["generated_at_unix"] = oldest
    write_json(paths["sbom_index"], payload)

    result, errors = validate(tmp_path)

    assert errors == []
    assert result is not None
    assert result.generated_at_unix == oldest


def test_vulnerability_counts_are_derived_from_opened_sarif(tmp_path: Path) -> None:
    first_target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    write_source_bundle(
        tmp_path,
        source_severities=("high",),
        platform_severities={first_target: ("9.8", "medium")},
    )

    result, errors = validate(tmp_path)

    assert errors == []
    assert result is not None
    assert result.target_results[0].critical_vulnerability_count == 1
    assert result.target_results[0].high_vulnerability_count == 1
    assert result.target_results[1].critical_vulnerability_count == 0
    assert result.target_results[1].high_vulnerability_count == 1


def test_failed_receipts_derive_false_results_without_self_assertion(
    tmp_path: Path,
) -> None:
    first_target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    write_source_bundle(
        tmp_path,
        release_statuses={first_target: {"rollback": "failed", "yank": "failed"}},
        provenance_statuses={
            first_target: {
                "oidc_identity_status": "failed",
                "cosign_provenance_status": "failed",
            }
        },
    )

    result, errors = validate(tmp_path)

    assert errors == []
    assert result is not None
    assert result.target_results[0].rollback_verified is False
    assert result.target_results[0].yank_verified is False
    assert result.target_results[0].oidc_identity_verified is False
    assert result.target_results[0].cosign_provenance_verified is False


def test_rejects_schema_aliases_and_extra_fields(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    payload = read_json(paths["sbom_index"])
    payload["schema"] = "sorafs.reference_sdk.sbom_index.v0"
    payload["legacy_alias"] = True
    write_json(paths["sbom_index"], payload)

    result, errors = validate(tmp_path)

    assert result is None
    diagnostics = "\n".join(errors)
    assert "fields must match the schema-closed contract" in diagnostics
    assert "schema must match the canonical v1 source schema" in diagnostics


def test_rejects_duplicate_json_keys(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    paths["release_rehearsal"].write_text(
        '{"schema":"one","schema":"two"}\n',
        encoding="utf-8",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("without duplicate keys" in error for error in errors)


@pytest.mark.parametrize(
    "payload",
    (
        "[" * 10_000 + "]" * 10_000,
        '{"overflow":1e9999}',
    ),
    ids=("deep", "nonfinite"),
)
def test_rejects_unbounded_or_nonfinite_json(tmp_path: Path, payload: str) -> None:
    paths = write_source_bundle(tmp_path)
    paths["release_rehearsal"].write_text(payload, encoding="utf-8")

    result, errors = validate(tmp_path)

    assert result is None
    assert any("strict UTF-8 JSON" in error for error in errors)


@pytest.mark.parametrize(
    ("path_key", "field", "value", "diagnostic"),
    (
        (
            "sbom_index",
            "generated_at_unix",
            GENERATED_AT - MODULE.DEFAULT_MAX_SOURCE_AGE_SECS - 1,
            "exceeds the maximum source age",
        ),
        (
            "vulnerability_report",
            "deployment_id",
            "different-production-release",
            "deployment_id must match",
        ),
        (
            "provenance_bundle",
            "release_manifest_digest_hex",
            hashlib.sha256(b"other-manifest").hexdigest(),
            "must match the reviewed release manifest",
        ),
    ),
)
def test_rejects_stale_or_mismatched_top_level_sources(
    tmp_path: Path,
    path_key: str,
    field: str,
    value: Any,
    diagnostic: str,
) -> None:
    paths = write_source_bundle(tmp_path)
    payload = read_json(paths[path_key])
    payload[field] = value
    write_json(paths[path_key], payload)

    result, errors = validate(tmp_path)

    assert result is None
    assert any(diagnostic in error for error in errors)


def test_rejects_stale_indexed_receipt(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    receipt = tmp_path / f"receipts/{MODULE.REQUIRED_RELEASE_TARGETS[0]}.release.json"
    payload = read_json(receipt)
    payload["generated_at_unix"] = GENERATED_AT - MODULE.DEFAULT_MAX_SOURCE_AGE_SECS - 1
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("exceeds the maximum source age" in error for error in errors)


def test_rejects_indexed_receipt_context_mismatch(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    receipt = tmp_path / f"receipts/{MODULE.REQUIRED_RELEASE_TARGETS[0]}.release.json"
    payload = read_json(receipt)
    payload["environment"] = "staging"
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("environment must match the reviewed context" in error for error in errors)


def test_rejects_missing_reordered_and_duplicate_targets(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    payload = read_json(paths["vulnerability_report"])
    payload["targets"][0], payload["targets"][1] = (
        payload["targets"][1],
        payload["targets"][0],
    )
    payload["targets"][2]["target"] = payload["targets"][1]["target"]
    payload["targets"].pop()
    write_json(paths["vulnerability_report"], payload)

    result, errors = validate(tmp_path)

    assert result is None
    diagnostics = "\n".join(errors)
    assert "exactly five target rows" in diagnostics
    assert "canonical target order" in diagnostics
    assert "must not contain duplicate targets" in diagnostics


def test_rejects_indexed_digest_substitution(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    payload = read_json(paths["sbom_index"])
    payload["source_sbom"]["sha256"] = hashlib.sha256(b"substitution").hexdigest()
    write_json(paths["sbom_index"], payload)

    result, errors = validate(tmp_path)

    assert result is None
    assert any("sha256 must match the indexed file bytes" in error for error in errors)


def test_rejects_relative_path_escape(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    outside = write_json(tmp_path.parent / "outside-sbom.json", spdx("outside"))
    payload = read_json(paths["sbom_index"])
    payload["source_sbom"] = {
        "artifact_path": "../outside-sbom.json",
        "sha256": hashlib.sha256(outside.read_bytes()).hexdigest(),
    }
    write_json(paths["sbom_index"], payload)

    result, errors = validate(tmp_path)

    assert result is None
    assert any("canonical relative path" in error for error in errors)


def test_rejects_symlinked_top_level_source(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    target = tmp_path / "real-release-rehearsal.json"
    paths["release_rehearsal"].replace(target)
    try:
        os.symlink(target, paths["release_rehearsal"])
    except (OSError, NotImplementedError):  # pragma: no cover - platform policy
        pytest.skip("symlinks unavailable")

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must not contain symlinks" in error for error in errors)


def test_rejects_symlinked_indexed_file(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    real = tmp_path / "raw/real-source.spdx.json"
    paths["source_sbom"].replace(real)
    try:
        os.symlink(real, paths["source_sbom"])
    except (OSError, NotImplementedError):  # pragma: no cover - platform policy
        pytest.skip("symlinks unavailable")
    payload = read_json(paths["sbom_index"])
    payload["source_sbom"]["sha256"] = hashlib.sha256(real.read_bytes()).hexdigest()
    write_json(paths["sbom_index"], payload)

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must not contain symlinks" in error for error in errors)


def test_rejects_symlinked_intermediate_directory(tmp_path: Path) -> None:
    write_source_bundle(tmp_path)
    raw = tmp_path / "raw"
    actual_raw = tmp_path / "actual-raw"
    raw.replace(actual_raw)
    try:
        os.symlink(actual_raw, raw)
    except (OSError, NotImplementedError):  # pragma: no cover - platform policy
        pytest.skip("symlinks unavailable")

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must not contain symlinks" in error for error in errors)


def test_rejects_symlinked_source_root(tmp_path: Path) -> None:
    real_root = tmp_path / "real"
    write_source_bundle(real_root)
    linked_root = tmp_path / "linked"
    try:
        os.symlink(real_root, linked_root)
    except (OSError, NotImplementedError):  # pragma: no cover - platform policy
        pytest.skip("symlinks unavailable")

    result, errors = validate(linked_root)

    assert result is None
    assert errors == ["supply-chain source root must not be a symlink"]


def test_fails_closed_without_descriptor_relative_file_access(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_source_bundle(tmp_path)
    monkeypatch.setattr(MODULE.os, "supports_dir_fd", frozenset())

    result, errors = validate(tmp_path)

    assert result is None
    assert errors == [
        "supply-chain validation requires descriptor-relative file access"
    ]


def test_rejects_same_size_mutation_during_file_read(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    paths = write_source_bundle(tmp_path)
    source = paths["release_rehearsal"]
    source.write_text(
        source.read_text(encoding="utf-8") + " " * 70_000,
        encoding="utf-8",
    )
    real_read = os.read
    mutated = False

    def read_then_mutate(descriptor: int, amount: int) -> bytes:
        nonlocal mutated
        chunk = real_read(descriptor, amount)
        if chunk and not mutated:
            with source.open("r+b", buffering=0) as handle:
                handle.seek(-1, os.SEEK_END)
                handle.write(b"\t")
            mutated = True
        return chunk

    monkeypatch.setattr(MODULE.os, "read", read_then_mutate)

    result, errors = validate(tmp_path)

    assert mutated is True
    assert result is None
    assert any("changed while it was being read" in error for error in errors)


def test_rejects_duplicate_indexed_file_identity(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    payload = read_json(paths["sbom_index"])
    payload["targets"][1]["platform_sbom"] = payload["targets"][0]["platform_sbom"]
    write_json(paths["sbom_index"], payload)

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must not duplicate another source file" in error for error in errors)


def test_rejects_hard_linked_indexed_files(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    first_target, second_target = MODULE.REQUIRED_RELEASE_TARGETS[:2]
    first = tmp_path / f"raw/{first_target}.spdx.json"
    second = tmp_path / f"raw/{second_target}.spdx.json"
    second.unlink()
    try:
        os.link(first, second)
    except (OSError, NotImplementedError):  # pragma: no cover - platform policy
        pytest.skip("hard links unavailable")
    update_reference(
        tmp_path,
        paths["sbom_index"],
        "targets",
        second,
        row=1,
        field="platform_sbom",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must not duplicate another source file" in error for error in errors)


def test_rejects_duplicate_top_level_source_identity(tmp_path: Path) -> None:
    write_source_bundle(tmp_path)

    result, errors = validate(
        tmp_path,
        provenance_bundle_path="sbom-index.json",
    )

    assert result is None
    assert any("must not duplicate another source file" in error for error in errors)


def test_rejects_malformed_spdx_after_valid_digest_binding(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    write_json(paths["source_sbom"], {"spdxVersion": "SPDX-1.0"})
    update_reference(
        tmp_path,
        paths["sbom_index"],
        "source_sbom",
        paths["source_sbom"],
    )

    result, errors = validate(tmp_path)

    assert result is None
    diagnostics = "\n".join(errors)
    assert "spdxVersion must be `SPDX-2.3`" in diagnostics
    assert "packages must be a non-empty array" in diagnostics


def test_rejects_sarif_finding_without_explicit_severity(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    malformed = sarif(("high",))
    malformed["runs"][0]["tool"]["driver"]["rules"][0]["properties"] = {}
    write_json(paths["source_report"], malformed)
    update_reference(
        tmp_path,
        paths["vulnerability_report"],
        "source_report",
        paths["source_report"],
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must carry an explicit severity" in error for error in errors)


def test_rejects_oversized_integer_sarif_severity_without_crashing(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    report = tmp_path / f"raw/{target}-vulnerabilities.sarif"
    payload = sarif(("high",))
    payload["runs"][0]["tool"]["driver"]["rules"][0]["properties"][
        "security-severity"
    ] = 10**1_000
    write_json(report, payload)
    update_reference(
        tmp_path,
        paths["vulnerability_report"],
        "targets",
        report,
        row=0,
        field="platform_report",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must carry a recognized severity" in error for error in errors)


def test_rejects_untrusted_provenance_identity_and_issuer(tmp_path: Path) -> None:
    write_source_bundle(tmp_path)

    result, errors = validate(
        tmp_path,
        expected_certificate_identity="https://github.com/other/workflow",
        expected_oidc_issuer="https://issuer.example",
    )

    assert result is None
    diagnostics = "\n".join(errors)
    assert "must match the trusted identity" in diagnostics
    assert "must match the trusted issuer" in diagnostics


def test_rejects_release_receipt_subject_not_bound_to_indexed_artifact(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.release.json"
    payload = read_json(receipt)
    payload["subject_sha256"] = hashlib.sha256(b"different release").hexdigest()
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "subject_sha256 must match the indexed release artifact" in error
        for error in errors
    )


def test_receipt_signing_domains_are_distinct_and_schema_closed(
    tmp_path: Path,
) -> None:
    write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    release_receipt = read_json(
        tmp_path / f"receipts/{target}.release.json"
    )
    provenance_receipt = read_json(
        tmp_path / f"receipts/{target}.provenance.json"
    )

    release_signing_bytes = MODULE.release_rehearsal_receipt_signing_bytes(
        release_receipt
    )
    provenance_signing_bytes = MODULE.provenance_receipt_signing_bytes(
        provenance_receipt
    )

    assert release_signing_bytes != provenance_signing_bytes
    assert authenticate_test_receipt(
        release_receipt["verification_key_fingerprint_hex"],
        release_signing_bytes,
        bytes.fromhex(release_receipt["signature_hex"]),
    )
    assert authenticate_test_receipt(
        provenance_receipt["verification_key_fingerprint_hex"],
        provenance_signing_bytes,
        bytes.fromhex(provenance_receipt["signature_hex"]),
    )
    with pytest.raises(ValueError, match="wrong exact schema"):
        MODULE.release_rehearsal_receipt_signing_bytes(provenance_receipt)
    with pytest.raises(ValueError, match="wrong exact schema"):
        MODULE.provenance_receipt_signing_bytes(release_receipt)


@pytest.mark.parametrize(
    ("missing_field", "diagnostic"),
    (
        (
            "verification_key_fingerprint_hex",
            "verification_key_fingerprint_hex must be non-zero lowercase SHA-256",
        ),
        (
            "signature_algorithm",
            "signature_algorithm must be `ed25519`",
        ),
        (
            "signature_hex",
            "signature_hex must be a non-zero lowercase Ed25519 signature",
        ),
    ),
)
def test_rejects_unsigned_or_incomplete_release_rehearsal_receipt(
    tmp_path: Path,
    missing_field: str,
    diagnostic: str,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.release.json"
    payload = read_json(receipt)
    payload.pop(missing_field)
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    diagnostics = "\n".join(errors)
    assert "fields must match the schema-closed contract" in diagnostics
    assert diagnostic in diagnostics


def test_rejects_tampered_release_rehearsal_signature(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.release.json"
    payload = read_json(receipt)
    payload["signature_hex"] = "ab" * 64
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "signature must authenticate with the trusted verifier" in error
        for error in errors
    )


def test_rejects_substituted_release_rehearsal_signature(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    first_target, second_target = MODULE.REQUIRED_RELEASE_TARGETS[:2]
    first_receipt = tmp_path / f"receipts/{first_target}.release.json"
    second_receipt = tmp_path / f"receipts/{second_target}.release.json"
    first_payload = read_json(first_receipt)
    first_payload["signature_hex"] = read_json(second_receipt)["signature_hex"]
    write_json(first_receipt, first_payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        first_receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "signature must authenticate with the trusted verifier" in error
        for error in errors
    )


def test_rejects_cross_type_receipt_signature_substitution(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    release_receipt = tmp_path / f"receipts/{target}.release.json"
    provenance_receipt = tmp_path / f"receipts/{target}.provenance.json"
    release_payload = read_json(release_receipt)
    release_payload["signature_hex"] = read_json(provenance_receipt)[
        "signature_hex"
    ]
    write_json(release_receipt, release_payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        release_receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "signature must authenticate with the trusted verifier" in error
        for error in errors
    )


def test_rejects_release_rehearsal_signature_from_substituted_key(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.release.json"
    payload = read_json(receipt)
    payload["verification_key_fingerprint_hex"] = hashlib.sha256(
        b"substituted-release-rehearsal-key"
    ).hexdigest()
    sign_release_rehearsal_receipt(payload)
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "verification_key_fingerprint_hex must match the trusted verifier"
        in error
        for error in errors
    )


def test_rejects_signed_provenance_subject_for_another_artifact(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.provenance.json"
    payload = read_json(receipt)
    payload["subject_sha256"] = hashlib.sha256(b"different release").hexdigest()
    sign_provenance_receipt(payload)
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["provenance_bundle"],
        "targets",
        receipt,
        row=0,
        field="verification_receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "subject_sha256 must match the indexed release artifact" in error
        for error in errors
    )


def test_rejects_forged_provenance_verification_receipt(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.provenance.json"
    payload = read_json(receipt)
    payload["signature_hex"] = "ab" * 64
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["provenance_bundle"],
        "targets",
        receipt,
        row=0,
        field="verification_receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "signature must authenticate with the trusted verifier" in error
        for error in errors
    )


def test_rejects_missing_provenance_authenticator(tmp_path: Path) -> None:
    write_source_bundle(tmp_path)

    result, errors = validate(
        tmp_path,
        verification_receipt_authenticator=None,
    )

    assert result is None
    assert errors == ["verification receipt authenticator must be callable"]


def test_rejects_provenance_receipt_not_bound_to_opened_bundle(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    attestation = tmp_path / f"raw/{target}.attestation.json"
    write_json(attestation, {"bundle": "tampered"})
    update_reference(
        tmp_path,
        paths["provenance_bundle"],
        "targets",
        attestation,
        row=0,
        field="attestation_bundle",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "attestation_bundle_sha256 must match the opened bundle" in error
        for error in errors
    )


def test_valid_provenance_rows_bind_signed_candidate_checksums(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)

    result, errors = validate(tmp_path)

    assert errors == []
    assert result is not None
    provenance = read_json(paths["provenance_bundle"])
    for index, target in enumerate(MODULE.REQUIRED_RELEASE_TARGETS):
        row = provenance["targets"][index]
        assert set(row) == {
            "target",
            "attestation_bundle",
            "cosign_bundle",
            "sha256sums",
            "sha256sums_cosign_bundle",
            "verification_receipt",
        }
        assert row["target"] == target
        receipt = read_json(tmp_path / row["verification_receipt"]["artifact_path"])
        sha256sums = tmp_path / row["sha256sums"]["artifact_path"]
        sha256sums_cosign = (
            tmp_path / row["sha256sums_cosign_bundle"]["artifact_path"]
        )
        assert receipt["sha256sums_sha256"] == hashlib.sha256(
            sha256sums.read_bytes()
        ).hexdigest()
        assert receipt[
            "sha256sums_cosign_bundle_sha256"
        ] == hashlib.sha256(sha256sums_cosign.read_bytes()).hexdigest()


def test_rejects_noncanonical_provenance_checksum_schema(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    provenance = read_json(paths["provenance_bundle"])
    provenance["targets"][0]["checksums_compatibility_alias"] = provenance[
        "targets"
    ][0]["sha256sums"]
    write_json(paths["provenance_bundle"], provenance)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    receipt = tmp_path / f"receipts/{target}.provenance.json"
    receipt_payload = read_json(receipt)
    receipt_payload["checksums_digest_compatibility_alias"] = receipt_payload[
        "sha256sums_sha256"
    ]
    write_json(receipt, receipt_payload)
    update_reference(
        tmp_path,
        paths["provenance_bundle"],
        "targets",
        receipt,
        row=0,
        field="verification_receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    diagnostics = "\n".join(errors)
    assert (
        "provenance_bundle source.targets[0] fields must match the "
        "schema-closed contract"
    ) in diagnostics
    assert (
        "provenance_bundle verification receipt[0] fields must match the "
        "schema-closed contract"
    ) in diagnostics


@pytest.mark.parametrize(
    ("field", "expected_error"),
    (
        (
            "sha256sums",
            "sha256sums_sha256 must match the opened SHA256SUMS",
        ),
        (
            "sha256sums_cosign_bundle",
            "sha256sums_cosign_bundle_sha256 must match the opened bundle",
        ),
    ),
)
def test_rejects_mutated_candidate_checksum_provenance(
    tmp_path: Path,
    field: str,
    expected_error: str,
) -> None:
    paths = write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    if field == "sha256sums":
        artifact = tmp_path / f"raw/{target}/SHA256SUMS"
        artifact.write_text(
            f"{hashlib.sha256(b'tampered').hexdigest()}  tampered.release\n",
            encoding="utf-8",
        )
    else:
        artifact = tmp_path / f"raw/{target}/SHA256SUMS.sigstore.json"
        write_json(
            artifact,
            {"bundle": "tampered-sha256sums-cosign", "target": target},
        )
    update_reference(
        tmp_path,
        paths["provenance_bundle"],
        "targets",
        artifact,
        row=0,
        field=field,
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any(expected_error in error for error in errors)


def test_rejects_cross_target_sha256sums_cosign_bundle(
    tmp_path: Path,
) -> None:
    paths = write_source_bundle(tmp_path)
    provenance = read_json(paths["provenance_bundle"])
    first = provenance["targets"][0]
    second = provenance["targets"][1]
    first["sha256sums_cosign_bundle"], second[
        "sha256sums_cosign_bundle"
    ] = (
        second["sha256sums_cosign_bundle"],
        first["sha256sums_cosign_bundle"],
    )
    write_json(paths["provenance_bundle"], provenance)

    result, errors = validate(tmp_path)

    assert result is None
    assert sum(
        "sha256sums_cosign_bundle_sha256 must match the opened bundle" in error
        for error in errors
    ) == 2


@pytest.mark.parametrize(
    "filename",
    ("SHA256SUMS", "SHA256SUMS.sigstore.json"),
)
def test_rejects_missing_candidate_checksum_provenance_file(
    tmp_path: Path,
    filename: str,
) -> None:
    write_source_bundle(tmp_path)
    target = MODULE.REQUIRED_RELEASE_TARGETS[0]
    (tmp_path / f"raw/{target}/{filename}").unlink()

    result, errors = validate(tmp_path)

    assert result is None
    assert any(
        "provenance_bundle SHA256SUMS" in error
        and "must reference an existing regular file" in error
        for error in errors
    )


def test_rejects_invalid_release_operation_status(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    receipt = tmp_path / f"receipts/{MODULE.REQUIRED_RELEASE_TARGETS[0]}.release.json"
    payload = read_json(receipt)
    payload["operations"]["rollback"] = ["claimed"]
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["release_rehearsal"],
        "targets",
        receipt,
        row=0,
        field="receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must be `passed` or `failed`" in error for error in errors)


def test_rejects_non_scalar_provenance_status(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    receipt = (
        tmp_path
        / f"receipts/{MODULE.REQUIRED_RELEASE_TARGETS[0]}.provenance.json"
    )
    payload = read_json(receipt)
    payload["oidc_identity_status"] = {"status": "verified"}
    sign_provenance_receipt(payload)
    write_json(receipt, payload)
    update_reference(
        tmp_path,
        paths["provenance_bundle"],
        "targets",
        receipt,
        row=0,
        field="verification_receipt",
    )

    result, errors = validate(tmp_path)

    assert result is None
    assert any("must be `verified` or `failed`" in error for error in errors)


def test_rejects_missing_canonical_source_artifact(tmp_path: Path) -> None:
    paths = write_source_bundle(tmp_path)
    paths["provenance_bundle"].unlink()

    result, errors = validate(tmp_path)

    assert result is None
    diagnostics = "\n".join(errors)
    assert "must reference an existing regular file" in diagnostics
    assert "must contain all four canonical source artifacts" in diagnostics
