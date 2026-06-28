"""Tests for scripts/check_sorafs_reference_sdk_release_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "check_sorafs_reference_sdk_release_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reference_sdk_release_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_700_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "12" * 32
DIGEST_2 = "34" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "reference-sdk-release-2026-06",
        "environment": "release",
        "deployment_context_reviewed": True,
    }


def release_archive(*, target_count: int = 4, missing_target: bool = False) -> dict:
    targets = [
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ]
    if missing_target:
        targets.pop()
    payload = base("sorafs.reference_sdk.release_archive_canary.v1")
    payload.update(
        {
            "packaging_helper_used": True,
            "deterministic_archive_verified": True,
            "archive_checksums_published": True,
            "binary_checksums_published": True,
            "dist_gitkeep_only_tracked": True,
            "target_count": target_count,
            "targets": targets,
            "archive_index_digest_hex": DIGEST,
            "release_manifest_digest_hex": DIGEST,
            "raw_archives_included": False,
        }
    )
    return payload


def signed_manifest(*, private_key_absent: bool = True) -> dict:
    payload = base("sorafs.reference_sdk.signed_manifest_canary.v1")
    payload.update(
        {
            "manifest_signed": True,
            "manifest_signature_verified": True,
            "manifest_sha256_published": True,
            "governed_release_key_used": True,
            "public_key_fingerprint_recorded": True,
            "private_key_absent": private_key_absent,
            "signature_algorithm": "ed25519",
            "manifest_digest_hex": DIGEST,
            "public_key_fingerprint_hex": DIGEST,
            "raw_manifest_included": False,
        }
    )
    return payload


def downstream_bindings(*, package_count: int = 5, missing_package: bool = False) -> dict:
    packages = ["javascript", "python", "kotlin_jvm", "java_android", "swift"]
    if missing_package:
        packages.pop()
    payload = base("sorafs.reference_sdk.downstream_bindings_canary.v1")
    payload.update(
        {
            "packages": packages,
            "package_count": package_count,
            "sdk_exports_verified": True,
            "validation_outcome_contract_verified": True,
            "version_alignment_verified": True,
            "native_bridge_header_bound": True,
            "published_package_digests_recorded": True,
            "release_manifest_digest_hex": DIGEST,
            "package_index_digest_hex": DIGEST,
            "raw_packages_included": False,
        }
    )
    return payload


def cookbook_smoke(*, duration: int = 600) -> dict:
    payload = base("sorafs.reference_sdk.cookbook_smoke_canary.v1")
    payload.update(
        {
            "published_archive_smoke_passed": True,
            "cookbook_replay_passed": True,
            "fixture_bundle_validation_passed": True,
            "manifest_car_replay_passed": True,
            "validation_outcomes_emitted": True,
            "smoke_duration_seconds": duration,
            "release_manifest_digest_hex": DIGEST,
            "smoke_output_digest_hex": DIGEST,
            "raw_smoke_outputs_included": False,
        }
    )
    return payload


def ffi_header_contract(*, ci_guard: bool = True) -> dict:
    payload = base("sorafs.reference_sdk.ffi_header_contract_canary.v1")
    payload.update(
        {
            "ci_guard_passed": ci_guard,
            "rust_exports_match_header": True,
            "selector_constants_match": True,
            "c_signatures_match": True,
            "bridge_bindings_verified": True,
            "release_manifest_digest_hex": DIGEST,
            "header_digest_hex": DIGEST,
            "ffi_contract_digest_hex": DIGEST,
            "raw_header_included": False,
        }
    )
    return payload


def governance_approval(*, source: str = "governed_release") -> dict:
    payload = base("sorafs.reference_sdk.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "release_key_roster_bound": True,
            "release_targets_bound": True,
            "downstream_packages_bound": True,
            "smoke_evidence_bound": True,
            "governance_source": source,
            "release_manifest_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "release-archive.json", release_archive())
    write_json(root / "signed-manifest.json", signed_manifest())
    write_json(root / "downstream-bindings.json", downstream_bindings())
    write_json(root / "cookbook-smoke.json", cookbook_smoke())
    write_json(root / "ffi-header-contract.json", ffi_header_contract())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_release_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.reference_sdk.release_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["release_archive"]["valid"] is True


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "reference-sdk.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_release_archive_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "release-archive.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_release_archive_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = release_archive()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "release-archive.json", payload)

    assert run_gate(tmp_path) == 1


def test_raw_archive_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = release_archive()
    payload["raw_archive"] = "leaked"
    write_json(tmp_path / "release-archive.json", payload)

    assert run_gate(tmp_path) == 1


def test_release_archive_requires_all_targets(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "release-archive.json", release_archive(missing_target=True))

    assert run_gate(tmp_path) == 1


def test_release_archive_requires_minimum_target_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "release-archive.json", release_archive(target_count=3))

    assert run_gate(tmp_path) == 1


def test_signed_manifest_rejects_private_key_presence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "signed-manifest.json", signed_manifest(private_key_absent=False))

    assert run_gate(tmp_path) == 1


def test_stale_signed_manifest_does_not_anchor_release_bound_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = signed_manifest()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "signed-manifest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["release_archive"]
    artifact = required["artifacts"][0]
    assert payload["valid_release_manifest_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "release_archive release_manifest_digest_hex requires a valid "
        "signed_manifest manifest_digest_hex"
    ]


def test_downstream_bindings_require_all_packages(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "downstream-bindings.json",
        downstream_bindings(missing_package=True),
    )

    assert run_gate(tmp_path) == 1


def test_downstream_bindings_require_minimum_package_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "downstream-bindings.json", downstream_bindings(package_count=4))

    assert run_gate(tmp_path) == 1


def test_downstream_bindings_require_release_manifest_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = downstream_bindings()
    del payload["release_manifest_digest_hex"]
    write_json(tmp_path / "downstream-bindings.json", payload)

    assert run_gate(tmp_path) == 1


def test_release_archive_manifest_digest_must_match_signed_manifest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = release_archive()
    payload["release_manifest_digest_hex"] = DIGEST_2
    write_json(tmp_path / "release-archive.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["release_archive"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "release_archive release_manifest_digest_hex must reference a valid "
        "signed_manifest manifest_digest_hex"
    ]


def test_cookbook_smoke_duration_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "cookbook-smoke.json", cookbook_smoke(duration=4_000))

    assert run_gate(tmp_path) == 1


def test_ffi_contract_requires_ci_guard(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "ffi-header-contract.json", ffi_header_contract(ci_guard=False))

    assert run_gate(tmp_path) == 1


def test_governance_source_must_be_release_governance(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "governance-approval.json", governance_approval(source="local"))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.reference_sdk.unknown.v1"},
    )

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "release-archive.json", release_archive())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.reference_sdk.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "release_archive") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "release-archive.json", release_archive())
    write_json(tmp_path / "signed-manifest.json", signed_manifest(private_key_absent=False))

    assert run_gate(tmp_path, "--require-kind", "release_archive") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
