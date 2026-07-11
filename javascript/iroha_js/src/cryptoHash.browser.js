import { Buffer } from "buffer";
import { sha256 } from "@noble/hashes/sha2";

const SHA256_ALIASES = new Set(["sha256", "sha-256"]);
const MAX_RANDOM_BYTES = 0x7fff_ffff;
const WEB_CRYPTO_CHUNK_BYTES = 65_536;

function invalidArgumentType(name, expected, value) {
  const received = value === null ? "null" : typeof value;
  const error = new TypeError(
    `The "${name}" argument must be ${expected}. Received ${received}`,
  );
  error.code = "ERR_INVALID_ARG_TYPE";
  return error;
}

function finalizedError() {
  const error = new Error("Digest already called");
  error.code = "ERR_CRYPTO_HASH_FINALIZED";
  return error;
}

function normalizeInput(data, inputEncoding) {
  if (typeof data === "string") {
    return Buffer.from(data, inputEncoding);
  }
  if (ArrayBuffer.isView(data)) {
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  }
  throw invalidArgumentType(
    "data",
    "of type string or an instance of Buffer, TypedArray, or DataView",
    data,
  );
}

/**
 * Browser-field replacement for the narrow crypto surface used by browser-safe
 * instruction and canonical-request entrypoints.
 */
export function createHash(algorithm) {
  if (typeof algorithm !== "string") {
    throw invalidArgumentType("algorithm", "of type string", algorithm);
  }
  if (!SHA256_ALIASES.has(algorithm.toLowerCase())) {
    throw new Error("Digest method not supported");
  }

  const hash = sha256.create();
  let finalized = false;
  const browserHash = {
    update(data, inputEncoding) {
      if (finalized) throw finalizedError();
      hash.update(normalizeInput(data, inputEncoding));
      return browserHash;
    },
    digest(outputEncoding) {
      if (finalized) throw finalizedError();
      finalized = true;
      const output = Buffer.from(hash.digest());
      if (outputEncoding === undefined) return output;
      if (outputEncoding === "hex") return output.toString("hex");
      throw new TypeError(
        "browser SHA-256 digest supports only byte and hexadecimal output",
      );
    },
  };
  return browserHash;
}

function invalidRandomSize(size) {
  const error = new RangeError(
    `The value of "size" is out of range. It must be an integer between 0 and ${MAX_RANDOM_BYTES}. Received ${String(size)}`,
  );
  error.code = "ERR_OUT_OF_RANGE";
  return error;
}

/**
 * Synchronous secure random bytes backed exclusively by Web Crypto.
 */
export function randomBytes(size) {
  if (typeof size !== "number") {
    throw invalidArgumentType("size", "of type number", size);
  }
  if (!Number.isSafeInteger(size) || size < 0 || size > MAX_RANDOM_BYTES) {
    throw invalidRandomSize(size);
  }
  const webCrypto = globalThis.crypto;
  if (!webCrypto || typeof webCrypto.getRandomValues !== "function") {
    throw new Error(
      "secure browser random bytes require globalThis.crypto.getRandomValues",
    );
  }
  const output = Buffer.alloc(size);
  for (let offset = 0; offset < output.length; offset += WEB_CRYPTO_CHUNK_BYTES) {
    const length = Math.min(WEB_CRYPTO_CHUNK_BYTES, output.length - offset);
    const chunk = new Uint8Array(
      output.buffer,
      output.byteOffset + offset,
      length,
    );
    webCrypto.getRandomValues(chunk);
  }
  return output;
}
