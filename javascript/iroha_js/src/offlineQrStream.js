import { blake2b256 } from "./blake2b.js";

const QR_STREAM_MAGIC = Buffer.from([0x49, 0x51]);
const QR_STREAM_VERSION = 1;
const ENVELOPE_VERSION = 1;
const ENCODING_BINARY = 0;
const QR_TEXT_PREFIX = "iroha:qr1:";

export const OfflineQrStreamFrameKind = Object.freeze({
  header: 0,
  data: 1,
  parity: 2,
});

export const OfflineQrStreamFrameEncoding = Object.freeze({
  binary: "binary",
  base64: "base64",
});

export const OfflineQrPayloadKind = Object.freeze({
  unspecified: 0,
  offlineToOnlineTransfer: 1,
  offlineSpendReceipt: 2,
  offlineEnvelope: 3,
});

export class OfflineQrStreamOptions {
  constructor({
    chunkSize = 360,
    parityGroup = 0,
    payloadKind = OfflineQrPayloadKind.unspecified,
  } = {}) {
    this.chunkSize = normalizePositiveInt(chunkSize, "chunkSize");
    this.parityGroup = normalizeNonNegativeInt(parityGroup, "parityGroup");
    this.payloadKind = normalizeNonNegativeInt(payloadKind, "payloadKind");
    if (this.parityGroup > 0xff) {
      throw new TypeError("parityGroup must be <= 255");
    }
    if (this.payloadKind > 0xffff) {
      throw new TypeError("payloadKind must be <= 65535");
    }
  }
}

export class OfflineQrStreamEnvelope {
  constructor({
    flags = 0,
    encoding = ENCODING_BINARY,
    parityGroup = 0,
    chunkSize,
    dataChunks,
    parityChunks,
    payloadKind,
    payloadLength,
    payloadHash,
  }) {
    this.version = ENVELOPE_VERSION;
    this.flags = flags;
    this.encoding = encoding;
    this.parityGroup = parityGroup;
    this.chunkSize = chunkSize;
    this.dataChunks = dataChunks;
    this.parityChunks = parityChunks;
    this.payloadKind = payloadKind;
    this.payloadLength = payloadLength;
    this.payloadHash = toBuffer(payloadHash, "payloadHash");
    if (this.payloadHash.length !== 32) {
      throw new TypeError("payloadHash must be 32 bytes");
    }
  }

  get streamId() {
    return this.payloadHash.subarray(0, 16);
  }

  encode() {
    const buffer = Buffer.alloc(1 + 1 + 1 + 1 + 2 + 2 + 2 + 2 + 4 + 32);
    let offset = 0;
    buffer.writeUInt8(this.version, offset++);
    buffer.writeUInt8(this.flags, offset++);
    buffer.writeUInt8(this.encoding, offset++);
    buffer.writeUInt8(this.parityGroup, offset++);
    buffer.writeUInt16LE(this.chunkSize, offset);
    offset += 2;
    buffer.writeUInt16LE(this.dataChunks, offset);
    offset += 2;
    buffer.writeUInt16LE(this.parityChunks, offset);
    offset += 2;
    buffer.writeUInt16LE(this.payloadKind, offset);
    offset += 2;
    buffer.writeUInt32LE(this.payloadLength, offset);
    offset += 4;
    this.payloadHash.copy(buffer, offset);
    return buffer;
  }

  static decode(bytes) {
    const buffer = toBuffer(bytes, "envelope");
    if (buffer.length < 46) {
      throw new TypeError("envelope is too short");
    }
    let offset = 0;
    const version = buffer.readUInt8(offset++);
    if (version !== ENVELOPE_VERSION) {
      throw new TypeError(`unsupported envelope version ${version}`);
    }
    const flags = buffer.readUInt8(offset++);
    const encoding = buffer.readUInt8(offset++);
    const parityGroup = buffer.readUInt8(offset++);
    const chunkSize = buffer.readUInt16LE(offset);
    offset += 2;
    const dataChunks = buffer.readUInt16LE(offset);
    offset += 2;
    const parityChunks = buffer.readUInt16LE(offset);
    offset += 2;
    const payloadKind = buffer.readUInt16LE(offset);
    offset += 2;
    const payloadLength = buffer.readUInt32LE(offset);
    offset += 4;
    const payloadHash = buffer.subarray(offset, offset + 32);
    return new OfflineQrStreamEnvelope({
      flags,
      encoding,
      parityGroup,
      chunkSize,
      dataChunks,
      parityChunks,
      payloadKind,
      payloadLength,
      payloadHash,
    });
  }
}

export class OfflineQrStreamFrame {
  constructor({ kind, streamId, index, total, payload }) {
    this.kind = kind;
    this.streamId = toBuffer(streamId, "streamId");
    if (this.streamId.length !== 16) {
      throw new TypeError("streamId must be 16 bytes");
    }
    this.index = index;
    this.total = total;
    this.payload = toBuffer(payload ?? Buffer.alloc(0), "payload");
  }

  encode() {
    const payloadLength = this.payload.length;
    if (payloadLength > 0xffff) {
      throw new TypeError("payload length exceeds 65535");
    }
    const headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2;
    const buffer = Buffer.alloc(headerLength + payloadLength + 4);
    let offset = 0;
    QR_STREAM_MAGIC.copy(buffer, offset);
    offset += 2;
    buffer.writeUInt8(QR_STREAM_VERSION, offset++);
    buffer.writeUInt8(this.kind, offset++);
    this.streamId.copy(buffer, offset);
    offset += 16;
    buffer.writeUInt16LE(this.index, offset);
    offset += 2;
    buffer.writeUInt16LE(this.total, offset);
    offset += 2;
    buffer.writeUInt16LE(payloadLength, offset);
    offset += 2;
    this.payload.copy(buffer, offset);
    offset += payloadLength;
    const crc = crc32(buffer.subarray(2, offset));
    buffer.writeUInt32LE(crc, offset);
    return buffer;
  }

  static decode(bytes) {
    const buffer = toBuffer(bytes, "frame");
    const headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2;
    if (buffer.length < headerLength + 4) {
      throw new TypeError("frame is too short");
    }
    if (buffer[0] !== QR_STREAM_MAGIC[0] || buffer[1] !== QR_STREAM_MAGIC[1]) {
      throw new TypeError("frame magic mismatch");
    }
    const version = buffer.readUInt8(2);
    if (version !== QR_STREAM_VERSION) {
      throw new TypeError(`unsupported frame version ${version}`);
    }
    const kind = buffer.readUInt8(3);
    if (!Object.values(OfflineQrStreamFrameKind).includes(kind)) {
      throw new TypeError(`unsupported frame kind ${kind}`);
    }
    const streamId = buffer.subarray(4, 20);
    const index = buffer.readUInt16LE(20);
    const total = buffer.readUInt16LE(22);
    const payloadLength = buffer.readUInt16LE(24);
    const payloadEnd = 26 + payloadLength;
    if (payloadEnd + 4 > buffer.length) {
      throw new TypeError("frame payload length exceeds buffer size");
    }
    const payload = buffer.subarray(26, payloadEnd);
    const expected = buffer.readUInt32LE(payloadEnd);
    const computed = crc32(buffer.subarray(2, payloadEnd));
    if (expected !== computed) {
      throw new TypeError("frame checksum mismatch");
    }
    return new OfflineQrStreamFrame({ kind, streamId, index, total, payload });
  }
}

export class OfflineQrStreamEncoder {
  static encodeFrames(payload, options = {}) {
    const normalized = toBuffer(payload, "payload");
    const { chunkSize, parityGroup, payloadKind } = new OfflineQrStreamOptions(options);
    if (normalized.length > 0xffffffff) {
      throw new TypeError("payload length exceeds 4GB");
    }
    const dataChunks =
      normalized.length === 0 ? 0 : Math.ceil(normalized.length / chunkSize);
    if (dataChunks > 0xffff) {
      throw new TypeError("data chunk count exceeds 65535");
    }
    const parityChunks =
      parityGroup > 0 ? Math.ceil(dataChunks / parityGroup) : 0;
    if (parityChunks > 0xffff) {
      throw new TypeError("parity chunk count exceeds 65535");
    }
    const payloadHash = Buffer.from(blake2b256(normalized));
    const envelope = new OfflineQrStreamEnvelope({
      parityGroup,
      chunkSize,
      dataChunks,
      parityChunks,
      payloadKind,
      payloadLength: normalized.length,
      payloadHash,
    });
    const frames = [];
    frames.push(
      new OfflineQrStreamFrame({
        kind: OfflineQrStreamFrameKind.header,
        streamId: envelope.streamId,
        index: 0,
        total: 1,
        payload: envelope.encode(),
      }),
    );
    for (let index = 0; index < dataChunks; index += 1) {
      const start = index * chunkSize;
      const end = Math.min(normalized.length, start + chunkSize);
      frames.push(
        new OfflineQrStreamFrame({
          kind: OfflineQrStreamFrameKind.data,
          streamId: envelope.streamId,
          index,
          total: dataChunks,
          payload: normalized.subarray(start, end),
        }),
      );
    }
    if (parityGroup > 0) {
      for (let groupIndex = 0; groupIndex < parityChunks; groupIndex += 1) {
        frames.push(
          new OfflineQrStreamFrame({
            kind: OfflineQrStreamFrameKind.parity,
            streamId: envelope.streamId,
            index: groupIndex,
            total: parityChunks,
            payload: xorParity(
              normalized,
              chunkSize,
              dataChunks,
              groupIndex,
              parityGroup,
            ),
          }),
        );
      }
    }
    return frames;
  }

  static encodeFrameBytes(payload, options = {}) {
    return OfflineQrStreamEncoder.encodeFrames(payload, options).map((frame) =>
      frame.encode(),
    );
  }
}

export class OfflineQrStreamDecoder {
  constructor() {
    this.envelope = null;
    this.dataChunks = [];
    this.parityChunks = [];
    this.pendingFrames = [];
    this.recovered = new Set();
  }

  ingest(frameBytes) {
    const frame = OfflineQrStreamFrame.decode(frameBytes);
    this._ingestFrame(frame);
    const payload = this._finalizeIfComplete();
    return {
      payload,
      receivedChunks: this.dataChunks.filter(Boolean).length,
      totalChunks: this.dataChunks.length,
      recoveredChunks: this.recovered.size,
      progress:
        this.dataChunks.length === 0
          ? 0
          : this.dataChunks.filter(Boolean).length / this.dataChunks.length,
      isComplete: Boolean(payload),
    };
  }

  _ingestFrame(frame) {
    if (frame.kind === OfflineQrStreamFrameKind.header) {
      const envelope = OfflineQrStreamEnvelope.decode(frame.payload);
      if (!bufferEquals(envelope.streamId, frame.streamId)) {
        throw new TypeError("stream id mismatch");
      }
      this.envelope = envelope;
      this.dataChunks = new Array(envelope.dataChunks).fill(null);
      this.parityChunks = new Array(envelope.parityChunks).fill(null);
      if (this.pendingFrames.length > 0) {
        const pending = this.pendingFrames.slice();
        this.pendingFrames = [];
        for (const pendingFrame of pending) {
          if (bufferEquals(pendingFrame.streamId, envelope.streamId)) {
            this._ingestFrame(pendingFrame);
          }
        }
      }
      return;
    }
    if (!this.envelope) {
      this.pendingFrames.push(frame);
      return;
    }
    if (!bufferEquals(frame.streamId, this.envelope.streamId)) {
      return;
    }
    if (frame.kind === OfflineQrStreamFrameKind.data) {
      const index = frame.index;
      if (index < this.dataChunks.length && !this.dataChunks[index]) {
        this.dataChunks[index] = frame.payload;
      }
    } else if (frame.kind === OfflineQrStreamFrameKind.parity) {
      const index = frame.index;
      if (index < this.parityChunks.length && !this.parityChunks[index]) {
        this.parityChunks[index] = frame.payload;
      }
    }
    this._recoverMissing();
  }

  _recoverMissing() {
    if (!this.envelope || this.envelope.parityGroup === 0) {
      return;
    }
    const groupSize = this.envelope.parityGroup;
    const chunkSize = this.envelope.chunkSize;
    for (let groupIndex = 0; groupIndex < this.parityChunks.length; groupIndex += 1) {
      const parity = this.parityChunks[groupIndex];
      if (!parity) {
        continue;
      }
      const start = groupIndex * groupSize;
      const end = Math.min(this.dataChunks.length, start + groupSize);
      if (start >= end) {
        continue;
      }
      let missingIndex = null;
      const xor = Buffer.from(parity);
      for (let dataIndex = start; dataIndex < end; dataIndex += 1) {
        const chunk = this.dataChunks[dataIndex];
        if (chunk) {
          const padded = Buffer.alloc(chunkSize);
          chunk.copy(padded, 0, 0, Math.min(chunk.length, chunkSize));
          for (let offset = 0; offset < chunkSize; offset += 1) {
            xor[offset] ^= padded[offset];
          }
        } else if (missingIndex === null) {
          missingIndex = dataIndex;
        } else {
          missingIndex = null;
          break;
        }
      }
      if (missingIndex !== null) {
        const length = this._expectedChunkLength(missingIndex);
        this.dataChunks[missingIndex] = xor.subarray(0, length);
        this.recovered.add(missingIndex);
      }
    }
  }

  _expectedChunkLength(index) {
    if (!this.envelope) {
      return 0;
    }
    const chunkSize = this.envelope.chunkSize;
    const total = this.envelope.dataChunks;
    if (index < total - 1) {
      return chunkSize;
    }
    const tail = this.envelope.payloadLength - chunkSize * (total - 1);
    return Math.max(0, Math.min(chunkSize, tail));
  }

  _finalizeIfComplete() {
    if (!this.envelope) {
      return null;
    }
    if (this.dataChunks.some((chunk) => !chunk)) {
      return null;
    }
    const payload = Buffer.concat(
      this.dataChunks.map((chunk, index) =>
        chunk.subarray(0, this._expectedChunkLength(index)),
      ),
    );
    const expected = Buffer.from(blake2b256(payload));
    if (!bufferEquals(expected, this.envelope.payloadHash)) {
      throw new TypeError("payload hash mismatch");
    }
    return payload.subarray(0, this.envelope.payloadLength);
  }
}

export class OfflineQrStreamScanSession {
  constructor({ frameEncoding = OfflineQrStreamFrameEncoding.base64 } = {}) {
    this.decoder = new OfflineQrStreamDecoder();
    this.frameEncoding = frameEncoding;
  }

  ingest(frame, encoding = this.frameEncoding) {
    const bytes = normalizeFrameInput(frame, encoding);
    return this.decoder.ingest(bytes);
  }
}

export class OfflineQrStreamTheme {
  constructor({ name, backgroundStart, backgroundEnd, accent, petal, petalCount, pulsePeriod }) {
    this.name = name;
    this.backgroundStart = backgroundStart;
    this.backgroundEnd = backgroundEnd;
    this.accent = accent;
    this.petal = petal;
    this.petalCount = petalCount;
    this.pulsePeriod = pulsePeriod;
  }

  frameStyle(frameIndex, totalFrames) {
    const safeTotal = Math.max(1, totalFrames);
    const phase = (frameIndex % safeTotal) / safeTotal;
    const pulse = (Math.sin((frameIndex / this.pulsePeriod) * Math.PI * 2) + 1) / 2;
    return {
      petalPhase: phase,
      accentStrength: pulse,
      gradientAngle: phase * 360,
    };
  }
}

export const sakuraQrStreamTheme = new OfflineQrStreamTheme({
  name: "sakura",
  backgroundStart: { red: 0.98, green: 0.94, blue: 0.96 },
  backgroundEnd: { red: 1.0, green: 0.98, blue: 0.99 },
  accent: { red: 0.92, green: 0.48, blue: 0.6 },
  petal: { red: 0.98, green: 0.8, blue: 0.86 },
  petalCount: 6,
  pulsePeriod: 48,
});

export class OfflineQrStreamPlaybackSkin {
  constructor({
    name,
    theme,
    frameRate = 12,
    petalDriftSpeed = 1.0,
    progressOverlayAlpha = 0.4,
    reducedMotion = false,
    lowPower = false,
  }) {
    this.name = assertNonEmptyString(name, "name");
    this.theme = theme ?? sakuraQrStreamTheme;
    this.frameRate = normalizePositiveInt(frameRate, "frameRate");
    this.petalDriftSpeed = Number(petalDriftSpeed);
    this.progressOverlayAlpha = Number(progressOverlayAlpha);
    this.reducedMotion = Boolean(reducedMotion);
    this.lowPower = Boolean(lowPower);
  }

  frameStyle(frameIndex, totalFrames, progress = 0) {
    const safeTotal = Math.max(1, totalFrames);
    const phase = (frameIndex % safeTotal) / safeTotal;
    const pulse =
      (Math.sin((frameIndex / this.theme.pulsePeriod) * Math.PI * 2) + 1) / 2;
    const angle = phase * 360;
    const drift = this.reducedMotion
      ? 0
      : Math.sin(phase * Math.PI * 2) * this.petalDriftSpeed;
    const clamped = Math.max(0, Math.min(1, progress));
    return {
      petalPhase: phase,
      accentStrength: pulse,
      gradientAngle: angle,
      driftOffset: drift,
      progressAlpha: this.progressOverlayAlpha * clamped,
    };
  }
}

export const sakuraQrStreamSkin = new OfflineQrStreamPlaybackSkin({
  name: "sakura",
  theme: sakuraQrStreamTheme,
  frameRate: 12,
  petalDriftSpeed: 1.0,
  progressOverlayAlpha: 0.4,
});

export const sakuraQrStreamReducedMotionSkin = new OfflineQrStreamPlaybackSkin({
  name: "sakura-reduced-motion",
  theme: sakuraQrStreamTheme,
  frameRate: 6,
  petalDriftSpeed: 0.0,
  progressOverlayAlpha: 0.25,
  reducedMotion: true,
});

export const sakuraQrStreamLowPowerSkin = new OfflineQrStreamPlaybackSkin({
  name: "sakura-low-power",
  theme: new OfflineQrStreamTheme({
    name: "sakura-low-power",
    backgroundStart: sakuraQrStreamTheme.backgroundStart,
    backgroundEnd: sakuraQrStreamTheme.backgroundEnd,
    accent: sakuraQrStreamTheme.accent,
    petal: sakuraQrStreamTheme.petal,
    petalCount: 4,
    pulsePeriod: 72,
  }),
  frameRate: 8,
  petalDriftSpeed: 0.4,
  progressOverlayAlpha: 0.3,
  lowPower: true,
});

export function encodeQrFrameText(bytes, encoding = OfflineQrStreamFrameEncoding.base64) {
  const buffer = toBuffer(bytes, "frameBytes");
  if (encoding === OfflineQrStreamFrameEncoding.base64) {
    return `${QR_TEXT_PREFIX}${buffer.toString("base64")}`;
  }
  return buffer.toString("base64");
}

export function decodeQrFrameText(value, encoding = OfflineQrStreamFrameEncoding.base64) {
  const text = assertNonEmptyString(value, "value").trim();
  if (encoding === OfflineQrStreamFrameEncoding.base64) {
    if (!text.startsWith(QR_TEXT_PREFIX)) {
      throw new Error("qr text prefix missing");
    }
    return Buffer.from(text.slice(QR_TEXT_PREFIX.length), "base64");
  }
  return Buffer.from(text, "base64");
}

export async function scanQrStreamFrames(frames, options = {}) {
  const {
    session = new OfflineQrStreamScanSession(),
    frameEncoding = OfflineQrStreamFrameEncoding.base64,
  } = options;
  let result = null;
  for await (const frame of frames) {
    result = session.ingest(frame, frameEncoding);
    if (result && result.isComplete) {
      break;
    }
  }
  return result;
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

function assertNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value;
}

function normalizeFrameInput(frame, encoding) {
  if (
    frame instanceof Buffer ||
    ArrayBuffer.isView(frame) ||
    frame instanceof ArrayBuffer
  ) {
    return Buffer.from(frame);
  }
  if (typeof frame === "string") {
    return decodeQrFrameText(frame, encoding);
  }
  throw new TypeError("frame must be bytes or text");
}

function xorParity(payload, chunkSize, dataChunks, groupIndex, groupSize) {
  const parity = Buffer.alloc(chunkSize);
  const startIndex = groupIndex * groupSize;
  const endIndex = Math.min(dataChunks, startIndex + groupSize);
  if (startIndex >= endIndex) {
    return parity;
  }
  for (let chunkIndex = startIndex; chunkIndex < endIndex; chunkIndex += 1) {
    const start = chunkIndex * chunkSize;
    const end = Math.min(payload.length, start + chunkSize);
    const chunk = payload.subarray(start, end);
    for (let offset = 0; offset < chunk.length; offset += 1) {
      parity[offset] ^= chunk[offset];
    }
  }
  return parity;
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

function bufferEquals(a, b) {
  if (a.length !== b.length) {
    return false;
  }
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) {
      return false;
    }
  }
  return true;
}
