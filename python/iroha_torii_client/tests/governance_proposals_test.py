"""Closed first-release governance proposal reader tests."""

from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest
from client_test_support import CANONICAL_OWNER
from iroha_torii_client.governance_kagemusha_release_schema_v1 import SCHEMAS_V1, validate_release_schema_v1
from iroha_torii_client.governance_proposals import (
    GovernanceContractLifecycleActionKind,
    GovernanceContractLifecycleEmergencyHoldRetrospective,
    GovernanceGlobalDataTriggerPermissionAction,
    GovernanceProposalContractEmergencyHold,
    GovernanceProposalContractLifecycleGovernance,
    GovernanceProposalDeployContract,
    GovernanceProposalGlobalDataTriggerPermissionGovernance,
    GovernanceProposalKagemushaVerifierPolicyInstall,
    GovernanceProposalKagemushaVerifierReleaseActivate,
    GovernanceProposalKagemushaVerifierReleaseInstall,
    GovernanceProposalKind,
    GovernanceProposalKindTag,
    GovernanceProposalMusubiRegistryGovernance,
    GovernanceProposalRecord,
    GovernanceProposalRuntimeUpgrade,
    GovernanceProposalSccpRouteGovernance,
    GovernanceProposalSorafsProviderGovernance,
    GovernanceProposalValidationFeePayoutLifecycle,
    GovernanceProposalValidationFeePolicy,
)

CONTRACT_ADDRESS = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"
KAGEMUSHA_SIGNER = (
    "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
)
KAGEMUSHA_SIGNER_B = (
    "ed012017CB79FB2B4120F2B1EC65E4198D6E08B28E813FEB01E4A400839B85E18080CE"
)
OTHER_PROPOSER = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"


def _kagemusha_multihash(code: int, payload: bytes) -> str:
    def varint(value: int) -> bytes:
        encoded = bytearray()
        while True:
            part = value & 0x7F
            value >>= 7
            encoded.append(part | (0x80 if value else 0))
            if value == 0:
                return bytes(encoded)

    return (varint(code) + varint(len(payload))).hex() + payload.hex().upper()


def _release_install_fixture() -> dict[str, object]:
    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "governance"
        / "kagemusha_verifier_release_install_v1.json"
    )
    return json.loads(path.read_text(encoding="utf-8"))


def _release_activate_fixture() -> dict[str, object]:
    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "governance"
        / "kagemusha_verifier_release_activate_v1.json"
    )
    return json.loads(path.read_text(encoding="utf-8"))


def _bsc_network() -> dict[str, object]:
    return {"network": "bsc_mainnet", "profile": None}


def _payout_binding() -> dict[str, object]:
    return {
        "contract_address": CONTRACT_ADDRESS,
        "code_hash": [17] * 32,
        "entrypoint": "autonomous_validation_fee_tick",
        "treasury_account_id": CANONICAL_OWNER,
        "ds_asset_id": "xor#wonderland",
        "xor_asset_id": "xor#sora",
        "pool_vault_account_id": CANONICAL_OWNER,
        "batch_ds": "10",
        "min_xor_out": "4",
        "max_xor_out": "100",
        "recipients": [
            {"account_id": CANONICAL_OWNER, "share": "0.25"} for _ in range(4)
        ],
    }


def _policy() -> dict[str, object]:
    return {
        "schema_version": 1,
        "network_id": NETWORK_ID,
        "policy_version": "1",
        "previous_policy_hash": None,
        "ds_asset_id": "xor#wonderland",
        "ds_scale": 0,
        "fee": "0",
        "treasury_account_id": CANONICAL_OWNER,
        "charging_mode": {"charging_mode": "DISABLED", "value": None},
        "effective_from_height": "1",
        "expires_after_height": None,
        "exemption_classes": [],
        "treasury_payout_binding": None,
    }


def _variants() -> list[tuple[str, dict[str, object], type[object]]]:
    return [
        (
            "DeployContract",
            {
                "contract_address": CONTRACT_ADDRESS,
                "code_hash": "11" * 32,
                "abi_hash": "22" * 32,
                "abi_version": 1,
                "manifest_provenance": None,
            },
            GovernanceProposalDeployContract,
        ),
        (
            "RuntimeUpgrade",
            {
                "manifest": {
                    "name": "runtime-v1",
                    "description": "first release",
                    "abi_version": 1,
                    "abi_hash": [34] * 32,
                    "added_syscalls": [],
                    "added_pointer_types": [],
                    "start_height": 10,
                    "end_height": 20,
                    "sbom_digests": [],
                    "slsa_attestation": "",
                    "provenance": [],
                }
            },
            GovernanceProposalRuntimeUpgrade,
        ),
        (
            "SccpRouteGovernance",
            {
                "proposal": {
                    "network_id": NETWORK_ID,
                    "base_revisions": [
                        {
                            "subject": {"subject": "route", "key": _bsc_network()},
                            "revision": 1,
                        }
                    ],
                    "actions": [
                        {
                            "action": "remove_staged",
                            "payload": {"network": _bsc_network(), "revision": 1},
                        }
                    ],
                }
            },
            GovernanceProposalSccpRouteGovernance,
        ),
        (
            "ValidationFeePolicy",
            {
                "proposal_operator": CANONICAL_OWNER,
                "policy": _policy(),
                "payout_lifecycle_proposal_id": None,
            },
            GovernanceProposalValidationFeePolicy,
        ),
        (
            "ValidationFeePayoutLifecycle",
            {
                "proposal_operator": CANONICAL_OWNER,
                "payout_binding": _payout_binding(),
            },
            GovernanceProposalValidationFeePayoutLifecycle,
        ),
        (
            "MusubiRegistryGovernance",
            {
                "kind": "RetargetAlias",
                "value": {
                    "alias": ["wallet"],
                    "target": {
                        "home_dataspace": 1,
                        "scope": {"kind": "DataspaceRoot", "value": None},
                        "name": ["wallet"],
                    },
                    "expected_revision": 1,
                },
            },
            GovernanceProposalMusubiRegistryGovernance,
        ),
        (
            "SorafsProviderGovernance",
            {
                "action": {
                    "action": "establish",
                    "value": {"provider_id": [[51] * 32], "owner": CANONICAL_OWNER},
                }
            },
            GovernanceProposalSorafsProviderGovernance,
        ),
        (
            "ContractLifecycleGovernance",
            {
                "contract_address": CONTRACT_ADDRESS,
                "expected_revision": 3,
                "action": {
                    "action": "CompleteEmergencyHoldRetrospective",
                    "payload": {
                        "hold_proposal_content_id": [81] * 32,
                        "hold_governance_attempt_id": [82] * 32,
                        "incident_digest": [83] * 32,
                        "retrospective_finding_root": [84] * 32,
                    },
                },
            },
            GovernanceProposalContractLifecycleGovernance,
        ),
        (
            "ContractEmergencyHold",
            {
                "contract_address": CONTRACT_ADDRESS,
                "expected_revision": 2,
                "expected_code_hash": "33" * 32,
                "incident_digest": [85] * 32,
                "reason": "contain active exploit",
                "duration_blocks": 3_600,
            },
            GovernanceProposalContractEmergencyHold,
        ),
        (
            "GlobalDataTriggerPermissionGovernance",
            {
                "authority": CANONICAL_OWNER,
                "action": {"action": "grant", "value": None},
            },
            GovernanceProposalGlobalDataTriggerPermissionGovernance,
        ),
        (
            "KagemushaVerifierPolicyInstall",
            {
                "proposal_operator": CANONICAL_OWNER,
                "network_id": NETWORK_ID,
                "expected_predecessor": {
                    "version": 1,
                    "authority_policy": None,
                    "active_release_id": None,
                    "releases": [],
                },
                "authority_policy": {
                    "version": 1,
                    "authority_set_id": [0x40] * 32,
                    "threshold": 1,
                    "authorized_signers": [KAGEMUSHA_SIGNER],
                },
            },
            GovernanceProposalKagemushaVerifierPolicyInstall,
        ),
        (
            "KagemushaVerifierReleaseInstall",
            _release_install_fixture()["payload"],
            GovernanceProposalKagemushaVerifierReleaseInstall,
        ),
        (
            "KagemushaVerifierReleaseActivate",
            _release_activate_fixture()["payload"],
            GovernanceProposalKagemushaVerifierReleaseActivate,
        ),
    ]


def test_shared_fixture_pins_closed_proposal_and_lifecycle_action_inventories() -> None:
    fixture_path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "governance"
        / "parliament_api_v1.json"
    )
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))

    assert fixture["proposal_kinds"] == [kind.value for kind in GovernanceProposalKindTag]
    assert fixture["contract_lifecycle_actions"] == [
        action.value for action in GovernanceContractLifecycleActionKind
    ]


def test_contract_lifecycle_action_inventory_admits_exactly_six_wire_tags() -> None:
    action_payloads: dict[GovernanceContractLifecycleActionKind, object] = {
        GovernanceContractLifecycleActionKind.ACTIVATE: {
            "code_hash": "11" * 32,
            "abi_hash": "22" * 32,
            "abi_version": 1,
            "manifest_provenance": None,
        },
        GovernanceContractLifecycleActionKind.DEACTIVATE: {
            "expected_code_hash": "33" * 32,
        },
        GovernanceContractLifecycleActionKind.OFFER_OWNERSHIP: {
            "new_owner": CANONICAL_OWNER,
        },
        GovernanceContractLifecycleActionKind.CANCEL_OWNERSHIP_OFFER: None,
        GovernanceContractLifecycleActionKind.ACCEPT_PARLIAMENT_OWNERSHIP: None,
        GovernanceContractLifecycleActionKind.COMPLETE_EMERGENCY_HOLD_RETROSPECTIVE: {
            "hold_proposal_content_id": [0x42] * 32,
            "hold_governance_attempt_id": [0x43] * 32,
            "incident_digest": [0x44] * 32,
            "retrospective_finding_root": [0x45] * 32,
        },
    }
    assert list(action_payloads) == list(GovernanceContractLifecycleActionKind)
    for action, payload in action_payloads.items():
        lifecycle = copy.deepcopy(_variants()[7][1])
        lifecycle["action"] = {"action": action.value, "payload": payload}
        proposal = GovernanceProposalKind.from_payload(
            {"kind": "ContractLifecycleGovernance", "payload": lifecycle}
        )
        assert proposal.payload.action.action is action  # type: ignore[union-attr]

    lifecycle = copy.deepcopy(_variants()[7][1])
    lifecycle["action"] = {"action": "Unknown", "payload": None}
    with pytest.raises(TypeError, match="not a first-release lifecycle action"):
        GovernanceProposalKind.from_payload(
            {"kind": "ContractLifecycleGovernance", "payload": lifecycle}
        )


@pytest.mark.parametrize(("tag", "payload", "payload_type"), _variants())
def test_proposal_kind_accepts_each_closed_v1_variant(
    tag: str, payload: dict[str, object], payload_type: type[object]
) -> None:
    proposal = GovernanceProposalKind.from_payload({"kind": tag, "payload": payload})

    assert proposal.kind is GovernanceProposalKindTag(tag)
    assert isinstance(proposal.payload, payload_type)


@pytest.mark.parametrize(
    ("path", "value", "error"),
    [
        (("expected_predecessor", "releases"), [{}], "exact empty V1 verifier registry"),
        (("expected_predecessor", "authority_policy"), {}, "exact empty V1 verifier registry"),
        (("authority_policy", "authority_set_id"), [0] * 32, "non-zero"),
        (("authority_policy", "threshold"), 0, "integer in 1"),
        (("authority_policy", "threshold"), 2, "must not exceed signer count"),
        (("authority_policy", "authorized_signers"), [], "1..32 keys"),
        (("authority_policy", "authorized_signers"), [KAGEMUSHA_SIGNER] * 2, "strictly ordered"),
        (("authority_policy", "authorized_signers"), [KAGEMUSHA_SIGNER_B, KAGEMUSHA_SIGNER], "strictly ordered"),
        (("authority_policy", "authorized_signers"), [KAGEMUSHA_SIGNER.lower()], "canonical public-key multihash"),
        (("authority_policy", "authorized_signers"), ["ed0120" + "00" * 32], "prime-order Ed25519"),
    ],
)
def test_kagemusha_policy_install_rejects_invalid_initial_policy(
    path: tuple[str, str], value: object, error: str
) -> None:
    payload = copy.deepcopy(_variants()[10][1])
    payload[path[0]][path[1]] = value  # type: ignore[index]
    with pytest.raises(TypeError, match=error):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierPolicyInstall", "payload": payload}
        )


def test_kagemusha_policy_install_rejects_unknown_and_missing_fields() -> None:
    payload = copy.deepcopy(_variants()[10][1])
    payload["authority_policy"]["legacy"] = True  # type: ignore[index]
    with pytest.raises(TypeError, match="unknown field `legacy`"):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierPolicyInstall", "payload": payload}
        )
    del payload["authority_policy"]["legacy"]  # type: ignore[index]
    del payload["proposal_operator"]
    with pytest.raises(TypeError, match="missing required field `proposal_operator`"):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierPolicyInstall", "payload": payload}
        )


def test_kagemusha_policy_install_read_binds_operator_to_retained_proposer() -> None:
    record = {
        "proposer": CANONICAL_OWNER,
        "kind": {"kind": "KagemushaVerifierPolicyInstall", "payload": _variants()[10][1]},
        "created_height": 1,
        "status": "Proposed",
    }
    assert isinstance(
        GovernanceProposalRecord.from_payload(record).kind.payload,
        GovernanceProposalKagemushaVerifierPolicyInstall,
    )
    record["proposer"] = OTHER_PROPOSER
    with pytest.raises(TypeError, match="operator must match the retained proposer"):
        GovernanceProposalRecord.from_payload(record)


def test_kagemusha_release_schema_matches_the_canonical_openapi_closure() -> None:
    root = Path(__file__).resolve().parents[3]
    schemas = json.loads(
        (root / "artifacts" / "openapi" / "torii.json").read_text(encoding="utf-8")
    )["components"]["schemas"]
    roots = (
        "GovernanceKagemushaGovernedVerifierRegistryV1",
        "GovernanceKagemushaReleaseManifestV1",
        "GovernanceKagemushaInternalValidationReceiptV1",
        "GovernanceKagemushaReleaseAttestationV1",
    )
    closure: set[str] = set()

    def visit(node: object) -> None:
        if isinstance(node, dict):
            reference = node.get("$ref")
            if isinstance(reference, str) and reference.startswith("#/components/schemas/"):
                name = reference.rsplit("/", 1)[-1]
                if name not in closure:
                    closure.add(name)
                    visit(schemas[name])
            for child in node.values():
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)

    def project(node: object) -> object:
        if isinstance(node, dict):
            return {
                key: project(child)
                for key, child in node.items()
                if key not in {"description", "title", "example"}
            }
        if isinstance(node, list):
            return [project(child) for child in node]
        return node

    for name in roots:
        closure.add(name)
        visit(schemas[name])
    assert set(SCHEMAS_V1) == closure
    assert SCHEMAS_V1 == {name: project(schemas[name]) for name in sorted(closure)}


def test_kagemusha_release_install_accepts_exact_fixture_and_freezes_nested_records() -> None:
    fixture = _release_install_fixture()
    proposal = GovernanceProposalKind.from_payload(fixture)
    assert isinstance(proposal.payload, GovernanceProposalKagemushaVerifierReleaseInstall)
    assert proposal.payload.manifest["version"] == 1
    assert len(proposal.payload.manifest["artifacts"]) == 50
    with pytest.raises(TypeError):
        proposal.payload.manifest["version"] = 2  # type: ignore[index]


@pytest.mark.parametrize(
    ("mutation", "error"),
    [
        (lambda p: p.pop("proposal_operator"), "missing required field"),
        (lambda p: p["manifest"].update({"retired": True}), "unknown field"),
        (lambda p: p["manifest"].update({"version": 2}), "wrong V1 constant"),
        (lambda p: p["manifest"].update({"release_id": [1] * 31}), "invalid array length"),
        (lambda p: p["receipt"].update({"fuzz_cases": 1 << 53}), "integer range"),
        (lambda p: p["expected_predecessor"].update({"authority_policy": None}), "governed signer policy"),
        (lambda p: p["attestation"]["subject"].update({"release_id": [2] * 32}), "must match manifest"),
        (lambda p: p["manifest"]["enabled_profiles"][0]["hardware_profile"].update({"capability_mask": True}), "must be an integer"),
        (lambda p: p["receipt"]["profile_qualifications"][0]["thermal"].update({"retired": 0}), "unknown field"),
    ],
)
def test_kagemusha_release_install_rejects_malformed_nested_json(
    mutation: object, error: str
) -> None:
    payload = copy.deepcopy(_release_install_fixture()["payload"])
    mutation(payload)  # type: ignore[operator]
    with pytest.raises(TypeError, match=error):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierReleaseInstall", "payload": payload}
        )


def test_kagemusha_release_activate_accepts_exact_fixture_and_freezes_predecessor() -> None:
    proposal = GovernanceProposalKind.from_payload(_release_activate_fixture())
    assert isinstance(proposal.payload, GovernanceProposalKagemushaVerifierReleaseActivate)
    assert len(proposal.payload.expected_predecessor["releases"]) == 1
    assert len(proposal.payload.successor_release_id) == 32
    with pytest.raises(TypeError):
        proposal.payload.expected_predecessor["version"] = 2  # type: ignore[index]


@pytest.mark.parametrize(
    ("mutation", "error"),
    [
        (lambda p: p.pop("successor_release_id"), "missing required field"),
        (lambda p: p.update({"retired": True}), "unknown field"),
        (lambda p: p.update({"successor_release_id": [1] * 31}), "invalid array length"),
        (lambda p: p["expected_predecessor"].update({"active_release_id": "AA" * 32}), "must be inactive"),
        (lambda p: p["expected_predecessor"].update({"releases": []}), "exactly one standby"),
        (lambda p: p["expected_predecessor"]["releases"].append(copy.deepcopy(p["expected_predecessor"]["releases"][0])), "exactly one standby"),
        (lambda p: p["expected_predecessor"]["releases"][0].update({"status": 1}), "sole standby"),
        (lambda p: p.update({"successor_release_id": [2] * 32}), "sole standby"),
    ],
)
def test_kagemusha_release_activate_rejects_invalid_first_transition(
    mutation: object, error: str
) -> None:
    payload = copy.deepcopy(_release_activate_fixture()["payload"])
    mutation(payload)  # type: ignore[operator]
    with pytest.raises(TypeError, match=error):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierReleaseActivate", "payload": payload}
        )


@pytest.mark.parametrize(
    ("code", "expected_length"),
    [
        (0xE7, 33),
        (0xEA, 48),
        (0xEB, 96),
        (0xEE, 1952),
        (0x1200, 64),
        (0x1201, 64),
        (0x1202, 64),
        (0x1203, 128),
        (0x1204, 128),
    ],
)
def test_kagemusha_policy_install_rejects_wrong_signer_algorithm_lengths(
    code: int, expected_length: int
) -> None:
    payload = copy.deepcopy(_variants()[10][1])
    payload["authority_policy"]["authorized_signers"] = [  # type: ignore[index]
        _kagemusha_multihash(code, bytes([1]) * (expected_length - 1))
    ]
    with pytest.raises(TypeError, match="public-key payload length"):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierPolicyInstall", "payload": payload}
        )


@pytest.mark.parametrize(
    ("signer", "error"),
    [
        (_kagemusha_multihash(0xE7, bytes([4]) * 33), "secp256k1 public-key envelope"),
        (_kagemusha_multihash(0xEE, bytes(1952)), "all-zero ML-DSA public key"),
        (_kagemusha_multihash(0x1306, bytes([4]) * 65), "SM2 public-key payload"),
        (_kagemusha_multihash(0x1306, b"\x00\x02\xff\xff" + bytes([4]) * 65), "UTF-8 SM2 distinguished ID"),
        (_kagemusha_multihash(0x1306, b"\x00\x00" + bytes([2]) * 65), "SM2 public-key payload"),
    ],
)
def test_kagemusha_policy_install_rejects_bad_signer_envelopes(
    signer: str, error: str
) -> None:
    payload = copy.deepcopy(_variants()[10][1])
    payload["authority_policy"]["authorized_signers"] = [signer]  # type: ignore[index]
    with pytest.raises(TypeError, match=error):
        GovernanceProposalKind.from_payload(
            {"kind": "KagemushaVerifierPolicyInstall", "payload": payload}
        )


@pytest.mark.parametrize(
    "payload",
    [
        {"DeployContract": _variants()[0][1]},
        {"kind": "ApproveGovernanceProposal", "payload": {}},
        {"kind": "DeployContract", "payload": {**_variants()[0][1], "window": {}}},
        {"kind": "DeployContract", "payload": _variants()[0][1], "legacy": True},
    ],
)
def test_proposal_kind_rejects_unknown_and_retired_shapes(payload: object) -> None:
    with pytest.raises(TypeError):
        GovernanceProposalKind.from_payload(payload)


def test_attempt_proposal_u64_numbers_obey_the_exact_json_boundary() -> None:
    maximum = (1 << 53) - 1
    runtime = copy.deepcopy(_variants()[1][1])
    runtime["manifest"]["start_height"] = maximum - 1  # type: ignore[index]
    runtime["manifest"]["end_height"] = maximum  # type: ignore[index]
    GovernanceProposalKind.from_payload({"kind": "RuntimeUpgrade", "payload": runtime})

    runtime["manifest"]["end_height"] = maximum + 1  # type: ignore[index]
    with pytest.raises(TypeError, match="9007199254740991"):
        GovernanceProposalKind.from_payload(
            {"kind": "RuntimeUpgrade", "payload": runtime}
        )

    musubi = copy.deepcopy(_variants()[5][1])
    musubi["value"]["target"]["home_dataspace"] = maximum + 1  # type: ignore[index]
    with pytest.raises(TypeError, match="9007199254740991"):
        GovernanceProposalKind.from_payload(
            {"kind": "MusubiRegistryGovernance", "payload": musubi}
        )


def test_closed_nested_action_tags_reject_unknown_values() -> None:
    variants = _variants()
    musubi = copy.deepcopy(variants[5][1])
    musubi["kind"] = "LegacyRecovery"
    with pytest.raises(TypeError, match="Musubi action"):
        GovernanceProposalKind.from_payload({"kind": "MusubiRegistryGovernance", "payload": musubi})

    sorafs = copy.deepcopy(variants[6][1])
    sorafs["action"]["action"] = "replace"  # type: ignore[index]
    with pytest.raises(TypeError, match="provider action"):
        GovernanceProposalKind.from_payload({"kind": "SorafsProviderGovernance", "payload": sorafs})

    direct_provider_id = copy.deepcopy(variants[6][1])
    direct_provider_id["action"]["value"]["provider_id"] = [51] * 32  # type: ignore[index]
    with pytest.raises(TypeError, match="one-field ProviderId tuple"):
        GovernanceProposalKind.from_payload(
            {"kind": "SorafsProviderGovernance", "payload": direct_provider_id}
        )

    scalar_musubi_newtypes = copy.deepcopy(variants[5][1])
    scalar_musubi_newtypes["value"]["alias"] = "wallet"  # type: ignore[index]
    with pytest.raises(TypeError, match="one-field string tuple"):
        GovernanceProposalKind.from_payload(
            {"kind": "MusubiRegistryGovernance", "payload": scalar_musubi_newtypes}
        )

    scalar_package_name = copy.deepcopy(variants[5][1])
    scalar_package_name["value"]["target"]["name"] = "wallet"  # type: ignore[index]
    with pytest.raises(TypeError, match="one-field string tuple"):
        GovernanceProposalKind.from_payload(
            {"kind": "MusubiRegistryGovernance", "payload": scalar_package_name}
        )


def test_contract_lifecycle_retrospective_and_unit_actions_are_closed() -> None:
    lifecycle = copy.deepcopy(_variants()[7][1])
    proposal = GovernanceProposalKind.from_payload(
        {"kind": "ContractLifecycleGovernance", "payload": lifecycle}
    )
    assert proposal.payload.action.action is (  # type: ignore[union-attr]
        GovernanceContractLifecycleActionKind.COMPLETE_EMERGENCY_HOLD_RETROSPECTIVE
    )
    assert isinstance(  # type: ignore[union-attr]
        proposal.payload.action.payload,
        GovernanceContractLifecycleEmergencyHoldRetrospective,
    )

    for action in ("CancelOwnershipOffer", "AcceptParliamentOwnership"):
        lifecycle["action"] = {"action": action, "payload": None}
        GovernanceProposalKind.from_payload(
            {"kind": "ContractLifecycleGovernance", "payload": lifecycle}
        )
        del lifecycle["action"]["payload"]  # type: ignore[index]
        with pytest.raises(TypeError, match="missing required field `payload`"):
            GovernanceProposalKind.from_payload(
                {"kind": "ContractLifecycleGovernance", "payload": lifecycle}
            )


def test_contract_hold_bounds_and_retrospective_root_fail_closed() -> None:
    lifecycle = copy.deepcopy(_variants()[7][1])
    lifecycle["action"]["payload"]["retrospective_finding_root"] = [0] * 32  # type: ignore[index]
    with pytest.raises(TypeError, match="must be non-zero"):
        GovernanceProposalKind.from_payload(
            {"kind": "ContractLifecycleGovernance", "payload": lifecycle}
        )

    emergency = copy.deepcopy(_variants()[8][1])
    emergency["duration_blocks"] = 3_601
    with pytest.raises(TypeError, match=r"1\.\.3600"):
        GovernanceProposalKind.from_payload(
            {"kind": "ContractEmergencyHold", "payload": emergency}
        )


def test_global_data_trigger_permission_is_exact_account_and_closed_action() -> None:
    proposal = GovernanceProposalKind.from_payload(
        {
            "kind": "GlobalDataTriggerPermissionGovernance",
            "payload": _variants()[9][1],
        }
    )
    assert proposal.payload.action is GovernanceGlobalDataTriggerPermissionAction.GRANT  # type: ignore[union-attr]

    malformed = copy.deepcopy(_variants()[9][1])
    malformed["action"]["value"] = {}  # type: ignore[index]
    with pytest.raises(TypeError, match="must be null"):
        GovernanceProposalKind.from_payload(
            {"kind": "GlobalDataTriggerPermissionGovernance", "payload": malformed}
        )


def test_sccp_route_governance_keeps_the_exact_proposal_object() -> None:
    payload = _variants()[2][1]
    proposal = GovernanceProposalKind.from_payload(
        {"kind": "SccpRouteGovernance", "payload": payload}
    ).payload
    assert isinstance(proposal, GovernanceProposalSccpRouteGovernance)
    assert proposal.proposal["network_id"] == NETWORK_ID
    assert proposal.proposal["actions"][0]["action"] == "remove_staged"

    anchor = {"anchor": {"network_id": NETWORK_ID, "action": {}}}
    with pytest.raises(TypeError, match="missing required field `proposal`"):
        GovernanceProposalSccpRouteGovernance.from_payload(anchor)

    extra = copy.deepcopy(payload)
    extra["proposal"]["expected_head"] = None  # type: ignore[index]
    with pytest.raises(TypeError, match="unknown field `expected_head`"):
        GovernanceProposalSccpRouteGovernance.from_payload(extra)

    for actions in ([], [payload["proposal"]["actions"][0]] * 17):  # type: ignore[index]
        bounded = copy.deepcopy(payload)
        bounded["proposal"]["actions"] = actions  # type: ignore[index]
        with pytest.raises(TypeError, match=r"1\.\.16 actions"):
            GovernanceProposalSccpRouteGovernance.from_payload(bounded)

    malformed_network = copy.deepcopy(payload)
    malformed_network["proposal"]["network_id"] = "chain"  # type: ignore[index]
    with pytest.raises(TypeError, match="proposal.network_id must use canonical"):
        GovernanceProposalSccpRouteGovernance.from_payload(malformed_network)


def test_proposal_record_is_exact_and_rejects_retired_wrapper_fields() -> None:
    record = {
        "proposer": CANONICAL_OWNER,
        "kind": {"kind": _variants()[0][0], "payload": _variants()[0][1]},
        "created_height": 7,
        "status": "Superseded",
    }
    parsed = GovernanceProposalRecord.from_payload(record)
    assert parsed.created_height == 7

    for old_field in ("pipeline", "parliament_snapshot", "finalization_evidence"):
        with pytest.raises(TypeError, match="unknown field"):
            GovernanceProposalRecord.from_payload({**record, old_field: None})


@pytest.mark.parametrize(
    "status",
    ["Proposed", "Rejected", "Enacted", "Superseded", "ExecutionFailed"],
)
def test_proposal_record_accepts_only_first_release_statuses(status: str) -> None:
    record = {
        "proposer": CANONICAL_OWNER,
        "kind": {"kind": _variants()[0][0], "payload": _variants()[0][1]},
        "created_height": 7,
        "status": status,
    }

    assert GovernanceProposalRecord.from_payload(record).status.value == status

    record["status"] = "Approved"
    with pytest.raises(TypeError, match="status is unsupported"):
        GovernanceProposalRecord.from_payload(record)


def test_kagemusha_release_schema_requires_network_and_closed_purpose() -> None:
    manifest = _release_install_fixture()["payload"]["manifest"]
    schema = "GovernanceKagemushaReleaseManifestV1"
    validate_release_schema_v1(schema, manifest)
    for field in ("network_id", "purpose"):
        missing = copy.deepcopy(manifest)
        del missing[field]
        with pytest.raises(TypeError, match="missing required"):
            validate_release_schema_v1(schema, missing)
    scope = {
        "asset_identity_digest": [1] * 32,
        "asset_incarnation": [2] * 32,
        "asset_scale": 28,
        "liability_pool_id": [3] * 32,
    }
    experiment = {**manifest, "purpose": {"kind": "testnet_experiment", "value": scope}}
    validate_release_schema_v1(schema, experiment)
    for purpose in (
        {"kind": "production"},
        {"kind": "production", "value": {}},
        {"kind": "unknown", "value": None},
        {"kind": "testnet_experiment", "value": None},
        {"kind": "testnet_experiment", "value": {**scope, "asset_scale": 29}},
        {"kind": "testnet_experiment", "value": {**scope, "retired": None}},
    ):
        with pytest.raises(TypeError):
            validate_release_schema_v1(schema, {**manifest, "purpose": purpose})
