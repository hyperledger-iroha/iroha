"""Atomic-private-settlement exact-route Python SDK tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
import requests
from client_test_support import CANONICAL_OWNER
from iroha_torii_client import (
    AtomicPrivateSettlementIdentifierV1,
    AtomicPrivateSettlementOperationV1,
    AtomicPrivateSettlementPreparedRequestV1,
    AtomicPrivateSettlementToriiErrorV1,
    ToriiCanonicalRequestAuth,
    ToriiClient,
)

FIXTURE_PATH = (
    Path(__file__).resolve().parents[3]
    / "fixtures"
    / "norito_rpc"
    / "atomic_private_settlement_sdk_v1.json"
)


class _ExactResponse(requests.Response):
    def __init__(self, payload: Any, *, status: int = 200) -> None:
        super().__init__()
        self.status_code = status
        self.was_closed = False
        self.headers["Content-Type"] = "application/json"
        self._content = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self._content_consumed = True

    def close(self) -> None:
        self.was_closed = True
        super().close()


class _ExactSession(requests.Session):
    def __init__(self, response: _ExactResponse) -> None:
        super().__init__()
        self.response = response
        self.calls: list[dict[str, Any]] = []

    @staticmethod
    def get_adapter(_url: str) -> requests.adapters.HTTPAdapter:
        return requests.adapters.HTTPAdapter(max_retries=0)

    def request(self, method: str, url: str, **kwargs: Any) -> requests.Response:
        self.calls.append({"method": method, "url": url, **kwargs})
        self.response.url = url
        self.response.history = []
        return self.response

    def send(
        self,
        request: requests.PreparedRequest,
        **kwargs: Any,
    ) -> requests.Response:
        self.calls.append(
            {
                "method": request.method,
                "url": request.url,
                "headers": dict(request.headers),
                "data": request.body,
                **kwargs,
            }
        )
        self.response.url = request.url
        self.response.history = []
        self.response.request = request
        return self.response


def _fixture() -> dict[str, Any]:
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


def test_shared_route_fixture_matches_python_operation_catalog() -> None:
    rows = _fixture()["request_routes"]
    for row in rows:
        operation = AtomicPrivateSettlementOperationV1[row["operation"]]
        assert operation.path == row["path"]
        assert operation.auth.name == row["auth"]
        assert operation.top_level_fields == frozenset(row["top_level_fields"])


def test_prepared_request_rejects_duplicate_fields_and_redacts_body() -> None:
    with pytest.raises(ValueError, match="strict JSON object"):
        AtomicPrivateSettlementPreparedRequestV1.from_native_prepared_json(
            AtomicPrivateSettlementOperationV1.PREPARE_VOTE,
            b'{"manifest":{},"manifest":{},"payload_digest":"x"}',
        )

    request = AtomicPrivateSettlementPreparedRequestV1.from_native_prepared_json(
        AtomicPrivateSettlementOperationV1.PREPARE_VOTE,
        b'{"manifest":{},"payload_digest":"x"}',
    )
    assert "payload_digest" not in repr(request)
    assert "[REDACTED]" in repr(request)
    request.close()
    with pytest.raises(RuntimeError, match="closed"):
        request.bytes()


def test_public_receipt_is_path_bound_bounded_and_allowlisted() -> None:
    fixture = _fixture()
    bundle = AtomicPrivateSettlementIdentifierV1(fixture["identifiers"]["bundle_hex"])
    response = _ExactResponse(fixture["responses"]["receipt_pending"])
    session = _ExactSession(response)
    client = ToriiClient("https://node.test", session=session)

    result = client.private_settlement_bundle_receipt_v1(bundle)
    assert json.loads(result.bytes()) == fixture["responses"]["receipt_pending"]
    assert session.calls[0]["allow_redirects"] is False
    assert session.calls[0]["stream"] is True
    assert response.was_closed
    result.close()
    with pytest.raises(RuntimeError, match="closed"):
        result.bytes()


def test_sponsor_phase_certificate_recovery_is_bound_and_strictly_allowlisted() -> None:
    fixture = _fixture()
    payload = AtomicPrivateSettlementIdentifierV1(fixture["identifiers"]["payload_hex"])
    response = _ExactResponse(fixture["responses"]["phase_certificates"])
    session = _ExactSession(response)
    client = ToriiClient("https://node.test", session=session)
    signed_messages: list[bytes] = []
    auth = ToriiCanonicalRequestAuth(
        network_id=fixture["identifiers"]["bundle_json"],
        account_id=CANONICAL_OWNER,
        signer=lambda message: signed_messages.append(message) or b"\x55" * 64,
        timestamp_ms=1_700_000_000_000,
        nonce="settlement-phase-certificate-recovery-1",
    )

    result = client.private_settlement_phase_certificates_v1(
        payload,
        canonical_auth=auth,
    )

    assert json.loads(result.bytes()) == fixture["responses"]["phase_certificates"]
    assert len(signed_messages) == 1
    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["url"].endswith(
        f"/v1/nexus/private-settlements/legs/{payload.path_component}/phase-certificates"
    )
    assert "X-Iroha-Signature" in session.calls[0]["headers"]
    assert "X-Iroha-Operator-Signature" not in session.calls[0]["headers"]
    assert "[REDACTED]" in repr(result)

    missing = dict(fixture["responses"]["phase_certificates"])
    missing.pop("commit_certificate")
    with pytest.raises(AtomicPrivateSettlementToriiErrorV1, match="response is invalid"):
        ToriiClient(
            "https://node.test",
            session=_ExactSession(_ExactResponse(missing)),
        ).private_settlement_phase_certificates_v1(payload, canonical_auth=auth)

    non_object = dict(fixture["responses"]["phase_certificates"])
    non_object["prepare_certificate"] = []
    with pytest.raises(AtomicPrivateSettlementToriiErrorV1, match="response is invalid"):
        ToriiClient(
            "https://node.test",
            session=_ExactSession(_ExactResponse(non_object)),
        ).private_settlement_phase_certificates_v1(payload, canonical_auth=auth)

    leaked = dict(fixture["responses"]["phase_certificates"])
    leaked["plaintext"] = "LEAK_CANARY"
    with pytest.raises(AtomicPrivateSettlementToriiErrorV1) as caught:
        ToriiClient(
            "https://node.test",
            session=_ExactSession(_ExactResponse(leaked)),
        ).private_settlement_phase_certificates_v1(payload, canonical_auth=auth)
    assert "LEAK_CANARY" not in str(caught.value)


def test_response_from_substituted_url_fails_without_leaking_body() -> None:
    fixture = _fixture()
    bundle = AtomicPrivateSettlementIdentifierV1(fixture["identifiers"]["bundle_hex"])
    response = _ExactResponse(fixture["responses"]["receipt_pending"])
    session = _ExactSession(response)

    def substituted_request(method: str, url: str, **kwargs: Any) -> requests.Response:
        session.calls.append({"method": method, "url": url, **kwargs})
        response.url = "https://attacker.invalid/substituted"
        response.history = []
        return response

    session.request = substituted_request  # type: ignore[method-assign]
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(
        AtomicPrivateSettlementToriiErrorV1,
        match="provenance is invalid",
    ) as caught:
        client.private_settlement_bundle_receipt_v1(bundle)
    assert fixture["identifiers"]["bundle_json"] not in str(caught.value)
