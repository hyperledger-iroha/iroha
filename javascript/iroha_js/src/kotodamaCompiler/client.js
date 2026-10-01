const propertyDescriptor = Object.getOwnPropertyDescriptor.bind(Object);
const hasOwn = Object.hasOwn.bind(Object);
const applyIntrinsic = Reflect.apply.bind(Reflect);
const TEXT_KOTODAMA_COMPILER = "Kotodama compiler ";
const TEXT_MUST_BE = "must be ";
const TEXT_KOTODAMA_SOURCE = "Kotodama source ";
const TEXT_EXCEEDS_THE = "exceeds the ";
const TEXT_RESPONSE = "response ";
const TEXT_STANDARDS_COMPLIANT = "standards-compliant ";
const TEXT_BODY_LENGTH_DOES_NOT_MATCH_ITS_CONTENT_LENGTH_HEADER = "body length does not match its Content-Length header";
const TEXT_MUST_NOT_CONTAIN = "must not contain ";
const TEXT_DOES_NOT_EXPOSE = "does not expose ";
const TEXT_MUST_CONTAIN_VALID_UNICODE_SCALAR_VALUES = "must contain valid Unicode scalar values";
const TEXT_RETURNED_AN_INVALID = "returned an invalid ";
const TEXT_SOURCENAME = "sourceName ";
const TEXT_STRING = "string";
const TEXT_PACKAGES = "packages";
const TEXT_BASEURL = "baseUrl ";
const TEXT_SOURCES = "sources";
const TEXT_IMPORTS = "imports";
const TEXT_APPLICATION = "application";
const TEXT_SOURCENAME_SHARED = "sourceName";
const TEXT_DUPLICATE = "duplicate ";
const TEXT_IDENTITY = "identity";
const TEXT_SERVICE = "service ";
const TEXT_YIELDED = "yielded ";
const TEXT_FETCHIMPL = "fetchImpl";
const TEXT_A_NON_EMPTY = "a non-empty ";
const TEXT_REQUIRES = "requires ";
const TEXT_SOURCE_PATH = "source path ";
const TEXT_BOOLEAN = "boolean";
const TEXT_THE_SOURCE_SET_ROOT = "the source-set root";
function rejectError(ErrorType, ...args) { throw new ErrorType(...args); }
import { normalizeCompilerResult } from "./normalize.js";

const DEFAULT_COMPILE_PATH = "/v1/kotodama/compile";
const DEFAULT_COMPILER_TIMEOUT_MS = 30_000;
const MAX_COMPILER_TIMEOUT_MS = 120_000;
const COMPILER_REQUEST_OPTION_NAMES = new Set([(TEXT_SOURCENAME_SHARED), (TEXT_SOURCES), (TEXT_IMPORTS), (TEXT_PACKAGES), "zk"]);
const COMPILER_CALL_OPTION_NAMES = new Set([
  ...COMPILER_REQUEST_OPTION_NAMES,
  "signal",
  "timeoutMs",
]);
const COMPILER_OPTION_NAMES = new Set([
  "compilerUrl",
  (TEXT_FETCHIMPL),
  ...COMPILER_CALL_OPTION_NAMES,
]);
const COMPILER_CLIENT_OPTION_NAMES = new Set([(TEXT_FETCHIMPL)]);
const MAX_COMPILER_SOURCE_BYTES = 1024 * 1024;
const MAX_COMPILER_SOURCE_NAME_BYTES = 4096;
const MAX_COMPILER_RESPONSE_BYTES = 16 * 1024 * 1024;
const MAX_COMPILER_ERROR_BYTES = 64 * 1024;
const MAX_COMPILER_RESPONSE_CHUNKS = 65_536;

const DefaultFetch = globalThis.fetch;
const AbortControllerIntrinsic = globalThis.AbortController;
const abortControllerAbort = AbortControllerIntrinsic?.prototype?.abort ?? null;
const abortControllerSignalGetter = AbortControllerIntrinsic
  ? (propertyDescriptor(AbortControllerIntrinsic.prototype, "signal")
      ?.get ?? null)
  : null;
const abortSignalAbortedGetter = globalThis.AbortSignal
  ? (propertyDescriptor(AbortSignal.prototype, "aborted")?.get ?? null)
  : null;
const abortSignalReasonGetter = globalThis.AbortSignal
  ? (propertyDescriptor(AbortSignal.prototype, "reason")?.get ?? null)
  : null;
const eventTargetAddEventListener = globalThis.EventTarget?.prototype?.addEventListener ?? null;
const eventTargetRemoveEventListener =
  globalThis.EventTarget?.prototype?.removeEventListener ?? null;
const responseOkGetter = globalThis.Response
  ? (propertyDescriptor(Response.prototype, "ok")?.get ?? null)
  : null;
const responseStatusGetter = globalThis.Response
  ? (propertyDescriptor(Response.prototype, "status")?.get ?? null)
  : null;
const responseRedirectedGetter = globalThis.Response
  ? (propertyDescriptor(Response.prototype, "redirected")?.get ?? null)
  : null;
const responseHeadersGetter = globalThis.Response
  ? (propertyDescriptor(Response.prototype, "headers")?.get ?? null)
  : null;
const responseBodyGetter = globalThis.Response
  ? (propertyDescriptor(Response.prototype, "body")?.get ?? null)
  : null;
const headersGet = globalThis.Headers?.prototype?.get ?? null;
const readableStreamGetReader = globalThis.ReadableStream?.prototype?.getReader ?? null;
const readerRead = globalThis.ReadableStreamDefaultReader?.prototype?.read ?? null;
const readerCancel = globalThis.ReadableStreamDefaultReader?.prototype?.cancel ?? null;
const readerReleaseLock =
  globalThis.ReadableStreamDefaultReader?.prototype?.releaseLock ?? null;
const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const typedArrayBufferGetter = propertyDescriptor(
  typedArrayPrototype,
  "buffer",
)?.get;
const typedArrayByteOffsetGetter = propertyDescriptor(
  typedArrayPrototype,
  "byteOffset",
)?.get;
const typedArrayByteLengthGetter = propertyDescriptor(
  typedArrayPrototype,
  "byteLength",
)?.get;
const typedArrayTagGetter = propertyDescriptor(
  typedArrayPrototype,
  Symbol.toStringTag,
)?.get;
const sharedArrayBufferByteLengthGetter = globalThis.SharedArrayBuffer
  ? (propertyDescriptor(SharedArrayBuffer.prototype, "byteLength")?.get ??
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
  if (typeof source !== (TEXT_STRING)) {
    rejectError(TypeError, (TEXT_KOTODAMA_SOURCE + TEXT_MUST_BE + "a " + TEXT_STRING));
  }
  if (!validateUnicodeScalarString(source)) {
    rejectError(TypeError, (TEXT_KOTODAMA_SOURCE + TEXT_MUST_CONTAIN_VALID_UNICODE_SCALAR_VALUES));
  }
  const sourceBytes = applyIntrinsic(textEncoderEncode, new TextEncoderIntrinsic(), [
    source,
  ]).length;
  if (sourceBytes > MAX_COMPILER_SOURCE_BYTES) {
    rejectError(RangeError, `${TEXT_KOTODAMA_SOURCE}${TEXT_EXCEEDS_THE}${MAX_COMPILER_SOURCE_BYTES}-byte V1 limit`,
    );
  }
}

function canonicalizeCompilerOptions(options, allowedNames) {
  if (options === undefined) {
    return Object.create(null);
  }
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "options " + TEXT_MUST_BE + "an object"));
  }
  const prototype = Object.getPrototypeOf(options);
  if (prototype !== Object.prototype && prototype !== null) {
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "options " + TEXT_MUST_BE + "a plain data object"));
  }
  const canonical = Object.create(null);
  for (const name of Reflect.ownKeys(options)) {
    if (typeof name !== (TEXT_STRING)) {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "options " + TEXT_MUST_NOT_CONTAIN + "symbol fields"));
    }
    if (!allowedNames.has(name)) {
      rejectError(TypeError, `unknown ${TEXT_KOTODAMA_COMPILER}option '${name}'`);
    }
    const descriptor = propertyDescriptor(options, name);
    if (
      descriptor === undefined ||
      !descriptor.enumerable ||
      !("value" in descriptor)
    ) {
      rejectError(TypeError, `${TEXT_KOTODAMA_COMPILER}option '${name}' ${TEXT_MUST_BE}an enumerable data property`,
      );
    }
    canonical[name] = descriptor.value;
  }
  return canonical;
}

function validateCompilerRequestFields(options) {
  if (hasOwn(options, (TEXT_SOURCENAME_SHARED))) {
    if (typeof options.sourceName !== (TEXT_STRING) || options.sourceName.length === 0) {
      rejectError(TypeError, (TEXT_SOURCENAME + TEXT_MUST_BE + TEXT_A_NON_EMPTY + TEXT_STRING));
    }
    if (!validateUnicodeScalarString(options.sourceName)) {
      rejectError(TypeError, (TEXT_SOURCENAME + TEXT_MUST_CONTAIN_VALID_UNICODE_SCALAR_VALUES));
    }
    const hasControlCharacter = Array.from(options.sourceName, (character) =>
      character.codePointAt(0),
    ).some((codePoint) => codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f));
    if (hasControlCharacter) {
      rejectError(TypeError, (TEXT_SOURCENAME + TEXT_MUST_NOT_CONTAIN + "control characters"));
    }
    const sourceNameBytes = applyIntrinsic(
      textEncoderEncode,
      new TextEncoderIntrinsic(),
      [options.sourceName],
    ).length;
    if (sourceNameBytes > MAX_COMPILER_SOURCE_NAME_BYTES) {
      rejectError(RangeError, `${TEXT_SOURCENAME}${TEXT_EXCEEDS_THE}${MAX_COMPILER_SOURCE_NAME_BYTES}-byte limit`,
      );
    }
  }
  if (hasOwn(options, "zk") && typeof options.zk !== (TEXT_BOOLEAN)) {
    rejectError(TypeError, ("zk " + TEXT_MUST_BE + "a " + TEXT_BOOLEAN));
  }
  if ([(TEXT_SOURCES), (TEXT_IMPORTS), (TEXT_PACKAGES)].some((key) => hasOwn(options, key))) {
    if (options.sourceName === undefined) rejectError(TypeError, (TEXT_SOURCENAME + "is required when " + TEXT_SOURCES + " are supplied"));
    const names = new Set([canonicalSourcePath(options.sourceName)]);
    if (hasOwn(options, (TEXT_SOURCES))) options.sources = canonicalSourceFiles(options.sources, names);
    if (hasOwn(options, (TEXT_IMPORTS))) options.imports = canonicalSourceImports(options.imports);
    if (hasOwn(options, (TEXT_PACKAGES))) {
      const identities = new Set();
      options.packages = canonicalDataArray(options.packages, (TEXT_PACKAGES)).map((value) => {
        const pkg = canonicalizeCompilerOptions(value, new Set([(TEXT_IDENTITY), "modules", (TEXT_SOURCES), "exports", (TEXT_IMPORTS)]));
        validateGraphIdentifier(pkg.identity, ("package " + TEXT_IDENTITY));
        if (identities.has(pkg.identity)) rejectError(TypeError, (TEXT_DUPLICATE + "package " + TEXT_IDENTITY));
        identities.add(pkg.identity);
        const paths = new Set();
        const modules = canonicalSourceFiles(pkg.modules, paths);
        const sources = canonicalSourceFiles(pkg.sources ?? [], paths);
        const exports = canonicalDataArray(pkg.exports, "exports");
        const exported = new Set();
        for (const name of exports) {
          validateGraphIdentifier(name, "package export");
          if (exported.has(name)) rejectError(TypeError, (TEXT_DUPLICATE + "package export"));
          exported.add(name);
        }
        return { identity: pkg.identity, modules, sources, exports, imports: canonicalSourceImports(pkg.imports ?? []) };
      });
    }
    const count = 1 + (options.sources?.length ?? 0) + (options.packages ?? []).reduce((total, pkg) => total + pkg.modules.length + pkg.sources.length, 0);
    if (count > 512) rejectError(RangeError, ("a " + TEXT_KOTODAMA_SOURCE + "set permits at most 512 files including its root"));
  }
  return options;
}

function canonicalDataArray(value, label) {
  if (!Array.isArray(value)) rejectError(TypeError, `${label} ${TEXT_MUST_BE}an array`);
  if (value.length > 512) rejectError(RangeError, `${label} ${TEXT_EXCEEDS_THE}512-item limit`);
  const result = [];
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = propertyDescriptor(value, String(index));
    if (!descriptor || !hasOwn(descriptor, "value")) rejectError(TypeError, `${label} must contain inert data entries`);
    result.push(descriptor.value);
  }
  return result;
}

function validateGraphIdentifier(value, label) {
  if (typeof value !== (TEXT_STRING) || value.length === 0 || value.length > 4096 || !validateUnicodeScalarString(value) || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
    rejectError(TypeError, `${label} ${TEXT_MUST_BE}a bounded nonempty ${TEXT_STRING}`);
  }
}

function canonicalSourceFiles(files, names) {
  return canonicalDataArray(files, (TEXT_SOURCES)).map((source) => {
    const file = canonicalizeCompilerOptions(source, new Set([(TEXT_SOURCENAME_SHARED), "source"]));
    if (file.sourceName === undefined) rejectError(TypeError, ("each source file " + TEXT_REQUIRES + TEXT_SOURCENAME_SHARED));
    validateCompilerRequestFields({ sourceName: file.sourceName });
    validateCompilerSource(file.source);
    const name = canonicalSourcePath(file.sourceName);
    if (names.has(name)) rejectError(TypeError, `${TEXT_DUPLICATE}${TEXT_KOTODAMA_SOURCE}path '${name}'`);
    names.add(name);
    return { sourceName: name, source: file.source };
  });
}

function canonicalSourceImports(imports) {
  const aliases = new Set();
  return canonicalDataArray(imports, (TEXT_IMPORTS)).map((value) => {
    const binding = canonicalizeCompilerOptions(value, new Set(["alias", "package"]));
    validateGraphIdentifier(binding.alias, "import alias");
    validateGraphIdentifier(binding.package, "import package");
    if (aliases.has(binding.alias)) rejectError(TypeError, (TEXT_DUPLICATE + "import alias"));
    aliases.add(binding.alias);
    return { alias: binding.alias, package: binding.package };
  });
}

function canonicalSourcePath(name) {
  if (/^(?:[\\/]|[A-Za-z]:)/u.test(name) || name.includes(":")) {
    rejectError(TypeError, ("source paths " + TEXT_MUST_BE + "relative to " + TEXT_THE_SOURCE_SET_ROOT));
  }
  const parts = [];
  for (const part of name.replaceAll("\\", "/").split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") {
      if (parts.length === 0) rejectError(TypeError, (TEXT_SOURCE_PATH + "escapes " + TEXT_THE_SOURCE_SET_ROOT));
      parts.pop();
    } else {
      if (/^\.+$/u.test(part)) rejectError(TypeError, (TEXT_SOURCE_PATH + "contains a nonportable component"));
      parts.push(part);
    }
  }
  if (parts.length === 0) rejectError(TypeError, (TEXT_SOURCE_PATH + "must name a file"));
  return parts.join("/");
}

function validateCompilerRequestOptions(options) {
  return validateCompilerRequestFields(
    canonicalizeCompilerOptions(options, COMPILER_REQUEST_OPTION_NAMES),
  );
}

function validateAbortSignal(signal) {
  if (abortSignalAbortedGetter === null) {
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "options.signal " + TEXT_REQUIRES + "AbortSignal support"));
  }
  try {
    applyIntrinsic(abortSignalAbortedGetter, signal, []);
  } catch {
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "options.signal " + TEXT_MUST_BE + "an AbortSignal"));
  }
}

function validateCompilerTransportFields(options) {
  if (hasOwn(options, "signal")) {
    validateAbortSignal(options.signal);
  }
  if (hasOwn(options, "timeoutMs")) {
    if (
      !Number.isInteger(options.timeoutMs) ||
      options.timeoutMs <= 0 ||
      options.timeoutMs > MAX_COMPILER_TIMEOUT_MS
    ) {
      rejectError(RangeError, `timeoutMs ${TEXT_MUST_BE}an integer from 1 through ${MAX_COMPILER_TIMEOUT_MS}`,
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
    hasOwn(options, "compilerUrl") &&
    (typeof options.compilerUrl !== (TEXT_STRING) || options.compilerUrl.length === 0)
  ) {
    rejectError(TypeError, ("compilerUrl " + TEXT_MUST_BE + TEXT_A_NON_EMPTY + TEXT_STRING));
  }
  if (hasOwn(options, (TEXT_FETCHIMPL)) && typeof options.fetchImpl !== "function") {
    rejectError(TypeError, (TEXT_FETCHIMPL + " " + TEXT_MUST_BE + "a function"));
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
  if ([(TEXT_SOURCES), (TEXT_IMPORTS), (TEXT_PACKAGES)].some((key) => options[key] !== undefined)) {
    const files = [...(options.sources ?? []), ...(options.packages ?? []).flatMap((pkg) => [...pkg.modules, ...pkg.sources])];
    const bytes = [source, ...files.map((file) => file.source)].reduce(
      (total, text) => total + applyIntrinsic(textEncoderEncode, new TextEncoderIntrinsic(), [text]).length,
      0,
    );
    if (bytes > 16 * 1024 * 1024) rejectError(RangeError, (TEXT_KOTODAMA_SOURCE + "set " + TEXT_EXCEEDS_THE + "16777216-byte limit"));
    request.sourceName = canonicalSourcePath(request.sourceName);
    for (const key of [(TEXT_SOURCES), (TEXT_IMPORTS), (TEXT_PACKAGES)]) {
      if (options[key] !== undefined) request[key] = options[key];
    }
  }
  return request;
}

/** Select request policy from an already validated top-level option object. */
export function selectCompilerRequestOptions(options) {
  const selected = {};
  for (const name of COMPILER_REQUEST_OPTION_NAMES) {
    if (hasOwn(options, name)) {
      selected[name] = options[name];
    }
  }
  return selected;
}

/** Select request and transport policy for a remote compiler invocation. */
export function selectCompilerCallOptions(options) {
  const selected = {};
  for (const name of COMPILER_CALL_OPTION_NAMES) {
    if (hasOwn(options, name)) {
      selected[name] = options[name];
    }
  }
  return selected;
}

function signalIsAborted(signal) {
  return applyIntrinsic(abortSignalAbortedGetter, signal, []);
}

function signalAbortReason(signal) {
  if (abortSignalReasonGetter !== null) {
    return applyIntrinsic(abortSignalReasonGetter, signal, []);
  }
  const error = new Error((TEXT_KOTODAMA_COMPILER + "request was aborted"));
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
    throw new Error((TEXT_KOTODAMA_COMPILER + TEXT_SERVICE + TEXT_REQUIRES + "AbortController support"));
  }

  const controller = new AbortControllerIntrinsic();
  const transportSignal = applyIntrinsic(abortControllerSignalGetter, controller, []);
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
      applyIntrinsic(abortControllerAbort, controller, [reason]);
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
      applyIntrinsic(eventTargetAddEventListener, signal, [
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
    timerId = applyIntrinsic(setTimeoutIntrinsic, globalThis, [
      () => {
        const error = new Error(
          `${TEXT_KOTODAMA_COMPILER}request timed out after ${timeoutMs}ms`,
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
        applyIntrinsic(clearTimeoutIntrinsic, globalThis, [timerId]);
        timerId = undefined;
      }
      if (callerListenerInstalled) {
        try {
          applyIntrinsic(eventTargetRemoveEventListener, signal, [
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
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "fetch " + TEXT_RETURNED_AN_INVALID + "Response"));
  }
  try {
    const ok = applyIntrinsic(responseOkGetter, response, []);
    const status = applyIntrinsic(responseStatusGetter, response, []);
    const redirected = applyIntrinsic(responseRedirectedGetter, response, []);
    const headers = applyIntrinsic(responseHeadersGetter, response, []);
    const body = applyIntrinsic(responseBodyGetter, response, []);
    if (
      typeof ok !== (TEXT_BOOLEAN) ||
      !Number.isInteger(status) ||
      status < 100 ||
      status > 599 ||
      typeof redirected !== (TEXT_BOOLEAN)
    ) {
      rejectError(TypeError, "invalid Response metadata");
    }
    return { ok, status, redirected, headers, body };
  } catch (error) {
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "fetch " + TEXT_RETURNED_AN_INVALID + "Response"), {
      cause: error,
    });
  }
}

function headerValue(headers, name, label) {
  if (headersGet === null) {
    rejectError(TypeError, `${label} ${TEXT_DOES_NOT_EXPOSE}${TEXT_STANDARDS_COMPLIANT}headers`);
  }
  try {
    return applyIntrinsic(headersGet, headers, [name]);
  } catch (error) {
    rejectError(TypeError, `${label} ${TEXT_DOES_NOT_EXPOSE}${TEXT_STANDARDS_COMPLIANT}headers`, {
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
  if (encoding !== null && encoding !== undefined && encoding.toLowerCase() !== (TEXT_IDENTITY)) {
    rejectError(TypeError, `${label} Content-Encoding ${TEXT_MUST_BE}absent or exactly ${TEXT_IDENTITY}`,
    );
  }
}

function cancelReaderBestEffort(reader, reason) {
  if (readerCancel === null) return;
  try {
    const cancellation = applyIntrinsic(readerCancel, reader, [reason]);
    Promise.resolve(cancellation).catch(() => {});
  } catch {
    // Cancellation is cleanup and must not replace the authoritative error.
  }
}

function releaseReaderBestEffort(reader) {
  if (readerReleaseLock === null) return;
  try {
    applyIntrinsic(readerReleaseLock, reader, []);
  } catch {
    // Lock release is cleanup and must not replace the authoritative result.
  }
}

function cancelResponseBestEffort(response, reason) {
  try {
    const body = applyIntrinsic(responseBodyGetter, response, []);
    if (body === null || readableStreamGetReader === null) return;
    const reader = applyIntrinsic(readableStreamGetReader, body, []);
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
    if (applyIntrinsic(typedArrayTagGetter, value, []) !== "Uint8Array") {
      rejectError(TypeError, "not Uint8Array");
    }
    buffer = applyIntrinsic(typedArrayBufferGetter, value, []);
    byteOffset = applyIntrinsic(typedArrayByteOffsetGetter, value, []);
    byteLength = applyIntrinsic(typedArrayByteLengthGetter, value, []);
  } catch {
    rejectError(TypeError, `${label} ${TEXT_YIELDED}a non-byte ${TEXT_RESPONSE}chunk`);
  }
  if (sharedArrayBufferByteLengthGetter !== null) {
    let isShared = false;
    try {
      applyIntrinsic(sharedArrayBufferByteLengthGetter, buffer, []);
      isShared = true;
    } catch {
      // Normal ArrayBuffers fail the SharedArrayBuffer brand check.
    }
    if (isShared) {
      rejectError(TypeError, `${label} ${TEXT_YIELDED}a SharedArrayBuffer-backed chunk`);
    }
  }
  if (byteLength === 0) {
    rejectError(TypeError, `${label} ${TEXT_YIELDED}an empty non-progress ${TEXT_RESPONSE}chunk`);
  }
  if (byteLength > remainingBytes) {
    rejectError(RangeError, `${label} ${TEXT_EXCEEDS_THE}${limit}-byte ${TEXT_RESPONSE}limit`);
  }
  const snapshot = new Uint8ArrayIntrinsic(byteLength);
  try {
    const view = new Uint8ArrayIntrinsic(buffer, byteOffset, byteLength);
    applyIntrinsic(uint8ArraySet, snapshot, [view]);
  } catch (error) {
    rejectError(TypeError, `${label} ${TEXT_YIELDED}an unstable ${TEXT_RESPONSE}chunk`, {
      cause: error,
    });
  }
  return snapshot;
}

async function readBoundedResponseBytes(metadata, limit, label, operation) {
  const declaredLength = contentLength(metadata.headers, label);
  if (declaredLength !== null && declaredLength > limit) {
    rejectError(RangeError, `${label} ${TEXT_EXCEEDS_THE}${limit}-byte ${TEXT_RESPONSE}limit`);
  }
  if (metadata.body === null) {
    if (declaredLength !== null && declaredLength !== 0) {
      rejectError(TypeError, `${label} ${TEXT_BODY_LENGTH_DOES_NOT_MATCH_ITS_CONTENT_LENGTH_HEADER}`,
      );
    }
    return new Uint8ArrayIntrinsic();
  }
  if (readableStreamGetReader === null || readerRead === null) {
    rejectError(TypeError, `${label} ${TEXT_DOES_NOT_EXPOSE}a ${TEXT_STANDARDS_COMPLIANT}readable body`);
  }

  let reader;
  try {
    reader = applyIntrinsic(readableStreamGetReader, metadata.body, []);
  } catch (error) {
    rejectError(TypeError, `${label} ${TEXT_DOES_NOT_EXPOSE}a ${TEXT_STANDARDS_COMPLIANT}readable body`,
      { cause: error },
    );
  }
  const chunks = [];
  let total = 0;
  try {
    for (;;) {
      operation.throwIfCancelled();
      const read = Promise.resolve().then(() =>
        applyIntrinsic(readerRead, reader, []),
      );
      const { done, value } = await operation.race(read);
      if (typeof done !== (TEXT_BOOLEAN)) {
        rejectError(TypeError, `${label} ${TEXT_RETURNED_AN_INVALID}stream read result`);
      }
      if (done) {
        if (value !== undefined) {
          rejectError(TypeError, `${label} returned data after the stream ended`);
        }
        break;
      }
      if (chunks.length >= MAX_COMPILER_RESPONSE_CHUNKS) {
        rejectError(RangeError, `${label} ${TEXT_YIELDED}too many fragmented ${TEXT_RESPONSE}chunks`);
      }
      const chunk = snapshotByteChunk(value, label, limit - total, limit);
      total += chunk.length;
      if (total > limit) {
        rejectError(RangeError, `${label} ${TEXT_EXCEEDS_THE}${limit}-byte ${TEXT_RESPONSE}limit`);
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
    rejectError(TypeError, `${label} ${TEXT_BODY_LENGTH_DOES_NOT_MATCH_ITS_CONTENT_LENGTH_HEADER}`,
    );
  }
  const bytes = new Uint8ArrayIntrinsic(total);
  let offset = 0;
  for (const chunk of chunks) {
    applyIntrinsic(uint8ArraySet, bytes, [chunk, offset]);
    offset += chunk.length;
  }
  return bytes;
}

async function readBoundedResponseText(metadata, limit, label, operation) {
  const bytes = await readBoundedResponseBytes(metadata, limit, label, operation);
  try {
    return applyIntrinsic(
      textDecoderDecode,
      new TextDecoderIntrinsic("utf-8", { fatal: true }),
      [bytes],
    );
  } catch {
    rejectError(TypeError, `${label} is not valid UTF-8`);
  }
}

async function readCompilerResult(metadata, operation) {
  const text = await readBoundedResponseText(
    metadata,
    MAX_COMPILER_RESPONSE_BYTES,
    (TEXT_KOTODAMA_COMPILER + "response"),
    operation,
  );
  let result;
  try {
    result = JSON.parse(text);
  } catch {
    rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_SERVICE + "returned malformed JSON"));
  }
  return normalizeCompilerResult(result);
}

/** Browser/Node client for an explicitly configured canonical Rust compiler service. */
export class KotodamaCompilerClient {
  #baseUrl;

  #fetchImpl;

  constructor(baseUrl, options = {}) {
    if (typeof baseUrl !== (TEXT_STRING) || baseUrl.length === 0) {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_BASEURL + TEXT_MUST_BE + TEXT_A_NON_EMPTY + TEXT_STRING));
    }
    options = canonicalizeCompilerOptions(options, COMPILER_CLIENT_OPTION_NAMES);
    const fetchImpl = hasOwn(options, (TEXT_FETCHIMPL))
      ? options.fetchImpl
      : DefaultFetch;
    let parsed;
    try {
      parsed = new URL(baseUrl);
    } catch {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_BASEURL + TEXT_MUST_BE + "an absolute URL"));
    }
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_BASEURL + "must use HTTP or HTTPS"));
    }
    if (parsed.protocol === "http:" && !isLoopbackHostname(parsed.hostname)) {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_BASEURL + "must use HTTPS except for loopback development services"),
      );
    }
    if (parsed.username !== "" || parsed.password !== "") {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_BASEURL + TEXT_MUST_NOT_CONTAIN + "credentials"));
    }
    if (parsed.search !== "" || parsed.hash !== "") {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_BASEURL + TEXT_MUST_NOT_CONTAIN + "a query or fragment"));
    }
    if (typeof fetchImpl !== "function") {
      rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + "client " + TEXT_REQUIRES + "fetch"));
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
        applyIntrinsic(this.#fetchImpl, undefined, [
          `${this.#baseUrl}${DEFAULT_COMPILE_PATH}`,
          {
            method: "POST",
            headers: {
              accept: (TEXT_APPLICATION + "/json"),
              "content-type": (TEXT_APPLICATION + "/json"),
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
          (TEXT_KOTODAMA_COMPILER + "response"),
        );
      } catch (error) {
        cancelResponseBestEffort(response, error);
        throw error;
      }
      if (metadata.redirected) {
        cancelResponseBestEffort(response, ("redirected compiler " + TEXT_RESPONSE + "rejected"));
        rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_SERVICE + "redirects are forbidden"));
      }
      if (!metadata.ok) {
        const detail = await readBoundedResponseText(
          metadata,
          MAX_COMPILER_ERROR_BYTES,
          (TEXT_KOTODAMA_COMPILER + "error response"),
          operation,
        );
        const suffix = detail.length === 0 ? "" : `: ${detail}`;
        throw new Error(
          `${TEXT_KOTODAMA_COMPILER}${TEXT_SERVICE}failed (${metadata.status})${suffix}`,
        );
      }
      if (metadata.status !== 200) {
        cancelResponseBestEffort(response, "unexpected compiler success status");
        rejectError(TypeError, `${TEXT_KOTODAMA_COMPILER}${TEXT_SERVICE}returned unexpected success status ${metadata.status}`,
        );
      }
      if (headerValue(metadata.headers, "content-type", (TEXT_KOTODAMA_COMPILER + "response")) !== (TEXT_APPLICATION + "/json")) {
        cancelResponseBestEffort(response, ("invalid compiler " + TEXT_RESPONSE + "media type"));
        rejectError(TypeError, (TEXT_KOTODAMA_COMPILER + TEXT_RESPONSE + "Content-Type " + TEXT_MUST_BE + "exactly " + TEXT_APPLICATION + "/json"),
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
