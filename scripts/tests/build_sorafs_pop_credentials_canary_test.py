"""Tests for scripts/build_sorafs_pop_credentials_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


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


ROOT_DIGEST = "a" * 64
REVOCATION_DIGEST = "b" * 64
BUNDLE_DIGEST = "c" * 64
POLICY_DIGEST = "d" * 64
SNAPSHOT_DIGEST = "e" * 64
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
                "issuer-prod-a",
                "--bundle-id-hex",
                BUNDLE_DIGEST,
                "--credential-count",
                "3",
            ]
        )
        for index in range(3):
            args.extend(["--credential", f"credential-{index:02d}"])
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
            ]
        )
    elif kind == "enrollment_portal":
        for route in MODULE.REQUIRED_ENROLLMENT_ROUTES:
            args.extend(["--route", route])
    elif kind == "verifier_service":
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
                "valid-proof-00",
                "--rejected-proof-probe",
                "invalid-proof-00",
                "--rejected-proof-probe",
                "invalid-proof-01",
                "--rejected-proof-probe",
                "invalid-proof-02",
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
                "sortition-probe-00",
                "--sortition-probe",
                "sortition-probe-01",
                "--commit-reveal-probe-count",
                "2",
                "--commit-reveal-probe",
                "commit-reveal-probe-00",
                "--commit-reveal-probe",
                "commit-reveal-probe-01",
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
        {"name": "valid-proof-00", "accepted": True},
        {"name": "invalid-proof-00", "accepted": False},
        {"name": "invalid-proof-01", "accepted": False},
        {"name": "invalid-proof-02", "accepted": False},
    ]
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
        {"name": "sortition-probe-00"},
        {"name": "sortition-probe-01"},
    ]
    assert payload["commit_reveal_probe_count"] == 2
    assert payload["commit_reveal_probes"] == [
        {"name": "commit-reveal-probe-00"},
        {"name": "commit-reveal-probe-01"},
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
    assert payload["issuer_id"] == "issuer-prod-a"
    assert payload["credential_count"] == payload["signed_credential_count"] == 3
    assert [credential["name"] for credential in payload["credentials"]] == [
        "credential-00",
        "credential-01",
        "credential-02",
    ]


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


def test_verifier_service_requires_policy_digest(tmp_path: Path, capsys) -> None:
    args = args_for("verifier_service", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for verifier_service" in captured.err
    assert not canary_path(tmp_path, "verifier_service").exists()


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


def test_verifier_probe_inventories_must_not_overlap(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("verifier_service", tmp_path)
    args.extend(["--accepted-proof-probe", "invalid-proof-00"])
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
