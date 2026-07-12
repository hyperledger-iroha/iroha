"""Protocol-v2-only parsing tests for the Torii Sumeragi status model."""

from __future__ import annotations

import copy

import pytest

from iroha_python.client import (
    SumeragiStatusSnapshot,
    SumeragiV2BodyState,
    SumeragiV2GlobalPhase,
    SumeragiV2StatusPhase,
)


def _subject(seed: str = "BLOCK") -> dict[str, str]:
    return {
        "parent_block_hash": f"hash:PARENT#{seed}",
        "block_hash": f"hash:{seed}#0001",
        "payload_hash": f"hash:PAYLOAD#{seed}",
    }


def _prepare_qc(view: int = 3) -> dict[str, object]:
    return {
        "round": {
            "context_id": ["hash:CONTEXT#0001"],
            "height": 15,
            "view": view,
        },
        "phase": {"phase": "Prepare", "details": None},
        "subject": _subject(),
    }


def _healthy_status() -> dict[str, object]:
    prepare_qc = _prepare_qc()
    return {
        "protocol_version": 2,
        "node_fingerprint": "hash:NODE#0001",
        "build_fingerprint": "hash:BUILD#0001",
        "config_fingerprint": "hash:CONFIG#0001",
        "height_context_id": ["hash:CONTEXT#0001"],
        "height": 15,
        "view": 4,
        "phase": {"phase": "Prepare", "details": None},
        "leader": 1,
        "locked_prepare_qc": copy.deepcopy(prepare_qc),
        "highest_prepare_qc": copy.deepcopy(prepare_qc),
        "last_timeout_certificate": {
            "round": {
                "context_id": ["hash:CONTEXT#0001"],
                "height": 15,
                "view": 3,
            },
            "highest_prepare_qc": copy.deepcopy(prepare_qc),
            "certificate_hash": "hash:TC#0001",
        },
        "body_state": {"state": "Validated", "details": None},
        "pending_persistence_id": 17,
        "last_committed_height": 14,
        "last_committed_subject": _subject("COMMITTED"),
        "lane_settlement_commitments": [],
        "lane_relay_envelopes": [],
    }


def test_status_parses_authoritative_reducer_state() -> None:
    status = SumeragiStatusSnapshot.from_payload(_healthy_status())

    assert status.protocol_version == 2
    assert status.height_context_id.hash == "hash:CONTEXT#0001"
    assert status.height == 15
    assert status.view == 4
    assert status.phase is SumeragiV2StatusPhase.PREPARE
    assert status.body_state is SumeragiV2BodyState.VALIDATED
    assert status.locked_prepare_qc is not None
    assert status.locked_prepare_qc.phase is SumeragiV2GlobalPhase.PREPARE
    assert status.locked_prepare_qc.round.view == 3
    assert status.last_timeout_certificate is not None
    assert status.last_timeout_certificate.certificate_hash == "hash:TC#0001"
    assert status.pending_persistence_id == 17
    assert status.last_committed_subject is not None
    assert status.last_committed_subject.block_hash == "hash:COMMITTED#0001"


def test_status_allows_genesis_without_optional_certificates() -> None:
    payload = _healthy_status()
    payload.update(
        {
            "height": 0,
            "view": 0,
            "phase": {"phase": "AwaitingProposal", "details": None},
            "body_state": {"state": "Missing", "details": None},
            "last_committed_height": 0,
            "last_committed_subject": None,
            "pending_persistence_id": None,
            "locked_prepare_qc": None,
            "highest_prepare_qc": None,
            "last_timeout_certificate": None,
        }
    )

    status = SumeragiStatusSnapshot.from_payload(payload)

    assert status.phase is SumeragiV2StatusPhase.AWAITING_PROPOSAL
    assert status.body_state is SumeragiV2BodyState.MISSING
    assert status.last_committed_subject is None


@pytest.mark.parametrize(
    ("mutate", "error"),
    [
        (lambda payload: payload.update(protocol_version=1), "expected 2"),
        (
            lambda payload: payload.update(pending_rbc={"sessions": 0}),
            "unsupported fields: pending_rbc",
        ),
        (
            lambda payload: payload.update(
                phase={"phase": "Prepare", "details": {}}
            ),
            "null `details`",
        ),
        (
            lambda payload: payload.update(last_committed_height=16),
            "exceeds sumeragi.height",
        ),
        (
            lambda payload: payload.update(pending_persistence_id=0),
            "must be positive",
        ),
        (
            lambda payload: payload["locked_prepare_qc"].update(
                phase={"phase": "Commit", "details": None}
            ),
            "must reference a PrepareQC",
        ),
        (
            lambda payload: payload["highest_prepare_qc"]["round"].update(view=5),
            "comes from a future view",
        ),
        (
            lambda payload: payload["last_timeout_certificate"]["round"].update(view=4),
            "must certify a preceding view",
        ),
        (
            lambda payload: payload.update(last_committed_subject=None),
            "required after the first commit",
        ),
    ],
)
def test_status_rejects_malformed_or_legacy_state(mutate, error: str) -> None:
    payload = _healthy_status()
    mutate(payload)

    with pytest.raises((TypeError, ValueError), match=error):
        SumeragiStatusSnapshot.from_payload(payload)
