import { getNativeBinding } from "./native.js";

const ALIGNMENT = 16;

function cloneJson(value) {
  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value));
}

function resolveNative(method) {
  const native = getNativeBinding();
  if (!native || typeof native[method] !== "function") {
    return null;
  }
  return native;
}

/**
 * Encode an instruction JSON payload to canonical Norito bytes.
 * @param {object | string} instruction
 * @returns {Buffer}
 */
export function noritoEncodeInstruction(instruction) {
  const native = resolveNative("noritoEncodeInstruction");
  if (native) {
    if (typeof instruction === "string") {
      try {
        const parsed = JSON.parse(instruction);
        return native.noritoEncodeInstruction(JSON.stringify(parsed));
      } catch {
        const trimmed = instruction.trim();
        const decoded = tryDecodeBase64(trimmed) ?? tryDecodeHex(trimmed);
        if (decoded) {
          return decoded;
        }
        return native.noritoEncodeInstruction(instruction);
      }
    }
    return native.noritoEncodeInstruction(JSON.stringify(cloneJson(instruction)));
  }

  // JS-only fallback: emit UTF-8 JSON bytes.
  if (typeof instruction === "string") {
    const trimmed = instruction.trim();
    const decoded = tryDecodeBase64(trimmed) ?? tryDecodeHex(trimmed);
    if (decoded) {
      return decoded;
    }
    const parsed = JSON.parse(trimmed);
    return encodeJsonInstruction(parsed);
  }
  return encodeJsonInstruction(instruction);
}

/**
 * Decode canonical Norito instruction bytes back to JSON.
 *
 * When `options.parseJson !== false`, the result is the parsed JSON payload.
 * Otherwise the raw JSON string returned by the native binding is emitted.
 *
 * @param {ArrayBufferView | ArrayBuffer | Buffer} bytes
 * @param {{ parseJson?: boolean }} [options]
 * @returns {string | unknown}
 */
export function noritoDecodeInstruction(bytes, options = {}) {
  const buffer = toBuffer(bytes);
  const native = resolveNative("noritoDecodeInstruction");
  if (native) {
    let json;
    try {
      json = native.noritoDecodeInstruction(buffer);
    } catch (error) {
      if (!isAlignmentError(error)) {
        throw error;
      }
      let decoded =
        tryDecodeWithAlignedBuffer(native, buffer) ??
        tryDecodeWithRelocatedStorage(native, buffer);
      if (decoded === null) {
        throw error;
      }
      json = decoded;
    }
    if (options.parseJson === false) {
      return json;
    }
    return JSON.parse(json);
  }

  const json = buffer.toString("utf8");
  try {
    const parsed = JSON.parse(json);
    return options.parseJson === false ? json : parsed;
  } catch {
    const hint =
      "Norito decode requires the native binding; run `npm run build:native` or supply JSON-encoded bytes in JS-only mode.";
    throw new Error(hint);
  }
}

function toBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  throw new TypeError("bytes must be a Buffer, ArrayBuffer, or typed array");
}

function encodeJsonInstruction(instruction) {
  return Buffer.from(JSON.stringify(cloneJson(instruction)), "utf8");
}

function isAlignmentError(error) {
  const message = error && typeof error.message === "string" ? error.message : "";
  return message.includes("requires 16-byte alignment");
}

function tryDecodeWithAlignedBuffer(native, buffer) {
  const candidate = allocateAlignedBuffer(buffer.length);
  if (candidate === null) {
    return null;
  }
  buffer.copy(candidate);
  try {
    return native.noritoDecodeInstruction(candidate);
  } catch (inner) {
    if (isAlignmentError(inner)) {
      return null;
    }
    throw inner;
  }
}

function allocateAlignedBuffer(length) {
  if (length === 0) {
    return Buffer.alloc(0);
  }
  const candidate = Buffer.alloc(length);
  if ((candidate.byteOffset & (ALIGNMENT - 1)) === 0) {
    return candidate;
  }
  return null;
}

function tryDecodeBase64(value) {
  if (!value) {
    return null;
  }
  const compact = value.replace(/\s+/g, "");
  if (compact.length === 0 || compact.length % 4 !== 0) {
    return null;
  }
  const paddingIndex = compact.indexOf("=");
  if (paddingIndex !== -1) {
    const head = compact.slice(0, paddingIndex);
    const padding = compact.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      return null;
    }
  } else if (!/^[0-9A-Za-z+/]+$/.test(compact)) {
    return null;
  }
  try {
    const decoded = Buffer.from(compact, "base64");
    if (decoded.length === 0) {
      return null;
    }
    if (decoded.toString("base64") !== compact) {
      return null;
    }
    return decoded;
  } catch {
    return null;
  }
}

function tryDecodeHex(value) {
  if (!value || value.length % 2 !== 0) {
    return null;
  }
  const compact = value.replace(/^0x/i, "");
  if (compact.length % 2 !== 0 || /[^0-9A-Fa-f]/.test(compact)) {
    return null;
  }
  try {
    const decoded = Buffer.from(compact, "hex");
    return decoded.length > 0 ? decoded : null;
  } catch {
    return null;
  }
}

function tryDecodeWithRelocatedStorage(native, buffer) {
  const extra = ALIGNMENT - 1;
  const constructors = [];
  if (typeof SharedArrayBuffer === "function") {
    constructors.push((size) => new SharedArrayBuffer(size));
  }
  constructors.push((size) => new ArrayBuffer(size));

  for (const createStorage of constructors) {
    for (let pad = 0; pad <= extra; pad += 1) {
      let storage;
      try {
        storage = createStorage(buffer.length + extra);
      } catch {
        continue;
      }
      const raw = new Uint8Array(storage);
      raw.set(buffer, pad);
      const candidate = Buffer.from(raw.buffer, pad, buffer.length);
      if ((candidate.byteOffset & (ALIGNMENT - 1)) !== 0) {
        continue;
      }
      try {
        return native.noritoDecodeInstruction(candidate);
      } catch (inner) {
        if (isAlignmentError(inner)) {
          continue;
        }
        throw inner;
      }
    }
  }
  return null;
}
