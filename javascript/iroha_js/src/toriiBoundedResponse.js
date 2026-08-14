// Hard transport ceiling for compact JSON rendering of a recovery sidecar.
// Canonical source is independently limited to one MiB before persistence;
// eight times that ceiling conservatively covers JSON escaping and structure.
export const PIPELINE_RECOVERY_JSON_MAX_BYTES = 8 * 1024 * 1024;
export const PIPELINE_FASTPQ_RECOVERY_JSON_MAX_BYTES = 24 * 1024 * 1024;

const EXACT_JSON_MEDIA_TYPE_PATTERN =
  /^[ \t]*application\/json(?:[ \t]*;[ \t]*[!#$%&'*+\-.^_`|~0-9A-Za-z]+=(?:[!#$%&'*+\-.^_`|~0-9A-Za-z]+|"(?:[ \t!#-\[\]-~\u0080-\u00ff]|\\[ \t!-~\u0080-\u00ff])*"))*[ \t]*$/i;

export function isExactJsonMediaType(value) {
  return typeof value === "string" && EXACT_JSON_MEDIA_TYPE_PATTERN.test(value);
}

/**
 * Parse optional JSON through a caller-owned bounded byte reader.
 *
 * The callback stays caller-owned so tests and specialised transports can
 * override the client's bounded reader without bypassing this error contract.
 */
export async function maybeJsonResponse(
  readBoundedJson,
  response,
  {
    maxBytes,
    context,
    signal,
    signalIsAborted,
  },
) {
  try {
    return await readBoundedJson(response, maxBytes, context, { signal });
  } catch (error) {
    // Preserve the historical optional-JSON contract for malformed or
    // body-less response shims while keeping resource and cancellation
    // failures observable. All bytes still pass through the bounded reader.
    if (
      signalIsAborted(signal)
      || error instanceof RangeError
      || error?.name === "AbortError"
      || error?.name === "TimeoutError"
    ) {
      throw error;
    }
    return null;
  }
}

/** Parse one exact JSON response after a bounded byte-stream read. */
export async function maybeBoundedJsonResponse(
  response,
  maxBytes,
  context,
  {
    signal,
    getHeader,
    readBoundedResponseBytes,
    cancelResponseBodyBestEffort,
    cancelReadableBodyBestEffort,
    signalIsAborted,
    bodyReadAbortError,
  },
) {
  let contentType;
  try {
    contentType = getHeader(response, "content-type");
  } catch (error) {
    cancelResponseBodyBestEffort(
      response,
      `${context} rejected an unreadable Content-Type header`,
    );
    throw error;
  }
  if (!isExactJsonMediaType(contentType)) {
    cancelResponseBodyBestEffort(
      response,
      `${context} rejected a non-JSON response body`,
    );
    return null;
  }
  const { bytes, body } = await readBoundedResponseBytes(
    response,
    maxBytes,
    context,
    { signal },
  );
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (error) {
    cancelReadableBodyBestEffort(body, `${context} rejected invalid UTF-8`);
    throw new TypeError(`${context} must be valid UTF-8`, { cause: error });
  }
  try {
    const parsed = JSON.parse(text);
    if (signalIsAborted(signal)) {
      cancelReadableBodyBestEffort(body, `${context} was aborted`);
      throw bodyReadAbortError(signal, context);
    }
    return parsed;
  } catch (error) {
    if (signalIsAborted(signal) || error?.name === "AbortError") {
      cancelReadableBodyBestEffort(body, `${context} was aborted`);
      throw error;
    }
    cancelReadableBodyBestEffort(body, `${context} rejected invalid JSON`);
    throw new TypeError(`${context} must contain valid JSON`, { cause: error });
  }
}
