import { normalizeCompilerResult } from "./normalize.js";

const DEFAULT_COMPILE_PATH = "/v1/kotodama/compile";
const DEFAULT_COMPILER_TIMEOUT_MS = 30_000;
const MAX_COMPILER_TIMEOUT_MS = 120_000;
const COMPILER_REQUEST_OPTION_NAMES = new Set(["sourceName", "sources", "imports", "packages", "zk"]);
const COMPILER_CALL_OPTION_NAMES = new Set([
  ...COMPILER_REQUEST_OPTION_NAMES,
  "signal",
  "timeoutMs",
]);
const COMPILER_OPTION_NAMES = new Set([
  "compilerUrl",
  "fetchImpl",
  ...COMPILER_CALL_OPTION_NAMES,
]);
const COMPILER_CLIENT_OPTION_NAMES = new Set(["fetchImpl"]);
const MAX_COMPILER_SOURCE_BYTES = 1024 * 1024;
const MAX_COMPILER_SOURCE_NAME_BYTES = 4096;
const MAX_COMPILER_RESPONSE_BYTES = 16 * 1024 * 1024;
const MAX_COMPILER_ERROR_BYTES = 64 * 1024;
const MAX_COMPILER_RESPONSE_CHUNKS = 65_536;

const DefaultFetch = globalThis.fetch;
const AbortControllerIntrinsic = globalThis.AbortController;
const abortControllerAbort = AbortControllerIntrinsic?.prototype?.abort ?? null;
const abortControllerSignalGetter = AbortControllerIntrinsic
  ? (Object.getOwnPropertyDescriptor(AbortControllerIntrinsic.prototype, "signal")
      ?.get ?? null)
  : null;
const abortSignalAbortedGetter = globalThis.AbortSignal
  ? (Object.getOwnPropertyDescriptor(AbortSignal.prototype, "aborted")?.get ?? null)
  : null;
const abortSignalReasonGetter = globalThis.AbortSignal
  ? (Object.getOwnPropertyDescriptor(AbortSignal.prototype, "reason")?.get ?? null)
  : null;
const eventTargetAddEventListener = globalThis.EventTarget?.prototype?.addEventListener ?? null;
const eventTargetRemoveEventListener =
  globalThis.EventTarget?.prototype?.removeEventListener ?? null;
const responseOkGetter = globalThis.Response
  ? (Object.getOwnPropertyDescriptor(Response.prototype, "ok")?.get ?? null)
  : null;
const responseStatusGetter = globalThis.Response
  ? (Object.getOwnPropertyDescriptor(Response.prototype, "status")?.get ?? null)
  : null;
const responseRedirectedGetter = globalThis.Response
  ? (Object.getOwnPropertyDescriptor(Response.prototype, "redirected")?.get ?? null)
  : null;
const responseHeadersGetter = globalThis.Response
  ? (Object.getOwnPropertyDescriptor(Response.prototype, "headers")?.get ?? null)
  : null;
const responseBodyGetter = globalThis.Response
  ? (Object.getOwnPropertyDescriptor(Response.prototype, "body")?.get ?? null)
  : null;
const headersGet = globalThis.Headers?.prototype?.get ?? null;
const readableStreamGetReader = globalThis.ReadableStream?.prototype?.getReader ?? null;
const readerRead = globalThis.ReadableStreamDefaultReader?.prototype?.read ?? null;
const readerCancel = globalThis.ReadableStreamDefaultReader?.prototype?.cancel ?? null;
const readerReleaseLock =
  globalThis.ReadableStreamDefaultReader?.prototype?.releaseLock ?? null;
const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const typedArrayBufferGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "buffer",
)?.get;
const typedArrayByteOffsetGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteOffset",
)?.get;
const typedArrayByteLengthGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  "byteLength",
)?.get;
const typedArrayTagGetter = Object.getOwnPropertyDescriptor(
  typedArrayPrototype,
  Symbol.toStringTag,
)?.get;
const sharedArrayBufferByteLengthGetter = globalThis.SharedArrayBuffer
  ? (Object.getOwnPropertyDescriptor(SharedArrayBuffer.prototype, "byteLength")?.get ??
    null)
  : null;
const Uint8ArrayIntrinsic = Uint8Array;
const uint8ArraySet = Uint8Array.prototype.set;
const TextEncoderIntrinsic = TextEncoder;
const textEncoderEncode = TextEncoder.prototype.encode;
const TextDecoderIntrinsic = TextDecoder;
const textDecoderDecode = TextDecoder.prototype.decode;
const setTimeoutIntrinsic = globalThis.setTimeout;
const clearTimeoutIntrinsic = globalThis.clearTimeout;

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
  const sourceBytes = Reflect.apply(textEncoderEncode, new TextEncoderIntrinsic(), [
    source,
  ]).length;
  if (sourceBytes > MAX_COMPILER_SOURCE_BYTES) {
    throw new RangeError(
      `Kotodama source exceeds the ${MAX_COMPILER_SOURCE_BYTES}-byte V1 limit`,
    );
  }
}

function canonicalizeCompilerOptions(options, allowedNames) {
  if (options === undefined) {
    return Object.create(null);
  }
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("Kotodama compiler options must be an object");
  }
  const prototype = Object.getPrototypeOf(options);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError("Kotodama compiler options must be a plain data object");
  }
  const canonical = Object.create(null);
  for (const name of Reflect.ownKeys(options)) {
    if (typeof name !== "string") {
      throw new TypeError("Kotodama compiler options must not contain symbol fields");
    }
    if (!allowedNames.has(name)) {
      throw new TypeError(`unknown Kotodama compiler option '${name}'`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(options, name);
    if (
      descriptor === undefined ||
      !descriptor.enumerable ||
      !("value" in descriptor)
    ) {
      throw new TypeError(
        `Kotodama compiler option '${name}' must be an enumerable data property`,
      );
    }
    canonical[name] = descriptor.value;
  }
  return canonical;
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
    const sourceNameBytes = Reflect.apply(
      textEncoderEncode,
      new TextEncoderIntrinsic(),
      [options.sourceName],
    ).length;
    if (sourceNameBytes > MAX_COMPILER_SOURCE_NAME_BYTES) {
      throw new RangeError(
        `sourceName exceeds the ${MAX_COMPILER_SOURCE_NAME_BYTES}-byte limit`,
      );
    }
  }
  if (Object.hasOwn(options, "zk") && typeof options.zk !== "boolean") {
    throw new TypeError("zk must be a boolean");
  }
  if (["sources", "imports", "packages"].some((key) => Object.hasOwn(options, key))) {
    if (options.sourceName === undefined) throw new TypeError("sourceName is required when sources are supplied");
    const names = new Set([canonicalSourcePath(options.sourceName)]);
    if (Object.hasOwn(options, "sources")) options.sources = canonicalSourceFiles(options.sources, names);
    if (Object.hasOwn(options, "imports")) options.imports = canonicalSourceImports(options.imports);
    if (Object.hasOwn(options, "packages")) {
      const identities = new Set();
      options.packages = canonicalDataArray(options.packages, "packages").map((value) => {
        const pkg = canonicalizeCompilerOptions(value, new Set(["identity", "modules", "sources", "exports", "imports"]));
        validateGraphIdentifier(pkg.identity, "package identity");
        if (identities.has(pkg.identity)) throw new TypeError("duplicate package identity");
        identities.add(pkg.identity);
        const paths = new Set();
        const modules = canonicalSourceFiles(pkg.modules, paths);
        const sources = canonicalSourceFiles(pkg.sources ?? [], paths);
        const exports = canonicalDataArray(pkg.exports, "exports");
        const exported = new Set();
        for (const name of exports) {
          validateGraphIdentifier(name, "package export");
          if (exported.has(name)) throw new TypeError("duplicate package export");
          exported.add(name);
        }
        return { identity: pkg.identity, modules, sources, exports, imports: canonicalSourceImports(pkg.imports ?? []) };
      });
    }
    const count = 1 + (options.sources?.length ?? 0) + (options.packages ?? []).reduce((total, pkg) => total + pkg.modules.length + pkg.sources.length, 0);
    if (count > 512) throw new RangeError("a Kotodama source set permits at most 512 files including its root");
  }
  return options;
}

function canonicalDataArray(value, label) {
  if (!Array.isArray(value)) throw new TypeError(`${label} must be an array`);
  if (value.length > 512) throw new RangeError(`${label} exceeds the 512-item limit`);
  const result = [];
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor || !Object.hasOwn(descriptor, "value")) throw new TypeError(`${label} must contain inert data entries`);
    result.push(descriptor.value);
  }
  return result;
}

function validateGraphIdentifier(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > 4096 || !validateUnicodeScalarString(value) || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
    throw new TypeError(`${label} must be a bounded nonempty string`);
  }
}

function canonicalSourceFiles(files, names) {
  return canonicalDataArray(files, "sources").map((source) => {
    const file = canonicalizeCompilerOptions(source, new Set(["sourceName", "source"]));
    if (file.sourceName === undefined) throw new TypeError("each source file requires sourceName");
    validateCompilerRequestFields({ sourceName: file.sourceName });
    validateCompilerSource(file.source);
    const name = canonicalSourcePath(file.sourceName);
    if (names.has(name)) throw new TypeError(`duplicate Kotodama source path '${name}'`);
    names.add(name);
    return { sourceName: name, source: file.source };
  });
}

function canonicalSourceImports(imports) {
  const aliases = new Set();
  return canonicalDataArray(imports, "imports").map((value) => {
    const binding = canonicalizeCompilerOptions(value, new Set(["alias", "package"]));
    validateGraphIdentifier(binding.alias, "import alias");
    validateGraphIdentifier(binding.package, "import package");
    if (aliases.has(binding.alias)) throw new TypeError("duplicate import alias");
    aliases.add(binding.alias);
    return { alias: binding.alias, package: binding.package };
  });
}

function canonicalSourcePath(name) {
  if (/^(?:[\\/]|[A-Za-z]:)/u.test(name) || name.includes(":")) {
    throw new TypeError("source paths must be relative to the source-set root");
  }
  const parts = [];
  for (const part of name.replaceAll("\\", "/").split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") {
      if (parts.length === 0) throw new TypeError("source path escapes the source-set root");
      parts.pop();
    } else {
      if (/^\.+$/u.test(part)) throw new TypeError("source path contains a nonportable component");
      parts.push(part);
    }
  }
  if (parts.length === 0) throw new TypeError("source path must name a file");
  return parts.join("/");
}

function validateCompilerRequestOptions(options) {
  return validateCompilerRequestFields(
    canonicalizeCompilerOptions(options, COMPILER_REQUEST_OPTION_NAMES),
  );
}

function validateAbortSignal(signal) {
  if (abortSignalAbortedGetter === null) {
    throw new TypeError("Kotodama compiler options.signal requires AbortSignal support");
  }
  try {
    Reflect.apply(abortSignalAbortedGetter, signal, []);
  } catch {
    throw new TypeError("Kotodama compiler options.signal must be an AbortSignal");
  }
}

function validateCompilerTransportFields(options) {
  if (Object.hasOwn(options, "signal")) {
    validateAbortSignal(options.signal);
  }
  if (Object.hasOwn(options, "timeoutMs")) {
    if (
      !Number.isInteger(options.timeoutMs) ||
      options.timeoutMs <= 0 ||
      options.timeoutMs > MAX_COMPILER_TIMEOUT_MS
    ) {
      throw new RangeError(
        `timeoutMs must be an integer from 1 through ${MAX_COMPILER_TIMEOUT_MS}`,
      );
    }
  }
  return options;
}

function validateCompilerCallOptions(options) {
  return validateCompilerTransportFields(
    validateCompilerRequestFields(
      canonicalizeCompilerOptions(options, COMPILER_CALL_OPTION_NAMES),
    ),
  );
}

export function validateCompilerOptions(options) {
  options = validateCompilerTransportFields(
    validateCompilerRequestFields(
      canonicalizeCompilerOptions(options, COMPILER_OPTION_NAMES),
    ),
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
  if (["sources", "imports", "packages"].some((key) => options[key] !== undefined)) {
    const files = [...(options.sources ?? []), ...(options.packages ?? []).flatMap((pkg) => [...pkg.modules, ...pkg.sources])];
    const bytes = [source, ...files.map((file) => file.source)].reduce(
      (total, text) => total + Reflect.apply(textEncoderEncode, new TextEncoderIntrinsic(), [text]).length,
      0,
    );
    if (bytes > 16 * 1024 * 1024) throw new RangeError("Kotodama source set exceeds the 16777216-byte limit");
    request.sourceName = canonicalSourcePath(request.sourceName);
    for (const key of ["sources", "imports", "packages"]) {
      if (options[key] !== undefined) request[key] = options[key];
    }
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

/** Select request and transport policy for a remote compiler invocation. */
export function selectCompilerCallOptions(options) {
  const selected = {};
  for (const name of COMPILER_CALL_OPTION_NAMES) {
    if (Object.hasOwn(options, name)) {
      selected[name] = options[name];
    }
  }
  return selected;
}

function signalIsAborted(signal) {
  return Reflect.apply(abortSignalAbortedGetter, signal, []);
}

function signalAbortReason(signal) {
  if (abortSignalReasonGetter !== null) {
    return Reflect.apply(abortSignalReasonGetter, signal, []);
  }
  const error = new Error("Kotodama compiler request was aborted");
  error.name = "AbortError";
  return error;
}

function createCompilerOperation(signal, timeoutMs) {
  if (
    typeof AbortControllerIntrinsic !== "function" ||
    abortControllerAbort === null ||
    abortControllerSignalGetter === null ||
    eventTargetAddEventListener === null ||
    eventTargetRemoveEventListener === null
  ) {
    throw new Error("Kotodama compiler service requires AbortController support");
  }

  const controller = new AbortControllerIntrinsic();
  const transportSignal = Reflect.apply(abortControllerSignalGetter, controller, []);
  let cancelled = false;
  let cancellationReason;
  let rejectCancellation;
  const cancellation = new Promise((_, reject) => {
    rejectCancellation = reject;
  });
  // The cancellation may win a synchronous preflight race. Keep the losing
  // promise handled after every operation path has cleaned up.
  cancellation.catch(() => {});

  const cancel = (reason) => {
    if (cancelled) return;
    cancelled = true;
    cancellationReason = reason;
    // Publish the authoritative caller/deadline rejection before notifying
    // transport listeners, which may synchronously reject with another value.
    rejectCancellation(reason);
    try {
      Reflect.apply(abortControllerAbort, controller, [reason]);
    } catch {
      // The local rejection remains authoritative if transport abort fails.
    }
  };
  const onCallerAbort = () => cancel(signalAbortReason(signal));

  let callerListenerInstalled = false;
  if (signal !== undefined) {
    if (signalIsAborted(signal)) {
      onCallerAbort();
    } else {
      Reflect.apply(eventTargetAddEventListener, signal, [
        "abort",
        onCallerAbort,
        { once: true },
      ]);
      callerListenerInstalled = true;
      // Close the check/listen race without trusting any instance property.
      if (signalIsAborted(signal)) onCallerAbort();
    }
  }

  let timerId;
  if (!cancelled) {
    timerId = Reflect.apply(setTimeoutIntrinsic, globalThis, [
      () => {
        const error = new Error(
          `Kotodama compiler request timed out after ${timeoutMs}ms`,
        );
        error.name = "TimeoutError";
        cancel(error);
      },
      timeoutMs,
    ]);
  }

  return {
    signal: transportSignal,
    race(promise) {
      return Promise.race([promise, cancellation]).then(
        (value) => {
          if (cancelled) throw cancellationReason;
          return value;
        },
        (error) => {
          if (cancelled) throw cancellationReason;
          throw error;
        },
      );
    },
    throwIfCancelled() {
      if (cancelled) throw cancellationReason;
    },
    isCancelled() {
      return cancelled;
    },
    cancellationReason() {
      return cancellationReason;
    },
    cleanup() {
      if (timerId !== undefined) {
        Reflect.apply(clearTimeoutIntrinsic, globalThis, [timerId]);
        timerId = undefined;
      }
      if (callerListenerInstalled) {
        try {
          Reflect.apply(eventTargetRemoveEventListener, signal, [
            "abort",
            onCallerAbort,
          ]);
        } catch {
          // Listener removal is cleanup and must not replace the result.
        }
        callerListenerInstalled = false;
      }
    },
  };
}

function responseMetadata(response) {
  if (
    response === null ||
    typeof response !== "object" ||
    responseOkGetter === null ||
    responseStatusGetter === null ||
    responseRedirectedGetter === null ||
    responseHeadersGetter === null ||
    responseBodyGetter === null
  ) {
    throw new TypeError("Kotodama compiler fetch returned an invalid Response");
  }
  try {
    const ok = Reflect.apply(responseOkGetter, response, []);
    const status = Reflect.apply(responseStatusGetter, response, []);
    const redirected = Reflect.apply(responseRedirectedGetter, response, []);
    const headers = Reflect.apply(responseHeadersGetter, response, []);
    const body = Reflect.apply(responseBodyGetter, response, []);
    if (
      typeof ok !== "boolean" ||
      !Number.isInteger(status) ||
      status < 100 ||
      status > 599 ||
      typeof redirected !== "boolean"
    ) {
      throw new TypeError("invalid Response metadata");
    }
    return { ok, status, redirected, headers, body };
  } catch (error) {
    throw new TypeError("Kotodama compiler fetch returned an invalid Response", {
      cause: error,
    });
  }
}

function headerValue(headers, name, label) {
  if (headersGet === null) {
    throw new TypeError(`${label} does not expose standards-compliant headers`);
  }
  try {
    return Reflect.apply(headersGet, headers, [name]);
  } catch (error) {
    throw new TypeError(`${label} does not expose standards-compliant headers`, {
      cause: error,
    });
  }
}

function contentLength(headers, label) {
  const raw = headerValue(headers, "content-length", label);
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

function validateIdentityContentEncoding(headers, label) {
  const encoding = headerValue(headers, "content-encoding", label);
  if (encoding !== null && encoding !== undefined && encoding.toLowerCase() !== "identity") {
    throw new TypeError(
      `${label} Content-Encoding must be absent or exactly identity`,
    );
  }
}

function cancelReaderBestEffort(reader, reason) {
  if (readerCancel === null) return;
  try {
    const cancellation = Reflect.apply(readerCancel, reader, [reason]);
    Promise.resolve(cancellation).catch(() => {});
  } catch {
    // Cancellation is cleanup and must not replace the authoritative error.
  }
}

function releaseReaderBestEffort(reader) {
  if (readerReleaseLock === null) return;
  try {
    Reflect.apply(readerReleaseLock, reader, []);
  } catch {
    // Lock release is cleanup and must not replace the authoritative result.
  }
}

function cancelResponseBestEffort(response, reason) {
  try {
    const body = Reflect.apply(responseBodyGetter, response, []);
    if (body === null || readableStreamGetReader === null) return;
    const reader = Reflect.apply(readableStreamGetReader, body, []);
    cancelReaderBestEffort(reader, reason);
    releaseReaderBestEffort(reader);
  } catch {
    // A late or malformed response cannot replace the authoritative result.
  }
}

function snapshotByteChunk(value, label, remainingBytes, limit) {
  let buffer;
  let byteOffset;
  let byteLength;
  try {
    if (Reflect.apply(typedArrayTagGetter, value, []) !== "Uint8Array") {
      throw new TypeError("not Uint8Array");
    }
    buffer = Reflect.apply(typedArrayBufferGetter, value, []);
    byteOffset = Reflect.apply(typedArrayByteOffsetGetter, value, []);
    byteLength = Reflect.apply(typedArrayByteLengthGetter, value, []);
  } catch {
    throw new TypeError(`${label} yielded a non-byte response chunk`);
  }
  if (sharedArrayBufferByteLengthGetter !== null) {
    let isShared = false;
    try {
      Reflect.apply(sharedArrayBufferByteLengthGetter, buffer, []);
      isShared = true;
    } catch {
      // Normal ArrayBuffers fail the SharedArrayBuffer brand check.
    }
    if (isShared) {
      throw new TypeError(`${label} yielded a SharedArrayBuffer-backed chunk`);
    }
  }
  if (byteLength === 0) {
    throw new TypeError(`${label} yielded an empty non-progress response chunk`);
  }
  if (byteLength > remainingBytes) {
    throw new RangeError(`${label} exceeds the ${limit}-byte response limit`);
  }
  const snapshot = new Uint8ArrayIntrinsic(byteLength);
  try {
    const view = new Uint8ArrayIntrinsic(buffer, byteOffset, byteLength);
    Reflect.apply(uint8ArraySet, snapshot, [view]);
  } catch (error) {
    throw new TypeError(`${label} yielded an unstable response chunk`, {
      cause: error,
    });
  }
  return snapshot;
}

async function readBoundedResponseBytes(metadata, limit, label, operation) {
  const declaredLength = contentLength(metadata.headers, label);
  if (declaredLength !== null && declaredLength > limit) {
    throw new RangeError(`${label} exceeds the ${limit}-byte response limit`);
  }
  if (metadata.body === null) {
    if (declaredLength !== null && declaredLength !== 0) {
      throw new TypeError(
        `${label} body length does not match its Content-Length header`,
      );
    }
    return new Uint8ArrayIntrinsic();
  }
  if (readableStreamGetReader === null || readerRead === null) {
    throw new TypeError(`${label} does not expose a standards-compliant readable body`);
  }

  let reader;
  try {
    reader = Reflect.apply(readableStreamGetReader, metadata.body, []);
  } catch (error) {
    throw new TypeError(
      `${label} does not expose a standards-compliant readable body`,
      { cause: error },
    );
  }
  const chunks = [];
  let total = 0;
  try {
    for (;;) {
      operation.throwIfCancelled();
      const read = Promise.resolve().then(() =>
        Reflect.apply(readerRead, reader, []),
      );
      const { done, value } = await operation.race(read);
      if (typeof done !== "boolean") {
        throw new TypeError(`${label} returned an invalid stream read result`);
      }
      if (done) {
        if (value !== undefined) {
          throw new TypeError(`${label} returned data after the stream ended`);
        }
        break;
      }
      if (chunks.length >= MAX_COMPILER_RESPONSE_CHUNKS) {
        throw new RangeError(`${label} yielded too many fragmented response chunks`);
      }
      const chunk = snapshotByteChunk(value, label, limit - total, limit);
      total += chunk.length;
      if (total > limit) {
        throw new RangeError(`${label} exceeds the ${limit}-byte response limit`);
      }
      chunks.push(chunk);
    }
  } catch (error) {
    cancelReaderBestEffort(reader, error);
    throw error;
  } finally {
    releaseReaderBestEffort(reader);
  }
  operation.throwIfCancelled();
  if (declaredLength !== null && total !== declaredLength) {
    throw new TypeError(
      `${label} body length does not match its Content-Length header`,
    );
  }
  const bytes = new Uint8ArrayIntrinsic(total);
  let offset = 0;
  for (const chunk of chunks) {
    Reflect.apply(uint8ArraySet, bytes, [chunk, offset]);
    offset += chunk.length;
  }
  return bytes;
}

async function readBoundedResponseText(metadata, limit, label, operation) {
  const bytes = await readBoundedResponseBytes(metadata, limit, label, operation);
  try {
    return Reflect.apply(
      textDecoderDecode,
      new TextDecoderIntrinsic("utf-8", { fatal: true }),
      [bytes],
    );
  } catch {
    throw new TypeError(`${label} is not valid UTF-8`);
  }
}

async function readCompilerResult(metadata, operation) {
  const text = await readBoundedResponseText(
    metadata,
    MAX_COMPILER_RESPONSE_BYTES,
    "Kotodama compiler response",
    operation,
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
  #baseUrl;

  #fetchImpl;

  constructor(baseUrl, options = {}) {
    if (typeof baseUrl !== "string" || baseUrl.length === 0) {
      throw new TypeError("Kotodama compiler baseUrl must be a non-empty string");
    }
    options = canonicalizeCompilerOptions(options, COMPILER_CLIENT_OPTION_NAMES);
    const fetchImpl = Object.hasOwn(options, "fetchImpl")
      ? options.fetchImpl
      : DefaultFetch;
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
    // Keep the validated transport policy in private slots. Public properties
    // can be added by callers for compatibility, but cannot redirect a later
    // compilation around the constructor's HTTPS/loopback boundary.
    this.#baseUrl = parsed.href.replace(/\/$/, "");
    this.#fetchImpl = fetchImpl;
  }

  async compile(source, options = {}) {
    options = validateCompilerCallOptions(options);
    const request = buildCompilerRequest(
      source,
      selectCompilerRequestOptions(options),
    );
    const timeoutMs = options.timeoutMs ?? DEFAULT_COMPILER_TIMEOUT_MS;
    const operation = createCompilerOperation(options.signal, timeoutMs);
    let response;
    try {
      operation.throwIfCancelled();
      const fetchPromise = Promise.resolve().then(() =>
        Reflect.apply(this.#fetchImpl, undefined, [
          `${this.#baseUrl}${DEFAULT_COMPILE_PATH}`,
          {
            method: "POST",
            headers: {
              accept: "application/json",
              "content-type": "application/json",
            },
            cache: "no-store",
            credentials: "omit",
            redirect: "error",
            referrerPolicy: "no-referrer",
            signal: operation.signal,
            body: JSON.stringify(request),
          },
        ]),
      );
      // If an injected Fetch ignores abort and resolves after our boundary has
      // rejected, drain/cancel its body without reviving the operation.
      fetchPromise.then(
        (lateResponse) => {
          if (operation.isCancelled()) {
            cancelResponseBestEffort(
              lateResponse,
              operation.cancellationReason(),
            );
          }
        },
        () => {},
      );
      response = await operation.race(fetchPromise);
      operation.throwIfCancelled();
      const metadata = responseMetadata(response);
      try {
        validateIdentityContentEncoding(
          metadata.headers,
          "Kotodama compiler response",
        );
      } catch (error) {
        cancelResponseBestEffort(response, error);
        throw error;
      }
      if (metadata.redirected) {
        cancelResponseBestEffort(response, "redirected compiler response rejected");
        throw new TypeError("Kotodama compiler service redirects are forbidden");
      }
      if (!metadata.ok) {
        const detail = await readBoundedResponseText(
          metadata,
          MAX_COMPILER_ERROR_BYTES,
          "Kotodama compiler error response",
          operation,
        );
        const suffix = detail.length === 0 ? "" : `: ${detail}`;
        throw new Error(
          `Kotodama compiler service failed (${metadata.status})${suffix}`,
        );
      }
      if (metadata.status !== 200) {
        cancelResponseBestEffort(response, "unexpected compiler success status");
        throw new TypeError(
          `Kotodama compiler service returned unexpected success status ${metadata.status}`,
        );
      }
      if (headerValue(metadata.headers, "content-type", "Kotodama compiler response") !== "application/json") {
        cancelResponseBestEffort(response, "invalid compiler response media type");
        throw new TypeError(
          "Kotodama compiler response Content-Type must be exactly application/json",
        );
      }
      const result = await readCompilerResult(metadata, operation);
      operation.throwIfCancelled();
      return result;
    } catch (error) {
      if (response !== undefined) {
        cancelResponseBestEffort(response, error);
      }
      throw error;
    } finally {
      operation.cleanup();
    }
  }
}
