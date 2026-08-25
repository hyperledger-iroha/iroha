import { x25519 } from "@noble/curves/ed25519";
import { blake2b256 } from "./blake2b.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { assertWellFormedUtf16 } from "./instructionBuilderPrimitives.js";
import { strictDecodeBase64 } from "./toriiClientEncoding.js";

const BASE58_PATTERN = /^[1-9A-HJ-NP-Za-km-z]+$/;
const HF_COMMIT_OID_PATTERN_V1 = /^[0-9a-f]{40}$/;
const REJECTED_SIGNING_SECRET_FIELDS = [
  "privateKeyHex",
  "privateKey",
  "private_key",
  "private_key_hex",
];
const PROVENANCE_SCHEMA = "soracloud.hf.deploy.provenance.v1";
const APP_INFRA_PROVENANCE_SCHEMA = "soracloud.app.infra.provenance.v1";
const HF_DEPLOY_PAYLOAD_FIELDS = new Set([
  "repo_id",
  "revision",
  "model_name",
  "service_name",
  "apartment_name",
  "storage_class",
  "lease_term_ms",
  "lease_asset_definition_id",
  "base_fee_nanos",
]);
export const SORACLOUD_APP_INFRA_DEPLOY_WIRE_ID =
  "iroha_data_model::isi::soracloud::DeploySoracloudAppInfra";
export const SORACLOUD_APP_INFRA_UPGRADE_WIRE_ID =
  "iroha_data_model::isi::soracloud::UpgradeSoracloudAppInfra";
const PRIVATE_UPLOADED_MODEL_COUNT_MODES = new Set(["bounded", "exact"]);
const PRIVATE_UPLOADED_MODEL_IDENTIFIER_PATTERN = /^[A-Za-z0-9._:#-]+$/u;
const PRIVATE_UPLOADED_MODEL_U32_MAX = 0xffff_ffff;
const PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT = 500;
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_PATTERN = /^[A-Za-z0-9_-]{114}$/u;
const PRIVATE_UPLOADED_MODEL_MAX_CIPHERTEXT_BYTES = 72 * 1024 * 1024;
const PRIVATE_UPLOADED_MODEL_ZERO_PREHASH_BODY = `${"00".repeat(31)}01`;
const PRIVATE_UPLOADED_MODEL_CONTROL_PATTERN = /[\u0000-\u001F\u007F-\u009F]/u;
const PRIVATE_UPLOADED_MODEL_BIDI_CONTROL_PATTERN =
  /[\u061C\u200E\u200F\u202A-\u202E\u2066-\u2069]/u;
const PRIVATE_UPLOADED_MODEL_NAME_FORBIDDEN_PATTERN = /[@#$]/u;
const PRIVATE_UPLOADED_MODEL_UTF8_ENCODER = new TextEncoder();
const X25519_LOW_ORDER_PROBE_PRIVATE_KEY = new Uint8Array(32).fill(1);
const JSON_ACCEPT_HEADERS = Object.freeze({ Accept: "application/json" });

function requireExactObject(input, label, allowedFields, requiredFields = allowedFields) {
  if (input == null || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError(`${label} must be an object`);
  }
  const prototype = Object.getPrototypeOf(input);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${label} inherited properties are not accepted`);
  }
  const allowed = new Set(allowedFields);
  for (const field of Object.getOwnPropertyNames(input)) {
    if (!allowed.has(field)) {
      throw new TypeError(`${label}.${field} is not accepted`);
    }
    if (!Object.getOwnPropertyDescriptor(input, field)?.enumerable) {
      throw new TypeError(`${label}.${field} must be enumerable`);
    }
  }
  if (Object.getOwnPropertySymbols(input).length > 0) {
    throw new TypeError(`${label} symbols are not accepted`);
  }
  for (const field of requiredFields) {
    if (!Object.hasOwn(input, field)) {
      throw new TypeError(`${label}.${field} is required`);
    }
  }
  return input;
}

function rejectSoracloudSigningSecrets(input) {
  if (input == null || (typeof input !== "object" && typeof input !== "function")) {
    return;
  }
  for (const field of REJECTED_SIGNING_SECRET_FIELDS) {
    if (field in input) {
      throw new TypeError(`${field} is not accepted by the Soracloud JS API`);
    }
  }
}

function requireString(input, field) {
  const value = input?.[field];
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${field} must be a non-empty string`);
  }
  return value.trim();
}

function optionalString(input, field) {
  const value = input?.[field];
  if (value == null) {
    return undefined;
  }
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${field} must be a non-empty string when provided`);
  }
  return value.trim();
}

function nullableString(input, field, label = field) {
  const value = input[field];
  if (value === null) {
    return null;
  }
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string or null`);
  }
  return value.trim();
}

function requireHfCommitOid(input) {
  const revision = input?.revision;
  if (typeof revision !== "string" || revision === "") {
    throw new TypeError("revision must be a non-empty string");
  }
  if (!HF_COMMIT_OID_PATTERN_V1.test(revision)) {
    throw new TypeError(
      "revision must be a full 40-character lowercase hexadecimal commit OID",
    );
  }
  return revision;
}

function normalizeIntegerStringValue(value, field) {
  const normalized = typeof value === "bigint" ? value.toString() : String(value ?? "");
  if (!/^[0-9]+$/.test(normalized)) {
    throw new TypeError(`${field} must be a non-negative integer`);
  }
  return BigInt(normalized).toString();
}

function normalizeIntegerString(input, field) {
  return normalizeIntegerStringValue(input?.[field], field);
}

function normalizeSafeIntegerValue(value, field) {
  const normalized = normalizeIntegerStringValue(value, field);
  const numeric = Number(normalized);
  if (!Number.isSafeInteger(numeric)) {
    throw new TypeError(`${field} must fit in a safe JavaScript integer`);
  }
  return numeric;
}

function normalizeSafeInteger(input, field) {
  return normalizeSafeIntegerValue(input?.[field], field);
}

function normalizeSafePositiveIntegerValue(value, field) {
  const normalized = normalizeSafeIntegerValue(value, field);
  if (normalized <= 0) {
    throw new TypeError(`${field} must be greater than zero`);
  }
  return normalized;
}

function normalizeSafePositiveInteger(input, field) {
  return normalizeSafePositiveIntegerValue(input?.[field], field);
}

/**
 * Fetch one exact-network account-authenticated Soracloud app status response.
 *
 * @param {object} client Torii client transport.
 * @param {Record<string, unknown>} options Request options including `canonicalAuth`.
 * @param {string | undefined} namedAppName Optional path-bound app name.
 * @returns {Promise<unknown>}
 */
export async function requestSoracloudAppInfraStatus(
  client,
  options,
  namedAppName = undefined,
) {
  const context = namedAppName === undefined
    ? "getSoracloudAppInfraStatus"
    : "getSoracloudNamedAppInfraStatus";
  if (options == null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError(`${context} options must be an object`);
  }
  if (options.canonicalAuth == null) {
    throw new TypeError(`${context} options.canonicalAuth is required`);
  }

  const params = {};
  const path = namedAppName === undefined
    ? "/v1/soracloud/apps/status"
    : `/v1/soracloud/apps/${encodeURIComponent(requireString({ appName: namedAppName }, "appName"))}/status`;
  if (namedAppName === undefined && options.appName != null) {
    params.app_name = requireString(options, "appName");
  }
  if (options.auditLimit != null) {
    params.audit_limit = normalizeSafePositiveInteger(options, "auditLimit");
  }
  const response = await client._request("GET", path, {
    params,
    headers: JSON_ACCEPT_HEADERS,
    signal: options.signal,
    canonicalAuth: options.canonicalAuth,
  });
  await client._expectStatus(response, [200]);
  return client._maybeJson(response);
}

function normalizeArray(input, field) {
  const value = input?.[field];
  if (!Array.isArray(value)) {
    throw new TypeError(`${field} must be an array`);
  }
  return value;
}

function normalizeCanonicalHashLiteral(value, field) {
  const match = typeof value === "string"
    ? /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u.exec(value)
    : null;
  if (match === null) {
    throw new TypeError(
      `${field} must be an exact uppercase checksummed marker-bit Iroha Hash literal`,
    );
  }
  const [, body, checksum] = match;
  if ((Number.parseInt(body.slice(-2), 16) & 1) === 0) {
    throw new TypeError(
      `${field} must be an exact uppercase checksummed marker-bit Iroha Hash literal`,
    );
  }
  if (computeHashLiteralCrc("hash", body) !== checksum) {
    throw new TypeError(`${field} has invalid checksum for its Iroha Hash literal`);
  }
  if (body === PRIVATE_UPLOADED_MODEL_ZERO_PREHASH_BODY) {
    throw new TypeError(`${field} must not be the zero prehash sentinel`);
  }
  return value;
}

function irohaBlake2b256PrehashLiteral(value) {
  const digest = blake2b256(value);
  digest[digest.length - 1] |= 1;
  const body = Array.from(
    digest,
    (byte) => byte.toString(16).toUpperCase().padStart(2, "0"),
  ).join("");
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

function normalizePrivateExactString(value, field, maxUtf8Bytes = undefined) {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${field} must be a non-empty string`);
  }
  assertWellFormedUtf16(value, field);
  if (value.trim() !== value) {
    throw new TypeError(`${field} must not contain surrounding whitespace`);
  }
  if (PRIVATE_UPLOADED_MODEL_CONTROL_PATTERN.test(value)) {
    throw new TypeError(`${field} must not contain control characters`);
  }
  if (
    maxUtf8Bytes !== undefined &&
    PRIVATE_UPLOADED_MODEL_UTF8_ENCODER.encode(value).length > maxUtf8Bytes
  ) {
    throw new TypeError(`${field} must contain at most ${maxUtf8Bytes} UTF-8 bytes`);
  }
  return value;
}

function normalizePrivateName(value, field) {
  const exact = normalizePrivateExactString(value, field, 255);
  if (exact.normalize("NFC") !== exact) {
    throw new TypeError(`${field} must use its exact NFC-normalized spelling`);
  }
  if (
    /\s/u.test(exact) ||
    PRIVATE_UPLOADED_MODEL_BIDI_CONTROL_PATTERN.test(exact) ||
    PRIVATE_UPLOADED_MODEL_NAME_FORBIDDEN_PATTERN.test(exact)
  ) {
    throw new TypeError(`${field} must be a canonical Iroha Name`);
  }
  return exact;
}

function normalizePrivateIdentifier(value, field) {
  const exact = normalizePrivateExactString(value, field, 128);
  if (!PRIVATE_UPLOADED_MODEL_IDENTIFIER_PATTERN.test(exact)) {
    throw new TypeError(
      `${field} must use only ASCII letters, digits, or [-_.:#]`,
    );
  }
  return exact;
}

function normalizeNullablePrivateValue(value, field, normalizer) {
  return value === null ? null : normalizer(value, field);
}

function normalizeOptionalPrivateValue(value, field, normalizer) {
  return value === undefined ? undefined : normalizer(value, field);
}

function normalizePrivatePositiveIntegerValue(value, field) {
  if (!["bigint", "number", "string"].includes(typeof value)) {
    throw new TypeError(`${field} must be a number, bigint, or decimal string`);
  }
  if (typeof value === "string" && !/^[1-9][0-9]*$/u.test(value)) {
    throw new TypeError(`${field} must be a canonical positive decimal integer`);
  }
  return normalizeSafePositiveIntegerValue(value, field);
}

function normalizeStorageClass(value) {
  if (value !== "hot" && value !== "warm" && value !== "cold") {
    throw new TypeError("storageClass must be hot, warm, or cold");
  }
  return value;
}

function normalizeLeaseAssetDefinitionId(input) {
  const value = requireString(input, "leaseAssetDefinitionId");
  if (!BASE58_PATTERN.test(value)) {
    throw new Error("Asset Definition ID must be valid Base58");
  }
  return value;
}

function normalizeExactUnsignedByteArray(value, field, expectedLength) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
    throw new TypeError(`${field} must be a plain array`);
  }
  if (value.length !== expectedLength) {
    throw new TypeError(`${field} must contain exactly ${expectedLength} bytes`);
  }
  if (Object.getOwnPropertySymbols(value).length > 0) {
    throw new TypeError(`${field} symbols are not accepted`);
  }
  for (const property of Object.getOwnPropertyNames(value)) {
    if (property === "length") {
      continue;
    }
    const index = Number(property);
    if (!Number.isInteger(index) || index < 0 || index >= value.length || String(index) !== property) {
      throw new TypeError(`${field}.${property} is not accepted`);
    }
  }
  const bytes = [];
  for (let index = 0; index < expectedLength; index += 1) {
    if (!Object.hasOwn(value, index)) {
      throw new TypeError(`${field}[${index}] is required`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (descriptor == null || !("value" in descriptor) || descriptor.enumerable !== true) {
      throw new TypeError(`${field}[${index}] must be an enumerable data property`);
    }
    const byte = descriptor.value;
    if (!Number.isInteger(byte) || byte < 0 || byte > 0xff) {
      throw new TypeError(`${field}[${index}] must be an unsigned byte`);
    }
    bytes.push(byte);
  }
  return bytes;
}

function normalizeManifestDigest(value, field) {
  return normalizeExactUnsignedByteArray(value, field, 32);
}

function normalizeManifestRootCid(value, field) {
  const bytes = normalizeExactUnsignedByteArray(value, field, 36);
  if (bytes[0] !== 1 || bytes[1] !== 0x71 || bytes[2] !== 0x1f || bytes[3] !== 32) {
    throw new TypeError(
      `${field} must use canonical CIDv1/dag-cbor/BLAKE3-256 framing`,
    );
  }
  if (bytes.slice(4).every((byte) => byte === 0)) {
    throw new TypeError(`${field} digest must not be all zero`);
  }
  return bytes;
}

function normalizePrivateCiphertextBytes(value, field) {
  const bytes = normalizePrivatePositiveIntegerValue(value, field);
  if (bytes > PRIVATE_UPLOADED_MODEL_MAX_CIPHERTEXT_BYTES) {
    throw new TypeError(
      `${field} must be between 1 and ${PRIVATE_UPLOADED_MODEL_MAX_CIPHERTEXT_BYTES}`,
    );
  }
  return bytes;
}

function normalizePrivateArtifactRef(input, field, expectedRole) {
  const artifact = input?.[field];
  if (artifact == null || typeof artifact !== "object" || Array.isArray(artifact)) {
    throw new TypeError(`${field} must be an object`);
  }
  rejectSoracloudSigningSecrets(artifact);
  requireExactObject(artifact, field, [
    "schemaVersion",
    "sorafsManifestDigest",
    "sorafsRootCid",
    "artifactHash",
    "ciphertextBytes",
    "artifactRole",
  ]);
  const role = artifact.artifactRole;
  if (role !== expectedRole) {
    throw new TypeError(`${field}.artifactRole must be ${expectedRole}`);
  }
  const schemaVersion = normalizePrivatePositiveIntegerValue(
    artifact.schemaVersion,
    `${field}.schemaVersion`,
  );
  if (schemaVersion !== 1) {
    throw new TypeError(`${field}.schemaVersion must be 1`);
  }
  return {
    schema_version: schemaVersion,
    sorafs_manifest_digest: normalizeManifestDigest(
      artifact.sorafsManifestDigest,
      `${field}.sorafsManifestDigest`,
    ),
    sorafs_root_cid: normalizeManifestRootCid(
      artifact.sorafsRootCid,
      `${field}.sorafsRootCid`,
    ),
    artifact_hash: normalizeCanonicalHashLiteral(
      artifact.artifactHash,
      `${field}.artifactHash`,
    ),
    ciphertext_bytes: normalizePrivateCiphertextBytes(
      artifact.ciphertextBytes,
      `${field}.ciphertextBytes`,
    ),
    artifact_role: role,
  };
}

function normalizePrivateOutputRecipient(input) {
  const recipient = input?.outputRecipient;
  if (recipient == null || typeof recipient !== "object" || Array.isArray(recipient)) {
    throw new TypeError("outputRecipient must be an object");
  }
  rejectSoracloudSigningSecrets(recipient);
  requireExactObject(recipient, "outputRecipient", [
    "schemaVersion",
    "keyId",
    "keyVersion",
    "kem",
    "aead",
    "publicKeyBytes",
    "publicKeyFingerprint",
  ]);
  if (recipient.kem !== "X25519HkdfSha256") {
    throw new TypeError("outputRecipient.kem must be X25519HkdfSha256");
  }
  if (recipient.aead !== "Aes256Gcm") {
    throw new TypeError("outputRecipient.aead must be Aes256Gcm");
  }
  const schemaVersion = normalizePrivatePositiveIntegerValue(
    recipient.schemaVersion,
    "outputRecipient.schemaVersion",
  );
  if (schemaVersion !== 1) {
    throw new TypeError("outputRecipient.schemaVersion must be 1");
  }
  const publicKeyBytes = normalizePrivateExactString(
    recipient.publicKeyBytes,
    "outputRecipient.publicKeyBytes",
  );
  if (!/^[A-Za-z0-9+/]{43}=$/u.test(publicKeyBytes)) {
    throw new TypeError("outputRecipient.publicKeyBytes must be canonical padded base64");
  }
  let decodedPublicKey;
  try {
    decodedPublicKey = strictDecodeBase64(publicKeyBytes);
  } catch (error) {
    throw new TypeError("outputRecipient.publicKeyBytes must be canonical padded base64", {
      cause: error,
    });
  }
  if (decodedPublicKey.length !== 32) {
    throw new TypeError("outputRecipient.publicKeyBytes must encode exactly 32 bytes");
  }
  try {
    x25519.getSharedSecret(X25519_LOW_ORDER_PROBE_PRIVATE_KEY, decodedPublicKey);
  } catch (error) {
    throw new TypeError(
      "outputRecipient.publicKeyBytes must not encode a low-order X25519 public key",
      { cause: error },
    );
  }
  const keyVersion = normalizePrivatePositiveIntegerValue(
    recipient.keyVersion,
    "outputRecipient.keyVersion",
  );
  if (keyVersion > PRIVATE_UPLOADED_MODEL_U32_MAX) {
    throw new TypeError("outputRecipient.keyVersion must fit in an unsigned 32-bit integer");
  }
  const publicKeyFingerprint = normalizeCanonicalHashLiteral(
    recipient.publicKeyFingerprint,
    "outputRecipient.publicKeyFingerprint",
  );
  const expectedFingerprint = irohaBlake2b256PrehashLiteral(decodedPublicKey);
  if (publicKeyFingerprint !== expectedFingerprint) {
    throw new TypeError(
      "outputRecipient.publicKeyFingerprint must equal the Iroha Blake2b-256 prehash of outputRecipient.publicKeyBytes",
    );
  }
  return {
    schema_version: schemaVersion,
    key_id: normalizePrivateExactString(recipient.keyId, "outputRecipient.keyId"),
    key_version: keyVersion,
    kem: { kem: "X25519HkdfSha256", value: null },
    aead: { aead: "Aes256Gcm", value: null },
    public_key_bytes: publicKeyBytes,
    public_key_fingerprint: publicKeyFingerprint,
  };
}

function canonicalSigningPayload(label, payload, schema = PROVENANCE_SCHEMA) {
  return {
    schema,
    label,
    payload: cloneCanonical(payload),
  };
}

function cloneCanonical(value) {
  if (Array.isArray(value)) {
    return value.map((item) => cloneCanonical(item));
  }
  if (value != null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value).map((key) => [key, cloneCanonical(value[key])]),
    );
  }
  return value;
}

function deepEqualCanonical(left, right) {
  if (Object.is(left, right)) {
    return true;
  }
  if (Array.isArray(left) || Array.isArray(right)) {
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => deepEqualCanonical(value, right[index]))
    );
  }
  if (
    left == null ||
    right == null ||
    typeof left !== "object" ||
    typeof right !== "object"
  ) {
    return false;
  }
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every(
      (key, index) =>
        key === rightKeys[index] && deepEqualCanonical(left[key], right[key]),
    )
  );
}

function requireAllowedDraftPayloadFields(payload) {
  for (const field of REJECTED_SIGNING_SECRET_FIELDS) {
    if (field in payload) {
      if (!Object.hasOwn(payload, field)) {
        throw new TypeError(`draft payload.${field} must be an own property`);
      }
      throw new TypeError(`draft payload.${field} is not accepted`);
    }
  }
  for (const field of HF_DEPLOY_PAYLOAD_FIELDS) {
    if (field in payload && !Object.hasOwn(payload, field)) {
      throw new TypeError(`draft payload.${field} must be an own property`);
    }
  }
  for (const field of Object.getOwnPropertyNames(payload)) {
    if (!HF_DEPLOY_PAYLOAD_FIELDS.has(field)) {
      throw new TypeError(`draft payload.${field} is not accepted`);
    }
    if (!Object.getOwnPropertyDescriptor(payload, field)?.enumerable) {
      throw new TypeError(`draft payload.${field} must be enumerable`);
    }
  }
  if (Object.getOwnPropertySymbols(payload).length > 0) {
    throw new TypeError("draft payload symbols are not accepted");
  }
  for (const field in payload) {
    if (!Object.hasOwn(payload, field)) {
      throw new TypeError(`draft payload.${field} must be an own property`);
    }
    if (!HF_DEPLOY_PAYLOAD_FIELDS.has(field)) {
      throw new TypeError(`draft payload.${field} is not accepted`);
    }
  }
}

function requireAssembledDraftPayloadShape(payload) {
  requireAllowedDraftPayloadFields(payload);
  for (const field of [
    "repo_id",
    "revision",
    "model_name",
    "service_name",
    "lease_asset_definition_id",
  ]) {
    if (
      !Object.hasOwn(payload, field) ||
      typeof payload[field] !== "string" ||
      payload[field].trim() === ""
    ) {
      throw new TypeError(`draft payload.${field} must be a non-empty string`);
    }
  }
  if (!HF_COMMIT_OID_PATTERN_V1.test(payload.revision)) {
    throw new TypeError(
      "draft payload.revision must be a full 40-character lowercase hexadecimal commit OID",
    );
  }
  if (!Object.hasOwn(payload, "apartment_name")) {
    throw new TypeError("draft payload.apartment_name is required");
  }
  if (
    payload.apartment_name !== null &&
    (typeof payload.apartment_name !== "string" || payload.apartment_name.trim() === "")
  ) {
    throw new TypeError("draft payload.apartment_name must be a non-empty string or null");
  }
  if (!BASE58_PATTERN.test(payload.lease_asset_definition_id)) {
    throw new Error("draft payload.lease_asset_definition_id must be valid Base58");
  }
  if (!["hot", "warm", "cold"].includes(payload.storage_class)) {
    throw new TypeError("draft payload.storage_class must be hot, warm, or cold");
  }
  if (
    !Number.isSafeInteger(payload.lease_term_ms) ||
    payload.lease_term_ms < 0
  ) {
    throw new TypeError("draft payload.lease_term_ms must be a safe non-negative integer");
  }
  if (
    !Object.hasOwn(payload, "base_fee_nanos") ||
    typeof payload.base_fee_nanos !== "string" ||
    !/^[0-9]+$/.test(payload.base_fee_nanos) ||
    BigInt(payload.base_fee_nanos).toString() !== payload.base_fee_nanos
  ) {
    throw new TypeError("draft payload.base_fee_nanos must be a canonical non-negative integer string");
  }
}

function generatedServiceSigningPayload(payload) {
  return {
    service_name: payload.service_name,
    repo_id: payload.repo_id,
    revision: payload.revision,
  };
}

function generatedApartmentSigningPayload(payload) {
  return {
    apartment_name: payload.apartment_name,
    service_name: payload.service_name,
  };
}

/**
 * Build an unsigned `/v1/soracloud/hf/deploy` draft.
 *
 * @param {{ repoId: string, revision: string, modelName: string, serviceName: string, apartmentName: string | null, storageClass: "hot" | "warm" | "cold", leaseTermMs: number | bigint | string, leaseAssetDefinitionId: string, baseFeeNanos: number | bigint | string }} input
 * @returns {{ payload: Record<string, unknown>, provenancePayloads: { deploy: Record<string, unknown>, generatedService: Record<string, unknown>, generatedApartment: Record<string, unknown> | null } }}
 */
export function buildSoracloudHfDeployDraft(input = {}) {
  rejectSoracloudSigningSecrets(input);
  requireExactObject(input, "input", [
    "repoId",
    "revision",
    "modelName",
    "serviceName",
    "apartmentName",
    "storageClass",
    "leaseTermMs",
    "leaseAssetDefinitionId",
    "baseFeeNanos",
  ]);
  const apartmentName = nullableString(input, "apartmentName");
  const payload = {
    repo_id: requireString(input, "repoId"),
    revision: requireHfCommitOid(input),
    model_name: requireString(input, "modelName"),
    service_name: requireString(input, "serviceName"),
    apartment_name: apartmentName,
    storage_class: normalizeStorageClass(input.storageClass),
    lease_term_ms: normalizeSafeInteger(input, "leaseTermMs"),
    lease_asset_definition_id: normalizeLeaseAssetDefinitionId(input),
    base_fee_nanos: normalizeIntegerString(input, "baseFeeNanos"),
  };

  const provenancePayloads = {
    deploy: canonicalSigningPayload("hf_deploy", payload),
    generatedService: canonicalSigningPayload("generated_service", {
      service_name: payload.service_name,
      repo_id: payload.repo_id,
      revision: payload.revision,
    }),
    generatedApartment: null,
  };
  if (payload.apartment_name !== null) {
    provenancePayloads.generatedApartment = canonicalSigningPayload("generated_apartment", {
      apartment_name: payload.apartment_name,
      service_name: payload.service_name,
    });
  }
  return { payload, provenancePayloads };
}

function normalizeStringMap(value, field) {
  if (value == null) {
    return {};
  }
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${field} must be an object`);
  }
  rejectSoracloudSigningSecrets(value);
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => {
      if (typeof key !== "string" || key.trim() === "") {
        throw new TypeError(`${field} keys must be non-empty strings`);
      }
      if (typeof entry !== "string") {
        throw new TypeError(`${field}.${key} must be a string`);
      }
      return [key.trim(), entry];
    }),
  );
}

function normalizeRouteSpec(route, field) {
  rejectSoracloudSigningSecrets(route);
  if (route == null || typeof route !== "object" || Array.isArray(route)) {
    throw new TypeError(`${field} must be an object`);
  }
  requireExactObject(route, field, ["path", "publicHost", "internalUrl"]);
  const path = requireString(route, "path");
  const publicHost = nullableString(route, "publicHost", `${field}.publicHost`);
  const internalUrl = nullableString(route, "internalUrl", `${field}.internalUrl`);
  return {
    schema_version: 1,
    public_host: publicHost,
    path_prefix: path,
    internal_url: internalUrl,
  };
}

function normalizeLeaseVolumeSpec(volume, field) {
  rejectSoracloudSigningSecrets(volume);
  if (volume == null || typeof volume !== "object" || Array.isArray(volume)) {
    throw new TypeError(`${field} must be an object`);
  }
  requireExactObject(volume, field, ["name", "mountPath", "maxTotalBytes", "temperature"]);
  requireString(volume, "mountPath");
  const temperature = requireString(volume, "temperature");
  if (!["hot", "warm", "cold"].includes(temperature)) {
    throw new TypeError(`${field}.temperature must be hot, warm, or cold`);
  }
  normalizeSafePositiveIntegerValue(
    volume.maxTotalBytes,
    `${field}.maxTotalBytes`,
  );
  return requireString(volume, "name");
}

function normalizeAppStaticSite(input) {
  const site = input.staticSite;
  if (site === null) {
    return null;
  }
  rejectSoracloudSigningSecrets(site);
  if (typeof site !== "object" || Array.isArray(site)) {
    throw new TypeError("staticSite must be an object");
  }
  requireExactObject(site, "staticSite", [
    "publicUrl",
    "contentCid",
    "manifestDigestHex",
    "mountPath",
    "apiBasePath",
  ]);
  const payload = {
    schema_version: 1,
    public_url: requireString(site, "publicUrl"),
    content_cid: nullableString(site, "contentCid", "staticSite.contentCid"),
    manifest_digest_hex: nullableString(
      site,
      "manifestDigestHex",
      "staticSite.manifestDigestHex",
    ),
    mount_path: requireString(site, "mountPath"),
    api_base_path: nullableString(site, "apiBasePath", "staticSite.apiBasePath"),
  };
  return payload;
}

function normalizeServiceRuntime(value, field) {
  const runtime = value;
  if (!["Inrou", "Ivm"].includes(runtime)) {
    throw new TypeError(`${field} must be Inrou or Ivm`);
  }
  return runtime;
}

function normalizeExecutionPlane(value, field) {
  const executionPlane = value;
  if (!["HttpService", "DeterministicService"].includes(executionPlane)) {
    throw new TypeError(`${field} must be HttpService or DeterministicService`);
  }
  return executionPlane;
}

function normalizeServiceSpec(service, index) {
  rejectSoracloudSigningSecrets(service);
  if (service == null || typeof service !== "object" || Array.isArray(service)) {
    throw new TypeError(`services[${index}] must be an object`);
  }
  requireExactObject(service, `services[${index}]`, [
    "name",
    "serviceVersion",
    "serviceManifestHash",
    "containerManifestHash",
    "runtime",
    "executionPlane",
    "routes",
    "leaseVolumes",
    "shards",
  ]);
  const serviceName = requireString(service, "name");
  const serviceVersion = requireString(service, "serviceVersion");
  const runtime = normalizeServiceRuntime(
    requireString(service, "runtime"),
    `services[${index}].runtime`,
  );
  const executionPlane = normalizeExecutionPlane(
    requireString(service, "executionPlane"),
    `services[${index}].executionPlane`,
  );
  const routes = normalizeArray(service, "routes").map((route, routeIndex) =>
    normalizeRouteSpec(route, `services[${index}].routes[${routeIndex}]`),
  );
  const leaseVolumes = normalizeArray(service, "leaseVolumes").map(
    (volume, volumeIndex) =>
      normalizeLeaseVolumeSpec(volume, `services[${index}].leaseVolumes[${volumeIndex}]`),
  );
  const base = {
    schema_version: 1,
    service_name: serviceName,
    service_version: serviceVersion,
    service_manifest_hash: requireString(service, "serviceManifestHash"),
    container_manifest_hash: requireString(service, "containerManifestHash"),
    execution_plane: executionPlane,
    runtime,
    routes,
    lease_volumes: leaseVolumes,
    shard: null,
  };
  const shards = service.shards;
  if (shards === null) {
    return [base];
  }
  rejectSoracloudSigningSecrets(shards);
  if (typeof shards !== "object" || Array.isArray(shards)) {
    throw new TypeError(`services[${index}].shards must be an object`);
  }
  requireExactObject(shards, `services[${index}].shards`, [
    "count",
    "shardIdEnv",
    "shardCountEnv",
  ]);
  const count = normalizeSafePositiveIntegerValue(shards.count, `services[${index}].shards.count`);
  const shardIdEnv = requireString(shards, "shardIdEnv");
  const shardCountEnv = requireString(shards, "shardCountEnv");
  return Array.from({ length: count }, (_, shardIndex) => ({
    ...base,
    service_name: `${serviceName}_${String(shardIndex).padStart(2, "0")}`,
    shard: `${shardIdEnv}=${shardIndex};${shardCountEnv}=${count}`,
  }));
}

/**
 * Build an unsigned Soracloud decentralized app-infra draft.
 *
 * This helper mirrors the request shape produced by
 * `iroha soracloud app simulate` and `iroha soracloud app release`: app
 * topology first, then external provenance, then app-infra deploy/upgrade
 * submission through Torii. It keeps low-level Torii clients usable without
 * hand-expanding worker shards.
 *
 * @param {{ appName: string, appVersion: string, publicUrl: string, staticSite: Record<string, unknown> | null, services: Array<Record<string, unknown>> }} input
 * @returns {{ payload: Record<string, unknown>, provenancePayloads: { deploy: Record<string, unknown>, services: Record<string, unknown>[] } }}
 */
export function buildSoracloudAppInfraDraft(input = {}) {
  rejectSoracloudSigningSecrets(input);
  requireExactObject(input, "input", [
    "appName",
    "appVersion",
    "publicUrl",
    "staticSite",
    "services",
  ]);
  const services = normalizeArray(input, "services")
    .flatMap((service, index) => normalizeServiceSpec(service, index));
  if (services.length === 0) {
    throw new TypeError("services must contain at least one service");
  }
  const payload = {
    schema_version: 1,
    app_name: requireString(input, "appName"),
    app_version: requireString(input, "appVersion"),
    public_url: requireString(input, "publicUrl"),
    static_site: normalizeAppStaticSite(input),
    services,
  };
  return {
    payload,
    provenancePayloads: {
      deploy: canonicalSigningPayload("app_infra_deploy", payload, APP_INFRA_PROVENANCE_SCHEMA),
      services: services.map((service) =>
        canonicalSigningPayload("app_infra_service", service, APP_INFRA_PROVENANCE_SCHEMA),
      ),
    },
  };
}

function requireProvenance(provenances, field) {
  rejectSoracloudSigningSecrets(provenances);
  if (
    provenances == null ||
    typeof provenances !== "object" ||
    Array.isArray(provenances) ||
    !Object.hasOwn(provenances, field)
  ) {
    throw new TypeError(`${field} provenance must include signer and signature`);
  }
  const provenance = provenances[field];
  rejectSoracloudSigningSecrets(provenance);
  if (
    provenance == null ||
    typeof provenance !== "object" ||
    Array.isArray(provenance) ||
    !Object.hasOwn(provenance, "signer") ||
    !Object.hasOwn(provenance, "signature") ||
    typeof provenance.signer !== "string" ||
    provenance.signer.trim() === "" ||
    typeof provenance.signature !== "string" ||
    provenance.signature.trim() === ""
  ) {
    throw new TypeError(`${field} provenance must include signer and signature`);
  }
  requireExactObject(provenance, `${field} provenance`, ["signer", "signature"]);
  return {
    signer: provenance.signer,
    signature: provenance.signature,
  };
}

function requireDraftSigningPayload(
  draft,
  field,
  label,
  expectedPayload,
  expectedSchema = PROVENANCE_SCHEMA,
) {
  if (
    !Object.hasOwn(draft, "provenancePayloads") ||
    draft.provenancePayloads == null ||
    typeof draft.provenancePayloads !== "object" ||
    Array.isArray(draft.provenancePayloads) ||
    !Object.hasOwn(draft.provenancePayloads, field)
  ) {
    throw new TypeError(`draft provenancePayloads.${field} is required`);
  }
  const signingPayload = draft.provenancePayloads[field];
  if (
    signingPayload == null ||
    typeof signingPayload !== "object" ||
    Array.isArray(signingPayload) ||
    !Object.hasOwn(signingPayload, "schema") ||
    !Object.hasOwn(signingPayload, "label") ||
    !Object.hasOwn(signingPayload, "payload") ||
    signingPayload.schema !== expectedSchema ||
    signingPayload.label !== label ||
    typeof signingPayload.payload !== "object" ||
    signingPayload.payload == null ||
    Array.isArray(signingPayload.payload)
  ) {
    throw new TypeError(`draft provenancePayloads.${field} is required`);
  }
  rejectSoracloudSigningSecrets(signingPayload);
  for (const payloadField in signingPayload.payload) {
    if (!Object.hasOwn(signingPayload.payload, payloadField)) {
      throw new TypeError(
        `draft provenancePayloads.${field} payload.${payloadField} must be an own property`,
      );
    }
  }
  rejectSoracloudSigningSecrets(signingPayload.payload);
  if (!deepEqualCanonical(signingPayload.payload, expectedPayload)) {
    throw new TypeError(`draft provenancePayloads.${field} payload must match draft payload`);
  }
}

function requireAppInfraDraftPayloadShape(payload) {
  if (
    payload == null ||
    typeof payload !== "object" ||
    Array.isArray(payload) ||
    payload.schema_version !== 1 ||
    typeof payload.app_name !== "string" ||
    payload.app_name.trim() === "" ||
    typeof payload.app_version !== "string" ||
    payload.app_version.trim() === "" ||
    typeof payload.public_url !== "string" ||
    payload.public_url.trim() === "" ||
    !Array.isArray(payload.services) ||
    payload.services.length === 0
  ) {
    throw new TypeError("draft payload must be a canonical Soracloud app infra manifest");
  }
}

/**
 * Assemble an app-infra deploy/upgrade request from a draft and external provenance.
 *
 * @param {{ payload: Record<string, unknown>, provenancePayloads?: Record<string, unknown> }} draft
 * @param {{ deploy: { signer: string, signature: string } }} provenances
 * @param {{ deployServices: unknown[], upgradeServices: unknown[] }} options
 * @returns {{ manifest: Record<string, unknown>, provenance: { signer: string, signature: string }, deploy_services: unknown[], upgrade_services: unknown[] }}
 */
export function assembleSoracloudAppInfraRequest(draft, provenances = {}, options = undefined) {
  rejectSoracloudSigningSecrets(draft);
  rejectSoracloudSigningSecrets(options);
  if (
    draft == null ||
    typeof draft !== "object" ||
    Array.isArray(draft) ||
    !Object.hasOwn(draft, "payload")
  ) {
    throw new TypeError("draft payload is required");
  }
  requireAppInfraDraftPayloadShape(draft.payload);
  requireDraftSigningPayload(
    draft,
    "deploy",
    "app_infra_deploy",
    draft.payload,
    APP_INFRA_PROVENANCE_SCHEMA,
  );
  requireExactObject(provenances, "provenances", ["deploy"]);
  requireExactObject(options, "options", ["deployServices", "upgradeServices"]);
  const deployServices = options.deployServices;
  const upgradeServices = options.upgradeServices;
  if (!Array.isArray(deployServices) || !Array.isArray(upgradeServices)) {
    throw new TypeError("deployServices and upgradeServices must be arrays when provided");
  }
  return {
    deploy_services: deployServices,
    upgrade_services: upgradeServices,
    manifest: cloneCanonical(draft.payload),
    provenance: requireProvenance(provenances, "deploy"),
  };
}

export function deploySoracloudAppInfraInstruction(manifest, provenance) {
  return {
    wire_id: SORACLOUD_APP_INFRA_DEPLOY_WIRE_ID,
    payload: {
      manifest: cloneCanonical(manifest),
      provenance: cloneCanonical(provenance),
    },
  };
}

export function upgradeSoracloudAppInfraInstruction(manifest, provenance) {
  return {
    wire_id: SORACLOUD_APP_INFRA_UPGRADE_WIRE_ID,
    payload: {
      manifest: cloneCanonical(manifest),
      provenance: cloneCanonical(provenance),
    },
  };
}

/**
 * Assemble a deploy request from an unsigned draft and externally signed provenance.
 *
 * @param {{ payload: Record<string, unknown>, provenancePayloads?: Record<string, unknown> }} draft
 * @param {{ deploy: { signer: string, signature: string }, generatedService: { signer: string, signature: string }, generatedApartment: { signer: string, signature: string } | null }} provenances
 * @returns {{ payload: Record<string, unknown>, provenance: { signer: string, signature: string }, generated_service_provenance: { signer: string, signature: string }, generated_apartment_provenance: { signer: string, signature: string } | null }}
 */
export function assembleSoracloudHfDeployRequest(draft, provenances = {}) {
  rejectSoracloudSigningSecrets(draft);
  if (
    draft == null ||
    typeof draft !== "object" ||
    Array.isArray(draft) ||
    !Object.hasOwn(draft, "payload") ||
    typeof draft.payload !== "object" ||
    draft.payload == null ||
    Array.isArray(draft.payload)
  ) {
    throw new TypeError("draft payload is required");
  }
  requireAssembledDraftPayloadShape(draft.payload);
  requireDraftSigningPayload(draft, "deploy", "hf_deploy", draft.payload);
  requireDraftSigningPayload(
    draft,
    "generatedService",
    "generated_service",
    generatedServiceSigningPayload(draft.payload),
  );
  requireExactObject(provenances, "provenances", [
    "deploy",
    "generatedService",
    "generatedApartment",
  ], ["deploy", "generatedService"]);
  const request = {
    payload: cloneCanonical(draft.payload),
    provenance: requireProvenance(provenances, "deploy"),
    generated_service_provenance: requireProvenance(provenances, "generatedService"),
    generated_apartment_provenance: null,
  };
  if (draft.payload.apartment_name !== null) {
    requireDraftSigningPayload(
      draft,
      "generatedApartment",
      "generated_apartment",
      generatedApartmentSigningPayload(draft.payload),
    );
    request.generated_apartment_provenance = requireProvenance(
      provenances,
      "generatedApartment",
    );
  } else {
    if (
      !Object.hasOwn(draft.provenancePayloads, "generatedApartment") ||
      draft.provenancePayloads.generatedApartment !== null
    ) {
      throw new TypeError(
        "draft provenancePayloads.generatedApartment must be null without an apartment",
      );
    }
    if (!Object.hasOwn(provenances, "generatedApartment")) {
      throw new TypeError("generatedApartment provenance is required");
    }
    if (provenances.generatedApartment !== null) {
      throw new TypeError("generatedApartment provenance must be null without an apartment");
    }
  }
  return request;
}

/**
 * Build a canonical private uploaded-model execution request.
 *
 * Exactly one model selector is required. The request binds an exact service
 * revision, committed decryption authorization, encrypted SoraFS input, and
 * output-recipient public key plus its Iroha prehash. It never accepts model
 * bytes, plaintext, validator-created output claims, runtime claims, or signing
 * secrets.
 *
 * @param {{ serviceName: string, serviceVersion: string, weightVersion: string, modelId: string | null, modelName: string | null, bundleRoot: string | null, decryptionRequestId: string, inputArtifact: Record<string, unknown>, outputRecipient: Record<string, unknown> }} input
 * @returns {Record<string, unknown>}
 */
export function buildSoracloudPrivateUploadedModelExecuteRequest(input = {}) {
  rejectSoracloudSigningSecrets(input);
  requireExactObject(input, "input", [
    "serviceName",
    "serviceVersion",
    "weightVersion",
    "modelId",
    "modelName",
    "bundleRoot",
    "decryptionRequestId",
    "inputArtifact",
    "outputRecipient",
  ]);
  const modelId = normalizeNullablePrivateValue(
    input.modelId,
    "modelId",
    normalizePrivateIdentifier,
  );
  const modelName = normalizeNullablePrivateValue(
    input.modelName,
    "modelName",
    normalizePrivateName,
  );
  if ((modelId === null) === (modelName === null)) {
    throw new TypeError("exactly one of modelId or modelName must be provided");
  }
  const bundleRoot = input.bundleRoot === null
    ? null
    : normalizeCanonicalHashLiteral(input.bundleRoot, "bundleRoot");
  return {
    service_name: normalizePrivateName(input.serviceName, "serviceName"),
    service_version: normalizePrivateExactString(
      input.serviceVersion,
      "serviceVersion",
      256,
    ),
    weight_version: normalizePrivateIdentifier(input.weightVersion, "weightVersion"),
    model_id: modelId,
    model_name: modelName,
    bundle_root: bundleRoot,
    decryption_request_id: normalizePrivateExactString(
      input.decryptionRequestId,
      "decryptionRequestId",
    ),
    input_artifact: normalizePrivateArtifactRef(input, "inputArtifact", "input"),
    output_recipient: normalizePrivateOutputRecipient(input),
  };
}

/**
 * Build query parameters for committed private uploaded-model execution receipts.
 *
 * @param {{ receiptId?: string, serviceName?: string, modelId?: string, weightVersion?: string, cursor?: string, limit?: number | bigint | string, countMode?: "bounded" | "exact" }} input
 * @returns {Record<string, string>}
 */
export function buildSoracloudPrivateUploadedModelReceiptQuery(input = {}) {
  rejectSoracloudSigningSecrets(input);
  requireExactObject(
    input,
    "input",
    ["receiptId", "serviceName", "modelId", "weightVersion", "cursor", "limit", "countMode"],
    [],
  );
  const query = {};
  if (input.receiptId !== undefined) {
    query.receipt_id = normalizeCanonicalHashLiteral(input.receiptId, "receiptId");
  }
  const serviceName = normalizeOptionalPrivateValue(
    input.serviceName,
    "serviceName",
    normalizePrivateName,
  );
  if (serviceName !== undefined) {
    query.service_name = serviceName;
  }
  const modelId = normalizeOptionalPrivateValue(
    input.modelId,
    "modelId",
    normalizePrivateIdentifier,
  );
  if (modelId !== undefined) {
    query.model_id = modelId;
  }
  const weightVersion = normalizeOptionalPrivateValue(
    input.weightVersion,
    "weightVersion",
    normalizePrivateIdentifier,
  );
  if (weightVersion !== undefined) {
    query.weight_version = weightVersion;
  }
  const cursor = normalizeOptionalPrivateValue(
    input.cursor,
    "cursor",
    normalizePrivateExactString,
  );
  if (cursor !== undefined) {
    if (!PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_PATTERN.test(cursor)) {
      throw new TypeError("cursor must be an exact canonical V1 receipt cursor");
    }
    query.cursor = cursor;
  }
  if (input.limit !== undefined) {
    const limit = normalizePrivatePositiveIntegerValue(input.limit, "limit");
    if (limit > PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT) {
      throw new TypeError(
        `limit must be between 1 and ${PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT}`,
      );
    }
    query.limit = String(limit);
  }
  const countMode = normalizeOptionalPrivateValue(
    input.countMode,
    "countMode",
    normalizePrivateExactString,
  );
  if (countMode !== undefined) {
    if (!PRIVATE_UPLOADED_MODEL_COUNT_MODES.has(countMode)) {
      throw new TypeError("countMode must be bounded or exact");
    }
    query.count_mode = countMode;
  }
  return query;
}
