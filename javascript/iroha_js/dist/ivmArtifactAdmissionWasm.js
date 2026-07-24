import { sha256 } from "@noble/hashes/sha2";

export const IVM_ARTIFACT_ADMISSION_MAX_INPUT_BYTES = 4 * 1024 * 1024;
const IVM_ARTIFACT_ADMISSION_MAX_OUTPUT_BYTES = 16 * 1024 * 1024;
const REQUIRED_EXPORTS = Object.freeze({
  inputPtr: "iroha_ivm_artifact_admission_input_ptr",
  verify: "iroha_ivm_artifact_admission_verify",
  outputPtr: "iroha_ivm_artifact_admission_output_ptr",
  outputLen: "iroha_ivm_artifact_admission_output_len",
});
const VERIFIER_STATE = new WeakMap();
const arrayBufferByteLengthGetter = Object.getOwnPropertyDescriptor(
  ArrayBuffer.prototype,
  "byteLength",
).get;
const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const typedArrayBufferGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "buffer",
).get;
const typedArrayByteOffsetGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteOffset",
).get;
const typedArrayByteLengthGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteLength",
).get;
const typedArraySet = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "set",
).value;
const dataViewBufferGetter = Object.getOwnPropertyDescriptor(
  DataView.prototype,
  "buffer",
).get;
const dataViewByteOffsetGetter = Object.getOwnPropertyDescriptor(
  DataView.prototype,
  "byteOffset",
).get;
const dataViewByteLengthGetter = Object.getOwnPropertyDescriptor(
  DataView.prototype,
  "byteLength",
).get;
const sharedArrayBufferByteLengthGetter =
  typeof SharedArrayBuffer === "undefined"
    ? null
    : Object.getOwnPropertyDescriptor(
        SharedArrayBuffer.prototype,
        "byteLength",
      ).get;

function isSharedArrayBuffer(value) {
  if (sharedArrayBufferByteLengthGetter === null) return false;
  try {
    sharedArrayBufferByteLengthGetter.call(value);
    return true;
  } catch {
    return false;
  }
}

function isArrayBuffer(value) {
  try {
    arrayBufferByteLengthGetter.call(value);
    return true;
  } catch {
    return false;
  }
}

function arrayBufferViewInfo(value) {
  try {
    return {
      buffer: typedArrayBufferGetter.call(value),
      byteOffset: typedArrayByteOffsetGetter.call(value),
      byteLength: typedArrayByteLengthGetter.call(value),
    };
  } catch {
    try {
      return {
        buffer: dataViewBufferGetter.call(value),
        byteOffset: dataViewByteOffsetGetter.call(value),
        byteLength: dataViewByteLengthGetter.call(value),
      };
    } catch {
      return null;
    }
  }
}

function copyBytes(value, context) {
  if (isSharedArrayBuffer(value)) {
    throw new TypeError(`${context} must not be backed by SharedArrayBuffer`);
  }
  let buffer;
  let byteOffset;
  let byteLength;
  if (isArrayBuffer(value)) {
    buffer = value;
    byteOffset = 0;
    byteLength = arrayBufferByteLengthGetter.call(value);
  } else {
    const view = arrayBufferViewInfo(value);
    if (view === null) {
      throw new TypeError(`${context} must be an ArrayBuffer or ArrayBuffer view`);
    }
    if (isSharedArrayBuffer(view.buffer)) {
      throw new TypeError(`${context} must not be backed by SharedArrayBuffer`);
    }
    ({ buffer, byteOffset, byteLength } = view);
  }
  const source = new Uint8Array(buffer, byteOffset, byteLength);
  const copy = new Uint8Array(byteLength);
  Reflect.apply(typedArraySet, copy, [source]);
  return copy;
}

async function readWasmBytes(value) {
  if (
    isSharedArrayBuffer(value) ||
    isArrayBuffer(value) ||
    arrayBufferViewInfo(value) !== null
  ) {
    return copyBytes(value, "wasmBytes");
  }
  if (value !== null && typeof value === "object" && typeof value.arrayBuffer === "function") {
    return copyBytes(await value.arrayBuffer(), "wasmBytes response body");
  }
  throw new TypeError(
    "wasmBytes must be an ArrayBuffer, ArrayBuffer view, or Response-like body",
  );
}

function requireHex32(value, context) {
  if (typeof value !== "string" || !/^[0-9a-fA-F]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be an exact 32-byte hexadecimal string`);
  }
  return value.toLowerCase();
}

function requireSha256Hex(value, context) {
  return requireHex32(value, context);
}

function requireIrohaHashHex(value, context) {
  const normalized = requireHex32(value, context);
  if ((Number.parseInt(normalized.slice(-2), 16) & 1) !== 1) {
    throw new TypeError(`${context} must carry the Iroha Hash marker bit`);
  }
  return normalized;
}

function hex(bytes) {
  let output = "";
  for (const byte of bytes) output += byte.toString(16).padStart(2, "0");
  return output;
}

function requirePlainObject(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${context} must be a plain object`);
  }
  return value;
}

function requireOnlyKeys(value, expected, context) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((key, index) => key !== wanted[index])
  ) {
    throw new Error(
      `${context} fields must be exactly ${wanted.join(", ")}`,
    );
  }
}

function requireUnsignedInteger(value, context) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${context} must be a non-negative safe integer`);
  }
  return value;
}

function deepFreeze(value) {
  const pending = [value];
  const visited = new Set();
  while (pending.length > 0) {
    const current = pending.pop();
    if (
      current === null ||
      typeof current !== "object" ||
      visited.has(current)
    ) {
      continue;
    }
    visited.add(current);
    for (const child of Object.values(current)) pending.push(child);
    Object.freeze(current);
  }
  return value;
}

function normalizeAdmissionOutput(bytes, status) {
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
  } catch (error) {
    throw new Error(`artifact admission WebAssembly returned invalid UTF-8: ${error.message}`);
  }
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    throw new Error(`artifact admission WebAssembly returned invalid JSON: ${error.message}`);
  }
  requirePlainObject(parsed, "artifact admission result");
  if (parsed.ok === true) {
    requireOnlyKeys(
      parsed,
      [
        "ok",
        "code_hash_hex",
        "abi_hash_hex",
        "header_len",
        "code_offset",
        "entrypoint_count",
        "manifest",
      ],
      "successful artifact admission result",
    );
    if (status !== 1) {
      throw new Error(
        "artifact admission WebAssembly status disagrees with its successful JSON result",
      );
    }
    const codeHashHex = requireIrohaHashHex(
      parsed.code_hash_hex,
      "artifact admission code_hash_hex",
    );
    const abiHashHex = requireIrohaHashHex(
      parsed.abi_hash_hex,
      "artifact admission abi_hash_hex",
    );
    const headerLength = requireUnsignedInteger(
      parsed.header_len,
      "artifact admission header_len",
    );
    const codeOffset = requireUnsignedInteger(
      parsed.code_offset,
      "artifact admission code_offset",
    );
    if (headerLength === 0 || codeOffset < headerLength) {
      throw new Error("artifact admission metadata offsets are inconsistent");
    }
    const entrypointCount = requireUnsignedInteger(
      parsed.entrypoint_count,
      "artifact admission entrypoint_count",
    );
    const manifest = deepFreeze(
      requirePlainObject(parsed.manifest, "artifact admission manifest"),
    );
    return Object.freeze({
      ok: true,
      codeHashHex,
      abiHashHex,
      headerLength,
      codeOffset,
      entrypointCount,
      manifest,
    });
  }
  if (parsed.ok === false) {
    requireOnlyKeys(parsed, ["ok", "error"], "failed artifact admission result");
    if (status !== 0) {
      throw new Error(
        "artifact admission WebAssembly status disagrees with its failed JSON result",
      );
    }
    if (
      typeof parsed.error !== "string" ||
      parsed.error.length === 0 ||
      /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/u.test(parsed.error)
    ) {
      throw new Error("artifact admission error must be a non-empty safe string");
    }
    return Object.freeze({ ok: false, error: parsed.error });
  }
  throw new Error("artifact admission JSON result must contain a boolean ok field");
}

function checkedMemoryRange(memory, pointer, length, context) {
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new Error("artifact admission WebAssembly must export linear memory");
  }
  if (!Number.isInteger(pointer) || pointer < 0 || pointer > 0xffff_ffff) {
    throw new Error(`${context} pointer is outside WebAssembly memory`);
  }
  if (!Number.isInteger(length) || length < 0 || length > 0xffff_ffff) {
    throw new Error(`${context} length is outside its u32 range`);
  }
  const end = pointer + length;
  if (!Number.isSafeInteger(end) || end > memory.buffer.byteLength) {
    throw new Error(`${context} range is outside WebAssembly memory`);
  }
  return new Uint8Array(memory.buffer, pointer, length);
}

function callU32(fn, context, argument) {
  const value = argument === undefined ? fn() : fn(argument);
  if (!Number.isInteger(value)) {
    throw new Error(`${context} did not return an i32`);
  }
  return value >>> 0;
}

function bindRawExports(instance) {
  if (!(instance instanceof WebAssembly.Instance)) {
    throw new Error("artifact admission WebAssembly did not instantiate");
  }
  const exports = instance.exports;
  const memory = exports.memory;
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new Error("artifact admission WebAssembly must export memory");
  }
  const functions = {};
  for (const [key, name] of Object.entries(REQUIRED_EXPORTS)) {
    if (typeof exports[name] !== "function") {
      throw new Error(`artifact admission WebAssembly is missing raw export ${name}`);
    }
    functions[key] = exports[name];
  }
  if (
    functions.inputPtr.length !== 0 ||
    functions.verify.length !== 1 ||
    functions.outputPtr.length !== 0 ||
    functions.outputLen.length !== 0
  ) {
    throw new Error("artifact admission WebAssembly raw export signatures are invalid");
  }
  return Object.freeze({ memory, ...functions });
}

function verifyWithState(state, artifactBytes) {
  const artifact = copyBytes(artifactBytes, "artifactBytes");
  if (
    artifact.length === 0 ||
    artifact.length > IVM_ARTIFACT_ADMISSION_MAX_INPUT_BYTES
  ) {
    throw new RangeError(
      `artifactBytes length must be within 1..=${IVM_ARTIFACT_ADMISSION_MAX_INPUT_BYTES}`,
    );
  }
  const inputPointer = callU32(state.inputPtr, "artifact admission input pointer");
  const input = checkedMemoryRange(
    state.memory,
    inputPointer,
    artifact.length,
    "artifact admission input",
  );
  input.set(artifact);
  let output;
  let status;
  try {
    status = callU32(
      state.verify,
      "artifact admission verification status",
      artifact.length,
    );
    if (status !== 0 && status !== 1) {
      throw new Error("artifact admission WebAssembly status must be 0 or 1");
    }
    const outputPointer = callU32(
      state.outputPtr,
      "artifact admission output pointer",
    );
    const outputLength = callU32(
      state.outputLen,
      "artifact admission output length",
    );
    if (
      outputLength === 0 ||
      outputLength > IVM_ARTIFACT_ADMISSION_MAX_OUTPUT_BYTES
    ) {
      throw new Error("artifact admission WebAssembly output length is invalid");
    }
    output = Uint8Array.from(
      checkedMemoryRange(
        state.memory,
        outputPointer,
        outputLength,
        "artifact admission output",
      ),
    );
  } finally {
    try {
      checkedMemoryRange(
        state.memory,
        inputPointer,
        artifact.length,
        "artifact admission input",
      ).fill(0);
    } catch {
      // A module that invalidates its input memory will fail output validation;
      // clearing remains best effort because the artifact itself is public.
    }
  }
  return normalizeAdmissionOutput(output, status);
}

/**
 * Instantiate the exact raw `ivm_artifact_admission` WebAssembly boundary.
 *
 * The expected SHA-256 digest is mandatory: callers must anchor the verifier
 * bytes in their signed application/release metadata instead of trusting a
 * mutable URL response at deployment time.
 */
export async function instantiateIvmArtifactAdmissionWasm({
  wasmBytes,
  expectedSha256Hex,
  imports = {},
}) {
  if (typeof WebAssembly !== "object") {
    throw new Error("WebAssembly is unavailable in this browser environment");
  }
  const bytes = await readWasmBytes(wasmBytes);
  const expectedDigest = requireSha256Hex(
    expectedSha256Hex,
    "expectedSha256Hex",
  );
  const actualDigest = hex(sha256(bytes));
  if (actualDigest !== expectedDigest) {
    throw new Error(
      `artifact admission WebAssembly SHA-256 mismatch: expected ${expectedDigest}, got ${actualDigest}`,
    );
  }
  let instantiated;
  try {
    instantiated = await WebAssembly.instantiate(bytes, imports);
  } catch (error) {
    throw new Error(`artifact admission WebAssembly failed to instantiate: ${error.message}`);
  }
  const state = {
    ...bindRawExports(instantiated.instance),
    module: instantiated.module,
    verifierSha256Hex: actualDigest,
  };
  const verifier = Object.freeze({
    verifierSha256Hex: actualDigest,
    verify(artifactBytes) {
      if (VERIFIER_STATE.get(verifier) !== state) {
        throw new Error("artifact admission verifier is not authentic");
      }
      return verifyWithState(state, artifactBytes);
    },
  });
  VERIFIER_STATE.set(verifier, state);
  return verifier;
}

/** Verify an artifact using a verifier created by this module. */
export function verifyIvmContractArtifactAdmission(verifier, artifactBytes) {
  const state =
    verifier !== null && typeof verifier === "object"
      ? VERIFIER_STATE.get(verifier)
      : undefined;
  if (state === undefined) {
    throw new TypeError(
      "artifactAdmissionVerifier must come from instantiateIvmArtifactAdmissionWasm",
    );
  }
  return verifyWithState(state, artifactBytes);
}
