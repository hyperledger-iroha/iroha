import assert from "node:assert/strict";
import test from "node:test";

import { keccak_256 } from "@noble/hashes/sha3";

import {
  SCCP_CODEC_CANONICAL_TEXT,
  SCCP_CODEC_SOLANA_PUBKEY32,
  SCCP_DOMAIN_SOLANA,
  SCCP_DOMAIN_SORA,
  SCCP_NETWORK_PROFILES,
  SCCP_SOLANA_TESTNET_GENESIS_HASH,
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
const SOLANA_LANE = Object.freeze({
  source: network("sora_taira"),
  target: network("solana_testnet"),
});
const SOLANA_KEY_HEX = `0x${"11".repeat(32)}`;
const HASH_22 = `0x${"22".repeat(32)}`;
const HASH_33 = `0x${"33".repeat(32)}`;
const HASH_44 = `0x${"44".repeat(32)}`;
const HASH_AA = `0x${"aa".repeat(32)}`;
const HASH_AB = `0x${"ab".repeat(32)}`;
const SOLANA_LANE_HASH =
  "0x890f8fdecb5770da0a8bc2119508a48cba6d0f908a4af559daa7bdba4c4b21db";
const SOLANA_BASE58_KEY = "7gyGAp71YXQRoxmFBaHxofQXAipvgHyBKPyxmdSJxyvz";

const TRANSFER = Object.freeze({
  version: 1,
  source_domain: SCCP_DOMAIN_SORA,
  dest_domain: SCCP_DOMAIN_SOLANA,
  nonce: "7",
  route_revision: 1,
  asset_home_domain: SCCP_DOMAIN_SORA,
  asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
  asset_id: "0x786f72",
  amount: "11",
  sender_codec: SCCP_CODEC_CANONICAL_TEXT,
  sender: "0x616c696365407461697261",
  recipient_codec: SCCP_CODEC_SOLANA_PUBKEY32,
  recipient: SOLANA_KEY_HEX,
  route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
  route_id: "0x74616972615f736f6c5f786f72",
});
const PAYLOAD = Object.freeze({ Transfer: TRANSFER });
const CONTEXT = Object.freeze({
  lane: SOLANA_LANE,
  destination_binding_hash: HASH_22,
  route_configuration_hash: HASH_33,
});

const GOLDEN_TRANSFER_HEX =
  "010000000003000000070000000000000001000000000000000103000000786f72" +
  "0b000000000000000000000000000000010b000000616c69636540746169726106" +
  "200000001111111111111111111111111111111111111111111111111111111111111111" +
  "010d00000074616972615f736f6c5f786f72";
const GOLDEN_PAYLOAD_HEX = `02${GOLDEN_TRANSFER_HEX}`;
const GOLDEN_MESSAGE_ID =
  "0x42e5569e97f40e74c3f122eaaf30cc96ca5c6ebd1775cb04112e240b02c89824";
const GOLDEN_PAYLOAD_HASH =
  "0x464d5263b570bd7f81630e4eea743eb812cada0d635213e767e8ba44383a3f6c";
const GOLDEN_COMMITMENT_HEX =
  "0105010d" +
  "22".repeat(32) +
  "33".repeat(32) +
  GOLDEN_MESSAGE_ID.slice(2) +
  GOLDEN_PAYLOAD_HASH.slice(2);
const GOLDEN_ROOT =
  "0x1b26e99ba6e0884f099efdc3143bc612b820c798007dd2d45ad5e712d1e9cc51";
const GOLDEN_PUBLIC_INPUTS_HEX =
  "01" +
  GOLDEN_MESSAGE_ID.slice(2) +
  GOLDEN_PAYLOAD_HASH.slice(2) +
  "03000000" +
  GOLDEN_ROOT.slice(2) +
  "0900000000000000" +
  "44".repeat(32);

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const clone = (value) => structuredClone(value);

function commitment() {
  return sccpHubCommitmentFromPayload(CONTEXT, PAYLOAD);
}

function bundle(overrides = {}) {
  return {
    version: 1,
    commitment_root: GOLDEN_ROOT,
    commitment: commitment(),
    merkle_proof: { steps: [] },
    payload: PAYLOAD,
    finality_proof: "0x01",
    ...overrides,
  };
}

test("Solana testnet has one exact first-release identity and no recycled network tags", () => {
  assert.equal(SCCP_DOMAIN_SOLANA, 3);
  assert.equal(SCCP_CODEC_SOLANA_PUBKEY32, 6);
  assert.equal(
    SCCP_SOLANA_TESTNET_GENESIS_HASH,
    "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY",
  );
  assert.deepEqual(SCCP_NETWORK_PROFILES["solana-testnet"], {
    profile: "solana-testnet",
    tag: 13,
    domain: 3,
    sora: false,
    genesisHash: SCCP_SOLANA_TESTNET_GENESIS_HASH,
  });
  const tags = Object.values(SCCP_NETWORK_PROFILES).map(({ tag }) => tag);
  assert.equal(new Set(tags).size, tags.length);
  for (const reserved of [0, 6, 7, 8, 9]) assert.equal(tags.includes(reserved), false);
  assert.equal(Object.values(SCCP_NETWORK_PROFILES).some(({ domain }) => domain === 4), false);
});

test("Solana canonical transfer, payload, lane identity, commitment, and public inputs match Rust V1 bytes", () => {
  assert.equal(hex(canonicalSccpTransferPayloadBytes(TRANSFER)), GOLDEN_TRANSFER_HEX);
  assert.equal(hex(canonicalSccpPayloadBytes(PAYLOAD)), GOLDEN_PAYLOAD_HEX);
  assert.equal(sccpMessageId(SOLANA_LANE, PAYLOAD), GOLDEN_MESSAGE_ID);
  assert.equal(sccpLaneIdHash(SOLANA_LANE), SOLANA_LANE_HASH);
  assert.equal(sccpPayloadHash(canonicalSccpPayloadBytes(PAYLOAD)), GOLDEN_PAYLOAD_HASH);

  const value = commitment();
  assert.equal(value.message_id, GOLDEN_MESSAGE_ID);
  assert.equal(value.payload_hash, GOLDEN_PAYLOAD_HASH);
  assert.equal(hex(canonicalSccpHubCommitmentBytes(value)), GOLDEN_COMMITMENT_HEX);
  assert.equal(sccpCommitmentLeafHash(value), GOLDEN_ROOT);
  assert.equal(sccpMerkleRootFromCommitment(value, { steps: [] }), GOLDEN_ROOT);
  assert.equal(hex(canonicalSccpMerkleProofBytes({ steps: [] })), "00000000");
  assert.equal(
    hex(
      canonicalSccpMessagePublicInputsBytes({
        version: 1,
        message_id: value.message_id,
        payload_hash: value.payload_hash,
        target_domain: SCCP_DOMAIN_SOLANA,
        commitment_root: GOLDEN_ROOT,
        finality_height: "9",
        finality_block_hash: HASH_44,
      }),
    ),
    GOLDEN_PUBLIC_INPUTS_HEX,
  );
});

test("canonical Taira bundle binds the exact commitment, payload, root, and length framing", () => {
  assert.equal(normalizeSccpMessageBundle(bundle()).version, 1);
  const encoded = hex(canonicalTairaSccpMessageBundleBytes(bundle()));
  const expected =
    `01${GOLDEN_ROOT.slice(2)}` +
    `84000000${GOLDEN_COMMITMENT_HEX}` +
    "0400000000000000" +
    `79000000${GOLDEN_PAYLOAD_HEX}` +
    "0100000001";
  assert.equal(encoded, expected);
});

test("message identity is lane- and revision-bound and rejects stale payload-only constructions", () => {
  const payloadBytes = canonicalSccpPayloadBytes(PAYLOAD);
  const stalePayloadOnly = `0x${Buffer.from(
    keccak_256(Buffer.concat([Buffer.from("sccp:lane-message-id:v1"), payloadBytes])),
  ).toString("hex")}`;
  assert.notEqual(stalePayloadOnly, GOLDEN_MESSAGE_ID);

  const revisionTwo = clone(PAYLOAD);
  revisionTwo.Transfer.route_revision = 2;
  assert.notEqual(sccpMessageId(SOLANA_LANE, revisionTwo), GOLDEN_MESSAGE_ID);
  assert.notEqual(
    hex(canonicalSccpTransferPayloadBytes(revisionTwo.Transfer)),
    GOLDEN_TRANSFER_HEX,
  );

  const reverseLane = {
    source: network("solana_testnet"),
    target: network("sora_taira"),
  };
  assert.throws(() => sccpMessageId(reverseLane, PAYLOAD), /domains do not match/u);
  const wrongProfile = clone(SOLANA_LANE);
  wrongProfile.target = network("bsc_testnet");
  assert.throws(() => sccpMessageId(wrongProfile, PAYLOAD), /domains do not match/u);
});

test("Solana pubkeys are raw nonzero 32-byte values and never Base58-on-wire text", () => {
  assert.deepEqual(
    normalizeSccpCodecValue(6, new Uint8Array(32).fill(1)),
    new Uint8Array(32).fill(1),
  );
  for (const invalid of [
    new Uint8Array(32),
    new Uint8Array(31).fill(1),
    new Uint8Array(33).fill(1),
    SOLANA_BASE58_KEY,
    Buffer.from(SOLANA_BASE58_KEY),
  ]) {
    assert.throws(() => normalizeSccpCodecValue(6, invalid), /binary|nonzero|32 bytes/u);
  }
  for (const recipient of [SOLANA_BASE58_KEY, "0x11", `0x${"00".repeat(32)}`]) {
    const candidate = clone(TRANSFER);
    candidate.recipient = recipient;
    assert.throws(
      () => canonicalSccpTransferPayloadBytes(candidate),
      /hex|solana_pubkey32/u,
    );
  }
  for (const retiredCodec of [3, 4]) {
    const candidate = clone(TRANSFER);
    candidate.recipient_codec = retiredCodec;
    assert.throws(() => canonicalSccpTransferPayloadBytes(candidate), /retired|protocol domain/u);
  }
});

test("canonical helpers reject reserved domains, unknown profiles, and profile-shaped aliases", () => {
  const reservedDomain = clone(TRANSFER);
  reservedDomain.dest_domain = 4;
  assert.throws(() => canonicalSccpTransferPayloadBytes(reservedDomain), /reserved/u);

  for (const target of [
    network("solana_mainnet"),
    network("solana_devnet"),
    network("solana-testnet"),
    { network: "solana_testnet", profile: "testnet" },
    { network: "solana_testnet", profile: null, tag: 13 },
  ]) {
    const lane = { source: network("sora_taira"), target };
    assert.throws(() => sccpMessageId(lane, PAYLOAD), /unsupported|canonical|profile|unknown/u);
  }
});

test("commitment construction rejects zero and colliding lane, binding, message, and payload roles", () => {
  const collidingContext = clone(CONTEXT);
  collidingContext.route_configuration_hash = collidingContext.destination_binding_hash;
  assert.throws(
    () => sccpHubCommitmentFromPayload(collidingContext, PAYLOAD),
    /colliding hash role/u,
  );
  const zeroContext = clone(CONTEXT);
  zeroContext.destination_binding_hash = `0x${"00".repeat(32)}`;
  assert.throws(() => sccpHubCommitmentFromPayload(zeroContext, PAYLOAD), /nonzero/u);
  const laneAliasingContext = clone(CONTEXT);
  laneAliasingContext.destination_binding_hash = SOLANA_LANE_HASH;
  assert.throws(
    () => sccpHubCommitmentFromPayload(laneAliasingContext, PAYLOAD),
    /colliding hash role/u,
  );
  const messageAliasingContext = clone(CONTEXT);
  messageAliasingContext.destination_binding_hash = GOLDEN_MESSAGE_ID;
  assert.throws(
    () => sccpHubCommitmentFromPayload(messageAliasingContext, PAYLOAD),
    /colliding hash role/u,
  );

  const collidingCommitment = clone(commitment());
  collidingCommitment.payload_hash = collidingCommitment.message_id;
  assert.throws(
    () => canonicalSccpHubCommitmentBytes(collidingCommitment),
    /colliding hash role/u,
  );
  const wrongDirection = clone(CONTEXT);
  wrongDirection.lane = {
    source: network("solana_testnet"),
    target: network("sora_taira"),
  };
  assert.throws(() => sccpHubCommitmentFromPayload(wrongDirection, PAYLOAD), /Taira-to-external/u);
});

test("Merkle reconstruction is direction-sensitive and bundle encoding rejects every path/root mutation", () => {
  const value = commitment();
  const right = { steps: [{ sibling_hash: HASH_AA, sibling_is_left: false }] };
  const left = { steps: [{ sibling_hash: HASH_AA, sibling_is_left: true }] };
  const changed = { steps: [{ sibling_hash: HASH_AB, sibling_is_left: false }] };
  const rightRoot = sccpMerkleRootFromCommitment(value, right);
  assert.notEqual(rightRoot, GOLDEN_ROOT);
  assert.notEqual(rightRoot, sccpMerkleRootFromCommitment(value, left));
  assert.notEqual(rightRoot, sccpMerkleRootFromCommitment(value, changed));
  assert.equal(canonicalTairaSccpMessageBundleBytes(bundle()).length > 0, true);

  for (const candidate of [
    bundle({ commitment_root: rightRoot, merkle_proof: left }),
    bundle({ commitment_root: rightRoot, merkle_proof: changed }),
    bundle({ commitment_root: HASH_AA }),
  ]) {
    assert.throws(
      () => canonicalTairaSccpMessageBundleBytes(candidate),
      /root does not match/u,
    );
  }
  assert.throws(
    () => canonicalSccpMerkleProofBytes({
      steps: [{ sibling_hash: HASH_AA, sibling_is_left: 1 }],
    }),
    /boolean/u,
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
});

test("bundle validation detects stale commitment fields after every payload mutation", () => {
  const mutations = [
    (value) => { value.Transfer.nonce = "8"; },
    (value) => { value.Transfer.route_revision = 2; },
    (value) => { value.Transfer.amount = "12"; },
    (value) => { value.Transfer.recipient = `0x${"12".repeat(32)}`; },
    (value) => { value.Transfer.route_id = "0x74616972615f736f6c5f786f73"; },
  ];
  for (const mutate of mutations) {
    const payload = clone(PAYLOAD);
    mutate(payload);
    assert.throws(
      () => canonicalTairaSccpMessageBundleBytes(bundle({ payload })),
      /commitment does not match/u,
    );
  }
});

test("canonical surface rejects unknown, aliased, missing, and noncanonical fields", () => {
  const sparseSteps = [];
  sparseSteps.length = 1;
  const accessorTransfer = clone(TRANSFER);
  Object.defineProperty(accessorTransfer, "nonce", {
    enumerable: true,
    get: () => "7",
  });
  const symbolTransfer = clone(TRANSFER);
  symbolTransfer[Symbol("hidden-alias")] = 1;
  const cases = [
    () => canonicalSccpTransferPayloadBytes(accessorTransfer),
    () => canonicalSccpTransferPayloadBytes(symbolTransfer),
    () => canonicalSccpTransferPayloadBytes({ ...TRANSFER, routeRevision: 1 }),
    () => canonicalSccpTransferPayloadBytes({ ...TRANSFER, route_revision: 0 }),
    () => canonicalSccpTransferPayloadBytes({ ...TRANSFER, nonce: "07" }),
    () => canonicalSccpTransferPayloadBytes({ ...TRANSFER, nonce: "9".repeat(100_000) }),
    () => canonicalSccpTransferPayloadBytes({ ...TRANSFER, amount: 11 }),
    () => canonicalSccpPayloadBytes({ kind: "Transfer", value: TRANSFER }),
    () => canonicalSccpPayloadBytes({ Transfer: TRANSFER, Burn: {} }),
    () => sccpMessageId({ ...SOLANA_LANE, destination_binding_hash: HASH_22 }, PAYLOAD),
    () => sccpHubCommitmentFromPayload({ ...CONTEXT, destinationBindingHash: HASH_22 }, PAYLOAD),
    () => canonicalSccpHubCommitmentBytes({ ...commitment(), target_domain: 3 }),
    () => canonicalSccpMerkleProofBytes({ steps: [], siblings: [] }),
    () => canonicalSccpMerkleProofBytes({ steps: sparseSteps }),
    () => canonicalSccpMerkleProofBytes({
      steps: [{ siblingHash: HASH_AA, sibling_is_left: false }],
    }),
    () => canonicalTairaSccpMessageBundleBytes({ ...bundle(), commitmentRoot: GOLDEN_ROOT }),
    () => canonicalSccpMessagePublicInputsBytes({
      version: 1,
      message_id: GOLDEN_MESSAGE_ID,
      payload_hash: GOLDEN_PAYLOAD_HASH,
      target_domain: 3,
      commitment_root: GOLDEN_ROOT,
      finality_height: "9",
      finality_block_hash: HASH_44,
      targetDomain: 3,
    }),
  ];
  for (const invoke of cases) {
    assert.throws(invoke, /unknown|missing|canonical|safe integer|dense|data fields/u);
  }
  assert.throws(
    () => parseSccpJsonObject(`${JSON.stringify(PAYLOAD)} true`, "SCCP payload"),
    /trailing/u,
  );
});

test("retired payload-only and Base58-era SDK exports remain absent", async () => {
  const exports = await import("../src/sccp.js");
  for (const retired of [
    "SCCP_DOMAIN_SOL",
    "SCCP_CODEC_SOLANA_BASE58",
    "canonicalSccpPayloadEnvelopeBytes",
    "sccpTransferMessageId",
    "canonicalSccpCommitmentBytes",
    "canonicalSccpMessageProofBundleBytes",
    "canonicalSccpMessageTransparentPublicInputsBytes",
  ]) {
    assert.equal(retired in exports, false, retired);
  }
});

test("package root and browser-safe SCCP subpath expose the same canonical Solana surface", async () => {
  const root = await import("../src/index.js");
  const sccp = await import("../src/sccp.js");
  for (const name of [
    "SCCP_DOMAIN_SOLANA",
    "SCCP_CODEC_SOLANA_PUBKEY32",
    "SCCP_SOLANA_TESTNET_GENESIS_HASH",
    "canonicalSccpTransferPayloadBytes",
    "canonicalSccpPayloadBytes",
    "sccpPayloadHash",
    "sccpLaneIdHash",
    "sccpMessageId",
    "sccpHubCommitmentFromPayload",
    "canonicalSccpHubCommitmentBytes",
    "sccpCommitmentLeafHash",
    "canonicalSccpMerkleProofBytes",
    "sccpMerkleRootFromCommitment",
    "canonicalTairaSccpMessageBundleBytes",
    "canonicalSccpMessagePublicInputsBytes",
    "deriveSccpSolanaDestinationHashesV1",
    "deriveSccpSolanaSourceIdentityHashesV1",
  ]) {
    assert.equal(root[name], sccp[name], name);
  }
});
