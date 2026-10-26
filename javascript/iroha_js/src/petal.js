// Petal Stream: the Sakura-storm optical transport.
//
// A Petal frame (format 5) is a card: a square code whose data lives in
// independent lanes, with a band of katakana tiles above and below it. In the
// square, the dots on three concentric rings (lane D), and a storm of 3,376
// petals on concentric bands inside them, cut into four sectors (lanes F0-F3),
// each petal lit or dark. On top of each field lane, the way its lit petals
// are turned carries a turn lane (T0-T3). The petals inside the SORA `天` are
// drawn white and the others pink, over a faint `天`. Each band's 298 data
// tiles show one bit as their polarity (lanes P0, P1) and four as their
// katakana (lanes K0, K1). Every lane is one Reed-Solomon codeword, so any
// lane that reads cleanly yields fountain-coded payload atoms.
//
// This module is a port of the `iroha_petal` Rust reference: the encoder is
// bit-exact (shared fixtures) and the decoder follows the reference step by
// step in IEEE double precision. It is
// pure JavaScript (typed arrays only, no Node built-ins, no WebAssembly) and
// runs in browsers, Node and React Native-like engines; the browser glue only
// touches DOM objects when a player or scanner is used.

import {
  ATOMS_PER_FRAME,
  ATOM_LEN,
  D_ATOMS,
  D_DATA,
  D_PARITY,
  D_WORD,
  FIELD_ATOMS,
  FIELD_DATA,
  FIELD_PARITY,
  FIELD_WORD,
  LANE_HEADER_LEN,
  SPARE_SECTOR_CELLS,
} from "./petal/lanes.js";
import { GLYPH_CHARS, GLYPH_COUNT, GLYPH_GRID, GLYPH_STROKES, STROKE_WIDTH } from "./petal/glyphs.js";
import {
  KANA_BLURS,
  KANA_CHECK_LEN,
  KANA_COARSE_GRID,
  KANA_DATA_TILES,
  KANA_GLYPH_ATOMS,
  KANA_GLYPH_PARITY,
  KANA_GLYPH_WORD,
  KANA_GRID,
  KANA_ID_BASE,
  KANA_LANES,
  KANA_POLARITY_ATOMS,
  KANA_POLARITY_PARITY,
  KANA_POLARITY_WORD,
} from "./petal/kana.js";
import {
  BACKDROP_RADIUS,
  BAND_COUNT,
  BAND_INNER,
  BAND_OFFSETS,
  BAND_SLOTS,
  BAND_SPACING,
  CANVAS,
  CARD_HEIGHT,
  CENTER,
  DOT_RADIUS,
  D_BITS,
  FIELD_CELLS,
  FINDER_CENTERS,
  FINDER_CORE,
  FINDER_NOTCH_RADIUS,
  FINDER_OUTER,
  FINDER_PETALS,
  FINDER_PETAL_DISTANCE,
  FINDER_PETAL_RADIUS,
  GATE_DOTS,
  KANA_BANDS,
  KANA_BAND_HEIGHT,
  KANA_BAND_TILES,
  KANA_COLUMNS,
  KANA_CORNER,
  KANA_FINDER_CLEARANCE,
  KANA_GAP,
  KANA_GLYPH_BOX,
  KANA_MARGIN,
  KANA_PILOTS,
  KANA_PILOT_CELLS,
  KANA_PILOT_SYMBOLS,
  KANA_PITCH,
  KANA_ROWS,
  KANA_TILE,
  KANA_TILE_COUNT,
  MASK,
  MASK_GRID,
  MASK_ORIGIN,
  MASK_PITCH,
  PETAL_HALF_LENGTH,
  PETAL_HALF_WIDTH,
  PETAL_NOTCH_DISTANCE,
  PETAL_NOTCH_RADIUS,
  PETAL_RADIUS,
  RING_RADII,
  RING_SLOTS,
  SECTORS,
  SECTOR_CELLS,
  SQUARE_TOP,
  TOTAL_SLOTS,
} from "./petal/layout.js";
import { DEFAULT_DECODE_OPTIONS } from "./petal/decode.js";
import { DEFAULT_PALETTE } from "./petal/render.js";
import {
  CAPTURE_BUDGET_MS,
  DEFAULT_SCAN_LIMITS,
  KANA_BACKOFF,
  KANA_MEMORY,
  TRACK_WINDOW_MS,
  TURN_BACKOFF,
  TURN_MEMORY,
} from "./petal/session.js";
import {
  BEACON_INTERVAL,
  DEFAULT_MAX_PAYLOAD_LEN,
  DEFAULT_MAX_PENDING_ATOMS,
  FORMAT_VERSION,
  MAX_PAYLOAD_LEN,
} from "./petal/stream.js";
import { deepFreeze } from "./petal/support.js";
import {
  TURN_ATOMS,
  TURN_BITS,
  TURN_BLURS,
  TURN_CARRIERS,
  TURN_CHECK_LEN,
  TURN_DATA,
  TURN_ID_BASE,
  TURN_PARITY,
  TURN_POSITIONS,
  TURN_WORD,
} from "./petal/turns.js";

export { PetalError } from "./petal/support.js";
export { crc32c } from "./petal/crc.js";
export { PetalXorshift32 } from "./petal/prng.js";
export { PetalReedSolomon, gfExp, gfMul } from "./petal/rs.js";
export {
  bandPhase,
  bandRadius,
  cellAngle,
  cellBandSlot,
  cellCenter,
  cellLight,
  dataSlots,
  finderLit,
  inMask,
  kanaDataTiles,
  kanaInked,
  kanaInside,
  kanaPilotTiles,
  kanaTileAt,
  kanaTiles,
  petalLit,
  ringOffset,
  sectorCells,
  slotCenter,
  slotRoles,
  splitSlot,
} from "./petal/layout.js";
export { glyphStrokes, isGlyphInked } from "./petal/glyphs.js";
export {
  PetalFrameCells,
  decodeLane,
  decodeLaneCounted,
  encodeLane,
  fieldLane,
  laneLetter,
  laneWhitening,
} from "./petal/lanes.js";
export { PetalFountainDecoder, encodeAtom, maskLen, maskWords, mix32, splitPayload } from "./petal/fountain.js";
export {
  PetalStreamAssembler,
  PetalStreamEncoder,
  atomsInFrame,
  firstAtomId,
  isBeaconFrame,
  laneFirstId,
  parseAtomLane,
  parseDLane,
} from "./petal/stream.js";
export { PetalLuma } from "./petal/image.js";
export { PetalHomography, PetalPose } from "./petal/geometry.js";
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
  PetalFieldEvidence,
  decodePetalFrame,
  decodePetalFrameAt,
  fieldEvidence,
  mergeFields,
  observedCells,
  trackPetalFrame,
} from "./petal/decode.js";
export {
  carrierOrder,
  carrierPosition,
  carriers,
  decodeTurnLaneCounted,
  encodeTurnLane,
  observedTurns,
  readTurns,
  turnAngle,
  turnCheck,
  turnData,
  turnFirstId,
  turnLabel,
  turnPacket,
  turnPosition,
  turnTemplate,
  turnWhitening,
} from "./petal/turns.js";
export {
  decodeKanaLaneCounted,
  encodeKanaLane,
  kanaCheck,
  kanaCoarseTemplateProbe,
  kanaData,
  kanaFirstId,
  kanaPacket,
  kanaTemplateProbe,
  kanaWhitening,
  observeBand,
  readKana,
} from "./petal/kana.js";
export { petalCardHeight, petalDrawList, renderPetalFrame } from "./petal/render.js";
export { PetalScanSession } from "./petal/session.js";
export {
  PetalCameraScanner,
  PetalStreamPlayer,
  drawPetalFrame,
  petalPlayerFrame,
  petalScanSize,
} from "./petal/browser.js";

/**
 * Normative frame geometry in design units (square coordinates: canvas 1024, origin top-left, `y`
 * down; the card adds a band of 322 units above and below the square).
 */
export const PETAL_LAYOUT = deepFreeze({
  canvas: CANVAS,
  center: CENTER,
  bandCount: BAND_COUNT,
  bandInner: BAND_INNER,
  bandSpacing: BAND_SPACING,
  bandSlots: BAND_SLOTS.slice(),
  bandOffsets: Array.from(BAND_OFFSETS),
  fieldCells: FIELD_CELLS,
  sectors: SECTORS,
  sectorCells: SECTOR_CELLS,
  petalRadius: PETAL_RADIUS,
  petalHalfLength: PETAL_HALF_LENGTH,
  petalHalfWidth: PETAL_HALF_WIDTH,
  petalNotchRadius: PETAL_NOTCH_RADIUS,
  petalNotchDistance: PETAL_NOTCH_DISTANCE,
  maskGrid: MASK_GRID,
  maskOrigin: MASK_ORIGIN,
  maskPitch: MASK_PITCH,
  backdropRadius: BACKDROP_RADIUS,
  mask: MASK.slice(),
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
  cardHeight: CARD_HEIGHT,
  squareTop: SQUARE_TOP,
  kanaTile: KANA_TILE,
  kanaPitch: KANA_PITCH,
  kanaGlyphBox: KANA_GLYPH_BOX,
  kanaCorner: KANA_CORNER,
  kanaRows: KANA_ROWS,
  kanaColumns: KANA_COLUMNS,
  kanaGap: KANA_GAP,
  kanaMargin: KANA_MARGIN,
  kanaBandHeight: KANA_BAND_HEIGHT,
  kanaFinderClearance: KANA_FINDER_CLEARANCE,
  kanaBands: KANA_BANDS,
  kanaPilots: KANA_PILOTS,
  kanaBandTiles: KANA_BAND_TILES,
  kanaTileCount: KANA_TILE_COUNT,
  kanaPilotCells: KANA_PILOT_CELLS.map((band) => band.map((cell) => cell.slice())),
  kanaPilotSymbols: KANA_PILOT_SYMBOLS.map((symbol) => symbol.slice()),
});

/**
 * The sixteen katakana of the band tiles, symbols `0`-`15` (`chars`), as stroke polylines of
 * `[x, y]` points on a `grid` x `grid` design grid (`y` down), drawn with round caps and joins of
 * width `strokeWidth`.
 */
export const PETAL_GLYPHS = deepFreeze({
  count: GLYPH_COUNT,
  grid: GLYPH_GRID,
  strokeWidth: STROKE_WIDTH,
  chars: GLYPH_CHARS,
  strokes: GLYPH_STROKES.map((strokes) => strokes.map((stroke) => stroke.map((point) => point.slice()))),
});

const fieldSize = () => ({ wordLen: FIELD_WORD, parityLen: FIELD_PARITY, dataLen: FIELD_DATA, atoms: FIELD_ATOMS });

/**
 * Codeword, parity and data sizes of lane D and the four field lanes, in bytes, in decode order
 * (the turn lanes are described by `PETAL_TURNS`).
 */
export const PETAL_LANES = deepFreeze({
  D: { wordLen: D_WORD, parityLen: D_PARITY, dataLen: D_DATA, atoms: D_ATOMS },
  F0: fieldSize(),
  F1: fieldSize(),
  F2: fieldSize(),
  F3: fieldSize(),
});

/**
 * The turn lanes `T0`-`T3`: positions and bits per carrier, carriers per sector, codeword,
 * parity and data sizes in bytes, atoms and check bytes per lane, the fountain id of the first
 * turn atom, the blurs the turn reader fits, and how many turn lanes a scan session remembers as
 * read.
 */
export const PETAL_TURNS = deepFreeze({
  positions: TURN_POSITIONS,
  bits: TURN_BITS,
  carriers: TURN_CARRIERS,
  wordLen: TURN_WORD,
  parityLen: TURN_PARITY,
  dataLen: TURN_DATA,
  atoms: TURN_ATOMS,
  checkLen: TURN_CHECK_LEN,
  idBase: TURN_ID_BASE,
  blurs: TURN_BLURS.slice(),
  memory: TURN_MEMORY,
  backoff: TURN_BACKOFF,
});

const kanaSize = (wordLen, parityLen, atoms) => ({ wordLen, parityLen, dataLen: wordLen - parityLen, atoms });

/**
 * The kana lanes of the katakana bands: their names (`P0`, `K0`, `P1`, `K1`), the codeword, parity
 * and data sizes and atoms of a polarity and of a glyph lane, the data tiles of a band, the check
 * bytes, the fountain id of the first kana atom, the blurs and grids of the band reader, and how
 * many bands a scan session remembers as tried and the most captures it passes over after band
 * tries that read nothing.
 */
export const PETAL_KANA = deepFreeze({
  lanes: KANA_LANES.slice(),
  polarity: kanaSize(KANA_POLARITY_WORD, KANA_POLARITY_PARITY, KANA_POLARITY_ATOMS),
  glyph: kanaSize(KANA_GLYPH_WORD, KANA_GLYPH_PARITY, KANA_GLYPH_ATOMS),
  dataTiles: KANA_DATA_TILES,
  checkLen: KANA_CHECK_LEN,
  idBase: KANA_ID_BASE,
  blurs: KANA_BLURS.slice(),
  grid: KANA_GRID,
  coarseGrid: KANA_COARSE_GRID,
  memory: KANA_MEMORY,
  backoff: KANA_BACKOFF,
});

/** Stream-level constants of the wire format and the receiver defaults. */
export const PETAL_STREAM = deepFreeze({
  atomLen: ATOM_LEN,
  laneHeaderLen: LANE_HEADER_LEN,
  atomsPerFrame: ATOMS_PER_FRAME,
  spareSectorCells: SPARE_SECTOR_CELLS,
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
/**
 * The time a scan session's capture may take in milliseconds, turn lanes included, for a capture
 * whose bands read a lane to try the turn lanes too: a 30 fps camera's frame time.
 */
export const PETAL_CAPTURE_BUDGET_MS = CAPTURE_BUDGET_MS;
