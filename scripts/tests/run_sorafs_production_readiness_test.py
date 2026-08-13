"""Tests for scripts/run_sorafs_production_readiness.py."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("run_sorafs_production_readiness", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import check_sorafs_production_readiness as CHECKER  # noqa: E402
import sorafs_l1_lane_evidence_inventory as LANE_INVENTORY  # noqa: E402
import sorafs_l1_lane_inventory_test_support as INVENTORY_SUPPORT  # noqa: E402
import sorafs_topology_qualification as TOPOLOGY  # noqa: E402
from sorafs_resilience_test_support import (  # noqa: E402
    DEFAULT_SIGNING_SEED as RESILIENCE_SIGNING_SEED,
    public_key_from_seed as resilience_public_key_from_seed,
    write_resilience_summary,
)
from sorafs_rollout_runner_test_support import signed_topology_cli_args  # noqa: E402

CHECKER_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_production_readiness.py"
FOUNDATIONAL_SIGNER_PUBLIC_KEY_HEX = "12" * 32
FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256 = "34" * 32
FOUNDATIONAL_RELEASE_SEQUENCE = 7
RESILIENCE_SIGNER_PUBLIC_KEY = resilience_public_key_from_seed(
    RESILIENCE_SIGNING_SEED
)


def test_promotion_docs_match_exact_22_input_replay_contract() -> None:
    """Keep the documented replay inventory identical to the executable one."""

    root = MODULE_PATH.parents[1]
    ledger = " ".join(
        (root / "specs/sorafs/v1_closure_ledger.md").read_text(
            encoding="utf-8"
        ).split()
    )
    resilience = " ".join(
        (root / "specs/sorafs/l1_resilience_qualification.md").read_text(
            encoding="utf-8"
        ).split()
    )
    runner = MODULE_PATH.read_text(encoding="utf-8")

    assert "Return the exact ordered 22-input promotion replay set." in runner
    assert (
        "contains 22 top-level inputs: topology qualification summary, signed "
        "topology qualification envelope, resilience qualification, signed L1 "
        "lane evidence inventory, foundational envelope, and the 17 lane summaries"
        in ledger
    )
    assert (
        "signed L1 lane evidence inventory + foundation + 17 lane summaries "
        "equals 22 immutable inputs" in resilience
    )
    assert "contains 20 top-level inputs" not in ledger
    assert "equals 21 immutable inputs" not in resilience


def write_json(path: Path) -> Path:
    path.write_text('{"schema":"placeholder"}\n', encoding="utf-8")
    return path


def write_lane_inventory(path: Path) -> Path:
    """Write a schema-closed signed-inventory placeholder for runner planning."""

    signer = LANE_INVENTORY.trusted_signer_binding(
        INVENTORY_SUPPORT.PUBLIC_KEY.hex(),
        service_id=INVENTORY_SUPPORT.SERVICE_ID,
        administrator_id=INVENTORY_SUPPORT.ADMINISTRATOR_ID,
        key_revision=INVENTORY_SUPPORT.KEY_REVISION,
        policy_revision=INVENTORY_SUPPORT.POLICY_REVISION,
        policy_digest_sha256=INVENTORY_SUPPORT.POLICY_DIGEST_SHA256,
    )
    signer["signature_hex"] = "12" * 64
    payload = {
        "schema": LANE_INVENTORY.INVENTORY_SCHEMA,
        "status": "ready",
        "signer_qualification": "software-key-qualified",
        "generated_at_unix": 1_800_800_000,
        "max_summary_age_secs": LANE_INVENTORY.MAX_SUMMARY_AGE_SECS,
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "deployment": {
            "deployment_id": "sorafs-mainnet-2026-06",
            "environment": "production",
            "network": LANE_INVENTORY.TAIRA_NETWORK,
            "chain_id": LANE_INVENTORY.TAIRA_CHAIN_ID,
            "chain_discriminant": LANE_INVENTORY.TAIRA_CHAIN_DISCRIMINANT,
        },
        "anchors": {
            "topology_qualification_summary_sha256": "21" * 32,
            "topology_manifest_sha256": "22" * 32,
            "topology_canonical_manifest_sha256": "23" * 32,
            "validator_ids_sha256": "24" * 32,
            "oldest_evidence_generated_at_unix": 1_800_799_900,
            "newest_evidence_generated_at_unix": 1_800_799_990,
        },
        "summaries": [
            {
                "lane": lane,
                "schema": schema,
                "summary_sha256": hashlib.sha256(lane.encode()).hexdigest(),
                "recognized_artifact_count": 1,
                "oldest_generated_at_unix": 1_800_799_900,
                "newest_generated_at_unix": 1_800_799_990,
            }
            for lane, schema in LANE_INVENTORY.LANES
        ],
        "signer": signer,
    }
    path.write_bytes(LANE_INVENTORY.canonical_file_bytes(payload))
    return path


def write_topology_qualification(path: Path) -> Path:
    """Write one exact schema-qualified four-validator topology summary."""

    payload = {
        "schema": "sorafs.l1.deployment_qualification.summary.v1",
        "status": "configuration-qualified",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": hashlib.sha256(b"runner-exact-manifest").hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"runner-canonical-manifest"
        ).hexdigest(),
        "deployment": {
            "deployment_id": "sorafs-mainnet-2026-06",
            "environment": "production",
            "network": "taira",
            "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
            "chain_discriminant": 369,
        },
        "validator_count": 4, "validator_ids": ["taira-validator-1", "taira-validator-2", "taira-validator-3", "taira-validator-4"],
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": ["monitoring", "external_signer", "kms", "webauthn"],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(MODULE.DEFAULT_REQUIRED_GATES),
        "recognized_lane_slot_count": 17,
        "errors": [],
    }
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def topology_args(tmp_path: Path) -> list[str]:
    """Return one complete independently signed topology trust tuple."""

    path = write_topology_qualification(tmp_path / "l1-topology-qualification.json")
    inventory_path = write_lane_inventory(tmp_path / "l1-lane-evidence.inventory")
    return [
        *signed_topology_cli_args(
            path,
            deployment_id="sorafs-mainnet-2026-06",
            environment="production",
            now_unix=1_800_800_000,
        ),
        *INVENTORY_SUPPORT.inventory_cli_args(inventory_path),
    ]


def complete_args(tmp_path: Path) -> list[str]:
    topology_path = write_topology_qualification(
        tmp_path / "l1-topology-qualification.json"
    )
    topology_args = signed_topology_cli_args(
        topology_path,
        deployment_id="sorafs-mainnet-2026-06",
        environment="production",
        now_unix=1_800_800_000,
    )
    topology_binding, topology_errors = TOPOLOGY.load_topology_qualification_binding(
        topology_path,
        expected_deployment_id="sorafs-mainnet-2026-06",
        expected_environment="production",
    )
    assert topology_errors == []
    assert topology_binding is not None
    resilience_path, public_key, _binding = write_resilience_summary(
        CHECKER,
        tmp_path / "l1-resilience-qualification.summary",
        deployment_id="sorafs-mainnet-2026-06",
        environment="production",
        topology_qualification=topology_binding,
        generated_at_unix=1_800_799_970,
        captured_at_unix=1_800_799_940,
    )
    assert public_key == RESILIENCE_SIGNER_PUBLIC_KEY
    inventory_path = write_lane_inventory(tmp_path / "l1-lane-evidence.inventory")
    args = [
        "--out-dir",
        str(tmp_path / "out"),
        "--verifier",
        str(CHECKER_PATH),
        *topology_args,
        "--resilience-qualification-summary",
        str(resilience_path),
        "--resilience-qualification-signer-public-key-hex",
        RESILIENCE_SIGNER_PUBLIC_KEY.hex(),
        *INVENTORY_SUPPORT.inventory_cli_args(inventory_path),
        "--deployment-id",
        "sorafs-mainnet-2026-06",
        "--environment",
        "production",
        "--now-unix",
        "1800800000",
        "--foundational-prerequisite-summary",
        str(write_json(tmp_path / "foundational_prerequisites.json")),
        "--foundational-prerequisite-signer-public-key-hex",
        FOUNDATIONAL_SIGNER_PUBLIC_KEY_HEX,
        "--foundational-prerequisite-signer-verifier",
        str(CHECKER_PATH),
        "--foundational-prerequisite-signer-verifier-sha256",
        hashlib.sha256(CHECKER_PATH.read_bytes()).hexdigest(),
        "--foundational-prerequisite-release-sequence",
        str(FOUNDATIONAL_RELEASE_SEQUENCE),
        "--foundational-prerequisite-previous-envelope-sha256",
        FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
    ]
    for gate, flag in MODULE.SUMMARY_FLAGS_BY_GATE.items():
        args.extend([flag, str(write_json(tmp_path / f"{gate}.json"))])
    return args


def test_dry_run_prints_complete_aggregate_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["schema"] == "sorafs.production_readiness.collection_plan.v1"
    assert payload["verifier_summary_schema"] == MODULE.SUMMARY_SCHEMA
    assert payload["deployment_context"] == {
        "deployment_id": "sorafs-mainnet-2026-06",
        "environment": "production",
    }
    topology_path = tmp_path / "l1-topology-qualification.json"
    assert payload["topology_qualification"]["summary"] == str(topology_path)
    assert payload["topology_qualification"][
        "qualification_summary_sha256"
    ] == hashlib.sha256(topology_path.read_bytes()).hexdigest()
    assert payload["resilience_qualification"] == MODULE.resilience_qualification_plan(
        MODULE.parse_args(complete_args(tmp_path))
    )
    assert payload["l1_lane_evidence_inventory"] == (
        MODULE.l1_lane_evidence_inventory_plan(MODULE.parse_args(complete_args(tmp_path)))
    )
    assert set(payload["summary_contract"]) == set(MODULE.DEFAULT_REQUIRED_GATES)
    assert payload["foundational_prerequisite"] == {
        "schema": MODULE.FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "summary": str(tmp_path / "foundational_prerequisites.json"),
        "required_ids": list(MODULE.FOUNDATIONAL_PREREQUISITE_IDS),
        "signer_public_key_fingerprint_sha256": hashlib.sha256(
            bytes.fromhex(FOUNDATIONAL_SIGNER_PUBLIC_KEY_HEX)
        ).hexdigest(),
        "release_sequence": FOUNDATIONAL_RELEASE_SEQUENCE,
        "previous_envelope_sha256": FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
    }
    assert payload["summary_contract"]["gateway_load"]["required_kinds"]
    assert len(payload["steps"]) == 2
    assert payload["steps"][0]["label"] == (
        "sorafs_production_readiness_gate_first"
    )
    assert payload["steps"][1]["label"] == (
        "sorafs_production_readiness_gate_replay"
    )
    assert "check_sorafs_production_readiness.py" in payload["steps"][0]["command"][1]
    assert len(MODULE.production_input_paths(MODULE.parse_args(complete_args(tmp_path)))) == 22
    assert "--foundational-prerequisite-signer-verifier" in payload["steps"][0]["command"]
    assert payload["steps"][0]["artifact"] != payload["steps"][1]["artifact"]


def test_foundational_prerequisite_runner_inputs_are_required_unique_and_distinct(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_prerequisite_summary = []
    errors = MODULE.validate_inputs(args)
    assert (
        "production readiness runner requires exactly one foundational prerequisite summary"
        in errors
    )

    first = write_json(tmp_path / "foundation-first.json")
    second = write_json(tmp_path / "foundation-second.json")
    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_prerequisite_summary = [first, second]
    errors = MODULE.validate_inputs(args)
    assert (
        "production readiness runner requires exactly one foundational prerequisite summary"
        in errors
    )

    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_prerequisite_summary = [args.gateway_load_summary[0]]
    errors = MODULE.validate_inputs(args)
    assert "duplicate input evidence file" in errors


def test_foundational_prerequisite_runner_rejects_malformed_trust_without_echo(
    tmp_path: Path,
) -> None:
    cases = (
        (
            "foundational_signer_public_key_hex",
            "runtime-only-private-key",
            "must be exactly 32 bytes of lowercase hex",
        ),
        (
            "foundational_signer_public_key_hex",
            "00" * 32,
            "must not be the all-zero key",
        ),
        (
            "foundational_release_sequence",
            True,
            "--foundational-release-sequence must be positive",
        ),
        (
            "foundational_previous_envelope_sha256",
            "RUNTIME-ONLY-PREDECESSOR",
            "foundational predecessor must be canonical lowercase SHA-256",
        ),
        (
            "foundational_signer_verifier",
            None,
            "requires a foundational signer receipt verifier",
        ),
        (
            "foundational_signer_verifier_sha256",
            None,
            "requires a foundational signer verifier SHA-256",
        ),
        (
            "foundational_signer_verifier_sha256",
            "RUNTIME-ONLY-VERIFIER-DIGEST",
            "must be a non-zero canonical lowercase SHA-256",
        ),
        (
            "foundational_release_sequence",
            1 << 63,
            "foundational release sequence must be in 1..2^63-1",
        ),
    )
    for field, value, expected_error in cases:
        args = MODULE.parse_args(complete_args(tmp_path))
        setattr(args, field, value)
        errors = MODULE.validate_inputs(args)
        diagnostics = "\n".join(errors)
        assert expected_error in diagnostics
        assert str(value) not in diagnostics

    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_release_sequence = 1
    errors = MODULE.validate_inputs(args)
    assert (
        "production readiness runner foundational sequence 1 requires the zero predecessor"
        in errors
    )

    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_previous_envelope_sha256 = "00" * 32
    errors = MODULE.validate_inputs(args)
    assert (
        "production readiness runner foundational sequence after 1 requires a non-zero predecessor"
        in errors
    )


def test_foundational_prerequisite_plan_is_schema_closed_and_payload_free(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    runtime_secret = "runtime-only-private-key-material"
    rendered["foundational_prerequisite"]["private_key"] = runtime_secret
    rendered["foundational_prerequisite"]["summary"] = "../private_key.json"

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)
    assert (
        "production readiness runner plan foundational_prerequisite fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan foundational_prerequisite must match reviewed inputs"
        in diagnostics
    )
    assert MODULE.PLAN_RENDERED_PATH_ERROR in diagnostics
    assert runtime_secret not in diagnostics
    assert "../private_key.json" not in diagnostics


def test_foundational_prerequisite_runner_rejects_symlink_and_parent_traversal(
    tmp_path: Path,
) -> None:
    target = write_json(tmp_path / "foundation-target.json")
    symlink = tmp_path / "foundation-link.json"
    symlink.symlink_to(target)
    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_prerequisite_summary = [symlink]
    errors = MODULE.validate_inputs(args)
    assert "input evidence file must not be a symlink" in errors

    nested = tmp_path / "nested"
    nested.mkdir()
    args = MODULE.parse_args(complete_args(tmp_path))
    args.foundational_prerequisite_summary = [
        nested / ".." / "foundation-target.json"
    ]
    errors = MODULE.validate_inputs(args)
    assert MODULE.PLAN_RENDERED_PATH_ERROR in errors


def test_help_marks_final_deployment_context_required(capsys) -> None:
    try:
        MODULE.parse_args(["--help"])
    except SystemExit as error:
        assert error.code == 0
    else:  # pragma: no cover - argparse always exits for --help
        raise AssertionError("expected --help to exit")

    help_text = " ".join(capsys.readouterr().out.split())

    assert (
        "Required final deployment id shared by every required lane summary"
        in help_text
    )
    assert (
        "Required final prod/production environment shared by every required"
        in help_text
    )
    assert "Optional expected deployment id" not in help_text
    assert "Optional expected environment" not in help_text


def test_now_unix_is_required_for_freshness_validation(
    tmp_path: Path,
    capsys,
) -> None:
    values = complete_args(tmp_path)
    flag_index = values.index("--now-unix")
    del values[flag_index : flag_index + 2]
    exit_code = MODULE.main(values)

    captured = capsys.readouterr()
    assert exit_code == 2
    assert "--now-unix must be positive" in captured.err


def test_topology_qualification_is_non_optional(tmp_path: Path) -> None:
    """The collection runner rejects omission before producing a plan."""

    values = complete_args(tmp_path)
    flag_index = values.index("--topology-qualification-summary")
    del values[flag_index : flag_index + 2]
    assert MODULE.main(values) == 2


def test_substituted_verifier_is_rejected_before_execution(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    substitute = tmp_path / "alternate-checker.py"
    substitute.write_text("#!/usr/bin/env python3\n", encoding="utf-8")
    values = complete_args(tmp_path)
    verifier_index = values.index("--verifier") + 1
    values[verifier_index] = str(substitute)
    executed = False

    def fake_run_command_plan(plan, out_dir):
        nonlocal executed
        executed = True
        return 0

    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    assert MODULE.main(values) == 2
    assert executed is False
    assert (
        "production readiness runner requires the bundled aggregate verifier"
        in capsys.readouterr().err
    )


def test_malformed_integer_arguments_fail_before_validation(capsys) -> None:
    cases = [
        ("--now-unix", "private-key-01", "must be an integer"),
        ("--now-unix", "0", "must be positive"),
        (
            "--max-summary-artifact-age-secs",
            "private-key-02",
            "must be an integer",
        ),
        ("--max-summary-artifact-age-secs", "-1", "must be non-negative"),
    ]

    for flag, value, diagnostic in cases:
        assert MODULE.main([flag, value]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert value not in captured.err
        assert captured.out == ""


def test_duplicate_required_gate_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    assert (
            MODULE.main(
                [
                    *complete_args(tmp_path),
                    "--require-gate",
                "gateway_load",
                "--require-gate",
                "gateway_load",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "duplicate required evidence kind" in captured.err
    assert "gateway_load" not in captured.err
    assert captured.out == ""


def test_unknown_required_gate_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    unknown_gate = "private-key-placeholder"

    assert (
            MODULE.main(
                [
                    *complete_args(tmp_path),
                    "--require-gate",
                unknown_gate,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert unknown_gate not in captured.err
    assert captured.out == ""


def test_malformed_required_gate_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    malformed_gate = "gateway_load,"

    assert (
            MODULE.main(
                [
                    *complete_args(tmp_path),
                    "--require-gate",
                malformed_gate,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "--require-kind entries must be non-empty canonical strings"
        in captured.err
    )
    assert malformed_gate not in captured.err
    assert captured.out == ""


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "production readiness runner plan must be an object"
    ]

    rendered["thresholds"]["now_unix"] = float("inf")
    errors = MODULE.validate_plan_json(rendered, plan, args)
    assert "production readiness runner plan must be strict JSON renderable" in errors
    assert "inf" not in "\n".join(errors)
    rendered = MODULE.plan_json(plan, args)

    rendered["private_key"] = "runtime-only-key-material"
    rendered["external_summaries"] = {
        "gateway_load": [
            "artifacts/sorafs/gateway-load/summary.json",
            "artifacts/sorafs/gateway-load/summary-copy.json",
        ]
    }
    rendered["summary_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)
    assert (
        "production readiness runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must match required gates"
        in diagnostics
    )
    assert "production readiness runner plan steps must match command plan" in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "summary-copy" not in diagnostics


def test_plan_json_schema_fields_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["schema"] = "sorafs\nproduction"
    rendered["verifier_summary_schema"] = "runtime-only\nschema"

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "production readiness runner plan schema must be canonical" in diagnostics
    assert (
        "production readiness runner plan schema must match the contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "production readiness runner plan verifier schema must match aggregate schema"
        in diagnostics
    )
    assert "sorafs\nproduction" not in diagnostics
    assert "runtime-only\nschema" not in diagnostics


def test_plan_json_top_level_fields_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan fields must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics


def test_plan_json_deployment_context_must_be_final_production(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-staging-a"
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-staging-a" not in "\n".join(errors)

    args = MODULE.parse_args(complete_args(tmp_path))
    args.environment = "staging"
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan environment must be production"
        in errors
    )
    assert "staging" not in "\n".join(errors)


def test_plan_json_deployment_context_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["deployment_context"] = {
        "deployment_id": "gateway\nproduction",
        "environment": 7,
        "private_key": "runtime-only-key-material",
        "bad\nkey": "production",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan deployment_context fields must be deployment_id and environment"
        in diagnostics
    )
    assert (
        "production readiness runner plan deployment_context keys must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan deployment_context must be canonical"
        in diagnostics
    )
    assert (
        "production readiness runner plan deployment_context must match args"
        in diagnostics
    )
    assert "gateway\nproduction" not in diagnostics
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "bad\nkey" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["deployment_context"] = ["deployment_id", "environment"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan deployment_context must be an object"
        in errors
    )
    assert (
        "production readiness runner plan deployment_context must match args"
        in errors
    )


def test_plan_json_thresholds_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["thresholds"] = {
        "max_summary_artifact_age_secs": -1,
        "now_unix": 0,
        "private_key": 7,
        "bad\nkey": 3,
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan thresholds must contain only max_summary_artifact_age_secs, max_topology_qualification_review_age_secs, and now_unix"
        in diagnostics
    )
    assert (
        "production readiness runner plan thresholds.max_summary_artifact_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "production readiness runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert "production readiness runner plan thresholds must match args" in diagnostics
    assert "private_key" not in diagnostics
    assert "bad\nkey" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["thresholds"] = {"now_unix": args.now_unix}

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan thresholds.max_summary_artifact_age_secs must be present"
        in errors
    )
    assert (
        "production readiness runner plan thresholds.max_summary_artifact_age_secs must be a non-negative integer"
        in errors
    )

    rendered = MODULE.plan_json(plan, args)
    rendered["thresholds"] = ["max_summary_artifact_age_secs"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan thresholds must be an object" in errors
    assert "production readiness runner plan thresholds must match args" in errors


def test_plan_json_required_gates_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["required_gates"] = [
        "gateway_load",
        "gateway_load",
        "unknown_gate",
        "gateway\nload",
    ]

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan required_gates must contain canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan required_gates must not contain duplicate gates"
        in diagnostics
    )
    assert (
        "production readiness runner plan required_gates must use known gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan required_gates must match args"
        in diagnostics
    )
    assert "unknown_gate" not in diagnostics
    assert "gateway\nload" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["required_gates"] = "gateway_load"

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan required_gates must be a list" in errors
    assert (
        "production readiness runner plan required_gates must match args"
        in errors
    )


def test_plan_json_external_summaries_shape_is_validated(tmp_path: Path) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    reputation_summary = write_json(tmp_path / "reputation.json")
    topology_path = write_topology_qualification(
        tmp_path / "l1-topology-qualification.json"
    )
    args = MODULE.parse_args(
        [
            *signed_topology_cli_args(
                topology_path,
                deployment_id="sorafs-mainnet-2026-06",
                environment="production",
                now_unix=1_800_800_000,
            ),
            *INVENTORY_SUPPORT.inventory_cli_args(
                write_lane_inventory(tmp_path / "l1-lane-evidence.inventory")
            ),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--now-unix",
            "1800800000",
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_summaries"] = {
        "gateway_load": [str(gateway_summary), str(gateway_summary)],
        "reputation": [str(reputation_summary)],
        "unknown_gate": [str(tmp_path / "unknown.json")],
        "gateway\nload": [str(gateway_summary)],
        "repair": "artifacts/repair/summary.json",
        "por": [7],
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan external_summaries keys must be canonical gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries keys must use known gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain only required gates"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must map each gate to a summary path list"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary path per gate"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries paths must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        in diagnostics
    )
    assert "unknown_gate" not in diagnostics
    assert "gateway\nload" not in diagnostics
    assert "reputation" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["external_summaries"] = ["gateway_load"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan external_summaries must be an object"
        in errors
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        in errors
    )


def test_plan_json_summary_contract_shape_is_validated(tmp_path: Path) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    args = MODULE.parse_args(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--now-unix",
            "1800800000",
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
        ]
    )
    plan = MODULE.build_command_plan(args)
    first_kind = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    rendered = MODULE.plan_json(plan, args)
    rendered["summary_contract"] = {
        "gateway_load": {
            "schema": "wrong.schema.v1",
            "required_kinds": [first_kind, first_kind, "gateway\nload"],
            "raw_payload": "not allowed",
            "bad\nfield": "not allowed",
        },
        "reputation": {
            "schema": MODULE.GATE_BY_NAME["reputation"].schema,
            "required_kinds": list(MODULE.GATE_BY_NAME["reputation"].required_kinds),
        },
        "unknown_gate": {"schema": "sorafs.unknown.v1", "required_kinds": []},
        "gateway\nload": {
            "schema": MODULE.GATE_BY_NAME["gateway_load"].schema,
            "required_kinds": [first_kind],
        },
        "repair": "contract-shaped-entry",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan summary_contract keys must be canonical gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract keys must use known gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must contain only required gates"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must map each gate to a contract object"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract gate fields must be schema and required_kinds"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract gate fields must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract schemas must match gate schema"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must be non-empty lists"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must match gate contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must match required gates"
        in diagnostics
    )
    assert "unknown_gate" not in diagnostics
    assert "gateway\nload" not in diagnostics
    assert "reputation" not in diagnostics
    assert "raw_payload" not in diagnostics
    assert "bad\nfield" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["summary_contract"] = ["gateway_load"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan summary_contract must be an object" in errors
    assert (
        "production readiness runner plan summary_contract must match required gates"
        in errors
    )


def test_plan_json_steps_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["steps"] = [
        {
            "label": "sorafs\nproduction",
            "artifact": 7,
            "command": [sys.executable, "bad\nargument"],
            "raw_payload": "not allowed",
            "bad\nfield": "not allowed",
        },
        "step-shaped-entry",
        {"label": "empty_command", "artifact": None, "command": []},
    ]

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan step fields must be label, artifact, and command"
        in diagnostics
    )
    assert (
        "production readiness runner plan step fields must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan step labels must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan step artifacts must be null or canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan step commands must be non-empty lists"
        in diagnostics
    )
    assert (
        "production readiness runner plan step commands must contain canonical strings"
        in diagnostics
    )
    assert "production readiness runner plan steps must contain objects" in diagnostics
    assert "production readiness runner plan steps must match command plan" in diagnostics
    assert "sorafs\nproduction" not in diagnostics
    assert "bad\nargument" not in diagnostics
    assert "raw_payload" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "step-shaped-entry" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["steps"] = "sorafs_production_readiness_gate"

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan steps must be a non-empty list" in errors
    assert "production readiness runner plan steps must match command plan" in errors


def test_plan_json_rejects_unsafe_rendered_paths(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    unsafe_summary = write_json(tmp_path / "private_key_summary.json")
    args.gateway_load_summary = [unsafe_summary]
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert MODULE.PLAN_RENDERED_PATH_ERROR in errors
    assert "private_key_summary" not in "\n".join(errors)


def test_plan_json_rejects_tampered_unsafe_rendered_path_positions(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    unsafe_path = str(tmp_path / "bearer%26%2395%3Btoken-summary.json")

    def rendered_with_mutation(position: str) -> dict:
        rendered = MODULE.plan_json(plan, args)
        if position == "external_summaries":
            rendered["external_summaries"]["gateway_load"] = [unsafe_path]
        elif position == "artifact":
            rendered["steps"][0]["artifact"] = unsafe_path
        elif position == "verifier":
            rendered["steps"][0]["command"][1] = unsafe_path
        elif position == "evidence":
            evidence_index = rendered["steps"][0]["command"].index("--evidence") + 1
            rendered["steps"][0]["command"][evidence_index] = unsafe_path
        elif position == "summary_out":
            summary_index = rendered["steps"][0]["command"].index("--summary-out") + 1
            rendered["steps"][0]["command"][summary_index] = unsafe_path
        else:  # pragma: no cover - fixed local matrix
            raise AssertionError(position)
        return rendered

    for position in (
        "external_summaries",
        "artifact",
        "verifier",
        "evidence",
        "summary_out",
    ):
        errors = MODULE.validate_plan_json(
            rendered_with_mutation(position),
            plan,
            args,
        )
        diagnostics = "\n".join(errors)
        assert MODULE.PLAN_RENDERED_PATH_ERROR in errors
        assert "bearer%26%2395%3Btoken-summary" not in diagnostics
        assert "bearer_token" not in diagnostics


def test_rendered_plan_path_guard_ignores_non_path_command_values(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["steps"][0]["command"].extend(["--future-label", "private_key_label"])

    assert MODULE.rendered_plan_paths_are_safe(rendered)


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["production readiness runner plan steps must match command plan"]

    def fake_run_command_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    assert exit_code == 2
    assert not ran_plan
    assert (
        "production readiness runner plan steps must match command plan"
        in capsys.readouterr().err
    )


def test_execution_rejects_non_object_plan_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_plan_json(plan, args):
        return ["step"]

    def fake_run_command_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "plan_json", fake_plan_json)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    captured = capsys.readouterr()
    assert exit_code == 2
    assert not ran_plan
    assert captured.out == ""
    assert "production readiness runner plan must be an object" in captured.err


def test_execution_rejects_unrenderable_plan_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    original_plan_json = MODULE.plan_json
    ran_plan = False

    def fake_plan_json(plan, args):
        rendered = original_plan_json(plan, args)
        rendered["thresholds"]["max_summary_artifact_age_secs"] = float("inf")
        return rendered

    def fake_run_command_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "plan_json", fake_plan_json)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    captured = capsys.readouterr()
    assert exit_code == 2
    assert not ran_plan
    assert captured.out == ""
    assert (
        "production readiness runner plan must be strict JSON renderable"
        in captured.err
    )
    assert "inf" not in captured.err


def test_missing_required_summary_fails(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--require-gate",
            "gateway_load",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert "missing required production readiness summary input" in captured.err
    assert "gateway_load" not in captured.err


def test_unrequired_summary_flag_fails(tmp_path: Path, capsys) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    reputation_summary = write_json(tmp_path / "reputation.json")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--reputation-summary",
            str(reputation_summary),
            "--require-gate",
            "gateway_load",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert "summary supplied for unrequired production readiness gate" in captured.err
    assert "reputation" not in captured.err


def test_duplicate_required_summary_flag_fails(tmp_path: Path, capsys) -> None:
    first_summary = write_json(tmp_path / "gateway-load.json")
    second_summary = write_json(tmp_path / "gateway-load-copy.json")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(first_summary),
            "--gateway-load-summary",
            str(second_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert (
        "production readiness runner requires exactly one summary input per required gate"
        in captured.err
    )
    assert "gateway-load-copy" not in captured.err


def test_response_file_arguments_pass(tmp_path: Path, capsys) -> None:
    args_file = tmp_path / "production-readiness.args"
    args_file.write_text("\n".join(complete_args(tmp_path) + ["--dry-run"]) + "\n", encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0
    assert json.loads(capsys.readouterr().out)["schema"] == (
        "sorafs.production_readiness.collection_plan.v1"
    )


def test_response_file_symlink_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    target = tmp_path / "private-key-args"
    target.write_text("--require-gate gateway_load\n", encoding="utf-8")
    symlink = tmp_path / "production-readiness.args"
    symlink.symlink_to(target)

    assert MODULE.main([f"@{symlink}"]) == 2

    captured = capsys.readouterr()
    assert "@ARGFILE must not be a symlink" in captured.err
    assert "private-key-args" not in captured.err
    assert "production-readiness.args" not in captured.err
    assert captured.out == ""


def test_response_file_malformed_line_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    args_file = tmp_path / "production-readiness.args"
    args_file.write_text(
        "--require-gate 'private-key-placeholder\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 2

    captured = capsys.readouterr()
    assert "@ARGFILE line 1:" in captured.err
    assert "private-key-placeholder" not in captured.err
    assert "production-readiness.args" not in captured.err
    assert captured.out == ""


def test_narrowed_required_gate_plan_is_rejected(
    tmp_path: Path,
    capsys,
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    foundational_summary = write_json(tmp_path / "foundational-prerequisites.json")
    topology_path = write_topology_qualification(
        tmp_path / "l1-topology-qualification.json"
    )
    exit_code = MODULE.main(
            [
                *signed_topology_cli_args(
                    topology_path,
                    deployment_id="sorafs-mainnet-2026-06",
                    environment="production",
                    now_unix=1_800_800_000,
                ),
                *INVENTORY_SUPPORT.inventory_cli_args(
                    write_lane_inventory(tmp_path / "l1-lane-evidence.inventory")
                ),
                "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--now-unix",
            "1800800000",
            "--gateway-load-summary",
            str(gateway_summary),
            "--foundational-prerequisite-summary",
            str(foundational_summary),
            "--foundational-prerequisite-signer-public-key-hex",
            FOUNDATIONAL_SIGNER_PUBLIC_KEY_HEX,
            "--foundational-prerequisite-release-sequence",
            str(FOUNDATIONAL_RELEASE_SEQUENCE),
            "--foundational-prerequisite-previous-envelope-sha256",
            FOUNDATIONAL_PREVIOUS_ENVELOPE_SHA256,
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert captured.out == ""
    assert MODULE.CANONICAL_GATE_INVENTORY_ERROR in captured.err


def test_reordered_complete_required_gate_plan_is_rejected(
    tmp_path: Path,
    capsys,
) -> None:
    reordered = ",".join(reversed(MODULE.DEFAULT_REQUIRED_GATES))

    assert (
        MODULE.main(
            [
                *complete_args(tmp_path),
                "--require-gate",
                reordered,
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert captured.out == ""
    assert MODULE.CANONICAL_GATE_INVENTORY_ERROR in captured.err


def test_partial_deployment_context_fails(tmp_path: Path, capsys) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert (
        "production readiness runner requires --deployment-id and --environment"
        in captured.err
    )


def test_malformed_deployment_context_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = " sorafs-mainnet-2026-06"
    args.environment = "prod\nsecret"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment context must use canonical labels"
        in errors
    )
    rendered = "\n".join(errors)
    assert "sorafs-mainnet-2026-06" not in rendered
    assert "prod\nsecret" not in rendered


def test_nonproduction_environment_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.environment = "staging"

    errors = MODULE.validate_inputs(args)

    assert "production readiness runner environment must be production" in errors
    assert "staging" not in "\n".join(errors)


def test_unreviewed_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-notproductionready-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['notproductionready']"
        in errors
    )
    assert "gateway-notproductionready-a" not in "\n".join(errors)


def test_staging_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-staging-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-staging-a" not in "\n".join(errors)


def test_numbered_staging_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-staging1-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-staging1-a" not in "\n".join(errors)


def test_compact_staging_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-stagingready-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-stagingready-a" not in "\n".join(errors)


def test_joined_nonproduction_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-testproduction-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['test']"
        in errors
    )
    assert "gateway-testproduction-202606" not in "\n".join(errors)


def test_prerelease_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prerelease-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['prerelease']"
        in errors
    )
    assert "gateway-prerelease-202606" not in "\n".join(errors)


def test_tokenized_prerelease_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-rc-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['rc']"
        in errors
    )
    assert "gateway-prod-rc-202606" not in "\n".join(errors)


def test_preview_prerelease_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-productionpreview-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['preview']"
        in errors
    )
    assert "gateway-productionpreview-202606" not in "\n".join(errors)


def test_canary_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-canary-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['canary']"
        in errors
    )
    assert "gateway-prod-canary-202606" not in "\n".join(errors)


def test_stg_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-stg-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['stg']"
        in errors
    )
    assert "gateway-prod-stg-202606" not in "\n".join(errors)


def test_poc_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-poc-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['poc']"
        in errors
    )
    assert "gateway-prod-poc-202606" not in "\n".join(errors)


def test_smoke_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-production-smoke-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['smoke']"
        in errors
    )
    assert "gateway-production-smoke-202606" not in "\n".join(errors)


def test_stress_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-stress-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['stress']"
        in errors
    )
    assert "gateway-prod-stress-202606" not in "\n".join(errors)


def test_shadow_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-shadow-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['shadow']"
        in errors
    )
    assert "gateway-prod-shadow-202606" not in "\n".join(errors)


def test_cutover_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-cutover-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['cutover']"
        in errors
    )
    assert "gateway-prod-cutover-202606" not in "\n".join(errors)


def test_summary_input_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    unsafe_summary = write_json(tmp_path / "gateway_private_key_summary.json")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(unsafe_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert (
        "production readiness runner summary input paths must not contain "
        "secret-looking, control-character, parent, current, or platform-specific components"
        in captured.err
    )
    assert captured.out == ""
    assert "gateway_private_key_summary" not in captured.err
    assert "private_key" not in captured.err


def test_encoded_summary_input_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    unsafe_summary = write_json(tmp_path / "gateway_private&#95;key_summary.json")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(unsafe_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert (
        "production readiness runner summary input paths must not contain "
        "secret-looking, control-character, parent, current, or platform-specific components"
        in captured.err
    )
    assert captured.out == ""
    assert "gateway_private&#95;key_summary" not in captured.err
    assert "&#95;" not in captured.err
    assert "private_key" not in captured.err


def test_plan_rendered_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "private_key_output"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "private_key_output" not in captured.err
    assert "private_key" not in captured.err


def test_plan_rendered_summary_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--summary-out",
            str(tmp_path / "bearer_token_summary.json"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "bearer_token_summary" not in captured.err
    assert "bearer_token" not in captured.err


def test_plan_rendered_verifier_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    unsafe_verifier = tmp_path / "private_key_verifier.py"
    unsafe_verifier.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(unsafe_verifier),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "private_key_verifier" not in captured.err
    assert "private_key" not in captured.err


def test_encoded_plan_rendered_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    encoded_output = tmp_path / "private%26%2395%3Bkey-output"

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(encoded_output),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "private%26%2395%3Bkey-output" not in captured.err
    assert "private_key" not in captured.err


def test_encoded_plan_rendered_summary_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    encoded_summary = tmp_path / "bearer%26%2395%3Btoken-summary.json"

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--summary-out",
            str(encoded_summary),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "bearer%26%2395%3Btoken-summary" not in captured.err
    assert "bearer_token" not in captured.err


def test_encoded_plan_rendered_verifier_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    encoded_verifier = tmp_path / "private%26%2395%3Bkey-verifier.py"
    encoded_verifier.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    exit_code = MODULE.main(
        [
            *topology_args(tmp_path),
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(encoded_verifier),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "private%26%2395%3Bkey-verifier" not in captured.err
    assert "private_key" not in captured.err


def test_plan_rendered_path_safety_rejects_drive_prefix() -> None:
    assert not MODULE.plan_rendered_path_is_safe(Path("C:/sorafs/summary.json"))


def test_summary_input_path_safety_accepts_digest_labels(tmp_path: Path) -> None:
    safe_summary = write_json(tmp_path / "gateway_load_digest.json")
    foundational_summary = write_json(
        tmp_path / "foundational_prerequisite_digest.json"
    )
    args = MODULE.parse_args(complete_args(tmp_path))
    args.gateway_load_summary = [safe_summary]
    args.foundational_prerequisite_summary = [foundational_summary]

    assert MODULE.validate_inputs(args) == []


def replay_input_snapshot() -> MODULE.InputDigestSnapshot:
    """Return signed topology, resilience, foundation, and 17-lane inputs."""

    return tuple(
        (slot, hashlib.sha256(slot.encode("utf-8")).hexdigest())
        for slot in MODULE.REPLAY_INPUT_SLOTS
    )


def promotion_payload() -> dict:
    """Return the explicit promotion fields enforced above aggregate validation."""

    return {
        "schema": MODULE.SUMMARY_SCHEMA,
        "status": "ready",
        "required_gates": list(MODULE.DEFAULT_REQUIRED_GATES),
        "summary_file_count": 17,
        "recognized_summary_count": 17,
        "resilience_qualification": {
            "present": True,
            "valid": True,
            "binding": {"schema": "trusted-test-binding"},
            "errors": [],
        },
        "l1_lane_evidence_inventory": {
            "present": True,
            "valid": True,
            "binding": {"schema": "trusted-test-inventory-binding"},
            "errors": [],
        },
        "required": {
            gate: {
                "present": True,
                "valid": True,
                "errors": [],
            }
            for gate in MODULE.DEFAULT_REQUIRED_GATES
        },
        "foundational_prerequisites": {
            "present": True,
            "valid": True,
            "errors": [],
        },
        "errors": [],
    }


def test_promotion_aggregate_requires_exact_ready_ordered_inventory(
    monkeypatch,
) -> None:
    monkeypatch.setattr(
        MODULE,
        "validate_aggregate_summary_output",
        lambda payload, required_gates, errors: None,
    )
    payload = promotion_payload()

    assert MODULE.validate_promotion_aggregate(payload) == []

    payload.pop("resilience_qualification")
    assert (
        "replayed aggregate resilience qualification must be present, valid, "
        "bound, and error-free"
        in MODULE.validate_promotion_aggregate(payload)
    )

    payload = promotion_payload()
    payload["required"] = dict(reversed(tuple(payload["required"].items())))
    assert (
        "replayed aggregate required rows must use the exact canonical ordered "
        "17-gate inventory"
        in MODULE.validate_promotion_aggregate(payload)
    )

    payload = promotion_payload()
    payload["status"] = "partial"
    assert (
        "replayed aggregate status must be ready"
        in MODULE.validate_promotion_aggregate(payload)
    )


def test_replay_manifest_is_schema_closed_digest_only() -> None:
    snapshot = replay_input_snapshot()
    payload = promotion_payload()
    aggregate_digest = hashlib.sha256(b"aggregate").hexdigest()
    replay = MODULE.ReplayAggregate(
        payload=payload,
        first_sha256=aggregate_digest,
        second_sha256=aggregate_digest,
        semantic_sha256=hashlib.sha256(b"semantic").hexdigest(),
    )

    manifest = MODULE.build_replay_manifest(snapshot, replay)

    assert MODULE.validate_replay_manifest(manifest, snapshot, replay) == []
    assert set(manifest) == MODULE.REPLAY_MANIFEST_FIELDS
    assert len(manifest["input_sha256"]) == len(MODULE.REPLAY_INPUT_SLOTS) == 22
    assert all(
        set(row) == MODULE.REPLAY_INPUT_DIGEST_FIELDS
        for row in manifest["input_sha256"]
    )
    rendered = json.dumps(manifest)
    assert "payload" not in rendered
    assert "path" not in rendered

    manifest["raw_payload"] = "runtime-only-material"
    diagnostics = "\n".join(
        MODULE.validate_replay_manifest(manifest, snapshot, replay)
    )
    assert "schema-closed contract" in diagnostics
    assert "runtime-only-material" not in diagnostics


def test_published_replay_manifest_requires_exact_valid_readback(
    tmp_path: Path,
) -> None:
    snapshot = replay_input_snapshot()
    aggregate_digest = hashlib.sha256(b"aggregate").hexdigest()
    replay = MODULE.ReplayAggregate(
        payload=promotion_payload(),
        first_sha256=aggregate_digest,
        second_sha256=aggregate_digest,
        semantic_sha256=hashlib.sha256(b"semantic").hexdigest(),
    )
    manifest = MODULE.build_replay_manifest(snapshot, replay)
    rendered = MODULE.render_checker_summary(manifest)
    manifest_path = tmp_path / MODULE.REPLAY_MANIFEST_FILENAME

    manifest_path.write_text(rendered, encoding="utf-8")
    assert (
        MODULE.validate_published_replay_manifest(
            manifest_path,
            rendered,
            snapshot,
            replay,
        )
        == []
    )

    manifest_path.write_text(rendered + " ", encoding="utf-8")
    assert MODULE.validate_published_replay_manifest(
        manifest_path,
        rendered,
        snapshot,
        replay,
    ) == [
        "deterministic replay manifest readback must match the exact published bytes"
    ]

    manifest_path.write_bytes(b"{")
    assert MODULE.validate_published_replay_manifest(
        manifest_path,
        rendered,
        snapshot,
        replay,
    ) == [
        "deterministic replay manifest publication failed exact bounded readback"
    ]


def test_deterministic_replay_hashes_before_between_and_after(
    tmp_path: Path,
    monkeypatch,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    snapshot = replay_input_snapshot()
    aggregate_digest = hashlib.sha256(b"aggregate").hexdigest()
    replay = MODULE.ReplayAggregate(
        payload=promotion_payload(),
        first_sha256=aggregate_digest,
        second_sha256=aggregate_digest,
        semantic_sha256=hashlib.sha256(b"semantic").hexdigest(),
    )
    hash_calls = 0
    executed: list[str] = []
    written: list[tuple[Path, dict[str, object]]] = []

    def fake_digest(_args):
        nonlocal hash_calls
        hash_calls += 1
        return snapshot

    def fake_run(step_plan, out_dir):
        assert out_dir == args.out_dir
        assert len(step_plan) == 1
        executed.append(step_plan[0].label)
        return 0

    def fake_write(path, manifest):
        written.append((path, manifest))
        rendered = MODULE.render_checker_summary(manifest)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(rendered, encoding="utf-8")
        return rendered, []

    monkeypatch.setattr(MODULE, "digest_production_inputs", fake_digest)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run)
    monkeypatch.setattr(
        MODULE,
        "load_and_validate_replayed_aggregates",
        lambda first, second: (replay, []),
    )
    monkeypatch.setattr(MODULE, "render_and_write_checker_summary", fake_write)

    assert MODULE.execute_deterministic_replay(args, plan) == 0
    assert hash_calls == 3
    assert executed == [
        "sorafs_production_readiness_gate_first",
        "sorafs_production_readiness_gate_replay",
    ]
    assert written[0][0] == MODULE.replay_manifest_path(args)
    assert MODULE.validate_replay_manifest(written[0][1], snapshot, replay) == []


def test_deterministic_replay_stops_on_between_execution_input_drift(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    before = replay_input_snapshot()
    changed = (*before[:-1], (before[-1][0], "00" * 32))
    snapshots = iter((before, changed))
    executed: list[str] = []

    monkeypatch.setattr(
        MODULE,
        "digest_production_inputs",
        lambda _args: next(snapshots),
    )
    monkeypatch.setattr(
        MODULE,
        "run_command_plan",
        lambda step_plan, out_dir: executed.append(step_plan[0].label) or 0,
    )

    assert MODULE.execute_deterministic_replay(args, plan) == 1
    assert executed == ["sorafs_production_readiness_gate_first"]
    assert "input set changed after first execution" in capsys.readouterr().err
