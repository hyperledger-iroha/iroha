// The receive-side object an app holds while its camera is open.
//
// After a frame decodes, the next frames are first read by tracking the code
// from its last pose, which skips the finder search; a full decode runs when
// tracking fails or the last pose is older than `TRACK_WINDOW_MS`.
//
// A camera sees each shown frame two to four times, and noise, display banding
// or hand shake can cost every one of those captures a field lane. When a
// capture reads some lane but not every field lane, the session keeps its field
// evidence under the shown frame its decoded lane headers name, for the last
// `MERGE_WINDOW` shown frames. From the second such capture on, it merges them
// (`mergeFields`) and offers a recovered lane only when that lane's own header
// names the same frame.
//
// Every field lane a capture decodes also lets the session read the turn lane
// on top of it (`readTurns`). A turn lane is tried once per shown frame: the
// session remembers the last `TURN_MEMORY` it tried and does not look at those
// petals again. Reading turns costs a slow phone core several times a frame's
// decode, so after a try that reads no turn lane (a camera too soft or too
// shaky for the turns) the session passes over the next captures that would
// try one, one more after every such try, at most `TURN_BACKOFF`, until a try
// reads a lane again.

import {
  DEFAULT_DECODE_OPTIONS,
  decodeFrameResult,
  fieldEvidence,
  mergeFields,
  resolveDecodeOptions,
  trackFrame,
} from "./decode.js";
import { FIELD_LANES, LANE_ORDER, laneLetter } from "./lanes.js";
import { SECTORS } from "./layout.js";
import {
  DEFAULT_MAX_PAYLOAD_LEN,
  DEFAULT_MAX_PENDING_ATOMS,
  PetalStreamAssembler,
  parseAtomLane,
} from "./stream.js";
import { requireIndex } from "./support.js";
import { readTurns, turnPacket } from "./turns.js";

/** How long a decoded pose stays usable for tracking the next frames, in milliseconds. */
export const TRACK_WINDOW_MS = 500;
/** Shown frames whose captures are held for merging. */
export const MERGE_WINDOW = 6;
/** Captures held per shown frame; a further capture replaces the oldest. */
export const MERGE_CAPTURES = 4;
/** Turn lanes remembered as read (the four of each of the last `MERGE_WINDOW` shown frames). */
export const TURN_MEMORY = MERGE_WINDOW * SECTORS;
/** Most captures passed over after tries that read no turn lane. */
export const TURN_BACKOFF = 8;

/** The header every lane starts with, or `null` for data too short to hold one. */
function laneHeader(data) {
  return data.length < 3 ? null : { tag: data[0], frame: (data[1] << 8) | data[2] };
}

/**
 * The shown frame every decoded lane header of `frame` names; `null` when no lane decoded or the
 * lanes of a torn capture name different frames.
 */
function shownFrame(frame) {
  let shown = null;
  for (const lane of LANE_ORDER) {
    const result = frame.lane(lane);
    if (result === null) continue;
    const header = laneHeader(result.data);
    if (header === null) return null;
    if (shown !== null && (shown.tag !== header.tag || shown.frame !== header.frame)) return null;
    shown = header;
  }
  return shown;
}

function sameMeta(a, b) {
  if (a === null || b === null) return a === b;
  return a.kind === b.kind && a.len === b.len && a.crc === b.crc;
}

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
   *   decode?: {tryMirrored?: boolean, maxPixels?: number}}} [limits]
   */
  constructor(limits) {
    this._limits = resolveScanLimits(limits);
    this._assembler = new PetalStreamAssembler(this._limits.assembler);
    this._stats = {
      frames: 0, located: 0, readable: 0, laneD: 0, fieldLanes: 0,
      tracked: 0, inferred: 0, merges: 0, mergedLanes: 0, turnTries: 0, turnLanes: 0,
    };
    this._startedMs = null;
    this._progressMs = 0;
    this._lastRank = 0;
    /** The last frame that decoded and when, for tracking: `{frame, atMs}` or `null`. */
    this._lastPose = null;
    /** Captures held for merging: `{shown, evidence, done}` per shown frame, oldest first. */
    this._held = [];
    /** The turn lanes tried: `{tag, frame, sector}`, oldest first, at most `TURN_MEMORY`. */
    this._tried = [];
    /** Tries in a row that read no turn lane (at most `TURN_BACKOFF`). */
    this._turnMisses = 0;
    /** Captures still to pass over before the next try. */
    this._turnWait = 0;
  }

  /** The limits in force. */
  get limits() {
    return this._limits;
  }

  /**
   * Diagnostic counters: camera frames offered (`frames`), frames in which a
   * code was located, whether or not a lane could be read (`located`), frames
   * in which at least one lane decoded (`readable`), lane D successes
   * (`laneD`), field lane successes (`fieldLanes`, up to four per frame),
   * frames read by tracking the previous pose instead of a full search
   * (`tracked`) and frames read with one corner finder hidden and inferred
   * (`inferred`), merges of two or more captures of one shown frame and the
   * field lanes they recovered (`merges`, `mergedLanes`), and turn lanes tried
   * (`turnTries`, each at most once per shown frame) and read (`turnLanes`).
   */
  stats() {
    return Object.freeze({ ...this._stats });
  }

  /** Current receive progress. */
  progress() {
    return this._assembler.progress();
  }

  /**
   * Drops all partial state, including the pose used for tracking, held captures and the turn
   * lanes read.
   */
  reset() {
    this._assembler.reset();
    this._startedMs = null;
    this._lastRank = 0;
    this._lastPose = null;
    this._held = [];
    this._forgetTurns();
  }

  /** Forgets the turn lanes tried and any back-off. */
  _forgetTurns() {
    this._tried = [];
    this._turnMisses = 0;
    this._turnWait = 0;
  }

  /**
   * Offers one camera luma plane captured at monotonic time `nowMs`.
   *
   * When the last frame that decoded is at most `TRACK_WINDOW_MS` old, the
   * code is first followed from its pose (no finder search); a full decode
   * runs when that fails.
   *
   * @returns {{error: string | null, lanes: string, merged: string, turned: string,
   *   progress: object, completed: {meta: object, payload: Uint8Array} | null}}
   *   `error` is a `PetalError` code when the frame produced nothing
   *   (`no_orientation` means a code was located but no lane could be read:
   *   too far or too blurry), `lanes` lists the lanes this capture decoded
   *   alone (`D` and the sector digits `0`-`3`), `merged` lists the field
   *   lanes newly recovered by merging it with earlier captures of its shown
   *   frame (sector digits), `turned` lists the turn lanes read from it (the
   *   sector digits of the field lanes they ride on), and `completed` is the
   *   finished payload, delivered exactly once.
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
    let merged = "";
    let turned = "";
    if (result.error !== undefined) {
      error = result.error;
    } else {
      if (result.frame.inferredCorner !== null) {
        this._stats.inferred += 1;
      }
      this._lastPose = { frame: result.frame, atMs: nowMs };
      lanes = this._absorb(result.frame);
      merged = this._merge(image, result.frame);
      turned = this._turn(image, result.frame);
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
    return Object.freeze({ error, lanes, merged, turned, progress, completed: this._assembler.takeCompleted() });
  }

  _absorb(frame) {
    const lanes = frame.letters();
    if (frame.d !== null) {
      this._stats.laneD += 1;
    }
    for (const field of frame.fields) {
      if (field !== null) this._stats.fieldLanes += 1;
    }
    if (lanes.length > 0) {
      this._stats.readable += 1;
    }
    const previousMeta = this._assembler.progress().meta;
    frame.feed(this._assembler);
    // An eight-bit tag may collide across streams; done flags, evidence and
    // the turn lanes tried belong to the full stream identity. Captures held
    // before the first beacon are kept.
    if (previousMeta !== null && !sameMeta(this._assembler.progress().meta, previousMeta)) {
      this._held = [];
      this._forgetTurns();
    }
    return lanes;
  }

  /**
   * Reads the turn lanes on top of the field lanes this capture decoded, except those already
   * read for their shown frame, and offers their atoms.
   */
  _turn(image, frame) {
    const skip = [true, true, true, true];
    const headers = [null, null, null, null];
    for (let sector = 0; sector < SECTORS; sector += 1) {
      const field = frame.fields[sector];
      const header = field === null ? null : laneHeader(field.data);
      if (header === null) continue;
      headers[sector] = header;
      skip[sector] = this._tried.some(
        (tried) => tried.tag === header.tag && tried.frame === header.frame && tried.sector === sector,
      );
    }
    if (skip.every(Boolean)) {
      return "";
    }
    if (this._turnWait > 0) {
      this._turnWait -= 1;
      return "";
    }
    for (let sector = 0; sector < SECTORS; sector += 1) {
      const header = headers[sector];
      if (skip[sector] || header === null) continue;
      if (this._tried.length === TURN_MEMORY) this._tried.shift();
      this._tried.push({ tag: header.tag, frame: header.frame, sector });
      this._stats.turnTries += 1;
    }
    let turned = "";
    readTurns(image, frame, skip).forEach((result, sector) => {
      const header = headers[sector];
      const packet = result === null || header === null ? null : turnPacket(header, sector, result.data);
      if (packet === null) {
        return;
      }
      this._assembler.pushAtoms(packet);
      this._stats.turnLanes += 1;
      turned += laneLetter(FIELD_LANES[sector]);
    });
    if (turned === "") {
      this._turnMisses = Math.min(this._turnMisses + 1, TURN_BACKOFF);
      this._turnWait = this._turnMisses;
    } else {
      this._turnMisses = 0;
    }
    return turned;
  }

  /**
   * Holds this capture's field evidence under its shown frame and, from the second capture on,
   * merges the held captures of that frame.
   */
  _merge(image, frame) {
    const shown = shownFrame(frame);
    if (shown === null) return "";
    let held = this._held.find((h) => h.shown.tag === shown.tag && h.shown.frame === shown.frame);
    if (held === undefined) {
      if (this._held.length === MERGE_WINDOW) this._held.shift();
      held = { shown, evidence: [], done: [false, false, false, false] };
      this._held.push(held);
    }
    for (let sector = 0; sector < SECTORS; sector += 1) {
      held.done[sector] ||= frame.fields[sector] !== null;
    }
    if (held.done.every(Boolean)) {
      held.evidence = [];
      return "";
    }
    const evidence = fieldEvidence(image, frame, this._limits.decode);
    if (evidence === null) return "";
    if (held.evidence.length === MERGE_CAPTURES) held.evidence.shift();
    held.evidence.push(evidence);
    if (held.evidence.length < 2) return "";
    this._stats.merges += 1;
    const results = mergeFields(held.evidence, held.done);
    let merged = "";
    FIELD_LANES.forEach((lane, sector) => {
      const result = results[sector];
      const packet = result === null ? null : parseAtomLane(lane, result.data);
      // a merge offers only atoms of the shown frame it was made for
      if (packet === null || packet.header.tag !== shown.tag || packet.header.frame !== shown.frame) {
        return;
      }
      held.done[sector] = true;
      this._assembler.pushAtoms(packet);
      this._stats.mergedLanes += 1;
      merged += laneLetter(lane);
    });
    if (held.done.every(Boolean)) held.evidence = [];
    return merged;
  }
}
