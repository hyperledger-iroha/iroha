"""Minimal Nexus App Facade transfer recipe with fake wallet/Torii dependencies."""

from __future__ import annotations

import hashlib
import json

from iroha_python.nexus_app import (
    NexusAppClient,
    NexusAppConfig,
    NexusConnectSession,
    NexusTransferInput,
    NexusWalletSignature,
)


SIGNING_PUBLIC_KEY = bytes([0x11]) * 32
WALLET_SIGNATURE = bytes([0x07]) * 64


class DemoConnectTransport:
    def start_connect(self, options, config):
        return NexusConnectSession(
            sid="sid-demo-1",
            wallet_launch_uri="iroha://connect?sid=sid-demo-1&role=wallet",
        )

    def await_approval(self, session, config):
        return {
            "account_id": "sora-demo-account",
            "signing_public_key": SIGNING_PUBLIC_KEY,
            "session": session,
        }

    def request_signature(self, session, signable, config):
        print("payload hash:", signable.payload_hash_hex)
        return NexusWalletSignature(WALLET_SIGNATURE)


class DemoTransactionCodec:
    def build_transfer_payload(self, payload_input):
        return json.dumps(payload_input, sort_keys=True, default=str).encode()

    def finalize_signed_transaction(self, signable, signature, signing_public_key):
        signed = b"nexus-demo:" + signable.payload_bytes + signature
        return {
            "signed_transaction": signed,
            "hash_hex": "demo-" + hashlib.blake2b(signed, digest_size=16).hexdigest(),
        }


class DemoToriiClient:
    def submit_transaction(self, signed_transaction):
        return {"accepted": True}

    def wait_for_transaction_status(self, hash_hex, **options):
        return {"hash": hash_hex, "status": "Committed"}


def main() -> None:
    client = NexusAppClient(
        NexusAppConfig(chain_id="test-chain"),
        connect_transport=DemoConnectTransport(),
        transaction_codec=DemoTransactionCodec(),
        torii_client=DemoToriiClient(),
    )

    session = client.start_connect()
    account, approved_session = client.await_approval(session)
    receipt = client.transfer_with_wallet(
        approved_session,
        NexusTransferInput(
            source_asset_id=f"7EAD8EFYUx1aVKZPUU1fyKvr8dF1#{account}",
            quantity="12.34",
            destination_account_id="sora-destination-account",
            creation_time_ms=1_700_000_000_000,
            ttl_ms=30_000,
            nonce=7,
        ),
    )

    print("wallet URI:", session.wallet_launch_uri)
    print("signed transaction hash:", receipt.signed_transaction_hash_hex)
    print("final status:", receipt.status)


if __name__ == "__main__":
    main()
