import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { ed25519 } from "@noble/curves/ed25519";
import { NetworkId } from "../src/networkId.js";
import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import { encodeGameValueV1, decodeGameValueV1, buildGameInstructionV1, buildJoinGameSessionV1, gameMessageHashV1, EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 } from "../src/game.js";
import { noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive, validateNoritoFrame } from "../src/norito.js";
import { buildBrowserInstructionTransactionPayload, validateBrowserInstructionTransactionSignable, finalizeBrowserInstructionTransaction, browserSignedTransactionHashHex, browserTransactionPayloadHashHex } from "../src/transactionCodec.js";

const fixture = JSON.parse(fs.readFileSync(new URL("fixtures/game-v1-codec.json", import.meta.url), "utf8"));
const networkId = NetworkId.parse(fixture.network_id);
for (const row of fixture.vectors) test(`native ${row.name} canonical fixture and gameplay digest`, () => {
  const bytes = encodeGameValueV1(row.name, row.value);
  assert.equal(Buffer.from(bytes).toString("hex").toUpperCase(), row.encoded_hex);
  const framed = Buffer.from(row.framed_hex, "hex");
  const frame = validateNoritoFrame(framed, { expectedPaddingLength: 0, requireNonEmptyPayload: true });
  assert.equal(frame.flags, 2, "native compact-length framing");
  assert.deepEqual(frame.payload, Buffer.from(bytes), "native framed payload equals canonical SDK bare bytes");
  assert.throws(() => decodeGameValueV1(row.name, framed), "bare codec must reject framed ingress");
  const badCrc = Buffer.from(framed); badCrc[31] ^= 1;
  assert.throws(() => validateNoritoFrame(badCrc), /CRC|checksum/i);
  const badSchema = Buffer.from(framed); badSchema[6] ^= 1;
  assert.throws(() => validateNoritoFrame(badSchema, { expectedSchemaHash: frame.schemaHash }), /schema/);
  assert.deepEqual(encodeGameValueV1(row.name, decodeGameValueV1(row.name, bytes)), bytes);
  if (row.domain) assert.equal(Buffer.from(gameMessageHashV1(networkId, row.domain, row.value)).toString("hex").toUpperCase(), row.gameplay_digest_hex);
  assert.throws(() => decodeGameValueV1(row.name, Buffer.concat([bytes, Buffer.of(0)])));
  assert.throws(() => encodeGameValueV1(row.name, { ...row.value, extra: 1 }), /unknown fields/);
});

test("native OpenGameSession browser transaction signs local canonical bytes and verifies the wallet signature", () => {
  const row = fixture.vectors.find((value) => value.name === "OpenGameSessionV1");
  const instruction = buildGameInstructionV1(row.name, row.value);
  const wire = noritoEncodeInstructionBoxArchive(instruction);
  assert.deepEqual(noritoDecodeInstructionBoxArchive(wire), instruction);
  const privateKey = new Uint8Array(32).fill(7), publicKey = ed25519.getPublicKey(privateKey);
  const authority = AccountAddress.fromAccount({ algorithm: "ed25519", publicKey }).toI105();
  const payloadBytes = buildBrowserInstructionTransactionPayload({ networkId, authority, instructions: [instruction], feePayment: { payer: "authority", chargeLimits: [{ kind: "nexus", assetDefinitionId: row.value.asset_definition, maxAmount: "1" }] }, creationTimeMs: 1, ttlMs: 100_000 });
  const signable = { networkId, authority, signingPublicKey: publicKey, payloadBytes, payloadHashHex: browserTransactionPayloadHashHex(payloadBytes) };
  validateBrowserInstructionTransactionSignable(signable);
  const hash = Uint8Array.from(blake2b256(payloadBytes)); hash[31] |= 1;
  const signature = ed25519.sign(hash, privateKey);
  const finalized = finalizeBrowserInstructionTransaction(signable, signature, publicKey);
  assert.equal(browserSignedTransactionHashHex(finalized.signedTransaction), finalized.hashHex);
  const badSignature = signature.slice(); badSignature[0] ^= 1;
  assert.throws(() => finalizeBrowserInstructionTransaction(signable, badSignature, publicKey), /signature/i);
});

test("join approval requires exact debit terms and altered terms invalidate the wallet signature", () => {
  const value = fixture.vectors.find(row => row.name === "JoinGameSessionV1").value;
  const open = fixture.vectors.find(row => row.name === "OpenGameSessionV1").value;
  const key = new Uint8Array(32).fill(7), publicKey = ed25519.getPublicKey(key);
  const authority = AccountAddress.fromAccount({ algorithm: "ed25519", publicKey }).toI105();
  const payload = terms => buildBrowserInstructionTransactionPayload({ networkId, authority,
    instructions: [buildJoinGameSessionV1(terms)], feePayment: { payer: "authority", chargeLimits: [{ kind: "nexus", assetDefinitionId: open.asset_definition, maxAmount: "0.25" }] }, creationTimeMs: 1, ttlMs: 100_000 });
  const original = payload(value), digest = Uint8Array.from(blake2b256(original)); digest[31] |= 1;
  const signature = ed25519.sign(digest, key);
  assert.equal(ed25519.verify(signature, digest, publicKey), true);
  for (const change of [{ expected_stake: "1000" }, { expected_manifest_hash: open.manifest.profile_id }]) {
    const altered = payload({ ...value, ...change });
    assert.notDeepEqual(altered, original);
    const changedDigest = Uint8Array.from(blake2b256(altered)); changedDigest[31] |= 1;
    assert.equal(ed25519.verify(signature, changedDigest, publicKey), false);
  }
  for (const name of ["expected_manifest_hash", "expected_asset_definition", "expected_stake"]) {
    const missing = { ...value }; delete missing[name];
    assert.throws(() => buildJoinGameSessionV1(missing));
  }
  // The old undeployed four-field encoding must not become an unbounded debit fallback.
  const legacy = Buffer.from("2001010101010101010101010101010101010101010101010101010101010101014A21000000000000000100018101390177010E01A8017D0117015F015601A30154016601C3014C017E01CC01CB018D018A019101B401EE013701A2015D01F6010F015B018F01C901B30194090100000000000000000100", "hex");
  assert.throws(() => decodeGameValueV1("JoinGameSessionV1", legacy));
});

test("generic sessions fail closed on malformed opaque bytes, policies and signatures", () => {
  const reveal = fixture.vectors.find((value) => value.name === "GameInputRevealV1").value;
  assert.throws(() => encodeGameValueV1("GameInputRevealV1", { ...reveal, payload: [256] }), /bytes/);
  assert.throws(() => encodeGameValueV1("GameInputRevealV1", { ...reveal, slot: 256 }), /u8/);
  assert.throws(() => encodeGameValueV1("GameInputRevealV1", { ...reveal, epoch: "01" }), /canonical integer/);
  assert.throws(() => encodeGameValueV1("GameSlotSignatureV1", { slot: 0, signature: "11" }), /bounded/);
  assert.throws(() => buildGameInstructionV1("OpenRaceV1", {}), /unknown/);
  const manifest = fixture.vectors.find(value => value.name === "GameManifestV1").value;
  assert.throws(() => encodeGameValueV1("GameManifestV1", { ...manifest, payout_policy: { kind: "operator_decides" } }), /payout/);

});

test("native u8 slot vectors stay packed and reject legacy element-length encodings", () => {
  // Native RaceDnfEventV1 fixture: struct field length, u64 count, then raw u8s.
  const packed = Buffer.from("04060000000A02000000000000000001", "hex");
  const value = { tick: 6, slots: [0, 1] };
  for (const name of ["GameDnfEventV1", "RaceDnfEventV1"]) {
    assert.deepEqual(Buffer.from(encodeGameValueV1(name, value)), packed);
    assert.deepEqual(decodeGameValueV1(name, packed), value);
    const legacy = Buffer.from("04060000000C020000000000000001000101", "hex");
    assert.throws(() => decodeGameValueV1(name, legacy));
  }
  assert.throws(() => encodeGameValueV1("RaceDnfEventV1", { tick: 6, slots: [8] }));
  assert.throws(() => decodeGameValueV1("RaceDnfEventV1", Buffer.from("040600000009010000000000000008", "hex")));
  assert.throws(() => encodeGameValueV1("GameDnfEventV1", { tick: 6, slots: [256] }));
});

test("execution envelopes and typed ISIs enforce the complete four MiB bound", () => {
  const source = fixture.vectors.find(row => row.name === "ExecutionProofEnvelopeV1").value;
  const envelope = { ...source, proof_bytes: Array(3 * 1024 * 1024).fill(255) };
  const encoded = encodeGameValueV1("ExecutionProofEnvelopeV1", envelope);
  assert.ok(encoded.length > 1024 * 1024 && encoded.length < EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1);
  assert.equal(decodeGameValueV1("ExecutionProofEnvelopeV1", encoded).proof_bytes.length, envelope.proof_bytes.length);
  assert.throws(() => encodeGameValueV1("ExecutionProofEnvelopeV1", { ...source, proof_bytes: Array(EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1).fill(0) }), /compiled payload limit/);
  assert.throws(() => decodeGameValueV1("ExecutionProofEnvelopeV1", Buffer.alloc(EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1 + 1)), /compiled payload limit/);
  assert.throws(() => encodeGameValueV1("OpaqueBytes", Array(512 * 1024 + 1).fill(0)), /bounded/);
});

test("large execution transaction builds, signs, validates and hashes without widening ordinary or mixed transactions", () => {
  const source = fixture.vectors.find(row => row.name === "ExecutionProofEnvelopeV1").value;
  const proof = { ...source, proof_bytes: Array(1024 * 1024 + 1).fill(255) };
  const instruction = buildGameInstructionV1("VerifyExecutionProofV1", { proof });
  const privateKey = new Uint8Array(32).fill(7), publicKey = ed25519.getPublicKey(privateKey);
  const authority = AccountAddress.fromAccount({ algorithm: "ed25519", publicKey }).toI105();
  const open = fixture.vectors.find(row => row.name === "OpenGameSessionV1").value;
  const input = { networkId, authority, instructions: [instruction], feePayment: { payer: "authority", chargeLimits: [{ kind: "nexus", assetDefinitionId: open.asset_definition, maxAmount: "1" }] }, creationTimeMs: 1, ttlMs: 100_000 };
  const payloadBytes = buildBrowserInstructionTransactionPayload(input);
  assert.ok(payloadBytes.length > 1024 * 1024);
  const signable = { networkId, authority, signingPublicKey: publicKey, payloadBytes, payloadHashHex: browserTransactionPayloadHashHex(payloadBytes) };
  validateBrowserInstructionTransactionSignable(signable);
  const hash = Uint8Array.from(blake2b256(payloadBytes)); hash[31] |= 1;
  const finalized = finalizeBrowserInstructionTransaction(signable, ed25519.sign(hash, privateKey), publicKey);
  assert.equal(browserSignedTransactionHashHex(finalized.signedTransaction), finalized.hashHex);
  assert.throws(() => buildBrowserInstructionTransactionPayload({ ...input, instructions: [instruction, buildGameInstructionV1("StartGameSessionV1", { session_id: source.statement.session_id })] }), /one native execution-proof/);
  const altered = Buffer.from(payloadBytes);
  const id = Buffer.from("iroha.instruction.v1::game::VerifyExecutionProofV1");
  const offset = altered.indexOf(id); assert.ok(offset > 0); altered[offset] = 120;
  assert.throws(() => browserTransactionPayloadHashHex(altered), /ordinary transaction payload/);
});
