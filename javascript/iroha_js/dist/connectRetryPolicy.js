const GAMMA = 0x9e3779b97f4a7c15n;
const ATTEMPT_MIX = 0xd1b54a32d192ed03n;
const SEED_INIT = 0xa0761d6478bd642fn;
const MASK64 = (1n << 64n) - 1n;

function loadLittleEndian(bytes, offset, length) {
  let value = 0n;
  for (let i = 0; i < length; i += 1) {
    value |= BigInt(bytes[offset + i]) << BigInt(i * 8);
  }
  return value;
}

function splitmix64(x) {
  let z = x & MASK64;
  z ^= z >> 30n;
  z = (z * 0xbf58476d1ce4e5b9n) & MASK64;
  z ^= z >> 27n;
  z = (z * 0x94d049bb133111ebn) & MASK64;
  z ^= z >> 31n;
  return z & MASK64;
}

function deterministicSample(bytes, attempt) {
  const data = bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes);
  let state = SEED_INIT;
  let offset = 0;
  while (offset < data.length) {
    const chunkLen = Math.min(8, data.length - offset);
    const loaded = loadLittleEndian(data, offset, chunkLen);
    state = (state + GAMMA) & MASK64;
    state ^= (loaded * GAMMA) & MASK64;
    state = splitmix64(state);
    offset += chunkLen;
  }
  state = (state + GAMMA) & MASK64;
  const attemptMix = BigInt(Math.max(0, attempt) >>> 0) * ATTEMPT_MIX;
  state ^= attemptMix & MASK64;
  return splitmix64(state);
}

export class ConnectRetryPolicy {
  static DEFAULT_BASE_DELAY_MS = 5_000;
  static DEFAULT_MAX_DELAY_MS = 60_000;

  constructor(baseDelayMs = ConnectRetryPolicy.DEFAULT_BASE_DELAY_MS,
              maxDelayMs = ConnectRetryPolicy.DEFAULT_MAX_DELAY_MS) {
    this.baseDelayMs = BigInt(Math.max(0, baseDelayMs));
    this.maxDelayMs = BigInt(Math.max(0, maxDelayMs));
  }

  capMillis(attempt) {
    if (this.baseDelayMs === 0n) {
      return 0;
    }
    let cap = this.baseDelayMs;
    let remaining = Math.max(0, attempt);
    while (remaining > 0 && cap < this.maxDelayMs) {
      const doubled = cap << 1n;
      if (doubled <= cap || doubled > this.maxDelayMs) {
        cap = this.maxDelayMs;
        break;
      }
      cap = doubled;
      remaining -= 1;
    }
    if (cap > this.maxDelayMs) {
      cap = this.maxDelayMs;
    }
    return Number(cap);
  }

  delayMillis(attempt, seed) {
    const cap = BigInt(this.capMillis(attempt));
    if (cap === 0n) {
      return 0;
    }
    const span = cap + 1n;
    const sample = deterministicSample(seed, attempt);
    return Number(sample % span);
  }
}
