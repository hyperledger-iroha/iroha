"""Governed runtime change-set helpers (`/v1/runtime/upgrades*`, `/v1/runtime/metrics`)."""

from __future__ import annotations

import copy
import json
from typing import Any, Callable, Dict

import pytest

import iroha_python
import iroha_python.client as client_module
from iroha_python.client import (
    RuntimeChangeSetStatus,
    RuntimeMetrics,
    RuntimeNotice,
    RuntimeUpgradeCounters,
    RuntimeUpgradesSnapshot,
    ToriiClient,
)

from .helpers import RecordingSession, StubResponse

NETWORK_ID = "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"
SELECTION_ID = "5a" * 32
SUCCESSOR_ID = "6b" * 32
COMMITTEE_DIGEST = "7c" * 32
COMMITTEE = [
    "ea013094D37A1FCA72E8734CAAD4163678D82C36FE2CA70B80F5626E6591709E0D44831BE86CBA9BD0471C6D0D73FF9C4B54E0",
    "ed012066BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
]
PROPOSAL_WIRE_ID = "iroha.instruction.v1::governance::ProposeRuntimeUpgradeProposal"


def _change_set(effective_height: int) -> Dict[str, Any]:
    return {
        "network": NETWORK_ID,
        "root": {"kind": "Universal", "content": None},
        "predecessor": SELECTION_ID,
        "successor": SUCCESSOR_ID,
        "effective_height": effective_height,
        "publish": [],
        "transitions": [],
        "qualification": "3c" * 32,
    }


def _item(id_hex: str, effective_height: int, status: Dict[str, Any]) -> Dict[str, Any]:
    return {
        "id_hex": id_hex,
        "record": {
            "change_set": _change_set(effective_height),
            "status": status,
            "proposed_ms": 1_700_000_000_000,
            "approved_height": 90,
            "approved_ms": 1_700_000_600_000,
            "notice": {"blocks": 3_600, "ms": 1_209_600_000, "approval_cushion_ms": 86_400_000},
            "attestations": [],
        },
    }


def _snapshot(*items: Dict[str, Any]) -> Dict[str, Any]:
    return {
        "selection_id_hex": SELECTION_ID,
        "selection": {"kernel": "11" * 32, "active": [], "bindings": []},
        "committee": list(COMMITTEE),
        "committee_digest_hex": COMMITTEE_DIGEST,
        "items": list(items),
    }


def _drafted(payload_hex: str) -> Dict[str, Any]:
    return {
        "ok": True,
        "tx_instructions": [{"wire_id": PROPOSAL_WIRE_ID, "payload_hex": payload_hex}],
    }


def _client(session: RecordingSession) -> ToriiClient:
    return ToriiClient("http://node.test", session=session, chain_discriminant=753)


def test_list_runtime_upgrades_typed_parses_selection_and_change_sets() -> None:
    payload = _snapshot(
        _item("aa" * 32, 120, {"kind": "Scheduled", "content": None}),
        _item("bb" * 32, 120, {"kind": "Scheduled"}),
        _item("cc" * 32, 140, {"kind": "Cancelled", "content": 101}),
    )
    session = RecordingSession(StubResponse(payload=payload))

    snapshot = _client(session).list_runtime_upgrades_typed()

    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades")
    assert isinstance(snapshot, RuntimeUpgradesSnapshot)
    assert snapshot.selection_id_hex == SELECTION_ID
    assert snapshot.selection == payload["selection"]
    assert snapshot.committee == COMMITTEE
    assert snapshot.committee_digest_hex == COMMITTEE_DIGEST
    scheduled, scheduled_without_content, cancelled = snapshot.items
    assert scheduled.status == RuntimeChangeSetStatus(kind="Scheduled")
    assert scheduled_without_content.status == RuntimeChangeSetStatus(kind="Scheduled")
    assert cancelled.status == RuntimeChangeSetStatus(kind="Cancelled", height=101)
    assert cancelled.id_hex == "cc" * 32
    assert cancelled.effective_height == 140
    assert cancelled.predecessor_hex == SELECTION_ID
    assert cancelled.successor_hex == SUCCESSOR_ID
    assert cancelled.notice == RuntimeNotice(
        blocks=3_600, ms=1_209_600_000, approval_cushion_ms=86_400_000
    )
    assert cancelled.record == payload["items"][2]["record"]


@pytest.mark.parametrize("kind", ["Activated", "Lapsed", "Cancelled"])
def test_runtime_change_set_status_carries_its_transition_height(kind: str) -> None:
    status = RuntimeChangeSetStatus.from_payload({"kind": kind, "content": 77})

    assert status == RuntimeChangeSetStatus(kind=kind, height=77)


@pytest.mark.parametrize(
    ("status", "error", "message"),
    [
        ({"kind": "Scheduled", "content": 5}, ValueError, "must not carry content"),
        ({"kind": "Cancelled"}, TypeError, "missing required `content`"),
        ({"kind": "Lapsed", "content": -1}, ValueError, "unsigned 64-bit integer"),
        ({"kind": "Canceled", "content": 3}, TypeError, "kind must be one of"),
        ({"Proposed": None}, TypeError, "kind must be one of"),
        ({"ActivatedAt": 12}, TypeError, "kind must be one of"),
    ],
)
def test_runtime_change_set_status_rejects_retired_and_malformed_variants(
    status: Dict[str, Any], error: type[Exception], message: str
) -> None:
    with pytest.raises(error, match=message):
        RuntimeChangeSetStatus.from_payload(status)


def _retired_manifest_record(payload: Dict[str, Any]) -> None:
    payload["items"][0]["record"] = {
        "manifest": {"name": "ABI v1 refresh", "abi_version": 1},
        "status": {"Proposed": None},
        "proposer": "proposer",
        "created_height": 8,
    }


@pytest.mark.parametrize(
    ("mutate", "error", "message"),
    [
        (_retired_manifest_record, ValueError, "unknown field `created_height`"),
        (lambda payload: payload.__setitem__("total", 2), ValueError, "unknown field `total`"),
        (
            lambda payload: payload.pop("committee_digest_hex"),
            TypeError,
            "missing required `committee_digest_hex`",
        ),
        (
            lambda payload: payload["committee"].__setitem__(0, COMMITTEE[0].lower()),
            TypeError,
            r"committee\[0\] must be an exact canonical public-key multihash",
        ),
        (
            lambda payload: payload.__setitem__("selection_id_hex", "0x" + "5a" * 31),
            ValueError,
            "selection_id_hex must be exactly 64 lowercase hexadecimal characters",
        ),
        (lambda payload: payload["items"].reverse(), ValueError, "ordered by effective height"),
        (
            lambda payload: payload["items"].append(copy.deepcopy(payload["items"][-1])),
            ValueError,
            "must be unique",
        ),
        (
            lambda payload: payload["items"][0]["record"]["change_set"].pop("effective_height"),
            TypeError,
            "effective_height must be an unsigned 64-bit integer",
        ),
    ],
)
def test_runtime_upgrades_snapshot_rejects_malformed_and_retired_payloads(
    mutate: Callable[[Dict[str, Any]], Any], error: type[Exception], message: str
) -> None:
    payload = _snapshot(
        _item("aa" * 32, 120, {"kind": "Scheduled", "content": None}),
        _item("bb" * 32, 130, {"kind": "Activated", "content": 130}),
    )
    mutate(payload)

    with pytest.raises(error, match=message):
        RuntimeUpgradesSnapshot.from_payload(payload)


def test_runtime_metrics_count_change_sets_by_status() -> None:
    metrics = RuntimeMetrics.from_payload(
        {
            "abi_version": 1,
            "upgrade_events_total": {"scheduled": 2, "activated": 3, "lapsed": 1, "canceled": 4},
        }
    )

    assert metrics.abi_version == 1
    assert metrics.upgrade_events_total == RuntimeUpgradeCounters(
        scheduled=2, activated=3, lapsed=1, canceled=4
    )


def test_runtime_metrics_reject_the_retired_proposed_counter() -> None:
    with pytest.raises(ValueError, match="unknown field `proposed`"):
        RuntimeMetrics.from_payload(
            {
                "abi_version": 1,
                "upgrade_events_total": {"proposed": 5, "activated": 3, "canceled": 1},
            }
        )


def test_propose_runtime_upgrade_posts_the_change_set_as_given() -> None:
    session = RecordingSession(StubResponse(payload=_drafted("aa" * 32)))
    change_set = _change_set(240)

    response = _client(session).propose_runtime_upgrade_typed(change_set)

    assert response.ok is True
    assert response.tx_instructions[0].wire_id == PROPOSAL_WIRE_ID
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/propose")
    assert json.loads(session.calls[0]["data"].decode("utf-8")) == change_set


def test_propose_runtime_upgrade_requires_a_change_set_object() -> None:
    session = RecordingSession(StubResponse(payload=_drafted("aa" * 32)))

    with pytest.raises(TypeError, match="runtime change set must be a JSON object"):
        _client(session).propose_runtime_upgrade([_change_set(240)])  # type: ignore[arg-type]

    assert session.calls == []


def test_cancel_runtime_upgrade_posts_the_change_set_id() -> None:
    session = RecordingSession(StubResponse(payload=_drafted("dd" * 32)))

    response = _client(session).cancel_runtime_upgrade_typed("aa" * 32)

    assert response.tx_instructions[0].wire_id == PROPOSAL_WIRE_ID
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/runtime/upgrades/cancel/" + "aa" * 32)
    assert session.calls[0]["data"] == b""


@pytest.mark.parametrize(
    ("change_set_id", "error"),
    [
        ("0x" + "aa" * 32, ValueError),
        ("AA" * 32, ValueError),
        ("aa" * 31, ValueError),
        (bytes([0xAA]) * 32, TypeError),
    ],
)
def test_cancel_runtime_upgrade_requires_an_exact_change_set_id(
    change_set_id: Any, error: type[Exception]
) -> None:
    session = RecordingSession(StubResponse(payload=_drafted("dd" * 32)))

    with pytest.raises(error, match="exactly 64 lowercase hexadecimal characters"):
        _client(session).cancel_runtime_upgrade(change_set_id)

    assert session.calls == []


def test_retired_runtime_upgrade_manifest_surface_is_absent() -> None:
    for name in ("activate_runtime_upgrade", "activate_runtime_upgrade_typed"):
        assert not hasattr(ToriiClient, name), name
    for name in (
        "RuntimeUpgradeManifest",
        "RuntimeUpgradeRecord",
        "RuntimeUpgradeStatus",
        "RuntimeUpgradeListPage",
        "GovernanceRuntimeUpgradeManifest",
    ):
        assert not hasattr(client_module, name), name
        assert name not in client_module.__all__, name
        assert not hasattr(iroha_python, name), name
        assert name not in iroha_python.__all__, name
