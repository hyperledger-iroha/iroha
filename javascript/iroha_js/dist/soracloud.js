const BASE58_PATTERN = /^[1-9A-HJ-NP-Za-km-z]+$/;

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
  return normalized;
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
    schema: "soracloud.hf.deploy.provenance.v1",
    label,
    payload,
  };
}

/**
 * Build an unsigned `/v1/soracloud/hf/deploy` draft.
 *
 * @param {{ repoId: string, revision?: string, modelName: string, serviceName: string, apartmentName?: string, storageClass: "hot" | "warm" | "cold", leaseTermMs: number | bigint | string, leaseAssetDefinitionId: string, baseFeeNanos: number | bigint | string }} input
 * @returns {{ payload: Record<string, unknown>, provenancePayloads: { deploy: Record<string, unknown>, generatedService: Record<string, unknown>, generatedApartment?: Record<string, unknown> } }}
 */
export function buildSoracloudHfDeployDraft(input = {}) {
  if (Object.hasOwn(input, "privateKeyHex")) {
    throw new TypeError("privateKeyHex is not accepted by the Soracloud JS API");
  }
  const payload = {
    repo_id: requireString(input, "repoId"),
    model_name: requireString(input, "modelName"),
    service_name: requireString(input, "serviceName"),
    storage_class: normalizeStorageClass(input.storageClass),
    lease_term_ms: Number(normalizeIntegerString(input, "leaseTermMs")),
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
  const provenance = provenances?.[field];
  if (
    provenance == null ||
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

/**
 * Assemble a deploy request from an unsigned draft and externally signed provenance.
 *
 * @param {{ payload: Record<string, unknown>, provenancePayloads?: Record<string, unknown> }} draft
 * @param {{ deploy: { signer: string, signature: string }, generatedService: { signer: string, signature: string }, generatedApartment?: { signer: string, signature: string } }} provenances
 * @returns {{ payload: Record<string, unknown>, provenance: { signer: string, signature: string }, generated_service_provenance: { signer: string, signature: string }, generated_apartment_provenance?: { signer: string, signature: string } }}
 */
export function assembleSoracloudHfDeployRequest(draft, provenances = {}) {
  if (draft == null || typeof draft.payload !== "object" || draft.payload == null) {
    throw new TypeError("draft payload is required");
  }
  const request = {
    payload: draft.payload,
    provenance: requireProvenance(provenances, "deploy"),
    generated_service_provenance: requireProvenance(provenances, "generatedService"),
  };
  if (draft.payload.apartment_name !== undefined) {
    request.generated_apartment_provenance = requireProvenance(
      provenances,
      "generatedApartment",
    );
  }
  return request;
}
