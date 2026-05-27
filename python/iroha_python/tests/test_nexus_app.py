from __future__ import annotations

import json
from pathlib import Path

import pytest

from iroha_python.nexus_app import (
    DefaultNexusTransactionCodec,
    NexusAppClient,
    NexusAppConfig,
    NexusAppError,
    NexusApprovedAccount,
    NexusConnectOptions,
    NexusConnectSession,
    NexusTransferInput,
    NexusWalletSignature,
)


FIXTURE = json.loads(
    (Path(__file__).parents[3] / "fixtures" / "sdk" / "nexus_connect_transfer_v1.json").read_text(
        encoding="utf-8"
    )
)


class FakeConnect:
    def __init__(self, signature: bytes, approval=None):
        self.signature = signature
        self.approval = approval
        self.requested_payloads: list[bytes] = []

    def start_connect(self, options: NexusConnectOptions, _config: NexusAppConfig) -> NexusConnectSession:
        sid = options.sid or "sid-generated"
        return NexusConnectSession(
            sid=sid,
            wallet_launch_uri=f"iroha://connect?sid={sid}",
            token_app="app-token",
        )

    def await_approval(self, _session: NexusConnectSession, _config: NexusAppConfig):
        if self.approval is not None:
            return self.approval
        return {
            "account_id": "approved-account-i105",
            "signing_public_key": bytes([1]) * 32,
        }

    def request_signature(self, _session, signable, _config):
        self.requested_payloads.append(signable.payload_bytes)
        return NexusWalletSignature(self.signature)


class FakeCodec:
    def __init__(
        self,
        payload: bytes,
        signed: bytes,
        hash_hex: str,
        *,
        expected_authority: str = "approved-account-i105",
    ):
        self.payload = payload
        self.signed = signed
        self.hash_hex = hash_hex
        self.expected_authority = expected_authority
        self.finalized = []

    def build_transfer_payload(self, payload_input):
        assert payload_input["chain_id"] == "test-chain"
        assert payload_input["authority"] == self.expected_authority
        assert payload_input["destination_account_id"] == "destination-i105"
        return self.payload

    def finalize_signed_transaction(self, signable, signature, signing_public_key):
        assert signable.payload_bytes == self.payload
        assert signature.signature == bytes([7]) * 64
        assert signing_public_key == bytes([1]) * 32
        self.finalized.append((signable, signature))
        return {"signed_transaction": self.signed, "hash_hex": self.hash_hex}


class FakeTorii:
    def __init__(self, *, submit_hash_hex=None, submit_error=None, wait_error=None):
        self.submitted = []
        self.waited = []
        self.submit_hash_hex = submit_hash_hex
        self.submit_error = submit_error
        self.wait_error = wait_error

    def submit_transaction(self, payload):
        if self.submit_error is not None:
            raise self.submit_error
        self.submitted.append(payload)
        return {"accepted": True, "hash_hex": self.submit_hash_hex} if self.submit_hash_hex else {"accepted": True}

    def wait_for_transaction_status(self, hash_hex, **_options):
        if self.wait_error is not None:
            raise self.wait_error
        self.waited.append(hash_hex)
        return {"status": "Applied"}


def test_nexus_app_builds_transfer_draft_and_computes_payload_hash():
    payload = b"canonical-transfer-payload"
    client = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            authority="approved-account-i105",
            signing_public_key=bytes([1]) * 32,
        ),
        transaction_codec=FakeCodec(payload, b"signed", "a" * 64),
    )

    draft = client.build_transfer_draft(
        NexusTransferInput(
            source_asset_id="asset#approved-account-i105",
            quantity="1.25",
            destination_account_id="destination-i105",
        )
    )

    assert draft.signable.payload_bytes == payload
    assert len(draft.signable.payload_hash_hex) == 64


def test_nexus_app_default_codec_matches_shared_fixture():
    transfer = FIXTURE["transfer_input"]
    expected = FIXTURE["expected"]
    approval = FIXTURE["connect"]["approval_frame"]
    client = NexusAppClient(
        NexusAppConfig(
            chain_id=transfer["chain_id"],
            authority=transfer["authority"],
            signing_public_key=bytes.fromhex(approval["signing_public_key_hex"]),
        ),
        transaction_codec=DefaultNexusTransactionCodec(),
    )

    draft = client.build_transfer_draft(
        NexusTransferInput(
            source_asset_id=transfer["source_asset_id"],
            quantity=transfer["quantity"],
            destination_account_id=transfer["destination_account_id"],
            metadata=transfer["metadata"],
            creation_time_ms=transfer["creation_time_ms"],
            ttl_ms=transfer["ttl_ms"],
            nonce=transfer["nonce"],
        )
    )

    assert draft.signable.payload_bytes.hex() == expected["payload_bytes_hex"]
    assert draft.signable.payload_hash_hex == expected["payload_hash_hex"]


def test_nexus_app_runs_wallet_transfer_flow():
    payload = b"canonical-transfer-payload"
    signed = b"signed-transaction"
    hash_hex = "b" * 64
    connect = FakeConnect(bytes([7]) * 64)
    codec = FakeCodec(payload, signed, hash_hex)
    torii = FakeTorii()
    client = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            signing_public_key=bytes([1]) * 32,
        ),
        connect_transport=connect,
        transaction_codec=codec,
        torii_client=torii,
    )

    session = client.start_connect(NexusConnectOptions(sid="sid-1"))
    approval = client.await_approval(session)
    assert isinstance(approval, NexusApprovedAccount)
    _account, approved_session = approval
    receipt = client.transfer_with_wallet(
        approved_session,
        NexusTransferInput(
            source_asset_id="asset#approved-account-i105",
            quantity=1,
            destination_account_id="destination-i105",
        ),
    )

    assert receipt.signed_transaction == signed
    assert receipt.signed_transaction_hash_hex == hash_hex
    assert connect.requested_payloads == [payload]
    assert torii.submitted == [signed]
    assert torii.waited == [hash_hex]


def test_nexus_app_rejects_unsupported_signature_algorithm():
    client = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            authority="account-i105",
            signing_public_key=bytes([1]) * 32,
        ),
        transaction_codec=FakeCodec(
            b"payload", b"signed", "c" * 64, expected_authority="account-i105"
        ),
    )
    draft = client.build_transfer_draft(
        NexusTransferInput(
            source_asset_id="asset#account-i105",
            quantity=1,
            destination_account_id="destination-i105",
        )
    )

    with pytest.raises(NexusAppError) as excinfo:
        client.finalize_and_submit(
            draft.signable,
            {"algorithm": "secp256k1", "signature": bytes([0]) * 64},
            wait=False,
        )

    assert excinfo.value.code == "unsupported_signature_algorithm"


def test_nexus_app_rejects_missing_approval_fields():
    missing_account = NexusAppClient(
        NexusAppConfig(chain_id="test-chain"),
        connect_transport=FakeConnect(bytes([7]) * 64, approval={}),
        transaction_codec=FakeCodec(b"payload", b"signed", "a" * 64),
    )

    with pytest.raises(NexusAppError) as account_exc:
        missing_account.await_approval(NexusConnectSession("sid-1", "iroha://connect?sid=sid-1"))
    assert account_exc.value.code == "approval_missing_account"

    missing_key = NexusAppClient(
        NexusAppConfig(chain_id="test-chain"),
        connect_transport=FakeConnect(bytes([7]) * 64, approval={"account_id": "not-an-i105-account"}),
        transaction_codec=FakeCodec(b"payload", b"signed", "a" * 64),
    )

    with pytest.raises(NexusAppError) as key_exc:
        missing_key.await_approval(NexusConnectSession("sid-1", "iroha://connect?sid=sid-1"))
    assert key_exc.value.code == "missing_signing_public_key"


def test_nexus_app_rejects_authority_mismatch_before_wallet_signature():
    connect = FakeConnect(bytes([7]) * 64)
    client = NexusAppClient(
        NexusAppConfig(chain_id="test-chain", signing_public_key=bytes([1]) * 32),
        connect_transport=connect,
        transaction_codec=FakeCodec(b"payload", b"signed", "a" * 64),
        torii_client=FakeTorii(),
    )
    session = NexusConnectSession(
        sid="sid-1",
        wallet_launch_uri="iroha://connect?sid=sid-1",
        approved_account="approved-account-i105",
        signing_public_key=bytes([1]) * 32,
    )

    with pytest.raises(NexusAppError) as excinfo:
        client.transfer_with_wallet(
            session,
            NexusTransferInput(
                source_asset_id="asset#other-account-i105",
                quantity=1,
                destination_account_id="destination-i105",
                authority="other-account-i105",
            ),
        )

    assert excinfo.value.code == "approval_account_mismatch"
    assert connect.requested_payloads == []


def test_nexus_app_rejects_invalid_signature_length():
    client = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            authority="account-i105",
            signing_public_key=bytes([1]) * 32,
        ),
        transaction_codec=FakeCodec(
            b"payload", b"signed", "c" * 64, expected_authority="account-i105"
        ),
        torii_client=FakeTorii(),
    )
    draft = client.build_transfer_draft(
        NexusTransferInput(
            source_asset_id="asset#account-i105",
            quantity=1,
            destination_account_id="destination-i105",
        )
    )

    with pytest.raises(NexusAppError) as excinfo:
        client.finalize_and_submit(draft.signable, bytes([0]) * 63, wait=False)

    assert excinfo.value.code == "invalid_signature"


def test_nexus_app_rejects_torii_hash_mismatch_and_maps_failures():
    hash_hex = "d" * 64
    client = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            authority="account-i105",
            signing_public_key=bytes([1]) * 32,
        ),
        transaction_codec=FakeCodec(
            b"payload", b"signed", hash_hex, expected_authority="account-i105"
        ),
        torii_client=FakeTorii(submit_hash_hex="e" * 64),
    )
    draft = client.build_transfer_draft(
        NexusTransferInput(
            source_asset_id="asset#account-i105",
            quantity=1,
            destination_account_id="destination-i105",
        )
    )

    with pytest.raises(NexusAppError) as mismatch_exc:
        client.finalize_and_submit(draft.signable, NexusWalletSignature(bytes([7]) * 64), wait=False)
    assert mismatch_exc.value.code == "transaction_hash_mismatch"

    submit_failure = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            authority="account-i105",
            signing_public_key=bytes([1]) * 32,
        ),
        transaction_codec=FakeCodec(
            b"payload", b"signed", hash_hex, expected_authority="account-i105"
        ),
        torii_client=FakeTorii(submit_error=RuntimeError("down")),
    )
    draft = submit_failure.build_transfer_draft(
        NexusTransferInput(
            source_asset_id="asset#account-i105",
            quantity=1,
            destination_account_id="destination-i105",
        )
    )
    with pytest.raises(NexusAppError) as submit_exc:
        submit_failure.finalize_and_submit(draft.signable, NexusWalletSignature(bytes([7]) * 64), wait=False)
    assert submit_exc.value.code == "submit_failed"

    status_failure = NexusAppClient(
        NexusAppConfig(
            chain_id="test-chain",
            authority="account-i105",
            signing_public_key=bytes([1]) * 32,
        ),
        transaction_codec=FakeCodec(
            b"payload", b"signed", hash_hex, expected_authority="account-i105"
        ),
        torii_client=FakeTorii(submit_hash_hex=hash_hex, wait_error=RuntimeError("timeout")),
    )
    draft = status_failure.build_transfer_draft(
        NexusTransferInput(
            source_asset_id="asset#account-i105",
            quantity=1,
            destination_account_id="destination-i105",
        )
    )
    with pytest.raises(NexusAppError) as status_exc:
        status_failure.finalize_and_submit(draft.signable, NexusWalletSignature(bytes([7]) * 64))
    assert status_exc.value.code == "status_wait_failed"


def test_nexus_app_fixture_error_codes_are_stable():
    expected_codes = {case["name"]: case["expected_code"] for case in FIXTURE["error_cases"]}
    assert expected_codes["unsupported signature algorithm"] == "unsupported_signature_algorithm"
    assert expected_codes["approval without signing key"] == "missing_signing_public_key"
    assert expected_codes["authority mismatch"] == "approval_account_mismatch"
