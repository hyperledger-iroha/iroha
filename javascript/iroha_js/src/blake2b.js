const UINT64_MASK = (1n << 64n) - 1n;
const BLAKE2B_BLOCK_LEN = 128;
const BLAKE2B_ROUNDS = 12;
const BLAKE2B_IV = Object.freeze([
  0x6A09E667F3BCC908n,
  0xBB67AE8584CAA73Bn,
  0x3C6EF372FE94F82Bn,
  0xA54FF53A5F1D36F1n,
  0x510E527FADE682D1n,
  0x9B05688C2B3E6C1Fn,
  0x1F83D9ABFB41BD6Bn,
  0x5BE0CD19137E2179n,
]);

const BLAKE2B_SIGMA = Object.freeze([
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
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
  [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
]);

function rotr64(value, shift) {
  const rotation = BigInt(shift & 63);
  return ((value >> rotation) | (value << (64n - rotation))) & UINT64_MASK;
}

function readUint64Le(bytes, offset) {
  let value = 0n;
  for (let index = 0; index < 8; index += 1) {
    const byte = bytes[offset + index];
    value |= BigInt(byte) << BigInt(index * 8);
  }
  return value & UINT64_MASK;
}

function asUint8Array(value, name) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  throw new TypeError(`${name ?? "data"} must be an ArrayBuffer or Uint8Array`);
}

function blake2bCompress(state, block, t0, t1, f0) {
  const message = new Array(16);
  for (let index = 0; index < 16; index += 1) {
    message[index] = readUint64Le(block, index * 8);
  }

  const v = new Array(16);
  for (let index = 0; index < 8; index += 1) {
    v[index] = state[index];
    v[index + 8] = BLAKE2B_IV[index];
  }

  v[12] = (v[12] ^ t0) & UINT64_MASK;
  v[13] = (v[13] ^ t1) & UINT64_MASK;
  v[14] = (v[14] ^ f0) & UINT64_MASK;

  function round(a, b, c, d, x, y) {
    v[a] = (v[a] + v[b] + x) & UINT64_MASK;
    v[d] = rotr64(v[d] ^ v[a], 32);
    v[c] = (v[c] + v[d]) & UINT64_MASK;
    v[b] = rotr64(v[b] ^ v[c], 24);
    v[a] = (v[a] + v[b] + y) & UINT64_MASK;
    v[d] = rotr64(v[d] ^ v[a], 16);
    v[c] = (v[c] + v[d]) & UINT64_MASK;
    v[b] = rotr64(v[b] ^ v[c], 63);
  }

  for (let roundIndex = 0; roundIndex < BLAKE2B_ROUNDS; roundIndex += 1) {
    const schedule = BLAKE2B_SIGMA[roundIndex];
    round(0, 4, 8, 12, message[schedule[0]], message[schedule[1]]);
    round(1, 5, 9, 13, message[schedule[2]], message[schedule[3]]);
    round(2, 6, 10, 14, message[schedule[4]], message[schedule[5]]);
    round(3, 7, 11, 15, message[schedule[6]], message[schedule[7]]);
    round(0, 5, 10, 15, message[schedule[8]], message[schedule[9]]);
    round(1, 6, 11, 12, message[schedule[10]], message[schedule[11]]);
    round(2, 7, 8, 13, message[schedule[12]], message[schedule[13]]);
    round(3, 4, 9, 14, message[schedule[14]], message[schedule[15]]);
  }

  for (let index = 0; index < 8; index += 1) {
    state[index] = (state[index] ^ v[index] ^ v[index + 8]) & UINT64_MASK;
  }
}

/**
 * Compute a BLAKE2b digest.
 * @param {ArrayBufferView | ArrayBuffer} data
 * @param {number} outputLength
 * @param {{ personalization?: ArrayBufferView | ArrayBuffer; includeZeroKeyBlock?: boolean }} [options]
 * @returns {Uint8Array}
 */
function blake2bDigest(data, outputLength, options = {}) {
  if (!Number.isInteger(outputLength) || outputLength < 1 || outputLength > 64) {
    throw new TypeError("outputLength must be an integer between 1 and 64");
  }
  const input = asUint8Array(data, "data");
  const state = BLAKE2B_IV.map((value) => value);
  state[0] ^= 0x01010000n ^ BigInt(outputLength);

  const personalization = options.personalization
    ? asUint8Array(options.personalization, "personalization")
    : null;
  if (personalization && personalization.length > 0) {
    const buffer = new Uint8Array(16);
    buffer.set(personalization.subarray(0, Math.min(16, personalization.length)));
    const p0 = readUint64Le(buffer, 0);
    const p1 = readUint64Le(buffer, 8);
    state[6] ^= p0;
    state[7] ^= p1;
  }

  let t0 = 0n;
  let t1 = 0n;
  if (options.includeZeroKeyBlock) {
    const zeroBlock = new Uint8Array(BLAKE2B_BLOCK_LEN);
    t0 = (t0 + BigInt(BLAKE2B_BLOCK_LEN)) & UINT64_MASK;
    blake2bCompress(state, zeroBlock, t0, t1, 0n);
  }

  const totalLength = input.length;
  const fullBlocks = Math.floor(totalLength / BLAKE2B_BLOCK_LEN);
  let offset = 0;

  for (let blockIndex = 0; blockIndex < fullBlocks; blockIndex += 1) {
    const block = input.subarray(offset, offset + BLAKE2B_BLOCK_LEN);
    offset += BLAKE2B_BLOCK_LEN;
    t0 = (t0 + BigInt(BLAKE2B_BLOCK_LEN)) & UINT64_MASK;
    if (t0 < BigInt(BLAKE2B_BLOCK_LEN)) {
      t1 = (t1 + 1n) & UINT64_MASK;
    }
    const lastFullBlock = blockIndex === fullBlocks - 1 && totalLength % BLAKE2B_BLOCK_LEN === 0;
    const flag = lastFullBlock ? UINT64_MASK : 0n;
    blake2bCompress(state, block, t0, t1, flag);
  }

  const remainder = totalLength % BLAKE2B_BLOCK_LEN;
  if (remainder > 0) {
    const lastBlock = new Uint8Array(BLAKE2B_BLOCK_LEN);
    lastBlock.set(input.subarray(offset));
    const add = BigInt(remainder);
    t0 = (t0 + add) & UINT64_MASK;
    if (t0 < add) {
      t1 = (t1 + 1n) & UINT64_MASK;
    }
    blake2bCompress(state, lastBlock, t0, t1, UINT64_MASK);
  } else if (totalLength === 0) {
    const zeroBlock = new Uint8Array(BLAKE2B_BLOCK_LEN);
    blake2bCompress(state, zeroBlock, 0n, 0n, UINT64_MASK);
  }

  const output = new Uint8Array(outputLength);
  let outIndex = 0;
  for (let index = 0; index < 8 && outIndex < outputLength; index += 1) {
    let word = state[index];
    for (let byteIndex = 0; byteIndex < 8 && outIndex < outputLength; byteIndex += 1) {
      output[outIndex] = Number(word & 0xffn);
      word >>= 8n;
      outIndex += 1;
    }
  }
  return output;
}

/**
 * Compute a BLAKE2b-256 digest.
 * @param {ArrayBufferView | ArrayBuffer} data
 * @param {{ personalization?: ArrayBufferView | ArrayBuffer; includeZeroKeyBlock?: boolean }} [options]
 * @returns {Uint8Array}
 */
export function blake2b256(data, options = {}) {
  return blake2bDigest(data, 32, options);
}

/**
 * Compute a BLAKE2b-512 digest.
 * @param {ArrayBufferView | ArrayBuffer} data
 * @param {{ personalization?: ArrayBufferView | ArrayBuffer; includeZeroKeyBlock?: boolean }} [options]
 * @returns {Uint8Array}
 */
export function blake2b512(data, options = {}) {
  return blake2bDigest(data, 64, options);
}
