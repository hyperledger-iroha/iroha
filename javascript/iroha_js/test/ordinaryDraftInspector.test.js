import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { noritoEncodeInstructionBoxArchive } from "../src/norito.js";
import {
  BrowserTransactionCodecError,
  inspectCanonicalTransactionPayloadBindings,
} from "../src/transactionCodec.js";
import {
  inspectCanonicalTransactionPayloadBindings as inspectOrdinaryDraft,
} from "../src/toriiOptional.js";

const fixture = JSON.parse(readFileSync(new URL("fixtures/game-v1-codec.json", import.meta.url), "utf8"));
const compact = readFileSync(new URL("../../../fixtures/norito_rpc/iroha_compact_hash_vector.properties", import.meta.url), "utf8");
const base64 = compact.split("\n").find((line) => line.startsWith("versioned.base64=")).slice(17);
const signed = Buffer.from(base64, "base64");
assert.equal(signed.toString("base64"), base64);

function splitFields(bytes) {
  const fields = [];
  let offset = 0;
  while (offset < bytes.length) {
    let size = 0;
    let bit = 0;
    let byte;
    do {
      byte = bytes[offset++];
      size += (byte & 127) * 2 ** bit;
      bit += 7;
    } while (byte & 128);
    fields.push(bytes.subarray(offset, offset + size));
    offset += size;
  }
  assert.equal(offset, bytes.length);
  return fields;
}
function field(bytes) {
  const prefix = [];
  let size = bytes.length;
  do {
    const low = size % 128;
    size = Math.floor(size / 128);
    prefix.push(low | (size ? 128 : 0));
  } while (size);
  return Buffer.concat([Buffer.from(prefix), bytes]);
}
const payloadFields = splitFields(splitFields(signed.subarray(1))[1]);
assert.equal(payloadFields.length, 10);
// Unsigned application drafts require an absent nonce and Ordinary admission.
payloadFields[5] = Buffer.of(0);
payloadFields[7] = Buffer.alloc(4);
const ordinaryPayload = Buffer.concat(payloadFields.map(field));

test("ordinary draft inspector retains the exact common envelope bindings", () => {
  assert.deepEqual(inspectOrdinaryDraft(ordinaryPayload), inspectCanonicalTransactionPayloadBindings(ordinaryPayload));
  assert.throws(() => inspectOrdinaryDraft(Buffer.concat([ordinaryPayload, Buffer.of(0)])), /trailing/u);
});

for (const name of ["VerifyExecutionProofV1", "SettleGameSessionV1"]) {
  test(`ordinary draft inspector rejects standalone large ${name} while the general inspector preserves its execution corridor`, () => {
    const proof = structuredClone(fixture.vectors.find((row) => row.name === "ExecutionProofEnvelopeV1").value);
    proof.proof_bytes = new Array(1024 * 1024 + 1).fill(1);
    const value = name === "VerifyExecutionProofV1" ? { proof } : {
      session_id: proof.statement.session_id,
      proof,
      outcome: structuredClone(fixture.vectors.find((row) => row.name === "GameOutcomeV1").value),
    };
    const instruction = noritoEncodeInstructionBoxArchive({ [name]: value });
    const count = Buffer.alloc(8);
    count.writeBigUInt64LE(1n);
    const executable = Buffer.concat([Buffer.alloc(4), field(Buffer.concat([count, field(instruction)]))]);
    const fields = [...payloadFields];
    fields[3] = executable;
    const payload = Buffer.concat(fields.map(field));
    assert.ok(payload.length > 1024 * 1024);
    assert.ok(payload.length <= 4 * 1024 * 1024);
    // This is wire admission and binding only, not verification of the opaque proof bytes.
    assert.deepEqual(inspectCanonicalTransactionPayloadBindings(payload).executableArchive, executable);
    assert.throws(() => inspectOrdinaryDraft(payload), (error) => {
      assert.ok(error instanceof BrowserTransactionCodecError);
      assert.equal(error.code, "bounds_exceeded");
      assert.equal(error.message, "transaction payload exceeds 1048576 bytes");
      return true;
    });
  });
}
