import assert from "node:assert/strict";
import fs from "node:fs/promises";

import { OperatorSigningContext } from "../src/toriiClient.js";
import { signEd25519 } from "../src/crypto.js";

const TEST_OPERATOR_PRIVATE_KEY = Buffer.alloc(32, 0x0b);
const TEST_OPERATOR_PUBLIC_KEY =
  "ed012066BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A";

export function makeTestOperatorSigningContext(networkId) {
  return new OperatorSigningContext(networkId, {
    publicKey: TEST_OPERATOR_PUBLIC_KEY,
    sign: (message) => signEd25519(message, TEST_OPERATOR_PRIVATE_KEY),
  });
}

export async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

export function cloneFixture(value) {
  return JSON.parse(JSON.stringify(value));
}

export function createSseResponse(chunks) {
  const body = {
    async *[Symbol.asyncIterator]() {
      const encoder = new TextEncoder();
      for (const chunk of chunks) {
        yield encoder.encode(chunk);
      }
    },
  };
  return {
    status: 200,
    headers: {
      get(name) {
        return name.toLowerCase() === "content-type" ? "text/event-stream" : null;
      },
    },
    body,
  };
}

export function readU64Length(buffer, offset, label) {
  assert.ok(offset + 8 <= buffer.length, `${label} length prefix is in bounds`);
  const value = buffer.readBigUInt64LE(offset);
  assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} length fits JS number`);
  return { length: Number(value), bytes: 8 };
}

export function readCompactLength(buffer, offset, label) {
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  for (; cursor < buffer.length; cursor += 1) {
    const byte = buffer[cursor];
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} length fits JS number`);
      return { length: Number(value), bytes: cursor + 1 - offset };
    }
    shift += 7n;
  }
  assert.fail(`${label} compact length prefix is unterminated`);
}

export function readNoritoFieldPayload(buffer, offset, label, compactLength) {
  const { length, bytes } = compactLength
    ? readCompactLength(buffer, offset, label)
    : readU64Length(buffer, offset, label);
  const start = offset + bytes;
  const end = start + length;
  assert.ok(end <= buffer.length, `${label} payload is in bounds`);
  return { payload: buffer.subarray(start, end), offset: end };
}

export function noritoFramePayload(body, label) {
  const buffer = Buffer.from(body);
  assert.equal(buffer.subarray(0, 4).toString("ascii"), "NRT0");
  const { length: payloadLength } = readU64Length(buffer, 23, `${label}.payloadLength`);
  assert.equal(buffer.length, 40 + payloadLength);
  return {
    flags: buffer[39],
    payload: buffer.subarray(40),
  };
}

export function assertFlattenedAliasSelector(body, alias, label) {
  const { flags, payload } = noritoFramePayload(body, label);
  const compactLength = (flags & 0x02) !== 0;
  const multisigAccountId = readNoritoFieldPayload(
    payload,
    0,
    `${label}.multisig_account_id`,
    compactLength,
  );
  assert.deepEqual([...multisigAccountId.payload], [0]);
  const multisigAccountAlias = readNoritoFieldPayload(
    payload,
    multisigAccountId.offset,
    `${label}.multisig_account_alias`,
    compactLength,
  );
  assert.equal(multisigAccountAlias.payload[0], 1);
  const aliasOption = readNoritoFieldPayload(
    multisigAccountAlias.payload,
    1,
    `${label}.multisig_account_alias.option`,
    compactLength,
  );
  const aliasString = readNoritoFieldPayload(
    aliasOption.payload,
    0,
    `${label}.multisig_account_alias.string`,
    compactLength,
  );
  assert.equal(aliasString.payload.toString("utf8"), alias);
}

export async function withEnv(overrides, fn) {
  const original = {};
  for (const [key, value] of Object.entries(overrides)) {
    original[key] = process.env[key];
    if (value === null || value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
  }
  try {
    await fn();
  } finally {
    for (const [key, value] of Object.entries(original)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  }
}
