from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPTS = Path(__file__).resolve().parents[1]
RUNNER_PATH = SCRIPTS / "run_sorafs_gateway_compliance_rollout_evidence.py"
FIXTURES_PATH = (
    SCRIPTS / "tests" / "check_sorafs_gateway_compliance_rollout_evidence_test.py"
)


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


MODULE = load_module("gateway_compliance_runner", RUNNER_PATH)
FIXTURES = load_module("gateway_compliance_runner_fixtures", FIXTURES_PATH)

from sorafs_rollout_runner_test_support import (  # noqa: E402
    write_topology_qualification,
)


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def evidence_args(root: Path) -> list[str]:
    args: list[str] = []
    for kind, builder in FIXTURES.BUILDERS.items():
        path = root / f"{kind}.json"
        write_json(path, builder())
        args.extend([MODULE.EVIDENCE_FLAGS_BY_KIND[kind], str(path)])
    return args


def base_args(root: Path, *, dry_run: bool = False) -> list[str]:
    args = [
        "--out-dir",
        str(root / "output"),
        "--now-unix",
        str(FIXTURES.NOW),
        "--topology-qualification-summary",
        str(
            write_topology_qualification(
                root / "topology-qualification.json",
                deployment_id="sorafs-gateway-release-42",
            )
        ),
        *evidence_args(root / "inputs"),
    ]
    if dry_run:
        args.append("--dry-run")
    return args


def test_dry_run_is_explicitly_non_production(
    tmp_path: Path, capsys
) -> None:
    assert MODULE.main(base_args(tmp_path, dry_run=True)) == 0
    plan = json.loads(capsys.readouterr().out)
    assert "execution_scope" not in plan
    assert plan["required_kinds"][0] == "catalog_promotion"
    assert "feed_promotion" not in plan["required_kinds"]
    assert "precedence" in plan["required_kinds"]
    assert "appeal_override" not in plan["required_kinds"]
    assert plan["thresholds"]["min_catalog_entries"] == 4
    assert plan["thresholds"]["min_catalog_changes"] == 1
    assert "min_denylist_entries" not in plan["thresholds"]
    contract = plan["evidence_contract"]["catalog_promotion"]
    assert contract["schema"] == (
        "sorafs.gateway_compliance.catalog_promotion_canary.v1"
    )
    assert "catalog_digest_hex" in contract["required_payload_fields"]
    assert "bundle_digest_hex" not in contract["required_payload_fields"]
    for kind in FIXTURES.MODULE.PREDECESSOR_BOUND_KINDS:
        assert (
            "predecessor_catalog_sequence"
            in plan["evidence_contract"][kind]["required_payload_fields"]
        )
    enforcement_fields = plan["evidence_contract"]["enforcement_probe"][
        "required_payload_fields"
    ]
    assert "denial_sources_observed" not in enforcement_fields
    assert "denial_source_count" not in enforcement_fields
    honey_fields = plan["evidence_contract"]["honey_audit"][
        "required_payload_fields"
    ]
    assert "attacks_observed" not in honey_fields
    assert "attack_count" not in honey_fields


def test_production_runner_verifies_canonical_inputs(tmp_path: Path) -> None:
    args = base_args(tmp_path)
    parsed = MODULE.parse_args(args)
    plan = MODULE.plan_json(MODULE.build_command_plan(parsed), parsed)
    assert "execution_scope" not in plan
    assert MODULE.main(args) == 0
    summary = json.loads(
        (tmp_path / "output" / "rollout-summary.json").read_text(encoding="utf-8")
    )
    assert summary["status"] == "ready"
    assert summary["valid_catalog_digests"] == [FIXTURES.CATALOG_DIGEST]
    assert summary["valid_catalog_history_bindings"] == [
        {
            "catalog_digest_hex": FIXTURES.CATALOG_DIGEST,
            "catalog_sequence": 8,
            "predecessor_catalog_digest_hex": FIXTURES.PREDECESSOR_DIGEST,
            "predecessor_catalog_sequence": 7,
        }
    ]


def test_catalog_flags_replace_legacy_flags(tmp_path: Path) -> None:
    args = base_args(tmp_path, dry_run=True)
    args.extend(["--feed-promotion-evidence", "legacy.json"])
    assert MODULE.main(args) == 2

    args = base_args(tmp_path, dry_run=True)
    args.extend(["--appeal-override-evidence", "legacy.json"])
    assert MODULE.main(args) == 2


def test_missing_required_catalog_promotion_fails_before_plan(
    tmp_path: Path,
) -> None:
    args = base_args(tmp_path, dry_run=True)
    flag = MODULE.EVIDENCE_FLAGS_BY_KIND["catalog_promotion"]
    index = args.index(flag)
    del args[index : index + 2]
    assert MODULE.main(args) == 2


def test_missing_required_precedence_fails_before_plan(tmp_path: Path) -> None:
    args = base_args(tmp_path, dry_run=True)
    flag = MODULE.EVIDENCE_FLAGS_BY_KIND["precedence"]
    index = args.index(flag)
    del args[index : index + 2]
    assert MODULE.main(args) == 2


def test_unrequired_evidence_fails_closed(tmp_path: Path) -> None:
    inputs = tmp_path / "inputs"
    promotion = inputs / "catalog.json"
    controller = inputs / "controller.json"
    write_json(promotion, FIXTURES.catalog_promotion())
    write_json(controller, FIXTURES.controller_runtime())
    args = [
        "--out-dir",
        str(tmp_path / "out"),
        "--now-unix",
        str(FIXTURES.NOW),
        "--require-kind",
        "catalog_promotion",
        "--catalog-promotion-evidence",
        str(promotion),
        "--controller-runtime-evidence",
        str(controller),
        "--dry-run",
    ]
    assert MODULE.main(args) == 2


@pytest.mark.parametrize(
    "option",
    [
        "--min-gateways",
        "--min-catalog-entries",
        "--min-catalog-changes",
        "--min-honey-probes",
        "--max-route-latency-ms",
        "--max-reload-latency-ms",
    ],
)
def test_positive_thresholds_fail_closed(tmp_path: Path, option: str) -> None:
    args = base_args(tmp_path, dry_run=True)
    args.extend([option, "0"])
    assert MODULE.main(args) == 2


def test_plan_validation_rejects_scope_tampering(tmp_path: Path) -> None:
    args = MODULE.parse_args(base_args(tmp_path, dry_run=True))
    command_plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(command_plan, args)
    rendered["execution_scope"] = "production_verification"
    errors = MODULE.validate_plan_json(rendered, command_plan, args)
    assert any("fields must match" in error for error in errors)


def test_plan_validation_rejects_legacy_anchor_field(tmp_path: Path) -> None:
    args = MODULE.parse_args(base_args(tmp_path, dry_run=True))
    command_plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(command_plan, args)
    fields = rendered["evidence_contract"]["catalog_promotion"][
        "required_payload_fields"
    ]
    fields.append("bundle_digest_hex")
    errors = MODULE.validate_plan_json(rendered, command_plan, args)
    assert errors


def test_response_file_plan_remains_non_production(
    tmp_path: Path, capsys
) -> None:
    args_file = tmp_path / "runner.args"
    args_file.write_text(
        "\n".join(base_args(tmp_path, dry_run=True)) + "\n",
        encoding="utf-8",
    )
    assert MODULE.main([f"@{args_file}"]) == 0
    plan = json.loads(capsys.readouterr().out)
    assert "execution_scope" not in plan


def test_duplicate_evidence_path_fails_closed(tmp_path: Path) -> None:
    inputs = tmp_path / "inputs"
    promotion = inputs / "catalog.json"
    write_json(promotion, FIXTURES.catalog_promotion())
    args = [
        "--out-dir",
        str(tmp_path / "out"),
        "--now-unix",
        str(FIXTURES.NOW),
        "--require-kind",
        "catalog_promotion",
        "--catalog-promotion-evidence",
        str(promotion),
        "--catalog-promotion-evidence",
        str(promotion),
        "--dry-run",
    ]
    assert MODULE.main(args) == 2
