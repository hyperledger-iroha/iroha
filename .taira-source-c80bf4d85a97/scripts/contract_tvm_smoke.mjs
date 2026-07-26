#!/usr/bin/env node
/** Exercise the production TRON verifier and value-moving bridge on real TVM. */

import assert from "node:assert/strict";
import fs from "node:fs";
import { createRequire } from "node:module";

import {
  expectConfirmedTvmFailure,
  requireSuccessfulTvmReceipt,
  sendAndConfirmTvm,
  waitForTvmTransaction,
} from "./contract_tvm_receipts.mjs";

const require = createRequire(import.meta.url);
const { blake2b } = require("@noble/hashes/blake2b");
const { TronWeb, utils } = require("tronweb");

const TRON_COMPILER_IDENTITY = "tron-solc-tvm-0.7.4+commit.3f05b770";
const TRON_COMPILER_SHA256 =
  "2b55ed5fec4d9625b6c7b3ab1abd2b7fb7dd2a9c68543bf0323db2c7e2d55af2";
const TRON_MAINNET_PROFILE = 10;
const TRON_MAINNET_CHAIN_ID = 0x2b6653dcn;
const DOMAIN_TAIRA = 0;
const DOMAIN_TRON = 5;
const CODEC_TEXT = 1;
const CODEC_TRON21 = 5;
const ROUTE_REVISION = 7;
const SCALE = 1_000_000_000n;
const FEE_LIMIT = 15_000_000_000;
const METHOD_FEE_LIMIT = 1_000_000_000;
const SEMANTIC_PROOF_PROFILE_HASH =
  "0xce5a1e17aca3cafe47a403fd66479f0a36339eb56092dafa67c8d97bdeeb60ef";
const TAIRA_FINALITY_ANCHOR_HASH =
  "0x7dda271d98d9e4333093da84236157e39ce67f6f68680fedbdc17fbe8b7b6a4a";
const CANONICAL_SORA_I105 =
  "sorauﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC";
const CANONICAL_TAIRA_I105 = `test${CANONICAL_SORA_I105.slice(4)}`;
const NUMERIC_TAIRA_ALIAS = `n369${CANONICAL_TAIRA_I105.slice(4)}`;
const BASE_FIELD =
  21888242871839275222246405745257275088696311157297823662689037894645226208583n;
const SCALAR_FIELD =
  21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const G1 = [1n, 2n];
const G2 = [
  10857046999023057135944570762232829481370756359578518086990519993285655852781n,
  11559732032986387107991004021392285783925812861821192530917403151452391805634n,
  8495653923123431417604973247489272438418190587263600148770280649306958101930n,
  4082367875863433681332203403145435568316851327593401208105741076214120093531n,
];
const CONFIGURED_IC = Array.from({ length: 12 }, () => G1).flat();
const SIGNAL_LABELS = [
  "message-id",
  "payload-hash",
  "target-domain",
  "commitment-root",
  "finality-height",
  "finality-block-hash",
  "source-domain",
  "statement-hash",
  "destination-binding-hash",
  "route-configuration-hash",
  "sora-finality-anchor-hash",
].map((name) => sha3Utf8(`sccp:groth16-bn254:signal:${name}:v1`));

if (process.argv.length !== 4) {
  throw new Error("usage: contract_tvm_smoke.mjs MANIFEST NATIVE_TRANSFER_VECTORS");
}

function strip0x(value) {
  return value.startsWith("0x") ? value.slice(2) : value;
}

function bytes32(value) {
  const raw = strip0x(String(value)).toLowerCase();
  assert.match(raw, /^[0-9a-f]{1,64}$/, "value is not a bounded hexadecimal word");
  return `0x${raw.padStart(64, "0")}`;
}

function word(value) {
  const integer = BigInt(value);
  assert(integer >= 0n && integer < 1n << 256n, "uint256 value is out of range");
  return `0x${integer.toString(16).padStart(64, "0")}`;
}

function wordBuffer(value) {
  return Buffer.from(word(value).slice(2), "hex");
}

function sha3Bytes(value) {
  return utils.ethersUtils.keccak256(Buffer.from(value));
}

function sha3Utf8(value) {
  return sha3Bytes(Buffer.from(value, "utf8"));
}

function blake2b256(value) {
  return `0x${Buffer.from(blake2b(Buffer.from(value), { dkLen: 32 })).toString("hex")}`;
}

function encodedHash(types, values) {
  return sha3Bytes(Buffer.from(strip0x(utils.abi.encodeParams(types, values)), "hex"));
}

function artifact(manifest, fullyQualifiedName) {
  const result = manifest.targets.tron.contracts.find(
    (entry) => entry.fully_qualified_name === fullyQualifiedName,
  );
  assert(result, `locked TVM artifact is missing: ${fullyQualifiedName}`);
  assert(result.creation_bytecode.byte_length > 0, `${fullyQualifiedName} has no creation code`);
  assert.match(result.creation_bytecode.hex, /^0x[0-9a-f]+$/);
  assert(Array.isArray(result.abi) && result.abi.length > 0, `${fullyQualifiedName} has no ABI`);
  return result;
}

function requireFunctions(contractArtifact, names) {
  const functions = new Set(
    contractArtifact.abi
      .filter((entry) => entry.type === "function")
      .map((entry) => entry.name),
  );
  for (const name of names) assert(functions.has(name), `artifact ABI is missing ${name}`);
}

function validateManifest(manifest) {
  assert.equal(manifest.schema, "iroha.sccp.contract-artifacts.v1");
  assert.notDeepEqual(
    manifest.targets.evm.contracts,
    manifest.targets.tron.contracts,
    "EVM and TVM artifact maps must remain distinct",
  );
  assert.equal(manifest.targets.tron.compiler.identity, TRON_COMPILER_IDENTITY);
  assert.equal(manifest.targets.tron.compiler.soljson_sha256_hex, TRON_COMPILER_SHA256);
  const verifier = artifact(
    manifest,
    "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol:SccpTronGroth16Bn254MessageVerifier",
  );
  const bridge = artifact(
    manifest,
    "contracts/tron/sccp/TairaXorSccpBridge.sol:TairaXorSccpBridge",
  );
  const token = artifact(manifest, "contracts/tron/sccp/TairaXOR.sol:TairaXOR");
  requireFunctions(verifier, [
    "networkId",
    "verifierCodeHash",
    "verifyingKeyHash",
    "verifySccpMessageProof",
  ]);
  requireFunctions(bridge, [
    "destinationBindingHash",
    "destinationLaneHash",
    "finalizeFromTaira",
    "routeConfigHash",
    "sccpDestinationMessageId",
    "sccpPayloadHash",
    "sourceEventDigest",
    "sourceLaneHash",
    "transferToTaira",
  ]);
  requireFunctions(token, ["balanceOf", "burnFrom", "mint", "totalSupply"]);
  return { verifier, bridge, token };
}

function mod(value) {
  const result = value % BASE_FIELD;
  return result < 0n ? result + BASE_FIELD : result;
}

function powMod(base, exponent) {
  let result = 1n;
  let current = mod(base);
  let power = exponent;
  while (power !== 0n) {
    if ((power & 1n) !== 0n) result = mod(result * current);
    current = mod(current * current);
    power >>= 1n;
  }
  return result;
}

function inverse(value) {
  const normalized = mod(value);
  assert.notEqual(normalized, 0n, "cannot invert zero in the BN254 base field");
  return powMod(normalized, BASE_FIELD - 2n);
}

function addPoints(left, right) {
  if (left === null) return right;
  if (right === null) return left;
  const [x1, y1] = left;
  const [x2, y2] = right;
  if (x1 === x2 && mod(y1 + y2) === 0n) return null;
  const slope =
    x1 === x2
      ? mod(3n * x1 * x1 * inverse(2n * y1))
      : mod((y2 - y1) * inverse(x2 - x1));
  const x3 = mod(slope * slope - x1 - x2);
  return [x3, mod(slope * (x1 - x3) - y1)];
}

function scalarMultiply(point, scalar) {
  let result = null;
  let addend = point;
  let remaining = BigInt(scalar);
  assert(remaining >= 0n, "negative BN254 scalar is invalid");
  while (remaining !== 0n) {
    if ((remaining & 1n) !== 0n) result = addPoints(result, addend);
    addend = addPoints(addend, addend);
    remaining >>= 1n;
  }
  return result;
}

function assertPoint(point) {
  assert(point !== null, "BN254 point at infinity is not admissible here");
  assert.equal(mod(point[1] * point[1]), mod(point[0] * point[0] * point[0] + 3n));
}

function verifyingKeyHash() {
  const keyWords = [...G1, ...G2, ...G2, ...G2, ...CONFIGURED_IC];
  return sha3Bytes(Buffer.concat(keyWords.map(wordBuffer)));
}

function proofSignals(publicInputs, statementHash, destinationBindingHash, routeConfigHash) {
  const values = [
    ...publicInputs,
    word(DOMAIN_TAIRA),
    statementHash,
    destinationBindingHash,
    routeConfigHash,
    TAIRA_FINALITY_ANCHOR_HASH,
  ];
  return values.map(
    (value, index) =>
      BigInt(encodedHash(["bytes32", "bytes32"], [SIGNAL_LABELS[index], value])) %
      SCALAR_FIELD,
  );
}

function acceptingProof(publicInputs, statementHash, destinationBindingHash, routeConfigHash) {
  const scalar = proofSignals(
    publicInputs,
    statementHash,
    destinationBindingHash,
    routeConfigHash,
  ).reduce((sum, value) => (sum + value) % SCALAR_FIELD, 1n);
  const vkX = scalarMultiply(G1, scalar);
  assertPoint(vkX);
  const c = [vkX[0], mod(-vkX[1])];
  return utils.abi.encodeParams(
    ["uint256", "bytes32", "uint256", "bytes32", "uint256[2]", "uint256[4]", "uint256[2]"],
    [
      "1",
      publicInputs[0],
      String(DOMAIN_TAIRA),
      publicInputs[3],
      G1.map(String),
      G2.map(String),
      c.map(String),
    ],
  );
}

function le(value, width) {
  let remaining = BigInt(value);
  const output = Buffer.alloc(width);
  for (let index = 0; index < width; index += 1) {
    output[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  assert.equal(remaining, 0n, `${value} exceeds ${width} bytes`);
  return output;
}

function vec(value) {
  const bytes = Buffer.from(value);
  return Buffer.concat([le(bytes.length, 4), bytes]);
}

function inboundTransferPayload(recipient) {
  return Buffer.concat([
    Buffer.from([2, 1]),
    le(DOMAIN_TAIRA, 4),
    le(DOMAIN_TRON, 4),
    le(23, 8),
    le(ROUTE_REVISION, 4),
    le(DOMAIN_TAIRA, 4),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from("xor", "utf8")),
    le(3, 16),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from(CANONICAL_TAIRA_I105, "utf8")),
    Buffer.from([CODEC_TRON21]),
    vec(recipient),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from("taira_tron_xor", "utf8")),
  ]);
}

function outboundTransferPayload(sender, recipient, nonce, amount) {
  return Buffer.concat([
    Buffer.from([2, 1]),
    le(DOMAIN_TRON, 4),
    le(DOMAIN_TAIRA, 4),
    le(nonce, 8),
    le(ROUTE_REVISION, 4),
    le(DOMAIN_TAIRA, 4),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from("xor", "utf8")),
    le(amount, 16),
    Buffer.from([CODEC_TRON21]),
    vec(sender),
    Buffer.from([CODEC_TEXT]),
    vec(recipient),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from("taira_tron_xor", "utf8")),
  ]);
}

function canonicalLane(source, target) {
  return Buffer.concat([Buffer.from([1]), vec(source), vec(target)]);
}

function independentPayloadHash(payload) {
  return blake2b256(Buffer.concat([Buffer.from("sccp:payload:v1", "utf8"), payload]));
}

function independentLaneHash(lane) {
  return blake2b256(Buffer.concat([Buffer.from("sccp:lane-id:v1", "utf8"), lane]));
}

function independentMessageId(lane, payload) {
  return sha3Bytes(
    Buffer.concat([
      Buffer.from("sccp:lane-message-id:v1", "utf8"),
      Buffer.from([1]),
      vec(lane),
      vec(payload),
    ]),
  );
}

function independentSourceEventDigest(laneHash, messageId, payloadHash) {
  return sha3Bytes(
    Buffer.concat([
      Buffer.from("sccp:source:event:v1", "utf8"),
      Buffer.from([1]),
      Buffer.from(strip0x(laneHash), "hex"),
      Buffer.from(strip0x(messageId), "hex"),
      Buffer.from(strip0x(payloadHash), "hex"),
    ]),
  );
}

function validateNativeVectors(document) {
  assert.deepEqual(
    Object.keys(document).sort(),
    ["digest_preimage", "vectors", "version"],
    "native vector root has unknown or missing fields",
  );
  assert.equal(document.version, 1);
  assert.equal(
    document.digest_preimage,
    "sccp:source:event:v1 || 0x01 || lane_hash || message_id || payload_hash",
  );
  assert(Array.isArray(document.vectors));
  assert.deepEqual(
    document.vectors.map((vector) => vector.source_profile),
    [
      "ethereum-mainnet",
      "ethereum-sepolia",
      "bsc-mainnet",
      "bsc-testnet",
      "tron-mainnet",
      "tron-nile",
      "tron-shasta",
    ],
  );
  const expectedFields = [
    "canonical_lane_hex",
    "canonical_payload_hex",
    "lane_hash_hex",
    "message_id_hex",
    "payload_hash_hex",
    "source_event_digest_hex",
    "source_profile",
    "target_profile",
  ];
  for (const vector of document.vectors) {
    assert.deepEqual(Object.keys(vector).sort(), expectedFields);
    assert.equal(vector.target_profile, "sora-taira");
    for (const field of expectedFields.filter((name) => name.endsWith("_hex"))) {
      assert.match(vector[field], /^(?:[0-9a-f]{2})+$/, `${vector.source_profile} ${field}`);
    }
    const lane = Buffer.from(vector.canonical_lane_hex, "hex");
    const payload = Buffer.from(vector.canonical_payload_hex, "hex");
    assert.equal(independentLaneHash(lane), `0x${vector.lane_hash_hex}`);
    assert.equal(independentPayloadHash(payload), `0x${vector.payload_hash_hex}`);
    assert.equal(independentMessageId(lane, payload), `0x${vector.message_id_hex}`);
    assert.equal(
      independentSourceEventDigest(
        vector.lane_hash_hex,
        vector.message_id_hex,
        vector.payload_hash_hex,
      ),
      `0x${vector.source_event_digest_hex}`,
    );
  }
  return document.vectors;
}

function solidityAddressWord(client, address) {
  return bytes32(tronHexAddress(client, address).slice(2));
}

function tronAddressWord(client, address) {
  return word(BigInt(`0x${tronHexAddress(client, address)}`));
}

function exactDestinationBinding({ client, verifierAddress, bridgeAddress, verifierCodeHash, keyHash }) {
  return encodedHash(
    [
      "bytes32",
      "bytes32",
      "bytes32",
      "uint256",
      "uint256",
      "bytes32",
      "bytes32",
      "bytes32",
      "bytes32",
      "bytes32",
      "bytes32",
    ],
    [
      sha3Utf8("iroha:sccp:tron-destination-binding:v1"),
      sha3Utf8("tron-groth16-bn254-v1"),
      word(TRON_MAINNET_CHAIN_ID),
      String(DOMAIN_TAIRA),
      String(DOMAIN_TRON),
      tronAddressWord(client, verifierAddress),
      tronAddressWord(client, bridgeAddress),
      verifierCodeHash,
      keyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      TAIRA_FINALITY_ANCHOR_HASH,
    ],
  );
}

function exactRouteConfig({
  client,
  tokenAddress,
  tokenCodeHash,
  verifierAddress,
  bridgeAddress,
  verifierCodeHash,
  keyHash,
  destinationBinding,
  sourceLaneHash,
  destinationLaneHash,
}) {
  const deploymentConfigHash = encodedHash(
    ["bytes32", "bytes32", "bytes32", "bytes32", "bytes32", "bytes32", "bytes32", "bytes32"],
    [
      solidityAddressWord(client, tokenAddress),
      tokenCodeHash,
      solidityAddressWord(client, verifierAddress),
      verifierCodeHash,
      keyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      TAIRA_FINALITY_ANCHOR_HASH,
      destinationBinding,
    ],
  );
  const assetRouteConfigHash = encodedHash(
    ["bytes32", "bytes32", "uint32", "uint256"],
    [sha3Utf8("xor"), sha3Utf8("taira_tron_xor"), ROUTE_REVISION, String(SCALE)],
  );
  return encodedHash(
    ["bytes32", "uint32", "uint8", "bytes32", "bytes32", "bytes32", "bytes32", "bytes32"],
    [
      sha3Utf8("sccp:concrete-route-config:v1"),
      DOMAIN_TRON,
      TRON_MAINNET_PROFILE,
      word(TRON_MAINNET_CHAIN_ID),
      sourceLaneHash,
      destinationLaneHash,
      deploymentConfigHash,
      assetRouteConfigHash,
    ],
  );
}

function decodeSccpTransferLog(client, bridgeAddress, receipt) {
  assert(Array.isArray(receipt.log), "source transaction receipt omitted TVM logs");
  const signature = strip0x(
    sha3Utf8("SccpTransfer(bytes32,bytes32,bytes32,bytes32,bytes32,bytes)"),
  );
  const bridgeHex = tronHexAddress(client, bridgeAddress);
  const candidates = receipt.log.filter((entry) => {
    const rawAddress = strip0x(String(entry.address || "")).toLowerCase();
    const address = rawAddress.length === 40 ? `41${rawAddress}` : rawAddress;
    const topic = strip0x(String(entry.topics?.[0] || "")).toLowerCase();
    return address === bridgeHex && topic === signature;
  });
  assert.equal(candidates.length, 1, "source transaction must emit exactly one SCCP transfer log");
  const entry = candidates[0];
  assert.equal(entry.topics.length, 4, "SCCP transfer log topic count drift");
  const dataHex = strip0x(String(entry.data || "")).toLowerCase();
  assert.match(dataHex, /^(?:[0-9a-f]{2})+$/, "SCCP transfer log data is malformed");
  const data = Buffer.from(dataHex, "hex");
  assert(data.length >= 128 && data.length % 32 === 0, "SCCP transfer log ABI data is truncated");
  const offset = Number(BigInt(`0x${data.subarray(64, 96).toString("hex")}`));
  assert.equal(offset, 96, "SCCP transfer log payload offset is noncanonical");
  const payloadLength = Number(BigInt(`0x${data.subarray(offset, offset + 32).toString("hex")}`));
  assert(Number.isSafeInteger(payloadLength), "SCCP transfer payload length is unsafe");
  const paddedLength = Math.ceil(payloadLength / 32) * 32;
  assert.equal(data.length, offset + 32 + paddedLength, "SCCP transfer log has trailing ABI data");
  assert(
    data.subarray(offset + 32 + payloadLength).every((value) => value === 0),
    "SCCP transfer log has nonzero ABI padding",
  );
  return {
    laneHash: bytes32(entry.topics[1]),
    messageId: bytes32(entry.topics[2]),
    sourceEventDigest: bytes32(entry.topics[3]),
    payloadHash: `0x${data.subarray(0, 32).toString("hex")}`,
    routeConfigHash: `0x${data.subarray(32, 64).toString("hex")}`,
    payload: data.subarray(offset + 32, offset + 32 + payloadLength),
  };
}

function pureSelfTest() {
  assert.equal(
    sha3Bytes(Buffer.alloc(0)),
    "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
    "SCCP requires legacy Keccak-256, not NIST SHA3-256",
  );
  assertPoint(G1);
  assert.deepEqual(scalarMultiply(G1, 1n), G1);
  assert.deepEqual(scalarMultiply(G1, 2n), [
    1368015179489954701390400359078579693043519447331113978918064868415326638035n,
    9918110051302171585080402603319702774565515993150576347155970296011118125764n,
  ]);
  assert.equal(verifyingKeyHash().length, 66);
  assert.notEqual(verifyingKeyHash(), SEMANTIC_PROOF_PROFILE_HASH);
  assert.notEqual(verifyingKeyHash(), TAIRA_FINALITY_ANCHOR_HASH);
}

async function fetchJson(endpoint, path, options = {}) {
  const response = await fetch(`${endpoint}${path}`, {
    ...options,
    headers: { accept: "application/json", ...(options.headers || {}) },
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`TRE endpoint ${path} returned HTTP ${response.status}`);
  return response.json();
}

async function assertMainnetChainId(endpoint) {
  const response = await fetchJson(endpoint, "/jsonrpc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "eth_chainId", params: [] }),
  });
  assert.deepEqual(Object.keys(response).sort(), ["id", "jsonrpc", "result"]);
  assert.equal(response.jsonrpc, "2.0");
  assert.equal(response.id, 1);
  assert.equal(String(response.result).toLowerCase(), "0x2b6653dc");
  assert.equal(BigInt(response.result), TRON_MAINNET_CHAIN_ID);
}

function collectPrivateKeys(value, output = []) {
  if (Array.isArray(value)) {
    for (const entry of value) collectPrivateKeys(entry, output);
    return output;
  }
  if (!value || typeof value !== "object") return output;
  for (const [key, entry] of Object.entries(value)) {
    if (key === "privateKeys" && Array.isArray(entry)) {
      for (const candidate of entry) {
        if (typeof candidate === "string" && /^[0-9a-fA-F]{64}$/.test(candidate)) {
          output.push(candidate.toLowerCase());
        }
      }
    } else collectPrivateKeys(entry, output);
  }
  return output;
}

async function deploy(client, contractArtifact, parameters, label) {
  const constructorAbi = contractArtifact.abi.find((entry) => entry.type === "constructor");
  assert(constructorAbi, `${label} artifact omitted its constructor ABI`);
  assert.equal(
    parameters.length,
    constructorAbi.inputs.length,
    `${label} constructor argument count does not match the authenticated ABI`,
  );
  const options = {
    abi: contractArtifact.abi,
    bytecode: strip0x(contractArtifact.creation_bytecode.hex),
    feeLimit: FEE_LIMIT,
    callValue: 0,
    userFeePercentage: 100,
    originEnergyLimit: 10_000_000,
    // TronWeb's legacy constructor encoder loses tuple component types. Its V2
    // encoder retains nested policy fields and applies TRON address conversion.
    funcABIV2: constructorAbi,
    parametersV2: structuredClone(parameters),
  };
  const unsigned = await client.transactionBuilder.createSmartContract(
    options,
    client.defaultAddress.base58,
  );
  const signed = await client.trx.sign(unsigned);
  const broadcast = await client.trx.sendRawTransaction(signed);
  if (broadcast.code || broadcast.result !== true) throw new Error(`${label} broadcast failed`);
  const receipt = await waitForTvmTransaction(client, signed.txID);
  requireSuccessfulTvmReceipt(signed.txID, receipt, `${label} deployment`);
  const address = signed.contract_address || receipt.contract_address;
  assert(address, `${label} receipt omitted its contract address`);
  const document = await client.trx.getContract(address);
  assert.match(document.bytecode || "", /^[0-9a-fA-F]+$/, `${label} deployed no runtime code`);
  return client.contract(contractArtifact.abi, address);
}

function tronHexAddress(client, value) {
  const text = String(value);
  if (/^0x[0-9a-fA-F]{40}$/.test(text)) return `41${text.slice(2).toLowerCase()}`;
  const encoded = strip0x(client.address.toHex(text)).toLowerCase();
  assert.match(encoded, /^41[0-9a-f]{40}$/);
  return encoded;
}

async function runtimeCodeHash(client, address) {
  const document = await client.trx.getContract(address);
  const encoded = strip0x(String(document.bytecode || ""));
  assert.match(encoded, /^[0-9a-fA-F]+$/);
  assert.equal(encoded.length % 2, 0);
  return sha3Bytes(Buffer.from(encoded, "hex"));
}

function asInteger(value) {
  return BigInt(value.toString());
}

const manifest = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const nativeVectorDocument = JSON.parse(fs.readFileSync(process.argv[3], "utf8"));
const artifacts = validateManifest(manifest);
const nativeVectors = validateNativeVectors(nativeVectorDocument);
const tronMainnetVector = nativeVectors.find(
  (vector) => vector.source_profile === "tron-mainnet",
);
assert(tronMainnetVector, "Rust-generated vectors omitted TRON mainnet");
pureSelfTest();
if (process.env.SCCP_TVM_STATIC_ONLY === "1") {
  process.stdout.write(
    "SCCP TVM artifact, Rust vector, and proof-helper static verification passed.\n",
  );
  process.exit(0);
}

const endpoint = process.env.SCCP_TVM_ENDPOINT;
if (!/^http:\/\/127\.0\.0\.1:[0-9]+$/.test(endpoint || "")) {
  throw new Error("SCCP_TVM_ENDPOINT must be one loopback HTTP endpoint");
}

// The chain-id probe is deliberately first: no deployment is attempted until
// official TRE proves that it is running the exact mainnet profile-10 chain.
await assertMainnetChainId(endpoint);

// The first-release token constructor must name the exact future route, while
// the route constructor must name the already-deployed token. Unlike the EVM
// nonce flow exercised by the Hardhat smoke, TRE does not expose an audited
// deterministic address primitive that can close this dependency cycle.
// Preserve the adversarial harness below for a future audited prebinding
// provider, but never read test keys or construct a transaction until that
// provider can return both exact addresses.
const prebinding = (() => {
  throw new Error(
    "SCCP TVM live deployment is disabled: TRE does not expose a deterministic " +
      "prebinding primitive for deploying the immutable token against the exact " +
      "future route address. No contract deployment or transaction was broadcast.",
  );
})();

const accountDocument = await fetchJson(endpoint, "/admin/accounts-json");
const privateKeys = [...new Set(collectPrivateKeys(accountDocument))];
assert(privateKeys.length >= 2, "TRE must expose at least two isolated test accounts");
const bridgeClient = new TronWeb({ fullHost: endpoint, privateKey: privateKeys[0] });
const hostileClient = new TronWeb({ fullHost: endpoint, privateKey: privateKeys[1] });
assert.notEqual(bridgeClient.defaultAddress.base58, hostileClient.defaultAddress.base58);

const keyHash = verifyingKeyHash();
const networkId = word(TRON_MAINNET_CHAIN_ID);
const verifierParameters = [
  G1.map(String),
  G2.map(String),
  G2.map(String),
  G2.map(String),
  CONFIGURED_IC.map(String),
  SEMANTIC_PROOF_PROFILE_HASH,
  TAIRA_FINALITY_ANCHOR_HASH,
  keyHash,
  networkId,
  DOMAIN_TAIRA,
  DOMAIN_TRON,
];

await expectConfirmedTvmFailure(
  () =>
    deploy(
      bridgeClient,
      artifacts.verifier,
      [...verifierParameters.slice(0, 8), word(0xcd8690dc), DOMAIN_TAIRA, DOMAIN_TRON],
      "wrong-chain verifier",
    ),
  "verifier deployment for the Nile chain id",
);

const verifier = await deploy(
  bridgeClient,
  artifacts.verifier,
  verifierParameters,
  "production TRON verifier",
);
const verifierCodeHash = await runtimeCodeHash(bridgeClient, verifier.address);
assert.equal(bytes32(await verifier.verifierCodeHash().call()), verifierCodeHash);
assert.equal(bytes32(await verifier.verifyingKeyHash().call()), keyHash);
assert.equal(bytes32(await verifier.networkId().call()), networkId);
assert.equal(asInteger(await verifier.expectedSourceDomain().call()), BigInt(DOMAIN_TAIRA));
assert.equal(asInteger(await verifier.expectedTargetDomain().call()), BigInt(DOMAIN_TRON));

// Nested tuple addresses bypass TronWeb's top-level TRON-prefix conversion, so
// pass the canonical 20-byte Solidity address explicitly.
const verifierAddress = `0x${tronHexAddress(bridgeClient, verifier.address).slice(2)}`;
const policy = [
  verifierAddress,
  verifierCodeHash,
  keyHash,
  SEMANTIC_PROOF_PROFILE_HASH,
  TAIRA_FINALITY_ANCHOR_HASH,
];
await expectConfirmedTvmFailure(
  () =>
    deploy(
      bridgeClient,
      artifacts.bridge,
      [prebinding.tokenAddress, policy, 11, ROUTE_REVISION],
      "wrong-chain bridge",
    ),
  "bridge deployment for the Nile profile",
);
const wrongCodeHash = `0x${(BigInt(verifierCodeHash) ^ 1n).toString(16).padStart(64, "0")}`;
await expectConfirmedTvmFailure(
  () =>
    deploy(
      bridgeClient,
      artifacts.bridge,
      [
        prebinding.tokenAddress,
        [verifierAddress, wrongCodeHash, ...policy.slice(2)],
        TRON_MAINNET_PROFILE,
        ROUTE_REVISION,
      ],
      "wrong-code-hash bridge",
    ),
  "bridge deployment with a forged EXTCODEHASH policy",
);
await expectConfirmedTvmFailure(
  () =>
    deploy(
      bridgeClient,
      artifacts.bridge,
      [prebinding.tokenAddress, policy, TRON_MAINNET_PROFILE, 0],
      "zero-revision bridge",
    ),
  "bridge deployment with a zero route revision",
);

const bridge = await deploy(
  bridgeClient,
  artifacts.bridge,
  [prebinding.tokenAddress, policy, TRON_MAINNET_PROFILE, ROUTE_REVISION],
  "production TRON bridge",
);
assert.equal(
  tronHexAddress(bridgeClient, bridge.address),
  tronHexAddress(bridgeClient, prebinding.routeAddress),
  "route deployment address drifted from the token's immutable binding",
);
assert.equal(asInteger(await bridge.tronProfile().call()), BigInt(TRON_MAINNET_PROFILE));
assert.equal(bytes32(await bridge.networkId().call()), networkId);
assert.equal(asInteger(await bridge.routeRevision().call()), BigInt(ROUTE_REVISION));
assert.equal(
  tronHexAddress(bridgeClient, await bridge.verifier().call()),
  tronHexAddress(bridgeClient, verifier.address),
);
const tokenAddress = prebinding.tokenAddress;
assert.equal(
  tronHexAddress(bridgeClient, await bridge.token().call()),
  tronHexAddress(bridgeClient, tokenAddress),
);
const token = bridgeClient.contract(artifacts.token.abi, tokenAddress);
const hostileToken = hostileClient.contract(artifacts.token.abi, tokenAddress);
const hostileBridge = hostileClient.contract(artifacts.bridge.abi, bridge.address);
assert.equal(
  tronHexAddress(bridgeClient, await token.bridge().call()),
  tronHexAddress(bridgeClient, bridge.address),
);
const tokenRuntimeCodeHash = await runtimeCodeHash(bridgeClient, tokenAddress);
assert.equal(bytes32(await bridge.tokenCodeHash().call()), tokenRuntimeCodeHash);
assert.equal(bytes32(await bridge.verifierCodeHash().call()), verifierCodeHash);

const canonicalTronMainnet = Buffer.concat([
  Buffer.from([1, TRON_MAINNET_PROFILE]),
  le(DOMAIN_TRON, 4),
  le(TRON_MAINNET_CHAIN_ID, 4),
]);
const canonicalTaira = Buffer.from("010100000000fc56984b2be7431d840e21514d1883f0", "hex");
const expectedSourceLane = canonicalLane(canonicalTronMainnet, canonicalTaira);
assert.equal(expectedSourceLane.toString("hex"), tronMainnetVector.canonical_lane_hex);
const expectedSourceLaneHash = `0x${tronMainnetVector.lane_hash_hex}`;
const expectedDestinationLaneHash = independentLaneHash(
  canonicalLane(canonicalTaira, canonicalTronMainnet),
);
assert.equal(bytes32(await bridge.sourceLaneHash().call()), expectedSourceLaneHash);
assert.equal(bytes32(await bridge.destinationLaneHash().call()), expectedDestinationLaneHash);

for (const vector of nativeVectors) {
  const vectorPayload = `0x${vector.canonical_payload_hex}`;
  assert.equal(
    bytes32(await bridge.sccpPayloadHash(vectorPayload).call()),
    `0x${vector.payload_hash_hex}`,
    `${vector.source_profile} Rust payload hash drifted on TVM`,
  );
}
assert.equal(
  bytes32(
    await bridge
      .sourceEventDigest(
        `0x${tronMainnetVector.message_id_hex}`,
        `0x${tronMainnetVector.payload_hash_hex}`,
      )
      .call(),
  ),
  `0x${tronMainnetVector.source_event_digest_hex}`,
  "Rust-generated TRON source event digest drifted on TVM",
);
for (const length of [0, 1, 127, 128, 129, 255, 256, 257, 511]) {
  const boundaryPayload = Buffer.from(
    Array.from({ length }, (_value, index) => (index * 197 + length) & 0xff),
  );
  assert.equal(
    bytes32(await bridge.sccpPayloadHash(`0x${boundaryPayload.toString("hex")}`).call()),
    independentPayloadHash(boundaryPayload),
    `TVM BLAKE2b-256 drift at ${length} payload bytes`,
  );
}

const expectedDestinationBinding = exactDestinationBinding({
  client: bridgeClient,
  verifierAddress: verifier.address,
  bridgeAddress: bridge.address,
  verifierCodeHash,
  keyHash,
});
assert.equal(bytes32(await bridge.destinationBindingHash().call()), expectedDestinationBinding);
const expectedRouteConfigHash = exactRouteConfig({
  client: bridgeClient,
  tokenAddress,
  tokenCodeHash: tokenRuntimeCodeHash,
  verifierAddress: verifier.address,
  bridgeAddress: bridge.address,
  verifierCodeHash,
  keyHash,
  destinationBinding: expectedDestinationBinding,
  sourceLaneHash: expectedSourceLaneHash,
  destinationLaneHash: expectedDestinationLaneHash,
});
assert.equal(bytes32(await bridge.routeConfigHash().call()), expectedRouteConfigHash);

const initialSupply = asInteger(await token.totalSupply().call());
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      hostileClient,
      hostileToken.mint(hostileClient.defaultAddress.base58, String(SCALE)),
      { feeLimit: METHOD_FEE_LIMIT },
      "unauthorized direct token mint",
    ),
  "unauthorized direct token mint",
);
assert.equal(asInteger(await token.totalSupply().call()), initialSupply);

const recipient = Buffer.from(tronHexAddress(bridgeClient, bridgeClient.defaultAddress.base58), "hex");
const payload = inboundTransferPayload(recipient);
const payloadHex = `0x${payload.toString("hex")}`;
const messageId = bytes32(await bridge.sccpDestinationMessageId(payloadHex).call());
const payloadHash = bytes32(await bridge.sccpPayloadHash(payloadHex).call());
const destinationBinding = bytes32(await bridge.destinationBindingHash().call());
const routeConfigHash = bytes32(await bridge.routeConfigHash().call());
const publicInputs = [
  messageId,
  payloadHash,
  word(DOMAIN_TRON),
  sha3Utf8("tvm-mainnet-commitment-root"),
  word(300),
  sha3Utf8("tvm-mainnet-finality-block"),
];
const statementHash = sha3Utf8("exact-taira-tron-mainnet-statement");
const proof = acceptingProof(publicInputs, statementHash, destinationBinding, routeConfigHash);
const wrongStatement = sha3Utf8("adversarial-wrong-taira-tron-statement");
const wrongProof = acceptingProof(publicInputs, wrongStatement, destinationBinding, routeConfigHash);

const verifierResult = await verifier
  .verifySccpMessageProof(proof, publicInputs, statementHash, destinationBinding, routeConfigHash)
  .call();
assert.equal(bytes32(verifierResult[0]), messageId);
assert.equal(asInteger(verifierResult[1]), BigInt(DOMAIN_TAIRA));
assert.equal(bytes32(verifierResult[2]), publicInputs[3]);

const ownerAddress = bridgeClient.defaultAddress.base58;
const balanceBeforeFailedFinalize = asInteger(await token.balanceOf(ownerAddress).call());
assert.equal(await bridge.usedDestinationMessages(messageId).call(), false);
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      hostileClient,
      hostileBridge.finalizeFromTaira(wrongProof, publicInputs, statementHash, payloadHex),
      { feeLimit: METHOD_FEE_LIMIT },
      "hostile invalid BN254 proof",
    ),
  "hostile invalid BN254 proof",
);
assert.equal(asInteger(await token.balanceOf(ownerAddress).call()), balanceBeforeFailedFinalize);
assert.equal(await bridge.usedDestinationMessages(messageId).call(), false);

const wrongRevisionPayload = Buffer.from(payload);
wrongRevisionPayload.writeUInt32LE(ROUTE_REVISION + 1, 18);
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      bridgeClient,
      bridge.finalizeFromTaira(
        proof,
        publicInputs,
        statementHash,
        `0x${wrongRevisionPayload.toString("hex")}`,
      ),
      { feeLimit: METHOD_FEE_LIMIT },
      "wrong route revision rollback",
    ),
  "wrong route revision rollback",
);
assert.equal(asInteger(await token.balanceOf(ownerAddress).call()), balanceBeforeFailedFinalize);
assert.equal(await bridge.usedDestinationMessages(messageId).call(), false);

await sendAndConfirmTvm(
  bridgeClient,
  bridge.finalizeFromTaira(proof, publicInputs, statementHash, payloadHex),
  { feeLimit: METHOD_FEE_LIMIT },
  "valid destination finalize",
);
assert.equal(asInteger(await token.balanceOf(ownerAddress).call()), 3n * SCALE);
assert.equal(await bridge.usedDestinationMessages(messageId).call(), true);
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      bridgeClient,
      bridge.finalizeFromTaira(proof, publicInputs, statementHash, payloadHex),
      { feeLimit: METHOD_FEE_LIMIT },
      "destination replay",
    ),
  "destination replay",
);
assert.equal(asInteger(await token.balanceOf(ownerAddress).call()), 3n * SCALE);

const nonceBeforeFailedBurns = asInteger(await bridge.transferNonce().call());
const balanceBeforeFailedBurns = asInteger(await token.balanceOf(ownerAddress).call());
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      bridgeClient,
      bridge.transferToTaira(
        Buffer.from(NUMERIC_TAIRA_ALIAS, "utf8"),
        String(SCALE),
        String(nonceBeforeFailedBurns),
      ),
      { feeLimit: METHOD_FEE_LIMIT },
      "noncanonical Taira recipient burn",
    ),
  "noncanonical Taira recipient burn",
);
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      bridgeClient,
      bridge.transferToTaira(
        Buffer.from(CANONICAL_TAIRA_I105, "utf8"),
        "1",
        String(nonceBeforeFailedBurns),
      ),
      { feeLimit: METHOD_FEE_LIMIT },
      "unaligned Taira burn amount",
    ),
  "unaligned Taira burn amount",
);
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      bridgeClient,
      bridge.transferToTaira(
        Buffer.from(CANONICAL_TAIRA_I105, "utf8"),
        String(SCALE),
        String(nonceBeforeFailedBurns + 1n),
      ),
      { feeLimit: METHOD_FEE_LIMIT },
      "mismatched source transfer nonce",
    ),
  "mismatched source transfer nonce",
);
await expectConfirmedTvmFailure(
  () =>
    sendAndConfirmTvm(
      hostileClient,
      hostileToken.burnFrom(ownerAddress, String(SCALE)),
      { feeLimit: METHOD_FEE_LIMIT },
      "unauthorized direct token burn",
    ),
  "unauthorized direct token burn",
);
assert.equal(asInteger(await bridge.transferNonce().call()), nonceBeforeFailedBurns);
assert.equal(asInteger(await token.balanceOf(ownerAddress).call()), balanceBeforeFailedBurns);

const sourceReceipt = await sendAndConfirmTvm(
  bridgeClient,
  bridge.transferToTaira(
    Buffer.from(CANONICAL_TAIRA_I105, "utf8"),
    String(SCALE),
    String(nonceBeforeFailedBurns),
  ),
  { feeLimit: METHOD_FEE_LIMIT },
  "valid source transfer",
);
const expectedSourcePayload = outboundTransferPayload(
  recipient,
  Buffer.from(CANONICAL_TAIRA_I105, "utf8"),
  nonceBeforeFailedBurns,
  1,
);
const expectedSourcePayloadHash = independentPayloadHash(expectedSourcePayload);
const expectedSourceMessageId = independentMessageId(expectedSourceLane, expectedSourcePayload);
const expectedSourceEventDigest = independentSourceEventDigest(
  expectedSourceLaneHash,
  expectedSourceMessageId,
  expectedSourcePayloadHash,
);
const emittedSource = decodeSccpTransferLog(bridgeClient, bridge.address, sourceReceipt);
assert.equal(emittedSource.laneHash, expectedSourceLaneHash);
assert.equal(emittedSource.messageId, expectedSourceMessageId);
assert.equal(emittedSource.sourceEventDigest, expectedSourceEventDigest);
assert.equal(emittedSource.payloadHash, expectedSourcePayloadHash);
assert.equal(emittedSource.routeConfigHash, expectedRouteConfigHash);
assert.deepEqual(emittedSource.payload, expectedSourcePayload);
assert.equal(await bridge.usedSourceMessages(expectedSourceMessageId).call(), true);
assert.equal(asInteger(await bridge.transferNonce().call()), nonceBeforeFailedBurns + 1n);
assert.equal(asInteger(await token.balanceOf(ownerAddress).call()), 2n * SCALE);

process.stdout.write(
  `SCCP production TVM verifier/bridge smoke passed on chain 0x${TRON_MAINNET_CHAIN_ID.toString(16)}.\n`,
);
