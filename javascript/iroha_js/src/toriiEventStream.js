/**
 * Payload frames of `GET /v1/events/sse`, shared by both Torii clients.
 *
 * Every unnamed SSE frame carries one JSON object with `category` and `event`
 * (`specs/torii/collection_queries.md`, "Event streams"). As in collection
 * pages, integers beyond `Number.MAX_SAFE_INTEGER` (heights, dataspace ids)
 * decode as `bigint`. Unrecognized `category` and `event` values are
 * delivered unchanged, and named frames such as `stream_error` pass through.
 */
import { ToriiError } from "./toriiErrors.js";
import { parseStrictLosslessJson } from "./strictLosslessJson.js";
import { hasUnquotedLongInteger, toPlainJson } from "./query/page.js";

function exactIntegers(raw, data, context) {
  if (typeof raw !== "string" || !hasUnquotedLongInteger(raw)) return data;
  try {
    return toPlainJson(parseStrictLosslessJson(raw.trim(), context, { floatingPointPaths: [] }));
  } catch {
    // Not strictly canonical JSON (for example a fractional number): keep
    // the standard decoding.
    return data;
  }
}

/**
 * Decode one frame: unnamed frames must carry a JSON object.
 *
 * @template F
 * @param {F & {event: string | null, data: unknown, raw: string | null}} frame
 * @param {string} context
 * @returns {F}
 */
export function decodeEventFrame(frame, context) {
  if (frame.event !== null && frame.event !== undefined) return frame;
  const data = exactIntegers(frame.raw, frame.data, context);
  if (data === null || typeof data !== "object" || Array.isArray(data)) {
    throw new ToriiError(`${context} sent an event whose data is not a JSON object`, {
      code: "invalid_response",
    });
  }
  return data === frame.data ? frame : { ...frame, data };
}

/**
 * Decode every frame of an SSE frame iterator. Leaving the loop or aborting
 * closes the underlying stream.
 *
 * @param {AsyncIterable<object>} frames
 * @param {string} context
 */
export async function* decodeEventFrames(frames, context) {
  for await (const frame of frames) {
    yield decodeEventFrame(frame, context);
  }
}
