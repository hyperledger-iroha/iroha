from __future__ import annotations

import asyncio

import pytest

from iroha_python.offline_cash import (
    KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
    KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME,
    KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME,
    OfflineCashConfigurationSnapshot,
    OfflineCashConfigurationSnapshotError,
    OfflineCashLifecycleController,
    OfflineCashNfcCapability,
    OfflineCashTransportCapabilities,
    offline_cash_available_transport_kinds,
)


def test_offline_cash_transport_availability_hides_unsupported_nfc() -> None:
    capabilities = OfflineCashTransportCapabilities(
        qr_streaming=True,
        nfc=OfflineCashNfcCapability(False, "missing HCE"),
        nearby=True,
    )

    assert tuple(offline_cash_available_transport_kinds(capabilities)) == ("qr", "nearby")
    assert (
        tuple(
            offline_cash_available_transport_kinds(
                OfflineCashTransportCapabilities(qr_streaming=True, nfc=None, nearby=True)
            )
        )
        == ("qr", "nearby")
    )


def test_offline_cash_lifecycle_syncs_pending_receipts_before_load() -> None:
    asyncio.run(_assert_lifecycle_syncs_pending_receipts_before_load())


async def _assert_lifecycle_syncs_pending_receipts_before_load() -> None:
    events: list[str] = []

    class Wallet:
        async def load(self, asset_definition_id: str, amount: str) -> dict[str, str]:
            events.append(f"load:{asset_definition_id}:{amount}")
            return {"ok": "true"}

        def prepare_receive(self, asset_definition_id: str, amount: str) -> None:
            raise AssertionError("not used")

        def pay(self, receive_request: object) -> None:
            raise AssertionError("not used")

        def accept(self, payment_token: object) -> None:
            raise AssertionError("not used")

        async def redeem(self, note: object, recipient: str | None = None) -> None:
            raise AssertionError("not used")

    async def has_pending() -> bool:
        events.append("hasPending")
        return True

    async def sync() -> None:
        events.append("sync")

    controller = OfflineCashLifecycleController(
        Wallet(),
        has_pending_audit_receipts=has_pending,
        sync_pending_audit_receipts=sync,
    )

    assert await controller.load("pkr#sbp", "10") == {"ok": "true"}
    assert events == ["hasPending", "sync", "load:pkr#sbp:10"]


def test_offline_cash_lifecycle_does_not_load_when_sync_fails() -> None:
    asyncio.run(_assert_lifecycle_does_not_load_when_sync_fails())


async def _assert_lifecycle_does_not_load_when_sync_fails() -> None:
    events: list[str] = []

    class Wallet:
        async def load(self, asset_definition_id: str, amount: str) -> dict[str, str]:
            events.append(f"load:{asset_definition_id}:{amount}")
            return {"ok": "true"}

        def prepare_receive(self, asset_definition_id: str, amount: str) -> None:
            raise AssertionError("not used")

        def pay(self, receive_request: object) -> None:
            raise AssertionError("not used")

        def accept(self, payment_token: object) -> None:
            raise AssertionError("not used")

        async def redeem(self, note: object, recipient: str | None = None) -> None:
            raise AssertionError("not used")

    async def has_pending() -> bool:
        events.append("hasPending")
        return True

    async def sync() -> None:
        events.append("sync")
        raise RuntimeError("audit sync failed")

    controller = OfflineCashLifecycleController(
        Wallet(),
        has_pending_audit_receipts=has_pending,
        sync_pending_audit_receipts=sync,
    )

    with pytest.raises(RuntimeError, match="audit sync failed"):
        await controller.load("pkr#sbp", "10")
    assert events == ["hasPending", "sync"]


def test_offline_cash_snapshot_requires_cached_issuer_key() -> None:
    snapshot = OfflineCashConfigurationSnapshot(
        chain_id="00000042",
        asset_definition_id="pkr#sbp",
        offline_payments_enabled=True,
        issuer_public_key_base64="issuer-key",
        bridge_abi_version=7,
        created_at_ms=100,
        expires_at_ms=1000,
    )
    snapshot.require_usable_for_offline_exchange(now_ms=999, required_bridge_abi_version=7)

    missing_key = OfflineCashConfigurationSnapshot(
        chain_id="00000042",
        asset_definition_id="pkr#sbp",
        offline_payments_enabled=True,
        issuer_public_key_base64=" ",
    )
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        missing_key.require_usable_for_offline_exchange(now_ms=200)
    assert error.value.code == "missing_issuer_public_key"

    disabled = OfflineCashConfigurationSnapshot(
        chain_id="00000042",
        asset_definition_id="pkr#sbp",
        offline_payments_enabled=False,
        issuer_public_key_base64="issuer-key",
        bridge_abi_version=7,
    )
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        disabled.require_usable_for_offline_exchange(now_ms=200, required_bridge_abi_version=7)
    assert error.value.code == "offline_payments_disabled"

    stale_abi = OfflineCashConfigurationSnapshot(
        chain_id="00000042",
        asset_definition_id="pkr#sbp",
        offline_payments_enabled=True,
        issuer_public_key_base64="issuer-key",
        bridge_abi_version=6,
    )
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        stale_abi.require_usable_for_offline_exchange(now_ms=200, required_bridge_abi_version=7)
    assert error.value.code == "unsupported_bridge_abi"

    expired = snapshot
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        expired.require_usable_for_offline_exchange(now_ms=1000, required_bridge_abi_version=7)
    assert error.value.code == "expired"


def test_kagemusha_wire_name_constants_are_canonical() -> None:
    assert (
        KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME
        == "iroha_data_model::isi::offline::KagemushaTransfer"
    )
    assert (
        KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME
        == "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
    )
    assert (
        KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME
        == "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
    )
