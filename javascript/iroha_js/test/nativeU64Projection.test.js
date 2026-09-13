/** Native SDK u64 projections remain lossless and preserve canonical Norito bytes. */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  noritoEncodeInstruction,
  noritoDecodeInstruction,
  noritoEncodeInstructionBoxArchive,
  noritoDecodeInstructionBoxArchive,
  noritoEncodeGameValueV1,
  noritoDecodeGameValueV1,
} from "../src/norito.js";

const fixture = JSON.parse(readFileSync(new URL("fixtures/game-v1-codec.json", import.meta.url), "utf8"));
const row = (name) => structuredClone(fixture.vectors.find((item) => item.name === name).value);
const safe = BigInt(Number.MAX_SAFE_INTEGER);
const boundaries = [0n, 1n, safe - 1n, safe, safe + 1n, (1n << 64n) - 1n];
const projected = (number) => number <= safe ? Number(number) : number.toString();

function assertNativeRoundtrip(instruction) {
  const frame = noritoEncodeInstruction(instruction);
  const archive = noritoEncodeInstructionBoxArchive(instruction);
  assert.deepEqual(noritoDecodeInstruction(frame), instruction);
  assert.deepEqual(JSON.parse(noritoDecodeInstruction(frame, { parseJson: false })), instruction);
  assert.deepEqual(noritoDecodeInstructionBoxArchive(archive), instruction);
  assert.deepEqual(noritoEncodeInstruction(JSON.stringify(instruction)), frame);
  assert.deepEqual(noritoEncodeInstructionBoxArchive(noritoDecodeInstructionBoxArchive(archive)), archive);
}

function gameExamples(value) {
  const checkpoint = row("SignedGameCheckpointV1");
  const session_id = checkpoint.checkpoint.session_id;
  const signatures = checkpoint.signatures;
  checkpoint.checkpoint.epoch = value;
  const frontier = { ...row("GameCommitmentSetBodyV1"), signatures, epoch: value };
  return [
    ["OpenGameSessionV1", { ...row("OpenGameSessionV1"), join_deadline_height: value }],
    ["CommitGameCheckpointV1", { session_id, checkpoint, frontier }],
    ["ChallengeGameSessionV1", { session_id, epoch: value, slot: 0, signature: signatures[0].signature }],
    ["CommitGameInputsV1", { input: { session_id, epoch: value, start_tick: 0, slot: 0, commitment: frontier.commitments[0], signature: signatures[0].signature } }],
    ["RevealGameInputsV1", { reveal: { ...row("GameInputRevealV1"), epoch: value } }],
  ];
}

function verifyingKey(name, height) {
  return {
    verifying_keys: {
      [name]: {
        id: { backend: "stark/fri-v1", name: "execution-v1" },
        record: {
          version: 7,
          circuit_id: "ivm-execution-v1",
          owner_manifest_id: null,
          namespace: "core",
          backend: "stark",
          curve: "goldilocks",
          public_inputs_schema_hash: Array(32).fill(0x11),
          commitment: Array(32).fill(0x22),
          vk_len: 0,
          max_proof_bytes: 1_000_000,
          gas_schedule_id: null,
          metadata_uri_cid: null,
          vk_bytes_cid: null,
          activation_height: height,
          withdraw_height: height,
          key: null,
          status: "Active",
        },
      },
    },
  };
}

test("native Game u64 values match public value codecs at all safe-integer boundaries", () => {
  for (const number of boundaries) {
    for (const [name, payload] of gameExamples(projected(number))) {
      assertNativeRoundtrip({ [name]: payload });
      const bytes = noritoEncodeGameValueV1(name, payload);
      assert.deepEqual(noritoDecodeGameValueV1(name, bytes), payload);
      assert.deepEqual(noritoEncodeGameValueV1(name, noritoDecodeGameValueV1(name, bytes)), bytes);
    }
  }
  // Optional u64 fields use the same recursive projection, including explicit null.
  for (const number of [...boundaries, null]) {
    const height = number === null ? null : projected(number);
    // GameSessionEventV1 has no unrelated participant or equipment payload here.
    const event = {
      session_id: row("OpenGameSessionV1").session_id,
      revision: 0,
      phase: 0,
      dispute_root: row("ExecutionProofEnvelopeV1").statement.dispute_root,
      payout_claims: [],
      item_stakes: [],
      resources: [],
      terminal_at_height: height,
    };
    const bytes = noritoEncodeGameValueV1("GameSessionEventV1", event);
    assert.deepEqual(noritoDecodeGameValueV1("GameSessionEventV1", bytes), event);
  }
});

test("native verifying-key optional heights retain full u64 without unsafe parsed numbers", () => {
  for (const name of ["RegisterVerifyingKey", "UpdateVerifyingKey"]) {
    for (const number of [...boundaries, null]) {
      assertNativeRoundtrip(verifyingKey(name, number === null ? null : projected(number)));
    }
  }
});

test("native Game and verifying-key JSON reject integer aliases and overflow", () => {
  const rejected = ["0", "9007199254740991", "09007199254740992", "+9007199254740992", "9007199254740992 ", "18446744073709551616", Number.MAX_SAFE_INTEGER + 1];
  for (const value of rejected) {
    const instructions = [
      ...gameExamples(value).map(([name, payload]) => ({ [name]: payload })),
      verifyingKey("RegisterVerifyingKey", value),
      verifyingKey("UpdateVerifyingKey", value),
    ];
    for (const instruction of instructions) {
      assert.throws(() => noritoEncodeInstruction(instruction));
      assert.throws(() => noritoEncodeInstruction(JSON.stringify(instruction)));
      assert.throws(() => noritoEncodeInstructionBoxArchive(instruction));
    }
  }
});

test("native execution instruction codecs reject an empty proof payload", () => {
  const proof = row("ExecutionProofEnvelopeV1");
  proof.proof_bytes = [];
  const instructions = [
    { VerifyExecutionProofV1: { proof } },
    { SettleGameSessionV1: { session_id: proof.statement.session_id, proof, outcome: row("GameOutcomeV1") } },
  ];
  for (const instruction of instructions) {
    assert.throws(() => noritoEncodeInstruction(instruction), /proof_bytes must not be empty/u);
    assert.throws(() => noritoEncodeInstructionBoxArchive(instruction), /proof_bytes must not be empty/u);
  }
});
