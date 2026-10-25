/**
 * Petal Stream: the Sakura-storm optical transport (a streaming visual code).
 *
 * A sender shows a sequence of square frames; a camera reads them and a
 * payload is reassembled. Every frame carries five independent lanes, each
 * one Reed-Solomon codeword over GF(256): the ring dots (lane `D`) and the
 * four sectors of a field of 3,376 petals, each lit or dark (lanes `F0`-`F3`).
 * Lanes carry fountain-coded 16-byte atoms; lane `D` carries the stream
 * beacon on every fourth frame.
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

/** Normative frame geometry in design units (canvas 1024, origin top-left, `y` down). */
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
  /** Radius of the disc a petal stands in for (decoders sample within it). */
  readonly petalRadius: 4.6;
  /** Half-length of a drawn petal along its outward direction. */
  readonly petalHalfLength: 5.2;
  /** Half-width of a drawn petal. */
  readonly petalHalfWidth: 4.3;
  /** Radius of the cosmetic notch cut into each petal tip. */
  readonly petalNotchRadius: 1.3;
  /** Distance from a petal centre to its notch centre, outward. */
  readonly petalNotchDistance: 5.4;
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

/** Lane sizes in decode order: D 30/11/19, F0-F3 105/22/83 (word/parity/data bytes). */
export const PETAL_LANES: { readonly [lane in PetalLane]: PetalLaneSize };

/** Stream-level wire constants and receiver defaults. */
export const PETAL_STREAM: {
  readonly atomLen: 16;
  readonly laneHeaderLen: 3;
  readonly atomsPerFrame: 21;
  /** Cells at the end of every sector that carry no bit (they stay dark). */
  readonly spareSectorCells: 4;
  /** Beacon version byte: format version 2, layout profile 0. */
  readonly formatVersion: 32;
  readonly beaconInterval: 4;
  readonly maxPayloadLen: 16777215;
  readonly defaultMaxPayloadLen: 65536;
  readonly defaultMaxPendingAtoms: 512;
};

/** Colours of the picture. */
export interface PetalPalette {
  readonly background: PetalRgb;
  /** Finders and the petals inside the `天`. */
  readonly light: PetalRgb;
  /** Sakura pink of the ring dots and of the petals outside the `天`. */
  readonly pink: PetalRgb;
  /** The faint `天` drawn under the petals. */
  readonly backdrop: PetalRgb;
}

/** Default palette: background (0,0,0), light (250,235,244), pink (245,175,208), backdrop (25,24,24). */
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
 * the ellipse of the petal minus its notch.
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

/** Every cell of one frame: what a renderer draws and a decoder samples. */
export class PetalFrameCells {
  constructor(dots?: ArrayLike<number | boolean>, petals?: ArrayLike<number | boolean>);
  /** Lit state (`0`/`1`) of each of the 296 ring slots, gate dots included. */
  readonly dots: Uint8Array;
  /** Lit state (`0`/`1`) of each of the 3,376 field cells (spare cells are dark). */
  readonly petals: Uint8Array;
  /** Builds the cells from the transmitted codewords of lane `D` and the four field lanes. */
  static fromWords(d: PetalBytes, fields: PetalSectors<PetalBytes>): PetalFrameCells;
  /** Packs the data dots into a lane `D` codeword. */
  dWord(): Uint8Array;
  /** Packs the petals of sector `sector` into its field codeword. */
  fieldWord(sector: number): Uint8Array;
  /** Whether `other` holds exactly the same cells. */
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

/** Data bytes or codewords of the five lanes of one frame. */
export interface PetalLaneWords {
  readonly d: Uint8Array;
  /** The field lanes, sector 0 first. */
  readonly fields: PetalSectors<Uint8Array>;
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
  /** Frames needed to send every source atom once. */
  systematicFrames(): number;
  /** Data bytes of every lane of `frame` (reduced modulo 65536). */
  laneData(frame: number): PetalLaneWords;
  /** Transmitted codewords of `frame`. */
  words(frame: number): PetalLaneWords;
  /** Every cell of `frame`, ready to render. */
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
  /** Output side in pixels (default 1024). */
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

/** Renders one frame in software, exactly like the reference renderer. */
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

/** The outline every petal is drawn with, in canvas units. */
export interface PetalDrawPetalShape {
  /** Half-length along the petal's outward direction. */
  readonly halfLength: number;
  /** Half-width across it. */
  readonly halfWidth: number;
  /** Radius of the cosmetic notch cut into the tip (renderers may omit it). */
  readonly notchRadius: number;
  /** Distance from the petal centre to the notch centre, outward. */
  readonly notchDistance: number;
}

/** One lit petal. */
export interface PetalDrawPetal {
  /** Field cell index. */
  readonly index: number;
  /** Centre in canvas units. */
  readonly x: number;
  readonly y: number;
  /** Outward direction (radians, clockwise on screen from 3 o'clock). */
  readonly angle: number;
  /** Whether the petal lies inside the `天` (light colour) or outside it (pink). */
  readonly light: boolean;
  readonly fill: PetalRgb;
}

/** The frame as vector primitives in canvas design units (0..1024). */
export interface PetalDrawList {
  readonly canvas: number;
  readonly palette: PetalPalette;
  readonly finders: readonly PetalDrawFinder[];
  /** Drawn first, under the petals. */
  readonly backdrop: PetalDrawBackdrop;
  readonly petal: PetalDrawPetalShape;
  readonly petals: readonly PetalDrawPetal[];
  /** Lit ring dots (radius 9), gate dots included. */
  readonly dots: readonly PetalDrawCircle[];
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
}

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
  readonly progress: PetalProgress;
  /** The finished payload, delivered exactly once. */
  readonly completed: PetalCompleted | null;
}

/** Decodes camera frames and reassembles the stream they carry. */
export class PetalScanSession {
  constructor(limits?: PetalScanLimits);
  readonly limits: PetalResolvedScanLimits;
  stats(): PetalScanStats;
  progress(): PetalProgress;
  /** Drops all partial state, including the pose used for tracking. */
  reset(): void;
  /**
   * Offers one camera luma plane captured at monotonic time `nowMs`. While the last decoded pose
   * is at most {@link PETAL_TRACK_WINDOW_MS} old, the code is first tracked from it.
   */
  push(image: PetalLumaLike, nowMs: number): PetalScanOutcome;
}

// ------------------------------------------------------------------ browser glue

/** A Canvas 2D context the player can draw on. */
export type PetalCanvasContext = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

/**
 * Draws a draw list onto a Canvas 2D context, scaling the 1024-unit design
 * canvas to `size` pixels (default: the smaller canvas side) at `(x, y)`.
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
  /** Drawn side in pixels (default: the smaller canvas side). */
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
