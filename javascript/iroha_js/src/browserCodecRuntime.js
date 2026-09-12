// Internal lifecycle and transport owner. This module is not a package export.
const METHODS = Object.freeze([
  "accountAddressParseEncoded",
  "accountAddressRender",
  "noritoEncodeInstruction",
  "noritoDecodeInstruction",
  "noritoEncodeInstructionBoxArchive",
  "noritoDecodeInstructionBoxArchive",
]);

export class BrowserCodecError extends Error {
  constructor(code, message, options) {
    super(message, options);
    this.name = "BrowserCodecError";
    Object.defineProperty(this, "code", { value: code, enumerable: true });
  }
}

function invalidOutput() {
  return new BrowserCodecError(
    "ERR_IROHA_CODEC_ABI",
    "The packaged Rust browser codec returned an invalid result.",
  );
}

function objectResult(json, keys) {
  if (typeof json !== "string") throw invalidOutput();
  let value;
  try { value = JSON.parse(json); } catch { throw invalidOutput(); }
  if (
    value === null || typeof value !== "object" || Array.isArray(value) ||
    Object.keys(value).sort().join(",") !== keys.slice().sort().join(",")
  ) throw invalidOutput();
  return value;
}

function bytesResult(value) {
  if (!(value instanceof Uint8Array)) throw invalidOutput();
  // A caller must never retain a view into mutable Wasm memory.
  return Uint8Array.from(value);
}

function stringResult(value) {
  if (typeof value !== "string") throw invalidOutput();
  return value;
}

function argumentError(message) {
  const error = new TypeError(message);
  Object.defineProperty(error, "code", { value: "InvalidArg", enumerable: true });
  return error;
}

function stringArgument(value) {
  if (typeof value !== "string") throw argumentError("Rust browser codec input must be a string");
  return value;
}

function bytesArgument(value) {
  if (!(value instanceof Uint8Array)) throw argumentError("Rust browser codec input must be a Uint8Array");
  return value;
}

function prefixArgument(value) {
  if (!Number.isInteger(value) || value < 0 || value > 65535) {
    throw argumentError("Account network prefix must be an integer in 0..65535");
  }
  return value;
}

function bindingFromModule(module) {
  const methods = Object.create(null);
  for (const name of METHODS) {
    if (!Object.prototype.hasOwnProperty.call(module, name)) throw invalidOutput();
    // Bundlers expose ES module live bindings through own getters. Resolve each
    // package-owned export once, then retain the verified function immutably.
    const method = module[name];
    if (typeof method !== "function") throw invalidOutput();
    methods[name] = method;
  }
  return Object.freeze({
    accountAddressParseEncoded(input, expectedPrefix) {
      const value = objectResult(
        methods.accountAddressParseEncoded(stringArgument(input), expectedPrefix == null ? undefined : prefixArgument(expectedPrefix)),
        ["canonicalBytes", "networkPrefix"],
      );
      if (
        !Array.isArray(value.canonicalBytes) ||
        !value.canonicalBytes.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255) ||
        !Number.isInteger(value.networkPrefix) || value.networkPrefix < 0 || value.networkPrefix > 65535
      ) throw invalidOutput();
      return {
        canonicalBytes: Uint8Array.from(value.canonicalBytes),
        networkPrefix: value.networkPrefix,
      };
    },
    accountAddressRender(bytes, networkPrefix) {
      const value = objectResult(
        methods.accountAddressRender(bytesArgument(bytes), prefixArgument(networkPrefix)),
        ["canonicalHex", "i105"],
      );
      if (typeof value.canonicalHex !== "string" || typeof value.i105 !== "string") {
        throw invalidOutput();
      }
      return value;
    },
    noritoEncodeInstruction(json) {
      return bytesResult(methods.noritoEncodeInstruction(stringArgument(json)));
    },
    noritoDecodeInstruction(bytes) {
      return stringResult(methods.noritoDecodeInstruction(bytesArgument(bytes)));
    },
    noritoEncodeInstructionBoxArchive(json) {
      return bytesResult(methods.noritoEncodeInstructionBoxArchive(stringArgument(json)));
    },
    noritoDecodeInstructionBoxArchive(bytes) {
      return stringResult(methods.noritoDecodeInstructionBoxArchive(bytesArgument(bytes)));
    },
  });
}

/** Internal dependency seam for source tests; production supplies its fixed loader. */
export function createBrowserCodecRuntime(load) {
  let binding;
  let pending;
  let phase = "uninitialized";
  return Object.freeze({
    initialize() {
      if (pending) return pending;
      phase = "initializing";
      pending = Promise.resolve().then(load).then((module) => {
        const verified = bindingFromModule(module);
        binding = verified;
        phase = "ready";
      }).catch((cause) => {
        phase = "failed";
        pending = undefined;
        throw new BrowserCodecError(
          "ERR_IROHA_CODEC_INITIALIZATION",
          "Unable to initialize the packaged Rust browser codec.",
          { cause },
        );
      });
      return pending;
    },
    binding() {
      if (binding) return binding;
      throw new BrowserCodecError(
        "ERR_IROHA_CODEC_NOT_READY",
        `Browser codec is ${phase}; await initializeBrowserCodec() before using account or instruction APIs.`,
      );
    },
  });
}

/** Fetch only the package-selected public artifact, with bounded memory and time. */
export async function fetchBrowserCodecBytes(url, fetchImpl = globalThis.fetch) {
  const maximumBytes = 64 * 1024 * 1024;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 30_000);
  let reader;
  try {
    const response = await fetchImpl(url, {
      credentials: "omit",
      redirect: "error",
      signal: controller.signal,
    });
    if (!response.ok) throw new Error(`Browser codec asset returned HTTP ${response.status}.`);
    const contentLength = response.headers.get("content-length");
    if (contentLength !== null && (!/^\d+$/.test(contentLength) || Number(contentLength) > maximumBytes)) {
      throw new Error("Browser codec asset exceeds its transport bound.");
    }
    if (!response.body || typeof response.body.getReader !== "function") {
      throw new Error("Browser codec asset requires a bounded streaming response.");
    }
    reader = response.body.getReader();
    const chunks = [];
    let length = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > maximumBytes) throw new Error("Browser codec asset exceeds its transport bound.");
      chunks.push(value);
    }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    return bytes;
  } finally {
    clearTimeout(timeout);
    controller.abort();
    if (reader) {
      await reader.cancel().catch(() => {});
      reader.releaseLock();
    }
  }
}
