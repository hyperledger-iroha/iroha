"""Tests for scripts/build_sorafs_pop_credentials_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_pop_credentials_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_pop_credentials_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_pop_credentials_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_pop_credentials_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)

from sorafs_rollout_runner_test_support import write_topology_qualification  # noqa: E402


ROOT_DIGEST = "a" * 64
REVOCATION_DIGEST = "b" * 64
BUNDLE_DIGEST = "c" * 64
POLICY_DIGEST = "d" * 64
SNAPSHOT_DIGEST = "e" * 64
ROUTE_BODY_DIGEST = "f" * 64
GENERATED_AT = 1_800_100_000


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_root_age_secs=CHECKER.DEFAULT_MAX_ROOT_AGE_SECS,
        max_revocation_age_secs=CHECKER.DEFAULT_MAX_REVOCATION_AGE_SECS,
        max_service_lag_secs=CHECKER.DEFAULT_MAX_SERVICE_LAG_SECS,
        max_verify_latency_ms=CHECKER.DEFAULT_MAX_VERIFY_LATENCY_MS,
    )


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "sorafs-pop-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
    ]
    if kind in MODULE.ROOT_DIGEST_KINDS:
        args.extend(["--root-digest-hex", ROOT_DIGEST])
    if kind in MODULE.REVOCATION_DIGEST_KINDS:
        args.extend(["--revocation-list-digest-hex", REVOCATION_DIGEST])
    for claim in MODULE.TRUE_CLAIMS[kind]:
        args.extend(["--verified-claim", claim])
    if kind == "issuer_bundle":
        args.extend(
            [
                "--issuer-id",
                "pop-issuer-prod-a",
                "--bundle-id-hex",
                BUNDLE_DIGEST,
                "--credential-count",
                "3",
            ]
        )
        for index in range(3):
            args.extend(["--credential", f"pop-credential-{index:02d}"])
    elif kind == "commitment_root":
        args.extend(
            [
                "--tree-version",
                "7",
                "--published-at-unix",
                str(GENERATED_AT),
            ]
        )
    elif kind == "revocation_registry":
        args.extend(
            [
                "--revocation-list-version",
                "8",
                "--published-at-unix",
                str(GENERATED_AT),
                "--revoked-nonce-count",
                "2",
                "--revoked-nonce-ref",
                "pop-revoked-nonce-00",
                "--revoked-nonce-ref",
                "pop-revoked-nonce-01",
            ]
        )
    elif kind == "enrollment_portal":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
        for route in MODULE.REQUIRED_ENROLLMENT_ROUTES:
            args.extend(["--route", route])
    elif kind == "verifier_service":
        args.extend(["--route-body-blake3-hex", ROUTE_BODY_DIGEST])
        for route in MODULE.REQUIRED_VERIFIER_ROUTES:
            args.extend(["--route", route])
        args.extend(
            [
                "--policy-digest-hex",
                POLICY_DIGEST,
                "--accepted-valid-proof-count",
                "1",
                "--rejected-invalid-proof-count",
                "3",
                "--accepted-proof-probe",
                "pop-valid-proof-00",
                "--rejected-proof-probe",
                "pop-invalid-proof-00",
                "--rejected-proof-probe",
                "pop-invalid-proof-01",
                "--rejected-proof-probe",
                "pop-invalid-proof-02",
                "--max-verify-latency-ms",
                "250",
                "--max-service-lag-seconds",
                "20",
            ]
        )
    elif kind == "moderation_integration":
        args.extend(
            [
                "--pop-snapshot-digest-hex",
                SNAPSHOT_DIGEST,
                "--sortition-probe-count",
                "2",
                "--sortition-probe",
                "pop-sortition-probe-00",
                "--sortition-probe",
                "pop-sortition-probe-01",
                "--commit-reveal-probe-count",
                "2",
                "--commit-reveal-probe",
                "pop-commit-reveal-probe-00",
                "--commit-reveal-probe",
                "pop-commit-reveal-probe-01",
            ]
        )
    elif kind == "metrics_alerts":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    elif kind == "governance_approval":
        args.extend(
            [
                "--privacy-proof-system",
                "groth16_membership_v1",
                "--policy-digest-hex",
                POLICY_DIGEST,
            ]
        )
    return args


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_verifier_service_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("verifier_service", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "verifier_service").read_text("utf-8"))

    assert payload["schema"] == "sorafs.pop.verifier_service_canary.v1"
    assert payload["root_digest_hex"] == ROOT_DIGEST
    assert payload["revocation_list_digest_hex"] == REVOCATION_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["proof_probe_count"] == 4
    assert payload["accepted_valid_proof_count"] == 1
    assert payload["rejected_invalid_proof_count"] == 3
    assert payload["probes"] == [
        {"name": "pop-valid-proof-00", "accepted": True},
        {"name": "pop-invalid-proof-00", "accepted": False},
        {"name": "pop-invalid-proof-01", "accepted": False},
        {"name": "pop-invalid-proof-02", "accepted": False},
    ]
    assert payload["route_count"] == len(MODULE.REQUIRED_VERIFIER_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_VERIFIER_ROUTES)
    assert all(
        route["body_blake3_hex"] == ROUTE_BODY_DIGEST for route in payload["routes"]
    )
    assert [route["name"] for route in payload["routes"]] == list(
        MODULE.REQUIRED_VERIFIER_ROUTES
    )
    for claim in MODULE.TRUE_CLAIMS["verifier_service"]:
        assert payload[claim] is True
    for field in MODULE.FORCED_FALSE_FIELDS["verifier_service"]:
        assert payload[field] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "verifier_service"
    assert errors == []


def test_builds_payload_free_moderation_integration_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("moderation_integration", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "moderation_integration").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.pop.moderation_integration_canary.v1"
    assert payload["sortition_probe_count"] == 2
    assert payload["sortition_probes"] == [
        {"name": "pop-sortition-probe-00"},
        {"name": "pop-sortition-probe-01"},
    ]
    assert payload["commit_reveal_probe_count"] == 2
    assert payload["commit_reveal_probes"] == [
        {"name": "pop-commit-reveal-probe-00"},
        {"name": "pop-commit-reveal-probe-01"},
    ]
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "moderation_integration"
    assert errors == []


def test_generated_canaries_pass_full_pop_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary), "--now-unix", str(GENERATED_AT)])
    command.extend(
        [
            "--topology-qualification-summary",
            str(
                write_topology_qualification(
                    tmp_path / "topology-qualification.json",
                    deployment_id="sorafs-pop-prod-20260701",
                )
            ),
        ]
    )

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_juror_sync_bindings"] == [
        {
            "synced_root_digest_hex": ROOT_DIGEST,
            "synced_revocation_list_digest_hex": REVOCATION_DIGEST,
        }
    ]
    assert payload["valid_pop_snapshot_digests"] == [SNAPSHOT_DIGEST]
    assert payload["valid_root_digests"] == [ROOT_DIGEST]
    assert payload["valid_revocation_list_digests"] == [REVOCATION_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_issuer_bundle_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "issuer.args"
    args_file.write_text("\n".join(args_for("issuer_bundle", tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "issuer_bundle").read_text("utf-8"))
    assert payload["issuer_id"] == "pop-issuer-prod-a"
    assert payload["credential_count"] == payload["signed_credential_count"] == 3
    assert [credential["name"] for credential in payload["credentials"]] == [
        "pop-credential-00",
        "pop-credential-01",
        "pop-credential-02",
    ]


def test_issuer_id_rejects_malformed_value_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("issuer_bundle", tmp_path)
    issuer_index = args.index("--issuer-id")
    args[issuer_index + 1] = "pop-issuer--prod"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert CHECKER.ISSUER_ID_ERROR.replace("issuer_id", "--issuer-id") in captured.err
    assert not canary_path(tmp_path, "issuer_bundle").exists()


def test_issuer_id_rejects_generic_issuer_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    issuer_index = args.index("--issuer-id")
    args[issuer_index + 1] = "issuer-prod-a"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert CHECKER.ISSUER_ID_ERROR.replace("issuer_id", "--issuer-id") in captured.err
    assert not canary_path(tmp_path, "issuer_bundle").exists()


def test_issuer_id_rejects_non_production_marker_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    issuer_index = args.index("--issuer-id")
    args[issuer_index + 1] = "pop-issuer-dev-a"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--issuer-id must not contain non-production markers ['dev']" in captured.err
    assert not canary_path(tmp_path, "issuer_bundle").exists()


def test_issuer_id_accepts_reviewed_future_label(tmp_path: Path) -> None:
    args = args_for("issuer_bundle", tmp_path)
    issuer_index = args.index("--issuer-id")
    args[issuer_index + 1] = "pop-issuer-governance-12"

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "issuer_bundle").read_text("utf-8"))
    assert payload["issuer_id"] == "pop-issuer-governance-12"
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "issuer_bundle"
    assert errors == []


def test_issuer_credential_inventory_must_match_credential_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    args[args.index("--credential-count") + 1] = "4"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--credential unique values must match --credential-count" in captured.err
    assert not canary_path(tmp_path, "issuer_bundle").exists()


def test_issuer_credential_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    first_credential = args.index("--credential") + 1
    args.extend(["--credential", args[first_credential]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--credential must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "issuer_bundle").exists()


def test_issuer_credential_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    credential_index = args.index("--credential") + 1
    args[credential_index] = "credential_00"

    assert_rejected_without_artifact(
        args,
        kind="issuer_bundle",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--credential must match canonical lowercase `pop-credential-*`",
    )


def test_issuer_credential_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    credential_index = args.index("--credential") + 1
    args[credential_index] = "pop-credential-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="issuer_bundle",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--credential must not contain non-production markers ['placeholder']"
        ),
    )


def test_issuer_credential_inventory_requires_pop_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("issuer_bundle", tmp_path)
    credential_index = args.index("--credential") + 1
    args[credential_index] = "credential-00"

    assert_rejected_without_artifact(
        args,
        kind="issuer_bundle",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--credential must match canonical lowercase `pop-credential-*`",
    )


def test_revocation_registry_builds_payload_free_nonce_refs(tmp_path: Path) -> None:
    assert MODULE.main(args_for("revocation_registry", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "revocation_registry").read_text("utf-8")
    )

    assert payload["revoked_nonce_count"] == 2
    assert payload["revoked_nonce_refs"] == [
        {"name": "pop-revoked-nonce-00"},
        {"name": "pop-revoked-nonce-01"},
    ]
    assert payload["revoked_nonces_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "revocation_registry"
    assert errors == []


def test_revocation_registry_nonce_ref_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("revocation_registry", tmp_path)
    args[args.index("--revoked-nonce-count") + 1] = "3"

    assert_rejected_without_artifact(
        args,
        kind="revocation_registry",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--revoked-nonce-ref unique values must match --revoked-nonce-count"
        ),
    )


def test_revocation_registry_nonce_ref_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("revocation_registry", tmp_path)
    first_ref = args.index("--revoked-nonce-ref") + 1
    args.extend(["--revoked-nonce-ref", args[first_ref]])

    assert_rejected_without_artifact(
        args,
        kind="revocation_registry",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--revoked-nonce-ref must not contain duplicates",
    )


def test_revocation_registry_nonce_ref_inventory_must_use_reviewed_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("revocation_registry", tmp_path)
    ref_index = args.index("--revoked-nonce-ref") + 1
    args[ref_index] = "revoked-nonce-00"

    assert_rejected_without_artifact(
        args,
        kind="revocation_registry",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--revoked-nonce-ref must match canonical lowercase "
            "`pop-revoked-nonce-*`"
        ),
    )


def test_revocation_registry_nonce_ref_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("revocation_registry", tmp_path)
    ref_index = args.index("--revoked-nonce-ref") + 1
    args[ref_index] = "pop-revoked-nonce-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="revocation_registry",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--revoked-nonce-ref must not contain non-production markers "
            "['placeholder']"
        ),
    )


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("issuer_bundle", tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not canary_path(tmp_path, "issuer_bundle").exists()


def test_missing_metric_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("metrics_alerts", tmp_path)
    index = args.index("--metric")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must include every required value" in captured.err
    assert not canary_path(tmp_path, "metrics_alerts").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "issuer_bundle",
            "--verified-claim",
            MODULE.TRUE_CLAIMS["issuer_bundle"][0],
            "unreviewed-pop-claim",
        ),
        (
            "enrollment_portal",
            "--route",
            MODULE.REQUIRED_ENROLLMENT_ROUTES[0],
            "unreviewed-enrollment-route",
        ),
        (
            "verifier_service",
            "--route",
            MODULE.REQUIRED_VERIFIER_ROUTES[0],
            "unreviewed-verifier-route",
        ),
        (
            "metrics_alerts",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "unreviewed-pop-metric",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_verifier_service_requires_policy_digest(tmp_path: Path, capsys) -> None:
    args = args_for("verifier_service", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for verifier_service" in captured.err
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_verifier_route_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    args.extend(["--route", MODULE.REQUIRED_VERIFIER_ROUTES[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "verifier_service").exists()


@pytest.mark.parametrize(
    "kind",
    ("enrollment_portal", "verifier_service"),
)
def test_route_canaries_require_route_body_digest(
    kind: str,
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for(kind, tmp_path)
    index = args.index("--route-body-blake3-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route-body-blake3-hex must be exact lowercase 32-byte hex" in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_verifier_accepted_probe_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    args[args.index("--accepted-valid-proof-count") + 1] = "2"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--accepted-proof-probe unique values must match "
        "--accepted-valid-proof-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_verifier_rejected_probe_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    first_rejected = args.index("--rejected-proof-probe") + 1
    args.extend(["--rejected-proof-probe", args[first_rejected]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--rejected-proof-probe must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_verifier_probe_inventory_must_use_partitioned_labels_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    accepted_index = args.index("--accepted-proof-probe") + 1
    rejected_index = args.index("--rejected-proof-probe") + 1
    args[accepted_index] = "pop-invalid-proof-on-accepted-side"
    args[rejected_index] = "pop-valid-proof-on-rejected-side"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--accepted-proof-probe must match canonical lowercase `pop-valid-proof-*`"
        in captured.err
    )
    assert (
        "--rejected-proof-probe must match canonical lowercase `pop-invalid-proof-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_verifier_probe_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    accepted_index = args.index("--accepted-proof-probe") + 1
    rejected_index = args.index("--rejected-proof-probe") + 1
    args[accepted_index] = "pop-valid-proof-placeholder"
    args[rejected_index] = "pop-invalid-proof-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--accepted-proof-probe must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert (
        "--rejected-proof-probe must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_verifier_probe_inventory_requires_pop_families_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    accepted_index = args.index("--accepted-proof-probe") + 1
    rejected_index = args.index("--rejected-proof-probe") + 1
    args[accepted_index] = "valid-proof-00"
    args[rejected_index] = "invalid-proof-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--accepted-proof-probe must match canonical lowercase `pop-valid-proof-*`"
        in captured.err
    )
    assert (
        "--rejected-proof-probe must match canonical lowercase `pop-invalid-proof-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_verifier_probe_inventories_must_not_overlap(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    args.extend(["--accepted-proof-probe", "pop-invalid-proof-00"])
    args[args.index("--accepted-valid-proof-count") + 1] = "2"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--accepted-proof-probe and --rejected-proof-probe must not overlap"
        in captured.err
    )
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_moderation_sortition_probe_inventory_must_match_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    args[args.index("--sortition-probe-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--sortition-probe unique values must match --sortition-probe-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "moderation_integration").exists()


def test_moderation_sortition_probe_must_use_reviewed_label_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    probe_index = args.index("--sortition-probe") + 1
    args[probe_index] = "sortition_probe_00"

    assert_rejected_without_artifact(
        args,
        kind="moderation_integration",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--sortition-probe must match canonical lowercase "
            "`pop-sortition-probe-name`"
        ),
    )


def test_moderation_sortition_probe_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    probe_index = args.index("--sortition-probe") + 1
    args[probe_index] = "pop-sortition-probe-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="moderation_integration",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--sortition-probe must not contain non-production markers ['placeholder']"
        ),
    )


def test_moderation_commit_reveal_probe_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    first_commit_probe = args.index("--commit-reveal-probe") + 1
    args.extend(["--commit-reveal-probe", args[first_commit_probe]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--commit-reveal-probe must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "moderation_integration").exists()


def test_moderation_commit_reveal_probe_must_use_reviewed_label_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    probe_index = args.index("--commit-reveal-probe") + 1
    args[probe_index] = "commit_reveal_probe_00"

    assert_rejected_without_artifact(
        args,
        kind="moderation_integration",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--commit-reveal-probe must match canonical lowercase "
            "`pop-commit-reveal-probe-name`"
        ),
    )


def test_moderation_commit_reveal_probe_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    probe_index = args.index("--commit-reveal-probe") + 1
    args[probe_index] = "pop-commit-reveal-probe-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="moderation_integration",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--commit-reveal-probe must not contain non-production markers "
            "['placeholder']"
        ),
    )


def test_moderation_probe_inputs_require_pop_families_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("moderation_integration", tmp_path)
    sortition_index = args.index("--sortition-probe") + 1
    commit_index = args.index("--commit-reveal-probe") + 1
    args[sortition_index] = "sortition-probe-00"
    args[commit_index] = "commit-reveal-probe-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--sortition-probe must match canonical lowercase "
        "`pop-sortition-probe-name`"
    ) in captured.err
    assert (
        "--commit-reveal-probe must match canonical lowercase "
        "`pop-commit-reveal-probe-name`"
    ) in captured.err
    assert not canary_path(tmp_path, "moderation_integration").exists()


def test_transcript_digest_privacy_backend_fails_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("governance_approval", tmp_path)
    args[args.index("--privacy-proof-system") + 1] = "transcript_digest_v1"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--privacy-proof-system must be a production" in captured.err
    assert not canary_path(tmp_path, "governance_approval").exists()


def test_stale_service_lag_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("verifier_service", tmp_path)
    args[args.index("--max-service-lag-seconds") + 1] = str(
        CHECKER.DEFAULT_MAX_SERVICE_LAG_SECS + 1
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--max-service-lag-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "verifier_service").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = canary_path(tmp_path, "issuer_bundle")
    symlink.symlink_to(target)

    assert MODULE.main(args_for("issuer_bundle", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = canary_path(tmp_path, "issuer_bundle")
    output_dir.mkdir()

    assert MODULE.main(args_for("issuer_bundle", tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
