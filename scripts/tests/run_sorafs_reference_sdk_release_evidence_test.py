"""Tests for scripts/run_sorafs_reference_sdk_release_evidence.py."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_reference_sdk_release_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_reference_sdk_release_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

from sorafs_topology_qualification import CANONICAL_READINESS_LANES  # noqa: E402


PROVENANCE_CERTIFICATE_IDENTITY = (
    "https://github.com/hyperledger-iroha/iroha/"
    ".github/workflows/sorafs-cli-release.yml@refs/heads/main"
)
PROVENANCE_OIDC_ISSUER = "https://token.actions.githubusercontent.com"
PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX = "11" * 32
TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX = "22" * 32
TOPOLOGY_SIGNER_IDENTITY = "sorafs-reference-sdk-topology-software"
TOPOLOGY_SIGNER_KEY_REVISION = 7
TOPOLOGY_SIGNER_POLICY_DIGEST_HEX = "33" * 32
TOPOLOGY_MAX_REVIEW_AGE_SECS = 3_600


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def write_topology_qualification(path: Path) -> Path:
    payload = {
        "schema": "sorafs.l1.deployment_qualification.summary.v1",
        "status": "configuration-qualified",
        "qualification_scope": "pre-deployment-configuration",
        "live_evidence_recognized": False,
        "promotion_eligible": False,
        "manifest_sha256": hashlib.sha256(b"release-runner-manifest").hexdigest(),
        "canonical_manifest_sha256": hashlib.sha256(
            b"canonical-release-runner-manifest"
        ).hexdigest(),
        "deployment": {
            "deployment_id": "reference-sdk-release-20260701",
            "environment": "production",
        },
        "validator_count": 4,
        "storage_provider_count": 2,
        "gateway_count": 2,
        "governance_dag_instance_count": 2,
        "runtime_handle_kinds": ["monitoring", "external_signer", "kms", "webauthn"],
        "runtime_material_policy_valid": True,
        "signed_model_artifact_count": 1,
        "required_lane_slots": list(CANONICAL_READINESS_LANES),
        "recognized_lane_slot_count": len(CANONICAL_READINESS_LANES),
        "errors": [],
    }
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def topology_args(tmp_path: Path) -> list[str]:
    summary = write_topology_qualification(tmp_path / "l1-topology.summary")
    envelope = write_payload(tmp_path / "l1-topology.envelope.json")
    return [
        "--topology-qualification-summary",
        str(summary),
        "--topology-qualification-envelope",
        str(envelope),
        "--topology-qualification-verification-public-key-hex",
        TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX,
        "--topology-qualification-signer-identity",
        TOPOLOGY_SIGNER_IDENTITY,
        "--topology-qualification-signer-key-revision",
        str(TOPOLOGY_SIGNER_KEY_REVISION),
        "--topology-qualification-signer-policy-digest-hex",
        TOPOLOGY_SIGNER_POLICY_DIGEST_HEX,
        "--max-topology-qualification-review-age-secs",
        str(TOPOLOGY_MAX_REVIEW_AGE_SECS),
    ]


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    payload_dir.mkdir(parents=True, exist_ok=True)
    source_root = tmp_path / "supply-chain-sources"
    source_root.mkdir(exist_ok=True)
    return [
        "--out-dir",
        str(tmp_path / "evidence"),
        "--now-unix",
        "1800700000",
        "--max-evidence-age-secs",
        "1209600",
        "--min-release-targets",
        "4",
        "--min-downstream-packages",
        "5",
        "--max-smoke-duration-secs",
        "1800",
        *topology_args(tmp_path),
        "--supply-chain-source-root",
        str(source_root),
        "--provenance-certificate-identity",
        PROVENANCE_CERTIFICATE_IDENTITY,
        "--provenance-oidc-issuer",
        PROVENANCE_OIDC_ISSUER,
        "--provenance-verification-public-key-hex",
        PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX,
        "--release-archive-evidence",
        str(write_payload(payload_dir / "release-archive.json")),
        "--signed-manifest-evidence",
        str(write_payload(payload_dir / "signed-manifest.json")),
        "--supply-chain-evidence",
        str(write_payload(payload_dir / "supply-chain.json")),
        "--downstream-bindings-evidence",
        str(write_payload(payload_dir / "downstream-bindings.json")),
        "--cookbook-smoke-evidence",
        str(write_payload(payload_dir / "cookbook-smoke.json")),
        "--ffi-header-contract-evidence",
        str(write_payload(payload_dir / "ffi-header-contract.json")),
        "--governance-approval-evidence",
        str(write_payload(payload_dir / "governance-approval.json")),
    ]


def write_args_file(path: Path, args: list[str]) -> Path:
    lines = [
        "# comments and blank lines are ignored",
        "",
    ]
    for index in range(0, len(args), 2):
        option = args[index]
        value = args[index + 1]
        lines.append(f"{option} {json.dumps(value)}")
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def write_split_args_file(path: Path, args: list[str]) -> Path:
    path.write_text(
        "\n".join(["# one token per line also works for long reviewed inputs", *args]),
        encoding="utf-8",
    )
    return path


def test_dry_run_prints_complete_reference_sdk_release_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reference_sdk.release_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.reference_sdk.release_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 1209600,
        "max_smoke_duration_secs": 1800,
        "max_topology_qualification_review_age_secs": (
            TOPOLOGY_MAX_REVIEW_AGE_SECS
        ),
        "min_downstream_packages": 5,
        "min_release_targets": 4,
        "now_unix": 1800700000,
    }
    assert plan["external_evidence"]["release_archive"] == [
        str(tmp_path / "payloads" / "release-archive.json")
    ]
    assert plan["external_evidence"]["supply_chain"] == [
        str(tmp_path / "payloads" / "supply-chain.json")
    ]
    assert plan["supply_chain_source"] == {
        "provenance_certificate_identity": PROVENANCE_CERTIFICATE_IDENTITY,
        "provenance_oidc_issuer": PROVENANCE_OIDC_ISSUER,
        "provenance_verification_key_fingerprint_hex": hashlib.sha256(
            bytes.fromhex(PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX)
        ).hexdigest(),
        "required": True,
        "source_root": str(tmp_path / "supply-chain-sources"),
    }
    assert plan["topology_qualification"] == {
        "summary_path": str(tmp_path / "l1-topology.summary"),
        "envelope_path": str(tmp_path / "l1-topology.envelope.json"),
        "verification_public_key_fingerprint_hex": hashlib.sha256(
            bytes.fromhex(TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX)
        ).hexdigest(),
        "signer_identity": TOPOLOGY_SIGNER_IDENTITY,
        "signer_key_revision": TOPOLOGY_SIGNER_KEY_REVISION,
        "signer_policy_digest_hex": TOPOLOGY_SIGNER_POLICY_DIGEST_HEX,
    }
    assert plan["evidence_contract"]["release_archive"]["schema"] == (
        "sorafs.reference_sdk.release_archive_canary.v1"
    )
    assert (
        "release_manifest_digest_hex"
        in plan["evidence_contract"]["release_archive"]["required_payload_fields"]
    )
    assert plan["evidence_contract"]["signed_manifest"]["schema"] == (
        "sorafs.reference_sdk.signed_manifest_canary.v1"
    )
    assert (
        "private_key_absent"
        in plan["evidence_contract"]["signed_manifest"]["required_payload_fields"]
    )
    assert plan["evidence_contract"]["supply_chain"]["schema"] == (
        "sorafs.reference_sdk.supply_chain_canary.v1"
    )
    assert (
        "vulnerability_report_digest_hex"
        in plan["evidence_contract"]["supply_chain"]["required_payload_fields"]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["signed_manifest"]["required_payload_fields"]
    )
    assert (
        "validation_outcome_contract_verified"
        in plan["evidence_contract"]["downstream_bindings"][
            "required_payload_fields"
        ]
    )
    assert (
        "raw_smoke_outputs_included"
        in plan["evidence_contract"]["cookbook_smoke"]["required_payload_fields"]
    )
    assert (
        "ffi_contract_digest_hex"
        in plan["evidence_contract"]["ffi_header_contract"][
            "required_payload_fields"
        ]
    )
    assert (
        "governance_source"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert [step["label"] for step in plan["steps"]] == ["release_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_reference_sdk_release_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "cookbook-smoke.json") in verifier
    assert str(tmp_path / "payloads" / "supply-chain.json") in verifier
    assert verifier.count("--release-archive-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-smoke-duration-secs" in verifier
    assert "--now-unix" in verifier
    assert verifier[
        verifier.index("--topology-qualification-summary") + 1
    ] == str(tmp_path / "l1-topology.summary")
    assert verifier[
        verifier.index("--topology-qualification-envelope") + 1
    ] == str(tmp_path / "l1-topology.envelope.json")
    assert verifier[
        verifier.index(
            "--topology-qualification-verification-public-key-hex"
        )
        + 1
    ] == TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX
    assert verifier[
        verifier.index("--topology-qualification-signer-identity") + 1
    ] == TOPOLOGY_SIGNER_IDENTITY
    assert verifier[
        verifier.index("--topology-qualification-signer-key-revision") + 1
    ] == str(TOPOLOGY_SIGNER_KEY_REVISION)
    assert verifier[
        verifier.index(
            "--topology-qualification-signer-policy-digest-hex"
        )
        + 1
    ] == TOPOLOGY_SIGNER_POLICY_DIGEST_HEX
    assert verifier[
        verifier.index(
            "--max-topology-qualification-review-age-secs"
        )
        + 1
    ] == str(TOPOLOGY_MAX_REVIEW_AGE_SECS)
    assert verifier[
        verifier.index("--supply-chain-source-root") + 1
    ] == str(tmp_path / "supply-chain-sources")
    assert verifier[
        verifier.index("--provenance-certificate-identity") + 1
    ] == PROVENANCE_CERTIFICATE_IDENTITY
    assert verifier[
        verifier.index("--provenance-oidc-issuer") + 1
    ] == PROVENANCE_OIDC_ISSUER
    assert verifier[
        verifier.index("--provenance-verification-public-key-hex") + 1
    ] == PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "reference SDK release runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.reference_sdk.release_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["supply_chain_source"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reference SDK release runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "reference SDK release runner plan schema must match the contract" in diagnostics
    assert "reference SDK release runner plan required_kinds must match args" in diagnostics
    assert "reference SDK release runner plan thresholds must match args" in diagnostics
    assert "reference SDK release runner plan external_evidence must match args" in diagnostics
    assert (
        "reference SDK release runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan supply_chain_source fields "
        "must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan supply_chain_source must match args"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "cookbook-smoke.json" not in diagnostics


def test_plan_json_rejects_missing_signed_topology_binding(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    del rendered["topology_qualification"]

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reference SDK release runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan topology_qualification must be an object"
        in diagnostics
    )


def test_plan_json_rejects_tampered_signed_topology_binding(
    tmp_path: Path,
) -> None:
    cases = (
        ("envelope_path", "runtime-only-envelope-substitution"),
        ("verification_public_key_fingerprint_hex", "44" * 32),
        ("signer_identity", "runtime-only-signer-substitution"),
        ("signer_key_revision", 0),
        ("signer_policy_digest_hex", "55" * 32),
    )
    for field, replacement in cases:
        args = MODULE.parse_args(complete_args(tmp_path))
        plan = MODULE.build_command_plan(args)
        rendered = MODULE.plan_json(plan, args)
        rendered["topology_qualification"][field] = replacement

        errors = MODULE.validate_plan_json(rendered, plan, args)
        diagnostics = "\n".join(errors)

        assert (
            "reference SDK release runner plan topology_qualification must match args"
            in diagnostics
        )
        assert str(replacement) not in diagnostics

    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    del rendered["topology_qualification"]["signer_identity"]
    rendered["topology_qualification"]["private_key"] = (
        "runtime-only-key-material"
    )

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reference SDK release runner plan topology_qualification fields "
        "must match the schema-closed contract"
        in diagnostics
    )
    assert "runtime-only-key-material" not in diagnostics
    assert "private_key" not in diagnostics


def test_plan_json_rejects_tampered_topology_verifier_command(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    command = rendered["steps"][0]["command"]
    policy_index = command.index(
        "--topology-qualification-signer-policy-digest-hex"
    )
    substituted_policy = "66" * 32
    command[policy_index + 1] = substituted_policy

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "runner plan steps must match command plan" in diagnostics
    assert substituted_policy not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nreference"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "release_archive",
        "release_archive",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_evidence_age_secs": -1,
        "max_topology_qualification_review_age_secs": -1,
        "min_release_targets": 0,
        "min_downstream_packages": False,
        "max_smoke_duration_secs": "slow",
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "release_archive": [],
        "unknown_kind": ["unknown.json"],
        "signed_manifest": "signed-manifest.json",
        "bad\nkind": ["release-archive.json"],
        "cookbook_smoke": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "release_archive": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.reference_sdk.unknown.v1",
            "required_payload_fields": [],
        },
        "signed_manifest": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["release_archive"].schema,
            "required_payload_fields": ["schema"],
        },
    }
    rendered["supply_chain_source"] = {
        "required": "yes",
        "source_root": "bad\npath",
        "provenance_certificate_identity": "runtime-only-key-material",
        "provenance_oidc_issuer": PROVENANCE_OIDC_ISSUER,
        "provenance_verification_key_fingerprint_hex": "private_key",
        "bad\nfield": True,
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "reference SDK release runner plan fields must be canonical strings" in diagnostics
    assert "reference SDK release runner plan schema must be canonical" in diagnostics
    assert (
        "reference SDK release runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds.max_evidence_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan "
        "thresholds.max_topology_qualification_review_age_secs must be a "
        "non-negative integer"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds.max_smoke_duration_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds.min_downstream_packages must be a positive integer"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds.min_release_targets must be a positive integer"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract required_payload_fields must match checker fields"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan supply_chain_source fields "
        "must be canonical strings"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan supply_chain_source fields "
        "must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan supply_chain_source.required "
        "must be boolean"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan supply_chain_source must match args"
        in diagnostics
    )
    assert "unknown_kind" not in diagnostics
    assert "bad\nkind" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private_key" not in diagnostics
    assert "wrong.schema.v1" not in diagnostics


def test_plan_json_rejects_unrequired_external_evidence_and_contracts(
    tmp_path: Path,
) -> None:
    payload = write_payload(tmp_path / "release-archive.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800700000",
            *topology_args(tmp_path),
            "--require-kind",
            "release_archive",
            "--release-archive-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["signed_manifest"] = [
        str(tmp_path / "signed-manifest.json")
    ]
    rendered["evidence_contract"]["signed_manifest"] = {
        "schema": MODULE.KIND_BY_NAME["signed_manifest"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["signed_manifest"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "reference SDK release runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "reference SDK release runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "signed_manifest" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["reference SDK release runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "reference SDK release runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_reference_sdk_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "reference-sdk-release.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "release_evidence_gate"
    assert plan["external_evidence"]["release_archive"]
    assert "evidence_contract" in plan


def test_split_response_file_dry_run_prints_complete_reference_sdk_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-reference-sdk-release.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reference_sdk.release_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "release_evidence_gate"
    assert "cookbook_smoke" in plan["evidence_contract"]


def test_missing_signed_topology_arguments_fail_before_plan(
    tmp_path: Path,
    capsys,
) -> None:
    required_options = (
        "--now-unix",
        "--topology-qualification-summary",
        "--topology-qualification-envelope",
        "--topology-qualification-verification-public-key-hex",
        "--topology-qualification-signer-identity",
        "--topology-qualification-signer-key-revision",
        "--topology-qualification-signer-policy-digest-hex",
    )
    for option in required_options:
        args = complete_args(tmp_path / option.removeprefix("--"))
        option_index = args.index(option)
        del args[option_index : option_index + 2]

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert option in captured.err
        assert captured.out == ""


def test_missing_signed_topology_envelope_file_fails_before_plan(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "runtime-only-missing-topology-envelope.json"
    envelope_index = args.index("--topology-qualification-envelope") + 1
    args[envelope_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_runner_preflight_rejects_reused_topology_and_provenance_key(
    tmp_path: Path,
    capsys,
) -> None:
    args = complete_args(tmp_path)
    provenance_key_index = args.index(
        "--provenance-verification-public-key-hex"
    ) + 1
    args[provenance_key_index] = TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert MODULE.INDEPENDENT_VERIFICATION_KEYS_ERROR in captured.err
    assert TOPOLOGY_VERIFICATION_PUBLIC_KEY_HEX not in captured.err
    assert captured.out == ""


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--release-archive-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required release evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-cookbook-smoke.json"
    evidence_index = args.index("--cookbook-smoke-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--cookbook-smoke-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_missing_supply_chain_source_inputs_fail_before_plan(
    tmp_path: Path,
    capsys,
) -> None:
    option_diagnostics = {
        "--supply-chain-source-root": "--supply-chain-source-root is required",
        "--provenance-certificate-identity": (
            "--provenance-certificate-identity is required"
        ),
        "--provenance-oidc-issuer": "--provenance-oidc-issuer is required",
        "--provenance-verification-public-key-hex": (
            "--provenance-verification-public-key-hex must be a non-zero"
        ),
    }
    for option, diagnostic in option_diagnostics.items():
        args = complete_args(tmp_path / option.removeprefix("--"))
        option_index = args.index(option)
        del args[option_index : option_index + 2]

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert captured.out == ""


def test_supply_chain_source_root_must_exist_and_not_be_a_symlink(
    tmp_path: Path,
    capsys,
) -> None:
    for case in ("missing", "symlink"):
        case_root = tmp_path / case
        case_root.mkdir()
        args = complete_args(case_root)
        source_index = args.index("--supply-chain-source-root") + 1
        if case == "missing":
            source_root = case_root / "missing-source-root"
        else:
            target = case_root / "source-target"
            target.mkdir()
            source_root = case_root / "source-link"
            source_root.symlink_to(target, target_is_directory=True)
        args[source_index] = str(source_root)

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert "--supply-chain-source-root" in captured.err
        assert (
            "must exist and be a directory" in captured.err
            if case == "missing"
            else "must not be a symlink" in captured.err
        )
        assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "release-archive.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800700000",
            *topology_args(tmp_path),
            "--require-kind",
            "release_archive",
            "--release-archive-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["release_archive"]
    assert list(plan["evidence_contract"]) == ["release_archive"]
    assert plan["supply_chain_source"] == {
        "provenance_certificate_identity": None,
        "provenance_oidc_issuer": None,
        "provenance_verification_key_fingerprint_hex": None,
        "required": False,
        "source_root": None,
    }
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "release_archive" in verifier
    assert "--supply-chain-source-root" not in verifier


def test_subset_gate_rejects_supply_chain_source_inputs(
    tmp_path: Path,
    capsys,
) -> None:
    payload = write_payload(tmp_path / "release-archive.json")
    source_root = tmp_path / "supply-chain-sources"
    source_root.mkdir()

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800700000",
            *topology_args(tmp_path),
            "--require-kind",
            "release_archive",
            "--release-archive-evidence",
            str(payload),
            "--supply-chain-source-root",
            str(source_root),
            "--provenance-certificate-identity",
            PROVENANCE_CERTIFICATE_IDENTITY,
            "--provenance-oidc-issuer",
            PROVENANCE_OIDC_ISSUER,
            "--provenance-verification-public-key-hex",
            PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX,
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert (
        "supply-chain source inputs require the `supply_chain` evidence kind"
        in captured.err
    )
    assert PROVENANCE_CERTIFICATE_IDENTITY not in captured.err
    assert PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX not in captured.err
    assert captured.out == ""


def test_supply_chain_verification_key_must_be_canonical_nonzero_ed25519_hex(
    tmp_path: Path,
    capsys,
) -> None:
    invalid_values = (
        "00" * 32,
        "11" * 31,
        "AA" * 32,
        "not-hex",
    )
    for index, invalid in enumerate(invalid_values):
        args = complete_args(tmp_path / f"invalid-key-{index}")
        key_index = args.index("--provenance-verification-public-key-hex") + 1
        args[key_index] = invalid

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert (
            "--provenance-verification-public-key-hex must be a non-zero "
            "raw 32-byte Ed25519 public key in lowercase hex"
            in captured.err
        )
        assert invalid not in captured.err
        assert captured.out == ""


def test_supply_chain_provenance_identity_must_be_canonical(
    tmp_path: Path,
    capsys,
) -> None:
    cases = (
        (
            "--provenance-certificate-identity",
            "https://example.com/workflow\ninjected",
            "argument must be a non-empty canonical string",
        ),
        (
            "--provenance-oidc-issuer",
            "https://issuer.example.com\ninjected",
            "argument must be a non-empty canonical string",
        ),
    )
    for index, (option, invalid, diagnostic) in enumerate(cases):
        args = complete_args(tmp_path / f"invalid-url-{index}")
        value_index = args.index(option) + 1
        args[value_index] = invalid

        assert MODULE.main([*args, "--dry-run"]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert invalid not in captured.err
        assert "injected" not in captured.err
        assert captured.out == ""


def test_subset_gate_rejects_evidence_for_unrequired_kind(
    tmp_path: Path, capsys
) -> None:
    release_payload = write_payload(tmp_path / "release-archive.json")
    extra_payload = write_payload(tmp_path / "signed-manifest.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800700000",
            *topology_args(tmp_path),
            "--require-kind",
            "release_archive",
            "--release-archive-evidence",
            str(release_payload),
            "--signed-manifest-evidence",
            str(extra_payload),
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert "release evidence supplied for unrequired kind" in captured.err
    assert "signed_manifest" not in captured.err
    assert str(extra_payload) not in captured.err
    assert captured.out == ""


def test_unknown_required_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence"),
                "--now-unix",
                "1800700000",
                *topology_args(tmp_path),
                "--require-kind",
                "unknown",
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert captured.out == ""
