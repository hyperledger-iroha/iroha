"""Exact operator-authentication tests for Sumeragi reads."""

from __future__ import annotations

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


def _sumeragi_evidence_common(*, admitted: Optional[int] = None) -> Dict[str, Any]:
    return {
        "recorded_height": 40,
        "recorded_view": 2,
        "recorded_ms": 1_700_000_000_000,
        "consensus_admitted_height": admitted,
    }


def _sumeragi_v2_equivocation_record() -> Dict[str, Any]:
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
        **_sumeragi_evidence_common(admitted=41),
    }


def _sumeragi_censorship_record() -> Dict[str, Any]:
    return {
        "kind": "Censorship",
        "tx_hash": "44" * 32,
        "receipt_count": 2,
        "signers": ["alice@test", "bob@test"],
        "submitted_at_height_min": 20,
        "submitted_at_height_max": 22,
        **_sumeragi_evidence_common(),
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
                    {
                        "kind": "DoublePrepare",
                        "recorded_height": 1,
                        "recorded_view": 2,
                        "recorded_ms": 3,
                        "consensus_admitted_height": None,
                        "phase": "Prepare",
                        "height": 4,
                        "view": 5,
                        "epoch": 6,
                        "signer": 1,
                        "block_hash_1": "aa" * 32,
                        "block_hash_2": "bb" * 32,
                    },
                    {
                        "kind": "InvalidProposal",
                        "recorded_height": 7,
                        "recorded_view": 8,
                        "recorded_ms": 9,
                        "consensus_admitted_height": None,
                        "height": 10,
                        "view": 11,
                        "epoch": 12,
                        "subject_block_hash": "cc" * 32,
                        "payload_hash": "dd" * 32,
                        "reason": "payload mismatch",
                    },
                    _sumeragi_censorship_record(),
                    _sumeragi_v2_equivocation_record(),
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

    page = client.list_sumeragi_evidence(
        limit=5, offset=1, kind="SumeragiV2Equivocation"
    )

    assert page.total == 4
    assert len(page.items) == 4
    prevote = page.items[0]
    assert isinstance(prevote, client_module.SumeragiDoubleVoteEvidenceRecord)
    assert prevote.kind == "DoublePrepare"
    assert prevote.phase == "Prepare"
    assert prevote.signer == 1
    assert prevote.block_hash_1 == "aa" * 32
    assert prevote.block_hash_2 == "bb" * 32
    invalid_proposal = page.items[1]
    assert isinstance(
        invalid_proposal, client_module.SumeragiInvalidProposalEvidenceRecord
    )
    assert invalid_proposal.payload_hash == "dd" * 32
    assert invalid_proposal.reason == "payload mismatch"
    censorship = page.items[2]
    assert isinstance(censorship, client_module.SumeragiCensorshipEvidenceRecord)
    assert censorship.submitted_at_height_min == 20
    assert censorship.submitted_at_height_max == 22
    equivocation = page.items[3]
    assert isinstance(
        equivocation, client_module.SumeragiV2EquivocationEvidenceRecord
    )
    assert equivocation.class_ == "phase_vote"
    assert equivocation.signer == 3
    assert equivocation.context_id == "11" * 32
    assert equivocation.consensus_admitted_height == 41
    call = session.calls[0]
    assert call["url"].endswith(
        "/v1/sumeragi/evidence?kind=SumeragiV2Equivocation&limit=5&offset=1"
    )
    assert call["params"] == {}
    assert call["allow_redirects"] is False
    assert call["data"] is None
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
