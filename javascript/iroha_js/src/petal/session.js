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
//
// The katakana bands above and below the square are read the same way
// (`readKana`): each band once per shown frame (the last `KANA_MEMORY` tried
// are remembered), with its own back-off of at most `KANA_BACKOFF` captures
// after tries that read no kana lane (a camera that does not see the bands, or
// too blurred for their glyphs and polarity).
//
// The bands go first. A capture whose bands read a kana lane tries the turn
// lanes too only when it has time to spare: when the work it has done (finding
// or following the code, a merge, the bands) and what the session's last turn
// try took fit together in `CAPTURE_BUDGET_MS`, a 30 fps camera's frame time
// (before the first turn try, twice the work done stands in for it). Otherwise
// the turn lanes of that shown frame wait for its next capture. On a slow phone
// core a turn try costs five tracked decodes and a band try one, so reading
// both on one capture overran the time a frame is shown at 12 fps and the
// session missed the next shown frames; a fast decoder reads both at once,
// which matters when its camera gives it few frames a second. A band try that
// reads nothing (bands out of view, too blurred, or a torn capture) leaves the
// turn lanes to the same capture.
//
// The session times its work on the monotonic clock (`performance.now()`); a
// `workCost` function given to the constructor puts a model's costs in place
// of the measured times, as tests do.

import {
  DEFAULT_DECODE_OPTIONS,
  decodeFrameResult,
  fieldEvidence,
  mergeFields,
  resolveDecodeOptions,
  trackFrame,
} from "./decode.js";
import { KANA_LANES, kanaPacket, readKana } from "./kana.js";
import { FIELD_LANES, LANE_ORDER, laneLetter } from "./lanes.js";
import { KANA_BANDS, SECTORS } from "./layout.js";
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
/** Bands remembered as tried (the two of each of the last `MERGE_WINDOW` shown frames). */
export const KANA_MEMORY = MERGE_WINDOW * KANA_BANDS;
/** Most captures passed over after band tries that read no kana lane. */
export const KANA_BACKOFF = 8;
/**
 * The time a capture's work may take in milliseconds, turn lanes included, for a capture whose
 * bands read a lane to try the turn lanes too: a 30 fps camera's frame time, so a decoder that
 * keeps to it misses no camera frame.
 */
export const CAPTURE_BUDGET_MS = 1000 / 30;

// The parts of a capture's work, as the session times them (see the module docs).
const SEARCH = Object.freeze({ kind: "search" });
const TRACK = Object.freeze({ kind: "track" });
const MERGE = Object.freeze({ kind: "merge" });
const TURNS = Object.freeze({ kind: "turns" });

/** The measured time, as a session on a camera judges its spare time. */
function measured(_work, measuredMs) {
  return measuredMs;
}

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
   * @param {(work: {kind: string, count?: number}, measuredMs: number) => number} [workCost]
   *   What a part of a capture costs in milliseconds, given the milliseconds it took (by default
   *   the measured time). `work.kind` is `"search"` (finding the code by a full search and taking
   *   in the lanes read), `"track"` (following the code from the previous capture's pose and taking
   *   in the lanes read), `"merge"` (merging the held captures of a shown frame), `"bands"` (reading
   *   the kana lanes of `work.count` bands) or `"turns"` (reading the turn lanes).
   */
  constructor(limits, workCost) {
    if (workCost !== undefined && typeof workCost !== "function") {
      throw new TypeError("workCost must be a function");
    }
    this._limits = resolveScanLimits(limits);
    this._workCost = workCost ?? measured;
    /** What the last turn try cost, in milliseconds, or `null` before the first. */
    this._turnCostMs = null;
    this._assembler = new PetalStreamAssembler(this._limits.assembler);
    this._stats = {
      frames: 0, located: 0, readable: 0, laneD: 0, fieldLanes: 0,
      tracked: 0, inferred: 0, merges: 0, mergedLanes: 0, turnTries: 0, turnLanes: 0,
      kanaTries: 0, kanaLanes: 0, spareTurns: 0,
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
    /** The bands tried: `{tag, frame, band}` (the shown frame and the band), oldest first, at most `KANA_MEMORY`. */
    this._kanaTried = [];
    /** Band tries in a row that read no kana lane (at most `KANA_BACKOFF`). */
    this._kanaMisses = 0;
    /** Captures still to pass over before the next band try. */
    this._kanaWait = 0;
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
   * field lanes they recovered (`merges`, `mergedLanes`), turn lanes tried
   * (`turnTries`, each at most once per shown frame) and read (`turnLanes`),
   * bands tried (`kanaTries`, each at most once per shown frame) and kana
   * lanes read (`kanaLanes`, up to four per shown frame), and captures whose
   * bands read a lane that also tried the turn lanes, having time to spare
   * (`spareTurns`).
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
   * lanes and bands tried.
   */
  reset() {
    this._assembler.reset();
    this._startedMs = null;
    this._lastRank = 0;
    this._lastPose = null;
    this._held = [];
    this._forgetTurns();
  }

  /** Forgets the turn lanes and bands tried and any back-off. */
  _forgetTurns() {
    this._tried = [];
    this._turnMisses = 0;
    this._turnWait = 0;
    this._kanaTried = [];
    this._kanaMisses = 0;
    this._kanaWait = 0;
  }

  /**
   * Offers one camera luma plane captured at monotonic time `nowMs`.
   *
   * When the last frame that decoded is at most `TRACK_WINDOW_MS` old, the
   * code is first followed from its pose (no finder search); a full decode
   * runs when that fails. The bands not yet tried for the capture's shown
   * frame are read first; the turn lanes are tried when they read no kana lane,
   * or when the capture has time to spare (see the module docs).
   *
   * @returns {{error: string | null, lanes: string, merged: string, turned: string, kana: string,
   *   progress: object, completed: {meta: object, payload: Uint8Array} | null}}
   *   `error` is a `PetalError` code when the frame produced nothing
   *   (`no_orientation` means a code was located but no lane could be read:
   *   too far or too blurry), `lanes` lists the lanes this capture decoded
   *   alone (`D` and the sector digits `0`-`3`), `merged` lists the field
   *   lanes newly recovered by merging it with earlier captures of its shown
   *   frame (sector digits), `turned` lists the turn lanes read from it (the
   *   sector digits of the field lanes they ride on), `kana` lists the kana
   *   lanes read from it (their names `P0`, `K0`, `P1`, `K1` concatenated), and
   *   `completed` is the finished payload, delivered exactly once.
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
    const pushStarted = performance.now();
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
    let kana = "";
    if (result.error !== undefined) {
      error = result.error;
    } else {
      if (result.frame.inferredCorner !== null) {
        this._stats.inferred += 1;
      }
      this._lastPose = { frame: result.frame, atMs: nowMs };
      lanes = this._absorb(result.frame);
      let spent = this._cost(tracked !== null ? TRACK : SEARCH, pushStarted);
      const merges = this._stats.merges;
      let partStarted = performance.now();
      merged = this._merge(image, result.frame);
      if (this._stats.merges > merges) {
        spent += this._cost(MERGE, partStarted);
      }
      const kanaTries = this._stats.kanaTries;
      partStarted = performance.now();
      kana = this._kana(image, result.frame);
      const bands = this._stats.kanaTries - kanaTries;
      if (bands > 0) {
        spent += this._cost(Object.freeze({ kind: "bands", count: bands }), partStarted);
      }
      // a capture whose bands read a lane tries the turn lanes only with time to spare (see the
      // module docs)
      if (kana === "") {
        turned = this._turn(image, result.frame);
      } else if (spent + (this._turnCostMs ?? 2 * spent) <= CAPTURE_BUDGET_MS) {
        const turnTries = this._stats.turnTries;
        turned = this._turn(image, result.frame);
        if (this._stats.turnTries > turnTries) {
          this._stats.spareTurns += 1;
        }
      }
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
    return Object.freeze({ error, lanes, merged, turned, kana, progress, completed: this._assembler.takeCompleted() });
  }

  /** What `work` cost, given that it started at `startedMs` on the monotonic clock. */
  _cost(work, startedMs) {
    return this._workCost(work, performance.now() - startedMs);
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
    const readStarted = performance.now();
    const results = readTurns(image, frame, skip);
    this._turnCostMs = this._cost(TURNS, readStarted);
    let turned = "";
    results.forEach((result, sector) => {
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
   * Tries the bands not yet tried for this capture's shown frame and offers the atoms of the kana
   * lanes they read; after tries that read nothing it passes over the next captures that would try
   * one (see the module docs).
   */
  _kana(image, frame) {
    const shown = shownFrame(frame);
    if (shown === null) return "";
    const skip = [false, false];
    for (let band = 0; band < KANA_BANDS; band += 1) {
      skip[band] = this._kanaTried.some(
        (tried) => tried.tag === shown.tag && tried.frame === shown.frame && tried.band === band,
      );
    }
    if (skip.every(Boolean)) {
      return "";
    }
    if (this._kanaWait > 0) {
      this._kanaWait -= 1;
      return "";
    }
    for (let band = 0; band < KANA_BANDS; band += 1) {
      if (skip[band]) continue;
      if (this._kanaTried.length === KANA_MEMORY) this._kanaTried.shift();
      this._kanaTried.push({ tag: shown.tag, frame: shown.frame, band });
      this._stats.kanaTries += 1;
    }
    let read = "";
    readKana(image, frame, shown, skip).forEach((result, index) => {
      const lane = KANA_LANES[index];
      const packet = result === null ? null : kanaPacket(result.header, lane, result.data);
      if (packet === null) {
        return;
      }
      this._assembler.pushAtoms(packet);
      this._stats.kanaLanes += 1;
      read += lane;
    });
    if (read === "") {
      this._kanaMisses = Math.min(this._kanaMisses + 1, KANA_BACKOFF);
      this._kanaWait = this._kanaMisses;
    } else {
      this._kanaMisses = 0;
    }
    return read;
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
