"""Tests for the pre-deployment SoraFS L1 topology contract."""

from __future__ import annotations

import importlib.util
import hashlib
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "check_sorafs_l1_deployment_qualification.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_l1_deployment_qualification",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

EXAMPLES_DIR = MODULE_PATH.parent / "examples"
ARGS_EXAMPLE = EXAMPLES_DIR / "sorafs_l1_deployment_qualification.args.example"
MANIFEST_EXAMPLE = (
    EXAMPLES_DIR / "sorafs_l1_deployment_qualification_manifest.json.example"
)
DEPLOYMENT_ID = "sorafs-mainnet-2026-07"
ENVIRONMENT = "production"
DIGEST = "ab" * 32


def valid_manifest() -> dict:
    """Return one complete, non-secret topology plan."""

    return {
        "schema": MODULE.MANIFEST_SCHEMA,
        "deployment": {
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
        },
        "validators": [
            {
                "validator_id": f"validator-{suffix}",
                "voting": True,
                "da_enabled": True,
                "rbc_enabled": True,
            }
            for suffix in ("a", "b", "c", "d")
        ],
        "storage_providers": [
            {"provider_id": "provider-a", "operator_id": "storage-operator-a"},
            {"provider_id": "provider-b", "operator_id": "storage-operator-b"},
        ],
        "gateways": [
            {
                "gateway_id": "gateway-eu",
                "region": "eu-west",
                "administrator_id": "gateway-admin-a",
            },
            {
                "gateway_id": "gateway-ap",
                "region": "ap-south",
                "administrator_id": "gateway-admin-b",
            },
        ],
        "governance_dag_instances": [
            {
                "instance_id": "governance-dag-a",
                "kubo_handle": "kubo-prod-primary-a",
                "administrator_id": "dag-admin-a",
            },
            {
                "instance_id": "governance-dag-b",
                "kubo_handle": "kubo-prod-primary-b",
                "administrator_id": "dag-admin-b",
            },
        ],
        "runtime_handles": {
            "monitoring": "monitoring-prod-fleet",
            "external_signer": "external-signer-prod-release",
            "kms": "kms-prod-envelope",
            "webauthn": "webauthn-prod-operators",
        },
        "runtime_material_policy": {
            "configuration_contains_credentials": False,
            "configuration_contains_private_material": False,
            "external_injection_required": True,
        },
        "signed_model_artifacts": [
            {
                "artifact_id": "moderation-prescreen-primary",
                "revision": 7,
                "artifact_sha256": "12" * 32,
                "signature_algorithm": "ed25519",
                "signature_sha256": "34" * 32,
                "signer_public_key_fingerprint_sha256": "56" * 32,
                "signature_verified": True,
            }
        ],
        "lane_slots": [
            {
                "gate": gate,
                "deployment_id": DEPLOYMENT_ID,
                "environment": ENVIRONMENT,
            }
            for gate in MODULE.DEFAULT_REQUIRED_GATES
        ],
    }


def validate(payload: dict) -> tuple[dict, list[str]]:
    return MODULE.validate_manifest(
        payload,
        DIGEST,
        expected_deployment_id=DEPLOYMENT_ID,
        expected_environment=ENVIRONMENT,
    )


def write_json(path: Path, payload: dict) -> None:
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def test_complete_topology_is_configuration_qualified_only() -> None:
    summary, errors = validate(valid_manifest())

    assert errors == []
    assert summary["status"] == "configuration-qualified"
    assert summary["qualification_scope"] == "pre-deployment-configuration"
    assert summary["live_evidence_recognized"] is False
    assert summary["promotion_eligible"] is False
    assert summary["validator_count"] == 4
    assert summary["storage_provider_count"] == 2
    assert summary["gateway_count"] == 2
    assert summary["governance_dag_instance_count"] == 2
    assert summary["runtime_handle_kinds"] == [
        "monitoring",
        "external_signer",
        "kms",
        "webauthn",
    ]
    assert summary["signed_model_artifact_count"] == 1
    assert summary["recognized_lane_slot_count"] == 17
    assert summary["required_lane_slots"] == list(MODULE.DEFAULT_REQUIRED_GATES)


def test_schema_complete_production_topology_example_qualifies() -> None:
    payload = json.loads(MANIFEST_EXAMPLE.read_text(encoding="utf-8"))
    summary, errors = validate(payload)
    args = MODULE.parse_args([f"@{ARGS_EXAMPLE}"])

    assert errors == []
    assert summary["status"] == "configuration-qualified"
    assert summary["recognized_lane_slot_count"] == 17
    assert args.evidence == [
        Path(
            "scripts/examples/"
            "sorafs_l1_deployment_qualification_manifest.json.example"
        )
    ]
    assert args.deployment_id == DEPLOYMENT_ID
    assert args.environment == ENVIRONMENT
    assert args.summary_out == Path(
        "artifacts/sorafs/production-readiness/l1-topology-qualification.json"
    )


@pytest.mark.parametrize(
    ("mutation", "diagnostic"),
    [
        (
            lambda payload: payload["validators"].pop(),
            "validators must contain exactly 4 voting validators",
        ),
        (
            lambda payload: payload["validators"][0].update(voting=False),
            "validators[0].voting must be true",
        ),
        (
            lambda payload: payload["validators"][0].update(da_enabled=False),
            "validators[0].da_enabled must be true",
        ),
        (
            lambda payload: payload["validators"][0].update(rbc_enabled=False),
            "validators[0].rbc_enabled must be true",
        ),
        (
            lambda payload: payload.update(
                storage_providers=payload["storage_providers"][:1]
            ),
            "storage_providers must contain between 2 and 64 providers",
        ),
        (
            lambda payload: payload["storage_providers"][1].update(
                operator_id="storage-operator-a"
            ),
            "storage providers must have at least two distinct operators",
        ),
        (
            lambda payload: payload["gateways"][1].update(
                administrator_id="gateway-admin-a"
            ),
            "gateway administrator id values must be unique",
        ),
        (
            lambda payload: payload["gateways"][1].update(region="eu-west"),
            "gateway region values must be unique",
        ),
        (
            lambda payload: payload["governance_dag_instances"].pop(),
            "governance_dag_instances must contain exactly 2 entries",
        ),
        (
            lambda payload: payload["governance_dag_instances"][1].update(
                kubo_handle="kubo-prod-primary-a"
            ),
            "Kubo runtime handles must be unique",
        ),
        (
            lambda payload: payload["governance_dag_instances"][1].update(
                administrator_id="dag-admin-a"
            ),
            "Governance DAG administrator identities must be unique",
        ),
        (
            lambda payload: payload["runtime_handles"].pop("external_signer"),
            "runtime_handles fields must match the schema-closed contract",
        ),
        (
            lambda payload: (
                payload["runtime_handles"].pop("external_signer"),
                payload["runtime_handles"].update(hsm="hsm-prod-release"),
            ),
            "runtime_handles fields must match the schema-closed contract",
        ),
        (
            lambda payload: payload["runtime_handles"].update(
                webauthn="webauthn-test-operators"
            ),
            "runtime_handles.webauthn must be a canonical production runtime handle",
        ),
        (
            lambda payload: payload["runtime_handles"].update(
                external_signer="mock-external-signer-prod-release"
            ),
            "runtime_handles.external_signer must be a canonical production runtime handle",
        ),
        (
            lambda payload: payload["runtime_handles"].update(
                webauthn="external-signer-prod-release"
            ),
            "runtime handles must be distinct",
        ),
        (
            lambda payload: payload["runtime_material_policy"].update(
                configuration_contains_credentials=True
            ),
            "runtime_material_policy.configuration_contains_credentials must be false",
        ),
        (
            lambda payload: payload["runtime_material_policy"].update(
                external_injection_required=False
            ),
            "runtime_material_policy.external_injection_required must be true",
        ),
        (
            lambda payload: payload.update(signed_model_artifacts=[]),
            "signed_model_artifacts must contain between 1 and 64 entries",
        ),
        (
            lambda payload: payload["signed_model_artifacts"][0].update(
                signature_verified=False
            ),
            "signed_model_artifacts[0].signature_verified must be true",
        ),
        (
            lambda payload: payload["signed_model_artifacts"][0].update(
                artifact_id="moderation-model-test"
            ),
            "signed_model_artifacts[0].artifact_id must identify a production model",
        ),
        (
            lambda payload: payload["signed_model_artifacts"][0].update(
                artifact_id="testmodel-primary"
            ),
            "signed_model_artifacts[0].artifact_id must identify a production model",
        ),
        (
            lambda payload: payload["signed_model_artifacts"][0].update(
                signature_algorithm="rsa-sha256"
            ),
            "signed_model_artifacts[0].signature_algorithm must be one of",
        ),
        (
            lambda payload: payload["signed_model_artifacts"][0].update(
                artifact_sha256="00" * 32
            ),
            "signed_model_artifacts[0].artifact_sha256 must not be zero",
        ),
        (
            lambda payload: payload["lane_slots"].pop(),
            "lane_slots must contain exactly 17 entries",
        ),
        (
            lambda payload: payload["lane_slots"].reverse(),
            "lane_slots must match all 17 readiness lanes in canonical order",
        ),
        (
            lambda payload: payload["lane_slots"][0].update(
                deployment_id="sorafs-mainnet-other"
            ),
            "lane_slots[0].deployment_id must match deployment context",
        ),
        (
            lambda payload: payload["lane_slots"][0].update(environment="prod"),
            "lane_slots[0].environment must match deployment context",
        ),
        (
            lambda payload: payload["lane_slots"][0].update(
                gate=payload["lane_slots"][1]["gate"]
            ),
            "lane_slots must not contain duplicate gates",
        ),
    ],
)
def test_topology_requirements_fail_closed(mutation, diagnostic: str) -> None:
    payload = valid_manifest()
    mutation(payload)

    summary, errors = validate(payload)

    assert any(diagnostic in error for error in errors)
    assert summary["status"] == "blocked"
    assert summary["promotion_eligible"] is False


def test_unknown_fields_and_embedded_secret_material_are_rejected() -> None:
    payload = valid_manifest()
    payload["private_key"] = "-----BEGIN PRIVATE KEY-----\nnot-a-key"

    summary, errors = validate(payload)
    diagnostics = "\n".join(errors)

    assert summary["status"] == "blocked"
    assert "<sensitive-key>" in diagnostics
    assert "not-a-key" not in diagnostics
    assert (
        "deployment qualification manifest fields must match the schema-closed contract"
        in errors
    )


def test_unknown_lane_fields_prevent_lane_slot_recognition() -> None:
    payload = valid_manifest()
    payload["lane_slots"][0]["alias"] = "ai"

    summary, errors = validate(payload)

    assert "lane_slots[0] fields must match the schema-closed contract" in errors
    assert summary["recognized_lane_slot_count"] == 0


def test_manifest_context_must_match_operator_reviewed_context() -> None:
    payload = valid_manifest()
    payload["deployment"]["deployment_id"] = "sorafs-mainnet-2026-08"
    for row in payload["lane_slots"]:
        row["deployment_id"] = "sorafs-mainnet-2026-08"

    summary, errors = validate(payload)

    assert summary["status"] == "blocked"
    assert (
        "deployment.deployment_id must match the operator-reviewed value" in errors
    )


def test_cli_writes_non_promotable_configuration_summary(tmp_path: Path) -> None:
    manifest = tmp_path / "topology.json"
    summary_path = tmp_path / "summary.json"
    write_json(manifest, valid_manifest())

    assert (
        MODULE.main(
            [
                "--manifest",
                str(manifest),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary_path),
            ]
        )
        == 0
    )
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["status"] == "configuration-qualified"
    assert summary["live_evidence_recognized"] is False
    assert summary["promotion_eligible"] is False
    assert summary["manifest_sha256"] == hashlib.sha256(
        manifest.read_bytes()
    ).hexdigest()
    assert summary["canonical_manifest_sha256"] == (
        MODULE.canonical_manifest_sha256(valid_manifest())
    )


def test_exact_and_canonical_manifest_digests_detect_byte_substitution(
    tmp_path: Path,
) -> None:
    """Equivalent JSON binds the same model but never the same reviewed bytes."""

    compact = tmp_path / "compact.json"
    pretty = tmp_path / "pretty.json"
    compact_summary = tmp_path / "compact-summary.json"
    pretty_summary = tmp_path / "pretty-summary.json"
    payload = valid_manifest()
    compact.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")),
        encoding="utf-8",
    )
    write_json(pretty, payload)

    for manifest, summary in (
        (compact, compact_summary),
        (pretty, pretty_summary),
    ):
        assert (
            MODULE.main(
                [
                    "--manifest",
                    str(manifest),
                    "--deployment-id",
                    DEPLOYMENT_ID,
                    "--environment",
                    ENVIRONMENT,
                    "--summary-out",
                    str(summary),
                ]
            )
            == 0
        )

    compact_payload = json.loads(compact_summary.read_text(encoding="utf-8"))
    pretty_payload = json.loads(pretty_summary.read_text(encoding="utf-8"))
    assert compact_payload["manifest_sha256"] != pretty_payload["manifest_sha256"]
    assert (
        compact_payload["canonical_manifest_sha256"]
        == pretty_payload["canonical_manifest_sha256"]
    )


def test_cli_rejects_duplicate_json_keys(tmp_path: Path) -> None:
    manifest = tmp_path / "topology.json"
    summary_path = tmp_path / "summary.json"
    manifest.write_text(
        '{"schema":"one","schema":"two"}\n',
        encoding="utf-8",
    )

    assert (
        MODULE.main(
            [
                "--manifest",
                str(manifest),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary_path),
            ]
        )
        == 1
    )
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert summary["status"] == "blocked"
    assert summary["manifest_sha256"] is None
    assert summary["canonical_manifest_sha256"] is None
    assert summary["promotion_eligible"] is False


def test_cli_rejects_symlinked_manifest(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    manifest = tmp_path / "topology.json"
    write_json(target, valid_manifest())
    manifest.symlink_to(target)

    assert (
        MODULE.main(
            [
                "--manifest",
                str(manifest),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
            ]
        )
        == 1
    )


def test_canonical_lane_inventory_is_exactly_seventeen() -> None:
    assert len(MODULE.DEFAULT_REQUIRED_GATES) == MODULE.EXPECTED_LANE_SLOT_COUNT
    assert len(set(MODULE.DEFAULT_REQUIRED_GATES)) == 17
