// Payload streams: the sender-side encoder and the receiver-side assembler.
//
// Every frame carries a handful of fountain atoms, one lane at a time: lane P
// one atom, lane K five, and lane D one atom, except on every fourth frame
// (`frame % 4 == 0`), when lane D carries the stream beacon instead, so a
// receiver can join at any frame within a fraction of a second. Atom ids run
// contiguously over the atoms actually sent (see `firstAtomId`). Any single
// readable lane is useful on its own.
//
// Lane data layouts (all big-endian):
//
// * every lane starts with `tag:u8, frame:u16`; `tag` is the low byte of the
//   payload CRC-32C.
// * lanes P, K and non-beacon D: `atoms...` (16 bytes each).
// * beacon D: `version:u8, kind:u8, len:u24, crc:u32`, zero padded.

import { crc32c } from "./crc.js";
import { PetalFountainDecoder, encodeAtom, splitPayload } from "./fountain.js";
import {
  ATOM_LEN,
  ATOMS_PER_FRAME,
  D_ATOMS,
  D_DATA,
  K_ATOMS,
  LANE_HEADER_LEN,
  P_ATOMS,
  PetalFrameCells,
  encodeLane,
  laneSpec,
} from "./lanes.js";
import { PetalError, requireIndex, toBytes } from "./support.js";

/** Version/profile byte of the beacon: format version 1, layout profile 0. */
export const FORMAT_VERSION = 0x10;
/** Largest payload a beacon can describe (`u24`). */
export const MAX_PAYLOAD_LEN = 2 ** 24 - 1;
/** Default receiver payload limit; override with the assembler limits. */
export const DEFAULT_MAX_PAYLOAD_LEN = 65536;
/** A beacon replaces the lane-D atom on frames divisible by this interval. */
export const BEACON_INTERVAL = 4;
/** Default number of atoms buffered while waiting for the first beacon. */
export const DEFAULT_MAX_PENDING_ATOMS = 128;

const BEACON_BODY_LEN = 9;

function frameNumber(frame) {
  if (!Number.isInteger(frame)) {
    throw new TypeError("frame must be an integer");
  }
  return ((frame % 65536) + 65536) % 65536;
}

/** Whether `frame` (a `u16` counter) carries the beacon in lane D. */
export function isBeaconFrame(frame) {
  return frameNumber(frame) % BEACON_INTERVAL === 0;
}

/** Fountain atoms carried by `frame`. */
export function atomsInFrame(frame) {
  return isBeaconFrame(frame) ? P_ATOMS + K_ATOMS : P_ATOMS + D_ATOMS + K_ATOMS;
}

/**
 * Fountain id of the first atom of `frame`.
 *
 * Frame `f` follows `f` earlier frames, `ceil(f / 4)` of which were beacon
 * frames with one atom fewer.
 */
export function firstAtomId(frame) {
  const f = frameNumber(frame);
  return f * ATOMS_PER_FRAME - Math.ceil(f / BEACON_INTERVAL);
}

function laneFirstId(lane, frame) {
  const base = firstAtomId(frame);
  if (lane === "P") return base;
  if (lane === "D") return base + P_ATOMS;
  return base + P_ATOMS + (frame % BEACON_INTERVAL === 0 ? 0 : D_ATOMS);
}

/**
 * Identity of a stream, as carried by every beacon.
 *
 * @returns {{kind: number, len: number, crc: number, tag: number, sourceAtoms: number}}
 */
function streamMeta(kind, len, crc) {
  return Object.freeze({ kind, len, crc, tag: crc & 0xff, sourceAtoms: Math.ceil(len / ATOM_LEN) });
}

function sameMeta(a, b) {
  return a.kind === b.kind && a.len === b.len && a.crc === b.crc;
}

function parseHeader(data) {
  return Object.freeze({ tag: data[0], frame: (data[1] << 8) | data[2] });
}

function parseAtoms(lane, data, header, count) {
  const atoms = [];
  for (let index = 0; index < count; index += 1) {
    const start = LANE_HEADER_LEN + index * ATOM_LEN;
    atoms.push(data.slice(start, start + ATOM_LEN));
  }
  return Object.freeze({ header, firstId: laneFirstId(lane, header.frame), atoms: Object.freeze(atoms) });
}

/**
 * Parses the data bytes of lane P or lane K.
 *
 * @returns {{header: {tag: number, frame: number}, firstId: number, atoms: Uint8Array[]} | null}
 */
export function parseAtomLane(lane, data) {
  const spec = laneSpec(lane);
  if (lane === "D") {
    return null;
  }
  const bytes = toBytes(data, "lane data");
  if (bytes.length !== spec.dataLen) {
    return null;
  }
  return parseAtoms(lane, bytes, parseHeader(bytes), spec.atoms);
}

/**
 * Parses the data bytes of lane D: a beacon on beacon frames, otherwise one
 * atom. Returns `null` for malformed data.
 */
export function parseDLane(data) {
  const bytes = toBytes(data, "lane D data");
  if (bytes.length !== D_DATA) {
    return null;
  }
  const header = parseHeader(bytes);
  if (header.frame % BEACON_INTERVAL !== 0) {
    return Object.freeze({ type: "atoms", packet: parseAtoms("D", bytes, header, D_ATOMS) });
  }
  const body = bytes.subarray(LANE_HEADER_LEN);
  if (body[0] !== FORMAT_VERSION) {
    return null;
  }
  const len = (body[2] << 16) | (body[3] << 8) | body[4];
  if (len === 0) {
    return null;
  }
  const crc = ((body[5] << 24) | (body[6] << 16) | (body[7] << 8) | body[8]) >>> 0;
  return Object.freeze({
    type: "beacon",
    beacon: Object.freeze({ header, meta: streamMeta(body[1], len, crc) }),
  });
}

/** Sender side: turns one payload into an endless sequence of frames. */
export class PetalStreamEncoder {
  /**
   * Prepares `payload` of application kind `kind` (`0..=255`) for streaming.
   *
   * @throws {PetalError} `empty_payload` or `payload_too_large`
   */
  constructor(payload, kind) {
    const bytes = toBytes(payload, "payload");
    if (!Number.isInteger(kind) || kind < 0 || kind > 255) {
      throw new TypeError("payload kind must be an integer in 0..=255");
    }
    if (bytes.length === 0) {
      throw new PetalError("empty_payload");
    }
    if (bytes.length > MAX_PAYLOAD_LEN) {
      throw new PetalError("payload_too_large");
    }
    this._meta = streamMeta(kind, bytes.length, crc32c(bytes));
    this._source = splitPayload(bytes);
  }

  /** Stream identity: `{kind, len, crc, tag, sourceAtoms}`. */
  get meta() {
    return this._meta;
  }

  /** Frames needed to send every source atom once (no losses, no repair). */
  systematicFrames() {
    let frames = 0;
    let atoms = 0;
    while (atoms < this._source.length) {
      atoms += atomsInFrame(frames % 65536);
      frames += 1;
    }
    return frames;
  }

  _atoms(firstId, count, out, offset) {
    for (let index = 0; index < count; index += 1) {
      out.set(encodeAtom(this._source, this._meta.crc, firstId + index), offset + index * ATOM_LEN);
    }
  }

  /**
   * The data bytes of every lane of `frame` (a counter reduced modulo 65536).
   *
   * @returns {{p: Uint8Array, k: Uint8Array, d: Uint8Array}}
   */
  laneData(frame) {
    const f = frameNumber(frame);
    const header = [this._meta.tag, f >> 8, f & 0xff];
    const build = (lane) => {
      const data = new Uint8Array(laneSpec(lane).dataLen);
      data.set(header);
      return data;
    };
    const p = build("P");
    this._atoms(laneFirstId("P", f), P_ATOMS, p, LANE_HEADER_LEN);
    const k = build("K");
    this._atoms(laneFirstId("K", f), K_ATOMS, k, LANE_HEADER_LEN);
    const d = build("D");
    if (f % BEACON_INTERVAL === 0) {
      const { kind, len, crc } = this._meta;
      const body = [
        FORMAT_VERSION,
        kind,
        len >>> 16,
        (len >>> 8) & 0xff,
        len & 0xff,
        crc >>> 24,
        (crc >>> 16) & 0xff,
        (crc >>> 8) & 0xff,
        crc & 0xff,
      ];
      d.set(body, LANE_HEADER_LEN);
      if (D_DATA - LANE_HEADER_LEN < BEACON_BODY_LEN) {
        throw new Error("beacon does not fit lane D");
      }
    } else {
      this._atoms(laneFirstId("D", f), D_ATOMS, d, LANE_HEADER_LEN);
    }
    return { p, k, d };
  }

  /**
   * The transmitted codewords of `frame`.
   *
   * @returns {{p: Uint8Array, k: Uint8Array, d: Uint8Array}}
   */
  words(frame) {
    const { p, k, d } = this.laneData(frame);
    return { p: encodeLane("P", p), k: encodeLane("K", k), d: encodeLane("D", d) };
  }

  /** Every cell of `frame`, ready to render. */
  cells(frame) {
    const { p, k, d } = this.words(frame);
    return PetalFrameCells.fromWords(p, k, d);
  }
}

function resolveLimits(limits) {
  if (limits === undefined || limits === null) {
    return { maxPayloadLen: DEFAULT_MAX_PAYLOAD_LEN, maxPendingAtoms: DEFAULT_MAX_PENDING_ATOMS };
  }
  if (typeof limits !== "object") {
    throw new TypeError("assembler limits must be an object");
  }
  const maxPayloadLen = limits.maxPayloadLen ?? DEFAULT_MAX_PAYLOAD_LEN;
  const maxPendingAtoms = limits.maxPendingAtoms ?? DEFAULT_MAX_PENDING_ATOMS;
  requireIndex(maxPayloadLen, "maxPayloadLen");
  requireIndex(maxPendingAtoms, "maxPendingAtoms");
  return { maxPayloadLen, maxPendingAtoms };
}

/** Receiver side: collects atoms from any lane of any frame. */
export class PetalStreamAssembler {
  /**
   * @param {{maxPayloadLen?: number, maxPendingAtoms?: number}} [limits]
   *   largest accepted payload (default 65536) and atoms buffered while
   *   waiting for the first beacon (default 128; zero buffers nothing)
   */
  constructor(limits) {
    this._limits = Object.freeze(resolveLimits(limits));
    this._active = null;
    this._pending = [];
    this._conflicting = null;
    this._completed = null;
    this._atomsReceived = 0;
    this._integrityFailures = 0;
  }

  /** The limits in force. */
  get limits() {
    return this._limits;
  }

  /**
   * Forgets the active stream, buffered atoms and any completed payload. The
   * integrity-failure count is cumulative over the assembler's lifetime and
   * is not cleared.
   */
  reset() {
    this._active = null;
    this._pending = [];
    this._conflicting = null;
    this._completed = null;
    this._atomsReceived = 0;
  }

  /**
   * Snapshot of receive progress for a UI. `integrityFailures` counts
   * reassembled payloads that failed the CRC check and were discarded
   * (cumulative, not cleared by `reset`).
   *
   * @returns {{meta: object | null, sourceAtoms: number, rank: number,
   *   atomsReceived: number, integrityFailures: number, complete: boolean}}
   */
  progress() {
    const active = this._active;
    return Object.freeze({
      meta: active === null ? null : active.meta,
      sourceAtoms: active === null ? 0 : active.decoder.sourceAtoms,
      rank: active === null ? 0 : active.decoder.rank,
      atomsReceived: this._atomsReceived,
      integrityFailures: this._integrityFailures,
      complete: active === null ? false : active.done,
    });
  }

  /** Takes the completed, CRC-verified payload (`{meta, payload}`), if any. */
  takeCompleted() {
    const completed = this._completed;
    this._completed = null;
    return completed;
  }

  _start(meta) {
    this._active = { meta, decoder: new PetalFountainDecoder(meta.sourceAtoms), done: false };
    this._conflicting = null;
    this._completed = null;
    this._atomsReceived = 0;
    const pending = this._pending;
    this._pending = [];
    for (const entry of pending) {
      if (entry.tag === meta.tag) {
        this._addAtom(entry.id, entry.atom);
      }
    }
  }

  /** Offers a beacon read from lane D. */
  pushBeacon(beacon) {
    const meta = beacon.meta;
    if (meta.len === 0 || meta.len > this._limits.maxPayloadLen) {
      return;
    }
    const normalized = streamMeta(meta.kind, meta.len, meta.crc);
    const active = this._active;
    if (active === null) {
      this._start(normalized);
    } else if (sameMeta(active.meta, normalized)) {
      this._conflicting = null;
    } else {
      // A different stream: switch only after two consecutive sightings.
      const conflicting = this._conflicting;
      const seen = conflicting !== null && sameMeta(conflicting.meta, normalized) ? conflicting.seen + 1 : 1;
      if (seen >= 2) {
        this._start(normalized);
      } else {
        this._conflicting = { meta: normalized, seen };
      }
    }
  }

  /** Offers atoms read from a lane (`{header, firstId, atoms}`). */
  pushAtoms(packet) {
    const tag = packet.header.tag;
    for (let index = 0; index < packet.atoms.length; index += 1) {
      const id = (packet.firstId + index) >>> 0;
      const atom = packet.atoms[index];
      const active = this._active;
      if (active !== null) {
        if (active.meta.tag === tag) {
          this._addAtom(id, atom);
        }
      } else {
        // Bounded FIFO of atoms seen before the first beacon; a zero limit
        // buffers nothing.
        if (this._limits.maxPendingAtoms === 0) {
          continue;
        }
        if (this._pending.length >= this._limits.maxPendingAtoms) {
          this._pending.shift();
        }
        this._pending.push({ tag, id, atom: Uint8Array.from(atom) });
      }
    }
  }

  /** Offers whatever lane D carried (the result of `parseDLane`). */
  pushDLane(lane) {
    if (lane.type === "beacon") {
      this.pushBeacon(lane.beacon);
    } else {
      this.pushAtoms(lane.packet);
    }
  }

  _addAtom(id, atom) {
    const active = this._active;
    if (active === null || active.done) {
      return;
    }
    this._atomsReceived = Math.min(this._atomsReceived + 1, 0xffffffff);
    active.decoder.addEncoded(active.meta.crc, id, atom);
    if (!active.decoder.isComplete()) {
      return;
    }
    const source = active.decoder.solve();
    if (source === null) {
      return;
    }
    const payload = new Uint8Array(active.meta.len);
    for (let index = 0; index < source.length; index += 1) {
      const start = index * ATOM_LEN;
      if (start >= payload.length) break;
      payload.set(source[index].subarray(0, Math.min(ATOM_LEN, payload.length - start)), start);
    }
    if (crc32c(payload) === active.meta.crc) {
      active.done = true;
      this._completed = Object.freeze({ meta: active.meta, payload });
    } else {
      // Corrupt atoms slipped through: start the elimination over.
      this._integrityFailures = Math.min(this._integrityFailures + 1, 0xffffffff);
      active.decoder = new PetalFountainDecoder(active.meta.sourceAtoms);
    }
  }
}
