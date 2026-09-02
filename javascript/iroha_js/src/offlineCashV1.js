// SPDX-License-Identifier: Apache-2.0

import { sha256 } from "@noble/hashes/sha2";

import { crc64Xz } from "./crc64Xz.js";
import { NetworkId, networkIdBytes } from "./networkId.js";
import {
  _canonicalAccountIdNoritoValue,
  encodeAccountIdNoritoValue,
  encodeAssetDefinitionIdNoritoValue,
  validateNoritoFrame,
} from "./norito.js";

const UTF8 = new TextEncoder();
const MODEL = "iroha_data_model::offline::offline_cash_v1::";
const MAX_U64 = (1n << 64n) - 1n;
const MAX_U128 = (1n << 128n) - 1n;
const COMPACT_LENGTHS = 0x02;
const HEADER_BYTES = 40;
const CREDIT_OPENING_BYTES = 200;
const ENCRYPTED_CREDIT_BYTES = CREDIT_OPENING_BYTES + 16;

const SCHEMAS = Object.freeze({
  request: `${MODEL}OfflineCashPaymentRequestV1`,
  acceptanceIntent: `${MODEL}OfflineCashAcceptanceIntentV1`,
  acceptanceAuthorizationStatement: `${MODEL}OfflineCashAcceptanceIntentAuthorizationStatementV1`,
  acceptanceAuthorization: `${MODEL}OfflineCashAcceptanceIntentAuthorizationV1`,
  noCommitClosureStatement: `${MODEL}OfflineCashNoCommitClosureStatementV1`,
  noCommitClosure: `${MODEL}OfflineCashNoCommitClosureV1`,
  acceptanceTicket: `${MODEL}OfflineCashAcceptanceTicketV1`,
  lifecycle: `${MODEL}OfflineCashLifecycleBindingV1`,
  commitCertificate: `${MODEL}OfflineCashCommitCertificateV1`,
  transferStatement: `${MODEL}OfflineCashTransferStatementV1`,
  payment: `${MODEL}OfflineCashPaymentV1`,
  acknowledgement: `${MODEL}OfflineCashAcknowledgementV1`,
  mintAuthorizationContext: `${MODEL}OfflineCashMintAuthorizationContextV1`,
  mintAuthorizationStatement: `${MODEL}OfflineCashMintAuthorizationStatementV1`,
  mintAuthorization: `${MODEL}OfflineCashMintAuthorizationV1`,
  mintStatement: `${MODEL}OfflineCashMintCreditStatementV1`,
  mintCredit: `${MODEL}OfflineCashMintCreditV1`,
  redemptionStatement: `${MODEL}OfflineCashRedemptionStatementV1`,
  redemptionVoucher: `${MODEL}OfflineCashRedemptionVoucherV1`,
  encryptedCreditEnvelope: `${MODEL}OfflineCashEncryptedCreditEnvelopeV1`,
  encryptedCreditAad: `${MODEL}OfflineCashEncryptedCreditAadV1`,
  creditOpening: `${MODEL}OfflineCashCreditOpeningV1`,
  topUpRequest: "iroha.torii.v1.offline_cash.top_up.request",
  redemptionRequest: "iroha.torii.v1.offline_cash.redeem.request",
});

const DOMAIN = Object.freeze({
  deviceKeyReference: ascii("iroha:offline-cash:v1:device-key-reference"),
  pastaStateCommitment: ascii("iroha:offline-cash:v1:pasta-state-commitment"),
  liabilityPool: ascii("iroha:offline-cash:v1:liability-pool"),
  requestSigning: ascii("iroha:offline-cash:v1:payment-request-signing"),
  requestDigest: ascii("iroha:offline-cash:v1:payment-request"),
  acceptanceIntentDigest: ascii("iroha:offline-cash:v1:acceptance-intent"),
  acceptanceAuthorizationStatementDigest: ascii("iroha:offline-cash:v1:acceptance-intent-authorization-statement"),
  acceptanceAuthorizationDigest: ascii("iroha:offline-cash:v1:acceptance-intent-authorization"),
  noCommitClosureStatementDigest: ascii("iroha:offline-cash:v1:no-commit-closure-statement"),
  noCommitClosureDigest: ascii("iroha:offline-cash:v1:no-commit-closure"),
  acceptanceTicketDigest: ascii("iroha:offline-cash:v1:acceptance-ticket"),
  creditId: ascii("iroha:offline-cash:v1:credit-id"),
  lifecycleDigest: ascii("iroha:offline-cash:v1:lifecycle-binding"),
  outboxReservationCommitment: ascii("iroha:offline-cash:v1:outbox-reservation"),
  statementDigest: ascii("iroha:offline-cash:v1:send-split-statement"),
  paymentDigest: ascii("iroha:offline-cash:v1:payment"),
  commitCertificateId: ascii("iroha:offline-cash:v1:commit-certificate-id"),
  commitCertificateDigest: ascii("iroha:offline-cash:v1:commit-certificate"),
  ciphertextDigest: ascii("iroha:offline-cash:v1:ciphertext"),
  mintAuthorizationContextDigest: ascii("iroha:offline-cash:v1:mint-authorization-context"),
  mintAuthorizationStatementDigest: ascii("iroha:offline-cash:v1:mint-authorization-statement"),
  mintAuthorizationDigest: ascii("iroha:offline-cash:v1:mint-authorization"),
  mintStatementDigest: ascii("iroha:offline-cash:v1:mint-statement"),
  redemptionStatementDigest: ascii("iroha:offline-cash:v1:redemption-statement"),
  redemptionId: ascii("iroha:offline-cash:v1:redemption-id"),
});

const LIMITS = Object.freeze({
  paymentRequest: [1024, 1370],
  acceptanceIntent: [256, 346],
  acceptanceIntentAuthorization: [7936, 10586],
  acceptanceTicket: [1024, 1370],
  payment: [7936, 10586],
  acknowledgement: [512, 687],
  mintAuthorization: [7936, 10586],
  mintCredit: [7936, 10586],
  redemptionVoucher: [7936, 10586],
  encryptedCreditEnvelope: [384, 516],
  encryptedCreditAad: [256, 346],
  creditOpening: [256, 346],
});

const PAYLOAD_KINDS = Object.freeze(Object.fromEntries(
  Object.entries(LIMITS).map(([name, [maximumRawBytes, maximumTextBytes]]) => [
    name,
    Object.freeze({ maximumRawBytes, maximumTextBytes }),
  ]),
));

class OfflineCashAssetDefinitionIdV1 {
  #payload;

  constructor(value) {
    const payload = typeof value === "string"
      ? encodeAssetDefinitionIdNoritoValue(value, "Offline Cash V1 asset")
      : bytes(value, "Offline Cash V1 asset payload");
    requireFixedArchive(payload, 16, "Offline Cash V1 asset payload");
    this.#payload = Uint8Array.from(payload);
    Object.freeze(this);
  }

  canonicalPayload() { return Uint8Array.from(this.#payload); }
}

class OfflineCashAccountIdV1 {
  #payload;

  constructor(value) {
    const payload = typeof value === "string"
      ? encodeAccountIdNoritoValue(value, "Offline Cash V1 account")
      : bytes(value, "Offline Cash V1 account payload");
    if (payload.length === 0 || payload.length > 512) throw new RangeError("Offline Cash V1 account payload is empty or oversized");
    const canonical = _canonicalAccountIdNoritoValue(payload, "Offline Cash V1 account");
    if (!equalBytes(payload, canonical)) throw new TypeError("Offline Cash V1 account payload is not canonical");
    this.#payload = Uint8Array.from(payload);
    Object.freeze(this);
  }

  canonicalPayload() { return Uint8Array.from(this.#payload); }
}

class OfflineCashAssetIncarnationV1 {
  #hash;

  constructor(value) {
    const raw = bytes(value, "Offline Cash V1 asset incarnation");
    if (raw.length !== 32 || (raw[31] & 1) !== 1) throw new TypeError("Offline Cash V1 asset incarnation must be a marked 32-byte Iroha hash");
    this.#hash = Uint8Array.from(raw);
    Object.freeze(this);
  }

  hashBytes() { return Uint8Array.from(this.#hash); }
}

class OfflineCashDevicePublicKeyV1 {
  #bytes;

  constructor(value) {
    const raw = bytes(value, "Offline Cash V1 device public key");
    if (raw.length !== 65 || raw[0] !== 4 || isZero(raw.subarray(1))) throw new TypeError("Offline Cash V1 device public key must be nonzero 65-byte uncompressed SEC1");
    this.#bytes = Uint8Array.from(raw);
    Object.freeze(this);
  }

  sec1Bytes() { return Uint8Array.from(this.#bytes); }
}

class OfflineCashDeviceSignatureV1 {
  #bytes;

  constructor(value) {
    const raw = bytes(value, "Offline Cash V1 device signature");
    if (raw.length !== 64 || isZero(raw.subarray(0, 32)) || isZero(raw.subarray(32))) throw new TypeError("Offline Cash V1 device signature must be nonzero fixed-width r || s");
    this.#bytes = Uint8Array.from(raw);
    Object.freeze(this);
  }

  rawBytes() { return Uint8Array.from(this.#bytes); }
}

const T = Object.freeze({
  U16: "u16", U32: "u32", U64: "u64", U128: "u128", FIXED32: "fixed32", RAW32: "raw32",
  FIXED24: "fixed24", NETWORK: "network", ASSET: "asset", INCARNATION: "incarnation", ACCOUNT: "account",
  PUBLIC_KEY: "publicKey", SIGNATURE: "signature", VECTOR: "vector",
  OPERATION_KIND: "operationKind", COMMIT_EVIDENCE: "commitEvidence", CREDIT_PURPOSE: "creditPurpose",
  OPTIONAL_MINT_AUTHORIZATION: "optionalMintAuthorization",
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

    _offlineCashValues() { return MODEL_VALUES.get(this); }
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

const OfflineCashHardwareCredentialV1 = defineModel(
  "OfflineCashHardwareCredentialV1",
  [["version", T.U16], ["credentialId", T.FIXED32], ["networkId", T.NETWORK], ["hardwareProfileId", T.FIXED32],
    ["suiteId", T.FIXED32], ["firmwarePolicyDigest", T.FIXED32], ["policyEpoch", T.U64], ["laneCommitment", T.FIXED32],
    ["hardwareEpochId", T.FIXED32], ["hardwareEpochGeneration", T.U64], ["devicePublicKey", T.PUBLIC_KEY],
    ["deviceKeyReference", T.FIXED32], ["issuedAtMs", T.U64], ["expiresAtMs", T.U64], ["governanceSignature", T.SIGNATURE]],
  (v) => {
    requireVersion(v.version);
    if (v.policyEpoch === 0n || v.issuedAtMs >= v.expiresAtMs) throw new TypeError("Offline Cash V1 hardware credential header is invalid");
    requireEqual(v.deviceKeyReference, deviceKeyReference(v.devicePublicKey), "hardware credential device key reference");
  },
);

const OfflineCashPastaStateCommitmentV1 = defineModel(
  "OfflineCashPastaStateCommitmentV1", [["eq", T.RAW32], ["ep", T.RAW32]],
  (v) => { if (isZero(v.eq) !== isZero(v.ep)) throw new TypeError("Pasta state commitment must be fully zero or fully present"); },
);

const OfflineCashPairedProofV1 = defineModel(
  "OfflineCashPairedProofV1",
  [["version", T.U16], ["eqProtocolDigest", T.FIXED32], ["epProtocolDigest", T.FIXED32], ["semanticDigest", T.FIXED32],
    ["guardEqCredentialAudit", T.FIXED32], ["guardEpCredentialAudit", T.FIXED32], ["eqDeferredAudit", T.FIXED32],
    ["epDeferredAudit", T.FIXED32], ["eqProof", T.VECTOR], ["epProof", T.VECTOR], ["eqHistory", T.VECTOR], ["epHistory", T.VECTOR]],
  validatePairedProofValues,
);

const OfflineCashAcceptanceIntentV1 = defineModel(
  "OfflineCashAcceptanceIntentV1",
  [["version", T.U16], ["requestDigest", T.FIXED32], ["intentId", T.FIXED32], ["exactAmount", T.U128], ["senderOneTimeCommitment", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.exactAmount === 0n) throw new RangeError("acceptance intent amount must be positive"); },
);
const OfflineCashAcceptanceIntentAuthorizationStatementV1 = defineModel(
  "OfflineCashAcceptanceIntentAuthorizationStatementV1",
  [["version", T.U16], ["intent", "OfflineCashAcceptanceIntentV1"], ["releaseId", T.FIXED32], ["suiteId", T.FIXED32],
    ["vkDigest", T.FIXED32], ["artifactManifestDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.intent.version !== v.version) throw new TypeError("acceptance authorization statement version mismatch"); },
);
const OfflineCashAcceptanceIntentAuthorizationV1 = defineModel(
  "OfflineCashAcceptanceIntentAuthorizationV1",
  [["version", T.U16], ["statement", "OfflineCashAcceptanceIntentAuthorizationStatementV1"], ["proof", "OfflineCashPairedProofV1"]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) throw new TypeError("acceptance authorization version mismatch"); },
);

const OfflineCashNoCommitClosureStatementV1 = defineModel(
  "OfflineCashNoCommitClosureStatementV1",
  [["version", T.U16], ["releaseId", T.FIXED32], ["suiteId", T.FIXED32], ["vkDigest", T.FIXED32],
    ["artifactManifestDigest", T.FIXED32], ["senderHardwareBindingCommitment", T.FIXED32],
    ["requestId", T.FIXED32], ["requestDigest", T.FIXED32], ["acceptanceTicketId", T.FIXED32],
    ["ticketDigest", T.FIXED32], ["intentAuthorizationDigest", T.FIXED32], ["intentDigest", T.FIXED32],
    ["exactAmount", T.U128], ["senderOneTimeCommitment", T.FIXED32], ["recoveryId", T.FIXED32],
    ["cancellationNullifier", T.FIXED32], ["equivalentDeliverySlotCommitment", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.exactAmount === 0n) throw new RangeError("no-commit closure amount must be positive"); },
);

const OfflineCashAcceptanceTicketV1 = defineModel(
  "OfflineCashAcceptanceTicketV1",
  [["version", T.U16], ["networkId", T.NETWORK], ["requestId", T.FIXED32], ["requestDigest", T.FIXED32],
    ["acceptanceTicketId", T.FIXED32], ["asset", T.ASSET], ["assetIncarnation", T.INCARNATION], ["scale", T.U32],
    ["intentDigest", T.FIXED32], ["exactAmount", T.U128], ["reservedInboxBytes", T.U32],
    ["recipientOneTimeKey", T.FIXED32], ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64], ["issuedAtMs", T.U64],
    ["expiresAtMs", T.U64], ["signature", T.SIGNATURE]],
  (v) => {
    requireVersion(v.version);
    if (v.exactAmount === 0n || v.reservedInboxBytes < 8960 || v.policyEpoch === 0n || v.issuedAtMs >= v.expiresAtMs) throw new TypeError("Offline Cash V1 acceptance ticket header is invalid");
    requireX25519Key(v.recipientOneTimeKey, "acceptance ticket recipient key");
  },
);

const OfflineCashCreditOpeningV1 = defineModel(
  "OfflineCashCreditOpeningV1",
  [["version", T.U16], ["creditId", T.FIXED32], ["amount", T.U128], ["creditCommitmentOpening", T.FIXED32],
    ["recipientBindingOpening", T.FIXED32], ["recoveryNonce", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.amount === 0n) throw new TypeError("Offline Cash V1 credit opening amount must be positive"); },
);
const OfflineCashEncryptedCreditAadV1 = defineModel(
  "OfflineCashEncryptedCreditAadV1",
  [["version", T.U16], ["purpose", T.CREDIT_PURPOSE], ["contextDigest", T.FIXED32], ["issuanceOrTransitionCommitment", T.FIXED32],
    ["creditId", T.FIXED32], ["amount", T.U128]],
  (v) => { requireVersion(v.version); if (v.amount === 0n) throw new TypeError("Offline Cash V1 encrypted-credit AAD amount must be positive"); },
);
const OfflineCashEncryptedCreditEnvelopeV1 = defineModel(
  "OfflineCashEncryptedCreditEnvelopeV1",
  [["version", T.U16], ["ephemeralX25519PublicKey", T.RAW32], ["nonce", T.FIXED24], ["ciphertextAndTag", T.VECTOR]],
  (v) => {
    requireVersion(v.version);
    requireX25519Key(v.ephemeralX25519PublicKey, "encrypted-credit ephemeral key");
    if (v.ciphertextAndTag.length !== ENCRYPTED_CREDIT_BYTES) throw new TypeError(`Offline Cash V1 ciphertext and tag must be exactly ${ENCRYPTED_CREDIT_BYTES} bytes`);
  },
);

const OPERATION_KINDS = Object.freeze(["bootstrap", "mintFold", "sendSplit", "receiveFold", "redeemSplit", "suiteUpgrade", "rotate"]);
const CREDIT_PURPOSES = Object.freeze(["mint", "peer"]);
const OfflineCashTrustedCommitTimeV1 = defineModel("OfflineCashTrustedCommitTimeV1", [["timeEvidenceCommitment", T.FIXED32]]);
const OfflineCashMonotonicCommitLeaseV1 = defineModel("OfflineCashMonotonicCommitLeaseV1", [["leaseEvidenceCommitment", T.FIXED32]]);
const COMMIT_EVIDENCE = Object.freeze({
  trustedTime: [0, OfflineCashTrustedCommitTimeV1],
  monotonicLease: [1, OfflineCashMonotonicCommitLeaseV1],
});

class OfflineCashCommitEvidenceV1 {
  constructor(input) {
    exactRecord(input, "OfflineCashCommitEvidenceV1", ["source", "evidence"]);
    if (typeof input.source !== "string" || !Object.hasOwn(COMMIT_EVIDENCE, input.source)) throw new TypeError("unknown Offline Cash V1 commit evidence source");
    const Model = COMMIT_EVIDENCE[input.source][1];
    if (!(input.evidence instanceof Model)) throw new TypeError(`commit evidence must be ${Model.name}`);
    this.source = input.source;
    this.evidence = input.evidence;
    Object.freeze(this);
  }

  static trustedTime(timeEvidenceCommitment) { return new this({ source: "trustedTime", evidence: new OfflineCashTrustedCommitTimeV1({ timeEvidenceCommitment }) }); }
  static monotonicLease(leaseEvidenceCommitment) { return new this({ source: "monotonicLease", evidence: new OfflineCashMonotonicCommitLeaseV1({ leaseEvidenceCommitment }) }); }
}

const OfflineCashOutboxReservationV1 = defineModel(
  "OfflineCashOutboxReservationV1",
  [["reservationId", T.FIXED32], ["operationKind", T.OPERATION_KIND], ["reservedOutboxBytes", T.U32],
    ["issuedAtMs", T.U64], ["expiresAtMs", T.U64]],
  (v) => {
    if (!new Set(["sendSplit", "redeemSplit"]).has(v.operationKind)
        || v.reservedOutboxBytes < 26112
        || v.issuedAtMs >= v.expiresAtMs) {
      throw new TypeError("Offline Cash V1 outbox reservation is invalid");
    }
  },
);

const OfflineCashLifecycleBindingV1 = defineModel(
  "OfflineCashLifecycleBindingV1",
  [["version", T.U16], ["networkId", T.NETWORK], ["protocolVersion", T.U16], ["suiteId", T.FIXED32], ["vkDigest", T.FIXED32],
    ["releaseId", T.FIXED32], ["asset", T.ASSET], ["assetIncarnation", T.INCARNATION], ["scale", T.U32],
    ["liabilityPoolId", T.FIXED32], ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64], ["operationKind", T.OPERATION_KIND],
    ["requestId", T.RAW32], ["acceptanceTicketId", T.RAW32], ["creditId", T.RAW32], ["ciphertextDigest", T.RAW32]],
  (v) => {
    requireVersion(v.version);
    if (v.protocolVersion !== 1 || v.policyEpoch === 0n) throw new TypeError("Offline Cash V1 lifecycle header is invalid");
    requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "lifecycle liability pool");
    const requestFieldsAreSet = !isZero(v.requestId) && !isZero(v.acceptanceTicketId);
    const creditFieldsAreSet = !isZero(v.creditId) && !isZero(v.ciphertextDigest);
    const allAreZero = [v.requestId, v.acceptanceTicketId, v.creditId, v.ciphertextDigest].every(isZero);
    if ((v.operationKind === "sendSplit" && !(requestFieldsAreSet && creditFieldsAreSet))
        || (v.operationKind === "mintFold" && (!isZero(v.requestId) || !isZero(v.acceptanceTicketId) || !creditFieldsAreSet))
        || (!new Set(["sendSplit", "mintFold"]).has(v.operationKind) && !allAreZero)) {
      throw new TypeError("Offline Cash V1 lifecycle operation identities are invalid");
    }
  },
);
const OfflineCashCommitCertificateV1 = defineModel(
  "OfflineCashCommitCertificateV1",
  [["version", T.U16], ["certificateId", T.FIXED32], ["candidateEnvelopeDigest", T.FIXED32], ["lifecycleBindingDigest", T.FIXED32],
    ["transitionNullifier", T.FIXED32], ["outboxReservationCommitment", T.FIXED32], ["commitEvidence", T.COMMIT_EVIDENCE],
    ["hardwareProfileId", T.FIXED32], ["policyEpoch", T.U64], ["hardwareTerminalCommitment", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.policyEpoch === 0n) throw new TypeError("Offline Cash V1 commit certificate policy epoch must be positive"); },
);
const OfflineCashCommitWrapperProofV1 = defineModel(
  "OfflineCashCommitWrapperProofV1",
  [["version", T.U16], ["eqProtocolDigest", T.FIXED32], ["epProtocolDigest", T.FIXED32], ["semanticDigest", T.FIXED32],
    ["candidateEnvelopeDigest", T.FIXED32], ["commitCertificateDigest", T.FIXED32], ["eqDeferredAudit", T.FIXED32],
    ["epDeferredAudit", T.FIXED32], ["eqProof", T.VECTOR], ["epProof", T.VECTOR], ["eqHistory", T.VECTOR], ["epHistory", T.VECTOR]],
  validateProofVectors,
);

const OfflineCashPaymentRequestV1 = defineModel(
  "OfflineCashPaymentRequestV1",
  [["version", T.U16], ["releaseId", T.FIXED32], ["networkId", T.NETWORK], ["asset", T.ASSET], ["assetIncarnation", T.INCARNATION],
    ["scale", T.U32], ["liabilityPoolId", T.FIXED32], ["recipient", T.ACCOUNT], ["amount", T.U128],
    ["hardwareCredential", "OfflineCashHardwareCredentialV1"], ["requestId", T.FIXED32], ["issuedAtMs", T.U64], ["expiresAtMs", T.U64],
    ["signature", T.SIGNATURE]],
  (v) => {
    header(v, true);
    if (v.expiresAtMs <= v.issuedAtMs || v.expiresAtMs - v.issuedAtMs > 300000n) throw new RangeError("Offline Cash V1 request validity window is invalid");
    if (!equalBytes(networkIdBytes(v.networkId), networkIdBytes(v.hardwareCredential.networkId)) || v.issuedAtMs < v.hardwareCredential.issuedAtMs || v.expiresAtMs > v.hardwareCredential.expiresAtMs) throw new TypeError("Offline Cash V1 request credential binding is invalid");
  },
);
const OfflineCashNoCommitClosureV1 = defineModel(
  "OfflineCashNoCommitClosureV1",
  [["version", T.U16], ["statement", "OfflineCashNoCommitClosureStatementV1"],
    ["request", "OfflineCashPaymentRequestV1"],
    ["intentAuthorization", "OfflineCashAcceptanceIntentAuthorizationV1"],
    ["acceptanceTicket", "OfflineCashAcceptanceTicketV1"], ["proof", "OfflineCashPairedProofV1"]],
  (v) => {
    requireVersion(v.version);
    if ([v.statement, v.request, v.intentAuthorization, v.acceptanceTicket, v.proof]
      .some((value) => value.version !== v.version)) throw new TypeError("no-commit closure version mismatch");
  },
);
const OfflineCashTransferStatementV1 = defineModel(
  "OfflineCashTransferStatementV1",
  [["version", T.U16], ["lifecycle", "OfflineCashLifecycleBindingV1"], ["amount", T.U128], ["transitionNullifier", T.FIXED32],
    ["requestDigest", T.FIXED32], ["acceptanceTicketDigest", T.FIXED32], ["recipientOneTimeKey", T.FIXED32],
    ["ciphertextCommitment", T.FIXED32], ["commitEvidence", T.COMMIT_EVIDENCE]],
  (v) => {
    requireVersion(v.version);
    if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "sendSplit" || v.amount === 0n) throw new TypeError("Offline Cash V1 transfer statement is invalid");
    requireX25519Key(v.recipientOneTimeKey, "transfer recipient key");
  },
);
const OfflineCashPaymentV1 = defineModel(
  "OfflineCashPaymentV1",
  [["version", T.U16], ["statement", "OfflineCashTransferStatementV1"], ["acceptanceIntent", "OfflineCashAcceptanceIntentV1"],
    ["acceptanceTicket", "OfflineCashAcceptanceTicketV1"], ["commitCertificate", "OfflineCashCommitCertificateV1"],
    ["proof", "OfflineCashCommitWrapperProofV1"], ["encryptedCredit", T.VECTOR], ["artifactManifestDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if ([v.statement.version, v.acceptanceIntent.version, v.acceptanceTicket.version, v.commitCertificate.version, v.proof.version].some((version) => version !== v.version)) throw new TypeError("Offline Cash V1 payment version mismatch"); },
);
const OfflineCashInboxReceiptV1 = defineModel(
  "OfflineCashInboxReceiptV1", [["version", T.U16], ["creditId", T.FIXED32], ["receiptCommitment", T.FIXED32]],
  (v) => requireVersion(v.version),
);
const OfflineCashAcknowledgementV1 = defineModel(
  "OfflineCashAcknowledgementV1",
  [["version", T.U16], ["requestDigest", T.FIXED32], ["paymentDigest", T.FIXED32], ["inboxReceipt", "OfflineCashInboxReceiptV1"], ["signature", T.SIGNATURE]],
  (v) => { requireVersion(v.version); if (v.inboxReceipt.version !== v.version) throw new TypeError("Offline Cash V1 acknowledgement version mismatch"); },
);

const OfflineCashMintAuthorizationContextV1 = defineModel(
  "OfflineCashMintAuthorizationContextV1",
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
const OfflineCashMintAuthorizationStatementV1 = defineModel(
  "OfflineCashMintAuthorizationStatementV1",
  [["version", T.U16], ["context", "OfflineCashMintAuthorizationContextV1"], ["issuanceCommitment", T.FIXED32], ["creditId", T.FIXED32], ["ciphertextDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.context.version !== v.version) throw new TypeError("mint authorization statement version mismatch"); },
);
const OfflineCashMintAuthorizationV1 = defineModel(
  "OfflineCashMintAuthorizationV1",
  [["version", T.U16], ["statement", "OfflineCashMintAuthorizationStatementV1"], ["proof", "OfflineCashPairedProofV1"]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) throw new TypeError("mint authorization version mismatch"); },
);
const OfflineCashMintCreditStatementV1 = defineModel(
  "OfflineCashMintCreditStatementV1",
  [["version", T.U16], ["lifecycle", "OfflineCashLifecycleBindingV1"], ["recipientCredentialCommitment", T.FIXED32],
    ["authorizationContextDigest", T.FIXED32], ["mintAuthorizationDigest", T.FIXED32], ["amount", T.U128],
    ["issuanceCommitment", T.FIXED32], ["recipient", T.ACCOUNT], ["creditCommitment", T.FIXED32], ["mintedAtMs", T.U64]],
  (v) => { requireVersion(v.version); if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "mintFold" || v.amount === 0n || v.mintedAtMs === 0n) throw new TypeError("Offline Cash V1 mint statement is invalid"); },
);
const OfflineCashMintCreditV1 = defineModel(
  "OfflineCashMintCreditV1",
  [["version", T.U16], ["statement", "OfflineCashMintCreditStatementV1"], ["proof", "OfflineCashPairedProofV1"],
    ["finalityCertificateBinding", T.FIXED32], ["finalityAuthorityHead", T.FIXED32], ["finalityGenesisRosterId", T.FIXED32],
    ["finalityProofBindingDigest", T.FIXED32], ["encryptedCredit", T.VECTOR], ["artifactManifestDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) throw new TypeError("mint credit version mismatch"); },
);

const OfflineCashRedemptionStatementV1 = defineModel(
  "OfflineCashRedemptionStatementV1",
  [["version", T.U16], ["lifecycle", "OfflineCashLifecycleBindingV1"], ["amount", T.U128], ["beneficiary", T.ACCOUNT],
    ["terminalNullifier", T.FIXED32], ["redemptionCommitment", T.FIXED32], ["redemptionId", T.FIXED32], ["commitEvidence", T.COMMIT_EVIDENCE]],
  (v) => { requireVersion(v.version); if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "redeemSplit" || v.amount === 0n) throw new TypeError("Offline Cash V1 redemption statement is invalid"); },
);
const OfflineCashRedemptionVoucherV1 = defineModel(
  "OfflineCashRedemptionVoucherV1",
  [["version", T.U16], ["statement", "OfflineCashRedemptionStatementV1"], ["commitCertificate", "OfflineCashCommitCertificateV1"],
    ["proof", "OfflineCashCommitWrapperProofV1"], ["artifactManifestDigest", T.FIXED32]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.commitCertificate.version !== v.version || v.proof.version !== v.version) throw new TypeError("redemption voucher version mismatch"); },
);

const OfflineCashTopUpRequestV1 = defineModel(
  "OfflineCashTopUpRequestV1",
  [["version", T.U16], ["operationId", T.FIXED32], ["issuanceCommitment", T.FIXED32], ["creditId", T.FIXED32],
    ["releaseId", T.FIXED32], ["suiteId", T.FIXED32], ["vkDigest", T.FIXED32], ["networkId", T.NETWORK], ["asset", T.ASSET],
    ["assetIncarnation", T.INCARNATION], ["scale", T.U32], ["amount", T.U128], ["liabilityPoolId", T.FIXED32],
    ["payer", T.ACCOUNT], ["recipient", T.ACCOUNT], ["hardwareCredential", "OfflineCashHardwareCredentialV1"],
    ["recipientCredentialCommitment", T.FIXED32], ["creditCommitment", T.FIXED32], ["recipientOneTimeKey", T.FIXED32],
    ["encryptedCredit", T.VECTOR], ["artifactManifestDigest", T.FIXED32], ["mintAuthorization", T.OPTIONAL_MINT_AUTHORIZATION]],
  (v) => { header(v, true); requireX25519Key(v.recipientOneTimeKey, "top-up recipient key"); },
);
const OfflineCashRedemptionRequestV1 = defineModel(
  "OfflineCashRedemptionRequestV1", [["version", T.U16], ["operationId", T.FIXED32], ["voucher", "OfflineCashRedemptionVoucherV1"]],
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

const encodePaymentRequest = (value) => encodeTopLevel(value, OfflineCashPaymentRequestV1, SCHEMAS.request, 1024, validateRequest);
const decodePaymentRequest = (raw) => decodeTopLevel(raw, OfflineCashPaymentRequestV1, SCHEMAS.request, 1024, validateRequest);
const encodeAcceptanceIntent = (value) => encodeTopLevel(value, OfflineCashAcceptanceIntentV1, SCHEMAS.acceptanceIntent, 256);
const decodeAcceptanceIntent = (raw) => decodeTopLevel(raw, OfflineCashAcceptanceIntentV1, SCHEMAS.acceptanceIntent, 256);
const encodeAcceptanceIntentAuthorization = (value, request) => encodeTopLevel(value, OfflineCashAcceptanceIntentAuthorizationV1, SCHEMAS.acceptanceAuthorization, 7936, (model) => validateAcceptanceAuthorization(model, request));
const decodeAcceptanceIntentAuthorization = (raw, request) => decodeTopLevel(raw, OfflineCashAcceptanceIntentAuthorizationV1, SCHEMAS.acceptanceAuthorization, 7936, (model) => validateAcceptanceAuthorization(model, request));
const encodeAcceptanceTicket = (value, request, intent) => encodeTopLevel(value, OfflineCashAcceptanceTicketV1, SCHEMAS.acceptanceTicket, 1024, (model) => validateAcceptanceTicket(model, request, intent));
const decodeAcceptanceTicket = (raw, request, intent) => decodeTopLevel(raw, OfflineCashAcceptanceTicketV1, SCHEMAS.acceptanceTicket, 1024, (model) => validateAcceptanceTicket(model, request, intent));
const encodeNoCommitClosure = (value) => encodeTopLevel(value, OfflineCashNoCommitClosureV1, SCHEMAS.noCommitClosure, 16384, validateNoCommitClosure);
const decodeNoCommitClosure = (raw) => decodeTopLevel(raw, OfflineCashNoCommitClosureV1, SCHEMAS.noCommitClosure, 16384, validateNoCommitClosure);
const encodePayment = (value, request) => encodeTopLevel(value, OfflineCashPaymentV1, SCHEMAS.payment, 7936, (model) => validatePayment(model, request));
const decodePayment = (raw, request) => decodeTopLevel(raw, OfflineCashPaymentV1, SCHEMAS.payment, 7936, (model) => validatePayment(model, request));
const encodeAcknowledgement = (value, request, payment) => encodeTopLevel(value, OfflineCashAcknowledgementV1, SCHEMAS.acknowledgement, 512, (model) => validateAcknowledgement(model, request, payment));
const decodeAcknowledgement = (raw, request, payment) => decodeTopLevel(raw, OfflineCashAcknowledgementV1, SCHEMAS.acknowledgement, 512, (model) => validateAcknowledgement(model, request, payment));
const encodeMintAuthorization = (value) => encodeTopLevel(value, OfflineCashMintAuthorizationV1, SCHEMAS.mintAuthorization, 7936, validateMintAuthorization);
const decodeMintAuthorization = (raw) => decodeTopLevel(raw, OfflineCashMintAuthorizationV1, SCHEMAS.mintAuthorization, 7936, validateMintAuthorization);
const encodeMintCredit = (value, authorization) => encodeTopLevel(value, OfflineCashMintCreditV1, SCHEMAS.mintCredit, 7936, (model) => validateMintCredit(model, authorization));
const decodeMintCredit = (raw, authorization) => decodeTopLevel(raw, OfflineCashMintCreditV1, SCHEMAS.mintCredit, 7936, (model) => validateMintCredit(model, authorization));
const encodeRedemptionVoucher = (value) => encodeTopLevel(value, OfflineCashRedemptionVoucherV1, SCHEMAS.redemptionVoucher, 7936, validateRedemptionVoucher);
const decodeRedemptionVoucher = (raw) => decodeTopLevel(raw, OfflineCashRedemptionVoucherV1, SCHEMAS.redemptionVoucher, 7936, validateRedemptionVoucher);
const encodeEncryptedCreditAad = (value) => encodeTopLevel(value, OfflineCashEncryptedCreditAadV1, SCHEMAS.encryptedCreditAad, 256);
const decodeEncryptedCreditAad = (raw) => decodeTopLevel(raw, OfflineCashEncryptedCreditAadV1, SCHEMAS.encryptedCreditAad, 256);
const encodeEncryptedCreditEnvelope = (value, recipientKey) => encodeTopLevel(value, OfflineCashEncryptedCreditEnvelopeV1, SCHEMAS.encryptedCreditEnvelope, 384, (model) => validateEnvelopeRecipient(model, recipientKey));
const decodeEncryptedCreditEnvelope = (raw, recipientKey) => decodeTopLevel(raw, OfflineCashEncryptedCreditEnvelopeV1, SCHEMAS.encryptedCreditEnvelope, 384, (model) => validateEnvelopeRecipient(model, recipientKey));

function encodeCreditOpening(value) {
  const raw = encodeTopLevel(value, OfflineCashCreditOpeningV1, SCHEMAS.creditOpening, 256);
  if (raw.length !== CREDIT_OPENING_BYTES) throw new TypeError("Offline Cash V1 credit opening has a noncanonical fixed size");
  return raw;
}

function decodeCreditOpening(raw, creditIdValue, amount) {
  const opening = decodeTopLevel(raw, OfflineCashCreditOpeningV1, SCHEMAS.creditOpening, 256, (value) => {
    if (creditIdValue !== undefined) requireEqual(value.creditId, fixed32(creditIdValue, "creditId"), "credit opening credit ID");
    if (amount !== undefined && value.amount !== unsigned(amount, MAX_U128, "amount")) throw new TypeError("credit opening amount does not match");
  });
  if (bytes(raw, "credit opening").length !== CREDIT_OPENING_BYTES) throw new TypeError("Offline Cash V1 credit opening has a noncanonical fixed size");
  return opening;
}

function validateRequest(request) {
  const v = rawValues(instance(request, OfflineCashPaymentRequestV1, "payment request"));
  requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "request liability pool");
}

function validateAcceptanceIntent(intent, request) {
  const i = rawValues(instance(intent, OfflineCashAcceptanceIntentV1, "acceptance intent"));
  const bound = instance(request, OfflineCashPaymentRequestV1, "payment request");
  requireEqual(i.requestDigest, paymentRequestDigest(bound), "acceptance intent request digest");
  if (bound.amount !== i.exactAmount) throw new TypeError("acceptance intent amount does not equal the request amount");
}

function validateAcceptanceAuthorization(authorization, request) {
  const v = rawValues(instance(authorization, OfflineCashAcceptanceIntentAuthorizationV1, "acceptance authorization"));
  if (request === undefined) throw new TypeError("acceptance authorization requires its signed request");
  validateAcceptanceIntent(v.statement.intent, request);
  requireEqual(v.statement.releaseId, request.releaseId, "acceptance authorization release");
  requireEqual(v.statement.suiteId, request.hardwareCredential.suiteId, "acceptance authorization suite");
  const digest = acceptanceAuthorizationStatementDigest(v.statement);
  requireEqual(v.proof.semanticDigest, digest, "acceptance authorization proof semantic digest");
}

function acceptanceIntentCircuitTranscript(intent) {
  const value = rawValues(instance(intent, OfflineCashAcceptanceIntentV1, "acceptance intent"));
  return join(
    u16(value.version),
    value.requestDigest,
    value.intentId,
    u128(value.exactAmount),
    value.senderOneTimeCommitment,
  );
}

function acceptanceIntentDigest(intent) {
  return digestEncoded(DOMAIN.acceptanceIntentDigest, acceptanceIntentCircuitTranscript(intent));
}

function acceptanceAuthorizationStatementCircuitTranscript(statement) {
  const value = rawValues(instance(
    statement,
    OfflineCashAcceptanceIntentAuthorizationStatementV1,
    "acceptance authorization statement",
  ));
  return join(
    u16(value.version),
    acceptanceIntentCircuitTranscript(value.intent),
    value.releaseId,
    value.suiteId,
    value.vkDigest,
    value.artifactManifestDigest,
  );
}

function acceptanceAuthorizationStatementDigest(statement) {
  return digestEncoded(
    DOMAIN.acceptanceAuthorizationStatementDigest,
    acceptanceAuthorizationStatementCircuitTranscript(statement),
  );
}

function acceptanceAuthorizationDigest(authorization) {
  return digestModel(DOMAIN.acceptanceAuthorizationDigest, SCHEMAS.acceptanceAuthorization, instance(authorization, OfflineCashAcceptanceIntentAuthorizationV1, "acceptance authorization"));
}

function noCommitClosureStatementDigest(statement) {
  const value = rawValues(instance(
    statement,
    OfflineCashNoCommitClosureStatementV1,
    "no-commit closure statement",
  ));
  const transcript = join(
    u16(value.version),
    value.releaseId,
    value.suiteId,
    value.vkDigest,
    value.artifactManifestDigest,
    value.senderHardwareBindingCommitment,
    value.requestId,
    value.requestDigest,
    value.acceptanceTicketId,
    value.ticketDigest,
    value.intentAuthorizationDigest,
    value.intentDigest,
    u128(value.exactAmount),
    value.senderOneTimeCommitment,
    value.recoveryId,
    value.cancellationNullifier,
    value.equivalentDeliverySlotCommitment,
  );
  return digestEncoded(DOMAIN.noCommitClosureStatementDigest, transcript);
}

function noCommitClosureDigest(closure) {
  validateNoCommitClosure(closure);
  return digestModel(DOMAIN.noCommitClosureDigest, SCHEMAS.noCommitClosure, closure);
}

function validateAcceptanceTicket(ticket, request, intent) {
  const t = rawValues(instance(ticket, OfflineCashAcceptanceTicketV1, "acceptance ticket"));
  const boundRequest = instance(request, OfflineCashPaymentRequestV1, "payment request");
  const boundIntent = instance(intent, OfflineCashAcceptanceIntentV1, "acceptance intent");
  validateAcceptanceIntent(boundIntent, boundRequest);
  requireEqual(t.requestId, boundRequest.requestId, "ticket request ID");
  requireEqual(t.requestDigest, paymentRequestDigest(boundRequest), "ticket request digest");
  requireEqual(t.intentDigest, acceptanceIntentDigest(boundIntent), "ticket intent digest");
  if (!equalBytes(networkIdBytes(t.networkId), networkIdBytes(boundRequest.networkId))
      || !equalBytes(encodeType(T.ASSET, t.asset), encodeType(T.ASSET, boundRequest.asset))
      || !equalBytes(encodeType(T.INCARNATION, t.assetIncarnation), encodeType(T.INCARNATION, boundRequest.assetIncarnation))
      || t.scale !== boundRequest.scale
      || t.exactAmount !== boundIntent.exactAmount
      || !equalBytes(t.hardwareProfileId, boundRequest.hardwareCredential.hardwareProfileId)
      || t.policyEpoch !== boundRequest.hardwareCredential.policyEpoch
      || t.issuedAtMs < boundRequest.issuedAtMs
      || t.expiresAtMs > boundRequest.expiresAtMs
      || t.issuedAtMs >= t.expiresAtMs
      || t.exactAmount !== boundRequest.amount) {
    throw new TypeError("Offline Cash V1 ticket request binding is invalid");
  }
}

function acceptanceTicketDigest(ticket) {
  return digestModel(DOMAIN.acceptanceTicketDigest, SCHEMAS.acceptanceTicket, instance(ticket, OfflineCashAcceptanceTicketV1, "acceptance ticket"));
}

function validateNoCommitClosure(closure) {
  const value = rawValues(instance(closure, OfflineCashNoCommitClosureV1, "no-commit closure"));
  const statement = rawValues(value.statement);
  validateRequest(value.request);
  validateAcceptanceAuthorization(value.intentAuthorization, value.request);
  const intent = value.intentAuthorization.statement.intent;
  validateAcceptanceTicket(value.acceptanceTicket, value.request, intent);
  const bindings = [
    [statement.requestId, value.request.requestId, "no-commit closure request ID"],
    [statement.requestDigest, paymentRequestDigest(value.request), "no-commit closure request digest"],
    [statement.acceptanceTicketId, value.acceptanceTicket.acceptanceTicketId, "no-commit closure ticket ID"],
    [statement.ticketDigest, acceptanceTicketDigest(value.acceptanceTicket), "no-commit closure ticket digest"],
    [statement.intentAuthorizationDigest, acceptanceAuthorizationDigest(value.intentAuthorization), "no-commit closure authorization digest"],
    [statement.intentDigest, acceptanceIntentDigest(intent), "no-commit closure intent digest"],
    [statement.senderOneTimeCommitment, intent.senderOneTimeCommitment, "no-commit closure sender commitment"],
    [statement.releaseId, value.intentAuthorization.statement.releaseId, "no-commit closure release"],
    [statement.suiteId, value.intentAuthorization.statement.suiteId, "no-commit closure suite"],
    [statement.vkDigest, value.intentAuthorization.statement.vkDigest, "no-commit closure verifying key"],
    [statement.artifactManifestDigest, value.intentAuthorization.statement.artifactManifestDigest, "no-commit closure artifact manifest"],
  ];
  for (const [actual, expected, context] of bindings) requireEqual(actual, expected, context);
  if (statement.exactAmount !== intent.exactAmount || statement.exactAmount !== value.acceptanceTicket.exactAmount) {
    throw new TypeError("no-commit closure exact amount does not match");
  }
  requireEqual(value.proof.semanticDigest, noCommitClosureStatementDigest(value.statement), "no-commit closure proof semantic digest");
  if (frame(SCHEMAS.noCommitClosure, encodeModel(closure), modelAlignment(OfflineCashNoCommitClosureV1)).length > 16384) {
    throw new RangeError("Offline Cash V1 no-commit closure exceeds 16384 bytes");
  }
}

function lifecycleDigest(lifecycle) {
  return digestModel(
    DOMAIN.lifecycleDigest,
    SCHEMAS.lifecycle,
    instance(lifecycle, OfflineCashLifecycleBindingV1, "lifecycle binding"),
    1,
  );
}

function expectedCommitCertificateId(certificate) {
  const value = instance(
    certificate,
    OfflineCashCommitCertificateV1,
    "commit certificate",
  );
  return digestEncoded(DOMAIN.commitCertificateId, commitCertificateCircuitTranscript(value, false));
}

function commitCertificateDigest(certificate) {
  return digestEncoded(
    DOMAIN.commitCertificateDigest,
    commitCertificateCircuitTranscript(certificate, true),
  );
}

function commitEvidenceCircuitTranscript(evidence) {
  const value = instance(evidence, OfflineCashCommitEvidenceV1, "commit evidence");
  const tag = COMMIT_EVIDENCE[value.source][0];
  const payload = rawValues(value.evidence);
  const commitment = value.source === "trustedTime"
    ? payload.timeEvidenceCommitment
    : payload.leaseEvidenceCommitment;
  return join(u32(tag), commitment);
}

function outboxReservationCommitment(reservation) {
  const value = rawValues(instance(
    reservation,
    OfflineCashOutboxReservationV1,
    "outbox reservation",
  ));
  return digestEncoded(
    DOMAIN.outboxReservationCommitment,
    join(
      value.reservationId,
      u32(OPERATION_KINDS.indexOf(value.operationKind)),
      u32(value.reservedOutboxBytes),
      u64(value.issuedAtMs),
      u64(value.expiresAtMs),
    ),
  );
}

function commitCertificateCircuitTranscript(certificate, includeCertificateId) {
  const value = rawValues(instance(
    certificate,
    OfflineCashCommitCertificateV1,
    "commit certificate",
  ));
  return join(
    u16(value.version),
    ...(includeCertificateId ? [value.certificateId] : []),
    value.candidateEnvelopeDigest,
    value.lifecycleBindingDigest,
    value.transitionNullifier,
    value.outboxReservationCommitment,
    commitEvidenceCircuitTranscript(value.commitEvidence),
    value.hardwareProfileId,
    u64(value.policyEpoch),
    value.hardwareTerminalCommitment,
  );
}

function validateCommitCertificate(certificate, lifecycle, transitionNullifier, evidence) {
  const value = rawValues(instance(
    certificate,
    OfflineCashCommitCertificateV1,
    "commit certificate",
  ));
  const boundLifecycle = instance(lifecycle, OfflineCashLifecycleBindingV1, "lifecycle binding");
  requireEqual(value.lifecycleBindingDigest, lifecycleDigest(boundLifecycle), "commit certificate lifecycle digest");
  requireEqual(value.transitionNullifier, transitionNullifier, "commit certificate transition nullifier");
  requireEqual(value.hardwareProfileId, boundLifecycle.hardwareProfileId, "commit certificate hardware profile");
  if (value.policyEpoch !== boundLifecycle.policyEpoch) {
    throw new TypeError("commit certificate policy epoch does not match");
  }
  requireEqual(
    encodeType(T.COMMIT_EVIDENCE, value.commitEvidence),
    encodeType(T.COMMIT_EVIDENCE, evidence),
    "commit certificate evidence",
  );
  requireEqual(value.certificateId, expectedCommitCertificateId(certificate), "commit certificate ID");
}

function validateWrapperProof(proof, semanticDigest, certificate) {
  const value = rawValues(instance(proof, OfflineCashCommitWrapperProofV1, "commit wrapper proof"));
  const boundCertificate = instance(
    certificate,
    OfflineCashCommitCertificateV1,
    "commit certificate",
  );
  requireEqual(value.semanticDigest, semanticDigest, "commit wrapper semantic digest");
  requireEqual(
    value.candidateEnvelopeDigest,
    boundCertificate.candidateEnvelopeDigest,
    "commit wrapper candidate digest",
  );
  requireEqual(
    value.commitCertificateDigest,
    commitCertificateDigest(boundCertificate),
    "commit wrapper certificate digest",
  );
}

function validatePayment(payment, request) {
  const p = rawValues(instance(payment, OfflineCashPaymentV1, "payment"));
  const boundRequest = instance(request, OfflineCashPaymentRequestV1, "payment request");
  validateAcceptanceTicket(p.acceptanceTicket, boundRequest, p.acceptanceIntent);
  const s = rawValues(p.statement);
  requireEqual(s.requestDigest, paymentRequestDigest(boundRequest), "payment request digest");
  requireEqual(s.acceptanceTicketDigest, acceptanceTicketDigest(p.acceptanceTicket), "payment ticket digest");
  requireEqual(s.recipientOneTimeKey, p.acceptanceTicket.recipientOneTimeKey, "payment recipient key");
  requireEqual(s.lifecycle.requestId, boundRequest.requestId, "payment lifecycle request ID");
  requireEqual(s.lifecycle.acceptanceTicketId, p.acceptanceTicket.acceptanceTicketId, "payment lifecycle ticket ID");
  if (s.amount !== p.acceptanceTicket.exactAmount
      || !equalBytes(s.lifecycle.releaseId, boundRequest.releaseId)
      || !equalBytes(networkIdBytes(s.lifecycle.networkId), networkIdBytes(boundRequest.networkId))
      || !equalBytes(encodeType(T.ASSET, s.lifecycle.asset), encodeType(T.ASSET, boundRequest.asset))
      || !equalBytes(encodeType(T.INCARNATION, s.lifecycle.assetIncarnation), encodeType(T.INCARNATION, boundRequest.assetIncarnation))
      || s.lifecycle.scale !== boundRequest.scale
      || !equalBytes(s.lifecycle.liabilityPoolId, boundRequest.liabilityPoolId)
      || !equalBytes(s.lifecycle.suiteId, boundRequest.hardwareCredential.suiteId)) {
    throw new TypeError("payment does not match the exact request and ticket");
  }
  decodeEncryptedCreditEnvelope(p.encryptedCredit, s.recipientOneTimeKey);
  requireEqual(s.lifecycle.ciphertextDigest, ciphertextDigest(p.encryptedCredit), "payment ciphertext digest");
  requireEqual(s.lifecycle.creditId, creditId(s.transitionNullifier, s.requestDigest, s.acceptanceTicketDigest, s.recipientOneTimeKey, s.amount, s.ciphertextCommitment), "payment credit ID");
  const digest = digestModel(DOMAIN.statementDigest, SCHEMAS.transferStatement, p.statement);
  validateCommitCertificate(p.commitCertificate, s.lifecycle, s.transitionNullifier, s.commitEvidence);
  validateWrapperProof(p.proof, digest, p.commitCertificate);
  if (frame(SCHEMAS.payment, encodeModel(payment), modelAlignment(OfflineCashPaymentV1)).length > p.acceptanceTicket.reservedInboxBytes) throw new RangeError("payment exceeds its reserved inbox capacity");
}

function paymentDigest(payment, request) {
  validatePayment(payment, request);
  return digestModel(DOMAIN.paymentDigest, SCHEMAS.payment, payment);
}

function validateAcknowledgement(acknowledgement, request, payment) {
  const a = rawValues(instance(acknowledgement, OfflineCashAcknowledgementV1, "acknowledgement"));
  const p = instance(payment, OfflineCashPaymentV1, "payment");
  requireEqual(a.requestDigest, paymentRequestDigest(request), "acknowledgement request digest");
  requireEqual(a.paymentDigest, paymentDigest(p, request), "acknowledgement payment digest");
  requireEqual(a.inboxReceipt.creditId, p.statement.lifecycle.creditId, "acknowledgement credit ID");
}

function validateMintAuthorization(authorization) {
  const a = rawValues(instance(authorization, OfflineCashMintAuthorizationV1, "mint authorization"));
  const digest = digestModel(DOMAIN.mintAuthorizationStatementDigest, SCHEMAS.mintAuthorizationStatement, a.statement);
  requireEqual(a.proof.semanticDigest, digest, "mint authorization proof semantic digest");
}

function mintAuthorizationContextDigest(context) {
  return digestModel(DOMAIN.mintAuthorizationContextDigest, SCHEMAS.mintAuthorizationContext, instance(context, OfflineCashMintAuthorizationContextV1, "mint authorization context"));
}

function mintAuthorizationStatementDigest(statement) {
  return digestModel(DOMAIN.mintAuthorizationStatementDigest, SCHEMAS.mintAuthorizationStatement, instance(statement, OfflineCashMintAuthorizationStatementV1, "mint authorization statement"));
}

function mintAuthorizationDigest(authorization) {
  return digestModel(DOMAIN.mintAuthorizationDigest, SCHEMAS.mintAuthorization, instance(authorization, OfflineCashMintAuthorizationV1, "mint authorization"));
}

function validateMintCredit(credit, authorization) {
  const c = rawValues(instance(credit, OfflineCashMintCreditV1, "mint credit"));
  decodeEncryptedCreditEnvelope(c.encryptedCredit);
  requireEqual(c.statement.lifecycle.ciphertextDigest, ciphertextDigest(c.encryptedCredit), "mint ciphertext digest");
  const digest = digestModel(DOMAIN.mintStatementDigest, SCHEMAS.mintStatement, c.statement);
  requireEqual(c.proof.semanticDigest, digest, "mint proof semantic digest");
  if (authorization !== undefined) validateMintCreditAgainstAuthorization(credit, authorization);
}

function expectedRedemptionId(statement) {
  const value = rawValues(instance(
    statement,
    OfflineCashRedemptionStatementV1,
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
    frame("iroha.offline-cash.v1.redemption-id-preimage", preimage, 16),
  );
}

function validateRedemptionStatement(statement) {
  const value = rawValues(instance(
    statement,
    OfflineCashRedemptionStatementV1,
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
    OfflineCashRedemptionVoucherV1,
    "redemption voucher",
  ));
  validateRedemptionStatement(value.statement);
  validateCommitCertificate(
    value.commitCertificate,
    value.statement.lifecycle,
    value.statement.terminalNullifier,
    value.statement.commitEvidence,
  );
  validateWrapperProof(
    value.proof,
    redemptionStatementDigest(value.statement),
    value.commitCertificate,
  );
}

function validateMintCreditAgainstAuthorization(credit, authorization) {
  const c = rawValues(instance(credit, OfflineCashMintCreditV1, "mint credit"));
  const a = rawValues(instance(authorization, OfflineCashMintAuthorizationV1, "mint authorization"));
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
  const s = rawValues(instance(statement, OfflineCashMintAuthorizationStatementV1, "mint authorization statement"));
  return new OfflineCashEncryptedCreditAadV1({
    version: 1,
    purpose: "mint",
    contextDigest: digestModel(DOMAIN.mintAuthorizationContextDigest, SCHEMAS.mintAuthorizationContext, s.context),
    issuanceOrTransitionCommitment: s.issuanceCommitment,
    creditId: s.creditId,
    amount: s.context.amount,
  });
}

function validateTopUpRequest(request) {
  if (request.mintAuthorization === null) throw new TypeError("canonical Offline Cash V1 top-up requires mint authorization");
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
  const payload = bounded(bytes(raw, "Offline Cash V1 payload"), maximumRaw, "payload");
  if (payload.length === 0) throw new TypeError("Offline Cash V1 payload is empty");
  const text = `oc1:${Buffer.from(payload).toString("base64url")}`;
  if (text.length > maximumText) throw new RangeError("Offline Cash V1 text is oversized");
  return text;
}

function decodeText(kind, text) {
  const [maximumRaw, maximumText] = kindLimits(kind);
  if (typeof text !== "string" || text.length > maximumText || !text.startsWith("oc1:")) throw new TypeError("Offline Cash V1 text prefix or size is invalid");
  const body = text.slice(4);
  if (!/^[A-Za-z0-9_-]+$/u.test(body) || body.length % 4 === 1) throw new TypeError("Offline Cash V1 text is not canonical unpadded base64url");
  const raw = Uint8Array.from(Buffer.from(body, "base64url"));
  bounded(raw, maximumRaw, "payload");
  if (encodeText(kind, raw) !== text) throw new TypeError("Offline Cash V1 text is not canonical");
  return raw;
}

function encodeTypedText(kind, value, ...bindings) {
  const encoders = {
    paymentRequest: encodePaymentRequest,
    acceptanceIntent: encodeAcceptanceIntent,
    acceptanceIntentAuthorization: encodeAcceptanceIntentAuthorization,
    acceptanceTicket: encodeAcceptanceTicket,
    payment: encodePayment,
    acknowledgement: encodeAcknowledgement,
    mintAuthorization: encodeMintAuthorization,
    mintCredit: encodeMintCredit,
    redemptionVoucher: encodeRedemptionVoucher,
    encryptedCreditEnvelope: encodeEncryptedCreditEnvelope,
    encryptedCreditAad: encodeEncryptedCreditAad,
    creditOpening: encodeCreditOpening,
  };
  if (!Object.hasOwn(encoders, kind)) throw new TypeError("unknown Offline Cash V1 payload kind");
  return encodeText(kind, encoders[kind](value, ...bindings));
}

function decodeTypedText(kind, text, ...bindings) {
  const decoders = {
    paymentRequest: decodePaymentRequest,
    acceptanceIntent: decodeAcceptanceIntent,
    acceptanceIntentAuthorization: decodeAcceptanceIntentAuthorization,
    acceptanceTicket: decodeAcceptanceTicket,
    payment: decodePayment,
    acknowledgement: decodeAcknowledgement,
    mintAuthorization: decodeMintAuthorization,
    mintCredit: decodeMintCredit,
    redemptionVoucher: decodeRedemptionVoucher,
    encryptedCreditEnvelope: decodeEncryptedCreditEnvelope,
    encryptedCreditAad: decodeEncryptedCreditAad,
    creditOpening: decodeCreditOpening,
  };
  if (!Object.hasOwn(decoders, kind)) throw new TypeError("unknown Offline Cash V1 payload kind");
  return decoders[kind](decodeText(kind, text), ...bindings);
}

function encodeTopUpRequest(value) {
  const request = instance(value, OfflineCashTopUpRequestV1, "top-up request");
  validateTopUpRequest(request);
  return encodeTopLevel(request, OfflineCashTopUpRequestV1, SCHEMAS.topUpRequest, 4096);
}
const decodeTopUpRequest = (raw) => decodeTopLevel(raw, OfflineCashTopUpRequestV1, SCHEMAS.topUpRequest, 4096, validateTopUpRequest);
const encodeRedemptionRequest = (value) => encodeTopLevel(value, OfflineCashRedemptionRequestV1, SCHEMAS.redemptionRequest, 8192);
const decodeRedemptionRequest = (raw) => decodeTopLevel(raw, OfflineCashRedemptionRequestV1, SCHEMAS.redemptionRequest, 8192);

function validatePreTicketExchange(request, authorization, ticket) {
  const parts = [encodePaymentRequest(request), encodeAcceptanceIntentAuthorization(authorization, request), encodeAcceptanceTicket(ticket, request, authorization.statement.intent)];
  const rawBytes = parts.reduce((sum, value) => sum + value.length, 0);
  const textBytes = parts.reduce((sum, value) => sum + textLength(value.length), 0);
  if (rawBytes > 9984 || textBytes > 13326) throw new RangeError("Offline Cash V1 pre-ticket exchange is oversized");
  return rawBytes;
}

function validateSession(request, payment, acknowledgement) {
  const parts = [encodePaymentRequest(request), encodePayment(payment, request), encodeAcknowledgement(acknowledgement, request, payment)];
  const rawBytes = parts.reduce((sum, value) => sum + value.length, 0);
  const textBytes = parts.reduce((sum, value) => sum + textLength(value.length), 0);
  if (rawBytes > 9211 || textBytes > 12288) throw new RangeError("Offline Cash V1 terminal trio is oversized");
  return rawBytes;
}

function validateCompleteExchange(request, authorization, ticket, payment, acknowledgement) {
  const pre = validatePreTicketExchange(request, authorization, ticket);
  const terminal = validateSession(request, payment, acknowledgement);
  if (!equalBytes(encodeModel(payment.acceptanceIntent), encodeModel(authorization.statement.intent))
      || !equalBytes(encodeModel(payment.acceptanceTicket), encodeModel(ticket))) {
    throw new TypeError("Offline Cash V1 complete exchange contains a substituted intent or ticket");
  }
  const requestBytes = encodePaymentRequest(request).length;
  const rawBytes = pre + terminal - requestBytes;
  const textBytes = textLength(encodeAcceptanceIntentAuthorization(authorization, request).length)
    + textLength(encodeAcceptanceTicket(ticket, request, authorization.statement.intent).length)
    + textLength(requestBytes)
    + textLength(encodePayment(payment, request).length)
    + textLength(encodeAcknowledgement(acknowledgement, request, payment).length);
  if (rawBytes > 18171 || textBytes > 24244) throw new RangeError("Offline Cash V1 complete five-message exchange is oversized");
  return rawBytes;
}

function deviceKeyReference(publicKey) {
  const key = instance(publicKey, OfflineCashDevicePublicKeyV1, "device public key");
  return sha256(join(DOMAIN.deviceKeyReference, Uint8Array.of(0), key.sec1Bytes()));
}

function pastaStateCommitment(value) {
  const state = rawValues(instance(value, OfflineCashPastaStateCommitmentV1, "Pasta state commitment"));
  return sha256(join(DOMAIN.pastaStateCommitment, Uint8Array.of(0), state.eq, state.ep));
}

function liabilityPoolId(networkId, asset, assetIncarnation) {
  const network = normalizeType(T.NETWORK, networkId, "networkId");
  const definition = normalizeType(T.ASSET, asset, "asset");
  const incarnation = normalizeType(T.INCARNATION, assetIncarnation, "assetIncarnation");
  const payload = join(field(networkIdBytes(network)), field(definition.canonicalPayload()), field(encodeType(T.INCARNATION, incarnation)));
  return digestEncoded(DOMAIN.liabilityPool, frame("iroha.offline-cash.v1.liability-pool-preimage", payload, 1));
}

function paymentRequestSigningBytes(value) {
  const request = rawValues(instance(value, OfflineCashPaymentRequestV1, "payment request"));
  const payload = join(
    field(vector(DOMAIN.requestSigning)), field(u16(request.version)), field(fixedArray(request.releaseId)), field(networkIdBytes(request.networkId)),
    field(request.asset.canonicalPayload()), field(encodeType(T.INCARNATION, request.assetIncarnation)), field(u32(request.scale)),
    field(fixedArray(request.liabilityPoolId)), field(request.recipient.canonicalPayload()), field(u128(request.amount)),
    field(fixedArray(request.hardwareCredential.credentialId)), field(fixedArray(request.requestId)), field(u64(request.issuedAtMs)), field(u64(request.expiresAtMs)),
  );
  return frame("iroha.offline-cash.v1.payment-request-signing-preimage", payload, 16);
}

function paymentRequestDigest(value) {
  validateRequest(value);
  return digestModel(DOMAIN.requestDigest, SCHEMAS.request, value);
}

function ciphertextDigest(value) { return digestEncoded(DOMAIN.ciphertextDigest, bytes(value, "encrypted credit")); }

function creditId(transitionNullifier, requestDigest, ticketDigest, recipientOneTimeKey, amount, ciphertextCommitment) {
  const payload = join(
    field(fixedArray(fixed32(transitionNullifier, "transitionNullifier"))),
    field(fixedArray(fixed32(requestDigest, "requestDigest"))),
    field(fixedArray(fixed32(ticketDigest, "acceptanceTicketDigest"))),
    field(fixedArray(fixed32(recipientOneTimeKey, "recipientOneTimeKey"))),
    field(u128(unsigned(amount, MAX_U128, "amount"))),
    field(fixedArray(fixed32(ciphertextCommitment, "ciphertextCommitment"))),
  );
  return digestEncoded(DOMAIN.creditId, frame("iroha.offline-cash.v1.credit-id-preimage", payload, 16));
}

function validatePairedProofValues(v) {
  if (equalBytes(v.guardEqCredentialAudit, v.guardEpCredentialAudit)) throw new TypeError("Offline Cash V1 proof credential audits are aliased");
  validateProofVectors(v);
}

function validateProofVectors(v) {
  requireVersion(v.version);
  if (equalBytes(v.eqProtocolDigest, v.epProtocolDigest) || equalBytes(v.eqDeferredAudit, v.epDeferredAudit)) throw new TypeError("Offline Cash V1 proof parity bindings are invalid");
  if (v.eqProof.length === 0 || v.epProof.length === 0 || v.eqProof.length > 2495 || v.epProof.length > 2495 || v.eqProof.length + v.epProof.length > 4990) throw new RangeError("Offline Cash V1 current proof bytes are out of bounds");
  if (v.eqHistory.length !== 544 || v.epHistory.length !== 544 || isZero(v.eqHistory) || isZero(v.epHistory) || equalBytes(v.eqHistory, v.epHistory)) throw new TypeError("Offline Cash V1 history accumulators are invalid");
}

function encodeModel(value) {
  const definition = DEFINITIONS[value.constructor.name];
  if (!definition) throw new TypeError("value is not an Offline Cash V1 model");
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
    case T.VECTOR: return vector(value);
    case T.OPERATION_KIND: return encodeUnitEnum(value, OPERATION_KINDS, "operation kind");
    case T.CREDIT_PURPOSE: return encodeUnitEnum(value, CREDIT_PURPOSES, "credit purpose");
    case T.COMMIT_EVIDENCE: return encodeCommitEvidence(value);
    case T.OPTIONAL_MINT_AUTHORIZATION: return value === null ? Uint8Array.of(0) : join(Uint8Array.of(1), field(encodeModel(value)));
    default: return encodeModel(value);
  }
}

function decodeType(type, payload, context) {
  switch (type) {
    case T.U16: return Number(readUnsigned(payload, 2, context));
    case T.U32: return Number(readUnsigned(payload, 4, context));
    case T.U64: return readUnsigned(payload, 8, context);
    case T.U128: return readUnsigned(payload, 16, context);
    case T.FIXED32: return fixed32(payload, context);
    case T.RAW32: return raw32(payload, context);
    case T.FIXED24: return fixedBytes(payload, 24, context, false);
    case T.NETWORK: if (payload.length !== 32) throw new TypeError(`${context} must be 32 bytes`); return NetworkId.fromBytes(payload);
    case T.ASSET: return new OfflineCashAssetDefinitionIdV1(payload);
    case T.INCARNATION: return decodeIncarnation(payload, context);
    case T.ACCOUNT: return new OfflineCashAccountIdV1(payload);
    case T.PUBLIC_KEY: return new OfflineCashDevicePublicKeyV1(payload);
    case T.SIGNATURE: return new OfflineCashDeviceSignatureV1(payload);
    case T.VECTOR: return readVector(payload, context);
    case T.OPERATION_KIND: return decodeUnitEnum(payload, OPERATION_KINDS, context);
    case T.CREDIT_PURPOSE: return decodeUnitEnum(payload, CREDIT_PURPOSES, context);
    case T.COMMIT_EVIDENCE: return decodeCommitEvidence(payload, context);
    case T.OPTIONAL_MINT_AUTHORIZATION: return decodeOptionalMintAuthorization(payload, context);
    default: return decodeModel(DEFINITIONS[type].Model, payload);
  }
}

function encodeCommitEvidence(value) {
  const evidence = instance(value, OfflineCashCommitEvidenceV1, "commit evidence");
  return join(u32(COMMIT_EVIDENCE[evidence.source][0]), field(encodeModel(evidence.evidence)));
}

function decodeCommitEvidence(payload, context) {
  if (payload.length < 5) throw new TypeError(`${context} is truncated`);
  const tag = Number(readUnsigned(payload.subarray(0, 4), 4, `${context}.tag`));
  const entry = Object.entries(COMMIT_EVIDENCE).find(([, [candidate]]) => candidate === tag);
  if (!entry) throw new TypeError(`${context} has an unknown tag`);
  const reader = new Reader(payload.subarray(4), context);
  const evidence = decodeModel(entry[1][1], reader.readField("evidence"));
  reader.eof();
  return new OfflineCashCommitEvidenceV1({ source: entry[0], evidence });
}

function decodeOptionalMintAuthorization(payload, context) {
  if (payload.length === 1 && payload[0] === 0) return null;
  if (payload.length < 3 || payload[0] !== 1) throw new TypeError(`${context} has an invalid option tag`);
  const reader = new Reader(payload.subarray(1), context);
  const value = decodeModel(OfflineCashMintAuthorizationV1, reader.readField("value"));
  reader.eof();
  return value;
}

function decodeIncarnation(payload, context) {
  const reader = new Reader(payload, context);
  const raw = reader.readField("hash");
  reader.eof();
  return new OfflineCashAssetIncarnationV1(raw);
}

function encodeUnitEnum(value, variants, context) {
  if (typeof value !== "string" || !variants.includes(value)) throw new TypeError(`unknown Offline Cash V1 ${context}`);
  return u32(variants.indexOf(value));
}

function decodeUnitEnum(payload, variants, context) {
  const tag = Number(readUnsigned(payload, 4, context));
  if (tag >= variants.length) throw new TypeError(`${context} has an unknown tag`);
  return variants[tag];
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

function digestModel(domain, schema, value, alignment = 16) { return digestEncoded(domain, frame(schema, encodeModel(value), alignment)); }
function digestEncoded(domain, canonical) { return sha256(join(domain, Uint8Array.of(0), u64(canonical.length), canonical)); }
function textLength(rawLength) { return 4 + Math.floor((rawLength * 4 + 2) / 3); }

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
  if (Model === OfflineCashEncryptedCreditEnvelopeV1) return 8;
  if (Model === OfflineCashAcknowledgementV1) return 2;
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
    case T.U16: return Number(unsigned(value, 0xffffn, context));
    case T.U32: return Number(unsigned(value, 0xffff_ffffn, context));
    case T.U64: return unsigned(value, MAX_U64, context);
    case T.U128: return unsigned(value, MAX_U128, context);
    case T.FIXED32: return fixed32(value, context);
    case T.RAW32: return raw32(value, context);
    case T.FIXED24: return fixedBytes(value, 24, context, false);
    case T.NETWORK: if (!(value instanceof NetworkId)) throw new TypeError(`${context} must be a NetworkId`); return value;
    case T.ASSET: return value instanceof OfflineCashAssetDefinitionIdV1 ? value : new OfflineCashAssetDefinitionIdV1(value);
    case T.INCARNATION: return value instanceof OfflineCashAssetIncarnationV1 ? value : new OfflineCashAssetIncarnationV1(value);
    case T.ACCOUNT: return value instanceof OfflineCashAccountIdV1 ? value : new OfflineCashAccountIdV1(value);
    case T.PUBLIC_KEY: return value instanceof OfflineCashDevicePublicKeyV1 ? value : new OfflineCashDevicePublicKeyV1(value);
    case T.SIGNATURE: return value instanceof OfflineCashDeviceSignatureV1 ? value : new OfflineCashDeviceSignatureV1(value);
    case T.VECTOR: return bytes(value, context);
    case T.OPERATION_KIND: if (typeof value !== "string" || !OPERATION_KINDS.includes(value)) throw new TypeError(`${context} is invalid`); return value;
    case T.CREDIT_PURPOSE: if (typeof value !== "string" || !CREDIT_PURPOSES.includes(value)) throw new TypeError(`${context} is invalid`); return value;
    case T.COMMIT_EVIDENCE: return instance(value, OfflineCashCommitEvidenceV1, context);
    case T.OPTIONAL_MINT_AUTHORIZATION: return value === null ? null : instance(value, OfflineCashMintAuthorizationV1, context);
    default: return instance(value, DEFINITIONS[type].Model, context);
  }
}

function unsigned(value, maximum, context) { let normalized; if (typeof value === "bigint") normalized = value; else if (typeof value === "number" && Number.isSafeInteger(value)) normalized = BigInt(value); else throw new TypeError(`${context} must be an unsigned integer`); if (normalized < 0n || normalized > maximum) throw new RangeError(`${context} is out of range`); return normalized; }
function header(value, positiveAmount = false) { requireVersion(value.version); if (value.scale > 28) throw new RangeError("Offline Cash V1 asset scale exceeds 28"); if (positiveAmount && value.amount === 0n) throw new RangeError("Offline Cash V1 amount must be positive"); }
function requireVersion(value) { if (value !== 1) throw new TypeError("Offline Cash V1 wire version must be 1"); }
function requireX25519Key(value, context) { if (value.length !== 32 || isZero(value)) throw new TypeError(`${context} must be a nonzero 32-byte X25519 key`); }
function bytes(value, context) { if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0)); if (ArrayBuffer.isView(value)) return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)); throw new TypeError(`${context} must be binary data`); }
function fixedBytes(value, width, context, nonzero = true) { const raw = bytes(value, context); if (raw.length !== width || (nonzero && isZero(raw))) throw new TypeError(`${context} must be ${nonzero ? "one nonzero " : ""}${width}-byte value`); return raw; }
function fixed32(value, context) { return fixedBytes(value, 32, context, true); }
function raw32(value, context) { return fixedBytes(value, 32, context, false); }
function requireFixedArchive(payload, width, context) { if (payload.length !== width * 2) throw new TypeError(`${context} has an invalid fixed-array length`); for (let index = 0; index < width; index += 1) if (payload[index * 2] !== 1) throw new TypeError(`${context} is not a canonical fixed-byte-array payload`); }
function ascii(value) { return UTF8.encode(value); }
function join(...parts) { const arrays = parts.map((part) => bytes(part, "bytes")); const out = new Uint8Array(arrays.reduce((sum, part) => sum + part.length, 0)); let offset = 0; for (const part of arrays) { out.set(part, offset); offset += part.length; } return out; }
function isZero(value) { return value.every((byte) => byte === 0); }
function equalBytes(left, right) { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
function requireEqual(actual, expected, context) { if (!equalBytes(actual, expected)) throw new TypeError(`${context} does not match`); }
function bounded(value, maximum, context) { if (value.length > maximum) throw new RangeError(`Offline Cash V1 ${context} exceeds ${maximum} bytes`); return Uint8Array.from(value); }
function instance(value, Model, context) { if (!(value instanceof Model)) throw new TypeError(`${context} must be a ${Model.name}`); return value; }
function rawValues(value) { return value._offlineCashValues(); }
function cloneValue(value) { return value instanceof Uint8Array ? Uint8Array.from(value) : value; }
function exactRecord(value, context, fields) { if (value === null || typeof value !== "object" || Array.isArray(value)) throw new TypeError(`${context} must be an object`); const actual = Object.keys(value); const expected = new Set(fields); if (actual.length !== fields.length || actual.some((key) => !expected.has(key))) throw new TypeError(`${context} contains missing or unknown fields`); }
function kindLimits(kind) { if (typeof kind !== "string" || !Object.hasOwn(LIMITS, kind)) throw new TypeError("unknown Offline Cash V1 payload kind"); return LIMITS[kind]; }

/**
 * Portable canonical codecs and orchestration bindings for Offline Cash V1.
 * Monetary proofs, signing, encryption, decryption, and hardware state changes must be supplied
 * by the release-pinned native implementation; this namespace intentionally has no fallback.
 */
export const OfflineCashV1 = Object.freeze({
  wireVersion: 1,
  deviceLifecycleVersion: 1,
  handoffCapability: "cash_handoff_v1",
  textPrefix: "oc1:",
  payloadKinds: PAYLOAD_KINDS,
  maximumRequestRawBytes: 1024,
  maximumRequestTextBytes: 1370,
  maximumPreTicketRawBytes: 9984,
  maximumPreTicketTextBytes: 13326,
  maximumSessionRawBytes: 9211,
  maximumSessionTextBytes: 12288,
  completeExchangeTargetBytes: 16384,
  maximumCompleteExchangeRawBytes: 18171,
  maximumCompleteExchangeTextBytes: 24244,
  maximumPairedProofBytes: 6528,
  maximumCurrentProofsBytes: 4990,
  maximumParityProofBytes: 2495,
  historyAccumulatorBytes: 544,
  maximumEncryptedCreditBytes: 384,
  maximumCreditOpeningBytes: 256,
  maximumNoCommitClosureBytes: 16384,
  paymentOutboxMinimumBytes: 26112,
  redemptionOutboxMinimumBytes: 26112,
  maximumTopUpRequestBytes: 4096,
  maximumRedemptionRequestBytes: 8192,
  maximumOperationStatusBytes: 4 * 1024 * 1024,
  maximumOperationStatusJsonBytes: 16 * 1024 * 1024,
  AssetDefinitionId: OfflineCashAssetDefinitionIdV1,
  AssetIncarnation: OfflineCashAssetIncarnationV1,
  AccountId: OfflineCashAccountIdV1,
  DevicePublicKey: OfflineCashDevicePublicKeyV1,
  DeviceSignature: OfflineCashDeviceSignatureV1,
  HardwareCredential: OfflineCashHardwareCredentialV1,
  PastaStateCommitment: OfflineCashPastaStateCommitmentV1,
  PairedProof: OfflineCashPairedProofV1,
  AcceptanceIntent: OfflineCashAcceptanceIntentV1,
  AcceptanceIntentAuthorizationStatement: OfflineCashAcceptanceIntentAuthorizationStatementV1,
  AcceptanceIntentAuthorization: OfflineCashAcceptanceIntentAuthorizationV1,
  NoCommitClosureStatement: OfflineCashNoCommitClosureStatementV1,
  NoCommitClosure: OfflineCashNoCommitClosureV1,
  AcceptanceTicket: OfflineCashAcceptanceTicketV1,
  CreditOpening: OfflineCashCreditOpeningV1,
  EncryptedCreditAad: OfflineCashEncryptedCreditAadV1,
  EncryptedCreditEnvelope: OfflineCashEncryptedCreditEnvelopeV1,
  TrustedCommitTime: OfflineCashTrustedCommitTimeV1,
  MonotonicCommitLease: OfflineCashMonotonicCommitLeaseV1,
  CommitEvidence: OfflineCashCommitEvidenceV1,
  OutboxReservation: OfflineCashOutboxReservationV1,
  LifecycleBinding: OfflineCashLifecycleBindingV1,
  CommitCertificate: OfflineCashCommitCertificateV1,
  CommitWrapperProof: OfflineCashCommitWrapperProofV1,
  PaymentRequest: OfflineCashPaymentRequestV1,
  TransferStatement: OfflineCashTransferStatementV1,
  Payment: OfflineCashPaymentV1,
  InboxReceipt: OfflineCashInboxReceiptV1,
  Acknowledgement: OfflineCashAcknowledgementV1,
  MintAuthorizationContext: OfflineCashMintAuthorizationContextV1,
  MintAuthorizationStatement: OfflineCashMintAuthorizationStatementV1,
  MintAuthorization: OfflineCashMintAuthorizationV1,
  MintCreditStatement: OfflineCashMintCreditStatementV1,
  MintCredit: OfflineCashMintCreditV1,
  RedemptionStatement: OfflineCashRedemptionStatementV1,
  RedemptionVoucher: OfflineCashRedemptionVoucherV1,
  TopUpRequest: OfflineCashTopUpRequestV1,
  RedemptionRequest: OfflineCashRedemptionRequestV1,
  encodePaymentRequest, decodePaymentRequest,
  encodeAcceptanceIntent, decodeAcceptanceIntent,
  encodeAcceptanceIntentAuthorization, decodeAcceptanceIntentAuthorization,
  encodeAcceptanceTicket, decodeAcceptanceTicket,
  encodeNoCommitClosure, decodeNoCommitClosure,
  encodePayment, decodePayment,
  encodeAcknowledgement, decodeAcknowledgement,
  encodeMintAuthorization, decodeMintAuthorization,
  encodeMintCredit, decodeMintCredit,
  encodeRedemptionVoucher, decodeRedemptionVoucher,
  encodeCreditOpening, decodeCreditOpening,
  encodeEncryptedCreditAad, decodeEncryptedCreditAad,
  encodeEncryptedCreditEnvelope, decodeEncryptedCreditEnvelope,
  encodeTopUpRequest, decodeTopUpRequest,
  encodeRedemptionRequest, decodeRedemptionRequest,
  encodeText, decodeText, encodeTypedText, decodeTypedText,
  validatePreTicketExchange, validateSession, validateCompleteExchange,
  validateMintCreditAgainstAuthorization,
  encryptedCreditAadForMint,
  deviceKeyReference, pastaStateCommitment, liabilityPoolId,
  paymentRequestSigningBytes, paymentRequestDigest, acceptanceIntentDigest,
  acceptanceAuthorizationStatementDigest, acceptanceAuthorizationDigest,
  acceptanceTicketDigest, noCommitClosureStatementDigest, noCommitClosureDigest,
  outboxReservationCommitment,
  paymentDigest, ciphertextDigest, creditId,
  mintAuthorizationContextDigest, mintAuthorizationStatementDigest, mintAuthorizationDigest,
});
