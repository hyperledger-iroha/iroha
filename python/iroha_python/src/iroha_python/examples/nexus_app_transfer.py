"""Minimal Nexus App Facade transfer recipe with fake wallet/Torii dependencies."""

from __future__ import annotations

from iroha_python import NetworkId
from iroha_python.connect import ConnectUri, build_connect_uri, generate_connect_sid
from iroha_python.nexus_app import (
    NexusAppClient,
    NexusAppConfig,
    NexusConnectSession,
    NexusTransferInput,
    NexusWalletSignature,
)

ACCOUNT_ID = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"
DESTINATION_ACCOUNT_ID = "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L"
SOURCE_ASSET_ID = f"7EAD8EFYUx1aVKZPUU1fyKvr8dF1#{ACCOUNT_ID}"
SIGNING_PUBLIC_KEY = bytes.fromhex(
    "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737"
)
PAYLOAD_BYTES = b"nexus-app-transfer-demo-v1"
WALLET_SIGNATURE = bytes.fromhex(
    "d39065822f28108f70f8089f64357cc33a0072e45aa65f6b3e2696b93a3d9779d376ddf19c8e7dabce79a484275b681dea5213df060848d8fe098edeebcc3c07"
)
SIGNED_TRANSACTION_HASH_HEX = "b410d55b960d396c1034221dea22464d08de1237363b02cb1f7c35d4c6eaf0a1"
FEE_PAYMENT = {
    "payer": "authority",
    "value": {"charge_limits": [], "gas_limit": None},
}


class DemoConnectTransport:
    def start_connect(self, options, config):
        _ = options
        app_public_key = bytes([0x41]) * 32
        nonce = bytes([0x51]) * 16
        sid = generate_connect_sid(
            network_id=config.network_id,
            app_public_key=app_public_key,
            nonce=nonce,
        )
        wallet_uri = build_connect_uri(
            ConnectUri(
                sid=sid.sid_base64url,
                network_id=config.network_id,
                app_public_key=app_public_key,
                nonce=nonce,
            )
        )
        return NexusConnectSession(
            sid=sid.sid_base64url,
            network_id=config.network_id,
            app_public_key=app_public_key,
            nonce=nonce,
            wallet_launch_uri=f"{wallet_uri}&role=wallet",
        )

    def await_approval(self, session, config):
        return {
            "account_id": ACCOUNT_ID,
            "signing_public_key": SIGNING_PUBLIC_KEY,
            "session": session,
        }

    def request_signature(self, session, signable, config):
        print("payload hash:", signable.payload_hash_hex)
        return NexusWalletSignature(WALLET_SIGNATURE)


class DemoTransactionCodec:
    def build_transfer_payload(self, payload_input):
        _ = payload_input
        return PAYLOAD_BYTES

    def finalize_signed_transaction(self, signable, signature, signing_public_key):
        _ = signing_public_key
        signed = b"nexus-demo:" + signable.payload_bytes + signature.signature
        return {
            "signed_transaction": signed,
            "hash_hex": SIGNED_TRANSACTION_HASH_HEX,
        }


class DemoToriiClient:
    def submit_transaction(self, signed_transaction):
        return {"accepted": True}

    def wait_for_transaction_status(self, hash_hex, **options):
        return {"hash": hash_hex, "status": "Committed"}


def main() -> None:
    client = NexusAppClient(
        NexusAppConfig(
            network_id=NetworkId.from_bytes(bytes([0xA5]) * 32),
        ),
        connect_transport=DemoConnectTransport(),
        transaction_codec=DemoTransactionCodec(),
        torii_client=DemoToriiClient(),
    )

    session = client.start_connect()
    _account, approved_session = client.await_approval(session)
    receipt = client.transfer_with_wallet(
        approved_session,
        NexusTransferInput(
            source_asset_id=SOURCE_ASSET_ID,
            quantity="12.34",
            destination_account_id=DESTINATION_ACCOUNT_ID,
            fee_payment=FEE_PAYMENT,
            metadata={"purpose": "nexus-app-fixture"},
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
