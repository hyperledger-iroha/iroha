import { x25519 } from "@noble/curves/ed25519";
import { blake3 } from "@noble/hashes/blake3";
import { AccountAddress } from "./address.js";
import { blake2b256 } from "./blake2b.js";
import { publicKeyMulticodecForCurveId } from "./curveRegistry.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { assertWellFormedUtf16 } from "./instructionBuilderPrimitives.js";
import { NetworkId } from "./networkId.js";
import { KotodamaQuantity } from "./numericV1.js";
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
const PRIVATE_UPLOADED_MODEL_U64_MAX = 0xffff_ffff_ffff_ffffn;
const PRIVATE_UPLOADED_MODEL_SAFE_INTEGER_MAX = BigInt(Number.MAX_SAFE_INTEGER);
const PRIVATE_UPLOADED_MODEL_RECEIPT_MAX_LIMIT = 500;
const PRIVATE_UPLOADED_MODEL_RECEIPT_CURSOR_PATTERN = /^[A-Za-z0-9_-]{114}$/u;
const PRIVATE_UPLOADED_MODEL_MAX_CIPHERTEXT_BYTES = 72 * 1024 * 1024;
const PRIVATE_UPLOADED_MODEL_ZERO_PREHASH_BODY = `${"00".repeat(31)}01`;
const PRIVATE_UPLOADED_MODEL_CONTROL_PATTERN = /[\u0000-\u001F\u007F-\u009F]/u;
const PRIVATE_UPLOADED_MODEL_BIDI_CONTROL_PATTERN =
  /[\u061C\u200E\u200F\u202A-\u202E\u2066-\u2069]/u;
const PRIVATE_UPLOADED_MODEL_NAME_FORBIDDEN_PATTERN = /[@#$]/u;
const PRIVATE_UPLOADED_MODEL_UTF8_ENCODER = new TextEncoder();
const PRIVATE_UPLOADED_MODEL_RUNTIME_VERSION_V1 = "soracloud.quantized-cpu.v1";
const PRIVATE_UPLOADED_MODEL_AUTO_ORDER_DOMAIN_V1 =
  PRIVATE_UPLOADED_MODEL_UTF8_ENCODER.encode("sorafs:auto-replication-order:v1");
const PRIVATE_UPLOADED_MODEL_SUBMISSION_PHASES = new Set([
  "awaiting_output_durability",
  "prepare_submitted",
  "receipt_submitted",
  "committed",
]);
const PRIVATE_UPLOADED_MODEL_EXECUTE_RESPONSE_FIELDS = [
  "schema_version",
  "status",
  "submission_phase",
  "transaction_hash",
  "receipt",
  "output_artifact",
];
const PRIVATE_UPLOADED_MODEL_RECEIPT_FIELDS = [
  "schema_version",
  "network_id",
  "receipt_id",
  "service_name",
  "service_version",
  "model_id",
  "weight_version",
  "runtime_version",
  "model_manifest_digest",
  "model_bundle_root",
  "policy_id",
  "decryption_request_id",
  "attesting_validator",
  "input_artifact",
  "output_artifact",
  "output_replication_order_id",
  "input_commitment",
  "output_commitment",
  "output_recipient",
  "request_commitment",
  "result_commitment",
  "authorization_claim_block_height",
  "authorization_claim_epoch",
  "emitted_sequence",
  "emitted_block_height",
  "emitted_epoch",
];
const PRIVATE_UPLOADED_MODEL_ARTIFACT_FIELDS = [
  "schema_version",
  "sorafs_manifest_digest",
  "sorafs_root_cid",
  "artifact_hash",
  "ciphertext_bytes",
  "artifact_role",
];
const PRIVATE_UPLOADED_MODEL_RECIPIENT_FIELDS = [
  "schema_version",
  "key_id",
  "key_version",
  "kem",
  "aead",
  "public_key_bytes",
  "public_key_fingerprint",
];
const PRIVATE_UPLOADED_MODEL_STATUS_FIELDS = ["schema_version", "bundle", "artifact"];
const PRIVATE_UPLOADED_MODEL_BUNDLE_FIELDS = [
  "schema_version",
  "service_name",
  "model_id",
  "weight_version",
  "family",
  "modalities",
  "plaintext_root",
  "runtime_format",
  "bundle_root",
  "sorafs_manifest_digest",
  "chunk_count",
  "plaintext_bytes",
  "ciphertext_bytes",
  "chunk_manifest_root",
  "upload_recipient",
  "wrapped_bundle_key",
  "pricing_policy",
  "decryption_policy_ref",
];
const PRIVATE_UPLOADED_MODEL_WRAPPED_KEY_FIELDS = [
  "schema_version",
  "recipient_key_id",
  "recipient_key_version",
  "kem",
  "aead",
  "ephemeral_public_key",
  "nonce",
  "wrapped_key_ciphertext",
  "ciphertext_hash",
  "aad_digest",
];
const PRIVATE_UPLOADED_MODEL_ARTIFACT_STATUS_FIELDS = [
  "service_name",
  "model_name",
  "artifact_id",
  "training_job_id",
  "weight_version",
  "weight_artifact_hash",
  "dataset_ref",
  "training_config_hash",
  "reproducibility_hash",
  "provenance_attestation_hash",
  "registered_sequence",
  "consumed_by_version",
  "chunk_manifest_root",
];
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

function requireExactPrivateResponseObject(input, label, fields) {
  if (input == null || typeof input !== "object" || Array.isArray(input)) {
    throw new TypeError(`${label} must be an object`);
  }
  const prototype = Object.getPrototypeOf(input);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${label} inherited properties are not accepted`);
  }
  const expected = new Set(fields);
  for (const field of Object.getOwnPropertyNames(input)) {
    if (!expected.has(field)) {
      throw new TypeError(`${label}.${field} is not accepted`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(input, field);
    if (descriptor == null || descriptor.enumerable !== true || !("value" in descriptor)) {
      throw new TypeError(`${label}.${field} must be an enumerable data property`);
    }
  }
  if (Object.getOwnPropertySymbols(input).length > 0) {
    throw new TypeError(`${label} symbols are not accepted`);
  }
  for (const field of fields) {
    if (!Object.hasOwn(input, field)) {
      throw new TypeError(`${label}.${field} is required`);
    }
  }
  return input;
}

function requirePlainDensePrivateArray(value, field) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
    throw new TypeError(`${field} must be a plain array`);
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
    const descriptor = Object.getOwnPropertyDescriptor(value, property);
    if (descriptor == null || descriptor.enumerable !== true || !("value" in descriptor)) {
      throw new TypeError(`${field}[${property}] must be an enumerable data property`);
    }
  }
  for (let index = 0; index < value.length; index += 1) {
    if (!Object.hasOwn(value, index)) {
      throw new TypeError(`${field}[${index}] is required`);
    }
  }
  return value;
}

function normalizePrivateWireUnsignedInteger(
  value,
  field,
  { minimum = 0n, maximum = PRIVATE_UPLOADED_MODEL_U64_MAX } = {},
) {
  let integer;
  if (
    typeof value === "number"
    && Number.isSafeInteger(value)
    && !Object.is(value, -0)
  ) {
    integer = BigInt(value);
  } else if (typeof value === "bigint") {
    integer = value;
  } else {
    throw new TypeError(`${field} must be a lossless unsigned integer`);
  }
  if (integer < minimum || integer > maximum) {
    throw new RangeError(
      `${field} must be between ${minimum.toString(10)} and ${maximum.toString(10)}`,
    );
  }
  return integer <= PRIVATE_UPLOADED_MODEL_SAFE_INTEGER_MAX ? Number(integer) : integer;
}

function normalizePrivateSchemaVersionOne(value, field) {
  const version = normalizePrivateWireUnsignedInteger(value, field, {
    minimum: 1n,
    maximum: 1n,
  });
  return version;
}

function normalizePrivateWireU32(value, field, positive = false) {
  return normalizePrivateWireUnsignedInteger(value, field, {
    minimum: positive ? 1n : 0n,
    maximum: BigInt(PRIVATE_UPLOADED_MODEL_U32_MAX),
  });
}

function normalizePrivateWireU64(value, field, positive = false) {
  return normalizePrivateWireUnsignedInteger(value, field, {
    minimum: positive ? 1n : 0n,
  });
}

function privateUnsignedIsZero(value) {
  return typeof value === "bigint" ? value === 0n : value === 0;
}

function privateUnsignedIsPositive(value) {
  return typeof value === "bigint" ? value > 0n : value > 0;
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

function normalizePrivateWireUnitVariant(value, field, tag, expected) {
  const variant = requireExactPrivateResponseObject(value, field, [tag, "value"]);
  if (variant[tag] !== expected) {
    throw new TypeError(`${field}.${tag} must be ${expected}`);
  }
  if (variant.value !== null) {
    throw new TypeError(`${field}.value must be null`);
  }
  return Object.freeze({ [tag]: expected, value: null });
}

function normalizeCanonicalPrivateX25519Key(value, field) {
  const encoded = normalizePrivateExactString(value, field);
  if (!/^[A-Za-z0-9+/]{43}=$/u.test(encoded)) {
    throw new TypeError(`${field} must be canonical padded base64 for exactly 32 bytes`);
  }
  let decoded;
  try {
    decoded = strictDecodeBase64(encoded);
  } catch (error) {
    throw new TypeError(`${field} must be canonical padded base64 for exactly 32 bytes`, {
      cause: error,
    });
  }
  if (decoded.length !== 32) {
    throw new TypeError(`${field} must encode exactly 32 bytes`);
  }
  try {
    x25519.getSharedSecret(X25519_LOW_ORDER_PROBE_PRIVATE_KEY, decoded);
  } catch (error) {
    throw new TypeError(`${field} must not encode a low-order X25519 public key`, {
      cause: error,
    });
  }
  return { encoded, decoded };
}

function normalizeCanonicalPrivateBase64(value, field, minimumBytes, maximumBytes) {
  const encoded = normalizePrivateExactString(value, field);
  if (
    encoded.length % 4 !== 0
    || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(
      encoded,
    )
  ) {
    throw new TypeError(`${field} must be canonical padded base64`);
  }
  let decoded;
  try {
    decoded = strictDecodeBase64(encoded);
  } catch (error) {
    throw new TypeError(`${field} must be canonical padded base64`, { cause: error });
  }
  if (decoded.length < minimumBytes || decoded.length > maximumBytes) {
    throw new RangeError(
      `${field} must encode between ${minimumBytes} and ${maximumBytes} bytes`,
    );
  }
  return { encoded, decoded };
}

function normalizePrivateWireArtifact(value, field, expectedRole) {
  const artifact = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_ARTIFACT_FIELDS,
  );
  if (artifact.artifact_role !== expectedRole) {
    throw new TypeError(`${field}.artifact_role must be ${expectedRole}`);
  }
  const ciphertextBytes = normalizePrivateWireU64(
    artifact.ciphertext_bytes,
    `${field}.ciphertext_bytes`,
    true,
  );
  if (
    typeof ciphertextBytes !== "number"
    || ciphertextBytes > PRIVATE_UPLOADED_MODEL_MAX_CIPHERTEXT_BYTES
  ) {
    throw new RangeError(
      `${field}.ciphertext_bytes must be between 1 and ${PRIVATE_UPLOADED_MODEL_MAX_CIPHERTEXT_BYTES}`,
    );
  }
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      artifact.schema_version,
      `${field}.schema_version`,
    ),
    sorafs_manifest_digest: Object.freeze(
      normalizeManifestDigest(
        artifact.sorafs_manifest_digest,
        `${field}.sorafs_manifest_digest`,
      ),
    ),
    sorafs_root_cid: Object.freeze(
      normalizeManifestRootCid(artifact.sorafs_root_cid, `${field}.sorafs_root_cid`),
    ),
    artifact_hash: normalizeCanonicalHashLiteral(
      artifact.artifact_hash,
      `${field}.artifact_hash`,
    ),
    ciphertext_bytes: ciphertextBytes,
    artifact_role: expectedRole,
  });
}

function normalizePrivateWireRecipient(value, field) {
  const recipient = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_RECIPIENT_FIELDS,
  );
  const kem = normalizePrivateWireUnitVariant(
    recipient.kem,
    `${field}.kem`,
    "kem",
    "X25519HkdfSha256",
  );
  const aead = normalizePrivateWireUnitVariant(
    recipient.aead,
    `${field}.aead`,
    "aead",
    "Aes256Gcm",
  );
  const { encoded: publicKeyBytes, decoded } = normalizeCanonicalPrivateX25519Key(
    recipient.public_key_bytes,
    `${field}.public_key_bytes`,
  );
  const publicKeyFingerprint = normalizeCanonicalHashLiteral(
    recipient.public_key_fingerprint,
    `${field}.public_key_fingerprint`,
  );
  if (publicKeyFingerprint !== irohaBlake2b256PrehashLiteral(decoded)) {
    throw new TypeError(
      `${field}.public_key_fingerprint must equal the Iroha Blake2b-256 prehash of ${field}.public_key_bytes`,
    );
  }
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      recipient.schema_version,
      `${field}.schema_version`,
    ),
    key_id: normalizePrivateExactString(recipient.key_id, `${field}.key_id`),
    key_version: normalizePrivateWireU32(
      recipient.key_version,
      `${field}.key_version`,
      true,
    ),
    kem,
    aead,
    public_key_bytes: publicKeyBytes,
    public_key_fingerprint: publicKeyFingerprint,
  });
}

function encodePrivateUnsignedLeb128(value) {
  let remaining = BigInt(value);
  const bytes = [];
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) {
      byte |= 0x80;
    }
    bytes.push(byte);
  } while (remaining !== 0n);
  return bytes;
}

function privateHex(bytes, uppercase) {
  return Array.from(bytes, (byte) => {
    const hex = byte.toString(16).padStart(2, "0");
    return uppercase ? hex.toUpperCase() : hex;
  }).join("");
}

function normalizePrivateAttestingValidator(value, field) {
  const host = requireExactPrivateResponseObject(value, field, [
    "lane_id",
    "validator_account_id",
    "peer_id",
  ]);
  const validatorAccountId = normalizePrivateExactString(
    host.validator_account_id,
    `${field}.validator_account_id`,
  );
  let account;
  try {
    account = AccountAddress.fromI105(validatorAccountId);
  } catch (error) {
    throw new TypeError(
      `${field}.validator_account_id must be an exact canonical universal AccountId`,
      { cause: error },
    );
  }
  const accountBytes = account.canonicalBytes();
  const addressClass = (accountBytes[0] >> 3) & 0b11;
  if (
    addressClass !== 0
    || accountBytes.length < 4
    || accountBytes[1] !== 0
    || accountBytes.length !== 4 + accountBytes[3]
  ) {
    throw new TypeError(`${field}.validator_account_id must contain exactly one signatory`);
  }
  const curve = accountBytes[2];
  const publicKey = accountBytes.subarray(4);
  const multicodec = publicKeyMulticodecForCurveId(curve);
  if (multicodec === null) {
    throw new TypeError(`${field}.validator_account_id uses an unsupported signatory curve`);
  }
  const expectedPeerId = `${privateHex(encodePrivateUnsignedLeb128(multicodec), false)}${privateHex(
    encodePrivateUnsignedLeb128(publicKey.length),
    false,
  )}${privateHex(publicKey, true)}`;
  const peerId = normalizePrivateExactString(host.peer_id, `${field}.peer_id`);
  if (peerId !== expectedPeerId) {
    throw new TypeError(
      `${field}.peer_id must equal validator_account_id's exact single signatory`,
    );
  }
  return Object.freeze({
    lane_id: normalizePrivateWireU32(host.lane_id, `${field}.lane_id`),
    validator_account_id: validatorAccountId,
    peer_id: peerId,
  });
}

function normalizePrivateWrappedBundleKey(value, field, recipient) {
  const wrapped = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_WRAPPED_KEY_FIELDS,
  );
  const recipientKeyId = normalizePrivateExactString(
    wrapped.recipient_key_id,
    `${field}.recipient_key_id`,
  );
  const recipientKeyVersion = normalizePrivateWireU32(
    wrapped.recipient_key_version,
    `${field}.recipient_key_version`,
    true,
  );
  const kem = normalizePrivateWireUnitVariant(
    wrapped.kem,
    `${field}.kem`,
    "kem",
    "X25519HkdfSha256",
  );
  const aead = normalizePrivateWireUnitVariant(
    wrapped.aead,
    `${field}.aead`,
    "aead",
    "Aes256Gcm",
  );
  if (
    recipientKeyId !== recipient.key_id
    || recipientKeyVersion !== recipient.key_version
    || kem.kem !== recipient.kem.kem
    || aead.aead !== recipient.aead.aead
  ) {
    throw new TypeError(`${field} must bind the exact upload_recipient key and suites`);
  }
  const { encoded: ephemeralPublicKey } = normalizeCanonicalPrivateX25519Key(
    wrapped.ephemeral_public_key,
    `${field}.ephemeral_public_key`,
  );
  const { encoded: nonce } = normalizeCanonicalPrivateBase64(
    wrapped.nonce,
    `${field}.nonce`,
    1,
    256,
  );
  const { encoded: wrappedKeyCiphertext, decoded: ciphertextBytes } =
    normalizeCanonicalPrivateBase64(
      wrapped.wrapped_key_ciphertext,
      `${field}.wrapped_key_ciphertext`,
      1,
      4_096,
    );
  const ciphertextHash = normalizeCanonicalHashLiteral(
    wrapped.ciphertext_hash,
    `${field}.ciphertext_hash`,
  );
  if (ciphertextHash !== irohaBlake2b256PrehashLiteral(ciphertextBytes)) {
    throw new TypeError(`${field}.ciphertext_hash must match wrapped_key_ciphertext`);
  }
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      wrapped.schema_version,
      `${field}.schema_version`,
    ),
    recipient_key_id: recipientKeyId,
    recipient_key_version: recipientKeyVersion,
    kem,
    aead,
    ephemeral_public_key: ephemeralPublicKey,
    nonce,
    wrapped_key_ciphertext: wrappedKeyCiphertext,
    ciphertext_hash: ciphertextHash,
    aad_digest: normalizeCanonicalHashLiteral(wrapped.aad_digest, `${field}.aad_digest`),
  });
}

function normalizePrivateModalities(value, field) {
  const values = requirePlainDensePrivateArray(value, field);
  if (values.length === 0) {
    throw new TypeError(`${field} must not be empty`);
  }
  const modalities = values.map((entry, index) =>
    normalizePrivateExactString(entry, `${field}[${index}]`),
  );
  if (new Set(modalities).size !== modalities.length) {
    throw new TypeError(`${field} entries must be unique`);
  }
  return Object.freeze(modalities);
}

function normalizePrivatePricingPolicy(value, field) {
  const pricing = requireExactPrivateResponseObject(value, field, ["storage_price"]);
  const storagePrice = normalizePrivateExactString(
    pricing.storage_price,
    `${field}.storage_price`,
  );
  let canonical;
  try {
    canonical = new KotodamaQuantity(storagePrice).toString();
  } catch (error) {
    throw new TypeError(`${field}.storage_price must be a canonical quantity`, {
      cause: error,
    });
  }
  if (canonical !== storagePrice) {
    throw new TypeError(`${field}.storage_price must be a canonical quantity`);
  }
  return Object.freeze({ storage_price: storagePrice });
}

function normalizePrivateUploadedModelBundle(value, field) {
  const bundle = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_BUNDLE_FIELDS,
  );
  const uploadRecipient = normalizePrivateWireRecipient(
    bundle.upload_recipient,
    `${field}.upload_recipient`,
  );
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      bundle.schema_version,
      `${field}.schema_version`,
    ),
    service_name: normalizePrivateName(bundle.service_name, `${field}.service_name`),
    model_id: normalizePrivateIdentifier(bundle.model_id, `${field}.model_id`),
    weight_version: normalizePrivateIdentifier(
      bundle.weight_version,
      `${field}.weight_version`,
    ),
    family: normalizePrivateExactString(bundle.family, `${field}.family`),
    modalities: normalizePrivateModalities(bundle.modalities, `${field}.modalities`),
    plaintext_root: normalizeCanonicalHashLiteral(
      bundle.plaintext_root,
      `${field}.plaintext_root`,
    ),
    runtime_format: normalizePrivateWireUnitVariant(
      bundle.runtime_format,
      `${field}.runtime_format`,
      "runtime_format",
      "DeterministicQuantizedCpuV1",
    ),
    bundle_root: normalizeCanonicalHashLiteral(bundle.bundle_root, `${field}.bundle_root`),
    sorafs_manifest_digest: Object.freeze(
      normalizeManifestDigest(
        bundle.sorafs_manifest_digest,
        `${field}.sorafs_manifest_digest`,
      ),
    ),
    chunk_count: normalizePrivateWireU32(bundle.chunk_count, `${field}.chunk_count`, true),
    plaintext_bytes: normalizePrivateWireU64(
      bundle.plaintext_bytes,
      `${field}.plaintext_bytes`,
      true,
    ),
    ciphertext_bytes: normalizePrivateWireU64(
      bundle.ciphertext_bytes,
      `${field}.ciphertext_bytes`,
      true,
    ),
    chunk_manifest_root: normalizeCanonicalHashLiteral(
      bundle.chunk_manifest_root,
      `${field}.chunk_manifest_root`,
    ),
    upload_recipient: uploadRecipient,
    wrapped_bundle_key: normalizePrivateWrappedBundleKey(
      bundle.wrapped_bundle_key,
      `${field}.wrapped_bundle_key`,
      uploadRecipient,
    ),
    pricing_policy: normalizePrivatePricingPolicy(
      bundle.pricing_policy,
      `${field}.pricing_policy`,
    ),
    decryption_policy_ref: normalizePrivateExactString(
      bundle.decryption_policy_ref,
      `${field}.decryption_policy_ref`,
    ),
  });
}

function normalizeNullablePrivateExactString(value, field, normalizer = normalizePrivateExactString) {
  return value === null ? null : normalizer(value, field);
}

function normalizeNullablePrivateHash(value, field) {
  return value === null ? null : normalizeCanonicalHashLiteral(value, field);
}

function normalizePrivateArtifactStatus(value, field) {
  const artifact = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_ARTIFACT_STATUS_FIELDS,
  );
  return Object.freeze({
    service_name: normalizePrivateName(artifact.service_name, `${field}.service_name`),
    model_name: normalizePrivateExactString(artifact.model_name, `${field}.model_name`),
    artifact_id: normalizePrivateExactString(artifact.artifact_id, `${field}.artifact_id`),
    training_job_id: normalizePrivateExactString(
      artifact.training_job_id,
      `${field}.training_job_id`,
    ),
    weight_version: normalizeNullablePrivateExactString(
      artifact.weight_version,
      `${field}.weight_version`,
      normalizePrivateIdentifier,
    ),
    weight_artifact_hash: normalizeCanonicalHashLiteral(
      artifact.weight_artifact_hash,
      `${field}.weight_artifact_hash`,
    ),
    dataset_ref: normalizePrivateExactString(artifact.dataset_ref, `${field}.dataset_ref`),
    training_config_hash: normalizeCanonicalHashLiteral(
      artifact.training_config_hash,
      `${field}.training_config_hash`,
    ),
    reproducibility_hash: normalizeCanonicalHashLiteral(
      artifact.reproducibility_hash,
      `${field}.reproducibility_hash`,
    ),
    provenance_attestation_hash: normalizeCanonicalHashLiteral(
      artifact.provenance_attestation_hash,
      `${field}.provenance_attestation_hash`,
    ),
    registered_sequence: normalizePrivateWireU64(
      artifact.registered_sequence,
      `${field}.registered_sequence`,
      true,
    ),
    consumed_by_version: normalizeNullablePrivateExactString(
      artifact.consumed_by_version,
      `${field}.consumed_by_version`,
    ),
    chunk_manifest_root: normalizeNullablePrivateHash(
      artifact.chunk_manifest_root,
      `${field}.chunk_manifest_root`,
    ),
  });
}

function normalizePrivateUploadedModelStatus(value, field) {
  const status = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_STATUS_FIELDS,
  );
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      status.schema_version,
      `${field}.schema_version`,
    ),
    bundle: normalizePrivateUploadedModelBundle(status.bundle, `${field}.bundle`),
    artifact: status.artifact === null
      ? null
      : normalizePrivateArtifactStatus(status.artifact, `${field}.artifact`),
  });
}

function derivePrivateAutomaticReplicationOrderId(outputManifestDigest) {
  const preimage = new Uint8Array(
    PRIVATE_UPLOADED_MODEL_AUTO_ORDER_DOMAIN_V1.length + outputManifestDigest.length,
  );
  preimage.set(PRIVATE_UPLOADED_MODEL_AUTO_ORDER_DOMAIN_V1);
  preimage.set(outputManifestDigest, PRIVATE_UPLOADED_MODEL_AUTO_ORDER_DOMAIN_V1.length);
  const orderId = Array.from(blake3(preimage));
  orderId[0] |= 0x80;
  return orderId;
}

function privateByteArraysEqual(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function privateArtifactsEqual(left, right) {
  return left.schema_version === right.schema_version
    && privateByteArraysEqual(left.sorafs_manifest_digest, right.sorafs_manifest_digest)
    && privateByteArraysEqual(left.sorafs_root_cid, right.sorafs_root_cid)
    && left.artifact_hash === right.artifact_hash
    && left.ciphertext_bytes === right.ciphertext_bytes
    && left.artifact_role === right.artifact_role;
}

function normalizePrivateNetworkId(value, field) {
  const literal = normalizePrivateExactString(value, field);
  try {
    return NetworkId.parse(literal).toString();
  } catch (error) {
    throw new TypeError(
      `${field} must be an exact canonical checksummed 32-byte NetworkId literal`,
      { cause: error },
    );
  }
}

function normalizePrivateUploadedModelReceipt(value, field) {
  const receipt = requireExactPrivateResponseObject(
    value,
    field,
    PRIVATE_UPLOADED_MODEL_RECEIPT_FIELDS,
  );
  const inputArtifact = normalizePrivateWireArtifact(
    receipt.input_artifact,
    `${field}.input_artifact`,
    "input",
  );
  const outputArtifact = normalizePrivateWireArtifact(
    receipt.output_artifact,
    `${field}.output_artifact`,
    "output",
  );
  if (inputArtifact.artifact_hash === outputArtifact.artifact_hash) {
    throw new TypeError(
      `${field}.output_artifact.artifact_hash must differ from input_artifact.artifact_hash`,
    );
  }
  const outputReplicationOrderId = Object.freeze(
    normalizeManifestDigest(
      receipt.output_replication_order_id,
      `${field}.output_replication_order_id`,
    ),
  );
  const expectedOrderId = derivePrivateAutomaticReplicationOrderId(
    outputArtifact.sorafs_manifest_digest,
  );
  if (!privateByteArraysEqual(outputReplicationOrderId, expectedOrderId)) {
    throw new TypeError(
      `${field}.output_replication_order_id must equal the tagged automatic replication-order ID derived from output_artifact.sorafs_manifest_digest`,
    );
  }
  const emittedSequence = normalizePrivateWireU64(
    receipt.emitted_sequence,
    `${field}.emitted_sequence`,
  );
  const emittedBlockHeight = normalizePrivateWireU64(
    receipt.emitted_block_height,
    `${field}.emitted_block_height`,
  );
  const emittedEpoch = normalizePrivateWireU64(
    receipt.emitted_epoch,
    `${field}.emitted_epoch`,
  );
  const authorizationClaimBlockHeight = normalizePrivateWireU64(
    receipt.authorization_claim_block_height,
    `${field}.authorization_claim_block_height`,
  );
  const authorizationClaimEpoch = normalizePrivateWireU64(
    receipt.authorization_claim_epoch,
    `${field}.authorization_claim_epoch`,
  );
  const ledgerCoordinates = [
    authorizationClaimBlockHeight,
    authorizationClaimEpoch,
    emittedSequence,
    emittedBlockHeight,
    emittedEpoch,
  ];
  const coordinatesAreZero = ledgerCoordinates.every(privateUnsignedIsZero);
  const coordinatesArePositive = ledgerCoordinates.every(privateUnsignedIsPositive);
  if (!coordinatesAreZero && !coordinatesArePositive) {
    throw new TypeError(
      `${field} ledger coordinates must all be zero or all be positive`,
    );
  }
  if (
    coordinatesArePositive
    && (
      BigInt(emittedBlockHeight) < BigInt(authorizationClaimBlockHeight)
      || BigInt(emittedEpoch) < BigInt(authorizationClaimEpoch)
    )
  ) {
    throw new TypeError(
      `${field} emission coordinates must not precede authorization claim coordinates`,
    );
  }
  const runtimeVersion = normalizePrivateExactString(
    receipt.runtime_version,
    `${field}.runtime_version`,
  );
  if (runtimeVersion !== PRIVATE_UPLOADED_MODEL_RUNTIME_VERSION_V1) {
    throw new TypeError(
      `${field}.runtime_version must be ${PRIVATE_UPLOADED_MODEL_RUNTIME_VERSION_V1}`,
    );
  }
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      receipt.schema_version,
      `${field}.schema_version`,
    ),
    network_id: normalizePrivateNetworkId(receipt.network_id, `${field}.network_id`),
    receipt_id: normalizeCanonicalHashLiteral(receipt.receipt_id, `${field}.receipt_id`),
    service_name: normalizePrivateName(receipt.service_name, `${field}.service_name`),
    service_version: normalizePrivateExactString(
      receipt.service_version,
      `${field}.service_version`,
      256,
    ),
    model_id: normalizePrivateIdentifier(receipt.model_id, `${field}.model_id`),
    weight_version: normalizePrivateIdentifier(
      receipt.weight_version,
      `${field}.weight_version`,
    ),
    runtime_version: runtimeVersion,
    model_manifest_digest: Object.freeze(
      normalizeManifestDigest(
        receipt.model_manifest_digest,
        `${field}.model_manifest_digest`,
      ),
    ),
    model_bundle_root: normalizeCanonicalHashLiteral(
      receipt.model_bundle_root,
      `${field}.model_bundle_root`,
    ),
    policy_id: normalizePrivateExactString(receipt.policy_id, `${field}.policy_id`),
    decryption_request_id: normalizePrivateExactString(
      receipt.decryption_request_id,
      `${field}.decryption_request_id`,
    ),
    attesting_validator: normalizePrivateAttestingValidator(
      receipt.attesting_validator,
      `${field}.attesting_validator`,
    ),
    input_artifact: inputArtifact,
    output_artifact: outputArtifact,
    output_replication_order_id: outputReplicationOrderId,
    input_commitment: normalizeCanonicalHashLiteral(
      receipt.input_commitment,
      `${field}.input_commitment`,
    ),
    output_commitment: normalizeCanonicalHashLiteral(
      receipt.output_commitment,
      `${field}.output_commitment`,
    ),
    output_recipient: normalizePrivateWireRecipient(
      receipt.output_recipient,
      `${field}.output_recipient`,
    ),
    request_commitment: normalizeCanonicalHashLiteral(
      receipt.request_commitment,
      `${field}.request_commitment`,
    ),
    result_commitment: normalizeCanonicalHashLiteral(
      receipt.result_commitment,
      `${field}.result_commitment`,
    ),
    authorization_claim_block_height: authorizationClaimBlockHeight,
    authorization_claim_epoch: authorizationClaimEpoch,
    emitted_sequence: emittedSequence,
    emitted_block_height: emittedBlockHeight,
    emitted_epoch: emittedEpoch,
  });
}

function requirePrivateStatusMatchesReceipt(status, receipt, field) {
  const { bundle, artifact } = status;
  for (const [name, expected] of [
    ["service_name", receipt.service_name],
    ["model_id", receipt.model_id],
    ["weight_version", receipt.weight_version],
    ["bundle_root", receipt.model_bundle_root],
    ["decryption_policy_ref", receipt.policy_id],
  ]) {
    if (bundle[name] !== expected) {
      throw new TypeError(`${field}.bundle.${name} must match receipt`);
    }
  }
  if (!privateByteArraysEqual(bundle.sorafs_manifest_digest, receipt.model_manifest_digest)) {
    throw new TypeError(
      `${field}.bundle.sorafs_manifest_digest must match receipt.model_manifest_digest`,
    );
  }
  if (artifact !== null) {
    if (artifact.service_name !== receipt.service_name) {
      throw new TypeError(`${field}.artifact.service_name must match receipt.service_name`);
    }
    if (artifact.weight_version !== receipt.weight_version) {
      throw new TypeError(`${field}.artifact.weight_version must match receipt.weight_version`);
    }
    if (artifact.chunk_manifest_root !== bundle.chunk_manifest_root) {
      throw new TypeError(
        `${field}.artifact.chunk_manifest_root must match bundle.chunk_manifest_root`,
      );
    }
  }
}

/**
 * Strictly normalize one private uploaded-model execution receipt decoded with lossless JSON.
 * Unsafe JSON integer tokens must reach this function as `bigint`, never rounded `number` values.
 *
 * @param {unknown} payload decoded Norito JSON receipt.
 * @returns {Readonly<Record<string, unknown>>}
 */
export function normalizeSoracloudPrivateUploadedModelExecutionReceipt(payload) {
  return normalizePrivateUploadedModelReceipt(
    payload,
    "soracloud private uploaded-model execution receipt",
  );
}

/**
 * Strictly normalize `/v1/soracloud/model/upload/private/execute` response JSON.
 * Unsafe JSON integer tokens must reach this function as `bigint`, never rounded `number` values.
 *
 * @param {unknown} payload decoded Norito JSON response.
 * @returns {Readonly<Record<string, unknown>>}
 */
export function normalizeSoracloudPrivateUploadedModelExecuteResponse(payload) {
  const field = "soracloud private uploaded-model execute response";
  const response = requireExactPrivateResponseObject(
    payload,
    field,
    PRIVATE_UPLOADED_MODEL_EXECUTE_RESPONSE_FIELDS,
  );
  const phase = normalizePrivateExactString(
    response.submission_phase,
    `${field}.submission_phase`,
  );
  if (!PRIVATE_UPLOADED_MODEL_SUBMISSION_PHASES.has(phase)) {
    throw new TypeError(`${field}.submission_phase is not a closed first-release phase`);
  }
  const transactionHash = response.transaction_hash === null
    ? null
    : normalizeCanonicalHashLiteral(
        response.transaction_hash,
        `${field}.transaction_hash`,
      );
  const hashRequired = phase === "prepare_submitted" || phase === "receipt_submitted";
  if (hashRequired !== (transactionHash !== null)) {
    throw new TypeError(
      hashRequired
        ? `${field}.transaction_hash is required for ${phase}`
        : `${field}.transaction_hash must be null for ${phase}`,
    );
  }
  const receipt = normalizePrivateUploadedModelReceipt(response.receipt, `${field}.receipt`);
  const assignedReceipt =
    privateUnsignedIsPositive(receipt.authorization_claim_block_height)
    && privateUnsignedIsPositive(receipt.authorization_claim_epoch)
    && privateUnsignedIsPositive(receipt.emitted_sequence)
    && privateUnsignedIsPositive(receipt.emitted_block_height)
    && privateUnsignedIsPositive(receipt.emitted_epoch);
  if ((phase === "committed") !== assignedReceipt) {
    throw new TypeError(
      phase === "committed"
        ? `${field}.receipt must use positive ledger coordinates for committed`
        : `${field}.receipt must use zero ledger coordinates for ${phase}`,
    );
  }
  const outputArtifact = normalizePrivateWireArtifact(
    response.output_artifact,
    `${field}.output_artifact`,
    "output",
  );
  if (!privateArtifactsEqual(outputArtifact, receipt.output_artifact)) {
    throw new TypeError(`${field}.output_artifact must match receipt.output_artifact`);
  }
  const status = normalizePrivateUploadedModelStatus(response.status, `${field}.status`);
  requirePrivateStatusMatchesReceipt(status, receipt, `${field}.status`);
  return Object.freeze({
    schema_version: normalizePrivateSchemaVersionOne(
      response.schema_version,
      `${field}.schema_version`,
    ),
    status,
    submission_phase: phase,
    transaction_hash: transactionHash,
    receipt,
    output_artifact: outputArtifact,
  });
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
 * The immutable model id and bundle root are required. The request binds an exact service
 * revision, committed decryption authorization, encrypted SoraFS input, and
 * output-recipient public key plus its Iroha prehash. It never accepts model
 * bytes, plaintext, validator-created output claims, runtime claims, or signing
 * secrets.
 *
 * @param {{ serviceName: string, serviceVersion: string, weightVersion: string, modelId: string, bundleRoot: string, decryptionRequestId: string, inputArtifact: Record<string, unknown>, outputRecipient: Record<string, unknown> }} input
 * @returns {Record<string, unknown>}
 */
export function buildSoracloudPrivateUploadedModelExecuteRequest(input = {}) {
  rejectSoracloudSigningSecrets(input);
  requireExactObject(input, "input", [
    "serviceName",
    "serviceVersion",
    "weightVersion",
    "modelId",
    "bundleRoot",
    "decryptionRequestId",
    "inputArtifact",
    "outputRecipient",
  ]);
  const modelId = normalizePrivateIdentifier(input.modelId, "modelId");
  const bundleRoot = normalizeCanonicalHashLiteral(input.bundleRoot, "bundleRoot");
  return {
    service_name: normalizePrivateName(input.serviceName, "serviceName"),
    service_version: normalizePrivateExactString(
      input.serviceVersion,
      "serviceVersion",
      256,
    ),
    weight_version: normalizePrivateIdentifier(input.weightVersion, "weightVersion"),
    model_id: modelId,
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
