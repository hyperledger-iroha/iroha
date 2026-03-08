import { Buffer } from "node:buffer";
import { readFileSync } from "node:fs";
import { dirname, resolve as resolvePath } from "node:path";
import { fileURLToPath } from "node:url";

const MODULE_DIR = dirname(fileURLToPath(import.meta.url));
const POSEIDON_FIXTURE_PATH = resolvePath(
  MODULE_DIR,
  "../../../crates/iroha_data_model/tests/fixtures/axt_poseidon_constants.json",
);
const poseidonConstants = JSON.parse(readFileSync(POSEIDON_FIXTURE_PATH, "utf8"));

const FIELD_MODULUS = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const FULL_ROUNDS = 8;
const PARTIAL_ROUNDS = 56;
const RATE = 2;
const AXT_PREFIX = Buffer.from("iroha:axt:desc:v1\u0000", "utf8");

function leHexToBigInt(hex) {
  const buf = Buffer.from(hex, "hex");
  let acc = 0n;
  for (let i = buf.length - 1; i >= 0; i -= 1) {
    acc = (acc << 8n) + BigInt(buf[i]);
  }
  return acc % FIELD_MODULUS;
}

function normalizeParams(fixture) {
  const roundConstants = fixture.round_constants.map((round) => round.map(leHexToBigInt));
  const mds = fixture.mds.map((row) => row.map(leHexToBigInt));
  return { roundConstants, mds };
}

const POSEIDON_WIDTH3 = normalizeParams(poseidonConstants.width3);

function addMod(a, b) {
  const sum = a + b;
  return sum >= FIELD_MODULUS ? sum - FIELD_MODULUS : sum;
}

function mulMod(a, b) {
  return (a * b) % FIELD_MODULUS;
}

function sbox(x) {
  const x2 = mulMod(x, x);
  const x4 = mulMod(x2, x2);
  return mulMod(x4, x);
}

function applyMds(state, mds) {
  const width = state.length;
  const next = new Array(width).fill(0n);
  for (let row = 0; row < width; row += 1) {
    let acc = 0n;
    for (let col = 0; col < width; col += 1) {
      acc = addMod(acc, mulMod(mds[row][col], state[col]));
    }
    next[row] = acc;
  }
  for (let i = 0; i < width; i += 1) {
    state[i] = next[i];
  }
}

function poseidonPermute(state, params) {
  const width = state.length;
  const { roundConstants, mds } = params;
  let idx = 0;

  for (let round = 0; round < FULL_ROUNDS / 2; round += 1, idx += 1) {
    for (let lane = 0; lane < width; lane += 1) {
      state[lane] = sbox(addMod(state[lane], roundConstants[idx][lane]));
    }
    applyMds(state, mds);
  }

  for (let round = 0; round < PARTIAL_ROUNDS; round += 1, idx += 1) {
    for (let lane = 0; lane < width; lane += 1) {
      state[lane] = addMod(state[lane], roundConstants[idx][lane]);
    }
    state[0] = sbox(state[0]);
    applyMds(state, mds);
  }

  for (let round = 0; round < FULL_ROUNDS / 2; round += 1, idx += 1) {
    for (let lane = 0; lane < width; lane += 1) {
      state[lane] = sbox(addMod(state[lane], roundConstants[idx][lane]));
    }
    applyMds(state, mds);
  }
}

function packBytesToFields(bytes) {
  if (bytes.length === 0) {
    return [];
  }
  const fields = [];
  for (let offset = 0; offset < bytes.length; offset += 8) {
    const chunk = bytes.subarray(offset, Math.min(offset + 8, bytes.length));
    let limb = 0n;
    for (let i = chunk.length - 1; i >= 0; i -= 1) {
      limb = (limb << 8n) + BigInt(chunk[i]);
    }
    fields.push(limb % FIELD_MODULUS);
  }
  return fields;
}

function hashWords(words) {
  const padded = [...words, 1n];
  while (padded.length % RATE !== 0) {
    padded.push(0n);
  }

  const state = [0n, 0n, 0n];
  for (let offset = 0; offset < padded.length; offset += RATE) {
    for (let lane = 0; lane < RATE; lane += 1) {
      state[lane] = addMod(state[lane], padded[offset + lane]);
    }
    poseidonPermute(state, POSEIDON_WIDTH3);
  }
  return state[0];
}

function fieldToLittleEndianBytes(field) {
  let value = field % FIELD_MODULUS;
  const out = Buffer.alloc(32);
  for (let i = 0; i < 32; i += 1) {
    out[i] = Number(value & 0xffn);
    value >>= 8n;
  }
  return out;
}

/**
 * Hash arbitrary bytes with the Poseidon2 permutation (rate 2, width 3).
 * @param {Buffer | Uint8Array | ArrayBuffer} input
 * @returns {Buffer}
 */
export function poseidonHashBytes(input) {
  const bytes = Buffer.from(input ?? []);
  const words = packBytesToFields(bytes);
  const digest = hashWords(words);
  return fieldToLittleEndianBytes(digest);
}

/**
 * Compute the AXT descriptor binding from Norito-encoded descriptor bytes.
 * @param {Buffer | Uint8Array | ArrayBuffer} descriptorBytes
 * @returns {Buffer}
 */
export function computeAxtBindingFromNorito(descriptorBytes) {
  const encoded = Buffer.concat([AXT_PREFIX, Buffer.from(descriptorBytes ?? [])]);
  return poseidonHashBytes(encoded);
}
