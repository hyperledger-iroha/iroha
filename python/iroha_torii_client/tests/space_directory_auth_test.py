"""Exact-account and exact-network Space Directory draft tests."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

import pytest
from client_test_support import (
    CANONICAL_OWNER,
    CANONICAL_OWNER_HEADER,
    app_api_transaction_draft,
    canonical_hash,
)
from sumeragi_exact_json_test_support import RecordingSession, StubResponse

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (  # noqa: E402
    ToriiCanonicalRequestAuth,
    ToriiClient,
    ToriiLocalSigningContext,
    build_canonical_request_headers,
)

GOVERNANCE_NETWORK_ID = canonical_hash(0xA5)
PUBLIC_NETWORK_ID = "a5" * 32


def governance_auth(captured: Optional[List[bytes]] = None) -> ToriiCanonicalRequestAuth:
    def signer(message: bytes) -> bytes:
        if captured is not None:
            captured.append(message)
        return b"\x44" * 64

    return ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=signer,
    )


def test_public_network_id_context_matches_normalized_canonical_auth() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=200, payload=app_api_transaction_draft()))
    client = ToriiClient(
        "http://node.test",
        session=session,
        local_signing_context=ToriiLocalSigningContext(PUBLIC_NETWORK_ID),
    )
    auth = ToriiCanonicalRequestAuth(
        network_id=PUBLIC_NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=lambda _message: b"\x44" * 64,
    )

    client.revoke_space_directory_manifest(
        authority=CANONICAL_OWNER,
        uaid="uaid:" + "23" * 32,
        dataspace=3,
        revoked_epoch=4096,
        canonical_auth=auth,
    )

    assert auth.network_id == GOVERNANCE_NETWORK_ID
    assert len(session.calls) == 1


def test_publish_space_directory_manifest_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=200, payload=app_api_transaction_draft()))
    client = ToriiClient(
        "http://node.test",
        session=session,
        local_signing_context=ToriiLocalSigningContext(GOVERNANCE_NETWORK_ID),
    )
    signed_messages: List[bytes] = []
    auth = governance_auth(signed_messages)

    manifest: Dict[str, Any] = {
        "version": "V1",
        "uaid": "uaid:" + "11" * 32,
        "dataspace": 7,
        "entries": [
            {
                "scope": {"program": "cbdc.transfer"},
                "effect": {"Allow": {"max_amount": "10"}},
            }
        ],
    }
    response = client.publish_space_directory_manifest(
        authority=CANONICAL_OWNER,
        manifest=manifest,
        reason="demo",
        canonical_auth=auth,
    )

    assert response.submitted is False
    assert session.calls[0]["method"] == "POST"
    assert session.calls[0]["url"].endswith("/v1/space-directory/manifests")
    assert session.calls[0]["headers"]["Content-Type"] == "application/json"
    assert session.calls[0]["headers"]["X-Iroha-Account"] == CANONICAL_OWNER_HEADER
    assert session.calls[0]["allow_redirects"] is False
    assert len(signed_messages) == 1
    body = json.loads(session.calls[0]["data"])
    assert body["authority"] == CANONICAL_OWNER
    assert "private_key" not in body
    assert body["reason"] == "demo"
    assert body["manifest"]["entries"][0]["scope"]["program"] == "cbdc.transfer"

    manifest["entries"][0]["scope"]["program"] = "mutated"
    assert body["manifest"]["entries"][0]["scope"]["program"] == "cbdc.transfer"


def test_revoke_space_directory_manifest_posts_payload() -> None:
    session = RecordingSession()
    session.queue(StubResponse(status_code=200, payload=app_api_transaction_draft()))
    client = ToriiClient(
        "http://node.test",
        session=session,
        local_signing_context=ToriiLocalSigningContext(GOVERNANCE_NETWORK_ID),
    )

    result = client.revoke_space_directory_manifest(
        authority=CANONICAL_OWNER,
        uaid="UAID:" + "23" * 32,
        dataspace=3,
        revoked_epoch=4096,
        reason="audit",
        canonical_auth=governance_auth(),
    )

    assert result.submitted is False
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"].endswith("/v1/space-directory/manifests/revoke")
    assert call["allow_redirects"] is False
    payload = json.loads(call["data"])
    assert "private_key" not in payload
    assert payload["uaid"] == "uaid:" + "23" * 32
    assert payload["dataspace"] == 3
    assert payload["revoked_epoch"] == 4096
    assert payload["reason"] == "audit"


@pytest.mark.parametrize(
    "method_name, kwargs",
    [
        (
            "publish_space_directory_manifest",
            {"manifest": {"version": "V1", "entries": []}},
        ),
        (
            "revoke_space_directory_manifest",
            {
                "uaid": "uaid:" + "23" * 32,
                "dataspace": 3,
                "revoked_epoch": 4096,
            },
        ),
    ],
)
def test_space_directory_drafts_reject_authority_substitution_before_dispatch(
    method_name: str,
    kwargs: Dict[str, Any],
) -> None:
    session = RecordingSession()
    client = ToriiClient(
        "http://node.test",
        session=session,
        local_signing_context=ToriiLocalSigningContext(GOVERNANCE_NETWORK_ID),
    )
    foreign_auth = ToriiCanonicalRequestAuth(
        network_id=GOVERNANCE_NETWORK_ID,
        account_id="sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY7",
        signer=lambda _message: b"signature",
    )

    with pytest.raises(ValueError, match="must equal the exact payload authority"):
        getattr(client, method_name)(
            authority=CANONICAL_OWNER,
            canonical_auth=foreign_auth,
            **kwargs,
        )

    assert session.calls == []


def test_space_directory_signature_is_bound_to_exact_genesis() -> None:
    body = json.dumps(
        {
            "authority": CANONICAL_OWNER,
            "manifest": {"entries": [], "version": "V1"},
        },
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    common = {
        "account_id": CANONICAL_OWNER,
        "signer": lambda message: hashlib.sha256(message).digest(),
        "method": "POST",
        "path": "/v1/space-directory/manifests",
        "body": body,
        "timestamp_ms": 123,
        "nonce": "space-directory-network-binding",
    }

    local = build_canonical_request_headers(
        network_id=GOVERNANCE_NETWORK_ID,
        **common,
    )
    foreign = build_canonical_request_headers(
        network_id=canonical_hash(0xA6),
        **common,
    )

    assert local["X-Iroha-Signature"] != foreign["X-Iroha-Signature"]


def test_space_directory_rejects_same_label_foreign_genesis_before_dispatch() -> None:
    session = RecordingSession()
    client = ToriiClient(
        "http://node.test",
        session=session,
        local_signing_context=ToriiLocalSigningContext(GOVERNANCE_NETWORK_ID),
    )
    foreign_auth = ToriiCanonicalRequestAuth(
        network_id=canonical_hash(0xA6),
        account_id=CANONICAL_OWNER,
        signer=lambda _message: b"signature",
    )

    with pytest.raises(ValueError, match="must match the immutable local_signing_context"):
        client.revoke_space_directory_manifest(
            authority=CANONICAL_OWNER,
            uaid="uaid:" + "23" * 32,
            dataspace=3,
            revoked_epoch=4096,
            canonical_auth=foreign_auth,
        )

    assert session.calls == []
