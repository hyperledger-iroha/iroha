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

ISSUER_PUBLIC_KEY_BASE64 = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
ISSUER_PUBLIC_KEY_BASE64URL = "__________________________________________8"
SHORT_ISSUER_PUBLIC_KEY_BASE64 = "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urqw"
LONG_ISSUER_PUBLIC_KEY_BASE64 = "zc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3N"


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


def test_offline_cash_snapshot_requires_cached_identity_time_issuer_key_and_abi() -> None:
    snapshot_kwargs = {
        "chain_id": "00000042",
        "asset_definition_id": "pkr#sbp",
        "offline_payments_enabled": True,
        "issuer_public_key_base64": ISSUER_PUBLIC_KEY_BASE64,
        "native_bridge_abi_version": 7,
        "artifact_set_id": "artifact-set",
        "circuit_id": "kagemusha-recursive-compact-v1",
        "created_at_ms": 100,
        "expires_at_ms": 1000,
    }
    snapshot = OfflineCashConfigurationSnapshot(**snapshot_kwargs)
    snapshot.require_usable_for_offline_exchange(now_ms=999, required_native_bridge_abi_version=7)
    url_key_kwargs = dict(snapshot_kwargs)
    url_key_kwargs["issuer_public_key_base64"] = ISSUER_PUBLIC_KEY_BASE64URL
    OfflineCashConfigurationSnapshot(**url_key_kwargs).require_usable_for_offline_exchange(
        now_ms=999,
        required_native_bridge_abi_version=7,
    )

    for field_name, value in (
        ("chain_id", ""),
        ("chain_id", " 00000042"),
        ("chain_id", "00000042\n"),
        ("chain_id", True),
        ("asset_definition_id", ""),
        ("asset_definition_id", "pkr sbp"),
        ("asset_definition_id", "pkr#sbp\u2603"),
        ("artifact_set_id", "artifact set"),
        ("circuit_id", "kagemusha-recursive-compact-v1\n"),
    ):
        malformed_identity_kwargs = dict(snapshot_kwargs)
        malformed_identity_kwargs[field_name] = value
        malformed_identity = OfflineCashConfigurationSnapshot(**malformed_identity_kwargs)
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            malformed_identity.require_usable_for_offline_exchange(
                now_ms=200,
                required_native_bridge_abi_version=7,
            )
        assert error.value.code == "malformed_snapshot"
        assert field_name in str(error.value)

    missing_key = OfflineCashConfigurationSnapshot(
        chain_id="00000042",
        asset_definition_id="pkr#sbp",
        offline_payments_enabled=True,
        issuer_public_key_base64=" ",
    )
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        missing_key.require_usable_for_offline_exchange(now_ms=200)
    assert error.value.code == "missing_issuer_public_key"

    for issuer_key in (
        "",
        f" {ISSUER_PUBLIC_KEY_BASE64}",
        f"{ISSUER_PUBLIC_KEY_BASE64} ",
        "not base64",
        "!!!!",
        f"{ISSUER_PUBLIC_KEY_BASE64}=",
        SHORT_ISSUER_PUBLIC_KEY_BASE64,
        LONG_ISSUER_PUBLIC_KEY_BASE64,
        "issuer-key\n",
        "issuer-key\u2603",
    ):
        noncanonical = OfflineCashConfigurationSnapshot(
            chain_id="00000042",
            asset_definition_id="pkr#sbp",
            offline_payments_enabled=True,
            issuer_public_key_base64=issuer_key,
            native_bridge_abi_version=7,
        )
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            noncanonical.require_usable_for_offline_exchange(
                now_ms=200,
                required_native_bridge_abi_version=7,
            )
        assert error.value.code == "missing_issuer_public_key"

    for field_name, value in (
        ("created_at_ms", -1),
        ("created_at_ms", 100.5),
        ("created_at_ms", True),
        ("expires_at_ms", -1),
        ("expires_at_ms", 100.5),
        ("expires_at_ms", True),
        ("expires_at_ms", 100),
    ):
        malformed_time_kwargs = dict(snapshot_kwargs)
        malformed_time_kwargs[field_name] = value
        malformed_time = OfflineCashConfigurationSnapshot(**malformed_time_kwargs)  # type: ignore[arg-type]
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            malformed_time.require_usable_for_offline_exchange(
                now_ms=200,
                required_native_bridge_abi_version=7,
            )
        assert error.value.code == "malformed_snapshot"
        assert field_name in str(error.value)

    for now_ms in (-1, 999.5, True):
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            snapshot.require_usable_for_offline_exchange(
                now_ms=now_ms,  # type: ignore[arg-type]
                required_native_bridge_abi_version=7,
            )
        assert error.value.code == "malformed_snapshot"
        assert "now_ms" in str(error.value)

    for offline_payments_enabled in (False, "true", 1):
        disabled = OfflineCashConfigurationSnapshot(
            chain_id="00000042",
            asset_definition_id="pkr#sbp",
            offline_payments_enabled=offline_payments_enabled,  # type: ignore[arg-type]
            issuer_public_key_base64=ISSUER_PUBLIC_KEY_BASE64,
            native_bridge_abi_version=7,
        )
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            disabled.require_usable_for_offline_exchange(
                now_ms=200,
                required_native_bridge_abi_version=7,
            )
        assert error.value.code == "offline_payments_disabled"

    stale_abi = OfflineCashConfigurationSnapshot(
        chain_id="00000042",
        asset_definition_id="pkr#sbp",
        offline_payments_enabled=True,
        issuer_public_key_base64=ISSUER_PUBLIC_KEY_BASE64,
        native_bridge_abi_version=6,
    )
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        stale_abi.require_usable_for_offline_exchange(now_ms=200, required_native_bridge_abi_version=7)
    assert error.value.code == "unsupported_native_bridge_abi"

    for native_bridge_abi_version in (0, -1, 7.5, True):
        malformed_native_abi = OfflineCashConfigurationSnapshot(
            chain_id="00000042",
            asset_definition_id="pkr#sbp",
            offline_payments_enabled=True,
            issuer_public_key_base64=ISSUER_PUBLIC_KEY_BASE64,
            native_bridge_abi_version=native_bridge_abi_version,  # type: ignore[arg-type]
        )
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            malformed_native_abi.require_usable_for_offline_exchange(
                now_ms=200,
                required_native_bridge_abi_version=7,
            )
        assert error.value.code == "malformed_snapshot"

    for required_native_bridge_abi_version in (0, -1, 7.5, True):
        with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
            snapshot.require_usable_for_offline_exchange(
                now_ms=999,
                required_native_bridge_abi_version=required_native_bridge_abi_version,  # type: ignore[arg-type]
            )
        assert error.value.code == "malformed_snapshot"

    expired = snapshot
    with pytest.raises(OfflineCashConfigurationSnapshotError) as error:
        expired.require_usable_for_offline_exchange(now_ms=1000, required_native_bridge_abi_version=7)
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
