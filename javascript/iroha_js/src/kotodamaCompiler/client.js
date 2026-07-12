import { normalizeCompilerResult } from "./normalize.js";

const DEFAULT_COMPILE_PATH = "/v1/kotodama/compile";
const COMPILER_REQUEST_OPTION_NAMES = new Set(["sourceName", "zk"]);
const COMPILER_OPTION_NAMES = new Set([
  "compilerUrl",
  "fetchImpl",
  ...COMPILER_REQUEST_OPTION_NAMES,
]);
const MAX_COMPILER_SOURCE_BYTES = 1024 * 1024;
const MAX_COMPILER_SOURCE_NAME_BYTES = 4096;
const MAX_COMPILER_RESPONSE_BYTES = 16 * 1024 * 1024;
const MAX_COMPILER_ERROR_BYTES = 64 * 1024;

function validateUnicodeScalarString(value) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!Number.isInteger(next) || next < 0xdc00 || next > 0xdfff) {
        return false;
      }
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function isLoopbackHostname(hostname) {
  const normalized = hostname.toLowerCase();
  return (
    normalized === "localhost" ||
    normalized.endsWith(".localhost") ||
    normalized === "[::1]" ||
    /^127(?:\.[0-9]{1,3}){3}$/.test(normalized)
  );
}

export function validateCompilerSource(source) {
  if (typeof source !== "string") {
    throw new TypeError("Kotodama source must be a string");
  }
  if (!validateUnicodeScalarString(source)) {
    throw new TypeError("Kotodama source must contain valid Unicode scalar values");
  }
  const sourceBytes = new TextEncoder().encode(source).length;
  if (sourceBytes > MAX_COMPILER_SOURCE_BYTES) {
    throw new RangeError(
      `Kotodama source exceeds the ${MAX_COMPILER_SOURCE_BYTES}-byte V1 limit`,
    );
  }
}

function canonicalizeCompilerOptions(options, allowedNames) {
  if (options === undefined) {
    return {};
  }
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("Kotodama compiler options must be an object");
  }
  for (const name of Object.keys(options)) {
    if (!allowedNames.has(name)) {
      throw new TypeError(`unknown Kotodama compiler option '${name}'`);
    }
  }
  return Object.fromEntries(Object.keys(options).map((name) => [name, options[name]]));
}

function validateCompilerRequestFields(options) {
  if (Object.hasOwn(options, "sourceName")) {
    if (typeof options.sourceName !== "string" || options.sourceName.length === 0) {
      throw new TypeError("sourceName must be a non-empty string");
    }
    if (!validateUnicodeScalarString(options.sourceName)) {
      throw new TypeError("sourceName must contain valid Unicode scalar values");
    }
    const hasControlCharacter = Array.from(options.sourceName, (character) =>
      character.codePointAt(0),
    ).some((codePoint) => codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f));
    if (hasControlCharacter) {
      throw new TypeError("sourceName must not contain control characters");
    }
    const sourceNameBytes = new TextEncoder().encode(options.sourceName).length;
    if (sourceNameBytes > MAX_COMPILER_SOURCE_NAME_BYTES) {
      throw new RangeError(
        `sourceName exceeds the ${MAX_COMPILER_SOURCE_NAME_BYTES}-byte limit`,
      );
    }
  }
  if (Object.hasOwn(options, "zk") && typeof options.zk !== "boolean") {
    throw new TypeError("zk must be a boolean");
  }
  return options;
}

function validateCompilerRequestOptions(options) {
  return validateCompilerRequestFields(
    canonicalizeCompilerOptions(options, COMPILER_REQUEST_OPTION_NAMES),
  );
}

export function validateCompilerOptions(options) {
  options = validateCompilerRequestFields(
    canonicalizeCompilerOptions(options, COMPILER_OPTION_NAMES),
  );
  if (
    Object.hasOwn(options, "compilerUrl") &&
    (typeof options.compilerUrl !== "string" || options.compilerUrl.length === 0)
  ) {
    throw new TypeError("compilerUrl must be a non-empty string");
  }
  if (Object.hasOwn(options, "fetchImpl") && typeof options.fetchImpl !== "function") {
    throw new TypeError("fetchImpl must be a function");
  }
  return options;
}

/** Build the exact bounded request shared by the native and service adapters. */
export function buildCompilerRequest(source, options = {}) {
  validateCompilerSource(source);
  options = validateCompilerRequestOptions(options);
  const request = { source, zk: options.zk ?? false };
  if (options.sourceName !== undefined) {
    request.sourceName = options.sourceName;
  }
  return request;
}

/** Select request policy from an already validated top-level option object. */
export function selectCompilerRequestOptions(options) {
  const selected = {};
  for (const name of COMPILER_REQUEST_OPTION_NAMES) {
    if (Object.hasOwn(options, name)) {
      selected[name] = options[name];
    }
  }
  return selected;
}

function contentLength(response, label) {
  const raw = response.headers?.get?.("content-length");
  if (raw === null || raw === undefined) {
    return null;
  }
  if (!/^(?:0|[1-9][0-9]*)$/.test(raw)) {
    throw new Error(`${label} has an invalid Content-Length header`);
  }
  const parsed = Number(raw);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${label} Content-Length is outside the safe integer range`);
  }
  return parsed;
}

async function readBoundedResponseBytes(response, limit, label) {
  const declaredLength = contentLength(response, label);
  if (declaredLength !== null && declaredLength > limit) {
    throw new RangeError(`${label} exceeds the ${limit}-byte response limit`);
  }
  if (response.body === null) {
    return new Uint8Array();
  }
  if (typeof response.body?.getReader !== "function") {
    throw new TypeError(`${label} does not expose a standards-compliant readable body`);
  }

  const reader = response.body.getReader();
  const chunks = [];
  let total = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      if (!(value instanceof Uint8Array)) {
        await reader.cancel("non-byte compiler response chunk");
        throw new TypeError(`${label} yielded a non-byte response chunk`);
      }
      total += value.length;
      if (total > limit) {
        await reader.cancel("compiler response size limit exceeded");
        throw new RangeError(`${label} exceeds the ${limit}-byte response limit`);
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock?.();
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

async function readBoundedResponseText(response, limit, label) {
  const bytes = await readBoundedResponseBytes(response, limit, label);
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new TypeError(`${label} is not valid UTF-8`);
  }
}

async function readCompilerResult(response) {
  const text = await readBoundedResponseText(
    response,
    MAX_COMPILER_RESPONSE_BYTES,
    "Kotodama compiler response",
  );
  let result;
  try {
    result = JSON.parse(text);
  } catch {
    throw new TypeError("Kotodama compiler service returned malformed JSON");
  }
  return normalizeCompilerResult(result);
}

/** Browser/Node client for an explicitly configured canonical Rust compiler service. */
export class KotodamaCompilerClient {
  constructor(baseUrl, { fetchImpl = globalThis.fetch } = {}) {
    if (typeof baseUrl !== "string" || baseUrl.length === 0) {
      throw new TypeError("Kotodama compiler baseUrl must be a non-empty string");
    }
    let parsed;
    try {
      parsed = new URL(baseUrl);
    } catch {
      throw new TypeError("Kotodama compiler baseUrl must be an absolute URL");
    }
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      throw new TypeError("Kotodama compiler baseUrl must use HTTP or HTTPS");
    }
    if (parsed.protocol === "http:" && !isLoopbackHostname(parsed.hostname)) {
      throw new TypeError(
        "Kotodama compiler baseUrl must use HTTPS except for loopback development services",
      );
    }
    if (parsed.username !== "" || parsed.password !== "") {
      throw new TypeError("Kotodama compiler baseUrl must not contain credentials");
    }
    if (parsed.search !== "" || parsed.hash !== "") {
      throw new TypeError("Kotodama compiler baseUrl must not contain a query or fragment");
    }
    if (typeof fetchImpl !== "function") {
      throw new TypeError("Kotodama compiler client requires fetch");
    }
    this.baseUrl = parsed.href.replace(/\/$/, "");
    this.fetchImpl = fetchImpl;
  }

  async compile(source, options = {}) {
    const request = buildCompilerRequest(source, options);
    const response = await this.fetchImpl(`${this.baseUrl}${DEFAULT_COMPILE_PATH}`, {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
      referrerPolicy: "no-referrer",
      body: JSON.stringify(request),
    });
    if (
      response === null ||
      typeof response !== "object" ||
      typeof response.ok !== "boolean" ||
      !Number.isInteger(response.status)
    ) {
      throw new TypeError("Kotodama compiler fetch returned an invalid Response");
    }
    if (!response.ok) {
      const detail = await readBoundedResponseText(
        response,
        MAX_COMPILER_ERROR_BYTES,
        "Kotodama compiler error response",
      );
      const suffix = detail.length === 0 ? "" : `: ${detail}`;
      throw new Error(`Kotodama compiler service failed (${response.status})${suffix}`);
    }
    return readCompilerResult(response);
  }
}
