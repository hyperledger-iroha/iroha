import { isCanonicalKotodamaIdentifier, isCanonicalKotodamaStateTypeName } from "./kotodamaIdentifiers.js";

function exactKeys(value, keys, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).length !== keys.length || keys.some((key) => !Object.hasOwn(value, key))) {
    throw new TypeError(`${context} must contain exactly ${keys.join(" and ")}`);
  }
}

/** Normalize one finite nominal error schema; variant codes are enum-local. */
export function normalizeContractErrorTypeV1(value, context = "error type") {
  exactKeys(value, ["identity", "variants"], context);
  if (typeof value.identity !== "string" || new TextEncoder().encode(value.identity).byteLength > 1024 ||
      !/^[\p{L}\p{N}_:/@.-]+$/u.test(value.identity) || value.identity.includes("__kotodama_link_")) {
    throw new TypeError(`${context}.identity must be a stable package/unit/enum identity`);
  }
  if (!Array.isArray(value.variants) || value.variants.length < 1 || value.variants.length > 256) {
    throw new TypeError(`${context}.variants must contain 1..256 variants`);
  }
  const names = new Set();
  let previous = 0;
  const variants = Array.from(value.variants, (variant, index) => {
    const label = `${context}.variants[${index}]`;
    exactKeys(variant, ["name", "code"], label);
    const name = variant.name;
    if (!(isCanonicalKotodamaIdentifier(name) ||
        (typeof name === "string" && /[^\x00-\x7f]/u.test(name) && /^[\p{L}_][\p{L}\p{N}_]*$/u.test(name))) || names.has(name)) {
      throw new TypeError(`${label}.name must be a unique canonical variant identifier`);
    }
    const code = typeof variant.code === "bigint" || (typeof variant.code === "string" && /^(?:0|[1-9][0-9]*)$/u.test(variant.code)) ? Number(variant.code) : variant.code;
    if (!Number.isSafeInteger(code) || code <= previous || code > 0xffff_ffff) {
      throw new TypeError(`${label}.code must be a nonzero u32 in strictly increasing order`);
    }
    previous = code;
    names.add(name);
    return { name, code };
  });
  return { identity: value.identity, variants };
}

/** Normalize the unique nominal catalog authenticated by the contract manifest. */
export function normalizeContractErrorTypesV1(value, context = "error_types") {
  if (value === undefined || value === null) return null;
  if (!Array.isArray(value) || value.length > 256) {
    throw new TypeError(`${context} must be an array of at most 256 error types`);
  }
  const identities = new Set();
  return Array.from(value, (entry, index) => {
    const error = normalizeContractErrorTypeV1(entry, `${context}[${index}]`);
    if (identities.has(error.identity)) throw new TypeError(`${context} contains a duplicate error identity`);
    identities.add(error.identity);
    return error;
  });
}

/** Require public error schemas and durable state identities to match the signed nominal catalog. */
export function validateManifestErrorTypeBindingsV1(manifest, context = "manifest") {
  const catalog = new Map((normalizeContractErrorTypesV1(manifest.error_types, `${context}.error_types`) ?? [])
    .map((error) => [error.identity, JSON.stringify(error)]));
  for (const state of manifest.states ?? []) {
    if (!isCanonicalKotodamaStateTypeName(state.type_name, catalog)) {
      throw new TypeError(`${context} state nominal error identity is not declared in its error_types catalog`);
    }
  }
  for (const entrypoint of manifest.entrypoints ?? []) {
    const schemas = [...(entrypoint.argument_schema?.fields ?? []).map((field) => field.ty), entrypoint.return_schema];
    for (const schema of schemas) for (const node of schema?.nodes ?? []) {
      if (node.kind === "Error" && catalog.get(node.value.identity) !== JSON.stringify(normalizeContractErrorTypeV1(node.value))) {
        throw new TypeError(`${context} boundary error schema does not match its error_types catalog`);
      }
    }
  }
}
