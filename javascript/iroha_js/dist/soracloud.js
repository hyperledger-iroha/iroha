const BASE58_PATTERN = /^[1-9A-HJ-NP-Za-km-z]+$/;
const REJECTED_SIGNING_SECRET_FIELDS = [
  "privateKeyHex",
  "privateKey",
  "private_key",
  "private_key_hex",
];
const PROVENANCE_SCHEMA = "soracloud.hf.deploy.provenance.v1";
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

function normalizeIntegerString(input, field) {
  const value = input?.[field];
  const normalized = typeof value === "bigint" ? value.toString() : String(value ?? "");
  if (!/^[0-9]+$/.test(normalized)) {
    throw new TypeError(`${field} must be a non-negative integer`);
  }
  return BigInt(normalized).toString();
}

function normalizeSafeInteger(input, field) {
  const normalized = normalizeIntegerString(input, field);
  const value = Number(normalized);
  if (!Number.isSafeInteger(value)) {
    throw new TypeError(`${field} must fit in a safe JavaScript integer`);
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
  if (!BASE58_PATTERN.test(value)) {
    throw new Error("Asset Definition ID must be valid Base58");
  }
  return value;
}

function canonicalSigningPayload(label, payload) {
  return {
    schema: PROVENANCE_SCHEMA,
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
  for (const field in payload) {
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

function requireProvenance(provenances, field) {
  if (
    provenances == null ||
    typeof provenances !== "object" ||
    !Object.hasOwn(provenances, field)
  ) {
    throw new TypeError(`${field} provenance must include signer and signature`);
  }
  const provenance = provenances[field];
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

function requireDraftSigningPayload(draft, field, label, expectedPayload) {
  if (
    draft.provenancePayloads == null ||
    typeof draft.provenancePayloads !== "object" ||
    !Object.hasOwn(draft.provenancePayloads, field)
  ) {
    throw new TypeError(`draft provenancePayloads.${field} is required`);
  }
  const signingPayload = draft.provenancePayloads[field];
  if (
    signingPayload == null ||
    typeof signingPayload !== "object" ||
    Array.isArray(signingPayload) ||
    signingPayload.schema !== PROVENANCE_SCHEMA ||
    signingPayload.label !== label ||
    typeof signingPayload.payload !== "object" ||
    signingPayload.payload == null ||
    Array.isArray(signingPayload.payload)
  ) {
    throw new TypeError(`draft provenancePayloads.${field} is required`);
  }
  if (!deepEqualCanonical(signingPayload.payload, expectedPayload)) {
    throw new TypeError(`draft provenancePayloads.${field} payload must match draft payload`);
  }
}

/**
 * Assemble a deploy request from an unsigned draft and externally signed provenance.
 *
 * @param {{ payload: Record<string, unknown>, provenancePayloads?: Record<string, unknown> }} draft
 * @param {{ deploy: { signer: string, signature: string }, generatedService: { signer: string, signature: string }, generatedApartment?: { signer: string, signature: string } }} provenances
 * @returns {{ payload: Record<string, unknown>, provenance: { signer: string, signature: string }, generated_service_provenance: { signer: string, signature: string }, generated_apartment_provenance?: { signer: string, signature: string } }}
 */
export function assembleSoracloudHfDeployRequest(draft, provenances = {}) {
  if (
    draft == null ||
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
