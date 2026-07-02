"""Tests for scripts/build_sorafs_pdp_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_pdp_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_pdp_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_pdp_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_pdp_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_500_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "a" * 64
POLICY_DIGEST = "b" * 64
VALIDATION_DIGEST = "c" * 64
ARCHIVE_DIGEST = "d" * 64
ROSTER_DIGEST = "e" * 64


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "pdp-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.PROOF_SUMMARY_DIGEST_KINDS:
        args.extend(["--proof-summary-digest-hex", DIGEST])
    if kind in MODULE.POLICY_DIGEST_KINDS:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind in MODULE.PROVIDER_ROSTER_DIGEST_KINDS:
        args.extend(["--provider-roster-digest-hex", ROSTER_DIGEST])
    if kind == "provider_transport":
        for route in MODULE.REQUIRED_ROUTES:
            args.extend(["--route", route])
    elif kind == "proof_generation":
        for index in range(CHECKER.DEFAULT_MIN_PROVIDERS):
            args.extend(["--provider", f"provider-{index:02d}"])
        for index in range(CHECKER.DEFAULT_MIN_CHALLENGES):
            args.extend(["--challenge", f"challenge-{index:02d}"])
        for index in range(CHECKER.DEFAULT_MIN_PROOFS):
            args.extend(["--proof", f"proof-{index:02d}"])
    elif kind == "validator_replay":
        args.extend(["--validation-bundle-digest-hex", VALIDATION_DIGEST])
    elif kind == "governance_repair":
        args.extend(["--archive-summary-digest-hex", ARCHIVE_DIGEST])
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    return args


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=NOW_UNIX,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_proof_latency_ms=CHECKER.DEFAULT_MAX_PROOF_LATENCY_MS,
        min_providers=CHECKER.DEFAULT_MIN_PROVIDERS,
        min_challenges=CHECKER.DEFAULT_MIN_CHALLENGES,
        min_proofs=CHECKER.DEFAULT_MIN_PROOFS,
    )


def test_builds_payload_free_provider_transport_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("provider_transport", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "provider_transport").read_text("utf-8"))

    assert payload["schema"] == "sorafs.pdp.provider_transport_canary.v1"
    assert payload["route_count"] == len(MODULE.REQUIRED_ROUTES)
    assert payload["passed_route_count"] == len(MODULE.REQUIRED_ROUTES)
    assert payload["response_bodies_included"] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "provider_transport"
    assert errors == []


def test_generated_canaries_pass_full_pdp_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = ["--now-unix", str(NOW_UNIX)]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_proof_summary_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["valid_provider_roster_digests"] == [ROSTER_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_observability_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "observability.args"
    args_file.write_text(
        "\n".join(args_for("observability", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "observability").read_text("utf-8"))
    assert payload["metrics"] == list(MODULE.REQUIRED_METRICS)


def test_response_file_can_build_proof_generation_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "proof-generation.args"
    args_file.write_text(
        "\n".join(args_for("proof_generation", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "proof_generation").read_text("utf-8"))
    assert payload["providers"] == [
        {"name": f"provider-{index:02d}"}
        for index in range(CHECKER.DEFAULT_MIN_PROVIDERS)
    ]
    assert payload["challenges"] == [
        {"name": f"challenge-{index:02d}"}
        for index in range(CHECKER.DEFAULT_MIN_CHALLENGES)
    ]
    assert payload["proofs"] == [
        {"name": f"proof-{index:02d}"}
        for index in range(CHECKER.DEFAULT_MIN_PROOFS)
    ]


def test_proof_generation_provider_inventory_must_match_provider_count(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("proof_generation", tmp_path)
    index = args.index("--provider")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider unique values must match --provider-count" in captured.err
    assert not canary_path(tmp_path, "proof_generation").exists()


def test_proof_generation_challenge_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("proof_generation", tmp_path)
    first_challenge_index = args.index("--challenge")
    args[first_challenge_index + 1] = "challenge-01"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--challenge must not contain duplicates" in captured.err
    assert "--challenge unique values must match --challenge-count" in captured.err
    assert not canary_path(tmp_path, "proof_generation").exists()


def test_proof_generation_proof_inventory_must_not_duplicate(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("proof_generation", tmp_path)
    first_proof_index = args.index("--proof")
    args[first_proof_index + 1] = "proof-01"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--proof must not contain duplicates" in captured.err
    assert "--proof unique values must match --proof-count" in captured.err
    assert not canary_path(tmp_path, "proof_generation").exists()


def test_missing_route_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("provider_transport", tmp_path)
    index = args.index("--route")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route must include every required value" in captured.err
    assert not canary_path(tmp_path, "provider_transport").exists()


def test_proof_thresholds_fail_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("proof_generation", tmp_path)
    args.extend(["--provider-count", "2", "--proof-latency-ms", "90001"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-count must be >=" in captured.err
    assert "--proof-latency-ms must be <=" in captured.err
    assert not canary_path(tmp_path, "proof_generation").exists()


def test_proof_generation_requires_policy_digest_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("proof_generation", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for proof_generation" in captured.err
    assert not canary_path(tmp_path, "proof_generation").exists()


def test_proof_generation_requires_provider_roster_digest_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("proof_generation", tmp_path)
    index = args.index("--provider-roster-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-roster-digest-hex is required for proof_generation" in captured.err
    assert not canary_path(tmp_path, "proof_generation").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("provider_transport", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()
