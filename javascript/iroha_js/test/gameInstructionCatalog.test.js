/** Canonical native instruction dispatch stays separate from public application value catalogs. */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive,
  noritoEncodeGameValueV1, noritoDecodeGameValueV1,
} from "../src/norito.js";
import {
  encodeGameResourceValueV1, decodeGameResourceValueV1,
  GAME_RESOURCE_MAX_NFT_ID_BYTES_V1,
} from "../src/noritoGameResourceCodecs.js";

const fixture = JSON.parse(readFileSync(new URL("fixtures/game-v1-codec.json", import.meta.url), "utf8"));
const resources = JSON.parse(readFileSync(new URL("fixtures/game-resource-v1-codec.json", import.meta.url), "utf8"));
const row = name => fixture.vectors.find(item => item.name === name);
const checkpoint = row("SignedGameCheckpointV1").value;
const session_id = checkpoint.checkpoint.session_id;
const signature = checkpoint.signatures[0].signature;
const commitment = row("GameCommitmentSetBodyV1").value;
const proof = row("ExecutionProofEnvelopeV1").value;

// These composite values reuse native fixture fields. The two complete existing
// native instruction frames below remain the independent byte oracle; these
// extra combinations exercise the remaining account-free dispatch closure.
const examples = {
  OpenGameSessionV1: row("OpenGameSessionV1").value,
  StartGameSessionV1: { session_id },
  CommitGameCheckpointV1: {
    session_id, checkpoint,
    frontier: { ...commitment, signatures: checkpoint.signatures },
  },
  ChallengeGameSessionV1: { session_id, epoch: 2, slot: 0, signature },
  CommitGameInputsV1: { input: {
    session_id, epoch: 2, start_tick: 0, slot: 0,
    commitment: commitment.commitments[0], signature,
  } },
  RevealGameInputsV1: { reveal: row("GameInputRevealV1").value },
  AdvanceGameDeadlineV1: { session_id },
  SettleGameSessionV1: { session_id, proof, outcome: row("GameOutcomeV1").value },
  ExpireGameSessionV1: { session_id },
  StakeGameItemV1: row("StakeGameItemV1").value,
  RegisterExecutionProofProfileV1: { profile_id: proof.profile_id },
  VerifyExecutionProofV1: { proof },
};

for (const [name, value] of Object.entries(examples)) {
  test(`native instruction catalog ${name} preserves exact public value bytes`, () => {
    const input = structuredClone(value), snapshot = structuredClone(input);
    const bare = noritoEncodeGameValueV1(name, input);
    const wire = noritoEncodeInstructionBoxArchive({ [name]: input });
    const decoded = noritoDecodeInstructionBoxArchive(wire);
    assert.deepEqual(decoded[name], noritoDecodeGameValueV1(name, bare));
    assert.deepEqual(noritoEncodeGameValueV1(name, decoded[name]), bare);
    assert.deepEqual(noritoEncodeInstructionBoxArchive(decoded), wire);
    const native = row(name);
    if (native) {
      assert.equal(bare.toString("hex").toUpperCase(), native.encoded_hex);
      assert.ok(wire.includes(Buffer.from(native.framed_hex, "hex")), "archive retains the exact native inner frame");
    }
    assert.deepEqual(input, snapshot);
    assert.throws(() => noritoEncodeInstructionBoxArchive({ [name]: { ...input, extra: 1 } }), /missing or unknown fields/);
  });
}

test("native checkpoint frontier preserves ordered signatures and shared field error context", () => {
  for (const field of ["checkpoint", "frontier"]) {
    const value = structuredClone(examples.CommitGameCheckpointV1);
    value[field].signatures.reverse();
    const expected = { name: "RangeError", message: "Race signatures must be unique and ordered by slot" };
    assert.throws(() => noritoEncodeGameValueV1("CommitGameCheckpointV1", value), expected);
    assert.throws(() => noritoEncodeInstructionBoxArchive({ CommitGameCheckpointV1: value }), expected);
  }
  const value = structuredClone(examples.RevealGameInputsV1);
  value.reveal.epoch = "01";
  const expected = { name: "TypeError", message: "RevealGameInputsV1.reveal.epoch must be a canonical integer" };
  assert.throws(() => noritoEncodeGameValueV1("RevealGameInputsV1", value), expected);
  assert.throws(() => noritoEncodeInstructionBoxArchive({ RevealGameInputsV1: value }), expected);
});

test("public resource facade retains independent native clause bytes and strict fields", () => {
  for (const name of ["GameResourceReservationClauseV1", "GameResourceRequirementV1"]) {
    const native = resources.vectors.find(item => item.name === name);
    const bytes = encodeGameResourceValueV1(name, native.value);
    assert.equal(bytes.toString("hex").toUpperCase(), native.encoded_hex);
    assert.deepEqual(decodeGameResourceValueV1(name, bytes), native.value);
    assert.throws(() => encodeGameResourceValueV1(name, { ...native.value, extra: 1 }), /exact fields/);
    let reads = 0;
    const getter = { ...native.value };
    Object.defineProperty(getter, "nft_id", { enumerable: true, get() { reads++; return native.value.nft_id; } });
    assert.throws(() => encodeGameResourceValueV1(name, getter), /enumerable data fields/);
    assert.equal(reads, 0);
    for (const nft_id of ["x".repeat(GAME_RESOURCE_MAX_NFT_ID_BYTES_V1 + 1), "é".repeat(257)]) {
      assert.throws(() => encodeGameResourceValueV1(name, { ...native.value, nft_id }), /string byte bound/);
    }
    assert.throws(() => decodeGameResourceValueV1(name, Buffer.concat([bytes, Buffer.of(0)])), /trailing/);
  }
});

test("resource primitive callbacks retain exact integer syntax before account decoding", () => {
  const record = resources.vectors.find(item => item.name === "GameResourceReservationRecordV1").value;
  for (const slot of [256, -1, 1.5, "01", "-0", "1".repeat(21)]) {
    assert.throws(() => encodeGameResourceValueV1("GameResourceReservationRecordV1", { ...record, slot }),
      /canonical integer|exceeds/);
  }
  const set = resources.vectors.find(item => item.name === "GameResourceReservationSetV1").value;
  for (const version of ["01", "-0", "1".repeat(21), 65536]) {
    assert.throws(() => encodeGameResourceValueV1("GameResourceReservationSetV1", { ...set, version }),
      /unsigned integer|exceeds/);
  }
  assert.throws(() => encodeGameResourceValueV1("unknown", {}), /unknown game resource value/);
  assert.throws(() => decodeGameResourceValueV1("unknown", Buffer.alloc(0)), /unknown game resource value/);
});
