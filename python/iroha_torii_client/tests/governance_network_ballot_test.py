"""Exact-network and one-shot transport tests for governance ballots."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, List, Optional

import pytest
from sumeragi_exact_json_test_support import RecordingSession, StubResponse

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (  # noqa: E402
    ToriiCanonicalRequestAuth,
    ToriiClient,
    canonical_network_request_signature_message,
)

CANONICAL_OWNER = "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP"
CANONICAL_OWNER_HEADER = (
    "0x02000120"
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
)


def _canonical_hash(seed: int) -> str:
    body_bytes = bytearray([seed & 0xFF] * 32)
    body_bytes[-1] |= 1
    body = body_bytes.hex().upper()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return f"hash:{body}#{crc:04X}"


GOVERNANCE_NETWORK_ID = _canonical_hash(0xA5)
FOREIGN_GOVERNANCE_NETWORK_ID = _canonical_hash(0xA7)


def _governance_auth(captured: Optional[List[bytes]] = None) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x44" * 64

    return ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=signer,
        timestamp_ms=4_102_444_801_000,
        nonce="low-python-governance-test",
    )


def test_governance_plain_ballot_uses_exact_network_auth_and_one_shot_transport() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "drafted": True,
                "tx_instructions": [{"wire_id": "CastPlainBallot", "payload_hex": "00"}],
            }
        )
    )
    captured: List[bytes] = []
    auth = _governance_auth(captured)
    client = ToriiClient("https://node.test", session=session)

    result = client.submit_plain_ballot(
        authority=CANONICAL_OWNER,
        network_id=GOVERNANCE_NETWORK_ID,
        referendum_id="referendum-1",
        owner=CANONICAL_OWNER,
        amount="1",
        duration_blocks=1,
        direction="Aye",
        canonical_auth=auth,
    )

    assert result.drafted is True
    assert result.tx_instructions[0].wire_id == "CastPlainBallot"
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["allow_redirects"] is False
    payload = json.loads(call["data"].decode("utf-8"))
    assert payload["network_id"] == GOVERNANCE_NETWORK_ID
    assert payload["duration_blocks"] == "1"
    assert "chain_id" not in payload
    assert call["headers"]["X-Iroha-Account"] == CANONICAL_OWNER_HEADER
    assert captured == [
        canonical_network_request_signature_message(
            GOVERNANCE_NETWORK_ID,
            "POST",
            "/v1/gov/ballots/plain",
            call["data"],
            timestamp_ms=auth.timestamp_ms or 0,
            nonce=auth.nonce or "",
        )
    ]


@pytest.mark.parametrize(
    "payload",
    [
        {"ok": True, "accepted": True, "reason": None, "tx_instructions": []},
        {"drafted": False, "tx_instructions": [{"wire_id": "CastPlainBallot", "payload_hex": "00"}]},
        {"drafted": True, "tx_instructions": []},
        {"drafted": True, "tx_instructions": [{"wire_id": "Cast PlainBallot", "payload_hex": "00"}]},
        {"drafted": True, "tx_instructions": [{"wire_id": "CastPlainBallot", "payload_hex": "AA"}]},
    ],
)
def test_governance_ballot_rejects_non_draft_response_contract(payload: Any) -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=payload))
    client = ToriiClient("https://node.test", session=session)

    with pytest.raises(RuntimeError, match="draft|fields|instruction"):
        client.submit_plain_ballot(
            authority=CANONICAL_OWNER,
            network_id=GOVERNANCE_NETWORK_ID,
            referendum_id="referendum-1",
            owner=CANONICAL_OWNER,
            amount="1",
            duration_blocks=1,
            direction="Aye",
            canonical_auth=_governance_auth(),
        )


@pytest.mark.parametrize("duration_blocks", [-1, 1 << 64, 1.0, "1", True])
def test_governance_plain_ballot_rejects_noncanonical_duration_before_transport(
    duration_blocks: Any,
) -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises((TypeError, ValueError), match="duration_blocks"):
        client.submit_plain_ballot(
            authority=CANONICAL_OWNER,
            network_id=GOVERNANCE_NETWORK_ID,
            referendum_id="referendum-1",
            owner=CANONICAL_OWNER,
            amount="1",
            duration_blocks=duration_blocks,  # type: ignore[arg-type]
            direction="Aye",
            canonical_auth=_governance_auth(),
        )
    assert session.calls == []


def test_governance_network_signature_cannot_replay_across_genesis_hashes() -> None:
    canonical = canonical_network_request_signature_message(
        GOVERNANCE_NETWORK_ID,
        "POST",
        "/v1/gov/ballots/plain",
        b"{}",
        timestamp_ms=1,
        nonce="same-label",
    )
    foreign = canonical_network_request_signature_message(
        FOREIGN_GOVERNANCE_NETWORK_ID,
        "POST",
        "/v1/gov/ballots/plain",
        b"{}",
        timestamp_ms=1,
        nonce="same-label",
    )
    assert canonical != foreign


def test_governance_ballot_rejects_retired_key_and_principal_mismatch_before_transport() -> None:
    session = RecordingSession()
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(TypeError, match="chain_id"):
        client.submit_plain_ballot(
            authority=CANONICAL_OWNER,
            chain_id="chain",  # type: ignore[call-arg]
            referendum_id="referendum-1",
            owner=CANONICAL_OWNER,
            amount="1",
            duration_blocks=1,
            direction="Aye",
            canonical_auth=_governance_auth(),
        )
    mismatched = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id="sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        signer=lambda _message: b"\x55" * 64,
    )
    with pytest.raises(ValueError, match="must equal payload authority"):
        client.submit_plain_ballot(
            authority=CANONICAL_OWNER,
            network_id=GOVERNANCE_NETWORK_ID,
            referendum_id="invalid selector with spaces",
            owner=CANONICAL_OWNER,
            amount="1",
            duration_blocks=1,
            direction="Aye",
            canonical_auth=mismatched,
        )
    assert session.calls == []


def test_governance_ballot_307_response_is_not_followed_or_retried() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=307, payload={"redirect": True}))
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(RuntimeError, match="unexpected status 307"):
        client.submit_plain_ballot(
            authority=CANONICAL_OWNER,
            network_id=GOVERNANCE_NETWORK_ID,
            referendum_id="referendum-1",
            owner=CANONICAL_OWNER,
            amount="1",
            duration_blocks=1,
            direction="Aye",
            canonical_auth=_governance_auth(),
        )
    assert len(session.calls) == 1
    assert session.calls[0]["allow_redirects"] is False
