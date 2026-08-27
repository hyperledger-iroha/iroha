"""Focused exact-network authentication coverage for Kaigi relay reads."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path
from typing import List, Optional
from urllib.parse import quote

import pytest
import requests
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
HPKE_PUBLIC_KEY = b"ABCD"
HPKE_PUBLIC_KEY_B64 = "QUJDRA=="
HPKE_FINGERPRINT_HEX = "58c7dab691f514e0bd6f4082852ac0f1e08df24b5864038ff70ecd68419f4a23"


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
        "relay_id": CANONICAL_OWNER,
        "domain": "kaigi.core",
        "bandwidth_class": 3,
        "hpke_fingerprint_hex": "ab" * 32,
        "status": "healthy",
        "reported_at_ms": 123,
    }
    payload.update(overrides)
    return payload


def _relay_detail_payload(**overrides: object) -> dict[str, object]:
    payload: dict[str, object] = {
        "relay": _relay_summary_payload(
            hpke_fingerprint_hex=HPKE_FINGERPRINT_HEX,
        ),
        "hpke_public_key_b64": HPKE_PUBLIC_KEY_B64,
        "reported_call": {"domain_id": "kaigi.core", "call_name": "health"},
        "reported_by": CANONICAL_OWNER,
    }
    payload.update(overrides)
    return payload


def test_kaigi_hpke_fingerprint_matches_iroha_hash_new_fixture() -> None:
    raw_digest = hashlib.blake2b(HPKE_PUBLIC_KEY, digest_size=32).digest()

    assert raw_digest.hex() == ("58c7dab691f514e0bd6f4082852ac0f1e08df24b5864038ff70ecd68419f4a22")
    assert raw_digest[-1] & 1 == 0
    detail = ToriiClient._parse_kaigi_relay_detail(
        _relay_detail_payload(),
        context="kaigi relay detail",
    )
    assert detail.relay.hpke_fingerprint_hex == HPKE_FINGERPRINT_HEX


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
    with pytest.raises(RuntimeError, match=r"(?:64 lowercase hex characters|string)"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(hpke_fingerprint_hex=fingerprint),
            context="kaigi relay summary",
        )


def test_kaigi_relay_summary_requires_the_iroha_hash_marker() -> None:
    with pytest.raises(RuntimeError, match="Iroha Hash marker bit"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(hpke_fingerprint_hex="aa" * 32),
            context="kaigi relay summary",
        )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("reported_at_ms", "123"),
        ("status", "HEALTHY"),
        ("hpke_fingerprint_hex", "AB" * 32),
    ],
)
def test_kaigi_relay_summary_rejects_noncanonical_wire_values(
    field: str,
    value: object,
) -> None:
    with pytest.raises(RuntimeError):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(**{field: value}),
            context="kaigi relay summary",
        )


@pytest.mark.parametrize("missing", ["status", "reported_at_ms"])
def test_kaigi_relay_summary_requires_health_fields_as_a_pair(missing: str) -> None:
    payload = _relay_summary_payload()
    del payload[missing]

    with pytest.raises(RuntimeError, match="present together"):
        ToriiClient._parse_kaigi_relay_summary(
            payload,
            context="kaigi relay summary",
        )


def test_kaigi_relay_summary_rejects_unknown_fields() -> None:
    with pytest.raises(RuntimeError, match="not part of the first-release contract"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(alias_tag="private"),
            context="kaigi relay summary",
        )


def test_kaigi_relay_summary_requires_canonical_i105_output() -> None:
    with pytest.raises(RuntimeError, match="canonical I105"):
        ToriiClient._parse_kaigi_relay_summary(
            _relay_summary_payload(relay_id="relay-alpha"),
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


def test_kaigi_relay_summary_list_rejects_partial_or_duplicate_results() -> None:
    item = _relay_summary_payload()
    with pytest.raises(RuntimeError, match="total must equal"):
        ToriiClient._parse_kaigi_relay_summary_list(
            {"total": 2, "items": [item]},
            context="kaigi relay summary response",
        )
    with pytest.raises(RuntimeError, match="duplicate relay ids"):
        ToriiClient._parse_kaigi_relay_summary_list(
            {"total": 2, "items": [item, item]},
            context="kaigi relay summary response",
        )


def test_kaigi_relay_detail_rejects_non_string_notes() -> None:
    with pytest.raises(RuntimeError, match=r"notes must be a string"):
        ToriiClient._parse_kaigi_relay_detail(
            _relay_detail_payload(notes=7),
            context="kaigi relay detail",
        )


@pytest.mark.parametrize("public_key", ["not-base64", "", []])
def test_kaigi_relay_detail_requires_exact_base64_public_key(public_key: object) -> None:
    with pytest.raises((TypeError, ValueError, RuntimeError), match=r"(?:string|empty|base64)"):
        ToriiClient._parse_kaigi_relay_detail(
            _relay_detail_payload(hpke_public_key_b64=public_key),
            context="kaigi relay detail",
        )


def test_kaigi_relay_detail_binds_public_key_to_fingerprint() -> None:
    with pytest.raises(RuntimeError, match="does not match the relay fingerprint"):
        ToriiClient._parse_kaigi_relay_detail(
            _relay_detail_payload(relay=_relay_summary_payload(hpke_fingerprint_hex="ab" * 32)),
            context="kaigi relay detail",
        )


def test_kaigi_relay_detail_preserves_empty_notes() -> None:
    detail = ToriiClient._parse_kaigi_relay_detail(
        _relay_detail_payload(notes=""),
        context="kaigi relay detail",
    )

    assert detail.notes == ""


def test_kaigi_relay_detail_metrics_must_match_relay_domain() -> None:
    with pytest.raises(RuntimeError, match="metrics.domain must match"):
        ToriiClient._parse_kaigi_relay_detail(
            _relay_detail_payload(
                metrics={
                    "domain": "other.core",
                    "registrations_total": 0,
                    "manifest_updates_total": 0,
                    "failovers_total": 0,
                    "health_reports_total": 0,
                }
            ),
            context="kaigi relay detail",
        )


def test_kaigi_relay_detail_requires_consistent_feedback_fields() -> None:
    payload = _relay_detail_payload()
    del payload["reported_by"]
    with pytest.raises(RuntimeError, match="present together"):
        ToriiClient._parse_kaigi_relay_detail(
            payload,
            context="kaigi relay detail",
        )

    relay_without_feedback = _relay_summary_payload(hpke_fingerprint_hex=HPKE_FINGERPRINT_HEX)
    del relay_without_feedback["status"]
    del relay_without_feedback["reported_at_ms"]
    with pytest.raises(RuntimeError, match="agree with the relay health summary"):
        ToriiClient._parse_kaigi_relay_detail(
            _relay_detail_payload(relay=relay_without_feedback),
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


def test_kaigi_relay_health_rejects_impossible_current_status_total() -> None:
    with pytest.raises(RuntimeError, match="status totals exceed"):
        ToriiClient._parse_kaigi_relay_health_snapshot(
            {
                "healthy_total": 501,
                "degraded_total": 0,
                "unavailable_total": 0,
                "reports_total": 0,
                "registrations_total": 0,
                "failovers_total": 0,
                "domains": [],
            },
            context="kaigi relay health snapshot",
        )


def test_kaigi_relay_health_requires_strictly_sorted_domains() -> None:
    with pytest.raises(RuntimeError, match="strictly sorted by domain"):
        ToriiClient._parse_kaigi_relay_health_snapshot(
            {
                "healthy_total": 0,
                "degraded_total": 0,
                "unavailable_total": 0,
                "reports_total": 3,
                "registrations_total": 3,
                "failovers_total": 3,
                "domains": [
                    {
                        "domain": "zeta",
                        "registrations_total": 1,
                        "manifest_updates_total": 0,
                        "failovers_total": 1,
                        "health_reports_total": 1,
                    },
                    {
                        "domain": "alpha",
                        "registrations_total": 2,
                        "manifest_updates_total": 0,
                        "failovers_total": 2,
                        "health_reports_total": 2,
                    },
                ],
            },
            context="kaigi relay health snapshot",
        )


def test_list_kaigi_relays_parses_summary() -> None:
    signed_messages: List[bytes] = []
    session = RecordingSession()
    response = StubResponse(
        payload={
            "total": 1,
            "items": [
                {
                    "relay_id": CANONICAL_OWNER,
                    "domain": "kaigi.core",
                    "bandwidth_class": 3,
                    "hpke_fingerprint_hex": "ab" * 32,
                    "status": "healthy",
                    "reported_at_ms": 123,
                }
            ],
        }
    )
    session.queue(response)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(signed_messages),
    )

    summary = client.list_kaigi_relays()

    assert summary.total == 1
    assert len(summary.items) == 1
    relay = summary.items[0]
    assert relay.relay_id == CANONICAL_OWNER
    assert relay.status == "healthy"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays")
    assert session.calls[0]["headers"]["Accept"] == "application/json"
    assert session.calls[0]["allow_redirects"] is False
    assert session.calls[0]["stream"] is True
    assert response.was_closed is True
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
    not_found_response = StubResponse(status_code=404)
    detail_response = StubResponse(
        payload={
            "relay": {
                "relay_id": relay_id,
                "domain": "kaigi.core",
                "bandwidth_class": 3,
                "hpke_fingerprint_hex": HPKE_FINGERPRINT_HEX,
                "status": "healthy",
                "reported_at_ms": 123,
            },
            "hpke_public_key_b64": HPKE_PUBLIC_KEY_B64,
            "reported_call": {"domain_id": "kaigi.core", "call_name": "register"},
            "reported_by": relay_id,
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
    session.queue(not_found_response)
    session.queue(detail_response)
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
    assert [call["stream"] for call in session.calls] == [True, True]
    assert not_found_response.was_closed is True
    assert detail_response.was_closed is True


def test_get_kaigi_relays_health_snapshot() -> None:
    session = RecordingSession()
    response = StubResponse(
        payload={
            "healthy_total": 2,
            "degraded_total": 1,
            "unavailable_total": 0,
            "reports_total": 4,
            "registrations_total": 5,
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
    session.queue(response)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    snapshot = client.get_kaigi_relays_health()

    assert snapshot.healthy_total == 2
    assert snapshot.domains[0].domain == "kaigi.core"
    assert session.calls[0]["url"].endswith("/v1/kaigi/relays/health")
    assert session.calls[0]["stream"] is True
    assert response.was_closed is True


@pytest.mark.parametrize("content_length", ["33", "1", None])
def test_kaigi_relay_reads_enforce_declared_and_actual_response_bounds(
    content_length: Optional[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(client_module, "_KAIGI_RELAY_RESPONSE_MAX_BYTES", 32)
    headers = {"Content-Type": "application/json"}
    if content_length is not None:
        headers["Content-Length"] = content_length
    response = StubResponse(raw=b"x" * 33, headers=headers)
    session = RecordingSession()
    session.queue(response)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(ValueError, match="32-byte size bound"):
        client.list_kaigi_relays()

    assert response.was_closed is True
    assert len(session.calls) == 1
    assert session.calls[0]["stream"] is True


@pytest.mark.parametrize(
    ("raw", "message"),
    [
        (b"\xff", "UTF-8 JSON"),
        (b"[]", "JSON object"),
        (b'{"total":0,"total":0,"items":[]}', "duplicate field"),
    ],
)
def test_kaigi_relay_reads_require_strict_json_objects(raw: bytes, message: str) -> None:
    response = StubResponse(raw=raw, headers={"Content-Type": "application/json"})
    session = RecordingSession()
    session.queue(response)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(ValueError, match=message):
        client.list_kaigi_relays()

    assert response.was_closed is True


def test_kaigi_relay_error_response_is_bounded_and_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(client_module, "_KAIGI_RELAY_RESPONSE_MAX_BYTES", 16)
    response = StubResponse(status_code=503, raw=b"x" * 17)
    session = RecordingSession()
    session.queue(response)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(ValueError, match="16-byte size bound"):
        client.list_kaigi_relays()

    assert response.was_closed is True
    assert len(session.calls) == 1


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

    cookie_session = RecordingSession()
    cookie_client = ToriiClient(
        "http://node.test",
        session=cookie_session,
        operator_signing_context=_operator_context(),
    )
    cookie_session.cookies.set("session", "ambient-authority")
    with pytest.raises(ValueError, match="Session.cookies"):
        cookie_client.list_kaigi_relays()
    assert cookie_session.calls == []


def test_kaigi_relay_reads_reject_retired_iso_profile_before_signing() -> None:
    signed_messages: List[bytes] = []
    session = RecordingSession()
    session.headers["X-Iroha-Iso-Profile"] = "legacy-profile"
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(signed_messages),
    )

    with pytest.raises(ValueError, match="X-Iroha-Iso-Profile"):
        client.list_kaigi_relays()

    assert signed_messages == []
    assert session.calls == []


def test_kaigi_relay_reads_reject_ambient_netrc_before_signing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    signed_messages: List[bytes] = []
    session = RecordingSession()
    original_headers = dict(session.headers)
    monkeypatch.setattr(
        requests.sessions,
        "get_netrc_auth",
        lambda *_args, **_kwargs: ("ambient-user", "ambient-secret"),
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(signed_messages),
    )

    with pytest.raises(
        ValueError,
        match="prepared transport authentication header Authorization",
    ):
        client.list_kaigi_relays()

    assert signed_messages == []
    assert session.calls == []
    assert session.trust_env is True
    assert session.auth is None
    assert dict(session.headers) == original_headers


def test_kaigi_relay_reads_reject_ambient_proxy_before_signing(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    signed_messages: List[bytes] = []
    session = RecordingSession()
    original_proxies = dict(session.proxies)
    monkeypatch.setattr(
        requests.sessions,
        "get_environ_proxies",
        lambda *_args, **_kwargs: {"http": "http://ambient-proxy.test:8080"},
    )
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(signed_messages),
    )

    with pytest.raises(ValueError, match="ambient environment proxies"):
        client.list_kaigi_relays()

    assert signed_messages == []
    assert session.calls == []
    assert session.trust_env is True
    assert session.proxies == original_proxies


def test_kaigi_relay_reads_reject_configured_proxy_auth_before_signing() -> None:
    signed_messages: List[bytes] = []
    session = RecordingSession()
    session.trust_env = False
    session.proxies["http"] = "http://proxy-user:proxy-secret@proxy.test:8080"
    original_proxies = dict(session.proxies)
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(signed_messages),
    )

    with pytest.raises(ValueError, match="proxy authentication"):
        client.list_kaigi_relays()

    assert signed_messages == []
    assert session.calls == []
    assert session.proxies == original_proxies


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


def test_kaigi_detail_empty_success_is_not_treated_as_not_found() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=200))
    client = ToriiClient(
        "http://node.test",
        session=session,
        operator_signing_context=_operator_context(),
    )

    with pytest.raises(RuntimeError, match="empty success response"):
        client.get_kaigi_relay(CANONICAL_OWNER)
