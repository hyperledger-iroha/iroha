import { blake2b256 } from "../blake2b.js";
import {
  isCanonicalKotodamaEntrypoint as isCanonicalEntrypointName,
  isCanonicalKotodamaIdentifier as isCanonicalIdentifier,
} from "../kotodamaIdentifiers.js";

const CONTRACT_HASH_DOMAIN = new TextEncoder().encode("iroha:ivm:contract-artifact:v1\0");
const DIAGNOSTIC_PHASES = new Set(["lex", "parse", "semantic", "lowering", "artifact"]);
const DIAGNOSTIC_SEVERITIES = new Set(["error", "warning"]);
const MANIFEST_ENTRYPOINT_KINDS = new Set([
  "Kotoage",
  "View",
  "Hajimari",
  "Kaizen",
]);
const MAX_DIAGNOSTICS = 64;
const MAX_ARTIFACT_BYTES = 32 * 1024 * 1024;
const UTF8_ENCODER = new TextEncoder();

function isRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function requireRecord(value, label) {
  if (!isRecord(value)) {
    throw new TypeError(`${label} must be an object`);
  }
  return value;
}

function requireExactKeys(value, keys, label) {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
  ) {
    throw new TypeError(`${label} has an invalid field set`);
  }
}

function requireOnlyKeys(value, keys, label) {
  const allowed = new Set(keys);
  if (Object.keys(value).some((key) => !allowed.has(key))) {
    throw new TypeError(`${label} contains an unknown field`);
  }
}

function parseJson(raw, label) {
  if (typeof raw !== "string") {
    throw new TypeError(`${label} must be a JSON string`);
  }
  try {
    return JSON.parse(raw);
  } catch {
    throw new TypeError(`${label} is not valid JSON`);
  }
}

function normalizeHashHex(value, label) {
  if (typeof value !== "string") {
    throw new TypeError(`Kotodama compiler response is missing ${label}`);
  }
  if (/^[0-9a-fA-F]{64}$/u.test(value)) {
    return requireIrohaHashMarker(value.toLowerCase(), label);
  }
  const literal = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u.exec(value);
  if (literal === null) {
    throw new TypeError(
      `Kotodama compiler response contains an invalid or noncanonical ${label}`,
    );
  }
  const [, body, checksum] = literal;
  const expected = crc16Literal("hash", body);
  if (checksum !== expected) {
    throw new TypeError(
      `Kotodama compiler response contains an invalid ${label} checksum; expected ${expected}`,
    );
  }
  return requireIrohaHashMarker(body.toLowerCase(), label);
}

function requireIrohaHashMarker(hex, label) {
  if ((Number.parseInt(hex.slice(-2), 16) & 1) !== 1) {
    throw new TypeError(
      `Kotodama compiler response contains an invalid ${label} marker bit`,
    );
  }
  return hex;
}

function crc16Literal(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let index = 0; index < 8; index += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  };
  for (const byte of UTF8_ENCODER.encode(tag)) {
    processByte(byte);
  }
  processByte(0x3a);
  for (const byte of UTF8_ENCODER.encode(body)) {
    processByte(byte);
  }
  return crc.toString(16).toUpperCase().padStart(4, "0");
}

function toHex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function normalizeArtifactBytes(value) {
  let bytes;
  if (value instanceof Uint8Array) {
    bytes = new Uint8Array(value);
  } else if (Array.isArray(value)) {
    if (value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
      throw new TypeError("Kotodama compiler artifactBytes must contain only bytes");
    }
    bytes = Uint8Array.from(value);
  } else {
    throw new TypeError("Kotodama compiler response is missing artifactBytes");
  }
  if (bytes.length === 0 || bytes.length > MAX_ARTIFACT_BYTES) {
    throw new TypeError(
      `Kotodama compiler artifactBytes must contain 1..${MAX_ARTIFACT_BYTES} bytes`,
    );
  }
  return bytes;
}

function artifactHashHex(artifactBytes) {
  const input = new Uint8Array(CONTRACT_HASH_DOMAIN.length + artifactBytes.length);
  input.set(CONTRACT_HASH_DOMAIN);
  input.set(artifactBytes, CONTRACT_HASH_DOMAIN.length);
  const digest = blake2b256(input);
  // `iroha_crypto::Hash::prehashed` reserves the low bit of the final byte.
  digest[digest.length - 1] |= 1;
  return toHex(digest);
}

function parseSidecar(raw, kind, artifactHash) {
  const sidecar = requireRecord(parseJson(raw, `${kind} sidecar`), `${kind} sidecar`);
  if (
    sidecar.sidecar_version !== 1 ||
    sidecar.kind !== kind ||
    normalizeHashHex(sidecar.artifact_hash, `${kind} artifact hash`) !== artifactHash ||
    !Array.isArray(sidecar.entries)
  ) {
    throw new Error(`Kotodama compiler returned an invalid or mismatched ${kind} sidecar`);
  }
  return sidecar.entries;
}

function validateCompilerManifest(manifest) {
  if (Object.prototype.hasOwnProperty.call(manifest, "contract_name")) {
    throw new TypeError(
      "Kotodama manifest must use seiyaku_name; contract_name is not a V1 field",
    );
  }
  if (!isCanonicalIdentifier(manifest.seiyaku_name, { declaration: true })) {
    throw new TypeError(
      "Kotodama manifest seiyaku_name must be a canonical V1 declaration identifier",
    );
  }
  if (
    typeof manifest.compiler_fingerprint !== "string" ||
    manifest.compiler_fingerprint.trim() === ""
  ) {
    throw new TypeError("Kotodama manifest compiler_fingerprint must not be empty");
  }
  validateCompilerManifestStates(manifest.states);
  validateCompilerManifestErrorCodes(manifest.error_codes);
  if (manifest.entrypoints === undefined || manifest.entrypoints === null) {
    return;
  }
  if (!Array.isArray(manifest.entrypoints)) {
    throw new TypeError("Kotodama manifest entrypoints must be an array or null");
  }
  const names = new Set();
  const lifecycleKinds = new Set();
  manifest.entrypoints.forEach((entrypoint, index) => {
    const entry = requireRecord(entrypoint, `Kotodama manifest entrypoint ${index}`);
    const kind = requireRecord(entry.kind, `Kotodama manifest entrypoint ${index}.kind`);
    if (!isCanonicalEntrypointName(entry.name)) {
      throw new TypeError(
        `Kotodama manifest entrypoint ${index}.name is not a canonical V1 identifier or branded lifecycle selector`,
      );
    }
    if (names.has(entry.name)) {
      throw new TypeError(`Kotodama manifest contains duplicate entrypoint ${entry.name}`);
    }
    names.add(entry.name);
    if (!MANIFEST_ENTRYPOINT_KINDS.has(kind.kind)) {
      throw new TypeError(
        `Kotodama manifest entrypoint ${index}.kind must be Kotoage, View, Hajimari, or Kaizen`,
      );
    }
    if (kind.value !== null) {
      throw new TypeError(`Kotodama manifest entrypoint ${index}.kind.value must be null`);
    }
    const lifecycleKind =
      entry.name === "hajimari" || entry.name === "始まり"
        ? "Hajimari"
        : entry.name === "kaizen" || entry.name === "改善"
          ? "Kaizen"
          : null;
    if (
      (lifecycleKind === null && (kind.kind === "Hajimari" || kind.kind === "Kaizen")) ||
      (lifecycleKind !== null && kind.kind !== lifecycleKind)
    ) {
      throw new TypeError(
        `Kotodama manifest entrypoint ${index}.kind does not match its branded lifecycle selector`,
      );
    }
    if (kind.kind === "Kotoage" && (typeof entry.permission !== "string" || entry.permission.trim() === "")) {
      throw new TypeError(
        `Kotodama manifest kotoage/言挙げ entrypoint ${index} is missing caller authorization`,
      );
    }
    if ((kind.kind === "Hajimari" || kind.kind === "Kaizen") && entry.permission != null) {
      throw new TypeError(
        `Kotodama manifest hajimari/始まり and kaizen/改善 entrypoint ${index} must use runtime authorization`,
      );
    }
    if (lifecycleKind !== null) {
      if (lifecycleKinds.has(lifecycleKind)) {
        throw new TypeError(`Kotodama manifest contains duplicate ${lifecycleKind} entrypoints`);
      }
      lifecycleKinds.add(lifecycleKind);
    }
  });

}

function validateCompilerManifestStates(states) {
  if (states === undefined || states === null) {
    return;
  }
  if (!Array.isArray(states)) {
    throw new TypeError("Kotodama manifest states must be an array or null");
  }
  const names = new Set();
  states.forEach((value, index) => {
    const state = requireRecord(value, `Kotodama manifest state ${index}`);
    if (!isCanonicalIdentifier(state.name, { declaration: true })) {
      throw new TypeError(`Kotodama manifest state ${index}.name is not canonical`);
    }
    if (names.has(state.name)) {
      throw new TypeError(`Kotodama manifest contains duplicate state ${state.name}`);
    }
    names.add(state.name);
    if (typeof state.type_name !== "string" || state.type_name.trim() === "") {
      throw new TypeError(`Kotodama manifest state ${index}.type_name must not be empty`);
    }
  });
}

function validateCompilerManifestErrorCodes(errorCodes) {
  if (errorCodes === undefined || errorCodes === null) {
    return;
  }
  if (!Array.isArray(errorCodes)) {
    throw new TypeError("Kotodama manifest error_codes must be an array or null");
  }
  const paths = new Set();
  const codes = new Set();
  errorCodes.forEach((value, index) => {
    const errorCode = requireRecord(value, `Kotodama manifest error code ${index}`);
    if (
      !isCanonicalIdentifier(errorCode.namespace, { declaration: true }) ||
      !isCanonicalIdentifier(errorCode.name)
    ) {
      throw new TypeError(
        `Kotodama manifest error code ${index} must use canonical namespace and variant identifiers`,
      );
    }
    if (!Number.isSafeInteger(errorCode.code) || errorCode.code <= 0 || errorCode.code > 0xffff_ffff) {
      throw new TypeError(`Kotodama manifest error code ${index}.code must be a non-zero u32`);
    }
    const path = `${errorCode.namespace}::${errorCode.name}`;
    if (paths.has(path) || codes.has(errorCode.code)) {
      throw new TypeError(`Kotodama manifest contains a duplicate error path or code at ${path}`);
    }
    paths.add(path);
    codes.add(errorCode.code);
  });
}

function validatePosition(value, label) {
  requireRecord(value, label);
  requireExactKeys(value, ["line", "column"], label);
  if (
    !Number.isSafeInteger(value.line) ||
    value.line < 1 ||
    !Number.isSafeInteger(value.column) ||
    value.column < 1
  ) {
    throw new TypeError(`${label} must contain one-based safe-integer line and column values`);
  }
}

function validateSpan(value, label) {
  requireRecord(value, label);
  requireExactKeys(value, ["source", "start", "end", "byte_range"], label);
  if (value.source !== null && typeof value.source !== "string") {
    throw new TypeError(`${label}.source must be a string or null`);
  }
  validatePosition(value.start, `${label}.start`);
  validatePosition(value.end, `${label}.end`);
  const startsAfterEnd =
    value.start.line > value.end.line ||
    (value.start.line === value.end.line && value.start.column > value.end.column);
  if (startsAfterEnd) {
    throw new TypeError(`${label} must be a forward half-open range`);
  }
  if (value.byte_range !== null) {
    requireRecord(value.byte_range, `${label}.byte_range`);
    requireExactKeys(value.byte_range, ["start", "end"], `${label}.byte_range`);
    if (
      !Number.isSafeInteger(value.byte_range.start) ||
      value.byte_range.start < 0 ||
      !Number.isSafeInteger(value.byte_range.end) ||
      value.byte_range.end < value.byte_range.start
    ) {
      throw new TypeError(`${label}.byte_range must be a forward safe-integer byte range`);
    }
  }
}

function validateDiagnostic(value, index) {
  const label = `Kotodama diagnostic ${index}`;
  requireRecord(value, label);
  requireExactKeys(
    value,
    ["code", "severity", "phase", "message", "primary_span", "labels", "notes", "help", "fix"],
    label,
  );
  if (typeof value.code !== "string" || !/^[EK][A-Z0-9_]+$/.test(value.code)) {
    throw new TypeError(`${label}.code is not a stable Kotodama diagnostic code`);
  }
  if (!DIAGNOSTIC_SEVERITIES.has(value.severity)) {
    throw new TypeError(`${label}.severity is invalid`);
  }
  if (!DIAGNOSTIC_PHASES.has(value.phase)) {
    throw new TypeError(`${label}.phase is invalid`);
  }
  if (typeof value.message !== "string" || value.message.length === 0) {
    throw new TypeError(`${label}.message must be a non-empty string`);
  }
  if (value.primary_span !== null) {
    validateSpan(value.primary_span, `${label}.primary_span`);
  }
  if (!Array.isArray(value.labels)) {
    throw new TypeError(`${label}.labels must be an array`);
  }
  value.labels.forEach((entry, labelIndex) => {
    const entryLabel = `${label}.labels[${labelIndex}]`;
    requireRecord(entry, entryLabel);
    requireExactKeys(entry, ["span", "message"], entryLabel);
    validateSpan(entry.span, `${entryLabel}.span`);
    if (typeof entry.message !== "string") {
      throw new TypeError(`${entryLabel}.message must be a string`);
    }
  });
  if (!Array.isArray(value.notes) || value.notes.some((note) => typeof note !== "string")) {
    throw new TypeError(`${label}.notes must be an array of strings`);
  }
  if (value.help !== null && typeof value.help !== "string") {
    throw new TypeError(`${label}.help must be a string or null`);
  }
  if (value.fix !== null) {
    requireRecord(value.fix, `${label}.fix`);
    requireExactKeys(value.fix, ["span", "replacement"], `${label}.fix`);
    validateSpan(value.fix.span, `${label}.fix.span`);
    if (typeof value.fix.replacement !== "string") {
      throw new TypeError(`${label}.fix.replacement must be a string`);
    }
  }
}

function parseDiagnostics(raw) {
  const diagnostics = parseJson(raw, "Kotodama diagnosticsJson");
  if (!Array.isArray(diagnostics) || diagnostics.length === 0) {
    throw new TypeError("failed Kotodama compilation must return a non-empty diagnostic array");
  }
  if (diagnostics.length > MAX_DIAGNOSTICS) {
    throw new TypeError(`Kotodama diagnostics exceed the ${MAX_DIAGNOSTICS}-diagnostic limit`);
  }
  diagnostics.forEach(validateDiagnostic);
  if (!diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    throw new TypeError("failed Kotodama compilation must contain at least one error diagnostic");
  }
  return diagnostics;
}

/** Validate and normalize one successful canonical Rust compiler wire output. */
export function normalizeCompilerOutput(output) {
  requireRecord(output, "Kotodama compiler output");
  requireExactKeys(
    output,
    [
      "artifactBytes",
      "manifestJson",
      "codeHash",
      "abiHash",
      "sourceMapJson",
      "budgetReportJson",
    ],
    "Kotodama compiler output",
  );
  const artifactBytes = normalizeArtifactBytes(output.artifactBytes);
  const codeHashHex = normalizeHashHex(output.codeHash, "codeHash");
  const abiHashHex = normalizeHashHex(output.abiHash, "abiHash");
  const actualCodeHash = artifactHashHex(artifactBytes);
  if (actualCodeHash !== codeHashHex) {
    throw new Error("Kotodama compiler artifact bytes do not match codeHash");
  }

  const manifest = requireRecord(
    parseJson(output.manifestJson, "Kotodama manifestJson"),
    "Kotodama manifest",
  );
  if (normalizeHashHex(manifest.code_hash, "manifest code_hash") !== codeHashHex) {
    throw new Error("Kotodama compiler manifest code_hash does not match the artifact");
  }
  if (normalizeHashHex(manifest.abi_hash, "manifest abi_hash") !== abiHashHex) {
    throw new Error("Kotodama compiler manifest abi_hash does not match abiHash");
  }
  validateCompilerManifest(manifest);

  const sourceMap = parseSidecar(output.sourceMapJson, "source-map", codeHashHex);
  const budgetReport = parseSidecar(output.budgetReportJson, "budget", codeHashHex);
  return {
    artifactBytes,
    codeHashHex,
    abiHashHex,
    compilerFingerprint: manifest.compiler_fingerprint ?? "kotodama_lang",
    manifest,
    sourceMap,
    budgetReport,
  };
}

/**
 * Normalize the canonical Rust `Result<CompileOutput, DiagnosticBundle>` envelope.
 * Compiler failures remain structured data; malformed/internal failures throw.
 */
export function normalizeCompilerResult(result) {
  requireRecord(result, "Kotodama compiler result");
  requireOnlyKeys(result, ["ok", "output", "diagnosticsJson"], "Kotodama compiler result");
  if (result.ok === true) {
    if (result.diagnosticsJson !== null && result.diagnosticsJson !== undefined) {
      throw new TypeError("successful Kotodama compilation must not contain diagnosticsJson");
    }
    return { ok: true, output: normalizeCompilerOutput(result.output) };
  }
  if (result.ok === false) {
    if (result.output !== null && result.output !== undefined) {
      throw new TypeError("failed Kotodama compilation must not contain output");
    }
    return { ok: false, diagnostics: parseDiagnostics(result.diagnosticsJson) };
  }
  throw new TypeError("Kotodama compiler result.ok must be a boolean");
}
