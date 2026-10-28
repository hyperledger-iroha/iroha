"""SCCP static proposal rules at the public Python governance reader boundary."""

from __future__ import annotations

import base64
import copy
import json
from collections.abc import Mapping
from pathlib import Path

import pytest
from client_test_support import CANONICAL_OWNER, canonical_hash
from iroha_torii_client import ToriiClient
from iroha_torii_client.governance_proposals import GovernanceProposalKind
from sumeragi_exact_json_test_support import RecordingSession

NETWORKS = ("ethereum_mainnet", "bsc_mainnet", "ton_mainnet")
SAFE_MAX = (1 << 53) - 1
# Expected subjects follow the Rust SccpGovernanceActionV1::subject inventory.
SUBJECTS = {
    "register_route": "route",
    "activate_revision": "route",
    "switch_revision": "route",
    "deactivate_outbound": "route",
    "retire_revision": "route",
    "remove_staged": "route",
    "release_stranded": "route",
    "set_taira_paused": "route",
    "set_destination_paused": "route_control",
    "initialize_light_client": "light_client",
    "install_trusted_checkpoint": "light_client",
    "freeze_light_client": "light_client",
    "set_parameters": "parameters",
    "activate_light_client_profile": "light_client",
    "attest_deployment_progress": "forgery_hold",
    "quarantine_revision": "forgery_hold",
    "open_reconciliation": "forgery_hold",
    "set_fast_pause_suspended": "fast_pause_control",
}


def network(name="bsc_mainnet"):
    return {"network": name, "profile": None}


def subject(tag="route", name="bsc_mainnet"):
    return {"subject": tag, "key": None if tag == "parameters" else network(name)}


def parameters():
    """The canonical Rust taira_default(), without live staking/attempt context."""
    day = 86_400_000
    return {
        "enabled": True,
        "committee_validity_ms": 14 * day,
        "committee_heartbeat_ms": day,
        "forgery_evidence_grace_ms": 7 * day,
        "destination_progress_cap_ms": 30 * day,
        "outbound_ttl_ms": 7 * day,
        "min_outbound_amount": "1000000000",
        "inbound_self_claim_fee": "10000000",
        "max_exempt_transactions_per_block": 32,
        "fast_pause_panel_seats": 9,
        "fast_pause_panel_term_ms": 7 * day,
        "fast_pause_seat_accept_window_ms": 43_200_000,
        "fast_pause_endorse_window_ms": 1_800_000,
        "fast_pause_renew_lead_ms": day,
        "fast_pause_hold_ms": 10 * day,
    }


def action_payload(action, name="bsc_mainnet"):
    key = {"network": network(name)}
    if action == "register_route":
        deployment = {
            "family": "evm",
            "deployment": {
                "address": [1] * 20,
                "runtime_code_hash": [0] * 32,
            },
        }
        if name == "ton_mainnet":
            deployment = {
                "family": "ton",
                "deployment": {
                    "master_account": [1] * 32,
                    **{
                        field: {"hash": [0] * 32, "depth": 0}
                        for field in ("minter_code", "wallet_code", "bucket_code")
                    },
                },
            }
        return {
            **key,
            "revision": 1,
            "deployment": deployment,
            "initial_committee": {
                "generation": 1,
                "committee_root": [1] * 32,
                "start_height": 1,
                "until_ms": 1,
            },
            "max_wrapped_supply": "1",
        }
    if action == "initialize_light_client":
        return {
            **key,
            "expected": {"expected": "absent", "detail": None},
            "params": {
                **key,
                "ws_bound_ms": 0 if name == "ton_mainnet" else 1,
                "checkpoint_stride": 0 if name == "ton_mainnet" else 1,
                "set_retention_ms": 1,
                "checkpoint_prune_after_ms": 1,
                **{
                    field: 1
                    for field in (
                        "max_updates_per_advance",
                        "max_segment_headers",
                        "max_ancestry_headers",
                        "max_backfill_headers",
                        "max_advance_bytes",
                        "max_proof_bytes",
                    )
                },
            },
            "bootstrap": {**key, "bytes": "AA=="},
        }
    if action in (
        "activate_revision",
        "deactivate_outbound",
        "retire_revision",
        "remove_staged",
        "quarantine_revision",
    ):
        return {**key, "revision": 1}
    if action == "switch_revision":
        return {**key, "from": 1, "to": 2}
    if action == "release_stranded":
        return {**key, "amount": "1", "recipient": CANONICAL_OWNER, "memo": ""}
    if action == "set_taira_paused":
        return {**key, "paused": False}
    if action == "set_destination_paused":
        return {**key, "revision": 1, "paused": True}
    if action == "install_trusted_checkpoint":
        return {
            **key,
            "checkpoint": {
                "source_height": 0,
                "block_hash": [0] * 32,
                "state_root": None,
                "receipts_or_tx_root": [0] * 32,
                "source_time_ms": 0,
            },
        }
    if action == "freeze_light_client":
        return key
    if action == "set_parameters":
        return {"next": parameters()}
    if action == "activate_light_client_profile":
        return {**key, "version": 2, "profile_hash": [1] * 32}
    if action == "attest_deployment_progress":
        return {**key, "revision": 1, "at": {"at": "frozen", "detail": None}}
    if action == "open_reconciliation":
        return {**key, "revision": 1, "window_ms": 1}
    if action == "set_fast_pause_suspended":
        return {**key, "duration_ms": 0}
    raise AssertionError(action)


def fixture(action="remove_staged", name="bsc_mainnet"):
    return {
        "kind": "SccpRouteGovernance",
        "payload": {
            "proposal": {
                "network_id": canonical_hash(0xA5),
                "base_revisions": [{"subject": subject(SUBJECTS[action], name), "revision": 0}],
                "actions": [{"action": action, "payload": action_payload(action, name)}],
            }
        },
    }


def thaw(value):
    if isinstance(value, Mapping):
        return {key: thaw(item) for key, item in value.items()}
    if isinstance(value, tuple):
        return [thaw(item) for item in value]
    return value


def read(value):
    return GovernanceProposalKind.from_payload(value).payload.proposal


@pytest.mark.parametrize("action", [a for a in SUBJECTS if a != "release_stranded"])
def test_all_non_account_actions_require_exact_fields_and_subjects(action):
    value = fixture(action)
    assert thaw(read(value)) == value["payload"]["proposal"]
    body = value["payload"]["proposal"]["actions"][0]["payload"]
    for field in list(body) + ["unexpected"]:
        invalid = copy.deepcopy(value)
        target = invalid["payload"]["proposal"]["actions"][0]["payload"]
        if field == "unexpected":
            target[field] = None
        else:
            del target[field]
        with pytest.raises(TypeError):
            read(invalid)
    value["payload"]["proposal"]["base_revisions"][0]["subject"] = subject("fast_hold")
    with pytest.raises(TypeError, match="subjects"):
        read(value)


@pytest.mark.parametrize(
    "path, replacement",
    [
        (("actions", 0, "action"), "unknown"),
        (("actions", 0, "action"), []),
        (("actions", 0, "payload"), None),
        (("actions", 0, "payload", "network", "network"), "sora_taira"),
        (("actions", 0, "payload", "network", "profile"), {}),
        (("base_revisions",), []),
        (("base_revisions", 0, "subject", "subject"), "unknown"),
        (("base_revisions", 0, "subject", "key", "network"), []),
        *[(("base_revisions", 0, "revision"), v) for v in (-1, True, 1.0, "1", SAFE_MAX + 1)],
    ],
)
def test_rejects_malformed_envelopes(path, replacement):
    value = fixture()
    target = value["payload"]["proposal"]
    for part in path[:-1]:
        target = target[part]
    target[path[-1]] = replacement
    with pytest.raises(TypeError):
        read(value)


def test_subject_order_repetition_limits_and_immutable_snapshot():
    value = fixture()
    proposal = value["payload"]["proposal"]
    proposal["actions"] = [
        {"action": action, "payload": action_payload(action, name)}
        for action, name in (
            ("freeze_light_client", "ton_mainnet"),
            ("remove_staged", "bsc_mainnet"),
            ("set_taira_paused", "ethereum_mainnet"),
            ("remove_staged", "bsc_mainnet"),
        )
    ]
    proposal["base_revisions"] = [
        {"subject": subject(tag, name), "revision": revision}
        for tag, name, revision in (
            ("route", "ethereum_mainnet", 0),
            ("route", "bsc_mainnet", SAFE_MAX),
            ("light_client", "ton_mainnet", 1),
        )
    ]
    frozen = read(value)
    assert thaw(frozen) == proposal
    proposal["base_revisions"].reverse()
    with pytest.raises(TypeError, match="subjects"):
        read(value)
    assert frozen["base_revisions"][0]["revision"] == 0
    value = fixture()
    proposal = value["payload"]["proposal"]
    proposal["actions"] *= 16
    read(value)
    proposal["actions"].append(proposal["actions"][0])
    with pytest.raises(TypeError):
        read(value)
    proposal["actions"].pop()
    proposal["base_revisions"] *= 2
    with pytest.raises(TypeError, match="subjects"):
        read(value)


def test_draft_rejects_invalid_sccp_before_authentication_or_transport():
    session = RecordingSession()
    client = ToriiClient("https://torii.invalid", session=session)
    value = fixture()
    value["payload"]["proposal"]["actions"][0]["payload"]["revision"] = 0
    with pytest.raises(TypeError, match="revision"):
        client.draft_parliament_attempt_v1(
            value,
            0,
            expected_proposal_content_id="11" * 32,
            expected_governance_attempt_id="22" * 32,
            canonical_auth=None,
        )
    assert session.calls == []


@pytest.mark.parametrize(
    "action, name, path",
    [
        ("register_route", "bsc_mainnet", ("deployment",)),
        ("register_route", "bsc_mainnet", ("deployment", "deployment")),
        ("register_route", "bsc_mainnet", ("initial_committee",)),
        ("register_route", "ton_mainnet", ("deployment", "deployment")),
        *[
            ("register_route", "ton_mainnet", ("deployment", "deployment", field))
            for field in ("minter_code", "wallet_code", "bucket_code")
        ],
        *[
            ("initialize_light_client", "bsc_mainnet", (field,))
            for field in ("expected", "params", "bootstrap")
        ],
        ("install_trusted_checkpoint", "bsc_mainnet", ("checkpoint",)),
        ("attest_deployment_progress", "bsc_mainnet", ("at",)),
        ("set_parameters", "bsc_mainnet", ("next",)),
    ],
)
def test_nested_records_require_exact_fields(action, name, path):
    value = fixture(action, name)
    read(value)
    target = value["payload"]["proposal"]["actions"][0]["payload"]
    for part in path:
        target = target[part]
    for field in list(target):
        original = target.pop(field)
        with pytest.raises(TypeError):
            read(value)
        target[field] = original
    target["unexpected"] = None
    with pytest.raises(TypeError):
        read(value)


def test_sixteen_distinct_subjects_follow_rust_declaration_order():
    value = fixture()
    proposal = value["payload"]["proposal"]
    proposal["actions"] = []
    proposal["base_revisions"] = []
    for action in (
        "set_taira_paused",
        "set_destination_paused",
        "freeze_light_client",
        "set_parameters",
        "quarantine_revision",
        "set_fast_pause_suspended",
    ):
        for name in ("bsc_mainnet",) if action == "set_parameters" else NETWORKS:
            proposal["actions"].append({"action": action, "payload": action_payload(action, name)})
            proposal["base_revisions"].append(
                {"subject": subject(SUBJECTS[action], name), "revision": 0}
            )
    assert len(proposal["actions"]) == 16
    read(value)
    proposal["base_revisions"][9]["subject"]["key"] = network()
    with pytest.raises(TypeError, match="key must be null"):
        read(value)


@pytest.mark.parametrize(
    "action, path, invalid",
    [
        *[
            (a, ("revision",), v)
            for a in (
                "activate_revision",
                "deactivate_outbound",
                "retire_revision",
                "remove_staged",
                "quarantine_revision",
                "register_route",
                "set_destination_paused",
                "attest_deployment_progress",
                "open_reconciliation",
            )
            for v in (0, -1, True, 1.0, "1", 1 << 32)
        ],
        ("switch_revision", ("to",), 1),
        ("switch_revision", ("from",), 0),
        ("switch_revision", ("to",), 1 << 32),
        *[
            (a, ("paused",), v)
            for a in ("set_taira_paused", "set_destination_paused")
            for v in (0, 1, "true", None)
        ],
        ("activate_light_client_profile", ("version",), 1),
        ("activate_light_client_profile", ("profile_hash",), [0] * 32),
        ("activate_light_client_profile", ("profile_hash",), [True] * 32),
        ("activate_light_client_profile", ("profile_hash",), [256] * 32),
        ("activate_light_client_profile", ("profile_hash",), [1] * 31),
        ("open_reconciliation", ("window_ms",), 0),
        ("open_reconciliation", ("window_ms",), SAFE_MAX + 1),
        ("set_fast_pause_suspended", ("duration_ms",), 7_776_000_001),
        ("attest_deployment_progress", ("at", "detail"), {}),
        ("attest_deployment_progress", ("at", "at"), "unknown"),
        ("install_trusted_checkpoint", ("network",), network("ton_mainnet")),
        ("install_trusted_checkpoint", ("checkpoint", "source_height"), SAFE_MAX + 1),
        ("install_trusted_checkpoint", ("checkpoint", "source_time_ms"), True),
        ("install_trusted_checkpoint", ("checkpoint", "state_root"), [1] * 31),
        ("install_trusted_checkpoint", ("checkpoint", "block_hash"), [1.0] * 32),
        ("install_trusted_checkpoint", ("checkpoint", "receipts_or_tx_root"), "00" * 32),
        ("register_route", ("deployment", "deployment", "address"), [0] * 20),
        ("register_route", ("deployment", "family"), "ton"),
        *[
            ("register_route", ("initial_committee", f), 0)
            for f in ("generation", "start_height", "until_ms")
        ],
        ("register_route", ("initial_committee", "generation"), SAFE_MAX + 1),
        ("register_route", ("initial_committee", "committee_root"), [0] * 32),
        *[
            ("register_route", ("max_wrapped_supply",), v)
            for v in ("0", "01", "-1", "1.0", 1, str(1 << 128), "9" * 10000)
        ],
        ("initialize_light_client", ("expected", "expected"), "present"),
        ("initialize_light_client", ("expected", "detail"), {}),
        ("initialize_light_client", ("params", "network"), network("ethereum_mainnet")),
        ("initialize_light_client", ("bootstrap", "network"), network("ton_mainnet")),
        *[
            ("initialize_light_client", ("bootstrap", "bytes"), v)
            for v in ("", "AA", "AB==", " AA==", "AA==\n", "AA-_", 1)
        ],
        *[
            ("initialize_light_client", ("params", f), 0)
            for f in (
                "ws_bound_ms",
                "checkpoint_stride",
                "set_retention_ms",
                "checkpoint_prune_after_ms",
                "max_updates_per_advance",
                "max_segment_headers",
                "max_ancestry_headers",
                "max_backfill_headers",
                "max_advance_bytes",
                "max_proof_bytes",
            )
        ],
        ("initialize_light_client", ("params", "max_proof_bytes"), 1 << 32),
        ("initialize_light_client", ("params", "set_retention_ms"), SAFE_MAX + 1),
    ],
)
def test_rejects_invalid_static_action_values(action, path, invalid):
    value = fixture(action)
    read(value)
    target = value["payload"]["proposal"]["actions"][0]["payload"]
    for part in path[:-1]:
        target = target[part]
    target[path[-1]] = invalid
    with pytest.raises(TypeError):
        read(value)


@pytest.mark.parametrize("name", NETWORKS)
def test_registration_and_light_client_network_boundaries(name):
    value = fixture("register_route", name)
    body = value["payload"]["proposal"]["actions"][0]["payload"]
    cap = (1 << (96 if name == "ton_mainnet" else 128)) - 1
    body["max_wrapped_supply"] = str(cap)
    body["revision"] = (1 << 32) - 1
    for field in ("generation", "start_height", "until_ms"):
        body["initial_committee"][field] = SAFE_MAX
    read(value)
    body["max_wrapped_supply"] = str(cap + 1)
    with pytest.raises(TypeError):
        read(value)
    value = fixture("initialize_light_client", name)
    body = value["payload"]["proposal"]["actions"][0]["payload"]
    body["expected"]["expected"] = "unusable"
    body["bootstrap"]["bytes"] = base64.b64encode(bytes(1_048_576)).decode("ascii")
    read(value)
    body["bootstrap"]["bytes"] = base64.b64encode(bytes(1_048_577)).decode("ascii")
    with pytest.raises(TypeError):
        read(value)


def test_ton_code_depth_and_derived_light_client_fields():
    value = fixture("register_route", "ton_mainnet")
    deployment = value["payload"]["proposal"]["actions"][0]["payload"]["deployment"]["deployment"]
    for field in ("minter_code", "wallet_code", "bucket_code"):
        deployment[field]["depth"] = 65535
    read(value)
    for field in ("minter_code", "wallet_code", "bucket_code"):
        deployment[field]["depth"] = 65536
        with pytest.raises(TypeError):
            read(value)
        deployment[field]["depth"] = 0
    value = fixture("initialize_light_client", "ton_mainnet")
    params = value["payload"]["proposal"]["actions"][0]["payload"]["params"]
    for field in ("ws_bound_ms", "checkpoint_stride"):
        params[field] = 1
        with pytest.raises(TypeError):
            read(value)
        params[field] = 0
    read(value)


@pytest.mark.parametrize("second_network", ("ethereum_mainnet", "ton_mainnet"))
def test_duplicate_destination_word_is_rejected_across_networks(second_network):
    value = fixture("register_route", "bsc_mainnet")
    proposal = value["payload"]["proposal"]
    other = action_payload("register_route", second_network)
    if second_network == "ton_mainnet":
        other["deployment"]["deployment"]["master_account"] = [0] * 12 + [1] * 20
    proposal["actions"].append({"action": "register_route", "payload": other})
    base = {"subject": subject("route", second_network), "revision": 0}
    proposal["base_revisions"].insert(0 if second_network == "ethereum_mainnet" else 1, base)
    with pytest.raises(TypeError, match="destination"):
        read(value)
    address = other["deployment"]["deployment"]
    address["master_account" if second_network == "ton_mainnet" else "address"][-1] = 2
    read(value)


@pytest.mark.parametrize(
    "field, invalid",
    [
        ("enabled", 1),
        ("committee_validity_ms", 2_592_000_001),
        ("committee_validity_ms", 1),
        ("committee_heartbeat_ms", 3_599_999),
        ("committee_heartbeat_ms", SAFE_MAX),
        ("forgery_evidence_grace_ms", 86_399_999),
        ("forgery_evidence_grace_ms", 2_592_000_001),
        ("destination_progress_cap_ms", -1),
        ("destination_progress_cap_ms", 2_592_000_001),
        ("outbound_ttl_ms", 86_399_999),
        ("outbound_ttl_ms", 7_776_000_001),
        ("min_outbound_amount", "0"),
        ("min_outbound_amount", "1000000000000001"),
        ("inbound_self_claim_fee", "10000000001"),
        ("max_exempt_transactions_per_block", 7),
        ("max_exempt_transactions_per_block", 1025),
        ("fast_pause_panel_seats", 6),
        ("fast_pause_panel_term_ms", 3_599_999),
        ("fast_pause_seat_accept_window_ms", 599_999),
        ("fast_pause_seat_accept_window_ms", 7 * 86_400_000 // 2 + 1),
        ("fast_pause_endorse_window_ms", 299_999),
        ("fast_pause_endorse_window_ms", 21_600_001),
        ("fast_pause_renew_lead_ms", 3_599_999),
        ("fast_pause_renew_lead_ms", 5 * 86_400_000 + 1),
        ("fast_pause_hold_ms", 2_505_600_001),
        ("fast_pause_panel_term_ms", SAFE_MAX + 1),
    ],
)
def test_parameter_own_and_joint_bounds(field, invalid):
    value = fixture("set_parameters")
    read(value)
    value["payload"]["proposal"]["actions"][0]["payload"]["next"][field] = invalid
    with pytest.raises(TypeError):
        read(value)


def test_progress_generation_and_checkpoint_boundary_values():
    value = fixture("attest_deployment_progress")
    at = value["payload"]["proposal"]["actions"][0]["payload"]["at"]
    at.update(at="generation", detail={"generation": SAFE_MAX, "committee_root": [1] * 32})
    read(value)
    for field, invalid in (
        ("generation", 0),
        ("generation", SAFE_MAX + 1),
        ("committee_root", [0] * 32),
    ):
        bad = copy.deepcopy(value)
        bad["payload"]["proposal"]["actions"][0]["payload"]["at"]["detail"][field] = invalid
        with pytest.raises(TypeError):
            read(bad)
    value = fixture("install_trusted_checkpoint")
    checkpoint = value["payload"]["proposal"]["actions"][0]["payload"]["checkpoint"]
    checkpoint.update(source_height=SAFE_MAX, source_time_ms=SAFE_MAX, state_root=[0] * 32)
    read(value)


@pytest.mark.parametrize(
    "field, invalid",
    [
        *[("amount", v) for v in ("0", "01", "-1", "1.0", 1, str(1 << 128), "9" * 10000)],
        *[("memo", v) for v in (None, 1, "a" * 257, "é" * 129, "🦊" * 65, "\ud800", "\udc00")],
    ],
)
def test_release_rejects_bad_amount_or_memo_before_native_account_check(field, invalid):
    value = fixture("release_stranded")
    value["payload"]["proposal"]["actions"][0]["payload"][field] = invalid
    with pytest.raises(TypeError):
        read(value)


SHARED = json.loads(
    (Path(__file__).resolve().parents[3] / "fixtures/sccp/governance_v1.json").read_text()
)


@pytest.mark.parametrize("row", SHARED["rows"], ids=lambda row: row["label"])
def test_shared_rust_governance_subject_vectors(row):
    source = row["input"]
    proposal = {
        "network_id": canonical_hash(0x11),
        "base_revisions": [
            {
                "subject": subject(
                    entry["subject"]["kind"], (entry["subject"]["network"] or "").replace("-", "_")
                ),
                "revision": entry["revision"],
            }
            for entry in source["base_revisions"]
        ],
        "actions": [
            {
                "action": entry["kind"],
                "payload": {
                    **{key: item for key, item in entry.items() if key not in ("kind", "network")},
                    "network": network(entry["network"].replace("-", "_")),
                },
            }
            for entry in source["actions"]
        ],
    }
    value = {"kind": "SccpRouteGovernance", "payload": {"proposal": proposal}}
    if row["expected"]["static_outcome"] == "Accepted":
        assert thaw(read(value)) == proposal
    else:
        with pytest.raises(TypeError):
            read(value)
