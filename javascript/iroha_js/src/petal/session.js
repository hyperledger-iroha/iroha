// The receive-side object an app holds while its camera is open.
//
// After a frame decodes, the next frames are first read by tracking the code
// from its last pose, which skips the finder search; a full decode runs when
// tracking fails or the last pose is older than `TRACK_WINDOW_MS`.

import { DEFAULT_DECODE_OPTIONS, decodeFrameResult, resolveDecodeOptions, trackFrame } from "./decode.js";
import { DEFAULT_MAX_PAYLOAD_LEN, DEFAULT_MAX_PENDING_ATOMS, PetalStreamAssembler } from "./stream.js";
import { requireIndex } from "./support.js";

/** How long a decoded pose stays usable for tracking the next frames, in milliseconds. */
export const TRACK_WINDOW_MS = 500;

/** Default limits of a scan session. */
export const DEFAULT_SCAN_LIMITS = Object.freeze({
  /** Forget a half-received stream after this long without progress. */
  idleTimeoutMs: 30000,
  /** Forget a stream that has not finished this long after it started. */
  absoluteTimeoutMs: 180000,
  /** Assembler memory and size limits. */
  assembler: Object.freeze({ maxPayloadLen: DEFAULT_MAX_PAYLOAD_LEN, maxPendingAtoms: DEFAULT_MAX_PENDING_ATOMS }),
  /** Image decoder options. */
  decode: DEFAULT_DECODE_OPTIONS,
});

function resolveScanLimits(limits) {
  if (limits === undefined || limits === null) {
    return DEFAULT_SCAN_LIMITS;
  }
  if (typeof limits !== "object") {
    throw new TypeError("scan limits must be an object");
  }
  const idleTimeoutMs = limits.idleTimeoutMs ?? DEFAULT_SCAN_LIMITS.idleTimeoutMs;
  const absoluteTimeoutMs = limits.absoluteTimeoutMs ?? DEFAULT_SCAN_LIMITS.absoluteTimeoutMs;
  for (const [name, value] of [
    ["idleTimeoutMs", idleTimeoutMs],
    ["absoluteTimeoutMs", absoluteTimeoutMs],
  ]) {
    if (typeof value !== "number" || !(value >= 0)) {
      throw new TypeError(`${name} must be a non-negative number`);
    }
  }
  const assembler = limits.assembler ?? DEFAULT_SCAN_LIMITS.assembler;
  const maxPayloadLen = requireIndex(assembler.maxPayloadLen ?? DEFAULT_MAX_PAYLOAD_LEN, "maxPayloadLen");
  const maxPendingAtoms = requireIndex(assembler.maxPendingAtoms ?? DEFAULT_MAX_PENDING_ATOMS, "maxPendingAtoms");
  return Object.freeze({
    idleTimeoutMs,
    absoluteTimeoutMs,
    assembler: Object.freeze({ maxPayloadLen, maxPendingAtoms }),
    decode: resolveDecodeOptions(limits.decode),
  });
}

/** Decodes camera frames and reassembles the stream they carry. */
export class PetalScanSession {
  /**
   * @param {{idleTimeoutMs?: number, absoluteTimeoutMs?: number,
   *   assembler?: {maxPayloadLen?: number, maxPendingAtoms?: number},
   *   decode?: {tryMirrored?: boolean, templateSigmas?: readonly number[], maxPixels?: number}}} [limits]
   */
  constructor(limits) {
    this._limits = resolveScanLimits(limits);
    this._assembler = new PetalStreamAssembler(this._limits.assembler);
    this._stats = { frames: 0, located: 0, readable: 0, laneP: 0, laneK: 0, laneD: 0, tracked: 0, inferred: 0 };
    this._startedMs = null;
    this._progressMs = 0;
    this._lastRank = 0;
    /** The last frame that decoded and when, for tracking: `{frame, atMs}` or `null`. */
    this._lastPose = null;
  }

  /** The limits in force. */
  get limits() {
    return this._limits;
  }

  /**
   * Diagnostic counters: camera frames offered (`frames`), frames in which a
   * code was located, whether or not a lane could be read (`located`), frames
   * in which at least one lane decoded (`readable`), per-lane successes,
   * frames read by tracking the previous pose instead of a full search
   * (`tracked`) and frames read with one corner finder hidden and inferred
   * (`inferred`).
   */
  stats() {
    return Object.freeze({ ...this._stats });
  }

  /** Current receive progress. */
  progress() {
    return this._assembler.progress();
  }

  /** Drops all partial state, including the pose used for tracking. */
  reset() {
    this._assembler.reset();
    this._startedMs = null;
    this._lastRank = 0;
    this._lastPose = null;
  }

  /**
   * Offers one camera luma plane captured at monotonic time `nowMs`.
   *
   * When the last frame that decoded is at most `TRACK_WINDOW_MS` old, the
   * code is first followed from its pose (no finder search); a full decode
   * runs when that fails.
   *
   * @returns {{error: string | null, lanes: string, progress: object,
   *   completed: {meta: object, payload: Uint8Array} | null}}
   *   `error` is a `PetalError` code when the frame produced nothing
   *   (`no_orientation` means a code was located but no lane could be read:
   *   too far or too blurry), `lanes` lists the lanes that decoded (letters
   *   from `"PKD"`) and `completed` is the finished payload, delivered
   *   exactly once.
   */
  push(image, nowMs) {
    if (typeof nowMs !== "number" || !Number.isFinite(nowMs)) {
      throw new TypeError("nowMs must be a finite number");
    }
    const limits = this._limits;
    if (
      this._startedMs !== null &&
      (Math.max(nowMs - this._progressMs, 0) > limits.idleTimeoutMs ||
        Math.max(nowMs - this._startedMs, 0) > limits.absoluteTimeoutMs)
    ) {
      this.reset();
    }
    this._stats.frames += 1;
    const pose = this._lastPose;
    const tracked =
      pose !== null && Math.max(nowMs - pose.atMs, 0) <= TRACK_WINDOW_MS
        ? trackFrame(image, pose.frame, limits.decode)
        : null;
    if (tracked !== null) {
      this._stats.tracked += 1;
    }
    const result = tracked !== null ? { frame: tracked } : decodeFrameResult(image, limits.decode);
    let error = null;
    let lanes = "";
    if (result.error !== undefined) {
      error = result.error;
    } else {
      if (result.frame.inferredCorner !== null) {
        this._stats.inferred += 1;
      }
      this._lastPose = { frame: result.frame, atMs: nowMs };
      lanes = this._absorb(result.frame);
    }
    // A code that was found but could not be read (`no_orientation`) still
    // counts as located.
    if (error !== "no_finders" && error !== "unsupported_image") {
      this._stats.located += 1;
    }
    const progress = this._assembler.progress();
    if (progress.rank > this._lastRank || (progress.meta !== null && this._startedMs === null)) {
      this._progressMs = nowMs;
      if (this._startedMs === null) {
        this._startedMs = nowMs;
      }
    }
    this._lastRank = progress.rank;
    return Object.freeze({ error, lanes, progress, completed: this._assembler.takeCompleted() });
  }

  _absorb(frame) {
    let lanes = "";
    if (frame.p !== null) {
      lanes += "P";
      this._stats.laneP += 1;
    }
    if (frame.k !== null) {
      lanes += "K";
      this._stats.laneK += 1;
    }
    if (frame.d !== null) {
      lanes += "D";
      this._stats.laneD += 1;
    }
    if (lanes.length > 0) {
      this._stats.readable += 1;
    }
    frame.feed(this._assembler);
    return lanes;
  }
}
