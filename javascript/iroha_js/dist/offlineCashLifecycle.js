export const KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME =
  "iroha_data_model::isi::offline::KagemushaTransfer";
export const KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME =
  "iroha_data_model::isi::offline::RedeemKagemushaRecursive";
export const KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME =
  "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1";

export const OFFLINE_CASH_TRANSPORT_QR = "qr";
export const OFFLINE_CASH_TRANSPORT_NFC = "nfc";
export const OFFLINE_CASH_TRANSPORT_NEARBY = "nearby";

export class OfflineCashConfigurationSnapshotError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "OfflineCashConfigurationSnapshotError";
    this.code = code;
  }
}

export function assertOfflineCashConfigurationSnapshotUsable(
  snapshot,
  { nowMs = Date.now(), requiredBridgeAbiVersion = null } = {},
) {
  if (!snapshot || typeof snapshot !== "object") {
    throw new TypeError("offline cash configuration snapshot must be an object");
  }
  if (!snapshot.offlinePaymentsEnabled) {
    throw new OfflineCashConfigurationSnapshotError(
      "offline_payments_disabled",
      "Offline cash is disabled in the cached configuration snapshot.",
    );
  }
  if (String(snapshot.issuerPublicKeyBase64 ?? "").trim().length === 0) {
    throw new OfflineCashConfigurationSnapshotError(
      "missing_issuer_public_key",
      "Offline cash requires a cached issuer public key before offline exchange.",
    );
  }
  const expiresAtMs = snapshot.expiresAtMs;
  if (expiresAtMs !== null && expiresAtMs !== undefined && Number(expiresAtMs) <= nowMs) {
    throw new OfflineCashConfigurationSnapshotError(
      "expired",
      `Offline cash configuration snapshot expired at ${expiresAtMs}.`,
    );
  }
  if (
    requiredBridgeAbiVersion !== null &&
    requiredBridgeAbiVersion !== undefined &&
    Number(snapshot.bridgeAbiVersion ?? -1) < Number(requiredBridgeAbiVersion)
  ) {
    throw new OfflineCashConfigurationSnapshotError(
      "unsupported_bridge_abi",
      `Offline cash requires bridge ABI ${requiredBridgeAbiVersion}.`,
    );
  }
  return true;
}

export function offlineCashAvailableTransportKinds(capabilities = {}) {
  const kinds = [];
  if (capabilities.qrStreaming !== false && capabilities.qr !== false) {
    kinds.push(OFFLINE_CASH_TRANSPORT_QR);
  }
  const nfc = capabilities.nfc;
  const nfcSupported = nfc === true || (nfc && typeof nfc === "object" && nfc.supported === true);
  if (nfcSupported) {
    kinds.push(OFFLINE_CASH_TRANSPORT_NFC);
  }
  if (capabilities.nearby !== false) {
    kinds.push(OFFLINE_CASH_TRANSPORT_NEARBY);
  }
  return kinds;
}

export class OfflineCashLifecycleController {
  constructor({ wallet, auditReceiptSynchronizer = null } = {}) {
    if (!wallet || typeof wallet !== "object") {
      throw new TypeError("OfflineCashLifecycleController requires a wallet object");
    }
    this.wallet = wallet;
    this.auditReceiptSynchronizer = auditReceiptSynchronizer;
  }

  async syncPendingAuditReceiptsIfNeeded() {
    const synchronizer = this.auditReceiptSynchronizer;
    if (!synchronizer) {
      return false;
    }
    const hasPending =
      typeof synchronizer.hasPendingAuditReceipts === "function"
        ? await synchronizer.hasPendingAuditReceipts()
        : Boolean(synchronizer.hasPendingAuditReceipts);
    if (!hasPending) {
      return false;
    }
    if (typeof synchronizer.syncPendingAuditReceipts !== "function") {
      throw new TypeError("auditReceiptSynchronizer.syncPendingAuditReceipts must be a function");
    }
    await synchronizer.syncPendingAuditReceipts();
    return true;
  }

  async load(assetDefinitionId, amount) {
    if (typeof this.wallet.load !== "function") {
      throw new TypeError("wallet.load must be a function");
    }
    await this.syncPendingAuditReceiptsIfNeeded();
    return this.wallet.load(assetDefinitionId, amount);
  }

  prepareReceive(assetDefinitionId, amount) {
    if (typeof this.wallet.prepareReceive !== "function") {
      throw new TypeError("wallet.prepareReceive must be a function");
    }
    return this.wallet.prepareReceive(assetDefinitionId, amount);
  }

  createPayment(receiveRequest) {
    if (typeof this.wallet.pay !== "function") {
      throw new TypeError("wallet.pay must be a function");
    }
    return this.wallet.pay(receiveRequest);
  }

  acceptPayment(paymentToken) {
    if (typeof this.wallet.accept !== "function") {
      throw new TypeError("wallet.accept must be a function");
    }
    return this.wallet.accept(paymentToken);
  }

  redeem(note, recipient = null) {
    if (typeof this.wallet.redeem !== "function") {
      throw new TypeError("wallet.redeem must be a function");
    }
    return this.wallet.redeem(note, recipient);
  }
}
