"""Headless offline-cash lifecycle and transport helpers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Optional, Protocol, Sequence, Union

KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME = (
    "iroha_data_model::isi::offline::KagemushaTransfer"
)
KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME = (
    "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
)
KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
)

OFFLINE_CASH_TRANSPORT_QR = "qr"
OFFLINE_CASH_TRANSPORT_NFC = "nfc"
OFFLINE_CASH_TRANSPORT_NEARBY = "nearby"


class OfflineCashConfigurationSnapshotError(ValueError):
    """Raised when a cached offline-cash snapshot cannot support offline exchange."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class OfflineCashConfigurationSnapshot:
    chain_id: str
    asset_definition_id: str
    offline_payments_enabled: bool
    issuer_public_key_base64: Optional[str]
    native_bridge_abi_version: Optional[int] = None
    artifact_set_id: Optional[str] = None
    circuit_id: Optional[str] = None
    created_at_ms: int = 0
    expires_at_ms: Optional[int] = None

    def require_usable_for_offline_exchange(
        self,
        *,
        now_ms: int,
        required_native_bridge_abi_version: Optional[int] = None,
    ) -> None:
        if not self.offline_payments_enabled:
            raise OfflineCashConfigurationSnapshotError(
                "offline_payments_disabled",
                "Offline cash is disabled in the cached configuration snapshot.",
            )
        if not (self.issuer_public_key_base64 or "").strip():
            raise OfflineCashConfigurationSnapshotError(
                "missing_issuer_public_key",
                "Offline cash requires a cached issuer public key before offline exchange.",
            )
        if self.expires_at_ms is not None and self.expires_at_ms <= now_ms:
            raise OfflineCashConfigurationSnapshotError(
                "expired",
                f"Offline cash configuration snapshot expired at {self.expires_at_ms}.",
            )
        if (
            required_native_bridge_abi_version is not None
            and (self.native_bridge_abi_version or -1) < required_native_bridge_abi_version
        ):
            raise OfflineCashConfigurationSnapshotError(
                "unsupported_native_bridge_abi",
                f"Offline cash requires native bridge ABI {required_native_bridge_abi_version}.",
            )


@dataclass(frozen=True)
class OfflineCashNfcCapability:
    supported: bool
    reason: Optional[str] = None


@dataclass(frozen=True)
class OfflineCashTransportCapabilities:
    qr_streaming: bool = True
    nfc: Optional[OfflineCashNfcCapability] = OfflineCashNfcCapability(
        supported=False,
        reason="NFC requires device and app HCE support.",
    )
    nearby: bool = True

    def supported_transport_kinds(self) -> tuple[str, ...]:
        kinds: list[str] = []
        if self.qr_streaming:
            kinds.append(OFFLINE_CASH_TRANSPORT_QR)
        if self.nfc is not None and self.nfc.supported:
            kinds.append(OFFLINE_CASH_TRANSPORT_NFC)
        if self.nearby:
            kinds.append(OFFLINE_CASH_TRANSPORT_NEARBY)
        return tuple(kinds)


class OfflineCashWalletProtocol(Protocol):
    async def load(self, asset_definition_id: str, amount: str) -> Any: ...

    def prepare_receive(self, asset_definition_id: str, amount: str) -> Any: ...

    def pay(self, receive_request: Any) -> Any: ...

    def accept(self, payment_token: Any) -> Any: ...

    async def redeem(self, note: Any, recipient: Optional[str] = None) -> Any: ...


PendingReceiptsCallable = Callable[[], Union[bool, Awaitable[bool]]]
SyncReceiptsCallable = Callable[[], Union[None, Awaitable[None]]]


class OfflineCashLifecycleController:
    def __init__(
        self,
        wallet: OfflineCashWalletProtocol,
        *,
        has_pending_audit_receipts: Optional[PendingReceiptsCallable] = None,
        sync_pending_audit_receipts: Optional[SyncReceiptsCallable] = None,
    ) -> None:
        self._wallet = wallet
        self._has_pending_audit_receipts = has_pending_audit_receipts
        self._sync_pending_audit_receipts = sync_pending_audit_receipts

    async def sync_pending_audit_receipts_if_needed(self) -> bool:
        if self._has_pending_audit_receipts is None:
            return False
        pending = self._has_pending_audit_receipts()
        if hasattr(pending, "__await__"):
            pending = await pending  # type: ignore[assignment]
        if not pending:
            return False
        if self._sync_pending_audit_receipts is None:
            raise TypeError("sync_pending_audit_receipts is required when receipts are pending")
        synced = self._sync_pending_audit_receipts()
        if hasattr(synced, "__await__"):
            await synced
        return True

    async def load(self, asset_definition_id: str, amount: str) -> Any:
        await self.sync_pending_audit_receipts_if_needed()
        return await self._wallet.load(asset_definition_id, amount)

    def prepare_receive(self, asset_definition_id: str, amount: str) -> Any:
        return self._wallet.prepare_receive(asset_definition_id, amount)

    def create_payment(self, receive_request: Any) -> Any:
        return self._wallet.pay(receive_request)

    def accept_payment(self, payment_token: Any) -> Any:
        return self._wallet.accept(payment_token)

    async def redeem(self, note: Any, recipient: Optional[str] = None) -> Any:
        return await self._wallet.redeem(note, recipient)


def offline_cash_available_transport_kinds(
    capabilities: OfflineCashTransportCapabilities,
) -> Sequence[str]:
    return capabilities.supported_transport_kinds()
