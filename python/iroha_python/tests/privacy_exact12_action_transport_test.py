"""Common fail-closed transport tests for all thirteen Exact12 operations."""

from __future__ import annotations

import json
from types import SimpleNamespace

import pytest
import requests

import iroha_python
import iroha_python.client as client_module
import iroha_python.crypto as crypto_module
from iroha_python import (
    AccountAddress,
    NetworkId,
    SorafsAliasPolicy,
    ToriiCanonicalRequestAuth,
)
from iroha_python.client import (
    PRIVACY_EXACT12_ACTION_OPERATIONS_V1,
    PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1,
    PRIVACY_LEDGER_EFFECT_KINDS_V1,
    LocalSigningContext,
    PrivacyActionOperationViewV1,
    PrivacyExact12ActionOperationV1,
    PrivacyExact12ActionRequestV1,
    ToriiClient,
)

NETWORK_ID = NetworkId.from_bytes(bytes([0x91]) * 32)
MANIFEST_DIGEST = bytes([0xA2]) * 32
TRANSACTION_HASH = bytes([0x31]) * 32
TRANSACTION_INTENT_DIGEST = bytes([0x32]) * 32
STATEMENT_DIGEST = bytes([0x33]) * 32
PROOF_ENVELOPE_HASH = bytes([0x34]) * 32
EXECUTION_MANIFEST_DIGEST = bytes([0xA3]) * 32
FINALIZED_BLOCK_HASH = bytes([0x35]) * 32
AUTOMATIC_AUTH = ToriiCanonicalRequestAuth(
    network_id=NETWORK_ID.literal,
    account_id=AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x11]) * 32,
    ).to_i105(0x02F1),
    signer=lambda _message: bytes([0x44]) * 64,
)
STATIC_AUTH = ToriiCanonicalRequestAuth(
    network_id=AUTOMATIC_AUTH.network_id,
    account_id=AUTOMATIC_AUTH.account_id,
    signer=AUTOMATIC_AUTH.signer,
    timestamp_ms=4_102_444_801_000,
    nonce="privacy-exact12-static-freshness",
)


class _FakeCrypto:
    def __init__(self, events: list[str]) -> None:
        self.events = events
        self.current_wire = b""
        self.authority_account_id = AUTOMATIC_AUTH.account_id

    def __getattr__(self, name: str):
        by_inspector = {
            spec.inspector: (operation, spec)
            for operation, spec in client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1.items()
        }
        if name not in by_inspector:
            raise AttributeError(name)
        operation, spec = by_inspector[name]

        def inspect(*args: object) -> dict[str, object]:
            expected_args = 2 if spec.inspector_requires_network_id else 1
            assert len(args) == expected_args
            assert args[0] == self.current_wire
            if spec.inspector_requires_network_id:
                assert args[1] == NETWORK_ID
            self.events.append(f"inspect:{operation}")
            return {
                "protocol_id": spec.protocol_id,
                "execution_classification": (
                    spec.inspection_execution_classification or spec.execution_mode
                ),
                "ledger_effect": (
                    None
                    if spec.ledger_effect_kind == "verification_only"
                    else spec.ledger_effect_kind
                ),
                "transaction_hash": TRANSACTION_HASH,
                "transaction_intent_digest": TRANSACTION_INTENT_DIGEST,
                "statement_digest": STATEMENT_DIGEST,
                "proof_envelope_hash": PROOF_ENVELOPE_HASH,
                **({"action_kind": spec.action_kind} if spec.action_kind is not None else {}),
            }

        return inspect

    def signed_transaction_envelope_from_versioned_v1(
        self,
        wire: object,
        network_id: object,
    ) -> SimpleNamespace:
        assert wire == self.current_wire
        assert network_id == NETWORK_ID
        self.events.append("reconstruct")
        return SimpleNamespace(
            signed_transaction_versioned=self.current_wire,
            authority=self.authority_account_id,
            hash=TRANSACTION_HASH,
        )


class _FakeManifest:
    manifest_digest = MANIFEST_DIGEST

    def __init__(
        self,
        operation: PrivacyExact12ActionOperationV1,
        events: list[str],
    ) -> None:
        self.operation = operation
        self.events = events

    def require_network_capability(self, protocol_id: str) -> dict[str, object]:
        spec = client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1[self.operation]
        assert protocol_id == spec.protocol_id
        self.events.append("gate")
        return {
            "protocol_id": spec.protocol_id,
            "operation_schemas": list(spec.manifest_operation_schemas),
            "execution_mode": spec.execution_mode,
            "manifest_digest": MANIFEST_DIGEST,
            "committed_height": 411,
        }


def _client(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[ToriiClient, _FakeCrypto, list[str]]:
    events: list[str] = []
    crypto = _FakeCrypto(events)
    monkeypatch.setattr(client_module, "_CRYPTO_MODULE", crypto)
    alias_policy = SorafsAliasPolicy(
        positive_ttl_secs=1,
        refresh_window_secs=1,
        hard_expiry_secs=1,
        negative_ttl_secs=1,
        revocation_ttl_secs=1,
        rotation_max_age_secs=1,
        successor_grace_secs=0,
        governance_grace_secs=0,
    )
    return (
        ToriiClient(
            "http://torii.invalid",
            sorafs_alias_policy=alias_policy,
            local_signing_context=LocalSigningContext(NETWORK_ID),
        ),
        crypto,
        events,
    )


def _submitted_operation_view(
    operation: PrivacyExact12ActionOperationV1 = "anonymous_pgc_payment_action_v1",
) -> PrivacyActionOperationViewV1:
    spec = client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1[operation]
    return PrivacyActionOperationViewV1(
        protocol_id=spec.protocol_id,
        operation_schema=operation,
        transaction_hash=TRANSACTION_HASH.hex(),
        transaction_intent_digest=TRANSACTION_INTENT_DIGEST.hex(),
        statement_digest=STATEMENT_DIGEST.hex(),
        proof_envelope_hash=PROOF_ENVELOPE_HASH.hex(),
        local_state="submitted",
        terminal_chain_state=None,
        committed_height=None,
        rejection_reason=None,
        ledger_effect_kind=spec.ledger_effect_kind,
        capability_manifest_digest=MANIFEST_DIGEST.hex(),
        capability_committed_height=411,
    )


def _authenticated_submitted_operation_view(
    client: ToriiClient,
    operation: PrivacyExact12ActionOperationV1 = "anonymous_pgc_payment_action_v1",
) -> PrivacyActionOperationViewV1:
    return client._bind_privacy_action_view_v1(  # noqa: SLF001 - provenance regression
        _submitted_operation_view(operation),
        NETWORK_ID,
    )


def _execution_receipt(
    operation: PrivacyExact12ActionOperationV1,
    *,
    admitted_at_height: int,
    finalized_height: int | None = None,
    finalized_block_hash: bytes = FINALIZED_BLOCK_HASH,
) -> dict[str, object]:
    spec = client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1[operation]
    finalized_height = admitted_at_height + 3 if finalized_height is None else finalized_height
    return {
        "version": 1,
        "network_id": bytes(NETWORK_ID.to_bytes()).hex(),
        "protocol_id": spec.protocol_id,
        "operation_schema": operation,
        "ledger_effect_kind": spec.ledger_effect_kind,
        "transaction_hash": TRANSACTION_HASH.hex(),
        "action_index": 0,
        "transaction_intent_digest": TRANSACTION_INTENT_DIGEST.hex(),
        "statement_digest": STATEMENT_DIGEST.hex(),
        "proof_envelope_hash": PROOF_ENVELOPE_HASH.hex(),
        "capability_manifest_digest": EXECUTION_MANIFEST_DIGEST.hex(),
        "capability_committed_height": 410,
        "admitted_at_height": admitted_at_height,
        "finalized_height": finalized_height,
        "finalized_block_hash": finalized_block_hash.hex(),
    }


def _install_canonical_submit_stub(
    monkeypatch: pytest.MonkeyPatch,
    client: ToriiClient,
    calls: list[tuple[bytes, object, object]] | None = None,
) -> None:
    def submit(
        wire: bytes,
        *,
        canonical_auth: object,
        timeout: object,
    ) -> None:
        assert canonical_auth is AUTOMATIC_AUTH
        if calls is not None:
            calls.append((wire, canonical_auth, timeout))

    monkeypatch.setattr(client, "_submit_signed_privacy_action_wire_v1", submit)


def test_exact12_request_union_is_closed_and_exported() -> None:
    assert len(PRIVACY_EXACT12_ACTION_OPERATIONS_V1) == 13
    assert len(set(PRIVACY_EXACT12_ACTION_OPERATIONS_V1)) == 13
    assert len(PRIVACY_LEDGER_EFFECT_KINDS_V1) == 10
    assert len(set(PRIVACY_LEDGER_EFFECT_KINDS_V1)) == 10
    assert {
        spec.ledger_effect_kind
        for spec in client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1.values()
    } == set(PRIVACY_LEDGER_EFFECT_KINDS_V1)
    assert tuple(client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1) == (
        PRIVACY_EXACT12_ACTION_OPERATIONS_V1
    )
    assert PRIVACY_EXACT12_ACTION_OPERATIONS_V1 is iroha_python.PRIVACY_EXACT12_ACTION_OPERATIONS_V1
    assert (
        PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1
        == iroha_python.PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1
        == 10 * 1024 * 1024
    )
    assert iroha_python.PrivacyExact12ActionRequestV1 is PrivacyExact12ActionRequestV1

    with pytest.raises(ValueError, match="one exact"):
        PrivacyExact12ActionRequestV1(
            "zk_ams_admission_" + "and_provisioning_v1",  # type: ignore[arg-type]
            b"wire",
        )
    with pytest.raises(ValueError, match="32 non-zero"):
        PrivacyExact12ActionRequestV1(
            "zk_ace_authorization_action_v1",
            b"wire",
            expected_manifest_digest=bytes(32),
        )
    with pytest.raises(ValueError, match="must not be empty"):
        PrivacyExact12ActionRequestV1(
            "zk_ace_authorization_action_v1",
            b"",
        )
    with pytest.raises(ValueError, match="10 MiB"):
        PrivacyExact12ActionRequestV1(
            "zk_ace_authorization_action_v1",
            bytes(PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1 + 1),
        )


def test_native_committed_result_projection_requires_and_preserves_block_height(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    projection = {
        "transaction_hash": TRANSACTION_HASH.hex(),
        "block_hash": (bytes([0x35]) * 32).hex(),
        "block_height": 19,
        "result_hash": (bytes([0x36]) * 32).hex(),
        "result_ok": True,
        "rejection_code": None,
        "rejection_message": None,
        "trigger_completion_count": 0,
    }
    native = SimpleNamespace(
        inspect_pipeline_transaction_details_json=lambda *_args: json.dumps(projection)
    )
    monkeypatch.setattr(crypto_module, "_crypto", native)

    inspected = crypto_module.inspect_pipeline_transaction_details(
        TRANSACTION_HASH.hex(),
        b"canonical-details",
    )
    assert inspected["block_height"] == 19

    del projection["block_height"]
    with pytest.raises(RuntimeError, match="invalid shape"):
        crypto_module.inspect_pipeline_transaction_details(
            TRANSACTION_HASH.hex(),
            b"canonical-details",
        )


def test_exact12_operation_view_rejects_forged_mappings_and_states() -> None:
    common: dict[str, object] = {
        "protocol_id": "anonymous-pgc-k-out-of-n-v1",
        "operation_schema": "anonymous_pgc_payment_action_v1",
        "transaction_hash": TRANSACTION_HASH.hex(),
        "transaction_intent_digest": TRANSACTION_INTENT_DIGEST.hex(),
        "statement_digest": STATEMENT_DIGEST.hex(),
        "proof_envelope_hash": PROOF_ENVELOPE_HASH.hex(),
        "local_state": "submitted",
        "terminal_chain_state": None,
        "committed_height": None,
        "rejection_reason": None,
        "ledger_effect_kind": "anonymous_pgc_account_state_transition",
        "capability_manifest_digest": MANIFEST_DIGEST.hex(),
        "capability_committed_height": 411,
    }
    assert PrivacyActionOperationViewV1(**common).local_state == "submitted"  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="protocol_id"):
        PrivacyActionOperationViewV1(  # type: ignore[arg-type]
            **{**common, "protocol_id": "pq-masp-stark-v0"}
        )
    with pytest.raises(ValueError, match="transaction_hash"):
        PrivacyActionOperationViewV1(  # type: ignore[arg-type]
            **{**common, "transaction_hash": "AB" * 32}
        )
    with pytest.raises(ValueError, match="terminal fields"):
        PrivacyActionOperationViewV1(  # type: ignore[arg-type]
            **{**common, "committed_height": 9}
        )
    with pytest.raises(ValueError, match="finalized execution evidence"):
        PrivacyActionOperationViewV1(  # type: ignore[arg-type]
            **{
                **common,
                "local_state": "terminal",
                "terminal_chain_state": "Applied",
                "committed_height": 411,
            }
        )
    with pytest.raises(ValueError, match="canonical non-empty reason"):
        PrivacyActionOperationViewV1(  # type: ignore[arg-type]
            **{
                **common,
                "local_state": "terminal",
                "terminal_chain_state": "Rejected",
                "committed_height": 411,
                "rejection_reason": " padded ",
            }
        )
    for hostile_reason in ("policy\u0001rejected", "é" * 513):
        with pytest.raises(ValueError, match="canonical non-empty reason"):
            PrivacyActionOperationViewV1(  # type: ignore[arg-type]
                **{
                    **common,
                    "local_state": "terminal",
                    "terminal_chain_state": "Rejected",
                    "committed_height": 411,
                    "rejection_reason": hostile_reason,
                }
            )


def test_status_rejects_detached_view_before_network_access(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: pytest.fail("detached view reached Torii"),
    )

    with pytest.raises(RuntimeError, match="authenticated submission"):
        client.get_privacy_action_status_v1(
            _submitted_operation_view(),
            canonical_auth=AUTOMATIC_AUTH,
        )


def test_id105_receipt_query_uses_canonical_auth_signer_and_exact_bindings(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    operation_name: PrivacyExact12ActionOperationV1 = "anonymous_pgc_payment_action_v1"
    operation = _authenticated_submitted_operation_view(client, operation_name)
    receipt = _execution_receipt(operation_name, admitted_at_height=612)
    query_calls: list[tuple[object, ...]] = []
    inspect_calls: list[tuple[object, ...]] = []
    request_calls: list[tuple[str, str, dict[str, object]]] = []

    def build_query(*args: object) -> bytes:
        query_calls.append(args)
        return b"signed-id105-query"

    def inspect(*args: object) -> dict[str, object]:
        inspect_calls.append(args)
        return receipt

    response = requests.Response()
    response.status_code = 200
    response.headers["Content-Type"] = "application/x-norito; charset=binary"
    response._content = b"canonical-id105-response"

    def request(method: str, path: str, **kwargs: object) -> requests.Response:
        request_calls.append((method, path, kwargs))
        return response

    monkeypatch.setattr(
        crypto_module,
        "build_find_privacy_action_execution_receipt_query_with_signer",
        build_query,
    )
    monkeypatch.setattr(
        crypto_module,
        "inspect_privacy_action_execution_receipt_response",
        inspect,
    )
    monkeypatch.setattr(client, "_request", request)

    observed = client.get_privacy_action_execution_receipt_v1(
        operation,
        canonical_auth=AUTOMATIC_AUTH,
        timeout=7.0,
    )

    assert observed == receipt
    assert query_calls == [
        (
            AUTOMATIC_AUTH.account_id,
            AUTOMATIC_AUTH.signer,
            NETWORK_ID,
            1,
            TRANSACTION_HASH.hex(),
            0,
        )
    ]
    assert inspect_calls == [
        (
            NETWORK_ID,
            1,
            TRANSACTION_HASH.hex(),
            0,
            TRANSACTION_INTENT_DIGEST,
            STATEMENT_DIGEST,
            PROOF_ENVELOPE_HASH,
            b"canonical-id105-response",
        )
    ]
    assert len(request_calls) == 1
    assert request_calls[0][0:2] == ("POST", "/v1/query")
    assert request_calls[0][2]["data"] == b"signed-id105-query"
    assert request_calls[0][2]["allow_retry"] is False
    assert request_calls[0][2]["allow_redirects"] is False


def test_id105_receipt_query_rejects_native_binding_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    operation_name: PrivacyExact12ActionOperationV1 = "verange_range_proof_v1"
    operation = _authenticated_submitted_operation_view(client, operation_name)
    response = requests.Response()
    response.status_code = 200
    response.headers["Content-Type"] = "application/x-norito"
    response._content = b"canonical-id105-response"
    monkeypatch.setattr(client, "_request", lambda *_args, **_kwargs: response)
    monkeypatch.setattr(
        crypto_module,
        "build_find_privacy_action_execution_receipt_query_with_signer",
        lambda *_args: b"signed-id105-query",
    )
    monkeypatch.setattr(
        crypto_module,
        "inspect_privacy_action_execution_receipt_response",
        lambda *_args: {
            **_execution_receipt(operation_name, admitted_at_height=612),
            "statement_digest": (bytes([0xEE]) * 32).hex(),
        },
    )

    with pytest.raises(RuntimeError, match="changed authenticated statement_digest"):
        client.get_privacy_action_execution_receipt_v1(
            operation,
            canonical_auth=AUTOMATIC_AUTH,
        )


def test_id105_receipt_query_treats_only_404_as_retryable_absence(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    operation = _authenticated_submitted_operation_view(client)
    response = requests.Response()
    response.status_code = 404
    response._content = b""
    monkeypatch.setattr(client, "_request", lambda *_args, **_kwargs: response)
    monkeypatch.setattr(
        crypto_module,
        "build_find_privacy_action_execution_receipt_query_with_signer",
        lambda *_args: b"signed-id105-query",
    )
    monkeypatch.setattr(
        crypto_module,
        "inspect_privacy_action_execution_receipt_response",
        lambda *_args: pytest.fail("404 receipt response reached native inspection"),
    )

    assert (
        client.get_privacy_action_execution_receipt_v1(
            operation,
            canonical_auth=AUTOMATIC_AUTH,
        )
        is None
    )


def test_async_status_refresh_keeps_committed_nonterminal_without_details(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    submitted = _authenticated_submitted_operation_view(client)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Committed", "block_height": 612},
            "resolved_from": "state",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: pytest.fail(
            "Committed pipeline hint queried transaction details"
        ),
    )
    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        lambda *_args, **_kwargs: pytest.fail(
            "Committed pipeline hint queried an execution receipt"
        ),
    )
    refreshed = client.get_privacy_action_status_v1(
        submitted,
        canonical_auth=AUTOMATIC_AUTH,
        timeout=19.0,
    )

    assert refreshed is submitted
    assert refreshed.local_state == "submitted"
    assert refreshed.terminal_chain_state is None


def test_async_status_refresh_preserves_authenticated_rejection_reason(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    submitted = _authenticated_submitted_operation_view(
        client,
        "pq_masp_note_action_v1",
    )
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Rejected", "block_height": 613},
            "resolved_from": "state",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 613,
            "result_ok": False,
            "rejection_code": "PolicyRejected",
            "rejection_message": "governed nullifier policy rejected the action",
        },
    )
    receipt_calls: list[tuple[object, object]] = []

    def no_receipt(
        operation: object,
        *,
        canonical_auth: object,
        timeout: object,
    ) -> None:
        receipt_calls.append((operation, canonical_auth))
        assert timeout is None
        return None

    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        no_receipt,
    )

    terminal = client.get_privacy_action_status_v1(
        submitted,
        canonical_auth=AUTOMATIC_AUTH,
    )

    assert terminal.terminal_chain_state == "Rejected"
    assert terminal.committed_height == 613
    assert (
        terminal.rejection_reason
        == "governed nullifier policy rejected the action"
    )
    assert receipt_calls == [(submitted, AUTOMATIC_AUTH)]


def test_rejected_status_fails_closed_if_any_execution_receipt_exists(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    operation_name: PrivacyExact12ActionOperationV1 = "pq_masp_note_action_v1"
    submitted = _authenticated_submitted_operation_view(client, operation_name)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Rejected", "block_height": 613},
            "resolved_from": "state",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 613,
            "result_ok": False,
            "rejection_code": "PolicyRejected",
            "rejection_message": "governed policy rejected the action",
        },
    )
    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        lambda *_args, **_kwargs: _execution_receipt(
            operation_name,
            admitted_at_height=613,
        ),
    )

    with pytest.raises(RuntimeError, match="carried an execution receipt"):
        client.get_privacy_action_status_v1(
            submitted,
            canonical_auth=AUTOMATIC_AUTH,
        )


def test_async_status_refresh_rejects_public_height_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Applied", "block_height": 614},
            "resolved_from": "cache",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 615,
            "result_ok": True,
            "rejection_code": None,
            "rejection_message": None,
        },
    )

    with pytest.raises(RuntimeError, match="height differs"):
        client.get_privacy_action_status_v1(
            _authenticated_submitted_operation_view(
                client,
                "verange_range_proof_v1",
            ),
            canonical_auth=AUTOMATIC_AUTH,
        )


def test_async_status_refresh_rejects_terminal_outcome_replacement(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    submitted = _authenticated_submitted_operation_view(
        client,
        "pq_masp_note_action_v1",
    )
    applied = PrivacyActionOperationViewV1(
        **{
            **submitted.__dict__,
            "local_state": "terminal",
            "terminal_chain_state": "Applied",
            "committed_height": 616,
            "execution_capability_manifest_digest": EXECUTION_MANIFEST_DIGEST.hex(),
            "execution_capability_committed_height": 410,
            "execution_receipt_finalized_height": 619,
            "execution_receipt_finalized_block_hash": FINALIZED_BLOCK_HASH.hex(),
        }
    )
    client._bind_privacy_action_view_v1(  # noqa: SLF001 - provenance regression
        applied,
        NETWORK_ID,
    )
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Rejected", "block_height": 617},
            "resolved_from": "state",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 617,
            "result_ok": False,
            "rejection_code": "PolicyRejected",
            "rejection_message": "replacement outcome",
        },
    )
    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        lambda *_args, **_kwargs: None,
    )

    with pytest.raises(RuntimeError, match="terminal privacy action outcome changed"):
        client.get_privacy_action_status_v1(
            applied,
            canonical_auth=AUTOMATIC_AUTH,
        )


def test_applied_receipt_finality_may_advance_but_never_regress(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    operation_name: PrivacyExact12ActionOperationV1 = "verange_range_proof_v1"
    submitted = _authenticated_submitted_operation_view(client, operation_name)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Applied", "block_height": 616},
            "resolved_from": "state",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 616,
            "result_ok": True,
            "rejection_code": None,
            "rejection_message": None,
        },
    )
    receipts = iter(
        (
            _execution_receipt(
                operation_name,
                admitted_at_height=616,
                finalized_height=620,
                finalized_block_hash=bytes([0x61]) * 32,
            ),
            _execution_receipt(
                operation_name,
                admitted_at_height=616,
                finalized_height=622,
                finalized_block_hash=bytes([0x62]) * 32,
            ),
            _execution_receipt(
                operation_name,
                admitted_at_height=616,
                finalized_height=621,
                finalized_block_hash=bytes([0x63]) * 32,
            ),
        )
    )
    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        lambda *_args, **_kwargs: next(receipts),
    )

    first = client.get_privacy_action_status_v1(
        submitted,
        canonical_auth=AUTOMATIC_AUTH,
    )
    advanced = client.get_privacy_action_status_v1(
        first,
        canonical_auth=AUTOMATIC_AUTH,
    )
    assert first.execution_receipt_finalized_height == 620
    assert advanced.execution_receipt_finalized_height == 622
    assert advanced.execution_receipt_finalized_block_hash == (bytes([0x62]) * 32).hex()

    with pytest.raises(RuntimeError, match="finality regressed"):
        client.get_privacy_action_status_v1(
            advanced,
            canonical_auth=AUTOMATIC_AUTH,
        )


def test_every_exact12_operation_authenticates_gates_and_submits_once(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    submitted: list[tuple[bytes, object, object]] = []
    active_operation: PrivacyExact12ActionOperationV1 = PRIVACY_EXACT12_ACTION_OPERATIONS_V1[0]

    def capabilities(*, canonical_auth: object) -> _FakeManifest:
        assert canonical_auth is AUTOMATIC_AUTH
        events.append("capabilities")
        return _FakeManifest(active_operation, events)

    monkeypatch.setattr(client, "privacy_capabilities_v1", capabilities)
    _install_canonical_submit_stub(monkeypatch, client, submitted)

    for index, operation in enumerate(PRIVACY_EXACT12_ACTION_OPERATIONS_V1, start=1):
        active_operation = operation
        wire = f"signed-wire-{index}".encode()
        crypto.current_wire = wire
        request = PrivacyExact12ActionRequestV1(
            operation,
            wire,
            expected_manifest_digest=MANIFEST_DIGEST,
        )
        event_offset = len(events)

        envelope, view = client.submit_signed_privacy_action_v1(
            request,
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            wait=False,
        )

        assert envelope.signed_transaction_versioned == submitted[-1][0]
        assert isinstance(view, PrivacyActionOperationViewV1)
        assert view.operation_schema == operation
        assert view.protocol_id == (
            client_module._PRIVACY_EXACT12_ACTION_OPERATION_SPECS_V1[operation].protocol_id
        )
        assert view.local_state == "submitted"
        assert view.terminal_chain_state is None
        assert view.transaction_hash == TRANSACTION_HASH.hex()
        assert view.capability_manifest_digest == MANIFEST_DIGEST.hex()
        assert view.capability_committed_height == 411
        assert events[event_offset:] == [
            f"inspect:{operation}",
            "reconstruct",
            "capabilities",
            "gate",
        ]

    assert len(submitted) == 13


def test_submission_requires_generated_freshness_and_bounded_polling(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "verange_range_proof_v1"
    crypto.current_wire = b"verange-wire"
    request = PrivacyExact12ActionRequestV1(operation, crypto.current_wire)
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: pytest.fail("invalid poll configuration reached Torii"),
    )

    with pytest.raises(ValueError, match="requires generated freshness"):
        client.submit_signed_privacy_action_v1(
            request,
            canonical_auth=STATIC_AUTH,
            network_id=NETWORK_ID,
            wait=False,
        )
    with pytest.raises(ValueError, match="must be bounded"):
        client.submit_signed_privacy_action_v1(
            request,
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            timeout=None,
            max_attempts=None,
        )
    with pytest.raises(ValueError, match="finite and non-negative"):
        client.submit_signed_privacy_action_v1(
            request,
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            interval=float("nan"),
            wait=False,
        )
    assert events == []


def test_wait_path_requires_applied_details_and_finalized_receipt(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "anonymous_pgc_payment_action_v1"
    crypto.current_wire = b"pgc-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    _install_canonical_submit_stub(monkeypatch, client)
    statuses = iter(
        (
            {
                "status": {"kind": "Committed", "block_height": 512},
                "resolved_from": "state",
            },
            {
                "status": {"kind": "Applied", "block_height": 512},
                "resolved_from": "cache",
            },
        )
    )
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: next(statuses),
    )
    detail_calls: list[tuple[str, object, object]] = []

    def details(
        transaction_hash: str,
        *,
        canonical_auth: object,
        timeout: object,
    ) -> dict[str, object]:
        detail_calls.append((transaction_hash, canonical_auth, timeout))
        return {
            "transaction_hash": transaction_hash,
            "block_height": 512,
            "result_ok": True,
            "rejection_code": None,
            "rejection_message": None,
        }

    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        details,
    )
    receipt = _execution_receipt(operation, admitted_at_height=512)
    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        lambda *_args, **_kwargs: receipt,
    )
    envelope, view = client.submit_signed_privacy_action_v1(
        PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
        canonical_auth=AUTOMATIC_AUTH,
        network_id=NETWORK_ID,
        interval=0.0,
        max_attempts=2,
    )

    assert envelope.signed_transaction_versioned == crypto.current_wire
    assert view.local_state == "terminal"
    assert view.terminal_chain_state == "Applied"
    assert view.committed_height == 512
    assert view.ledger_effect_kind == "anonymous_pgc_account_state_transition"
    assert view.execution_capability_manifest_digest == EXECUTION_MANIFEST_DIGEST.hex()
    assert view.execution_capability_committed_height == 410
    assert view.execution_receipt_finalized_height == 515
    assert view.execution_receipt_finalized_block_hash == FINALIZED_BLOCK_HASH.hex()
    assert len(detail_calls) == 1
    assert detail_calls[0][0:2] == (TRANSACTION_HASH.hex(), AUTOMATIC_AUTH)


def test_applied_receipt_404_is_retried_within_the_same_wait_bound(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, _crypto, _events = _client(monkeypatch)
    operation_name: PrivacyExact12ActionOperationV1 = "vega_credential_presentation_v1"
    submitted = _authenticated_submitted_operation_view(client, operation_name)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Applied", "block_height": 520},
            "resolved_from": "cache",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 520,
            "result_ok": True,
            "rejection_code": None,
            "rejection_message": None,
        },
    )
    receipts = iter(
        (
            None,
            _execution_receipt(operation_name, admitted_at_height=520),
        )
    )
    receipt_calls = 0

    def receipt(*_args: object, **_kwargs: object) -> object:
        nonlocal receipt_calls
        receipt_calls += 1
        return next(receipts)

    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        receipt,
    )

    terminal = client._wait_for_privacy_action_terminal_status_v1(  # noqa: SLF001
        submitted,
        canonical_auth=AUTOMATIC_AUTH,
        interval=0.0,
        timeout=5.0,
        max_attempts=2,
        on_status=None,
    )

    assert receipt_calls == 2
    assert terminal.terminal_chain_state == "Applied"
    assert terminal.committed_height == 520


def test_wait_path_rejects_public_status_height_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "vega_credential_presentation_v1"
    crypto.current_wire = b"vega-height-drift-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    _install_canonical_submit_stub(monkeypatch, client)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Applied", "block_height": 512},
            "resolved_from": "cache",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 513,
            "result_ok": True,
            "rejection_code": None,
            "rejection_message": None,
        },
    )

    with pytest.raises(RuntimeError, match="status height differs"):
        client.submit_signed_privacy_action_v1(
            PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            interval=0.0,
            max_attempts=1,
        )


def test_wait_path_rejects_unsuccessful_authenticated_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "jindo_polynomial_evaluation_v1"
    crypto.current_wire = b"jindo-result-drift-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    _install_canonical_submit_stub(monkeypatch, client)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Applied", "block_height": 514},
            "resolved_from": "cache",
        },
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: {
            "transaction_hash": TRANSACTION_HASH.hex(),
            "block_height": 514,
            "result_ok": False,
            "rejection_code": "InvalidParameter",
            "rejection_message": "committed result substitution",
        },
    )

    with pytest.raises(RuntimeError, match="applied privacy action resolved to a rejected"):
        client.submit_signed_privacy_action_v1(
            PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            interval=0.0,
            max_attempts=1,
        )


def test_rejected_wait_fetches_authenticated_committed_reason(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "pq_masp_note_action_v1"
    crypto.current_wire = b"pq-masp-rejected-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    _install_canonical_submit_stub(monkeypatch, client)
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: {
            "status": {"kind": "Rejected", "block_height": 513},
            "resolved_from": "state",
        },
    )

    detail_calls: list[tuple[str, object, object]] = []

    def details(
        transaction_hash: str,
        *,
        canonical_auth: object,
        timeout: object,
    ) -> dict[str, object]:
        detail_calls.append((transaction_hash, canonical_auth, timeout))
        return {
            "transaction_hash": transaction_hash,
            "block_height": 513,
            "result_ok": False,
            "rejection_code": "PolicyRejected",
            "rejection_message": "offline policy epoch is stale",
        }

    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        details,
    )
    monkeypatch.setattr(
        client,
        "get_privacy_action_execution_receipt_v1",
        lambda *_args, **_kwargs: None,
    )
    _envelope, view = client.submit_signed_privacy_action_v1(
        PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
        canonical_auth=AUTOMATIC_AUTH,
        network_id=NETWORK_ID,
        timeout=17.0,
        interval=0.0,
        max_attempts=1,
    )

    assert len(detail_calls) == 1
    assert detail_calls[0][0:2] == (TRANSACTION_HASH.hex(), AUTOMATIC_AUTH)
    assert view.local_state == "terminal"
    assert view.terminal_chain_state == "Rejected"
    assert view.committed_height == 513
    assert view.rejection_reason == "offline policy epoch is stale"


def test_expired_wait_does_not_invent_a_committed_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "orchard_note_action_v1"
    crypto.current_wire = b"orchard-expired-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    _install_canonical_submit_stub(monkeypatch, client)
    statuses = iter(
        (
            {
                "status": {"kind": "Expired"},
                "resolved_from": "cache",
            },
            {
                "status": {"kind": "Expired"},
                "resolved_from": "state",
            },
        )
    )
    monkeypatch.setattr(
        client,
        "_get_authenticated_privacy_action_status_v1",
        lambda *_args, **_kwargs: next(statuses),
    )
    monkeypatch.setattr(
        client,
        "get_pipeline_transaction_details_with_canonical_auth",
        lambda *_args, **_kwargs: pytest.fail(
            "expired transaction queried committed rejection details"
        ),
    )
    _envelope, view = client.submit_signed_privacy_action_v1(
        PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
        canonical_auth=AUTOMATIC_AUTH,
        network_id=NETWORK_ID,
        interval=0.0,
        max_attempts=2,
    )

    assert view.local_state == "terminal"
    assert view.terminal_chain_state == "Expired"
    assert view.committed_height is None
    assert view.rejection_reason is None


def test_manifest_rotation_fails_before_submission(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "zk_ams_batch_admission_action_v1"
    crypto.current_wire = b"zk-ams-wire"
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: _FakeManifest(operation, events),
    )
    monkeypatch.setattr(
        client,
        "_submit_signed_privacy_action_wire_v1",
        lambda *_args, **_kwargs: pytest.fail("rotated manifest permitted submission"),
    )

    with pytest.raises(RuntimeError, match="does not match the requested digest"):
        client.submit_signed_privacy_action_v1(
            PrivacyExact12ActionRequestV1(
                operation,
                crypto.current_wire,
                expected_manifest_digest=bytes([0xFF]) * 32,
            ),
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            wait=False,
        )
    assert events == [f"inspect:{operation}", "reconstruct"]


def test_signed_authority_must_equal_canonical_auth_before_any_torii_call(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, crypto, events = _client(monkeypatch)
    operation: PrivacyExact12ActionOperationV1 = "verange_range_proof_v1"
    crypto.current_wire = b"signed-by-another-account"
    crypto.authority_account_id = AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x12]) * 32,
    ).to_i105(0x02F1)
    monkeypatch.setattr(
        client,
        "privacy_capabilities_v1",
        lambda **_kwargs: pytest.fail("authority mismatch reached Torii"),
    )
    monkeypatch.setattr(
        client,
        "_submit_signed_privacy_action_wire_v1",
        lambda *_args, **_kwargs: pytest.fail("authority mismatch was submitted"),
    )

    with pytest.raises(RuntimeError, match="differs from canonical_auth account"):
        client.submit_signed_privacy_action_v1(
            PrivacyExact12ActionRequestV1(operation, crypto.current_wire),
            canonical_auth=AUTOMATIC_AUTH,
            network_id=NETWORK_ID,
            wait=False,
        )

    assert events == [f"inspect:{operation}", "reconstruct"]
