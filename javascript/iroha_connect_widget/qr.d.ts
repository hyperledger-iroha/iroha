// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/** QR error-correction level: roughly 7 %, 15 %, 25 % or 30 % recovery. */
export type QrErrorCorrectionLevel = "L" | "M" | "Q" | "H";

/** QR mask pattern index. */
export type QrMask = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7;

export interface QrEncodeOptions {
  /** Error-correction level; defaults to `"M"`. */
  level?: QrErrorCorrectionLevel;
  /** Smallest version to consider (1–40); defaults to 1. */
  minVersion?: number;
  /** Largest version to consider (1–40); defaults to 40. */
  maxVersion?: number;
  /** Fixed mask, or `"auto"` (default) for the lowest ISO/IEC 18004 penalty. */
  mask?: "auto" | QrMask;
}

/** An encoded QR symbol; `modules[y * size + x]` is 1 for a dark module. */
export interface QrSymbol {
  readonly version: number;
  readonly size: number;
  readonly level: QrErrorCorrectionLevel;
  readonly mask: QrMask;
  readonly modules: Uint8Array;
  /** True when the module at column `x`, row `y` is dark. */
  isDark(x: number, y: number): boolean;
}

export interface QrSvgOptions {
  /** Quiet zone in modules; defaults to 4. */
  margin?: number;
  /** Module color (hex, `rgb()` or a named color); defaults to `#15161a`. */
  dark?: string;
  /** Background color; defaults to `#ffffff`. */
  light?: string;
  /** Accessible name; the SVG is unlabeled when omitted. */
  title?: string;
}

export type QrEncodeErrorCode = "invalid_option" | "data_too_long";

/** Error raised when text cannot be encoded with the requested options. */
export class QrEncodeError extends Error {
  constructor(code: QrEncodeErrorCode, message: string);
  readonly code: QrEncodeErrorCode;
}

export const QR_MIN_VERSION: 1;
export const QR_MAX_VERSION: 40;
export const QR_QUIET_ZONE: 4;

/** Encode UTF-8 text as a QR symbol in byte mode. */
export function encodeQr(text: string, options?: QrEncodeOptions): QrSymbol;

/** SVG path data covering every dark module, offset by the quiet zone. */
export function qrSvgPath(qr: QrSymbol, margin?: number): string;

/** Standalone SVG markup for a QR symbol, including the quiet zone. */
export function qrToSvg(qr: QrSymbol, options?: QrSvgOptions): string;

/** Modules available for data and error correction in a version. */
export function rawDataModuleCount(version: number): number;

/** 8-bit data codewords a version holds at an error-correction level. */
export function dataCodewordCapacity(version: number, level?: QrErrorCorrectionLevel): number;

/** Center coordinates of the alignment patterns of a version. */
export function alignmentPatternPositions(version: number): number[];
