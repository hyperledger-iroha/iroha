// SPDX-License-Identifier: Apache-2.0

import { sha256 } from "@noble/hashes/sha2";

import { crc64Xz } from "./crc64Xz.js";
import { NetworkId, networkIdBytes } from "./networkId.js";
import {
  _canonicalAccountIdNoritoValue,
  encodeAccountIdNoritoValue,
  encodeAssetDefinitionIdNoritoValue,
  noritoDecodeInstruction,
  noritoEncodeInstruction,
  validateNoritoFrame,
} from "./norito.js";

const UTF8 = new TextEncoder();
const MODEL = "iroha_data_model::kagemusha::kagemusha_v1::";
const DEVICE_MODEL = "iroha_data_model::kagemusha::kagemusha_device_v1::";
const MAX_U64 = (1n << 64n) - 1n;
const MAX_U128 = (1n << 128n) - 1n;
const COMPACT_LENGTHS = 0x02;
const HEADER_BYTES = 40;
const CREDIT_OPENING_BYTES = 200;
const ENCRYPTED_CREDIT_BYTES = CREDIT_OPENING_BYTES + 16;
const TOP_UP_REQUEST_MAX_BYTES = 16 * 1024;
const DEVICE_MINT_STAGE_COMMAND_MAX_BYTES = 64 * 1024;
const DEVICE_MINT_STAGE_RESULT_MAX_BYTES = 128;
const TOP_UP_INSTRUCTION_WIRE_ID = "iroha.kagemusha.v1.top_up";

const SCHEMAS = Object.freeze({
  request: `${MODEL}KagemushaPaymentRequestV1`,
  peerCreditContext: `${MODEL}KagemushaPeerCreditContextV1`,
  lifecycle: `${MODEL}KagemushaLifecycleBindingV1`,
  commitCertificate: `${MODEL}KagemushaCommitCertificateV1`,
  redemptionProof: `${MODEL}KagemushaRedemptionProofV1`,
  paymentProof: `${MODEL}KagemushaPaymentProofV1`,
  paymentOutput: `${MODEL}KagemushaPaymentOutputV1`,
  payment: `${MODEL}KagemushaPaymentV1`,
  acknowledgement: `${MODEL}KagemushaAcknowledgementV1`,
  mintAuthorizationContext: `${MODEL}KagemushaMintAuthorizationContextV1`,
  mintAuthorizationStatement: `${MODEL}KagemushaMintAuthorizationStatementV1`,
  mintAuthorization: `${MODEL}KagemushaMintAuthorizationV1`,
  mintStatement: `${MODEL}KagemushaMintCreditStatementV1`,
  mintCredit: `${MODEL}KagemushaMintCreditV1`,
  deviceMintStageCommand: `${DEVICE_MODEL}KagemushaDeviceMintStageCommandV1`,
  deviceMintStageResult: `${DEVICE_MODEL}KagemushaDeviceMintStageResultV1`,
  redemptionStatement: `${MODEL}KagemushaRedemptionStatementV1`,
  redemptionVoucher: `${MODEL}KagemushaRedemptionVoucherV1`,
  encryptedCreditEnvelope: `${MODEL}KagemushaEncryptedCreditEnvelopeV1`,
  encryptedCreditAad: `${MODEL}KagemushaEncryptedCreditAadV1`,
  creditOpening: `${MODEL}KagemushaCreditOpeningV1`,
  topUpRequest: "iroha.torii.v1.kagemusha.top_up.request",
  redemptionRequest: "iroha.torii.v1.kagemusha.redeem.request",
});

const DOMAIN = Object.freeze({
  deviceKeyReference: ascii("iroha:kagemusha:v1:device-key-reference"),
  pastaStateCommitment: ascii("iroha:kagemusha:v1:pasta-state-commitment"),
  liabilityPool: ascii("iroha:kagemusha:v1:liability-pool"),
  requestSigning: ascii("iroha:kagemusha:v1:payment-request-signing"),
  requestDigest: ascii("iroha:kagemusha:v1:payment-request"),
  peerCreditContextDigest: ascii("iroha:kagemusha:v1:peer-credit-context"),
  peerCreditOpeningCommitment: ascii("iroha:kagemusha:v1:peer-credit-opening-commitment"),
  creditId: ascii("iroha:kagemusha:v1:credit-id"),
  lifecycleDigest: ascii("iroha:kagemusha:v1:lifecycle-binding"),
  commitCertificateId: ascii("iroha:kagemusha:v1:commit-certificate-id"),
  commitCertificateDigest: ascii("iroha:kagemusha:v1:commit-certificate"),
  statementDigest: ascii("iroha:kagemusha:v1:send-split-statement"),
  paymentDigest: ascii("iroha:kagemusha:v1:payment"),
  preparedTransfer: ascii("iroha:kagemusha:v1:prepared-transfer"),
  paymentBodyDigest: ascii("iroha:kagemusha:v1:payment-body"),
  assetIdentity: ascii("iroha:kagemusha:v1:asset-identity"),
  accountIdentity: ascii("iroha:kagemusha:v1:account-identity"),
  ciphertextDigest: ascii("iroha:kagemusha:v1:ciphertext"),
  mintAuthorizationContextDigest: ascii("iroha:kagemusha:v1:mint-authorization-context"),
  mintAuthorizationStatementDigest: ascii("iroha:kagemusha:v1:mint-authorization-statement"),
  mintAuthorizationDigest: ascii("iroha:kagemusha:v1:mint-authorization"),
  mintStatementDigest: ascii("iroha:kagemusha:v1:mint-statement"),
  mintLifecycleContextDigest: ascii("iroha:kagemusha:v1:mint-lifecycle-context"),
  mintCreditId: ascii("iroha:kagemusha:v1:mint-credit-id"),
  redemptionStatementDigest: ascii("iroha:kagemusha:v1:redemption-statement"),
  redemptionId: ascii("iroha:kagemusha:v1:redemption-id"),
});

const LIMITS = Object.freeze({
  paymentRequest: [928, 1243],
  payment: [7552, 10075],
  acknowledgement: [256, 347],
  mintAuthorization: [7936, 10587],
  mintCredit: [7936, 10587],
  redemptionVoucher: [7936, 10587],
});

const PAYLOAD_KINDS = Object.freeze(Object.fromEntries(
  Object.entries(LIMITS).map(([name, [maximumRawBytes, maximumTextBytes]]) => [
    name,
    Object.freeze({ maximumRawBytes, maximumTextBytes }),
  ]),
));

const IPM1_PAYLOAD_KINDS = Object.freeze({
  request: Object.freeze({ tag: 1, payloadKind: "paymentRequest" }),
  payment: Object.freeze({ tag: 2, payloadKind: "payment" }),
  acknowledgement: Object.freeze({ tag: 3, payloadKind: "acknowledgement" }),
});

const COMPLETE_EXCHANGE_TARGET_RAW_BYTES = 8960;
const COMPLETE_EXCHANGE_MAX_RAW_BYTES = 9211;
const COMPLETE_EXCHANGE_MAX_TEXT_BYTES = 12288;

class KagemushaAssetDefinitionIdV1 {
  #payload;

  constructor(value) {
    const payload = typeof value === "string"
      ? encodeAssetDefinitionIdNoritoValue(value, "KAGEMUSHA V1 asset")
      : bytes(value, "KAGEMUSHA V1 asset payload");
    requireFixedArchive(payload, 16, "KAGEMUSHA V1 asset payload");
    this.#payload = Uint8Array.from(payload);
    Object.freeze(this);
  }

  canonicalPayload() { return Uint8Array.from(this.#payload); }
}

class KagemushaAccountIdV1 {
  #payload;

  constructor(value) {
    const payload = typeof value === "string"
      ? encodeAccountIdNoritoValue(value, "KAGEMUSHA V1 account")
      : bytes(value, "KAGEMUSHA V1 account payload");
    if (payload.length === 0 || payload.length > 512) throw new RangeError("KAGEMUSHA V1 account payload is empty or oversized");
    const canonical = _canonicalAccountIdNoritoValue(payload, "KAGEMUSHA V1 account");
    if (!equalBytes(payload, canonical)) throw new TypeError("KAGEMUSHA V1 account payload is not canonical");
    this.#payload = Uint8Array.from(payload);
    Object.freeze(this);
  }

  canonicalPayload() { return Uint8Array.from(this.#payload); }
}

class KagemushaAssetIncarnationV1 {
  #hash;

  constructor(value) {
    const raw = bytes(value, "KAGEMUSHA V1 asset incarnation");
    if (raw.length !== 32 || (raw[31] & 1) !== 1) throw new TypeError("KAGEMUSHA V1 asset incarnation must be a marked 32-byte Iroha hash");
    this.#hash = Uint8Array.from(raw);
    Object.freeze(this);
  }

  hashBytes() { return Uint8Array.from(this.#hash); }
}

class KagemushaDevicePublicKeyV1 {
  #bytes;

  constructor(value) {
    const raw = bytes(value, "KAGEMUSHA V1 device public key");
    if (raw.length !== 65 || raw[0] !== 4 || isZero(raw.subarray(1))) throw new TypeError("KAGEMUSHA V1 device public key must be nonzero 65-byte uncompressed SEC1");
    this.#bytes = Uint8Array.from(raw);
    Object.freeze(this);
  }

  sec1Bytes() { return Uint8Array.from(this.#bytes); }
}

class KagemushaDeviceSignatureV1 {
  #bytes;

  constructor(value) {
    const raw = bytes(value, "KAGEMUSHA V1 device signature");
    if (raw.length !== 64 || isZero(raw.subarray(0, 32)) || isZero(raw.subarray(32))) throw new TypeError("KAGEMUSHA V1 device signature must be nonzero fixed-width r || s");
    this.#bytes = Uint8Array.from(raw);
    Object.freeze(this);
  }

  rawBytes() { return Uint8Array.from(this.#bytes); }
}

const T = Object.freeze({
  U8: "u8", U16: "u16", U32: "u32", U64: "u64", U128: "u128", FIXED32: "fixed32", RAW32: "raw32",
  FIXED24: "fixed24", NETWORK: "network", ASSET: "asset", INCARNATION: "incarnation", ACCOUNT: "account",
  PUBLIC_KEY: "publicKey", SIGNATURE: "signature", VECTOR: "vector",
  OPERATION_KIND: "operationKind", CREDIT_PURPOSE: "creditPurpose",
  COMMIT_EVIDENCE: "commitEvidence",
  OPTIONAL_MINT_AUTHORIZATION: "optionalMintAuthorization",
  MINT_FRAME: "mintFrame",
});

const DEFINITIONS = {};
const MODEL_VALUES = new WeakMap();

function defineModel(name, fields, validate) {
  const Model = class {
    constructor(input) {
      exactRecord(input, name, fields.map(([fieldName]) => fieldName));
      const normalized = {};
      for (const [fieldName, type] of fields) normalized[fieldName] = normalizeType(type, input[fieldName], `${name}.${fieldName}`);
      validate?.(normalized);
      MODEL_VALUES.set(this, normalized);
      Object.freeze(this);
    }
  };
  Object.defineProperty(Model, "name", { value: name });
  for (const [fieldName] of fields) {
    Object.defineProperty(Model.prototype, fieldName, {
      enumerable: true,
      get() { return cloneValue(MODEL_VALUES.get(this)[fieldName]); },
    });
  }
  DEFINITIONS[name] = { Model, fields };
  return Model;
}

const KagemushaHardwareCredentialV1 = defineModel(
  "KagemushaHardwareCredentialV1",
  [["version", T.U16], ["credentialId", T.FIXED32], ["networkId", T.NETWORK], ["hardwareProfileId", T.FIXED32],
    ["suiteId", T.FIXED32], ["firmwarePolicyDigest", T.FIXED32], ["policyEpoch", T.U64], ["laneCommitment", T.FIXED32],
    ["hardwareEpochId", T.FIXED32], ["hardwareEpochGeneration", T.U64], ["devicePublicKey", T.PUBLIC_KEY],
    ["deviceKeyReference", T.FIXED32], ["issuedAtMs", T.U64], ["expiresAtMs", T.U64], ["governanceSignature", T.SIGNATURE]],
  (v) => {
    requireVersion(v.version);
    if (v.policyEpoch === 0n || v.issuedAtMs >= v.expiresAtMs) throw new TypeError("KAGEMUSHA V1 hardware credential header is invalid");
    requireEqual(v.deviceKeyReference, deviceKeyReference(v.devicePublicKey), "hardware credential device key reference");
  },
);

const KagemushaPastaStateCommitmentV1 = defineModel(
  "KagemushaPastaStateCommitmentV1", [["eq", T.RAW32], ["ep", T.RAW32]],
  (v) => { if (isZero(v.eq) !== isZero(v.ep)) throw new TypeError("Pasta state commitment must be fully zero or fully present"); },
);

const KagemushaPairedProofV1 = defineModel(
  "KagemushaPairedProofV1",
  [["version", T.U16], ["eqProtocolDigest", T.FIXED32], ["epProtocolDigest", T.FIXED32], ["semanticDigest", T.FIXED32],
    ["guardEqCredentialAudit", T.FIXED32], ["guardEpCredentialAudit", T.FIXED32], ["eqDeferredAudit", T.FIXED32],
    ["epDeferredAudit", T.FIXED32], ["eqProof", T.VECTOR], ["epProof", T.VECTOR], ["eqHistory", T.VECTOR], ["epHistory", T.VECTOR]],
  validatePairedProofValues,
);

const KagemushaCreditOpeningV1 = defineModel(
  "KagemushaCreditOpeningV1",
  [["version", T.U16], ["creditId", T.FIXED32], ["amount", T.U128], ["creditCommitmentOpening", T.FIXED32],
    ["recipientBindingOpening", T.FIXED32], ["recoveryNonce", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.amount === 0n) throw new TypeError("KAGEMUSHA V1 credit opening amount must be positive"); },
);
const KagemushaEncryptedCreditAadV1 = defineModel(
  "KagemushaEncryptedCreditAadV1",
  [["version", T.U16], ["purpose", T.CREDIT_PURPOSE], ["contextDigest", T.FIXED32], ["issuanceOrTransitionCommitment", T.FIXED32],
    ["creditId", T.FIXED32], ["amount", T.U128]],
  (v) => { requireVersion(v.version); if (v.amount === 0n) throw new TypeError("KAGEMUSHA V1 encrypted-credit AAD amount must be positive"); },
);
const KagemushaEncryptedCreditEnvelopeV1 = defineModel(
  "KagemushaEncryptedCreditEnvelopeV1",
  [["version", T.U16], ["ephemeralX25519PublicKey", T.RAW32], ["nonce", T.FIXED24], ["ciphertextAndTag", T.VECTOR]],
  (v) => {
    requireVersion(v.version);
    requireX25519Key(v.ephemeralX25519PublicKey, "encrypted-credit ephemeral key");
    if (v.ciphertextAndTag.length !== ENCRYPTED_CREDIT_BYTES) throw new TypeError(`KAGEMUSHA V1 ciphertext and tag must be exactly ${ENCRYPTED_CREDIT_BYTES} bytes`);
  },
);

const OPERATION_KINDS = Object.freeze(["bootstrap", "mintFold", "sendSplit", "receiveFold", "redeemSplit", "rotate"]);
const CREDIT_PURPOSES = Object.freeze(["mint", "peer"]);
const KagemushaLifecycleBindingV1 = defineModel(
  "KagemushaLifecycleBindingV1",
  [["version", T.U16], ["networkId", T.NETWORK], ["protocolVersion", T.U16], ["suiteId", T.FIXED32], ["vkDigest", T.FIXED32],
    ["releaseId", T.FIXED32], ["asset", T.ASSET], ["assetIncarnation", T.INCARNATION], ["scale", T.U32],
    ["liabilityPoolId", T.FIXED32], ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64], ["operationKind", T.OPERATION_KIND],
    ["requestId", T.RAW32], ["receiverLaneCommitment", T.RAW32], ["creditId", T.RAW32], ["ciphertextDigest", T.RAW32]],
  (v) => {
    requireVersion(v.version);
    if (v.protocolVersion !== 1 || v.policyEpoch === 0n) throw new TypeError("KAGEMUSHA V1 lifecycle header is invalid");
    requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "lifecycle liability pool");
    const requestFieldsAreSet = !isZero(v.requestId) && !isZero(v.receiverLaneCommitment);
    const requestFieldsAreZero = isZero(v.requestId) && isZero(v.receiverLaneCommitment);
    const creditFieldsAreSet = !isZero(v.creditId) && !isZero(v.ciphertextDigest);
    const allAreZero = [v.requestId, v.receiverLaneCommitment, v.creditId, v.ciphertextDigest].every(isZero);
    if ((v.operationKind === "sendSplit" && !(requestFieldsAreSet && creditFieldsAreSet))
        || (v.operationKind === "mintFold" && (!requestFieldsAreZero || !creditFieldsAreSet))
        || (!new Set(["sendSplit", "mintFold"]).has(v.operationKind) && !allAreZero)) {
      throw new TypeError("KAGEMUSHA V1 lifecycle operation identities are invalid");
    }
  },
);
const KagemushaPaymentRequestV1 = defineModel(
  "KagemushaPaymentRequestV1",
  [["version", T.U16], ["releaseId", T.FIXED32], ["networkId", T.NETWORK], ["asset", T.ASSET], ["assetIncarnation", T.INCARNATION],
    ["scale", T.U32], ["liabilityPoolId", T.FIXED32], ["recipient", T.ACCOUNT], ["amount", T.U128],
    ["recipientEncryptionKey", T.FIXED32],
    ["hardwareCredential", "KagemushaHardwareCredentialV1"], ["requestId", T.FIXED32],
    ["issuedAtMs", T.U64], ["expiresAtMs", T.U64],
    ["signature", T.SIGNATURE]],
  (v) => {
    header(v);
    if (v.amount === 0n) throw new TypeError("KAGEMUSHA V1 request amount must be positive");
    requireX25519Key(v.recipientEncryptionKey, "request recipient encryption key");
    if (v.expiresAtMs <= v.issuedAtMs || v.expiresAtMs - v.issuedAtMs > 300000n) throw new RangeError("KAGEMUSHA V1 request validity window is invalid");
    if (!equalBytes(networkIdBytes(v.networkId), networkIdBytes(v.hardwareCredential.networkId))
        || v.issuedAtMs < v.hardwareCredential.issuedAtMs || v.expiresAtMs > v.hardwareCredential.expiresAtMs) throw new TypeError("KAGEMUSHA V1 request credential binding is invalid");
  },
);
const KagemushaPeerCreditContextV1 = defineModel(
  "KagemushaPeerCreditContextV1",
  [["version", T.U16], ["requestDigest", T.FIXED32], ["amount", T.U128],
    ["senderBeforeCommitment", T.FIXED32], ["senderAfterCommitment", T.FIXED32],
    ["preparedTransferDigest", T.FIXED32], ["recipientEncryptionKey", T.FIXED32]],
  (v) => {
    requireVersion(v.version);
    if (v.amount === 0n || equalBytes(v.senderBeforeCommitment, v.senderAfterCommitment)) {
      throw new TypeError("KAGEMUSHA V1 peer credit context is invalid");
    }
    requireX25519Key(v.recipientEncryptionKey, "peer credit recipient key");
  },
);
const KagemushaTrustedCommitTimeV1 = defineModel(
  "KagemushaTrustedCommitTimeV1", [["timeEvidenceCommitment", T.FIXED32]],
);
const KagemushaMonotonicLeaseV1 = defineModel(
  "KagemushaMonotonicLeaseV1", [["leaseEvidenceCommitment", T.FIXED32]],
);
const KagemushaOutboxReservationV1 = defineModel(
  "KagemushaOutboxReservationV1",
  [["reservationId", T.FIXED32], ["operationKind", T.OPERATION_KIND], ["reservedOutboxBytes", T.U32], ["issuedAtMs", T.U64], ["expiresAtMs", T.U64]],
  (v) => {
    const minimum = v.operationKind === "sendSplit" ? 25728 : v.operationKind === "redeemSplit" ? 26112 : null;
    if (minimum === null || v.reservedOutboxBytes < minimum || v.issuedAtMs >= v.expiresAtMs) throw new TypeError("KAGEMUSHA V1 outbox reservation is invalid");
  },
);
const KagemushaHardwareTerminalBodyV1 = defineModel(
  "KagemushaHardwareTerminalBodyV1",
  [["version", T.U16], ["candidateEnvelopeDigest", T.FIXED32], ["lifecycleBindingDigest", T.FIXED32],
    ["transitionNullifier", T.FIXED32], ["outboxReservationCommitment", T.FIXED32], ["commitEvidence", T.COMMIT_EVIDENCE],
    ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64], ["privateSuccessorCommitment", T.FIXED32],
    ["privateJournalCommitment", T.FIXED32], ["privateRecoveryCommitment", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.policyEpoch === 0n) throw new TypeError("KAGEMUSHA V1 terminal body policy epoch must be positive"); },
);
const KagemushaCommitCertificateV1 = defineModel(
  "KagemushaCommitCertificateV1",
  [["version", T.U16], ["certificateId", T.FIXED32], ["candidateEnvelopeDigest", T.FIXED32],
    ["lifecycleBindingDigest", T.FIXED32], ["transitionNullifier", T.FIXED32], ["outboxReservationCommitment", T.FIXED32],
    ["commitEvidence", T.COMMIT_EVIDENCE], ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64],
    ["hardwareTerminalCommitment", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.policyEpoch === 0n) throw new TypeError("KAGEMUSHA V1 commit certificate policy epoch must be positive"); },
);
const KagemushaRedemptionProofV1 = defineModel(
  "KagemushaRedemptionProofV1",
  [["version", T.U16], ["eqProtocolDigest", T.FIXED32], ["epProtocolDigest", T.FIXED32], ["semanticDigest", T.FIXED32],
    ["candidateEnvelopeDigest", T.FIXED32], ["commitCertificateDigest", T.FIXED32], ["eqDeferredAudit", T.FIXED32],
    ["epDeferredAudit", T.FIXED32], ["eqProof", T.VECTOR], ["epProof", T.VECTOR], ["eqHistory", T.VECTOR], ["epHistory", T.VECTOR]],
  validateProofVectors,
);
const KagemushaPaymentProofV1 = defineModel(
  "KagemushaPaymentProofV1",
  [["version", T.U16], ["eqProtocolDigest", T.FIXED32], ["epProtocolDigest", T.FIXED32], ["semanticDigest", T.FIXED32],
    ["candidateEnvelopeDigest", T.FIXED32], ["commitCertificateDigest", T.FIXED32], ["eqDeferredAudit", T.FIXED32],
    ["epDeferredAudit", T.FIXED32], ["eqProof", T.VECTOR], ["epProof", T.VECTOR], ["eqHistory", T.VECTOR], ["epHistory", T.VECTOR]],
  validateProofVectors,
);
const KagemushaPaymentOutputV1 = defineModel(
  "KagemushaPaymentOutputV1",
  [["version", T.U16], ["requestDigest", T.FIXED32], ["amount", T.U128],
    ["senderBeforeCommitment", T.FIXED32], ["senderAfterCommitment", T.FIXED32],
    ["transitionNullifier", T.FIXED32], ["creditId", T.FIXED32], ["ciphertextCommitment", T.FIXED32],
    ["commitEvidence", T.COMMIT_EVIDENCE], ["committedAtMs", T.U64]],
  (v) => {
    requireVersion(v.version);
    if (v.amount === 0n || v.committedAtMs === 0n || equalBytes(v.senderBeforeCommitment, v.senderAfterCommitment)) {
      throw new TypeError("KAGEMUSHA V1 payment output is invalid");
    }
  },
);
const KagemushaPaymentV1 = defineModel(
  "KagemushaPaymentV1",
  [["version", T.U16], ["output", "KagemushaPaymentOutputV1"], ["encryptedCredit", T.VECTOR],
    ["commitCertificate", "KagemushaCommitCertificateV1"], ["proof", "KagemushaPaymentProofV1"]],
  (v) => { requireVersion(v.version); if (v.output.version !== v.version || v.commitCertificate.version !== v.version || v.proof.version !== v.version) throw new TypeError("KAGEMUSHA V1 payment version mismatch"); },
);
const KagemushaInboxReceiptV1 = defineModel(
  "KagemushaInboxReceiptV1", [["version", T.U16], ["creditId", T.FIXED32], ["receiptCommitment", T.FIXED32]],
  (v) => requireVersion(v.version),
);
const KagemushaAcknowledgementV1 = defineModel(
  "KagemushaAcknowledgementV1",
  [["version", T.U16], ["requestDigest", T.FIXED32], ["paymentDigest", T.FIXED32], ["inboxReceipt", "KagemushaInboxReceiptV1"], ["signature", T.SIGNATURE]],
  (v) => { requireVersion(v.version); if (v.inboxReceipt.version !== v.version) throw new TypeError("KAGEMUSHA V1 acknowledgement version mismatch"); },
);

const KagemushaMintAuthorizationContextV1 = defineModel(
  "KagemushaMintAuthorizationContextV1",
  [["version", T.U16], ["operationId", T.FIXED32], ["releaseId", T.FIXED32], ["suiteId", T.FIXED32], ["vkDigest", T.FIXED32],
    ["artifactManifestDigest", T.FIXED32], ["networkId", T.NETWORK], ["asset", T.ASSET], ["assetIncarnation", T.INCARNATION],
    ["scale", T.U32], ["liabilityPoolId", T.FIXED32], ["amount", T.U128], ["payer", T.ACCOUNT], ["recipient", T.ACCOUNT],
    ["hardwareCredentialId", T.FIXED32], ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64],
    ["recipientCredentialCommitment", T.FIXED32], ["creditCommitment", T.FIXED32], ["recipientOneTimeKey", T.FIXED32]],
  (v) => {
    header(v, true);
    if (v.policyEpoch === 0n) throw new TypeError("mint authorization policy epoch must be positive");
    requireX25519Key(v.recipientOneTimeKey, "mint recipient key");
    requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "mint authorization liability pool");
  },
);
const KagemushaMintAuthorizationStatementV1 = defineModel(
  "KagemushaMintAuthorizationStatementV1",
  [["version", T.U16], ["context", "KagemushaMintAuthorizationContextV1"], ["issuanceCommitment", T.FIXED32], ["creditId", T.FIXED32], ["ciphertextDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.context.version !== v.version) throw new TypeError("mint authorization statement version mismatch"); },
);
const KagemushaMintAuthorizationV1 = defineModel(
  "KagemushaMintAuthorizationV1",
  [["version", T.U16], ["statement", "KagemushaMintAuthorizationStatementV1"], ["proof", "KagemushaPairedProofV1"]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) throw new TypeError("mint authorization version mismatch"); },
);
const KagemushaMintCreditStatementV1 = defineModel(
  "KagemushaMintCreditStatementV1",
  [["version", T.U16], ["lifecycle", "KagemushaLifecycleBindingV1"], ["recipientCredentialCommitment", T.FIXED32],
    ["authorizationContextDigest", T.FIXED32], ["mintAuthorizationDigest", T.FIXED32], ["amount", T.U128],
    ["issuanceCommitment", T.FIXED32], ["recipient", T.ACCOUNT], ["creditCommitment", T.FIXED32], ["mintedAtMs", T.U64]],
  (v) => { requireVersion(v.version); if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "mintFold" || v.amount === 0n || v.mintedAtMs === 0n) throw new TypeError("KAGEMUSHA V1 mint statement is invalid"); },
);
const KagemushaMintCreditV1 = defineModel(
  "KagemushaMintCreditV1",
  [["version", T.U16], ["statement", "KagemushaMintCreditStatementV1"], ["proof", "KagemushaPairedProofV1"],
    ["finalityCertificateBinding", T.FIXED32], ["finalityAuthorityHead", T.FIXED32], ["finalityGenesisRosterId", T.FIXED32],
    ["finalityProofBindingDigest", T.FIXED32], ["encryptedCredit", T.VECTOR], ["artifactManifestDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) throw new TypeError("mint credit version mismatch"); },
);

// Operation 16 exposes only public canonical archives; native hardware keeps reservations,
// openings, journal snapshots, and complete Guard certificates private.
const KagemushaDeviceMintStageCommandV1 = defineModel(
  "KagemushaDeviceMintStageCommandV1",
  [["version", T.U16], ["canonicalAuthorization", T.MINT_FRAME], ["canonicalMintCredit", T.MINT_FRAME]],
  (v) => requireVersion(v.version),
);
const KagemushaDeviceMintStageResultV1 = defineModel(
  "KagemushaDeviceMintStageResultV1",
  [["version", T.U16], ["disposition", T.U8], ["creditId", T.FIXED32]],
  (v) => {
    requireVersion(v.version);
    if (v.disposition !== 0 && v.disposition !== 1) throw new TypeError("KAGEMUSHA V1 mint-stage disposition must be 0 or 1");
  },
);

const KagemushaRedemptionStatementV1 = defineModel(
  "KagemushaRedemptionStatementV1",
  [["version", T.U16], ["lifecycle", "KagemushaLifecycleBindingV1"], ["amount", T.U128], ["beneficiary", T.ACCOUNT],
    ["terminalNullifier", T.FIXED32], ["redemptionCommitment", T.FIXED32], ["redemptionId", T.FIXED32],
    ["commitEvidence", T.COMMIT_EVIDENCE]],
  (v) => { requireVersion(v.version); if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "redeemSplit"
    || v.amount === 0n) throw new TypeError("KAGEMUSHA V1 redemption statement is invalid"); },
);
const KagemushaRedemptionVoucherV1 = defineModel(
  "KagemushaRedemptionVoucherV1",
  [["version", T.U16], ["statement", "KagemushaRedemptionStatementV1"], ["commitCertificate", "KagemushaCommitCertificateV1"],
    ["proof", "KagemushaRedemptionProofV1"], ["artifactManifestDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.commitCertificate.version !== v.version || v.proof.version !== v.version) throw new TypeError("redemption voucher version mismatch"); },
);

const KagemushaTopUpRequestV1 = defineModel(
  "KagemushaTopUpRequestV1",
  [["version", T.U16], ["operationId", T.FIXED32], ["issuanceCommitment", T.FIXED32], ["creditId", T.FIXED32],
    ["releaseId", T.FIXED32], ["suiteId", T.FIXED32], ["vkDigest", T.FIXED32], ["networkId", T.NETWORK], ["asset", T.ASSET],
    ["assetIncarnation", T.INCARNATION], ["scale", T.U32], ["amount", T.U128], ["liabilityPoolId", T.FIXED32],
    ["payer", T.ACCOUNT], ["recipient", T.ACCOUNT], ["hardwareCredential", "KagemushaHardwareCredentialV1"],
    ["recipientCredentialCommitment", T.FIXED32], ["creditCommitment", T.FIXED32], ["recipientOneTimeKey", T.FIXED32],
    ["encryptedCredit", T.VECTOR], ["artifactManifestDigest", T.FIXED32], ["mintAuthorization", T.OPTIONAL_MINT_AUTHORIZATION]],
  (v) => { header(v, true); requireX25519Key(v.recipientOneTimeKey, "top-up recipient key"); },
);
const KagemushaRedemptionRequestV1 = defineModel(
  "KagemushaRedemptionRequestV1", [["version", T.U16], ["operationId", T.FIXED32], ["voucher", "KagemushaRedemptionVoucherV1"]],
  (v) => { requireVersion(v.version); if (v.voucher.version !== v.version) throw new TypeError("redemption request version mismatch"); },
);

function encodeTopLevel(value, Model, schema, maximum, validate) {
  const model = instance(value, Model, Model.name);
  validate?.(model);
  return bounded(frame(schema, encodeModel(model), modelAlignment(Model)), maximum, Model.name);
}

function decodeTopLevel(raw, Model, schema, maximum, validate) {
  return decodeExact(raw, maximum, schema, Model, (value) => encodeTopLevel(value, Model, schema, maximum, validate));
}

const encodePaymentRequest = (value) => encodeTopLevel(value, KagemushaPaymentRequestV1, SCHEMAS.request, LIMITS.paymentRequest[0], validateRequest);
const decodePaymentRequest = (raw) => decodeTopLevel(raw, KagemushaPaymentRequestV1, SCHEMAS.request, LIMITS.paymentRequest[0], validateRequest);
const encodeCommitCertificate = (value, lifecycle, evidence, nullifier) => encodeTopLevel(value, KagemushaCommitCertificateV1, SCHEMAS.commitCertificate, 1024,
  lifecycle === undefined ? undefined : (model) => validateCommitCertificate(model, lifecycle, evidence, nullifier));
const decodeCommitCertificate = (raw, lifecycle, evidence, nullifier) => decodeTopLevel(raw, KagemushaCommitCertificateV1, SCHEMAS.commitCertificate, 1024,
  lifecycle === undefined ? undefined : (model) => validateCommitCertificate(model, lifecycle, evidence, nullifier));
const encodeRedemptionProof = (value) => encodeTopLevel(value, KagemushaRedemptionProofV1, SCHEMAS.redemptionProof, 6528);
const decodeRedemptionProof = (raw) => decodeTopLevel(raw, KagemushaRedemptionProofV1, SCHEMAS.redemptionProof, 6528);
const encodePaymentProof = (value) => encodeTopLevel(value, KagemushaPaymentProofV1, SCHEMAS.paymentProof, 6528);
const decodePaymentProof = (raw) => decodeTopLevel(raw, KagemushaPaymentProofV1, SCHEMAS.paymentProof, 6528);
const encodePeerCreditContext = (value) => encodeTopLevel(value, KagemushaPeerCreditContextV1, SCHEMAS.peerCreditContext, 512);
const decodePeerCreditContext = (raw) => decodeTopLevel(raw, KagemushaPeerCreditContextV1, SCHEMAS.peerCreditContext, 512);
const encodePayment = (value, request) => encodeTopLevel(value, KagemushaPaymentV1, SCHEMAS.payment, LIMITS.payment[0], (model) => validatePayment(model, request));
const decodePayment = (raw, request) => decodeTopLevel(raw, KagemushaPaymentV1, SCHEMAS.payment, LIMITS.payment[0], (model) => validatePayment(model, request));
const encodeAcknowledgement = (value, request, payment) => encodeTopLevel(value, KagemushaAcknowledgementV1, SCHEMAS.acknowledgement, LIMITS.acknowledgement[0], (model) => validateAcknowledgement(model, request, payment));
const decodeAcknowledgement = (raw, request, payment) => decodeTopLevel(raw, KagemushaAcknowledgementV1, SCHEMAS.acknowledgement, LIMITS.acknowledgement[0], (model) => validateAcknowledgement(model, request, payment));
const encodeMintAuthorization = (value) => encodeTopLevel(value, KagemushaMintAuthorizationV1, SCHEMAS.mintAuthorization, 7936, validateMintAuthorization);
const decodeMintAuthorization = (raw) => decodeTopLevel(raw, KagemushaMintAuthorizationV1, SCHEMAS.mintAuthorization, 7936, validateMintAuthorization);
const encodeMintCredit = (value, authorization) => encodeTopLevel(value, KagemushaMintCreditV1, SCHEMAS.mintCredit, 7936, (model) => validateMintCredit(model, authorization));
const decodeMintCredit = (raw, authorization) => decodeTopLevel(raw, KagemushaMintCreditV1, SCHEMAS.mintCredit, 7936, (model) => validateMintCredit(model, authorization));
/** Encode a public operation-16 body. Shape checks never authorize a hardware transition. */
function encodeDeviceMintStageCommandShape(value, canonicalMintCredit = undefined) {
  const command = canonicalMintCredit === undefined ? value : new KagemushaDeviceMintStageCommandV1({
    version: 1, canonicalAuthorization: value, canonicalMintCredit,
  });
  return encodeTopLevel(command, KagemushaDeviceMintStageCommandV1, SCHEMAS.deviceMintStageCommand,
    DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, validateDeviceMintStageCommand);
}
function decodeDeviceMintStageCommandShapeExact(raw) {
  return decodeTopLevel(boundedBytes(raw, DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, "mint-stage command"),
    KagemushaDeviceMintStageCommandV1, SCHEMAS.deviceMintStageCommand,
    DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, validateDeviceMintStageCommand);
}
/** This result still requires a qualified native response authenticator before it is trusted. */
function encodeDeviceMintStageResultShape(value, command = undefined) {
  return encodeTopLevel(value, KagemushaDeviceMintStageResultV1, SCHEMAS.deviceMintStageResult,
    DEVICE_MINT_STAGE_RESULT_MAX_BYTES, command === undefined ? validateDeviceMintStageResult
      : (result) => validateDeviceMintStageResultAgainstCommand(result, command));
}
function decodeDeviceMintStageResultShapeExact(raw, command = undefined) {
  return decodeTopLevel(boundedBytes(raw, DEVICE_MINT_STAGE_RESULT_MAX_BYTES, "mint-stage result"),
    KagemushaDeviceMintStageResultV1, SCHEMAS.deviceMintStageResult, DEVICE_MINT_STAGE_RESULT_MAX_BYTES,
    command === undefined ? validateDeviceMintStageResult : (result) => validateDeviceMintStageResultAgainstCommand(result, command));
}
const encodeRedemptionVoucher = (value) => encodeTopLevel(value, KagemushaRedemptionVoucherV1, SCHEMAS.redemptionVoucher, 7936, validateRedemptionVoucher);
const decodeRedemptionVoucher = (raw) => decodeTopLevel(raw, KagemushaRedemptionVoucherV1, SCHEMAS.redemptionVoucher, 7936, validateRedemptionVoucher);
const encodeEncryptedCreditAad = (value) => encodeTopLevel(value, KagemushaEncryptedCreditAadV1, SCHEMAS.encryptedCreditAad, 256);
const decodeEncryptedCreditAad = (raw) => decodeTopLevel(raw, KagemushaEncryptedCreditAadV1, SCHEMAS.encryptedCreditAad, 256);
const encodeEncryptedCreditEnvelope = (value, recipientKey) => encodeTopLevel(value, KagemushaEncryptedCreditEnvelopeV1, SCHEMAS.encryptedCreditEnvelope, 384, (model) => validateEnvelopeRecipient(model, recipientKey));
const decodeEncryptedCreditEnvelope = (raw, recipientKey) => decodeTopLevel(raw, KagemushaEncryptedCreditEnvelopeV1, SCHEMAS.encryptedCreditEnvelope, 384, (model) => validateEnvelopeRecipient(model, recipientKey));

function encodeCreditOpening(value) {
  const raw = encodeTopLevel(value, KagemushaCreditOpeningV1, SCHEMAS.creditOpening, 256);
  if (raw.length !== CREDIT_OPENING_BYTES) throw new TypeError("KAGEMUSHA V1 credit opening has a noncanonical fixed size");
  return raw;
}

function decodeCreditOpening(raw, creditIdValue, amount) {
  const opening = decodeTopLevel(raw, KagemushaCreditOpeningV1, SCHEMAS.creditOpening, 256, (value) => {
    if (creditIdValue !== undefined) requireEqual(value.creditId, fixed32(creditIdValue, "creditId"), "credit opening credit ID");
    if (amount !== undefined && value.amount !== unsigned(amount, MAX_U128, "amount")) throw new TypeError("credit opening amount does not match");
  });
  if (bytes(raw, "credit opening").length !== CREDIT_OPENING_BYTES) throw new TypeError("KAGEMUSHA V1 credit opening has a noncanonical fixed size");
  return opening;
}

function validateRequest(request) {
  const v = rawValues(instance(request, KagemushaPaymentRequestV1, "payment request"));
  requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "request liability pool");
}

function lifecycleDigest(lifecycle) {
  return digestModel(
    DOMAIN.lifecycleDigest,
    SCHEMAS.lifecycle,
    instance(lifecycle, KagemushaLifecycleBindingV1, "lifecycle binding"),
    8,
  );
}

function commitEvidenceTranscript(evidence) {
  const selected = normalizeType(T.COMMIT_EVIDENCE, evidence, "commit evidence");
  if (selected instanceof KagemushaTrustedCommitTimeV1) return join(u32(0), selected.timeEvidenceCommitment);
  return join(u32(1), selected.leaseEvidenceCommitment);
}

function commitCertificateIdTranscript(certificate) {
  const value = rawValues(instance(certificate, KagemushaCommitCertificateV1, "commit certificate"));
  return join(u16(value.version), value.candidateEnvelopeDigest, value.lifecycleBindingDigest, value.transitionNullifier,
    value.outboxReservationCommitment, commitEvidenceTranscript(value.commitEvidence), value.hardwareProfileId,
    u64(value.policyEpoch), value.hardwareTerminalCommitment);
}

function expectedCommitCertificateId(certificate) {
  return digestEncoded(DOMAIN.commitCertificateId, commitCertificateIdTranscript(certificate));
}

function commitCertificateTranscript(certificate) {
  const value = rawValues(instance(certificate, KagemushaCommitCertificateV1, "commit certificate"));
  return join(u16(value.version), value.certificateId, value.candidateEnvelopeDigest, value.lifecycleBindingDigest,
    value.transitionNullifier, value.outboxReservationCommitment, commitEvidenceTranscript(value.commitEvidence),
    value.hardwareProfileId, u64(value.policyEpoch), value.hardwareTerminalCommitment);
}

function validateCommitCertificate(certificate, lifecycle, evidence, nullifier) {
  const value = rawValues(instance(certificate, KagemushaCommitCertificateV1, "commit certificate"));
  const boundLifecycle = instance(lifecycle, KagemushaLifecycleBindingV1, "lifecycle binding");
  const boundEvidence = normalizeType(T.COMMIT_EVIDENCE, evidence, "commit evidence");
  requireEqual(value.lifecycleBindingDigest, lifecycleDigest(boundLifecycle), "commit certificate lifecycle digest");
  requireEqual(value.transitionNullifier, fixed32(nullifier, "transition nullifier"), "commit certificate transition nullifier");
  requireEqual(encodeCommitEvidence(value.commitEvidence), encodeCommitEvidence(boundEvidence), "commit certificate evidence");
  requireEqual(value.hardwareProfileId, boundLifecycle.hardwareProfileId, "commit certificate hardware profile");
  if (value.policyEpoch !== boundLifecycle.policyEpoch) throw new TypeError("commit certificate policy epoch does not match");
  requireEqual(value.certificateId, expectedCommitCertificateId(certificate), "commit certificate ID");
}

function commitCertificateDigest(certificate, lifecycle, evidence, nullifier) {
  if (lifecycle !== undefined) validateCommitCertificate(certificate, lifecycle, evidence, nullifier);
  else requireEqual(certificate.certificateId, expectedCommitCertificateId(certificate), "certificate ID");
  return digestEncoded(DOMAIN.commitCertificateDigest, commitCertificateTranscript(certificate));
}

function validatePaymentOutput(output, request) {
  const value = rawValues(instance(output, KagemushaPaymentOutputV1, "payment output"));
  const bound = instance(request, KagemushaPaymentRequestV1, "payment request");
  const requestDigest = paymentRequestDigest(bound);
  requireEqual(value.requestDigest, requestDigest, "payment request digest");
  if (value.amount !== bound.amount) throw new TypeError("payment amount does not match request");
  requireEqual(value.creditId, creditId(value.transitionNullifier, requestDigest), "payment credit ID");
  if (value.committedAtMs < bound.issuedAtMs || value.committedAtMs >= bound.expiresAtMs) {
    throw new TypeError("payment commit time is outside the request window");
  }
}
function validatePayment(payment, request) {
  const p = rawValues(instance(payment, KagemushaPaymentV1, "payment"));
  validatePaymentOutput(p.output, request);
  decodeEncryptedCreditEnvelope(p.encryptedCredit, request.recipientEncryptionKey);
  encryptedCreditAadForPeer(p.output, request);
  const certificate = p.commitCertificate;
  requireEqual(certificate.certificateId, expectedCommitCertificateId(certificate), "payment certificate ID");
  requireEqual(certificate.transitionNullifier, p.output.transitionNullifier, "payment certificate nullifier");
  requireEqual(commitEvidenceTranscript(certificate.commitEvidence), commitEvidenceTranscript(p.output.commitEvidence), "payment commit evidence");
  requireEqual(p.proof.candidateEnvelopeDigest, certificate.candidateEnvelopeDigest, "payment candidate digest");
  requireEqual(p.proof.commitCertificateDigest, digestEncoded(DOMAIN.commitCertificateDigest, commitCertificateTranscript(certificate)), "payment certificate digest");
  requireEqual(p.proof.semanticDigest, paymentBodyDigest(p.output, p.encryptedCredit), "payment body semantic digest");
}
function paymentDigest(payment, request) {
  validatePayment(payment, request);
  return digestModel(DOMAIN.paymentDigest, SCHEMAS.payment, payment);
}

function paymentOutputTranscript(output) {
  const v = rawValues(instance(output, KagemushaPaymentOutputV1, "payment output"));
  return join(u16(v.version), v.requestDigest, u128(v.amount), v.senderBeforeCommitment,
    v.senderAfterCommitment, v.transitionNullifier, v.creditId, v.ciphertextCommitment,
    commitEvidenceTranscript(v.commitEvidence), u64(v.committedAtMs));
}

function paymentOutputDigest(output, request) {
  if (request !== undefined) validatePaymentOutput(output, request);
  return digestEncoded(DOMAIN.statementDigest, paymentOutputTranscript(output));
}

function paymentBodyDigest(output, encryptedCredit) {
  decodeEncryptedCreditEnvelope(encryptedCredit);
  return digestEncoded(DOMAIN.paymentBodyDigest, join(paymentOutputDigest(output), ciphertextDigest(encryptedCredit)));
}

function validateAcknowledgement(acknowledgement, request, payment) {
  const a = rawValues(instance(acknowledgement, KagemushaAcknowledgementV1, "acknowledgement"));
  const p = instance(payment, KagemushaPaymentV1, "payment");
  requireEqual(a.requestDigest, paymentRequestDigest(request), "acknowledgement request digest");
  requireEqual(a.paymentDigest, paymentDigest(p, request), "acknowledgement payment digest");
  requireEqual(a.inboxReceipt.creditId, p.output.creditId, "acknowledgement credit ID");
}

function validateMintAuthorization(authorization) {
  const a = rawValues(instance(authorization, KagemushaMintAuthorizationV1, "mint authorization"));
  const digest = digestModel(DOMAIN.mintAuthorizationStatementDigest, SCHEMAS.mintAuthorizationStatement, a.statement);
  requireEqual(a.proof.semanticDigest, digest, "mint authorization proof semantic digest");
}

function mintAuthorizationContextDigest(context) {
  return digestModel(DOMAIN.mintAuthorizationContextDigest, SCHEMAS.mintAuthorizationContext, instance(context, KagemushaMintAuthorizationContextV1, "mint authorization context"));
}

function mintAuthorizationStatementDigest(statement) {
  return digestModel(DOMAIN.mintAuthorizationStatementDigest, SCHEMAS.mintAuthorizationStatement, instance(statement, KagemushaMintAuthorizationStatementV1, "mint authorization statement"));
}

function mintAuthorizationDigest(authorization) {
  return digestModel(DOMAIN.mintAuthorizationDigest, SCHEMAS.mintAuthorization, instance(authorization, KagemushaMintAuthorizationV1, "mint authorization"));
}

function mintCreditId(statement) {
  const s = rawValues(instance(statement, KagemushaMintCreditStatementV1, "mint statement"));
  const lifecycle = rawValues(s.lifecycle);
  // The frozen context ends at operationKind: current credit ID, ciphertext, and
  // authorization proof bytes are intentionally absent from the issuance preimage.
  const contextFields = DEFINITIONS.KagemushaLifecycleBindingV1.fields.slice(0, 13);
  const contextPreimage = join(...contextFields.map(([name, type]) => field(encodeType(type, lifecycle[name]))));
  const contextDigest = digestEncoded(DOMAIN.mintLifecycleContextDigest,
    frame("iroha.kagemusha.v1.mint-lifecycle-context-preimage", contextPreimage, 8));
  const preimage = join(field(contextDigest), field(s.recipientCredentialCommitment),
    field(s.authorizationContextDigest), field(u128(s.amount)), field(s.issuanceCommitment),
    field(s.recipient.canonicalPayload()), field(s.creditCommitment));
  return digestEncoded(DOMAIN.mintCreditId,
    frame("iroha.kagemusha.v1.mint-credit-id-preimage", preimage, 16));
}

function mintCreditStatementDigest(statement) {
  const value = instance(statement, KagemushaMintCreditStatementV1, "mint statement");
  requireEqual(value.lifecycle.creditId, mintCreditId(value), "mint credit ID");
  return digestModel(DOMAIN.mintStatementDigest, SCHEMAS.mintStatement, value);
}

function validateMintCredit(credit, authorization) {
  const c = rawValues(instance(credit, KagemushaMintCreditV1, "mint credit"));
  decodeEncryptedCreditEnvelope(c.encryptedCredit);
  requireEqual(c.statement.lifecycle.ciphertextDigest, ciphertextDigest(c.encryptedCredit), "mint ciphertext digest");
  const digest = mintCreditStatementDigest(c.statement);
  requireEqual(c.proof.semanticDigest, digest, "mint proof semantic digest");
  if (authorization !== undefined) validateMintCreditAgainstAuthorization(credit, authorization);
}

function validateDeviceMintStageCommand(command) {
  const value = rawValues(instance(command, KagemushaDeviceMintStageCommandV1, "mint-stage command"));
  requireVersion(value.version);
  const authorization = decodeMintAuthorization(boundedBytes(value.canonicalAuthorization, 7936, "mint authorization"));
  return decodeMintCredit(boundedBytes(value.canonicalMintCredit, 7936, "mint credit"), authorization);
}

function validateDeviceMintStageResult(result) {
  const value = rawValues(instance(result, KagemushaDeviceMintStageResultV1, "mint-stage result"));
  requireVersion(value.version);
  if (value.disposition !== 0 && value.disposition !== 1) throw new TypeError("KAGEMUSHA V1 mint-stage disposition must be 0 or 1");
  fixed32(value.creditId, "mint-stage result credit ID");
}

/** Structural credit binding only; this never authenticates an inbox receipt. */
function validateDeviceMintStageResultAgainstCommand(result, command) {
  validateDeviceMintStageResult(result);
  const value = rawValues(result);
  const credit = validateDeviceMintStageCommand(command);
  requireEqual(value.creditId, credit.statement.lifecycle.creditId, "mint-stage result credit ID");
  return true;
}

function expectedRedemptionId(statement) {
  const value = rawValues(instance(
    statement,
    KagemushaRedemptionStatementV1,
    "redemption statement",
  ));
  const preimage = join(
    field(encodeType(T.FIXED32, lifecycleDigest(value.lifecycle))),
    field(encodeType(T.FIXED32, value.terminalNullifier)),
    field(encodeType(T.U128, value.amount)),
    field(encodeType(T.ACCOUNT, value.beneficiary)),
    field(encodeType(T.FIXED32, value.redemptionCommitment)),
  );
  return digestEncoded(
    DOMAIN.redemptionId,
    frame("iroha.kagemusha.v1.redemption-id-preimage", preimage, 16),
  );
}

function validateRedemptionStatement(statement) {
  const value = rawValues(instance(
    statement,
    KagemushaRedemptionStatementV1,
    "redemption statement",
  ));
  if (equalBytes(value.terminalNullifier, value.redemptionCommitment)
      || equalBytes(value.terminalNullifier, value.redemptionId)
      || equalBytes(value.redemptionCommitment, value.redemptionId)) {
    throw new TypeError("redemption statement identities must be distinct");
  }
  requireEqual(value.redemptionId, expectedRedemptionId(statement), "redemption ID");
}

function redemptionStatementDigest(statement) {
  validateRedemptionStatement(statement);
  return digestModel(
    DOMAIN.redemptionStatementDigest,
    SCHEMAS.redemptionStatement,
    statement,
  );
}

function validateRedemptionVoucher(voucher) {
  const value = rawValues(instance(
    voucher,
    KagemushaRedemptionVoucherV1,
    "redemption voucher",
  ));
  validateRedemptionStatement(value.statement);
  validateCommitCertificate(value.commitCertificate, value.statement.lifecycle, value.statement.commitEvidence, value.statement.terminalNullifier);
  requireEqual(value.proof.semanticDigest, redemptionStatementDigest(value.statement), "redemption proof semantic digest");
  requireEqual(value.proof.candidateEnvelopeDigest, value.commitCertificate.candidateEnvelopeDigest, "redemption candidate envelope digest");
  requireEqual(value.proof.commitCertificateDigest,
    commitCertificateDigest(value.commitCertificate, value.statement.lifecycle, value.statement.commitEvidence, value.statement.terminalNullifier),
    "redemption commit certificate digest");
}

function validateMintCreditAgainstAuthorization(credit, authorization) {
  const c = rawValues(instance(credit, KagemushaMintCreditV1, "mint credit"));
  const a = rawValues(instance(authorization, KagemushaMintAuthorizationV1, "mint authorization"));
  validateMintCredit(credit);
  validateMintAuthorization(authorization);
  const context = rawValues(a.statement.context);
  const statement = rawValues(c.statement);
  requireEqual(statement.authorizationContextDigest, digestModel(DOMAIN.mintAuthorizationContextDigest, SCHEMAS.mintAuthorizationContext, a.statement.context), "mint authorization context digest");
  requireEqual(statement.mintAuthorizationDigest, digestModel(DOMAIN.mintAuthorizationDigest, SCHEMAS.mintAuthorization, authorization), "mint authorization digest");
  requireEqual(statement.issuanceCommitment, a.statement.issuanceCommitment, "mint issuance commitment");
  requireEqual(statement.lifecycle.creditId, a.statement.creditId, "mint credit ID");
  requireEqual(statement.lifecycle.ciphertextDigest, a.statement.ciphertextDigest, "mint ciphertext binding");
  requireEqual(statement.recipientCredentialCommitment, context.recipientCredentialCommitment, "mint recipient credential commitment");
  requireEqual(statement.creditCommitment, context.creditCommitment, "mint credit commitment");
  if (statement.amount !== context.amount
      || !equalBytes(statement.recipient.canonicalPayload(), context.recipient.canonicalPayload())
      || !equalBytes(statement.lifecycle.releaseId, context.releaseId)
      || !equalBytes(statement.lifecycle.suiteId, context.suiteId)
      || !equalBytes(statement.lifecycle.vkDigest, context.vkDigest)
      || !equalBytes(networkIdBytes(statement.lifecycle.networkId), networkIdBytes(context.networkId))
      || !equalBytes(encodeType(T.ASSET, statement.lifecycle.asset), encodeType(T.ASSET, context.asset))
      || !equalBytes(encodeType(T.INCARNATION, statement.lifecycle.assetIncarnation), encodeType(T.INCARNATION, context.assetIncarnation))
      || statement.lifecycle.scale !== context.scale
      || !equalBytes(statement.lifecycle.liabilityPoolId, context.liabilityPoolId)
      || !equalBytes(statement.lifecycle.hardwareProfileId, context.hardwareProfileId)
      || statement.lifecycle.policyEpoch !== context.policyEpoch
      || !equalBytes(c.artifactManifestDigest, context.artifactManifestDigest)) {
    throw new TypeError("mint authorization context binding is invalid");
  }
  requireEqual(a.statement.ciphertextDigest, ciphertextDigest(c.encryptedCredit), "mint authorization ciphertext digest");
  decodeEncryptedCreditEnvelope(c.encryptedCredit, context.recipientOneTimeKey);
  return true;
}

function encryptedCreditAadForMint(statement) {
  const s = rawValues(instance(statement, KagemushaMintAuthorizationStatementV1, "mint authorization statement"));
  return new KagemushaEncryptedCreditAadV1({
    version: 1,
    purpose: "mint",
    contextDigest: digestModel(DOMAIN.mintAuthorizationContextDigest, SCHEMAS.mintAuthorizationContext, s.context),
    issuanceOrTransitionCommitment: s.issuanceCommitment,
    creditId: s.creditId,
    amount: s.context.amount,
  });
}

function peerCreditContext(output, request) {
  validatePaymentOutput(output, request);
  return new KagemushaPeerCreditContextV1({
    version: 1,
    requestDigest: output.requestDigest,
    amount: output.amount,
    senderBeforeCommitment: output.senderBeforeCommitment,
    senderAfterCommitment: output.senderAfterCommitment,
    preparedTransferDigest: preparedTransferDigest(request, output.senderBeforeCommitment,
      output.senderAfterCommitment, output.transitionNullifier, output.ciphertextCommitment),
    recipientEncryptionKey: request.recipientEncryptionKey,
  });
}
function encryptedCreditAadForPeer(output, request) {
  const context = peerCreditContext(output, request);
  return new KagemushaEncryptedCreditAadV1({
    version: 1, purpose: "peer",
    contextDigest: digestModel(DOMAIN.peerCreditContextDigest, SCHEMAS.peerCreditContext, context),
    issuanceOrTransitionCommitment: output.ciphertextCommitment,
    creditId: output.creditId, amount: output.amount,
  });
}
function validateTopUpRequest(request) {
  if (request.mintAuthorization === null) throw new TypeError("canonical KAGEMUSHA V1 top-up requires mint authorization");
  validateMintAuthorization(request.mintAuthorization);
  const context = request.mintAuthorization.statement.context;
  requireEqual(request.liabilityPoolId, liabilityPoolId(request.networkId, request.asset, request.assetIncarnation), "top-up liability pool");
  requireEqual(ciphertextDigest(request.encryptedCredit), request.mintAuthorization.statement.ciphertextDigest, "top-up ciphertext digest");
  requireEqual(request.issuanceCommitment, request.mintAuthorization.statement.issuanceCommitment, "top-up issuance commitment");
  requireEqual(request.creditId, request.mintAuthorization.statement.creditId, "top-up credit ID");
  if (!equalBytes(request.operationId, context.operationId)
      || !equalBytes(request.releaseId, context.releaseId)
      || !equalBytes(request.suiteId, context.suiteId)
      || !equalBytes(request.vkDigest, context.vkDigest)
      || !equalBytes(networkIdBytes(request.networkId), networkIdBytes(context.networkId))
      || !equalBytes(encodeType(T.ASSET, request.asset), encodeType(T.ASSET, context.asset))
      || !equalBytes(encodeType(T.INCARNATION, request.assetIncarnation), encodeType(T.INCARNATION, context.assetIncarnation))
      || request.scale !== context.scale || request.amount !== context.amount
      || !equalBytes(request.liabilityPoolId, context.liabilityPoolId)
      || !equalBytes(request.payer.canonicalPayload(), context.payer.canonicalPayload())
      || !equalBytes(request.recipient.canonicalPayload(), context.recipient.canonicalPayload())
      || !equalBytes(request.hardwareCredential.credentialId, context.hardwareCredentialId)
      || !equalBytes(request.hardwareCredential.hardwareProfileId, context.hardwareProfileId)
      || request.hardwareCredential.policyEpoch !== context.policyEpoch
      || !equalBytes(request.recipientCredentialCommitment, context.recipientCredentialCommitment)
      || !equalBytes(request.creditCommitment, context.creditCommitment)
      || !equalBytes(request.recipientOneTimeKey, context.recipientOneTimeKey)
      || !equalBytes(request.artifactManifestDigest, context.artifactManifestDigest)) {
    throw new TypeError("top-up mint authorization context binding is invalid");
  }
}

function validateEnvelopeRecipient(_envelope, recipientKey) {
  if (recipientKey !== undefined) requireX25519Key(raw32(recipientKey, "recipient X25519 key"), "recipient X25519 key");
}

function encodeText(kind, raw) {
  const [maximumRaw, maximumText] = kindLimits(kind);
  const payload = bounded(bytes(raw, "KAGEMUSHA V1 payload"), maximumRaw, "payload");
  if (payload.length === 0) throw new TypeError("KAGEMUSHA V1 payload is empty");
  const text = `kgm1:${Buffer.from(payload).toString("base64url")}`;
  if (text.length > maximumText) throw new RangeError("KAGEMUSHA V1 text is oversized");
  return text;
}

function decodeText(kind, text) {
  const [maximumRaw, maximumText] = kindLimits(kind);
  if (typeof text !== "string" || text.length > maximumText || !text.startsWith("kgm1:")) throw new TypeError("KAGEMUSHA V1 text prefix or size is invalid");
  const body = text.slice("kgm1:".length);
  if (!/^[A-Za-z0-9_-]+$/u.test(body) || body.length % 4 === 1) throw new TypeError("KAGEMUSHA V1 text is not canonical unpadded base64url");
  const raw = Uint8Array.from(Buffer.from(body, "base64url"));
  bounded(raw, maximumRaw, "payload");
  if (encodeText(kind, raw) !== text) throw new TypeError("KAGEMUSHA V1 text is not canonical");
  return raw;
}

function encodeTypedText(kind, value, ...bindings) {
  const encoders = {
    paymentRequest: encodePaymentRequest,
    payment: encodePayment,
    acknowledgement: encodeAcknowledgement,
    mintAuthorization: encodeMintAuthorization,
    mintCredit: encodeMintCredit,
    redemptionVoucher: encodeRedemptionVoucher,
  };
  if (!Object.hasOwn(encoders, kind)) throw new TypeError("unknown KAGEMUSHA V1 payload kind");
  return encodeText(kind, encoders[kind](value, ...bindings));
}

function decodeTypedText(kind, text, ...bindings) {
  const decoders = {
    paymentRequest: decodePaymentRequest,
    payment: decodePayment,
    acknowledgement: decodeAcknowledgement,
    mintAuthorization: decodeMintAuthorization,
    mintCredit: decodeMintCredit,
    redemptionVoucher: decodeRedemptionVoucher,
  };
  if (!Object.hasOwn(decoders, kind)) throw new TypeError("unknown KAGEMUSHA V1 payload kind");
  return decoders[kind](decodeText(kind, text), ...bindings);
}

function encodeTopUpRequest(value) {
  const request = instance(value, KagemushaTopUpRequestV1, "top-up request");
  validateTopUpRequest(request);
  return encodeTopLevel(
    request,
    KagemushaTopUpRequestV1,
    SCHEMAS.topUpRequest,
    TOP_UP_REQUEST_MAX_BYTES,
  );
}
const decodeTopUpRequest = (raw) => decodeTopLevel(
  raw,
  KagemushaTopUpRequestV1,
  SCHEMAS.topUpRequest,
  TOP_UP_REQUEST_MAX_BYTES,
  validateTopUpRequest,
);
const encodeRedemptionRequest = (value) => encodeTopLevel(value, KagemushaRedemptionRequestV1, SCHEMAS.redemptionRequest, 8192);
const decodeRedemptionRequest = (raw) => decodeTopLevel(raw, KagemushaRedemptionRequestV1, SCHEMAS.redemptionRequest, 8192);

function buildTopUpInstruction(value) {
  return Object.freeze({
    TopUpKagemushaV1: Object.freeze({
      request: Uint8Array.from(encodeTopUpRequest(value)),
    }),
  });
}

function encodeTopUpInstruction(value) {
  return Uint8Array.from(noritoEncodeInstruction(buildTopUpInstruction(value)));
}

function decodeTopUpInstruction(raw) {
  const decoded = noritoDecodeInstruction(raw);
  exactRecord(decoded, "top-up instruction", ["TopUpKagemushaV1"]);
  exactRecord(
    decoded.TopUpKagemushaV1,
    "top-up instruction body",
    ["request"],
  );
  return decodeTopUpRequest(decoded.TopUpKagemushaV1.request);
}

function validateCompleteExchange(request, payment, acknowledgement) {
  const paymentValue = instance(payment, KagemushaPaymentV1, "payment");
  validatePayment(paymentValue, request);
  validateAcknowledgement(acknowledgement, request, paymentValue);
  const parts = [encodePaymentRequest(request), encodePayment(paymentValue, request),
    encodeAcknowledgement(acknowledgement, request, paymentValue)];
  const rawBytes = parts.reduce((sum, value) => sum + value.length, 0);
  const textBytes = parts.reduce((sum, value) => sum + textLength(value.length), 0);
  if (rawBytes > COMPLETE_EXCHANGE_MAX_RAW_BYTES
      || textBytes > COMPLETE_EXCHANGE_MAX_TEXT_BYTES) {
    throw new RangeError("KAGEMUSHA V1 complete three-message exchange is oversized");
  }
  return rawBytes;
}

function deviceKeyReference(publicKey) {
  const key = instance(publicKey, KagemushaDevicePublicKeyV1, "device public key");
  return sha256(join(DOMAIN.deviceKeyReference, Uint8Array.of(0), key.sec1Bytes()));
}

function pastaStateCommitment(value) {
  const state = rawValues(instance(value, KagemushaPastaStateCommitmentV1, "Pasta state commitment"));
  return sha256(join(DOMAIN.pastaStateCommitment, Uint8Array.of(0), state.eq, state.ep));
}

function liabilityPoolId(networkId, asset, assetIncarnation) {
  const network = normalizeType(T.NETWORK, networkId, "networkId");
  const definition = normalizeType(T.ASSET, asset, "asset");
  const incarnation = normalizeType(T.INCARNATION, assetIncarnation, "assetIncarnation");
  const payload = join(field(networkIdBytes(network)), field(definition.canonicalPayload()), field(encodeType(T.INCARNATION, incarnation)));
  return digestEncoded(DOMAIN.liabilityPool, frame("iroha.kagemusha.v1.liability-pool-preimage", payload, 1));
}

function assetIdentityDigest(asset) {
  const value = normalizeType(T.ASSET, asset, "asset");
  return digestEncoded(DOMAIN.assetIdentity, frame("iroha_data_model::asset::id::model::AssetDefinitionId", value.canonicalPayload(), 1));
}

function accountIdentityDigest(account) {
  const value = normalizeType(T.ACCOUNT, account, "account");
  return digestEncoded(DOMAIN.accountIdentity, frame("iroha_data_model::account::model::AccountId", value.canonicalPayload(), 8));
}

function paymentRequestUnsignedTranscript(value) {
  const r = rawValues(instance(value, KagemushaPaymentRequestV1, "payment request"));
  return join(u16(r.version), r.releaseId, networkIdBytes(r.networkId), assetIdentityDigest(r.asset),
    r.assetIncarnation.hashBytes(), u32(r.scale), r.liabilityPoolId, accountIdentityDigest(r.recipient),
    u128(r.amount), r.recipientEncryptionKey, r.hardwareCredential.credentialId,
    r.requestId, u64(r.issuedAtMs), u64(r.expiresAtMs));
}

function paymentRequestTranscript(value) {
  return join(paymentRequestUnsignedTranscript(value), value.signature.rawBytes());
}

function paymentRequestSigningBytes(value) {
  return join(DOMAIN.requestSigning, Uint8Array.of(0), paymentRequestUnsignedTranscript(value));
}
function acknowledgementSigningBytes(value) {
  const acknowledgement = rawValues(instance(value, KagemushaAcknowledgementV1, "acknowledgement"));
  const payload = join(
    field(vector(ascii("iroha:kagemusha:v1:acknowledgement-signing"))),
    field(u16(acknowledgement.version)), field(fixedArray(acknowledgement.requestDigest)),
    field(fixedArray(acknowledgement.paymentDigest)), field(encodeModel(acknowledgement.inboxReceipt)),
  );
  return frame("iroha.kagemusha.v1.acknowledgement-signing-preimage", payload, 8);
}

function paymentRequestDigest(value) {
  validateRequest(value);
  return digestEncoded(DOMAIN.requestDigest, paymentRequestTranscript(value));
}
function ciphertextDigest(value) { return digestEncoded(DOMAIN.ciphertextDigest, bytes(value, "encrypted credit")); }

function creditId(transitionNullifier, requestDigestValue) {
  return sha256(join(DOMAIN.creditId, Uint8Array.of(0),
    fixed32(transitionNullifier, "transitionNullifier"), fixed32(requestDigestValue, "requestDigest")));
}
function peerCreditOpeningCommitment(
  requestDigest,
  recipientEncryptionKey,
  amount,
  creditCommitmentOpening,
  recipientBindingOpening,
  recoveryNonce,
) {
  const exactAmount = unsigned(amount, MAX_U128, "amount");
  if (exactAmount === 0n) throw new TypeError("amount must be positive");
  return sha256(join(
    DOMAIN.peerCreditOpeningCommitment,
    Uint8Array.of(0),
    u16(1),
    fixed32(requestDigest, "requestDigest"),
    fixed32(recipientEncryptionKey, "recipientEncryptionKey"),
    u128(exactAmount),
    fixed32(creditCommitmentOpening, "creditCommitmentOpening"),
    fixed32(recipientBindingOpening, "recipientBindingOpening"),
    fixed32(recoveryNonce, "recoveryNonce"),
  ));
}

function preparedTransferDigest(request, senderBeforeCommitment, senderAfterCommitment,
  transitionNullifier, ciphertextCommitment) {
  const bound = instance(request, KagemushaPaymentRequestV1, "payment request");
  validateRequest(bound);
  const before = fixed32(senderBeforeCommitment, "senderBeforeCommitment");
  const after = fixed32(senderAfterCommitment, "senderAfterCommitment");
  if (equalBytes(before, after)) throw new TypeError("sender state commitments must differ");
  return digestEncoded(DOMAIN.preparedTransfer, join(u16(1), paymentRequestDigest(request),
    u128(bound.amount), before, after, fixed32(transitionNullifier, "transitionNullifier"),
    bound.recipientEncryptionKey, fixed32(ciphertextCommitment, "ciphertextCommitment")));
}
function validatePairedProofValues(v) {
  if (equalBytes(v.guardEqCredentialAudit, v.guardEpCredentialAudit)) throw new TypeError("KAGEMUSHA V1 proof credential audits are aliased");
  validateProofVectors(v);
}

function validateProofVectors(v) {
  requireVersion(v.version);
  if (equalBytes(v.eqProtocolDigest, v.epProtocolDigest) || equalBytes(v.eqDeferredAudit, v.epDeferredAudit)) throw new TypeError("KAGEMUSHA V1 proof parity bindings are invalid");
  if (v.eqProof.length === 0 || v.epProof.length === 0 || v.eqProof.length > 2495 || v.epProof.length > 2495 || v.eqProof.length + v.epProof.length > 4990) throw new RangeError("KAGEMUSHA V1 current proof bytes are out of bounds");
  if (v.eqHistory.length !== 544 || v.epHistory.length !== 544 || isZero(v.eqHistory) || isZero(v.epHistory) || equalBytes(v.eqHistory, v.epHistory)) throw new TypeError("KAGEMUSHA V1 history accumulators are invalid");
}

function encodeModel(value) {
  const definition = DEFINITIONS[value.constructor.name];
  if (!definition) throw new TypeError("value is not a KAGEMUSHA V1 model");
  const raw = rawValues(value);
  return join(...definition.fields.map(([name, type]) => field(encodeType(type, raw[name]))));
}

function decodeModel(Model, payload) {
  const definition = DEFINITIONS[Model.name];
  const reader = new Reader(payload, Model.name);
  const value = {};
  for (const [name, type] of definition.fields) value[name] = decodeType(type, reader.readField(name), `${Model.name}.${name}`);
  reader.eof();
  return new Model(value);
}

function encodeType(type, value) {
  switch (type) {
    case T.U8: return Uint8Array.of(value);
    case T.U16: return u16(value);
    case T.U32: return u32(value);
    case T.U64: return u64(value);
    case T.U128: return u128(value);
    case T.FIXED32: case T.RAW32: return fixedArray(value);
    case T.FIXED24: return bytes(value, "fixed24");
    case T.NETWORK: return networkIdBytes(value);
    case T.ASSET: case T.ACCOUNT: return value.canonicalPayload();
    case T.INCARNATION: return field(value.hashBytes());
    case T.PUBLIC_KEY: return value.sec1Bytes();
    case T.SIGNATURE: return value.rawBytes();
    case T.VECTOR: case T.MINT_FRAME: return vector(value);
    case T.OPERATION_KIND: return encodeUnitEnum(value, OPERATION_KINDS, "operation kind");
    case T.CREDIT_PURPOSE: return encodeUnitEnum(value, CREDIT_PURPOSES, "credit purpose");
    case T.COMMIT_EVIDENCE: return encodeCommitEvidence(value);
    case T.OPTIONAL_MINT_AUTHORIZATION: return value === null ? Uint8Array.of(0) : join(Uint8Array.of(1), field(encodeModel(value)));
    default: return encodeModel(value);
  }
}

function decodeType(type, payload, context) {
  switch (type) {
    case T.U8: return Number(readUnsigned(payload, 1, context));
    case T.U16: return Number(readUnsigned(payload, 2, context));
    case T.U32: return Number(readUnsigned(payload, 4, context));
    case T.U64: return readUnsigned(payload, 8, context);
    case T.U128: return readUnsigned(payload, 16, context);
    case T.FIXED32: return fixed32(payload, context);
    case T.RAW32: return raw32(payload, context);
    case T.FIXED24: return fixedBytes(payload, 24, context, false);
    case T.NETWORK: if (payload.length !== 32) throw new TypeError(`${context} must be 32 bytes`); return NetworkId.fromBytes(payload);
    case T.ASSET: return new KagemushaAssetDefinitionIdV1(payload);
    case T.INCARNATION: return decodeIncarnation(payload, context);
    case T.ACCOUNT: return new KagemushaAccountIdV1(payload);
    case T.PUBLIC_KEY: return new KagemushaDevicePublicKeyV1(payload);
    case T.SIGNATURE: return new KagemushaDeviceSignatureV1(payload);
    case T.VECTOR: case T.MINT_FRAME: return readVector(payload, context);
    case T.OPERATION_KIND: return decodeUnitEnum(payload, OPERATION_KINDS, context);
    case T.CREDIT_PURPOSE: return decodeUnitEnum(payload, CREDIT_PURPOSES, context);
    case T.COMMIT_EVIDENCE: return decodeCommitEvidence(payload, context);
    case T.OPTIONAL_MINT_AUTHORIZATION: return decodeOptionalMintAuthorization(payload, context);
    default: return decodeModel(DEFINITIONS[type].Model, payload);
  }
}

function decodeOptionalMintAuthorization(payload, context) {
  if (payload.length === 1 && payload[0] === 0) return null;
  if (payload.length < 3 || payload[0] !== 1) throw new TypeError(`${context} has an invalid option tag`);
  const reader = new Reader(payload.subarray(1), context);
  const value = decodeModel(KagemushaMintAuthorizationV1, reader.readField("value"));
  reader.eof();
  return value;
}

function decodeIncarnation(payload, context) {
  const reader = new Reader(payload, context);
  const raw = reader.readField("hash");
  reader.eof();
  return new KagemushaAssetIncarnationV1(raw);
}

function encodeUnitEnum(value, variants, context) {
  if (typeof value !== "string" || !variants.includes(value)) throw new TypeError(`unknown KAGEMUSHA V1 ${context}`);
  return u32(variants.indexOf(value));
}

function decodeUnitEnum(payload, variants, context) {
  const tag = Number(readUnsigned(payload, 4, context));
  if (tag >= variants.length) throw new TypeError(`${context} has an unknown tag`);
  return variants[tag];
}

function encodeCommitEvidence(value) {
  if (value instanceof KagemushaTrustedCommitTimeV1) return join(u32(0), field(encodeModel(value)));
  if (value instanceof KagemushaMonotonicLeaseV1) return join(u32(1), field(encodeModel(value)));
  throw new TypeError("unknown KAGEMUSHA V1 commit evidence");
}

function decodeCommitEvidence(payload, context) {
  if (payload.length < 5) throw new TypeError(`${context} is truncated`);
  const tag = Number(readUnsigned(payload.subarray(0, 4), 4, `${context}.tag`));
  const Model = [KagemushaTrustedCommitTimeV1, KagemushaMonotonicLeaseV1][tag];
  if (Model === undefined) throw new TypeError(`${context} has an unknown tag`);
  const reader = new Reader(payload.subarray(4), context);
  const value = decodeModel(Model, reader.readField("evidence"));
  reader.eof();
  return value;
}

function decodeExact(raw, maximum, schema, Model, reencode) {
  const canonical = bounded(bytes(raw, Model.name), maximum, Model.name);
  if (canonical.length === 0) throw new TypeError(`${Model.name} archive is empty`);
  const decoded = validateNoritoFrame(canonical, { context: Model.name, expectedTypeName: schema, expectedPaddingLength: headerPadding(modelAlignment(Model)), requireNonEmptyPayload: true });
  if (decoded.flags !== COMPACT_LENGTHS) throw new TypeError(`${Model.name} must use compact field lengths`);
  const value = decodeModel(Model, decoded.payload);
  requireEqual(canonical, reencode(value), `${Model.name} canonical archive`);
  return value;
}

function digestModel(domain, schema, value, alignment = undefined) {
  const selectedAlignment = alignment ?? modelAlignment(value.constructor);
  return digestEncoded(domain, frame(schema, encodeModel(value), selectedAlignment));
}
function digestEncoded(domain, canonical) { return sha256(join(domain, Uint8Array.of(0), u64(canonical.length), canonical)); }
function textLength(rawLength) { return 5 + Math.floor((rawLength * 4 + 2) / 3); }

function frame(typeName, payload, alignment) {
  const padding = headerPadding(alignment);
  const typeHash = sha256(join(ascii("norito:v1:type-name\0"), ascii(typeName))).subarray(0, 16);
  const headerBytes = new Uint8Array(HEADER_BYTES);
  headerBytes.set(ascii("NRT0"), 0);
  headerBytes.set(typeHash, 6);
  headerBytes.set(u64(payload.length), 23);
  headerBytes.set(u64(crc64Xz(payload)), 31);
  headerBytes[39] = COMPACT_LENGTHS;
  return join(headerBytes, new Uint8Array(padding), payload);
}
function headerPadding(alignment) { return alignment <= 1 ? 0 : (alignment - (HEADER_BYTES % alignment)) % alignment; }
function modelAlignment(Model) {
  if (Model === KagemushaDeviceMintStageCommandV1) return 8;
  if (Model === KagemushaDeviceMintStageResultV1) return 2;
  if (Model === KagemushaEncryptedCreditEnvelopeV1) return 8;
  if (Model === KagemushaPaymentV1 || Model === KagemushaPaymentOutputV1
      || Model === KagemushaPeerCreditContextV1) return 16;
  if (Model === KagemushaCommitCertificateV1) return 8;
  if (Model === KagemushaRedemptionProofV1 || Model === KagemushaPaymentProofV1) return 8;
  if (Model === KagemushaAcknowledgementV1) return 2;
  return 16;
}

function field(payload) { const raw = bytes(payload, "field"); return join(compact(raw.length), raw); }
function compact(input) {
  let value = BigInt(input);
  if (value < 0n || value > MAX_U64) throw new RangeError("compact length is out of range");
  const out = [];
  do { let byte = Number(value & 0x7fn); value >>= 7n; if (value !== 0n) byte |= 0x80; out.push(byte); } while (value !== 0n);
  return Uint8Array.from(out);
}
function fixedArray(value) { return bytes(value, "fixed array"); }
function vector(value) { const raw = bytes(value, "byte vector"); return join(u64(raw.length), raw); }
function readVector(payload, context) {
  if (payload.length < 8) throw new TypeError(`${context} is truncated`);
  const length = readUnsigned(payload.subarray(0, 8), 8, `${context}.length`);
  if (length > BigInt(Number.MAX_SAFE_INTEGER) || Number(length) !== payload.length - 8) throw new TypeError(`${context} length is invalid`);
  return Uint8Array.from(payload.subarray(8));
}
function u16(value) { const out = new Uint8Array(2); new DataView(out.buffer).setUint16(0, Number(value), true); return out; }
function u32(value) { const out = new Uint8Array(4); new DataView(out.buffer).setUint32(0, Number(value), true); return out; }
function u64(value) { return unsignedLittleEndian(BigInt(value), 8); }
function u128(value) { return unsignedLittleEndian(BigInt(value), 16); }
function unsignedLittleEndian(value, width) { const out = new Uint8Array(width); for (let index = 0; index < width; index += 1) { out[index] = Number(value & 0xffn); value >>= 8n; } if (value !== 0n) throw new RangeError("unsigned integer is out of range"); return out; }
function readUnsigned(payload, width, context) { if (payload.length !== width) throw new TypeError(`${context} must contain ${width} bytes`); let value = 0n; for (let index = width - 1; index >= 0; index -= 1) value = (value << 8n) | BigInt(payload[index]); return value; }

class Reader {
  constructor(value, context) { this.value = bytes(value, context); this.offset = 0; this.context = context; }
  readField(name) {
    let length = 0n; let shift = 0n; let used = 0;
    for (; used < 10; used += 1) {
      if (this.offset >= this.value.length) throw new TypeError(`${this.context}.${name} is truncated`);
      const byte = this.value[this.offset++];
      if (used === 9 && (byte & 0xfe) !== 0) throw new TypeError(`${this.context}.${name} length exceeds u64`);
      length |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) { if (used > 0 && byte === 0) throw new TypeError(`${this.context}.${name} length is not minimal`); break; }
      shift += 7n;
    }
    if (used === 10 || length > BigInt(this.value.length - this.offset)) throw new TypeError(`${this.context}.${name} length is invalid`);
    const end = this.offset + Number(length); const payload = this.value.subarray(this.offset, end); this.offset = end; return payload;
  }
  eof() { if (this.offset !== this.value.length) throw new TypeError(`${this.context} contains trailing bytes`); }
}

function normalizeType(type, value, context) {
  switch (type) {
    case T.U8: return Number(unsigned(value, 0xffn, context));
    case T.U16: return Number(unsigned(value, 0xffffn, context));
    case T.U32: return Number(unsigned(value, 0xffff_ffffn, context));
    case T.U64: return unsigned(value, MAX_U64, context);
    case T.U128: return unsigned(value, MAX_U128, context);
    case T.FIXED32: return fixed32(value, context);
    case T.RAW32: return raw32(value, context);
    case T.FIXED24: return fixedBytes(value, 24, context, false);
    case T.NETWORK: if (!(value instanceof NetworkId)) throw new TypeError(`${context} must be a NetworkId`); return value;
    case T.ASSET: return value instanceof KagemushaAssetDefinitionIdV1 ? value : new KagemushaAssetDefinitionIdV1(value);
    case T.INCARNATION: return value instanceof KagemushaAssetIncarnationV1 ? value : new KagemushaAssetIncarnationV1(value);
    case T.ACCOUNT: return value instanceof KagemushaAccountIdV1 ? value : new KagemushaAccountIdV1(value);
    case T.PUBLIC_KEY: return value instanceof KagemushaDevicePublicKeyV1 ? value : new KagemushaDevicePublicKeyV1(value);
    case T.SIGNATURE: return value instanceof KagemushaDeviceSignatureV1 ? value : new KagemushaDeviceSignatureV1(value);
    case T.VECTOR: return bytes(value, context);
    case T.MINT_FRAME: return boundedBytes(value, 7936, context);
    case T.OPERATION_KIND: if (typeof value !== "string" || !OPERATION_KINDS.includes(value)) throw new TypeError(`${context} is invalid`); return value;
    case T.CREDIT_PURPOSE: if (typeof value !== "string" || !CREDIT_PURPOSES.includes(value)) throw new TypeError(`${context} is invalid`); return value;
    case T.COMMIT_EVIDENCE: encodeCommitEvidence(value); return value;
    case T.OPTIONAL_MINT_AUTHORIZATION: return value === null ? null : instance(value, KagemushaMintAuthorizationV1, context);
    default: return instance(value, DEFINITIONS[type].Model, context);
  }
}

function unsigned(value, maximum, context) { let normalized; if (typeof value === "bigint") normalized = value; else if (typeof value === "number" && Number.isSafeInteger(value)) normalized = BigInt(value); else throw new TypeError(`${context} must be an unsigned integer`); if (normalized < 0n || normalized > maximum) throw new RangeError(`${context} is out of range`); return normalized; }
function header(value, positiveAmount = false) { requireVersion(value.version); if (value.scale > 28) throw new RangeError("KAGEMUSHA V1 asset scale exceeds 28"); if (positiveAmount && value.amount === 0n) throw new RangeError("KAGEMUSHA V1 amount must be positive"); }
function requireVersion(value) { if (value !== 1) throw new TypeError("KAGEMUSHA V1 wire version must be 1"); }
function requireX25519Key(value, context) { if (value.length !== 32 || isZero(value)) throw new TypeError(`${context} must be a nonzero 32-byte X25519 key`); }
function bytes(value, context) { if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0)); if (ArrayBuffer.isView(value)) return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)); throw new TypeError(`${context} must be binary data`); }
function boundedBytes(value, maximum, context) {
  if (!(value instanceof ArrayBuffer) && !ArrayBuffer.isView(value)) throw new TypeError(`${context} must be binary data`);
  if (value.byteLength > maximum) throw new RangeError(`KAGEMUSHA V1 ${context} exceeds ${maximum} bytes`);
  return bytes(value, context);
}
function fixedBytes(value, width, context, nonzero = true) { const raw = bytes(value, context); if (raw.length !== width || (nonzero && isZero(raw))) throw new TypeError(`${context} must be ${nonzero ? "one nonzero " : ""}${width}-byte value`); return raw; }
function fixed32(value, context) { return fixedBytes(value, 32, context, true); }
function raw32(value, context) { return fixedBytes(value, 32, context, false); }
function requireFixedArchive(payload, width, context) { if (payload.length !== width * 2) throw new TypeError(`${context} has an invalid fixed-array length`); for (let index = 0; index < width; index += 1) if (payload[index * 2] !== 1) throw new TypeError(`${context} is not a canonical fixed-byte-array payload`); }
function ascii(value) { return UTF8.encode(value); }
function join(...parts) { const arrays = parts.map((part) => bytes(part, "bytes")); const out = new Uint8Array(arrays.reduce((sum, part) => sum + part.length, 0)); let offset = 0; for (const part of arrays) { out.set(part, offset); offset += part.length; } return out; }
function isZero(value) { return value.every((byte) => byte === 0); }
function equalBytes(left, right) { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
function requireEqual(actual, expected, context) { if (!equalBytes(actual, expected)) throw new TypeError(`${context} does not match`); }
function bounded(value, maximum, context) { if (value.length > maximum) throw new RangeError(`KAGEMUSHA V1 ${context} exceeds ${maximum} bytes`); return Uint8Array.from(value); }
function instance(value, Model, context) { if (!(value instanceof Model)) throw new TypeError(`${context} must be a ${Model.name}`); return value; }
function rawValues(value) { return MODEL_VALUES.get(value); }
function cloneValue(value) { return value instanceof Uint8Array ? Uint8Array.from(value) : value; }
function exactRecord(value, context, fields) { if (value === null || typeof value !== "object" || Array.isArray(value)) throw new TypeError(`${context} must be an object`); const actual = Object.keys(value); const expected = new Set(fields); if (actual.length !== fields.length || actual.some((key) => !expected.has(key))) throw new TypeError(`${context} contains missing or unknown fields`); }
function kindLimits(kind) { if (typeof kind !== "string" || !Object.hasOwn(LIMITS, kind)) throw new TypeError("unknown KAGEMUSHA V1 payload kind"); return LIMITS[kind]; }
function ipm1PayloadTag(kind) {
  if (typeof kind !== "string" || !Object.hasOwn(IPM1_PAYLOAD_KINDS, kind)) throw new TypeError("unknown KAGEMUSHA V1 IPM1 payload kind");
  return IPM1_PAYLOAD_KINDS[kind].tag;
}
function ipm1PayloadKindFromTag(tag) {
  if (!Number.isInteger(tag)) throw new TypeError("KAGEMUSHA V1 IPM1 payload tag must be an integer");
  const entry = Object.entries(IPM1_PAYLOAD_KINDS).find(([, value]) => value.tag === tag);
  if (entry === undefined) throw new TypeError("unknown KAGEMUSHA V1 IPM1 payload tag");
  return entry[0];
}

/**
 * Portable canonical codecs and orchestration bindings for KAGEMUSHA V1.
 * Monetary proofs, signing, encryption, decryption, and hardware state changes must be supplied
 * by the release-pinned native implementation; this namespace intentionally has no fallback.
 */
export const Kagemusha = Object.freeze({
  wireVersion: 1,
  deviceLifecycleVersion: 1,
  handoffCapability: "kagemusha_handoff_v1",
  textPrefix: "kgm1:",
  payloadKinds: PAYLOAD_KINDS,
  ipm1PayloadKinds: IPM1_PAYLOAD_KINDS,
  operationKinds: Object.freeze(Object.fromEntries(OPERATION_KINDS.map((kind, tag) => [kind, tag]))),
  maximumRequestRawBytes: LIMITS.paymentRequest[0],
  maximumRequestTextBytes: LIMITS.paymentRequest[1],
  targetCompleteExchangeRawBytes: COMPLETE_EXCHANGE_TARGET_RAW_BYTES,
  maximumCompleteExchangeRawBytes: COMPLETE_EXCHANGE_MAX_RAW_BYTES,
  maximumCompleteExchangeTextBytes: COMPLETE_EXCHANGE_MAX_TEXT_BYTES,
  maximumPairedProofBytes: 6528,
  maximumRedemptionProofBytes: 6528,
  maximumPaymentProofBytes: 6528,
  maximumCommitCertificateBytes: 1024,
  maximumCurrentProofsBytes: 4990,
  maximumParityProofBytes: 2495,
  historyAccumulatorBytes: 544,
  maximumEncryptedCreditBytes: 384,
  maximumCreditOpeningBytes: 256,
  paymentOutboxMinimumBytes: 25728,
  redemptionOutboxMinimumBytes: 26112,
  maximumTopUpRequestBytes: TOP_UP_REQUEST_MAX_BYTES,
  maximumDeviceMintStageCommandBytes: DEVICE_MINT_STAGE_COMMAND_MAX_BYTES,
  maximumDeviceMintStageResultBytes: DEVICE_MINT_STAGE_RESULT_MAX_BYTES,
  deviceMintStageDispositions: Object.freeze({ staged: 0, exactDuplicate: 1 }),
  topUpInstructionWireId: TOP_UP_INSTRUCTION_WIRE_ID,
  maximumRedemptionRequestBytes: 8192,
  maximumOperationStatusBytes: 4 * 1024 * 1024,
  maximumOperationStatusJsonBytes: 16 * 1024 * 1024,
  AssetDefinitionId: KagemushaAssetDefinitionIdV1,
  AssetIncarnation: KagemushaAssetIncarnationV1,
  AccountId: KagemushaAccountIdV1,
  DevicePublicKey: KagemushaDevicePublicKeyV1,
  DeviceSignature: KagemushaDeviceSignatureV1,
  HardwareCredential: KagemushaHardwareCredentialV1,
  PastaStateCommitment: KagemushaPastaStateCommitmentV1,
  PairedProof: KagemushaPairedProofV1,
  CreditOpening: KagemushaCreditOpeningV1,
  EncryptedCreditAad: KagemushaEncryptedCreditAadV1,
  EncryptedCreditEnvelope: KagemushaEncryptedCreditEnvelopeV1,
  LifecycleBinding: KagemushaLifecycleBindingV1,
  TrustedCommitTime: KagemushaTrustedCommitTimeV1,
  MonotonicLease: KagemushaMonotonicLeaseV1,
  OutboxReservation: KagemushaOutboxReservationV1,
  HardwareTerminalBody: KagemushaHardwareTerminalBodyV1,
  CommitCertificate: KagemushaCommitCertificateV1,
  RedemptionProof: KagemushaRedemptionProofV1,
  PaymentProof: KagemushaPaymentProofV1,
  PaymentRequest: KagemushaPaymentRequestV1,
  PeerCreditContext: KagemushaPeerCreditContextV1,
  PaymentOutput: KagemushaPaymentOutputV1,
  Payment: KagemushaPaymentV1,
  InboxReceipt: KagemushaInboxReceiptV1,
  Acknowledgement: KagemushaAcknowledgementV1,
  MintAuthorizationContext: KagemushaMintAuthorizationContextV1,
  MintAuthorizationStatement: KagemushaMintAuthorizationStatementV1,
  MintAuthorization: KagemushaMintAuthorizationV1,
  MintCreditStatement: KagemushaMintCreditStatementV1,
  MintCredit: KagemushaMintCreditV1,
  DeviceMintStageCommand: KagemushaDeviceMintStageCommandV1,
  DeviceMintStageResult: KagemushaDeviceMintStageResultV1,
  RedemptionStatement: KagemushaRedemptionStatementV1,
  RedemptionVoucher: KagemushaRedemptionVoucherV1,
  TopUpRequest: KagemushaTopUpRequestV1,
  RedemptionRequest: KagemushaRedemptionRequestV1,
  encodePaymentRequest, decodePaymentRequest,
  encodeCommitCertificate, decodeCommitCertificate,
  encodeRedemptionProof, decodeRedemptionProof,
  encodePaymentProof, decodePaymentProof,
  encodePeerCreditContext, decodePeerCreditContext,
  encodePayment, decodePayment,
  encodeAcknowledgement, decodeAcknowledgement,
  encodeMintAuthorization, decodeMintAuthorization,
  encodeMintCredit, decodeMintCredit,
  encodeDeviceMintStageCommandShape, decodeDeviceMintStageCommandShapeExact,
  encodeDeviceMintStageResultShape, decodeDeviceMintStageResultShapeExact,
  validateDeviceMintStageResultAgainstCommand,
  encodeRedemptionVoucher, decodeRedemptionVoucher,
  encodeCreditOpening, decodeCreditOpening,
  encodeEncryptedCreditAad, decodeEncryptedCreditAad,
  encodeEncryptedCreditEnvelope, decodeEncryptedCreditEnvelope,
  encodeTopUpRequest, decodeTopUpRequest,
  buildTopUpInstruction, encodeTopUpInstruction, decodeTopUpInstruction,
  encodeRedemptionRequest, decodeRedemptionRequest,
  encodeText, decodeText, encodeTypedText, decodeTypedText,
  validateCompleteExchange,
  validateMintCreditAgainstAuthorization,
  encryptedCreditAadForMint, encryptedCreditAadForPeer, peerCreditContext,
  deviceKeyReference, pastaStateCommitment, liabilityPoolId,
  paymentRequestSigningBytes, paymentRequestDigest, paymentRequestTranscript, assetIdentityDigest, accountIdentityDigest,
  acknowledgementSigningBytes,
  lifecycleBindingDigest: lifecycleDigest, preparedTransferDigest, paymentOutputDigest, paymentOutputTranscript, paymentBodyDigest,
  redemptionId: expectedRedemptionId, redemptionStatementDigest,
  paymentDigest, ciphertextDigest, creditId, expectedCommitCertificateId, commitCertificateDigest,
  peerCreditOpeningCommitment,
  ipm1PayloadTag, ipm1PayloadKindFromTag,
  mintAuthorizationContextDigest, mintAuthorizationStatementDigest, mintAuthorizationDigest,
  mintCreditId, mintCreditStatementDigest,
});
