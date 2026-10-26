/**
 * Petal Stream: the Sakura-storm optical transport (a streaming visual code), format 5.
 *
 * A sender shows a sequence of frames; a camera reads them and a payload is
 * reassembled. Every frame is a portrait card of 1024 x 1668 design units: a
 * square code with a band of 306 katakana tiles above and below it. Every
 * frame carries independent lanes, each one Reed-Solomon codeword over
 * GF(256): the ring dots (lane `D`), the four sectors of a field of 3,376
 * petals, each lit or dark (lanes `F0`-`F3`), on top of each field lane the
 * turns of its first 360 lit petals, 0, 45, 90 or 135 degrees about their
 * centres (turn lanes `T0`-`T3`, named by the digit of their field lane), and
 * in each band the polarity (light or dark, lanes `P0`, `P1`) and the
 * katakana (one of sixteen, lanes `K0`, `K1`) of its 298 data tiles. Lanes
 * carry fountain-coded 16-byte atoms; lane `D` carries the stream beacon on
 * every fourth frame.
 *
 * This subpath is a port of the `iroha_petal` Rust reference: the encoder is
 * bit-exact (shared fixtures) and the decoder follows the reference step by
 * step, reading the golden captures exactly as recorded. It is
 * pure JavaScript (typed arrays only, no Node built-ins, no WebAssembly) and
 * browser-safe; DOM objects are only touched by {@link PetalStreamPlayer} and
 * {@link PetalCameraScanner} when they run.
 */

/** A field lane name: the petals of sector `0`-`3`. */
export type PetalFieldLane = "F0" | "F1" | "F2" | "F3";

/** A lane name: the ring dots (`D`) or a field lane. */
export type PetalLane = "D" | PetalFieldLane;

/** Four values, one per field sector (sector 0 first). */
export type PetalSectors<T> = readonly [T, T, T, T];

/** A kana lane name: the polarity (`P`) or glyph (`K`) lane of band 0 (above the square) or 1 (below it). */
export type PetalKanaLane = "P0" | "K0" | "P1" | "K1";

/** Four values, one per kana lane, in the order `P0`, `K0`, `P1`, `K1`. */
export type PetalKanaLanes<T> = readonly [T, T, T, T];

/** Two values, one per katakana band (band 0, above the square, first). */
export type PetalBands<T> = readonly [T, T];

/** Byte input accepted by the codec. */
export type PetalBytes = Uint8Array | ArrayBuffer | ArrayBufferView | readonly number[];

/** An `[r, g, b]` byte triple. */
export type PetalRgb = readonly [number, number, number];

/** Codes carried by {@link PetalError}. */
export type PetalErrorCode =
  | "rs_invalid_shape"
  | "rs_uncorrectable"
  | "empty_payload"
  | "payload_too_large"
  | "unsupported_image"
  | "no_finders"
  | "no_orientation";

/** Why a camera frame could not be decoded at all. */
export type PetalDecodeErrorCode = "unsupported_image" | "no_finders" | "no_orientation";

/** Error raised by the Petal Stream codec; `code` identifies the failure. */
export class PetalError extends Error {
  constructor(code: PetalErrorCode);
  readonly code: PetalErrorCode;
}

// ------------------------------------------------------------------ constants

/**
 * Normative frame geometry in design units (square coordinates: canvas 1024, origin top-left, `y`
 * down; the card adds a band of 322 units above the square and one below it).
 */
export interface PetalLayout {
  readonly canvas: 1024;
  readonly center: 512;
  /** Concentric petal bands; band `j` has radius `bandInner + bandSpacing * j`. */
  readonly bandCount: 34;
  readonly bandInner: 16;
  readonly bandSpacing: 11;
  /** Slots (field cells) of every band, inner band first; every count is a multiple of four. */
  readonly bandSlots: readonly number[];
  /** Index of the first cell of every band, and the total at the end. */
  readonly bandOffsets: readonly number[];
  readonly fieldCells: 3376;
  /** Angular sectors of the field, one field lane each. */
  readonly sectors: 4;
  /** Cells per sector: 840 carry its codeword, the last four stay dark. */
  readonly sectorCells: 844;
  /**
   * Radius of the disc a petal stands in for: decoders sample within it, and no other petal,
   * however it is turned, reaches into it.
   */
  readonly petalRadius: 4.6;
  /** Half-length of a drawn petal along its axis (the outward direction plus its turn). */
  readonly petalHalfLength: 6;
  /** Half-width of a drawn petal. */
  readonly petalHalfWidth: 3.73;
  /** Radius of the notch cut into each petal tip; it tells a petal from one turned by half a turn. */
  readonly petalNotchRadius: 1.12;
  /** Distance from a petal centre to its notch centre, along its axis. */
  readonly petalNotchDistance: 6.24;
  readonly maskGrid: 20;
  /** Canvas coordinate of the `天` lattice's left and top edge. */
  readonly maskOrigin: 182;
  readonly maskPitch: 33;
  /** The `天` backdrop is drawn inside this radius only. */
  readonly backdropRadius: 390;
  /** The `天` silhouette, one 20-character row string per lattice row (`#` is inside the character). */
  readonly mask: readonly string[];
  readonly ringRadii: readonly number[];
  readonly ringSlots: readonly number[];
  readonly dotRadius: 9;
  readonly totalSlots: 296;
  /** Dots per gate (right, bottom, left) and ring. */
  readonly gateDots: readonly (readonly number[])[];
  readonly dBits: 240;
  /** Corner finder centres, clockwise from the top-left. */
  readonly finderCenters: readonly (readonly [number, number])[];
  readonly finderCore: 12;
  readonly finderPetals: 5;
  readonly finderPetalDistance: 34;
  readonly finderPetalRadius: 26;
  readonly finderNotchRadius: 6;
  readonly finderOuter: 60;
  /** Height of the card: the square with a band above and below it. */
  readonly cardHeight: 1668;
  /** Card `y` of the square's top edge: card coordinates are square coordinates plus this in `y`. */
  readonly squareTop: 322;
  /** Side of a katakana tile, a square with rounded corners. */
  readonly kanaTile: 25;
  /** Pitch of the tile lattice. */
  readonly kanaPitch: 29;
  /** Side of a tile's glyph box, centred in the tile. */
  readonly kanaGlyphBox: 23;
  /** Corner radius of a tile. */
  readonly kanaCorner: 3;
  readonly kanaRows: 10;
  readonly kanaColumns: 35;
  /** Gap between a band's lattice and the square. */
  readonly kanaGap: 24;
  /** Margin outside a band's lattice. */
  readonly kanaMargin: 8;
  /** Height of a band with its gap and margin. */
  readonly kanaBandHeight: 322;
  /** Tile centres keep at least this distance from every finder centre (lattice cells closer hold no tile). */
  readonly kanaFinderClearance: 198;
  readonly kanaBands: 2;
  /** Pilot tiles of each band, with fixed symbols. */
  readonly kanaPilots: 8;
  /** Tiles of each band. */
  readonly kanaBandTiles: 306;
  /** Tiles of both bands. */
  readonly kanaTileCount: 612;
  /** The pilot tiles of each band as `[row, column]`, in pilot order (band 1 mirrors band 0's rows). */
  readonly kanaPilotCells: readonly (readonly (readonly [number, number])[])[];
  /** What every pilot shows: `[glyph, light]` in pilot order. */
  readonly kanaPilotSymbols: readonly (readonly [number, boolean])[];
}

/** Normative frame geometry. */
export const PETAL_LAYOUT: PetalLayout;

/** Byte sizes of one lane. */
export interface PetalLaneSize {
  readonly wordLen: number;
  readonly parityLen: number;
  readonly dataLen: number;
  /** Atoms carried (lane `D` carries the beacon instead on beacon frames). */
  readonly atoms: number;
}

/** Lane sizes in decode order: D 30/11/19, F0-F3 105/22/83 (word/parity/data bytes; turn lanes: {@link PETAL_TURNS}). */
export const PETAL_LANES: { readonly [lane in PetalLane]: PetalLaneSize };

/** Stream-level wire constants and receiver defaults. */
export const PETAL_STREAM: {
  readonly atomLen: 16;
  readonly laneHeaderLen: 3;
  readonly atomsPerFrame: 21;
  /** Cells at the end of every sector that carry no bit (they stay dark). */
  readonly spareSectorCells: 4;
  /** Beacon version byte: format version 5, layout profile 0 (`0x50`; `0x30`, `0x20` and `0x10` are refused). */
  readonly formatVersion: 80;
  readonly beaconInterval: 4;
  readonly maxPayloadLen: 16777215;
  readonly defaultMaxPayloadLen: 65536;
  readonly defaultMaxPendingAtoms: 512;
};

/**
 * The turn lanes `T0`-`T3`. The first `carriers` lit petals of a sector (in its carrier order)
 * each show `bits` bits as one of `positions` turns; the lane is one codeword of `wordLen` bytes
 * (`parityLen` parity), whose `dataLen` data bytes are `atoms` atoms and a `checkLen`-byte check.
 */
export const PETAL_TURNS: {
  readonly positions: 4;
  readonly bits: 2;
  readonly carriers: 360;
  readonly wordLen: 90;
  readonly parityLen: 22;
  readonly dataLen: 68;
  readonly atoms: 4;
  readonly checkLen: 4;
  /** Fountain id of the first turn atom of frame 0 (`2^31`). */
  readonly idBase: 2147483648;
  /** Gaussian blurs (sigma in canvas units) the turn reader fits: 1.0, 1.4, 1.9, 2.5, 3.2, 4.0. */
  readonly blurs: readonly number[];
  /** Turn lanes a scan session remembers as tried (the four of each of the last six shown frames). */
  readonly memory: 24;
  /** Most captures a scan session passes over after tries that read no turn lane. */
  readonly backoff: 8;
};

/** Byte sizes of a kana lane. */
export interface PetalKanaLaneSize {
  readonly wordLen: number;
  readonly parityLen: number;
  /** The atoms, then a 4-byte check. */
  readonly dataLen: number;
  readonly atoms: number;
}

/**
 * The kana lanes of the katakana bands: the polarity lanes `P0`, `P1` (37-byte words, 17 parity,
 * one atom) and the glyph lanes `K0`, `K1` (149-byte words, 33 parity, seven atoms), each closed by
 * a check that binds its atoms to the stream tag, the frame and the lane.
 */
export const PETAL_KANA: {
  /** The lanes in their order: `P0`, `K0`, `P1`, `K1`. */
  readonly lanes: readonly PetalKanaLane[];
  readonly polarity: PetalKanaLaneSize;
  readonly glyph: PetalKanaLaneSize;
  /** Data tiles of each band (306 tiles less 8 pilots). */
  readonly dataTiles: 298;
  readonly checkLen: 4;
  /** Fountain id of the first kana atom of frame 0 (`2^31 + 2^20`). */
  readonly idBase: 2148532224;
  /** Gaussian blurs (sigma in canvas units) of the band reader's glyph templates: 1.0, 1.6, 2.4, 3.4, 4.8. */
  readonly blurs: readonly number[];
  /** Samples per tile side. */
  readonly grid: 12;
  /** Samples per tile side in the coarse pilot search. */
  readonly coarseGrid: 6;
  /** Bands a scan session remembers as tried (the two of each of the last six shown frames). */
  readonly memory: 12;
  /** Most captures a scan session passes over after band tries that read no kana lane. */
  readonly backoff: 8;
};

/**
 * The sixteen katakana of the band tiles, symbols `0`-`15` (`chars`: イロハニヘトワカレムノケアヒスン),
 * as stroke polylines of `[x, y]` points on a `grid` x `grid` design grid (`y` down), drawn with
 * round caps and joins of width `strokeWidth`: a point is inked within half a stroke width of a
 * stroke segment.
 */
export const PETAL_GLYPHS: {
  readonly count: 16;
  readonly grid: 32;
  readonly strokeWidth: 6.5;
  readonly chars: string;
  /** `strokes[glyph][stroke][point]` is `[x, y]`. */
  readonly strokes: readonly (readonly (readonly (readonly [number, number])[])[])[];
};

/** Colours of the picture. */
export interface PetalPalette {
  readonly background: PetalRgb;
  /** Finders, the petals inside the `天` and the light katakana tiles. */
  readonly light: PetalRgb;
  /** Sakura pink of the ring dots, of the petals outside the `天` and of the glyphs on dark tiles. */
  readonly pink: PetalRgb;
  /** The faint `天` drawn under the petals. */
  readonly backdrop: PetalRgb;
  /** Glyphs on light katakana tiles. */
  readonly ink: PetalRgb;
}

/** Default palette: background (0,0,0), light (250,235,244), pink (245,175,208), backdrop (25,24,24), ink (20,4,14). */
export const PETAL_PALETTE: PetalPalette;

/** Decoder tuning. */
export interface PetalDecodeOptions {
  /** Also try horizontally mirrored images (front-camera previews). Default `true`. */
  readonly tryMirrored?: boolean;
  /** Largest image (in pixels) the decoder accepts. Default 12,000,000. */
  readonly maxPixels?: number;
}

/** Default decoder options. */
export const PETAL_DEFAULT_DECODE_OPTIONS: Required<PetalDecodeOptions>;

/** Receiver memory and size limits. */
export interface PetalAssemblerLimits {
  /** Largest payload accepted (default 65536). */
  readonly maxPayloadLen?: number;
  /** Atoms buffered while waiting for the first beacon (default 512; zero buffers nothing). */
  readonly maxPendingAtoms?: number;
}

/** Limits of a scan session. */
export interface PetalScanLimits {
  /** Forget a half-received stream after this long without progress (default 30 s). */
  readonly idleTimeoutMs?: number;
  /** Forget a stream that has not finished this long after it started (default 180 s). */
  readonly absoluteTimeoutMs?: number;
  readonly assembler?: PetalAssemblerLimits;
  readonly decode?: PetalDecodeOptions;
}

/** Scan limits with every default filled in. */
export interface PetalResolvedScanLimits {
  readonly idleTimeoutMs: number;
  readonly absoluteTimeoutMs: number;
  readonly assembler: Required<PetalAssemblerLimits>;
  readonly decode: Required<PetalDecodeOptions>;
}

/** Default scan-session limits. */
export const PETAL_DEFAULT_SCAN_LIMITS: PetalResolvedScanLimits;
/** How long (in milliseconds) a scan session tracks a code from its last decoded pose (500). */
export const PETAL_TRACK_WINDOW_MS: number;
/**
 * The time a scan session's capture may take in milliseconds, turn lanes included, for a capture
 * whose bands read a lane to try the turn lanes too: a 30 fps camera's frame time (1000 / 30).
 */
export const PETAL_CAPTURE_BUDGET_MS: number;

// ------------------------------------------------------------------ primitives

/** CRC-32C (Castagnoli, reflected, init and final xor `0xFFFFFFFF`) as an unsigned integer. */
export function crc32c(bytes: PetalBytes): number;

/** Marsaglia xorshift32 (13, 17, 5); a zero seed becomes `0xDEADBEEF`. Part of the wire format. */
export class PetalXorshift32 {
  constructor(seed: number);
  /** Advances the generator and returns the next unsigned 32-bit word. */
  nextU32(): number;
  /** Returns the top byte of the next word. */
  nextByte(): number;
}

/** Multiplies two GF(256) elements (polynomial 0x11D). */
export function gfMul(a: number, b: number): number;
/** Returns alpha^exponent in GF(256). */
export function gfExp(exponent: number): number;

/**
 * Systematic Reed-Solomon code over GF(256) (0x11D, first root alpha^0) with
 * errors-and-erasures decoding.
 */
export class PetalReedSolomon {
  /** @param nsym parity byte count, 1..=254 */
  constructor(nsym: number);
  /** Number of parity bytes. */
  readonly parityLen: number;
  /** Returns `data || parity`. */
  encode(data: PetalBytes): Uint8Array;
  /**
   * Corrects `word` in place (succeeds while `2 * errors + erasures <= parityLen`)
   * and returns the number of corrected positions.
   *
   * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
   */
  decode(word: Uint8Array, erasures?: readonly number[]): number;
}

// ------------------------------------------------------------------ layout

/** Role of a ring slot. */
export type PetalSlotRole =
  | { readonly kind: "gate" | "guard" | "spare" }
  | { readonly kind: "data"; readonly bit: number };

/** Radius of petal band `band`. */
export function bandRadius(band: number): number;
/** Fraction of a slot by which band `band` is turned (odd bands sit half a slot over). */
export function bandPhase(band: number): number;
/** Band and slot of field cell `cell` (0..3376). */
export function cellBandSlot(cell: number): [number, number];
/** Angle (radians, clockwise on screen from 3 o'clock) of the centre of field cell `cell`. */
export function cellAngle(cell: number): number;
/** Canvas coordinates of the centre of field cell `cell`. */
export function cellCenter(cell: number): [number, number];
/** Whether field cell `cell` lies inside the `天` (its petal is drawn in the light colour). */
export function cellLight(cell: number): boolean;
/**
 * The cells of sector `sector` in bit order: band by band from the centre out, ascending slot
 * within a band. The first 840 carry the sector's codeword; the last four are spare.
 */
export function sectorCells(sector: number): number[];
/** Whether the canvas point `(x, y)` lies inside the `天` mask. */
export function inMask(x: number, y: number): boolean;
/**
 * Whether the point `(dx, dy)` relative to the centre of a petal pointing along `angle` is lit:
 * the ellipse of the petal (half-length 6 along `angle`, half-width 3.73) minus its notch.
 */
export function petalLit(dx: number, dy: number, angle: number): boolean;
/** Role of every one of the 296 ring slots in flat index order. */
export function slotRoles(): PetalSlotRole[];
/** Flat slot index of every lane `D` bit, in bit order. */
export function dataSlots(): number[];
/** Splits a flat slot index into `[ring, slot]`. */
export function splitSlot(flat: number): [number, number];
/** Canvas coordinates (single-precision values) of slot `slot` on ring `ring`; slot 0 is at 3 o'clock, clockwise. */
export function slotCenter(ring: number, slot: number): [number, number];
/** Offset of ring `ring` inside the flat slot index space. */
export function ringOffset(ring: number): number;
/** Whether the point `(dx, dy)` relative to a finder centre is lit (five-petal blossom). */
export function finderLit(dx: number, dy: number): boolean;

/** One katakana tile of the card. */
export interface PetalKanaTile {
  /** Band: 0 above the square, 1 below it. */
  readonly band: number;
  /** Row within its band, top first. */
  readonly row: number;
  /** Column, left first. */
  readonly column: number;
  /** Left edge of the tile square in square coordinates. */
  readonly x: number;
  /** Top edge of the tile square in square coordinates (negative above the square). */
  readonly y: number;
  /** Index among the pilots of its band, or `null` for a data tile. */
  readonly pilot: number | null;
}

/**
 * Every tile of both bands: band 0 first, each in reading order (rows top first, columns left
 * first); lattice cells within 198 units of a finder centre hold no tile (306 per band remain).
 */
export function kanaTiles(): PetalKanaTile[];
/** The data tiles of band `band` (its 298 tiles that are not pilots) in reading order, as indices into {@link kanaTiles}. */
export function kanaDataTiles(band: number): number[];
/** The eight pilot tiles of band `band` in pilot order, as indices into {@link kanaTiles}. */
export function kanaPilotTiles(band: number): number[];
/**
 * The tile under the square-coordinate point `(x, y)` and the point's position inside the tile's
 * square (`0..=25` on both axes) as `[tile, u, v]`; `null` outside every tile square.
 */
export function kanaTileAt(x: number, y: number): [number, number, number] | null;
/** Whether the tile-local point `(u, v)` lies inside the rounded tile square. */
export function kanaInside(u: number, v: number): boolean;
/**
 * Whether the tile-local point `(u, v)` lies on `glyph`'s strokes: inside the tile square and
 * inked in the glyph box (the tile inset by 1 unit, design point `((u - 1) * 32 / 23, (v - 1) * 32 / 23)`).
 */
export function kanaInked(glyph: number, u: number, v: number): boolean;
/** Whether the design-grid point `(x, y)` is inked in `glyph` (`0..15`): within 3.25 of a stroke segment. */
export function isGlyphInked(glyph: number, x: number, y: number): boolean;
/** The strokes of `glyph` (`0..15`): polylines of `[x, y]` points on the 32 x 32 design grid (a copy). */
export function glyphStrokes(glyph: number): [number, number][][];

// ------------------------------------------------------------------ lanes

/** The field lane of sector `sector` (0..4). */
export function fieldLane(sector: number): PetalFieldLane;
/** The letter that names `lane` in reports: `D`, or the sector digit `0`-`3`. */
export function laneLetter(lane: PetalLane): "D" | "0" | "1" | "2" | "3";
/** The fixed whitening sequence of a lane. */
export function laneWhitening(lane: PetalLane): Uint8Array;
/** Encodes lane data (exactly the lane's data length) into the transmitted, whitened codeword. */
export function encodeLane(lane: PetalLane, data: PetalBytes): Uint8Array;
/**
 * Decodes a transmitted codeword, returning the lane data bytes.
 *
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeLane(lane: PetalLane, transmitted: PetalBytes, erasures?: readonly number[]): Uint8Array;
/**
 * Like {@link decodeLane}, also returning how many byte positions the
 * Reed-Solomon decoder rewrote: the erased bytes plus unflagged errors (`0`
 * for a word that was already valid).
 *
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeLaneCounted(
  lane: PetalLane,
  transmitted: PetalBytes,
  erasures?: readonly number[],
): { readonly data: Uint8Array; readonly corrected: number };

/**
 * Every cell of one frame: what a renderer draws and a decoder samples: which ring slots and petals
 * are lit, how every petal is turned, and what every katakana tile of the card's bands shows.
 */
export class PetalFrameCells {
  /**
   * Omitted katakana symbols are the pilots' fixed ones and light tiles of glyph 0.
   */
  constructor(
    dots?: ArrayLike<number | boolean>,
    petals?: ArrayLike<number | boolean>,
    turns?: ArrayLike<number>,
    kanaGlyphs?: ArrayLike<number>,
    kanaLight?: ArrayLike<number | boolean>,
  );
  /** Lit state (`0`/`1`) of each of the 296 ring slots, gate dots included. */
  readonly dots: Uint8Array;
  /** Lit state (`0`/`1`) of each of the 3,376 field cells (spare cells are dark). */
  readonly petals: Uint8Array;
  /**
   * Turn position (0-3, see {@link turnAngle}) of each of the 3,376 field cells: zero for dark
   * petals and for lit petals that carry no turn bits.
   */
  readonly turns: Uint8Array;
  /** The glyph (0-15) each of the 612 katakana tiles shows, in {@link kanaTiles} order. */
  readonly kanaGlyphs: Uint8Array;
  /** Whether each of the 612 katakana tiles is light (`1`: filled, glyph in ink) or dark (`0`: glyph in pink). */
  readonly kanaLight: Uint8Array;
  /**
   * Builds the cells from the transmitted codewords of lane `D` and the four field lanes, with
   * every petal unturned (see {@link PetalFrameCells.setTurns}) and only the pilots set among the
   * katakana tiles (see {@link PetalFrameCells.setKana}).
   */
  static fromWords(d: PetalBytes, fields: PetalSectors<PetalBytes>): PetalFrameCells;
  /**
   * Turns the carriers of every sector to show its transmitted turn codeword (sector 0 first); a
   * sector with too few lit petals stays unturned.
   */
  setTurns(words: PetalSectors<PetalBytes>): void;
  /** Packs the turns of the carriers of sector `sector` into its turn codeword; `null` without carriers. */
  turnWord(sector: number): Uint8Array | null;
  /**
   * Shows the transmitted kana codewords (`P0`, `K0`, `P1`, `K1`) on the data tiles of both bands:
   * data tile `i` shows bit `i` of its band's polarity codeword (the two tiles beyond it are light)
   * and nibble `i` of its glyph codeword; the pilots keep their fixed symbols.
   */
  setKana(words: PetalKanaLanes<PetalBytes>): void;
  /** Packs the polarity codeword (37 bytes) the data tiles of band `band` show. */
  kanaPolarityWord(band: number): Uint8Array;
  /** Packs the glyph codeword (149 bytes) the data tiles of band `band` show. */
  kanaGlyphWord(band: number): Uint8Array;
  /** Packs the data dots into a lane `D` codeword. */
  dWord(): Uint8Array;
  /** Packs the petals of sector `sector` into its field codeword. */
  fieldWord(sector: number): Uint8Array;
  /** Whether `other` holds exactly the same cells, turns and katakana tiles included. */
  equals(other: unknown): boolean;
}

// ------------------------------------------------------------------ fountain

/** Splits a payload into zero-padded 16-byte source atoms. */
export function splitPayload(payload: PetalBytes): Uint8Array[];
/** Number of 32-bit mask words for `k` source atoms. */
export function maskLen(k: number): number;
/** MurmurHash3 `fmix32`. */
export function mix32(value: number): number;
/** Combination mask (little-endian bit words) of encoded atom `id`. */
export function maskWords(k: number, crc: number, id: number): Uint32Array;
/** Encodes fountain atom `id` from the source atoms. */
export function encodeAtom(source: readonly Uint8Array[], crc: number, id: number): Uint8Array;

/** Incremental GF(2) Gaussian-elimination decoder of the fountain code. */
export class PetalFountainDecoder {
  constructor(k: number);
  /** Number of source atoms. */
  readonly sourceAtoms: number;
  /** Number of independent atoms received so far. */
  readonly rank: number;
  isComplete(): boolean;
  /** Adds encoded atom `id`; returns whether it raised the rank. */
  addEncoded(crc: number, id: number, atom: PetalBytes): boolean;
  /** Adds a received combination; returns whether it raised the rank. */
  add(mask: ArrayLike<number>, atom: PetalBytes): boolean;
  /** The source atoms once complete, otherwise `null`. */
  solve(): Uint8Array[] | null;
}

// ------------------------------------------------------------------ stream

/** Identity of a stream, as carried by every beacon. */
export interface PetalStreamMeta {
  /** Application payload kind (`0..=255`). */
  readonly kind: number;
  /** Payload length in bytes. */
  readonly len: number;
  /** CRC-32C of the payload. */
  readonly crc: number;
  /** One-byte stream tag repeated in every lane header (low byte of `crc`). */
  readonly tag: number;
  /** Number of fountain source atoms. */
  readonly sourceAtoms: number;
}

/** The common three-byte header of every lane. */
export interface PetalLaneHeader {
  readonly tag: number;
  /** Frame counter (wraps at 65536). */
  readonly frame: number;
}

/** A decoded beacon. */
export interface PetalBeacon {
  readonly header: PetalLaneHeader;
  readonly meta: PetalStreamMeta;
}

/** Atoms read from one lane; ids run consecutively from `firstId`. */
export interface PetalAtomPacket {
  readonly header: PetalLaneHeader;
  readonly firstId: number;
  readonly atoms: readonly Uint8Array[];
}

/** What lane `D` carried. */
export type PetalDLane =
  | { readonly type: "beacon"; readonly beacon: PetalBeacon }
  | { readonly type: "atoms"; readonly packet: PetalAtomPacket };

/** Data bytes or codewords of the lanes of one frame. */
export interface PetalLaneWords {
  readonly d: Uint8Array;
  /** The field lanes, sector 0 first. */
  readonly fields: PetalSectors<Uint8Array>;
  /** The turn lanes, sector 0 first: atoms and check (68 bytes), or the 90-byte codewords. */
  readonly turns: PetalSectors<Uint8Array>;
  /** The kana lanes `P0`, `K0`, `P1`, `K1`: atoms and check (20 or 116 bytes), or the 37- or 149-byte codewords. */
  readonly kana: PetalKanaLanes<Uint8Array>;
}

/** Snapshot of receive progress for a UI. */
export interface PetalProgress {
  readonly meta: PetalStreamMeta | null;
  readonly sourceAtoms: number;
  readonly rank: number;
  /** Atoms offered to the decoder (including duplicates). */
  readonly atomsReceived: number;
  /**
   * Reassembled payloads that failed the CRC check and were discarded
   * (cumulative over the assembler's lifetime, not cleared by `reset`).
   */
  readonly integrityFailures: number;
  readonly complete: boolean;
}

/** A reassembled, CRC-verified payload. */
export interface PetalCompleted {
  readonly meta: PetalStreamMeta;
  readonly payload: Uint8Array;
}

/** Whether `frame` carries the beacon in lane `D` (every fourth frame). */
export function isBeaconFrame(frame: number): boolean;
/** Fountain atoms carried by `frame` (20 on beacon frames, otherwise 21). */
export function atomsInFrame(frame: number): number;
/** Fountain id of the first atom of `frame`. */
export function firstAtomId(frame: number): number;
/** Fountain id of the first atom `lane` carries in `frame`: field lanes in sector order, then `D`. */
export function laneFirstId(lane: PetalLane, frame: number): number;
/** Parses the data bytes of a field lane; `null` for lane `D` or a wrong length. */
export function parseAtomLane(lane: PetalLane, data: PetalBytes): PetalAtomPacket | null;
/** Parses the data bytes of lane `D`; `null` for malformed data. */
export function parseDLane(data: PetalBytes): PetalDLane | null;

/** Sender side: turns one payload into an endless sequence of frames. */
export class PetalStreamEncoder {
  /**
   * @param kind application payload kind, `0..=255`
   * @throws {PetalError} `empty_payload` or `payload_too_large`
   */
  constructor(payload: PetalBytes, kind: number);
  readonly meta: PetalStreamMeta;
  /** Frames needed to send every source atom once in the field lanes and lane `D`. */
  systematicFrames(): number;
  /** Data bytes of every lane of `frame` (reduced modulo 65536), turn and kana lanes included. */
  laneData(frame: number): PetalLaneWords;
  /** Transmitted codewords of `frame`. */
  words(frame: number): PetalLaneWords;
  /** Every cell of `frame`, lit, turned and with its katakana tiles, ready to render. */
  cells(frame: number): PetalFrameCells;
}

/** Receiver side: collects atoms from any lane of any frame. */
export class PetalStreamAssembler {
  constructor(limits?: PetalAssemblerLimits);
  readonly limits: Required<PetalAssemblerLimits>;
  /** Forgets the active stream, buffered atoms and any completed payload (not `integrityFailures`). */
  reset(): void;
  progress(): PetalProgress;
  /** Takes the completed payload, if any. */
  takeCompleted(): PetalCompleted | null;
  /** Offers a beacon; a different stream replaces the active one after two consecutive sightings. */
  pushBeacon(beacon: PetalBeacon): void;
  pushAtoms(packet: PetalAtomPacket): void;
  pushDLane(lane: PetalDLane): void;
}

// ------------------------------------------------------------------ turn lanes

/**
 * The angle (radians, clockwise on screen) by which a petal at turn position `position` (0-3) is
 * turned from its outward direction: `pi * position / 4`.
 */
export function turnAngle(position: number): number;
/**
 * The two bits a turn position shows: its Gray code `position ^ (position >> 1)`, so that
 * neighbouring turns (135 and 0 degrees included) differ in one bit.
 */
export function turnLabel(position: number): number;
/** The turn position that shows the two bits `label` (the inverse of {@link turnLabel}). */
export function turnPosition(label: number): number;
/**
 * The order in which the lit petals of sector `sector` become carriers: its 840 codeword cells
 * shuffled by Fisher-Yates with an xorshift32 seeded `0x4F524430 + sector`.
 */
export function carrierOrder(sector: number): number[];
/**
 * The carriers of sector `sector` under the lit map `petals` (one flag per field cell): the first
 * 360 lit cells in {@link carrierOrder}, or `null` when fewer are lit.
 */
export function carriers(petals: ArrayLike<number | boolean>, sector: number): number[] | null;
/**
 * Fountain id of the first of the four atoms the turn lane of `sector` carries in `frame`
 * (reduced modulo 65536): `2^31 + 16 * frame + 4 * sector`.
 */
export function turnFirstId(frame: number, sector: number): number;
/**
 * The check of a turn lane: CRC-32C over the stream tag, the frame (big-endian), the sector and
 * the atoms of the field lane header `header` the turn lane rides on.
 */
export function turnCheck(header: PetalLaneHeader, sector: number, atoms: PetalBytes): number;
/** The data of a turn lane: the four atoms (64 bytes) followed by their check (big-endian). */
export function turnData(header: PetalLaneHeader, sector: number, atoms: PetalBytes): Uint8Array;
/**
 * The atoms of turn lane data whose check matches `header` and `sector`; `null` for data of the
 * wrong length, a sector out of range or a check that does not match.
 */
export function turnPacket(header: PetalLaneHeader, sector: number, data: PetalBytes): PetalAtomPacket | null;
/** The whitening sequence of the turn lane of `sector` (xorshift32 seeded `0x54524E30 + sector`). */
export function turnWhitening(sector: number): Uint8Array;
/** Encodes turn lane data (68 bytes) into the transmitted, whitened 90-byte codeword. */
export function encodeTurnLane(sector: number, data: PetalBytes): Uint8Array;
/**
 * Decodes a transmitted turn codeword, returning its data and how many byte positions the
 * Reed-Solomon decoder rewrote; the check is not verified (see {@link turnPacket}).
 *
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeTurnLaneCounted(
  sector: number,
  transmitted: PetalBytes,
  erasures?: readonly number[],
): { readonly data: Uint8Array; readonly corrected: number };
/** The turn position carrier `index` of a transmitted turn codeword shows. */
export function carrierPosition(word: PetalBytes, index: number): number;

/** A decoded frame as the turn reader needs it: its pose and its field lanes. */
export interface PetalTurnFrame {
  readonly pose: PetalPose;
  readonly fields: PetalSectors<PetalLaneResult | null>;
}

/**
 * Reads the turn lanes of the sectors `frame` decoded from `image` (the luma plane it was decoded
 * from), except those flagged in `skip`. Every carrier is sampled in its petal's own frame,
 * following a smooth correction of the pose fitted to the carriers' centroids; the blur is fitted
 * to the carriers and their lit neighbours (none is read from a fitted blur of 3.2 units on), and
 * each carrier takes the turn whose template matches best once its lit neighbours are
 * subtracted. A lane passes when its codeword decodes with 0, 2, 5, 7 or 11 of its least
 * confident bytes erased into data whose check matches its sector's field lane header. Returns
 * the lane data (atoms and check) per sector.
 */
export function readTurns(
  image: PetalLumaLike,
  frame: PetalTurnFrame,
  skip?: PetalSectors<boolean>,
): PetalSectors<PetalLaneResult | null>;

/** What the turn reader decided for one capture. */
export interface PetalObservedTurns {
  /** The blur (Gaussian sigma in canvas units) that fits the carriers best. */
  readonly blur: number;
  /**
   * Every carrier of the decoded sectors, sector by sector in carrier order: its cell, the turn
   * position decided for it, and how far that position's score led the runner-up's.
   */
  readonly carriers: readonly { readonly cell: number; readonly position: number; readonly lead: number }[];
}

/**
 * What {@link readTurns} decides for every carrier of the sectors `frame` decoded, before any
 * Reed-Solomon correction and whatever the blur, for diagnostics; `null` when no decoded sector
 * has carriers or the image cannot be sampled.
 */
export function observedTurns(image: PetalLumaLike, frame: PetalTurnFrame): PetalObservedTurns | null;

/**
 * The turn reader's blurred petal template for blur `blur` (an index into `PETAL_TURNS.blurs`) at
 * a point of the unturned petal's own frame (`along` its axis, `across` it); conformance tests
 * compare it with the fixture's probes.
 */
export function turnTemplate(blur: number, along: number, across: number): number;

// ------------------------------------------------------------------ kana lanes

/**
 * Fountain id of the first atom `lane` carries in `frame` (reduced modulo 65536):
 * `2^31 + 2^20 + 16 * frame + 8 * band`, plus 7 for the polarity lane (the glyph lane's seven atoms
 * come first).
 */
export function kanaFirstId(frame: number, lane: PetalKanaLane): number;
/**
 * The check of a kana lane: CRC-32C over the stream tag, the frame (big-endian), the lane's code
 * (`0x10 | band << 1 | kind`, polarity 0 and glyph 1) and the atoms.
 */
export function kanaCheck(header: PetalLaneHeader, lane: PetalKanaLane, atoms: PetalBytes): number;
/** The data of a kana lane: its atoms (one or seven) followed by their check (big-endian). */
export function kanaData(header: PetalLaneHeader, lane: PetalKanaLane, atoms: PetalBytes): Uint8Array;
/** The atoms of kana lane data whose check matches `header` and `lane`; `null` for the wrong length or check. */
export function kanaPacket(header: PetalLaneHeader, lane: PetalKanaLane, data: PetalBytes): PetalAtomPacket | null;
/** The whitening sequence of a kana lane (xorshift32 seeded "POL0", "POL1", "KAN0" or "KAN1"). */
export function kanaWhitening(lane: PetalKanaLane): Uint8Array;
/** Encodes kana lane data (20 or 116 bytes) into the transmitted, whitened codeword (37 or 149 bytes). */
export function encodeKanaLane(lane: PetalKanaLane, data: PetalBytes): Uint8Array;
/**
 * Decodes a transmitted kana codeword, returning its data and how many byte positions the
 * Reed-Solomon decoder rewrote; the check is not verified (see {@link kanaPacket}).
 *
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeKanaLaneCounted(
  lane: PetalKanaLane,
  transmitted: PetalBytes,
  erasures?: readonly number[],
): { readonly data: Uint8Array; readonly corrected: number };

/** A kana lane read from a capture, with the header (stream tag and frame) its check matched. */
export interface PetalKanaResult extends PetalLaneResult {
  /** The square's frame, the frame after it or the frame before it. */
  readonly header: PetalLaneHeader;
}

/** A decoded frame as the band reader needs it: its pose. */
export interface PetalKanaFrame {
  readonly pose: PetalPose;
}

/**
 * Reads the kana lanes of the bands not flagged in `skip` of a capture whose square `frame` decoded
 * under `header` (the stream tag and frame its lanes name). The square's pose, extended beyond the
 * square, places every tile; each band's pilots are found by a small search, pick the band's blur
 * and set the light and dark levels, and an affine shift fitted to their offsets moves every tile.
 * A lane passes when its codeword decodes, with 0, 1/8, 1/4, 1/3 or 1/2 of its parity bytes' worth of
 * its least confident bytes erased, into data whose check matches `header`'s frame, the frame after
 * it or the frame before it. Returns `P0`, `K0`, `P1`, `K1`.
 */
export function readKana(
  image: PetalLumaLike,
  frame: PetalKanaFrame,
  header: PetalLaneHeader,
  skip?: PetalBands<boolean>,
): PetalKanaLanes<PetalKanaResult | null>;

/** What the band reader decided for one data tile. */
export interface PetalTileReading {
  /** Index into {@link kanaTiles}. */
  readonly tile: number;
  /** Decided polarity. */
  readonly light: boolean;
  /** How far the tile's level lay from the threshold, in half-contrasts. */
  readonly lightMargin: number;
  /** Decided glyph (0-15). */
  readonly glyph: number;
  /** How far the glyph's correlation led the runner-up's. */
  readonly glyphLead: number;
}

/** What the band reader saw of one band. */
export interface PetalBandReading {
  /** The blur (sigma in canvas units) the pilots chose. */
  readonly blur: number;
  /** Pilots found. */
  readonly pilots: number;
  /** Every data tile of the band in data order, `null` for the tiles it could not read. */
  readonly tiles: readonly (PetalTileReading | null)[];
}

/**
 * What {@link readKana} sees of band `band` before any Reed-Solomon correction, for diagnostics;
 * `null` when fewer than two pilots are found or the pilots show no contrast.
 */
export function observeBand(image: PetalLumaLike, frame: PetalKanaFrame, band: number): PetalBandReading | null;

/**
 * The band reader's template of `glyph` (a light or dark tile) under blur `blur` (an index into
 * `PETAL_KANA.blurs`) at point `(i, j)` of the 12 x 12 grid, mean-centred and scaled to unit length;
 * conformance tests compare it with the fixture's probes.
 */
export function kanaTemplateProbe(blur: number, glyph: number, light: boolean, i: number, j: number): number;
/** The coarse template of `glyph` (a light or dark tile) at point `(i, j)` of the 6 x 6 grid. */
export function kanaCoarseTemplateProbe(glyph: number, light: boolean, i: number, j: number): number;

// ------------------------------------------------------------------ images

/** A row-major 8-bit luma plane. */
export interface PetalLumaLike {
  readonly width: number;
  readonly height: number;
  readonly data: Uint8Array | Uint8ClampedArray;
}

/** A single-channel 8-bit image (camera luma plane). Pixel `(i, j)` covers `[i, i+1) x [j, j+1)`. */
export class PetalLuma implements PetalLumaLike {
  /** Wraps `data` without copying (black when omitted). @throws {RangeError} on a length mismatch */
  constructor(width: number, height: number, data?: Uint8Array | Uint8ClampedArray);
  readonly width: number;
  readonly height: number;
  readonly data: Uint8Array;
  /** Copies a strided plane (e.g. the Y plane of a camera frame); `null` when too short or `stride < width`. */
  static fromStrided(
    width: number,
    height: number,
    stride: number,
    plane: Uint8Array | Uint8ClampedArray,
  ): PetalLuma | null;
  /** Rec. 601 luma `(299 R + 587 G + 114 B + 500) / 1000` of an RGBA plane; `null` when too short. */
  static fromRgba(width: number, height: number, rgba: Uint8Array | Uint8ClampedArray, stride?: number): PetalLuma | null;
  /** Converts a canvas `ImageData` (or any `{width, height, data}` RGBA image). */
  static fromImageData(imageData: {
    readonly width: number;
    readonly height: number;
    readonly data: Uint8Array | Uint8ClampedArray;
  }): PetalLuma | null;
  /** Reads pixel `(x, y)`. */
  at(x: number, y: number): number;
  /** Bilinear sample at pixel-edge coordinates, clamped to the border. */
  sample(x: number, y: number): number;
}

/** A 3x3 projective transform stored row-major. */
export class PetalHomography {
  constructor(values: ArrayLike<number>);
  static identity(): PetalHomography;
  /** Hartley-normalised DLT fit taking `src[i]` to `dst[i]`; `null` when degenerate or fewer than four pairs. */
  static fromPoints(
    src: readonly (readonly [number, number])[],
    dst: readonly (readonly [number, number])[],
  ): PetalHomography | null;
  /** The nine row-major coefficients (a copy). */
  readonly values: number[];
  apply(x: number, y: number): [number, number];
  inverse(): PetalHomography | null;
  /** `this * other` (apply `other` first). */
  compose(other: PetalHomography): PetalHomography;
}

/**
 * A camera pose: a homography from the canvas onto undistorted image pixels, followed by radial
 * lens distortion about the image centre `c`. A pixel an undistorted camera sees at `u` is seen
 * at `c + (u - c) * (1 + k * |u - c|^2 / s^2)` with `s = 0.8 * max(width, height)`; `k = 0` is a
 * plain homography and negative `k` is barrel distortion.
 */
export class PetalPose {
  constructor(homography: PetalHomography, distortion?: number, center?: readonly [number, number], scale2?: number);
  /** A pose without lens distortion. */
  static plain(homography: PetalHomography): PetalPose;
  /**
   * The pose with distortion `distortion` taking the canvas points `src` to the image points `dst`
   * of a `width x height` image (the image points are undistorted, then the homography is fitted);
   * `null` when degenerate or fewer than four pairs.
   */
  static fit(
    src: readonly (readonly [number, number])[],
    dst: readonly (readonly [number, number])[],
    distortion: number,
    width: number,
    height: number,
  ): PetalPose | null;
  /** Canvas to undistorted pixels (a copy). */
  readonly homography: PetalHomography;
  /** Radial distortion coefficient `k`. */
  readonly distortion: number;
  /** Distortion centre in pixels. */
  readonly center: [number, number];
  /** Squared distortion scale `s^2` in pixels^2. */
  readonly scale2: number;
  /** Maps a canvas point to image pixels. */
  apply(x: number, y: number): [number, number];
}

// ------------------------------------------------------------------ finder search

/** A detected finder in pixel-edge coordinates; `size` is the apparent diameter. */
export interface PetalFinder {
  readonly x: number;
  readonly y: number;
  readonly size: number;
}

/** A 4-connected component with its bounding box and moments. */
export interface PetalComponent {
  readonly area: number;
  readonly minX: number;
  readonly maxX: number;
  readonly minY: number;
  readonly maxY: number;
  readonly sumX: number;
  readonly sumY: number;
  readonly sumXX: number;
  readonly sumYY: number;
  readonly sumXY: number;
}

/** Four finders, clockwise from the top-left. */
export type PetalFinderQuad = [PetalFinder, PetalFinder, PetalFinder, PetalFinder];

/** One plausible set of corner finders for a frame. */
export interface PetalFinderSet {
  /** The four corners, clockwise from the one nearest the top-left of the image. */
  readonly corners: PetalFinderQuad;
  /** Index into `corners` of a corner that was not seen but inferred from the other three. */
  readonly inferred: number | null;
}

/** Marks pixels clearly brighter than their neighbourhood (local mean via an integral image). */
export function adaptiveBinarize(image: PetalLumaLike, sensitivity: number): Uint8Array;
/** Labels 4-connected components, ordered by first appearance in raster order. */
export function labelComponents(mask: ArrayLike<number | boolean>, width: number, height: number): PetalComponent[];
/** Detects blossom finders: large, round, isolated blobs. */
export function blossoms(components: readonly PetalComponent[]): PetalFinder[];
/** Chooses the four finders that look like the corners of one code, combining candidates largest first. */
export function selectQuad(finders: readonly PetalFinder[]): PetalFinderQuad | null;
/**
 * Chooses three finders that look like three corners of one code (similar sizes, two similar legs
 * at a roughly right angle) and completes the fourth corner as a parallelogram: the clockwise
 * quad and the index of the inferred corner in it.
 */
export function selectTriple(
  finders: readonly PetalFinder[],
): { readonly corners: PetalFinderQuad; readonly inferred: number } | null;
/** Sharpens a finder centre with an intensity-weighted centroid. */
export function refineCenter(image: PetalLumaLike, finder: PetalFinder): PetalFinder;
/**
 * Re-finds a finder near where it is expected (from the previous frame's pose); `null` when
 * nothing bright is there or it moved more than 0.75 diameters.
 */
export function followFinder(image: PetalLumaLike, expected: PetalFinder): PetalFinder | null;
/**
 * All candidate finder sets of a frame in the order the decoder tries them: for each threshold
 * the quad of the large blobs, then three large blobs forming a corner completed by a smaller
 * blob, the quad of smaller blobs, and last the corner with its fourth finder inferred.
 */
export function locateCandidates(image: PetalLumaLike): PetalFinderSet[];
/** Locates four seen corner finders (the first candidate without an inferred corner). */
export function locate(image: PetalLumaLike): PetalFinderQuad | null;

// ------------------------------------------------------------------ decoder

/** A lane that passed its Reed-Solomon check. */
export interface PetalLaneResult {
  /** Lane data bytes (header and atoms, or the beacon). */
  readonly data: Uint8Array;
  /**
   * Byte positions the Reed-Solomon decoder rewrote: the erased bytes plus any
   * errors it found among the others (a measure of how close the lane was to
   * failing).
   */
  readonly corrected: number;
  /** Bytes passed to the decoder as erasures. */
  readonly erasures: number;
}

/** Everything read from one camera frame. */
export class PetalDecodedFrame {
  private constructor();
  /** Canvas-to-pixel pose that was used, lens distortion included. */
  readonly pose: PetalPose;
  /** Orientation hypothesis: quarter turns of the finder assignment. */
  readonly rotation: number;
  /** Whether the image was mirrored. */
  readonly mirrored: boolean;
  readonly d: PetalLaneResult | null;
  /** The field lanes, sector 0 first. */
  readonly fields: PetalSectors<PetalLaneResult | null>;
  /**
   * The corner finder that was hidden (by a finger, a glare or the edge of the frame) and
   * inferred from the other three, as its canonical index: 0 top-left, 1 top-right,
   * 2 bottom-right, 3 bottom-left of the upright code; `null` when all four were seen.
   */
  readonly inferredCorner: number | null;
  /** Number of lanes that decoded. */
  lanesOk(): number;
  /** The result of `lane`, when it decoded. */
  lane(lane: PetalLane): PetalLaneResult | null;
  /** The lanes that decoded, as letters: `D` and the sector digits `0`-`3` (for example `"D0123"`). */
  letters(): string;
  /** What lane `D` carried, when it decoded. */
  dLane(): PetalDLane | null;
  /** The beacon, when lane `D` decoded on a beacon frame. */
  beacon(): PetalBeacon | null;
  /** Atom packets from every lane that decoded (the field lanes, then `D`). */
  atomPackets(): PetalAtomPacket[];
  /** Offers everything this frame carries to `assembler`. */
  feed(assembler: PetalStreamAssembler): void;
}

/** What one capture shows of a frame's petal field: the normalised level of every field cell. */
export class PetalFieldEvidence {
  /** @param levels the normalised level of each of the 3,376 field cells */
  constructor(levels: ArrayLike<number>);
  readonly levels: Float64Array;
}

/**
 * Decodes one camera frame.
 *
 * Four corner blossoms are looked for, or three that form a corner with the fourth inferred
 * (reported in `inferredCorner`); orientations are ranked by the ring gates. Every petal is
 * thresholded per codeword and colour class; while field lanes stay unread the pose is searched
 * for lens distortion, and lanes the finder levels cannot read (shadows, glare, banding,
 * veiling light) are read again from local levels. An orientation is accepted when lane `D` or
 * a field lane decodes.
 *
 * @throws {PetalError} `unsupported_image` (a side below 48 pixels, more than
 *   `maxPixels` pixels or inconsistent data), `no_finders` or `no_orientation`
 */
export function decodePetalFrame(image: PetalLumaLike, options?: PetalDecodeOptions): PetalDecodedFrame;

/**
 * Reads all lanes with a known canvas-to-pixel pose (no finder search, no distortion search);
 * `null` when the image is unusable or the reference levels are too weak.
 */
export function decodePetalFrameAt(
  image: PetalLumaLike,
  pose: PetalPose,
  options?: PetalDecodeOptions,
): PetalDecodedFrame | null;

/**
 * Follows a code from the previous frame that decoded without searching the whole image for
 * finders: each corner is re-found near where the previous pose puts it (a previously hidden
 * blossom counts again once it is back where the others predict it), one lost corner is
 * inferred, the orientation is kept and the distortion search starts from the previous
 * distortion. `null` for a broken previous pose, when two corners are lost or when no lane
 * decodes (run {@link decodePetalFrame} then). {@link PetalScanSession} does this by itself.
 */
export function trackPetalFrame(
  image: PetalLumaLike,
  previous: PetalDecodedFrame,
  options?: PetalDecodeOptions,
): PetalDecodedFrame | null;

/** The cells the decoder believes it saw, for diagnostics. */
export function observedCells(
  image: PetalLumaLike,
  frame: { readonly pose: PetalPose; readonly inferredCorner?: number | null },
  options?: PetalDecodeOptions,
): PetalFrameCells | null;

/**
 * The field evidence of `image` under the pose `frame` was decoded with; `null` when the image is
 * unusable, the frame names a corner that does not exist or the finder levels are too weak.
 */
export function fieldEvidence(
  image: PetalLumaLike,
  frame: { readonly pose: PetalPose; readonly inferredCorner?: number | null },
  options?: PetalDecodeOptions,
): PetalFieldEvidence | null;

/**
 * The field lanes read from the cell-by-cell mean of several captures of one shown frame (local
 * read included); sectors flagged in `skip` are not read. The caller checks that every evidence
 * comes from the same shown frame: a lane read here names its frame in its own header.
 */
export function mergeFields(
  evidence: readonly PetalFieldEvidence[],
  skip?: PetalSectors<boolean>,
): PetalSectors<PetalLaneResult | null>;

// ------------------------------------------------------------------ rendering

/** Software renderer options. */
export interface PetalRenderOptions {
  /** Output width in pixels (default 1024); the card is {@link petalCardHeight} pixels tall. */
  readonly size?: number;
  /** Samples per pixel side, 1-4 (default 3). */
  readonly supersample?: number;
  readonly palette?: Partial<PetalPalette>;
}

/** An RGBA image, ready for `new ImageData(data, width, height)`. */
export interface PetalRgbaImage {
  readonly width: number;
  readonly height: number;
  readonly data: Uint8ClampedArray<ArrayBuffer>;
}

/** Height in pixels of a card rendered `size` pixels wide: `size * 1668 / 1024`, rounded. */
export function petalCardHeight(size: number): number;

/**
 * Renders one frame in software, exactly like the reference renderer: the whole card, the square
 * starting 322 canvas units below its top edge.
 */
export function renderPetalFrame(cells: PetalFrameCells, options?: PetalRenderOptions): PetalRgbaImage;

/** A circle in canvas design units. */
export interface PetalDrawCircle {
  readonly x: number;
  readonly y: number;
  readonly r: number;
}

/** A corner finder: fill `core` and `petals` with the light colour, then `notches` with the background. */
export interface PetalDrawFinder {
  readonly x: number;
  readonly y: number;
  readonly core: PetalDrawCircle;
  readonly petals: readonly PetalDrawCircle[];
  readonly notches: readonly PetalDrawCircle[];
}

/** The faint `天`: its lattice squares, filled with the backdrop colour and clipped to the disc. */
export interface PetalDrawBackdrop extends PetalDrawCircle {
  readonly cells: readonly { readonly x: number; readonly y: number; readonly size: number }[];
}

/**
 * The outline every petal is drawn with, in canvas units: an ellipse minus a notch at its tip.
 * Draw the notch (it tells a petal from one turned by half a turn) and draw antialiased.
 */
export interface PetalDrawPetalShape {
  /** Half-length along the petal's axis (6). */
  readonly halfLength: number;
  /** Half-width across it (3.73). */
  readonly halfWidth: number;
  /** Radius of the notch cut into the tip (1.12). */
  readonly notchRadius: number;
  /** Distance from the petal centre to the notch centre, along the axis (6.24). */
  readonly notchDistance: number;
}

/** One lit petal. */
export interface PetalDrawPetal {
  /** Field cell index. */
  readonly index: number;
  /** Centre in canvas units. */
  readonly x: number;
  readonly y: number;
  /** Axis: the outward direction plus the petal's turn (radians, clockwise on screen from 3 o'clock). */
  readonly angle: number;
  /** Whether the petal lies inside the `天` (light colour) or outside it (pink). */
  readonly light: boolean;
  readonly fill: PetalRgb;
}

/** The shape every katakana tile is drawn with, in canvas units, and the strokes of every glyph. */
export interface PetalDrawTileShape {
  /** Side of the tile square (25). */
  readonly side: number;
  /** Corner radius of the tile square (3). */
  readonly corner: number;
  /** Width of the glyph strokes (`6.5 * 23 / 32`), drawn with round caps and joins. */
  readonly strokeWidth: number;
  /**
   * `glyphs[glyph][stroke][point]`: the strokes of every glyph as polylines of `[u, v]` points in
   * tile-local units from the tile's top-left corner; every stroke lies inside its rounded tile.
   */
  readonly glyphs: readonly (readonly (readonly (readonly [number, number])[])[])[];
}

/** One katakana tile. */
export interface PetalDrawTile {
  /** Tile index ({@link kanaTiles} order). */
  readonly index: number;
  /** Top-left corner of the tile square in square coordinates (negative `y` above the square). */
  readonly x: number;
  readonly y: number;
  /** The glyph it shows (0-15). */
  readonly glyph: number;
  readonly light: boolean;
  /** The tile's fill: the light colour, or `null` for a dark tile (not filled). */
  readonly fill: PetalRgb | null;
  /** The colour of its glyph's strokes: ink on a light tile, pink on a dark one. */
  readonly ink: PetalRgb;
}

/**
 * The frame as vector primitives in square design units (0..1024, `y` down); the card extends
 * `squareTop` units above and below the square, so a `canvas` x `cardHeight` area translated down by
 * `squareTop` holds it all.
 */
export interface PetalDrawList {
  readonly canvas: number;
  /** Height of the card (1668). */
  readonly cardHeight: number;
  /** Distance from the card's top edge to the square's (322). */
  readonly squareTop: number;
  readonly palette: PetalPalette;
  readonly finders: readonly PetalDrawFinder[];
  /** Drawn first, under the petals. */
  readonly backdrop: PetalDrawBackdrop;
  readonly petal: PetalDrawPetalShape;
  readonly petals: readonly PetalDrawPetal[];
  /** Lit ring dots (radius 9), gate dots included. */
  readonly dots: readonly PetalDrawCircle[];
  readonly tile: PetalDrawTileShape;
  /** Every katakana tile of both bands in tile order: fill the light ones, then stroke every glyph. */
  readonly kanaTiles: readonly PetalDrawTile[];
}

/** Builds the platform-neutral draw list of a frame for vector backends. */
export function petalDrawList(
  cells: PetalFrameCells,
  options?: { readonly palette?: Partial<PetalPalette> },
): PetalDrawList;

// ------------------------------------------------------------------ scan session

/** Diagnostic counters of a scan session. */
export interface PetalScanStats {
  /** Camera frames offered. */
  readonly frames: number;
  /** Frames in which a code was located, whether or not a lane could be read. */
  readonly located: number;
  /** Frames in which at least one lane decoded. */
  readonly readable: number;
  /** Lane `D` successes. */
  readonly laneD: number;
  /** Field lane successes (up to four per frame). */
  readonly fieldLanes: number;
  /** Frames read by tracking the previous pose instead of a full search. */
  readonly tracked: number;
  /** Frames read with one corner finder hidden and inferred. */
  readonly inferred: number;
  /** Merges of two or more captures of one shown frame. */
  readonly merges: number;
  /** Field lanes recovered by merging captures. */
  readonly mergedLanes: number;
  /** Turn lanes tried (each at most once per shown frame). */
  readonly turnTries: number;
  /** Turn lanes read. */
  readonly turnLanes: number;
  /** Bands tried (each at most once per shown frame). */
  readonly kanaTries: number;
  /** Kana lanes read (up to four per shown frame). */
  readonly kanaLanes: number;
  /** Captures whose bands read a lane that also tried the turn lanes, having time to spare. */
  readonly spareTurns: number;
}

/**
 * A part of a capture's work, as a scan session times it to tell whether a capture has time to
 * spare for the turn lanes: finding the code by a full search (`search`) or following it from the
 * previous capture's pose (`track`), each with taking in the lanes read, merging the held captures
 * of a shown frame (`merge`), reading the kana lanes of `count` bands (`bands`) and reading the
 * turn lanes (`turns`).
 */
export type PetalWork =
  | { readonly kind: "search" }
  | { readonly kind: "track" }
  | { readonly kind: "merge" }
  | { readonly kind: "bands"; readonly count: number }
  | { readonly kind: "turns" };

/** The result of offering one camera frame. */
export interface PetalScanOutcome {
  /**
   * Why the frame produced nothing, when it did not. `no_orientation` means a
   * code was located but no lane could be read (too far, too blurry).
   */
  readonly error: PetalDecodeErrorCode | null;
  /** Lanes this capture decoded alone, as letters: `D` and the sector digits `0`-`3`. */
  readonly lanes: string;
  /** Field lanes newly recovered by merging this capture with earlier ones of its frame, as sector digits. */
  readonly merged: string;
  /** Turn lanes read from this capture, as the sector digits of the field lanes they ride on. */
  readonly turned: string;
  /** Kana lanes read from this capture, their names concatenated (for example `"P0K0P1K1"`). */
  readonly kana: string;
  readonly progress: PetalProgress;
  /** The finished payload, delivered exactly once. */
  readonly completed: PetalCompleted | null;
}

/** Decodes camera frames and reassembles the stream they carry. */
export class PetalScanSession {
  /**
   * `workCost` says what a part of a capture costs in milliseconds, given the milliseconds it took;
   * by default the measured time (`performance.now()`). A model's costs in its place make the
   * session's choices repeatable, as tests do.
   */
  constructor(limits?: PetalScanLimits, workCost?: (work: PetalWork, measuredMs: number) => number);
  readonly limits: PetalResolvedScanLimits;
  stats(): PetalScanStats;
  progress(): PetalProgress;
  /** Drops all partial state, including the pose used for tracking and the turn lanes and bands tried. */
  reset(): void;
  /**
   * Offers one camera luma plane captured at monotonic time `nowMs`. While the last decoded pose
   * is at most {@link PETAL_TRACK_WINDOW_MS} old, the code is first tracked from it. The katakana
   * bands are read first, each once per shown frame; the turn lanes on top of the decoded field
   * lanes are tried, each once per shown frame, when the bands read no kana lane or when the
   * capture's work and the last turn try fit in {@link PETAL_CAPTURE_BUDGET_MS} (otherwise a
   * capture whose bands read leaves its frame's turn lanes to the next capture). After tries that
   * read nothing the session passes over up to eight captures before it tries again, with separate
   * counts for the bands and the turn lanes.
   */
  push(image: PetalLumaLike, nowMs: number): PetalScanOutcome;
}

// ------------------------------------------------------------------ browser glue

/** A Canvas 2D context the player can draw on. */
export type PetalCanvasContext = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

/**
 * Draws a draw list onto a Canvas 2D context: the whole card, its 1024-unit width scaled to `size`
 * pixels (default: the widest card the canvas holds; the card is `size * 1668 / 1024` pixels tall)
 * at `(x, y)`. Petals are drawn turned, notched and antialiased (Canvas 2D path fills), tiles as
 * rounded squares with their glyphs stroked (round caps and joins).
 */
export function drawPetalFrame(
  context: PetalCanvasContext,
  drawList: PetalDrawList,
  options?: { readonly x?: number; readonly y?: number; readonly size?: number },
): void;

/** Frame shown `elapsedMs` after `startFrame` at `fps` (wraps at 65536). */
export function petalPlayerFrame(startFrame: number, elapsedMs: number, fps: number): number;

/** Analysis size of a camera frame: the long side scaled down to at most `maxSide` (default 1280). */
export function petalScanSize(width: number, height: number, maxSide?: number): { width: number; height: number };

/** Options of {@link PetalStreamPlayer}. */
export interface PetalStreamPlayerOptions {
  /** Usually a {@link PetalStreamEncoder}. */
  readonly encoder: { cells(frame: number): PetalFrameCells };
  readonly context: PetalCanvasContext;
  /**
   * Frames per second (default 12). Keep it at or below half the camera frame rate.
   */
  readonly fps?: number;
  /** Drawn width of the card in pixels (default: the widest card the canvas holds). */
  readonly size?: number;
  readonly x?: number;
  readonly y?: number;
  readonly palette?: Partial<PetalPalette>;
  readonly startFrame?: number;
  readonly onFrame?: (frame: number) => void;
  readonly requestAnimationFrame?: (callback: (time: number) => void) => unknown;
  readonly cancelAnimationFrame?: (handle: never) => void;
}

/** Plays a stream on a Canvas 2D context, advancing frames with `requestAnimationFrame`. */
export class PetalStreamPlayer {
  constructor(options: PetalStreamPlayerOptions);
  /** The frame number on screen. */
  readonly frame: number;
  readonly playing: boolean;
  /** Draws `frame` immediately. */
  drawFrame(frame: number): void;
  /** Starts (or resumes) the animation from the frame on screen. */
  start(): void;
  /** Stops the animation; the last frame stays on screen. */
  stop(): void;
}

/** The 2D context surface the scanner needs from its offscreen canvas. */
export interface PetalScanContext {
  drawImage(image: never, dx: number, dy: number, dw: number, dh: number): void;
  getImageData(sx: number, sy: number, sw: number, sh: number): {
    readonly width: number;
    readonly height: number;
    readonly data: Uint8ClampedArray;
  };
}

/** An offscreen canvas (`OffscreenCanvas` or `HTMLCanvasElement`). */
export interface PetalScanCanvas {
  width: number;
  height: number;
  getContext(contextId: "2d", options?: { willReadFrequently?: boolean }): PetalScanContext | null;
}

/** Options of {@link PetalCameraScanner}; pass `video`, `stream` or both. */
export interface PetalCameraScannerOptions {
  readonly video?: HTMLVideoElement;
  readonly stream?: MediaStream;
  /** Session to feed (a new one with `limits` by default). */
  readonly session?: PetalScanSession;
  readonly limits?: PetalScanLimits;
  /** Longest side of the analysed image in pixels (default 1280). */
  readonly maxSide?: number;
  /** Stop once a payload completed (default `true`). */
  readonly stopOnComplete?: boolean;
  /**
   * Called after every analysed frame with its outcome and the session's counters
   * (`stats.inferred` grows while a corner blossom is hidden).
   */
  readonly onProgress?: (outcome: PetalScanOutcome, stats: PetalScanStats) => void;
  readonly onComplete?: (completed: PetalCompleted) => void;
  readonly onError?: (error: unknown) => void;
  readonly createCanvas?: (width: number, height: number) => PetalScanCanvas;
  readonly requestAnimationFrame?: (callback: (time: number) => void) => unknown;
  readonly cancelAnimationFrame?: (handle: never) => void;
  /** Monotonic clock in milliseconds (default `performance.now`). */
  readonly now?: () => number;
}

/**
 * Reads a stream from a camera: frames come from `requestVideoFrameCallback`
 * (or `requestAnimationFrame`), are downsized on an offscreen canvas,
 * converted to Rec. 601 luma and fed to a {@link PetalScanSession} (which tracks the code from
 * frame to frame and reads it with one corner blossom hidden).
 *
 * The scanner does not change the camera's settings; set the stream up like
 * this (evidence: `specs/petal_stream.md` section 8):
 *
 * - Resolution: ask `getUserMedia` for about 1280x720
 *   (`video: {facingMode: "environment", width: {ideal: 1280}, height: {ideal: 720}}`).
 *   The default `maxSide` of 1280 already matches it. Only drop to 640 when
 *   the device cannot sustain about 5 decoded frames per second, because at
 *   480p a little defocus already costs the petal field (lane `D` carries on).
 * - Exposure: automatic exposure over-exposes a mostly black screen. Where the
 *   browser supports it, apply an exposure compensation of about -1 EV to the
 *   video track: feature-detect `track.getCapabilities().exposureCompensation`,
 *   then call `track.applyConstraints({advanced: [{exposureCompensation: -1}]})`
 *   (Chrome on Android exposes it). The decoder tolerates the over-exposure
 *   that is left, but it should not be asked to.
 */
export class PetalCameraScanner {
  constructor(options: PetalCameraScannerOptions);
  readonly session: PetalScanSession;
  readonly video: HTMLVideoElement | null;
  readonly scanning: boolean;
  /** Attaches the stream (when given), starts playback and begins scanning. */
  start(): Promise<void>;
  /** Stops scanning; a video element created by the scanner is released. */
  stop(): void;
  /** Offers the current video frame to the session; `null` while the video has no frame. */
  scanFrame(): PetalScanOutcome | null;
}
