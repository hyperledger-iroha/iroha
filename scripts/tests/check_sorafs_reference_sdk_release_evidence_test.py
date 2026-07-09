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


def release_archive(
    *,
    target_count: int | None = None,
    missing_target: bool = False,
    duplicate_target: bool = False,
) -> dict:
    targets = [
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ]
    if missing_target:
        targets.pop()
    if duplicate_target:
        targets.append(targets[0])
    payload = base("sorafs.reference_sdk.release_archive_canary.v1")
    payload.update(
        {
            "packaging_helper_used": True,
            "deterministic_archive_verified": True,
            "archive_checksums_published": True,
            "binary_checksums_published": True,
            "dist_gitkeep_only_tracked": True,
            "target_count": len(targets) if target_count is None else target_count,
            "targets": targets,
            "archive_index_digest_hex": DIGEST,
            "release_manifest_digest_hex": DIGEST,
            "raw_archives_included": False,
        }
    )
    return payload


def signed_manifest(
    *,
    private_key_absent: bool = True,
    signature_algorithm: str = "ed25519",
) -> dict:
    payload = base("sorafs.reference_sdk.signed_manifest_canary.v1")
    payload.update(
        {
            "manifest_signed": True,
            "manifest_signature_verified": True,
            "manifest_sha256_published": True,
            "governed_release_key_used": True,
            "public_key_fingerprint_recorded": True,
            "private_key_absent": private_key_absent,
            "signature_algorithm": signature_algorithm,
            "manifest_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
            "public_key_fingerprint_hex": DIGEST,
            "raw_manifest_included": False,
        }
    )
    return payload


def downstream_bindings(
    *,
    package_count: int | None = None,
    missing_package: bool = False,
    duplicate_package: bool = False,
) -> dict:
    packages = ["javascript", "python", "kotlin_jvm", "java_android", "swift"]
    if missing_package:
        packages.pop()
    if duplicate_package:
        packages.append(packages[0])
    payload = base("sorafs.reference_sdk.downstream_bindings_canary.v1")
    payload.update(
        {
            "packages": packages,
            "package_count": len(packages) if package_count is None else package_count,
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
            "public_key_fingerprint_hex": DIGEST,
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


RELEASE_MANIFEST_BOUND_FIXTURES = (
    ("release_archive", "release-archive.json", release_archive),
    ("downstream_bindings", "downstream-bindings.json", downstream_bindings),
    ("cookbook_smoke", "cookbook-smoke.json", cookbook_smoke),
    ("ffi_header_contract", "ffi-header-contract.json", ffi_header_contract),
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)

RELEASE_KEY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_release_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.reference_sdk.release_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["valid_archive_index_digests"] == [DIGEST]
    assert payload["valid_ffi_contract_digests"] == [DIGEST]
    assert payload["valid_header_digests"] == [DIGEST]
    assert payload["valid_package_index_digests"] == [DIGEST]
    assert payload["valid_release_manifest_digests"] == [DIGEST]
    assert payload["valid_release_manifest_reference_digests"] == [DIGEST]
    assert payload["valid_release_key_fingerprints"] == [DIGEST]
    assert payload["signature_algorithms"] == ["ed25519"]
    signed_manifest_artifact = payload["required"]["signed_manifest"]["artifacts"][0]
    assert signed_manifest_artifact["fingerprint"]["signature_algorithm"] == "ed25519"
    governance_artifact = payload["required"]["governance_approval"]["artifacts"][0]
    assert governance_artifact["fingerprint"]["public_key_fingerprint_hex"] == DIGEST
    assert payload["valid_smoke_output_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["required"]["release_archive"]["valid"] is True


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(
            kind_name
            for kind_name, _file_name, _factory in RELEASE_MANIFEST_BOUND_FIXTURES
        )
        == MODULE.RELEASE_MANIFEST_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in RELEASE_KEY_BOUND_FIXTURES)
        == MODULE.RELEASE_KEY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(release_archive()["targets"]) == MODULE.REQUIRED_RELEASE_TARGETS
    assert tuple(downstream_bindings()["packages"]) == (
        MODULE.REQUIRED_DOWNSTREAM_PACKAGES
    )
    assert (
        signed_manifest()["signature_algorithm"]
        in MODULE.ALLOWED_MANIFEST_SIGNATURE_ALGORITHMS
    )


def test_all_manifest_bound_artifacts_reject_signed_manifest_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in RELEASE_MANIFEST_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["release_manifest_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_release_manifest_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} release_manifest_digest_hex must reference a valid "
            "signed_manifest manifest_digest_hex"
        ) in artifact["errors"]


def test_all_policy_bound_artifacts_reject_signed_manifest_policy_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in POLICY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["policy_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_policy_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must reference a valid "
            "signed_manifest policy_digest_hex"
        ) in artifact["errors"]


def test_all_release_key_bound_artifacts_reject_signed_manifest_key_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in RELEASE_KEY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["public_key_fingerprint_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_release_key_fingerprints"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} public_key_fingerprint_hex must reference a valid "
            "signed_manifest public_key_fingerprint_hex"
        ) in artifact["errors"]


def test_multiple_valid_release_manifest_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = signed_manifest()
    payload["manifest_digest_hex"] = DIGEST_2
    write_json(tmp_path / "signed-manifest-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_release_manifest_digests"] == []
    assert (
        "valid_release_manifest_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = signed_manifest()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "signed-manifest-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_release_key_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = signed_manifest()
    payload["public_key_fingerprint_hex"] = DIGEST_2
    write_json(tmp_path / "signed-manifest-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_release_key_fingerprints"] == []
    assert (
        "valid_release_key_fingerprints must contain exactly one active digest"
        in result["errors"]
    )


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


def test_release_archive_target_count_must_match_unique_targets(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "release-archive.json", release_archive(target_count=5))

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["release_archive"]["artifacts"][0]
    assert "target_count must match unique targets count" in artifact["errors"]


def test_release_archive_targets_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "release-archive.json",
        release_archive(duplicate_target=True),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["release_archive"]["artifacts"][0]
    assert "targets must not contain duplicate values" in artifact["errors"]
    assert "target_count must match unique targets count" in artifact["errors"]


def test_release_archive_targets_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = release_archive()
    payload["targets"].append("riscv64-unknown-linux-gnu")
    payload["target_count"] = len(payload["targets"])
    write_json(tmp_path / "release-archive.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["release_archive"]["artifacts"][0]
    assert "targets must not include unknown values" in artifact["errors"]


def test_signed_manifest_rejects_private_key_presence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "signed-manifest.json", signed_manifest(private_key_absent=False))

    assert run_gate(tmp_path) == 1


def test_signed_manifest_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = signed_manifest()
    del payload["policy_digest_hex"]
    write_json(tmp_path / "signed-manifest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_policy_digests"] == []
    required = payload["required"]["signed_manifest"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]


def test_signed_manifest_rejects_rsa_sha256_signature_algorithm(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "signed-manifest.json",
        signed_manifest(signature_algorithm="rsa-sha256"),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["signed_manifest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "signature_algorithm must be `ed25519`" in artifact["errors"]
    assert payload["valid_release_manifest_digests"] == []


def test_signed_manifest_rejects_unsupported_signature_algorithm(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "signed-manifest.json",
        signed_manifest(signature_algorithm="none"),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["signed_manifest"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert "signature_algorithm must be `ed25519`" in artifact["errors"]
    assert payload["valid_release_manifest_digests"] == []


def test_signed_manifest_unsupported_signature_algorithm_stdout_does_not_echo_algorithm(
    tmp_path: Path,
    capsys,
) -> None:
    write_complete_evidence(tmp_path)
    invalid_algorithm = "none"
    write_json(
        tmp_path / "signed-manifest.json",
        signed_manifest(signature_algorithm=invalid_algorithm),
    )

    assert run_gate(tmp_path) == 1

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert "signature_algorithm must be `ed25519`" in diagnostics
    assert invalid_algorithm not in diagnostics
    assert "signature_algorithm must be `ed25519`" in captured.err
    assert invalid_algorithm not in captured.err


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
    assert payload["valid_release_manifest_reference_digests"] == []
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


def test_downstream_package_count_must_match_unique_packages(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "downstream-bindings.json",
        downstream_bindings(package_count=6),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["downstream_bindings"]["artifacts"][0]
    assert "package_count must match unique packages count" in artifact["errors"]


def test_downstream_packages_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "downstream-bindings.json",
        downstream_bindings(duplicate_package=True),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["downstream_bindings"]["artifacts"][0]
    assert "packages must not contain duplicate values" in artifact["errors"]
    assert "package_count must match unique packages count" in artifact["errors"]


def test_downstream_packages_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = downstream_bindings()
    payload["packages"].append("csharp")
    payload["package_count"] = len(payload["packages"])
    write_json(tmp_path / "downstream-bindings.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["downstream_bindings"]["artifacts"][0]
    assert "packages must not include unknown values" in artifact["errors"]


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
    assert payload["valid_release_manifest_reference_digests"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "release_archive release_manifest_digest_hex must reference a valid "
        "signed_manifest manifest_digest_hex"
    ]


def test_governance_policy_digest_must_match_signed_manifest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = governance_approval()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert payload["valid_policy_digests"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval policy_digest_hex must reference a valid "
        "signed_manifest policy_digest_hex"
    ]


def test_governance_release_key_fingerprint_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = governance_approval()
    del payload["public_key_fingerprint_hex"]
    write_json(tmp_path / "governance-approval.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert "public_key_fingerprint_hex must be a non-empty string" in artifact["errors"]


def test_governance_release_key_fingerprint_must_match_signed_manifest(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = governance_approval()
    payload["public_key_fingerprint_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert payload["valid_release_key_fingerprints"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval public_key_fingerprint_hex must reference a valid "
        "signed_manifest public_key_fingerprint_hex"
    ]


def test_cookbook_smoke_duration_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "cookbook-smoke.json", cookbook_smoke(duration=4_000))

    assert run_gate(tmp_path) == 1


def test_cookbook_smoke_duration_must_be_integer(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "cookbook-smoke.json", cookbook_smoke(duration=12.5))
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["cookbook_smoke"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "smoke_duration_seconds must be a positive integer" in artifact["errors"]


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
