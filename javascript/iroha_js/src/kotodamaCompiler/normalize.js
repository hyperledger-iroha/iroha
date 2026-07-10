import { blake2b256 } from "../blake2b.js";

const CONTRACT_HASH_DOMAIN = new TextEncoder().encode("iroha:ivm:contract-artifact:v1\0");
const DIAGNOSTIC_PHASES = new Set(["lex", "parse", "semantic", "lowering", "artifact"]);
const DIAGNOSTIC_SEVERITIES = new Set(["error", "warning"]);
const MAX_DIAGNOSTICS = 64;
const MAX_ARTIFACT_BYTES = 32 * 1024 * 1024;

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
  const hex = value.replace(/^(?:hash:|0x)/i, "");
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new TypeError(`Kotodama compiler response contains an invalid ${label}`);
  }
  return hex.toLowerCase();
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
  return toHex(blake2b256(input));
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
