import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { ed25519 } from "@noble/curves/ed25519";
import { NetworkId } from "../src/networkId.js";
import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import { encodeRaceValueV1, decodeRaceValueV1, buildRaceInstructionV1, raceGameplayHashV1 } from "../src/race.js";
import { noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive } from "../src/norito.js";
import { buildBrowserInstructionTransactionPayload, validateBrowserInstructionTransactionSignable, finalizeBrowserInstructionTransaction, browserSignedTransactionHashHex, browserTransactionPayloadHashHex } from "../src/transactionCodec.js";

const fixture = JSON.parse(fs.readFileSync(new URL("fixtures/race-v1-codec.json", import.meta.url), "utf8"));
const networkId = NetworkId.parse(fixture.network_id);
for (const row of fixture.vectors) test(`native ${row.name} canonical fixture and gameplay digest`, () => {
  const bytes = encodeRaceValueV1(row.name, row.value);
  assert.equal(Buffer.from(bytes).toString("hex").toUpperCase(), row.encoded_hex);
  assert.deepEqual(encodeRaceValueV1(row.name, decodeRaceValueV1(row.name, bytes)), bytes);
  if (row.domain) assert.equal(Buffer.from(raceGameplayHashV1(networkId, row.domain, row.value)).toString("hex").toUpperCase(), row.gameplay_digest_hex);
  assert.throws(() => decodeRaceValueV1(row.name, Buffer.concat([bytes, Buffer.of(0)])));
  assert.throws(() => encodeRaceValueV1(row.name, { ...row.value, extra: 1 }), /unknown fields/);
});

test("native OpenRace browser transaction signs local canonical bytes and verifies the wallet signature", () => {
  const row = fixture.vectors.find((value) => value.name === "OpenRaceV1");
  const instruction = buildRaceInstructionV1(row.name, row.value);
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

test("native racing fails closed on malformed controls, enum tags, slots and signatures", () => {
  const reveal = fixture.vectors.find((value) => value.name === "RaceInputRevealV1").value;
  assert.throws(() => encodeRaceValueV1("RaceInputRevealV1", { ...reveal, controls: [64, 0, 0, 0, 0, 0] }), /undefined control bits/);
  assert.throws(() => encodeRaceValueV1("RaceInputRevealV1", { ...reveal, slot: 8 }), /slot/);
  assert.throws(() => encodeRaceValueV1("RaceInputRevealV1", { ...reveal, epoch: "01" }), /canonical integer/);
  assert.throws(() => encodeRaceValueV1("RaceRulesV1", { version: 1, track: { kind: "custom" }, max_racers: 2 }), /compiled track/);
  assert.throws(() => encodeRaceValueV1("RaceSlotSignatureV1", { slot: 0, signature: "11" }), /bounded/);
  assert.throws(() => buildRaceInstructionV1("CustomRace", {}), /unknown/);
});
