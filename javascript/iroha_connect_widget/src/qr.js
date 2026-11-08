// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * QR Code Model 2 encoder (ISO/IEC 18004) for IrohaConnect pairing URIs.
 *
 * The widget renders the Torii-issued wallet URI as an ordinary QR symbol so
 * every IrohaConnect wallet can read it with a standard scanner. Only byte
 * mode (UTF-8) is implemented; it covers every URI the Connect relay issues.
 * The encoder is deterministic: the same text, level and mask always produce
 * the same modules on every platform.
 */

const ECC_LEVELS = Object.freeze({
  L: Object.freeze({ name: "L", ordinal: 0, formatBits: 1 }),
  M: Object.freeze({ name: "M", ordinal: 1, formatBits: 0 }),
  Q: Object.freeze({ name: "Q", ordinal: 2, formatBits: 3 }),
  H: Object.freeze({ name: "H", ordinal: 3, formatBits: 2 }),
});

// ISO/IEC 18004 Table 9, indexed [level][version]; index 0 is unused.
const ECC_CODEWORDS_PER_BLOCK = Object.freeze([
  [-1, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  [-1, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28],
  [-1, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  [-1, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
]);

const ERROR_CORRECTION_BLOCKS = Object.freeze([
  [-1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25],
  [-1, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49],
  [-1, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68],
  [-1, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81],
]);

const PENALTY_N1 = 3;
const PENALTY_N2 = 3;
const PENALTY_N3 = 40;
const PENALTY_N4 = 10;
const BYTE_MODE_INDICATOR = 0b0100;
const PAD_BYTES = Object.freeze([0xec, 0x11]);

/** Smallest QR version. */
export const QR_MIN_VERSION = 1;
/** Largest QR version. */
export const QR_MAX_VERSION = 40;
/** Light modules around the symbol required by the specification. */
export const QR_QUIET_ZONE = 4;

/** Error raised when text cannot be encoded with the requested options. */
export class QrEncodeError extends Error {
  /**
   * @param {"invalid_option" | "data_too_long"} code
   * @param {string} message
   */
  constructor(code, message) {
    super(message);
    this.name = "QrEncodeError";
    this.code = code;
  }
}

function getBit(value, index) {
  return ((value >>> index) & 1) !== 0;
}

function requireLevel(level) {
  const resolved = ECC_LEVELS[level];
  if (!resolved) {
    throw new QrEncodeError("invalid_option", `unknown QR error-correction level ${String(level)}`);
  }
  return resolved;
}

function requireVersion(value, name) {
  if (!Number.isInteger(value) || value < QR_MIN_VERSION || value > QR_MAX_VERSION) {
    throw new QrEncodeError("invalid_option", `${name} must be an integer from 1 to 40`);
  }
  return value;
}

function requireMask(value) {
  if (value === "auto" || value === undefined) {
    return "auto";
  }
  if (!Number.isInteger(value) || value < 0 || value > 7) {
    throw new QrEncodeError("invalid_option", "mask must be \"auto\" or an integer from 0 to 7");
  }
  return value;
}

/** Number of modules available for data and error correction in a version. */
export function rawDataModuleCount(version) {
  requireVersion(version, "version");
  let result = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const alignmentCount = Math.floor(version / 7) + 2;
    result -= (25 * alignmentCount - 10) * alignmentCount - 55;
    if (version >= 7) {
      result -= 36;
    }
  }
  return result;
}

/** Number of 8-bit data codewords a version holds at an error-correction level. */
export function dataCodewordCapacity(version, level = "M") {
  const ecc = requireLevel(level);
  return (
    Math.floor(rawDataModuleCount(version) / 8) -
    ECC_CODEWORDS_PER_BLOCK[ecc.ordinal][version] * ERROR_CORRECTION_BLOCKS[ecc.ordinal][version]
  );
}

/** Center coordinates of the alignment patterns of a version (ascending). */
export function alignmentPatternPositions(version) {
  requireVersion(version, "version");
  if (version === 1) {
    return [];
  }
  const count = Math.floor(version / 7) + 2;
  const step = Math.floor((version * 8 + count * 3 + 5) / (count * 4 - 4)) * 2;
  const result = [6];
  for (let position = version * 4 + 17 - 7; result.length < count; position -= step) {
    result.splice(1, 0, position);
  }
  return result;
}

function gfMultiply(x, y) {
  let product = 0;
  for (let bit = 7; bit >= 0; bit -= 1) {
    product = (product << 1) ^ ((product >>> 7) * 0x11d);
    product ^= ((y >>> bit) & 1) * x;
  }
  return product & 0xff;
}

function reedSolomonDivisor(degree) {
  const result = new Uint8Array(degree);
  result[degree - 1] = 1;
  let root = 1;
  for (let i = 0; i < degree; i += 1) {
    for (let j = 0; j < degree; j += 1) {
      result[j] = gfMultiply(result[j], root);
      if (j + 1 < degree) {
        result[j] ^= result[j + 1];
      }
    }
    root = gfMultiply(root, 0x02);
  }
  return result;
}

function reedSolomonRemainder(data, divisor) {
  const result = new Uint8Array(divisor.length);
  for (const byte of data) {
    const factor = byte ^ result[0];
    result.copyWithin(0, 1);
    result[result.length - 1] = 0;
    for (let i = 0; i < divisor.length; i += 1) {
      result[i] ^= gfMultiply(divisor[i], factor);
    }
  }
  return result;
}

function characterCountBits(version) {
  return version <= 9 ? 8 : 16;
}

function selectVersion(byteLength, ecc, minVersion, maxVersion) {
  for (let version = minVersion; version <= maxVersion; version += 1) {
    const countBits = characterCountBits(version);
    if (byteLength >= 2 ** countBits) {
      continue;
    }
    const usedBits = 4 + countBits + byteLength * 8;
    if (usedBits <= dataCodewordCapacity(version, ecc.name) * 8) {
      return version;
    }
  }
  throw new QrEncodeError(
    "data_too_long",
    `text needs more than QR version ${maxVersion} holds at level ${ecc.name} (${byteLength} bytes)`,
  );
}

function buildDataCodewords(bytes, version, ecc) {
  const capacityBits = dataCodewordCapacity(version, ecc.name) * 8;
  const bits = [];
  const append = (value, length) => {
    for (let i = length - 1; i >= 0; i -= 1) {
      bits.push((value >>> i) & 1);
    }
  };
  append(BYTE_MODE_INDICATOR, 4);
  append(bytes.length, characterCountBits(version));
  for (const byte of bytes) {
    append(byte, 8);
  }
  append(0, Math.min(4, capacityBits - bits.length));
  append(0, (8 - (bits.length % 8)) % 8);
  const codewords = new Uint8Array(capacityBits / 8);
  for (let i = 0; i < bits.length; i += 1) {
    codewords[i >>> 3] |= bits[i] << (7 - (i & 7));
  }
  for (let i = bits.length / 8, pad = 0; i < codewords.length; i += 1, pad ^= 1) {
    codewords[i] = PAD_BYTES[pad];
  }
  return codewords;
}

function addErrorCorrectionAndInterleave(data, version, ecc) {
  const blockCount = ERROR_CORRECTION_BLOCKS[ecc.ordinal][version];
  const blockEccLength = ECC_CODEWORDS_PER_BLOCK[ecc.ordinal][version];
  const rawCodewords = Math.floor(rawDataModuleCount(version) / 8);
  const shortBlockCount = blockCount - (rawCodewords % blockCount);
  const shortBlockLength = Math.floor(rawCodewords / blockCount);
  const divisor = reedSolomonDivisor(blockEccLength);
  const blocks = [];
  for (let i = 0, offset = 0; i < blockCount; i += 1) {
    const dataLength = shortBlockLength - blockEccLength + (i < shortBlockCount ? 0 : 1);
    const blockData = data.subarray(offset, offset + dataLength);
    offset += dataLength;
    const block = new Uint8Array(shortBlockLength + 1);
    block.set(blockData, 0);
    // Short blocks keep one unused slot so every block shares the same layout.
    block.set(reedSolomonRemainder(blockData, divisor), shortBlockLength + 1 - blockEccLength);
    blocks.push(block);
  }
  const result = new Uint8Array(rawCodewords);
  let index = 0;
  for (let column = 0; column <= shortBlockLength; column += 1) {
    for (let blockIndex = 0; blockIndex < blocks.length; blockIndex += 1) {
      if (column !== shortBlockLength - blockEccLength || blockIndex >= shortBlockCount) {
        result[index] = blocks[blockIndex][column];
        index += 1;
      }
    }
  }
  return result;
}

class ModuleMatrix {
  constructor(version) {
    this.version = version;
    this.size = version * 4 + 17;
    this.modules = new Uint8Array(this.size * this.size);
    this.reserved = new Uint8Array(this.size * this.size);
  }

  setFunction(x, y, dark) {
    const index = y * this.size + x;
    this.modules[index] = dark ? 1 : 0;
    this.reserved[index] = 1;
  }
}

function drawFinderPattern(matrix, centerX, centerY) {
  for (let dy = -4; dy <= 4; dy += 1) {
    for (let dx = -4; dx <= 4; dx += 1) {
      const distance = Math.max(Math.abs(dx), Math.abs(dy));
      const x = centerX + dx;
      const y = centerY + dy;
      if (x >= 0 && x < matrix.size && y >= 0 && y < matrix.size) {
        matrix.setFunction(x, y, distance !== 2 && distance !== 4);
      }
    }
  }
}

function drawAlignmentPattern(matrix, centerX, centerY) {
  for (let dy = -2; dy <= 2; dy += 1) {
    for (let dx = -2; dx <= 2; dx += 1) {
      matrix.setFunction(centerX + dx, centerY + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1);
    }
  }
}

function drawFormatBits(matrix, ecc, mask) {
  const data = (ecc.formatBits << 3) | mask;
  let remainder = data;
  for (let i = 0; i < 10; i += 1) {
    remainder = (remainder << 1) ^ ((remainder >>> 9) * 0x537);
  }
  const bits = ((data << 10) | remainder) ^ 0x5412;
  const { size } = matrix;
  for (let i = 0; i <= 5; i += 1) {
    matrix.setFunction(8, i, getBit(bits, i));
  }
  matrix.setFunction(8, 7, getBit(bits, 6));
  matrix.setFunction(8, 8, getBit(bits, 7));
  matrix.setFunction(7, 8, getBit(bits, 8));
  for (let i = 9; i < 15; i += 1) {
    matrix.setFunction(14 - i, 8, getBit(bits, i));
  }
  for (let i = 0; i < 8; i += 1) {
    matrix.setFunction(size - 1 - i, 8, getBit(bits, i));
  }
  for (let i = 8; i < 15; i += 1) {
    matrix.setFunction(8, size - 15 + i, getBit(bits, i));
  }
  matrix.setFunction(8, size - 8, true);
}

function drawVersionBits(matrix) {
  if (matrix.version < 7) {
    return;
  }
  let remainder = matrix.version;
  for (let i = 0; i < 12; i += 1) {
    remainder = (remainder << 1) ^ ((remainder >>> 11) * 0x1f25);
  }
  const bits = (matrix.version << 12) | remainder;
  for (let i = 0; i < 18; i += 1) {
    const dark = getBit(bits, i);
    const a = matrix.size - 11 + (i % 3);
    const b = Math.floor(i / 3);
    matrix.setFunction(a, b, dark);
    matrix.setFunction(b, a, dark);
  }
}

function drawFunctionPatterns(matrix, ecc) {
  const { size } = matrix;
  for (let i = 0; i < size; i += 1) {
    matrix.setFunction(6, i, i % 2 === 0);
    matrix.setFunction(i, 6, i % 2 === 0);
  }
  drawFinderPattern(matrix, 3, 3);
  drawFinderPattern(matrix, size - 4, 3);
  drawFinderPattern(matrix, 3, size - 4);
  const positions = alignmentPatternPositions(matrix.version);
  const last = positions.length - 1;
  for (let i = 0; i <= last; i += 1) {
    for (let j = 0; j <= last; j += 1) {
      const nearFinder = (i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0);
      if (!nearFinder) {
        drawAlignmentPattern(matrix, positions[i], positions[j]);
      }
    }
  }
  // Reserve the format area now; the final mask overwrites it.
  drawFormatBits(matrix, ecc, 0);
  drawVersionBits(matrix);
}

function drawCodewords(matrix, codewords) {
  const { size, modules, reserved } = matrix;
  const totalBits = codewords.length * 8;
  let bitIndex = 0;
  for (let right = size - 1; right >= 1; right -= 2) {
    if (right === 6) {
      right = 5;
    }
    const upward = ((right + 1) & 2) === 0;
    for (let vertical = 0; vertical < size; vertical += 1) {
      const y = upward ? size - 1 - vertical : vertical;
      for (let j = 0; j < 2; j += 1) {
        const index = y * size + right - j;
        if (!reserved[index] && bitIndex < totalBits) {
          modules[index] = getBit(codewords[bitIndex >>> 3], 7 - (bitIndex & 7)) ? 1 : 0;
          bitIndex += 1;
        }
      }
    }
  }
}

function maskApplies(mask, x, y) {
  switch (mask) {
    case 0:
      return (x + y) % 2 === 0;
    case 1:
      return y % 2 === 0;
    case 2:
      return x % 3 === 0;
    case 3:
      return (x + y) % 3 === 0;
    case 4:
      return (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0;
    case 5:
      return ((x * y) % 2) + ((x * y) % 3) === 0;
    case 6:
      return (((x * y) % 2) + ((x * y) % 3)) % 2 === 0;
    default:
      return (((x + y) % 2) + ((x * y) % 3)) % 2 === 0;
  }
}

function applyMask(matrix, mask) {
  const { size, modules, reserved } = matrix;
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const index = y * size + x;
      if (!reserved[index] && maskApplies(mask, x, y)) {
        modules[index] ^= 1;
      }
    }
  }
}

function penaltyScore(matrix) {
  const { size, modules } = matrix;
  const history = new Int32Array(7);
  let result = 0;

  const addHistory = (runLength) => {
    // The first run of a line also covers the light quiet zone before it.
    const length = history[0] === 0 ? runLength + size : runLength;
    history.copyWithin(1, 0, 6);
    history[0] = length;
  };
  const countFinderLikePatterns = () => {
    const n = history[1];
    const core =
      n > 0 && history[2] === n && history[3] === n * 3 && history[4] === n && history[5] === n;
    return (
      (core && history[0] >= n * 4 && history[6] >= n ? 1 : 0) +
      (core && history[6] >= n * 4 && history[0] >= n ? 1 : 0)
    );
  };
  const scoreLine = (moduleAt) => {
    history.fill(0);
    let runColor = 0;
    let runLength = 0;
    for (let i = 0; i < size; i += 1) {
      const color = moduleAt(i);
      if (color === runColor) {
        runLength += 1;
        if (runLength === 5) {
          result += PENALTY_N1;
        } else if (runLength > 5) {
          result += 1;
        }
      } else {
        addHistory(runLength);
        if (runColor === 0) {
          result += countFinderLikePatterns() * PENALTY_N3;
        }
        runColor = color;
        runLength = 1;
      }
    }
    // Close the line against the light quiet zone after it.
    if (runColor === 1) {
      addHistory(runLength);
      runLength = 0;
    }
    addHistory(runLength + size);
    result += countFinderLikePatterns() * PENALTY_N3;
  };

  for (let y = 0; y < size; y += 1) {
    scoreLine((x) => modules[y * size + x]);
  }
  for (let x = 0; x < size; x += 1) {
    scoreLine((y) => modules[y * size + x]);
  }
  for (let y = 0; y < size - 1; y += 1) {
    for (let x = 0; x < size - 1; x += 1) {
      const color = modules[y * size + x];
      if (
        color === modules[y * size + x + 1] &&
        color === modules[(y + 1) * size + x] &&
        color === modules[(y + 1) * size + x + 1]
      ) {
        result += PENALTY_N2;
      }
    }
  }
  let dark = 0;
  for (const module of modules) {
    dark += module;
  }
  const total = size * size;
  result += (Math.ceil(Math.abs(dark * 20 - total * 10) / total) - 1) * PENALTY_N4;
  return result;
}

/**
 * Encode text as a QR symbol in byte mode.
 *
 * @param {string} text UTF-8 text such as an IrohaConnect wallet URI.
 * @param {{ level?: "L" | "M" | "Q" | "H", minVersion?: number, maxVersion?: number, mask?: "auto" | number }} [options]
 */
export function encodeQr(text, options = {}) {
  if (typeof text !== "string") {
    throw new QrEncodeError("invalid_option", "QR text must be a string");
  }
  const ecc = requireLevel(options.level ?? "M");
  const minVersion = requireVersion(options.minVersion ?? QR_MIN_VERSION, "minVersion");
  const maxVersion = requireVersion(options.maxVersion ?? QR_MAX_VERSION, "maxVersion");
  if (minVersion > maxVersion) {
    throw new QrEncodeError("invalid_option", "minVersion must not exceed maxVersion");
  }
  const requestedMask = requireMask(options.mask);
  const bytes = new TextEncoder().encode(text);
  const version = selectVersion(bytes.length, ecc, minVersion, maxVersion);
  const codewords = addErrorCorrectionAndInterleave(buildDataCodewords(bytes, version, ecc), version, ecc);

  const matrix = new ModuleMatrix(version);
  drawFunctionPatterns(matrix, ecc);
  drawCodewords(matrix, codewords);

  let mask = requestedMask;
  if (mask === "auto") {
    let bestPenalty = Infinity;
    for (let candidate = 0; candidate < 8; candidate += 1) {
      applyMask(matrix, candidate);
      drawFormatBits(matrix, ecc, candidate);
      const penalty = penaltyScore(matrix);
      if (penalty < bestPenalty) {
        bestPenalty = penalty;
        mask = candidate;
      }
      applyMask(matrix, candidate);
    }
  }
  applyMask(matrix, mask);
  drawFormatBits(matrix, ecc, mask);

  const { size, modules } = matrix;
  return Object.freeze({
    version,
    size,
    level: ecc.name,
    mask,
    modules,
    /** True when the module at column `x`, row `y` is dark. */
    isDark(x, y) {
      return x >= 0 && y >= 0 && x < size && y < size && modules[y * size + x] === 1;
    },
  });
}

/**
 * SVG path data covering every dark module, offset by the quiet zone.
 *
 * Adjacent dark modules in a row share one rectangle, which keeps the path
 * short and leaves no hairline seams between modules.
 */
export function qrSvgPath(qr, margin = QR_QUIET_ZONE) {
  if (!Number.isInteger(margin) || margin < 0) {
    throw new QrEncodeError("invalid_option", "margin must be a non-negative integer");
  }
  const parts = [];
  for (let y = 0; y < qr.size; y += 1) {
    let x = 0;
    while (x < qr.size) {
      if (!qr.isDark(x, y)) {
        x += 1;
        continue;
      }
      const start = x;
      while (x < qr.size && qr.isDark(x, y)) {
        x += 1;
      }
      parts.push(`M${start + margin} ${y + margin}h${x - start}v1h${start - x}z`);
    }
  }
  return parts.join("");
}

function escapeXml(value) {
  return String(value).replace(/[&<>"']/gu, (character) => `&#${character.charCodeAt(0)};`);
}

const SAFE_COLOR = /^(#[0-9a-f]{3,8}|[a-z]+|rgba?\([0-9.,\s%]+\))$/iu;

function requireColor(value, name) {
  if (typeof value !== "string" || !SAFE_COLOR.test(value)) {
    throw new QrEncodeError("invalid_option", `${name} must be a hex, rgb() or named color`);
  }
  return value;
}

/**
 * Standalone SVG markup for a QR symbol, including the quiet zone.
 *
 * @param {ReturnType<typeof encodeQr>} qr
 * @param {{ margin?: number, dark?: string, light?: string, title?: string }} [options]
 */
export function qrToSvg(qr, options = {}) {
  const margin = options.margin ?? QR_QUIET_ZONE;
  const dark = requireColor(options.dark ?? "#15161a", "dark");
  const light = requireColor(options.light ?? "#ffffff", "light");
  const extent = qr.size + margin * 2;
  const title = options.title ? `<title>${escapeXml(options.title)}</title>` : "";
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${extent} ${extent}" ` +
    `shape-rendering="crispEdges" role="img"${options.title ? ` aria-label="${escapeXml(options.title)}"` : ""}>` +
    `${title}<rect width="${extent}" height="${extent}" fill="${light}"/>` +
    `<path fill="${dark}" d="${qrSvgPath(qr, margin)}"/></svg>`
  );
}
