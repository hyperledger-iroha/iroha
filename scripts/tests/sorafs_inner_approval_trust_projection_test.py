"""Exact independent signer projection controls; these claims confer no native authority."""
from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import sys

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_production_promotion_bundle.py"
SPEC = importlib.util.spec_from_file_location("sorafs_inner_trust_projection_checker", MODULE_PATH)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

ROLES = ("topology", "resilience", "foundational")
REVISION_FIELDS = ("signer_key_revision", "signer_policy_revision")
DIGEST = "11" * 32


def signer_coordinates(revision: int = 1) -> tuple[dict, dict]:
    """Return independent scalar trust and its exact public projection, not signed evidence."""
    trusted = {
        "public_key_hex": "22" * 32,
        "service_id": "topology-signer",
        "administrator_id": "independent-approver",
        "key_revision": revision,
        "policy_revision": revision,
        "policy_digest_sha256": DIGEST,
    }
    observed = {
        "signer_service_id": trusted["service_id"],
        "signer_administrator_id": trusted["administrator_id"],
        "signer_key_revision": revision,
        "signer_policy_revision": revision,
        "signer_policy_digest_sha256": DIGEST,
        "signer_public_key_fingerprint_sha256": hashlib.sha256(
            bytes.fromhex(trusted["public_key_hex"])
        ).hexdigest(),
    }
    return trusted, observed


def inspect_role(role: str, trusted: dict, observed: dict) -> list[str]:
    """Exercise each actual projection consumer with consistency-only local test claims."""
    runner = MODULE.promotion_runner
    if role == "topology":
        claim = dict.fromkeys(MODULE.topology_qualification.AUTHENTICATED_TOPOLOGY_BINDING_FIELDS)
        claim.update(observed, signer_authentication_kind="external-ed25519")
        return MODULE._topology_native_authority_errors(claim, claim, trusted)
    if role == "resilience":
        claim = dict.fromkeys(runner.RESILIENCE_QUALIFICATION_BINDING_FIELDS)
        claim.update(observed, schema=runner.RESILIENCE_QUALIFICATION_BINDING_SCHEMA,
                     summary_sha256=DIGEST)
        return MODULE._resilience_native_authority_errors(claim, claim, trusted, DIGEST)
    assert role == "foundational"
    lanes = {lane: DIGEST for lane in runner.DEFAULT_REQUIRED_GATES}
    summary = {
        **observed,
        "schema": runner.FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": True,
        "valid": True,
        "errors": [],
        "release_sequence": 1,
        "previous_envelope_sha256": DIGEST,
        "l1_lane_evidence_inventory_sha256": DIGEST,
        "topology_qualification": {},
        "resilience_qualification": {},
        "lane_summary_sha256": [{"gate": lane, "sha256": DIGEST} for lane in lanes],
    }
    bundle = dict.fromkeys(MODULE.FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_FIELDS)
    bundle.update(schema=MODULE.FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_SCHEMA,
                  verifier_sha256=DIGEST)
    return MODULE._foundational_native_authority_errors(
        summary, {"signer_receipt_bundle": bundle}, trusted, DIGEST,
        (60, 1, DIGEST), DIGEST, {}, {}, lanes,
    )


@pytest.mark.parametrize("role", ROLES)
@pytest.mark.parametrize("revision", [1, 11, (1 << 63) - 1])
def test_exact_integer_trust_coordinates_keep_native_authority_blocked(role: str, revision: int) -> None:
    trusted, observed = signer_coordinates(revision)
    assert inspect_role(role, trusted, observed) == [
        getattr(MODULE, f"{role.upper()}_NATIVE_AUTHORITY_BLOCKER")
    ]


@pytest.mark.parametrize("role", ROLES)
@pytest.mark.parametrize("field", REVISION_FIELDS)
@pytest.mark.parametrize("substitute", [True, 1.0], ids=["boolean-alias", "float-alias"])
def test_equal_python_numeric_alias_is_not_independent_integer_trust(
    role: str, field: str, substitute: object,
) -> None:
    trusted, observed = signer_coordinates()
    assert substitute == observed[field] and type(substitute) is not int
    observed[field] = substitute
    assert inspect_role(role, trusted, observed) == [
        f"inner approval {role} {field} must match independent trust",
        getattr(MODULE, f"{role.upper()}_NATIVE_AUTHORITY_BLOCKER"),
    ]


@pytest.mark.parametrize("role", ROLES)
@pytest.mark.parametrize("field", REVISION_FIELDS)
@pytest.mark.parametrize("substitute", ["1", None, 2], ids=["string", "absent", "different-integer"])
def test_other_revision_substitutions_remain_rejected(role: str, field: str, substitute: object) -> None:
    trusted, observed = signer_coordinates()
    observed[field] = substitute
    assert inspect_role(role, trusted, observed) == [
        f"inner approval {role} {field} must match independent trust",
        getattr(MODULE, f"{role.upper()}_NATIVE_AUTHORITY_BLOCKER"),
    ]
