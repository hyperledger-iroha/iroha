"""Minimal Nexus App Facade transfer recipe with fake wallet/Torii dependencies."""

from __future__ import annotations

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
PAYLOAD_BYTES = bytes.fromhex(
    "0c0b0a746573742d636861696e4f000000004a2100000000000000010001d0014a01b201320174012b01b401ab013a0113016801bd0146011501e401e601d00122014a01b7011a0101016b01af0185012001a3013201c9017701870137080068e5cf8b010000ac0200000000a60201000000000000009c020f0e69726f68612e7472616e736665728a0202010000000000004e5254300000a4174c78d6341f8f98fc2adae8ed67b900da000000000000006356adc8a15d041a0202000000d401764f000000004a2100000000000000010001d0014a01b201320174012b01b401ab013a0113016801bd0146011501e401e601d00122014a01b7011a0101016b01af0185012001a3013201c90177018701372001be01f5013c011c01cd0117014901e1018001df01ba01d60151019b01fd016604000000000c0602000000d20404020000004f000000004a2100000000000000010001a0019a01a501f4017a016701590180012f01f9015501f801dc012d012a011401a501c9019d012301be019701f801640112017f01f901380134015501a401f00a01083075000000000000060104070000002801000000000000001f0807707572706f7365151413226e657875732d6170702d6669787475726522"
)
WALLET_SIGNATURE = bytes.fromhex(
    "c82d2ee732a9251153eff6f510a0d12b292cb51a5d961a7eddb84f6ee944e34eaca60ca2f1ccfe7a53fd6813fc9a6db9e35cb276b2411b7d583d45fdc6caee05"
)
SIGNED_TRANSACTION_HASH_HEX = "2d22bf944c58886de938e4094bf9887a43e66d598162bd2205f0812b64e180bb"


class DemoConnectTransport:
    def start_connect(self, options, config):
        return NexusConnectSession(
            sid="sid-demo-1",
            wallet_launch_uri="iroha://connect?sid=sid-demo-1&role=wallet",
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
        NexusAppConfig(chain_id="test-chain"),
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
