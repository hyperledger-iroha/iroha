"""Exact operator-authentication tests for the Connect aggregate read."""

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

from iroha_torii_client import (  # noqa: E402
    ToriiClient,
    ToriiOperatorSigningContext,
    operator_network_request_signature_message,
)

NETWORK_ID = canonical_hash(0xA5)


def _operator_context(captured: Optional[List[bytes]] = None) -> ToriiOperatorSigningContext:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x55" * 64

    return ToriiOperatorSigningContext(
        network_id=NETWORK_ID,
        public_key="ed0120" + "66" * 32,
        signer=signer,
    )


def _aggregate_payload() -> Dict[str, Any]:
    return {
        "enabled": True,
        "sessions_total": 5,
        "sessions_active": 3,
        "per_ip_sessions": [{"ip": "192.0.2.1", "sessions": 2}],
        "buffered_sessions": 1,
        "total_buffer_bytes": 42,
        "dedupe_size": 7,
        "frames_in_total": 10,
        "frames_out_total": 11,
        "ciphertext_total": 12,
        "dedupe_drops_total": 0,
        "buffer_drops_total": 0,
        "plaintext_control_drops_total": 0,
        "monotonic_drops_total": 0,
        "sequence_violation_closes_total": 1,
        "role_direction_mismatch_total": 2,
        "ping_miss_total": 0,
        "p2p_rebroadcasts_total": 3,
        "p2p_rebroadcast_skipped_total": 4,
        "p2p_auth_failures_total": 5,
        "p2p_ttl_drops_total": 6,
        "p2p_unknown_session_drops_total": 7,
        "p2p_session_claims_in_total": 8,
        "p2p_session_claims_installed_total": 9,
        "p2p_session_claim_conflicts_total": 10,
        "p2p_role_consumed_total": 11,
        "p2p_session_terminated_total": 12,
        "policy": {
            "relay_enabled": True,
            "relay_strategy": "broadcast",
            "relay_effective_strategy": "local_only",
            "relay_p2p_attached": False,
            "p2p_ttl_hops": 2,
            "ws_max_sessions": 32,
            "session_ttl_ms": 10_000,
            "heartbeat_interval_ms": 5_000,
            "heartbeat_miss_tolerance": 3,
            "heartbeat_min_interval_ms": 1_000,
        },
    }


def test_connect_aggregate_rejects_missing_context_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient("http://node.test", session=session)

    with pytest.raises(ValueError, match="ToriiOperatorSigningContext"):
        client.get_connect_status()

    assert session.calls == []


def test_connect_aggregate_signs_exact_target_once_and_parses_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(payload=_aggregate_payload()))
    captured: List[bytes] = []
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(captured),
    )

    snapshot = client.get_connect_status()

    assert snapshot.enabled is True
    assert snapshot.sessions_total == 5
    assert snapshot.per_ip_sessions[0].ip == "192.0.2.1"
    assert snapshot.policy is not None
    assert snapshot.policy.ws_max_sessions == 32
    assert snapshot.policy.relay_strategy == "broadcast"
    assert snapshot.policy.p2p_ttl_hops == 2
    assert snapshot.sequence_violation_closes_total == 1
    assert snapshot.p2p_auth_failures_total == 5
    assert snapshot.p2p_session_claims_installed_total == 9
    assert snapshot.p2p_session_terminated_total == 12
    assert snapshot.policy.heartbeat_interval_ms == 5_000

    target = "/v1/connect/status/aggregate"
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["url"] == f"http://node.test{target}"
    assert call["params"] == {}
    assert call["data"] is None
    assert call["allow_redirects"] is False
    assert len(captured) == 1
    assert captured[0] == operator_network_request_signature_message(
        NETWORK_ID,
        "GET",
        target,
        b"",
        timestamp_ms=int(call["headers"]["X-Iroha-Operator-Timestamp-Ms"]),
        nonce=call["headers"]["X-Iroha-Operator-Nonce"],
    )
