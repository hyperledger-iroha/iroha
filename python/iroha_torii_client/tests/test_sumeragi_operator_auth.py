"""Exact operator-authentication tests for Sumeragi reads."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

import pytest

from client_test_support import canonical_hash
from sumeragi_exact_json_test_support import RecordingSession, StubResponse

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import iroha_torii_client.client as client_module  # noqa: E402
from iroha_torii_client import (  # noqa: E402
    ToriiClient,
    ToriiOperatorSigningContext,
    operator_network_request_signature_message,
)

GOVERNANCE_NETWORK_ID = canonical_hash(0xA5)


def _operator_context(captured: Optional[List[bytes]] = None) -> ToriiOperatorSigningContext:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x55" * 64

    return ToriiOperatorSigningContext(
        network_id=GOVERNANCE_NETWORK_ID,
        public_key="ed0120" + "66" * 32,
        signer=signer,
    )


def _sumeragi_v2_equivocation_record(
    *, penalty_status: Optional[Dict[str, Any]] = None
) -> Dict[str, Any]:
    return {
        "kind": "SumeragiV2Equivocation",
        "class": "phase_vote",
        "height": 31,
        "view": 4,
        "epoch": 2,
        "signer": 3,
        "context_id": "11" * 32,
        "artifact_hash_1": "22" * 32,
        "artifact_hash_2": "33" * 32,
        "recorded_height": 40,
        "recorded_view": 2,
        "recorded_ms": 1_700_000_000_000,
        "consensus_admitted_height": 41,
        "penalty_status": penalty_status
        if penalty_status is not None
        else {"status": "pending", "details": None},
    }


def test_operator_reads_reject_missing_or_fallback_auth_before_dispatch() -> None:
    missing_session = RecordingSession()
    missing_client = ToriiClient("http://node.test", session=missing_session)
    with pytest.raises(ValueError, match="ToriiOperatorSigningContext"):
        missing_client.get_sumeragi_qc()
    assert missing_session.calls == []

    fallback_session = RecordingSession()
    fallback_session.headers["Authorization"] = "Bearer retired"
    fallback_client = ToriiClient(
        "http://node.test",
        session=fallback_session,
        operator_signing_context=_operator_context(),
    )
    with pytest.raises(ValueError, match="reject token"):
        fallback_client.get_sumeragi_qc()
    assert fallback_session.calls == []

    precomputed_session = RecordingSession()
    precomputed_client = ToriiClient(
        "http://node.test",
        session=precomputed_session,
        operator_signing_context=_operator_context(),
    )
    with pytest.raises(ValueError, match="precomputed operator"):
        precomputed_client._operator_get(
            "/v1/sumeragi/qc",
            headers={"X-Iroha-Operator-Signature": "precomputed"},
        )
    assert precomputed_session.calls == []


def test_list_sumeragi_evidence_signs_canonical_query_and_parses_records() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 4,
                "items": [
                    _sumeragi_v2_equivocation_record(),
                    _sumeragi_v2_equivocation_record(
                        penalty_status={
                            "status": "applied",
                            "details": {"height": 42},
                        }
                    ),
                    _sumeragi_v2_equivocation_record(
                        penalty_status={
                            "status": "cancelled",
                            "details": {"height": 43},
                        }
                    ),
                ],
            }
        )
    )
    captured: List[bytes] = []
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(captured),
    )

    page = client.list_sumeragi_evidence(limit=5, offset=1, kind="SumeragiV2Equivocation")

    assert page.total == 4
    assert len(page.items) == 3
    equivocation = page.items[0]
    assert isinstance(equivocation, client_module.SumeragiV2EquivocationEvidenceRecord)
    assert equivocation.class_ == "phase_vote"
    assert equivocation.signer == 3
    assert equivocation.context_id == "11" * 32
    assert equivocation.consensus_admitted_height == 41
    assert isinstance(
        equivocation.penalty_status,
        client_module.SumeragiEvidencePendingPenaltyStatus,
    )
    assert isinstance(
        page.items[1].penalty_status,
        client_module.SumeragiEvidenceAppliedPenaltyStatus,
    )
    assert page.items[1].penalty_status.details.height == 42
    assert isinstance(
        page.items[2].penalty_status,
        client_module.SumeragiEvidenceCancelledPenaltyStatus,
    )
    assert page.items[2].penalty_status.details.height == 43
    call = session.calls[0]
    assert call["url"].endswith(
        "/v1/sumeragi/evidence?kind=SumeragiV2Equivocation&limit=5&offset=1"
    )
    assert call["params"] == {}
    assert call["allow_redirects"] is False
    assert call["data"] is None
    assert call["stream"] is True
    assert len(captured) == 1
    timestamp_ms = int(call["headers"]["X-Iroha-Operator-Timestamp-Ms"])
    nonce = call["headers"]["X-Iroha-Operator-Nonce"]
    assert captured[0] == operator_network_request_signature_message(
        GOVERNANCE_NETWORK_ID,
        "GET",
        "/v1/sumeragi/evidence?kind=SumeragiV2Equivocation&limit=5&offset=1",
        b"",
        timestamp_ms=timestamp_ms,
        nonce=nonce,
    )


@pytest.mark.parametrize("route", ["list", "count"])
@pytest.mark.parametrize(
    ("failure", "error_type", "message"),
    [
        ("content_type", TypeError, "application/json content type"),
        ("content_length", ValueError, ""),
        ("actual_body", ValueError, ""),
        ("duplicate", ValueError, "duplicate field"),
    ],
)
def test_sumeragi_evidence_reads_enforce_strict_bounded_json(
    route: str,
    failure: str,
    error_type: type[Exception],
    message: str,
) -> None:
    maximum_body_bytes = 1024 * 1024 if route == "list" else 1024
    if route == "list":
        canonical_body = json.dumps({"total": 0, "items": []}).encode()
        duplicate_body = b'{"total":0,"total":1,"items":[]}'
    else:
        canonical_body = json.dumps({"count": 0}).encode()
        duplicate_body = b'{"count":0,"count":1}'

    headers = {"Content-Type": "application/json"}
    body = canonical_body
    if failure == "content_type":
        headers["Content-Type"] = "text/plain"
    elif failure == "content_length":
        headers["Content-Length"] = str(maximum_body_bytes + 1)
        message = f"{maximum_body_bytes}-byte size bound"
    elif failure == "actual_body":
        body = b" " * (maximum_body_bytes + 1)
        message = f"{maximum_body_bytes}-byte size bound"
    else:
        body = duplicate_body

    response = StubResponse(raw=body, headers=headers)
    session = RecordingSession()
    session.queue(response)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(error_type, match=message):
        if route == "list":
            client.list_sumeragi_evidence()
        else:
            client.get_sumeragi_evidence_count()

    assert response.was_closed is True
    assert session.calls[0]["stream"] is True
    assert session.calls[0]["headers"]["Accept"] == "application/json"
