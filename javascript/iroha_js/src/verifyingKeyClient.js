import { createHash, timingSafeEqual } from "node:crypto";
import { isDeepStrictEqual } from "node:util";
import { Buffer } from "buffer";

import { decodeCanonicalVerifyingKeyTransactionPayload } from "./transactionCodec.js";
import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";
import {
  isVerifierBackendRegistryLabelV1,
  verifierBackendRegistryTagV1,
} from "./verifierBackendRegistry.js";
import { requestActiveVerifyingKeyIds } from "./activeVerifyingKeyIds.js";

const VERIFYING_KEY_TRANSACTION_PAYLOAD_MAX_BYTES = 16 * 1024 * 1024;
const VERIFYING_KEY_STATUS_VALUES = new Set([
  "Proposed",
  "Active",
  "Withdrawn",
]);
const VERIFYING_KEY_STATUS_ALIASES = new Map(
  [...VERIFYING_KEY_STATUS_VALUES].map((value) => [value.toLowerCase(), value]),
);
/**
 * Create the lazily loaded verifying-key registry validator.
 *
 * Generic Torii validators are injected so their exact behavior remains
 * single-sourced in the client while the registry and transaction-codec graph
 * stays outside consumers that never use verifying-key routes.
 *
 * @returns {Readonly<object>}
 */
export function createVerifyingKeyClient(
  allowedListOptionKeys,
  assertSupportedOptionKeys,
  decodeDraftBase64,
  ensureRecord,
  irohaPrehash,
  normalizeAccountId,
  normalizeRequiredBase64Payload,
  normalizeSignalOption,
  normalizeUnsignedInteger,
  rejectPrivateKeyFields,
  requireBooleanLike,
  requireExactBoolean,
  requireExactNonEmptyString,
  requireHexString,
  requireNonEmptyString,
) {

  function normalizeVerifyingKeyStatusValue(
    value,
    context,
    { optional = false } = {},
  ) {
    if (value === undefined || value === null || value === "") {
      if (optional) return undefined;
      throw new TypeError(`${context} must be a verifying key status`);
    }
    const normalized = requireNonEmptyString(String(value), context).toLowerCase();
    const canonical = VERIFYING_KEY_STATUS_ALIASES.get(normalized);
    if (!canonical) {
      throw new TypeError(
        `${context} must be one of ${[...VERIFYING_KEY_STATUS_VALUES].join(", ")}`,
      );
    }
    return canonical;
  }

  function assertProductionVerifyBackendLabel(value, context) {
    if (typeof value !== "string" || value.trim() === "") {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must be a non-empty string`,
        context,
      );
    }
    const backend = value;
    if (backend.trim() !== backend) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must not contain surrounding whitespace`,
        context,
      );
    }
    if (!isVerifierBackendRegistryLabelV1(backend)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} uses unsupported production verifier backend ${backend}`,
        context,
      );
    }
    return backend;
  }

  function normalizeVerifyingKeyName(value, context) {
    const name = requireExactNonEmptyString(value, context);
    if (name.includes(":")) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must not contain ':'`,
        context,
      );
    }
    return name;
  }

  function validateVerifyingKeyHeightRange(
    activationHeight,
    withdrawHeight,
    context,
  ) {
    if (
      activationHeight !== undefined &&
      activationHeight !== null &&
      withdrawHeight !== undefined &&
      withdrawHeight !== null &&
      withdrawHeight < activationHeight
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.withdraw_height must be >= activation_height`,
        `${context}.withdrawHeight`,
      );
    }
  }

  function buildVerifyingKeyListQuery(options = {}) {
    const { signal } = normalizeSignalOption(options, "listVerifyingKeys");
    assertSupportedOptionKeys(
      options ?? {},
      allowedListOptionKeys,
      "listVerifyingKeys options",
    );
    const params = {};
    const backendValue = options.backend ?? options.backend_filter;
    if (backendValue !== undefined && backendValue !== null) {
      params.backend = assertProductionVerifyBackendLabel(
        backendValue,
        "listVerifyingKeys.backend",
      );
    }
    const statusValue =
      options.status ?? options.statusFilter ?? options.verifyingKeyStatus;
    const normalizedStatus = normalizeVerifyingKeyStatusValue(
      statusValue,
      "listVerifyingKeys.status",
      { optional: true },
    );
    if (normalizedStatus) params.status = normalizedStatus;
    const nameContains = options.nameContains ?? options.name_contains;
    if (nameContains !== undefined && nameContains !== null) {
      params.name_contains = requireNonEmptyString(
        nameContains,
        "listVerifyingKeys.nameContains",
      );
    }
    if (options.limit !== undefined && options.limit !== null) {
      params.limit = normalizeUnsignedInteger(
        options.limit,
        "listVerifyingKeys.limit",
        { allowZero: false },
      );
    }
    if (options.offset !== undefined && options.offset !== null) {
      params.offset = normalizeUnsignedInteger(
        options.offset,
        "listVerifyingKeys.offset",
        { allowZero: true },
      );
    }
    const orderValue = options.order ?? options.sort ?? options.sortOrder;
    if (orderValue !== undefined && orderValue !== null) {
      const normalizedOrder = requireNonEmptyString(
        orderValue,
        "listVerifyingKeys.order",
      );
      const lower = normalizedOrder.toLowerCase();
      if (lower !== "asc" && lower !== "desc") {
        throw new TypeError('listVerifyingKeys.order must be "asc" or "desc"');
      }
      params.order = lower;
    }
    const idsOnlyValue = options.idsOnly ?? options.ids_only;
    if (idsOnlyValue !== undefined && idsOnlyValue !== null) {
      params.ids_only = requireBooleanLike(
        idsOnlyValue,
        "listVerifyingKeys.idsOnly",
      );
    }
    return {
      signal,
      params: Object.keys(params).length === 0 ? undefined : params,
    };
  }

  function normalizeVerifyingKeyListPayload(
    payload,
    context = "verifying key list response",
  ) {
    if (payload === undefined || payload === null) return [];
    if (Array.isArray(payload)) {
      return payload.map((entry, index) =>
        normalizeVerifyingKeyListItem(entry, `${context}[${index}]`),
      );
    }
    const record = ensureRecord(payload, context);
    if (!Array.isArray(record.items)) {
      throw new TypeError(`${context} must be an array or { items: [] } object`);
    }
    return record.items.map((entry, index) =>
      normalizeVerifyingKeyListItem(entry, `${context}.items[${index}]`),
    );
  }

  function normalizeVerifyingKeyListItem(payload, context) {
    const record = ensureRecord(payload, context);
    let idPayload = record.id;
    if (!idPayload && record.backend && record.name) {
      idPayload = { backend: record.backend, name: record.name };
    }
    if (!idPayload) throw new TypeError(`${context} must include an id`);
    const id = normalizeVerifyingKeyId(idPayload, `${context}.id`);
    let normalizedRecord = null;
    if (record.record !== undefined && record.record !== null) {
      normalizedRecord = normalizeVerifyingKeyRecord(
        record.record,
        `${context}.record`,
      );
    }
    return { id, record: normalizedRecord };
  }

  function normalizeVerifyingKeyDetail(
    payload,
    context = "verifying key detail response",
  ) {
    const record = ensureRecord(payload, context);
    const id = normalizeVerifyingKeyId(record.id, `${context}.id`);
    return {
      id,
      record: normalizeVerifyingKeyRecord(record.record, `${context}.record`),
    };
  }

  function normalizeVerifyingKeyId(payload, context) {
    const record = ensureRecord(payload, context);
    return {
      backend: assertProductionVerifyBackendLabel(
        record.backend,
        `${context}.backend`,
      ),
      name: normalizeVerifyingKeyName(record.name, `${context}.name`),
    };
  }

  function normalizeVerifyingKeyRecord(payload, context) {
    const record = ensureRecord(payload, context);
    const gasSchedule = record.gas_schedule_id ?? null;
    const metadataCid = record.metadata_uri_cid ?? null;
    const vkBytesCid = record.vk_bytes_cid ?? null;
    const inlinePayload = record.key ?? null;
    const activationHeight =
      record.activation_height === undefined || record.activation_height === null
        ? null
        : normalizeUnsignedInteger(
            record.activation_height,
            `${context}.activation_height`,
            { allowZero: true },
          );
    const withdrawHeight =
      record.withdraw_height === undefined || record.withdraw_height === null
        ? null
        : normalizeUnsignedInteger(
            record.withdraw_height,
            `${context}.withdraw_height`,
            { allowZero: true },
          );
    validateVerifyingKeyHeightRange(activationHeight, withdrawHeight, context);
    return {
      version: normalizeUnsignedInteger(record.version, `${context}.version`, {
        allowZero: false,
      }),
      circuit_id: requireExactNonEmptyString(
        record.circuit_id,
        `${context}.circuit_id`,
      ),
      backend: assertProductionVerifyBackendLabel(
        record.backend,
        `${context}.backend`,
      ),
      curve:
        record.curve === undefined || record.curve === null
          ? null
          : requireNonEmptyString(record.curve, `${context}.curve`),
      public_inputs_schema_hash: requireNonEmptyString(
        record.public_inputs_schema_hash,
        `${context}.public_inputs_schema_hash`,
      ),
      commitment_hex: requireHexString(
        record.commitment,
        `${context}.commitment_hex`,
      ),
      vk_len: normalizeUnsignedInteger(record.vk_len, `${context}.vk_len`, {
        allowZero: false,
      }),
      max_proof_bytes:
        record.max_proof_bytes === undefined || record.max_proof_bytes === null
          ? null
          : normalizeUnsignedInteger(
              record.max_proof_bytes,
              `${context}.max_proof_bytes`,
              { allowZero: false },
            ),
      gas_schedule_id:
        gasSchedule === null
          ? null
          : requireExactNonEmptyString(
              gasSchedule,
              `${context}.gas_schedule_id`,
            ),
      metadata_uri_cid:
        metadataCid === null
          ? null
          : requireNonEmptyString(
              metadataCid,
              `${context}.metadata_uri_cid`,
            ),
      vk_bytes_cid:
        vkBytesCid === null
          ? null
          : requireNonEmptyString(vkBytesCid, `${context}.vk_bytes_cid`),
      activation_height: activationHeight,
      withdraw_height: withdrawHeight,
      status: normalizeVerifyingKeyStatusValue(
        record.status,
        `${context}.status`,
      ),
      inline_key: normalizeVerifyingKeyInline(
        inlinePayload,
        `${context}.inline_key`,
      ),
    };
  }

  function normalizeVerifyingKeyInline(value, context) {
    if (value === undefined || value === null) return null;
    const record = ensureRecord(value, context);
    return {
      backend: assertProductionVerifyBackendLabel(
        record.backend,
        `${context}.backend`,
      ),
      bytes_b64: normalizeRequiredBase64Payload(
        record.bytes_b64,
        `${context}.bytes_b64`,
      ),
    };
  }

  function requireVerifyingKeySigningContext(value, context) {
    if (value === null) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context} requires immutable ToriiClient options.localSigningContext`,
        "ToriiClient.options.localSigningContext",
      );
    }
    return value;
  }

  function normalizeVerifyingKeyHex32(value, context) {
    const literal = requireExactNonEmptyString(value, context);
    const normalized = /^0x/iu.test(literal) ? literal.slice(2) : literal;
    if (normalized.length !== 64 || !/^[0-9a-fA-F]{64}$/u.test(normalized)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${context} must contain exactly 32 bytes`,
        context,
      );
    }
    return normalized.toLowerCase();
  }

  function normalizeVerifyingKeyRegisterPayload(input) {
    const record = ensureRecord(input, "registerVerifyingKey payload");
    rejectPrivateKeyFields(record, "registerVerifyingKey");
    const payload = {
      authority: normalizeAccountId(
        record.authority,
        "registerVerifyingKey.authority",
      ),
      backend: assertProductionVerifyBackendLabel(
        record.backend,
        "registerVerifyingKey.backend",
      ),
      name: normalizeVerifyingKeyName(
        record.name,
        "registerVerifyingKey.name",
      ),
      version: normalizeUnsignedInteger(
        record.version,
        "registerVerifyingKey.version",
        { allowZero: false },
      ),
      circuit_id: requireExactNonEmptyString(
        record.circuit_id,
        "registerVerifyingKey.circuitId",
      ),
      public_inputs_schema_hash_hex: normalizeVerifyingKeyHex32(
        record.public_inputs_schema_hash_hex,
        "registerVerifyingKey.publicInputsSchemaHashHex",
      ),
      gas_schedule_id: requireExactNonEmptyString(
        record.gas_schedule_id,
        "registerVerifyingKey.gasScheduleId",
      ),
    };
    assignVerifyingKeyOptionalFields(record, payload, "registerVerifyingKey");
    return payload;
  }

  function normalizeVerifyingKeyUpdatePayload(input) {
    const record = ensureRecord(input, "updateVerifyingKey payload");
    rejectPrivateKeyFields(record, "updateVerifyingKey");
    const payload = {
      authority: normalizeAccountId(
        record.authority,
        "updateVerifyingKey.authority",
      ),
      backend: assertProductionVerifyBackendLabel(
        record.backend,
        "updateVerifyingKey.backend",
      ),
      name: normalizeVerifyingKeyName(record.name, "updateVerifyingKey.name"),
      version: normalizeUnsignedInteger(
        record.version,
        "updateVerifyingKey.version",
        { allowZero: false },
      ),
      circuit_id: requireExactNonEmptyString(
        record.circuit_id,
        "updateVerifyingKey.circuitId",
      ),
      public_inputs_schema_hash_hex: normalizeVerifyingKeyHex32(
        record.public_inputs_schema_hash_hex,
        "updateVerifyingKey.publicInputsSchemaHashHex",
      ),
    };
    const gasSchedule = record.gas_schedule_id;
    if (gasSchedule !== undefined && gasSchedule !== null) {
      payload.gas_schedule_id = requireExactNonEmptyString(
        gasSchedule,
        "updateVerifyingKey.gasScheduleId",
      );
    }
    assignVerifyingKeyOptionalFields(record, payload, "updateVerifyingKey");
    return payload;
  }

  function normalizeVerifyingKeyTransactionDraft(
    input,
    context,
    { networkId, operation, request },
  ) {
    const record = ensureRecord(input, context);
    assertSupportedOptionKeys(
      record,
      new Set([
        "submitted",
        "transaction_payload_b64",
        "signing_message_b64",
      ]),
      context,
    );
    if (requireExactBoolean(record.submitted, `${context}.submitted`) !== false) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.submitted must be false`,
        `${context}.submitted`,
      );
    }
    const transactionPayloadB64 = record.transaction_payload_b64;
    const transactionPayload = decodeDraftBase64(
      transactionPayloadB64,
      `${context}.transaction_payload_b64`,
      {
        maxBytes: VERIFYING_KEY_TRANSACTION_PAYLOAD_MAX_BYTES,
        limitLabel: "transaction payload",
      },
    );
    const signingMessageB64 = record.signing_message_b64;
    const signingMessage = decodeDraftBase64(
      signingMessageB64,
      `${context}.signing_message_b64`,
      { exactBytes: 32 },
    );
    const expectedSigningMessage = irohaPrehash(transactionPayload);
    if (!timingSafeEqual(signingMessage, expectedSigningMessage)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.signing_message_b64 must equal the canonical Iroha HashOf(transaction_payload_b64)`,
        `${context}.signing_message_b64`,
      );
    }
    let decodedInstruction;
    try {
      decodedInstruction = decodeCanonicalVerifyingKeyTransactionPayload(
        transactionPayload,
        {
          expectedNetworkId: networkId,
          expectedAuthority: request.authority,
          operation,
        },
      );
    } catch (error) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.transaction_payload_b64 is not the requested canonical verifying-key transaction: ${error.message}`,
        `${context}.transaction_payload_b64`,
      );
    }
    const expectedRecord = expectedVerifyingKeyRecord(request);
    if (
      decodedInstruction.id?.backend !== request.backend ||
      decodedInstruction.id?.name !== request.name ||
      !isDeepStrictEqual(decodedInstruction.record, expectedRecord)
    ) {
      throw createValidationError(
        ValidationErrorCode.INVALID_OBJECT,
        `${context}.transaction_payload_b64 does not contain the exact requested verifying-key registry record`,
        `${context}.transaction_payload_b64`,
      );
    }
    return {
      submitted: false,
      transaction_payload_b64: transactionPayloadB64,
      signing_message_b64: signingMessageB64,
    };
  }

  function expectedVerifyingKeyRecord(request) {
    const keyBytes =
      request.vk_bytes === undefined
        ? null
        : Buffer.from(request.vk_bytes, "base64");
    const commitmentHex =
      keyBytes === null
        ? request.commitment_hex
        : computeVerifyingKeyCommitmentHex(request.backend, request.vk_bytes);
    return {
      version: request.version,
      circuit_id: request.circuit_id,
      owner_manifest_id: null,
      namespace: "core",
      backend: verifierBackendRegistryTagV1(request.backend),
      curve: request.curve ?? "unknown",
      public_inputs_schema_hash: Array.from(
        Buffer.from(request.public_inputs_schema_hash_hex, "hex"),
      ),
      commitment: Array.from(Buffer.from(commitmentHex, "hex")),
      vk_len: keyBytes === null ? request.vk_len : keyBytes.length,
      max_proof_bytes: request.max_proof_bytes ?? 0,
      gas_schedule_id: request.gas_schedule_id ?? null,
      metadata_uri_cid: request.metadata_uri_cid ?? null,
      vk_bytes_cid: request.vk_bytes_cid ?? null,
      activation_height: request.activation_height ?? null,
      withdraw_height: request.withdraw_height ?? null,
      key:
        keyBytes === null
          ? null
          : {
              backend: request.backend,
              bytes: Array.from(keyBytes),
            },
      status: request.status ?? "Active",
    };
  }

  function assignVerifyingKeyOptionalFields(record, payload, context) {
    if (record.curve !== undefined && record.curve !== null) {
      payload.curve = requireNonEmptyString(record.curve, `${context}.curve`);
    }
    if (record.max_proof_bytes !== undefined && record.max_proof_bytes !== null) {
      payload.max_proof_bytes = normalizeUnsignedInteger(
        record.max_proof_bytes,
        `${context}.maxProofBytes`,
        { allowZero: true },
      );
    }
    if (record.metadata_uri_cid !== undefined && record.metadata_uri_cid !== null) {
      payload.metadata_uri_cid = requireNonEmptyString(
        record.metadata_uri_cid,
        `${context}.metadataUriCid`,
      );
    }
    if (record.vk_bytes_cid !== undefined && record.vk_bytes_cid !== null) {
      payload.vk_bytes_cid = requireNonEmptyString(
        record.vk_bytes_cid,
        `${context}.vkBytesCid`,
      );
    }
    if (record.activation_height !== undefined && record.activation_height !== null) {
      payload.activation_height = normalizeUnsignedInteger(
        record.activation_height,
        `${context}.activationHeight`,
        { allowZero: true },
      );
    }
    if (record.withdraw_height !== undefined && record.withdraw_height !== null) {
      payload.withdraw_height = normalizeUnsignedInteger(
        record.withdraw_height,
        `${context}.withdrawHeight`,
        { allowZero: true },
      );
    }
    validateVerifyingKeyHeightRange(
      payload.activation_height,
      payload.withdraw_height,
      context,
    );
    if (record.commitment_hex !== undefined && record.commitment_hex !== null) {
      payload.commitment_hex = normalizeVerifyingKeyHex32(
        record.commitment_hex,
        `${context}.commitmentHex`,
      );
    }
    const statusValue = normalizeVerifyingKeyStatusValue(
      record.status,
      `${context}.status`,
      { optional: true },
    );
    if (statusValue) payload.status = statusValue;
    const bytesValue =
      record.vk_bytes ??
      record.verifyingKeyBytes ??
      record.bytes ??
      record.inlineKeyBytes;
    const lenValue = record.vk_len;
    if (bytesValue !== undefined && bytesValue !== null) {
      const base64 = normalizeRequiredBase64Payload(
        bytesValue,
        `${context}.vk_bytes`,
      );
      const length = Buffer.from(base64, "base64").length;
      payload.vk_bytes = base64;
      if (lenValue !== undefined && lenValue !== null) {
        const normalizedLen = normalizeUnsignedInteger(
          lenValue,
          `${context}.vkLen`,
          { allowZero: false },
        );
        if (normalizedLen !== length) {
          throw createValidationError(
            ValidationErrorCode.INVALID_OBJECT,
            `${context}.vk_len must match vk_bytes length (${length})`,
            `${context}.vkLen`,
          );
        }
        payload.vk_len = normalizedLen;
      } else {
        payload.vk_len = length;
      }
    } else if (lenValue !== undefined && lenValue !== null) {
      payload.vk_len = normalizeUnsignedInteger(
        lenValue,
        `${context}.vkLen`,
        { allowZero: false },
      );
    }
    if (payload.vk_bytes === undefined) {
      if (payload.commitment_hex === undefined) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.commitment_hex is required when vk_bytes is omitted`,
          `${context}.commitmentHex`,
        );
      }
      if (payload.vk_len === undefined) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.vk_len is required when vk_bytes is omitted`,
          `${context}.vkLen`,
        );
      }
    }
    if (payload.vk_bytes !== undefined && payload.commitment_hex !== undefined) {
      const expectedCommitmentHex = computeVerifyingKeyCommitmentHex(
        payload.backend,
        payload.vk_bytes,
      );
      if (payload.commitment_hex !== expectedCommitmentHex) {
        throw createValidationError(
          ValidationErrorCode.INVALID_OBJECT,
          `${context}.commitment_hex must match domain-separated SHA-256 of backend and vk_bytes`,
          `${context}.commitmentHex`,
        );
      }
    }
  }

  function computeVerifyingKeyCommitmentHex(backend, base64Bytes) {
    const backendBytes = Buffer.from(backend, "utf8");
    const vkBytes = Buffer.from(base64Bytes, "base64");
    return createHash("sha256")
      .update(Buffer.from("iroha:zk:v1:vk", "utf8"))
      .update(u64BeBuffer(backendBytes.length))
      .update(backendBytes)
      .update(u64BeBuffer(vkBytes.length))
      .update(vkBytes)
      .digest("hex");
  }

  return Object.freeze({
    activeIds: requestActiveVerifyingKeyIds,
    backend: assertProductionVerifyBackendLabel,
    detail: normalizeVerifyingKeyDetail,
    draft: normalizeVerifyingKeyTransactionDraft,
    id: normalizeVerifyingKeyId,
    list: normalizeVerifyingKeyListPayload,
    listQuery: buildVerifyingKeyListQuery,
    name: normalizeVerifyingKeyName,
    register: normalizeVerifyingKeyRegisterPayload,
    signingContext: requireVerifyingKeySigningContext,
    update: normalizeVerifyingKeyUpdatePayload,
  });
}

function u64BeBuffer(value) {
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64BE(BigInt(value));
  return buffer;
}
