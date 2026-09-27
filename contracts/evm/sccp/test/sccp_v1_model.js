"use strict";

// Independent JavaScript model of the SCCP v1 contract-visible encodings
// (`specs/sccp.md` §2 and §3): payloads, message ids, transfer and control
// leaves, promote-odd Merkle trees, the history accumulator, roster digests,
// EIP-712 attestation digests and §3.8 signature sets. It uses only ethers'
// Keccak-256 and secp256k1 so that the EDR suite checks the Solidity contract
// against a second implementation, never against itself.
//
// Requires the locked EVM runtime dependencies installed under
// `scripts/contract_tooling/evm-runtime` (`npm ci --ignore-scripts`). The smoke
// script points `SCCP_EVM_RUNTIME_DIR` at a private, freshly audited copy of
// that directory instead (test-only convenience).

const assert = require("node:assert/strict");
const path = require("node:path");
const { createRequire } = require("node:module");

const REPO = path.resolve(__dirname, "..", "..", "..", "..");
const RUNTIME_DIR = process.env.SCCP_EVM_RUNTIME_DIR
  ? path.resolve(process.env.SCCP_EVM_RUNTIME_DIR)
  : path.join(REPO, "scripts", "contract_tooling", "evm-runtime");
const runtimeRequire = createRequire(path.join(RUNTIME_DIR, "package.json"));
let ethers;
try {
  ethers = runtimeRequire("ethers");
} catch (error) {
  throw new Error(
    `the locked EVM runtime is not installed; run \`npm ci --ignore-scripts\` in ${RUNTIME_DIR}`,
    { cause: error },
  );
}

const { AbiCoder, SigningKey, computeAddress, concat, getBytes, hexlify, keccak256, toBeHex, toUtf8Bytes, zeroPadValue } =
  ethers;
const abi = AbiCoder.defaultAbiCoder();

/** secp256k1 group order and half order (§0). */
const N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n;
const HALF_N = 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0n;
const ZERO32 = `0x${"00".repeat(32)}`;
const ZERO_ADDRESS = `0x${"00".repeat(20)}`;

/** §3.9 contract constants. */
const PREVIOUS_ROSTER_GRACE_MS = 86_400_000n;
const MAX_ROSTER_VALIDITY_MS = 2_592_000_000n;
const MAX_CLOCK_SKEW_MS = 3_600_000n;

const TAIRA_TAG = 0x40;

/** §2 destination profiles served by `SccpTairaXor.sol`. */
const PROFILES = Object.freeze({
  ethereum: Object.freeze({
    name: "ethereum",
    tag: 0x41,
    chainId: 1n,
    domain: 1,
    routeId: "taira_eth_xor",
    accountCodec: 2,
    directCaller: false,
  }),
  bsc: Object.freeze({
    name: "bsc",
    tag: 0x42,
    chainId: 56n,
    domain: 2,
    routeId: "taira_bsc_xor",
    accountCodec: 2,
    directCaller: false,
  }),
  tron: Object.freeze({
    name: "tron",
    tag: 0x43,
    chainId: 0x2b6653dcn,
    domain: 5,
    routeId: "taira_tron_xor",
    accountCodec: 5,
    directCaller: true,
  }),
});

/** Big-endian unsigned integer of exactly `width` bytes. */
function be(value, width) {
  const number = BigInt(value);
  assert(number >= 0n && number < 1n << BigInt(8 * width), `value does not fit in ${width} bytes`);
  return width === 0 ? "0x" : toBeHex(number, width);
}

/** ASCII bytes of a literal tag. */
function ascii(text) {
  return hexlify(toUtf8Bytes(text));
}

/** EIP-712 `word(x)` of an unsigned integer. */
function word(value) {
  return be(value, 32);
}

/** Left-zero-padded 32-byte word of a 20-byte address. */
function addressWord(address) {
  return zeroPadValue(address, 32);
}

function byteLength(hex) {
  return getBytes(hex).length;
}

/** §2.1 identity word of a destination profile. */
function identityWord(profile) {
  return word(profile.chainId);
}

/** §2.1 `network_bytes` of Taira and of a destination profile. */
function tairaNetworkBytes(networkId) {
  return concat([be(TAIRA_TAG, 1), networkId]);
}

function profileNetworkBytes(profile) {
  return concat([be(profile.tag, 1), identityWord(profile)]);
}

/** §3.1 own-account encoding of a 20-byte address: EVM `evm_address20`, TRON `tron_address21`. */
function accountBytes(profile, address) {
  return profile.accountCodec === 5 ? concat(["0x41", address]) : hexlify(getBytes(address));
}

/**
 * §3.2 payload encoder. Every field may be overridden (including explicit
 * length prefixes) so that negative tests can build noncanonical payloads.
 */
function encodePayload(fields) {
  const f = {
    kind: 2,
    version: 1,
    assetHomeDomain: 0,
    assetIdCodec: 1,
    assetId: ascii("xor"),
    routeIdCodec: 1,
    trailing: "0x",
    ...fields,
  };
  const parts = [
    be(f.kind, 1),
    be(f.version, 1),
    be(f.sourceDomain, 4),
    be(f.destDomain, 4),
    be(f.nonce, 8),
    be(f.routeRevision, 4),
    be(f.deadlineMs, 8),
    be(f.assetHomeDomain, 4),
    be(f.assetIdCodec, 1),
    be(f.assetIdLen ?? byteLength(f.assetId), 2),
    f.assetId,
    be(f.amount, 16),
    be(f.senderCodec, 1),
    be(f.senderLen ?? byteLength(f.sender), 2),
    f.sender,
    be(f.recipientCodec, 1),
    be(f.recipientLen ?? byteLength(f.recipient), 2),
    f.recipient,
    be(f.routeIdCodec, 1),
    be(f.routeIdLen ?? byteLength(f.routeId), 2),
    f.routeId,
    f.trailing,
  ];
  return concat(parts);
}

/** Canonical Taira -> destination payload fields (codec 3 sender, own-codec recipient). */
function outboundFields(profile, { nonce, revision, deadlineMs, amount, sender, recipient }) {
  return {
    sourceDomain: 0,
    destDomain: profile.domain,
    nonce,
    routeRevision: revision,
    deadlineMs,
    amount,
    senderCodec: 3,
    sender,
    recipientCodec: profile.accountCodec,
    recipient: accountBytes(profile, recipient),
    routeId: ascii(profile.routeId),
  };
}

/** Canonical destination -> Taira payload built by `transferToTaira` (§5.1.7). */
function inboundPayload(profile, { nonce, revision, amount, sender, tairaRecipient }) {
  return encodePayload({
    sourceDomain: profile.domain,
    destDomain: 0,
    nonce,
    routeRevision: revision,
    deadlineMs: 0,
    amount,
    senderCodec: profile.accountCodec,
    sender: accountBytes(profile, sender),
    recipientCodec: 3,
    recipient: tairaRecipient,
    routeId: ascii(profile.routeId),
  });
}

/** §3.3 payload hash and message id. */
function payloadHash(payload) {
  return keccak256(concat([ascii("SCCP/PAYLOAD/V1"), payload]));
}

function messageId(laneBytes, payload) {
  assert.equal(byteLength(laneBytes), 66, "lane bytes are 66 bytes");
  return keccak256(concat([ascii("SCCP/MESSAGE/V1"), laneBytes, payloadHash(payload)]));
}

function outboundMessageId(networkId, profile, payload) {
  return messageId(concat([tairaNetworkBytes(networkId), profileNetworkBytes(profile)]), payload);
}

function inboundMessageId(networkId, profile, payload) {
  return messageId(concat([profileNetworkBytes(profile), tairaNetworkBytes(networkId)]), payload);
}

/** §3.4 transfer leaf, control leaf and internal node. */
function transferLeaf(id, destinationWord) {
  return keccak256(concat([ascii("SCCP/LEAF/V1"), id, destinationWord]));
}

function controlLeaf({ networkId, targetTag, targetIdentity, destinationWord, revision, nonce, paused }) {
  const preimage = concat([
    ascii("SCCP/CONTROL/V1"),
    be(TAIRA_TAG, 1),
    networkId,
    be(targetTag, 1),
    targetIdentity,
    destinationWord,
    be(revision, 4),
    be(nonce, 8),
    be(paused ? 1 : 0, 1),
  ]);
  assert.equal(byteLength(preimage), 126, "control leaf preimage is 126 bytes");
  return keccak256(preimage);
}

function node(left, right) {
  return keccak256(concat([ascii("SCCP/NODE/V1"), left, right]));
}

/** Promote-odd levels of a leaf list (§3.4); `levels[0]` is the leaf list. */
function merkleLevels(leaves) {
  assert(leaves.length >= 1, "a tree has at least one leaf");
  const levels = [leaves.slice()];
  while (levels[levels.length - 1].length > 1) {
    const current = levels[levels.length - 1];
    const next = [];
    for (let i = 0; i < current.length; i += 2) {
      next.push(i + 1 < current.length ? node(current[i], current[i + 1]) : current[i]);
    }
    levels.push(next);
  }
  return levels;
}

function merkleRoot(leaves) {
  const levels = merkleLevels(leaves);
  return levels[levels.length - 1][0];
}

/** Sibling path of leaf `index`; promoted levels contribute no sibling. */
function merklePath(leaves, index) {
  const levels = merkleLevels(leaves);
  const path = [];
  let position = index;
  for (let level = 0; level < levels.length - 1; level += 1) {
    const current = levels[level];
    if (position % 2 === 1) path.push(current[position - 1]);
    else if (position + 1 < current.length) path.push(current[position + 1]);
    position >>= 1;
  }
  return path;
}

/** The spec's positional `merkle_root(leaf, index, count, siblings)` (§3.4). */
function merkleRootFromPath(leaf, index, count, siblings) {
  let h = leaf;
  let k = 0;
  let i = BigInt(index);
  let c = BigInt(count);
  assert(c >= 1n && i < c, "index out of range");
  while (c > 1n) {
    if (i % 2n === 1n) {
      h = node(siblings[k], h);
      k += 1;
    } else if (i + 1n < c) {
      h = node(h, siblings[k]);
      k += 1;
    }
    i >>= 1n;
    c = (c + 1n) >> 1n;
  }
  assert.equal(k, siblings.length, "path length mismatch");
  return h;
}

/** §3.5 history leaf. */
function historyLeaf(height, sccpRoot, messageCount) {
  return keccak256(concat([ascii("SCCP/HISTORY/V1"), be(height, 8), sccpRoot, be(messageCount, 4)]));
}

/** §3.7 threshold and roster digest. */
function thresholdOf(n) {
  return Math.floor((2 * n) / 3) + 1;
}

function rosterDigest(networkId, roster) {
  const members = roster.members;
  const n = byteLength(members) / 20;
  return keccak256(
    concat([
      ascii("SCCP/ROSTER/V1"),
      networkId,
      be(roster.generation, 8),
      be(roster.validFromMs, 8),
      be(roster.validUntilMs, 8),
      be(roster.n ?? n, 1),
      be(roster.threshold, 1),
      members,
    ]),
  );
}

/** Deterministic, publicly known test keys; never use them for value. */
function testKey(label, index) {
  const scalar = (BigInt(keccak256(toUtf8Bytes(`sccp-v1-edr-test-key/${label}/${index}`))) % (N - 1n)) + 1n;
  return new SigningKey(be(scalar, 32));
}

/**
 * Builds a §3.7-ordered roster from fresh keys: `zeroSlots` keyless members
 * first, then the key addresses strictly ascending. `signers[i]` is the key of
 * member `i` (null for a zero slot).
 */
function makeRoster(label, { n, zeroSlots = 0, generation, validFromMs, validUntilMs }) {
  const keys = [];
  for (let i = 0; i < n - zeroSlots; i += 1) keys.push(testKey(label, i));
  keys.sort((a, b) => {
    const left = BigInt(computeAddress(a.publicKey));
    const right = BigInt(computeAddress(b.publicKey));
    return left < right ? -1 : left > right ? 1 : 0;
  });
  const signers = [...Array(zeroSlots).fill(null), ...keys];
  const members = concat(signers.map((key) => (key === null ? ZERO_ADDRESS : computeAddress(key.publicKey))));
  return {
    struct: {
      generation: BigInt(generation),
      validFromMs: BigInt(validFromMs),
      validUntilMs: BigInt(validUntilMs),
      threshold: thresholdOf(n),
      members,
    },
    signers,
    n,
  };
}

/** Roster struct for ABI encoding (strips model-only fields). */
function rosterArg(roster) {
  const { generation, validFromMs, validUntilMs, threshold, members } = roster.struct ?? roster;
  return { generation, validFromMs, validUntilMs, threshold, members };
}

/** §3.6 EIP-712 constants and digests. */
const DOMAIN_TYPE = "EIP712Domain(string name,string version,bytes32 salt)";
const ATTESTATION_TYPE =
  "SccpAttestation(uint64 height,uint64 epoch,uint64 timestampMs,bytes32 blockHash,bytes32 sccpRoot," +
  "uint32 messageCount,bytes32 historyRoot,uint64 historySize,bytes32 rosterDigest,bytes32 nextRosterDigest)";
const KEY_POP_TYPE = "SccpBridgeKey(bytes32 peerKeyHash,address bridgeAddress,uint64 activationEpoch)";

function domainSeparator(networkId) {
  return keccak256(
    abi.encode(
      ["bytes32", "bytes32", "bytes32", "bytes32"],
      [keccak256(toUtf8Bytes(DOMAIN_TYPE)), keccak256(toUtf8Bytes("SCCP")), keccak256(toUtf8Bytes("1")), networkId],
    ),
  );
}

const ATTESTATION_FIELDS = [
  ["uint64", "height"],
  ["uint64", "epoch"],
  ["uint64", "timestampMs"],
  ["bytes32", "blockHash"],
  ["bytes32", "sccpRoot"],
  ["uint32", "messageCount"],
  ["bytes32", "historyRoot"],
  ["uint64", "historySize"],
  ["bytes32", "rosterDigest"],
  ["bytes32", "nextRosterDigest"],
];

function attestationDigest(networkId, attestation) {
  const structHash = keccak256(
    abi.encode(
      ["bytes32", ...ATTESTATION_FIELDS.map(([type]) => type)],
      [keccak256(toUtf8Bytes(ATTESTATION_TYPE)), ...ATTESTATION_FIELDS.map(([, name]) => attestation[name])],
    ),
  );
  return keccak256(concat(["0x1901", domainSeparator(networkId), structHash]));
}

/** 65-byte `r ‖ s ‖ v` of one low-S RFC 6979 signature (§3.8). */
function signDigest(key, digest) {
  const signature = key.sign(digest);
  assert(BigInt(signature.s) <= HALF_N, "ethers produces low-S signatures");
  return { r: signature.r, s: signature.s, v: signature.v };
}

function signatureBytes({ r, s, v }) {
  return concat([be(BigInt(r), 32), be(BigInt(s), 32), be(v, 1)]);
}

/**
 * §3.8 signature set over `digest` by the roster members at `indices`
 * (ascending). `tamper(index, signature)` may replace one member's signature.
 */
function signatureSet(roster, digest, indices, tamper = null) {
  const sorted = [...indices].sort((a, b) => a - b);
  let bitmap = 0n;
  const parts = [];
  for (const index of sorted) {
    bitmap |= 1n << BigInt(index);
    const key = roster.signers[index];
    let signature = key === null ? { r: 1n, s: 1n, v: 27 } : signDigest(key, digest);
    if (tamper) signature = tamper(index, signature) ?? signature;
    parts.push(signatureBytes(signature));
  }
  return { signerBitmap: bitmap, signatures: parts.length ? concat(parts) : "0x" };
}

/** Indices of the first `count` keyed members. */
function signerIndices(roster, count = roster.struct.threshold) {
  const indices = [];
  roster.signers.forEach((key, index) => {
    if (key !== null && indices.length < count) indices.push(index);
  });
  assert.equal(indices.length, count, "roster has too few keyed members");
  return indices;
}

/**
 * Minimal Taira simulator: numbered blocks, their commitment trees and the
 * append-only history accumulator (§3.4, §3.5).
 */
class TairaModel {
  constructor(networkId) {
    this.networkId = networkId;
    this.height = 100n;
    this.history = [];
    this.blocks = new Map();
  }

  /** Commits one block with the given leaves (possibly none) at `timestampMs`. */
  commit(leaves, timestampMs) {
    this.height += 1n;
    const height = this.height;
    const messageCount = leaves.length;
    const sccpRoot = messageCount ? merkleRoot(leaves) : ZERO32;
    if (messageCount) this.history.push(historyLeaf(height, sccpRoot, messageCount));
    const historySize = this.history.length;
    const historyRoot = historySize ? merkleRoot(this.history) : ZERO32;
    const block = {
      height,
      timestampMs: BigInt(timestampMs),
      leaves: leaves.slice(),
      sccpRoot,
      messageCount,
      historySize,
      historyRoot,
      historyIndex: messageCount ? historySize - 1 : null,
    };
    this.blocks.set(height, block);
    return block;
  }

  /** Attestation statement over `block` signed by roster `digest` (and `nextDigest` at rotations). */
  statement(block, digest, nextDigest = ZERO32) {
    return {
      height: block.height,
      epoch: block.height / 3600n,
      timestampMs: block.timestampMs,
      blockHash: keccak256(concat([ascii("block"), be(block.height, 8)])),
      sccpRoot: block.sccpRoot,
      messageCount: block.messageCount,
      historyRoot: block.historyRoot,
      historySize: BigInt(block.historySize),
      rosterDigest: digest,
      nextRosterDigest: nextDigest,
    };
  }

  /** `HistoryProofV1` for an older SCCP-bearing block against a history of `historySize` leaves. */
  historyProof(block, historySize) {
    const leaves = this.history.slice(0, Number(historySize));
    return {
      height: block.height,
      sccpRoot: block.sccpRoot,
      messageCount: block.messageCount,
      leafIndex: BigInt(block.historyIndex),
      path: merklePath(leaves, block.historyIndex),
    };
  }
}

/** Function selector and event topic from a canonical signature string. */
function selector(signature) {
  return keccak256(toUtf8Bytes(signature)).slice(0, 10);
}

function topic(signature) {
  return keccak256(toUtf8Bytes(signature));
}

module.exports = {
  ATTESTATION_TYPE,
  DOMAIN_TYPE,
  HALF_N,
  KEY_POP_TYPE,
  MAX_CLOCK_SKEW_MS,
  MAX_ROSTER_VALIDITY_MS,
  N,
  PREVIOUS_ROSTER_GRACE_MS,
  PROFILES,
  REPO,
  RUNTIME_DIR,
  TAIRA_TAG,
  TairaModel,
  ZERO32,
  ZERO_ADDRESS,
  accountBytes,
  addressWord,
  ascii,
  attestationDigest,
  be,
  controlLeaf,
  domainSeparator,
  encodePayload,
  ethers,
  historyLeaf,
  identityWord,
  inboundMessageId,
  inboundPayload,
  makeRoster,
  merklePath,
  merkleRoot,
  merkleRootFromPath,
  messageId,
  node,
  outboundFields,
  outboundMessageId,
  payloadHash,
  profileNetworkBytes,
  rosterArg,
  rosterDigest,
  runtimeRequire,
  selector,
  signDigest,
  signatureBytes,
  signatureSet,
  signerIndices,
  tairaNetworkBytes,
  testKey,
  thresholdOf,
  topic,
  transferLeaf,
  word,
};
