// Petal Stream: the Sakura-storm optical transport.
//
// A Petal frame is a square image whose data lives in three independent
// lanes: the light/dark polarity of 256 tiles shaped like the SORA `天`
// (lane P), the katakana glyph drawn in each tile (lane K) and the dots on
// three concentric rings (lane D). Every lane is one Reed-Solomon codeword,
// so any lane that reads cleanly yields fountain-coded payload atoms.
//
// This module is a port of the `iroha_petal` Rust reference: the encoder is
// bit-exact (shared fixtures) and the decoder follows the reference step by
// step in IEEE double precision. It is
// pure JavaScript (typed arrays only, no Node built-ins, no WebAssembly) and
// runs in browsers, Node and React Native-like engines; the browser glue only
// touches DOM objects when a player or scanner is used.

import { GLYPH_CHARS, GLYPH_COUNT, GLYPH_GRID, STROKES, STROKE_WIDTH, TEMPLATES, TEMPLATE_N } from "./petal/glyphs.js";
import {
  ATOMS_PER_FRAME,
  ATOM_LEN,
  D_ATOMS,
  D_DATA,
  D_PARITY,
  D_WORD,
  K_ATOMS,
  K_DATA,
  K_PARITY,
  K_WORD,
  LANE_HEADER_LEN,
  P_ATOMS,
  P_DATA,
  P_PARITY,
  P_WORD,
} from "./petal/lanes.js";
import {
  CANVAS,
  CENTER,
  DOT_RADIUS,
  D_BITS,
  FINDER_CENTERS,
  FINDER_CORE,
  FINDER_NOTCH_RADIUS,
  FINDER_OUTER,
  FINDER_PETALS,
  FINDER_PETAL_DISTANCE,
  FINDER_PETAL_RADIUS,
  GATE_DOTS,
  GLYPH_BOX,
  MASK,
  RING_RADII,
  RING_SLOTS,
  TILE_COLS,
  TILE_CORNER_RADIUS,
  TILE_COUNT,
  TILE_GRID,
  TILE_ORIGIN,
  TILE_PITCH,
  TILE_ROWS,
  TILE_SIZE,
  TOTAL_SLOTS,
} from "./petal/layout.js";
import { DEFAULT_DECODE_OPTIONS } from "./petal/decode.js";
import { DEFAULT_PALETTE, GLYPH_STROKE_WIDTH } from "./petal/render.js";
import { DEFAULT_SCAN_LIMITS, TRACK_WINDOW_MS } from "./petal/session.js";
import {
  BEACON_INTERVAL,
  DEFAULT_MAX_PAYLOAD_LEN,
  DEFAULT_MAX_PENDING_ATOMS,
  FORMAT_VERSION,
  MAX_PAYLOAD_LEN,
} from "./petal/stream.js";
import { deepFreeze } from "./petal/support.js";

export { PetalError } from "./petal/support.js";
export { crc32c } from "./petal/crc.js";
export { PetalXorshift32 } from "./petal/prng.js";
export { PetalReedSolomon, gfExp, gfMul } from "./petal/rs.js";
export { dataSlots, finderLit, ringOffset, slotCenter, slotRoles, splitSlot, tileCenter } from "./petal/layout.js";
export { generateTemplates, isInked } from "./petal/glyphs.js";
export { PetalFrameCells, decodeLane, decodeLaneCounted, encodeLane, laneWhitening } from "./petal/lanes.js";
export { PetalFountainDecoder, encodeAtom, maskLen, maskWords, mix32, splitPayload } from "./petal/fountain.js";
export {
  PetalStreamAssembler,
  PetalStreamEncoder,
  atomsInFrame,
  firstAtomId,
  isBeaconFrame,
  parseAtomLane,
  parseDLane,
} from "./petal/stream.js";
export { PetalLuma } from "./petal/image.js";
export { PetalHomography } from "./petal/geometry.js";
export {
  adaptiveBinarize,
  blossoms,
  followFinder,
  labelComponents,
  locate,
  locateCandidates,
  refineCenter,
  selectQuad,
  selectTriple,
} from "./petal/locate.js";
export {
  PetalDecodedFrame,
  decodePetalFrame,
  decodePetalFrameAt,
  observedCells,
  tileMatchError,
  trackPetalFrame,
} from "./petal/decode.js";
export { petalDrawList, renderPetalFrame } from "./petal/render.js";
export { PetalScanSession } from "./petal/session.js";
export {
  PetalCameraScanner,
  PetalStreamPlayer,
  drawPetalFrame,
  petalPlayerFrame,
  petalScanSize,
} from "./petal/browser.js";

/** Normative frame geometry in design units (canvas 1024, origin top-left, `y` down). */
export const PETAL_LAYOUT = deepFreeze({
  canvas: CANVAS,
  center: CENTER,
  tileGrid: TILE_GRID,
  tileOrigin: TILE_ORIGIN,
  tilePitch: TILE_PITCH,
  tileSize: TILE_SIZE,
  tileCornerRadius: TILE_CORNER_RADIUS,
  glyphBox: GLYPH_BOX,
  tileCount: TILE_COUNT,
  mask: MASK.slice(),
  tiles: Array.from(TILE_COLS, (col, index) => [col, TILE_ROWS[index]]),
  ringRadii: RING_RADII.slice(),
  ringSlots: RING_SLOTS.slice(),
  dotRadius: DOT_RADIUS,
  totalSlots: TOTAL_SLOTS,
  gateDots: GATE_DOTS.map((row) => row.slice()),
  dBits: D_BITS,
  finderCenters: FINDER_CENTERS.map((center) => center.slice()),
  finderCore: FINDER_CORE,
  finderPetals: FINDER_PETALS,
  finderPetalDistance: FINDER_PETAL_DISTANCE,
  finderPetalRadius: FINDER_PETAL_RADIUS,
  finderNotchRadius: FINDER_NOTCH_RADIUS,
  finderOuter: FINDER_OUTER,
});

/** The sixteen katakana of lane K: characters, stroke polylines and matching templates. */
export const PETAL_GLYPHS = deepFreeze({
  count: GLYPH_COUNT,
  grid: GLYPH_GRID,
  strokeWidth: STROKE_WIDTH,
  templateN: TEMPLATE_N,
  chars: GLYPH_CHARS.slice(),
  strokes: STROKES.map((strokes) => strokes.map((stroke) => stroke.slice())),
  templates: TEMPLATES.map((template) => template.slice()),
  drawStrokeWidth: GLYPH_STROKE_WIDTH,
});

/** Codeword, parity and data sizes of the three lanes, in bytes. */
export const PETAL_LANES = deepFreeze({
  P: { wordLen: P_WORD, parityLen: P_PARITY, dataLen: P_DATA, atoms: P_ATOMS },
  K: { wordLen: K_WORD, parityLen: K_PARITY, dataLen: K_DATA, atoms: K_ATOMS },
  D: { wordLen: D_WORD, parityLen: D_PARITY, dataLen: D_DATA, atoms: D_ATOMS },
});

/** Stream-level constants of the wire format and the receiver defaults. */
export const PETAL_STREAM = deepFreeze({
  atomLen: ATOM_LEN,
  laneHeaderLen: LANE_HEADER_LEN,
  atomsPerFrame: ATOMS_PER_FRAME,
  formatVersion: FORMAT_VERSION,
  beaconInterval: BEACON_INTERVAL,
  maxPayloadLen: MAX_PAYLOAD_LEN,
  defaultMaxPayloadLen: DEFAULT_MAX_PAYLOAD_LEN,
  defaultMaxPendingAtoms: DEFAULT_MAX_PENDING_ATOMS,
});

/** Default colours as `[r, g, b]` triples. */
export const PETAL_PALETTE = DEFAULT_PALETTE;
/** Default decoder options. */
export const PETAL_DEFAULT_DECODE_OPTIONS = DEFAULT_DECODE_OPTIONS;
/** Default scan-session limits. */
export const PETAL_DEFAULT_SCAN_LIMITS = DEFAULT_SCAN_LIMITS;
/** How long (in milliseconds) a scan session tracks a code from its last decoded pose. */
export const PETAL_TRACK_WINDOW_MS = TRACK_WINDOW_MS;
