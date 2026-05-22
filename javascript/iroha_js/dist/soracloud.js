const BASE58_PATTERN = /^[1-9A-HJ-NP-Za-km-z]+$/;
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
const PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID =
  "iroha_data_model::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt";
export const SORACLOUD_APP_INFRA_DEPLOY_WIRE_ID =
  "iroha_data_model::isi::soracloud::DeploySoracloudAppInfra";
export const SORACLOUD_APP_INFRA_UPGRADE_WIRE_ID =
  "iroha_data_model::isi::soracloud::UpgradeSoracloudAppInfra";
const PRIVATE_UPLOADED_MODEL_COUNT_MODES = new Set(["bounded", "exact"]);

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

function normalizeSignedI32(value, field) {
  if (!Number.isInteger(value) || value < -2147483648 || value > 2147483647) {
    throw new TypeError(`${field} must be a signed 32-bit integer`);
  }
  return value;
}

function normalizeSignedI8(value, field) {
  if (!Number.isInteger(value) || value < -128 || value > 127) {
    throw new TypeError(`${field} must be a signed 8-bit integer`);
  }
  return value;
}

function normalizeArray(input, field) {
  const value = input?.[field];
  if (!Array.isArray(value)) {
    throw new TypeError(`${field} must be an array`);
  }
  return value;
}

function normalizeHashLike(input, field) {
  return normalizeHashLikeValue(input?.[field], field);
}

function normalizeHashLikeValue(value, field) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new TypeError(`${field} must be a non-empty string`);
  }
  return value.trim();
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

function normalizePrivateArtifactRef(input, field, expectedRole) {
  const artifact = input?.[field];
  if (artifact == null || typeof artifact !== "object" || Array.isArray(artifact)) {
    throw new TypeError(`${field} must be an object`);
  }
  rejectSoracloudSigningSecrets(artifact);
  const role = artifact.artifactRole ?? artifact.artifact_role;
  if (role !== expectedRole) {
    throw new TypeError(`${field}.artifactRole must be ${expectedRole}`);
  }
  return {
    schema_version: normalizeSafePositiveIntegerValue(
      artifact.schemaVersion ?? artifact.schema_version,
      `${field}.schemaVersion`,
    ),
    sorafs_manifest_digest: normalizeHashLikeValue(
      artifact.sorafsManifestDigest ?? artifact.sorafs_manifest_digest,
      `${field}.sorafsManifestDigest`,
    ),
    artifact_hash: normalizeHashLikeValue(
      artifact.artifactHash ?? artifact.artifact_hash,
      `${field}.artifactHash`,
    ),
    ciphertext_bytes: normalizeSafePositiveIntegerValue(
      artifact.ciphertextBytes ?? artifact.ciphertext_bytes,
      `${field}.ciphertextBytes`,
    ),
    artifact_role: role,
  };
}

function normalizeQuantizedCpuModel(input) {
  const model = input?.model;
  if (model == null || typeof model !== "object" || Array.isArray(model)) {
    throw new TypeError("model must be an object");
  }
  rejectSoracloudSigningSecrets(model);
  const inputLen = normalizeSafePositiveIntegerValue(
    model.inputLen ?? model.input_len,
    "model.inputLen",
  );
  const outputLen = normalizeSafePositiveIntegerValue(
    model.outputLen ?? model.output_len,
    "model.outputLen",
  );
  const weights = normalizeArray(
    { value: model.weightsI8 ?? model.weights_i8 },
    "value",
  ).map((value, index) => normalizeSignedI8(value, `model.weightsI8[${index}]`));
  const bias = normalizeArray(
    { value: model.biasI32 ?? model.bias_i32 },
    "value",
  ).map((value, index) => normalizeSignedI32(value, `model.biasI32[${index}]`));
  if (weights.length !== inputLen * outputLen) {
    throw new TypeError("model.weightsI8 length must equal inputLen * outputLen");
  }
  if (bias.length !== outputLen) {
    throw new TypeError("model.biasI32 length must equal outputLen");
  }
  const outputShift = normalizeSafeIntegerValue(
    model.outputShift ?? model.output_shift,
    "model.outputShift",
  );
  if (outputShift > 30) {
    throw new TypeError("model.outputShift must be <= 30");
  }
  const outputMin = normalizeSignedI32(
    model.outputMin ?? model.output_min,
    "model.outputMin",
  );
  const outputMax = normalizeSignedI32(
    model.outputMax ?? model.output_max,
    "model.outputMax",
  );
  if (outputMin > outputMax) {
    throw new TypeError("model.outputMin must be <= model.outputMax");
  }
  return {
    input_len: inputLen,
    output_len: outputLen,
    weights_i8: weights,
    bias_i32: bias,
    output_shift: outputShift,
    output_min: outputMin,
    output_max: outputMax,
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
  for (const field of ["repo_id", "model_name", "service_name", "lease_asset_definition_id"]) {
    if (
      !Object.hasOwn(payload, field) ||
      typeof payload[field] !== "string" ||
      payload[field].trim() === ""
    ) {
      throw new TypeError(`draft payload.${field} must be a non-empty string`);
    }
  }
  for (const field of ["revision", "apartment_name"]) {
    if (
      payload[field] !== undefined &&
      (typeof payload[field] !== "string" || payload[field].trim() === "")
    ) {
      throw new TypeError(`draft payload.${field} must be a non-empty string when provided`);
    }
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
    revision: payload.revision ?? null,
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
 * @param {{ repoId: string, revision?: string, modelName: string, serviceName: string, apartmentName?: string, storageClass: "hot" | "warm" | "cold", leaseTermMs: number | bigint | string, leaseAssetDefinitionId: string, baseFeeNanos: number | bigint | string }} input
 * @returns {{ payload: Record<string, unknown>, provenancePayloads: { deploy: Record<string, unknown>, generatedService: Record<string, unknown>, generatedApartment?: Record<string, unknown> } }}
 */
export function buildSoracloudHfDeployDraft(input = {}) {
  rejectSoracloudSigningSecrets(input);
  const payload = {
    repo_id: requireString(input, "repoId"),
    model_name: requireString(input, "modelName"),
    service_name: requireString(input, "serviceName"),
    storage_class: normalizeStorageClass(input.storageClass),
    lease_term_ms: normalizeSafeInteger(input, "leaseTermMs"),
    lease_asset_definition_id: normalizeLeaseAssetDefinitionId(input),
    base_fee_nanos: normalizeIntegerString(input, "baseFeeNanos"),
  };
  const revision = optionalString(input, "revision");
  if (revision !== undefined) {
    payload.revision = revision;
  }
  const apartmentName = optionalString(input, "apartmentName");
  if (apartmentName !== undefined) {
    payload.apartment_name = apartmentName;
  }

  const provenancePayloads = {
    deploy: canonicalSigningPayload("hf_deploy", payload),
    generatedService: canonicalSigningPayload("generated_service", {
      service_name: payload.service_name,
      repo_id: payload.repo_id,
      revision: payload.revision ?? null,
    }),
  };
  if (payload.apartment_name !== undefined) {
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
  const path = requireString(route, "path");
  const publicHost = optionalString(route, "publicHost") ?? optionalString(route, "public_host");
  const internalUrl = optionalString(route, "internalUrl") ?? optionalString(route, "internal_url");
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
  const temperature = optionalString(volume, "temperature") ?? "hot";
  if (!["hot", "warm", "cold"].includes(temperature)) {
    throw new TypeError(`${field}.temperature must be hot, warm, or cold`);
  }
  normalizeSafePositiveIntegerValue(
    volume.maxTotalBytes ?? volume.max_total_bytes ?? 1,
    `${field}.maxTotalBytes`,
  );
  return requireString(volume, "name");
}

function normalizeAppStaticSite(input) {
  const site = input?.staticSite ?? input?.static_site;
  if (site === undefined || site === null) {
    return undefined;
  }
  rejectSoracloudSigningSecrets(site);
  if (typeof site !== "object" || Array.isArray(site)) {
    throw new TypeError("staticSite must be an object");
  }
  const payload = {
    schema_version: 1,
    public_url: requireString(site, "publicUrl"),
    mount_path: optionalString(site, "mountPath") ?? "/",
  };
  const contentCid = optionalString(site, "contentCid") ?? optionalString(site, "content_cid");
  if (contentCid !== undefined) {
    payload.content_cid = contentCid;
  }
  const manifestDigestHex =
    optionalString(site, "manifestDigestHex") ?? optionalString(site, "manifest_digest_hex");
  if (manifestDigestHex !== undefined) {
    payload.manifest_digest_hex = manifestDigestHex;
  }
  const apiBasePath = optionalString(site, "apiBasePath") ?? optionalString(site, "api_base_path");
  if (apiBasePath !== undefined) {
    payload.api_base_path = apiBasePath;
  }
  return payload;
}

function normalizeServiceRuntime(value, field) {
  const runtime = value ?? "Inrou";
  if (!["Inrou", "Ivm"].includes(runtime)) {
    throw new TypeError(`${field} must be Inrou or Ivm`);
  }
  return runtime;
}

function normalizeExecutionPlane(value, field) {
  const executionPlane = value ?? "HttpService";
  if (executionPlane === "Ivm") {
    return "DeterministicService";
  }
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
  const serviceName = requireString(service, "name");
  const serviceVersion =
    optionalString(service, "serviceVersion") ??
    optionalString(service, "service_version") ??
    optionalString(service, "version");
  if (serviceVersion === undefined) {
    throw new TypeError(`services[${index}].serviceVersion must be a non-empty string`);
  }
  const runtime = normalizeServiceRuntime(optionalString(service, "runtime"), `services[${index}].runtime`);
  const executionPlane = normalizeExecutionPlane(
    optionalString(service, "executionPlane") ?? optionalString(service, "execution_plane"),
    `services[${index}].executionPlane`,
  );
  const routes = (service.routes ?? []).map((route, routeIndex) =>
    normalizeRouteSpec(route, `services[${index}].routes[${routeIndex}]`),
  );
  const leaseVolumes = (service.leaseVolumes ?? service.lease_volumes ?? []).map(
    (volume, volumeIndex) =>
      normalizeLeaseVolumeSpec(volume, `services[${index}].leaseVolumes[${volumeIndex}]`),
  );
  const base = {
    schema_version: 1,
    service_name: serviceName,
    service_version: serviceVersion,
    service_manifest_hash:
      optionalString(service, "serviceManifestHash") ??
      optionalString(service, "service_manifest_hash") ??
      requireString(service, "serviceManifestHash"),
    container_manifest_hash:
      optionalString(service, "containerManifestHash") ??
      optionalString(service, "container_manifest_hash") ??
      requireString(service, "containerManifestHash"),
    execution_plane: executionPlane,
    runtime,
    routes,
    lease_volumes: leaseVolumes,
  };
  const shards = service.shards;
  if (shards === undefined || shards === null) {
    return [base];
  }
  rejectSoracloudSigningSecrets(shards);
  if (typeof shards !== "object" || Array.isArray(shards)) {
    throw new TypeError(`services[${index}].shards must be an object`);
  }
  const count = normalizeSafePositiveIntegerValue(shards.count, `services[${index}].shards.count`);
  const shardIdEnv = optionalString(shards, "shardIdEnv") ?? "SORACLOUD_SHARD_ID";
  const shardCountEnv = optionalString(shards, "shardCountEnv") ?? "SORACLOUD_SHARD_COUNT";
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
 * @param {{ appName: string, appVersion?: string, publicUrl: string, staticSite?: Record<string, unknown>, services: Array<Record<string, unknown>> }} input
 * @returns {{ payload: Record<string, unknown>, provenancePayloads: { deploy: Record<string, unknown>, services: Record<string, unknown>[] } }}
 */
export function buildSoracloudAppInfraDraft(input = {}) {
  rejectSoracloudSigningSecrets(input);
  const services = normalizeArray(input, "services")
    .flatMap((service, index) => normalizeServiceSpec(service, index));
  if (services.length === 0) {
    throw new TypeError("services must contain at least one service");
  }
  const payload = {
    schema_version: 1,
    app_name: requireString(input, "appName"),
    app_version: optionalString(input, "appVersion") ?? "dev",
    public_url: requireString(input, "publicUrl"),
    services,
  };
  const staticSite = normalizeAppStaticSite(input);
  if (staticSite !== undefined) {
    payload.static_site = staticSite;
  }
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
 * @param {{ deployServices?: unknown[], upgradeServices?: unknown[] }} [options]
 * @returns {{ manifest: Record<string, unknown>, provenance: { signer: string, signature: string }, deploy_services: unknown[], upgrade_services: unknown[] }}
 */
export function assembleSoracloudAppInfraRequest(draft, provenances = {}, options = {}) {
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
  const deployServices = options.deployServices ?? options.deploy_services ?? [];
  const upgradeServices = options.upgradeServices ?? options.upgrade_services ?? [];
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
 * @param {{ deploy: { signer: string, signature: string }, generatedService: { signer: string, signature: string }, generatedApartment?: { signer: string, signature: string } }} provenances
 * @returns {{ payload: Record<string, unknown>, provenance: { signer: string, signature: string }, generated_service_provenance: { signer: string, signature: string }, generated_apartment_provenance?: { signer: string, signature: string } }}
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
  const request = {
    payload: draft.payload,
    provenance: requireProvenance(provenances, "deploy"),
    generated_service_provenance: requireProvenance(provenances, "generatedService"),
  };
  if (draft.payload.apartment_name !== undefined) {
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
  }
  return request;
}

/**
 * Build a deterministic private uploaded-model execution request.
 *
 * The helper only normalizes client-side request shape. It never accepts raw
 * signing secrets and does not submit the returned receipt instruction.
 *
 * @param {{ serviceName: string, weightVersion: string, modelId?: string, modelName?: string, bundleRoot?: string, policyId: string, model: { inputLen: number, outputLen: number, weightsI8: number[], biasI32: number[], outputShift: number, outputMin: number, outputMax: number }, plaintextInputI32: number[], inputArtifact: Record<string, unknown>, outputArtifact: Record<string, unknown>, emittedSequence: number | bigint | string }} input
 * @returns {Record<string, unknown>}
 */
export function buildSoracloudPrivateUploadedModelExecuteRequest(input = {}) {
  rejectSoracloudSigningSecrets(input);
  const modelId = optionalString(input, "modelId");
  const modelName = optionalString(input, "modelName");
  if ((modelId === undefined) === (modelName === undefined)) {
    throw new TypeError("exactly one of modelId or modelName must be provided");
  }
  const plaintextInput = normalizeArray(input, "plaintextInputI32").map((value, index) =>
    normalizeSignedI32(value, `plaintextInputI32[${index}]`),
  );
  const request = {
    service_name: requireString(input, "serviceName"),
    weight_version: requireString(input, "weightVersion"),
    policy_id: requireString(input, "policyId"),
    model: normalizeQuantizedCpuModel(input),
    plaintext_input_i32: plaintextInput,
    input_artifact: normalizePrivateArtifactRef(input, "inputArtifact", "input"),
    output_artifact: normalizePrivateArtifactRef(input, "outputArtifact", "output"),
    emitted_sequence: normalizeSafePositiveInteger(input, "emittedSequence"),
  };
  if (modelId !== undefined) {
    request.model_id = modelId;
  }
  if (modelName !== undefined) {
    request.model_name = modelName;
  }
  const bundleRoot = optionalString(input, "bundleRoot");
  if (bundleRoot !== undefined) {
    request.bundle_root = bundleRoot;
  }
  return request;
}

/**
 * Build query parameters for committed private uploaded-model execution receipts.
 *
 * @param {{ receiptId?: string, serviceName?: string, modelId?: string, weightVersion?: string, limit?: number | bigint | string, countMode?: "bounded" | "exact" }} input
 * @returns {Record<string, string>}
 */
export function buildSoracloudPrivateUploadedModelReceiptQuery(input = {}) {
  rejectSoracloudSigningSecrets(input);
  const query = {};
  const receiptId = optionalString(input, "receiptId");
  if (receiptId !== undefined) {
    query.receipt_id = receiptId;
  }
  const serviceName = optionalString(input, "serviceName");
  if (serviceName !== undefined) {
    query.service_name = serviceName;
  }
  const modelId = optionalString(input, "modelId");
  if (modelId !== undefined) {
    query.model_id = modelId;
  }
  const weightVersion = optionalString(input, "weightVersion");
  if (weightVersion !== undefined) {
    query.weight_version = weightVersion;
  }
  if (input.limit !== undefined && input.limit !== null) {
    query.limit = String(normalizeSafePositiveInteger(input, "limit"));
  }
  const countMode = optionalString(input, "countMode");
  if (countMode !== undefined) {
    if (!PRIVATE_UPLOADED_MODEL_COUNT_MODES.has(countMode)) {
      throw new TypeError("countMode must be bounded or exact");
    }
    query.count_mode = countMode;
  }
  return query;
}

/**
 * Validate and extract the receipt-recording instruction from a private execute response.
 *
 * @param {Record<string, unknown>} response
 * @returns {{ wire_id: string, payload_hex: string }}
 */
export function privateUploadedModelReceiptInstruction(response) {
  if (response == null || typeof response !== "object" || Array.isArray(response)) {
    throw new TypeError("response must be an object");
  }
  const instructions = response.tx_instructions;
  if (!Array.isArray(instructions) || instructions.length !== 1) {
    throw new TypeError("response.tx_instructions must contain exactly one instruction");
  }
  const instruction = instructions[0];
  if (instruction == null || typeof instruction !== "object" || Array.isArray(instruction)) {
    throw new TypeError("response.tx_instructions[0] must be an object");
  }
  if (instruction.wire_id !== PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID) {
    throw new TypeError(
      `response.tx_instructions[0].wire_id must be ${PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID}`,
    );
  }
  if (
    typeof instruction.payload_hex !== "string" ||
    instruction.payload_hex.length === 0 ||
    !/^[0-9a-fA-F]+$/.test(instruction.payload_hex)
  ) {
    throw new TypeError("response.tx_instructions[0].payload_hex must be non-empty hex");
  }
  return {
    wire_id: instruction.wire_id,
    payload_hex: instruction.payload_hex,
  };
}
