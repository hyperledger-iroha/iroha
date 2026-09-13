import { rejectRange, rejectType } from "./validationThrow.js";
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

const TEXT_AMOUNT_MUST_BE_POSITIVE = "amount must be positive";
const TEXT_IS_INVALID = " is invalid";
const TEXT_VERSION_MISMATCH = "version mismatch";
const TEXT_POLICY_EPOCH_MUST_BE_POSITIVE = "policy epoch must be positive";
const TEXT_CREDIT_OPENING_HAS_A_NONCANONICAL_FIXED_SIZE = "credit opening has a noncanonical fixed size";
const TEXT_MINT_STAGE_DISPOSITION_MUST_BE_0_OR_1 = "mint-stage disposition must be 0 or 1";


const TEXT_IROHA_KAGEMUSHA_V1 = "iroha:kagemusha:v1:";
const TEXT_KAGEMUSHA_V1 = "KAGEMUSHA V1 ";
const TEXT_MINT_AUTHORIZATION = "mint authorization ";
const TEXT_UNKNOWN_KAGEMUSHA_V1 = "unknown KAGEMUSHA V1 ";
const TEXT_IROHA_KAGEMUSHA_V1_2 = "iroha.kagemusha.v1.";
const TEXT_COMMIT_CERTIFICATE = "commit certificate ";
const TEXT_KAGEMUSHA_MINT_AUTHORIZATION_STATEMENT_V1 = "KagemushaMintAuthorizationStatementV1";


// Reuse exact wire names and diagnostic fields throughout this module.
const FIELD_ACKNOWLEDGEMENT = "acknowledgement";
const FIELD_COMMIT_EVIDENCE = "commitEvidence";
const FIELD_NETWORK_ID = "networkId";
const FIELD_HARDWARE_PROFILE_ID = "hardwareProfileId";
const FIELD_POLICY_EPOCH = "policyEpoch";
const FIELD_EQ_PROTOCOL_DIGEST = "eqProtocolDigest";
const FIELD_EP_PROTOCOL_DIGEST = "epProtocolDigest";
const FIELD_SEMANTIC_DIGEST = "semanticDigest";
const FIELD_EQ_DEFERRED_AUDIT = "eqDeferredAudit";
const FIELD_EP_DEFERRED_AUDIT = "epDeferredAudit";
const FIELD_CREDIT_ID = "creditId";
const FIELD_ASSET_INCARNATION = "assetIncarnation";
const FIELD_LIABILITY_POOL_ID = "liabilityPoolId";
const FIELD_RECIPIENT_ENCRYPTION_KEY = "recipientEncryptionKey";
const FIELD_REQUEST_DIGEST = "requestDigest";
const FIELD_SENDER_BEFORE_COMMITMENT = "senderBeforeCommitment";
const FIELD_SENDER_AFTER_COMMITMENT = "senderAfterCommitment";
const FIELD_CANDIDATE_ENVELOPE_DIGEST = "candidateEnvelopeDigest";
const FIELD_TRANSITION_NULLIFIER = "transitionNullifier";
const FIELD_ENCRYPTED_CREDIT = "encryptedCredit";
const FIELD_ARTIFACT_MANIFEST_DIGEST = "artifactManifestDigest";
const FIELD_RECIPIENT_CREDENTIAL_COMMITMENT = "recipientCredentialCommitment";
const FIELD_CREDIT_COMMITMENT = "creditCommitment";
const FIELD_ISSUANCE_COMMITMENT = "issuanceCommitment";
const CONTEXT_PAYMENT_REQUEST = "payment request";
const CONTEXT_COMMIT_CERTIFICATE = "commit certificate";
const CONTEXT_MINT_AUTHORIZATION = "mint authorization";
const CONTEXT_UNKNOWN_KAGEMUSHA_V1_PAYLOAD_KIND = (TEXT_UNKNOWN_KAGEMUSHA_V1 + "payload kind");

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
const TOP_UP_INSTRUCTION_WIRE_ID = (TEXT_IROHA_KAGEMUSHA_V1_2 + "top_up");

const K_SCHEMA_REQUEST = (MODEL + "KagemushaPaymentRequestV1");
const K_SCHEMA_PEER_CREDIT_CONTEXT = (MODEL + "KagemushaPeerCreditContextV1");
const K_SCHEMA_LIFECYCLE = (MODEL + "KagemushaLifecycleBindingV1");
const K_SCHEMA_COMMIT_CERTIFICATE = (MODEL + "KagemushaCommitCertificateV1");
const K_SCHEMA_REDEMPTION_PROOF = (MODEL + "KagemushaRedemptionProofV1");
const K_SCHEMA_PAYMENT_PROOF = (MODEL + "KagemushaPaymentProofV1");
const K_SCHEMA_PAYMENT = (MODEL + "KagemushaPaymentV1");
const K_SCHEMA_ACKNOWLEDGEMENT = (MODEL + "KagemushaAcknowledgementV1");
const K_SCHEMA_MINT_AUTHORIZATION_CONTEXT = (MODEL + "KagemushaMintAuthorizationContextV1");
const K_SCHEMA_MINT_AUTHORIZATION_STATEMENT = (MODEL + TEXT_KAGEMUSHA_MINT_AUTHORIZATION_STATEMENT_V1);
const K_SCHEMA_MINT_AUTHORIZATION = (MODEL + "KagemushaMintAuthorizationV1");
const K_SCHEMA_MINT_STATEMENT = (MODEL + "KagemushaMintCreditStatementV1");
const K_SCHEMA_MINT_CREDIT = (MODEL + "KagemushaMintCreditV1");
const K_SCHEMA_DEVICE_MINT_STAGE_COMMAND = (DEVICE_MODEL + "KagemushaDeviceMintStageCommandV1");
const K_SCHEMA_DEVICE_MINT_STAGE_RESULT = (DEVICE_MODEL + "KagemushaDeviceMintStageResultV1");
const K_SCHEMA_REDEMPTION_STATEMENT = (MODEL + "KagemushaRedemptionStatementV1");
const K_SCHEMA_REDEMPTION_VOUCHER = (MODEL + "KagemushaRedemptionVoucherV1");
const K_SCHEMA_ENCRYPTED_CREDIT_ENVELOPE = (MODEL + "KagemushaEncryptedCreditEnvelopeV1");
const K_SCHEMA_ENCRYPTED_CREDIT_AAD = (MODEL + "KagemushaEncryptedCreditAadV1");
const K_SCHEMA_CREDIT_OPENING = (MODEL + "KagemushaCreditOpeningV1");
const K_SCHEMA_TOP_UP_REQUEST = "iroha.torii.v1.kagemusha.top_up.request";
const K_SCHEMA_REDEMPTION_REQUEST = "iroha.torii.v1.kagemusha.redeem.request";

const K_DOMAIN_DEVICE_KEY_REFERENCE = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "device-key-reference"));
const K_DOMAIN_PASTA_STATE_COMMITMENT = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "pasta-state-commitment"));
const K_DOMAIN_LIABILITY_POOL = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "liability-pool"));
const K_DOMAIN_REQUEST_SIGNING = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "payment-request-signing"));
const K_DOMAIN_REQUEST_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "payment-request"));
const K_DOMAIN_PEER_CREDIT_CONTEXT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "peer-credit-context"));
const K_DOMAIN_PEER_CREDIT_OPENING_COMMITMENT = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "peer-credit-opening-commitment"));
const K_DOMAIN_CREDIT_ID = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "credit-id"));
const K_DOMAIN_LIFECYCLE_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "lifecycle-binding"));
const K_DOMAIN_COMMIT_CERTIFICATE_ID = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "commit-certificate-id"));
const K_DOMAIN_COMMIT_CERTIFICATE_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "commit-certificate"));
const K_DOMAIN_STATEMENT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "send-split-statement"));
const K_DOMAIN_PAYMENT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "payment"));
const K_DOMAIN_PREPARED_TRANSFER = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "prepared-transfer"));
const K_DOMAIN_PAYMENT_BODY_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "payment-body"));
const K_DOMAIN_ASSET_IDENTITY = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "asset-identity"));
const K_DOMAIN_ACCOUNT_IDENTITY = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "account-identity"));
const K_DOMAIN_CIPHERTEXT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "ciphertext"));
const K_DOMAIN_MINT_AUTHORIZATION_CONTEXT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "mint-authorization-context"));
const K_DOMAIN_MINT_AUTHORIZATION_STATEMENT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "mint-authorization-statement"));
const K_DOMAIN_MINT_AUTHORIZATION_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "mint-authorization"));
const K_DOMAIN_MINT_STATEMENT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "mint-statement"));
const K_DOMAIN_MINT_LIFECYCLE_CONTEXT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "mint-lifecycle-context"));
const K_DOMAIN_MINT_CREDIT_ID = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "mint-credit-id"));
const K_DOMAIN_REDEMPTION_STATEMENT_DIGEST = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "redemption-statement"));
const K_DOMAIN_REDEMPTION_ID = /* @__PURE__ */ ascii((TEXT_IROHA_KAGEMUSHA_V1 + "redemption-id"));

const LIMITS = Object.freeze({
  paymentRequest: [928, 1243],
  payment: [7552, 10075],
  acknowledgement: [256, 347],
  mintAuthorization: [7936, 10587],
  mintCredit: [7936, 10587],
  redemptionVoucher: [7936, 10587],
});

const PAYLOAD_KINDS = /* @__PURE__ */ (() => Object.freeze(Object.fromEntries(
  Object.entries(LIMITS).map(([name, [maximumRawBytes, maximumTextBytes]]) => [
    name,
    Object.freeze({ maximumRawBytes, maximumTextBytes }),
  ]),
)))();

const IPM1_PAYLOAD_KINDS = Object.freeze({
  request: Object.freeze({ tag: 1, payloadKind: "paymentRequest" }),
  payment: Object.freeze({ tag: 2, payloadKind: "payment" }),
  acknowledgement: Object.freeze({ tag: 3, payloadKind: FIELD_ACKNOWLEDGEMENT }),
});

const COMPLETE_EXCHANGE_TARGET_RAW_BYTES = 8960;
const COMPLETE_EXCHANGE_MAX_RAW_BYTES = 9211;
const COMPLETE_EXCHANGE_MAX_TEXT_BYTES = 12288;

class KagemushaAssetDefinitionIdV1 {
  #payload;

  constructor(value) {
    const payload = typeof value === "string"
      ? encodeAssetDefinitionIdNoritoValue(value, (TEXT_KAGEMUSHA_V1 + "asset"))
      : bytes(value, (TEXT_KAGEMUSHA_V1 + "asset payload"));
    requireFixedArchive(payload, 16, (TEXT_KAGEMUSHA_V1 + "asset payload"));
    this.#payload = Uint8Array.from(payload);
    Object.freeze(this);
  }

  canonicalPayload() { return Uint8Array.from(this.#payload); }
}

class KagemushaAccountIdV1 {
  #payload;

  constructor(value) {
    const payload = typeof value === "string"
      ? encodeAccountIdNoritoValue(value, (TEXT_KAGEMUSHA_V1 + "account"))
      : bytes(value, (TEXT_KAGEMUSHA_V1 + "account payload"));
    if (payload.length === 0 || payload.length > 512) rejectRange((TEXT_KAGEMUSHA_V1 + "account payload is empty or oversized"));
    const canonical = _canonicalAccountIdNoritoValue(payload, (TEXT_KAGEMUSHA_V1 + "account"));
    if (!equalBytes(payload, canonical)) rejectType((TEXT_KAGEMUSHA_V1 + "account payload is not canonical"));
    this.#payload = Uint8Array.from(payload);
    Object.freeze(this);
  }

  canonicalPayload() { return Uint8Array.from(this.#payload); }
}

class KagemushaAssetIncarnationV1 {
  #hash;

  constructor(value) {
    const raw = bytes(value, (TEXT_KAGEMUSHA_V1 + "asset incarnation"));
    if (raw.length !== 32 || (raw[31] & 1) !== 1) rejectType((TEXT_KAGEMUSHA_V1 + "asset incarnation must be a marked 32-byte Iroha hash"));
    this.#hash = Uint8Array.from(raw);
    Object.freeze(this);
  }

  hashBytes() { return Uint8Array.from(this.#hash); }
}

class KagemushaDevicePublicKeyV1 {
  #bytes;

  constructor(value) {
    const raw = bytes(value, (TEXT_KAGEMUSHA_V1 + "device public key"));
    if (raw.length !== 65 || raw[0] !== 4 || isZero(raw.subarray(1))) rejectType((TEXT_KAGEMUSHA_V1 + "device public key must be nonzero 65-byte uncompressed SEC1"));
    this.#bytes = Uint8Array.from(raw);
    Object.freeze(this);
  }

  sec1Bytes() { return Uint8Array.from(this.#bytes); }
}

class KagemushaDeviceSignatureV1 {
  #bytes;

  constructor(value) {
    const raw = bytes(value, (TEXT_KAGEMUSHA_V1 + "device signature"));
    if (raw.length !== 64 || isZero(raw.subarray(0, 32)) || isZero(raw.subarray(32))) rejectType((TEXT_KAGEMUSHA_V1 + "device signature must be nonzero fixed-width r || s"));
    this.#bytes = Uint8Array.from(raw);
    Object.freeze(this);
  }

  rawBytes() { return Uint8Array.from(this.#bytes); }
}

// Private wire discriminants remain literal strings while their identifiers minify.
const K_TYPE_U8 = "u8";
const K_TYPE_U16 = "u16";
const K_TYPE_U32 = "u32";
const K_TYPE_U64 = "u64";
const K_TYPE_U128 = "u128";
const K_TYPE_FIXED32 = "fixed32";
const K_TYPE_RAW32 = "raw32";
const K_TYPE_FIXED24 = "fixed24";
const K_TYPE_NETWORK = "network";
const K_TYPE_ASSET = "asset";
const K_TYPE_INCARNATION = "incarnation";
const K_TYPE_ACCOUNT = "account";
const K_TYPE_PUBLIC_KEY = "publicKey";
const K_TYPE_SIGNATURE = "signature";
const K_TYPE_VECTOR = "vector";
const K_TYPE_OPERATION_KIND = "operationKind";
const K_TYPE_CREDIT_PURPOSE = "creditPurpose";
const K_TYPE_COMMIT_EVIDENCE = FIELD_COMMIT_EVIDENCE;
const K_TYPE_MINT_FRAME = "mintFrame";

// Private constructor metadata keeps unrelated model graphs independently removable.
const MODEL_DEFINITIONS = new WeakMap();
const MODEL_VALUES = new WeakMap();

function defineModel(name, fields, validate, alignment = 16) {
  const Model = class {
    constructor(input) {
      exactRecord(input, name, fields.map(([fieldName]) => fieldName));
      const normalized = {};
      for (const [fieldName, type] of fields) normalized[fieldName] = normalizeType(type, input[fieldName], `${name}.${fieldName}`);
      validate?.(normalized);
      MODEL_VALUES.set(this, { values: normalized, fields, alignment });
      Object.freeze(this);
    }
  };
  Object.defineProperty(Model, "name", { value: name });
  for (const [fieldName] of fields) {
    Object.defineProperty(Model.prototype, fieldName, {
      enumerable: true,
      get() { return cloneValue(rawValues(this)[fieldName]); },
    });
  }
  MODEL_DEFINITIONS.set(Model, { fields, alignment });
  return Model;
}

const KagemushaHardwareCredentialV1 = /* @__PURE__ */ defineModel(
  "KagemushaHardwareCredentialV1",
  [["version", K_TYPE_U16], ["credentialId", K_TYPE_FIXED32], [FIELD_NETWORK_ID, K_TYPE_NETWORK], [FIELD_HARDWARE_PROFILE_ID, K_TYPE_FIXED32],
    ["suiteId", K_TYPE_FIXED32], ["firmwarePolicyDigest", K_TYPE_FIXED32], [FIELD_POLICY_EPOCH, K_TYPE_U64], ["laneCommitment", K_TYPE_FIXED32],
    ["hardwareEpochId", K_TYPE_FIXED32], ["hardwareEpochGeneration", K_TYPE_U64], ["devicePublicKey", K_TYPE_PUBLIC_KEY],
    ["deviceKeyReference", K_TYPE_FIXED32], ["issuedAtMs", K_TYPE_U64], ["expiresAtMs", K_TYPE_U64], ["governanceSignature", K_TYPE_SIGNATURE]],
  (v) => {
    requireVersion(v.version);
    if (v.policyEpoch === 0n || v.issuedAtMs >= v.expiresAtMs) rejectType((TEXT_KAGEMUSHA_V1 + "hardware credential header" + TEXT_IS_INVALID));
    requireEqual(v.deviceKeyReference, deviceKeyReference(v.devicePublicKey), "hardware credential device key reference");
  },
);

const KagemushaPastaStateCommitmentV1 = /* @__PURE__ */ defineModel(
  "KagemushaPastaStateCommitmentV1", [["eq", K_TYPE_RAW32], ["ep", K_TYPE_RAW32]],
  (v) => { if (isZero(v.eq) !== isZero(v.ep)) rejectType("Pasta state commitment must be fully zero or fully present"); },
);

const KagemushaPairedProofV1 = /* @__PURE__ */ defineModel(
  "KagemushaPairedProofV1",
  [["version", K_TYPE_U16], [FIELD_EQ_PROTOCOL_DIGEST, K_TYPE_FIXED32], [FIELD_EP_PROTOCOL_DIGEST, K_TYPE_FIXED32], [FIELD_SEMANTIC_DIGEST, K_TYPE_FIXED32],
    ["guardEqCredentialAudit", K_TYPE_FIXED32], ["guardEpCredentialAudit", K_TYPE_FIXED32], [FIELD_EQ_DEFERRED_AUDIT, K_TYPE_FIXED32],
    [FIELD_EP_DEFERRED_AUDIT, K_TYPE_FIXED32], ["eqProof", K_TYPE_VECTOR], ["epProof", K_TYPE_VECTOR], ["eqHistory", K_TYPE_VECTOR], ["epHistory", K_TYPE_VECTOR]],
  validatePairedProofValues,
);

const KagemushaCreditOpeningV1 = /* @__PURE__ */ defineModel(
  "KagemushaCreditOpeningV1",
  [["version", K_TYPE_U16], [FIELD_CREDIT_ID, K_TYPE_FIXED32], ["amount", K_TYPE_U128], ["creditCommitmentOpening", K_TYPE_FIXED32],
    ["recipientBindingOpening", K_TYPE_FIXED32], ["recoveryNonce", K_TYPE_FIXED32]],
  (v) => { requireVersion(v.version); if (v.amount === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "credit opening " + TEXT_AMOUNT_MUST_BE_POSITIVE)); },
);
const KagemushaEncryptedCreditAadV1 = /* @__PURE__ */ defineModel(
  "KagemushaEncryptedCreditAadV1",
  [["version", K_TYPE_U16], ["purpose", K_TYPE_CREDIT_PURPOSE], ["contextDigest", K_TYPE_FIXED32], ["issuanceOrTransitionCommitment", K_TYPE_FIXED32],
    [FIELD_CREDIT_ID, K_TYPE_FIXED32], ["amount", K_TYPE_U128]],
  (v) => { requireVersion(v.version); if (v.amount === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "encrypted-credit AAD " + TEXT_AMOUNT_MUST_BE_POSITIVE)); },
);
const KagemushaEncryptedCreditEnvelopeV1 = /* @__PURE__ */ defineModel(
  "KagemushaEncryptedCreditEnvelopeV1",
  [["version", K_TYPE_U16], ["ephemeralX25519PublicKey", K_TYPE_RAW32], ["nonce", K_TYPE_FIXED24], ["ciphertextAndTag", K_TYPE_VECTOR]],
  (v) => {
    requireVersion(v.version);
    requireX25519Key(v.ephemeralX25519PublicKey, "encrypted-credit ephemeral key");
    if (v.ciphertextAndTag.length !== ENCRYPTED_CREDIT_BYTES) rejectType(`${TEXT_KAGEMUSHA_V1}ciphertext and tag must be exactly ${ENCRYPTED_CREDIT_BYTES} bytes`);
  },
  8,
);

const OPERATION_KINDS = Object.freeze(["bootstrap", "mintFold", "sendSplit", "receiveFold", "redeemSplit", "rotate"]);
const CREDIT_PURPOSES = Object.freeze(["mint", "peer"]);
const KagemushaLifecycleBindingV1 = /* @__PURE__ */ defineModel(
  "KagemushaLifecycleBindingV1",
  [["version", K_TYPE_U16], [FIELD_NETWORK_ID, K_TYPE_NETWORK], ["protocolVersion", K_TYPE_U16], ["suiteId", K_TYPE_FIXED32], ["vkDigest", K_TYPE_FIXED32],
    ["releaseId", K_TYPE_FIXED32], ["asset", K_TYPE_ASSET], [FIELD_ASSET_INCARNATION, K_TYPE_INCARNATION], ["scale", K_TYPE_U32],
    [FIELD_LIABILITY_POOL_ID, K_TYPE_FIXED32], [FIELD_HARDWARE_PROFILE_ID, K_TYPE_FIXED32], [FIELD_POLICY_EPOCH, K_TYPE_U64], ["operationKind", K_TYPE_OPERATION_KIND],
    ["requestId", K_TYPE_RAW32], ["receiverLaneCommitment", K_TYPE_RAW32], [FIELD_CREDIT_ID, K_TYPE_RAW32], ["ciphertextDigest", K_TYPE_RAW32]],
  (v) => {
    requireVersion(v.version);
    if (v.protocolVersion !== 1 || v.policyEpoch === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "lifecycle header" + TEXT_IS_INVALID));
    requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "lifecycle liability pool");
    const requestFieldsAreSet = !isZero(v.requestId) && !isZero(v.receiverLaneCommitment);
    const requestFieldsAreZero = isZero(v.requestId) && isZero(v.receiverLaneCommitment);
    const creditFieldsAreSet = !isZero(v.creditId) && !isZero(v.ciphertextDigest);
    const allAreZero = [v.requestId, v.receiverLaneCommitment, v.creditId, v.ciphertextDigest].every(isZero);
    if ((v.operationKind === "sendSplit" && !(requestFieldsAreSet && creditFieldsAreSet))
        || (v.operationKind === "mintFold" && (!requestFieldsAreZero || !creditFieldsAreSet))
        || (!new Set(["sendSplit", "mintFold"]).has(v.operationKind) && !allAreZero)) {
      rejectType((TEXT_KAGEMUSHA_V1 + "lifecycle operation identities are invalid"));
    }
  },
);
const KagemushaPaymentRequestV1 = /* @__PURE__ */ defineModel(
  "KagemushaPaymentRequestV1",
  [["version", K_TYPE_U16], ["releaseId", K_TYPE_FIXED32], [FIELD_NETWORK_ID, K_TYPE_NETWORK], ["asset", K_TYPE_ASSET], [FIELD_ASSET_INCARNATION, K_TYPE_INCARNATION],
    ["scale", K_TYPE_U32], [FIELD_LIABILITY_POOL_ID, K_TYPE_FIXED32], ["recipient", K_TYPE_ACCOUNT], ["amount", K_TYPE_U128],
    [FIELD_RECIPIENT_ENCRYPTION_KEY, K_TYPE_FIXED32],
    ["hardwareCredential", KagemushaHardwareCredentialV1], ["requestId", K_TYPE_FIXED32],
    ["issuedAtMs", K_TYPE_U64], ["expiresAtMs", K_TYPE_U64],
    ["signature", K_TYPE_SIGNATURE]],
  (v) => {
    header(v);
    if (v.amount === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "request " + TEXT_AMOUNT_MUST_BE_POSITIVE));
    requireX25519Key(v.recipientEncryptionKey, "request recipient encryption key");
    if (v.expiresAtMs <= v.issuedAtMs || v.expiresAtMs - v.issuedAtMs > 300000n) rejectRange((TEXT_KAGEMUSHA_V1 + "request validity window" + TEXT_IS_INVALID));
    if (!equalBytes(networkIdBytes(v.networkId), networkIdBytes(v.hardwareCredential.networkId))
        || v.issuedAtMs < v.hardwareCredential.issuedAtMs || v.expiresAtMs > v.hardwareCredential.expiresAtMs) rejectType((TEXT_KAGEMUSHA_V1 + "request credential binding" + TEXT_IS_INVALID));
  },
);
const KagemushaPeerCreditContextV1 = /* @__PURE__ */ defineModel(
  "KagemushaPeerCreditContextV1",
  [["version", K_TYPE_U16], [FIELD_REQUEST_DIGEST, K_TYPE_FIXED32], ["amount", K_TYPE_U128],
    [FIELD_SENDER_BEFORE_COMMITMENT, K_TYPE_FIXED32], [FIELD_SENDER_AFTER_COMMITMENT, K_TYPE_FIXED32],
    ["preparedTransferDigest", K_TYPE_FIXED32], [FIELD_RECIPIENT_ENCRYPTION_KEY, K_TYPE_FIXED32]],
  (v) => {
    requireVersion(v.version);
    if (v.amount === 0n || equalBytes(v.senderBeforeCommitment, v.senderAfterCommitment)) {
      rejectType((TEXT_KAGEMUSHA_V1 + "peer credit context" + TEXT_IS_INVALID));
    }
    requireX25519Key(v.recipientEncryptionKey, "peer credit recipient key");
  },
);
const KagemushaTrustedCommitTimeV1 = /* @__PURE__ */ defineModel(
  "KagemushaTrustedCommitTimeV1", [["timeEvidenceCommitment", K_TYPE_FIXED32]],
);
const KagemushaMonotonicLeaseV1 = /* @__PURE__ */ defineModel(
  "KagemushaMonotonicLeaseV1", [["leaseEvidenceCommitment", K_TYPE_FIXED32]],
);
const KagemushaOutboxReservationV1 = /* @__PURE__ */ defineModel(
  "KagemushaOutboxReservationV1",
  [["reservationId", K_TYPE_FIXED32], ["operationKind", K_TYPE_OPERATION_KIND], ["reservedOutboxBytes", K_TYPE_U32], ["issuedAtMs", K_TYPE_U64], ["expiresAtMs", K_TYPE_U64]],
  (v) => {
    const minimum = v.operationKind === "sendSplit" ? 25728 : v.operationKind === "redeemSplit" ? 26112 : null;
    if (minimum === null || v.reservedOutboxBytes < minimum || v.issuedAtMs >= v.expiresAtMs) rejectType((TEXT_KAGEMUSHA_V1 + "outbox reservation" + TEXT_IS_INVALID));
  },
);
const KagemushaHardwareTerminalBodyV1 = /* @__PURE__ */ defineModel(
  "KagemushaHardwareTerminalBodyV1",
  [["version", K_TYPE_U16], [FIELD_CANDIDATE_ENVELOPE_DIGEST, K_TYPE_FIXED32], ["lifecycleBindingDigest", K_TYPE_FIXED32],
    [FIELD_TRANSITION_NULLIFIER, K_TYPE_FIXED32], ["outboxReservationCommitment", K_TYPE_FIXED32], [FIELD_COMMIT_EVIDENCE, K_TYPE_COMMIT_EVIDENCE],
    [FIELD_HARDWARE_PROFILE_ID, K_TYPE_FIXED32], [FIELD_POLICY_EPOCH, K_TYPE_U64], ["privateSuccessorCommitment", K_TYPE_FIXED32],
    ["privateJournalCommitment", K_TYPE_FIXED32], ["privateRecoveryCommitment", K_TYPE_FIXED32]],
  (v) => { requireVersion(v.version); if (v.policyEpoch === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "terminal body " + TEXT_POLICY_EPOCH_MUST_BE_POSITIVE)); },
);
const KagemushaCommitCertificateV1 = /* @__PURE__ */ defineModel(
  "KagemushaCommitCertificateV1",
  [["version", K_TYPE_U16], ["certificateId", K_TYPE_FIXED32], [FIELD_CANDIDATE_ENVELOPE_DIGEST, K_TYPE_FIXED32],
    ["lifecycleBindingDigest", K_TYPE_FIXED32], [FIELD_TRANSITION_NULLIFIER, K_TYPE_FIXED32], ["outboxReservationCommitment", K_TYPE_FIXED32],
    [FIELD_COMMIT_EVIDENCE, K_TYPE_COMMIT_EVIDENCE], [FIELD_HARDWARE_PROFILE_ID, K_TYPE_FIXED32], [FIELD_POLICY_EPOCH, K_TYPE_U64],
    ["hardwareTerminalCommitment", K_TYPE_FIXED32]],
  (v) => { requireVersion(v.version); if (v.policyEpoch === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "commit certificate " + TEXT_POLICY_EPOCH_MUST_BE_POSITIVE)); },
  8,
);
const KagemushaRedemptionProofV1 = /* @__PURE__ */ defineModel(
  "KagemushaRedemptionProofV1",
  [["version", K_TYPE_U16], [FIELD_EQ_PROTOCOL_DIGEST, K_TYPE_FIXED32], [FIELD_EP_PROTOCOL_DIGEST, K_TYPE_FIXED32], [FIELD_SEMANTIC_DIGEST, K_TYPE_FIXED32],
    [FIELD_CANDIDATE_ENVELOPE_DIGEST, K_TYPE_FIXED32], ["commitCertificateDigest", K_TYPE_FIXED32], [FIELD_EQ_DEFERRED_AUDIT, K_TYPE_FIXED32],
    [FIELD_EP_DEFERRED_AUDIT, K_TYPE_FIXED32], ["eqProof", K_TYPE_VECTOR], ["epProof", K_TYPE_VECTOR], ["eqHistory", K_TYPE_VECTOR], ["epHistory", K_TYPE_VECTOR]],
  validateProofVectors,
  8,
);
const KagemushaPaymentProofV1 = /* @__PURE__ */ defineModel(
  "KagemushaPaymentProofV1",
  [["version", K_TYPE_U16], [FIELD_EQ_PROTOCOL_DIGEST, K_TYPE_FIXED32], [FIELD_EP_PROTOCOL_DIGEST, K_TYPE_FIXED32], [FIELD_SEMANTIC_DIGEST, K_TYPE_FIXED32],
    [FIELD_CANDIDATE_ENVELOPE_DIGEST, K_TYPE_FIXED32], ["commitCertificateDigest", K_TYPE_FIXED32], [FIELD_EQ_DEFERRED_AUDIT, K_TYPE_FIXED32],
    [FIELD_EP_DEFERRED_AUDIT, K_TYPE_FIXED32], ["eqProof", K_TYPE_VECTOR], ["epProof", K_TYPE_VECTOR], ["eqHistory", K_TYPE_VECTOR], ["epHistory", K_TYPE_VECTOR]],
  validateProofVectors,
  8,
);
const KagemushaPaymentOutputV1 = /* @__PURE__ */ defineModel(
  "KagemushaPaymentOutputV1",
  [["version", K_TYPE_U16], [FIELD_REQUEST_DIGEST, K_TYPE_FIXED32], ["amount", K_TYPE_U128],
    [FIELD_SENDER_BEFORE_COMMITMENT, K_TYPE_FIXED32], [FIELD_SENDER_AFTER_COMMITMENT, K_TYPE_FIXED32],
    [FIELD_TRANSITION_NULLIFIER, K_TYPE_FIXED32], [FIELD_CREDIT_ID, K_TYPE_FIXED32], ["ciphertextCommitment", K_TYPE_FIXED32],
    [FIELD_COMMIT_EVIDENCE, K_TYPE_COMMIT_EVIDENCE], ["committedAtMs", K_TYPE_U64]],
  (v) => {
    requireVersion(v.version);
    if (v.amount === 0n || v.committedAtMs === 0n || equalBytes(v.senderBeforeCommitment, v.senderAfterCommitment)) {
      rejectType((TEXT_KAGEMUSHA_V1 + "payment output" + TEXT_IS_INVALID));
    }
  },
);
const KagemushaPaymentV1 = /* @__PURE__ */ defineModel(
  "KagemushaPaymentV1",
  [["version", K_TYPE_U16], ["output", KagemushaPaymentOutputV1], [FIELD_ENCRYPTED_CREDIT, K_TYPE_VECTOR],
    ["commitCertificate", KagemushaCommitCertificateV1], ["proof", KagemushaPaymentProofV1]],
  (v) => { requireVersion(v.version); if (v.output.version !== v.version || v.commitCertificate.version !== v.version || v.proof.version !== v.version) rejectType((TEXT_KAGEMUSHA_V1 + "payment " + TEXT_VERSION_MISMATCH)); },
);
const KagemushaInboxReceiptV1 = /* @__PURE__ */ defineModel(
  "KagemushaInboxReceiptV1", [["version", K_TYPE_U16], [FIELD_CREDIT_ID, K_TYPE_FIXED32], ["receiptCommitment", K_TYPE_FIXED32]],
  (v) => requireVersion(v.version),
);
const KagemushaAcknowledgementV1 = /* @__PURE__ */ defineModel(
  "KagemushaAcknowledgementV1",
  [["version", K_TYPE_U16], [FIELD_REQUEST_DIGEST, K_TYPE_FIXED32], ["paymentDigest", K_TYPE_FIXED32], ["inboxReceipt", KagemushaInboxReceiptV1], ["signature", K_TYPE_SIGNATURE]],
  (v) => { requireVersion(v.version); if (v.inboxReceipt.version !== v.version) rejectType((TEXT_KAGEMUSHA_V1 + "acknowledgement " + TEXT_VERSION_MISMATCH)); },
  2,
);

const KagemushaMintAuthorizationContextV1 = /* @__PURE__ */ defineModel(
  "KagemushaMintAuthorizationContextV1",
  [["version", K_TYPE_U16], ["operationId", K_TYPE_FIXED32], ["releaseId", K_TYPE_FIXED32], ["suiteId", K_TYPE_FIXED32], ["vkDigest", K_TYPE_FIXED32],
    [FIELD_ARTIFACT_MANIFEST_DIGEST, K_TYPE_FIXED32], [FIELD_NETWORK_ID, K_TYPE_NETWORK], ["asset", K_TYPE_ASSET], [FIELD_ASSET_INCARNATION, K_TYPE_INCARNATION],
    ["scale", K_TYPE_U32], [FIELD_LIABILITY_POOL_ID, K_TYPE_FIXED32], ["amount", K_TYPE_U128], ["payer", K_TYPE_ACCOUNT], ["recipient", K_TYPE_ACCOUNT],
    ["hardwareCredentialId", K_TYPE_FIXED32], [FIELD_HARDWARE_PROFILE_ID, K_TYPE_FIXED32], [FIELD_POLICY_EPOCH, K_TYPE_U64],
    [FIELD_RECIPIENT_CREDENTIAL_COMMITMENT, K_TYPE_FIXED32], [FIELD_CREDIT_COMMITMENT, K_TYPE_FIXED32], ["recipientOneTimeKey", K_TYPE_FIXED32]],
  (v) => {
    header(v, true);
    if (v.policyEpoch === 0n) rejectType((TEXT_MINT_AUTHORIZATION + TEXT_POLICY_EPOCH_MUST_BE_POSITIVE));
    requireX25519Key(v.recipientOneTimeKey, "mint recipient key");
    requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), (TEXT_MINT_AUTHORIZATION + "liability pool"));
  },
);
const KagemushaMintAuthorizationStatementV1 = /* @__PURE__ */ defineModel(
  TEXT_KAGEMUSHA_MINT_AUTHORIZATION_STATEMENT_V1,
  [["version", K_TYPE_U16], ["context", KagemushaMintAuthorizationContextV1], [FIELD_ISSUANCE_COMMITMENT, K_TYPE_FIXED32], [FIELD_CREDIT_ID, K_TYPE_FIXED32], ["ciphertextDigest", K_TYPE_FIXED32]],
  (v) => { requireVersion(v.version); if (v.context.version !== v.version) rejectType((TEXT_MINT_AUTHORIZATION + "statement " + TEXT_VERSION_MISMATCH)); },
);
const KagemushaMintAuthorizationV1 = /* @__PURE__ */ defineModel(
  "KagemushaMintAuthorizationV1",
  [["version", K_TYPE_U16], ["statement", KagemushaMintAuthorizationStatementV1], ["proof", KagemushaPairedProofV1]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) rejectType((TEXT_MINT_AUTHORIZATION + TEXT_VERSION_MISMATCH)); },
);
const KagemushaMintCreditStatementV1 = /* @__PURE__ */ defineModel(
  "KagemushaMintCreditStatementV1",
  [["version", K_TYPE_U16], ["lifecycle", KagemushaLifecycleBindingV1], [FIELD_RECIPIENT_CREDENTIAL_COMMITMENT, K_TYPE_FIXED32],
    ["authorizationContextDigest", K_TYPE_FIXED32], ["mintAuthorizationDigest", K_TYPE_FIXED32], ["amount", K_TYPE_U128],
    [FIELD_ISSUANCE_COMMITMENT, K_TYPE_FIXED32], ["recipient", K_TYPE_ACCOUNT], [FIELD_CREDIT_COMMITMENT, K_TYPE_FIXED32], ["mintedAtMs", K_TYPE_U64]],
  (v) => { requireVersion(v.version); if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "mintFold" || v.amount === 0n || v.mintedAtMs === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "mint statement" + TEXT_IS_INVALID)); },
);
const KagemushaMintCreditV1 = /* @__PURE__ */ defineModel(
  "KagemushaMintCreditV1",
  [["version", K_TYPE_U16], ["statement", KagemushaMintCreditStatementV1], ["proof", KagemushaPairedProofV1],
    ["finalityCertificateBinding", K_TYPE_FIXED32], ["finalityAuthorityHead", K_TYPE_FIXED32], ["finalityGenesisRosterId", K_TYPE_FIXED32],
    ["finalityProofBindingDigest", K_TYPE_FIXED32], [FIELD_ENCRYPTED_CREDIT, K_TYPE_VECTOR], [FIELD_ARTIFACT_MANIFEST_DIGEST, K_TYPE_FIXED32]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.proof.version !== v.version) rejectType(("mint credit " + TEXT_VERSION_MISMATCH)); },
);

// Operation 16 exposes only public canonical archives; native hardware keeps reservations,
// openings, journal snapshots, and complete Guard certificates private.
const KagemushaDeviceMintStageCommandV1 = /* @__PURE__ */ defineModel(
  "KagemushaDeviceMintStageCommandV1",
  [["version", K_TYPE_U16], ["canonicalAuthorization", K_TYPE_MINT_FRAME], ["canonicalMintCredit", K_TYPE_MINT_FRAME]],
  (v) => requireVersion(v.version),
  8,
);
const KagemushaDeviceMintStageResultV1 = /* @__PURE__ */ defineModel(
  "KagemushaDeviceMintStageResultV1",
  [["version", K_TYPE_U16], ["disposition", K_TYPE_U8], [FIELD_CREDIT_ID, K_TYPE_FIXED32]],
  (v) => {
    requireVersion(v.version);
    if (v.disposition !== 0 && v.disposition !== 1) rejectType((TEXT_KAGEMUSHA_V1 + TEXT_MINT_STAGE_DISPOSITION_MUST_BE_0_OR_1));
  },
  2,
);

const KagemushaRedemptionStatementV1 = /* @__PURE__ */ defineModel(
  "KagemushaRedemptionStatementV1",
  [["version", K_TYPE_U16], ["lifecycle", KagemushaLifecycleBindingV1], ["amount", K_TYPE_U128], ["beneficiary", K_TYPE_ACCOUNT],
    ["terminalNullifier", K_TYPE_FIXED32], ["redemptionCommitment", K_TYPE_FIXED32], ["redemptionId", K_TYPE_FIXED32],
    [FIELD_COMMIT_EVIDENCE, K_TYPE_COMMIT_EVIDENCE]],
  (v) => { requireVersion(v.version); if (v.lifecycle.version !== v.version || v.lifecycle.operationKind !== "redeemSplit"
    || v.amount === 0n) rejectType((TEXT_KAGEMUSHA_V1 + "redemption statement" + TEXT_IS_INVALID)); },
);
const KagemushaRedemptionVoucherV1 = /* @__PURE__ */ defineModel(
  "KagemushaRedemptionVoucherV1",
  [["version", K_TYPE_U16], ["statement", KagemushaRedemptionStatementV1], ["commitCertificate", KagemushaCommitCertificateV1],
    ["proof", KagemushaRedemptionProofV1], [FIELD_ARTIFACT_MANIFEST_DIGEST, K_TYPE_FIXED32]],
  (v) => { requireVersion(v.version); if (v.statement.version !== v.version || v.commitCertificate.version !== v.version || v.proof.version !== v.version) rejectType(("redemption voucher " + TEXT_VERSION_MISMATCH)); },
);

const KagemushaTopUpRequestV1 = /* @__PURE__ */ defineModel(
  "KagemushaTopUpRequestV1",
  [["version", K_TYPE_U16], ["operationId", K_TYPE_FIXED32], [FIELD_ISSUANCE_COMMITMENT, K_TYPE_FIXED32], [FIELD_CREDIT_ID, K_TYPE_FIXED32],
    ["releaseId", K_TYPE_FIXED32], ["suiteId", K_TYPE_FIXED32], ["vkDigest", K_TYPE_FIXED32], [FIELD_NETWORK_ID, K_TYPE_NETWORK], ["asset", K_TYPE_ASSET],
    [FIELD_ASSET_INCARNATION, K_TYPE_INCARNATION], ["scale", K_TYPE_U32], ["amount", K_TYPE_U128], [FIELD_LIABILITY_POOL_ID, K_TYPE_FIXED32],
    ["payer", K_TYPE_ACCOUNT], ["recipient", K_TYPE_ACCOUNT], ["hardwareCredential", KagemushaHardwareCredentialV1],
    [FIELD_RECIPIENT_CREDENTIAL_COMMITMENT, K_TYPE_FIXED32], [FIELD_CREDIT_COMMITMENT, K_TYPE_FIXED32], ["recipientOneTimeKey", K_TYPE_FIXED32],
    [FIELD_ENCRYPTED_CREDIT, K_TYPE_VECTOR], [FIELD_ARTIFACT_MANIFEST_DIGEST, K_TYPE_FIXED32], ["mintAuthorization", { optional: KagemushaMintAuthorizationV1 }]],
  (v) => { header(v, true); requireX25519Key(v.recipientOneTimeKey, "top-up recipient key"); },
);
const KagemushaRedemptionRequestV1 = /* @__PURE__ */ defineModel(
  "KagemushaRedemptionRequestV1", [["version", K_TYPE_U16], ["operationId", K_TYPE_FIXED32], ["voucher", KagemushaRedemptionVoucherV1]],
  (v) => { requireVersion(v.version); if (v.voucher.version !== v.version) rejectType(("redemption request " + TEXT_VERSION_MISMATCH)); },
);

function encodeTopLevel(value, Model, schema, maximum, validate) {
  const model = instance(value, Model, Model.name);
  validate?.(model);
  return bounded(frame(schema, encodeModel(model), modelAlignment(Model)), maximum, Model.name);
}

function decodeTopLevel(raw, Model, schema, maximum, validate) {
  return decodeExact(raw, maximum, schema, Model, (value) => encodeTopLevel(value, Model, schema, maximum, validate));
}

const encodePaymentRequest = (value) => encodeTopLevel(value, KagemushaPaymentRequestV1, K_SCHEMA_REQUEST, LIMITS.paymentRequest[0], validateRequest);
const decodePaymentRequest = (raw) => decodeTopLevel(raw, KagemushaPaymentRequestV1, K_SCHEMA_REQUEST, LIMITS.paymentRequest[0], validateRequest);
const encodeCommitCertificate = (value, lifecycle, evidence, nullifier) => encodeTopLevel(value, KagemushaCommitCertificateV1, K_SCHEMA_COMMIT_CERTIFICATE, 1024,
  lifecycle === undefined ? undefined : (model) => validateCommitCertificate(model, lifecycle, evidence, nullifier));
const decodeCommitCertificate = (raw, lifecycle, evidence, nullifier) => decodeTopLevel(raw, KagemushaCommitCertificateV1, K_SCHEMA_COMMIT_CERTIFICATE, 1024,
  lifecycle === undefined ? undefined : (model) => validateCommitCertificate(model, lifecycle, evidence, nullifier));
const encodeRedemptionProof = (value) => encodeTopLevel(value, KagemushaRedemptionProofV1, K_SCHEMA_REDEMPTION_PROOF, 6528);
const decodeRedemptionProof = (raw) => decodeTopLevel(raw, KagemushaRedemptionProofV1, K_SCHEMA_REDEMPTION_PROOF, 6528);
const encodePaymentProof = (value) => encodeTopLevel(value, KagemushaPaymentProofV1, K_SCHEMA_PAYMENT_PROOF, 6528);
const decodePaymentProof = (raw) => decodeTopLevel(raw, KagemushaPaymentProofV1, K_SCHEMA_PAYMENT_PROOF, 6528);
const encodePeerCreditContext = (value) => encodeTopLevel(value, KagemushaPeerCreditContextV1, K_SCHEMA_PEER_CREDIT_CONTEXT, 512);
const decodePeerCreditContext = (raw) => decodeTopLevel(raw, KagemushaPeerCreditContextV1, K_SCHEMA_PEER_CREDIT_CONTEXT, 512);
const encodePayment = (value, request) => encodeTopLevel(value, KagemushaPaymentV1, K_SCHEMA_PAYMENT, LIMITS.payment[0], (model) => validatePayment(model, request));
const decodePayment = (raw, request) => decodeTopLevel(raw, KagemushaPaymentV1, K_SCHEMA_PAYMENT, LIMITS.payment[0], (model) => validatePayment(model, request));
const encodeAcknowledgement = (value, request, payment) => encodeTopLevel(value, KagemushaAcknowledgementV1, K_SCHEMA_ACKNOWLEDGEMENT, LIMITS.acknowledgement[0], (model) => validateAcknowledgement(model, request, payment));
const decodeAcknowledgement = (raw, request, payment) => decodeTopLevel(raw, KagemushaAcknowledgementV1, K_SCHEMA_ACKNOWLEDGEMENT, LIMITS.acknowledgement[0], (model) => validateAcknowledgement(model, request, payment));
const encodeMintAuthorization = (value) => encodeTopLevel(value, KagemushaMintAuthorizationV1, K_SCHEMA_MINT_AUTHORIZATION, 7936, validateMintAuthorization);
const decodeMintAuthorization = (raw) => decodeTopLevel(raw, KagemushaMintAuthorizationV1, K_SCHEMA_MINT_AUTHORIZATION, 7936, validateMintAuthorization);
const encodeMintCredit = (value, authorization) => encodeTopLevel(value, KagemushaMintCreditV1, K_SCHEMA_MINT_CREDIT, 7936, (model) => validateMintCredit(model, authorization));
const decodeMintCredit = (raw, authorization) => decodeTopLevel(raw, KagemushaMintCreditV1, K_SCHEMA_MINT_CREDIT, 7936, (model) => validateMintCredit(model, authorization));
/** Encode a public operation-16 body. Shape checks never authorize a hardware transition. */
function encodeDeviceMintStageCommandShape(value, canonicalMintCredit = undefined) {
  const command = canonicalMintCredit === undefined ? value : new KagemushaDeviceMintStageCommandV1({
    version: 1, canonicalAuthorization: value, canonicalMintCredit,
  });
  return encodeTopLevel(command, KagemushaDeviceMintStageCommandV1, K_SCHEMA_DEVICE_MINT_STAGE_COMMAND,
    DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, validateDeviceMintStageCommand);
}
function decodeDeviceMintStageCommandShapeExact(raw) {
  return decodeTopLevel(boundedBytes(raw, DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, "mint-stage command"),
    KagemushaDeviceMintStageCommandV1, K_SCHEMA_DEVICE_MINT_STAGE_COMMAND,
    DEVICE_MINT_STAGE_COMMAND_MAX_BYTES, validateDeviceMintStageCommand);
}
/** This result still requires a qualified native response authenticator before it is trusted. */
function encodeDeviceMintStageResultShape(value, command = undefined) {
  return encodeTopLevel(value, KagemushaDeviceMintStageResultV1, K_SCHEMA_DEVICE_MINT_STAGE_RESULT,
    DEVICE_MINT_STAGE_RESULT_MAX_BYTES, command === undefined ? validateDeviceMintStageResult
      : (result) => validateDeviceMintStageResultAgainstCommand(result, command));
}
function decodeDeviceMintStageResultShapeExact(raw, command = undefined) {
  return decodeTopLevel(boundedBytes(raw, DEVICE_MINT_STAGE_RESULT_MAX_BYTES, "mint-stage result"),
    KagemushaDeviceMintStageResultV1, K_SCHEMA_DEVICE_MINT_STAGE_RESULT, DEVICE_MINT_STAGE_RESULT_MAX_BYTES,
    command === undefined ? validateDeviceMintStageResult : (result) => validateDeviceMintStageResultAgainstCommand(result, command));
}
const encodeRedemptionVoucher = (value) => encodeTopLevel(value, KagemushaRedemptionVoucherV1, K_SCHEMA_REDEMPTION_VOUCHER, 7936, validateRedemptionVoucher);
const decodeRedemptionVoucher = (raw) => decodeTopLevel(raw, KagemushaRedemptionVoucherV1, K_SCHEMA_REDEMPTION_VOUCHER, 7936, validateRedemptionVoucher);
const encodeEncryptedCreditAad = (value) => encodeTopLevel(value, KagemushaEncryptedCreditAadV1, K_SCHEMA_ENCRYPTED_CREDIT_AAD, 256);
const decodeEncryptedCreditAad = (raw) => decodeTopLevel(raw, KagemushaEncryptedCreditAadV1, K_SCHEMA_ENCRYPTED_CREDIT_AAD, 256);
const encodeEncryptedCreditEnvelope = (value, recipientKey) => encodeTopLevel(value, KagemushaEncryptedCreditEnvelopeV1, K_SCHEMA_ENCRYPTED_CREDIT_ENVELOPE, 384, (model) => validateEnvelopeRecipient(model, recipientKey));
const decodeEncryptedCreditEnvelope = (raw, recipientKey) => decodeTopLevel(raw, KagemushaEncryptedCreditEnvelopeV1, K_SCHEMA_ENCRYPTED_CREDIT_ENVELOPE, 384, (model) => validateEnvelopeRecipient(model, recipientKey));

function encodeCreditOpening(value) {
  const raw = encodeTopLevel(value, KagemushaCreditOpeningV1, K_SCHEMA_CREDIT_OPENING, 256);
  if (raw.length !== CREDIT_OPENING_BYTES) rejectType((TEXT_KAGEMUSHA_V1 + TEXT_CREDIT_OPENING_HAS_A_NONCANONICAL_FIXED_SIZE));
  return raw;
}

function decodeCreditOpening(raw, creditIdValue, amount) {
  const opening = decodeTopLevel(raw, KagemushaCreditOpeningV1, K_SCHEMA_CREDIT_OPENING, 256, (value) => {
    if (creditIdValue !== undefined) requireEqual(value.creditId, fixed32(creditIdValue, FIELD_CREDIT_ID), "credit opening credit ID");
    if (amount !== undefined && value.amount !== unsigned(amount, MAX_U128, "amount")) rejectType("credit opening amount does not match");
  });
  if (bytes(raw, "credit opening").length !== CREDIT_OPENING_BYTES) rejectType((TEXT_KAGEMUSHA_V1 + TEXT_CREDIT_OPENING_HAS_A_NONCANONICAL_FIXED_SIZE));
  return opening;
}

function validateRequest(request) {
  const v = rawValues(instance(request, KagemushaPaymentRequestV1, CONTEXT_PAYMENT_REQUEST));
  requireEqual(v.liabilityPoolId, liabilityPoolId(v.networkId, v.asset, v.assetIncarnation), "request liability pool");
}

function lifecycleDigest(lifecycle) {
  return digestModel(
    K_DOMAIN_LIFECYCLE_DIGEST,
    K_SCHEMA_LIFECYCLE,
    instance(lifecycle, KagemushaLifecycleBindingV1, "lifecycle binding"),
    8,
  );
}

function commitEvidenceTranscript(evidence) {
  const selected = normalizeType(K_TYPE_COMMIT_EVIDENCE, evidence, "commit evidence");
  if (selected instanceof KagemushaTrustedCommitTimeV1) return join(u32(0), selected.timeEvidenceCommitment);
  return join(u32(1), selected.leaseEvidenceCommitment);
}

function commitCertificateIdTranscript(certificate) {
  const value = rawValues(instance(certificate, KagemushaCommitCertificateV1, CONTEXT_COMMIT_CERTIFICATE));
  return join(u16(value.version), value.candidateEnvelopeDigest, value.lifecycleBindingDigest, value.transitionNullifier,
    value.outboxReservationCommitment, commitEvidenceTranscript(value.commitEvidence), value.hardwareProfileId,
    u64(value.policyEpoch), value.hardwareTerminalCommitment);
}

function expectedCommitCertificateId(certificate) {
  return digestEncoded(K_DOMAIN_COMMIT_CERTIFICATE_ID, commitCertificateIdTranscript(certificate));
}

function commitCertificateTranscript(certificate) {
  const value = rawValues(instance(certificate, KagemushaCommitCertificateV1, CONTEXT_COMMIT_CERTIFICATE));
  return join(u16(value.version), value.certificateId, value.candidateEnvelopeDigest, value.lifecycleBindingDigest,
    value.transitionNullifier, value.outboxReservationCommitment, commitEvidenceTranscript(value.commitEvidence),
    value.hardwareProfileId, u64(value.policyEpoch), value.hardwareTerminalCommitment);
}

function validateCommitCertificate(certificate, lifecycle, evidence, nullifier) {
  const value = rawValues(instance(certificate, KagemushaCommitCertificateV1, CONTEXT_COMMIT_CERTIFICATE));
  const boundLifecycle = instance(lifecycle, KagemushaLifecycleBindingV1, "lifecycle binding");
  const boundEvidence = normalizeType(K_TYPE_COMMIT_EVIDENCE, evidence, "commit evidence");
  requireEqual(value.lifecycleBindingDigest, lifecycleDigest(boundLifecycle), (TEXT_COMMIT_CERTIFICATE + "lifecycle digest"));
  requireEqual(value.transitionNullifier, fixed32(nullifier, "transition nullifier"), (TEXT_COMMIT_CERTIFICATE + "transition nullifier"));
  requireEqual(encodeCommitEvidence(value.commitEvidence), encodeCommitEvidence(boundEvidence), (TEXT_COMMIT_CERTIFICATE + "evidence"));
  requireEqual(value.hardwareProfileId, boundLifecycle.hardwareProfileId, (TEXT_COMMIT_CERTIFICATE + "hardware profile"));
  if (value.policyEpoch !== boundLifecycle.policyEpoch) rejectType((TEXT_COMMIT_CERTIFICATE + "policy epoch does not match"));
  requireEqual(value.certificateId, expectedCommitCertificateId(certificate), (TEXT_COMMIT_CERTIFICATE + "ID"));
}

function commitCertificateDigest(certificate, lifecycle, evidence, nullifier) {
  if (lifecycle !== undefined) validateCommitCertificate(certificate, lifecycle, evidence, nullifier);
  else requireEqual(certificate.certificateId, expectedCommitCertificateId(certificate), "certificate ID");
  return digestEncoded(K_DOMAIN_COMMIT_CERTIFICATE_DIGEST, commitCertificateTranscript(certificate));
}

function validatePaymentOutput(output, request) {
  const value = rawValues(instance(output, KagemushaPaymentOutputV1, "payment output"));
  const bound = instance(request, KagemushaPaymentRequestV1, CONTEXT_PAYMENT_REQUEST);
  const requestDigest = paymentRequestDigest(bound);
  requireEqual(value.requestDigest, requestDigest, "payment request digest");
  if (value.amount !== bound.amount) rejectType("payment amount does not match request");
  requireEqual(value.creditId, creditId(value.transitionNullifier, requestDigest), "payment credit ID");
  if (value.committedAtMs < bound.issuedAtMs || value.committedAtMs >= bound.expiresAtMs) {
    rejectType("payment commit time is outside the request window");
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
  requireEqual(p.proof.commitCertificateDigest, digestEncoded(K_DOMAIN_COMMIT_CERTIFICATE_DIGEST, commitCertificateTranscript(certificate)), "payment certificate digest");
  requireEqual(p.proof.semanticDigest, paymentBodyDigest(p.output, p.encryptedCredit), "payment body semantic digest");
}
function paymentDigest(payment, request) {
  validatePayment(payment, request);
  return digestModel(K_DOMAIN_PAYMENT_DIGEST, K_SCHEMA_PAYMENT, payment);
}

function paymentOutputTranscript(output) {
  const v = rawValues(instance(output, KagemushaPaymentOutputV1, "payment output"));
  return join(u16(v.version), v.requestDigest, u128(v.amount), v.senderBeforeCommitment,
    v.senderAfterCommitment, v.transitionNullifier, v.creditId, v.ciphertextCommitment,
    commitEvidenceTranscript(v.commitEvidence), u64(v.committedAtMs));
}

function paymentOutputDigest(output, request) {
  if (request !== undefined) validatePaymentOutput(output, request);
  return digestEncoded(K_DOMAIN_STATEMENT_DIGEST, paymentOutputTranscript(output));
}

function paymentBodyDigest(output, encryptedCredit) {
  decodeEncryptedCreditEnvelope(encryptedCredit);
  return digestEncoded(K_DOMAIN_PAYMENT_BODY_DIGEST, join(paymentOutputDigest(output), ciphertextDigest(encryptedCredit)));
}

function validateAcknowledgement(acknowledgement, request, payment) {
  const a = rawValues(instance(acknowledgement, KagemushaAcknowledgementV1, FIELD_ACKNOWLEDGEMENT));
  const p = instance(payment, KagemushaPaymentV1, "payment");
  requireEqual(a.requestDigest, paymentRequestDigest(request), "acknowledgement request digest");
  requireEqual(a.paymentDigest, paymentDigest(p, request), "acknowledgement payment digest");
  requireEqual(a.inboxReceipt.creditId, p.output.creditId, "acknowledgement credit ID");
}

function validateMintAuthorization(authorization) {
  const a = rawValues(instance(authorization, KagemushaMintAuthorizationV1, CONTEXT_MINT_AUTHORIZATION));
  const digest = digestModel(K_DOMAIN_MINT_AUTHORIZATION_STATEMENT_DIGEST, K_SCHEMA_MINT_AUTHORIZATION_STATEMENT, a.statement);
  requireEqual(a.proof.semanticDigest, digest, (TEXT_MINT_AUTHORIZATION + "proof semantic digest"));
}

function mintAuthorizationContextDigest(context) {
  return digestModel(K_DOMAIN_MINT_AUTHORIZATION_CONTEXT_DIGEST, K_SCHEMA_MINT_AUTHORIZATION_CONTEXT, instance(context, KagemushaMintAuthorizationContextV1, (TEXT_MINT_AUTHORIZATION + "context")));
}

function mintAuthorizationStatementDigest(statement) {
  return digestModel(K_DOMAIN_MINT_AUTHORIZATION_STATEMENT_DIGEST, K_SCHEMA_MINT_AUTHORIZATION_STATEMENT, instance(statement, KagemushaMintAuthorizationStatementV1, (TEXT_MINT_AUTHORIZATION + "statement")));
}

function mintAuthorizationDigest(authorization) {
  return digestModel(K_DOMAIN_MINT_AUTHORIZATION_DIGEST, K_SCHEMA_MINT_AUTHORIZATION, instance(authorization, KagemushaMintAuthorizationV1, CONTEXT_MINT_AUTHORIZATION));
}

function mintCreditId(statement) {
  const s = rawValues(instance(statement, KagemushaMintCreditStatementV1, "mint statement"));
  const lifecycle = rawValues(s.lifecycle);
  // The frozen context ends at operationKind: current credit ID, ciphertext, and
  // authorization proof bytes are intentionally absent from the issuance preimage.
  const contextFields = MODEL_DEFINITIONS.get(KagemushaLifecycleBindingV1).fields.slice(0, 13);
  const contextPreimage = join(...contextFields.map(([name, type]) => field(encodeType(type, lifecycle[name]))));
  const contextDigest = digestEncoded(K_DOMAIN_MINT_LIFECYCLE_CONTEXT_DIGEST,
    frame((TEXT_IROHA_KAGEMUSHA_V1_2 + "mint-lifecycle-context-preimage"), contextPreimage, 8));
  const preimage = join(field(contextDigest), field(s.recipientCredentialCommitment),
    field(s.authorizationContextDigest), field(u128(s.amount)), field(s.issuanceCommitment),
    field(s.recipient.canonicalPayload()), field(s.creditCommitment));
  return digestEncoded(K_DOMAIN_MINT_CREDIT_ID,
    frame((TEXT_IROHA_KAGEMUSHA_V1_2 + "mint-credit-id-preimage"), preimage, 16));
}

function mintCreditStatementDigest(statement) {
  const value = instance(statement, KagemushaMintCreditStatementV1, "mint statement");
  requireEqual(value.lifecycle.creditId, mintCreditId(value), "mint credit ID");
  return digestModel(K_DOMAIN_MINT_STATEMENT_DIGEST, K_SCHEMA_MINT_STATEMENT, value);
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
  const authorization = decodeMintAuthorization(boundedBytes(value.canonicalAuthorization, 7936, CONTEXT_MINT_AUTHORIZATION));
  return decodeMintCredit(boundedBytes(value.canonicalMintCredit, 7936, "mint credit"), authorization);
}

function validateDeviceMintStageResult(result) {
  const value = rawValues(instance(result, KagemushaDeviceMintStageResultV1, "mint-stage result"));
  requireVersion(value.version);
  if (value.disposition !== 0 && value.disposition !== 1) rejectType((TEXT_KAGEMUSHA_V1 + TEXT_MINT_STAGE_DISPOSITION_MUST_BE_0_OR_1));
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
    field(encodeType(K_TYPE_FIXED32, lifecycleDigest(value.lifecycle))),
    field(encodeType(K_TYPE_FIXED32, value.terminalNullifier)),
    field(encodeType(K_TYPE_U128, value.amount)),
    field(encodeType(K_TYPE_ACCOUNT, value.beneficiary)),
    field(encodeType(K_TYPE_FIXED32, value.redemptionCommitment)),
  );
  return digestEncoded(
    K_DOMAIN_REDEMPTION_ID,
    frame((TEXT_IROHA_KAGEMUSHA_V1_2 + "redemption-id-preimage"), preimage, 16),
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
    rejectType("redemption statement identities must be distinct");
  }
  requireEqual(value.redemptionId, expectedRedemptionId(statement), "redemption ID");
}

function redemptionStatementDigest(statement) {
  validateRedemptionStatement(statement);
  return digestModel(
    K_DOMAIN_REDEMPTION_STATEMENT_DIGEST,
    K_SCHEMA_REDEMPTION_STATEMENT,
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
  const a = rawValues(instance(authorization, KagemushaMintAuthorizationV1, CONTEXT_MINT_AUTHORIZATION));
  validateMintCredit(credit);
  validateMintAuthorization(authorization);
  const context = rawValues(a.statement.context);
  const statement = rawValues(c.statement);
  requireEqual(statement.authorizationContextDigest, digestModel(K_DOMAIN_MINT_AUTHORIZATION_CONTEXT_DIGEST, K_SCHEMA_MINT_AUTHORIZATION_CONTEXT, a.statement.context), (TEXT_MINT_AUTHORIZATION + "context digest"));
  requireEqual(statement.mintAuthorizationDigest, digestModel(K_DOMAIN_MINT_AUTHORIZATION_DIGEST, K_SCHEMA_MINT_AUTHORIZATION, authorization), (TEXT_MINT_AUTHORIZATION + "digest"));
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
      || !equalBytes(encodeType(K_TYPE_ASSET, statement.lifecycle.asset), encodeType(K_TYPE_ASSET, context.asset))
      || !equalBytes(encodeType(K_TYPE_INCARNATION, statement.lifecycle.assetIncarnation), encodeType(K_TYPE_INCARNATION, context.assetIncarnation))
      || statement.lifecycle.scale !== context.scale
      || !equalBytes(statement.lifecycle.liabilityPoolId, context.liabilityPoolId)
      || !equalBytes(statement.lifecycle.hardwareProfileId, context.hardwareProfileId)
      || statement.lifecycle.policyEpoch !== context.policyEpoch
      || !equalBytes(c.artifactManifestDigest, context.artifactManifestDigest)) {
    rejectType((TEXT_MINT_AUTHORIZATION + "context binding" + TEXT_IS_INVALID));
  }
  requireEqual(a.statement.ciphertextDigest, ciphertextDigest(c.encryptedCredit), (TEXT_MINT_AUTHORIZATION + "ciphertext digest"));
  decodeEncryptedCreditEnvelope(c.encryptedCredit, context.recipientOneTimeKey);
  return true;
}

function encryptedCreditAadForMint(statement) {
  const s = rawValues(instance(statement, KagemushaMintAuthorizationStatementV1, (TEXT_MINT_AUTHORIZATION + "statement")));
  return new KagemushaEncryptedCreditAadV1({
    version: 1,
    purpose: "mint",
    contextDigest: digestModel(K_DOMAIN_MINT_AUTHORIZATION_CONTEXT_DIGEST, K_SCHEMA_MINT_AUTHORIZATION_CONTEXT, s.context),
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
    contextDigest: digestModel(K_DOMAIN_PEER_CREDIT_CONTEXT_DIGEST, K_SCHEMA_PEER_CREDIT_CONTEXT, context),
    issuanceOrTransitionCommitment: output.ciphertextCommitment,
    creditId: output.creditId, amount: output.amount,
  });
}
function validateTopUpRequest(request) {
  if (request.mintAuthorization === null) rejectType("canonical KAGEMUSHA V1 top-up requires mint authorization");
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
      || !equalBytes(encodeType(K_TYPE_ASSET, request.asset), encodeType(K_TYPE_ASSET, context.asset))
      || !equalBytes(encodeType(K_TYPE_INCARNATION, request.assetIncarnation), encodeType(K_TYPE_INCARNATION, context.assetIncarnation))
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
    rejectType(("top-up mint authorization context binding" + TEXT_IS_INVALID));
  }
}

function validateEnvelopeRecipient(_envelope, recipientKey) {
  if (recipientKey !== undefined) requireX25519Key(raw32(recipientKey, "recipient X25519 key"), "recipient X25519 key");
}

function encodeText(kind, raw) {
  const [maximumRaw, maximumText] = kindLimits(kind);
  const payload = bounded(bytes(raw, (TEXT_KAGEMUSHA_V1 + "payload")), maximumRaw, "payload");
  if (payload.length === 0) rejectType((TEXT_KAGEMUSHA_V1 + "payload is empty"));
  const text = `kgm1:${Buffer.from(payload).toString("base64url")}`;
  if (text.length > maximumText) rejectRange((TEXT_KAGEMUSHA_V1 + "text is oversized"));
  return text;
}

function decodeText(kind, text) {
  const [maximumRaw, maximumText] = kindLimits(kind);
  if (typeof text !== "string" || text.length > maximumText || !text.startsWith("kgm1:")) rejectType((TEXT_KAGEMUSHA_V1 + "text prefix or size" + TEXT_IS_INVALID));
  const body = text.slice("kgm1:".length);
  if (!/^[A-Za-z0-9_-]+$/u.test(body) || body.length % 4 === 1) rejectType((TEXT_KAGEMUSHA_V1 + "text is not canonical unpadded base64url"));
  const raw = Uint8Array.from(Buffer.from(body, "base64url"));
  bounded(raw, maximumRaw, "payload");
  if (encodeText(kind, raw) !== text) rejectType((TEXT_KAGEMUSHA_V1 + "text is not canonical"));
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
  if (!Object.hasOwn(encoders, kind)) rejectType(CONTEXT_UNKNOWN_KAGEMUSHA_V1_PAYLOAD_KIND);
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
  if (!Object.hasOwn(decoders, kind)) rejectType(CONTEXT_UNKNOWN_KAGEMUSHA_V1_PAYLOAD_KIND);
  return decoders[kind](decodeText(kind, text), ...bindings);
}

function encodeTopUpRequest(value) {
  const request = instance(value, KagemushaTopUpRequestV1, "top-up request");
  validateTopUpRequest(request);
  return encodeTopLevel(
    request,
    KagemushaTopUpRequestV1,
    K_SCHEMA_TOP_UP_REQUEST,
    TOP_UP_REQUEST_MAX_BYTES,
  );
}
const decodeTopUpRequest = (raw) => decodeTopLevel(
  raw,
  KagemushaTopUpRequestV1,
  K_SCHEMA_TOP_UP_REQUEST,
  TOP_UP_REQUEST_MAX_BYTES,
  validateTopUpRequest,
);
const encodeRedemptionRequest = (value) => encodeTopLevel(value, KagemushaRedemptionRequestV1, K_SCHEMA_REDEMPTION_REQUEST, 8192);
const decodeRedemptionRequest = (raw) => decodeTopLevel(raw, KagemushaRedemptionRequestV1, K_SCHEMA_REDEMPTION_REQUEST, 8192);

function buildTopUpInstruction(value) {
  return Object.freeze({
    TopUpKagemushaV1: Object.freeze({
      request: Uint8Array.from(encodeTopUpRequest(value)),
    }),
  });
}

function encodeTopUpInstruction(value, networkPrefix) {
  return Uint8Array.from(noritoEncodeInstruction(buildTopUpInstruction(value), networkPrefix));
}

function decodeTopUpInstruction(raw, networkPrefix) {
  const decoded = noritoDecodeInstruction(raw, networkPrefix);
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
    rejectRange((TEXT_KAGEMUSHA_V1 + "complete three-message exchange is oversized"));
  }
  return rawBytes;
}

function deviceKeyReference(publicKey) {
  const key = instance(publicKey, KagemushaDevicePublicKeyV1, "device public key");
  return sha256(join(K_DOMAIN_DEVICE_KEY_REFERENCE, Uint8Array.of(0), key.sec1Bytes()));
}

function pastaStateCommitment(value) {
  const state = rawValues(instance(value, KagemushaPastaStateCommitmentV1, "Pasta state commitment"));
  return sha256(join(K_DOMAIN_PASTA_STATE_COMMITMENT, Uint8Array.of(0), state.eq, state.ep));
}

function liabilityPoolId(networkId, asset, assetIncarnation) {
  const network = normalizeType(K_TYPE_NETWORK, networkId, FIELD_NETWORK_ID);
  const definition = normalizeType(K_TYPE_ASSET, asset, "asset");
  const incarnation = normalizeType(K_TYPE_INCARNATION, assetIncarnation, FIELD_ASSET_INCARNATION);
  const payload = join(field(networkIdBytes(network)), field(definition.canonicalPayload()), field(encodeType(K_TYPE_INCARNATION, incarnation)));
  return digestEncoded(K_DOMAIN_LIABILITY_POOL, frame((TEXT_IROHA_KAGEMUSHA_V1_2 + "liability-pool-preimage"), payload, 1));
}

function assetIdentityDigest(asset) {
  const value = normalizeType(K_TYPE_ASSET, asset, "asset");
  return digestEncoded(K_DOMAIN_ASSET_IDENTITY, frame("iroha_data_model::asset::id::model::AssetDefinitionId", value.canonicalPayload(), 1));
}

function accountIdentityDigest(account) {
  const value = normalizeType(K_TYPE_ACCOUNT, account, "account");
  return digestEncoded(K_DOMAIN_ACCOUNT_IDENTITY, frame("iroha_data_model::account::model::AccountId", value.canonicalPayload(), 8));
}

function paymentRequestUnsignedTranscript(value) {
  const r = rawValues(instance(value, KagemushaPaymentRequestV1, CONTEXT_PAYMENT_REQUEST));
  return join(u16(r.version), r.releaseId, networkIdBytes(r.networkId), assetIdentityDigest(r.asset),
    r.assetIncarnation.hashBytes(), u32(r.scale), r.liabilityPoolId, accountIdentityDigest(r.recipient),
    u128(r.amount), r.recipientEncryptionKey, r.hardwareCredential.credentialId,
    r.requestId, u64(r.issuedAtMs), u64(r.expiresAtMs));
}

function paymentRequestTranscript(value) {
  return join(paymentRequestUnsignedTranscript(value), value.signature.rawBytes());
}

function paymentRequestSigningBytes(value) {
  return join(K_DOMAIN_REQUEST_SIGNING, Uint8Array.of(0), paymentRequestUnsignedTranscript(value));
}
function acknowledgementSigningBytes(value) {
  const acknowledgement = rawValues(instance(value, KagemushaAcknowledgementV1, FIELD_ACKNOWLEDGEMENT));
  const payload = join(
    field(vector(ascii((TEXT_IROHA_KAGEMUSHA_V1 + "acknowledgement-signing")))),
    field(u16(acknowledgement.version)), field(fixedArray(acknowledgement.requestDigest)),
    field(fixedArray(acknowledgement.paymentDigest)), field(encodeModel(acknowledgement.inboxReceipt)),
  );
  return frame((TEXT_IROHA_KAGEMUSHA_V1_2 + "acknowledgement-signing-preimage"), payload, 8);
}

function paymentRequestDigest(value) {
  validateRequest(value);
  return digestEncoded(K_DOMAIN_REQUEST_DIGEST, paymentRequestTranscript(value));
}
function ciphertextDigest(value) { return digestEncoded(K_DOMAIN_CIPHERTEXT_DIGEST, bytes(value, "encrypted credit")); }

function creditId(transitionNullifier, requestDigestValue) {
  return sha256(join(K_DOMAIN_CREDIT_ID, Uint8Array.of(0),
    fixed32(transitionNullifier, FIELD_TRANSITION_NULLIFIER), fixed32(requestDigestValue, FIELD_REQUEST_DIGEST)));
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
  if (exactAmount === 0n) rejectType(TEXT_AMOUNT_MUST_BE_POSITIVE);
  return sha256(join(
    K_DOMAIN_PEER_CREDIT_OPENING_COMMITMENT,
    Uint8Array.of(0),
    u16(1),
    fixed32(requestDigest, FIELD_REQUEST_DIGEST),
    fixed32(recipientEncryptionKey, FIELD_RECIPIENT_ENCRYPTION_KEY),
    u128(exactAmount),
    fixed32(creditCommitmentOpening, "creditCommitmentOpening"),
    fixed32(recipientBindingOpening, "recipientBindingOpening"),
    fixed32(recoveryNonce, "recoveryNonce"),
  ));
}

function preparedTransferDigest(request, senderBeforeCommitment, senderAfterCommitment,
  transitionNullifier, ciphertextCommitment) {
  const bound = instance(request, KagemushaPaymentRequestV1, CONTEXT_PAYMENT_REQUEST);
  validateRequest(bound);
  const before = fixed32(senderBeforeCommitment, FIELD_SENDER_BEFORE_COMMITMENT);
  const after = fixed32(senderAfterCommitment, FIELD_SENDER_AFTER_COMMITMENT);
  if (equalBytes(before, after)) rejectType("sender state commitments must differ");
  return digestEncoded(K_DOMAIN_PREPARED_TRANSFER, join(u16(1), paymentRequestDigest(request),
    u128(bound.amount), before, after, fixed32(transitionNullifier, FIELD_TRANSITION_NULLIFIER),
    bound.recipientEncryptionKey, fixed32(ciphertextCommitment, "ciphertextCommitment")));
}
function validatePairedProofValues(v) {
  if (equalBytes(v.guardEqCredentialAudit, v.guardEpCredentialAudit)) rejectType((TEXT_KAGEMUSHA_V1 + "proof credential audits are aliased"));
  validateProofVectors(v);
}

function validateProofVectors(v) {
  requireVersion(v.version);
  if (equalBytes(v.eqProtocolDigest, v.epProtocolDigest) || equalBytes(v.eqDeferredAudit, v.epDeferredAudit)) rejectType((TEXT_KAGEMUSHA_V1 + "proof parity bindings are invalid"));
  if (v.eqProof.length === 0 || v.epProof.length === 0 || v.eqProof.length > 2495 || v.epProof.length > 2495 || v.eqProof.length + v.epProof.length > 4990) rejectRange((TEXT_KAGEMUSHA_V1 + "current proof bytes are out of bounds"));
  if (v.eqHistory.length !== 544 || v.epHistory.length !== 544 || isZero(v.eqHistory) || isZero(v.epHistory) || equalBytes(v.eqHistory, v.epHistory)) rejectType((TEXT_KAGEMUSHA_V1 + "history accumulators are invalid"));
}

function encodeModel(value) {
  const state = MODEL_VALUES.get(value);
  if (!state) rejectType("value is not a KAGEMUSHA V1 model");
  return join(...state.fields.map(([name, type]) => field(encodeType(type, state.values[name]))));
}

function decodeModel(Model, payload) {
  const definition = MODEL_DEFINITIONS.get(Model);
  const reader = new Reader(payload, Model.name);
  const value = {};
  for (const [name, type] of definition.fields) value[name] = decodeType(type, reader.readField(name), `${Model.name}.${name}`);
  reader.eof();
  return new Model(value);
}

function encodeType(type, value) {
  switch (type) {
    case K_TYPE_U8: return Uint8Array.of(value);
    case K_TYPE_U16: return u16(value);
    case K_TYPE_U32: return u32(value);
    case K_TYPE_U64: return u64(value);
    case K_TYPE_U128: return u128(value);
    case K_TYPE_FIXED32: case K_TYPE_RAW32: return fixedArray(value);
    case K_TYPE_FIXED24: return bytes(value, "fixed24");
    case K_TYPE_NETWORK: return networkIdBytes(value);
    case K_TYPE_ASSET: case K_TYPE_ACCOUNT: return value.canonicalPayload();
    case K_TYPE_INCARNATION: return field(value.hashBytes());
    case K_TYPE_PUBLIC_KEY: return value.sec1Bytes();
    case K_TYPE_SIGNATURE: return value.rawBytes();
    case K_TYPE_VECTOR: case K_TYPE_MINT_FRAME: return vector(value);
    case K_TYPE_OPERATION_KIND: return encodeUnitEnum(value, OPERATION_KINDS, "operation kind");
    case K_TYPE_CREDIT_PURPOSE: return encodeUnitEnum(value, CREDIT_PURPOSES, "credit purpose");
    case K_TYPE_COMMIT_EVIDENCE: return encodeCommitEvidence(value);
    default: return type.optional
      ? value === null ? Uint8Array.of(0) : join(Uint8Array.of(1), field(encodeModel(value)))
      : encodeModel(value);
  }
}

function decodeType(type, payload, context) {
  switch (type) {
    case K_TYPE_U8: return Number(readUnsigned(payload, 1, context));
    case K_TYPE_U16: return Number(readUnsigned(payload, 2, context));
    case K_TYPE_U32: return Number(readUnsigned(payload, 4, context));
    case K_TYPE_U64: return readUnsigned(payload, 8, context);
    case K_TYPE_U128: return readUnsigned(payload, 16, context);
    case K_TYPE_FIXED32: return fixed32(payload, context);
    case K_TYPE_RAW32: return raw32(payload, context);
    case K_TYPE_FIXED24: return fixedBytes(payload, 24, context, false);
    case K_TYPE_NETWORK: if (payload.length !== 32) rejectType(`${context} must be 32 bytes`); return NetworkId.fromBytes(payload);
    case K_TYPE_ASSET: return new KagemushaAssetDefinitionIdV1(payload);
    case K_TYPE_INCARNATION: return decodeIncarnation(payload, context);
    case K_TYPE_ACCOUNT: return new KagemushaAccountIdV1(payload);
    case K_TYPE_PUBLIC_KEY: return new KagemushaDevicePublicKeyV1(payload);
    case K_TYPE_SIGNATURE: return new KagemushaDeviceSignatureV1(payload);
    case K_TYPE_VECTOR: case K_TYPE_MINT_FRAME: return readVector(payload, context);
    case K_TYPE_OPERATION_KIND: return decodeUnitEnum(payload, OPERATION_KINDS, context);
    case K_TYPE_CREDIT_PURPOSE: return decodeUnitEnum(payload, CREDIT_PURPOSES, context);
    case K_TYPE_COMMIT_EVIDENCE: return decodeCommitEvidence(payload, context);
    default: return type.optional
      ? decodeOptionalModel(type.optional, payload, context)
      : decodeModel(type, payload);
  }
}

function decodeOptionalModel(Model, payload, context) {
  if (payload.length === 1 && payload[0] === 0) return null;
  if (payload.length < 3 || payload[0] !== 1) rejectType(`${context} has an invalid option tag`);
  const reader = new Reader(payload.subarray(1), context);
  const value = decodeModel(Model, reader.readField("value"));
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
  if (typeof value !== "string" || !variants.includes(value)) rejectType(`${TEXT_UNKNOWN_KAGEMUSHA_V1}${context}`);
  return u32(variants.indexOf(value));
}

function decodeUnitEnum(payload, variants, context) {
  const tag = Number(readUnsigned(payload, 4, context));
  if (tag >= variants.length) rejectType(`${context} has an unknown tag`);
  return variants[tag];
}

function encodeCommitEvidence(value) {
  if (value instanceof KagemushaTrustedCommitTimeV1) return join(u32(0), field(encodeModel(value)));
  if (value instanceof KagemushaMonotonicLeaseV1) return join(u32(1), field(encodeModel(value)));
  rejectType((TEXT_UNKNOWN_KAGEMUSHA_V1 + "commit evidence"));
}

function decodeCommitEvidence(payload, context) {
  if (payload.length < 5) rejectType(`${context} is truncated`);
  const tag = Number(readUnsigned(payload.subarray(0, 4), 4, `${context}.tag`));
  const Model = [KagemushaTrustedCommitTimeV1, KagemushaMonotonicLeaseV1][tag];
  if (Model === undefined) rejectType(`${context} has an unknown tag`);
  const reader = new Reader(payload.subarray(4), context);
  const value = decodeModel(Model, reader.readField("evidence"));
  reader.eof();
  return value;
}

function decodeExact(raw, maximum, schema, Model, reencode) {
  const canonical = bounded(bytes(raw, Model.name), maximum, Model.name);
  if (canonical.length === 0) rejectType(`${Model.name} archive is empty`);
  const decoded = validateNoritoFrame(canonical, { context: Model.name, expectedTypeName: schema, expectedPaddingLength: headerPadding(modelAlignment(Model)), requireNonEmptyPayload: true });
  if (decoded.flags !== COMPACT_LENGTHS) rejectType(`${Model.name} must use compact field lengths`);
  const value = decodeModel(Model, decoded.payload);
  requireEqual(canonical, reencode(value), `${Model.name} canonical archive`);
  return value;
}

function digestModel(domain, schema, value, alignment = undefined) {
  const selectedAlignment = alignment ?? MODEL_VALUES.get(value).alignment;
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
function modelAlignment(Model) { return MODEL_DEFINITIONS.get(Model).alignment; }

function field(payload) { const raw = bytes(payload, "field"); return join(compact(raw.length), raw); }
function compact(input) {
  let value = BigInt(input);
  if (value < 0n || value > MAX_U64) rejectRange("compact length is out of range");
  const out = [];
  do { let byte = Number(value & 0x7fn); value >>= 7n; if (value !== 0n) byte |= 0x80; out.push(byte); } while (value !== 0n);
  return Uint8Array.from(out);
}
function fixedArray(value) { return bytes(value, "fixed array"); }
function vector(value) { const raw = bytes(value, "byte vector"); return join(u64(raw.length), raw); }
function readVector(payload, context) {
  if (payload.length < 8) rejectType(`${context} is truncated`);
  const length = readUnsigned(payload.subarray(0, 8), 8, `${context}.length`);
  if (length > BigInt(Number.MAX_SAFE_INTEGER) || Number(length) !== payload.length - 8) rejectType(`${context} length${TEXT_IS_INVALID}`);
  return Uint8Array.from(payload.subarray(8));
}
function u16(value) { const out = new Uint8Array(2); new DataView(out.buffer).setUint16(0, Number(value), true); return out; }
function u32(value) { const out = new Uint8Array(4); new DataView(out.buffer).setUint32(0, Number(value), true); return out; }
function u64(value) { return unsignedLittleEndian(BigInt(value), 8); }
function u128(value) { return unsignedLittleEndian(BigInt(value), 16); }
function unsignedLittleEndian(value, width) { const out = new Uint8Array(width); for (let index = 0; index < width; index += 1) { out[index] = Number(value & 0xffn); value >>= 8n; } if (value !== 0n) rejectRange("unsigned integer is out of range"); return out; }
function readUnsigned(payload, width, context) { if (payload.length !== width) rejectType(`${context} must contain ${width} bytes`); let value = 0n; for (let index = width - 1; index >= 0; index -= 1) value = (value << 8n) | BigInt(payload[index]); return value; }

class Reader {
  constructor(value, context) { this.value = bytes(value, context); this.offset = 0; this.context = context; }
  readField(name) {
    let length = 0n; let shift = 0n; let used = 0;
    for (; used < 10; used += 1) {
      if (this.offset >= this.value.length) rejectType(`${this.context}.${name} is truncated`);
      const byte = this.value[this.offset++];
      if (used === 9 && (byte & 0xfe) !== 0) rejectType(`${this.context}.${name} length exceeds u64`);
      length |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) { if (used > 0 && byte === 0) rejectType(`${this.context}.${name} length is not minimal`); break; }
      shift += 7n;
    }
    if (used === 10 || length > BigInt(this.value.length - this.offset)) rejectType(`${this.context}.${name} length${TEXT_IS_INVALID}`);
    const end = this.offset + Number(length); const payload = this.value.subarray(this.offset, end); this.offset = end; return payload;
  }
  eof() { if (this.offset !== this.value.length) rejectType(`${this.context} contains trailing bytes`); }
}

function normalizeType(type, value, context) {
  switch (type) {
    case K_TYPE_U8: return Number(unsigned(value, 0xffn, context));
    case K_TYPE_U16: return Number(unsigned(value, 0xffffn, context));
    case K_TYPE_U32: return Number(unsigned(value, 0xffff_ffffn, context));
    case K_TYPE_U64: return unsigned(value, MAX_U64, context);
    case K_TYPE_U128: return unsigned(value, MAX_U128, context);
    case K_TYPE_FIXED32: return fixed32(value, context);
    case K_TYPE_RAW32: return raw32(value, context);
    case K_TYPE_FIXED24: return fixedBytes(value, 24, context, false);
    case K_TYPE_NETWORK: if (!(value instanceof NetworkId)) rejectType(`${context} must be a NetworkId`); return value;
    case K_TYPE_ASSET: return value instanceof KagemushaAssetDefinitionIdV1 ? value : new KagemushaAssetDefinitionIdV1(value);
    case K_TYPE_INCARNATION: return value instanceof KagemushaAssetIncarnationV1 ? value : new KagemushaAssetIncarnationV1(value);
    case K_TYPE_ACCOUNT: return value instanceof KagemushaAccountIdV1 ? value : new KagemushaAccountIdV1(value);
    case K_TYPE_PUBLIC_KEY: return value instanceof KagemushaDevicePublicKeyV1 ? value : new KagemushaDevicePublicKeyV1(value);
    case K_TYPE_SIGNATURE: return value instanceof KagemushaDeviceSignatureV1 ? value : new KagemushaDeviceSignatureV1(value);
    case K_TYPE_VECTOR: return bytes(value, context);
    case K_TYPE_MINT_FRAME: return boundedBytes(value, 7936, context);
    case K_TYPE_OPERATION_KIND: if (typeof value !== "string" || !OPERATION_KINDS.includes(value)) rejectType(`${context}${TEXT_IS_INVALID}`); return value;
    case K_TYPE_CREDIT_PURPOSE: if (typeof value !== "string" || !CREDIT_PURPOSES.includes(value)) rejectType(`${context}${TEXT_IS_INVALID}`); return value;
    case K_TYPE_COMMIT_EVIDENCE: encodeCommitEvidence(value); return value;
    default: return type.optional
      ? value === null ? null : instance(value, type.optional, context)
      : instance(value, type, context);
  }
}

function unsigned(value, maximum, context) { let normalized; if (typeof value === "bigint") normalized = value; else if (typeof value === "number" && Number.isSafeInteger(value)) normalized = BigInt(value); else rejectType(`${context} must be an unsigned integer`); if (normalized < 0n || normalized > maximum) rejectRange(`${context} is out of range`); return normalized; }
function header(value, positiveAmount = false) { requireVersion(value.version); if (value.scale > 28) rejectRange((TEXT_KAGEMUSHA_V1 + "asset scale exceeds 28")); if (positiveAmount && value.amount === 0n) rejectRange((TEXT_KAGEMUSHA_V1 + TEXT_AMOUNT_MUST_BE_POSITIVE)); }
function requireVersion(value) { if (value !== 1) rejectType((TEXT_KAGEMUSHA_V1 + "wire version must be 1")); }
function requireX25519Key(value, context) { if (value.length !== 32 || isZero(value)) rejectType(`${context} must be a nonzero 32-byte X25519 key`); }
function bytes(value, context) { if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0)); if (ArrayBuffer.isView(value)) return Uint8Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)); rejectType(`${context} must be binary data`); }
function boundedBytes(value, maximum, context) {
  if (!(value instanceof ArrayBuffer) && !ArrayBuffer.isView(value)) rejectType(`${context} must be binary data`);
  if (value.byteLength > maximum) rejectRange(`${TEXT_KAGEMUSHA_V1}${context} exceeds ${maximum} bytes`);
  return bytes(value, context);
}
function fixedBytes(value, width, context, nonzero = true) { const raw = bytes(value, context); if (raw.length !== width || (nonzero && isZero(raw))) rejectType(`${context} must be ${nonzero ? "one nonzero " : ""}${width}-byte value`); return raw; }
function fixed32(value, context) { return fixedBytes(value, 32, context, true); }
function raw32(value, context) { return fixedBytes(value, 32, context, false); }
function requireFixedArchive(payload, width, context) { if (payload.length !== width * 2) rejectType(`${context} has an invalid fixed-array length`); for (let index = 0; index < width; index += 1) if (payload[index * 2] !== 1) rejectType(`${context} is not a canonical fixed-byte-array payload`); }
function ascii(value) { return UTF8.encode(value); }
function join(...parts) { const arrays = parts.map((part) => bytes(part, "bytes")); const out = new Uint8Array(arrays.reduce((sum, part) => sum + part.length, 0)); let offset = 0; for (const part of arrays) { out.set(part, offset); offset += part.length; } return out; }
function isZero(value) { return value.every((byte) => byte === 0); }
function equalBytes(left, right) { return left.length === right.length && left.every((byte, index) => byte === right[index]); }
function requireEqual(actual, expected, context) { if (!equalBytes(actual, expected)) rejectType(`${context} does not match`); }
function bounded(value, maximum, context) { if (value.length > maximum) rejectRange(`${TEXT_KAGEMUSHA_V1}${context} exceeds ${maximum} bytes`); return Uint8Array.from(value); }
function instance(value, Model, context) { if (!(value instanceof Model)) rejectType(`${context} must be a ${Model.name}`); return value; }
function rawValues(value) { return MODEL_VALUES.get(value).values; }
function cloneValue(value) { return value instanceof Uint8Array ? Uint8Array.from(value) : value; }
function exactRecord(value, context, fields) { if (value === null || typeof value !== "object" || Array.isArray(value)) rejectType(`${context} must be an object`); const actual = Object.keys(value); const expected = new Set(fields); if (actual.length !== fields.length || actual.some((key) => !expected.has(key))) rejectType(`${context} contains missing or unknown fields`); }
function kindLimits(kind) { if (typeof kind !== "string" || !Object.hasOwn(LIMITS, kind)) rejectType(CONTEXT_UNKNOWN_KAGEMUSHA_V1_PAYLOAD_KIND); return LIMITS[kind]; }
function ipm1PayloadTag(kind) {
  if (typeof kind !== "string" || !Object.hasOwn(IPM1_PAYLOAD_KINDS, kind)) rejectType((TEXT_UNKNOWN_KAGEMUSHA_V1 + "IPM1 payload kind"));
  return IPM1_PAYLOAD_KINDS[kind].tag;
}
function ipm1PayloadKindFromTag(tag) {
  if (!Number.isInteger(tag)) rejectType((TEXT_KAGEMUSHA_V1 + "IPM1 payload tag must be an integer"));
  const entry = Object.entries(IPM1_PAYLOAD_KINDS).find(([, value]) => value.tag === tag);
  if (entry === undefined) rejectType((TEXT_UNKNOWN_KAGEMUSHA_V1 + "IPM1 payload tag"));
  return entry[0];
}

/**
 * Portable canonical codecs and orchestration bindings for KAGEMUSHA V1.
 * Monetary proofs, signing, encryption, decryption, and hardware state changes must be supplied
 * by the release-pinned native implementation; this namespace intentionally has no fallback.
 */
export const Kagemusha = /* @__PURE__ */ (() => Object.freeze({
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
}))();

// Internal Torii boundary: importing one operation must not retain every wallet type.
export { encodeRedemptionRequest as _encodeRedemptionRequestV1 };
