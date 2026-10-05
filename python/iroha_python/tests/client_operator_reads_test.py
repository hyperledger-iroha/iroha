"""Exact-network operator authentication tests for node-local GET helpers."""

from __future__ import annotations

import base64
import json
from typing import Any, Callable

import pytest
import requests
from iroha_torii_client.client import canonical_request_message
from requests.adapters import HTTPAdapter

from iroha_python import (
    NetworkId,
    OperatorSigningContext,
    SumeragiEvidenceAppliedPenaltyStatus,
    SumeragiEvidenceCount,
    SumeragiEvidenceListPage,
    SumeragiEvidencePendingPenaltyStatus,
    SumeragiEvidenceRecord,
    ToriiClient,
)
from iroha_python.crypto import Ed25519KeyPair

NETWORK_BYTES = bytes([0xA5]) * 32
NETWORK_ID = NetworkId.from_bytes(NETWORK_BYTES)
FOREIGN_NETWORK_ID = NetworkId.from_bytes(bytes([0xA7]) * 32)
KEY_PAIR = Ed25519KeyPair.from_private_key(bytes([0x0B]) * 32)


def evidence_record_payload(*, penalty_status: dict[str, Any] | None = None) -> dict[str, Any]:
    """Return one exact first-release evidence projection."""

    return {
        "kind": "NativeSumeragiEvidence",
        "class": "phase_vote",
        "height": 31,
        "epoch": 2,
        "context_id": "11" * 32,
        "instance": "22" * 32,
        "authority_generation": "33" * 32,
        "offenders": [{"signer": 3, "peer_id": "ea013082F39DD89C3AA5C497C7C1843C21C117CF77E7A569E80B827B401A275179DD57F67CA52601BF4C127F848D71740A5D08"}],
        "safety_violation": False,
        "native_frame_hash": "44" * 32,
        "recorded_height": 40,
        "recorded_view": 2,
        "recorded_ms": 1_700_000_000_000,
        "consensus_admitted_height": 41,
        "penalty_status": penalty_status
        if penalty_status is not None
        else {"status": "pending", "details": None},
    }


def test_sumeragi_evidence_models_parse_the_closed_contract() -> None:
    page = SumeragiEvidenceListPage.from_payload(
        {
            "total": 3,
            "items": [
                evidence_record_payload(),
                evidence_record_payload(
                    penalty_status={
                        "status": "applied",
                        "details": {"height": 42},
                    }
                ),
                evidence_record_payload(
                    penalty_status={
                        "status": "applied",
                        "details": {"height": 43},
                    }
                ),
            ],
        }
    )

    assert isinstance(page.items[0].penalty_status, SumeragiEvidencePendingPenaltyStatus)
    assert isinstance(page.items[1].penalty_status, SumeragiEvidenceAppliedPenaltyStatus)
    assert page.items[1].penalty_status.details.height == 42
    assert isinstance(page.items[2].penalty_status, SumeragiEvidenceAppliedPenaltyStatus)
    assert page.items[2].penalty_status.details.height == 43
    assert SumeragiEvidenceCount.from_payload({"count": 3}).count == 3


@pytest.mark.parametrize(
    "payload",
    [
        {"items": []},
        {"total": 0},
        {"total": 0, "items": [], "cursor": None},
        {"total": "0", "items": []},
    ],
)
def test_sumeragi_evidence_page_rejects_noncanonical_envelopes(
    payload: dict[str, Any],
) -> None:
    with pytest.raises((TypeError, ValueError)):
        SumeragiEvidenceListPage.from_payload(payload)


def test_sumeragi_evidence_page_rejects_impossible_or_oversized_results() -> None:
    record = evidence_record_payload()
    with pytest.raises(ValueError, match="at most 50"):
        SumeragiEvidenceListPage.from_payload({"total": 51, "items": [record] * 51})
    with pytest.raises(ValueError, match="cover offset"):
        SumeragiEvidenceListPage.from_payload(
            {"total": 1, "items": [record]},
            offset=1,
        )


def test_sumeragi_evidence_page_accepts_empty_page_beyond_total() -> None:
    page = SumeragiEvidenceListPage.from_payload(
        {"total": 1, "items": []},
        offset=10,
    )

    assert page.total == 1
    assert page.items == []


@pytest.mark.parametrize(
    "payload",
    [
        {"count": "1"},
        {},
        {"count": 1, "total": 1},
    ],
)
def test_sumeragi_evidence_count_rejects_noncanonical_envelopes(
    payload: dict[str, Any],
) -> None:
    with pytest.raises((TypeError, ValueError)):
        SumeragiEvidenceCount.from_payload(payload)


@pytest.mark.parametrize(
    ("mutate", "expected"),
    [
        (lambda value: value.update(kind="DoublePrepare"), "kind"),
        (lambda value: value.update(consensus_admitted_height=None), "consensus_admitted_height"),
        (lambda value: value.update(penalty_applied=False), "penalty_applied"),
        (
            lambda value: value.update(penalty_status={"status": "pending", "details": {}}),
            "details",
        ),
        (
            lambda value: value.update(penalty_status={"status": "retired", "details": None}),
            "status",
        ),
    ],
)
def test_sumeragi_evidence_record_rejects_retired_or_malformed_shapes(
    mutate: Callable[[dict[str, Any]], None],
    expected: str,
) -> None:
    payload = evidence_record_payload()
    mutate(payload)

    with pytest.raises((TypeError, ValueError), match=expected):
        SumeragiEvidenceRecord.from_payload(payload)


class RecordingSession(requests.Session):
    """Record requests and return a supplied response or an unavailable default."""

    def __init__(self, response: requests.Response | None = None) -> None:
        super().__init__()
        self.calls: list[dict[str, Any]] = []
        self.response = response

    def request(self, method: str | bytes, url: str | bytes, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        if self.response is not None:
            return self.response
        response = requests.Response()
        response.status_code = 503
        response._content = b""
        response._content_consumed = True
        return response


def signing_context(network_id: NetworkId = NETWORK_ID) -> OperatorSigningContext:
    return OperatorSigningContext(network_id, KEY_PAIR)


@pytest.mark.parametrize(
    ("kwargs", "expected"),
    [
        ({"limit": 0}, "limit"),
        ({"limit": 1_001}, "limit"),
        ({"offset": -1}, "offset"),
        ({"offset": 10_001}, "offset"),
        ({"kind": "DoublePrepare"}, "kind"),
    ],
)
def test_sumeragi_evidence_query_rejects_noncanonical_values_before_dispatch(
    kwargs: dict[str, Any],
    expected: str,
) -> None:
    session = RecordingSession()
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=signing_context(),
    )

    with pytest.raises((TypeError, ValueError), match=expected):
        client.list_sumeragi_evidence(**kwargs)

    assert session.calls == []


def _evidence_response(body: bytes, headers: dict[str, str]) -> requests.Response:
    response = requests.Response()
    response.status_code = 200
    response._content = body
    response._content_consumed = True
    response.headers.update(headers)
    return response


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

    response = _evidence_response(body, headers)
    session = RecordingSession(response)
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=signing_context(),
    )

    with pytest.raises(error_type, match=message):
        if route == "list":
            client.list_sumeragi_evidence()
        else:
            client.get_sumeragi_evidence_count()

    assert session.calls[0]["stream"] is True
    assert session.calls[0]["headers"]["Accept"] == "application/json"


OPERATOR_READS: tuple[tuple[str, Callable[[ToriiClient], object]], ...] = (
    ("/v1/configuration", lambda client: client.get_configuration()),
    ("/v1/peers", lambda client: client.list_peers()),
    ("/v1/time/status", lambda client: client.get_time_status()),
    ("/v1/pipeline/preflight", lambda client: client.get_pipeline_preflight()),
    ("/v1/pipeline/recovery/42", lambda client: client.get_pipeline_recovery(42)),
    ("/v1/sumeragi/status", lambda client: client.get_sumeragi_status()),
    ("/v1/sumeragi/status", lambda client: client.get_sumeragi_status_typed()),
    ("/v1/sumeragi/lanes", lambda client: client.get_sumeragi_lanes()),
    (
        "/v1/sumeragi/evidence/count",
        lambda client: client.get_sumeragi_evidence_count(),
    ),
    (
        "/v1/sumeragi/evidence?kind=NativeSumeragiEvidence&limit=2&offset=1",
        lambda client: client.list_sumeragi_evidence(
            limit=2,
            offset=1,
            kind="NativeSumeragiEvidence",
        ),
    ),
    ("/v1/sumeragi/params", lambda client: client.get_sumeragi_params()),
)


@pytest.mark.parametrize(("path", "invoke"), OPERATOR_READS)
def test_operator_reads_sign_exact_path_network_and_empty_body_once(
    path: str,
    invoke: Callable[[ToriiClient], object],
) -> None:
    session = RecordingSession()
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=signing_context(),
        max_retries=5,
        retry_on_methods=["GET"],
        retry_on_status=[503],
    )

    with pytest.raises(RuntimeError):
        invoke(client)

    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "GET"
    assert call["url"] == f"https://torii.example{path}"
    assert call["params"] is None
    assert call["data"] == b""
    assert call["allow_redirects"] is False
    headers = call["headers"]
    assert "Authorization" not in headers
    assert "X-API-Token" not in headers

    timestamp = headers["x-iroha-operator-timestamp-ms"]
    nonce = headers["x-iroha-operator-nonce"]
    signature = base64.b64decode(headers["x-iroha-operator-signature"], validate=True)
    canonical = canonical_request_message("GET", path, b"")
    local_message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            NETWORK_BYTES,
            canonical,
            f"\n{timestamp}\n{nonce}".encode("ascii"),
        )
    )
    foreign_message = b"".join(
        (
            b"iroha.operator.http-request.network.v1\0",
            bytes(FOREIGN_NETWORK_ID.to_bytes()),
            canonical,
            f"\n{timestamp}\n{nonce}".encode("ascii"),
        )
    )
    assert KEY_PAIR.verify(local_message, signature)
    assert not KEY_PAIR.verify(foreign_message, signature)


@pytest.mark.parametrize(("path", "invoke"), OPERATOR_READS)
def test_operator_reads_fail_before_dispatch_without_context(
    path: str,
    invoke: Callable[[ToriiClient], object],
) -> None:
    del path
    session = RecordingSession()
    client = ToriiClient("https://torii.example", session=session)

    with pytest.raises(ValueError, match="operator_signing_context"):
        invoke(client)

    assert session.calls == []


def test_operator_reads_reject_session_auth_and_adapter_retries_before_dispatch() -> None:
    header_session = RecordingSession()
    header_session.headers["Authorization"] = "Bearer retired"
    with pytest.raises(ValueError, match="session.headers.*Authorization"):
        ToriiClient(
            "https://torii.example",
            session=header_session,
            operator_signing_context=signing_context(),
        )
    assert header_session.calls == []

    auth_session = RecordingSession()
    auth_session.auth = ("retired-user", "retired-password")
    with pytest.raises(ValueError, match="session.auth"):
        ToriiClient(
            "https://torii.example",
            session=auth_session,
            operator_signing_context=signing_context(),
        )
    assert auth_session.calls == []

    retry_session = RecordingSession()
    retry_session.mount("https://", HTTPAdapter(max_retries=1))
    retry_client = ToriiClient(
        "https://torii.example",
        session=retry_session,
        operator_signing_context=signing_context(),
    )
    with pytest.raises(ValueError, match="retries to be disabled"):
        retry_client.get_time_status()
    assert retry_session.calls == []


def test_operator_reads_generate_a_fresh_nonce_for_each_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient(
        "https://torii.example",
        session=session,
        operator_signing_context=signing_context(),
    )

    for _ in range(2):
        with pytest.raises(RuntimeError):
            client.list_peers()

    assert len(session.calls) == 2
    assert (
        session.calls[0]["headers"]["x-iroha-operator-nonce"]
        != session.calls[1]["headers"]["x-iroha-operator-nonce"]
    )


@pytest.mark.parametrize("field", list(evidence_record_payload()))
def test_native_evidence_requires_each_exact_field(field: str) -> None:
    payload = evidence_record_payload()
    del payload[field]
    with pytest.raises((TypeError, ValueError), match=field):
        SumeragiEvidenceRecord.from_payload(payload)


@pytest.mark.parametrize("changes", [
    {"kind": "SumeragiEquivocation"},
    {"view": 4}, {"signer": 3}, {"artifact_hash_1": "22" * 32},
    {"offenders": []}, {"offenders": [{}]},
    {"offenders": [{"signer": 1024, "peer_id": "invalid"}]},
    {"offenders": [{"signer": 0, "peer_id": "invalid"}]},
    {"safety_violation": 1},
    {"authority_generation": "AB" * 32},
])
def test_native_evidence_rejects_retired_and_malformed_attribution(changes: dict[str, Any]) -> None:
    payload = evidence_record_payload()
    payload.update(changes)
    with pytest.raises((TypeError, ValueError)):
        SumeragiEvidenceRecord.from_payload(payload)


def test_native_evidence_rejects_duplicate_signers_and_peers() -> None:
    for duplicate_signer in (True, False):
        payload = evidence_record_payload()
        offender = dict(payload["offenders"][0])
        if not duplicate_signer:
            offender["signer"] += 1
        payload["offenders"].append(offender)
        with pytest.raises(ValueError):
            SumeragiEvidenceRecord.from_payload(payload)


@pytest.mark.parametrize("native_class", ["proposal", "phase_vote", "timeout_vote", "invalid_proposal", "conflicting_certificates"])
def test_native_evidence_accepts_each_native_class(native_class: str) -> None:
    payload = evidence_record_payload()
    payload["class"] = native_class
    assert SumeragiEvidenceRecord.from_payload(payload).class_ == native_class


def test_evidence_preserves_unattributed_certificate_safety_violation() -> None:
    payload = evidence_record_payload()
    payload.update({
        "class": "conflicting_certificates", "safety_violation": True, "offenders": [],
    })
    parsed = SumeragiEvidenceRecord.from_payload(payload)
    assert parsed.class_ == "conflicting_certificates"
    assert parsed.safety_violation is True
    assert parsed.offenders == ()
    assert parsed.native_frame_hash == payload["native_frame_hash"]


@pytest.mark.parametrize("evidence_class,safety_violation", [
    ("conflicting_certificates", False), ("conflicting_certificates", 1),
    ("phase_vote", True),
])
def test_empty_offenders_require_exact_certificate_safety_violation(
    evidence_class: str, safety_violation: Any,
) -> None:
    payload = evidence_record_payload()
    payload.update({
        "class": evidence_class, "safety_violation": safety_violation, "offenders": [],
    })
    with pytest.raises(ValueError, match="offenders must contain"):
        SumeragiEvidenceRecord.from_payload(payload)


@pytest.mark.parametrize("details", [{"height": 43}, None, {}, {"height": 43, "note": "x"}])
def test_sumeragi_evidence_models_reject_retired_cancelled_status(details: Any) -> None:
    payload = evidence_record_payload(
        penalty_status={"status": "cancelled", "details": details}
    )
    with pytest.raises(ValueError, match="status must be pending or applied"):
        SumeragiEvidenceRecord.from_payload(payload)
