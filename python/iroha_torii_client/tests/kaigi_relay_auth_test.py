"""Focused exact-network authentication coverage for Kaigi relay reads."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import List, Optional
from urllib.parse import quote

import pytest

from client_test_support import CANONICAL_OWNER, canonical_hash
from sumeragi_exact_json_test_support import RecordingSession, StubResponse

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

import iroha_torii_client.client as client_module  # noqa: E402
from iroha_torii_client import (  # noqa: E402
    ToriiClient,
    ToriiOperatorSigningContext,
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


def _relay_summary_payload(**overrides: object) -> dict[str, object]:
    payload: dict[str, object] = {
        "relay_id": "relay-alpha",
        "domain": "kaigi.core",
        "bandwidth_class": 3,
        "hpke_fingerprint_hex": "ab" * 32,
        "status": "healthy",
        "reported_at_ms": 123,
    }
    payload.update(overrides)
    return payload


@pytest.mark.parametrize("bandwidth_class", [1, 255])
def test_kaigi_relay_summary_accepts_u8_bandwidth_boundaries(
    bandwidth_class: int,
) -> None:
    summary = ToriiClient._parse_kaigi_relay_summary(
        _relay_summary_payload(bandwidth_class=bandwidth_class),
        context="kaigi relay summary",
    )

    assert summary.bandwidth_class == bandwidth_class


@pytest.mark.parametrize("bandwidth_class", [0, -1, 256])
def test_kaigi_relay_summary_rejects_out_of_range_bandwidth(
    bandwidth_class: int,
) -> None:
    with pytest.raises(RuntimeError, match=r"bandwidth_class must be within 1\.\.=255"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(bandwidth_class=bandwidth_class),
            context="kaigi relay summary",
        )


def test_kaigi_relay_summary_requires_bandwidth_class() -> None:
    payload = _relay_summary_payload()
    del payload["bandwidth_class"]

    with pytest.raises(RuntimeError, match="bandwidth_class is required"):
        ToriiClient._parse_kaigi_relay_summary(payload, context="kaigi relay summary")


def test_kaigi_relay_summary_rejects_boolean_bandwidth() -> None:
    with pytest.raises(RuntimeError, match="bandwidth_class must be an integer"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(bandwidth_class=True),
            context="kaigi relay summary",
        )


@pytest.mark.parametrize("reported_at_ms", [True, 1.5, -1, 1 << 64])
def test_kaigi_relay_summary_rejects_lossy_or_out_of_range_timestamps(
    reported_at_ms: object,
) -> None:
    with pytest.raises(RuntimeError, match=r"reported_at_ms must"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(reported_at_ms=reported_at_ms),
            context="kaigi relay summary",
        )


@pytest.mark.parametrize("fingerprint", ["ab", ["ab"]])
def test_kaigi_relay_summary_requires_a_32_byte_fingerprint(fingerprint: object) -> None:
    with pytest.raises(RuntimeError, match=r"(?:64 hex characters|hex string)"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(hpke_fingerprint_hex=fingerprint),
            context="kaigi relay summary",
        )


@pytest.mark.parametrize(
    "payload",
    [
        {"total": 0, "items": False},
        {"items": []},
        {"total": 501, "items": [{}] * 501},
    ],
)
def test_kaigi_relay_summary_list_rejects_malformed_or_oversized_envelopes(
    payload: dict[str, object],
) -> None:
    with pytest.raises(RuntimeError, match=r"(?:items|total).*(?:list|required|limit)"):
        ToriiClient._parse_kaigi_relay_summary_list(
            payload,
            context="kaigi relay summary response",
        )


def test_kaigi_relay_detail_rejects_non_string_notes() -> None:
    with pytest.raises(RuntimeError, match=r"notes must be a string"):
        ToriiClient._parse_kaigi_relay_detail(
            {
                "relay": _relay_summary_payload(),
                "hpke_public_key_b64": "QUJDRA==",
                "notes": 7,
            },
            context="kaigi relay detail",
        )


@pytest.mark.parametrize("public_key", ["not-base64", "", []])
def test_kaigi_relay_detail_requires_exact_base64_public_key(public_key: object) -> None:
    with pytest.raises((TypeError, ValueError, RuntimeError), match=r"(?:string|empty|base64)"):
        ToriiClient._parse_kaigi_relay_detail(
            {
                "relay": _relay_summary_payload(),
                "hpke_public_key_b64": public_key,
            },
            context="kaigi relay detail",
        )


@pytest.mark.parametrize(
    "payload",
    [
        {
            "healthy_total": 0,
            "degraded_total": 0,
            "unavailable_total": 0,
            "reports_total": 0,
            "registrations_total": 0,
            "failovers_total": 0,
            "domains": False,
        },
        {
            "healthy_total": 0,
            "degraded_total": 0,
            "unavailable_total": 0,
            "registrations_total": 0,
            "failovers_total": 0,
            "domains": [],
        },
    ],
)
def test_kaigi_relay_health_rejects_malformed_required_fields(
    payload: dict[str, object],
) -> None:
    with pytest.raises(RuntimeError, match=r"(?:domains|reports_total).*(?:list|required)"):
        ToriiClient._parse_kaigi_relay_health_snapshot(
            payload,
            context="kaigi relay health snapshot",
        )


def test_list_kaigi_relays_parses_summary() -> None:
    signed_messages: List[bytes] = []
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "total": 1,
                "items": [
                    {
                        "relay_id": "relay-alpha",
                        "domain": "kaigi.core",
                        "bandwidth_class": 3,
                        "hpke_fingerprint_hex": "ab" * 32,
                        "status": "healthy",
                        "reported_at_ms": 123,
                    }
                ],
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(signed_messages),
    )

    summary = client.list_kaigi_relays()

    assert summary.total == 1
    assert len(summary.items) == 1
    relay = summary.items[0]
    assert relay.relay_id == "relay-alpha"
    assert relay.status == "healthy"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays")
    assert session.calls[0]["headers"]["Accept"] == "application/json"
    assert session.calls[0]["allow_redirects"] is False
    assert "X-Iroha-Operator-Signature" in session.calls[0]["headers"]
    exact_prefix = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            bytes.fromhex(NETWORK_ID[5:69]),
            client_module.canonical_request_message("GET", "/v1/kaigi/relays", b""),
            b"\n",
        )
    )
    assert len(signed_messages) == 1
    assert signed_messages[0].startswith(exact_prefix)
    assert not signed_messages[0].startswith(
        exact_prefix.replace(
            bytes.fromhex(NETWORK_ID[5:69]),
            bytes([0xA6]) * 32,
            1,
        )
    )
    assert not signed_messages[0].startswith(
        exact_prefix.replace(b"/v1/kaigi/relays\n", b"/v1/kaigi/relays?format=json\n", 1)
    )


def test_get_kaigi_relay_returns_detail_and_none_on_404() -> None:
    relay_id = CANONICAL_OWNER
    session = RecordingSession()
    session.queue(StubResponse(status_code=404))
    session.queue(
        StubResponse(
            payload={
                "relay": {
                    "relay_id": relay_id,
                    "domain": "kaigi.core",
                    "bandwidth_class": 3,
                    "hpke_fingerprint_hex": "cd" * 32,
                },
                "hpke_public_key_b64": "QUJDRA==",
                "reported_call": {"domain_id": "kaigi.core", "call_name": "register"},
                "reported_by": "ops@example",
                "notes": "Primary relay",
                "metrics": {
                    "domain": "kaigi.core",
                    "registrations_total": 5,
                    "manifest_updates_total": 7,
                    "failovers_total": 1,
                    "health_reports_total": 9,
                },
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    assert client.get_kaigi_relay(relay_id) is None
    detail = client.get_kaigi_relay(relay_id)

    assert detail is not None
    assert detail.relay.domain == "kaigi.core"
    assert detail.metrics is not None and detail.metrics.failovers_total == 1
    assert detail.reported_call is not None
    assert detail.reported_call.call_name == "register"
    assert session.calls[1]["url"].endswith(f"/v1/kaigi/relays/{quote(relay_id, safe='')}")


def test_get_kaigi_relays_health_snapshot() -> None:
    session = RecordingSession()
    session.queue(
        StubResponse(
            payload={
                "healthy_total": 2,
                "degraded_total": 1,
                "unavailable_total": 0,
                "reports_total": 5,
                "registrations_total": 7,
                "failovers_total": 1,
                "domains": [
                    {
                        "domain": "kaigi.core",
                        "registrations_total": 5,
                        "manifest_updates_total": 3,
                        "failovers_total": 1,
                        "health_reports_total": 4,
                    }
                ],
            }
        )
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    snapshot = client.get_kaigi_relays_health()

    assert snapshot.healthy_total == 2
    assert snapshot.domains[0].domain == "kaigi.core"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays/health")


def test_kaigi_relay_reads_require_fresh_operator_auth_before_dispatch() -> None:
    missing_session = RecordingSession()
    missing = ToriiClient("http://node.test", session=missing_session)
    with pytest.raises(ValueError, match="ToriiOperatorSigningContext"):
        missing.list_kaigi_relays()
    assert missing_session.calls == []

    precomputed_session = RecordingSession()
    precomputed_session.headers["X-Iroha-Operator-Nonce"] = "precomputed"
    precomputed = ToriiClient(
        "http://node.test",
        session=precomputed_session,
        operator_signing_context=_operator_context(),
    )
    with pytest.raises(ValueError, match="precomputed operator authentication"):
        precomputed.get_kaigi_relays_health()
    assert precomputed_session.calls == []


def test_kaigi_relay_operator_read_is_one_shot() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=503, payload={"error": "unavailable"}))
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(RuntimeError):
        client.list_kaigi_relays()
    assert len(session.calls) == 1
    assert session.calls[0]["allow_redirects"] is False
