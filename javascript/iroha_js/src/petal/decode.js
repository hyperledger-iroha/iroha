// From a camera luma plane to lane data.
//
// The decoder locates the corner finders (four, or three with the fourth
// inferred), derives a pose for each orientation hypothesis, ranks the
// orientations by how well the ring gates line up (lane D must check out),
// then reads the dots and the petal field. Every petal is one bit: its level,
// normalised by the finder levels, is compared with a threshold computed for
// its codeword and colour class (the petals inside the `天` are lighter than
// the pink ones). Bytes the decoder is unsure about become Reed-Solomon
// erasures.
//
// When some field lanes do not decode, the pose is searched for radial lens
// distortion: a cheap camera's barrel distortion moves the petals by a third
// of their pitch at the edge of a 480p frame, while the finders that fix the
// homography sit far outside the field. Sectors the finder levels leave
// unread are read again from local levels, which shadows, glare, banding and
// veiling light do not disturb. A scan session follows a code from one frame
// to the next by tracking its pose instead of searching again.
//
// A camera sees each shown frame two to four times. `fieldEvidence` keeps what
// one capture shows of the petal field, and `mergeFields` reads the field
// lanes from the average of several captures of one shown frame, which
// recovers lanes that noise, display banding or hand shake cost each capture
// alone.
//
// Every floating-point expression follows the reference operation by
// operation. Intermediates match the reference wherever the platform's sin,
// cos, exp and atan2 agree (they are not guaranteed to across platforms);
// conformance is that the golden captures decode to the recorded lanes.

import { PetalPose, fitPose } from "./geometry.js";
import { sampleLuma } from "./image.js";
import {
  D_WORD,
  FIELD_BITS,
  FIELD_LANES,
  FIELD_WORD,
  LANE_ORDER,
  PetalFrameCells,
  decodeLaneCountedStatus,
  laneLetter,
  laneSpec,
} from "./lanes.js";
import {
  BAND_COUNT,
  BAND_OFFSETS,
  CELL_BAND,
  CELL_CENTER_X,
  CELL_CENTER_Y,
  CELL_LIGHT,
  D_BITS,
  DATA_SLOT,
  DOT_RADIUS,
  FIELD_CELLS,
  FINDER_CENTERS,
  GATE_SLOT_LIST,
  GUARD_SLOT_LIST,
  PETAL_RADIUS_F32,
  RING_COUNT,
  SECTORS,
  SECTOR_CELL_LIST,
  SLOT_CENTER_X,
  SLOT_CENTER_Y,
  SLOT_RING,
  TOTAL_SLOTS,
} from "./layout.js";
import { finderCandidates, followFinder } from "./locate.js";
import { parseAtomLane, parseDLane } from "./stream.js";
import { PetalError, clamp, fmax, fmin, totalCmp } from "./support.js";

/**
 * Distortion steps tried, relative to the starting pose, when a plain pose leaves field lanes
 * unread; the best is then refined by +-`DISTORTION_REFINE`.
 */
const DISTORTION_STEPS = Object.freeze([-0.03, -0.06, -0.09, -0.12, -0.16, 0.03]);
/** Final refinement step of the distortion search. */
const DISTORTION_REFINE = 0.015;
/** A colour class with fewer cells in a codeword than this takes the codeword's threshold. */
const CLASS_MIN_CELLS = 16;
/** Field cells within this distance of a cell are its neighbourhood for the local read. */
const NEIGHBOURHOOD = 32;
/** Cells averaged at each end of a sorted neighbourhood for its local dark and lit levels. */
const NEIGHBOURHOOD_ENDS = 5;
/** Sample spread of a ring dot, in canvas units. */
const DOT_SPREAD = 0.32 * DOT_RADIUS;
/** Sample spread of a petal, in canvas units. */
const PETAL_SPREAD = 0.4 * PETAL_RADIUS_F32;

/** Default decoder tuning. */
export const DEFAULT_DECODE_OPTIONS = Object.freeze({
  /** Also try horizontally mirrored images (front-camera previews). */
  tryMirrored: true,
  /**
   * Largest image (in pixels) the decoder accepts; larger frames should be
   * downscaled by the caller. Bounds memory and work on hostile input.
   */
  maxPixels: 12000000,
});

/** Validates and completes decoder options. */
export function resolveDecodeOptions(options) {
  if (options === undefined || options === null) {
    return DEFAULT_DECODE_OPTIONS;
  }
  if (typeof options !== "object") {
    throw new TypeError("decode options must be an object");
  }
  const tryMirrored = options.tryMirrored ?? DEFAULT_DECODE_OPTIONS.tryMirrored;
  const maxPixels = options.maxPixels ?? DEFAULT_DECODE_OPTIONS.maxPixels;
  if (typeof tryMirrored !== "boolean") {
    throw new TypeError("tryMirrored must be a boolean");
  }
  if (typeof maxPixels !== "number" || !(maxPixels >= 0)) {
    throw new TypeError("maxPixels must be a non-negative number");
  }
  return Object.freeze({ tryMirrored, maxPixels });
}

// Unit vectors of the eight reference-level sample directions.
const REFERENCE_COS = new Float64Array(8);
const REFERENCE_SIN = new Float64Array(8);
for (let k = 0; k < 8; k += 1) {
  const angle = (2 * Math.PI * k) / 8;
  REFERENCE_COS[k] = Math.cos(angle);
  REFERENCE_SIN[k] = Math.sin(angle);
}

/** Maps canvas `(x, y)` through `pose` and samples the image there. */
function sampleCanvas(image, pose, x, y) {
  const m = pose._m;
  const w = m[6] * x + m[7] * y + m[8];
  let px = (m[0] * x + m[1] * y + m[2]) / w;
  let py = (m[3] * x + m[4] * y + m[5]) / w;
  const k = pose.distortion;
  if (k !== 0) {
    const dx = px - pose._cx;
    const dy = py - pose._cy;
    const factor = 1 + (k * (dx * dx + dy * dy)) / pose.scale2;
    px = pose._cx + dx * factor;
    py = pose._cy + dy * factor;
  }
  return sampleLuma(image.data, image.width, image.height, px, py);
}

function dotSamples(image, pose, x, y, spread) {
  let sum = 0;
  sum += sampleCanvas(image, pose, x + 0, y + 0);
  sum += sampleCanvas(image, pose, x + spread, y + 0);
  sum += sampleCanvas(image, pose, x + -spread, y + 0);
  sum += sampleCanvas(image, pose, x + 0, y + spread);
  sum += sampleCanvas(image, pose, x + 0, y + -spread);
  return sum / 5;
}

/**
 * Light and dark levels at the four corners: the solid blossom core, and the black canvas 100
 * units inward of it (interpolated bilinearly over the canvas by the readers), or `null` when a
 * finder has too little contrast. An `inferred` corner (canonical index) was not seen, so its
 * levels are extrapolated from the other three by the parallelogram rule and kept within their
 * range, and must differ by 12 like a seen corner's. Exported for tests only.
 */
export function referenceLevels(image, pose, inferred = null) {
  const lit = new Float64Array(4);
  const dark = new Float64Array(4);
  for (let i = 0; i < 4; i += 1) {
    if (inferred === i) {
      continue;
    }
    const cx = FINDER_CENTERS[i][0];
    const cy = FINDER_CENTERS[i][1];
    // the blossom is solid out to radius 24 around its centre
    let sum = sampleCanvas(image, pose, cx, cy);
    for (let k = 0; k < 8; k += 1) {
      sum += sampleCanvas(image, pose, cx + 20 * REFERENCE_COS[k], cy + 20 * REFERENCE_SIN[k]);
    }
    lit[i] = sum / 9;
    const sx = cx < 512 ? 1 : -1;
    const sy = cy < 512 ? 1 : -1;
    const a = dotSamples(image, pose, cx + sx * 100, cy, 5);
    const b = dotSamples(image, pose, cx, cy + sy * 100, 5);
    dark[i] = 0.5 * (a + b);
    // also refuses NaN levels, which only a non-finite pose can produce
    if (!Number.isFinite(lit[i] - dark[i]) || lit[i] - dark[i] < 12) {
      return null;
    }
  }
  if (inferred !== null) {
    const n1 = (inferred + 1) % 4;
    const opposite = (inferred + 2) % 4;
    const n2 = (inferred + 3) % 4;
    const extrapolate = (v) => {
      const low = fmin(fmin(v[n1], v[opposite]), v[n2]);
      const high = fmax(fmax(v[n1], v[opposite]), v[n2]);
      return clamp(v[n1] + v[n2] - v[opposite], low, high);
    };
    lit[inferred] = extrapolate(lit);
    dark[inferred] = extrapolate(dark);
    // uneven light can push the estimates past each other; an inferred corner
    // needs the same contrast as a seen one
    if (lit[inferred] - dark[inferred] < 12) {
      return null;
    }
  }
  return { lit, dark };
}

function mixCorners(c, u, v) {
  const top = c[0] * (1 - u) + c[1] * u;
  const bottom = c[3] * (1 - u) + c[2] * u;
  return top * (1 - v) + bottom * v;
}

function clampUnit(value) {
  return value < 0 ? 0 : value > 1 ? 1 : value;
}

// Where every field cell and ring slot sits for the bilinear level interpolation.
const CELL_U = Float64Array.from(CELL_CENTER_X, (x) => clampUnit(x / 1024));
const CELL_V = Float64Array.from(CELL_CENTER_Y, (y) => clampUnit(y / 1024));
const SLOT_U = Float64Array.from(SLOT_CENTER_X, (x) => clampUnit(x / 1024));
const SLOT_V = Float64Array.from(SLOT_CENTER_Y, (y) => clampUnit(y / 1024));

const CANONICAL_X = Float64Array.from(FINDER_CENTERS, (center) => center[0]);
const CANONICAL_Y = Float64Array.from(FINDER_CENTERS, (center) => center[1]);

/** The pose with lens distortion `distortion` through the four finder centres (canonical corner order). */
function fitCanonical(image, dstX, dstY, distortion) {
  return fitPose(CANONICAL_X, CANONICAL_Y, dstX, dstY, distortion, image.width, image.height);
}

/**
 * Canonical index (0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left of the upright code)
 * of the corner at index `index` of a finder quad under one orientation hypothesis.
 */
function canonicalCorner(index, rotation, mirrored) {
  return mirrored ? (rotation + 4 - index) % 4 : (index + 4 - rotation) % 4;
}

/**
 * The brightness summed over all ring slots under the plain pose that maps the canonical corners
 * onto the quad `(xs[i], ys[i])`, or `null` when that pose is degenerate. The slots form the same
 * set of points under every quarter turn and mirror of the canvas (88, 100 and 108 are multiples
 * of four), so the value does not depend on the orientation.
 */
function ringBrightness(image, xs, ys) {
  const pose = fitCanonical(image, xs, ys, 0);
  if (pose === null) {
    return null;
  }
  let sum = -0;
  for (let flat = 0; flat < TOTAL_SLOTS; flat += 1) {
    sum += dotSamples(image, pose, SLOT_CENTER_X[flat], SLOT_CENTER_Y[flat], DOT_SPREAD);
  }
  return sum;
}

/**
 * Moves an inferred corner to where the three dotted rings line up best: a 13 x 13 search in
 * steps of 2 % of the mean leg around the parallelogram estimate, then a 9 x 9 search in steps
 * of 0.5 % around the best point. The rings fix the geometry only; the orientation is decided
 * afterwards by the gates and the lanes. Returns a new quad. Exported for tests only.
 */
export function refineInferredCorner(image, corners, inferred) {
  const xs = Float64Array.from(corners, (finder) => finder.x);
  const ys = Float64Array.from(corners, (finder) => finder.y);
  const startX = xs[inferred];
  const startY = ys[inferred];
  const distance = (other) =>
    Math.sqrt((xs[other] - startX) * (xs[other] - startX) + (ys[other] - startY) * (ys[other] - startY));
  const leg = 0.5 * (distance((inferred + 1) % 4) + distance((inferred + 3) % 4));
  let bestBrightness = -Number.MAX_VALUE;
  let bestX = startX;
  let bestY = startY;
  const search = (centreX, centreY, step, reach) => {
    for (let dy = -reach; dy <= reach; dy += 1) {
      for (let dx = -reach; dx <= reach; dx += 1) {
        const x = centreX + dx * step;
        const y = centreY + dy * step;
        xs[inferred] = x;
        ys[inferred] = y;
        const brightness = ringBrightness(image, xs, ys);
        if (brightness !== null && brightness > bestBrightness) {
          bestBrightness = brightness;
          bestX = x;
          bestY = y;
        }
      }
    }
  };
  search(startX, startY, 0.02 * leg, 6);
  search(bestX, bestY, 0.005 * leg, 4);
  const refined = corners.slice();
  refined[inferred] = { x: bestX, y: bestY, size: corners[inferred].size };
  return refined;
}

function normalisedDot(image, pose, reference, flat) {
  const u = SLOT_U[flat];
  const v = SLOT_V[flat];
  const lit = mixCorners(reference.lit, u, v);
  const dark = mixCorners(reference.dark, u, v);
  return (dotSamples(image, pose, SLOT_CENTER_X[flat], SLOT_CENTER_Y[flat], DOT_SPREAD) - dark) / (lit - dark);
}

/** The mean normalised level of the gate dots minus that of the guards. Exported for tests only. */
export function gateScore(image, pose, reference) {
  let gates = -0;
  for (const slot of GATE_SLOT_LIST) gates += normalisedDot(image, pose, reference, slot);
  let guards = -0;
  for (const slot of GUARD_SLOT_LIST) guards += normalisedDot(image, pose, reference, slot);
  return gates / GATE_SLOT_LIST.length - guards / GUARD_SLOT_LIST.length;
}

/** Reads lane D: returns the transmitted bytes and per-byte confidence. */
function readDots(image, pose, reference) {
  // per-ring thresholds from the gates (lit) and guards (dark)
  const thresholds = [0.5, 0.5, 0.5];
  for (let ring = 0; ring < RING_COUNT; ring += 1) {
    let litSum = -0;
    let litCount = 0;
    for (const slot of GATE_SLOT_LIST) {
      if (SLOT_RING[slot] !== ring) continue;
      litSum += normalisedDot(image, pose, reference, slot);
      litCount += 1;
    }
    let darkSum = -0;
    let darkCount = 0;
    for (const slot of GUARD_SLOT_LIST) {
      if (SLOT_RING[slot] !== ring) continue;
      darkSum += normalisedDot(image, pose, reference, slot);
      darkCount += 1;
    }
    if (litCount > 0 && darkCount > 0) {
      const l = litSum / litCount;
      const d = darkSum / darkCount;
      if (l - d > 0.2) {
        thresholds[ring] = 0.5 * (l + d);
      }
    }
  }
  const word = new Uint8Array(D_WORD);
  const confidence = new Float64Array(D_WORD).fill(Number.MAX_VALUE);
  for (let bit = 0; bit < D_BITS; bit += 1) {
    const slot = DATA_SLOT[bit];
    const value = normalisedDot(image, pose, reference, slot);
    const threshold = thresholds[SLOT_RING[slot]];
    if (value > threshold) {
      word[bit >> 3] |= 1 << (7 - (bit & 7));
    }
    confidence[bit >> 3] = fmin(confidence[bit >> 3], Math.abs(value - threshold));
  }
  return { word, confidence };
}

/** Lane D under one pose. */
function readLaneD(image, pose, reference) {
  const dots = readDots(image, pose, reference);
  return decodeWithErasures("D", dots.word, dots.confidence);
}

/**
 * Tries Reed-Solomon with growing numbers of erasures, least confident first.
 *
 * The schedule erases 0, 1/8, 1/4, 1/3 and 1/2 of the parity bytes (integer
 * division: lane D 0, 1, 2, 3, 5; a field lane 0, 2, 5, 7, 11). Lane D has
 * only 11 parity bytes, and a further step leaves so few spare ones that it
 * accepts wrong codewords (at 7 erasures about 0.4 % of random words).
 *
 * Returns `{data, corrected, erasures}`: `corrected` is the number of byte
 * positions the decoder rewrote (the erased bytes plus any errors it found
 * among the others, a measure of how close the lane was to failing) and
 * `erasures` the number of bytes passed as erasures. Exported for tests only.
 */
export function decodeWithErasures(lane, word, confidence) {
  const spec = laneSpec(lane);
  const nsym = spec.parityLen;
  // The ranking must be stable: byte-valued confidences tie a lot, and the tie order decides
  // which bytes are erased.
  const order = [];
  for (let index = 0; index < word.length; index += 1) order.push(index);
  order.sort((a, b) => totalCmp(confidence[a], confidence[b]));
  const steps = [0, Math.floor(nsym / 8), Math.floor(nsym / 4), Math.floor(nsym / 3), Math.floor(nsym / 2)];
  // consecutive duplicates are dropped, as with the reference's `dedup`
  const schedule = [];
  for (const erasures of steps) {
    if (schedule.length === 0 || schedule[schedule.length - 1] !== erasures) {
      schedule.push(erasures);
    }
  }
  for (const erasures of schedule) {
    const positions = order.slice(0, erasures);
    const trial = Uint8Array.from(word);
    // zero the erased bytes so stale values cannot leak through
    for (const position of positions) trial[position] = 0;
    const result = decodeLaneCountedStatus(spec, trial, positions);
    if (typeof result !== "number") {
      return Object.freeze({ data: result.data, corrected: result.corrected, erasures: positions.length });
    }
  }
  return null;
}

/**
 * The two-means threshold of the first `count` values: start at their mean, then move to the
 * midpoint of the means of the values above and at or below the threshold, at most eight times,
 * stopping when it moves less than 1e-4 or one side is empty. Sums run in the order given.
 * Exported for tests only.
 */
export function twoMeans(values, count = values.length) {
  let sum = -0;
  for (let index = 0; index < count; index += 1) sum += values[index];
  let threshold = sum / count;
  for (let round = 0; round < 8; round += 1) {
    let low = 0;
    let lows = 0;
    let high = 0;
    let highs = 0;
    for (let index = 0; index < count; index += 1) {
      const value = values[index];
      if (value > threshold) {
        high += value;
        highs += 1;
      } else {
        low += value;
        lows += 1;
      }
    }
    if (lows === 0 || highs === 0) {
      break;
    }
    const next = 0.5 * (low / lows + high / highs);
    const moved = Math.abs(next - threshold);
    threshold = next;
    if (moved < 1e-4) {
      break;
    }
  }
  return threshold;
}

/**
 * The level of every field cell under one pose, normalised by the finder levels; written to
 * `out` (allocated when omitted) and returned. Exported for tests only.
 */
export function fieldLevels(image, pose, reference, out = new Float64Array(FIELD_CELLS)) {
  const lit = reference.lit;
  const dark = reference.dark;
  for (let cell = 0; cell < FIELD_CELLS; cell += 1) {
    const u = CELL_U[cell];
    const v = CELL_V[cell];
    const cellLit = mixCorners(lit, u, v);
    const cellDark = mixCorners(dark, u, v);
    out[cell] = (dotSamples(image, pose, CELL_CENTER_X[cell], CELL_CENTER_Y[cell], PETAL_SPREAD) - cellDark) / (cellLit - cellDark);
  }
  return out;
}

let neighbourTable = null;

/**
 * The neighbourhood of every field cell: the cells (itself included) whose centres lie within
 * `NEIGHBOURHOOD` units, in index order, as `cells[start[c] .. start[c + 1]]`. Bands are 11
 * units apart, so only the three bands on either side can hold neighbours. Built on first use.
 * Exported for tests only.
 */
export function neighbourhoods() {
  if (neighbourTable === null) {
    const start = new Uint32Array(FIELD_CELLS + 1);
    const lists = [];
    let total = 0;
    let longest = 0;
    for (let cell = 0; cell < FIELD_CELLS; cell += 1) {
      const x = CELL_CENTER_X[cell];
      const y = CELL_CENTER_Y[cell];
      const band = CELL_BAND[cell];
      const first = BAND_OFFSETS[Math.max(band - 3, 0)];
      const last = BAND_OFFSETS[Math.min(band + 4, BAND_COUNT)];
      const around = [];
      for (let other = first; other < last; other += 1) {
        const dx = CELL_CENTER_X[other] - x;
        const dy = CELL_CENTER_Y[other] - y;
        if (dx * dx + dy * dy <= NEIGHBOURHOOD * NEIGHBOURHOOD) {
          around.push(other);
        }
      }
      start[cell] = total;
      total += around.length;
      longest = Math.max(longest, around.length);
      lists.push(around);
    }
    start[FIELD_CELLS] = total;
    const cells = new Uint16Array(total);
    lists.forEach((around, cell) => cells.set(around, start[cell]));
    const sorted = new Float64Array(longest);
    // one view per neighbourhood size, so sorting a neighbourhood allocates nothing
    const views = Array.from({ length: longest + 1 }, (_, length) => sorted.subarray(0, length));
    neighbourTable = Object.freeze({ start, cells, sorted, views });
  }
  return neighbourTable;
}

/**
 * Cell levels rescaled by their neighbourhood: the mean of the lowest and of the highest
 * `NEIGHBOURHOOD_ENDS` levels around a cell become 0 and 1 (a span below 0.05 counts as 0.05).
 * About half the petals around any cell are lit, so shadows, glare, banding and veiling light,
 * which the finder levels cannot describe, cancel. Writes to `out` (allocated when omitted) and
 * returns it. Exported for tests only.
 */
export function localLevels(levels, out = new Float64Array(FIELD_CELLS)) {
  const { start, cells, views } = neighbourhoods();
  for (let cell = 0; cell < FIELD_CELLS; cell += 1) {
    const begin = start[cell];
    const length = start[cell + 1] - begin;
    const values = views[length];
    for (let index = 0; index < length; index += 1) values[index] = levels[cells[begin + index]];
    // ascending, -0 before +0 and NaN last: the reference's `total_cmp` order
    values.sort();
    const ends = Math.max(Math.min(NEIGHBOURHOOD_ENDS, Math.floor(length / 2)), 1);
    let low = -0;
    for (let index = 0; index < ends; index += 1) low += values[index];
    low /= ends;
    let high = -0;
    for (let index = length - ends; index < length; index += 1) high += values[index];
    high /= ends;
    out[cell] = (levels[cell] - low) / fmax(high - low, 0.05);
  }
  return out;
}

// Bit positions (within a sector's 840 bit cells) of the light and the pink cells.
const SECTOR_CLASSES = Object.freeze(
  SECTOR_CELL_LIST.map((order) => {
    const light = [];
    const pink = [];
    for (let bit = 0; bit < FIELD_BITS; bit += 1) {
      (CELL_LIGHT[order[bit]] === 1 ? light : pink).push(bit);
    }
    return Object.freeze({ light: Uint16Array.from(light), pink: Uint16Array.from(pink) });
  }),
);

const SECTOR_VALUES = new Float64Array(FIELD_BITS);
const CLASS_VALUES = new Float64Array(FIELD_BITS);
const NO_SKIP = Object.freeze([false, false, false, false]);

/** The two-means threshold of one colour class of a sector, or `all` when the class is small. */
function classThreshold(members, all) {
  if (members.length < CLASS_MIN_CELLS) {
    return all;
  }
  for (let index = 0; index < members.length; index += 1) CLASS_VALUES[index] = SECTOR_VALUES[members[index]];
  return twoMeans(CLASS_VALUES, members.length);
}

/**
 * The transmitted word of one sector read from cell levels with per-colour thresholds, written to
 * `bytes`, and each byte's confidence (the smallest margin of its bits), written to `confidence`.
 * Every bit's margin from its threshold is added to the running sum `margins`, which is returned.
 */
function readWord(levels, sector, bytes, confidence, margins) {
  const order = SECTOR_CELL_LIST[sector];
  for (let bit = 0; bit < FIELD_BITS; bit += 1) SECTOR_VALUES[bit] = levels[order[bit]];
  const all = twoMeans(SECTOR_VALUES, FIELD_BITS);
  // the light (`天`) and pink petals are thresholded apart: their lit levels differ
  const classes = SECTOR_CLASSES[sector];
  const lightThreshold = classThreshold(classes.light, all);
  const pinkThreshold = classThreshold(classes.pink, all);
  bytes.fill(0);
  confidence.fill(Number.MAX_VALUE);
  let sum = margins;
  for (let bit = 0; bit < FIELD_BITS; bit += 1) {
    const value = SECTOR_VALUES[bit];
    const threshold = CELL_LIGHT[order[bit]] === 1 ? lightThreshold : pinkThreshold;
    if (value > threshold) {
      bytes[bit >> 3] |= 1 << (7 - (bit & 7));
    }
    const margin = Math.abs(value - threshold);
    sum += margin;
    confidence[bit >> 3] = fmin(confidence[bit >> 3], margin);
  }
  return sum;
}

/**
 * One read of the field lanes from cell levels with per-codeword, per-colour thresholds,
 * skipping the sectors flagged in `skip`. Writes a lane result or `null` for every sector to
 * `results` and returns how far the cells sit from their thresholds on average (a pose that lands
 * on the petals separates lit from dark cells best). Exported for tests only.
 */
export function readSectors(levels, skip, results) {
  let separation = 0;
  let counted = 0;
  for (let sector = 0; sector < SECTORS; sector += 1) {
    results[sector] = null;
    if (skip[sector]) {
      continue;
    }
    const bytes = new Uint8Array(FIELD_WORD);
    const confidence = new Float64Array(FIELD_WORD);
    separation = readWord(levels, sector, bytes, confidence, separation);
    counted += FIELD_BITS;
    results[sector] = decodeWithErasures(FIELD_LANES[sector], bytes, confidence);
  }
  return separation / Math.max(counted, 1);
}

const LEVELS_LOCAL = new Float64Array(FIELD_CELLS);

/**
 * The field lanes read from normalised cell levels, skipping the sectors in `skip`; sectors the
 * levels leave unread are read again from their `localLevels`. Writes the results to `results`
 * and returns the separation of the first read. Exported for tests only.
 */
export function readFields(levels, skip, results) {
  const separation = readSectors(levels, skip, results);
  const retrySkip = [true, true, true, true];
  let unread = false;
  for (let sector = 0; sector < SECTORS; sector += 1) {
    if (!skip[sector] && results[sector] === null) {
      retrySkip[sector] = false;
      unread = true;
    }
  }
  if (unread) {
    const retry = [null, null, null, null];
    readSectors(localLevels(levels, LEVELS_LOCAL), retrySkip, retry);
    for (let sector = 0; sector < SECTORS; sector += 1) {
      if (results[sector] === null) results[sector] = retry[sector];
    }
  }
  return separation;
}

function countDecoded(fields) {
  let decoded = 0;
  for (const field of fields) if (field !== null) decoded += 1;
  return decoded;
}

/** More lanes, then a clearer separation. */
function betterThan(read, other) {
  return read.decoded > other.decoded || (read.decoded === other.decoded && read.separation > other.separation);
}

// Level buffers of the reads under way: the plain read of a hypothesis and the best and the
// current pose of a distortion search. A decode works through them one at a time.
const LEVELS_PLAIN = new Float64Array(FIELD_CELLS);
const LEVELS_BEST = new Float64Array(FIELD_CELLS);
const LEVELS_TRIAL = new Float64Array(FIELD_CELLS);

/**
 * A read of the field under one pose (no local read): `{pose, reference, levels, fields,
 * decoded, separation}` with the levels in `levels`, or `null` when the pose or the reference
 * levels fail.
 */
function readFieldAt(image, dstX, dstY, distortion, inferred, levels) {
  const pose = fitCanonical(image, dstX, dstY, distortion);
  if (pose === null) {
    return null;
  }
  const reference = referenceLevels(image, pose, inferred);
  if (reference === null) {
    return null;
  }
  fieldLevels(image, pose, reference, levels);
  const fields = [null, null, null, null];
  const separation = readSectors(levels, NO_SKIP, fields);
  return { pose, reference, levels, fields, decoded: countDecoded(fields), separation };
}

/** Reads the sectors a field read left unread again from its `localLevels`. */
function finishRead(read) {
  if (read.decoded < SECTORS) {
    const skip = read.fields.map((field) => field !== null);
    const retry = [null, null, null, null];
    readSectors(localLevels(read.levels, LEVELS_LOCAL), skip, retry);
    for (let sector = 0; sector < SECTORS; sector += 1) {
      if (read.fields[sector] === null) read.fields[sector] = retry[sector];
    }
    read.decoded = countDecoded(read.fields);
  }
  return read;
}

/**
 * Reads the field through the finder centres `(dstX[i], dstY[i])`, starting from lens distortion
 * `start`, and searches the distortion while some field lane stays unread: `start` plus each of
 * `DISTORTION_STEPS` in turn, then the best +-`DISTORTION_REFINE`. The pose that decodes the most
 * lanes wins (ties: the clearer separation); the search stops once every lane decodes. Lanes the
 * winning pose still leaves unread are read again from its `localLevels`.
 */
function readFieldSearching(image, dstX, dstY, start, inferred) {
  let best = readFieldAt(image, dstX, dstY, start, inferred, LEVELS_BEST);
  if (best === null) {
    return null;
  }
  if (best.decoded === SECTORS) {
    return best;
  }
  let spare = LEVELS_TRIAL;
  for (const step of DISTORTION_STEPS) {
    const read = readFieldAt(image, dstX, dstY, start + step, inferred, spare);
    if (read !== null && betterThan(read, best)) {
      spare = best.levels;
      best = read;
      if (best.decoded === SECTORS) {
        return best;
      }
    }
  }
  const centre = best.pose.distortion;
  for (const k of [centre - DISTORTION_REFINE, centre + DISTORTION_REFINE]) {
    const read = readFieldAt(image, dstX, dstY, k, inferred, spare);
    if (read !== null && betterThan(read, best)) {
      spare = best.levels;
      best = read;
    }
  }
  return finishRead(best);
}

/**
 * The orientation hypotheses `{rotation, mirrored, dstX, dstY}` of a finder quad: four quarter
 * turns, and four mirrored ones when `tryMirrored`, each with the finder centres in canonical
 * corner order. Exported for tests only.
 */
export function hypotheses(finders, tryMirrored) {
  const out = [];
  for (const mirrored of [false, true]) {
    if (mirrored && !tryMirrored) {
      continue;
    }
    for (let rotation = 0; rotation < 4; rotation += 1) {
      const dstX = new Float64Array(4);
      const dstY = new Float64Array(4);
      for (let i = 0; i < 4; i += 1) {
        const q = mirrored ? finders[(rotation + 4 - i) % 4] : finders[(i + rotation) % 4];
        dstX[i] = q.x;
        dstY[i] = q.y;
      }
      out.push({ rotation, mirrored, dstX, dstY });
    }
  }
  return out;
}

function freezeFields(fields) {
  if (!Array.isArray(fields) || fields.length !== SECTORS) {
    throw new TypeError("fields must hold one result (or null) per field lane");
  }
  return Object.freeze(fields.slice());
}

/** Everything read from one camera frame. */
export class PetalDecodedFrame {
  constructor(pose, rotation, mirrored, d, fields, inferredCorner) {
    /** Canvas-to-pixel pose that was used (`PetalPose`). */
    this.pose = pose;
    /** Orientation: how many quarter turns the code is rotated. */
    this.rotation = rotation;
    /** Whether the image was mirrored. */
    this.mirrored = mirrored;
    /** Lane D result (`{data, corrected, erasures}`) or `null`. */
    this.d = d;
    /** Field lane results (or `null`), sector 0 first. */
    this.fields = freezeFields(fields);
    /**
     * The corner finder that was hidden (by a finger, a glare or the edge of the frame) and
     * inferred from the other three, as its canonical index: 0 top-left, 1 top-right,
     * 2 bottom-right, 3 bottom-left of the upright code; `null` when all four were seen.
     */
    this.inferredCorner = inferredCorner;
    Object.freeze(this);
  }

  /** Number of lanes that decoded. */
  lanesOk() {
    return (this.d !== null ? 1 : 0) + countDecoded(this.fields);
  }

  /** The result of `lane` (`"D"` or `"F0"` ... `"F3"`), or `null` when it did not decode. */
  lane(lane) {
    const spec = laneSpec(lane);
    return spec.sector === null ? this.d : this.fields[spec.sector];
  }

  /** The lanes that decoded, as letters: `D` and the sector digits `0`-`3`. */
  letters() {
    let letters = "";
    for (const lane of LANE_ORDER) {
      if (this.lane(lane) !== null) letters += laneLetter(lane);
    }
    return letters;
  }

  /** What lane D carried, when it decoded (see `parseDLane`). */
  dLane() {
    return this.d === null ? null : parseDLane(this.d.data);
  }

  /** The beacon, when lane D decoded on a beacon frame. */
  beacon() {
    const lane = this.dLane();
    return lane !== null && lane.type === "beacon" ? lane.beacon : null;
  }

  /** Atom packets from every lane that decoded (the field lanes, then D). */
  atomPackets() {
    const packets = [];
    FIELD_LANES.forEach((lane, sector) => {
      const result = this.fields[sector];
      const packet = result === null ? null : parseAtomLane(lane, result.data);
      if (packet !== null) packets.push(packet);
    });
    const lane = this.dLane();
    if (lane !== null && lane.type === "atoms") {
      packets.push(lane.packet);
    }
    return packets;
  }

  /** Offers everything this frame carries to `assembler` (D first, then the field lanes). */
  feed(assembler) {
    const lane = this.dLane();
    if (lane !== null) {
      assembler.pushDLane(lane);
    }
    FIELD_LANES.forEach((name, sector) => {
      const result = this.fields[sector];
      const packet = result === null ? null : parseAtomLane(name, result.data);
      if (packet !== null) assembler.pushAtoms(packet);
    });
  }
}

/**
 * What one capture shows of a frame's petal field: the level of every field cell sampled under
 * the capture's decoded pose and normalised by that capture's finder levels. Captures of one
 * shown frame merge with {@link mergeFields}. One evidence holds about 27 KB.
 */
export class PetalFieldEvidence {
  /** @param {ArrayLike<number>} levels the normalised level of each of the 3,376 field cells */
  constructor(levels) {
    if (levels === null || typeof levels !== "object" || levels.length !== FIELD_CELLS) {
      throw new TypeError(`field evidence holds exactly ${FIELD_CELLS} cell levels`);
    }
    /** The normalised level of every field cell. */
    this.levels = levels instanceof Float64Array ? levels : Float64Array.from(levels);
    Object.freeze(this);
  }
}

function isByteArray(value) {
  return value instanceof Uint8Array || value instanceof Uint8ClampedArray;
}

function requireLumaShape(image) {
  if (
    image === null ||
    typeof image !== "object" ||
    !Number.isInteger(image.width) ||
    !Number.isInteger(image.height) ||
    image.width < 0 ||
    image.height < 0 ||
    !isByteArray(image.data)
  ) {
    throw new TypeError("image must be a PetalLuma or {width, height, data: Uint8Array}");
  }
}

/**
 * Whether `image` is something the decoder may work on: at least 48 pixels on a side, within
 * `maxPixels`, and with a buffer that matches its size.
 */
function isDecodable(image, options) {
  return (
    image.width >= 48 &&
    image.height >= 48 &&
    image.width * image.height <= options.maxPixels &&
    image.data.length === image.width * image.height
  );
}

/** Whether `corner` is a corner a decoded frame may name: `null` or `0..3`. */
function isCorner(corner) {
  return corner === null || (Number.isInteger(corner) && corner >= 0 && corner < 4);
}

function requirePose(pose, name) {
  if (!(pose instanceof PetalPose)) {
    throw new TypeError(`${name} must be a PetalPose`);
  }
  return pose;
}

/**
 * Decodes one frame; returns `{frame}` or `{error}` with a `PetalError` code.
 * Shared by `decodePetalFrame` and the scan session.
 *
 * Tries the finder candidates of `finderCandidates` in order and returns the
 * first that reads.
 */
export function decodeFrameResult(image, options) {
  requireLumaShape(image);
  if (!isDecodable(image, options)) {
    return { error: "unsupported_image" };
  }
  let located = false;
  for (const set of finderCandidates(image)) {
    located = true;
    const frame = decodeCandidate(image, options, set);
    if (frame !== null) {
      return { frame };
    }
  }
  return { error: located ? "no_orientation" : "no_finders" };
}

/**
 * One finder set: an inferred corner is first refined against the rings; the orientation
 * hypotheses are ranked by gate score under their plain poses; lane D is tried under the best
 * three whose gate score is at least 0.2 (the field is then read with the distortion search), then
 * the field lanes under the best four.
 */
function decodeCandidate(image, options, set) {
  const corners = set.inferred === null ? set.corners : refineInferredCorner(image, set.corners, set.inferred);
  const inferredAt = (rotation, mirrored) =>
    set.inferred === null ? null : canonicalCorner(set.inferred, rotation, mirrored);
  const scored = [];
  for (const hypothesis of hypotheses(corners, options.tryMirrored)) {
    const pose = fitCanonical(image, hypothesis.dstX, hypothesis.dstY, 0);
    if (pose === null) continue;
    const reference = referenceLevels(image, pose, inferredAt(hypothesis.rotation, hypothesis.mirrored));
    if (reference === null) continue;
    scored.push({ ...hypothesis, pose, reference, gate: gateScore(image, pose, reference) });
  }
  // a stable sort, like the reference's
  scored.sort((a, b) => totalCmp(b.gate, a.gate));
  // 1. the ring beacon is the cheapest and strongest orientation check
  for (const candidate of scored.slice(0, 3)) {
    if (candidate.gate < 0.2) {
      continue;
    }
    const d = readLaneD(image, candidate.pose, candidate.reference);
    if (d !== null) {
      const inferred = inferredAt(candidate.rotation, candidate.mirrored);
      const read = readFieldSearching(image, candidate.dstX, candidate.dstY, 0, inferred);
      return new PetalDecodedFrame(
        read === null ? candidate.pose : read.pose,
        candidate.rotation,
        candidate.mirrored,
        d,
        read === null ? [null, null, null, null] : read.fields,
        inferred,
      );
    }
  }
  // 2. fall back to the field lanes under the most promising orientations: a wrong orientation
  //    reads permuted petals, which no field codeword accepts. Strong lens distortion can hide
  //    both lane D and the field from a plain pose, so the distortion is searched under the best
  //    orientation whose gates line up even when its plain pose reads nothing.
  const fallbacks = scored.slice(0, 4);
  for (let index = 0; index < fallbacks.length; index += 1) {
    const candidate = fallbacks[index];
    const inferred = inferredAt(candidate.rotation, candidate.mirrored);
    const plain = readFieldAt(image, candidate.dstX, candidate.dstY, 0, inferred, LEVELS_PLAIN);
    if (plain === null) {
      continue;
    }
    const confident = index === 0 && candidate.gate >= 0.2;
    // the local read costs a sort per cell: only the best orientation gets it for free
    let read;
    if (plain.decoded === SECTORS) {
      read = plain;
    } else if (plain.decoded === 0 && !confident) {
      read = index === 0 ? finishRead(plain) : plain;
    } else {
      read = readFieldSearching(image, candidate.dstX, candidate.dstY, 0, inferred) ?? finishRead(plain);
    }
    if (read.decoded === 0) {
      continue;
    }
    return new PetalDecodedFrame(
      read.pose,
      candidate.rotation,
      candidate.mirrored,
      readLaneD(image, read.pose, read.reference),
      read.fields,
      inferred,
    );
  }
  return null;
}

/**
 * Decodes one camera frame.
 *
 * The finder candidates are tried in order: four corner blossoms, or three that form a corner
 * with the fourth inferred (hidden by a finger, a glare or the edge of the frame; reported in
 * `inferredCorner`). Orientations are ranked by the ring gates; lane D is tried under the best
 * three, the field lanes under the best four. Every petal is thresholded per codeword and colour
 * class; the pose is searched for lens distortion while field lanes stay unread, and lanes the
 * finder levels cannot read (shadows, glare, banding, veiling light) are read again from local
 * levels. An orientation is accepted when lane D or a field lane decodes.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image luma plane
 * @param {{tryMirrored?: boolean, maxPixels?: number}} [options]
 * @returns {PetalDecodedFrame}
 * @throws {PetalError} `unsupported_image` (smaller than 48 pixels on a side,
 *   larger than `maxPixels` or inconsistent), `no_finders` or `no_orientation`
 */
export function decodePetalFrame(image, options) {
  const result = decodeFrameResult(image, resolveDecodeOptions(options));
  if (result.error !== undefined) {
    throw new PetalError(result.error);
  }
  return result.frame;
}

/**
 * Follows a code from the previous frame that decoded; returns the frame or `null`. The shared
 * part of `trackPetalFrame` and the scan session (options already validated).
 */
export function trackFrame(image, previous, options) {
  requireLumaShape(image);
  const p0 = requirePose(previous.pose, "previous.pose");
  // a frame built by the caller may name a corner that does not exist
  const previousInferred = previous.inferredCorner ?? null;
  if (!isDecodable(image, options) || !isCorner(previousInferred)) {
    return null;
  }
  const span = (a, b) => Math.sqrt((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]));
  const expected = FINDER_CENTERS.map(([cx, cy]) => {
    const [x, y] = p0.apply(cx, cy);
    const size = fmax(
      span(p0.apply(cx - 60, cy), p0.apply(cx + 60, cy)),
      span(p0.apply(cx, cy - 60), p0.apply(cx, cy + 60)),
    );
    return { x, y, size };
  });
  // a broken previous pose (non-finite, or finders larger than the image) is not followed
  const short = Math.min(image.width, image.height);
  if (
    expected.some(
      (finder) =>
        !(Number.isFinite(finder.x) && Number.isFinite(finder.y) && Number.isFinite(finder.size)) || finder.size > short,
    )
  ) {
    return null;
  }
  // every corner but the one inferred in the previous frame is followed
  const found = expected.map((finder, index) => (previousInferred === index ? null : followFinder(image, finder)));
  // the mean movement of the corners that were followed predicts the others
  let sumX = -0;
  let sumY = -0;
  let moved = 0;
  for (let index = 0; index < 4; index += 1) {
    if (found[index] === null) continue;
    sumX += found[index].x - expected[index].x;
    sumY += found[index].y - expected[index].y;
    moved += 1;
  }
  if (moved < 3) {
    return null;
  }
  const shiftX = sumX / moved;
  const shiftY = sumY / moved;
  const predicted = (index) => ({
    x: expected[index].x + shiftX,
    y: expected[index].y + shiftY,
    size: expected[index].size,
  });
  // a corner that was hidden is seen again only when its blossom is found right where the
  // others say it is (a bright thumb beside it must not count)
  if (previousInferred !== null) {
    const at = predicted(previousInferred);
    const again = followFinder(image, at);
    found[previousInferred] =
      again !== null && Math.sqrt((again.x - at.x) * (again.x - at.x) + (again.y - at.y) * (again.y - at.y)) <= 0.25 * at.size
        ? again
        : null;
  }
  const lost = [0, 1, 2, 3].filter((index) => found[index] === null);
  let inferred = null;
  if (lost.length === 1) {
    inferred = lost[0];
    found[inferred] = predicted(inferred);
  } else if (lost.length > 1) {
    return null;
  }
  const corners = inferred === null ? found : refineInferredCorner(image, found, inferred);
  const read = readFieldSearching(
    image,
    Float64Array.from(corners, (finder) => finder.x),
    Float64Array.from(corners, (finder) => finder.y),
    p0.distortion,
    inferred,
  );
  if (read === null) {
    return null;
  }
  const d = readLaneD(image, read.pose, read.reference);
  if (d === null && read.decoded === 0) {
    return null;
  }
  return new PetalDecodedFrame(read.pose, previous.rotation, previous.mirrored, d, read.fields, inferred);
}

/**
 * Follows a code from the previous frame that decoded, without searching the whole image for
 * finders (the most expensive part of {@link decodePetalFrame}).
 *
 * Each corner finder is re-found near where the previous pose puts it; a corner inferred in the
 * previous frame is seen again only when its blossom is found within a quarter diameter of where
 * the mean movement of the others puts it. When exactly one corner is not found it is placed by
 * that mean movement and refined against the rings like an inferred corner. The orientation is
 * kept from the previous frame and the lens distortion search starts from its distortion.
 * Returns `null` for a broken previous pose, when two corners are lost or when no lane decodes;
 * the caller then runs {@link decodePetalFrame}. A {@link PetalScanSession} does this by itself.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image luma plane
 * @param {PetalDecodedFrame} previous the last frame that decoded
 * @param {{tryMirrored?: boolean, maxPixels?: number}} [options]
 * @returns {PetalDecodedFrame | null}
 */
export function trackPetalFrame(image, previous, options) {
  const resolved = resolveDecodeOptions(options);
  if (!(previous instanceof PetalDecodedFrame)) {
    throw new TypeError("previous must be a PetalDecodedFrame");
  }
  return trackFrame(image, previous, resolved);
}

/**
 * Reads all lanes with a known canvas-to-pixel pose (no finder search, no distortion search).
 *
 * Returns `null` when the image is unusable or the finder reference levels are too weak. Used by
 * refinement passes and by qualification tooling with a ground-truth pose. The reference levels
 * come from all four corner finders, so a frame whose corner blossom is hidden is refused here
 * (the hidden corner has no contrast); {@link decodePetalFrame} and {@link trackPetalFrame} read
 * such frames by inferring that corner.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image luma plane
 * @param {PetalPose} pose
 * @param {{tryMirrored?: boolean, maxPixels?: number}} [options]
 * @returns {PetalDecodedFrame | null}
 */
export function decodePetalFrameAt(image, pose, options) {
  const resolved = resolveDecodeOptions(options);
  requireLumaShape(image);
  requirePose(pose, "pose");
  if (!isDecodable(image, resolved)) {
    return null;
  }
  const reference = referenceLevels(image, pose);
  if (reference === null) {
    return null;
  }
  const fields = [null, null, null, null];
  readFields(fieldLevels(image, pose, reference, LEVELS_PLAIN), NO_SKIP, fields);
  return new PetalDecodedFrame(pose, 0, false, readLaneD(image, pose, reference), fields, null);
}

/** Validated pose and inferred corner of a frame-like value, or `null` for a corner that does not exist. */
function frameGeometry(frame) {
  if (frame === null || typeof frame !== "object") {
    throw new TypeError("frame must be a decoded frame");
  }
  const pose = requirePose(frame.pose, "frame.pose");
  const inferred = frame.inferredCorner ?? null;
  return isCorner(inferred) ? { pose, inferred } : null;
}

/** The field evidence of `image` under `pose`, or `null` when the finder levels are too weak. */
function evidenceUnder(image, pose, inferred) {
  const reference = referenceLevels(image, pose, inferred);
  return reference === null ? null : new PetalFieldEvidence(fieldLevels(image, pose, reference));
}

/**
 * The field evidence of `image` under the pose `frame` was decoded with: the normalised level of
 * every field cell.
 *
 * Returns `null` when the image is unusable (see `unsupported_image`), the frame names a corner
 * that does not exist, or the finder levels under that pose are too weak.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image luma plane
 * @param {{pose: PetalPose, inferredCorner?: number | null}} frame usually a {@link PetalDecodedFrame}
 * @param {{tryMirrored?: boolean, maxPixels?: number}} [options]
 * @returns {PetalFieldEvidence | null}
 */
export function fieldEvidence(image, frame, options) {
  const resolved = resolveDecodeOptions(options);
  requireLumaShape(image);
  const geometry = frameGeometry(frame);
  if (!isDecodable(image, resolved) || geometry === null) {
    return null;
  }
  return evidenceUnder(image, geometry.pose, geometry.inferred);
}

const LEVELS_MEAN = new Float64Array(FIELD_CELLS);

/**
 * The field lanes from several captures of one shown frame: the cell-by-cell mean of their
 * levels (summed in capture order, then divided by the number of captures), read as one
 * capture's levels are, local read included. Averaging cancels the sensor noise, banding and
 * shake that differ between captures; one evidence reads exactly the field lanes the decoder
 * read from that capture's pose. The caller checks that every evidence comes from captures of the
 * same shown frame: a lane read here names its frame in its own header. Sectors flagged in `skip`
 * are not read.
 *
 * @param {readonly PetalFieldEvidence[]} evidence
 * @param {readonly boolean[]} [skip] one flag per sector
 * @returns {Array<{data: Uint8Array, corrected: number, erasures: number} | null>} sector 0 first
 */
export function mergeFields(evidence, skip = NO_SKIP) {
  if (!Array.isArray(evidence) || evidence.some((item) => !(item instanceof PetalFieldEvidence))) {
    throw new TypeError("evidence must be an array of PetalFieldEvidence");
  }
  if (!Array.isArray(skip) || skip.length !== SECTORS) {
    throw new TypeError("skip must hold one flag per sector");
  }
  const results = [null, null, null, null];
  if (evidence.length === 0) {
    return results;
  }
  const mean = LEVELS_MEAN.fill(0);
  for (const capture of evidence) {
    const levels = capture.levels;
    for (let cell = 0; cell < FIELD_CELLS; cell += 1) mean[cell] += levels[cell];
  }
  for (let cell = 0; cell < FIELD_CELLS; cell += 1) mean[cell] /= evidence.length;
  readFields(mean, skip.map(Boolean), results);
  return results;
}

/**
 * Builds the cells a decoder believes it saw under `frame`'s pose, for diagnostics; `null` when
 * the image is unusable, the frame names a corner that does not exist or the finder reference
 * levels are too weak. The levels of the frame's inferred corner, if any, are extrapolated as in
 * the decoder, and every sector is thresholded per colour class as the decoder reads it.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image luma plane
 * @param {{pose: PetalPose, inferredCorner?: number | null}} frame usually a {@link PetalDecodedFrame}
 * @param {{tryMirrored?: boolean, maxPixels?: number}} [options]
 * @returns {PetalFrameCells | null}
 */
export function observedCells(image, frame, options) {
  const evidence = fieldEvidence(image, frame, options);
  if (evidence === null) {
    return null;
  }
  const { pose, inferred } = frameGeometry(frame);
  const reference = referenceLevels(image, pose, inferred);
  if (reference === null) {
    return null;
  }
  const { word } = readDots(image, pose, reference);
  const confidence = new Float64Array(FIELD_WORD);
  const words = FIELD_LANES.map((_, sector) => {
    const field = new Uint8Array(FIELD_WORD);
    readWord(evidence.levels, sector, field, confidence, 0);
    return field;
  });
  return PetalFrameCells.fromWords(word, words);
}
