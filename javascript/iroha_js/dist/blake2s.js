// Ported from the Android SDK implementation (Apache-2.0) to provide
// a minimal Blake2s-256 hash with keyed MAC support in pure JavaScript.

const BLOCK_BYTES = 64;
const ROUNDS = 10;

const IV = new Uint32Array([
  0x6a09e667,
  0xbb67ae85,
  0x3c6ef372,
  0xa54ff53a,
  0x510e527f,
  0x9b05688c,
  0x1f83d9ab,
  0x5be0cd19,
]);

const SIGMA = [
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
  [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
  [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
  [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
  [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
  [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
  [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
  [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
  [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
  [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

function ensureUint8Array(value) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (Buffer.isBuffer(value)) {
    return new Uint8Array(value);
  }
  if (typeof value === "string") {
    return new TextEncoder().encode(value);
  }
  return Uint8Array.from(value ?? []);
}

function rotateRight(value, shift) {
  return ((value >>> shift) | (value << (32 - shift))) >>> 0;
}

function readIntLE(bytes, offset) {
  return (
    bytes[offset] |
    (bytes[offset + 1] << 8) |
    (bytes[offset + 2] << 16) |
    (bytes[offset + 3] << 24)
  );
}

function gMix(v, a, b, c, d, x, y) {
  v[a] = (v[a] + v[b] + x) >>> 0;
  v[d] = rotateRight(v[d] ^ v[a], 16);
  v[c] = (v[c] + v[d]) >>> 0;
  v[b] = rotateRight(v[b] ^ v[c], 12);
  v[a] = (v[a] + v[b] + y) >>> 0;
  v[d] = rotateRight(v[d] ^ v[a], 8);
  v[c] = (v[c] + v[d]) >>> 0;
  v[b] = rotateRight(v[b] ^ v[c], 7);
}

function compress(state, block, blockOffset, t0, t1, last) {
  const m = new Uint32Array(16);
  for (let i = 0; i < 16; i += 1) {
    m[i] = readIntLE(block, blockOffset + i * 4);
  }

  const v = new Uint32Array(16);
  v.set(state);
  v.set(IV, 8);
  v[12] ^= t0;
  v[13] ^= t1;
  if (last) {
    v[14] ^= 0xffff_ffff;
  }

  for (let round = 0; round < ROUNDS; round += 1) {
    const s = SIGMA[round];
    gMix(v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
    gMix(v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
    gMix(v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
    gMix(v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
    gMix(v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
    gMix(v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
    gMix(v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
    gMix(v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
  }

  for (let i = 0; i < 8; i += 1) {
    state[i] ^= v[i] ^ v[i + 8];
  }
}

function incrementCounter(t0, t1, increment) {
  const sum = (t0 >>> 0) + increment;
  t0 = sum >>> 0;
  if (sum >= 2 ** 32) {
    t1 = (t1 + 1) >>> 0;
  }
  return [t0, t1];
}

function toOutput(state, outlen) {
  const out = new Uint8Array(outlen);
  let offset = 0;
  for (let value of state) {
    for (let i = 0; i < 4 && offset < outlen; i += 1) {
      out[offset++] = value & 0xff;
      value = value >>> 8;
    }
    if (offset >= outlen) {
      break;
    }
  }
  return out;
}

function blake2sDigest(message, key, outlen) {
  if (outlen <= 0 || outlen > 32) {
    throw new RangeError("blake2s outlen must be between 1 and 32 bytes");
  }
  if (key.length > 32) {
    throw new RangeError("blake2s key must be at most 32 bytes");
  }

  const state = IV.slice();
  const param = outlen | (key.length << 8) | (1 << 16) | (1 << 24);
  state[0] ^= param;

  let t0 = 0;
  let t1 = 0;

  if (key.length > 0) {
    const block = new Uint8Array(BLOCK_BYTES);
    block.set(key);
    [t0, t1] = incrementCounter(t0, t1, BLOCK_BYTES);
    compress(state, block, 0, t0, t1, message.length === 0);
  }

  if (message.length > 0) {
    let offset = 0;
    const fullBlocks = Math.floor(message.length / BLOCK_BYTES);
    const remainder = message.length % BLOCK_BYTES;

    for (let i = 0; i < fullBlocks; i += 1) {
      offset += BLOCK_BYTES;
      [t0, t1] = incrementCounter(t0, t1, BLOCK_BYTES);
      const isLastFull = i === fullBlocks - 1 && remainder === 0;
      compress(state, message, offset - BLOCK_BYTES, t0, t1, isLastFull);
    }

    if (remainder > 0) {
      const block = new Uint8Array(BLOCK_BYTES);
      block.set(message.subarray(message.length - remainder));
      [t0, t1] = incrementCounter(t0, t1, remainder);
      compress(state, block, 0, t0, t1, true);
    }
  } else if (key.length === 0) {
    const block = new Uint8Array(BLOCK_BYTES);
    compress(state, block, 0, 0, 0, true);
  }

  return toOutput(state, outlen);
}

export function blake2s(input, { key, outlen = 32 } = {}) {
  const message = ensureUint8Array(input);
  const keyBytes = key ? ensureUint8Array(key) : new Uint8Array();
  return blake2sDigest(message, keyBytes, outlen);
}

export function blake2sMac(input, key, outlen = 32) {
  if (!key) {
    throw new TypeError("blake2sMac requires a key");
  }
  return blake2s(input, { key, outlen });
}
