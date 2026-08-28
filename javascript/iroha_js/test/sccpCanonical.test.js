import assert from "node:assert/strict";
import test from "node:test";

import { keccak_256 } from "@noble/hashes/sha3";

import {
  SCCP_CODEC_CANONICAL_TEXT,
  SCCP_CODEC_EVM_ADDRESS20,
  SCCP_DOMAIN_BSC,
  SCCP_DOMAIN_SORA,
  SCCP_NETWORK_PROFILES,
  canonicalSccpHubCommitmentBytes,
  canonicalSccpMerkleProofBytes,
  canonicalSccpMessagePublicInputsBytes,
  canonicalSccpPayloadBytes,
  canonicalSccpTransferPayloadBytes,
  canonicalTairaSccpMessageBundleBytes,
  normalizeSccpCodecValue,
  normalizeSccpMessageBundle,
  parseSccpJsonObject,
  sccpCommitmentLeafHash,
  sccpHubCommitmentFromPayload,
  sccpLaneIdHash,
  sccpMerkleRootFromCommitment,
  sccpMessageId,
  sccpPayloadHash,
} from "../src/sccp.js";

const network = (name) => ({ network: name, profile: null });
const BSC_LANE = Object.freeze({
  source: network("sora_taira"),
  target: network("bsc_mainnet"),
});
const HASH_22 = `0x${"22".repeat(32)}`;
const HASH_33 = `0x${"33".repeat(32)}`;
const HASH_44 = `0x${"44".repeat(32)}`;
const HASH_AA = `0x${"aa".repeat(32)}`;
const HASH_AB = `0x${"ab".repeat(32)}`;

const TRANSFER = Object.freeze({
  version: 1,
  source_domain: SCCP_DOMAIN_SORA,
  dest_domain: SCCP_DOMAIN_BSC,
  nonce: "7",
  route_revision: 1,
  asset_home_domain: SCCP_DOMAIN_SORA,
  asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
  asset_id: "0x786f72",
  amount: "11",
  sender_codec: SCCP_CODEC_CANONICAL_TEXT,
  sender: "0x616c696365407461697261",
  recipient_codec: SCCP_CODEC_EVM_ADDRESS20,
  recipient: `0x${"11".repeat(20)}`,
  route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
  route_id: "0x74616972615f6273635f786f72",
});
const PAYLOAD = Object.freeze({ Transfer: TRANSFER });
const CONTEXT = Object.freeze({
  lane: BSC_LANE,
  destination_binding_hash: HASH_22,
  route_configuration_hash: HASH_33,
});

const GOLDEN_TRANSFER_HEX =
  "010000000002000000070000000000000001000000000000000103000000786f72" +
  "0b000000000000000000000000000000010b000000616c69636540746169726102" +
  "140000001111111111111111111111111111111111111111010d00000074616972615f6273635f786f72";
const GOLDEN_PAYLOAD_HEX = `02${GOLDEN_TRANSFER_HEX}`;
const GOLDEN_LANE_HASH =
  "0xf71bfe17ca31ff2c0396f328327fbbcb052af40588b777860341328d146ab00e";
const GOLDEN_MESSAGE_ID =
  "0x03feec37ab66cb47cf04b2aab7a06c6f15e3e0dd16c50ba38ba0c654a3691917";
const GOLDEN_PAYLOAD_HASH =
  "0xe972db5f05760e959c89c940500f01c068816ac91ed717c7d1e3e2fb437cacfa";
const GOLDEN_COMMITMENT_HEX =
  "01054042" +
  "22".repeat(32) +
  "33".repeat(32) +
  GOLDEN_MESSAGE_ID.slice(2) +
  GOLDEN_PAYLOAD_HASH.slice(2);
const GOLDEN_ROOT =
  "0x27f040864c87b1fe7162c9e8e15a555123121933827bd4ff3d45d59706307de6";
const GOLDEN_PUBLIC_INPUTS_HEX =
  "01" +
  GOLDEN_MESSAGE_ID.slice(2) +
  GOLDEN_PAYLOAD_HASH.slice(2) +
  "02000000" +
  GOLDEN_ROOT.slice(2) +
  "0900000000000000" +
  "44".repeat(32);

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const clone = (value) => structuredClone(value);
const commitment = () => sccpHubCommitmentFromPayload(CONTEXT, PAYLOAD);
const bundle = (overrides = {}) => ({
  version: 1,
  commitment_root: GOLDEN_ROOT,
  commitment: commitment(),
  merkle_proof: { steps: [] },
  payload: PAYLOAD,
  finality_proof: "0x01",
  ...overrides,
});

test("canonical SCCP inventory contains only Taira and the four external mainnets", async () => {
  assert.deepEqual(Object.keys(SCCP_NETWORK_PROFILES), [
    "sora-taira",
    "ethereum-mainnet",
    "bsc-mainnet",
    "tron-mainnet",
    "ton-mainnet",
  ]);
  const exports = await import("../src/sccp.js");
  for (const retired of [
    "SCCP_DOMAIN_SOLANA",
    "SCCP_CODEC_SOLANA_PUBKEY32",
    "SCCP_SOLANA_TESTNET_GENESIS_HASH",
    "deriveSccpSolanaDestinationHashesV1",
    "deriveSccpSolanaNativeVerifierConfigHashV1",
    "deriveSccpSolanaSourceIdentityHashesV1",
  ]) {
    assert.equal(retired in exports, false, retired);
  }
});

test("BSC mainnet canonical transfer, lane, commitment, and public inputs match V1 bytes", () => {
  assert.equal(hex(canonicalSccpTransferPayloadBytes(TRANSFER)), GOLDEN_TRANSFER_HEX);
  assert.equal(hex(canonicalSccpPayloadBytes(PAYLOAD)), GOLDEN_PAYLOAD_HEX);
  assert.equal(sccpLaneIdHash(BSC_LANE), GOLDEN_LANE_HASH);
  assert.equal(sccpMessageId(BSC_LANE, PAYLOAD), GOLDEN_MESSAGE_ID);
  assert.equal(sccpPayloadHash(canonicalSccpPayloadBytes(PAYLOAD)), GOLDEN_PAYLOAD_HASH);

  const value = commitment();
  assert.equal(hex(canonicalSccpHubCommitmentBytes(value)), GOLDEN_COMMITMENT_HEX);
  assert.equal(sccpCommitmentLeafHash(value), GOLDEN_ROOT);
  assert.equal(sccpMerkleRootFromCommitment(value, { steps: [] }), GOLDEN_ROOT);
  assert.equal(hex(canonicalSccpMerkleProofBytes({ steps: [] })), "00000000");
  assert.equal(
    hex(canonicalSccpMessagePublicInputsBytes({
      version: 1,
      message_id: value.message_id,
      payload_hash: value.payload_hash,
      target_domain: SCCP_DOMAIN_BSC,
      commitment_root: GOLDEN_ROOT,
      finality_height: "9",
      finality_block_hash: HASH_44,
    })),
    GOLDEN_PUBLIC_INPUTS_HEX,
  );
});

test("canonical Taira bundle binds the exact payload and commitment framing", () => {
  assert.equal(normalizeSccpMessageBundle(bundle()).version, 1);
  const expected =
    `01${GOLDEN_ROOT.slice(2)}` +
    `84000000${GOLDEN_COMMITMENT_HEX}` +
    "0400000000000000" +
    `6d000000${GOLDEN_PAYLOAD_HEX}` +
    "0100000001";
  assert.equal(hex(canonicalTairaSccpMessageBundleBytes(bundle())), expected);

  for (const mutate of [
    (value) => { value.Transfer.nonce = "8"; },
    (value) => { value.Transfer.route_revision = 2; },
    (value) => { value.Transfer.amount = "12"; },
    (value) => { value.Transfer.recipient = `0x${"12".repeat(20)}`; },
  ]) {
    const payload = clone(PAYLOAD);
    mutate(payload);
    assert.throws(
      () => canonicalTairaSccpMessageBundleBytes(bundle({ payload })),
      /commitment does not match/u,
    );
  }
});

test("canonical helpers reject retired networks, codecs, aliases, and role collisions", () => {
  assert.deepEqual(
    normalizeSccpCodecValue(SCCP_CODEC_EVM_ADDRESS20, new Uint8Array(20).fill(1)),
    new Uint8Array(20).fill(1),
  );
  for (const target of [
    network("ethereum_sepolia"),
    network("bsc_testnet"),
    network("tron_nile"),
    network("tron_shasta"),
    network("solana_testnet"),
    network("ton_testnet"),
  ]) {
    assert.throws(
      () => sccpMessageId({ source: network("sora_taira"), target }, PAYLOAD),
      /unsupported|retired/u,
    );
  }
  for (const retiredCodec of [3, 4, 6]) {
    const candidate = clone(TRANSFER);
    candidate.recipient_codec = retiredCodec;
    assert.throws(() => canonicalSccpTransferPayloadBytes(candidate), /retired|protocol domain/u);
  }

  const stalePayloadOnly = `0x${Buffer.from(
    keccak_256(Buffer.concat([
      Buffer.from("sccp:lane-message-id:v1"),
      canonicalSccpPayloadBytes(PAYLOAD),
    ])),
  ).toString("hex")}`;
  assert.notEqual(stalePayloadOnly, GOLDEN_MESSAGE_ID);
  const colliding = clone(CONTEXT);
  colliding.route_configuration_hash = colliding.destination_binding_hash;
  assert.throws(() => sccpHubCommitmentFromPayload(colliding, PAYLOAD), /colliding hash role/u);
});

test("Merkle reconstruction and closed object shapes reject every malformed path", () => {
  const value = commitment();
  const right = { steps: [{ sibling_hash: HASH_AA, sibling_is_left: false }] };
  const left = { steps: [{ sibling_hash: HASH_AA, sibling_is_left: true }] };
  assert.notEqual(
    sccpMerkleRootFromCommitment(value, right),
    sccpMerkleRootFromCommitment(value, left),
  );
  assert.notEqual(
    sccpMerkleRootFromCommitment(value, right),
    sccpMerkleRootFromCommitment(value, {
      steps: [{ sibling_hash: HASH_AB, sibling_is_left: false }],
    }),
  );
  assert.throws(
    () => canonicalSccpMerkleProofBytes({
      steps: Array.from({ length: 65 }, () => ({
        sibling_hash: HASH_AA,
        sibling_is_left: false,
      })),
    }),
    /64/u,
  );
  assert.throws(
    () => canonicalSccpTransferPayloadBytes({ ...TRANSFER, routeRevision: 1 }),
    /unknown/u,
  );
  assert.throws(
    () => parseSccpJsonObject(`${JSON.stringify(PAYLOAD)} true`, "SCCP payload"),
    /trailing/u,
  );
});
