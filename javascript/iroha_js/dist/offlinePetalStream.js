import { OfflineQrStreamScanSession } from "./offlineQrStream.js";

const PETAL_STREAM_MAGIC = Buffer.from([0x50, 0x53]);
const PETAL_STREAM_VERSION = 1;
const PETAL_STREAM_HEADER_LEN = 9;
const DEFAULT_BORDER = 1;
const DEFAULT_ANCHOR = 3;

export const PETAL_STREAM_GRID_SIZES = Object.freeze([
  33, 37, 41, 45, 49, 53, 57, 61, 65, 69,
]);

export class OfflinePetalStreamOptions {
  constructor({ gridSize = 0, border = DEFAULT_BORDER, anchorSize = DEFAULT_ANCHOR } = {}) {
    this.gridSize = normalizeNonNegativeInt(gridSize, "gridSize");
    this.border = normalizePositiveInt(border, "border");
    this.anchorSize = normalizePositiveInt(anchorSize, "anchorSize");
    if (this.gridSize > 0xffff) {
      throw new TypeError("gridSize must be <= 65535");
    }
    if (this.border > 0xff) {
      throw new TypeError("border must be <= 255");
    }
    if (this.anchorSize > 0xff) {
      throw new TypeError("anchorSize must be <= 255");
    }
  }
}

export class OfflinePetalStreamGrid {
  constructor({ gridSize, cells }) {
    this.gridSize = normalizePositiveInt(gridSize, "gridSize");
    if (!Array.isArray(cells)) {
      throw new TypeError("cells must be an array");
    }
    const expected = this.gridSize * this.gridSize;
    if (cells.length !== expected) {
      throw new TypeError("cells length does not match grid size");
    }
    this.cells = cells.map(Boolean);
  }

  get(x, y) {
    if (x < 0 || y < 0 || x >= this.gridSize || y >= this.gridSize) {
      return null;
    }
    return this.cells[y * this.gridSize + x];
  }
}

export class OfflinePetalStreamSampleGrid {
  constructor({ gridSize, samples }) {
    this.gridSize = normalizePositiveInt(gridSize, "gridSize");
    if (!Array.isArray(samples)) {
      throw new TypeError("samples must be an array");
    }
    const expected = this.gridSize * this.gridSize;
    if (samples.length !== expected) {
      throw new TypeError("samples length does not match grid size");
    }
    this.samples = samples.map((value) => normalizeByte(value, "sample"));
  }
}

export class OfflinePetalStreamEncoder {
  static encodeGrid(payload, options = {}) {
    const bytes = toBuffer(payload, "payload");
    if (bytes.length > 0xffff) {
      throw new TypeError("payload length exceeds 65535");
    }
    const normalized = new OfflinePetalStreamOptions(options);
    const gridSize = resolveGridSize(bytes.length, normalized);
    const capacity = capacityBits(gridSize, normalized);
    const bitsNeeded = (PETAL_STREAM_HEADER_LEN + bytes.length) * 8;
    if (bitsNeeded > capacity) {
      throw new TypeError("petal stream grid too small for payload");
    }
    const header = encodeHeader(bytes);
    const bits = [];
    pushBytesAsBits(header, bits);
    pushBytesAsBits(bytes, bits);
    const cells = new Array(gridSize * gridSize).fill(false);
    let bitIndex = 0;
    for (let y = 0; y < gridSize; y += 1) {
      for (let x = 0; x < gridSize; x += 1) {
        const idx = y * gridSize + x;
        const role = cellRole(x, y, gridSize, normalized);
        if (role === CellRole.BORDER || role === CellRole.ANCHOR_DARK) {
          cells[idx] = true;
        } else if (role === CellRole.ANCHOR_LIGHT) {
          cells[idx] = false;
        } else if (bitIndex < bits.length) {
          cells[idx] = bits[bitIndex];
          bitIndex += 1;
        }
      }
    }
    return new OfflinePetalStreamGrid({ gridSize, cells });
  }

  static encodeGrids(payloads, options = {}) {
    if (!Array.isArray(payloads)) {
      throw new TypeError("payloads must be an array");
    }
    const normalized = new OfflinePetalStreamOptions(options);
    const buffers = payloads.map((payload) => toBuffer(payload, "payload"));
    const maxLen = buffers.reduce((max, buffer) => Math.max(max, buffer.length), 0);
    const gridSize = normalized.gridSize || resolveGridSize(maxLen, normalized);
    const fixedOptions = new OfflinePetalStreamOptions({ ...options, gridSize });
    const grids = buffers.map((buffer) => OfflinePetalStreamEncoder.encodeGrid(buffer, fixedOptions));
    return { gridSize, grids };
  }
}

export class OfflinePetalStreamDecoder {
  static decodeGrid(grid, options = {}) {
    const normalized = new OfflinePetalStreamOptions(options);
    const gridSize = resolveGridSizeForDecode(grid.gridSize, normalized);
    if (gridSize !== grid.gridSize) {
      throw new TypeError("grid size mismatch");
    }
    const capacity = capacityBits(gridSize, normalized);
    const bits = [];
    bits.length = 0;
    for (let y = 0; y < gridSize; y += 1) {
      for (let x = 0; x < gridSize; x += 1) {
        if (cellRole(x, y, gridSize, normalized) === CellRole.DATA) {
          const bit = grid.get(x, y);
          if (bit !== null) {
            bits.push(bit);
          }
        }
      }
    }
    if (bits.length > capacity) {
      throw new TypeError("petal stream capacity exceeded");
    }
    const bytes = bitsToBytes(bits);
    return decodePayload(bytes);
  }

  static decodeSamples(sampleGrid, options = {}) {
    const normalized = new OfflinePetalStreamOptions(options);
    const gridSize = resolveGridSizeForDecode(sampleGrid.gridSize, normalized);
    if (gridSize !== sampleGrid.gridSize) {
      throw new TypeError("sample grid size mismatch");
    }
    let darkSum = 0;
    let lightSum = 0;
    let darkCount = 0;
    let lightCount = 0;
    for (let y = 0; y < gridSize; y += 1) {
      for (let x = 0; x < gridSize; x += 1) {
        const idx = y * gridSize + x;
        const value = sampleGrid.samples[idx];
        const role = cellRole(x, y, gridSize, normalized);
        if (role === CellRole.ANCHOR_DARK) {
          darkSum += value;
          darkCount += 1;
        } else if (role === CellRole.ANCHOR_LIGHT) {
          lightSum += value;
          lightCount += 1;
        }
      }
    }
    if (darkCount === 0 || lightCount === 0) {
      throw new TypeError("anchor sampling failed");
    }
    const darkAvg = darkSum / darkCount;
    const lightAvg = lightSum / lightCount;
    if (darkAvg >= lightAvg) {
      throw new TypeError("anchor contrast too low");
    }
    const threshold = Math.round((darkAvg + lightAvg) / 2);
    const cells = new Array(gridSize * gridSize);
    for (let i = 0; i < sampleGrid.samples.length; i += 1) {
      cells[i] = sampleGrid.samples[i] < threshold;
    }
    const grid = new OfflinePetalStreamGrid({ gridSize, cells });
    return OfflinePetalStreamDecoder.decodeGrid(grid, normalized);
  }
}

export function samplePetalStreamGridFromRgba(image, gridSize) {
  const normalizedSize = normalizePositiveInt(gridSize, "gridSize");
  const { data, width, height } = normalizeImageInput(image);
  const size = Math.min(width, height);
  const offsetX = Math.floor((width - size) / 2);
  const offsetY = Math.floor((height - size) / 2);
  const cellSize = Math.max(1, Math.floor(size / normalizedSize));
  const samples = [];
  samples.length = normalizedSize * normalizedSize;
  let outIndex = 0;
  for (let y = 0; y < normalizedSize; y += 1) {
    for (let x = 0; x < normalizedSize; x += 1) {
      let sum = 0;
      let count = 0;
      for (const oy of [0.25, 0.5, 0.75]) {
        for (const ox of [0.25, 0.5, 0.75]) {
          const px = Math.min(
            width - 1,
            offsetX + Math.floor((x + ox) * cellSize),
          );
          const py = Math.min(
            height - 1,
            offsetY + Math.floor((y + oy) * cellSize),
          );
          const idx = (py * width + px) * 4;
          const r = data[idx];
          const g = data[idx + 1];
          const b = data[idx + 2];
          const luma = (77 * r + 150 * g + 29 * b) >> 8;
          sum += luma;
          count += 1;
        }
      }
      samples[outIndex] = Math.floor(sum / Math.max(1, count));
      outIndex += 1;
    }
  }
  return new OfflinePetalStreamSampleGrid({ gridSize: normalizedSize, samples });
}

export function decodePetalStreamFrameAuto(image, options = {}) {
  const normalized = new OfflinePetalStreamOptions({ ...options, gridSize: 0 });
  for (const candidate of PETAL_STREAM_GRID_SIZES) {
    try {
      const samples = samplePetalStreamGridFromRgba(image, candidate);
      const payload = OfflinePetalStreamDecoder.decodeSamples(samples, normalized);
      return { gridSize: candidate, payload };
    } catch {
      // Try the next candidate.
    }
  }
  throw new TypeError("failed to auto-detect grid size");
}

export class OfflinePetalStreamScanSession {
  constructor({ qrSession = new OfflineQrStreamScanSession(), petalOptions = {} } = {}) {
    this.qrSession = qrSession;
    this.petalOptions = new OfflinePetalStreamOptions(petalOptions);
    this.gridSize = this.petalOptions.gridSize || null;
  }

  ingestSampleGrid(sampleGrid) {
    const payload = OfflinePetalStreamDecoder.decodeSamples(sampleGrid, this.petalOptions);
    return this.qrSession.ingest(payload);
  }

  ingestRgba(image, gridSize = this.gridSize) {
    if (!gridSize) {
      return this.ingestRgbaAuto(image);
    }
    const samples = samplePetalStreamGridFromRgba(image, gridSize);
    const payload = OfflinePetalStreamDecoder.decodeSamples(samples, this.petalOptions);
    return this.qrSession.ingest(payload);
  }

  ingestRgbaAuto(image) {
    const result = decodePetalStreamFrameAuto(image, this.petalOptions);
    if (!this.gridSize) {
      this.gridSize = result.gridSize;
    }
    return this.qrSession.ingest(result.payload);
  }
}

const CellRole = Object.freeze({
  BORDER: 0,
  ANCHOR_DARK: 1,
  ANCHOR_LIGHT: 2,
  DATA: 3,
});

function resolveGridSize(payloadLength, options) {
  if (options.border === 0) {
    throw new TypeError("border must be > 0");
  }
  if (options.anchorSize === 0) {
    throw new TypeError("anchorSize must be > 0");
  }
  const bitsNeeded = (PETAL_STREAM_HEADER_LEN + payloadLength) * 8;
  if (options.gridSize !== 0) {
    const capacity = capacityBits(options.gridSize, options);
    if (bitsNeeded > capacity) {
      throw new TypeError("petal stream grid too small for payload");
    }
    return options.gridSize;
  }
  for (const candidate of PETAL_STREAM_GRID_SIZES) {
    if (candidate === 0) {
      continue;
    }
    const capacity = capacityBits(candidate, options);
    if (bitsNeeded <= capacity) {
      return candidate;
    }
  }
  throw new TypeError("petal stream grid too small for payload");
}

function resolveGridSizeForDecode(gridSize, options) {
  if (options.gridSize !== 0) {
    return options.gridSize;
  }
  if (gridSize === 0) {
    throw new TypeError("grid size is zero");
  }
  return gridSize;
}

function capacityBits(gridSize, options) {
  if (gridSize <= 0) {
    throw new TypeError("grid size must be > 0");
  }
  const minGrid = options.border * 2 + options.anchorSize * 2 + 1;
  if (gridSize < minGrid) {
    throw new TypeError("grid size too small for anchors");
  }
  const total = gridSize * gridSize;
  const borderCells = gridSize * 4 - 4;
  const anchorCells = options.anchorSize * options.anchorSize * 4;
  const dataCells = Math.max(0, total - borderCells - anchorCells);
  return dataCells;
}

function cellRole(x, y, gridSize, options) {
  const border = options.border;
  const anchor = options.anchorSize;
  if (x < border || y < border || x >= gridSize - border || y >= gridSize - border) {
    return CellRole.BORDER;
  }
  const right = gridSize - border - anchor;
  const bottom = gridSize - border - anchor;
  const inLeft = x >= border && x < border + anchor;
  const inRight = x >= right && x < right + anchor;
  const inTop = y >= border && y < border + anchor;
  const inBottom = y >= bottom && y < bottom + anchor;
  if (inLeft && inTop) {
    return CellRole.ANCHOR_DARK;
  }
  if (inLeft && inBottom) {
    return CellRole.ANCHOR_DARK;
  }
  if (inRight && inTop) {
    return CellRole.ANCHOR_LIGHT;
  }
  if (inRight && inBottom) {
    return CellRole.ANCHOR_LIGHT;
  }
  return CellRole.DATA;
}

function encodeHeader(payload) {
  const header = Buffer.alloc(PETAL_STREAM_HEADER_LEN);
  PETAL_STREAM_MAGIC.copy(header, 0);
  header.writeUInt8(PETAL_STREAM_VERSION, 2);
  header.writeUInt16LE(payload.length, 3);
  header.writeUInt32LE(crc32(payload), 5);
  return header;
}

function decodePayload(bytes) {
  if (bytes.length < PETAL_STREAM_HEADER_LEN) {
    throw new TypeError("petal stream header too short");
  }
  if (bytes[0] !== PETAL_STREAM_MAGIC[0] || bytes[1] !== PETAL_STREAM_MAGIC[1]) {
    throw new TypeError("petal stream magic mismatch");
  }
  const version = bytes[2];
  if (version !== PETAL_STREAM_VERSION) {
    throw new TypeError(`unsupported petal stream version ${version}`);
  }
  const payloadLength = bytes.readUInt16LE(3);
  const crc = bytes.readUInt32LE(5);
  const start = PETAL_STREAM_HEADER_LEN;
  const end = start + payloadLength;
  if (end > bytes.length) {
    throw new TypeError("petal stream payload length exceeds data");
  }
  const payload = bytes.subarray(start, end);
  const expected = crc32(payload);
  if (expected !== crc) {
    throw new TypeError("petal stream checksum mismatch");
  }
  return payload;
}

function pushBytesAsBits(bytes, out) {
  for (const byte of bytes) {
    for (let bit = 7; bit >= 0; bit -= 1) {
      out.push((byte & (1 << bit)) !== 0);
    }
  }
}

function bitsToBytes(bits) {
  const out = [];
  for (let offset = 0; offset < bits.length; offset += 8) {
    let value = 0;
    for (let idx = 0; idx < 8 && offset + idx < bits.length; idx += 1) {
      if (bits[offset + idx]) {
        value |= 1 << (7 - idx);
      }
    }
    out.push(value);
  }
  return Buffer.from(out);
}

const CRC32_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i += 1) {
    let c = i;
    for (let k = 0; k < 8; k += 1) {
      if (c & 1) {
        c = 0xedb88320 ^ (c >>> 1);
      } else {
        c >>>= 1;
      }
    }
    table[i] = c >>> 0;
  }
  return table;
})();

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function normalizeImageInput(image) {
  if (!image || typeof image !== "object") {
    throw new TypeError("image must be an object");
  }
  const { data, width, height } = image;
  if (data === undefined || typeof width !== "number" || typeof height !== "number") {
    throw new TypeError("image must include data, width, and height");
  }
  const bytes = Buffer.isBuffer(data) ? data : Buffer.from(data);
  if (width <= 0 || height <= 0) {
    throw new TypeError("image dimensions must be positive");
  }
  if (bytes.length < width * height * 4) {
    throw new TypeError("image data length is too small");
  }
  return { data: bytes, width, height };
}

function toBuffer(value, context) {
  if (
    value instanceof ArrayBuffer ||
    ArrayBuffer.isView(value) ||
    value instanceof Buffer
  ) {
    return Buffer.from(value);
  }
  throw new TypeError(`${context} must be a Buffer, ArrayBuffer, or ArrayBufferView`);
}

function normalizePositiveInt(value, context) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || !Number.isInteger(numeric) || numeric <= 0) {
    throw new TypeError(`${context} must be a positive integer`);
  }
  return numeric;
}

function normalizeNonNegativeInt(value, context) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || !Number.isInteger(numeric) || numeric < 0) {
    throw new TypeError(`${context} must be a non-negative integer`);
  }
  return numeric;
}

function normalizeByte(value, context) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || numeric < 0 || numeric > 255) {
    throw new TypeError(`${context} must be a byte`);
  }
  return Math.round(numeric);
}
