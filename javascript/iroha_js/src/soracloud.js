import { parseHashLiteral } from "./instructionBuilderPrimitives.js";
import { getCurveEntryByPublicKeyMulticodec } from "./curveRegistry.js";
import {
  canonicalizeMultihashHex,
  ensureCanonicalAccountId,
  normalizeAssetDefinitionId,
} from "./normalizers.js";
import {
  KotodamaQuantity,
  NumericV1,
  NumericV1Error,
} from "./numericV1.js";

const HF_COMMIT_OID_PATTERN_V1 = /^[0-9a-f]{40}$/;
const HF_REPO_ID_MAX_BYTES_V1 = 96;
const IROHA_NAME_MAX_BYTES_V1 = 255;
const REJECTED_SIGNING_SECRET_FIELDS = [
  "privateKeyHex",
  "privateKey",
  "private_key",
  "private_key_hex",
];
const PROVENANCE_SCHEMA = "soracloud.hf.shared_lease_join.provenance.v1";
const APP_INFRA_PROVENANCE_SCHEMA = "soracloud.app.infra.provenance.v1";
const HF_SHARED_LEASE_JOIN_PAYLOAD_FIELDS = new Set([
  "repo_id",
  "revision",
  "service_name",
  "apartment_name",
  "storage_class",
  "lease_term_ms",
  "lease_asset_definition_id",
  "base_fee",
]);
const TX_INSTRUCTION_FIELDS = ["wire_id", "payload_hex"];
const MUTATION_DRAFT_RESPONSE_FIELDS = [
  "ok",
  "authority",
  "signed_by",
  "tx_instructions",
];
const APP_INFRA_STATUS_RESPONSE_FIELDS = [
  "schema_version",
  "app_count",
  "audit_event_count",
  "apps",
  "recent_audit_events",
];
const APP_INFRA_STATE_FIELDS = [
  "schema_version",
  "app_name",
  "current_app_version",
  "current_manifest_hash",
  "revision_count",
  "deployed_sequence",
  "updated_sequence",
  "manifest",
];
const APP_INFRA_AUDIT_EVENT_FIELDS = [
  "schema_version",
  "sequence",
  "action",
  "app_name",
  "from_version",
  "to_version",
  "app_manifest_hash",
  "service_count",
  "signer",
];
const APP_INFRA_MANIFEST_FIELDS = [
  "schema_version",
  "app_name",
  "app_version",
  "public_url",
  "static_site",
  "services",
];
const APP_INFRA_STATIC_SITE_FIELDS = [
  "schema_version",
  "public_url",
  "content_cid",
  "manifest_digest_hex",
  "mount_path",
  "api_base_path",
];
const APP_INFRA_SERVICE_FIELDS = [
  "schema_version",
  "service_name",
  "service_version",
  "service_manifest_hash",
  "container_manifest_hash",
  "execution_plane",
  "runtime",
  "routes",
  "lease_volumes",
  "shard",
];
const APP_INFRA_ROUTE_FIELDS = [
  "schema_version",
  "public_host",
  "path_prefix",
  "internal_url",
];
const LOWERCASE_EVEN_HEX_PATTERN = /^(?:[0-9a-f]{2})+$/;
export const SORACLOUD_APP_INFRA_DEPLOY_WIRE_ID =
  "iroha.instruction.v1::soracloud::DeploySoracloudAppInfra";
export const SORACLOUD_APP_INFRA_UPGRADE_WIRE_ID =
  "iroha.instruction.v1::soracloud::UpgradeSoracloudAppInfra";
const JSON_ACCEPT_HEADERS = Object.freeze({ Accept: "application/json" });
const SORACLOUD_JSON_RESPONSE_MAX_BYTES = 64 * 1024 * 1024;

/**
 * Decode one Soracloud V1 response without accepting media-type or body aliases.
 *
 * @param {object} client Torii client transport.
 * @param {Response} response Fetch response.
 * @param {string} context Operation name for diagnostics.
 * @param {AbortSignal | undefined} signal Optional request signal.
 * @returns {Promise<unknown>}
 */
export async function decodeExactSoracloudJsonResponse(
  client,
  response,
  context,
  signal = undefined,
) {
  const contentType = client._getHeader(response, "content-type");
  if (contentType !== "application/json") {
    throw new TypeError(
      `${context} response Content-Type must be exactly application/json`,
    );
  }
  return client._maybeBoundedJson(
    response,
    SORACLOUD_JSON_RESPONSE_MAX_BYTES,
    `${context} response`,
    { signal },
  );
}

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
    const descriptor = Object.getOwnPropertyDescriptor(input, field);
    if (!descriptor?.enumerable) {
      throw new TypeError(`${label}.${field} must be enumerable`);
    }
    if (!Object.prototype.hasOwnProperty.call(descriptor, "value")) {
      throw new TypeError(`${label}.${field} must be an enumerable data field`);
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
  if (value.trim() !== value) {
    throw new TypeError(`${field} must not contain surrounding whitespace`);
  }
  return value;
}

function requireCanonicalStringValue(value, label) {
  if (
    typeof value !== "string" ||
    value === "" ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new TypeError(`${label} must be a canonical non-empty string`);
  }
  return value;
}

function requireSafeNonNegativeIntegerValue(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${label} must be a safe non-negative integer`);
  }
  return value;
}

function requireCanonicalHashValue(value, label) {
  if (typeof value !== "string") {
    throw new TypeError(`${label} must be a canonical Norito hash literal`);
  }
  let canonical;
  try {
    canonical = parseHashLiteral(value, label);
  } catch {
    throw new TypeError(`${label} must be a canonical Norito hash literal`);
  }
  if (canonical !== value) {
    throw new TypeError(`${label} must be a canonical Norito hash literal`);
  }
  return value;
}

function nullableString(input, field, label = field) {
  const value = input[field];
  if (value === null) {
    return null;
  }
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${label} must be a non-empty string or null`);
  }
  if (value.trim() !== value) {
    throw new TypeError(`${label} must not contain surrounding whitespace`);
  }
  return value;
}

function utf8ByteLength(value) {
  return new TextEncoder().encode(value).length;
}

function requireCanonicalHfRepoIdValue(value, label) {
  requireCanonicalStringValue(value, label);
  if (utf8ByteLength(value) > HF_REPO_ID_MAX_BYTES_V1 || value.includes("--") || value.includes("..")) {
    throw new TypeError(`${label} must be one exact fully-qualified namespace/repository identifier`);
  }
  if (value.slice(-4).toLowerCase() === ".git") {
    throw new TypeError(`${label} must be one exact fully-qualified namespace/repository identifier`);
  }
  const components = value.split("/");
  if (
    components.length !== 2 ||
    components.some(
      (component) =>
        component === "" ||
        component.startsWith(".") ||
        component.startsWith("-") ||
        component.endsWith(".") ||
        component.endsWith("-") ||
        !/^[A-Za-z0-9._-]+$/u.test(component),
    )
  ) {
    throw new TypeError(`${label} must be one exact fully-qualified namespace/repository identifier`);
  }
  return value;
}

function requireCanonicalIrohaNameValue(value, label) {
  requireCanonicalStringValue(value, label);
  if (
    utf8ByteLength(value) > IROHA_NAME_MAX_BYTES_V1 ||
    value.normalize("NFC") !== value ||
    /[\p{Cc}\p{White_Space}@#$]/u.test(value) ||
    /[\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(value)
  ) {
    throw new TypeError(`${label} must use its exact canonical Iroha Name spelling`);
  }
  return value;
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

function normalizeQuantity(input, field) {
  const value = input[field];
  try {
    let normalized;
    if (value instanceof KotodamaQuantity) {
      normalized = NumericV1.encodeQuantityJson(value);
    } else if (typeof value === "string") {
      normalized = NumericV1.decodeQuantityJson(value).toString();
    } else if (typeof value === "bigint") {
      normalized = new KotodamaQuantity(value, 0).toString();
    } else {
      throw new TypeError(
        `${field} must be a KotodamaQuantity, canonical Quantity string, or bigint`,
      );
    }
    if (normalized === "0") {
      throw new TypeError(`${field} must be greater than zero`);
    }
    return normalized;
  } catch (error) {
    if (error instanceof NumericV1Error) {
      throw new TypeError(
        `${field} must be a canonical positive Quantity (${error.code})`,
      );
    }
    throw error;
  }
}

function normalizeSafeIntegerValue(value, field) {
  const normalized = normalizeIntegerStringValue(value, field);
  const numeric = Number(normalized);
  if (!Number.isSafeInteger(numeric)) {
    throw new TypeError(`${field} must fit in a safe JavaScript integer`);
  }
  return numeric;
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

function requireSchemaVersionOne(value, label) {
  if (value !== 1) {
    throw new TypeError(`${label} must be 1`);
  }
  return value;
}

function requireU32Value(value, label, { positive = false } = {}) {
  const normalized = requireSafeNonNegativeIntegerValue(value, label);
  if (normalized > 0xffff_ffff || (positive && normalized === 0)) {
    throw new TypeError(
      `${label} must be ${positive ? "a positive" : "an unsigned"} 32-bit integer`,
    );
  }
  return normalized;
}

function requirePositiveSafeIntegerValue(value, label) {
  const normalized = requireSafeNonNegativeIntegerValue(value, label);
  if (normalized === 0) {
    throw new TypeError(`${label} must be greater than zero`);
  }
  return normalized;
}

function requireNullableCanonicalStringValue(value, label) {
  if (value === null) {
    return null;
  }
  return requireCanonicalStringValue(value, label);
}

function requireHttpUrlValue(value, label) {
  const normalized = requireCanonicalStringValue(value, label);
  if (!normalized.startsWith("http://") && !normalized.startsWith("https://")) {
    throw new TypeError(`${label} must start with http:// or https://`);
  }
  return normalized;
}

function requireAbsolutePathValue(value, label) {
  const normalized = requireCanonicalStringValue(value, label);
  if (!normalized.startsWith("/")) {
    throw new TypeError(`${label} must start with /`);
  }
  return normalized;
}

function decodeCanonicalPublicKeyVarint(bytes, start, label) {
  let value = 0n;
  let shift = 0n;
  for (let cursor = start; cursor < bytes.length && cursor - start < 10; cursor += 1) {
    const byte = BigInt(bytes[cursor]);
    value |= (byte & 0x7fn) << shift;
    if ((byte & 0x80n) === 0n) {
      if (cursor > start && byte === 0n) {
        throw new TypeError(`${label} contains a non-canonical multihash varint`);
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new TypeError(`${label} contains an oversized multihash varint`);
      }
      return { value: Number(value), nextIndex: cursor + 1 };
    }
    shift += 7n;
  }
  throw new TypeError(`${label} contains an invalid multihash varint`);
}

function requireCanonicalPublicKeyValue(value, label) {
  const literal = requireCanonicalStringValue(value, label);
  if (literal.includes(":")) {
    throw new TypeError(`${label} must not include an algorithm prefix`);
  }
  let canonical;
  try {
    canonical = canonicalizeMultihashHex(literal, label);
  } catch {
    throw new TypeError(`${label} must be a canonical public-key multihash literal`);
  }
  const bytePairs = canonical.match(/../gu);
  const bytes = Uint8Array.from(bytePairs ?? [], (pair) => Number.parseInt(pair, 16));
  const multicodec = decodeCanonicalPublicKeyVarint(bytes, 0, label);
  const length = decodeCanonicalPublicKeyVarint(bytes, multicodec.nextIndex, label);
  const entry = getCurveEntryByPublicKeyMulticodec(multicodec.value);
  if (
    entry == null ||
    length.value !== entry.publicKeyLength ||
    bytes.length - length.nextIndex !== entry.publicKeyLength
  ) {
    throw new TypeError(`${label} must use a supported public-key multihash`);
  }
  const headerEnd = length.nextIndex * 2;
  const expected = `${canonical.slice(0, headerEnd).toLowerCase()}${canonical.slice(headerEnd)}`;
  if (literal !== expected) {
    throw new TypeError(`${label} must use canonical public-key spelling`);
  }
  return literal;
}

function requireCanonicalAccountIdValue(value, label) {
  const literal = requireCanonicalStringValue(value, label);
  let canonical;
  try {
    canonical = ensureCanonicalAccountId(literal, label);
  } catch {
    throw new TypeError(`${label} must be a canonical I105 account id`);
  }
  if (canonical !== literal) {
    throw new TypeError(`${label} must be a canonical I105 account id`);
  }
  return literal;
}

function requireTaggedUnitChoice(value, label, tagField, allowedTags) {
  requireExactObject(value, label, [tagField, "value"]);
  if (!allowedTags.has(value[tagField]) || value.value !== null) {
    throw new TypeError(
      `${label} must be one of ${[...allowedTags].join(", ")} with an explicit null value`,
    );
  }
  return value[tagField];
}

function validateAppInfraRoute(route, label) {
  requireExactObject(route, label, APP_INFRA_ROUTE_FIELDS);
  requireSchemaVersionOne(route.schema_version, `${label}.schema_version`);
  requireNullableCanonicalStringValue(route.public_host, `${label}.public_host`);
  requireAbsolutePathValue(route.path_prefix, `${label}.path_prefix`);
  requireNullableCanonicalStringValue(route.internal_url, `${label}.internal_url`);
}

function validateAppInfraStaticSite(site, label) {
  requireExactObject(site, label, APP_INFRA_STATIC_SITE_FIELDS);
  requireSchemaVersionOne(site.schema_version, `${label}.schema_version`);
  requireHttpUrlValue(site.public_url, `${label}.public_url`);
  requireNullableCanonicalStringValue(site.content_cid, `${label}.content_cid`);
  requireNullableCanonicalStringValue(
    site.manifest_digest_hex,
    `${label}.manifest_digest_hex`,
  );
  requireAbsolutePathValue(site.mount_path, `${label}.mount_path`);
  if (site.api_base_path !== null) {
    requireAbsolutePathValue(site.api_base_path, `${label}.api_base_path`);
  }
}

function validateAppInfraService(service, label) {
  requireExactObject(service, label, APP_INFRA_SERVICE_FIELDS);
  requireSchemaVersionOne(service.schema_version, `${label}.schema_version`);
  requireCanonicalStringValue(service.service_name, `${label}.service_name`);
  requireCanonicalStringValue(service.service_version, `${label}.service_version`);
  requireCanonicalHashValue(service.service_manifest_hash, `${label}.service_manifest_hash`);
  requireCanonicalHashValue(
    service.container_manifest_hash,
    `${label}.container_manifest_hash`,
  );
  requireTaggedUnitChoice(
    service.execution_plane,
    `${label}.execution_plane`,
    "execution_plane",
    new Set(["DeterministicService", "HttpService"]),
  );
  requireTaggedUnitChoice(
    service.runtime,
    `${label}.runtime`,
    "runtime",
    new Set(["Ivm", "Inrou"]),
  );
  if (!Array.isArray(service.routes)) {
    throw new TypeError(`${label}.routes must be an array`);
  }
  const routeKeys = new Set();
  service.routes.forEach((route, index) => {
    const routeLabel = `${label}.routes[${index}]`;
    validateAppInfraRoute(route, routeLabel);
    const routeKey = `${route.public_host ?? ""}\u0000${route.path_prefix}`;
    if (routeKeys.has(routeKey)) {
      throw new TypeError(`${label}.routes must not contain duplicates`);
    }
    routeKeys.add(routeKey);
  });
  if (!Array.isArray(service.lease_volumes)) {
    throw new TypeError(`${label}.lease_volumes must be an array`);
  }
  const leaseVolumes = service.lease_volumes.map((volume, index) =>
    requireCanonicalStringValue(volume, `${label}.lease_volumes[${index}]`),
  );
  if (new Set(leaseVolumes).size !== leaseVolumes.length) {
    throw new TypeError(`${label}.lease_volumes must not contain duplicates`);
  }
  requireNullableCanonicalStringValue(service.shard, `${label}.shard`);
}

function validateAppInfraManifest(manifest, label) {
  requireExactObject(manifest, label, APP_INFRA_MANIFEST_FIELDS);
  requireSchemaVersionOne(manifest.schema_version, `${label}.schema_version`);
  requireCanonicalStringValue(manifest.app_name, `${label}.app_name`);
  requireCanonicalStringValue(manifest.app_version, `${label}.app_version`);
  requireHttpUrlValue(manifest.public_url, `${label}.public_url`);
  if (manifest.static_site !== null) {
    validateAppInfraStaticSite(manifest.static_site, `${label}.static_site`);
  }
  if (!Array.isArray(manifest.services) || manifest.services.length === 0) {
    throw new TypeError(`${label}.services must contain at least one service`);
  }
  const serviceNames = new Set();
  manifest.services.forEach((service, index) => {
    validateAppInfraService(service, `${label}.services[${index}]`);
    if (serviceNames.has(service.service_name)) {
      throw new TypeError(`${label}.services must not contain duplicate service names`);
    }
    serviceNames.add(service.service_name);
  });
}

function validateAppInfraState(state, label) {
  requireExactObject(state, label, APP_INFRA_STATE_FIELDS);
  requireSchemaVersionOne(state.schema_version, `${label}.schema_version`);
  requireCanonicalStringValue(state.app_name, `${label}.app_name`);
  requireCanonicalStringValue(
    state.current_app_version,
    `${label}.current_app_version`,
  );
  requireCanonicalHashValue(state.current_manifest_hash, `${label}.current_manifest_hash`);
  requireU32Value(state.revision_count, `${label}.revision_count`, { positive: true });
  const deployedSequence = requirePositiveSafeIntegerValue(
    state.deployed_sequence,
    `${label}.deployed_sequence`,
  );
  const updatedSequence = requirePositiveSafeIntegerValue(
    state.updated_sequence,
    `${label}.updated_sequence`,
  );
  if (updatedSequence < deployedSequence) {
    throw new TypeError(`${label}.updated_sequence must not precede deployed_sequence`);
  }
  validateAppInfraManifest(state.manifest, `${label}.manifest`);
  if (
    state.app_name !== state.manifest.app_name ||
    state.current_app_version !== state.manifest.app_version
  ) {
    throw new TypeError(`${label} identity must match its embedded manifest`);
  }
}

function validateAppInfraAuditEvent(event, label) {
  requireExactObject(event, label, APP_INFRA_AUDIT_EVENT_FIELDS);
  requireSchemaVersionOne(event.schema_version, `${label}.schema_version`);
  requirePositiveSafeIntegerValue(event.sequence, `${label}.sequence`);
  const action = requireTaggedUnitChoice(
    event.action,
    `${label}.action`,
    "action",
    new Set(["Deploy", "Upgrade"]),
  );
  requireCanonicalStringValue(event.app_name, `${label}.app_name`);
  requireNullableCanonicalStringValue(event.from_version, `${label}.from_version`);
  requireCanonicalStringValue(event.to_version, `${label}.to_version`);
  requireCanonicalHashValue(event.app_manifest_hash, `${label}.app_manifest_hash`);
  requireU32Value(event.service_count, `${label}.service_count`, { positive: true });
  requireCanonicalPublicKeyValue(event.signer, `${label}.signer`);
  if (
    (action === "Deploy" && event.from_version !== null) ||
    (action === "Upgrade" && event.from_version === null)
  ) {
    throw new TypeError(`${label}.from_version must match the ${action} action`);
  }
}

/**
 * Validate the exact first-release response returned by a Soracloud mutation endpoint.
 *
 * @param {unknown} response
 * @returns {Record<string, unknown>}
 */
export function normalizeSoracloudMutationDraftResponse(response) {
  requireExactObject(response, "response", MUTATION_DRAFT_RESPONSE_FIELDS);
  if (response.ok !== true) {
    throw new TypeError("response.ok must be true");
  }
  requireCanonicalAccountIdValue(response.authority, "response.authority");
  requireCanonicalPublicKeyValue(response.signed_by, "response.signed_by");
  if (!Array.isArray(response.tx_instructions) || response.tx_instructions.length === 0) {
    throw new TypeError("response.tx_instructions must contain at least one instruction");
  }
  response.tx_instructions.forEach((instruction, index) => {
    const label = `response.tx_instructions[${index}]`;
    requireExactObject(instruction, label, TX_INSTRUCTION_FIELDS);
    requireCanonicalStringValue(instruction.wire_id, `${label}.wire_id`);
    if (
      typeof instruction.payload_hex !== "string" ||
      !LOWERCASE_EVEN_HEX_PATTERN.test(instruction.payload_hex)
    ) {
      throw new TypeError(`${label}.payload_hex must be non-empty lowercase even-length hex`);
    }
  });
  return response;
}

/**
 * Validate an exact authoritative Soracloud app-infra V1 status response.
 *
 * @param {unknown} response
 * @param {string | undefined} expectedAppName
 * @returns {Record<string, unknown>}
 */
export function normalizeSoracloudAppInfraStatusResponse(
  response,
  expectedAppName = undefined,
) {
  requireExactObject(response, "response", APP_INFRA_STATUS_RESPONSE_FIELDS);
  requireSchemaVersionOne(response.schema_version, "response.schema_version");
  const appCount = requireU32Value(response.app_count, "response.app_count");
  const auditEventCount = requireU32Value(
    response.audit_event_count,
    "response.audit_event_count",
  );
  if (!Array.isArray(response.apps) || response.apps.length !== appCount) {
    throw new TypeError("response.apps length must equal response.app_count");
  }
  const appNames = new Set();
  let previousAppName = null;
  response.apps.forEach((state, index) => {
    validateAppInfraState(state, `response.apps[${index}]`);
    if (appNames.has(state.app_name)) {
      throw new TypeError("response.apps must not contain duplicate app names");
    }
    if (previousAppName !== null && previousAppName.localeCompare(state.app_name) >= 0) {
      throw new TypeError("response.apps must be sorted by app_name");
    }
    appNames.add(state.app_name);
    previousAppName = state.app_name;
  });
  if (
    !Array.isArray(response.recent_audit_events) ||
    response.recent_audit_events.length > auditEventCount
  ) {
    throw new TypeError(
      "response.recent_audit_events must be an array bounded by response.audit_event_count",
    );
  }
  const sequences = new Set();
  let previousSequence = Number.POSITIVE_INFINITY;
  response.recent_audit_events.forEach((event, index) => {
    validateAppInfraAuditEvent(event, `response.recent_audit_events[${index}]`);
    if (sequences.has(event.sequence) || event.sequence >= previousSequence) {
      throw new TypeError(
        "response.recent_audit_events must have unique descending sequences",
      );
    }
    sequences.add(event.sequence);
    previousSequence = event.sequence;
  });
  if (expectedAppName !== undefined) {
    const canonicalExpectedName = requireCanonicalStringValue(
      expectedAppName,
      "expectedAppName",
    );
    if (
      response.apps.length !== 1 ||
      response.apps[0].app_name !== canonicalExpectedName ||
      response.recent_audit_events.some(
        (event) => event.app_name !== canonicalExpectedName,
      )
    ) {
      throw new TypeError("response must contain only the requested app identity");
    }
  }
  return response;
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
  const exactNamedAppName = namedAppName === undefined
    ? undefined
    : requireCanonicalIrohaNameValue(namedAppName, "appName");
  const path = namedAppName === undefined
    ? "/v1/soracloud/apps/status"
    : `/v1/soracloud/apps/${encodeURIComponent(exactNamedAppName)}/status`;
  if (namedAppName === undefined && options.appName != null) {
    params.app_name = requireCanonicalIrohaNameValue(
      requireString(options, "appName"),
      "appName",
    );
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
  const payload = await decodeExactSoracloudJsonResponse(
    client,
    response,
    context,
    options.signal,
  );
  return normalizeSoracloudAppInfraStatusResponse(
    payload,
    namedAppName ?? options.appName,
  );
}

function normalizeArray(input, field) {
  const value = input?.[field];
  if (!Array.isArray(value)) {
    throw new TypeError(`${field} must be an array`);
  }
  return value;
}

function normalizeStorageClass(value) {
  if (value !== "hot" && value !== "warm" && value !== "cold") {
    throw new TypeError("storageClass must be hot, warm, or cold");
  }
  return value;
}

function normalizeLeaseAssetDefinitionId(input) {
  const value = requireString(input, "leaseAssetDefinitionId");
  return normalizeAssetDefinitionId(value, "leaseAssetDefinitionId");
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
  for (const field of HF_SHARED_LEASE_JOIN_PAYLOAD_FIELDS) {
    if (field in payload && !Object.hasOwn(payload, field)) {
      throw new TypeError(`draft payload.${field} must be an own property`);
    }
  }
  for (const field of Object.getOwnPropertyNames(payload)) {
    if (!HF_SHARED_LEASE_JOIN_PAYLOAD_FIELDS.has(field)) {
      throw new TypeError(`draft payload.${field} is not accepted`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(payload, field);
    if (!descriptor?.enumerable) {
      throw new TypeError(`draft payload.${field} must be enumerable`);
    }
    if (!Object.prototype.hasOwnProperty.call(descriptor, "value")) {
      throw new TypeError(`draft payload.${field} must be an enumerable data field`);
    }
  }
  if (Object.getOwnPropertySymbols(payload).length > 0) {
    throw new TypeError("draft payload symbols are not accepted");
  }
  for (const field in payload) {
    if (!Object.hasOwn(payload, field)) {
      throw new TypeError(`draft payload.${field} must be an own property`);
    }
    if (!HF_SHARED_LEASE_JOIN_PAYLOAD_FIELDS.has(field)) {
      throw new TypeError(`draft payload.${field} is not accepted`);
    }
  }
}

function requireAssembledDraftPayloadShape(payload) {
  requireAllowedDraftPayloadFields(payload);
  requireCanonicalHfRepoIdValue(payload.repo_id, "draft payload.repo_id");
  requireCanonicalIrohaNameValue(payload.service_name, "draft payload.service_name");
  requireCanonicalStringValue(
    payload.lease_asset_definition_id,
    "draft payload.lease_asset_definition_id",
  );
  if (!HF_COMMIT_OID_PATTERN_V1.test(payload.revision)) {
    throw new TypeError(
      "draft payload.revision must be a full 40-character lowercase hexadecimal commit OID",
    );
  }
  if (!Object.hasOwn(payload, "apartment_name")) {
    throw new TypeError("draft payload.apartment_name is required");
  }
  if (payload.apartment_name !== null) {
    requireCanonicalIrohaNameValue(
      payload.apartment_name,
      "draft payload.apartment_name",
    );
  }
  normalizeAssetDefinitionId(
    payload.lease_asset_definition_id,
    "draft payload.lease_asset_definition_id",
  );
  if (!["hot", "warm", "cold"].includes(payload.storage_class)) {
    throw new TypeError("draft payload.storage_class must be hot, warm, or cold");
  }
  if (
    !Number.isSafeInteger(payload.lease_term_ms) ||
    payload.lease_term_ms <= 0
  ) {
    throw new TypeError("draft payload.lease_term_ms must be a safe positive integer");
  }
  if (!Object.hasOwn(payload, "base_fee")) {
    throw new TypeError("draft payload.base_fee is required");
  }
  try {
    const baseFee =
      typeof payload.base_fee === "string"
        ? NumericV1.decodeQuantityJson(payload.base_fee)
        : null;
    if (
      baseFee === null ||
      baseFee.toString() !== payload.base_fee ||
      baseFee.mantissa === 0n
    ) {
      throw new TypeError("draft payload.base_fee must be a canonical positive Quantity string");
    }
  } catch (error) {
    if (error instanceof NumericV1Error) {
      throw new TypeError("draft payload.base_fee must be a canonical positive Quantity string");
    }
    throw error;
  }
}

/**
 * Build an unsigned `/v1/soracloud/hf/lease/join` draft.
 *
 * @param {{ repoId: string, revision: string, serviceName: string, apartmentName: string | null, storageClass: "hot" | "warm" | "cold", leaseTermMs: number | bigint | string, leaseAssetDefinitionId: string, baseFee: import("./numericV1.js").KotodamaQuantity | string | bigint }} input
 * @returns {{ payload: Record<string, unknown>, provenancePayloads: { join: Record<string, unknown> } }}
 */
export function buildSoracloudHfSharedLeaseJoinDraft(input = {}) {
  rejectSoracloudSigningSecrets(input);
  requireExactObject(input, "input", [
    "repoId",
    "revision",
    "serviceName",
    "apartmentName",
    "storageClass",
    "leaseTermMs",
    "leaseAssetDefinitionId",
    "baseFee",
  ]);
  const apartmentName = nullableString(input, "apartmentName");
  if (apartmentName !== null) {
    requireCanonicalIrohaNameValue(apartmentName, "apartmentName");
  }
  const payload = {
    repo_id: requireCanonicalHfRepoIdValue(input.repoId, "repoId"),
    revision: requireHfCommitOid(input),
    service_name: requireCanonicalIrohaNameValue(input.serviceName, "serviceName"),
    apartment_name: apartmentName,
    storage_class: normalizeStorageClass(input.storageClass),
    lease_term_ms: normalizeSafePositiveInteger(input, "leaseTermMs"),
    lease_asset_definition_id: normalizeLeaseAssetDefinitionId(input),
    base_fee: normalizeQuantity(input, "baseFee"),
  };

  const provenancePayloads = {
    join: canonicalSigningPayload("hf_shared_lease_join", payload),
  };
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
  requireCanonicalIrohaNameValue(serviceName, `services[${index}].name`);
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
    service_manifest_hash: requireCanonicalHashValue(
      service.serviceManifestHash,
      `services[${index}].serviceManifestHash`,
    ),
    container_manifest_hash: requireCanonicalHashValue(
      service.containerManifestHash,
      `services[${index}].containerManifestHash`,
    ),
    execution_plane: { execution_plane: executionPlane, value: null },
    runtime: { runtime, value: null },
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
    app_name: requireCanonicalIrohaNameValue(
      requireString(input, "appName"),
      "appName",
    ),
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

function normalizeManifestProvenance(provenance, label) {
  rejectSoracloudSigningSecrets(provenance);
  if (
    provenance == null ||
    typeof provenance !== "object" ||
    Array.isArray(provenance)
  ) {
    throw new TypeError(`${label} must include signer and signature`);
  }
  requireExactObject(provenance, label, ["signer", "signature"]);
  if (
    typeof provenance.signer !== "string" ||
    provenance.signer.trim() === "" ||
    typeof provenance.signature !== "string" ||
    provenance.signature.trim() === ""
  ) {
    throw new TypeError(`${label} must include signer and signature`);
  }
  return {
    signer: provenance.signer,
    signature: provenance.signature,
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
  return normalizeManifestProvenance(provenances[field], `${field} provenance`);
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
  if (signingPayload == null || typeof signingPayload !== "object" || Array.isArray(signingPayload)) {
    throw new TypeError(`draft provenancePayloads.${field} is required`);
  }
  requireExactObject(signingPayload, `draft provenancePayloads.${field}`, [
    "schema",
    "label",
    "payload",
  ]);
  if (signingPayload.schema !== expectedSchema || signingPayload.label !== label) {
    throw new TypeError(`draft provenancePayloads.${field} is required`);
  }
  requireExactObject(
    signingPayload.payload,
    `draft provenancePayloads.${field} payload`,
    Object.keys(expectedPayload),
  );
  rejectSoracloudSigningSecrets(signingPayload);
  rejectSoracloudSigningSecrets(signingPayload.payload);
  if (!deepEqualCanonical(signingPayload.payload, expectedPayload)) {
    throw new TypeError(`draft provenancePayloads.${field} payload must match draft payload`);
  }
}

function requireAppInfraDraftPayloadShape(payload) {
  validateAppInfraManifest(payload, "draft payload");
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
  requireExactObject(draft, "draft", ["payload", "provenancePayloads"]);
  requireAppInfraDraftPayloadShape(draft.payload);
  requireExactObject(draft.provenancePayloads, "draft provenancePayloads", [
    "deploy",
    "services",
  ]);
  requireDraftSigningPayload(
    draft,
    "deploy",
    "app_infra_deploy",
    draft.payload,
    APP_INFRA_PROVENANCE_SCHEMA,
  );
  if (
    !Array.isArray(draft.provenancePayloads.services) ||
    draft.provenancePayloads.services.length !== draft.payload.services.length
  ) {
    throw new TypeError(
      "draft provenancePayloads.services must match every manifest service",
    );
  }
  draft.payload.services.forEach((service, index) => {
    const signingPayload = draft.provenancePayloads.services[index];
    const signingDraft = {
      provenancePayloads: { service: signingPayload },
    };
    requireDraftSigningPayload(
      signingDraft,
      "service",
      "app_infra_service",
      service,
      APP_INFRA_PROVENANCE_SCHEMA,
    );
  });
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
  requireAppInfraDraftPayloadShape(manifest);
  return {
    wire_id: SORACLOUD_APP_INFRA_DEPLOY_WIRE_ID,
    payload: {
      manifest: cloneCanonical(manifest),
      provenance: normalizeManifestProvenance(provenance, "provenance"),
    },
  };
}

export function upgradeSoracloudAppInfraInstruction(manifest, provenance) {
  requireAppInfraDraftPayloadShape(manifest);
  return {
    wire_id: SORACLOUD_APP_INFRA_UPGRADE_WIRE_ID,
    payload: {
      manifest: cloneCanonical(manifest),
      provenance: normalizeManifestProvenance(provenance, "provenance"),
    },
  };
}

/**
 * Assemble a shared-lease join request from an unsigned draft and externally signed provenance.
 *
 * @param {{ payload: Record<string, unknown>, provenancePayloads?: Record<string, unknown> }} draft
 * @param {{ join: { signer: string, signature: string } }} provenances
 * @returns {{ payload: Record<string, unknown>, provenance: { signer: string, signature: string } }}
 */
export function assembleSoracloudHfSharedLeaseJoinRequest(draft, provenances = {}) {
  rejectSoracloudSigningSecrets(draft);
  requireExactObject(draft, "draft", ["payload", "provenancePayloads"]);
  requireExactObject(draft.provenancePayloads, "draft provenancePayloads", [
    "join",
  ]);
  if (typeof draft.payload !== "object" || draft.payload == null || Array.isArray(draft.payload)) {
    throw new TypeError("draft payload is required");
  }
  requireAssembledDraftPayloadShape(draft.payload);
  requireDraftSigningPayload(draft, "join", "hf_shared_lease_join", draft.payload);
  requireExactObject(provenances, "provenances", ["join"], ["join"]);
  return {
    payload: cloneCanonical(draft.payload),
    provenance: requireProvenance(provenances, "join"),
  };
}
