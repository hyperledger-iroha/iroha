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
  { nowMs = Date.now(), requiredNativeBridgeAbiVersion = null } = {},
) {
  if (!snapshot || typeof snapshot !== "object") {
    throw new TypeError("offline cash configuration snapshot must be an object");
  }
  const checkedNowMs = nonnegativeIntegerOfflineCashSnapshotTimestamp(nowMs, "nowMs");
  if (snapshot.offlinePaymentsEnabled !== true) {
    throw new OfflineCashConfigurationSnapshotError(
      "offline_payments_disabled",
      "Offline cash is disabled in the cached configuration snapshot.",
    );
  }
  requireCanonicalOfflineCashSnapshotText(snapshot.chainId, "chainId");
  requireCanonicalOfflineCashSnapshotText(snapshot.assetDefinitionId, "assetDefinitionId");
  requireOptionalCanonicalOfflineCashSnapshotText(snapshot.artifactSetId, "artifactSetId");
  requireOptionalCanonicalOfflineCashSnapshotText(snapshot.circuitId, "circuitId");
  if (!isValidOfflineIssuerPublicKeyBase64(snapshot.issuerPublicKeyBase64)) {
    throw new OfflineCashConfigurationSnapshotError(
      "missing_issuer_public_key",
      "Offline cash requires a cached issuer public key before offline exchange.",
    );
  }
  const createdAtMs = nonnegativeIntegerOfflineCashSnapshotTimestamp(
    snapshot.createdAtMs,
    "createdAtMs",
  );
  const expiresAtMs = snapshot.expiresAtMs;
  const checkedExpiresAtMs =
    expiresAtMs === null || expiresAtMs === undefined
      ? null
      : nonnegativeIntegerOfflineCashSnapshotTimestamp(expiresAtMs, "expiresAtMs");
  if (checkedExpiresAtMs !== null && checkedExpiresAtMs <= createdAtMs) {
    throw new OfflineCashConfigurationSnapshotError(
      "malformed_snapshot",
      "Offline cash configuration snapshot field expiresAtMs must be after createdAtMs.",
    );
  }
  if (
    checkedExpiresAtMs !== null &&
    checkedExpiresAtMs <= checkedNowMs
  ) {
    throw new OfflineCashConfigurationSnapshotError(
      "expired",
      `Offline cash configuration snapshot expired at ${checkedExpiresAtMs}.`,
    );
  }
  const nativeBridgeAbiVersion = snapshot.nativeBridgeAbiVersion;
  const checkedNativeBridgeAbiVersion =
    nativeBridgeAbiVersion === null || nativeBridgeAbiVersion === undefined
      ? null
      : positiveIntegerOfflineCashSnapshotNumber(nativeBridgeAbiVersion, "nativeBridgeAbiVersion");
  const checkedRequiredNativeBridgeAbiVersion =
    requiredNativeBridgeAbiVersion === null || requiredNativeBridgeAbiVersion === undefined
      ? null
      : positiveIntegerOfflineCashSnapshotNumber(
          requiredNativeBridgeAbiVersion,
          "requiredNativeBridgeAbiVersion",
        );
  if (
    checkedRequiredNativeBridgeAbiVersion !== null &&
    (checkedNativeBridgeAbiVersion === null || checkedNativeBridgeAbiVersion < checkedRequiredNativeBridgeAbiVersion)
  ) {
    throw new OfflineCashConfigurationSnapshotError(
      "unsupported_native_bridge_abi",
      `Offline cash requires native bridge ABI ${requiredNativeBridgeAbiVersion}.`,
    );
  }
  return true;
}

function isCanonicalOfflineCashSnapshotText(value) {
  return typeof value === "string" && /^[\x21-\x7E]+$/.test(value);
}

function requireCanonicalOfflineCashSnapshotText(value, fieldName) {
  if (!isCanonicalOfflineCashSnapshotText(value)) {
    throw new OfflineCashConfigurationSnapshotError(
      "malformed_snapshot",
      `Offline cash configuration snapshot field ${fieldName} must be a non-empty printable ASCII string with no whitespace.`,
    );
  }
}

function requireOptionalCanonicalOfflineCashSnapshotText(value, fieldName) {
  if (value === null || value === undefined) {
    return;
  }
  requireCanonicalOfflineCashSnapshotText(value, fieldName);
}

const OFFLINE_ISSUER_PUBLIC_KEY_BASE64_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function isValidOfflineIssuerPublicKeyBase64(value) {
  if (!isCanonicalOfflineCashSnapshotText(value) || value.includes("=")) {
    return false;
  }
  const normalized = value.replaceAll("-", "+").replaceAll("_", "/");
  if (!/^[A-Za-z0-9+/]+$/.test(normalized) || normalized.length % 4 === 1) {
    return false;
  }
  const decoded = decodeUnpaddedBase64(normalized);
  return (
    decoded !== null &&
    decoded.length === 32 &&
    encodeUnpaddedBase64(decoded) === normalized
  );
}

function decodeUnpaddedBase64(normalized) {
  const padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4);
  const bytes = [];
  for (let index = 0; index < padded.length; index += 4) {
    const first = base64Index(padded[index]);
    const second = base64Index(padded[index + 1]);
    const thirdChar = padded[index + 2];
    const fourthChar = padded[index + 3];
    const third = thirdChar === "=" ? 0 : base64Index(thirdChar);
    const fourth = fourthChar === "=" ? 0 : base64Index(fourthChar);
    if (first < 0 || second < 0 || third < 0 || fourth < 0) {
      return null;
    }
    const triple = (first << 18) | (second << 12) | (third << 6) | fourth;
    bytes.push((triple >> 16) & 0xff);
    if (thirdChar !== "=") {
      bytes.push((triple >> 8) & 0xff);
    }
    if (fourthChar !== "=") {
      bytes.push(triple & 0xff);
    }
  }
  return bytes;
}

function encodeUnpaddedBase64(bytes) {
  let encoded = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const remaining = bytes.length - index;
    const first = bytes[index];
    const second = remaining > 1 ? bytes[index + 1] : 0;
    const third = remaining > 2 ? bytes[index + 2] : 0;
    encoded += OFFLINE_ISSUER_PUBLIC_KEY_BASE64_ALPHABET[first >> 2];
    encoded += OFFLINE_ISSUER_PUBLIC_KEY_BASE64_ALPHABET[((first & 0x03) << 4) | (second >> 4)];
    if (remaining > 1) {
      encoded += OFFLINE_ISSUER_PUBLIC_KEY_BASE64_ALPHABET[((second & 0x0f) << 2) | (third >> 6)];
    }
    if (remaining > 2) {
      encoded += OFFLINE_ISSUER_PUBLIC_KEY_BASE64_ALPHABET[third & 0x3f];
    }
  }
  return encoded;
}

function base64Index(char) {
  return OFFLINE_ISSUER_PUBLIC_KEY_BASE64_ALPHABET.indexOf(char);
}

function finiteOfflineCashSnapshotNumber(value, fieldName) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new OfflineCashConfigurationSnapshotError(
      "malformed_snapshot",
      `Offline cash configuration snapshot field ${fieldName} must be a finite number.`,
    );
  }
  return value;
}

function nonnegativeIntegerOfflineCashSnapshotTimestamp(value, fieldName) {
  const checked = finiteOfflineCashSnapshotNumber(value, fieldName);
  if (!Number.isSafeInteger(checked) || checked < 0) {
    throw new OfflineCashConfigurationSnapshotError(
      "malformed_snapshot",
      `Offline cash configuration snapshot field ${fieldName} must be a nonnegative integer timestamp.`,
    );
  }
  return checked;
}

function positiveIntegerOfflineCashSnapshotNumber(value, fieldName) {
  const checked = finiteOfflineCashSnapshotNumber(value, fieldName);
  if (!Number.isSafeInteger(checked) || checked <= 0) {
    throw new OfflineCashConfigurationSnapshotError(
      "malformed_snapshot",
      `Offline cash configuration snapshot field ${fieldName} must be a positive integer.`,
    );
  }
  return checked;
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
