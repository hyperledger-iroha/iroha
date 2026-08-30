import { compareUtf16 } from "./ordering.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";
import { requireVerifierBackendRegistryLabelV1 } from "./verifierBackendRegistry.js";

export const ACTIVE_VERIFYING_KEY_IDS_RESPONSE_MAX_BYTES = 512 * 1024;

const ACTIVE_VERIFYING_KEY_IDS_RESPONSE_CONTEXT =
  "active verifying-key ids response";
const ACTIVE_VERIFYING_KEY_IDS_MAX_ITEMS = 1_000;
const ACTIVE_VERIFYING_KEY_ID_FIELDS = Object.freeze(["backend", "name"]);
const PORTABLE_REGISTRY_FIELD_PATTERN =
  /^[a-z0-9](?:[a-z0-9._/:-]{0,254}[a-z0-9])?$/u;
const FORBIDDEN_PORTABLE_REGISTRY_SEQUENCES = Object.freeze([
  "..",
  "//",
  ":::",
  "/:",
  ":/",
  "/.",
  "./",
  ":.",
  ".:",
]);
const PUBLIC_READ_CREDENTIAL_HEADER_NAMES = new Set([
  "authorization",
  "cookie",
  "proxy-authorization",
  "x-account-id",
  "x-api-token",
  "x-dataspace-id",
  "x-iroha-account",
  "x-iroha-nonce",
  "x-iroha-onboarding-token",
  "x-iroha-signature",
  "x-iroha-timestamp-ms",
  "x-iroha-witness",
]);
const PUBLIC_JSON_ACCEPT_HEADERS = Object.freeze({
  Accept: "application/json",
  "Accept-Encoding": "identity",
  Authorization: null,
  Cookie: null,
  "Proxy-Authorization": null,
  "X-Account-Id": null,
  "X-API-Token": null,
  "X-Dataspace-Id": null,
  "X-Iroha-Account": null,
  "X-Iroha-Nonce": null,
  "X-Iroha-Onboarding-Token": null,
  "X-Iroha-Signature": null,
  "X-Iroha-Timestamp-Ms": null,
  "X-Iroha-Witness": null,
});

/** Remove credential and representation-changing defaults from public reads. */
export function stripPublicReadCredentialHeaders(headers) {
  for (const name of Object.keys(headers)) {
    const normalized = name.toLowerCase();
    if (
      normalized === "accept-encoding"
      || normalized.startsWith("x-iroha-operator-")
      || PUBLIC_READ_CREDENTIAL_HEADER_NAMES.has(normalized)
    ) {
      delete headers[name];
    }
  }
  return headers;
}

function requirePortableRegistryField(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  // Every admitted character is ASCII, so code-unit length is exactly the
  // UTF-8 byte length used by Rust's portable verifier-registry grammar.
  if (
    !PORTABLE_REGISTRY_FIELD_PATTERN.test(value)
    || FORBIDDEN_PORTABLE_REGISTRY_SEQUENCES.some((sequence) => value.includes(sequence))
  ) {
    throw new TypeError(`${context} is not a portable verifier-registry field`);
  }
  return value;
}

function requireExactIdObject(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const fields = Object.keys(value).sort(compareUtf16);
  if (
    fields.length !== ACTIVE_VERIFYING_KEY_ID_FIELDS.length
    || fields.some((field, index) => field !== ACTIVE_VERIFYING_KEY_ID_FIELDS[index])
  ) {
    throw new TypeError(`${context} must contain only backend and name`);
  }
  const backend = requireVerifierBackendRegistryLabelV1(
    requirePortableRegistryField(value.backend, `${context}.backend`),
    `${context}.backend`,
  );
  const name = requirePortableRegistryField(value.name, `${context}.name`);
  return Object.freeze({ backend, name });
}

/**
 * Parse Torii's bounded `ids_only=true` active verifying-key projection.
 * Duplicate JSON keys are rejected by the strict decoder before projection.
 *
 * @param {string} text
 * @param {string} [context]
 * @returns {ReadonlyArray<Readonly<{backend: string, name: string}>>}
 */
export function parseActiveVerifyingKeyIdsJson(
  text,
  context = ACTIVE_VERIFYING_KEY_IDS_RESPONSE_CONTEXT,
) {
  const payload = parseStrictLosslessIntegerJson(text, context);
  if (!Array.isArray(payload)) {
    throw new TypeError(`${context} must be a JSON array`);
  }
  if (payload.length > ACTIVE_VERIFYING_KEY_IDS_MAX_ITEMS) {
    throw new RangeError(`${context} exceeds 1000 identifiers`);
  }

  const ids = [];
  const backendsByName = new Map();
  let previous = null;
  for (let index = 0; index < payload.length; index += 1) {
    const id = requireExactIdObject(payload[index], `${context}[${index}]`);
    let backends = backendsByName.get(id.name);
    if (backends === undefined) {
      backends = new Set();
      backendsByName.set(id.name, backends);
    }
    if (backends.has(id.backend)) {
      throw new TypeError(`${context} contains a duplicate identifier`);
    }
    backends.add(id.backend);

    if (
      previous !== null
      && (
        compareUtf16(previous.name, id.name) > 0
        || (
          previous.name === id.name
          && compareUtf16(previous.backend, id.backend) > 0
        )
      )
    ) {
      throw new TypeError(`${context} is not in requested ascending order`);
    }
    ids.push(id);
    previous = id;
  }
  return Object.freeze(ids);
}

/** Node Torii transport for the strict public active-id projection. */
export async function requestActiveVerifyingKeyIds(client, signal) {
  const headers = { ...PUBLIC_JSON_ACCEPT_HEADERS };
  for (const name of Object.keys(client._config.defaultHeaders ?? {})) {
    if (name.toLowerCase().startsWith("x-iroha-operator-")) headers[name] = null;
  }
  const response = await client._request("GET", "/v1/zk/vk", {
    headers,
    params: {
      status: "Active",
      ids_only: true,
      limit: 1_000,
      order: "asc",
    },
    redirect: "error",
    signal,
    omitCredentials: true,
  });
  await client._expectStatus(response, [200]);
  client._requireIdentityEncoding(
    response,
    ACTIVE_VERIFYING_KEY_IDS_RESPONSE_CONTEXT,
  );
  if (client._getHeader(response, "content-type") !== "application/json") {
    throw new TypeError(
      `${ACTIVE_VERIFYING_KEY_IDS_RESPONSE_CONTEXT} Content-Type must be exactly application/json`,
    );
  }
  const { bytes } = await client._readBoundedResponseBytes(
    response,
    ACTIVE_VERIFYING_KEY_IDS_RESPONSE_MAX_BYTES,
    ACTIVE_VERIFYING_KEY_IDS_RESPONSE_CONTEXT,
    { signal },
  );
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (error) {
    throw new TypeError(
      `${ACTIVE_VERIFYING_KEY_IDS_RESPONSE_CONTEXT} must be valid UTF-8`,
      { cause: error },
    );
  }
  return parseActiveVerifyingKeyIdsJson(text);
}
